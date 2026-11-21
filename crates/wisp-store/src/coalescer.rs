//! Write coalescing (plan §4.1, "write cadence"): token deltas and activity events arrive many times a
//! second. Buffered here, they commit once per harness step or 250 ms after the first one at most, in
//! one transaction, so a crash loses at most 250 ms of text (which the harness can replay) and the bytes
//! written stay within the 15× budget (one event per transaction costs ~113×, ten per transaction ~9×).
//!
//! Patches to one row are concatenated and committed as one net diff; appends of a kind are grouped per
//! parent into one change row. The window is `interval_ms` (250 by default). The timer is the store's injected
//! clock, so tests drive it by hand.
//!
//! One coalescer may serve many writers (wisp.runtime shares one across every running task, ADR-0083): their
//! writes in a window commit together, so the per-commit cost (the change feed's tail pages, the indexes'
//! leaves, page 1) is paid once a window, not once per task.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use crate::entity::{Id, Row};
use crate::error::{Result, StoreError};
use crate::json_patch::PatchOp;
use crate::store::Store;
use crate::writer::Writer;

/// One buffered write, as the error callback sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum Buffered {
    Append { kind: String, rows: Vec<Row> },
    Patch { kind: String, id: Id, ops: Vec<PatchOp> },
}

pub type OnError = Arc<dyn Fn(&StoreError, &Buffered) + Send + Sync>;

pub struct CoalescerOptions {
    pub interval_ms: u64,
    /// a buffered write the store refused (its row was deleted meanwhile, …); the rest still commit
    pub on_error: OnError,
}

impl Default for CoalescerOptions {
    fn default() -> Self {
        Self { interval_ms: 250, on_error: Arc::new(|e, item| tracing::warn!(error = %e, ?item, "a coalesced write was refused")) }
    }
}

impl std::fmt::Debug for CoalescerOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoalescerOptions").field("interval_ms", &self.interval_ms).finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Buf {
    items: Vec<Buffered>,
    index: HashMap<(bool, String, String), usize>,
    /// the generation of the armed timer; a timer whose generation is gone does nothing
    armed: Option<u64>,
    generation: u64,
    closed: bool,
}

struct Inner {
    store: Store,
    opts: CoalescerOptions,
    buf: Mutex<Buf>,
    /// one flush at a time, so batches reach the writer in the order they were taken
    flushing: tokio::sync::Mutex<()>,
    runtime: Option<tokio::runtime::Handle>,
}

/// Cheap to clone; every clone feeds the same buffer. Needs a tokio runtime for its timer (without one,
/// only `step`/`flush` commit). Dropping it is safe: an armed timer still flushes. `close` flushes now.
#[derive(Clone)]
pub struct Coalescer {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Coalescer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Coalescer").field("buffered", &self.len()).finish_non_exhaustive()
    }
}

impl Coalescer {
    pub fn new(store: &Store, opts: CoalescerOptions) -> Self {
        let runtime = tokio::runtime::Handle::try_current().ok();
        Self {
            inner: Arc::new(Inner {
                store: store.clone(),
                opts,
                buf: Mutex::new(Buf::default()),
                flushing: tokio::sync::Mutex::new(()),
                runtime,
            }),
        }
    }

    /// rows (patches per row, append batches per kind) waiting to commit
    pub fn len(&self) -> usize {
        self.lock().items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn append(&self, kind: &str, rows: Vec<Row>) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        self.buffer(
            (false, kind.to_owned(), String::new()),
            rows,
            |item, rows| match item {
                Buffered::Append { rows: had, .. } => had.extend(rows),
                Buffered::Patch { .. } => {}
            },
            |rows| Buffered::Append { kind: kind.to_owned(), rows },
        )
    }

    /// Ops for one row; later ops apply after earlier ones, and the change feed gets the net diff.
    pub fn patch(&self, kind: &str, id: impl Into<Id>, ops: Vec<PatchOp>) -> Result<()> {
        if ops.is_empty() {
            return Ok(());
        }
        let id = id.into();
        self.buffer(
            (true, kind.to_owned(), id.to_string()),
            ops,
            |item, ops| match item {
                Buffered::Patch { ops: had, .. } => had.extend(ops),
                Buffered::Append { .. } => {}
            },
            |ops| Buffered::Patch { kind: kind.to_owned(), id, ops },
        )
    }

