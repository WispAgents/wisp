//! Settings → Connectors end to end on a real `wisp serve` with the fake agents (v1 server/connectors.js,
//! connector-auth.js, test/connectors.test.js): a connector added once reaches every harness's runs, its secret
//! read for the run and never shown; a remote one signs in with OAuth against a stand-in server (discovery from
//! its 401, registration, PKCE, the browser's callback, the token as a bearer header); and wisp's own tools stay
//! callable in Claude Code's Plan mode and in Codex's Edits and Plan.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};

use common::{Api, ended, start_with, temp};
use serde_json::{Value, json};

/// Runs one task to the end of its turn; its id.
async fn run(api: &Api, dir: &Path, agent: &str, mode: &str) -> String {
    let made = api
        .cmd("runtime.create", json!({ "prompt": "hi", "cwd": dir.join("proj"), "agent": agent, "mode": mode, "worktree": false }))
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    api.until(&id, "the turn ends", |t| t.last().is_some_and(ended)).await;
    id
}

/// The last run's argv for `harness` (`claude`, `codex`).
fn last_argv(rec: &Path, harness: &str) -> Vec<String> {
    common::records(&rec.join(format!("{harness}.jsonl"))).last().map(common::argv).unwrap_or_default()
}

/// Claude's MCP config of its last run (the file `--mcp-config` names, ADR-0116).
fn mcp_config(rec: &Path) -> Value {
    common::mcp_config(common::records(&rec.join("claude.jsonl")).last().expect("a claude run"))
}

/// Codex's last run: its argv, and what its MCP servers read from its environment.
fn last_codex(rec: &Path) -> (String, Value) {
    let r = common::records(&rec.join("codex.jsonl")).last().cloned().unwrap_or_default();
    (common::argv(&r).join(" "), r["mcpEnv"].clone())
}

