//! A project's own worktree setup (`.wisp/worktree.json`, audit D5; v1 project-trust.js:160–162): in Full auto it runs
//! in the new worktree at once, and when it fails the turn is held with why (Start now runs it anyway); in another mode
//! an untrusted setup holds the turn for trust, listing the setup, and Trust and start runs it, then the turn. Before,
//! a failing setup neither ran visibly nor held anything: each run went straight to the harness.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::Command;

use common::{Api, ended, start_with, temp};
use serde_json::{Value, json};

fn git(dir: &Path, args: &[&str]) {
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
}

fn repo_with_setup(dir: &Path, setup: &str) {
    std::fs::create_dir_all(dir.join(".wisp")).unwrap();
    std::fs::write(dir.join(".wisp/worktree.json"), json!({ "setup": setup }).to_string()).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-qm", "init"]);
}

async fn create(api: &Api, proj: &Path, prompt: &str, mode: &str) -> (String, String) {
    let mut a = json!({ "prompt": prompt, "cwd": proj, "agent": "claude", "worktree": true });
    // "": no mode at all, as `wisp "…"` and `wisp "…" --mode auto` send it (the run's mode is then Full auto)
    if !mode.is_empty() {
        a["mode"] = json!(mode);
    }
    let made = api.cmd("runtime.create", a).await;
    let id = made["id"].as_str().unwrap_or_else(|| panic!("{made}")).to_owned();
    let s = api.cmd("runtime.get", json!({ "id": id })).await;
    let dir = s["ext"]["wisp.worktrees"]["path"].as_str().unwrap_or_else(|| panic!("no worktree: {s}")).to_owned();
    (id, dir)
}

async fn first(api: &Api, id: &str, what: &str, f: impl Fn(&Value) -> bool) -> Value {
    api.until(id, what, |t| t.len() == 1 && f(&t[0])).await[0].clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_setup_holds_the_turn_in_full_auto_and_an_untrusted_one_holds_it_for_trust() {
    let dir = temp("serve-wt-setup");
    let proj = dir.join("proj");
    repo_with_setup(&proj, "touch setup-ran && exit 3");
    let (serve, port) = start_with(&dir, &[]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    // Full auto: the setup runs in the worktree at once; it failed, so the turn waits with why
    let (id, wt) = create(&api, &proj, "D5 auto", "auto").await;
    assert!(Path::new(&wt).join("setup-ran").exists(), "the setup ran in {wt}");
    let t = first(&api, &id, "held for its setup", |t| t["status"] == "held").await;
    assert_eq!(t["held"]["kind"], "setup", "{t}");
    assert!(t["held"]["data"]["why"].as_str().unwrap_or_default().contains("exit 3"), "{t}");
    // Start now runs it anyway
    let r = api.try_cmd("runtime.release", json!({ "id": id, "turnId": t["id"], "action": "start" })).await;
    assert!(r.get("error").is_none(), "{r}");
    let t = first(&api, &id, "it runs", ended).await;
    assert_eq!(t["status"], "done", "{t}");

    // audit E D5: no mode given (the CLI's Full auto) is Full auto for the setup too: it ran, failed and holds with why.
    // Before, the worktree was made for mode "", the setup waited for trust, and the auto run passed trust without it
    let (id, wt) = create(&api, &proj, "D5 cli auto", "").await;
    assert!(Path::new(&wt).join("setup-ran").exists(), "the setup ran in {wt}");
    let t = first(&api, &id, "held for its setup", |t| t["status"] == "held").await;
    assert_eq!(t["held"]["kind"], "setup", "{t}");
    let r = api.try_cmd("runtime.release", json!({ "id": id, "turnId": t["id"], "action": "skip" })).await;
    assert!(r.get("error").is_none(), "{r}");

    // Edits: the repo's setup isn't trusted yet: the turn is held for trust, listing it, and the setup hasn't run
    let (id, wt) = create(&api, &proj, "D5 edits", "edits").await;
    let t = first(&api, &id, "held for trust", |t| t["status"] == "held").await;
    assert_eq!((t["held"]["by"].as_str(), t["held"]["kind"].as_str()), (Some("wisp.worktrees"), Some("trust")), "{t}");
    let kinds: Vec<&str> = t["held"]["data"]["items"].as_array().unwrap().iter().filter_map(|i| i["kind"].as_str()).collect();
    assert_eq!(kinds, ["setup"], "{t}");
    assert!(!Path::new(&wt).join("setup-ran").exists(), "an untrusted setup waits");
    // Trust and start: the setup runs in its worktree, then the turn (its failure is said, not held again: a yes)
    let r = api.try_cmd("runtime.release", json!({ "id": id, "turnId": t["id"], "action": "start" })).await;
    assert!(r.get("error").is_none(), "{r}");
    let t = first(&api, &id, "it runs after its setup", ended).await;
    assert_eq!(t["status"], "done", "{t}");
    assert!(Path::new(&wt).join("setup-ran").exists(), "Trust and start ran the setup in {wt}");
    drop(serve);
}
