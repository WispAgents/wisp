//! Audit B #18: Settings › Worktrees showed the last look's 13 of 41 worktrees, with no sign of looking, until a slow
//! look ended. The pane's rows are republished when a look starts: they say it's looking, with the worktrees known now.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::Semaphore;
use wisp_kernel::{BoxFuture, Call, Caller, CancellationToken};
use wisp_worktrees::cleanup::Cleanup;
use wisp_worktrees::ports::Port;
use wisp_worktrees::{Engine, MemPort, TaskInfo, WtSettings};

/// `MemPort`, but the look's own read of the tasks waits until the test lets it go (a slow look).
struct Slow {
    mem: Arc<MemPort>,
    armed: AtomicBool,
    /// a permit once the look is held
    held: Arc<Semaphore>,
    go: Arc<Semaphore>,
}
impl Port for Slow {
    fn tasks(&self) -> BoxFuture<'static, Vec<TaskInfo>> {
        let hold = self.armed.swap(false, Ordering::SeqCst);
        let (inner, go, held) = (self.mem.tasks(), self.go.clone(), self.held.clone());
        Box::pin(async move {
            if hold {
                held.add_permits(1);
                go.acquire().await.unwrap().forget();
            }
            inner.await
        })
    }
    fn set_kept(&self, id: String, kept: bool) -> BoxFuture<'static, ()> {
        self.mem.set_kept(id, kept)
    }
    fn settings(&self) -> BoxFuture<'static, WtSettings> {
        self.mem.settings()
    }
    fn save_settings(&self, s: WtSettings) -> BoxFuture<'static, ()> {
        self.mem.save_settings(s)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_pane_says_its_looking_from_the_start_of_a_look() {
    let dir = std::env::temp_dir().join(format!("wisp-wt-look-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("h/worktrees")).unwrap();
    let git = wisp_git::service(wisp_proc::local(), None, Arc::new(|| 1_000_000), None);
    let engine = Engine::new(
        Arc::new(wisp_git::Service(git)),
        wisp_proc::local(),
        wisp_testkit::FakeClock::at(1_000_000).service(),
        dir.join("h").to_string_lossy().into_owned(),
        dir.join("u").to_string_lossy().into_owned(),
        "macos",
        Vec::new(),
        CancellationToken::new(),
    );
    let port = Arc::new(Slow {
        mem: MemPort::new(Vec::new(), WtSettings::default()),
        armed: AtomicBool::new(true),
        held: Arc::new(Semaphore::new(0)),
        go: Arc::new(Semaphore::new(0)),
    });
    let c = Cleanup::new(Arc::new(engine), port.clone(), Call::new_root(Caller::user()));
    let look = tokio::spawn({
        let c = c.clone();
        async move { c.scan().await }
    });
    // the look is under way (held on its read of the tasks): the pane is told now, not when it ends
    let wait = Duration::from_secs(10);
    tokio::time::timeout(wait, port.held.acquire()).await.expect("the look reads the tasks").unwrap().forget();
    tokio::time::timeout(wait, c.changed.notified()).await.expect("republished as the look starts");
    assert!(tokio::time::timeout(wait, c.view(false)).await.expect("the view answers").looking, "the pane's rows say it's looking");
    port.go.add_permits(1);
    tokio::time::timeout(wait, look).await.expect("the look ends").unwrap();
    tokio::time::timeout(Duration::from_secs(10), c.changed.notified()).await.expect("and again as it ends");
    assert!(!c.view(false).await.looking);
    std::fs::remove_dir_all(&dir).ok();
}

/// Audit B #18 (round C): `worktrees.view` still said `looking: true` 40+ minutes after start. A look that never ends
/// (here: its read of the tasks never comes back) ends at `look_max` with what the last look had, and the pane is told
/// once it's over, so its rows stop saying it's looking.
#[tokio::test(flavor = "multi_thread")]
async fn a_look_that_hangs_ends_and_the_pane_stops_saying_its_looking() {
    let dir = std::env::temp_dir().join(format!("wisp-wt-hang-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("h/worktrees")).unwrap();
    let git = wisp_git::service(wisp_proc::local(), None, Arc::new(|| 1_000_000), None);
    let engine = Engine::new(
        Arc::new(wisp_git::Service(git)),
        wisp_proc::local(),
        wisp_testkit::FakeClock::at(1_000_000).service(),
        dir.join("h").to_string_lossy().into_owned(),
        dir.join("u").to_string_lossy().into_owned(),
        "macos",
        Vec::new(),
        CancellationToken::new(),
    );
    // armed and never let go: the look's own read of the tasks hangs
    let port = Arc::new(Slow {
        mem: MemPort::new(Vec::new(), WtSettings::default()),
        armed: AtomicBool::new(true),
        held: Arc::new(Semaphore::new(0)),
        go: Arc::new(Semaphore::new(0)),
    });
    let c = Cleanup::new(Arc::new(engine), port.clone(), Call::new_root(Caller::user()));
    c.set_look_max(300);
    let wait = Duration::from_secs(10);
    tokio::time::timeout(wait, c.scan()).await.expect("the look ends though its read never came back");
    assert!(!c.view(false).await.looking, "nor does the pane say it's still looking");
    // a later look starts and ends as usual
    tokio::time::timeout(wait, c.scan()).await.expect("the next look ends");
    assert!(!c.view(false).await.looking);
    // Bring in showed up 4–9 s after the page opened, with nothing said before it: while its look runs the pane's
    // status says it's looking ("· still looking…"), and not once it's done
    assert!(wisp_worktrees::pane::looking_for_offers(c.view(false).await, true).looking);
    assert!(!wisp_worktrees::pane::looking_for_offers(c.view(false).await, false).looking);
    std::fs::remove_dir_all(&dir).ok();
}

/// `local()`, but `du` never answers (a home with ~60 worktrees and their `node_modules`, audit E6).
struct SlowDu(wisp_proc::Proc);
impl wisp_proc::ProcService for SlowDu {
    fn which(&self, bin: &str, env: Option<Vec<(String, String)>>) -> BoxFuture<'static, Option<std::path::PathBuf>> {
        self.0.which(bin, env)
    }
    fn spawn(&self, bin: &str, args: &[String], opts: wisp_proc::SpawnOpts) -> wisp_proc::Result<wisp_proc::Child> {
        self.0.spawn(bin, args, opts)
    }
    fn exec(&self, bin: &str, args: &[String], opts: wisp_proc::ExecOpts) -> BoxFuture<'static, wisp_proc::Result<wisp_proc::ExecResult>> {
        if bin == "du" {
            return Box::pin(std::future::pending());
        }
        self.0.exec(bin, args, opts)
    }
    fn spawn_detached(&self, bin: &str, args: &[String], opts: wisp_proc::SpawnOpts) -> wisp_proc::Result<u32> {
        self.0.spawn_detached(bin, args, opts)
    }
    fn live(&self) -> usize {
        self.0.live()
    }
    fn kill_all(&self, grace: Duration) -> BoxFuture<'static, ()> {
        self.0.kill_all(grace)
    }
}

/// Audit E6: with ~60 worktrees `worktrees.view` said `looking: true, last: null` for 45 minutes. The look waited on `du`
/// (120 s) before it looked at a single worktree, inside its own 90 s limit: it was cut every time, with nothing kept,
/// and the next view started it again. The sizes come on their own; the look ends with all 60 found.
#[tokio::test(flavor = "multi_thread")]
async fn a_look_over_sixty_worktrees_ends_whatever_du_takes_audit_e6() {
    let dir = std::env::temp_dir().join(format!("wisp-wt-sixty-{}", std::process::id()));
    for i in 0..60 {
        std::fs::create_dir_all(dir.join(format!("h/worktrees/proj-{i:02}"))).unwrap();
        std::fs::write(dir.join(format!("h/worktrees/proj-{i:02}/a.txt")), "x").unwrap();
    }
    let git = wisp_git::service(wisp_proc::local(), None, Arc::new(|| 1_000_000), None);
    let engine = Engine::new(
        Arc::new(wisp_git::Service(git)),
        Arc::new(SlowDu(wisp_proc::local())),
        wisp_testkit::FakeClock::at(1_000_000).service(),
        dir.join("h").to_string_lossy().into_owned(),
        dir.join("u").to_string_lossy().into_owned(),
        "macos",
        Vec::new(),
        CancellationToken::new(),
    );
    let c = Cleanup::new(Arc::new(engine), MemPort::new(Vec::new(), WtSettings::default()), Call::new_root(Caller::user()));
    c.set_look_max(10_000);
    let at = std::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(30), c.scan()).await.expect("the look ends");
    assert!(at.elapsed() < Duration::from_secs(9), "it ended on its own, not cut at its limit: {:?}", at.elapsed());
    let v = c.view(false).await;
    assert!(!v.looking, "done looking");
    assert_eq!(v.leftovers.len(), 60, "every worktree found");
    std::fs::remove_dir_all(&dir).ok();
}
