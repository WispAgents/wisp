//! Audit D6: a generated MCP tool whose command declares no argument type (`crew_list`, `later_plans`, `stuff_list`,
//! `machines_list`, …) answered a call with nothing, as its schema allows, with the command's type error
//! (`wisp: crew.list: invalid type: null, expected struct NoArgs`). It runs; its fields may be given at the top level.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{start_with, temp};
use serde_json::{Value, json};

async fn call(http: &reqwest::Client, port: u16, name: &str, arguments: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": arguments } });
    let r = http
        .post(format!("http://127.0.0.1:{port}/api/mcp"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(body.to_string())
        .send()
        .await
        .unwrap();
    let text = r.text().await.unwrap();
    // a JSON answer, or one SSE `data:` line
    let json = text.lines().find_map(|l| l.strip_prefix("data:")).map_or(text.as_str(), str::trim).to_owned();
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tool_whose_command_takes_no_fields_runs_when_called_with_nothing() {
    let dir = temp("serve-mcp-no-args");
    let (serve, port) = start_with(&dir, &[]);
    let http = reqwest::Client::new();
    for (tool, args) in
        [("crew_list", json!({})), ("later_plans", json!({})), ("stuff_list", json!({})), ("crew_list", json!({ "args": {} }))]
    {
        let r = call(&http, port, tool, args.clone()).await;
        let res = &r["result"];
        let text = res["content"][0]["text"].as_str().unwrap_or_default();
        assert!(res["isError"] != json!(true) && !text.contains("invalid type"), "{tool} {args}: {r}");
    }
    drop(serve);
}
