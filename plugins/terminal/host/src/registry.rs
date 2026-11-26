//! The terminals that run, one per (kind, task): their PTY, state, kept output and watching pages.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use wisp_kernel::Ctx;
use wisp_kernel::http::{Response, StreamTx};
use wisp_proc::{ChildHandle, Env, Proc, PtySize, SpawnOpts};
use wisp_terminal_api::commands::OpenArgs;
use wisp_terminal_api::{InputOrder, TermInfo, TermKind, TermState};
use wisp_terminal_core::flow::{FlowEvent, SLOW_MS, exit_event, step};
use wisp_terminal_core::replay::{KEEP, Replay, Utf8Stream};
use wisp_terminal_core::size::{COLS, ROWS, clamp};
use wisp_terminal_core::{order, sse};

struct Term {
    info: TermInfo,
    handle: Option<ChildHandle>,
    replay: Replay,
    clients: Vec<StreamTx>,
    order: order::InputOrder,
    /// which open this is: a timer or task of a Term that was replaced (a restart) or hung up finds another one
    /// (or none) under its key and does nothing. Not `started_at`: two opens in one millisecond share that
    nth: u64,
    /// a gap timer runs for its held keys ([`Registry::input`]): one per Term
    gap_armed: bool,
}

type Key = (TermKind, String);
type Map = Arc<Mutex<HashMap<Key, Term>>>;

static NTH: AtomicU64 = AtomicU64::new(1);

/// How long an ended terminal's last screen is kept for a page that opens late (its exit code, Start again); then it's
/// forgotten (attach answers gone), so a long-running wisp doesn't keep every ended shell's megabyte of output.
pub const ENDED_KEEP_MS: u64 = 5 * 60 * 1000;

impl Term {
    fn new(info: TermInfo, handle: Option<ChildHandle>, replay: Replay) -> Self {
        let nth = NTH.fetch_add(1, Ordering::Relaxed);
        Self { info, handle, replay, clients: Vec::new(), order: order::InputOrder::new(), nth, gap_armed: false }
    }

    fn type_in(&self, parts: Vec<String>) {
        if let Some(h) = &self.handle {
            for p in parts {
                let _ = h.write(p.into_bytes());
            }
        }
    }
}

pub struct Registry {
    proc: Proc,
    map: Map,
    /// The port each task's app was given (`PORT`, ADR-0086): chosen once per start, read by every caller after
    apps: Arc<Mutex<HashMap<String, u16>>>,
    /// Preview's proxies (ADR-0049), under the plugin's scope: switched off, they stop; a run that ends stops its own
    pub proxies: Arc<crate::proxy::Proxies>,
    pub scope: wisp_kernel::CancellationToken,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now(ctx: &Ctx) -> i64 {
    ctx.clock().map_or(0, |c| c.now_ms())
}

/// The port task `id`'s app was given (the guard dropped before any await).
fn given(apps: &Mutex<HashMap<String, u16>>, id: &str) -> Option<u16> {
    lock(apps).get(id).copied()
}

/// Sends `frame` to every page; a page that went away, or can't keep up (its stream full), is dropped. A kept page
/// never misses a frame: a dropped one's stream ends, and it attaches again for the whole screen from the replay.
fn broadcast(t: &mut Term, frame: &[u8]) {
    t.clients.retain(|c| c.try_send(frame.to_vec()).is_ok());
    t.info.clients = u32::try_from(t.clients.len()).unwrap_or(u32::MAX);
}

impl Registry {
    pub fn new(proc: Proc, scope: wisp_kernel::CancellationToken) -> Self {
        Self { proc, map: Arc::new(Mutex::new(HashMap::new())), apps: Arc::new(Mutex::new(HashMap::new())), proxies: Arc::default(), scope }
    }

    /// The port task `id`'s app was last given (`PORT`), if it was started since wisp started.
    pub fn app_port(&self, id: &str) -> Option<u16> {
        given(&self.apps, id)
    }

    /// Another task's app holds `port` in `apps`: given it and not ended (starting counts: its program may not have
    /// bound yet; a hung-up run's record is gone, [`Self::close`]). Lock order: `apps`, then `map`.
    fn held_in(&self, apps: &HashMap<String, u16>, id: &str, port: u16) -> bool {
        let m = lock(&self.map);
        apps.iter().any(|(k, &p)| p == port && k != id && m.get(&(TermKind::Run, k.clone())).is_none_or(|t| t.info.state.live()))
    }

