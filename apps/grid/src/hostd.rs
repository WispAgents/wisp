//! hostd (v1 grid/hostd.js): one `wisp serve` per user, in box mode (`WISP_BOX=1`: the `box` set, `BoxPlugins`)
//! and trusted-proxy mode (a random secret per start that only the edge sends). Two drivers:
//!
//! - **process** (`boxd: None`): a process on this machine with its own `WISP_HOME` and `HOME`. For development
//!   and one trusted team: the users share an OS user, so it is not isolation between them.
//! - **container** (`boxd: Some(socket)`): boxd (`apps/grid/boxd`, root) runs it as a container (gVisor, no
//!   capabilities, its own network, its home in boxd's folder); the grid asks over boxd's socket.
//!
//! - **Use:** `ensure` (unless `wake`) and a `begin(…, true)` guard count as use; a passive request never
//!   gets here (the edge answers it asleep), so watching a box never wakes or keeps it up.
//! - **Idle:** every `checkMs`, a running box with no request open, nothing going on (its `/api/status`:
//!   no turn, side question, terminal or channel at work) and no use for `idleMs` is stopped. Its home stays.
//! - **One start:** several callers waiting on a stopped box share one start, and a stop and a start of one box never
//!   overlap (both take its `starts` gate).
//! - **Room:** a start past `maxRunning` (or a sandbox past its owner's `maxRunningPerUser`) waits in line
//!   (`hostd_queue.rs`) and starts once there's room, people ahead of scheduled wakes.
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::broadcast;

use crate::boxhost::{BoxChange, BoxHost, BoxState, BoxUserWire, Busy, Running};
use crate::clock::SharedClock;
use crate::error::{Result, bad};
use crate::rows::BoxUser;

#[path = "hostd_queue.rs"]
mod queue;
#[path = "hostd_spawn.rs"]
mod spawn;
#[path = "hostd_wake.rs"]
mod wake;
use queue::Waiting;
use spawn::BoxProc;

#[derive(Clone)]
pub struct ProcessOptions {
    /// the wisp binary
    pub bin: PathBuf,
    /// homes go under `<data>/users/<id>/`
    pub data: PathBuf,
    pub idle_ms: i64,
    pub check_ms: i64,
    pub start_ms: i64,
    pub max_running: usize,
    /// a user's own box and their sandboxes that may run at once (config `hostd.maxRunningPerUser`): past it a
    /// sandbox waits in line, so one person's sandboxes can't fill the grid; their own box is never held back
    pub max_running_per_user: usize,
    /// a scheduled wake that didn't start tries again after this, doubling (to 10 min) while the wake is recent
    pub wake_retry_ms: i64,
    /// more environment for every box (the operator's choice: a harness's path, a test's fakes)
    pub env: Vec<(String, String)>,
    /// `<handle>` → the box's public URL (`WISP_PUBLIC_URL`)
    pub url_of: Arc<dyn Fn(&str) -> String + Send + Sync>,
    /// the container driver (boxd's socket and the limits); None runs boxes as processes here
    pub boxd: Option<crate::boxd::Driver>,
    /// pages in boxes show at `p<port>--<handle>` (config `hostd.previews`): each box is told its template
    pub previews: bool,
}

/// `https://ada.grid.example` → `https://p{port}--ada.grid.example`: a box's preview addresses (`{port}` literal).
pub fn preview_template(public: &str) -> String {
    match public.split_once("://") {
        Some((scheme, rest)) => format!("{scheme}://p{{port}}--{}", rest.trim_end_matches('/')),
        None => String::new(),
    }
}

