//! Server errors as `PostHog`'s `$exception` (v1 server/index.js `captureError` on a 5xx, services/telemetry.js
//! `crashed` on `uncaughtExceptionMonitor`), through the same queue, opt-out and scrubbing as every event:
//!
//! - a root command that failed with `internal`, or a route that answered 5xx: `kernel.failures@1` (ADR-0062), `route`
//!   naming the command or `METHOD /pattern` (never the request's own values);
//! - a panic anywhere in the process: a hook (installed once, chained to the one before) captures it unhandled and
//!   keeps the queue in the store, since a release build aborts right after; the next start sends it.
//!
//! A kernel failure that only says a handler panicked is left to the hook, which has the message and the stack.

use std::sync::{Arc, Mutex, Once, Weak};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_kernel::{FailureRecord, FailureWatch};
use wisp_telemetry_core::{ErrorInfo, Props};

use crate::{Slot, State, lock};

/// What `kernel.failures` reports as `$exception`: `route` (the command, or `METHOD /pattern`) and the error's text.
/// None for a panic (the hook reports those).
pub fn failure_exception(f: &FailureRecord) -> Option<(ErrorInfo, Props)> {
    let (route, message) = f.message.split_once(": ").unwrap_or((f.message.as_str(), ""));
    if f.message.ends_with(" panicked") {
        return None;
    }
    let mut props = Props::new();
    props.insert("route".into(), json!(route));
    let err = ErrorInfo { name: Some("InternalError".into()), message: message.to_owned(), stack: None };
    Some((err, props))
}

/// The `kernel.failures` watcher: queues the failure (a lock and a push, nothing awaited).
pub struct Failures(pub Slot);

impl FailureWatch for Failures {
    fn failed(&self, f: &FailureRecord) {
        let (Some(st), Some((err, props))) = (self.0.get(), failure_exception(f)) else { return };
        st.capture(&err, props);
    }
}

static LIVE: Mutex<Vec<Weak<State>>> = Mutex::new(Vec::new());
static HOOK: Once = Once::new();
/// How long a crash waits for the queue to be kept before the process goes on to abort.
const KEEP_MS: u64 = 2_000;

/// Reports this process's panics to `st` too, from now until its plugin is switched off (its scope cancelled) or `st`
/// is dropped.
pub fn watch_panics(st: &Arc<State>) {
    {
        let mut live = lock(&LIVE);
        live.retain(|w| w.strong_count() > 0);
        live.push(Arc::downgrade(st));
    }
    HOOK.call_once(|| {
        let before = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            crashed(info);
            before(info);
        }));
    });
}

/// A panic as an error: its message, and the stack where it happened (`file:line:col` first).
pub fn panic_error(info: &std::panic::PanicHookInfo<'_>, backtrace: &str) -> ErrorInfo {
    let p = info.payload();
    let message = p.downcast_ref::<&str>().map(|s| (*s).to_owned()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
    let at = info.location().map(|l| format!("   at {}:{}:{}\n", l.file(), l.line(), l.column())).unwrap_or_default();
    ErrorInfo { name: Some("Panic".into()), message, stack: Some(format!("{at}{backtrace}")) }
}

fn crashed(info: &std::panic::PanicHookInfo<'_>) {
    // a panic while the list is held (or a state's queue, below) is skipped: the hook must never deadlock
    let live: Vec<Arc<State>> = match LIVE.try_lock() {
        Ok(l) => l.iter().filter_map(Weak::upgrade).filter(|st| !st.ctx.scope().is_cancelled()).collect(),
        Err(_) => return,
    };
    if live.is_empty() {
        return;
    }
    let err = panic_error(info, &std::backtrace::Backtrace::force_capture().to_string());
    for st in live {
        // the wall clock, not the kernel's: its services lock may be what the panicking thread holds
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0));
        let kept = match st.t.try_lock() {
            Ok(mut t) => t.crashed(&err, now, wisp_kernel::ids::uuid7),
            Err(_) => continue,
        };
        if kept.is_empty() {
            continue;
        }
        // kept in the store on a thread of its own, waited for at most KEEP_MS: a stuck writer can't hold the crash
        let (tx, rx) = std::sync::mpsc::channel();
        let st2 = st.clone();
        let spawned = std::thread::Builder::new().name("wisp-telemetry-crash".into()).spawn(move || {
            let _ = tx.send(st2.keep_pending(kept));
        });
        if spawned.is_ok() {
            let _ = rx.recv_timeout(Duration::from_millis(KEEP_MS));
        }
    }
}

impl State {
    /// A crash's queue, kept for the next start: the store's blocking write (the panic hook can't await; it runs this
    /// on a thread of its own and waits a bounded time). → whether it was written.
    pub(crate) fn keep_pending(&self, kept: Vec<Value>) -> bool {
        {
            // after the queue kept by an earlier panic this run (a build that unwinds goes on), the last PENDING_MAX
            let mut p = lock(&self.pending);
            p.extend(kept);
            let cut = p.len().saturating_sub(wisp_telemetry_core::env::PENDING_MAX);
            p.drain(..cut);
        }
        let (body, _) = self.saved();
        self.store.write_blocking(move |w| w.upsert(crate::META.kind, body).map(|_| ())).is_ok()
    }

    /// An error as `$exception` (v1 `captureError`): queued like any event, off when stats are off.
    pub(crate) fn capture(self: &Arc<Self>, err: &ErrorInfo, props: Props) {
        let (tracked, new_id, last) = {
            let mut t = lock(&self.t);
            let had = t.install_id.is_some();
            let r = t.capture_error(err, props, crate::now(&self.ctx), wisp_kernel::ids::uuid7);
            (r, !had && t.install_id.is_some(), t.queue.last().cloned())
        };
        if tracked == wisp_telemetry_core::Tracked::Off {
            return;
        }
        if new_id {
            let me = self.clone();
            self.ctx.spawn(async move { me.save().await });
        }
        self.queued(last, tracked);
    }
}

/// What a crash left (`pending` in the saved body), as the queue to restore.
pub fn pending_of(body: &Value) -> Vec<Value> {
    body["pending"].as_array().cloned().unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn rec(kind: &str, message: &str) -> FailureRecord {
        FailureRecord {
            at: 0,
            plugin: "wisp.x".into(),
            kind: kind.into(),
            code: "internal".into(),
            message: message.into(),
            stack: Vec::new(),
        }
    }

    /// What `kernel.failures` reports: the command or the route pattern as `route`, the text apart; a panic is the hook's.
    #[test]
    fn a_failure_names_its_command_or_route_and_a_panic_is_left_to_the_hook() {
        let (e, p) = failure_exception(&rec("command", "runtime.send: database is locked")).unwrap();
        assert_eq!((p["route"].as_str(), e.message.as_str()), (Some("runtime.send"), "database is locked"));
        let (_, p) = failure_exception(&rec("route", "GET /sessions/:id: boom")).unwrap();
        assert_eq!(p["route"], "GET /sessions/:id");
        assert!(failure_exception(&rec("command", "runtime.send: runtime.send panicked")).is_none());
    }
}
