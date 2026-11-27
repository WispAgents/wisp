//! MCP Events from a remote connector (ADR-0145), in one process: `connectors.events` lists what the server serves,
//! and the `mcp.event` trigger source polls it: the first poll starts from now, later ones bring what happened since
//! (each event keyed once, the cursor kept), `hasMore` drained at once, a server with no events said in words, and a
//! modern-only server (it refuses `initialize`) asked the `2026-07-28` way. The server is a fake `Net`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::many_single_char_names)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::services::{NetRequest, NetResponse};
use wisp_kernel::{BoxFuture, Caller, Kernel, Result};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};

const MCP: &str = "https://events.test/mcp";

#[derive(Default)]
struct Secrets(Mutex<BTreeMap<String, String>>);
impl wisp_kernel::services::Secrets for Secrets {
    fn get(&self, key: &str) -> BoxFuture<'static, Result<Option<String>>> {
        let v = self.0.lock().unwrap().get(key).cloned();
        Box::pin(async move { Ok(v) })
    }
    fn set(&self, key: &str, value: &str) -> BoxFuture<'static, Result<()>> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Box::pin(async { Ok(()) })
    }
    fn delete(&self, key: &str) -> BoxFuture<'static, Result<()>> {
        self.0.lock().unwrap().remove(key);
        Box::pin(async { Ok(()) })
    }
    fn keys(&self, prefix: &str) -> BoxFuture<'static, Result<Vec<String>>> {
        let ks = self.0.lock().unwrap().keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        Box::pin(async move { Ok(ks) })
    }
}

/// An events server: `happened` is its upstream, in order; a cursor is how many it had seen. `events: false` serves
/// none (method not found); `modern` refuses `initialize` with a 400, as a `2026-07-28`-only server does.
#[derive(Default)]
struct Server {
    st: Mutex<St>,
}
#[derive(Default)]
struct St {
    no_events: bool,
    modern: bool,
    page: usize,
    happened: Vec<(String, String)>,
    /// every request: (method, the MCP-Protocol-Version header, params)
    log: Vec<(String, Option<String>, Value)>,
}

fn reply(status: u16, body: &Value) -> NetResponse {
    NetResponse { status, headers: vec![("content-type".into(), "application/json".into())], body: body.to_string().into_bytes() }
}

impl Server {
    fn with<T>(&self, f: impl FnOnce(&mut St) -> T) -> T {
        f(&mut self.st.lock().unwrap())
    }
    fn answer(&self, req: &NetRequest) -> NetResponse {
        let m: Value = serde_json::from_slice(&req.body).unwrap_or_default();
        let version = req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("mcp-protocol-version")).map(|(_, v)| v.clone());
        let mut s = self.st.lock().unwrap();
        let method = m["method"].as_str().unwrap_or_default().to_owned();
        s.log.push((method.clone(), version.clone(), m["params"].clone()));
        let id = m["id"].clone();
        let ok = |result: Value| reply(200, &json!({ "jsonrpc": "2.0", "id": id, "result": result }));
        let err =
            |code: i64, message: &str| reply(200, &json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }));
        match method.as_str() {
            "initialize" if s.modern => reply(400, &json!({ "error": "initialize is a legacy method" })),
            "initialize" => ok(
                json!({ "protocolVersion": "2025-06-18", "capabilities": { "extensions": { "io.modelcontextprotocol/events": {} } }, "serverInfo": { "name": "fake" } }),
            ),
            "notifications/initialized" => NetResponse { status: 202, ..NetResponse::default() },
            "tools/list" => ok(json!({ "tools": [] })),
            _ if s.no_events => err(-32601, "Method not found"),
            _ if s.modern && version.as_deref() != Some("2026-07-28") => reply(400, &json!({})),
            "events/list" => {
                ok(json!({ "events": [{ "name": "incident.created", "delivery": ["poll"], "inputSchema": { "type": "object" } }] }))
            }
            "events/poll" => {
                let p = &m["params"];
                if p["name"] != "incident.created" {
                    return err(-32023, "NotFound");
                }
                let n = s.happened.len();
                let Some(c) = p["cursor"].as_str() else {
                    return ok(json!({ "events": [], "cursor": n.to_string(), "hasMore": false, "nextPollMs": 1000 }));
                };
                let from: usize = c.parse().unwrap();
                let take = if s.page > 0 { s.page } else { usize::MAX };
                let batch: Vec<Value> = s.happened[from..]
                    .iter()
                    .take(take)
                    .map(|(eid, title)| json!({ "eventId": eid, "name": "incident.created", "timestamp": "2026-11-27T10:00:00Z", "data": { "title": title, "service": "api" } }))
                    .collect();
                let to = from + batch.len();
                ok(json!({ "events": batch, "cursor": to.to_string(), "hasMore": to < n, "truncated": false, "nextPollMs": 1000 }))
            }
            _ => err(-32601, "Method not found"),
        }
    }
}
impl wisp_kernel::services::Net for Server {
    fn fetch(&self, req: NetRequest) -> BoxFuture<'static, Result<NetResponse>> {
        let r = self.answer(&req);
        Box::pin(async move { Ok(r) })
    }
}

