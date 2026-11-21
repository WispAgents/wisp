//! The heap handed back to the OS after big reads (ADR-0084). glibc keeps what a thread freed in that thread's arena
//! and trims only an arena's top: on Linux a 5 000-task list read left ~5 MB resident each time (idle RSS 300 MB in
//! CI, 97 MB on macOS, whose allocator gives freed pages back). The stream's snapshot source is wrapped so every first
//! look (the reads that build a whole topic) wakes one trimmer, which waits for the burst to end and trims once, on
//! the blocking pool. Where `wisp_proc::trim_heap` has nothing to do (macOS, Windows) the source isn't wrapped.
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;
use wisp_api::v2::{PageSink, Read, Snapshots};
use wisp_kernel::BoxFuture;
use wisp_protocol::{Row, Topic};

/// A burst of reads (every client's first look after a restart, a benchmark's 30) is over once none ended for this
/// long, which also leaves the last page time to leave its socket and be freed.
pub const QUIET: Duration = Duration::from_millis(300);

/// A burst that never goes quiet (a busy grid box, a client stuck reconnecting) still trims this long after it began.
pub const LONGEST: Duration = Duration::from_secs(5);

/// On glibc, `snaps` wrapped so each burst of first looks hands the heap's free pages back once it ends, and one trim
/// now for what the start freed; elsewhere the allocator does it and `snaps` is as it was. The trimmer is `wisp
/// serve`'s to stop.
pub fn wrap(snaps: Arc<dyn Snapshots>) -> (Arc<dyn Snapshots>, Option<tokio::task::JoinHandle<()>>) {
    if !cfg!(all(feature = "proc", target_os = "linux", target_env = "gnu")) {
        return (snaps, None);
    }
    let wake = Arc::new(Notify::new());
    wake.notify_one();
    let task = tokio::spawn(trimmer(wake.clone(), (QUIET, LONGEST), on_the_pool));
    (Arc::new(TrimAfter::new(snaps, wake)), Some(task))
}

/// `inner`, waking `wake` each time a first look (`since` none) has handed over its last page.
pub struct TrimAfter {
    inner: Arc<dyn Snapshots>,
    wake: Arc<Notify>,
}

impl TrimAfter {
    pub fn new(inner: Arc<dyn Snapshots>, wake: Arc<Notify>) -> Self {
        Self { inner, wake }
    }

    fn waker(&self, since: Option<u64>) -> Option<Arc<Notify>> {
        since.is_none().then(|| self.wake.clone())
    }
}

impl Snapshots for TrimAfter {
    fn epoch(&self) -> String {
        self.inner.epoch()
    }
    fn head_seq(&self) -> u64 {
        self.inner.head_seq()
    }
    fn oldest_seq(&self) -> u64 {
        self.inner.oldest_seq()
    }
    fn snapshot(&self, topic: &Topic, since: Option<u64>) -> BoxFuture<'static, Read> {
        let (read, wake) = (self.inner.snapshot(topic, since), self.waker(since));
        Box::pin(async move {
            let read = read.await;
            if let Some(w) = wake {
                w.notify_one();
            }
            read
        })
    }
    fn snapshot_pages(&self, topic: &Topic, since: Option<u64>, page: PageSink) -> BoxFuture<'static, ()> {
        let (read, wake) = (self.inner.snapshot_pages(topic, since, page), self.waker(since));
        Box::pin(async move {
            read.await;
            if let Some(w) = wake {
                w.notify_one();
            }
        })
    }
    fn window(&self, topic: &Topic, before_seq: u64, limit: u32) -> BoxFuture<'static, (Vec<Row>, bool)> {
        self.inner.window(topic, before_seq, limit)
    }
}

/// Runs `trim` once after each burst of wakes: the first wake, then none for `quiet`, or `longest` after the first
/// when they keep coming. Never ends by itself (`wisp serve` aborts it when it stops).
pub async fn trimmer<T: Future<Output = bool>>(wake: Arc<Notify>, (quiet, longest): (Duration, Duration), trim: impl Fn() -> T) {
    loop {
        wake.notified().await;
        let until = tokio::time::Instant::now() + longest;
        loop {
            let left = until.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() || tokio::time::timeout(quiet.min(left), wake.notified()).await.is_err() {
                break;
            }
        }
        let released = trim().await;
        tracing::debug!(released, "heap trimmed after a burst of first looks");
    }
}

