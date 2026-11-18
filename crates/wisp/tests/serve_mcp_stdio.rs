//! Setup audit F28 (H): `wisp mcp` (the stdio server editors and other agents add, and `wisp skill install --project`
//! writes into `.mcp.json`) lost v1's task tools: it listed only the four history tools. It hands tasks to wisp again,
//! with v1's names (`server/mcp-wisp.js`): `wisp_launch`, `wisp_list`, `wisp_get`, `wisp_wait`, `wisp_reply`,
//! `wisp_stop` and `wisp_agents`, each over the catalogue's commands (ADR-0040). The real binary against `wisp serve`, the fake harness.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::io::Write as _;
use std::process::{Command, Stdio};

use common::{Api, start, temp};
use serde_json::{Value, json};

/// One `wisp mcp` session: these JSON-RPC requests in, every answer out (by id).
fn session(dir: &std::path::Path, port: u16, calls: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .arg("mcp")
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .current_dir(dir.join("proj"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        for c in calls {
            writeln!(input, "{c}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn call(id: u64, name: &str, args: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": name, "arguments": args } })
}

fn text(answers: &[Value], id: u64) -> String {
    let a = answers.iter().find(|a| a["id"] == id).unwrap_or_else(|| panic!("no answer {id}: {answers:?}"));
    assert!(a["result"]["isError"] != json!(true), "{a}");
    a["result"]["content"][0]["text"].as_str().unwrap_or_default().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_mcp_hands_tasks_to_wisp_with_v1_s_tools() {
    let dir = temp("serve-mcp-stdio");
    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };

    let answers = session(
        &dir,
        port,
        &[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            call(3, "wisp_launch", &json!({ "prompt": "fix the add bug", "agent": "claude", "wait": true, "timeout_seconds": 60 })),
            call(4, "wisp_agents", &json!({})),
            // nothing to do is refused, never a run; a wait past `Duration::MAX` is a day, not a crash
            call(5, "wisp_launch", &json!({ "prompt": "  " })),
            call(6, "wisp_wait", &json!({ "id": "nope", "timeout_seconds": 1e300 })),
        ],
    );
    let empty = answers.iter().find(|a| a["id"] == 5).unwrap();
    assert!(empty["result"]["isError"] == json!(true) && empty.to_string().contains("needs a prompt"), "{empty}");
    assert!(answers.iter().any(|a| a["id"] == 6), "the server answered after a huge timeout: {answers:?}");
    let tools: Vec<String> = answers.iter().find(|a| a["id"] == 2).unwrap()["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    for name in
        ["wisp_launch", "wisp_list", "wisp_get", "wisp_wait", "wisp_reply", "wisp_stop", "wisp_agents", "wisp_history", "wisp_resume"]
    {
        assert!(tools.contains(&name.to_owned()), "{name} in {tools:?}");
    }
    // launched in the server's folder, waited for, and read back as its answer
    let launched = text(&answers, 3);
    let tasks = api.cmd("runtime.list", json!({})).await;
    let id = tasks[0]["id"].as_str().unwrap().to_owned();
    assert!(launched.contains(&id) && launched.contains("fix the add bug"), "{launched}");
    // the server's folder (`/private/var/…` or `/var/…` on macOS: either spelling)
    let cwd = tasks[0]["cwd"].as_str().unwrap().trim_end_matches('/').to_owned();
    let want = dir.join("proj").display().to_string();
    assert!(
        cwd == want || cwd.strip_prefix("/private") == Some(want.as_str()) || want.strip_prefix("/private") == Some(cwd.as_str()),
        "{cwd} vs {want}"
    );
    assert!(text(&answers, 4).contains("claude"), "{}", text(&answers, 4));

    let answers = session(
        &dir,
        port,
        &[
            call(1, "wisp_list", &json!({})),
            call(2, "wisp_get", &json!({ "id": id })),
            call(3, "wisp_reply", &json!({ "id": id, "text": "also cover zero" })),
            call(4, "wisp_wait", &json!({ "id": id, "timeout_seconds": 60 })),
            call(5, "wisp_stop", &json!({ "id": id })),
        ],
    );
    assert!(text(&answers, 1).contains(&id), "{}", text(&answers, 1));
    assert!(text(&answers, 2).contains("fix the add bug"), "{}", text(&answers, 2));
    assert!(text(&answers, 3).contains("sent") || text(&answers, 3).contains("queued"), "{}", text(&answers, 3));
    assert!(text(&answers, 4).contains(&id), "{}", text(&answers, 4));
    assert!(text(&answers, 5).contains("stopped"), "{}", text(&answers, 5));
    let turns = api.cmd("runtime.turns", json!({ "id": id })).await;
    assert!(turns.to_string().contains("also cover zero"), "the reply reached the task: {turns}");
    drop(serve);
}
