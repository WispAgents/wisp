//! The harness probes' memo (ADR-0011): `runtime.agents`, Settings → Agents, wisp.accounts' start, `wisp_available`
//! and a task's setup record each run every harness's detect, and each detect spawns its CLI a few times (`--version`,
//! `--help`, `models`). Round 211 saw one run start `agy --help` 8 times, and with a CLI that hangs every one of them
//! waited its full 15 s. Here one probe runs once:
//!
//! - **in flight, shared**: the same probe (program, args, env, folder) asked while it runs joins that run;
//! - **answered, kept**: its answer stands while the program's file is the same one (path, size, mtime, inode): an
//!   update or a reinstall is probed again. And only for a while: an answer for [`KEEP_MS`] (a version manager's
//!   shim stays the same file while the CLI under it is updated; a sign-in changes `models`), a failure for
//!   [`FAILED_FOR_MS`] (signed out, offline), a hang for [`HUNG_FOR_MS`] (asked again, it costs its 15 s again).
//!   Re-check (`fresh`) runs it again, joining only a run that started after it was asked.
//!
//! Only probes are kept: an exec with input, or of a program wisp can't find, runs as asked. A probe nobody waits
//! for any more is ended (the memo holds no strong handle to a run), so a caller that hangs up still ends its probes
//! (round 211's `EndOnDrop`).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use futures::future::{FutureExt, Shared, WeakShared};
use wisp_kernel::{BoxFuture, Clock, Result};
use wisp_proc::Proc;
use wisp_runtime_api::harness::{ExecOut, ExecReq};

/// How long a probe's answer stands (exit 0).
pub const KEEP_MS: i64 = 10 * 60_000;
/// How long a probe that failed (another exit) stands.
pub const FAILED_FOR_MS: i64 = 30_000;
/// How long a probe that timed out stands.
pub const HUNG_FOR_MS: i64 = 5 * 60_000;

/// How long `out` stands.
fn kept_for(out: &ExecOut) -> i64 {
    if out.timed_out {
        HUNG_FOR_MS
    } else if out.code == Some(0) {
        KEEP_MS
    } else {
        FAILED_FOR_MS
    }
}

/// Which file a program is: a change (an update, a reinstall, another one first on PATH) is another probe.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
    ino: u64,
}

type Run = BoxFuture<'static, Result<ExecOut>>;

/// What is asked: the same program the same way.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    program: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    cwd: Option<String>,
    timeout_ms: u64,
}

enum Slot {
    /// `seq`: when it started, on the memo's count (a Re-check joins only one started after it was asked)
    Running {
        stamp: Stamp,
        run: WeakShared<Run>,
        seq: u64,
    },
    Done {
        stamp: Stamp,
        out: ExecOut,
        at: i64,
    },
}

/// One per runtime.
pub struct Probes {
    clock: Arc<dyn Clock>,
    slots: Mutex<HashMap<Key, Slot>>,
    /// how many runs it started
    seq: AtomicU64,
}

impl std::fmt::Debug for Probes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Probes").finish_non_exhaustive()
    }
}

/// What the memo has for a probe.
enum Found {
    Answer(ExecOut),
    Join(Shared<Run>),
}

