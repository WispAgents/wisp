//! MCP Events from a remote connector (ADR-0146), in one process: `connectors.events` lists what the server serves,
//! and the `mcp.event` trigger source polls it: the first poll starts from now, later ones bring what happened since
//! (each event keyed once, the cursor kept), `hasMore` drained at once, a server with no events said in words, and a
//! modern-only server (it refuses `initialize`) asked the `2026-07-28` way. The server is a fake `Net`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::assert_is_empty,
    clippy::struct_excessive_bools
)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use std::time::Duration;

use wisp_automations_api::TriggerEvent;
use wisp_automations_api::commands::{Count, TriggerPush};
use wisp_kernel::services::{NetRequest, NetResponse, NetStream};
use wisp_kernel::{BoxFuture, Caller, Kernel, Manifest, Plugin, Result, Setup};
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
    /// its listing offers push, and `events/stream` answers with SSE fed by `stream`
    push: bool,
    /// its listing offers webhook; `events/subscribe` refuses (it can't reach the address)
    webhook: bool,
    stream: Option<futures::channel::mpsc::UnboundedSender<Vec<u8>>>,
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
                let mut delivery = if s.push { json!(["push", "poll"]) } else { json!(["poll"]) };
                if s.webhook {
                    delivery.as_array_mut().unwrap().insert(0, json!("webhook"));
                }
                ok(json!({ "events": [{ "name": "incident.created", "delivery": delivery, "inputSchema": { "type": "object" } }] }))
            }
            "events/subscribe" => err(-32027, "CallbackEndpointError"),
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
    fn stream(&self, req: NetRequest) -> BoxFuture<'static, Result<NetStream>> {
        let m: Value = serde_json::from_slice(&req.body).unwrap_or_default();
        let mut s = self.st.lock().unwrap();
        if m["method"] != "events/stream" || !s.push {
            drop(s);
            let r = self.answer(&req);
            return Box::pin(async move {
                Ok(NetStream { status: r.status, headers: r.headers, body: Box::pin(futures::stream::once(async move { Ok(r.body) })) })
            });
        }
        s.log.push(("events/stream".into(), None, m["params"].clone()));
        let (tx, rx) = futures::channel::mpsc::unbounded::<Vec<u8>>();
        s.stream = Some(tx);
        let body = futures::StreamExt::boxed(futures::StreamExt::map(rx, Ok));
        Box::pin(async move { Ok(NetStream { status: 200, headers: vec![("content-type".into(), "text/event-stream".into())], body }) })
    }
}

/// wisp.automations as far as the source calls it: `automations.trigger.push` keeps what it got.
#[derive(Clone, Default)]
struct Pushed(Arc<Mutex<Vec<TriggerEvent>>>);
struct Automations(Pushed);
impl Plugin for Automations {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.automations", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let got = self.0.clone();
        s.command::<TriggerPush, _, _>(move |_, _, ev: TriggerEvent| {
            got.0.lock().unwrap().push(ev);
            async move { Ok(Count { n: 1 }) }
        });
        Ok(())
    }
}

