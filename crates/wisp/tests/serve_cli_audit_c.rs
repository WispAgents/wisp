//! Audit C (2026-11-19), the CLI's words, end to end on a real `wisp serve` with the fake agent (nothing paid runs):
//!   - C9: `wisp runtime queue order <id> <order>`, as its usage line shows, reorders (it said "✓ done" and changed
//!     nothing): the order as commas or as words.
//!   - C10: `wisp runtime queue edit --items` takes items without the `at` its help never mentions.
//!   - C12: `wisp rerun <id> -d` reruns (it printed the usage text), as `-d` does for a launch.
//!   - C13: `wisp show` on a task that hit its usage limit shows the friendly line, never the raw stderr and its epoch.
//!   - C14: `wisp automations trigger ls` has a header and says times as words; `queue send-now` says no `turnId`.
//!   - C15: `wisp ls` names a worktree task's project, not its worktree folder (`proj-01a1f…`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::{Api, ended, start_with, temp};
use serde_json::{Value, json};

fn wisp(dir: &Path, cwd: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

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

/// `dir/proj` as a git repo with one commit.
fn repo(dir: &Path) -> PathBuf {
    let repo = wisp_proc::canonical_blocking(dir.join("proj")).unwrap();
    std::fs::write(repo.join("math.js"), "export const add = (a, b) => a + b;\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "core.autocrlf", "false"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    repo
}

fn queue_ids(task: &Value) -> Vec<String> {
    task["queue"].as_array().into_iter().flatten().filter_map(|q| q["id"].as_str().map(str::to_owned)).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_queue_verbs_do_what_their_usage_says_audit_c_9_10_14() {
    let dir = temp("serve-auditC-queue");
    let gates = common::gates(&dir);
    let (_serve, port) = start_with(&dir, &[("FAKE_GATE", gates.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let proj = dir.join("proj");
    let made = api.cmd("runtime.create", json!({ "prompt": "first", "cwd": proj, "agent": "claude" })).await;
    let id = made["id"].as_str().unwrap().to_owned();
    api.until(&id, "the first turn", |t| t.len() == 1 && ended(&t[0])).await;
    // a turn that waits on its gate (never opened), so what follows provably queues
    api.cmd("runtime.followUp", json!({ "id": id, "text": "gate hold" })).await;
    api.until(&id, "the second turn running", |t| t.len() == 2 && t[1]["status"] == "running").await;
    let a = api.cmd("runtime.followUp", json!({ "id": id, "text": "q-one", "queue": true })).await["queued"].as_str().unwrap().to_owned();
    let b = api.cmd("runtime.followUp", json!({ "id": id, "text": "q-two", "queue": true })).await["queued"].as_str().unwrap().to_owned();
    let queue = || async { api.cmd("runtime.get", json!({ "id": id })).await };
    assert_eq!(queue_ids(&queue().await), [a.clone(), b.clone()]);

    // C9: the order as its usage line shows it, commas or words
    let (code, out, err) = wisp(&dir, &proj, port, &["runtime", "queue", "order", &id, &format!("{b},{a}")]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(queue_ids(&queue().await), [b.clone(), a.clone()], "reordered by commas: {out}");
    let (code, out, err) = wisp(&dir, &proj, port, &["runtime", "queue", "order", &id, &a, &b]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(queue_ids(&queue().await), [a.clone(), b.clone()], "reordered by words: {out}");

    // C10: items as the user writes them, with no `at`
    let items = json!([{ "id": b, "text": "q-two edited" }, { "id": a, "text": "q-one" }]).to_string();
    let (code, out, err) = wisp(&dir, &proj, port, &["runtime", "queue", "edit", &id, "--items", &items]);
    assert_eq!(code, 0, "{out}{err}");
    let t = queue().await;
    assert_eq!(queue_ids(&t), [b.clone(), a.clone()], "{t}");
    assert_eq!(t["queue"][0]["text"], "q-two edited", "{t}");
    let (_, help, _) = wisp(&dir, &proj, port, &["runtime", "queue", "edit", "--help"]);
    assert!(!help.contains("at:"), "the help asks for no `at`: {help}");

    // C14: send-now says what happened in words, no `turnId`
    api.cmd("runtime.stop", json!({ "id": id })).await;
    api.until(&id, "the held turn stopped", |t| t.len() == 2 && ended(&t[1])).await;
    let (code, out, err) = wisp(&dir, &proj, port, &["runtime", "queue", "send-now", &id, &b]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("turnId") && !out.contains("turn id"), "{out}");
    let t = api.until(&id, "the sent follow-up ran", |t| t.len() >= 3 && ended(&t[2])).await;
    assert_eq!(t[2]["prompt"], "q-two edited", "the one sent went first");
}

#[tokio::test(flavor = "multi_thread")]
async fn rerun_show_ls_and_trigger_ls_say_it_in_words_audit_c_12_13_14_15() {
    let dir = temp("serve-auditC-words");
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    let repo = repo(&dir);
    let limit = dir.join("limit-hit");
    let (_serve, port) = start_with(&dir, &[("FAKE_LIMIT_ONCE", limit.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    api.cmd("runtime.agents", json!({})).await;
    let _ = std::fs::remove_file(&limit);

    // C13: the first run hits its usage limit; show says so in words, never the raw stderr
    let made = api.cmd("runtime.create", json!({ "prompt": "hit the limit", "cwd": repo, "agent": "claude" })).await;
    let limited = made["id"].as_str().unwrap().to_owned();
    api.until(&limited, "the limited turn ends", |t| t.first().is_some_and(ended)).await;
    let (code, out, err) = wisp(&dir, &repo, port, &["show", &limited]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.to_lowercase().contains("limit"), "the friendly line: {out}");
    assert!(!out.contains("1767225600") && !out.contains("limit reached|"), "no raw stderr: {out}");
    assert!(!out.lines().any(|l| l.trim() == "$"), "no empty command line: {out}");

    // C15: a worktree task's row names its project
    let made = api.cmd("runtime.create", json!({ "prompt": "in a worktree", "cwd": repo, "agent": "claude", "worktree": true })).await;
    let wt = made["id"].as_str().unwrap().to_owned();
    api.until(&wt, "the worktree turn ends", |t| t.first().is_some_and(ended)).await;
    let cwd = api.cmd("runtime.get", json!({ "id": wt })).await["cwd"].as_str().unwrap().to_owned();
    assert_ne!(Path::new(&cwd), repo, "it ran in a worktree: {cwd}");
    let (code, out, err) = wisp(&dir, &repo, port, &["ls"]);
    assert_eq!(code, 0, "{out}{err}");
    let row = out.lines().find(|l| l.starts_with(&wt)).unwrap_or_else(|| panic!("{out}"));
    assert!(row.split_whitespace().any(|w| w == "proj"), "the project column: {row}");
    assert!(!row.contains("proj-"), "not the worktree's folder: {row}");

    // C12: `-d` as a launch takes it
    let (code, out, err) = wisp(&dir, &repo, port, &["rerun", &wt, "-d"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains(&format!("{wt} → ")), "it reran: {out}{err}");

    // C14: a trigger's row under a header, its last run as words
    let r = api
        .try_cmd(
            "resources.apply",
            json!({ "resources": [{ "kind": "trigger", "name": "uitrig", "spec": { "on": "schedule", "cron": "0 3 * * *", "prompt": "Tidy UITRIG", "project": repo } }] }),
        )
        .await;
    assert_eq!(r["ok"], true, "{r}");
    let fired = api.cmd("automations.trigger.fire", json!({ "name": "uitrig" })).await;
    assert!(fired["session"].is_string(), "{fired}");
    let (code, out, err) = wisp(&dir, &repo, port, &["automations", "trigger", "ls"]);
    assert_eq!(code, 0, "{out}{err}");
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(lines.first().is_some_and(|h| h.starts_with("name") && h.contains("state")), "a header: {out}");
    assert!(lines.iter().any(|l| l.starts_with("uitrig")), "{out}");
    assert!(!out.split(|c: char| !c.is_ascii_digit()).any(|n| n.len() >= 12), "no epoch: {out}");
}
