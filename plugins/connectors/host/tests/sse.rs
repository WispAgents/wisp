//! Checking a remote connector over the 2024 SSE transport, end to end in one process: the GET that stays open names
//! where to POST (`endpoint`), each answer comes back on it, and the row lists the tools. A connector set to SSE is
//! asked that way at once; one whose streamable POST gets a 405 falls back to it (the check said "Didn't start" while
//! runs reached it). Also: a connector can't take the name of a server wisp gives every run. The server is a fake
//! `Net` whose `stream` is a channel.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use wisp_kernel::services::{NetRequest, NetResponse, NetStream};
use wisp_kernel::{BoxFuture, Caller, Kernel, Result};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

const SSE: &str = "https://old.test/sse";
const POST_TO: &str = "https://old.test/messages?sessionId=7";

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

/// An MCP server that speaks only the 2024 transport: a POST to the stream's URL is a 405; the GET opens the stream,
/// whose first event names where to POST; each POST there is a 202 and its answer goes down the stream.
#[derive(Default)]
struct OldServer {
    stream: Mutex<Option<UnboundedSender<Vec<u8>>>>,
    log: Mutex<Vec<(String, String)>>,
}

impl OldServer {
    fn requests(&self) -> Vec<(String, String)> {
        self.log.lock().unwrap().clone()
    }
}

fn event(name: &str, data: &str) -> Vec<u8> {
    format!("event: {name}\ndata: {data}\n\n").into_bytes()
}

impl wisp_kernel::services::Net for OldServer {
    fn fetch(&self, req: NetRequest) -> BoxFuture<'static, Result<NetResponse>> {
        self.log.lock().unwrap().push((req.method.clone(), req.url.clone()));
        let r = if req.method == "POST" && req.url == POST_TO {
            let m: Value = serde_json::from_slice(&req.body).unwrap_or_default();
            let result = match m["method"].as_str() {
                Some("initialize") => Some(json!({ "protocolVersion": "2024-11-05", "capabilities": {}, "serverInfo": { "name": "old" } })),
                Some("tools/list") => Some(json!({ "tools": [{ "name": "search" }, { "name": "fetch" }] })),
                _ => None,
            };
            if let (Some(result), Some(tx)) = (result, self.stream.lock().unwrap().as_ref()) {
                let line = json!({ "jsonrpc": "2.0", "id": m["id"], "result": result }).to_string();
                let _ = tx.send(event("message", &line));
            }
            NetResponse { status: 202, ..NetResponse::default() }
        } else {
            NetResponse { status: 405, ..NetResponse::default() }
        };
        Box::pin(async move { Ok(r) })
    }

    fn stream(&self, req: NetRequest) -> BoxFuture<'static, Result<NetStream>> {
        self.log.lock().unwrap().push((req.method.clone(), req.url.clone()));
        let sse = req.method == "GET"
            && req.url == SSE
            && req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("accept") && v.contains("text/event-stream"));
        if !sse {
            let f = self.fetch(req);
            return Box::pin(async move {
                let r = f.await?;
                Ok(NetStream { status: r.status, headers: r.headers, body: Box::pin(futures::stream::empty()) })
            });
        }
        let (tx, mut rx) = unbounded_channel();
        let _ = tx.send(event("endpoint", "/messages?sessionId=7"));
        *self.stream.lock().unwrap() = Some(tx);
        let body = futures::stream::poll_fn(move |cx| rx.poll_recv(cx).map(|c| c.map(Ok)));
        Box::pin(async move {
            Ok(NetStream { status: 200, headers: vec![("content-type".into(), "text/event-stream".into())], body: Box::pin(body) })
        })
    }
}

struct App {
    k: Kernel,
    server: Arc<OldServer>,
    dir: PathBuf,
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("wisp-connectors-sse-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let server = Arc::new(OldServer::default());
    let store = Store::open(Location::Path(dir.join("wisp.db")), Arc::new(SystemClock)).unwrap();
    let secrets: wisp_kernel::services::SecretsService = Arc::new(Secrets::default());
    let net: wisp_kernel::services::NetService = server.clone();
    let k = TestApp::new().service(store).service(secrets).service(net).with(wisp_connectors::ConnectorsPlugin).start().await;
    App { k, server, dir }
}

impl App {
    async fn ok(&self, id: &str, args: Value) -> Value {
        self.k.call(Caller::user(), id, args).await.unwrap_or_else(|e| panic!("{id}: {}", e.message))
    }
    async fn row(&self, id: &str) -> Value {
        let v = self.ok("connectors.list", json!({})).await;
        v["connectors"].as_array().unwrap().iter().find(|c| c["id"] == id).cloned().unwrap_or_default()
    }
    /// The row once its check is over.
    async fn tested(&self, id: &str) -> Value {
        for _ in 0..400 {
            let r = self.row(id).await;
            if r["test"].is_object() && r["testing"] == false {
                return r;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("the check of {id} never ended");
    }
}

/// A connector set to SSE is asked over SSE: the stream first, then initialize and tools/list where it said.
#[tokio::test(flavor = "multi_thread")]
async fn an_sse_connector_is_checked_over_its_event_stream() {
    let a = app("set").await;
    a.ok("connectors.add", json!({ "name": "old", "transport": "sse", "url": SSE })).await;
    let r = a.tested("old").await;
    assert_eq!(r["test"]["ok"], true, "{r}");
    assert_eq!(r["test"]["tools"], json!(["search", "fetch"]));
    let reqs = a.server.requests();
    assert_eq!(reqs[0], ("GET".to_owned(), SSE.to_owned()), "the stream first, no streamable POST: {reqs:?}");
    assert!(reqs[1..].iter().all(|(m, u)| m == "POST" && u == POST_TO), "{reqs:?}");
}

/// A server that only speaks SSE answers the streamable POST with a 405: the check falls back to the event stream.
#[tokio::test(flavor = "multi_thread")]
async fn a_405_to_the_streamable_post_falls_back_to_sse() {
    let a = app("fallback").await;
    a.ok("connectors.add", json!({ "name": "old", "transport": "remote", "url": SSE })).await;
    let r = a.tested("old").await;
    assert_eq!(r["test"]["ok"], true, "{r}");
    assert_eq!(r["test"]["count"], 2);
    let reqs = a.server.requests();
    assert_eq!(reqs[0], ("POST".to_owned(), SSE.to_owned()), "{reqs:?}");
    assert_eq!(reqs[1], ("GET".to_owned(), SSE.to_owned()), "{reqs:?}");
}

/// Claude and Codex run the tools of wisp's own servers (`browser`, `computer`, `wisp`) without asking, by name: a
/// connector by one of those names would too, so it can't be made.
#[tokio::test(flavor = "multi_thread")]
async fn a_connector_cant_take_the_name_of_wisps_own_server() {
    let a = app("reserved").await;
    for name in ["browser", "computer", "wisp"] {
        let e = a.k.call(Caller::user(), "connectors.add", json!({ "name": name, "transport": "remote", "url": SSE })).await.unwrap_err();
        assert!(e.message.contains("wisp gives every run"), "{name}: {}", e.message);
    }
    let v = a.ok("connectors.list", json!({})).await;
    assert_eq!(v["connectors"], json!([]));
}
