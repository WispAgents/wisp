//! Durable events (ADR-0044): one log, two speeds. An `Event` with `DURABLE = true` is appended to the
//! store's `event` table and delivered to durable subscriptions from a cursor, so a restart, a lag or a
//! plugin switched off loses nothing. The in-process path is unchanged and comes first.
//!
//! - **Append:** in the emitting command's own transaction when it writes (`take` / `InTx`, called by the
//!   store around each `write` made while a command runs), else in one transaction when it returns Ok. A
//!   rolled-back write takes its events with it. The write also stages the command's events emitted so far
//!   (ADR-0075: `InTx::stage`, queued held before COMMIT, let go after it), so a reader of its rows finds them. A repeated `(type, key)` within 24 h is recorded once (the
//!   log answers `None` for it), and isn't published either.
//! - **Deliver:** a durable subscription (`Setup::on_durable`) has a stable name and a cursor in
//!   `event_cursor`. Its worker takes `cursor + 1` from the ring of recent events, or reads the table when
//!   it's behind (start, lag, after being off). Seqs are contiguous (one writer), so it never skips one.
//! - **Effectively once:** the handler runs inside a scope that rides its cursor on the store writes it
//!   makes (`InTx::cursor`): a crash before the commit replays it, after it doesn't. A handler that writes
//!   nothing has its cursor saved later (ADR-0088): every subscription's idle cursor goes in one write within
//!   `SAVE_WITHIN_MS` of the first one waiting (and when its worker stops), so a crash in that second replays
//!   those events to handlers that wrote nothing for them (they stay idempotent: the envelope has seq and key).
//! - **Batches** (`Setup::on_durable_batch`, ADR-0088): the events already in after the next one, matching, one
//!   after another, go to the handler in one call, its cursor riding its one write at the last of them; a batch
//!   that fails is delivered again one by one (each with its own tries, parked alone).
//! - **Retries:** 3 tries; then the event is parked for that subscription (with why) and the cursor moves on.
//!   `retry` delivers a parked one again.
//! - **Gaps:** a cursor that fell off the retention window gets `Durable::Gap` once (re-read current state).
//! - **Causation:** a handler's call carries the event's seq as its `cause`; events it emits record it, and
//!   their chain depth (`Call::cause_depth`) lets triggers cut loops.
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use futures::FutureExt as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::call::{CallInner, RootTx};
use crate::events::{EventEnvelope, Logged, Payload, Pending, Staged};
use crate::host::Ctx;
use crate::perm::glob;
use crate::state::KernelState;
use crate::{BoxFuture, Call, Caller, Event, Result, WireError};

/// How many recent durable events the kernel keeps in memory for live delivery.
pub const RING: usize = 8192;
const ATTEMPTS: u32 = 3;
/// A subscription's cursor is saved at least every this many deliveries (and whenever it catches up).
const SAVE_EVERY: u32 = 256;
/// Idle cursors are saved together, this long after the first one waiting (ADR-0088): one commit for every
/// subscription a burst of events woke, not one each (what a crash in it replays to handlers that wrote nothing).
pub const SAVE_WITHIN_MS: u64 = 1_000;
/// The most events a batch subscription gets in one call.
const BATCH: usize = 64;
/// The type a subscription's gap is delivered as.
pub const GAP: &str = "event.gap";

/// A durable event on its way into the log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewEvent {
    pub ty: String,
    pub plugin: String,
    pub subject: Option<String>,
    pub key: Option<String>,
    pub cause: Option<u64>,
    pub root: String,
    pub at: i64,
    pub data: Value,
}

/// A durable event as the log keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredEvent {
    pub seq: u64,
    #[serde(flatten)]
    pub event: NewEvent,
}

/// An event a subscription gave up on (3 failed tries), kept to retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parked {
    pub plugin: String,
    pub name: String,
    pub seq: u64,
    pub why: String,
    pub at: i64,
}