impl std::fmt::Debug for ProcessOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessOptions").field("bin", &self.bin).field("data", &self.data).finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct Inst {
    /// who it runs for (set on a start or a restore): what a scheduled wake starts, and what change events name
    user: Option<BoxUser>,
    state: BoxState,
    port: Option<u16>,
    secret: String,
    child: Option<BoxProc>,
    last_touch: i64,
    inflight: usize,
    started_at: Option<i64>,
    stopped_at: Option<i64>,
    /// its next scheduled task, as its `/api/status` last said (v1 hostd.js `nextWakeAt`)
    next_wake_at: Option<i64>,
    /// its own last activity, from `/api/status`, or the last use the edge saw
    last_active_at: Option<i64>,
    /// the timer that wakes it for `next_wake_at` while it sleeps
    wake: Option<tokio::task::AbortHandle>,
    /// monotonic per-start id so an old exit watcher never reaps a newer start
    run_id: u64,
    /// why its last start didn't work (cleared when the next one begins): what its waiting page says
    error: String,
}

#[derive(Debug)]
pub struct ProcessHost {
    o: ProcessOptions,
    clock: SharedClock,
    inst: Arc<Mutex<HashMap<String, Inst>>>,
    starts: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// starts waiting for room, in order (locked after `inst`, never before it)
    queue: Mutex<VecDeque<Waiting>>,
    tx: broadcast::Sender<BoxChange>,
    peak: Mutex<(usize, usize)>,
    http: reqwest::Client,
    env_for: EnvSlot,
    /// itself, for the wake timers it starts
    me: std::sync::Weak<Self>,
    /// the grid is stopping: nothing starts, and a start under way gives up
    closed: std::sync::atomic::AtomicBool,
    next_run: std::sync::atomic::AtomicU64,
}

/// More environment for one box (a sandbox's owner's env and sign-ins).
pub type EnvFor = Arc<dyn Fn(&BoxUser) -> Vec<(String, String)> + Send + Sync>;

#[derive(Default)]
struct EnvSlot(Mutex<Option<EnvFor>>);

impl std::fmt::Debug for EnvSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if lock(&self.0).is_some() { "EnvSlot(set)" } else { "EnvSlot(none)" })
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A port nobody listens on, now. Only for boxd's containers, which bind it through boxd: a box this host starts as a
/// process binds a port of its own (`WISP_PORT=0`) and says which in its `server.json` (`hostd_spawn::bound_port`). A
/// port read here and closed is anyone's until the box binds it: on macOS CI another bind took it in that window and
/// the box stopped as it started, exit 101 (`tests/box_lifetimes.rs`).
fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| bad(format!("no free port: {e}"), 503))?;
    Ok(l.local_addr().map_err(|e| bad(e.to_string(), 503))?.port())
}

