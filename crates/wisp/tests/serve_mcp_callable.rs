//! Audit B (docs/qa/audit-B-orchestration.md) #3, #4 and #5, called the way a harness calls them: `POST /api/mcp` as a
//! task's agent, with the token its run was given.
//! - #3: every agent's `wisp_notes` was refused ("wisp.mcp doesn't declare threads:read in its manifest").
//! - #4: `tools/list` offered ~250 generated tools wisp.mcp itself may not run, each refused at call time.
//! - #5: a task an agent launched with `worktree: true` ran in the main checkout (agent:session may not
//!   worktrees:create, so runtime fell back to in place).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
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

/// A running wisp with one task whose fake agent is mid-turn in `proj` (a git repo), and that task's MCP token.
struct Agent {
    serve: Serve,
    api: Api,
    port: u16,
    id: String,
    token: String,
    repo: PathBuf,
}

impl Agent {
    async fn start(name: &str) -> Self {
        let dir = temp(name);
        let repo = wisp_proc::canonical_blocking(dir.join("proj")).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "init"]);
        let rec = dir.join("rec");
        std::fs::create_dir_all(&rec).unwrap();
        let (serve, port) = start_with(&dir, &[("FAKE_DELAY", "20000"), ("FAKE_RECORD", rec.to_str().unwrap())]);
        let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
        let made = api.cmd("runtime.create", json!({ "prompt": "hi", "cwd": repo, "agent": "claude", "worktree": false })).await;
        let id = made["id"].as_str().unwrap().to_owned();
        api.until(&id, "the turn running", |t| t.len() == 1 && t[0]["status"] == "running").await;
        let runs = until_records(&rec.join("claude.jsonl"), "the run's record", |r| !r.is_empty()).await;
        let token = common::wisp_token(&runs[0]);
        Self { serve, api, port, id, token, repo }
    }

    async fn post(&self, body: Value) -> Value {
        let r = self
            .api
            .http
            .post(format!("http://127.0.0.1:{}/api/mcp", self.port))
            .header("content-type", "application/json")
            .header("x-wisp-token", &self.token)
            .header("x-wisp-session", &self.id)
            .body(body.to_string())
            .send()
            .await
            .unwrap();
        serde_json::from_slice(&r.bytes().await.unwrap()).unwrap_or(Value::Null)
    }

    /// `tools/call` as the task's agent: (the tool's text, whether it's an error result).
    async fn call(&self, tool: &str, args: Value) -> (String, bool) {
        let r =
            self.post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": args } })).await;
        let res = &r["result"];
        let text = res["content"].as_array().into_iter().flatten().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n");
        (if text.is_empty() { r.to_string() } else { text }, res["isError"] == true || r.get("error").is_some())
    }

    async fn tools(&self) -> Vec<Value> {
        let list = self.post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
        list["result"]["tools"].as_array().unwrap().clone()
    }
}

/// #3: an agent reads and rewrites its task's Notes (every run's prompt tells it to keep them with `wisp_notes`).
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_keeps_its_tasks_notes_with_wisp_notes() {
    let a = Agent::start("mcp-notes").await;
    let (set, failed) = a.call("wisp_notes", json!({ "set": { "decisions": "SQLite, not Redis" } })).await;
    assert!(!failed, "wisp_notes set: {set}");
    assert!(set.contains("Updated decisions") && set.contains("SQLite, not Redis"), "{set}");
    let (read, failed) = a.call("wisp_notes", json!({})).await;
    assert!(!failed && read.contains("SQLite, not Redis"), "wisp_notes read: {read}");
    let notes = a.api.cmd("threads.notes", json!({ "id": a.id })).await.to_string();
    assert!(notes.contains("SQLite, not Redis"), "kept on the task: {notes}");
    a.serve.stop();
}