/// The durable log (the store implements it; `App::service` provides it). Without one, durable events
/// behave as ephemeral ones: delivered in process, not kept.
pub trait EventLog: Send + Sync + 'static {
    /// Appends `rows` in one transaction: each one's seq, or `None` for a `(type, key)` seen within 24 h.
    fn append(&self, rows: Vec<NewEvent>) -> BoxFuture<'static, Result<Vec<Option<u64>>>>;
    /// Up to `limit` events after `after`, in order.
    fn read(&self, after: u64, limit: usize) -> BoxFuture<'static, Result<Vec<StoredEvent>>>;
    fn get(&self, seq: u64) -> BoxFuture<'static, Result<Option<StoredEvent>>>;
    /// (first, last) seq kept; (0, 0) when there's none.
    fn bounds(&self) -> BoxFuture<'static, Result<(u64, u64)>>;
    fn cursor(&self, plugin: &str, name: &str) -> BoxFuture<'static, Result<Option<u64>>>;
    fn set_cursor(&self, plugin: &str, name: &str, seq: u64) -> BoxFuture<'static, Result<()>>;
    /// Sets several cursors (plugin, subscription, seq), one write when the log can (ADR-0088; additive, 2026-11-16).
    fn set_cursors(&self, rows: Vec<(String, String, u64)>) -> BoxFuture<'static, Result<()>> {
        let each: Vec<_> = rows.iter().map(|(p, n, seq)| self.set_cursor(p, n, *seq)).collect();
        Box::pin(async move {
            for f in each {
                f.await?;
            }
            Ok(())
        })
    }
    fn park(&self, p: Parked) -> BoxFuture<'static, Result<()>>;
    fn parked(&self) -> BoxFuture<'static, Result<Vec<Parked>>>;
    fn unpark(&self, plugin: &str, name: &str, seq: u64) -> BoxFuture<'static, Result<bool>>;
}

/// The log as a kernel service.
#[derive(Clone)]
pub struct EventLogService(pub Arc<dyn EventLog>);

impl std::fmt::Debug for EventLogService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EventLogService")
    }
}

/// A durable subscription's delivery.
#[derive(Debug, Clone)]
pub enum Durable<E> {
    Event {
        event: E,
        /// its seq in the log (dedupe with it)
        seq: u64,
        key: Option<String>,
        envelope: Arc<EventEnvelope>,
    },
    /// The cursor fell off the retention window: events `from..=to` are gone; re-read current state.
    Gap { from: u64, to: u64 },
}

// --- the scope a command or handler runs in -------------------------------------------------------------

