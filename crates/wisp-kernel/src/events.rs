//! Events (plan §6.11.2 rule 6): published after the emitting command commits, delivered asynchronously
//! and in order, one queue per subscribing plugin so a slow one only delays itself. At least once: a
//! handler that fails (or panics) gets the same envelope again, up to 3 tries (dedupe by `seq`). Past
//! 10k queued a plugin is `lagging`: its queue stops growing and it catches up from the change feed.
use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

use futures::FutureExt;
use serde::ser::SerializeStruct;
use serde_json::Value;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::host::Ctx;
use crate::perm::glob;
use crate::state::{KernelState, Work};
use crate::{BoxFuture, Event, Result};

pub const QUEUE_CAP: usize = 10_000;
const ATTEMPTS: u32 = 3;

/// An event's payload: the typed value when it was emitted in-process (no JSON for in-process
/// subscribers of the same type), JSON on demand.
pub(crate) trait AnyEvent: Any + Send + Sync {
    fn json(&self) -> Value;
    fn any(&self) -> &dyn Any;
}

impl<E: Event> AnyEvent for E {
    fn json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
    fn any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
pub(crate) enum Payload {
    Typed(Arc<dyn AnyEvent>),
    Json(Arc<Value>),
}

impl std::fmt::Debug for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Typed(_) => "Typed",
            Self::Json(_) => "Json",
        })
    }
}

/// An event waiting for its command to commit.
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub(crate) ty: String,
    pub(crate) plugin: Arc<str>,
    pub(crate) data: Payload,
    /// kept in the log (ADR-0044)
    pub(crate) durable: bool,
    pub(crate) subject: Option<String>,
    pub(crate) key: Option<String>,
    /// the durable event whose handler emitted it, and how deep that chain is
    pub(crate) cause: Option<u64>,
    pub(crate) depth: u32,
    pub(crate) root: String,
    pub(crate) at: i64,
    /// durable: whether it's in the log yet (a repeat within 24 h is never published)
    pub(crate) written: Logged,
    /// the write in flight that carries it (`durable::InTx`'s token; 0: none)
    pub(crate) taken: u64,
}

/// Whether a durable event is in the log (ADR-0044).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Logged {
    Not,
    /// a repeat of its (type, key) within 24 h: recorded once, so not again (and not published)
    Repeat,
    At(u64),
}

impl Pending {
    /// An ephemeral event (the kernel's own, a vetoed notice).
    pub(crate) fn plain(ty: &str, plugin: Arc<str>, data: Payload) -> Self {
        Self {
            ty: ty.to_owned(),
            plugin,
            data,
            durable: false,
            subject: None,
            key: None,
            cause: None,
            depth: 0,
            root: String::new(),
            at: 0,
            written: Logged::Not,
            taken: 0,
        }
    }
}

/// A published event as subscribers see it.
#[derive(Debug, Clone)]
pub struct EventEnvelope {
    /// the kernel's publish order, for deduplication
    pub seq: u64,
    pub ty: String,
    /// the plugin that emitted it
    pub plugin: String,
    pub at: i64,
    /// the root call it came from
    pub root: String,
    /// the store's change cursor when the store published it on commit
    pub cursor: Option<u64>,
    /// a durable event's seq in the log (ADR-0044); None for an ephemeral one
    pub durable_seq: Option<u64>,
    /// what it's about (a task, a PR)
    pub subject: Option<String>,
    /// its dedupe key
    pub key: Option<String>,
    /// the durable event whose handler emitted it
    pub cause: Option<u64>,
    /// how many causes deep it is (0: nothing caused it)
    pub depth: u32,
    pub(crate) data: Payload,
}

impl EventEnvelope {
    pub fn data(&self) -> Value {
        match &self.data {
            Payload::Typed(t) => t.json(),
            Payload::Json(v) => (**v).clone(),
        }
    }
    /// The payload as `E`: a clone when it was emitted as `E` in this process, else parsed.
    pub fn get<E: Event>(&self) -> Option<E> {
        match &self.data {
            Payload::Typed(t) => t.any().downcast_ref::<E>().cloned().or_else(|| serde_json::from_value(t.json()).ok()),
            Payload::Json(v) => serde_json::from_value((**v).clone()).ok(),
        }
    }
}

