//! A workflow run cut by a wisp restart: its view says it stopped, and the task it held in Working lands. Before, the
//! run's task said Working for ever (a held run task from `wisp run`, or a lead whose agent's turn ended while its run
//! went): nothing was left to land it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};

/// Waits (at most a minute) until the task satisfies `f`, and returns it.
async fn until_task(api: &Api, id: &str, what: &str, f: impl Fn(&Value) -> bool) -> Value {
    let at = Instant::now();
    loop {
        let t = api.try_cmd("runtime.get", json!({ "id": id })).await;
        let t = t.get("result").cloned().unwrap_or(t);
        if f(&t) {
            return t;
        }
        assert!(at.elapsed() < Duration::from_secs(60), "{what}: {t}");
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

fn run_of(t: &Value) -> Value {
    t.pointer("/ext/wisp.workflows/runs/0").cloned().unwrap_or(Value::Null)
}

/// The run's first step has a task whose turn is running.
async fn step_running(api: &Api, owner: &str) {
    let t = until_task(api, owner, "the step starts", |t| run_of(t)["steps"][0]["task"].is_string()).await;
    let step = run_of(&t)["steps"][0]["task"].as_str().unwrap().to_owned();
    api.until(&step, "the step's turn runs", |t| t.last().is_some_and(|x| x["status"] == "running")).await;
}

/// `wisp run` from a shell: its run is a held task made for it (`<name> (workflow)`), in Working while it goes.
#[tokio::test(flavor = "multi_thread")]
async fn a_run_task_cut_by_a_restart_lands_and_says_its_run_stopped() {
    let dir = temp("wf-restart-held");
    let gates = common::gates(&dir);
    let (serve, port) = start_with(&dir, &[("FAKE_DELAY", "0"), ("FAKE_GATE", gates.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let spec = json!({ "tasks": { "a": { "prompt": "gate steps look at math.js" } } });
    let started = api.cmd("workflows.run", json!({ "name": "cut", "spec": spec, "project": dir.join("proj") })).await;
    let id = started["id"].as_str().unwrap().to_owned();
    step_running(&api, &id).await;
    assert_eq!(api.cmd("runtime.get", json!({ "id": id })).await["state"], "running");
    serve.stop();
    let (serve, port) = restart(&dir);
    // a fake left over from the server before goes on and exits
    open_gate(&gates, "steps");
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let t = until_task(&api, &id, "the run task lands", |t| t["state"] != "running" && run_of(t)["state"] == "stopped").await;
    assert_eq!(t["title"], "cut (workflow)");
    assert_eq!(run_of(&t)["notes"][0], "wisp stopped while this ran: run it again to start it over", "{t}");
    serve.stop();
    let _ = std::fs::remove_dir_all(&dir);
}

/// An agent's run (`workflows_run`): its turn ended while the run went, so its task stayed in Working on it (Hold).
#[tokio::test(flavor = "multi_thread")]
async fn a_lead_held_on_its_run_lands_after_a_restart_cuts_it() {
    let dir = temp("wf-restart-lead");
    let gates = common::gates(&dir);
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let (serve, port) =
        start_with(&dir, &[("FAKE_DELAY", "0"), ("FAKE_GATE", gates.to_str().unwrap()), ("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let made = api.cmd("runtime.create", json!({ "prompt": "gate lead hi", "cwd": dir.join("proj"), "agent": "claude" })).await;
    let id = made["id"].as_str().unwrap().to_owned();
    api.until(&id, "the lead's turn runs", |t| t.len() == 1 && t[0]["status"] == "running").await;
    let runs = until_records(&rec.join("claude.jsonl"), "the lead's run", |r| !r.is_empty()).await;
    let token = common::wisp_token(&runs[0]);
    // as its agent: `workflows_run`
    let spec = json!({ "tasks": { "a": { "prompt": "gate steps look at math.js" } } });
    let call = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "workflows_run", "arguments": { "name": "cut", "spec": spec } } });
    let r = api
        .http
        .post(format!("http://127.0.0.1:{port}/api/mcp"))
        .header("content-type", "application/json")
        .header("x-wisp-token", &token)
        .header("x-wisp-session", &id)
        .body(call.to_string())
        .send()
        .await
        .unwrap();
    let r: Value = serde_json::from_slice(&r.bytes().await.unwrap()).unwrap();
    assert!(r["result"]["isError"] != true && r.get("error").is_none(), "{r}");
    step_running(&api, &id).await;
    // the agent ends its turn; the run goes on, and holds its task in Working
    open_gate(&gates, "lead");
    api.until(&id, "the lead's turn ends", |t| t.len() == 1 && ended(&t[0])).await;
    assert_eq!(api.cmd("runtime.get", json!({ "id": id })).await["state"], "running", "held on its run");
    serve.stop();
    let (serve, port) = restart(&dir);
    open_gate(&gates, "steps");
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let t = until_task(&api, &id, "the lead lands", |t| t["state"] != "running" && run_of(t)["state"] == "stopped").await;
    assert_eq!(t["unread"], true, "{t}");
    serve.stop();
    let _ = std::fs::remove_dir_all(&dir);
}