struct App {
    k: Kernel,
    server: Arc<Server>,
    dir: std::path::PathBuf,
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn app(name: &str, server: Server) -> App {
    let dir = std::env::temp_dir().join(format!("wisp-connectors-events-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let server = Arc::new(server);
    let store = Store::open(Location::Path(dir.join("wisp.db")), Arc::new(SystemClock)).unwrap();
    let (sec, net): (wisp_kernel::services::SecretsService, wisp_kernel::services::NetService) =
        (Arc::new(Secrets::default()), server.clone());
    let k = TestApp::new()
        .clock(&FakeClock::at(1_000_000_000))
        .service(store)
        .service(sec)
        .service(net)
        .with(wisp_connectors::ConnectorsPlugin)
        .start()
        .await;
    k.call(
        Caller::user(),
        "connectors.add",
        json!({ "name": "pd", "transport": "remote", "url": MCP, "headers": { "Authorization": "Bearer k" } }),
    )
    .await
    .unwrap();
    App { k, server, dir }
}

impl App {
    fn source(&self) -> Arc<dyn wisp_automations_api::TriggerSource> {
        self.k
            .contributions(&wisp_automations_api::SOURCE)
            .into_iter()
            .find(|s| s.ons().iter().any(|o| o.on == "mcp.event"))
            .expect("mcp.event")
    }
    async fn poll(&self, cursor: Value) -> Result<wisp_automations_api::Polled> {
        let spec = json!({ "on": "mcp.event", "prompt": "x", "match": { "connector": "pd", "event": "incident.created", "arguments": { "severity": "P1" } } });
        self.source().poll("mcp.event", &spec, cursor).await
    }
    fn happen(&self, id: &str, title: &str) {
        self.server.with(|s| s.happened.push((id.into(), title.into())));
    }
    fn sent(&self, method: &str) -> Vec<(Option<String>, Value)> {
        self.server.with(|s| s.log.iter().filter(|(m, _, _)| m == method).map(|(_, v, p)| (v.clone(), p.clone())).collect())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connectors_events_start_triggers_from_its_cursor_each_once() {
    let a = app("poll", Server::default()).await;
    let listed = a.k.call(Caller::user(), "connectors.events", json!({ "id": "pd" })).await.unwrap();
    assert_eq!(listed["served"], true);
    assert_eq!(listed["events"][0]["name"], "incident.created");
    a.happen("e0", "before the trigger");
    // the first poll: from now on, nothing from before
    let first = a.poll(Value::Null).await.unwrap();
    assert!(first.events.is_empty(), "{first:?}");
    assert_eq!(first.cursor, json!({ "c": "1" }));
    let sent = a.sent("events/poll");
    assert_eq!(sent[0].1["cursor"], Value::Null);
    assert_eq!(sent[0].1["arguments"], json!({ "severity": "P1" }));
    assert!(sent[0].1["_meta"]["io.modelcontextprotocol/clientCapabilities"]["extensions"]["io.modelcontextprotocol/events"].is_object());
    a.happen("e1", "Database down");
    a.happen("e2", "Disk full");
    let next = a.poll(first.cursor.clone()).await.unwrap();
    let keys: Vec<&str> = next.events.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, ["pd:incident.created:e1", "pd:incident.created:e2"]);
    assert_eq!(next.events[0].on, "mcp.event");
    assert_eq!(next.events[0].summary, "pd: incident.created: Database down");
    assert_eq!(next.events[0].at, 1_795_773_600_000);
    // the payload echoes the match, so the engine's filter passes it; data is the server's
    assert_eq!(
        (next.events[0].payload["connector"].clone(), next.events[0].payload["event"].clone()),
        (json!("pd"), json!("incident.created"))
    );
    assert_eq!(next.events[0].payload["data"]["service"], "api");
    let again = a.poll(next.cursor.clone()).await.unwrap();
    assert!(again.events.is_empty(), "nothing new: {again:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn has_more_is_drained_in_one_poll() {
    let a = app("more", Server { st: Mutex::new(St { page: 1, ..St::default() }) }).await;
    let first = a.poll(Value::Null).await.unwrap();
    for i in 0..3 {
        a.happen(&format!("e{i}"), "x");
    }
    let next = a.poll(first.cursor).await.unwrap();
    assert_eq!(next.events.len(), 3, "three pages of one");
    assert_eq!(next.cursor, json!({ "c": "3" }));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_with_no_events_says_so() {
    let a = app("none", Server { st: Mutex::new(St { no_events: true, ..St::default() }) }).await;
    let listed = a.k.call(Caller::user(), "connectors.events", json!({ "id": "pd" })).await.unwrap();
    assert_eq!(listed, json!({ "served": false, "events": [] }));
    let e = a.poll(Value::Null).await.unwrap_err();
    assert_eq!(e.message, "pd serves no MCP events");
    // a trigger without its event is refused before any request
    let spec = json!({ "on": "mcp.event", "match": { "connector": "pd" } });
    let e = a.source().poll("mcp.event", &spec, Value::Null).await.unwrap_err();
    assert!(e.message.contains("match.event"), "{}", e.message);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_modern_only_server_is_asked_the_2026_07_28_way() {
    let a = app("modern", Server { st: Mutex::new(St { modern: true, ..St::default() }) }).await;
    let first = a.poll(Value::Null).await.unwrap();
    assert_eq!(first.cursor, json!({ "c": "0" }));
    let sent = a.sent("events/poll");
    let (version, params) = sent.last().unwrap();
    assert_eq!(version.as_deref(), Some("2026-07-28"));
    assert_eq!(params["_meta"]["io.modelcontextprotocol/protocolVersion"], "2026-07-28");
    assert!(params["_meta"]["io.modelcontextprotocol/clientCapabilities"]["extensions"]["io.modelcontextprotocol/events"].is_object());
}