/// v1 #340: added once in Settings → Connectors, a connector goes to Claude Code's and Codex's runs alike, its
/// secret read into the run's env and never in the page's rows; switched off, no run gets it.
#[tokio::test(flavor = "multi_thread")]
async fn a_connector_added_in_settings_reaches_every_harness() {
    let dir = temp("serve-connectors-add");
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let (_serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let view = api
        .cmd("connectors.add", json!({ "name": "docs", "command": "docs-mcp", "args": ["--stdio"], "env": { "DOCS_TOKEN": "tok-777" } }))
        .await;
    assert!(!view.to_string().contains("tok-777"), "the page never carries a secret's value: {view}");
    let listed = api.cmd("connectors.list", json!({})).await;
    assert!(!listed.to_string().contains("tok-777"), "{listed}");

    run(&api, &dir, "claude", "edits").await;
    let cfg = mcp_config(&rec);
    assert_eq!(
        cfg["mcpServers"]["docs"],
        json!({ "type": "stdio", "command": "docs-mcp", "args": ["--stdio"], "env": { "DOCS_TOKEN": "tok-777" } }),
        "claude: the connector, its secret read: {cfg}"
    );
    run(&api, &dir, "codex", "edits").await;
    let (codex, codex_env) = last_codex(&rec);
    assert!(codex.contains(r#"mcp_servers.docs.command="docs-mcp""#), "codex: the same connector: {codex}");
    // its secret read, through codex's environment, never on its argv (ADR-0116, audit I IS3)
    assert!(codex.contains(r#"mcp_servers.docs.env_vars=["DOCS_TOKEN"]"#), "codex: its secret passed by name: {codex}");
    assert_eq!(codex_env["DOCS_TOKEN"], "tok-777", "codex: its secret read: {codex_env}");
    assert!(!codex.contains("tok-777"), "{codex}");

    api.cmd("connectors.update", json!({ "id": "docs", "enabled": false })).await;
    run(&api, &dir, "claude", "edits").await;
    assert!(mcp_config(&rec)["mcpServers"].get("docs").is_none(), "switched off: no run gets it");
    run(&api, &dir, "codex", "edits").await;
    assert!(!last_argv(&rec, "codex").join(" ").contains("mcp_servers.docs."), "nor codex's");
}

type Seen = Arc<Mutex<Vec<(String, String, String)>>>;

/// A remote MCP server that wants OAuth, with its own authorization server (MCP 2025-06-18): a POST without a
/// token is a 401 naming its metadata; `/register` gives `cid-1`; `/token` swaps `code-1` for `tok-abc`.
/// Every request kept as (method, path, body).
fn fake_oauth_server() -> (u16, Seen) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        let base = format!("http://127.0.0.1:{port}");
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap_or(0);
            let mut parts = line.split_whitespace();
            let (method, path) = (parts.next().unwrap_or("").to_owned(), parts.next().unwrap_or("").to_owned());
            let (mut len, mut auth) = (0, String::new());
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                    break;
                }
                let lower = h.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
                if lower.starts_with("authorization:") {
                    h["authorization:".len()..].trim().clone_into(&mut auth);
                }
            }
            let mut body = vec![0; len];
            r.read_exact(&mut body).unwrap_or(());
            let body = String::from_utf8_lossy(&body).into_owned();
            s2.lock().unwrap().push((method.clone(), path.clone(), body.clone()));
            let (status, extra, out) = match (method.as_str(), path.as_str()) {
                ("POST", "/mcp") if auth != "Bearer tok-abc" => (
                    "401 Unauthorized",
                    format!("www-authenticate: Bearer resource_metadata=\"{base}/.well-known/oauth-protected-resource\"\r\n"),
                    json!({ "error": "unauthorized" }),
                ),
                ("POST", "/mcp") => {
                    let rpc: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                    let result = if rpc["method"] == "tools/list" {
                        json!({ "tools": [{ "name": "list_issues", "inputSchema": { "type": "object" } }] })
                    } else {
                        json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} }, "serverInfo": { "name": "fake", "version": "1" } })
                    };
                    ("200 OK", String::new(), json!({ "jsonrpc": "2.0", "id": rpc["id"], "result": result }))
                }
                ("GET", "/.well-known/oauth-protected-resource") => {
                    ("200 OK", String::new(), json!({ "resource": format!("{base}/mcp"), "authorization_servers": [base] }))
                }
                ("GET", "/.well-known/oauth-authorization-server") => (
                    "200 OK",
                    String::new(),
                    json!({
                        "issuer": base,
                        "authorization_endpoint": format!("{base}/authorize"),
                        "token_endpoint": format!("{base}/token"),
                        "registration_endpoint": format!("{base}/register"),
                        "code_challenge_methods_supported": ["S256"]
                    }),
                ),
                ("POST", "/register") => ("201 Created", String::new(), json!({ "client_id": "cid-1" })),
                ("POST", "/token") if body.contains("code=code-1") => (
                    "200 OK",
                    String::new(),
                    json!({ "access_token": "tok-abc", "refresh_token": "ref-1", "expires_in": 3600, "token_type": "Bearer" }),
                ),
                ("POST", "/token") => ("400 Bad Request", String::new(), json!({ "error": "invalid_grant" })),
                _ => ("404 Not Found", String::new(), json!({})),
            };
            let out = out.to_string();
            let _ = write!(
                s,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n{out}",
                out.len()
            );
        }
    });
    (port, seen)
}

fn query_of(url: &str) -> std::collections::BTreeMap<String, String> {
    reqwest::Url::parse(url).unwrap().query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect()
}

