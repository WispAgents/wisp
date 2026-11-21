//! Audit A (2026-11-18, /tmp/qa-keep/audit-A-tasks.md, audit-A-orchestration.md): what a task does to folders it
//! doesn't own, end to end on a real `wisp serve` with the fake agent and real git in temp repos.
//!
//! - tasks #1: Restore to before this turn reset the project's main checkout, not the task's worktree.
//! - orch #3, tasks #5: Merge removed a worktree that had uncommitted work.
//! - orch #4, tasks #2: a fork ran in its parent's worktree.
//! - tasks #3: a rerun of a worktree task ran in the main checkout.
//! - orch #7: Compare from the CLI or API ran both agents in one folder.
//! - orch #6: Bring in failed for every worktree, and offered wisp's own.
//! - orch #21: Fix conflicts spent a turn when git saw no conflict.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Api, ended, start_with, temp};
use serde_json::{Value, json};

const BEFORE: &str = "export const add = (a, b) => a - b;\n";

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

/// `proj` as a repo with one commit, pushed to a bare `origin` beside it.
fn repo(dir: &Path) -> PathBuf {
    let repo = wisp_proc::canonical_blocking(dir.join("proj")).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    // a Windows runner's git has core.autocrlf=true: checkouts (a worktree, a fork's copy, a restore) would write the
    // files back with CRLF, as git is told to, and v1's fork did too. The repo's own setting keeps the bytes as written.
    git(&repo, &["config", "core.autocrlf", "false"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    let origin = dir.join("origin.git");
    git(dir, &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&repo, &["push", "-q", "-u", "origin", "main"]);
    repo
}

fn read(p: impl AsRef<Path>) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

struct Fixture {
    _serve: common::Serve,
    api: Api,
    repo: PathBuf,
    state: PathBuf,
}

fn fixture(name: &str) -> Fixture {
    let dir = temp(name);
    let repo = repo(&dir);
    let bin = dir.join("ghbin");
    common::install_fakes(&bin, &["gh"]);
    let state = dir.join("gh-state.json");
    std::fs::write(&state, json!({ "prs": {} }).to_string()).unwrap();
    // ship.ship commits in the task's worktree: a CI runner has no git identity (no ~/.gitconfig), and v1 never
    // invented one for a user's commit, so the test gives the server one, as `git()` does for the test's own commits
    let (serve, port) = start_with(
        &dir,
        &[
            ("WISP_GH_BIN", bin.join("gh").to_str().unwrap()),
            ("FAKE_GH_STATE", state.to_str().unwrap()),
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@t"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@t"),
        ],
    );
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    Fixture { _serve: serve, api, repo, state }
}

/// A task in its own worktree whose first turn (the fake's edit of math.js) has ended: (id, its slot).
async fn task_in_a_worktree(f: &Fixture, prompt: &str) -> (String, Value) {
    let made = f.api.cmd("runtime.create", json!({ "prompt": prompt, "cwd": f.repo, "agent": "claude", "worktree": true })).await;
    let id = made["id"].as_str().unwrap().to_owned();
    f.api.until(&id, "the first turn ends", |t| t.last().is_some_and(ended)).await;
    let wt = f.api.cmd("runtime.get", json!({ "id": id })).await["ext"]["wisp.worktrees"].clone();
    assert!(wt["path"].is_string(), "it ran in a worktree: {wt}");
    (id, wt)
}

fn path_of(wt: &Value) -> PathBuf {
    PathBuf::from(wt["path"].as_str().unwrap())
}

/// tasks #1: the checkpoint is put back in the task's own folder; the main checkout, with a change and an untracked
/// file of the user's, is left as it was. The answer can undo it.
#[tokio::test(flavor = "multi_thread")]
async fn restore_puts_back_the_tasks_worktree_and_never_touches_the_main_checkout() {
    let f = fixture("wt-safe-restore");
    let (id, wt) = task_in_a_worktree(&f, "fix it").await;
    let tree = path_of(&wt);
    let edited = read(tree.join("math.js"));
    assert_ne!(edited, BEFORE, "the fake edited the worktree");
    // the user's own work in the main checkout
    std::fs::write(f.repo.join("math.js"), "// mine, not committed\n").unwrap();
    std::fs::write(f.repo.join("untracked.txt"), "mine too\n").unwrap();
    let turn = f.api.turns(&id).await[0]["id"].as_str().unwrap().to_owned();
    let r = f.api.cmd("git.restoreTurn", json!({ "id": id, "turn": turn })).await;
    assert_eq!(r["restored"], true, "{r}");
    assert_eq!(read(tree.join("math.js")), BEFORE, "the worktree is back to before the turn");
    assert_eq!(read(f.repo.join("math.js")), "// mine, not committed\n", "the main checkout's change stays");
    assert_eq!(read(f.repo.join("untracked.txt")), "mine too\n", "and its untracked file");
    // undo: the folder as it was just before the restore
    let undo = r["undo"].clone();
    assert!(undo["to"].is_string() && undo["head"].is_string(), "{r}");
    let back = f.api.cmd("git.restoreTurn", json!({ "id": id, "turn": turn, "undo": undo })).await;
    assert_eq!(back["restored"], true, "{back}");
    assert_eq!(read(tree.join("math.js")), edited, "undone: the turn's edit is back");
    assert_eq!(read(f.repo.join("untracked.txt")), "mine too\n");
}

/// orch #3, tasks #5: a merged task's worktree with uncommitted work is kept, and the answer says so; a clean one goes.
#[tokio::test(flavor = "multi_thread")]
async fn merge_keeps_a_worktree_with_uncommitted_work_and_says_so() {
    let f = fixture("wt-safe-merge");
    let (id, wt) = task_in_a_worktree(&f, "fix it").await;
    let tree = path_of(&wt);
    let shipped = f.api.cmd("ship.ship", json!({ "session": id, "title": "Fix it" })).await;
    assert!(shipped["url"].is_string() || shipped["pr"].is_object(), "{shipped}");
    std::fs::write(tree.join("precious.txt"), "precious work\n").unwrap();
    let merged = f.api.cmd("ship.merge", json!({ "session": id })).await;
    assert!(tree.join("precious.txt").exists(), "the worktree with uncommitted work stays: {merged}");
    let said = merged["sync"]["text"].as_str().unwrap_or_default();
    assert!(said.contains("kept") && said.contains("uncommitted"), "and the answer says why: {merged}");

    // a clean one goes after its merge, as before
    let (id2, wt2) = task_in_a_worktree(&f, "fix it again").await;
    // its work committed (and pushed by Ship below): nothing left only in the folder
    std::fs::write(path_of(&wt2).join("again.txt"), "done\n").unwrap();
    git(&path_of(&wt2), &["add", "-A"]);
    git(&path_of(&wt2), &["commit", "-qm", "the fix"]);
    std::fs::write(&f.state, json!({ "prs": {} }).to_string()).unwrap();
    f.api.cmd("ship.ship", json!({ "session": id2, "title": "Fix it again" })).await;
    f.api.cmd("ship.merge", json!({ "session": id2 })).await;
    let at = Instant::now();
    while path_of(&wt2).exists() {
        assert!(at.elapsed() < Duration::from_secs(20), "a clean worktree goes after its merge");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// orch #4, tasks #2: a fork gets its own worktree and branch, starting from a copy of its parent's files as they
/// are (uncommitted ones too); what one changes the other doesn't see.
#[tokio::test(flavor = "multi_thread")]
async fn a_fork_gets_its_own_worktree_and_branch_with_a_copy_of_the_files() {
    let f = fixture("wt-safe-fork");
    let (id, wt) = task_in_a_worktree(&f, "fix it").await;
    let parent = path_of(&wt);
    std::fs::write(parent.join("wip.txt"), "parent's work in progress\n").unwrap();
    let made = f.api.cmd("runtime.fork", json!({ "id": id })).await;
    let fork = made["id"].as_str().unwrap().to_owned();
    let s = f.api.cmd("runtime.get", json!({ "id": fork })).await;
    let fwt = s["ext"]["wisp.worktrees"].clone();
    let ftree = PathBuf::from(fwt["path"].as_str().unwrap_or_else(|| panic!("the fork has a worktree of its own: {s}")));
    assert_ne!(ftree, parent, "{s}");
    assert_ne!(fwt["branch"], wt["branch"], "and its own branch: {fwt}");
    assert_eq!(s["cwd"].as_str().unwrap(), fwt["cwd"].as_str().unwrap(), "it runs there: {s}");
    assert_eq!(read(ftree.join("math.js")), read(parent.join("math.js")), "a copy of the parent's files");
    assert_eq!(read(ftree.join("wip.txt")), "parent's work in progress\n", "uncommitted ones too");
    std::fs::write(ftree.join("math.js"), "// the fork's own\n").unwrap();
    assert_ne!(read(parent.join("math.js")), "// the fork's own\n", "the parent's folder is its own");
    assert_eq!(git(&f.repo, &["status", "--porcelain"]), "", "the main checkout untouched");
}

/// tasks #3: a rerun of a task that had a worktree gets one of its own; the main checkout stays clean.
#[tokio::test(flavor = "multi_thread")]
async fn a_rerun_of_a_worktree_task_runs_in_a_worktree_of_its_own() {
    let f = fixture("wt-safe-rerun");
    let (id, wt) = task_in_a_worktree(&f, "fix it").await;
    // what the Rerun dialog sends: the spec's folder (the project), and the rerun's lineage
    let made =
        f.api.cmd("runtime.create", json!({ "prompt": "fix it", "cwd": f.repo, "agent": "claude", "lineage": { "rerunOf": id } })).await;
    let again = made["id"].as_str().unwrap().to_owned();
    f.api.until(&again, "the rerun ends", |t| t.last().is_some_and(ended)).await;
    let s = f.api.cmd("runtime.get", json!({ "id": again })).await;
    let path = s["ext"]["wisp.worktrees"]["path"].as_str().unwrap_or_else(|| panic!("a worktree of its own: {s}"));
    assert_ne!(path, wt["path"].as_str().unwrap());
    assert_eq!(git(&f.repo, &["status", "--porcelain"]), "", "the main checkout untouched");
}

/// audit C8: a rerun at an older commit (`wisp rerun`, its spec's base) was checked out in a detached
/// `<home>/worktrees/rerun-<sha>-<x>` folder that stayed after the rerun moved into a worktree of its own (Settings ›
/// Worktrees listed it as "No task"). Once the rerun runs in its own worktree, that folder is gone, and the rerun's
/// branch starts at the commit it reran.
#[tokio::test(flavor = "multi_thread")]
async fn a_rerun_at_an_older_commit_leaves_no_detached_folder_and_runs_at_that_commit() {
    let f = fixture("wt-safe-rerun-sha");
    let old = git(&f.repo, &["rev-parse", "HEAD"]);
    std::fs::write(f.repo.join("later.txt"), "a later commit\n").unwrap();
    git(&f.repo, &["add", "."]);
    git(&f.repo, &["commit", "-qm", "later"]);
    let made = f
        .api
        .cmd("runtime.create", json!({ "prompt": "fix it", "cwd": f.repo, "agent": "claude", "worktree": true, "base": { "sha": old } }))
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    f.api.until(&id, "the rerun ends", |t| t.last().is_some_and(ended)).await;
    let s = f.api.cmd("runtime.get", json!({ "id": id })).await;
    let path = PathBuf::from(s["ext"]["wisp.worktrees"]["path"].as_str().unwrap_or_else(|| panic!("a worktree of its own: {s}")));
    let list = git(&f.repo, &["worktree", "list", "--porcelain"]);
    assert!(!list.contains("/worktrees/rerun-") && !list.lines().any(|l| l == "detached"), "no detached rerun folder left: {list}");
    let home = path.parent().unwrap();
    let left: Vec<String> =
        std::fs::read_dir(home).unwrap().filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(!left.iter().any(|n| n.starts_with("rerun-")), "the folder went too: {left:?}");
    assert!(!path.join("later.txt").exists(), "it runs on the commit it reran, not today's");
    let branch = s["ext"]["wisp.worktrees"]["branch"].as_str().unwrap().to_owned();
    assert_eq!(git(&f.repo, &["merge-base", &old, &branch]), old, "its branch starts at the commit it reran");
}

/// orch #7: Compare's runs (one group) each get a worktree and branch of their own, whatever Worktree per task says.
#[tokio::test(flavor = "multi_thread")]
async fn compare_runs_each_get_a_worktree_of_their_own() {
    let f = fixture("wt-safe-compare");
    let mut ids = Vec::new();
    for agent in ["claude", "codex"] {
        let a = json!({ "prompt": "fix it", "cwd": f.repo, "agent": agent, "ext": { "wisp.runtime": { "group": "g-cmp" } } });
        ids.push(f.api.cmd("runtime.create", a).await["id"].as_str().unwrap().to_owned());
    }
    let mut paths = Vec::new();
    for id in &ids {
        f.api.until(id, "its run ends", |t| t.last().is_some_and(ended)).await;
        let s = f.api.cmd("runtime.get", json!({ "id": id })).await;
        paths.push(s["ext"]["wisp.worktrees"]["path"].as_str().unwrap_or_else(|| panic!("a worktree of its own: {s}")).to_owned());
    }
    assert_ne!(paths[0], paths[1]);
    assert_eq!(git(&f.repo, &["status", "--porcelain"]), "", "the main checkout untouched");
}

/// orch #6: Bring in makes a task of a worktree another tool left, on its branch; wisp's own task worktrees are never
/// offered.
#[tokio::test(flavor = "multi_thread")]
async fn bring_in_makes_a_task_of_a_stray_worktree_and_never_offers_wisps_own() {
    let f = fixture("wt-safe-bring");
    let (_, wt) = task_in_a_worktree(&f, "fix it").await;
    let project = f.api.cmd("runtime.projectAdd", json!({ "path": f.repo })).await["name"].as_str().unwrap().to_owned();
    let stray = f.repo.parent().unwrap().join("stray");
    git(&f.repo, &["worktree", "add", "-q", stray.to_str().unwrap(), "-b", "stray-branch"]);
    let stray = wisp_proc::canonical_blocking(&stray).unwrap().display().to_string();
    f.api.cmd("worktrees.view", json!({})).await; // the pane opened: offers are looked for again
    let at = Instant::now();
    let made = loop {
        let out = f.api.cmd("worktrees.bringIn", json!({ "project": project, "paths": [stray] })).await;
        if let Some(id) = out["created"].as_array().and_then(|c| c.first()).and_then(Value::as_str) {
            break id.to_owned();
        }
        assert!(at.elapsed() < Duration::from_secs(20), "the stray worktree is brought in: {out}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    let s = f.api.cmd("runtime.get", json!({ "id": made })).await;
    assert_eq!(s["ext"]["wisp.worktrees"]["branch"], "stray-branch", "on its own branch: {s}");
    assert_ne!(s["ext"]["wisp.runtime"]["readOnly"], true, "a task you go on with: {s}");
    // audit I IS13: it names the harness its next follow-up runs on (the default one), never none ("· Default model",
    // "Follow up with …")
    assert_eq!(s["agent"], "claude", "the default harness: {s}");
    let own = f.api.cmd("worktrees.bringIn", json!({ "project": project, "paths": [wt["path"]] })).await;
    assert_eq!(own["created"], json!([]), "wisp's own worktree is never offered: {own}");
}

/// orch #21: with GitHub saying the PR conflicts but git merging it cleanly with its base, Fix conflicts says so and
/// spends no turn.
#[tokio::test(flavor = "multi_thread")]
async fn fix_conflicts_spends_no_turn_when_git_sees_no_conflict() {
    let f = fixture("wt-safe-conflict");
    let (id, _) = task_in_a_worktree(&f, "fix it").await;
    f.api.cmd("ship.ship", json!({ "session": id, "title": "Fix it" })).await;
    let mut state: Value = serde_json::from_str(&read(&f.state)).unwrap();
    state["mergeable"] = json!("CONFLICTING");
    std::fs::write(&f.state, state.to_string()).unwrap();
    f.api.cmd("ship.refresh", json!({ "session": id })).await;
    let turns = f.api.turns(&id).await.len();
    let r = f.api.try_cmd("ship.fix_conflicts", json!({ "session": id })).await;
    assert!(r["error"]["message"].as_str().is_some_and(|m| m.contains("no conflict")), "it says git sees none: {r}");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(f.api.turns(&id).await.len(), turns, "no turn was spent");
}

/// Audit E6: with about 50 wisp worktrees, a new stray worktree wasn't offered in 40 s (well over a minute), though with
/// few it was in under a second: Bring in's look must not wait on anything that grows with wisp's own worktrees.
#[tokio::test(flavor = "multi_thread")]
async fn bring_in_offers_a_new_stray_worktree_in_seconds_with_fifty_worktrees_audit_e6() {
    let f = fixture("wt-safe-many");
    let project = f.api.cmd("runtime.projectAdd", json!({ "path": f.repo })).await["name"].as_str().unwrap().to_owned();
    // 50 wisp worktrees, the audit's: each made by its task's create (one at a time, so no two `git worktree add`s race
    // on the repo), their first turns running meanwhile, all ended before the look. One by one, each create waited for
    // its turn: ~60 s on Windows
    let mut ids = Vec::new();
    for i in 0..50 {
        let made =
            f.api.cmd("runtime.create", json!({ "prompt": format!("many {i}"), "cwd": f.repo, "agent": "claude", "worktree": true })).await;
        ids.push(made["id"].as_str().unwrap().to_owned());
    }
    for id in &ids {
        f.api.until(id, "the first turn ends", |t| t.last().is_some_and(ended)).await;
        let wt = f.api.cmd("runtime.get", json!({ "id": id })).await["ext"]["wisp.worktrees"].clone();
        assert!(wt["path"].is_string(), "it ran in a worktree: {wt}");
    }
    f.api.cmd("worktrees.view", json!({})).await; // the cleanup's look over the 50 starts
    let stray = f.repo.parent().unwrap().join("stray-many");
    git(&f.repo, &["worktree", "add", "-q", stray.to_str().unwrap(), "-b", "stray-many"]);
    let stray = wisp_proc::canonical_blocking(&stray).unwrap().display().to_string();
    let at = Instant::now();
    f.api.cmd("worktrees.view", json!({})).await; // the pane opened again
    loop {
        let out = f.api.cmd("worktrees.bringIn", json!({ "project": project, "paths": [stray] })).await;
        if out["created"].as_array().is_some_and(|c| !c.is_empty()) {
            break;
        }
        assert!(at.elapsed() < Duration::from_secs(8), "offered in seconds: {out}");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    eprintln!("offered after {} ms", at.elapsed().as_millis());
}
