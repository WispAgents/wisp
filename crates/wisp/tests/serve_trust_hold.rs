//! A turn held for project trust (audit D1–D4; v1 hold.js projectCard): a repo whose `.claude/settings.json` runs a
//! hook and whose `.mcp.json` starts a server holds a run in Edits until you say. **Trust and start** trusts what it
//! listed and runs it (D1: it was held again, forever, and the next task too); **Run without them** runs it without
//! the repo's programs and trusts nothing; **Skip this run** says the repo's programs weren't trusted (D3: it named a
//! usage window); `wisp show` says what is held and how to start or skip it (D4: it said nothing).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

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

/// A repo that asks to run its own programs: a `PreToolUse` hook and an MCP server.
fn asking_repo(dir: &Path) {
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    std::fs::write(
        dir.join(".claude/settings.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"*","hooks":[{"type":"command","command":"echo hi"}]}]}}"#,
    )
    .unwrap();
    std::fs::write(dir.join(".mcp.json"), r#"{"mcpServers":{"local":{"command":"node","args":["server.js"]}}}"#).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-qm", "init"]);
}

fn wisp(dir: &Path, port: u16, args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
}

/// A task in Edits on `proj`, held for trust: (task id, its held turn).
async fn held(api: &Api, proj: &Path, prompt: &str) -> (String, Value) {
    let made = api.cmd("runtime.create", json!({ "prompt": prompt, "cwd": proj, "agent": "claude", "mode": "edits" })).await;
    let id = made["id"].as_str().unwrap_or_else(|| panic!("{made}")).to_owned();
    let t = api.until(&id, "held for trust", |t| t.len() == 1 && t[0]["status"] == "held").await;
    assert_eq!((t[0]["held"]["by"].as_str(), t[0]["held"]["kind"].as_str()), (Some("wisp.worktrees"), Some("trust")), "{:?}", t[0]);
    let names: Vec<&str> = t[0]["held"]["data"]["items"].as_array().unwrap().iter().filter_map(|i| i["name"].as_str()).collect();
    assert!(names.contains(&"local"), "the card lists what it would run: {names:?}");
    (id, t[0].clone())
}

async fn release(api: &Api, id: &str, turn: &Value, action: &str) {
    let r = api.try_cmd("runtime.release", json!({ "id": id, "turnId": turn["id"], "action": action })).await;
    assert!(r.get("error").is_none(), "{action}: {r}");
}

#[tokio::test(flavor = "multi_thread")]
async fn trust_and_start_runs_it_and_trusts_the_repo_run_without_trusts_nothing_and_skip_says_why() {
    let dir = temp("serve-trust-hold");
    let (one, two) = (dir.join("proj"), dir.join("proj2"));
    std::fs::create_dir_all(&two).unwrap();
    asking_repo(&one);
    asking_repo(&two);
    let (serve, port) = start_with(&dir, &[]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    // D4: `wisp show` says what holds it, what it would run, and how to go on
    let (id, turn) = held(&api, &one, "D1 trust and start").await;
    let shown = wisp(&dir, port, &["show", &id]);
    assert!(shown.contains("Held before it started") && shown.contains("local"), "{shown}");
    assert!(shown.contains(&format!("wisp held {id} start")) && shown.contains(&format!("wisp held {id} skip")), "{shown}");

    // D1: Trust and start runs it
    release(&api, &id, &turn, "start").await;
    let t = api.until(&id, "it runs", |t| t.len() == 1 && ended(&t[0])).await;
    assert_eq!(t[0]["status"], "done", "{:?}", t[0]);
    // and the yes was kept: the next task on the same repo isn't held
    let made = api.cmd("runtime.create", json!({ "prompt": "D1 again", "cwd": one, "agent": "claude", "mode": "edits" })).await;
    let again = made["id"].as_str().unwrap().to_owned();
    let t = api.until(&again, "the next one runs", |t| t.len() == 1 && ended(&t[0])).await;
    assert_eq!(t[0]["status"], "done", "trusted once, not asked again: {:?}", t[0]);

    // Run without them: it runs, without the repo's programs, and trusts nothing
    let (id, turn) = held(&api, &two, "D2 without them").await;
    release(&api, &id, &turn, "without").await;
    let t = api.until(&id, "it runs without them", |t| t.len() == 1 && ended(&t[0])).await;
    assert_eq!(t[0]["status"], "done", "{:?}", t[0]);

    // D3: Skip says what it was held for (and the repo is still untrusted: it's held again)
    let (id, turn) = held(&api, &two, "D3 skip").await;
    release(&api, &id, &turn, "skip").await;
    let t = api.until(&id, "skipped", |t| t.len() == 1 && ended(&t[0])).await;
    let said = t[0]["presentation"].to_string();
    assert!(said.contains("hooks, servers or setup") && !said.contains("near its limit"), "{said}");
    drop(serve);
}