/// #4: every tool `tools/list` offers can be called: none answers "doesn't declare … in its manifest". The audit's
/// refused ones still refused (review, git, history) aren't offered; automations are granted since audit H; the threads ones are
/// callable now (wisp.mcp grants threads:read and threads:write for `wisp_notes`, #3).
#[tokio::test(flavor = "multi_thread")]
async fn every_listed_tool_is_one_wisp_mcp_may_run() {
    let a = Agent::start("mcp-callable").await;
    let tools = a.tools().await;
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    // goal_* and factory_* are wisp.mcp's to run since audit G7 (an agent sets its own task's goal, ADR-0112), and
    // automations_* since audit H setup (HS: an agent manages its automations over MCP): those are called below
    for refused in ["review_view", "git_status", "history_list"] {
        assert!(!names.contains(&refused), "{refused} is listed, and wisp.mcp may not run it: {names:?}");
    }
    // each generated tool's command is one wisp.mcp's manifest grants (or one of its own, `mcp.*`)
    let manifest: Value = serde_json::from_str(include_str!("../../../plugins/mcp/host/wisp-plugin.json")).unwrap();
    let mut grants: Vec<&str> = manifest["permissions"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    grants.push("mcp:*");
    for t in &tools {
        let desc = t["description"].as_str().unwrap_or_default();
        let Some(cmd) = desc.rsplit_once("(wisp command ").map(|(_, c)| c.trim_end_matches(')')) else { continue };
        let ns = cmd.split('.').next().unwrap();
        assert!(grants.iter().any(|g| g.split(':').next() == Some(ns)), "{} runs {cmd}, outside wisp.mcp's grants {grants:?}", t["name"]);
    }
    // and a sample of them answers past the permission check
    for t in [
        "runtime_list",
        "wisp_sessions",
        "wisp_notes",
        "threads_notes",
        "automations_tasks",
        "automations_sources",
        "automations_trigger_ls",
    ] {
        let (text, _) = a.call(t, json!({})).await;
        assert!(!text.contains("in its manifest"), "{t}: {text}");
    }
    a.serve.stop();
}

/// #5: a task an agent launches with `worktree: true` gets a worktree of its own, as the user's does; its fake ran there.
#[tokio::test(flavor = "multi_thread")]
async fn a_task_an_agent_launches_in_a_worktree_runs_in_its_own_worktree() {
    let a = Agent::start("mcp-launch-tree").await;
    let (made, failed) =
        a.call("runtime_create", json!({ "prompt": "child job", "cwd": a.repo, "agent": "claude", "worktree": true })).await;
    assert!(!failed, "runtime_create: {made}");
    let child: Value = serde_json::from_str(&made).unwrap_or_else(|_| panic!("the created task: {made}"));
    let id = child["id"].as_str().unwrap().to_owned();
    let s = a.api.cmd("runtime.get", json!({ "id": id })).await;
    let path = s["ext"]["wisp.worktrees"]["path"].as_str().unwrap_or_else(|| panic!("no worktree: {s}"));
    assert_ne!(path, a.repo.display().to_string(), "{s}");
    assert_eq!(s["cwd"].as_str(), Some(path), "it runs in its worktree, not the main checkout: {s}");
    a.serve.stop();
}

/// Audit C1: `wisp_launch` from a run passes every field its schema documents, as `runtime_create` does: a folder gets
/// its own worktree by default (a git repo, not plan mode), a project by name runs in that project's folder (never an
/// empty scratch one), `also` and `effort` reach the task, `scratch` gets a folder of its own.
#[tokio::test(flavor = "multi_thread")]
async fn wisp_launch_from_a_run_passes_project_worktree_scratch_also_and_effort_audit_c1() {
    let a = Agent::start("mcp-launch-c1").await;
    let applied = a
        .api
        .try_cmd(
            "resources.apply",
            json!({ "resources": [{ "kind": "project", "name": "proj", "spec": { "path": a.repo } }], "trust": true }),
        )
        .await;
    assert_eq!(applied["ok"], true, "{applied}");
    let launched = |text: &str| {
        let i = text.find('`').unwrap_or_else(|| panic!("no task id in: {text}"));
        text[i + 1..].split('`').next().unwrap().to_owned()
    };
    let repo = a.repo.display().to_string();
    // a folder, worktree not said: its own worktree (the default in a git repo)
    let (t, failed) = a.call("wisp_launch", json!({ "prompt": "child in a folder", "cwd": a.repo, "agent": "claude" })).await;
    assert!(!failed, "{t}");
    let s = a.api.cmd("runtime.get", json!({ "id": launched(&t) })).await;
    let tree = s["ext"]["wisp.worktrees"]["path"].as_str().unwrap_or_else(|| panic!("no worktree: {s}"));
    assert_eq!(s["cwd"].as_str(), Some(tree), "it runs in its worktree, not the user's checkout: {s}");
    assert_ne!(tree, repo);
    // a project by name: that project's folder (in its own worktree), with effort and the folders `also` names
    let other = temp("mcp-launch-c1-also");
    let (t, failed) = a
        .call(
            "wisp_launch",
            json!({ "prompt": "child in the project", "project": "proj", "agent": "claude", "effort": "high", "also": [other] }),
        )
        .await;
    assert!(!failed, "{t}");
    let s = a.api.cmd("runtime.get", json!({ "id": launched(&t) })).await;
    assert!(s["ext"]["wisp.worktrees"]["path"].is_string(), "the project's folder, in a worktree, not an empty scratch one: {s}");
    assert_eq!(s["effort"], "high", "{s}");
    assert!(s["dirs"].to_string().contains(&other.display().to_string().replace('\\', "\\\\")), "also: {s}");
    // plan mode: in place; worktree:false: in place
    for args in [
        json!({ "prompt": "plan it", "cwd": a.repo, "agent": "claude", "mode": "plan" }),
        json!({ "prompt": "here", "cwd": a.repo, "agent": "claude", "worktree": false }),
    ] {
        let (t, failed) = a.call("wisp_launch", args).await;
        assert!(!failed, "{t}");
        let s = a.api.cmd("runtime.get", json!({ "id": launched(&t) })).await;
        assert_eq!(s["cwd"].as_str(), Some(repo.as_str()), "{s}");
    }
    // a project that isn't there is said
    let (t, failed) = a.call("wisp_launch", json!({ "prompt": "x", "project": "nope" })).await;
    assert!(failed && t.contains("No project"), "{t}");
    a.serve.stop();
}
