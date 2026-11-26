//! Audit fork-turn-includes-later: a fork from an earlier turn got a copy of its parent's folder as it is now, the
//! later turns' edits in it. Given where the folder was then (git's checkpoint before the turn after it), the fork's
//! branch starts at that commit and its folder holds that snapshot. Over a real repo and the real git service.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use wisp_kernel::{Call, Caller, CancellationToken};
use wisp_worktrees::Engine;
use wisp_worktrees_api::{CreateArgs, ForkPoint};

static N: AtomicU32 = AtomicU32::new(0);

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmp(name: &str) -> Tmp {
    let d = std::env::temp_dir().join(format!("wisp-wtf-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    Tmp(wisp_proc::canonical_blocking(&d).unwrap())
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).env("GIT_CONFIG_NOSYSTEM", "1").output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn commit(dir: &Path, text: &str) -> String {
    std::fs::write(dir.join("a.txt"), text).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", text.trim()]);
    git(dir, &["rev-parse", "HEAD"])
}

fn engine(t: &Tmp) -> Engine {
    let git = wisp_git::service(wisp_proc::local(), None, Arc::new(|| 1_000_000), None);
    Engine::new(
        Arc::new(wisp_git::Service(git)),
        wisp_proc::local(),
        wisp_testkit::FakeClock::at(1_000_000).service(),
        t.0.join("h").to_string_lossy().into_owned(),
        t.0.join("u").to_string_lossy().into_owned(),
        "macos",
        Vec::new(),
        CancellationToken::new(),
    )
}

#[tokio::test]
async fn a_fork_from_an_earlier_turn_gets_the_folder_as_it_was_then_not_as_it_is_now() {
    let t = tmp("at");
    let dir = t.0.join("src/app");
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    for (k, v) in [("user.email", "qa@wisp.test"), ("user.name", "QA"), ("commit.gpgsign", "false")] {
        git(&dir, &["config", k, v]);
    }
    let head = commit(&dir, "one\n");
    // the folder before the next turn: the turn's work so far (here a commit standing for git's snapshot)
    let then = commit(&dir, "then\n");
    // a later turn's edit, in the folder now
    commit(&dir, "now\n");
    std::fs::write(dir.join("a.txt"), "now, uncommitted\n").unwrap();
    let cwd = dir.to_string_lossy().into_owned();
    let a = CreateArgs {
        session_id: format!("{:032x}", 7),
        cwd: cwd.clone(),
        title: "Fork".into(),
        from: Some(cwd),
        at: Some(ForkPoint { head: head.clone(), to: then }),
        ..CreateArgs::default()
    };
    let wt = engine(&t).create(&Call::new_root(Caller::user()), &a, None, &[]).await.unwrap().unwrap();
    assert_eq!(std::fs::read_to_string(Path::new(&wt.path).join("a.txt")).unwrap(), "then\n", "its files as they were then");
    assert_eq!(git(Path::new(&wt.path), &["rev-parse", "HEAD"]), head, "on a branch from the commit it was on then");
}