impl ProcessHost {
    pub fn new(o: ProcessOptions, clock: SharedClock) -> Arc<Self> {
        let (tx, _) = broadcast::channel(64);
        let http = reqwest::Client::builder().no_proxy().connect_timeout(Duration::from_secs(2)).build().unwrap_or_default();
        let h = Arc::new_cyclic(|me| Self {
            o,
            clock,
            inst: Arc::new(Mutex::new(HashMap::new())),
            starts: Mutex::new(HashMap::new()),
            queue: Mutex::new(VecDeque::new()),
            tx,
            peak: Mutex::new((0, 0)),
            http,
            env_for: EnvSlot::default(),
            me: me.clone(),
            closed: std::sync::atomic::AtomicBool::new(false),
            next_run: std::sync::atomic::AtomicU64::new(0),
        });
        let me = Arc::downgrade(&h);
        let every = Duration::from_millis(u64::try_from(h.o.check_ms.max(200)).unwrap_or(5000));
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(every).await;
                let Some(h) = me.upgrade() else { return };
                h.tick().await;
            }
        });
        h
    }

    /// Sets the hook that gives a box more environment (the grid's sandboxes).
    pub fn set_env_for(&self, f: EnvFor) {
        *lock(&self.env_for.0) = Some(f);
    }

    /// Stop every box (the grid is stopping).
    pub async fn close_all(&self) {
        BoxHost::close(self).await;
    }

    /// The secret the edge sends this box as `x-wisp-proxy-secret` (none while it's stopped).
    pub fn secret(&self, user_id: &str) -> Option<String> {
        lock(&self.inst).get(user_id).filter(|i| i.state == BoxState::Running).map(|i| i.secret.clone())
    }

    fn change(&self, user: &BoxUser, state: BoxState, event: &str) {
        let (who, started_at, stopped_at, next, last, error) = lock(&self.inst).get(&user.id).map_or_else(
            || (user.clone(), None, None, None, None, String::new()),
            |i| {
                let last = i.last_active_at.max(Some(i.last_touch)).filter(|t| *t > 0);
                (i.user.clone().unwrap_or_else(|| user.clone()), i.started_at, i.stopped_at, i.next_wake_at, last, i.error.clone())
            },
        );
        let _ = self.tx.send(BoxChange {
            user: BoxUserWire::from(&who),
            state,
            event: event.into(),
            started_at,
            stopped_at,
            next_wake_at: next,
            last_active_at: last,
            error,
        });
    }

    /// Starts the box (its gate held): now if there's room, else in line (`Queued`, a 503 naming its place).
    /// `wake`: a scheduled wake's, behind people in line.
    async fn start(&self, user: &BoxUser, wake: bool) -> Result<Running> {
        if self.is_closed() {
            return Err(bad("The grid is shutting down.", 503));
        }
        // a process box binds its own port and says which (`WISP_PORT=0`): none is read here and handed over
        let port = if self.o.boxd.is_some() { free_port()? } else { 0 };
        let secret = crate::accounts::token("");
        // room is counted and taken in one critical section: two cold starts can't both see the last slot
        if let Some(e) = self.admit_or_queue(user, wake) {
            return Err(e);
        }
        self.change(user, BoxState::Starting, "starting");
        let failed = if wake { "wake failed" } else { "failed" };
        let child = match self.spawn(user, port, &secret).await {
            Ok(c) => c,
            Err(e) => {
                self.start_failed(user, failed, wake, &e);
                return Err(e);
            }
        };
        let (child, port) = match self.healthy(user, port, child).await {
            Ok(up) => up,
            Err((event, e)) => {
                self.start_failed(user, if event == "failed" { failed } else { event }, wake, &e);
                return Err(e);
            }
        };
        let now = self.clock.now_ms();
        let run_id = self.next_run.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let mut exit_rx = child.watch_exit();
        {
            let mut m = lock(&self.inst);
            let i = m.entry(user.id.clone()).or_default();
            (i.state, i.port, i.secret, i.child, i.started_at, i.last_touch, i.run_id) =
                (BoxState::Running, Some(port), secret, Some(child), Some(now), now, run_id);
            // awake now: its schedule is its own again until it next sleeps (v1). Only now: a start that didn't come
            // up leaves the box asleep with its schedule, so its wake still comes (grid-r2-2)
            i.next_wake_at = None;
            if let Some(w) = i.wake.take() {
                w.abort();
            }
            let up = m.values().filter(|i| i.state == BoxState::Running).count();
            let mut p = lock(&self.peak);
            p.0 = p.0.max(up);
        }
        self.change(user, BoxState::Running, "woke");
        // Watch for an unexpected exit after startup (crash, OOM, boxd restart), matching v1 hostd.js:
        // without this, a dead box stays Running on a dead port and every request returns 502.
        let me = self.me.clone();
        let who = user.clone();
        tokio::spawn(async move {
            let _ = exit_rx.wait_for(Option::is_some).await;
            let why = exit_rx.borrow().clone().unwrap_or_else(|| "exited".into());
            let Some(h) = me.upgrade() else { return };
            h.on_unexpected_exit(&who, run_id, &why);
        });
        Ok(Running { port })
    }

    /// A start that didn't come up: asleep again, with why (its waiting page says it) and its schedule kept; the room
    /// it held goes to the next in line. A person's start re-arms the box's wake; a scheduled wake's tries again by
    /// itself (`wake_failed`).
    fn start_failed(&self, user: &BoxUser, event: &str, wake: bool, e: &crate::error::GridError) {
        {
            let mut m = lock(&self.inst);
            let i = m.entry(user.id.clone()).or_default();
            i.state = BoxState::Stopped;
            i.error = start_error(e);
        }
        self.change(user, BoxState::Stopped, event);
        if !wake {
            self.schedule_wake(&user.id);
        }
        self.drain();
    }

    /// Reaps a running box whose process or container exited unexpectedly (if `run_id` still matches).
    fn on_unexpected_exit(&self, user: &BoxUser, run_id: u64, why: &str) {
        let now = self.clock.now_ms();
        let who = {
            let mut m = lock(&self.inst);
            let Some(i) = m.get_mut(&user.id) else { return };
            if i.state != BoxState::Running || i.run_id != run_id {
                return;
            }
            (i.state, i.port, i.child, i.stopped_at) = (BoxState::Stopped, None, None, Some(now));
            i.secret.clear();
            i.user.clone().unwrap_or_else(|| user.clone())
        };
        self.record_unexpected_exit(&who, why);
    }

    /// Synchronous check before handing back a running port or polling `/api/status`: reaps a box that has
    /// already exited even if its watcher task has not run yet.
    pub(super) fn reap_if_exited(&self, user_id: &str) -> bool {
        let now = self.clock.now_ms();
        let reaped = {
            let mut m = lock(&self.inst);
            let Some(i) = m.get_mut(user_id).filter(|i| i.state == BoxState::Running) else { return false };
            let Some(why) = i.child.as_ref().and_then(BoxProc::exited) else { return false };
            (i.state, i.port, i.child, i.stopped_at) = (BoxState::Stopped, None, None, Some(now));
            i.secret.clear();
            let who =
                i.user.clone().unwrap_or_else(|| BoxUser { id: user_id.to_owned(), handle: String::new(), sandbox: None, owner: None });
            (who, why)
        };
        self.record_unexpected_exit(&reaped.0, &reaped.1);
        true
    }

    fn record_unexpected_exit(&self, user: &BoxUser, why: &str) {
        let reason = exit_reason(why);
        tracing::warn!("hostd: {} ({}) stopped unexpectedly: {reason}", user.handle, user.id);
        self.change(user, BoxState::Stopped, &reason);
        self.schedule_wake(&user.id);
        self.drain();
    }

    /// The box's `starts` gate: one start or stop of a box at a time.
    fn gate(&self, user_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        lock(&self.starts).entry(user_id.to_owned()).or_default().clone()
    }

    /// `stop` with the box's gate already held (`remove` holds it across the stop and the delete).
    async fn stop_held(&self, user_id: &str, reason: &str) -> Result<bool> {
        let (child, run_id) = {
            let mut m = lock(&self.inst);
            let Some(i) = m.get_mut(user_id) else { return Ok(false) };
            if i.state == BoxState::Queued {
                // put to sleep while waiting for room: out of line
                i.state = BoxState::Stopped;
                drop(m);
                lock(&self.queue).retain(|w| w.user.id != user_id);
                return Ok(false);
            }
            if i.state != BoxState::Running {
                return Ok(false);
            }
            i.state = BoxState::Stopping;
            (i.child.take(), i.run_id)
        };
        if let Some(c) = child {
            c.stop().await;
        }
        let now = self.clock.now_ms();
        {
            let mut m = lock(&self.inst);
            // only the run this stop began on: a start in between (the gate should rule that out) keeps its box
            let Some(i) = m.get_mut(user_id).filter(|i| i.state == BoxState::Stopping && i.run_id == run_id) else { return Ok(true) };
            (i.state, i.port, i.stopped_at) = (BoxState::Stopped, None, Some(now));
            i.secret.clear();
        }
        let user = BoxUser { id: user_id.to_owned(), handle: String::new(), sandbox: None, owner: None };
        self.change(&user, BoxState::Stopped, reason);
        // asleep with a task scheduled: it wakes for it (v1 hostd.js `#scheduleWake`)
        self.schedule_wake(user_id);
        self.drain();
        Ok(true)
    }
}

