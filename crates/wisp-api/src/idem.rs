//! Command answers remembered for a retry (`Idempotency-Key`, a v2 mutate's `mid`): a write retried over a flaky
//! tunnel runs once and gets the first answer. Only writes come here (a read is run again: nothing to protect), and
//! what's kept is bounded by count, by bytes and by age, so answering commands never grows the server (perf audit
//! 2026-11-18 #1: 10,000 answers of any size were kept for good, 60 task-list reads took it to 1 GB).
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::OnceCell;

/// `(HTTP status, answer)`, as `command_answer` builds it.
pub type Done = (u16, Value);

/// At most this many answers kept.
pub const KEPT: usize = 2_000;
/// At most this many bytes of answers (their JSON) kept in all; the newest answer stays even when it alone is over.
pub const KEPT_BYTES: usize = 8 << 20;
/// A retry comes within seconds (a tunnel reconnecting); an answer older than this is forgotten.
pub const KEPT_FOR: Duration = Duration::from_secs(10 * 60);

type Cell = Arc<OnceCell<Done>>;

struct Kept {
    cell: Cell,
    at: Instant,
    /// the answer's JSON size once it's there (0 while it runs)
    bytes: usize,
}

#[derive(Default)]
struct Inner {
    map: HashMap<String, Kept>,
    /// keys, oldest first
    order: VecDeque<String>,
    bytes: usize,
}

impl Inner {
    fn forget_oldest(&mut self) {
        if let Some(old) = self.order.pop_front()
            && let Some(k) = self.map.remove(&old)
        {
            self.bytes -= k.bytes;
        }
    }

    fn expire(&mut self, now: Instant, ttl: Duration) {
        while let Some(k) = self.order.front().and_then(|key| self.map.get(key)) {
            if now.duration_since(k.at) < ttl {
                break;
            }
            self.forget_oldest();
        }
    }

    fn trim(&mut self, count: usize, bytes: usize) {
        while self.order.len() > 1 && (self.order.len() > count || self.bytes > bytes) {
            self.forget_oldest();
        }
    }
}

pub struct Idempotency {
    inner: Mutex<Inner>,
    count: usize,
    bytes: usize,
    ttl: Duration,
}

impl std::fmt::Debug for Idempotency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (n, b) = self.kept();
        f.debug_struct("Idempotency").field("kept", &n).field("bytes", &b).finish()
    }
}

impl Default for Idempotency {
    fn default() -> Self {
        Self::with_limits(KEPT, KEPT_BYTES, KEPT_FOR)
    }
}

impl Idempotency {
    pub fn with_limits(count: usize, bytes: usize, ttl: Duration) -> Self {
        Self { inner: Mutex::default(), count: count.max(1), bytes, ttl }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `run` once per `key` while its answer is kept; a retry (even one that arrives while the first still runs)
    /// gets the first answer.
    pub async fn once<F>(&self, key: String, run: F) -> Done
    where
        F: std::future::Future<Output = Done>,
    {
        let cell = {
            let mut g = self.lock();
            g.expire(Instant::now(), self.ttl);
            if let Some(k) = g.map.get(&key) {
                k.cell.clone()
            } else {
                let c: Cell = Arc::new(OnceCell::new());
                g.map.insert(key.clone(), Kept { cell: c.clone(), at: Instant::now(), bytes: 0 });
                g.order.push_back(key.clone());
                g.trim(self.count, self.bytes);
                c
            }
        };
        let done = cell.get_or_init(|| run).await.clone();
        // counted once, by the caller whose run filled it (a retry finds `bytes` already set)
        let size = serde_json::to_vec(&done.1).map_or(0, |v| v.len()).max(1);
        let mut g = self.lock();
        let counted = match g.map.get_mut(&key) {
            Some(k) if k.bytes == 0 && Arc::ptr_eq(&k.cell, &cell) => {
                k.bytes = size;
                true
            }
            _ => false,
        };
        if counted {
            g.bytes += size;
            g.trim(self.count, self.bytes);
        }
        done
    }

    /// (answers kept, their bytes)
    pub fn kept(&self) -> (usize, usize) {
        let g = self.lock();
        (g.map.len(), g.bytes)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[tokio::test]
    async fn a_retry_gets_the_first_answer_and_runs_nothing() {
        let idem = Idempotency::default();
        let runs = AtomicU64::new(0);
        let run = || async {
            runs.fetch_add(1, Ordering::SeqCst);
            (200, json!({"n": 1}))
        };
        let (a, b) = tokio::join!(idem.once("k".into(), run()), idem.once("k".into(), run()));
        assert_eq!(a, b);
        assert_eq!(idem.once("k".into(), run()).await.1["n"], 1);
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        idem.once("k2".into(), run()).await;
        assert_eq!(runs.load(Ordering::SeqCst), 2);
        assert_eq!(idem.kept().0, 2);
    }

    #[tokio::test]
    async fn big_answers_stay_under_the_byte_bound() {
        let idem = Idempotency::with_limits(10_000, 1 << 20, KEPT_FOR);
        let big = json!({ "rows": "x".repeat(100_000) });
        for i in 0..1_000 {
            idem.once(format!("k{i}"), async { (200, big.clone()) }).await;
        }
        let (n, bytes) = idem.kept();
        assert!(bytes <= 1 << 20, "{bytes} bytes kept");
        assert!(n <= 11, "{n} answers kept");
        // the newest is still answered from the cache
        let fresh = idem.once("k999".into(), async { (500, json!("ran again")) }).await;
        assert_eq!(fresh.0, 200);
    }

    #[tokio::test]
    async fn answers_are_forgotten_by_count_and_age() {
        let idem = Idempotency::with_limits(3, KEPT_BYTES, KEPT_FOR);
        for i in 0..10 {
            idem.once(format!("k{i}"), async { (200, json!(1)) }).await;
        }
        assert_eq!(idem.kept().0, 3);
        let idem = Idempotency::with_limits(100, KEPT_BYTES, Duration::from_millis(20));
        idem.once("old".into(), async { (200, json!(1)) }).await;
        tokio::time::sleep(Duration::from_millis(40)).await;
        let again = idem.once("old".into(), async { (201, json!(2)) }).await;
        assert_eq!(again.0, 201, "an answer older than its age bound is run again");
        assert_eq!(idem.kept().0, 1);
    }
}