impl serde::Serialize for EventEnvelope {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("EventEnvelope", 12)?;
        st.serialize_field("seq", &self.seq)?;
        st.serialize_field("type", &self.ty)?;
        st.serialize_field("plugin", &self.plugin)?;
        st.serialize_field("at", &self.at)?;
        st.serialize_field("root", &self.root)?;
        st.serialize_field("cursor", &self.cursor)?;
        st.serialize_field("durableSeq", &self.durable_seq)?;
        st.serialize_field("subject", &self.subject)?;
        st.serialize_field("key", &self.key)?;
        st.serialize_field("cause", &self.cause)?;
        st.serialize_field("depth", &self.depth)?;
        st.serialize_field("data", &self.data())?;
        st.end()
    }
}

pub(crate) type SubFn = Arc<dyn Fn(Ctx, Arc<EventEnvelope>) -> BoxFuture<'static, Result<()>> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Sub {
    pub(crate) pattern: String,
    pub(crate) f: SubFn,
    pub(crate) ctx: Ctx,
}

/// A staged event's gate (ADR-0074): queued before its write commits, held there until the write settles; then
/// delivered (committed) or dropped unseen (rolled back).
struct Gate(AtomicU8);
const HELD: u8 = 0;
const GO: u8 = 1;
const DROPPED: u8 = 2;

/// One queued event: its envelope, and its gate when a write's commit lets it go.
#[derive(Clone)]
struct Item {
    env: Arc<EventEnvelope>,
    gate: Option<Arc<Gate>>,
}

impl Item {
    fn state(&self) -> u8 {
        self.gate.as_ref().map_or(GO, |g| g.0.load(Ordering::Acquire))
    }
}

struct Queue {
    owner: Arc<str>,
    subs: RwLock<Vec<Sub>>,
    items: Mutex<VecDeque<Item>>,
    wake: Notify,
    /// an event left the queue (its handlers ran): `delivered` waiters look again
    popped: Notify,
    lagging: Mutex<Option<u64>>,
    stop: CancellationToken,
}

/// Which queues want a type: exact types in a map, patterns (`x.*`, `*`) in a list. Rebuilt on
/// (un)subscribe, read as a snapshot on publish.
#[derive(Default)]
struct Index {
    exact: HashMap<String, Vec<Arc<Queue>>>,
    patterns: Vec<(String, Arc<Queue>)>,
}

pub(crate) struct Bus {
    pub(crate) ring: crate::durable::Ring,
    /// durable subscriptions by (plugin, name), for `retry`
    pub(crate) durables: Mutex<HashMap<(String, String), crate::durable::DSub>>,
    seq: AtomicU64,
    queues: Mutex<HashMap<Arc<str>, Arc<Queue>>>,
    index: RwLock<Arc<Index>>,
    work: Arc<Work>,
}

impl Bus {
    pub(crate) fn new(work: Arc<Work>) -> Self {
        Self {
            ring: crate::durable::Ring::new(work.clone()),
            durables: Mutex::default(),
            seq: AtomicU64::new(0),
            queues: Mutex::default(),
            index: RwLock::default(),
            work,
        }
    }

    fn reindex(&self, queues: &HashMap<Arc<str>, Arc<Queue>>) {
        let mut ix = Index::default();
        for q in queues.values() {
            let subs = q.subs.read().map(|s| s.iter().map(|x| x.pattern.clone()).collect::<Vec<_>>()).unwrap_or_default();
            let mut seen: Vec<String> = Vec::new();
            for p in subs {
                if seen.contains(&p) {
                    continue;
                }
                seen.push(p.clone());
                if p.contains('*') {
                    ix.patterns.push((p, q.clone()));
                } else {
                    ix.exact.entry(p).or_default().push(q.clone());
                }
            }
        }
        if let Ok(mut w) = self.index.write() {
            *w = Arc::new(ix);
        }
    }