/// Why a start didn't work, for its waiting page: what hostd said (its 503s and 504s are written for people).
fn start_error(e: &crate::error::GridError) -> String {
    match e {
        crate::error::GridError::Http { status: 503 | 504, message, .. } => message.clone(),
        _ => e.public_message(),
    }
}

fn exit_reason(why: &str) -> String {
    if why.contains("137") || why.contains("SIGKILL") || why.contains("signal: 9") || why.contains("signal 9") {
        "out of memory".into()
    } else {
        format!("exited ({why})")
    }
}

#[async_trait::async_trait]
impl BoxHost for ProcessHost {
    fn state(&self, user_id: &str) -> BoxState {
        lock(&self.inst).get(user_id).map_or(BoxState::Stopped, |i| i.state)
    }
    fn port(&self, user_id: &str) -> Option<u16> {
        lock(&self.inst).get(user_id).filter(|i| i.state == BoxState::Running).and_then(|i| i.port)
    }
    fn touch(&self, user_id: &str) {
        let now = self.clock.now_ms();
        if let Some(i) = lock(&self.inst).get_mut(user_id) {
            i.last_touch = i.last_touch.max(now);
        }
    }
    fn begin(&self, user_id: &str, touch: bool) -> Busy {
        match lock(&self.inst).get_mut(user_id) {
            Some(i) => i.inflight += 1,
            None => return Busy::none(),
        }
        // the guard finds the box again by id when the request ends
        let (inst, id, clock) = (self.inst.clone(), user_id.to_owned(), self.clock.clone());
        Busy::new(move || {
            if let Some(i) = lock(&inst).get_mut(&id) {
                i.inflight = i.inflight.saturating_sub(1);
                if touch {
                    i.last_touch = i.last_touch.max(clock.now_ms());
                }
            }
        })
    }
    async fn ensure(&self, user: &BoxUser, wake: bool) -> Result<Running> {
        let gate = self.gate(&user.id);
        // also waits out a stop under way: a start never begins on a box that's still stopping (grid-r2-3)
        let _one = gate.lock().await;
        self.reap_if_exited(&user.id);
        if let Some(port) = self.port(&user.id) {
            if !wake {
                self.touch(&user.id);
            }
            return Ok(Running { port });
        }
        self.start(user, wake).await
    }
    async fn stop(&self, user_id: &str, reason: &str) -> Result<bool> {
        // the same gate as a start: a stop never ends a run that began while it was stopping the last one
        let gate = self.gate(user_id);
        let _one = gate.lock().await;
        self.stop_held(user_id, reason).await
    }
    async fn remove(&self, user: &BoxUser) -> Result<()> {
        let gate = self.gate(&user.id);
        let _one = gate.lock().await;
        // out of line first: room freed by the stop below never starts it again
        lock(&self.queue).retain(|w| w.user.id != user.id);
        self.stop_held(&user.id, "deleted").await?;
        match &self.o.boxd {
            // the container, then its folder (home, logins and all): boxd's to delete
            Some(d) => {
                crate::boxd::call(&d.socket, &serde_json::json!({ "op": "remove", "id": user.id })).await?;
            }
            None => {
                let _ = tokio::fs::remove_dir_all(self.o.data.join("users").join(&user.id)).await;
            }
        }
        lock(&self.inst).remove(&user.id);
        Ok(())
    }
    fn restore(&self, user: &BoxUser, next_wake_at: Option<i64>) {
        self.restore_wake(user, next_wake_at);
    }
    fn capacity(&self) -> usize {
        self.o.max_running
    }
    fn take_peak(&self) -> (usize, usize) {
        std::mem::take(&mut *lock(&self.peak))
    }
    fn changes(&self) -> broadcast::Receiver<BoxChange> {
        self.tx.subscribe()
    }
    fn previews(&self) -> bool {
        self.o.previews
    }
    async fn close(&self) {
        self.close_boxes().await;
    }
}