/// `wisp_proc::trim_heap` on the blocking pool: it walks every arena, never on an async worker.
pub async fn on_the_pool() -> bool {
    #[cfg(feature = "proc")]
    {
        tokio::task::spawn_blocking(wisp_proc::trim_heap).await.unwrap_or(false)
    }
    #[cfg(not(feature = "proc"))]
    {
        false
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures::FutureExt as _;

    use super::*;

    /// A trimmer on `wake` that counts its trims.
    fn counted(wake: &Arc<Notify>) -> (Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let n = Arc::new(AtomicUsize::new(0));
        let m = n.clone();
        let count = move || {
            let m = m.clone();
            async move {
                m.fetch_add(1, Ordering::SeqCst);
                true
            }
        };
        (n, tokio::spawn(trimmer(wake.clone(), (QUIET, LONGEST), count)))
    }

    /// Lets the trimmer take what happened so far, moves the clock by `by`, and lets it run until it waits again.
    async fn after(by: Duration) {
        let settle = || async {
            for _ in 0..20 {
                tokio::task::yield_now().await;
            }
        };
        settle().await;
        tokio::time::advance(by).await;
        settle().await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_burst_of_first_looks_trims_the_heap_once_after_it_ends_and_a_later_one_again() {
        let wake = Arc::new(Notify::new());
        let (trims, t) = counted(&wake);
        after(Duration::ZERO).await;
        assert_eq!(trims.load(Ordering::SeqCst), 0, "nothing read, nothing trimmed");
        for _ in 0..5 {
            wake.notify_one();
            after(QUIET / 2).await;
        }
        assert_eq!(trims.load(Ordering::SeqCst), 0, "reads still coming: no trim mid-burst");
        after(QUIET).await;
        after(Duration::ZERO).await;
        assert_eq!(trims.load(Ordering::SeqCst), 1, "one trim once the burst is quiet");
        after(QUIET * 10).await;
        assert_eq!(trims.load(Ordering::SeqCst), 1, "an idle server never trims again on its own");
        wake.notify_one();
        after(QUIET * 2).await;
        after(Duration::ZERO).await;
        assert_eq!(trims.load(Ordering::SeqCst), 2, "a later read trims again");
        t.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn first_looks_that_never_go_quiet_still_trim_once_the_longest_wait_is_over() {
        let wake = Arc::new(Notify::new());
        let (trims, t) = counted(&wake);
        // a first look ends every QUIET / 2, never a quiet QUIET: past the longest wait and beyond
        let mut waited = Duration::ZERO;
        while waited < LONGEST + QUIET {
            if waited + QUIET < LONGEST {
                assert_eq!(trims.load(Ordering::SeqCst), 0, "under the longest wait, a busy stream doesn't trim yet");
            }
            wake.notify_one();
            after(QUIET / 2).await;
            waited += QUIET / 2;
        }
        assert_eq!(trims.load(Ordering::SeqCst), 1, "a stream that never goes quiet trims once the longest wait is over");
        t.abort();
    }

    /// Every topic one row at seq 7.
    struct One;

    impl Snapshots for One {
        fn epoch(&self) -> String {
            "e".into()
        }
        fn head_seq(&self) -> u64 {
            7
        }
        fn oldest_seq(&self) -> u64 {
            1
        }
        fn snapshot(&self, _: &Topic, since: Option<u64>) -> BoxFuture<'static, Read> {
            let row = Row { e: "session".into(), id: "a".into(), v: serde_json::json!({"id": "a"}) };
            Box::pin(async move { Read { seq: 7, rows: vec![row], full: since.is_none(), deleted: Vec::new() } })
        }
        fn window(&self, _: &Topic, _: u64, _: u32) -> BoxFuture<'static, (Vec<Row>, bool)> {
            Box::pin(async { (Vec::new(), true) })
        }
    }

    #[tokio::test]
    async fn only_a_first_look_wakes_the_trimmer_and_every_read_passes_through_unchanged() {
        let wake = Arc::new(Notify::new());
        let s = TrimAfter::new(Arc::new(One), wake.clone());
        let topic = Topic::parse("list").unwrap();
        assert_eq!((s.epoch().as_str(), s.head_seq(), s.oldest_seq()), ("e", 7, 1));
        let resumed = s.snapshot(&topic, Some(3)).await;
        assert!(!resumed.full && resumed.rows.len() == 1);
        // woken: a permit waits, taken by one poll
        let waits = |w: &Arc<Notify>| w.notified().now_or_never().is_some();
        assert!(!waits(&wake), "a resume isn't a big read: no wake");
        let got = Arc::new(std::sync::Mutex::new(Vec::new()));
        let g = got.clone();
        s.snapshot_pages(&topic, Some(3), Box::new(move |r, more| g.lock().unwrap().push((r.rows.len(), more)))).await;
        assert!(!waits(&wake), "nor a resume in pages");
        let first = s.snapshot(&topic, None).await;
        assert!(first.full && first.rows.len() == 1);
        assert!(waits(&wake), "a first look wakes it");
        let g = got.clone();
        s.snapshot_pages(&topic, None, Box::new(move |r, more| g.lock().unwrap().push((r.rows.len(), more)))).await;
        assert!(waits(&wake), "a first look in pages wakes it once its last page is handed over");
        assert_eq!(*got.lock().unwrap(), vec![(1, false), (1, false)]);
        let (rows, done) = s.window(&topic, 9, 10).await;
        assert!(rows.is_empty() && done);
    }
}