    /// Adds a subscription for `owner`; its queue and worker start on the first one.
    pub(crate) fn subscribe(&self, k: &Arc<KernelState>, owner: &Arc<str>, sub: Sub, scope: &CancellationToken) {
        let Ok(mut qs) = self.queues.lock() else {
            return;
        };
        let q = qs.entry(owner.clone()).or_insert_with(|| {
            let q = Arc::new(Queue {
                owner: owner.clone(),
                subs: RwLock::default(),
                items: Mutex::default(),
                wake: Notify::new(),
                popped: Notify::new(),
                lagging: Mutex::default(),
                stop: scope.child_token(),
            });
            tokio::spawn(worker(q.clone(), Arc::downgrade(k), self.work.clone()));
            q
        });
        if let Ok(mut s) = q.subs.write() {
            s.push(sub);
        }
        self.reindex(&qs);
    }

    /// Drops `owner`'s queue: its subscriptions and whatever was still queued for it.
    pub(crate) fn drop_owner(&self, owner: &str) -> usize {
        let Ok(mut qs) = self.queues.lock() else {
            return 0;
        };
        let Some(q) = qs.remove(owner) else { return 0 };
        q.stop.cancel();
        let n = q.subs.read().map_or(0, |s| s.len());
        let queued = q.items.lock().map_or(0, |mut i| std::mem::take(&mut *i).len());
        self.work.done(queued);
        self.reindex(&qs);
        n
    }

    pub(crate) fn subscriptions(&self, owner: &str) -> usize {
        self.queues.lock().ok().and_then(|qs| qs.get(owner).map(|q| q.subs.read().map_or(0, |s| s.len()))).unwrap_or(0)
    }

    pub(crate) fn lagging(&self, owner: &str) -> Option<u64> {
        let qs = self.queues.lock().ok()?;
        let q = qs.get(owner)?;
        *q.lagging.lock().ok()?
    }

    /// Waits until `owner`'s handlers have run on every event published before this call (ADR-0073): an event stays
    /// in its queue while its handlers run, so a queue with nothing from before the call is done (all of it looked
    /// at: two publishes can queue out of seq order). Never await it from `owner`'s own event handler: that handler's
    /// event is queued, and it would wait on itself.
    pub(crate) async fn delivered(&self, owner: &str) {
        let upto = self.seq.load(Ordering::Acquire);
        let Some(q) = self.queues.lock().ok().and_then(|qs| qs.get(owner).cloned()) else { return };
        loop {
            let wait = q.popped.notified();
            tokio::pin!(wait);
            wait.as_mut().enable();
            let behind = q.items.lock().is_ok_and(|i| i.iter().any(|e| e.env.seq <= upto));
            if !behind {
                return;
            }
            tokio::select! {
                () = q.stop.cancelled() => return,
                () = wait => {}
            }
        }
    }

    pub(crate) fn caught_up(&self, owner: &str) {
        if let Some(q) = self.queues.lock().ok().and_then(|qs| qs.get(owner).cloned())
            && let Ok(mut l) = q.lagging.lock()
        {
            *l = None;
        }
    }

    /// A commit's durable events are in the log: their durable subscribers may take them now, all at once (the head
    /// moves once, so a worker never wakes between them: a batch subscription gets them together, ADR-0088).
    pub(crate) fn remember(&self, k: &KernelState, fresh: &[(u64, Pending)]) {
        let envs = fresh.iter().map(|(seq, p)| {
            Arc::new(EventEnvelope {
                seq: 0,
                ty: p.ty.clone(),
                plugin: p.plugin.to_string(),
                at: if p.at > 0 { p.at } else { k.now_ms() },
                root: p.root.clone(),
                cursor: None,
                durable_seq: Some(*seq),
                subject: p.subject.clone(),
                key: p.key.clone(),
                cause: p.cause,
                depth: p.depth,
                data: p.data.clone(),
            })
        });
        self.ring.insert(envs.collect());
    }

    /// Publishes one committed event to every queue that wants it. Returns its envelope.
    pub(crate) fn publish(&self, k: &KernelState, root: &str, cursor: Option<u64>, p: Pending) -> Arc<EventEnvelope> {
        self.publish_gated(k, root, cursor, p, None, &mut Vec::new())
    }

