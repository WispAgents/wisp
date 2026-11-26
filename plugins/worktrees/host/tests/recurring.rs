//! Recurring runs never grow worktrees without bound (ADR-0108, audit C3: a crew member's check-ins and an every-minute
//! trigger took one project from 25 to 65 worktrees in 40 minutes, past `worktrees.max` 40). Over real repos in temp
//! folders and the real git service: a member's check-ins reuse one folder (`wisp/<member>-check-in`), a run another
//! working task is in gets a fresh one, and a run started for you that changed nothing leaves no folder and no branch.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use wisp_kernel::{Call, Caller, CancellationToken};
use wisp_runtime_api::model::{Session, SessionState};
use wisp_worktrees::{Engine, archived};
use wisp_worktrees_api::{CreateArgs, EXT, Worktree};

static N: AtomicU32 = AtomicU32::new(0);

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmp(name: &str) -> Tmp {
    let d = std::env::temp_dir().join(format!("wisp-wtr-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn user() -> Call {
    Call::new_root(Caller::user())
}

fn args(root: &Path, id: &str, reuse: Option<&str>) -> CreateArgs {
    CreateArgs {
        session_id: id.into(),
        cwd: root.to_string_lossy().into_owned(),
        title: "Ada · Check-in".into(),
        reuse: reuse.map(str::to_owned),
        tidy: true,
        ..CreateArgs::default()
    }
}

/// The worktrees git knows for the repo, the main checkout left out.
fn trees(root: &Path) -> Vec<String> {
    git(root, &["worktree", "list", "--porcelain"])
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .filter(|p| Path::new(p) != root)
        .map(str::to_owned)
        .collect()
}

/// A task marked Done, in `wt`.
fn ended(id: &str, wt: &Worktree) -> Session {
    let mut s = Session { id: id.to_owned().into(), state: SessionState::Done, ..Session::default() };
    s.ext.insert(EXT.into(), serde_json::to_value(wt).unwrap());
    s
}

#[tokio::test]
async fn a_members_check_ins_reuse_one_worktree_and_a_busy_one_gets_a_fresh_folder() {
    let t = tmp("reuse");
    let root = repo(&t);
    let e = engine(&t);
    let mut paths = Vec::new();
    for i in 0..6 {
        let wt = e.create(&user(), &args(&root, &format!("{i:032x}"), Some("Ada check-in")), None, &[]).await.unwrap().unwrap();
        assert_eq!(wt.branch, "wisp/ada-check-in");
        assert!(wt.tidy);
        // each check-in leaves an edit behind (the fake's): the folder stays, the next one works on in it
        std::fs::write(Path::new(&wt.path).join(format!("note-{i}.txt")), "x\n").unwrap();
        paths.push(wt.path);
    }
    paths.dedup();
    assert_eq!(paths.len(), 1, "six check-ins, one folder: {paths:?}");
    assert_eq!(trees(&root).len(), 1, "git knows one worktree besides the checkout");
    // the cap holds: however many check-ins run, the member has one folder in the project
    // a check-in while the last one still works in its folder: a fresh one of its own, not the same folder
    let busy = e.create(&user(), &args(&root, &format!("{:032x}", 99), Some("Ada check-in")), None, &paths).await.unwrap().unwrap();
    assert_ne!(busy.path, paths[0]);
    assert_eq!(trees(&root).len(), 2);
    // its folder was removed (a clean run): the next check-in gets it back, on its branch
    let kept = &paths[0];
    git(&root, &["-C", kept, "add", "-A"]);
    git(&root, &["-C", kept, "commit", "-q", "-m", "notes"]);
    git(&root, &["worktree", "remove", "--force", kept]);
    let again = e.create(&user(), &args(&root, &format!("{:032x}", 100), Some("Ada check-in")), None, &[]).await.unwrap().unwrap();
    assert_eq!(&again.path, kept);
    assert!(Path::new(kept).join("note-0.txt").exists(), "back on its branch, with what was committed there");
}

#[tokio::test]
async fn a_run_started_for_you_that_changed_nothing_leaves_no_worktree_and_one_with_work_stays() {
    let t = tmp("tidy");
    let root = repo(&t);
    let e = engine(&t);
    // a trigger's run that changed nothing: no folder and no branch left
    let wt = e.create(&user(), &args(&root, &format!("{:032x}", 1), Some("trigger-uitrig")), None, &[]).await.unwrap().unwrap();
    let (gone, r) = archived::tidy_ended(&e, &user(), &ended("t1", &wt), e.mark(), &[], false).await.unwrap();
    assert!(r.removed && r.branch_deleted, "{r:?}");
    assert_eq!(gone.path, wt.path);
    assert_eq!(trees(&root), Vec::<String>::new());
    assert_eq!(git(&root, &["branch", "--list", "wisp/trigger-uitrig"]), "");
    // a crew job (no key) that left an edit: it stays
    let job = e.create(&user(), &args(&root, &format!("{:032x}", 2), None), None, &[]).await.unwrap().unwrap();
    std::fs::write(Path::new(&job.path).join("b.txt"), "work\n").unwrap();
    let r = archived::tidy_ended(&e, &user(), &ended("t2", &job), e.mark(), &[], false).await.map(|(_, r)| r).unwrap();
    assert!(!r.removed && r.dirty.is_some(), "{r:?}");
    // still running, a task you started (not tidy), or another working task in the folder: nothing happens
    let mut running = ended("t3", &wt);
    running.state = SessionState::Running;
    assert!(archived::to_tidy(&running, 0).is_none());
    // a turn that ended (Review) isn't done: it may have asked you something (ADR-0131); archived, it is
    let mut asked = ended("t3", &wt);
    asked.state = SessionState::Review;
    assert!(archived::to_tidy(&asked, 0).is_none(), "a turn ending alone never tidies");
    asked.archived = true;
    assert!(archived::to_tidy(&asked, 0).is_some());
    let yours = Worktree { tidy: false, ..job.clone() };
    assert!(archived::to_tidy(&ended("t4", &yours), 0).is_none());
    let shared = e.create(&user(), &args(&root, &format!("{:032x}", 3), Some("trigger-uitrig")), None, &[]).await.unwrap().unwrap();
    assert!(archived::tidy_ended(&e, &user(), &ended("t5", &shared), e.mark(), std::slice::from_ref(&shared.path), false).await.is_none());
    assert!(Path::new(&shared.path).exists());
}

/// The check-in that asked a question (2026-11-26): its folder and empty branch went, and the follow-up was refused with
/// "its branch is gone". Now the follow-up goes on in the worktree made again from the base, and says so once.
#[tokio::test]
async fn a_follow_up_after_its_branch_went_goes_on_in_a_fresh_worktree_from_the_base() {
    let t = tmp("fresh");
    let root = repo(&t);
    let e = engine(&t);
    let wt = e.create(&user(), &args(&root, &format!("{:032x}", 7), Some("Ada check-in")), None, &[]).await.unwrap().unwrap();
    let (_, r) = archived::tidy_ended(&e, &user(), &ended("t7", &wt), e.mark(), &[], false).await.unwrap();
    assert!(r.removed && r.branch_deleted, "{r:?}");
    assert_eq!(git(&root, &["branch", "--list", &wt.branch]), "");
    // main moved on meanwhile: the fresh branch starts where main is now
    std::fs::write(root.join("c.txt"), "later\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "later"]);
    // a plain ensure (worktrees.ensure) still says the branch is gone, in plain words
    let err = e.ensure(&user(), &wt).await.unwrap_err().to_string();
    assert!(err.contains("is gone"), "{err}");
    let who = wisp_worktrees::Claimant { task: "t7", turn: Some("turn-2") };
    assert!(archived::bring_back(&e, &user(), who, &wt).await.unwrap(), "made again from the base");
    assert!(Path::new(&wt.path).join("c.txt").exists());
    assert_eq!(git(&root, &["rev-parse", &wt.branch]), git(&root, &["rev-parse", "main"]));
    // the next turn finds it there: nothing to say again
    assert!(!archived::bring_back(&e, &user(), who, &wt).await.unwrap());
    let note = archived::fresh_note(&wt);
    assert!(note.contains("was deleted") && note.contains(&wt.branch), "{note}");
}
