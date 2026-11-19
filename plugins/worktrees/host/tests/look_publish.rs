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