    fn held_by_other(&self, id: &str, port: u16) -> bool {
        self.held_in(&lock(&self.apps), id, port)
    }

    /// Records `port` as task `id`'s app's, checked and written under one lock: none when another task's app took it
    /// while this one probed; the recorded one when this task's app already runs (a second Run won the race).
    fn record(&self, id: &str, port: u16) -> Option<u16> {
        let mut apps = lock(&self.apps);
        if let Some(&theirs) = apps.get(id).filter(|_| self.live(TermKind::Run, id)) {
            return Some(theirs);
        }
        if self.held_in(&apps, id, port) {
            return None;
        }
        apps.insert(id.to_owned(), port);
        Some(port)
    }

    /// Chooses task `id`'s app port and records it (v1 `pickPort`, ADR-0086): its own when free, else the next free of
    /// [`wisp_terminal_core::run::ports_for`], else any free one the system gives. Free: no other task's app has it,
    /// loopback binds it and nothing answers on it, so a port another program holds or the system reserves (Windows'
    /// excluded ranges) is passed over. The chosen one is what every caller reads after ([`Self::app_port`]).
    pub async fn claim_app_port(&self, id: &str) -> u16 {
        for port in wisp_terminal_core::run::ports_for(id) {
            if self.held_by_other(id, port) || !crate::ports::free(port).await {
                continue;
            }
            // checked again as it's recorded: another start may have taken it while this one probed
            if let Some(p) = self.record(id, port) {
                return p;
            }
        }
        // none of its own: the system's (only the task's own as a last resort, when loopback binds nothing at all)
        let port = crate::ports::any_free().await.unwrap_or_else(|| wisp_terminal_core::run::port_for(id));
        lock(&self.apps).insert(id.to_owned(), port);
        port
    }

    pub fn info(&self, kind: TermKind, id: &str) -> Option<TermInfo> {
        lock(&self.map).get(&(kind, id.to_owned())).map(|t| t.info.clone())
    }

    pub fn live(&self, kind: TermKind, id: &str) -> bool {
        self.info(kind, id).is_some_and(|i| i.state.live())
    }

    /// The port the run's own output says the app listens on (v1 runs.js `r.url`), from its kept screen.
    pub fn local_port(&self, kind: TermKind, id: &str) -> Option<u16> {
        let mut m = lock(&self.map);
        let t = m.get_mut(&(kind, id.to_owned()))?;
        wisp_terminal_core::pick::local_port_in(t.replay.read())
    }

