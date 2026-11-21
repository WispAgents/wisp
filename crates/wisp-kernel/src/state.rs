//! The kernel's shared state. Plugins reach it only through `Ctx` (which holds a weak reference, so a
//! plugin's handles never keep a stopped kernel alive) and the edge through `Kernel`.
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::reg::Registry;
use crate::{Clock, Result, WireError};

pub(crate) type Erased = Arc<dyn Any + Send + Sync>;

/// Work the kernel started and owes: queued event deliveries and follow-up root calls. `idle` waits
/// for it to reach zero (tests, graceful shutdown).
#[derive(Debug, Default)]
pub(crate) struct Work {
    n: AtomicUsize,
    zero: Notify,
}

impl Work {
    pub(crate) fn add(&self, k: usize) {
        self.n.fetch_add(k, Ordering::AcqRel);
    }
    pub(crate) fn done(&self, k: usize) {
        if k > 0 && self.n.fetch_sub(k, Ordering::AcqRel) == k {
            self.zero.notify_waiters();
        }
    }
    pub(crate) async fn idle(&self) {
        loop {
            let wait = self.zero.notified();
            tokio::pin!(wait);
            wait.as_mut().enable();
            if self.n.load(Ordering::Acquire) == 0 {
                return;
            }
            wait.await;
        }
    }
}

/// Exec children not reaped yet, per plugin (ADR-0046): uninstalling a plugin waits for its process to be gone, so
/// its folder can be replaced (on Windows a running program's folder can't be moved or emptied).
#[derive(Debug, Default)]
pub(crate) struct Children {
    n: Mutex<HashMap<String, usize>>,
    gone: Notify,
}

/// One live child of a plugin; dropped once the child is reaped.
pub(crate) struct Child(Arc<Children>, String);

impl Drop for Child {
    fn drop(&mut self) {
        if let Ok(mut n) = self.0.n.lock()
            && let Some(c) = n.get_mut(&self.1)
        {
            *c = c.saturating_sub(1);
            if *c == 0 {
                n.remove(&self.1);
            }
        }
        self.0.gone.notify_waiters();
    }
}

impl Children {
    pub(crate) fn live(self: &Arc<Self>, id: &str) -> Child {
        if let Ok(mut n) = self.n.lock() {
            *n.entry(id.to_owned()).or_default() += 1;
        }
        Child(self.clone(), id.to_owned())
    }
    /// Until every child of `id` is reaped.
    pub(crate) async fn gone(&self, id: &str) {
        loop {
            let wait = self.gone.notified();
            tokio::pin!(wait);
            wait.as_mut().enable();
            if self.n.lock().map_or(true, |n| !n.contains_key(id)) {
                return;
            }
            wait.await;
        }
    }
}

/// Test-only knobs (plan §6.11.2 rule 8): a random order for interceptors of equal priority, and
/// injected slowness per (plugin, hook).
#[derive(Debug, Default, Clone)]
pub(crate) struct Chaos {
    pub(crate) seed: Option<u64>,
    pub(crate) delays: Vec<(String, String, u64)>,
}

pub(crate) struct KernelState {
    pub(crate) services: RwLock<HashMap<TypeId, Erased>>,
    pub(crate) root: CancellationToken,
    pub(crate) caps: crate::caps::Registry,
    pub(crate) reg: RwLock<Registry>,
    pub(crate) plugins: Mutex<crate::life::Plugins>,
    pub(crate) bus: crate::events::Bus,
    pub(crate) diag: crate::diag::Diagnostics,
    pub(crate) settings: crate::settings::Settings,
    pub(crate) work: Arc<Work>,
    pub(crate) chaos: RwLock<Chaos>,
    pub(crate) children: Arc<Children>,
}

impl Default for KernelState {
    fn default() -> Self {
        let work = Arc::new(Work::default());
        Self {
            services: RwLock::default(),
            root: CancellationToken::new(),
            caps: crate::caps::Registry::default(),
            reg: RwLock::default(),
            plugins: Mutex::default(),
            bus: crate::events::Bus::new(work.clone()),
            diag: crate::diag::Diagnostics::new(50),
            settings: crate::settings::Settings::default(),
            work,
            chaos: RwLock::default(),
            children: Arc::default(),
        }
    }
}

impl Drop for KernelState {
    fn drop(&mut self) {
        // the last handle is gone: every plugin task, timer and child goes with it
        self.root.cancel();
    }
}

impl KernelState {
    pub(crate) fn service<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let map = self.services.read().ok()?;
        map.get(&TypeId::of::<T>()).and_then(|v| v.clone().downcast::<T>().ok())
    }

    /// The durable event log, when the app provides one (ADR-0044).
    pub(crate) fn event_log(&self) -> Option<crate::durable::EventLogService> {
        self.service::<crate::durable::EventLogService>().map(|l| (*l).clone())
    }

    pub(crate) fn clock(&self) -> Option<Arc<dyn Clock>> {
        self.service::<Arc<dyn Clock>>().map(|c| (*c).clone())
    }

    /// Milliseconds since the epoch, from the injected clock when there is one.
    pub(crate) fn now_ms(&self) -> i64 {
        if let Some(c) = self.clock() {
            return c.now_ms();
        }
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
    }

    /// Sleeps on the injected clock (a fake one in tests), else on tokio's.
    pub(crate) fn sleep(&self, ms: u64) -> crate::BoxFuture<'static, ()> {
        match self.clock() {
            Some(c) => c.sleep(ms),
            None => Box::pin(tokio::time::sleep(std::time::Duration::from_millis(ms))),
        }
    }

    pub(crate) fn read_reg(&self) -> Result<std::sync::RwLockReadGuard<'_, Registry>> {
        self.reg.read().map_err(|_| WireError::internal("registry lock poisoned"))
    }

    pub(crate) fn write_reg(&self) -> Result<std::sync::RwLockWriteGuard<'_, Registry>> {
        self.reg.write().map_err(|_| WireError::internal("registry lock poisoned"))
    }

    pub(crate) fn lock_plugins(&self) -> Result<std::sync::MutexGuard<'_, crate::life::Plugins>> {
        self.plugins.lock().map_err(|_| WireError::internal("plugin table lock poisoned"))
    }
}