impl Probes {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self { clock, slots: Mutex::default(), seq: AtomicU64::new(0) }
    }

    fn slots(&self) -> std::sync::MutexGuard<'_, HashMap<Key, Slot>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `req` through the memo; `start` runs it when nothing kept answers (`fresh`: nothing finished does, nor a run
    /// that started before this ask).
    pub async fn exec(&self, proc: &Proc, req: ExecReq, fresh: bool, start: impl FnOnce(ExecReq) -> Run) -> Result<ExecOut> {
        if req.stdin.is_some() {
            return start(req).await;
        }
        let asked = self.seq.load(Ordering::SeqCst);
        let Some(stamp) = stamp(proc, &req.program, req.cwd.as_deref()).await else {
            return start(req).await;
        };
        let key = Key {
            program: req.program.clone(),
            args: req.args.clone(),
            env: req.env.clone(),
            cwd: req.cwd.clone(),
            timeout_ms: req.timeout_ms,
        };
        let fresh = fresh.then_some(asked);
        // looked up and started under one lock: two asks at once must not both start it
        let found = {
            let mut slots = self.slots();
            if let Some(found) = self.find(&slots, &key, &stamp, fresh) {
                found
            } else {
                let run = start(req).shared();
                let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
                if let Some(weak) = run.downgrade() {
                    slots.insert(key.clone(), Slot::Running { stamp: stamp.clone(), run: weak, seq });
                }
                Found::Join(run)
            }
        };
        let run = match found {
            Found::Answer(out) => return Ok(out),
            Found::Join(run) => run,
        };
        let out = run.clone().await;
        let mut slots = self.slots();
        let ours = matches!(slots.get(&key), Some(Slot::Running { run: weak, .. }) if weak.upgrade().is_some_and(|r| r.ptr_eq(&run)));
        if ours {
            match &out {
                Ok(o) => {
                    slots.insert(key, Slot::Done { stamp, out: o.clone(), at: self.clock.now_ms() });
                }
                // couldn't run it: asked again, it runs again
                Err(_) => {
                    slots.remove(&key);
                }
            }
        }
        out
    }

    /// What stands for `key` (`fresh`: the count when the Re-check was asked).
    fn find(&self, slots: &HashMap<Key, Slot>, key: &Key, stamp: &Stamp, fresh: Option<u64>) -> Option<Found> {
        match slots.get(key)? {
            Slot::Done { stamp: s, out, at } if s == stamp && fresh.is_none() && self.clock.now_ms() - at < kept_for(out) => {
                Some(Found::Answer(out.clone()))
            }
            Slot::Running { stamp: s, run, seq } if s == stamp && fresh.is_none_or(|asked| *seq > asked) => run.upgrade().map(Found::Join),
            _ => None,
        }
    }
}