    /// Starts one, or answers the one that runs. A program that can't start is an answer (`failed`).
    pub fn open(&self, ctx: &Ctx, a: &OpenArgs) -> TermInfo {
        let key = (a.kind, a.id.clone());
        if let Some(t) = lock(&self.map).get(&key).filter(|t| t.info.state.live()) {
            return t.info.clone();
        }
        let (cols, rows) = (clamp(a.cols, COLS, COLS.default), clamp(a.rows, ROWS, ROWS.default));
        let at = now(ctx);
        let mut info = TermInfo {
            id: a.id.clone(),
            kind: a.kind,
            state: step(TermState::Idle, FlowEvent::Start),
            cols,
            rows,
            resizable: true,
            code: None,
            started_at: at,
            used_at: at,
            clients: 0,
            tui: a.tui,
        };
        let patch = wisp_terminal_core::env::term_env(a.kind == TermKind::Takeover);
        let opts = SpawnOpts {
            cwd: Some(a.cwd.clone().into()),
            env: Env { clear: false, set: patch.set.clone(), remove: patch.remove.clone() },
            pty: Some(PtySize { cols, rows }),
            // a run's app gets v1's grace to stop (close waits for its tree to be gone): a restart gets its port back
            grace: (a.kind == TermKind::Run).then(|| std::time::Duration::from_millis(wisp_terminal_core::run::STOP_GRACE_MS)),
            // the plugin switched off (or reloaded): its shells, take overs and apps' trees go with it, never orphaned
            scope: Some(self.scope.child_token()),
            ..SpawnOpts::default()
        };
        let mut replay = Replay::new(KEEP);
        // "" or a relative folder would start it in the server's own folder
        let no_dir = wisp_runtime_api::paths::Folder::of(&a.cwd).err();
        let spawned = match a.argv.split_first() {
            Some((p, rest)) if no_dir.is_none() => self.proc.spawn(p, rest, opts).ok(),
            _ => None,
        };
        let Some(child) = spawned else {
            // it never ran: wisp's line on its screen, code 127 (v1)
            info.state = step(info.state, FlowEvent::Fail);
            info.code = Some(127);
            let why = no_dir.map_or_else(String::new, |e| format!(" ({e})"));
            replay.push(&format!("wisp: couldn't start {}{why}\r\n", a.argv.first().map_or("", String::as_str)));
            let t = Term::new(info.clone(), None, replay);
            let nth = t.nth;
            lock(&self.map).insert(key.clone(), t);
            forget_later(ctx, &self.map, &self.apps, key, nth);
            return info;
        };
        let (handle, mut rx) = child.split();
        let t = Term::new(info.clone(), Some(handle.clone()), replay);
        let nth = t.nth;
        lock(&self.map).insert(key.clone(), t);
        let (map, apps, proxies) = (self.map.clone(), self.apps.clone(), self.proxies.clone());
        // a slow start says so (#440)
        if let Ok(clock) = ctx.clock() {
            let (map, key) = (map.clone(), key.clone());
            ctx.spawn(async move {
                clock.sleep(SLOW_MS).await;
                if let Some(t) = lock(&map).get_mut(&key).filter(|t| t.nth == nth && t.info.state == TermState::Starting) {
                    t.info.state = step(t.info.state, FlowEvent::Slow);
                }
            });
        }
        let ectx = ctx.clone();
        ctx.spawn(async move {
            let mut utf8 = Utf8Stream::new();
            while let Some(o) = rx.recv().await {
                let text = utf8.decode(&o.data);
                if text.is_empty() {
                    continue;
                }
                let mut m = lock(&map);
                let Some(t) = m.get_mut(&key) else { return };
                if t.nth != nth {
                    return; // a newer terminal took its place
                }
                t.info.state = step(t.info.state, FlowEvent::Ready);
                t.replay.push(&text);
                broadcast(t, &sse::data(&text));
            }
            let exit = handle.exited().await;
            let code = exit.code.unwrap_or(-1);
            let ended = {
                let mut m = lock(&map);
                m.get_mut(&key).filter(|t| t.nth == nth).map(|t| {
                    t.info.state = step(t.info.state, exit_event(t.info.state, code));
                    t.info.code = Some(code);
                    t.handle = None;
                    broadcast(t, &sse::exit(Some(code)));
                    t.clients.clear();
                    t.info.clients = 0;
                    t.info.state
                })
            };
            if let Some(state) = ended {
                ended_by_itself(&ectx, &map, &apps, &proxies, key, nth, (state, code)).await;
            }
        });
        info
    }

    /// A page watches it: the size, the screen so far, then the output as it comes (v1 `attach`).
    pub fn attach(&self, kind: TermKind, id: &str) -> Response {
        let (tx, res) = Response::sse();
        let mut m = lock(&self.map);
        let Some(t) = m.get_mut(&(kind, id.to_owned())) else {
            let _ = tx.try_send(sse::GONE.to_vec());
            return res;
        };
        let _ = tx.try_send(sse::hello(t.info.cols, t.info.rows, t.info.resizable, t.info.state));
        let screen = t.replay.read().to_owned();
        let _ = tx.try_send(sse::data(&screen)); // always, even empty: the page replays it
        if let Some(code) = t.info.code {
            let _ = tx.try_send(sse::exit(Some(code)));
            return res;
        }
        t.clients.push(tx);
        t.info.clients = u32::try_from(t.clients.len()).unwrap_or(u32::MAX);
        res
    }

    /// Typed keys; numbered ones (seq per page) go in in order, a gap skipped after a while.
    pub fn input(&self, kind: TermKind, id: &str, data: &str, o: &InputOrder, ctx: &Ctx) -> bool {
        let mut m = lock(&self.map);
        let Some(t) = m.get_mut(&(kind, id.to_owned())).filter(|t| t.info.state.live()) else { return false };
        let at = now(ctx);
        t.info.used_at = at;
        let parts = match (o.seq, o.from.as_deref()) {
            (Some(seq), Some(from)) => t.order.put(data.to_owned(), seq, from, at),
            _ => vec![data.to_owned()],
        };
        t.type_in(parts);
        // keys held behind a gap (a send that failed, a page that kept counting): they go in once the gap is
        // skipped, not only when the next key comes (it may never)
        if t.order.deadline().is_some()
            && !t.gap_armed
            && let Ok(clock) = ctx.clock()
        {
            t.gap_armed = true;
            let (map, key, nth) = (self.map.clone(), (kind, id.to_owned()), t.nth);
            ctx.spawn(async move { skip_gaps(&*clock, &map, &key, nth).await });
        }
        true
    }