    /// Queues a write's events before its COMMIT (ADR-0074): each takes its seq and its place in every queue that
    /// wants it now, held there (never delivered, but waited for by `delivered`) until the write settles. So a
    /// reader who can see the write's rows finds its events already queued.
    pub(crate) fn stage(&self, k: &KernelState, root: &str, cursor: Option<u64>, events: Vec<Pending>) -> Staged {
        let gate = Arc::new(Gate(AtomicU8::new(HELD)));
        let mut queues = Vec::new();
        for p in events {
            self.publish_gated(k, root, cursor, p, Some(&gate), &mut queues);
        }
        Staged { gate, queues, fresh: Vec::new(), kernel: Weak::new(), undo: None, settled: false }
    }

    fn publish_gated(
        &self,
        k: &KernelState,
        root: &str,
        cursor: Option<u64>,
        p: Pending,
        gate: Option<&Arc<Gate>>,
        reached: &mut Vec<Arc<Queue>>,
    ) -> Arc<EventEnvelope> {
        // AcqRel: a reader that saw the staged write commit and then takes `delivered`'s bound sees this seq
        let seq = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
        let env = Arc::new(EventEnvelope {
            seq,
            ty: p.ty,
            plugin: p.plugin.to_string(),
            at: k.now_ms(),
            root: root.to_owned(),
            cursor,
            durable_seq: match p.written {
                Logged::At(s) => Some(s),
                _ => None,
            },
            subject: p.subject,
            key: p.key,
            cause: p.cause,
            depth: p.depth,
            data: p.data,
        });
        let ix = self.index.read().map(|i| i.clone()).unwrap_or_default();
        let mut to = |q: &Arc<Queue>| {
            if self.enqueue(k, q, &env, gate) && gate.is_some() && !reached.iter().any(|x| Arc::ptr_eq(x, q)) {
                reached.push(q.clone());
            }
        };
        if let Some(qs) = ix.exact.get(&env.ty) {
            for q in qs {
                to(q);
            }
        }
        for (pat, q) in &ix.patterns {
            // a queue matched exactly already has it
            if glob(pat, &env.ty) && !ix.exact.get(&env.ty).is_some_and(|l| l.iter().any(|x| Arc::ptr_eq(x, q))) {
                to(q);
            }
        }
        env
    }

    /// → whether it was queued (not past `QUEUE_CAP`).
    fn enqueue(&self, k: &KernelState, q: &Arc<Queue>, env: &Arc<EventEnvelope>, gate: Option<&Arc<Gate>>) -> bool {
        let Ok(mut items) = q.items.lock() else {
            return false;
        };
        if items.len() >= QUEUE_CAP {
            drop(items);
            if let Ok(mut l) = q.lagging.lock()
                && l.is_none()
            {
                *l = Some(env.seq);
                tracing::warn!(plugin = %q.owner, seq = env.seq, "event queue full: lagging, catches up from the change feed");
                k.diag.record(
                    k.now_ms(),
                    &q.owner,
                    "event",
                    "lagging",
                    format!("{} queued events: missed from seq {}", QUEUE_CAP, env.seq),
                    Vec::new(),
                );
            }
            return false;
        }
        items.push_back(Item { env: env.clone(), gate: gate.cloned() });
        drop(items);
        self.work.add(1);
        q.wake.notify_one();
        true
    }
}

/// A write's events, queued and held until the write settles (ADR-0074): `commit` right after its COMMIT lets them
/// be delivered (in the order they were staged, each queue in seq order), `abort` after a ROLLBACK drops them unseen.
/// Dropped unsettled, it aborts: an event whose write isn't known to have committed is never delivered.
pub struct Staged {
    gate: Arc<Gate>,
    queues: Vec<Arc<Queue>>,
    /// durable events now in the log: their durable subscribers take them once the write commits
    pub(crate) fresh: Vec<(u64, Pending)>,
    pub(crate) kernel: Weak<KernelState>,
    /// a command's write (ADR-0075): what it took from its root's buffer, put back if the write rolls back
    pub(crate) undo: Option<Undo>,
    settled: bool,
}

/// What a command's write took from its root's buffer (ADR-0075), settled on the writer thread with the write: on
/// COMMIT the events it carried are gone from the buffer for good; on ROLLBACK they go back to its front, unlogged,
/// for the command's next write or its return (Ok: published then; Err: never).
#[derive(Debug)]
pub(crate) struct Undo {
    pub(crate) tx: Arc<crate::call::RootTx>,
    pub(crate) token: u64,
    pub(crate) drained: Vec<Pending>,
}

