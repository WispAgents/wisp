//! What a removal never takes, whoever asks (audit, git round 1). A recurring run's kept folder is shared by its runs
//! (ADR-0108): while any task works in it, the sweep, Done, archiving and a run ending all leave it. And a bad record
//! never costs the user a repo: the main checkout, a folder outside wisp's worktrees, a worktree git refuses to remove
//! (locked), one git can't read, and a full clone left in wisp's worktrees folder all stay. Over real repos in temp
//! folders and the real git service.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::many_single_char_names)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use wisp_kernel::{Call, Caller, CancellationToken};
use wisp_runtime_api::model::{Session, SessionState};
use wisp_worktrees::cleanup::Cleanup;
use wisp_worktrees::{Engine, MemPort, TaskInfo, TaskTree, WtSettings, archived, done};
use wisp_worktrees_api::{CleanArgs, DoneApplyArgs, EXT, RemoveOpts, Worktree};
use wisp_worktrees_core::sweep::{DAY, DiskLimits};

static N: AtomicU32 = AtomicU32::new(0);

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmp(name: &str) -> Tmp {
    let d = std::env::temp_dir().join(format!("wisp-wtu-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    Tmp(wisp_proc::canonical_blocking(&d).unwrap())
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).env("GIT_CONFIG_NOSYSTEM", "1").output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// `<t>/src/app` on `main`, one commit, no remote.
fn repo(t: &Tmp) -> PathBuf {
    let dir = t.0.join("src/app");
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    for (k, v) in [("user.email", "qa@wisp.test"), ("user.name", "QA"), ("commit.gpgsign", "false")] {
        git(&dir, &["config", k, v]);
    }
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    dir
}

/// A worktree on a new branch at `path`, as wisp records one it made.
fn add_tree(root: &Path, path: &Path, branch: &str) -> Worktree {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    git(root, &["worktree", "add", "-q", "-b", branch, path.to_str().unwrap()]);
    let p = path.to_string_lossy().into_owned();
    Worktree {
        path: p.clone(),
        cwd: p,
        branch: branch.into(),
        base: "main".into(),
        base_ref: "main".into(),
        root: root.to_string_lossy().into_owned(),
        created_at: 1,
        ..Worktree::default()
    }
}

fn engine(t: &Tmp, now: i64) -> Engine {
    let git = wisp_git::service(wisp_proc::local(), None, Arc::new(|| 1_000_000), None);
    Engine::new(
        Arc::new(wisp_git::Service(git)),
        wisp_proc::local(),
        wisp_testkit::FakeClock::at(now).service(),
        t.0.join("h").to_string_lossy().into_owned(),
        t.0.join("u").to_string_lossy().into_owned(),
        "macos",
        Vec::new(),
        CancellationToken::new(),
    )
}

fn user() -> Call {
    Call::new_root(Caller::user())
}

fn task(id: &str, wt: &Worktree) -> TaskInfo {
    TaskInfo {
        id: id.into(),
        title: id.into(),
        project: "app".into(),
        trees: vec![TaskTree { worktree: wt.clone(), pr: None }],
        ..TaskInfo::default()
    }
}

fn set(port: &MemPort, id: &str, f: impl Fn(&mut TaskInfo)) {
    for t in port.tasks.lock().unwrap().iter_mut().filter(|t| t.id == id) {
        f(t);
    }
}

/// Two runs of one recurring job share its kept folder: A is done and idle past `DONE_IDLE`, B works in it. The sweep,
/// Done on A, archiving A and A's run ending each leave the folder (said in use, never a failure); once B stops, the
/// sweep takes it.
#[tokio::test(flavor = "multi_thread")]
async fn a_shared_kept_folder_stays_while_any_task_works_in_it() {
    let t = tmp("shared");
    let root = repo(&t);
    // the clock two days past now: the folder's mtime (now) is long idle for a Done task
    let now = i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()).unwrap() + 2 * DAY;
    let e = Arc::new(engine(&t, now));
    let wt = add_tree(&root, &t.0.join("h/worktrees/app-ada-check-in"), "wisp/ada-check-in");
    let a = TaskInfo { done: true, updated_at: 1, ..task("a", &wt) };
    let b = TaskInfo { busy: true, updated_at: now, ..task("b", &wt) };
    let port = MemPort::new(vec![a, b], WtSettings::default());
    let c = Cleanup::new(e.clone(), port.clone(), user());
    let limits = DiskLimits { low: 0, enough: 0 };

    c.sweep(limits).await;
    assert!(Path::new(&wt.path).is_dir(), "the sweep: B works in it");

    let args = DoneApplyArgs { session_id: "a".into(), remove: vec![wt.path.clone()], keep: Vec::new(), force: Vec::new() };
    let out = done::apply(&e, port.as_ref(), &user(), &args).await.unwrap();
    assert!(out.removed.is_empty() && Path::new(&wt.path).is_dir(), "Done on A");
    assert_eq!(out.failed[0].error, "It’s in use.", "said in use, not a failure to remove");
    set(&port, "a", |x| x.kept = false);

    let r = archived::tidy(&e, &user(), &wt, e.mark()).await.unwrap();
    assert!(!r.removed && r.busy && Path::new(&wt.path).is_dir(), "archiving A: {r:?}");

    let mut ended = Session { id: "a".to_owned().into(), state: SessionState::Review, ..Session::default() };
    ended.ext.insert(EXT.into(), serde_json::to_value(Worktree { tidy: true, ..wt.clone() }).unwrap());
    let r = archived::tidy_ended(&e, &user(), &ended, e.mark(), &[], false).await.map(|(_, r)| r).unwrap();
    assert!(!r.removed && Path::new(&wt.path).is_dir(), "A's run ending: {r:?}");

    set(&port, "b", |x| x.busy = false);
    c.sweep(limits).await;
    assert!(!Path::new(&wt.path).exists(), "B stopped: the sweep takes it");
}

/// `Engine::remove` never deletes the repo itself, a folder outside `<home>/worktrees` that no task made, a worktree
/// git refuses to remove (locked), or one whose status git can't read (it counts as holding work).
#[tokio::test]
async fn a_removal_keeps_the_repo_a_stray_folder_a_locked_worktree_and_one_git_cant_read() {
    let t = tmp("guard");
    let root = repo(&t);
    let e = engine(&t, 1_000_000);
    let force = RemoveOpts { force: true, ..RemoveOpts::default() };
    let r = root.to_string_lossy().into_owned();
    let itself = Worktree { path: r.clone(), cwd: r.clone(), root: r, branch: "main".into(), created_at: 1, ..Worktree::default() };
    assert!(e.remove(&user(), &itself, force, None).await.is_err(), "the main checkout: refused");
    assert!(root.join("a.txt").is_file() && root.join(".git").is_dir());

    let stray = Worktree { created_at: 0, ..add_tree(&root, &t.0.join("elsewhere/app-x"), "wisp/x") };
    assert!(e.remove(&user(), &stray, force, None).await.is_err(), "outside wisp's worktrees, made by no task");
    assert!(Path::new(&stray.path).join("a.txt").is_file());

    let locked = add_tree(&root, &t.0.join("h/worktrees/app-locked"), "wisp/locked");
    git(&root, &["worktree", "lock", &locked.path]);
    assert!(e.remove(&user(), &locked, RemoveOpts::default(), None).await.is_err(), "git refuses: its error");
    assert!(Path::new(&locked.path).join("a.txt").is_file(), "and the folder stays");

    let broken = add_tree(&root, &t.0.join("h/worktrees/app-broken"), "wisp/broken");
    std::fs::write(root.join(".git/worktrees/app-broken/index"), "not an index").unwrap();
    let r = e.remove(&user(), &broken, RemoveOpts::default(), None).await.unwrap();
    assert!(!r.removed && r.dirty.is_some_and(|n| n >= 1), "git can't tell: kept as dirty {r:?}");
    assert!(Path::new(&broken.path).is_dir());
}

/// A full clone someone left in wisp's worktrees folder (its `.git` a folder) is listed as a leftover, but removing
/// it by hand skips it: it was never a worktree of wisp's.
#[tokio::test(flavor = "multi_thread")]
async fn a_leftover_that_is_a_full_clone_is_skipped() {
    let t = tmp("clone");
    let e = Arc::new(engine(&t, 1_000_000));
    let clone = t.0.join("h/worktrees/a-clone");
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q", "-b", "main"]);
    std::fs::write(clone.join("work.txt"), "mine\n").unwrap();
    let c = Cleanup::new(e, MemPort::new(Vec::new(), WtSettings::default()), user());
    c.scan().await;
    let args = CleanArgs { ids: Some(Vec::new()), leftovers: vec!["a-clone".into()], force: true };
    assert_eq!(c.clean(args).await.unwrap().queued, 1);
    tokio::time::timeout(Duration::from_secs(20), async {
        while c.view(false).await.cleaning.is_some() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(clone.join("work.txt").is_file() && clone.join(".git").is_dir(), "the clone stays");
}