    pub fn resize(&self, kind: TermKind, id: &str, cols: Option<f64>, rows: Option<f64>) -> bool {
        let (Some(_), Some(_)) = (cols, rows) else { return false };
        let mut m = lock(&self.map);
        let Some(t) = m.get_mut(&(kind, id.to_owned())).filter(|t| t.info.state.live()) else { return false };
        t.info.cols = clamp(cols, COLS, t.info.cols);
        t.info.rows = clamp(rows, ROWS, t.info.rows);
        t.handle.as_ref().is_some_and(|h| h.resize(PtySize { cols: t.info.cols, rows: t.info.rows }).is_ok())
    }

    /// Its program's pid, while it has one.
    #[cfg(test)]
    pub(crate) fn pid(&self, kind: TermKind, id: &str) -> Option<u32> {
        lock(&self.map).get(&(kind, id.to_owned()))?.handle.as_ref().map(ChildHandle::pid)
    }

    /// Its kept screen.
    #[cfg(test)]
    pub(crate) fn screen(&self, kind: TermKind, id: &str) -> String {
        lock(&self.map).get_mut(&(kind, id.to_owned())).map(|t| t.replay.read().to_owned()).unwrap_or_default()
    }

    /// Task `id`'s app was given `port` (a restart that claimed another one).
    #[cfg(test)]
    pub(crate) fn set_app_port(&self, id: &str, port: u16) {
        lock(&self.apps).insert(id.to_owned(), port);
    }

