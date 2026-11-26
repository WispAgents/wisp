//! wisp.computer on a real `wisp serve` with the fake agents and a stand-in Cua Driver (`fixtures/fake_cua.py`,
//! ADR-0127): switched on, every harness's run gets the `computer` server, its task's own token; the gateway
//! answers before the driver starts, starts it on the first call, passes a screenshot back as an image, keeps a
//! Plan mode task to the tools that look and every task off the deny list; switched off, no run gets it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::path::{Path, PathBuf};

use common::{Api, ended, start_with, temp};
use serde_json::{Value, json};

/// The fake driver, executable, where the server finds it by path.
fn fake_driver(dir: &Path) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_cua.py");
    let dst = dir.join("fake-cua-driver");
    std::fs::copy(src, &dst).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dst
}

async fn run(api: &Api, dir: &Path, agent: &str, mode: &str) -> String {
    let made = api
        .cmd("runtime.create", json!({ "prompt": "hi", "cwd": dir.join("proj"), "agent": agent, "mode": mode, "worktree": false }))
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    api.until(&id, "the turn ends", |t| t.last().is_some_and(ended)).await;
    id
}

fn last_claude(rec: &Path) -> Value {
    common::records(&rec.join("claude.jsonl")).last().cloned().expect("a claude run")
}

/// One JSON-RPC message to the `computer` server as the run's harness sends it.
async fn mcp(http: &reqwest::Client, server: &Value, msg: Value) -> (u16, Value) {
    let mut req = http.post(server["url"].as_str().unwrap()).header("content-type", "application/json");
    for (k, v) in server["headers"].as_object().unwrap() {
        req = req.header(k, v.as_str().unwrap());
    }
    let r = req.body(msg.to_string()).send().await.unwrap();
    let status = r.status().as_u16();
    let body = r.bytes().await.unwrap();
    (status, if body.is_empty() { Value::Null } else { serde_json::from_slice(&body).unwrap() })
}

