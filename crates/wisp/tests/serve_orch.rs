//! Orchestration paths end to end (audit A, 2026-11-18): what the browser and the API hand a task reaches the agent
//! the task runs. The real `wisp serve`, wisp-testkit's fake `claude`, and `FAKE_RECORD` to read what the harness got
//! (its argv and stdin).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};

fn served(name: &str, gated: bool) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let dir = temp(name);
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let gates = if gated { common::gates(&dir) } else { dir.join("no-gates") };
    (dir, rec, gates)
}

/// orch #1: a file attached in the browser (`POST /uploads`, then the launch names it by id, name, mime and size, as
/// the page sends it) reaches the agent: its prompt lists it with its path, which the turn itself never stores.
#[tokio::test(flavor = "multi_thread")]
async fn a_file_attached_by_upload_id_reaches_the_agents_prompt() {
    let (dir, rec, _) = served("orch-attach", false);
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let up: Value = api
        .http
        .post(format!("http://127.0.0.1:{port}/api/uploads?name=attach.txt&type=text%2Fplain"))
        .header("content-type", "application/octet-stream")
        .body("hello\n")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = up["id"].as_str().unwrap_or_else(|| panic!("an upload id: {up}")).to_owned();
    // what the page keeps of the upload (packages/shell/src/frame/attach.ts): no path
    let file = json!({ "id": id, "name": up["name"], "mime": up["mime"], "size": up["size"] });
    let made =
        api.cmd("runtime.create", json!({ "prompt": "read it", "cwd": dir.join("proj"), "agent": "claude", "attachments": [file] })).await;
    let task = made["id"].as_str().unwrap().to_owned();
    let turns = api.until(&task, "the turn ends", |t| t.last().is_some_and(ended)).await;
    assert!(turns[0]["attachments"][0].get("path").is_none(), "the turn keeps the upload's id, not a path: {}", turns[0]);
    let log = record_text(&rec.join("claude.jsonl"));
    assert!(log.contains("<attachments>") && log.contains("attach.txt"), "the agent was told about the file: {log}");
    drop(serve);
}