struct App {
    k: Kernel,
    server: Arc<Server>,
    pushed: Pushed,
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
    let pushed = Pushed::default();
    let store = Store::open(Location::Path(dir.join("wisp.db")), Arc::new(SystemClock)).unwrap();
    let (sec, net): (wisp_kernel::services::SecretsService, wisp_kernel::services::NetService) =
        (Arc::new(Secrets::default()), server.clone());
    let k = TestApp::new()
        .clock(&FakeClock::at(1_000_000_000))
        .service(store)
        .service(sec)
        .service(net)
        .with(wisp_connectors::ConnectorsPlugin)
        .with(Automations(pushed.clone()))
        .start()
        .await;
    k.call(
        Caller::user(),
        "connectors.add",
        json!({ "name": "pd", "transport": "remote", "url": MCP, "headers": { "Authorization": "Bearer k" } }),
    )
    .await
    .unwrap();
    App { k, server, pushed, dir }
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
    /// One SSE frame down the open stream.
    fn say(&self, msg: &Value) {
        self.server.with(|s| s.stream.as_ref().unwrap().unbounded_send(format!("data: {msg}\n\n").into_bytes()).unwrap());
    }
    fn pushed(&self) -> Vec<String> {
        self.pushed.0.lock().unwrap().iter().map(|e| e.key.clone()).collect()
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

async fn eventually(what: &str, f: impl Fn() -> bool) {
    for _ in 0..600 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never: {what}");
}

/// Push (ADR-0146): offered, the first poll opens `events/stream`; each event reaches the engine as it comes
/// (`automations.trigger.push`), and a poll meanwhile asks nothing and hands back the stream's cursor. A stream that
/// drops opens again from its cursor; one the server ends hands back to polling, from that same cursor.
#[tokio::test(flavor = "multi_thread")]
async fn push_brings_each_event_at_once_and_polls_take_over_when_it_ends() {
    let a = app("push", Server { st: Mutex::new(St { push: true, ..St::default() }) }).await;
    let first = a.poll(Value::Null).await.unwrap();
    assert_eq!(first.events, []);
    eventually("the stream is open", || a.server.with(|s| s.stream.is_some())).await;
    assert_eq!(a.sent("events/stream")[0].1["cursor"], Value::Null, "from now");
    assert_eq!(a.sent("events/stream")[0].1["arguments"], json!({ "severity": "P1" }));
    a.say(&json!({ "jsonrpc": "2.0", "method": "notifications/events/active", "params": { "cursor": "0", "heartbeatMs": 15000 } }));
    let event = json!({ "jsonrpc": "2.0", "method": "notifications/events/event", "params": { "eventId": "e1", "name": "incident.created", "timestamp": "2026-11-27T10:00:00Z", "data": { "title": "Database down" }, "cursor": "1" } });
    a.say(&event);
    eventually("the event reached the triggers", || a.pushed() == ["pd:incident.created:e1"]).await;
    // a poll while it streams: no request, the stream's cursor for the trigger's row
    let polls = a.sent("events/poll").len();
    let p = a.poll(first.cursor.clone()).await.unwrap();
    assert_eq!(p.events, []);
    assert_eq!(p.cursor, json!({ "c": "1" }));
    assert_eq!(a.sent("events/poll").len(), polls, "no poll while it streams");
    // it drops: opened again, from its cursor
    a.server.with(|s| s.stream = None);
    eventually("opened again", || a.sent("events/stream").len() == 2).await;
    assert_eq!(a.sent("events/stream")[1].1["cursor"], "1");
    eventually("the new stream is there", || a.server.with(|s| s.stream.is_some())).await;
    // the server ends it: polling carries on from the stream's cursor
    a.say(&json!({ "jsonrpc": "2.0", "method": "notifications/events/terminated", "params": { "error": { "code": -32024, "message": "Forbidden" } } }));
    tokio::time::sleep(Duration::from_millis(100)).await;
    a.happen("e0", "seen by the server before");
    a.happen("e2", "after");
    let p = a.poll(json!({ "c": "0" })).await.unwrap();
    assert_eq!(a.sent("events/poll").last().unwrap().1["cursor"], "1", "the stream's cursor, not the engine's older one");
    assert_eq!(p.events.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["pd:incident.created:e2"]);
}

/// A trigger that says `match.delivery: poll` is polled even when the server offers push.
#[tokio::test(flavor = "multi_thread")]
async fn a_trigger_can_keep_to_polling() {
    let a = app("keep-poll", Server { st: Mutex::new(St { push: true, ..St::default() }) }).await;
    let spec = json!({ "on": "mcp.event", "match": { "connector": "pd", "event": "incident.created", "delivery": "poll" } });
    a.source().poll("mcp.event", &spec, Value::Null).await.unwrap();
    assert!(a.sent("events/stream").is_empty());
    assert_eq!(a.sent("events/poll").len(), 1);
}

/// Webhook (ADR-0146): with a public address set and webhook offered, the first poll subscribes; a refused
/// subscribe leaves the trigger to poll at once, and it isn't asked again on the next poll (an hour).
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_webhook_falls_back_to_polling_and_isnt_asked_again_soon() {
    let a = app("hook-refused", Server { st: Mutex::new(St { webhook: true, ..St::default() }) }).await;
    a.k.set_setting("connectors.events.callback", Some(json!("https://me.example"))).unwrap();
    let first = a.poll(Value::Null).await.unwrap();
    let subscribed = a.sent("events/subscribe");
    assert_eq!(subscribed.len(), 1);
    let d = &subscribed[0].1["delivery"];
    assert_eq!(d["mode"], "webhook");
    assert!(d["url"].as_str().unwrap().starts_with("https://me.example/api/connectors/events/"), "{d}");
    assert!(d["secret"].as_str().unwrap().starts_with("whsec_"));
    assert_eq!(a.sent("events/poll").len(), 1, "polled at once instead");
    a.poll(first.cursor).await.unwrap();
    assert_eq!(a.sent("events/subscribe").len(), 1, "not asked again so soon");
    assert_eq!(a.sent("events/poll").len(), 2);
}

/// Without a public address, a webhook offer is ignored: no subscribe, push or poll as before.
#[tokio::test(flavor = "multi_thread")]
async fn without_a_public_address_no_webhook_is_asked_for() {
    let a = app("hook-none", Server { st: Mutex::new(St { webhook: true, ..St::default() }) }).await;
    a.poll(Value::Null).await.unwrap();
    assert_eq!(a.sent("events/subscribe"), []);
    assert_eq!(a.sent("events/poll").len(), 1);
}