    fn buffer<V>(
        &self,
        key: (bool, String, String),
        v: V,
        merge: impl FnOnce(&mut Buffered, V),
        make: impl FnOnce(V) -> Buffered,
    ) -> Result<()> {
        let mut b = self.lock();
        if b.closed {
            return Err(StoreError::Invalid("this coalescer was closed".into()));
        }
        if let Some(i) = b.index.get(&key).copied() {
            merge(&mut b.items[i], v);
        } else {
            let i = b.items.len();
            b.items.push(make(v));
            b.index.insert(key, i);
        }
        self.arm(&mut b);
        Ok(())
    }

    fn arm(&self, b: &mut Buf) {
        let Some(rt) = self.inner.runtime.as_ref().filter(|_| b.armed.is_none()) else {
            return;
        };
        b.generation += 1;
        let generation = b.generation;
        b.armed = Some(generation);
        let me = self.clone();
        let sleep = self.inner.store.clock().sleep(self.inner.opts.interval_ms);
        rt.spawn(async move {
            sleep.await;
            if me.lock().armed == Some(generation) {
                me.flush().await;
            }
        });
    }

    /// The end of a harness step: commit what is buffered now.
    pub async fn step(&self) {
        self.flush().await;
    }

    /// Commits everything buffered in one transaction; if the store refuses it, each write commits on its
    /// own and the refused ones go to `on_error`, so one bad write never drops the others.
    pub async fn flush(&self) {
        let _one = self.inner.flushing.lock().await;
        let batch = {
            let mut b = self.lock();
            b.armed = None;
            b.index.clear();
            std::mem::take(&mut b.items)
        };
        if batch.is_empty() {
            return;
        }
        let all = batch.clone();
        if self.inner.store.write(move |w| all.iter().try_for_each(|item| write(w, item))).await.is_ok() {
            return;
        }
        for item in batch {
            let one = item.clone();
            let Err(e) = self.inner.store.write(move |w| write(w, &one)).await else {
                continue;
            };
            match item {
                // a batch of many writers' rows: one refused parent (a deleted task's) never drops the others
                Buffered::Append { kind, rows } if rows.len() > 1 => self.append_by_parent(kind, rows).await,
                item => (self.inner.opts.on_error)(&e, &item),
            }
        }
    }

    /// A refused append, again one parent at a time (each parent's rows in their order, still one change row
    /// each); only a parent the store refuses again goes to `on_error`, whole.
    async fn append_by_parent(&self, kind: String, rows: Vec<Row>) {
        let parent = self.inner.store.shared.kind(&kind).ok().and_then(|k| k.def.parent);
        let mut groups: Vec<(serde_json::Value, Vec<Row>)> = Vec::new();
        for row in rows {
            let key = parent.and_then(|p| row.get(p).cloned()).unwrap_or(serde_json::Value::Null);
            match groups.iter_mut().find(|(g, _)| *g == key) {
                Some((_, list)) => list.push(row),
                None => groups.push((key, vec![row])),
            }
        }
        for (_, rows) in groups {
            let (k, all) = (kind.clone(), rows.clone());
            if let Err(e) = self.inner.store.write(move |w| w.append(&k, all).map(|_| ())).await {
                (self.inner.opts.on_error)(&e, &Buffered::Append { kind: kind.clone(), rows });
            }
        }
    }

    /// Flushes and refuses more.
    pub async fn close(&self) {
        self.lock().closed = true;
        self.flush().await;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Buf> {
        self.inner.buf.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn write(w: &mut Writer<'_>, item: &Buffered) -> Result<()> {
    match item {
        Buffered::Append { kind, rows } => w.append(kind, rows.clone()).map(|_| ()),
        Buffered::Patch { kind, id, ops } => w.patch(kind, id.clone(), ops).map(|_| ()),
    }
}