/// orch #2: the team picked in Delegate (or a preset) reaches the lead: its system prompt names it, and wisp's
/// `delegate` tool lists it (not "No team yet").
#[tokio::test(flavor = "multi_thread")]
async fn the_leads_team_is_in_its_prompt_and_its_delegate_tool() {
    let (dir, rec, gates) = served("orch-team", true);
    let (serve, port) = start_with(&dir, &[("FAKE_GATE", gates.to_str().unwrap()), ("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Arc::new(Api { http: reqwest::Client::new(), port, n: 0.into() });
    let team = json!([{ "agent": "claude", "name": "tester", "count": 2, "use": "run the tests" }]);
    let made = api.cmd("runtime.create", json!({ "prompt": "gate team", "cwd": dir.join("proj"), "agent": "claude", "team": team })).await;
    let id = made["id"].as_str().unwrap().to_owned();
    let runs = until_records(&rec.join("claude.jsonl"), "the lead's run", |r| !r.is_empty()).await;
    let text = runs[0].to_string();
    assert!(text.contains("## Subagents") && text.contains("`tester` ×2"), "the lead hears about its team: {text}");
    let argv = argv(&runs[0]);
    let cfg: Value = serde_json::from_str(&argv[argv.iter().position(|a| a == "--mcp-config").unwrap() + 1]).unwrap();
    let token = cfg["mcpServers"]["wisp"]["headers"]["x-wisp-token"].as_str().unwrap().to_owned();
    let list: Value = api
        .http
        .post(format!("http://127.0.0.1:{port}/api/mcp"))
        .header("content-type", "application/json")
        .header("x-wisp-token", token)
        .header("x-wisp-session", id.clone())
        .body(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }).to_string())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let delegate = list["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "delegate").cloned().unwrap_or_default();
    let words = delegate["description"].as_str().unwrap_or_default();
    assert!(words.contains("tester") && !words.contains("No team yet"), "delegate names the team: {delegate}");
    let names = delegate.pointer("/inputSchema/properties/tasks/items/properties/subagent/enum").cloned().unwrap_or_default();
    assert_eq!(names, json!(["tester", "tester#1", "tester#2"]), "{delegate}");
    common::open_gate(&gates, "team");
    api.until(&id, "the lead's turn ends", |t| t.last().is_some_and(ended)).await;
    drop(serve);
}

/// orch #5, tasks #6: a side discussion (`/side`, a row's Side discussion…: a task the user starts under another) runs
/// apart: a task of its own in the main task's folder, told where the main task stands, and the main task's
/// conversation gets nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_side_discussion_runs_apart_in_the_main_tasks_folder() {
    let (dir, rec, _) = served("orch-side", false);
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (_, main) = run_task(&api, &dir, "claude").await;
    let main_id = main["id"].as_str().unwrap().to_owned();
    let made = api.cmd("runtime.create", json!({ "prompt": "what does math.js export?", "parentId": main_id })).await;
    let side = made["id"].as_str().unwrap().to_owned();
    api.until(&side, "the side discussion answers", |t| t.last().is_some_and(ended)).await;
    let s = api.cmd("runtime.get", json!({ "id": side })).await;
    assert_eq!(s["ext"]["wisp.runtime"]["sideOf"]["id"], main_id.as_str(), "{s}");
    assert_eq!(s["cwd"], main["cwd"], "it works in the main task's folder");
    let runs = records(&rec.join("claude.jsonl"));
    assert_eq!(runs.len(), 2, "one run for the main task, one for the side discussion");
    let last = runs[1].to_string();
    assert!(last.contains("The main task now") && last.contains("side discussion"), "it hears where the main task stands: {last}");
    if let Some(rid) = api.cmd("runtime.get", json!({ "id": main_id })).await["resumeId"].as_str() {
        assert!(!argv(&runs[1]).iter().any(|a| a == rid), "never the main task's conversation: {last}");
    }
    assert_eq!(api.turns(&main_id).await.len(), 1, "the main task got no follow-up");
    drop(serve);
}

/// orch #10: a /btw answer that ends with a wisp block reads as its text, not the block's JSON.
#[tokio::test(flavor = "multi_thread")]
async fn a_btw_answer_reads_as_markdown_not_wisp_json() {
    let (dir, rec, _) = served("orch-btw", false);
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (_, task) = run_task(&api, &dir, "claude").await;
    let id = task["id"].as_str().unwrap().to_owned();
    api.cmd("runtime.btw", json!({ "id": id, "question": "did you run the tests?" })).await;
    let at = Instant::now();
    let answer = loop {
        let s = api.cmd("runtime.get", json!({ "id": id })).await;
        let b = s.pointer("/ext/wisp.runtime/btw/0").cloned().unwrap_or_default();
        // `pending` is left out once answered
        if b.is_object() && b["pending"] != true {
            break b;
        }
        assert!(at.elapsed() < Duration::from_secs(60), "the side question is answered: {s}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let text = answer["answer"].as_str().unwrap_or_default();
    assert!(!text.is_empty() && !text.contains("```wisp") && !text.contains("\"blocks\""), "{answer}");
    let log = record_text(&rec.join("claude.jsonl"));
    assert!(log.contains("# wisp btw"), "the side question's rules went with it: {log}");
    drop(serve);
}

/// orch #2, the crew half: a member's delegates (the Org chart's "Make Bob Ada's delegate") are its job's team.
#[tokio::test(flavor = "multi_thread")]
async fn a_crew_members_linked_delegates_are_its_jobs_team() {
    let (dir, rec, gates) = served("orch-crewteam", true);
    let (serve, port) = start_with(&dir, &[("FAKE_GATE", gates.to_str().unwrap()), ("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let proj = dir.join("proj");
    let bob = api.cmd("crew.create", json!({ "name": "Bob", "role": "CI watcher", "agent": "claude", "cwd": proj })).await;
    let ada = api.cmd("crew.create", json!({ "name": "Ada", "agent": "claude", "cwd": proj })).await;
    let (bob, ada) = (bob["id"].as_str().unwrap().to_owned(), ada["id"].as_str().unwrap().to_owned());
    api.cmd("crew.update", json!({ "id": ada, "team": [{ "crew": bob }] })).await;
    api.cmd("crew.assign", json!({ "id": ada, "prompt": "gate crewteam" })).await;
    let runs = until_records(&rec.join("claude.jsonl"), "Ada's job", |r| r.iter().any(|x| x.to_string().contains("gate crewteam"))).await;
    let job = runs.iter().find(|x| x.to_string().contains("gate crewteam")).unwrap().to_string();
    assert!(
        job.contains("## Subagents") && job.contains("`bob`") && job.contains("Bob, CI watcher"),
        "Ada's job hears about its delegate: {job}"
    );
    common::open_gate(&gates, "crewteam");
    drop(serve);
}