fn names(list: &Value) -> Vec<String> {
    list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn every_harness_gets_the_computer_behind_wisps_policy() {
    if cfg!(windows) || std::process::Command::new("python3").arg("-V").output().is_err() {
        return;
    }
    let dir = temp("serve-computer");
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let driver = fake_driver(&dir);
    let log = dir.join("cua.jsonl");
    let (_serve, port) = start_with(
        &dir,
        &[
            ("FAKE_RECORD", rec.to_str().unwrap()),
            ("WISP_COMPUTER", "on"),
            ("WISP_COMPUTER_DRIVER", driver.to_str().unwrap()),
            ("FAKE_CUA_LOG", log.to_str().unwrap()),
        ],
    );
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let status = api.cmd("computer.status", json!({})).await;
    assert_eq!(status["enabled"], true, "{status}");
    assert_eq!(status["driver"], driver.to_str().unwrap(), "{status}");
    assert_eq!(status["running"], false, "nothing started it yet: {status}");

    let id = run(&api, &dir, "claude", "auto").await;
    let cfg = common::mcp_config(&last_claude(&rec));
    let server = cfg["mcpServers"]["computer"].clone();
    assert_eq!(server["type"], "http", "{cfg}");
    assert!(server["url"].as_str().unwrap().ends_with("/api/computer/mcp"), "{server}");
    assert_eq!(server["headers"]["x-wisp-session"], id.as_str());
    assert_eq!(server["headers"]["x-wisp-token"], cfg["mcpServers"]["wisp"]["headers"]["x-wisp-token"], "the task's own token");
    assert!(cfg["mcpServers"].get("browser").is_none(), "no browser outside a box: {cfg}");

    let http = reqwest::Client::new();
    let (_, init) =
        mcp(&http, &server, json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }))
            .await;
    assert_eq!(init["result"]["serverInfo"]["name"], "computer", "{init}");
    let (code, _) = mcp(&http, &server, json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).await;
    assert_eq!(code, 202);
    // before any call: the pinned driver's list, the driver not started (a run that never looks pays nothing)
    let (_, list) = mcp(&http, &server, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    assert!(names(&list).contains(&"get_window_state".to_owned()), "{list}");
    assert!(!std::fs::exists(&log).unwrap_or(false));

    // a screenshot comes back as the driver's image
    let (_, shot) =
        mcp(&http, &server, json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "get_desktop_state" } })).await;
    assert_eq!(shot["result"]["content"][1]["type"], "image", "{shot}");
    assert_eq!(shot["result"]["content"][1]["mimeType"], "image/png");
    let (_, click) = mcp(
        &http,
        &server,
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "click", "arguments": { "x": 5, "y": 6 } } }),
    )
    .await;
    assert_eq!(click["result"]["content"][0]["text"], "did click", "{click}");
    // the live list now: the driver's own, less what wisp never offers
    let (_, list) = mcp(&http, &server, json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/list" })).await;
    assert_eq!(names(&list), ["get_screen_size", "get_desktop_state", "click"], "{list}");
    let (_, cfg_call) =
        mcp(&http, &server, json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": { "name": "set_config" } })).await;
    assert_eq!(cfg_call["result"]["isError"], true, "never offered: {cfg_call}");
    let (_, init2) = mcp(&http, &server, json!({ "jsonrpc": "2.0", "id": 7, "method": "initialize" })).await;
    assert!(init2["result"]["instructions"].as_str().unwrap().contains("FAKE-DRIVER-INSTRUCTIONS"), "the driver's own words: {init2}");

    let calls = common::records(&log);
    assert_eq!(
        calls.iter().map(|c| c["name"].as_str().unwrap()).collect::<Vec<_>>(),
        ["get_desktop_state", "click"],
        "set_config never reached it"
    );
    assert_eq!(calls[1]["args"], json!({ "x": 5, "y": 6 }));
    assert_eq!(calls[0]["telemetry"], "0", "the driver's product telemetry is off");
    assert_eq!(calls[0]["update"], "false", "and its own update check");
    assert_eq!(api.cmd("computer.status", json!({})).await["running"], true);

    // Plan mode: it looks, it doesn't act
    let plan = run(&api, &dir, "claude", "plan").await;
    let pserver = common::mcp_config(&last_claude(&rec))["mcpServers"]["computer"].clone();
    assert_eq!(pserver["headers"]["x-wisp-session"], plan.as_str());
    let (_, list) = mcp(&http, &pserver, json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    assert_eq!(names(&list), ["get_screen_size", "get_desktop_state"], "{list}");
    let (_, refused) =
        mcp(&http, &pserver, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "click" } })).await;
    assert_eq!(refused["result"]["isError"], true, "{refused}");
    assert!(refused["result"]["content"][0]["text"].as_str().unwrap().contains("Plan mode"));
    assert_eq!(common::records(&log).len(), 2, "the refused click never reached the driver");
    // another task's token can't speak for this one
    let mut forged = pserver.clone();
    forged["headers"]["x-wisp-token"] = server["headers"]["x-wisp-token"].clone();
    let (code, _) = mcp(&http, &forged, json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" })).await;
    assert_eq!(code, 403);

    // Codex's runs get it too
    run(&api, &dir, "codex", "auto").await;
    let codex = common::records(&rec.join("codex.jsonl")).last().map(common::argv).unwrap_or_default().join(" ");
    assert!(codex.contains("mcp_servers.computer.url=") && codex.contains("/api/computer/mcp"), "{codex}");
}

#[tokio::test(flavor = "multi_thread")]
async fn off_by_default_no_run_gets_it() {
    let dir = temp("serve-computer-off");
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let driver = fake_driver(&dir);
    let (_serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap()), ("WISP_COMPUTER_DRIVER", driver.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let status = api.cmd("computer.status", json!({})).await;
    assert_eq!(status["enabled"], false, "{status}");
    assert!(status["hint"].as_str().unwrap().contains("Settings"), "{status}");
    run(&api, &dir, "claude", "auto").await;
    let cfg = common::mcp_config(&last_claude(&rec));
    assert!(cfg["mcpServers"].get("computer").is_none(), "{cfg}");
    // switched on in Settings: the next run has it
    api.cmd("settings.set", json!({ "key": "computer.enabled", "value": true })).await;
    run(&api, &dir, "claude", "auto").await;
    let cfg = common::mcp_config(&last_claude(&rec));
    assert_eq!(cfg["mcpServers"]["computer"]["type"], "http", "{cfg}");
}