/// The file `program` runs (a bare name: the first on PATH; a relative path: from `cwd`), as it is now; None: there
/// is none.
async fn stamp(proc: &Proc, program: &str, cwd: Option<&str>) -> Option<Stamp> {
    let p = Path::new(program);
    let path = if p.is_absolute() {
        p.to_owned()
    } else if p.components().count() > 1 {
        cwd.map_or_else(|| p.to_owned(), |c| Path::new(c).join(p))
    } else {
        proc.which(program, None).await?
    };
    let m = tokio::fs::metadata(&path).await.ok()?;
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(&m);
    #[cfg(not(unix))]
    let ino = 0;
    Some(Stamp { path, len: m.len(), modified: m.modified().ok(), ino })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, AtomicUsize};

    struct Fixed(AtomicI64);
    impl Clock for Fixed {
        fn now_ms(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
        fn sleep(&self, _ms: u64) -> BoxFuture<'static, ()> {
            Box::pin(async {})
        }
    }

    fn rig() -> (Probes, Arc<Fixed>, PathBuf) {
        let clock = Arc::new(Fixed(AtomicI64::new(1_000)));
        let dir = std::env::temp_dir().join(format!("wisp-probes-{}", wisp_kernel::ids::uuid7()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("cli");
        std::fs::write(&bin, "one").unwrap();
        (Probes::new(clock.clone()), clock, bin)
    }

    fn req(bin: &Path, args: &[&str]) -> ExecReq {
        ExecReq { program: bin.to_string_lossy().into_owned(), args: args.iter().map(|a| (*a).to_owned()).collect(), ..ExecReq::default() }
    }

    /// A run that counts itself and answers after `gate` opens (None: at once).
    fn counted(
        n: &Arc<AtomicUsize>,
        gate: Option<tokio::sync::watch::Receiver<bool>>,
        timed_out: bool,
    ) -> impl FnOnce(ExecReq) -> Run + use<> {
        exits(n, gate, timed_out, Some(0))
    }

    fn exits(
        n: &Arc<AtomicUsize>,
        gate: Option<tokio::sync::watch::Receiver<bool>>,
        timed_out: bool,
        code: Option<i32>,
    ) -> impl FnOnce(ExecReq) -> Run + use<> {
        let n = n.clone();
        move |r: ExecReq| {
            let seen = n.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if let Some(mut g) = gate {
                    let _ = g.wait_for(|v| *v).await;
                }
                Ok(ExecOut { code, stdout: format!("{} #{seen}", r.args.join(" ")), timed_out, ..ExecOut::default() })
            })
        }
    }

    #[tokio::test]
    async fn the_same_probe_asked_while_it_runs_runs_once() {
        let (p, _, bin) = rig();
        let proc = wisp_proc::local();
        let n = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = tokio::sync::watch::channel(false);
        let asks = (0..8).map(|_| p.exec(&proc, req(&bin, &["--help"]), false, counted(&n, Some(rx.clone()), false)));
        let all = futures::future::join_all(asks);
        let open = async {
            tokio::task::yield_now().await;
            let _ = tx.send(true);
        };
        let (outs, ()) = futures::future::join(all, open).await;
        assert_eq!(n.load(Ordering::SeqCst), 1, "agy --help ran more than once");
        assert!(outs.iter().all(|o| o.as_ref().unwrap().stdout == "--help #0"));
    }

    /// Round 212's flake: asks on two threads at once each found nothing and each started it (`opencode models`
    /// ran twice on a server start); the lookup and the start share one lock.
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn asks_on_many_threads_at_once_start_it_once() {
        let (p, _, bin) = rig();
        let (p, proc) = (Arc::new(p), wisp_proc::local());
        for round in 0..200 {
            let n = Arc::new(AtomicUsize::new(0));
            let go = Arc::new(tokio::sync::Barrier::new(16));
            let arg = format!("probe-{round}");
            let asks: Vec<_> = (0..16)
                .map(|_| {
                    let (p, proc, n, go, bin, arg) = (p.clone(), proc.clone(), n.clone(), go.clone(), bin.clone(), arg.clone());
                    tokio::spawn(async move {
                        go.wait().await;
                        p.exec(&proc, req(&bin, &[&arg]), false, counted(&n, None, false)).await.unwrap();
                    })
                })
                .collect();
            for a in asks {
                a.await.unwrap();
            }
            assert_eq!(n.load(Ordering::SeqCst), 1, "round {round}: started more than once");
        }
    }

    #[tokio::test]
    async fn an_answer_stands_until_the_program_changes() {
        let (p, _, bin) = rig();
        let proc = wisp_proc::local();
        let n = Arc::new(AtomicUsize::new(0));
        p.exec(&proc, req(&bin, &["models"]), false, counted(&n, None, false)).await.unwrap();
        p.exec(&proc, req(&bin, &["models"]), false, counted(&n, None, false)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 1, "a second ask ran it again");
        // another probe of it is its own
        p.exec(&proc, req(&bin, &["--help"]), false, counted(&n, None, false)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 2);
        // an update: other bytes, another mtime
        std::fs::write(&bin, "two, longer").unwrap();
        p.exec(&proc, req(&bin, &["models"]), false, counted(&n, None, false)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 3, "the changed program wasn't probed again");
    }

    #[tokio::test]
    async fn re_check_runs_it_again() {
        let (p, _, bin) = rig();
        let proc = wisp_proc::local();
        let n = Arc::new(AtomicUsize::new(0));
        p.exec(&proc, req(&bin, &["models"]), false, counted(&n, None, false)).await.unwrap();
        p.exec(&proc, req(&bin, &["models"]), true, counted(&n, None, false)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_timed_out_probe_stands_only_for_a_while() {
        let (p, clock, bin) = rig();
        let proc = wisp_proc::local();
        let n = Arc::new(AtomicUsize::new(0));
        p.exec(&proc, req(&bin, &["models"]), false, counted(&n, None, true)).await.unwrap();
        p.exec(&proc, req(&bin, &["models"]), false, counted(&n, None, true)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 1, "the hang is the answer for a while");
        clock.0.fetch_add(HUNG_FOR_MS, Ordering::SeqCst);
        p.exec(&proc, req(&bin, &["models"]), false, counted(&n, None, false)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 2, "a hang stood for good");
    }

    /// Signed out, offline: `cursor-agent --list-models` exits 1; signed in a minute later, it's read again.
    #[tokio::test]
    async fn a_failed_probe_stands_only_briefly() {
        let (p, clock, bin) = rig();
        let proc = wisp_proc::local();
        let n = Arc::new(AtomicUsize::new(0));
        p.exec(&proc, req(&bin, &["--list-models"]), false, exits(&n, None, false, Some(1))).await.unwrap();
        p.exec(&proc, req(&bin, &["--list-models"]), false, exits(&n, None, false, Some(1))).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 1);
        clock.0.fetch_add(FAILED_FOR_MS, Ordering::SeqCst);
        p.exec(&proc, req(&bin, &["--list-models"]), false, counted(&n, None, false)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 2, "a failure stood past `FAILED_FOR_MS`");
    }

    /// A shim on PATH stays the same file while the CLI under it is updated: an answer is read again after `KEEP_MS`.
    #[tokio::test]
    async fn an_answer_is_read_again_after_a_while() {
        let (p, clock, bin) = rig();
        let proc = wisp_proc::local();
        let n = Arc::new(AtomicUsize::new(0));
        p.exec(&proc, req(&bin, &["--version"]), false, counted(&n, None, false)).await.unwrap();
        clock.0.fetch_add(KEEP_MS - 1, Ordering::SeqCst);
        p.exec(&proc, req(&bin, &["--version"]), false, counted(&n, None, false)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 1);
        clock.0.fetch_add(1, Ordering::SeqCst);
        p.exec(&proc, req(&bin, &["--version"]), false, counted(&n, None, false)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 2, "an answer stood for good");
    }

    /// Signed in, then Re-check while an earlier `auth list` (asked signed out) still runs: Re-check runs its own,
    /// and an ask with no Re-check after it joins that newer one.
    #[tokio::test]
    async fn re_check_never_joins_a_run_started_before_it() {
        let (p, _, bin) = rig();
        let p = Arc::new(p);
        let n = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = tokio::sync::watch::channel(false);
        let ask = |fresh: bool| {
            let (p, bin, run) = (p.clone(), bin.clone(), counted(&n, Some(rx.clone()), false));
            tokio::spawn(async move { p.exec(&wisp_proc::local(), req(&bin, &["auth", "list"]), fresh, run).await })
        };
        let started = async |k: usize| {
            let at = std::time::Instant::now();
            while n.load(Ordering::SeqCst) < k {
                assert!(at.elapsed() < std::time::Duration::from_secs(2), "run {k} never started: Re-check joined the run asked before it");
                tokio::task::yield_now().await;
            }
        };
        let old = ask(false);
        started(1).await;
        let recheck = ask(true);
        started(2).await;
        let later = ask(false);
        tokio::task::yield_now().await;
        let _ = tx.send(true);
        let (old, recheck, later) = (old.await.unwrap().unwrap(), recheck.await.unwrap().unwrap(), later.await.unwrap().unwrap());
        assert_eq!(n.load(Ordering::SeqCst), 2, "Re-check joined the run asked before it");
        assert_eq!(old.stdout, "auth list #0");
        assert_eq!(recheck.stdout, "auth list #1");
        assert_eq!(later.stdout, "auth list #1", "a plain ask joins the newest run");
    }

    #[tokio::test]
    async fn input_or_a_missing_program_is_never_kept() {
        let (p, _, bin) = rig();
        let proc = wisp_proc::local();
        let n = Arc::new(AtomicUsize::new(0));
        let mut with_input = req(&bin, &["run"]);
        with_input.stdin = Some(b"hi".to_vec());
        p.exec(&proc, with_input.clone(), false, counted(&n, None, false)).await.unwrap();
        p.exec(&proc, with_input, false, counted(&n, None, false)).await.unwrap();
        let gone = req(&bin.with_file_name("not-there"), &["--version"]);
        p.exec(&proc, gone.clone(), false, counted(&n, None, false)).await.unwrap();
        p.exec(&proc, gone, false, counted(&n, None, false)).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 4);
    }

    /// Everyone who asked gave up: the run is dropped (its process group ended), not kept running by the memo.
    #[tokio::test]
    async fn a_probe_nobody_waits_for_is_dropped() {
        struct Flag(Arc<AtomicUsize>);
        impl Drop for Flag {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let (p, _, bin) = rig();
        let proc = wisp_proc::local();
        let dropped = Arc::new(AtomicUsize::new(0));
        let d = dropped.clone();
        let start = move |_r: ExecReq| -> Run {
            let flag = Flag(d);
            Box::pin(async move {
                let _f = flag;
                std::future::pending::<()>().await;
                Ok(ExecOut::default())
            })
        };
        let ask = p.exec(&proc, req(&bin, &["--help"]), false, start);
        assert!(tokio::time::timeout(std::time::Duration::from_millis(50), ask).await.is_err());
        assert_eq!(dropped.load(Ordering::SeqCst), 1, "the memo kept a hung probe alive");
    }
}
