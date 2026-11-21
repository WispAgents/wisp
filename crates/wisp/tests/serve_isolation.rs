//! A test server never reads the real user's files (owner-list row 66: `serve_mcp_tools` read this Mac's real Cursor
//! and opencode MCP servers). Every harness's config is found under the test's own home (`common::user_home`), and
//! nothing else: each harness here gets one server planted there, and the servers wisp finds are exactly those, on
//! any machine (one with real configs would add its own; one without would find none of these).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn a_test_server_finds_harness_configs_only_in_its_own_home_never_the_real_one() {
    let dir = temp("isolation");
    let home = user_home(&dir);
    let put = |rel: &str, text: &str| {
        let p = home.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    let server = |name: &str| json!({ "mcpServers": { name: { "command": "echo", "args": [name] } } }).to_string();
    put(".claude.json", &server("iso-claude"));
    put(".cursor/mcp.json", &server("iso-cursor"));
    put(".copilot/mcp-config.json", &server("iso-copilot"));
    put(".config/opencode/opencode.json", &json!({ "mcp": { "iso-opencode": { "type": "local", "command": ["echo", "x"] } } }).to_string());
    // CODEX_HOME is the test's own (common::start_with), also under the test's folder
    let codex = dir.join("codex-home/config.toml");
    std::fs::create_dir_all(codex.parent().unwrap()).unwrap();
    std::fs::write(&codex, "[mcp_servers.iso-codex]\ncommand = \"echo\"\n").unwrap();

    let (serve, port) = start(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let v = api.cmd("connectors.list", json!({})).await;
    let mut found: Vec<String> = v["found"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap().to_owned()).collect();
    found.sort();
    assert_eq!(found, ["iso_claude", "iso_codex", "iso_copilot", "iso_cursor", "iso_opencode"], "only the test's own home is read: {v}");
    drop(serve);
}

/// Every test server starts through `common::start_with`, which isolates it: no test spawns `wisp serve` on its own.
#[test]
fn no_test_starts_a_server_outside_common() {
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    for e in std::fs::read_dir(&tests).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        let spawns = text.contains("CARGO_BIN_EXE_wisp\"))");
        let serves = text.contains(".arg(\"serve\")") || text.contains(".args([\"serve\"");
        assert!(!(spawns && serves), "{} starts `wisp serve` itself: use common::start_with (an isolated home)", p.display());
    }
}
