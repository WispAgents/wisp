//! A task's runs that are starting (a turn between its claim and its spawn, a /btw), so a delete always finds them:
//! `Live` only exists once a turn's CLI is spawned, and a delete landing before that (its folder made ready,
//! `before:run`, the run parts) stopped nothing: the run spawned its CLI for a deleted task and its writes put the
//! task's row back. Each start claims its task before its first await; a delete cancels every claim on the task and
//! waits for them to let go. A cancelled claim never spawns: [`Claim::unless_cancelled`] holds the claim's flag
//! across the (sync) spawn, so a cancel lands either before it (nothing spawns) or after the run's `Live` entry is in
//! (the delete stops it there).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::watch;

#[derive(Debug, Default)]
struct Inner {
    next: u64,
    by_task: HashMap<String, Vec<Entry>>,
}

#[derive(Debug)]
struct Entry {
    n: u64,
    stop: watch::Sender<bool>,
    ended: watch::Receiver<()>,
}

/// Every task's claims (one `Runtime` keeps one).
#[derive(Debug, Default)]
pub(crate) struct Claims {
    inner: Arc<Mutex<Inner>>,
}

fn lock(m: &Mutex<Inner>) -> std::sync::MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Claims {
    /// `task`'s run starts: its claim, held until the run let go (or never spawned).
    pub(crate) fn claim(&self, task: &str) -> Claim {
        let (stop_tx, stop) = watch::channel(false);
        let (ended_tx, ended) = watch::channel(());
        let mut g = lock(&self.inner);
        g.next += 1;
        let n = g.next;
        g.by_task.entry(task.to_owned()).or_default().push(Entry { n, stop: stop_tx, ended });
        Claim { inner: self.inner.clone(), task: task.to_owned(), n, stop, _ended: ended_tx }
    }

    /// Every claim on `task` cancelled; what to wait on for each to let go (closed when its claim is dropped).
    pub(crate) fn cancel(&self, task: &str) -> Vec<watch::Receiver<()>> {
        // the senders, out of the registry's lock: a `send_replace` waits for a spawn under `unless_cancelled`
        let entries: Vec<(watch::Sender<bool>, watch::Receiver<()>)> =
            lock(&self.inner).by_task.get(task).map(|v| v.iter().map(|e| (e.stop.clone(), e.ended.clone())).collect()).unwrap_or_default();
        entries
            .into_iter()
            .map(|(stop, ended)| {
                stop.send_replace(true);
                ended
            })
            .collect()
    }

    #[cfg(test)]
    fn count(&self, task: &str) -> usize {
        lock(&self.inner).by_task.get(task).map_or(0, Vec::len)
    }
}

/// One starting run's claim on its task: gone (and its waiters told) when dropped, however the start ends.
#[derive(Debug)]
pub(crate) struct Claim {
    inner: Arc<Mutex<Inner>>,
    task: String,
    n: u64,
    stop: watch::Receiver<bool>,
    _ended: watch::Sender<()>,
}

impl Claim {
    /// A delete cancelled it: the run goes no further and writes nothing.
    pub(crate) fn cancelled(&self) -> bool {
        *self.stop.borrow()
    }

    /// What turns true when it's cancelled (a side run's stop).
    pub(crate) fn stop(&self) -> watch::Receiver<bool> {
        self.stop.clone()
    }

    /// `f` (a sync spawn, and the run's `Live` entry) unless cancelled, as one step against [`Claims::cancel`]: the
    /// claim's flag is held while `f` runs, so a cancel waits for it to finish. `f` must not block.
    pub(crate) fn unless_cancelled<T>(&self, f: impl FnOnce() -> T) -> Option<T> {
        let held = self.stop.borrow();
        if *held {
            return None;
        }
        let out = f();
        drop(held);
        Some(out)
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let mut g = lock(&self.inner);
        if let Some(v) = g.by_task.get_mut(&self.task) {
            v.retain(|e| e.n != self.n);
            if v.is_empty() {
                g.by_task.remove(&self.task);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A claim cancelled before its spawn never spawns; one cancelled after it did is waited on until dropped; a
    /// claim's place goes with it, and another task's claims are untouched.
    #[tokio::test]
    async fn a_cancelled_claim_never_spawns_and_is_waited_on_until_it_lets_go() {
        let claims = Claims::default();
        let a = claims.claim("t1");
        let b = claims.claim("t1");
        let other = claims.claim("t2");
        assert_eq!(a.unless_cancelled(|| 1), Some(1), "not cancelled: it spawns");
        let mut waits = claims.cancel("t1");
        assert_eq!(waits.len(), 2);
        assert!(a.cancelled() && b.cancelled() && !other.cancelled());
        assert_eq!(b.unless_cancelled(|| 2), None, "cancelled: nothing spawns");
        assert!(*b.stop().borrow(), "a side run's stop reads it");
        drop((a, b));
        for w in &mut waits {
            assert!(w.changed().await.is_err(), "closed once its claim let go");
        }
        assert_eq!(claims.count("t1"), 0);
        assert_eq!(claims.count("t2"), 1);
        assert!(claims.cancel("none").is_empty());
    }
}
