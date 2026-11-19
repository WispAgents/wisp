//! Audit B #4, its stdio part (re-checked in audit C): the user's harness gets wisp through `wisp mcp` (the stdio
//! server `wisp skill install` writes into `.mcp.json`), and it offered 11 tools while `/api/mcp` offered the user 240,
//! the catalogue ADR-0040 promises "inside any harness". With a wisp running, `wisp mcp` offers what `/api/mcp` offers
//! the user (ADR-0107: only tools it can run), plus its own history and task tools, and runs each through it. The real
//! binary against `wisp serve` with the fake harness.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::io::Write as _;
use std::process::{Command, Stdio};

use common::{Api, start, temp};
use serde_json::{Value, json};

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

fn names(list: &Value) -> Vec<String> {
    list["tools"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str().map(str::to_owned)).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_mcp_offers_the_user_what_api_mcp_does_audit_b_4() {
    let dir = temp("serve-mcp-stdio-cat");
    let (_serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    // what /api/mcp lists for the user's editor
    let over_http: Value = api
        .http
        .post(format!("http://127.0.0.1:{port}/api/mcp"))
        .header("content-type", "application/json")
        .header("x-wisp-cwd", dir.join("proj").to_string_lossy().into_owned())
        .body(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }).to_string())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let http = names(&over_http["result"]);
    assert!(http.len() > 50, "the catalogue: {}", http.len());

    let answers = session(
        &dir,
        port,
        &[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "wisp_crew", "arguments": {} } }),
        ],
    );
    let listed = answers.iter().find(|a| a["id"] == 2).unwrap();
    let stdio = names(&listed["result"]);
    let missing: Vec<&String> = http.iter().filter(|n| !stdio.contains(n)).collect();
    assert!(missing.is_empty(), "stdio lacks {} of /api/mcp's tools: {missing:?}", missing.len());
    for own in ["wisp_launch", "wisp_list", "wisp_get", "wisp_history", "wisp_blame"] {
        assert!(stdio.iter().any(|n| n == own), "its own {own}: {stdio:?}");
    }
    let mut seen = stdio.clone();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), stdio.len(), "each name once");
    // a catalogue tool runs through it: v1's `wisp_crew`, which the stdio server had lost
    assert!(http.iter().any(|n| n == "wisp_crew"), "{http:?}");
    let a = answers.iter().find(|a| a["id"] == 3).unwrap_or_else(|| panic!("{answers:?}"));
    assert!(a["result"]["isError"] != json!(true) && a.get("error").is_none(), "{a}");
    assert!(a["result"]["content"][0]["text"].is_string(), "{a}");
}
