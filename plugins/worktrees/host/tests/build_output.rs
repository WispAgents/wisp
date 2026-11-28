//! Build output (ADR-0144): `target/` and `node_modules/` go, the folder, its branch and its changes stay. A leftover's
//! Remove takes only that leftover's (it once took every task folder's), only what git ignores goes (a tracked folder
//! named `target` stays, and a `target` without cargo's tag), and a low disk takes a leftover's first. Over real repos
//! in temp folders and the real git service.
// Unix only: sizes and build output come from `du`, which Windows has no copy of on its PATH, so a Windows home's are
// never sized and nothing is trimmed there yet (the sizes need a Windows walk of their own)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::many_single_char_names)]
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use wisp_kernel::{Call, Caller, CancellationToken};
use wisp_worktrees::cleanup::Cleanup;
use wisp_worktrees::{Engine, MemPort, TaskInfo, TaskTree, WtSettings};
use wisp_worktrees_api::{CleanArgs, Worktree};
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

/// `target/` (with cargo's tag) and `node_modules/` in `dir`, ignored by the repo, and one uncommitted change.
fn build_in(dir: &Path) {
    std::fs::write(dir.join(".gitignore"), "target/\nnode_modules/\n").unwrap();
    std::fs::create_dir_all(dir.join("target/debug")).unwrap();
    std::fs::write(dir.join("target/CACHEDIR.TAG"), "Signature: 8a477f597d28d172789f06886806bc55\n").unwrap();
    std::fs::write(dir.join("target/debug/big.bin"), vec![0u8; 64 * 1024]).unwrap();
    std::fs::create_dir_all(dir.join("node_modules/x")).unwrap();
    std::fs::write(dir.join("node_modules/x/i.js"), "x").unwrap();
    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
}

async fn idle(c: &Arc<Cleanup>) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while c.view(false).await.cleaning.is_some() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}

/// Looks until the sizes (and so the build output) are in.
async fn sized(c: &Arc<Cleanup>) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            c.scan().await;
            let v = c.view(false).await;
            if v.leftovers.iter().all(|l| l.build.is_some()) && v.items.iter().all(|i| i.build.is_some()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
}

fn now_plus(days: i64) -> i64 {
    i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()).unwrap() + days * DAY
}

#[tokio::test(flavor = "multi_thread")]
async fn a_leftovers_remove_takes_only_its_build_output_and_keeps_its_changes() {
    let t = tmp("build-left");
    let root = repo(&t);
    let e = Arc::new(engine(&t, now_plus(2)));
    let task_tree = add_tree(&root, &t.0.join("h/worktrees/app-task"), "wisp/task");
    build_in(Path::new(&task_tree.path));
    let left = add_tree(&root, &t.0.join("h/worktrees/app-left"), "wisp/left");
    build_in(Path::new(&left.path));
    let port = MemPort::new(vec![task("a", &task_tree)], WtSettings::default());
    let c = Cleanup::new(e, port, user());
    sized(&c).await;
    let v = c.view(false).await;
    assert!(v.leftovers[0].build.unwrap() > 60 * 1024, "the leftover's build output is counted: {:?}", v.leftovers);

    // the UI's leftover Remove: no ids, one leftover
    let args = CleanArgs { leftovers: vec!["app-left".into()], build: true, ..CleanArgs::default() };
    assert_eq!(c.clean(args).await.unwrap().queued, 1);
    idle(&c).await;
    let l = Path::new(&left.path);
    assert!(!l.join("target").exists() && !l.join("node_modules").exists(), "the leftover's build output went");
    assert_eq!(std::fs::read_to_string(l.join("a.txt")).unwrap(), "changed\n", "its change stays");
    assert!(l.join(".git").exists(), "the folder stays a worktree");
    let rec = c.take_cleaned().expect("an ended clean-up is told (worktrees.cleaned)");
    assert_eq!(rec.count, 1);
    assert!(rec.freed > 60 * 1024);
    assert!(c.take_cleaned().is_none(), "once");
    let k = Path::new(&task_tree.path);
    assert!(k.join("target/debug/big.bin").is_file() && k.join("node_modules/x").is_dir(), "the task folder's stays");
}

#[tokio::test(flavor = "multi_thread")]
async fn only_ignored_build_output_goes_and_a_target_without_cargos_tag_stays() {
    let t = tmp("build-ignored");
    let root = repo(&t);
    let e = Arc::new(engine(&t, now_plus(2)));
    let wt = add_tree(&root, &t.0.join("h/worktrees/app-x"), "wisp/x");
    let dir = Path::new(&wt.path);
    // a tracked folder named node_modules (vendored), and a target/ git ignores but cargo never made
    std::fs::create_dir_all(dir.join("node_modules")).unwrap();
    std::fs::write(dir.join("node_modules/keep.js"), "mine").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["-c", "user.email=q@w", "-c", "user.name=q", "commit", "-q", "-m", "vendor"]);
    std::fs::write(dir.join(".gitignore"), "target/\n").unwrap();
    std::fs::create_dir_all(dir.join("target")).unwrap();
    std::fs::write(dir.join("target/notes.md"), "mine too").unwrap();
    let port = MemPort::new(vec![task("x", &wt)], WtSettings::default());
    let c = Cleanup::new(e, port, user());
    sized(&c).await;
    let args = CleanArgs { ids: Some(vec!["x".into()]), build: true, ..CleanArgs::default() };
    c.clean(args).await.unwrap();
    idle(&c).await;
    assert!(dir.join("node_modules/keep.js").is_file(), "git tracks it: it stays");
    assert!(dir.join("target/notes.md").is_file(), "no CACHEDIR.TAG: not cargo's, it stays");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_low_disk_takes_a_leftovers_build_output_first_and_keeps_the_folder() {
    let t = tmp("build-low");
    let root = repo(&t);
    let e = Arc::new(engine(&t, now_plus(2)));
    let left = add_tree(&root, &t.0.join("h/worktrees/app-left"), "wisp/left");
    build_in(Path::new(&left.path));
    let c = Cleanup::new(e, MemPort::new(Vec::new(), WtSettings::default()), user());
    sized(&c).await;
    // every disk is "low" under u64::MAX free
    let out = c.sweep(DiskLimits { low: u64::MAX, enough: u64::MAX }).await;
    assert_eq!(out.removed, 1);
    let l = Path::new(&left.path);
    assert!(!l.join("target").exists(), "its build output went");
    assert!(l.join("a.txt").is_file() && l.join(".git").exists(), "the folder and its change stay");
}