/// Where a handler's cursor goes once a write commits.
#[derive(Debug)]
pub(crate) struct CursorMark {
    plugin: String,
    name: String,
    seq: u64,
    written: AtomicBool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Scope {
    pub(crate) tx: Option<Arc<RootTx>>,
    pub(crate) cursor: Option<Arc<CursorMark>>,
}

tokio::task_local! {
    static SCOPE: Scope;
}

pub(crate) fn current() -> Scope {
    SCOPE.try_with(Clone::clone).unwrap_or_default()
}

/// Runs `fut` with this scope (the command's root, the handler's cursor).
pub(crate) async fn scoped<T>(s: Scope, fut: impl Future<Output = T>) -> T {
    SCOPE.scope(s, fut).await
}

/// What one store write carries along: the command's durable events emitted so far and not yet in the
/// log, the cursor of the handler it runs in, and (ADR-0075) the command's buffered events, staged with the write
/// so whoever reads its rows finds them queued. Taken just before the write, staged inside it, settled after it.
#[derive(Debug)]
pub struct InTx {
    tx: Option<Arc<RootTx>>,
    /// marks the buffered durable events this write logs (`Pending::taken`)
    token: u64,
    rows: Vec<NewEvent>,
    cursor: Option<Arc<CursorMark>>,
    kernel: Weak<KernelState>,
}

static TOKENS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// For the store, around each write: None when nothing rides on it (not inside a command, no handler cursor).
/// Inside a command it's always Some: the write stages the command's events (ADR-0075).
pub fn take() -> Option<InTx> {
    let s = current();
    let token = TOKENS.fetch_add(1, Ordering::Relaxed);
    let (rows, kernel) = match &s.tx {
        Some(tx) => {
            let mut ev = tx.events.lock().unwrap_or_else(PoisonError::into_inner);
            let mut rows = Vec::new();
            for p in ev.iter_mut() {
                if p.durable && p.written == Logged::Not && p.taken == 0 {
                    p.taken = token;
                    rows.push(p.row());
                }
            }
            (rows, tx.kernel.clone())
        }
        None => (Vec::new(), Weak::new()),
    };
    let cursor = s.cursor.filter(|c| !c.written.load(Ordering::SeqCst));
    if s.tx.is_none() && cursor.is_none() {
        return None;
    }
    Some(InTx { tx: s.tx, token, rows, cursor, kernel })
}

impl InTx {
    pub fn rows(&self) -> &[NewEvent] {
        &self.rows
    }
    /// (plugin, subscription, seq) to set in `event_cursor` in the same transaction.
    pub fn cursor(&self) -> Option<(&str, &str, u64)> {
        self.cursor.as_ref().map(|c| (c.plugin.as_str(), c.name.as_str(), c.seq))
    }
    /// For the store, inside the write's transaction after its last op and the log append (`seqs`: what the log
    /// answered for `rows`), just before COMMIT (ADR-0075): the command's buffered events, from the first on, up
    /// to the first durable one this write doesn't log (emitted during it: per-source order, it and what follows
    /// wait for the next write or the return), are queued held, with `head` as their cursor. The store settles
    /// what this returns on the writer thread right after COMMIT (they go out; the durable ones this write logged
    /// reach durable subscribers) or ROLLBACK (dropped unseen and back in the buffer, unlogged). None outside a
    /// command.
    pub fn stage(&self, head: Option<u64>, seqs: &[Option<u64>]) -> Option<Staged> {
        let tx = self.tx.as_ref()?;
        let k = self.kernel.upgrade()?;
        let mut fresh = Vec::new();
        let drained: Vec<Pending> = {
            let mut ev = tx.events.lock().unwrap_or_else(PoisonError::into_inner);
            for (n, p) in ev.iter_mut().filter(|p| p.taken == self.token).enumerate() {
                p.written = seqs.get(n).copied().flatten().map_or(Logged::Repeat, Logged::At);
                if let Logged::At(seq) = p.written {
                    fresh.push((seq, p.clone()));
                }
            }
            let cut = ev.iter().position(|p| p.durable && p.written == Logged::Not).unwrap_or(ev.len());
            ev.drain(..cut).collect()
        };
        // a durable repeat (same type and key within 24 h) is the same fact: not published again
        let out: Vec<Pending> = drained.iter().filter(|p| p.written != Logged::Repeat).cloned().collect();
        let root = out.first().map(|p| p.root.clone()).unwrap_or_default();
        let mut st = k.bus.stage(&k, &root, head, out);
        st.fresh = fresh;
        st.kernel = Arc::downgrade(&k);
        st.undo = Some(crate::events::Undo { tx: tx.clone(), token: self.token, drained });
        Some(st)
    }
    /// The write committed: the handler's cursor went with it.
    pub fn committed(&self) {
        if let Some(c) = &self.cursor {
            c.written.store(true, Ordering::SeqCst);
        }
        self.release(false);
    }
    /// The write rolled back: its events wait for the next write (or the command's end), its cursor too. (When it
    /// got as far as `stage`, the `Staged` already put them back, on the writer thread.)
    pub fn rolled_back(&self) {
        self.release(true);
    }
    fn release(&self, unlog: bool) {
        let Some(tx) = &self.tx else { return };
        let mut ev = tx.events.lock().unwrap_or_else(PoisonError::into_inner);
        for p in ev.iter_mut().filter(|p| p.taken == self.token) {
            p.taken = 0;
            if unlog {
                p.written = Logged::Not;
            }
        }
    }
}

impl Pending {
    pub(crate) fn row(&self) -> NewEvent {
        NewEvent {
            ty: self.ty.clone(),
            plugin: self.plugin.to_string(),
            subject: self.subject.clone(),
            key: self.key.clone(),
            cause: self.cause,
            root: self.root.clone(),
            at: self.at,
            data: match &self.data {
                Payload::Typed(t) => t.json(),
                Payload::Json(v) => (**v).clone(),
            },
        }
    }
}

/// The durable events of a root command still not in the log go in now, in one transaction (the command
/// returned Ok and made no write after emitting them).
pub(crate) async fn append_rest(k: &Arc<KernelState>, tx: &Arc<RootTx>) {
    let Some(log) = k.event_log() else { return };
    // marked with a token, not by position: a write still in flight (its future dropped, its job queued) may drain
    // the buffer's front while this awaits; the events it carries (`taken`) are its to log and stage
    let token = TOKENS.fetch_add(1, Ordering::Relaxed);
    let rows: Vec<NewEvent> = {
        let mut ev = tx.events.lock().unwrap_or_else(PoisonError::into_inner);
        ev.iter_mut()
            .filter(|p| p.durable && p.written == Logged::Not && p.taken == 0)
            .map(|p| {
                p.taken = token;
                p.row()
            })
            .collect()
    };
    if rows.is_empty() {
        return;
    }
    let r = log.0.append(rows).await;
    let mut fresh = Vec::new();
    {
        let mut ev = tx.events.lock().unwrap_or_else(PoisonError::into_inner);
        for (n, p) in ev.iter_mut().filter(|p| p.taken == token).enumerate() {
            p.taken = 0;
            if let Ok(seqs) = &r {
                p.written = seqs.get(n).copied().flatten().map_or(Logged::Repeat, Logged::At);
                if let Logged::At(seq) = p.written {
                    fresh.push((seq, p.clone()));
                }
            }
        }
    }
    match r {
        Ok(_) => {
            k.bus.remember(k, &fresh);
        }
        Err(e) => {
            tracing::warn!("durable events not kept: {}", e.message);
            k.diag.record(k.now_ms(), "wisp.kernel", "event", "internal", format!("durable events not kept: {}", e.message), Vec::new());
        }
    }
}

// --- the ring and the subscriptions ---------------------------------------------------------------------

pub(crate) type DurableFn = Arc<dyn Fn(Ctx, Call, Arc<EventEnvelope>) -> BoxFuture<'static, Result<()>> + Send + Sync>;
pub(crate) type BatchFn = Arc<dyn Fn(Ctx, Call, Vec<Arc<EventEnvelope>>) -> BoxFuture<'static, Result<()>> + Send + Sync>;

pub(crate) struct DSub {
    pub(crate) name: String,
    pub(crate) pattern: String,
    pub(crate) f: DurableFn,
    /// a batch subscription's handler (`f` is it called with one)
    pub(crate) batch: Option<BatchFn>,
    pub(crate) ctx: Ctx,
}

/// Idle cursors waiting for their one write (ADR-0088).
#[derive(Default)]
pub(crate) struct Saver {
    dirty: Mutex<BTreeMap<(String, String), u64>>,
    armed: AtomicBool,
    /// one save at a time: a flush that finds nothing waiting still returns only once a save in flight landed
    writing: tokio::sync::Mutex<()>,
}

impl Saver {
    /// A worker's cursor moved past what's saved and no write carried it: kept here until the next save (with the
    /// others, or when it stops), so a stop saves what it delivered.
    fn note(&self, plugin: &str, name: &str, seq: u64) {
        let mut d = self.dirty.lock().unwrap_or_else(PoisonError::into_inner);
        let at = d.entry((plugin.to_owned(), name.to_owned())).or_insert(seq);
        *at = (*at).max(seq);
    }
    /// A worker's cursor moved: `note`d, or (`carried`: its handler's write carried it) nothing waits for it.
    fn moved(&self, plugin: &str, name: &str, seq: u64, carried: bool) {
        if carried {
            self.dirty.lock().unwrap_or_else(PoisonError::into_inner).remove(&(plugin.to_owned(), name.to_owned()));
        } else {
            self.note(plugin, name, seq);
        }
    }
    /// What waits for it (a worker started again before its last save landed resumes from here).
    fn pending(&self, plugin: &str, name: &str) -> Option<u64> {
        self.dirty.lock().unwrap_or_else(PoisonError::into_inner).get(&(plugin.to_owned(), name.to_owned())).copied()
    }
}

/// A caught-up subscription's cursor, saved with the others within `SAVE_WITHIN_MS`.
fn mark(k: &Arc<KernelState>, plugin: &str, name: &str, seq: u64) {
    k.bus.ring.saver.note(plugin, name, seq);
    arm(k);
}

/// The next save, `SAVE_WITHIN_MS` from now, unless one is already due.
fn arm(k: &Arc<KernelState>) {
    if !k.bus.ring.saver.armed.swap(true, Ordering::SeqCst) {
        let (w, nap) = (Arc::downgrade(k), k.sleep(SAVE_WITHIN_MS));
        tokio::spawn(async move {
            nap.await;
            if let Some(k) = w.upgrade() {
                flush(&k).await;
            }
        });
    }
}

/// Every waiting cursor, in one write (and when a plugin is switched off, before `set_enabled` returns: wisp's stop
/// switches every plugin off before it closes the store).
pub(crate) async fn flush(k: &Arc<KernelState>) {
    let sv = &k.bus.ring.saver;
    let _one = sv.writing.lock().await;
    // disarmed before taking: a cursor marked after this arms the next write
    sv.armed.store(false, Ordering::SeqCst);
    let rows: Vec<(String, String, u64)> =
        std::mem::take(&mut *sv.dirty.lock().unwrap_or_else(PoisonError::into_inner)).into_iter().map(|((p, n), s)| (p, n, s)).collect();
    let Some(log) = k.event_log().filter(|_| !rows.is_empty()) else { return };
    if let Err(e) = log.0.set_cursors(rows.clone()).await {
        tracing::warn!(cursors = rows.len(), "durable cursors not saved (the next save tries again): {}", e.message);
        for (p, n, seq) in rows {
            sv.note(&p, &n, seq);
        }
        arm(k);
    }
}

pub(crate) struct Ring {
    items: Mutex<BTreeMap<u64, Arc<EventEnvelope>>>,
    head: watch::Sender<u64>,
    workers: Mutex<HashMap<String, Vec<CancellationToken>>>,
    /// each worker's "counted as busy" flag: the head moving makes idle ones busy (in `Work`) under this
    /// lock, so `Kernel::idle` never resolves between an event and its delivery
    busy: Mutex<Vec<Arc<AtomicBool>>>,
    work: Arc<crate::state::Work>,
    saver: Saver,
}

impl Ring {
    pub(crate) fn new(work: Arc<crate::state::Work>) -> Self {
        Self {
            items: Mutex::default(),
            head: watch::channel(0).0,
            workers: Mutex::default(),
            busy: Mutex::default(),
            work,
            saver: Saver::default(),
        }
    }
    fn wake_all(&self, flags: &[Arc<AtomicBool>]) {
        for f in flags {
            if !f.swap(true, Ordering::SeqCst) {
                self.work.add(1);
            }
        }
    }
    /// The head goes up to `to` (if it's higher), and idle workers count as busy.
    fn raise(&self, to: u64) {
        let flags = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        let up = self.head.send_if_modified(|h| {
            let up = to > *h;
            if up {
                *h = to;
            }
            up
        });
        if up {
            self.wake_all(&flags);
        }
    }
    /// A worker caught up at `cursor` goes idle, unless the head moved meanwhile. → whether it did.
    fn rest(&self, flag: &AtomicBool, cursor: u64) -> bool {
        let _g = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        if self.head() > cursor {
            return false;
        }
        if flag.swap(false, Ordering::SeqCst) {
            self.work.done(1);
        }
        true
    }
}

impl Ring {
    /// A commit's durable events, the head raised once to the newest.
    pub(crate) fn insert(&self, envs: Vec<Arc<EventEnvelope>>) {
        let mut top = None;
        {
            let mut m = self.items.lock().unwrap_or_else(PoisonError::into_inner);
            for env in envs {
                let Some(seq) = env.durable_seq else { continue };
                top = top.max(Some(seq));
                m.insert(seq, env);
            }
            while m.len() > RING {
                m.pop_first();
            }
        }
        if let Some(seq) = top {
            self.raise(seq);
        }
    }
    fn first_after(&self, seq: u64) -> Option<u64> {
        self.items.lock().unwrap_or_else(PoisonError::into_inner).range(seq + 1..).next().map(|(s, _)| *s)
    }
    fn get(&self, seq: u64) -> Option<Arc<EventEnvelope>> {
        self.items.lock().unwrap_or_else(PoisonError::into_inner).get(&seq).cloned()
    }
    pub(crate) fn head_watch(&self) -> watch::Receiver<u64> {
        self.head.subscribe()
    }
    pub(crate) fn head(&self) -> u64 {
        *self.head.borrow()
    }
    pub(crate) fn raise_head(&self, to: u64) {
        self.raise(to);
    }
    pub(crate) fn stop_owner(&self, owner: &str) {
        for t in self.workers.lock().unwrap_or_else(PoisonError::into_inner).remove(owner).unwrap_or_default() {
            t.cancel();
        }
    }
}

fn envelope_of(s: &StoredEvent, depth: u32) -> Arc<EventEnvelope> {
    Arc::new(EventEnvelope {
        seq: 0,
        ty: s.event.ty.clone(),
        plugin: s.event.plugin.clone(),
        at: s.event.at,
        root: s.event.root.clone(),
        cursor: None,
        durable_seq: Some(s.seq),
        subject: s.event.subject.clone(),
        key: s.event.key.clone(),
        cause: s.event.cause,
        depth,
        data: Payload::Json(Arc::new(s.event.data.clone())),
    })
}

/// How deep `seq`'s chain of causes goes (0: nothing caused it), at most 8.
pub(crate) async fn depth_of(log: &EventLogService, mut cause: Option<u64>) -> u32 {
    let mut d = 0;
    while let Some(c) = cause {
        if d >= 8 {
            break;
        }
        d += 1;
        cause = log.0.get(c).await.ok().flatten().and_then(|e| e.event.cause);
    }
    d
}

/// Starts a durable subscription's worker.
pub(crate) fn start(k: &Arc<KernelState>, ctx: &Ctx, sub: DSub) {
    let stop = ctx.scope().child_token();
    k.bus.ring.workers.lock().unwrap_or_else(PoisonError::into_inner).entry(ctx.plugin().to_owned()).or_default().push(stop.clone());
    let flag = Arc::new(AtomicBool::new(true));
    k.bus.ring.busy.lock().unwrap_or_else(PoisonError::into_inner).push(flag.clone());
    k.work.add(1);
    let w = Arc::downgrade(k);
    let work = k.work.clone();
    // a new subscription starts here: what's emitted after it registers is its
    let from = k.bus.ring.head();
    tokio::spawn(async move {
        worker(w.clone(), sub, stop, flag.clone(), from).await;
        // gone: out of the busy list (under its lock), and settled if it was counted
        if let Some(k) = w.upgrade() {
            k.bus.ring.busy.lock().unwrap_or_else(PoisonError::into_inner).retain(|f| !Arc::ptr_eq(f, &flag));
        }
        if flag.swap(false, Ordering::SeqCst) {
            work.done(1);
        }
    });
}

async fn worker(k: Weak<KernelState>, sub: DSub, stop: CancellationToken, flag: Arc<AtomicBool>, from: u64) {
    let Some(kk) = k.upgrade() else { return };
    let owner = sub.ctx.plugin().to_owned();
    let log = kk.event_log();
    let mut head_rx = kk.bus.ring.head.subscribe();
    let stored = if let Some(l) = &log { l.0.cursor(&owner, &sub.name).await.ok().flatten() } else { None };
    // a new subscription starts from now; a known one from where it stopped (its last save may not have landed yet)
    let mut cursor = stored.max(kk.bus.ring.saver.pending(&owner, &sub.name)).unwrap_or(from);
    let mut persisted = stored;
    drop(kk);
    let mut batch: VecDeque<Arc<EventEnvelope>> = VecDeque::new();
    let mut unsaved = 0u32;
    // the work counter counts this worker as busy only while it's behind
    loop {
        if stop.is_cancelled() {
            stopped(&k, &owner, &sub.name, log.is_some() && persisted != Some(cursor), cursor).await;
            return;
        }
        let Some(kk) = k.upgrade() else { return };
        let head = kk.bus.ring.head();
        if cursor >= head {
            if log.is_some() && persisted != Some(cursor) {
                // saved with every other caught-up cursor, in one write (ADR-0088)
                mark(&kk, &owner, &sub.name, cursor);
                (persisted, unsaved) = (Some(cursor), 0);
            }
            if !kk.bus.ring.rest(&flag, cursor) {
                continue;
            }
            drop(kk);
            let woke = tokio::select! {
                () = stop.cancelled() => false,
                r = head_rx.changed() => r.is_ok(),
            };
            // busy again (the head's raise may have counted it already)
            if !flag.swap(true, Ordering::SeqCst)
                && let Some(k) = k.upgrade()
            {
                k.work.add(1);
            }
            if !woke {
                // the loop's top saves what it delivered and ends it
                stop.cancel();
            }
            continue;
        }
        let next = cursor + 1;
        let env = if let Some(e) = batch.front().filter(|e| e.durable_seq == Some(next)).cloned() {
            batch.pop_front();
            Some(e)
        } else if let Some(e) = kk.bus.ring.get(next) {
            Some(e)
        } else if let Some(l) = &log {
            batch.clear();
            let rows = l.0.read(cursor, 256).await.unwrap_or_default();
            match rows.first() {
                Some(first) if first.seq > next => {
                    // fell off the end of the retention window
                    let to = first.seq - 1;
                    kk.diag.record(
                        kk.now_ms(),
                        &owner,
                        "event",
                        "gap",
                        format!("{}: events {next}..={to} are gone (retention)", sub.name),
                        Vec::new(),
                    );
                    let gap = gap_envelope(next, to);
                    let Some(saved) = deliver(&kk, &sub, log.as_ref(), &gap, &stop).await else { continue };
                    (cursor, persisted) = (to, if saved { Some(to) } else { persisted });
                    continue;
                }
                Some(_) => {
                    for r in &rows {
                        let d = if r.event.cause.is_some() { depth_of(l, r.event.cause).await } else { 0 };
                        batch.push_back(envelope_of(r, d));
                    }
                    continue;
                }
                // nothing kept past the cursor (retention took it all)
                None => {
                    cursor = head;
                    continue;
                }
            }
        } else {
            // no log: what the ring no longer has is gone
            cursor = kk.bus.ring.first_after(cursor).map_or(head, |s| s - 1);
            continue;
        };
        let Some(env) = env else { continue };
        let carried = if matches(&sub.pattern, &env.ty) {
            // stopped before it was delivered: not past it
            let Some((last, saved)) = hand(&kk, &sub, log.as_ref(), env, next, &mut batch, &stop).await else { continue };
            let n = u32::try_from(last - cursor).unwrap_or(u32::MAX);
            cursor = last;
            (persisted, unsaved) = if saved { (Some(cursor), 0) } else { (persisted, unsaved.saturating_add(n)) };
            saved
        } else {
            cursor = next;
            false
        };
        // where it is, for the next save; a cursor its handler's write carried has nothing waiting
        kk.bus.ring.saver.moved(&owner, &sub.name, cursor, carried);
        if unsaved >= SAVE_EVERY && log.is_some() {
            mark(&kk, &owner, &sub.name, cursor);
            (persisted, unsaved) = (Some(cursor), 0);
        }
    }
}

/// The next event (seq `next`) delivered, and to a batch subscription those already in after it. → the last one's
/// seq and whether its cursor reached the log with it; None when stopped before it was delivered.
async fn hand(
    k: &Arc<KernelState>,
    sub: &DSub,
    log: Option<&EventLogService>,
    env: Arc<EventEnvelope>,
    next: u64,
    queue: &mut VecDeque<Arc<EventEnvelope>>,
    stop: &CancellationToken,
) -> Option<(u64, bool)> {
    if sub.batch.is_none() {
        return deliver(k, sub, log, &env, stop).await.map(|saved| (next, saved));
    }
    let envs = gather(env, next, k.bus.ring.head(), queue, &k.bus.ring, &sub.pattern);
    let last = next + envs.len() as u64 - 1;
    deliver_batch(k, sub, log, envs, stop).await.map(|saved| (last, saved))
}

/// Stopped (switched off, wisp stopping): what it delivered is saved now (`unsaved`: its cursor isn't yet), with
/// whatever else waits.
async fn stopped(k: &Weak<KernelState>, owner: &str, name: &str, unsaved: bool, cursor: u64) {
    let Some(k) = k.upgrade() else { return };
    if unsaved {
        mark(&k, owner, name, cursor);
    }
    flush(&k).await;
}

/// `env` (seq `next`) and the events already in after it, matching, one after another, up to `BATCH`, from the
/// worker's queue or the ring: one call, one write (ADR-0088).
fn gather(
    env: Arc<EventEnvelope>,
    next: u64,
    head: u64,
    queue: &mut VecDeque<Arc<EventEnvelope>>,
    ring: &Ring,
    pattern: &str,
) -> Vec<Arc<EventEnvelope>> {
    let mut envs = vec![env];
    while envs.len() < BATCH {
        let s = next + envs.len() as u64;
        if s > head {
            break;
        }
        let queued = queue.front().is_some_and(|e| e.durable_seq == Some(s));
        let e = if queued { queue.front().cloned() } else { ring.get(s) };
        let Some(e) = e.filter(|e| matches(pattern, &e.ty)) else { break };
        if queued {
            queue.pop_front();
        }
        envs.push(e);
    }
    envs
}

/// `a|b*`: any of the alternatives.
pub(crate) fn matches(pattern: &str, ty: &str) -> bool {
    pattern.split('|').any(|p| glob(p, ty))
}

fn gap_envelope(from: u64, to: u64) -> Arc<EventEnvelope> {
    Arc::new(EventEnvelope {
        seq: 0,
        ty: GAP.into(),
        plugin: "wisp.kernel".into(),
        at: 0,
        root: String::new(),
        cursor: None,
        durable_seq: Some(to),
        subject: None,
        key: None,
        cause: None,
        depth: 0,
        data: Payload::Json(Arc::new(serde_json::json!({ "from": from, "to": to }))),
    })
}

/// Several events to a batch subscription in one call, its cursor riding its write at the last of them (ADR-0088);
/// a batch that fails without having written goes again one by one. → as `deliver`.
async fn deliver_batch(
    k: &Arc<KernelState>,
    sub: &DSub,
    log: Option<&EventLogService>,
    envs: Vec<Arc<EventEnvelope>>,
    stop: &CancellationToken,
) -> Option<bool> {
    let (Some(f), [_, _, ..]) = (sub.batch.clone(), envs.as_slice()) else {
        return match envs.first() {
            Some(e) => deliver(k, sub, log, e, stop).await,
            None => Some(false),
        };
    };
    let owner = sub.ctx.plugin().to_owned();
    let seq = envs.last().and_then(|e| e.durable_seq).unwrap_or(0);
    let mark = Arc::new(CursorMark { plugin: owner.clone(), name: sub.name.clone(), seq, written: AtomicBool::new(false) });
    let mut inner = CallInner::detached(k, Caller::plugin(&owner), Some(Arc::from(owner.as_str())), stop.child_token());
    inner.cause = Some(seq);
    inner.cause_depth = envs.iter().map(|e| e.depth).max().unwrap_or(0) + 1;
    let call = inner.into_call();
    let fut = std::panic::AssertUnwindSafe(scoped(Scope { tx: None, cursor: Some(mark.clone()) }, f(sub.ctx.clone(), call, envs.clone())))
        .catch_unwind();
    let r = tokio::select! {
        () = stop.cancelled() => return None,
        r = fut => r,
    };
    if matches!(r, Ok(Ok(()))) {
        return Some(mark.written.load(Ordering::SeqCst));
    }
    if mark.written.load(Ordering::SeqCst) {
        k.diag.record(
            k.now_ms(),
            &owner,
            "event",
            "internal",
            format!("{} batch up to seq {seq} failed after its write", sub.name),
            Vec::new(),
        );
        return Some(true);
    }
    let mut saved = false;
    for e in &envs {
        saved = deliver(k, sub, log, e, stop).await?;
    }
    Some(saved)
}

/// One event to one subscription: 3 tries in its cursor scope, then parked with why. → None when stopped before it
/// was delivered, else whether its cursor reached the log with it (its handler's write, or parked).
async fn deliver(
    k: &Arc<KernelState>,
    sub: &DSub,
    log: Option<&EventLogService>,
    env: &Arc<EventEnvelope>,
    stop: &CancellationToken,
) -> Option<bool> {
    let owner = sub.ctx.plugin().to_owned();
    let seq = env.durable_seq.unwrap_or(0);
    let mut why = String::new();
    for _ in 0..ATTEMPTS {
        if stop.is_cancelled() {
            return None;
        }
        let mark = Arc::new(CursorMark { plugin: owner.clone(), name: sub.name.clone(), seq, written: AtomicBool::new(false) });
        let mut inner = CallInner::detached(k, Caller::plugin(&owner), Some(Arc::from(owner.as_str())), stop.child_token());
        inner.cause = Some(seq);
        inner.cause_depth = env.depth + 1;
        let call = inner.into_call();
        let fut = std::panic::AssertUnwindSafe(scoped(
            Scope { tx: None, cursor: Some(mark.clone()) },
            (sub.f)(sub.ctx.clone(), call, env.clone()),
        ))
        .catch_unwind();
        let r = tokio::select! {
            () = stop.cancelled() => return None,
            r = fut => r,
        };
        match r {
            // a handler that wrote nothing has its cursor saved by the worker (every 256, and with the others once caught up, ADR-0088):
            // it changed nothing a replay could repeat, and handlers stay idempotent (the envelope has seq)
            Ok(Ok(())) => return Some(mark.written.load(Ordering::SeqCst)),
            Ok(Err(e)) => why = e.message,
            Err(_) => why = "the subscriber panicked".into(),
        }
        // its write committed with the cursor: the effect happened, and running it again would repeat it
        if mark.written.load(Ordering::SeqCst) {
            k.diag.record(
                k.now_ms(),
                &owner,
                "event",
                "internal",
                format!("{} {} (seq {seq}) failed after its write: {why}", sub.name, env.ty),
                Vec::new(),
            );
            return Some(true);
        }
    }
    tracing::warn!(plugin = %owner, sub = %sub.name, seq, "durable subscriber failed {ATTEMPTS} times, parked: {why}");
    k.diag.record(k.now_ms(), &owner, "event", "parked", format!("{} {} (seq {seq}): {why}", sub.name, env.ty), Vec::new());
    if let Some(l) = log {
        let _ = l.0.park(Parked { plugin: owner.clone(), name: sub.name.clone(), seq, why, at: k.now_ms() }).await;
        return Some(l.0.set_cursor(&owner, &sub.name, seq).await.is_ok());
    }
    Some(false)
}

/// `wisp events retry <seq>`: a parked event delivered again to the subscription that parked it.
pub(crate) async fn retry(k: &Arc<KernelState>, seq: u64) -> Result<usize> {
    let log = k.event_log().ok_or_else(|| WireError::unavailable("no event log"))?;
    let parked: Vec<Parked> = log.0.parked().await?.into_iter().filter(|p| p.seq == seq).collect();
    if parked.is_empty() {
        return Err(WireError::not_found(format!("event {seq} isn't parked")));
    }
    let ev = log.0.get(seq).await?.ok_or_else(|| WireError::not_found(format!("event {seq} isn't kept any more")))?;
    let env = envelope_of(&ev, depth_of(&log, ev.event.cause).await);
    let mut n = 0;
    for p in parked {
        let subs = k.bus.durables.lock().unwrap_or_else(PoisonError::into_inner).get(&(p.plugin.clone(), p.name.clone())).cloned();
        let Some(sub) = subs else { continue };
        log.0.unpark(&p.plugin, &p.name, seq).await?;
        deliver(k, &sub, Some(&log), &env, &sub.ctx.scope().child_token()).await;
        n += 1;
    }
    Ok(n)
}

impl Clone for DSub {
    fn clone(&self) -> Self {
        Self { name: self.name.clone(), pattern: self.pattern.clone(), f: self.f.clone(), batch: self.batch.clone(), ctx: self.ctx.clone() }
    }
}

/// The typed form of a durable delivery.
pub(crate) fn typed<E: Event>(env: &Arc<EventEnvelope>) -> Option<Durable<E>> {
    if env.ty == GAP {
        let d = env.data();
        return Some(Durable::Gap { from: d["from"].as_u64().unwrap_or(0), to: d["to"].as_u64().unwrap_or(0) });
    }
    env.get::<E>().map(|event| Durable::Event { event, seq: env.durable_seq.unwrap_or(0), key: env.key.clone(), envelope: env.clone() })
}