    /// Hangs it up (a start in progress is cancelled, never raced: #435); the registry forgets it.
    pub async fn close(&self, kind: TermKind, id: &str) -> bool {
        if kind == TermKind::Run {
            lock(&self.apps).remove(id); // hung up (Stop, its route, a restart): its port is no one's
        }
        let t = lock(&self.map).remove(&(kind, id.to_owned()));
        let Some(mut t) = t else { return false };
        t.info.state = step(t.info.state, FlowEvent::Stop);
        broadcast(&mut t, &sse::exit(None));
        if let Some(h) = t.handle.take() {
            let _ = h.kill().await;
        }
        true
    }
}

/// Term `nth` under `key` ended on its own (a take over's CLI quit, the app exited): whoever owns the task hears it;
/// a run's proxy stops with it and its row says Run app again; its screen is forgotten after a while.
async fn ended_by_itself(
    ctx: &Ctx,
    map: &Map,
    apps: &Arc<Mutex<HashMap<String, u16>>>,
    proxies: &crate::proxy::Proxies,
    key: Key,
    nth: u64,
    (state, code): (TermState, i32),
) {
    ctx.emit(&ctx.root_call(), &wisp_terminal_api::Changed { kind: key.0, id: key.1.clone(), state, code: Some(code), gone: false });
    if key.0 == TermKind::Run {
        // a new run that took this one's place meanwhile keeps its proxy and its row (the map lock is held over the
        // stop, and start_run puts its Term in the map before it starts a proxy)
        let ours = {
            let m = lock(map);
            let ours = m.get(&key).is_some_and(|t| t.nth == nth);
            if ours {
                proxies.stop(&key.1);
            }
            ours
        };
        if ours {
            crate::mark_run(ctx, &ctx.root_call(), &key.1, false, given(apps, &key.1)).await;
        }
    }
    forget_later(ctx, map, apps, key, nth);
}

/// The gap timer of Term `nth` under `key`: wakes at its order's deadline, types what [`order::InputOrder::tick`]
/// lets in, and goes on while keys are still held. A Term replaced or hung up meanwhile isn't touched.
async fn skip_gaps(clock: &dyn wisp_kernel::Clock, map: &Map, key: &Key, nth: u64) {
    loop {
        let due = {
            let mut m = lock(map);
            let Some(t) = m.get_mut(key).filter(|t| t.nth == nth) else { return };
            let Some(due) = t.order.deadline() else {
                t.gap_armed = false; // nothing held: the next held key arms it again
                return;
            };
            due
        };
        let wait = due.saturating_sub(clock.now_ms());
        clock.sleep(u64::try_from(wait).unwrap_or(0)).await;
        let mut m = lock(map);
        let Some(t) = m.get_mut(key).filter(|t| t.nth == nth) else { return };
        let parts = t.order.tick(clock.now_ms());
        t.type_in(parts);
    }
}

/// An ended (or never started) Term under `key` is forgotten [`ENDED_KEEP_MS`] after it ended, with its app's port
/// record (an ended run's port is no one's; a record without a Term would count as taken). Without a clock it's kept,
/// as before.
fn forget_later(ctx: &Ctx, map: &Map, apps: &Arc<Mutex<HashMap<String, u16>>>, key: Key, nth: u64) {
    let Ok(clock) = ctx.clock() else { return };
    let (map, apps) = (map.clone(), apps.clone());
    ctx.spawn(async move {
        clock.sleep(ENDED_KEEP_MS).await;
        // lock order: `apps`, then `map`
        let mut a = lock(&apps);
        let mut m = lock(&map);
        if m.get(&key).is_some_and(|t| t.nth == nth && !t.info.state.live() && t.clients.is_empty()) {
            m.remove(&key);
            if key.0 == TermKind::Run {
                a.remove(&key.1);
            }
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;
    use wisp_testkit::FakeClock;

    /// The plugin's context on a kernel whose clock is `clock` (the registries below are the tests' own).
    pub(crate) async fn kernel(clock: &FakeClock) -> (wisp_kernel::Kernel, Ctx) {
        let k = wisp_testkit::TestApp::new().clock(clock).service(wisp_proc::local()).with(crate::TerminalPlugin).start().await;
        let ctx = k.ctx("wisp.terminal").expect("wisp.terminal is on");
        (k, ctx)
    }

    pub(crate) fn args(kind: TermKind, id: &str, argv: &[&str]) -> OpenArgs {
        let cwd = std::env::temp_dir().to_string_lossy().into_owned();
        OpenArgs { kind, id: id.to_owned(), argv: argv.iter().map(|&a| a.to_owned()).collect(), cwd, ..OpenArgs::default() }
    }

    /// Waits (real time, at most 10 s) for `f`.
    pub(crate) async fn until(f: impl Fn() -> bool) -> bool {
        for _ in 0..500 {
            if f() {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        f()
    }

    /// A numbered key held behind a gap (seq 1 never came: its send failed) goes in once the gap is skipped, with no
    /// further key: the host wakes the order's tick at its deadline. It sat there until the next key came, forever.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_key_held_behind_a_gap_goes_in_after_the_gap_with_no_further_key() {
        let clock = FakeClock::at(1_000);
        let (_k, ctx) = kernel(&clock).await;
        let r = Registry::new(wisp_proc::local(), wisp_kernel::CancellationToken::new());
        r.open(&ctx, &args(TermKind::Shell, "gap", &["cat"]));
        let o = |seq| InputOrder { seq: Some(seq), from: Some("page".into()) };
        assert!(r.input(TermKind::Shell, "gap", "AAA", &o(0), &ctx));
        assert!(until(|| r.screen(TermKind::Shell, "gap").contains("AAA")).await, "seq 0 goes in at once");
        assert!(r.input(TermKind::Shell, "gap", "CCC", &o(2), &ctx));
        // the start's slow timer and the gap's
        assert!(until(|| clock.sleepers() >= 2).await, "a gap timer is armed");
        assert!(!r.screen(TermKind::Shell, "gap").contains("CCC"), "held for seq 1");
        clock.advance(u64::try_from(order::GAP_MS).unwrap());
        assert!(until(|| r.screen(TermKind::Shell, "gap").contains("CCC")).await, "{:?}", r.screen(TermKind::Shell, "gap"));
        r.close(TermKind::Shell, "gap").await;
    }

    /// Switching wisp.terminal off (or reloading it) ends its terminals' programs: they were spawned outside its scope
    /// and kept running, orphaned.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_plugins_scope_ending_kills_its_terminals() {
        let clock = FakeClock::at(1_000);
        let (_k, ctx) = kernel(&clock).await;
        let scope = wisp_kernel::CancellationToken::new();
        let r = Registry::new(wisp_proc::local(), scope.clone());
        r.open(&ctx, &args(TermKind::Shell, "scoped", &["sleep", "60"]));
        let pid = r.pid(TermKind::Shell, "scoped").expect("it runs");
        scope.cancel();
        assert!(
            until(|| r.info(TermKind::Shell, "scoped").is_some_and(|i| i.state.over())).await,
            "{:?}",
            r.info(TermKind::Shell, "scoped")
        );
        assert!(until(|| !wisp_proc::pid_alive(pid)).await, "its program is gone");
    }

    /// A page that can't keep up (its stream full) is dropped, so its stream ends and it attaches again for the whole
    /// screen; it was kept and the frames it had no room for were lost, the exit frame among them.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_page_whose_stream_is_full_is_dropped_not_kept_missing_frames() {
        let clock = FakeClock::at(1_000);
        let (_k, ctx) = kernel(&clock).await;
        let r = Registry::new(wisp_proc::local(), wisp_kernel::CancellationToken::new());
        r.open(&ctx, &args(TermKind::Shell, "slow", &[])); // never starts: a Term with no program, enough here
        let (full, _full_res) = Response::sse();
        while full.try_send(b"x".to_vec()).is_ok() {}
        let (ok, _ok_res) = Response::sse();
        let mut m = lock(&r.map);
        let t = m.get_mut(&(TermKind::Shell, "slow".to_owned())).unwrap();
        t.clients = vec![full, ok];
        broadcast(t, &sse::data("hi"));
        assert_eq!(t.clients.len(), 1, "the full one is dropped");
        assert_eq!(t.info.clients, 1);
    }

    /// An ended terminal (its last screen, its code) is forgotten a while after it ended: they were kept for as long as
    /// wisp ran, a megabyte of output each.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_ended_terminal_is_forgotten_after_a_while() {
        let clock = FakeClock::at(1_000);
        let (_k, ctx) = kernel(&clock).await;
        let r = Registry::new(wisp_proc::local(), wisp_kernel::CancellationToken::new());
        r.open(&ctx, &args(TermKind::Shell, "done", &["true"]));
        assert!(until(|| r.info(TermKind::Shell, "done").is_some_and(|i| i.state.over())).await);
        clock.advance(ENDED_KEEP_MS - 1);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(r.info(TermKind::Shell, "done").is_some(), "kept for a page that opens late");
        // its timer may arm a little after the state changes: time moves on until it's gone
        assert!(
            until(|| {
                clock.advance(ENDED_KEEP_MS);
                r.info(TermKind::Shell, "done").is_none()
            })
            .await,
            "forgotten"
        );
        let Response::Stream { mut body, .. } = r.attach(TermKind::Shell, "done") else { panic!("a stream") };
        assert_eq!(body.try_recv().unwrap(), sse::GONE, "attach answers gone");
    }

    /// v1 runs.js pickPort for the app (ADR-0086): the task's own port held by another program (or in a range Windows
    /// reserves) isn't given to its app; the next free one is, and what's recorded is that one, not the id's.
    #[tokio::test]
    async fn a_taken_app_port_moves_to_the_next_free_one_and_is_the_one_recorded() {
        // an id whose own port this test can hold: a random id's port may already be some other program's on the
        // runner (CI: AddrInUse), which isn't the case under test
        let mut found = None;
        for _ in 0..50 {
            let id = format!("app-taken-{}", wisp_kernel::ids::uuid7());
            let derived = wisp_terminal_core::run::port_for(&id);
            // held here, listening; a port the system refuses already (a reserved range) is the same case
            if let Ok(held) = tokio::net::TcpListener::bind(("127.0.0.1", derived)).await {
                found = Some((id, derived, held));
                break;
            }
        }
        let (id, derived, held) = found.expect("an id whose own port is free to hold");
        let r = Registry::new(wisp_proc::local(), wisp_kernel::CancellationToken::new());
        assert_eq!(r.app_port(&id), None, "never started: nothing recorded");
        let port = r.claim_app_port(&id).await;
        assert_ne!(port, derived, "not the taken one");
        assert!(wisp_terminal_core::run::ports_for(&id).any(|p| p == port), "the next of its own, while one is free");
        assert_eq!(r.app_port(&id), Some(port), "the port it was given is what's read back");
        // another task whose own port is this one doesn't get it while the first one's app starts
        assert!(r.held_by_other("someone-else", port));
        drop(held);
        r.close(TermKind::Run, &id).await;
        assert_eq!(r.app_port(&id), None, "hung up: forgotten");
        // its own port free again: that's the one (the stable origin)
        assert_eq!(r.claim_app_port(&id).await, derived);
    }
}
