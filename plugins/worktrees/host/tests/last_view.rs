//! Audit C3: This computer showed "Disk — free" and "Nothing to free" until something asked Settings › Worktrees for a
//! full view: its sample reads `worktrees.viewLast` (the last look, never a new one), and on a server that never looked
//! there was none. The disk's free and total don't need a look: the last view reads them at once. And the first time
//! it's asked, with no look ever made, one starts, so wisp's folders' sizes fill in.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use wisp_kernel::{Call, Caller, CancellationToken};
use wisp_worktrees::cleanup::Cleanup;
use wisp_worktrees::{Engine, MemPort, WtSettings};

#[tokio::test(flavor = "multi_thread")]
async fn the_last_view_reads_the_disk_at_once_and_starts_the_first_look() {
    let dir = std::env::temp_dir().join(format!("wisp-wt-last-{}", std::process::id()));
    // a fresh home: no worktrees folder yet (the disk is still there to read)
    std::fs::create_dir_all(dir.join("h")).unwrap();
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
    let c = Cleanup::new(Arc::new(engine), MemPort::new(Vec::new(), WtSettings::default()), Call::new_root(Caller::user()));
    let wait = Duration::from_secs(10);
    let v = tokio::time::timeout(wait, c.view(false)).await.expect("the view answers");
    let d = v.disk.expect("the disk's free and total, without a look");
    assert!(d.total > 0 && d.free <= d.total, "{d:?}");
    // the first look was started by that view: it ends, and the pane no longer says it's looking
    tokio::time::timeout(wait, async {
        while c.view(false).await.looking {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("a first look ran");
    std::fs::remove_dir_all(&dir).ok();
}