impl Undo {
    fn settle(self, committed: bool) {
        let mut ev = self.tx.events.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        for p in ev.iter_mut().filter(|p| p.taken == self.token) {
            p.taken = 0;
            if !committed {
                p.written = Logged::Not;
            }
        }
        if committed {
            return;
        }
        let mut back = self.drained;
        for p in back.iter_mut().filter(|p| p.taken == self.token) {
            p.taken = 0;
            p.written = Logged::Not;
        }
        back.append(&mut ev);
        *ev = back;
    }
}

impl std::fmt::Debug for Staged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Staged").field("queues", &self.queues.len()).field("durable", &self.fresh.len()).finish_non_exhaustive()
    }
}

impl Staged {
    /// Nothing staged (the kernel is gone).
    pub(crate) fn inert() -> Self {
        Self {
            gate: Arc::new(Gate(AtomicU8::new(DROPPED))),
            queues: Vec::new(),
            fresh: Vec::new(),
            kernel: Weak::new(),
            undo: None,
            settled: true,
        }
    }

    /// The write committed: its events go out.
    pub fn commit(mut self) {
        self.settle(GO);
        if let Some(k) = self.kernel.upgrade() {
            k.bus.remember(&k, &std::mem::take(&mut self.fresh));
        }
    }

    /// The write rolled back: its events never happened.
    pub fn abort(mut self) {
        self.settle(DROPPED);
    }

    fn settle(&mut self, to: u8) {
        if self.settled {
            return;
        }
        self.settled = true;
        if let Some(u) = self.undo.take() {
            u.settle(to == GO);
        }
        self.gate.0.store(to, Ordering::Release);
        for q in &self.queues {
            q.wake.notify_one();
        }
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        self.settle(DROPPED);
    }
}

async fn worker(q: Arc<Queue>, k: Weak<KernelState>, work: Arc<Work>) {
    loop {
        let next = q.items.lock().ok().and_then(|i| i.front().cloned());
        let Some(item) = next else {
            tokio::select! {
                () = q.stop.cancelled() => return,
                () = q.wake.notified() => continue,
            }
        };
        match item.state() {
            // its write hasn't settled: everything behind it waits too (per-queue order)
            HELD => {
                tokio::select! {
                    () = q.stop.cancelled() => return,
                    () = q.wake.notified() => continue,
                }
            }
            DROPPED => {}
            _ => {
                let env = &item.env;
                let subs: Vec<Sub> =
                    q.subs.read().map(|s| s.iter().filter(|x| glob(&x.pattern, &env.ty)).cloned().collect()).unwrap_or_default();
                for s in subs {
                    deliver(&q, &k, &s, env).await;
                }
            }
        }
        let popped = q.items.lock().ok().and_then(|mut i| i.pop_front()).is_some();
        if popped {
            q.popped.notify_waiters();
            work.done(1);
        }
    }
}

#[allow(clippy::many_single_char_names)]
async fn deliver(q: &Queue, k: &Weak<KernelState>, s: &Sub, env: &Arc<EventEnvelope>) {
    for attempt in 1..=ATTEMPTS {
        if q.stop.is_cancelled() {
            return;
        }
        let fut = std::panic::AssertUnwindSafe((s.f)(s.ctx.clone(), env.clone())).catch_unwind();
        let why = tokio::select! {
            () = q.stop.cancelled() => return,
            r = fut => match r {
                Ok(Ok(())) => return,
                Ok(Err(e)) => e.message,
                Err(_) => "the subscriber panicked".to_owned(),
            },
        };
        if attempt == ATTEMPTS
            && let Some(k) = k.upgrade()
        {
            tracing::warn!(plugin = %q.owner, ty = %env.ty, seq = env.seq, "subscriber failed {ATTEMPTS} times: {why}");
            k.diag.record(k.now_ms(), &q.owner, "event", "internal", format!("{} (seq {}): {why}", env.ty, env.seq), Vec::new());
        }
    }
}