/// v1 connector-auth.js: Sign in on a remote connector finds the server's authorization server from its 401,
/// registers wisp, sends the browser there with PKCE; the callback swaps the code (with the verifier) for a
/// token kept in secrets, and every run then reaches the server with it as a bearer header. A callback with a
/// state wisp never gave is refused; Sign out takes the header away again.
#[tokio::test(flavor = "multi_thread")]
async fn a_remote_connector_signs_in_with_oauth_and_runs_get_its_token() {
    let (mcp, seen) = fake_oauth_server();
    let dir = temp("serve-connectors-oauth");
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let (_serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let server = format!("http://127.0.0.1:{mcp}/mcp");
    api.cmd("connectors.add", json!({ "name": "linear", "url": server })).await;

    let origin = format!("http://127.0.0.1:{port}");
    let out = api.cmd("connectors.signin", json!({ "id": "linear", "origin": origin })).await;
    let url = out["url"].as_str().unwrap().to_owned();
    assert!(url.starts_with(&format!("http://127.0.0.1:{mcp}/authorize?")), "the issuer's own sign-in page: {url}");
    let q = query_of(&url);
    assert_eq!(q.get("client_id").map(String::as_str), Some("cid-1"), "registered: {url}");
    assert_eq!(q.get("code_challenge_method").map(String::as_str), Some("S256"), "PKCE: {url}");
    assert!(q.get("code_challenge").is_some_and(|c| c.len() >= 43), "{url}");
    assert_eq!(q.get("redirect_uri").map(String::as_str), Some(format!("{origin}/api/connectors/oauth/callback").as_str()));
    let state = q.get("state").cloned().unwrap();

    let back = |query: String| {
        let (http, at) = (api.http.clone(), origin.clone());
        async move { http.get(format!("{at}/api/connectors/oauth/callback?{query}")).send().await.unwrap().text().await.unwrap() }
    };
    let forged = back("state=not-ours&code=code-1".into()).await;
    assert!(!forged.contains("tok-abc"), "{forged}");
    assert!(!seen.lock().unwrap().iter().any(|(_, p, _)| p == "/token"), "a state wisp never gave swaps nothing");
    let page = back(format!("state={state}&code=code-1")).await;
    assert!(page.to_lowercase().contains("linear"), "the page says which one signed in: {page}");
    let token_req = seen.lock().unwrap().iter().find(|(_, p, _)| p == "/token").cloned().unwrap();
    assert!(token_req.2.contains("code_verifier=") && token_req.2.contains("grant_type=authorization_code"), "{token_req:?}");
    let again = back(format!("state={state}&code=code-1")).await;
    assert_eq!(seen.lock().unwrap().iter().filter(|(_, p, _)| p == "/token").count(), 1, "a state is used once: {again}");

    let listed = api.cmd("connectors.list", json!({})).await;
    assert!(!listed.to_string().contains("tok-abc"), "the token never on the page: {listed}");
    run(&api, &dir, "claude", "edits").await;
    let cfg = mcp_config(&rec);
    assert_eq!(cfg["mcpServers"]["linear"]["url"], json!(server), "{cfg}");
    assert_eq!(cfg["mcpServers"]["linear"]["headers"]["Authorization"], "Bearer tok-abc", "runs reach it signed in: {cfg}");

    api.cmd("connectors.signout", json!({ "id": "linear" })).await;
    run(&api, &dir, "claude", "edits").await;
    let cfg = mcp_config(&rec);
    assert!(cfg["mcpServers"]["linear"]["headers"].get("Authorization").is_none(), "signed out: no token: {cfg}");
}

/// v1 #634 and #340: Claude Code's Plan mode refuses an MCP tool without `readOnlyHint` whatever
/// `--allowedTools` says, so wisp's read/ask/show tools carry it and still answer; Codex in Edits and Plan gets
/// wisp's server (and a connector) with `default_tools_approval_mode="approve"`, without which `codex exec`'s
/// approval policy refuses every MCP call.
#[tokio::test(flavor = "multi_thread")]
async fn wisps_tools_work_in_plan_mode_and_in_codex_edits_and_plan() {
    let dir = temp("serve-connectors-plan");
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let (_serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    api.cmd("connectors.add", json!({ "name": "docs", "command": "docs-mcp" })).await;

    let id = run(&api, &dir, "claude", "plan").await;
    let argv = last_argv(&rec, "claude");
    let cfg = mcp_config(&rec);
    let wisp = &cfg["mcpServers"]["wisp"];
    assert!(wisp.is_object(), "plan mode still gets wisp's server: {cfg}");
    let token = wisp["headers"]["x-wisp-token"].as_str().unwrap().to_owned();
    let mcp = |body: Value| {
        let (http, token, id) = (api.http.clone(), token.clone(), id.clone());
        async move {
            let r = http
                .post(format!("http://127.0.0.1:{port}/api/mcp"))
                .header("content-type", "application/json")
                .header("x-wisp-token", token)
                .header("x-wisp-session", id)
                .body(body.to_string())
                .send()
                .await
                .unwrap();
            serde_json::from_slice::<Value>(&r.bytes().await.unwrap()).unwrap_or(Value::Null)
        }
    };
    let list = mcp(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    let tools = list["result"]["tools"].as_array().cloned().unwrap_or_default();
    let hinted = |n: &str| tools.iter().find(|t| t["name"] == n).is_some_and(|t| t["annotations"]["readOnlyHint"] == true);
    for n in ["ask_user", "wisp_tag", "wisp_sessions", "wisp_connectors"] {
        assert!(hinted(n), "{n} carries readOnlyHint, the only thing Plan mode lets through: {list}");
    }
    let allowed: Vec<&String> = argv.iter().skip_while(|a| *a != "--allowedTools").skip(1).take_while(|a| !a.starts_with("--")).collect();
    assert!(allowed.iter().any(|a| *a == "mcp__wisp"), "wisp's tools allowed in plan mode: {allowed:?}");
    let tag = mcp(
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "wisp_tag", "arguments": { "tags": ["plan"] } } }),
    )
    .await;
    assert_ne!(tag["result"]["isError"], true, "a plan-mode task tags itself: {tag}");
    assert!(tag.get("error").is_none(), "{tag}");

    for mode in ["edits", "plan"] {
        run(&api, &dir, "codex", mode).await;
        let a = last_argv(&rec, "codex").join(" ");
        assert!(a.contains(r#"mcp_servers.wisp.default_tools_approval_mode="approve""#), "codex {mode}: wisp's tools aren't refused: {a}");
        assert!(a.contains(r#"mcp_servers.docs.default_tools_approval_mode="approve""#), "codex {mode}: nor a connector's: {a}");
    }
}

/// Audit I IS3 (ADR-0116): a connector's secret env and the run's wisp token were on the harness's command line
/// (Claude's inline `--mcp-config`, Codex's `-c …env=` and `http_headers=`), which any local user reads with `ps`. Run on
/// every harness, no recorded argv holds the secret, the token or the `x-wisp-token` header; Claude's and Copilot's
/// config files are gone once their run ends.
#[tokio::test(flavor = "multi_thread")]
async fn no_harness_gets_a_secret_or_its_wisp_token_on_its_command_line_audit_i_is3() {
    let dir = temp("serve-connectors-argv");
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let (_serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    api.cmd("connectors.add", json!({ "name": "docs", "command": "docs-mcp", "env": { "DOCS_TOKEN": "argv-SECRET-tok-913" } })).await;
    for (agent, bin) in [
        ("claude", "claude"),
        ("codex", "codex"),
        ("opencode", "opencode"),
        ("cursor", "cursor-agent"),
        ("copilot", "copilot"),
        ("grok", "grok"),
        ("antigravity", "agy"),
    ] {
        run(&api, &dir, agent, "edits").await;
        let r = common::records(&rec.join(format!("{bin}.jsonl"))).last().cloned().unwrap_or_else(|| panic!("{agent}: a run"));
        let argv = common::argv(&r).join(" ");
        assert!(!argv.contains("argv-SECRET-tok-913"), "{agent}: the connector's secret on argv: {argv}");
        // the header may be named (codex reads its value from a variable: `x-wisp-token="WISP_MCP_…"`), never given one
        for (at, _) in argv.match_indices("x-wisp-token") {
            let after = argv[at + "x-wisp-token".len()..].trim_start_matches(['"', ':', '=', ' ']);
            assert!(after.starts_with("WISP_MCP_"), "{agent}: wisp's token header with its value on argv: {argv}");
        }
        let token = r["mcpConfig"]["mcpServers"]["wisp"]["headers"]["x-wisp-token"].as_str().unwrap_or_default();
        if !token.is_empty() {
            assert!(!argv.contains(token), "{agent}: the run's token on argv");
        }
        for (flag, file) in [("--mcp-config", ""), ("--additional-mcp-config", "@")] {
            if let Some(i) = common::argv(&r).iter().position(|a| a == flag) {
                let path = common::argv(&r)[i + 1].trim_start_matches(file).to_owned();
                assert!(!r["mcpConfig"].is_null(), "{agent}: the config was there for the run");
                for _ in 0..100 {
                    if !Path::new(&path).exists() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                assert!(!Path::new(&path).exists(), "{agent}: its config file is removed when the run ends: {path}");
            }
        }
    }
    // the token Claude's run had reached its MCP calls all the same (from the file)
    assert_ne!(common::wisp_token(common::records(&rec.join("claude.jsonl")).last().unwrap()), "");
}
