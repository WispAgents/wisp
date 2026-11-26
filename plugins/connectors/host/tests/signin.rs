//! A signed-in remote connector end to end in one process: the sign-in (registration, code, tokens), a token that ran
//! out or was refused refreshed once and tried once more, a sign-in that can't be renewed left on the row as "needs
//! sign-in" (kept across a restart), and a sign-in started again that stores nothing until it's finished. The server
//! and its sign-in endpoints are a fake `Net`; the clock a `FakeClock`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_kernel::services::{NetRequest, NetResponse};
use wisp_kernel::{BoxFuture, Caller, Kernel, Result};
use wisp_runtime_api::model::Session;
use wisp_runtime_api::points::{McpQuery, RUN};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};

const MCP: &str = "https://mcp.test/mcp";
const TOKEN: &str = "https://mcp.test/token";
const HERE: &str = "http://localhost:4477";

#[derive(Default)]
struct Secrets(Mutex<BTreeMap<String, String>>);
impl Secrets {
    fn get(&self, k: &str) -> Option<String> {
        self.0.lock().unwrap().get(k).cloned()
    }
}
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

/// The connector's server and its sign-in: bearers in `valid` are let in; the token endpoint trades a code, and a
/// refresh token while `refresh_ok`, for `a<n>`/`r<n>` that last an hour.
struct Server {
    st: Mutex<St>,
}
struct St {
    valid: HashSet<String>,
    reject_all: bool,
    refresh_ok: bool,
    refresh: Option<String>,
    reg_secret: Option<String>,
    n: u32,
    log: Vec<(String, String, Option<String>, String)>,
}
impl Default for Server {
    fn default() -> Self {
        Self {
            st: Mutex::new(St {
                valid: HashSet::new(),
                reject_all: false,
                refresh_ok: true,
                refresh: None,
                reg_secret: None,
                n: 0,
                log: Vec::new(),
            }),
        }
    }
}

fn reply(status: u16, body: &Value) -> NetResponse {
    NetResponse { status, headers: vec![("content-type".into(), "application/json".into())], body: body.to_string().into_bytes() }
}

impl Server {
    fn with<T>(&self, f: impl FnOnce(&mut St) -> T) -> T {
        f(&mut self.st.lock().unwrap())
    }
    /// Every form sent to the token endpoint.
    fn token_forms(&self) -> Vec<BTreeMap<String, String>> {
        self.with(|s| s.log.iter().filter(|(_, u, _, _)| u == TOKEN).map(|(_, _, _, b)| url_decode(b)).collect())
    }
    fn refreshes(&self) -> usize {
        self.token_forms().iter().filter(|f| f["grant_type"] == "refresh_token").count()
    }
    /// The bearer of every MCP request whose body names `method`.
    fn bearers(&self, method: &str) -> Vec<Option<String>> {
        self.with(|s| {
            s.log.iter().filter(|(_, u, _, b)| u == MCP && b.contains(&format!("\"{method}\""))).map(|(_, _, a, _)| a.clone()).collect()
        })
    }
    fn clear(&self) {
        self.with(|s| s.log.clear());
    }
    fn answer(&self, req: &NetRequest) -> NetResponse {
        let body = String::from_utf8_lossy(&req.body).into_owned();
        let auth = req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("authorization")).map(|(_, v)| v.clone());
        let mut s = self.st.lock().unwrap();
        s.log.push((req.method.clone(), req.url.clone(), auth.clone(), body.clone()));
        match (req.method.as_str(), req.url.as_str()) {
            ("GET", "https://mcp.test/.well-known/oauth-authorization-server") => reply(
                200,
                &json!({ "issuer": "https://mcp.test", "authorization_endpoint": "https://mcp.test/authorize", "token_endpoint": TOKEN, "registration_endpoint": "https://mcp.test/register" }),
            ),
            ("POST", "https://mcp.test/register") => {
                s.n += 1;
                let mut r = json!({ "client_id": format!("c{}", s.n) });
                if let Some(sc) = &s.reg_secret {
                    r["client_secret"] = json!(sc);
                }
                reply(201, &r)
            }
            ("POST", TOKEN) => {
                let f = url_decode(&body);
                let ok = match f["grant_type"].as_str() {
                    "authorization_code" => true,
                    "refresh_token" => s.refresh_ok && s.refresh.as_deref() == f.get("refresh_token").map(String::as_str),
                    _ => false,
                };
                if !ok {
                    return reply(400, &json!({ "error": "invalid_grant" }));
                }
                s.n += 1;
                let (a, r) = (format!("a{}", s.n), format!("r{}", s.n));
                s.valid.insert(a.clone());
                s.refresh = Some(r.clone());
                reply(200, &json!({ "access_token": a, "refresh_token": r, "expires_in": 3600, "token_type": "Bearer" }))
            }
            ("POST", MCP) => {
                let token = auth.as_deref().and_then(|a| a.strip_prefix("Bearer "));
                if s.reject_all || !token.is_some_and(|t| s.valid.contains(t)) {
                    return reply(401, &json!({ "error": "unauthorized" }));
                }
                let m: Value = serde_json::from_str(&body).unwrap_or_default();
                let id = m["id"].clone();
                let result = match m["method"].as_str() {
                    Some("initialize") => json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "serverInfo": { "name": "fake" } }),
                    Some("tools/list") => json!({ "tools": [{ "name": "echo" }] }),
                    Some("tools/call") => json!({ "content": [{ "type": "text", "text": "hi" }] }),
                    _ => return NetResponse { status: 202, ..NetResponse::default() },
                };
                let mut r = reply(200, &json!({ "jsonrpc": "2.0", "id": id, "result": result }));
                r.headers.push(("mcp-session-id".into(), "s1".into()));
                r
            }
            _ => reply(404, &json!({})),
        }
    }
}
impl wisp_kernel::services::Net for Server {
    fn fetch(&self, req: NetRequest) -> BoxFuture<'static, Result<NetResponse>> {
        let r = self.answer(&req);
        Box::pin(async move { Ok(r) })
    }
}

fn url_decode(body: &str) -> BTreeMap<String, String> {
    let dec = |s: &str| {
        let s = s.replace('+', " ");
        let mut out = Vec::new();
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' && i + 2 < b.len() {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).unwrap()
    };
    body.split('&').filter_map(|p| p.split_once('=')).map(|(k, v)| (dec(k), dec(v))).collect()
}

struct App {
    k: Kernel,
    secrets: Arc<Secrets>,
    server: Arc<Server>,
    clock: FakeClock,
    dir: PathBuf,
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn kernel(dir: &std::path::Path, secrets: &Arc<Secrets>, server: &Arc<Server>, clock: &FakeClock) -> Kernel {
    let store = Store::open(Location::Path(dir.join("wisp.db")), Arc::new(SystemClock)).unwrap();
    let (sec, net): (wisp_kernel::services::SecretsService, wisp_kernel::services::NetService) = (secrets.clone(), server.clone());
    TestApp::new().clock(clock).service(store).service(sec).service(net).with(wisp_connectors::ConnectorsPlugin).start().await
}

async fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("wisp-connectors-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (secrets, server, clock) = (Arc::new(Secrets::default()), Arc::new(Server::default()), FakeClock::at(1_000_000_000));
    let k = kernel(&dir, &secrets, &server, &clock).await;
    App { k, secrets, server, clock, dir }
}

async fn eventually(what: &str, f: impl AsyncFn() -> bool) {
    for _ in 0..400 {
        if f().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("never: {what}");
}

impl App {
    async fn call(&self, id: &str, args: Value) -> Result<Value> {
        self.k.call(Caller::user(), id, args).await
    }
    async fn ok(&self, id: &str, args: Value) -> Value {
        self.call(id, args).await.unwrap_or_else(|e| panic!("{id}: {}", e.message))
    }
    async fn row(&self) -> Value {
        let v = self.ok("connectors.list", json!({})).await;
        v["connectors"].as_array().unwrap().iter().find(|c| c["id"] == "fake").cloned().unwrap_or_default()
    }
    /// The sign-in page's URL for the connector, from `origin`; → its `state`.
    async fn start_signin(&self, origin: &str) -> String {
        let u = self.ok("connectors.signin", json!({ "id": "fake", "origin": origin })).await;
        let url = u["url"].as_str().unwrap().to_owned();
        url_decode(url.split_once('?').unwrap().1)["state"].clone()
    }
    async fn finish(&self, state: &str) {
        self.ok("connectors.oauthFinish", json!({ "query": { "state": state, "code": "the-code" } })).await;
    }
    /// The connector added (its first check asks for a sign-in), then signed in: `a<n>` works for an hour.
    async fn signed_in(&self) {
        self.ok("connectors.add", json!({ "name": "fake", "transport": "remote", "url": MCP })).await;
        eventually("the first check is over", async || {
            let r = self.row().await;
            r["test"].is_object() && r["testing"] == false
        })
        .await;
        let state = self.start_signin(HERE).await;
        self.finish(&state).await;
        let r = self.row().await;
        assert_eq!(r["test"]["ok"], true, "signed in, its check lists its tools: {r}");
        self.server.clear();
    }
    async fn tool(&self) -> Result<Value> {
        self.call("connectors.call", json!({ "id": "fake", "tool": "echo", "args": {} })).await
    }
    fn token(&self) -> Option<String> {
        self.secrets.get("connector:fake:token")
    }
    /// The bearer a new run of any task gets for the connector.
    fn run_bearer(&self) -> Option<String> {
        let q = McpQuery {
            mode: wisp_runtime_api::model::Mode::default(),
            harness: "claude".into(),
            team: Vec::new(),
            turn_id: String::new(),
            sub: false,
        };
        let s = Session::default();
        self.k
            .contributions(&RUN)
            .iter()
            .find_map(|r| r.mcp_servers(&s, &q).get("fake").cloned())
            .and_then(|v| v["headers"]["Authorization"].as_str().map(str::to_owned))
    }
}

/// doc29a (1): a token that ran out, with a refresh token stored, is refreshed before the call: the refresh form goes
/// to the token endpoint, the new token and the rotated refresh token are stored, and the call carries the new bearer.
#[tokio::test(flavor = "multi_thread")]
async fn a_token_that_ran_out_is_refreshed_before_the_call() {
    let a = app("ran-out").await;
    a.signed_in().await;
    let (old, old_refresh) = (a.token().unwrap(), a.secrets.get("connector:fake:refresh").unwrap());
    a.clock.advance(3_600_000 + 1_000);
    let out = a.tool().await.unwrap_or_else(|e| panic!("the call: {}", e.message));
    assert_eq!(out["result"]["content"][0]["text"], "hi");
    let forms = a.server.token_forms();
    let refresh = forms.iter().find(|f| f["grant_type"] == "refresh_token").expect("a refresh");
    assert_eq!(refresh["refresh_token"], old_refresh);
    assert_eq!(a.server.refreshes(), 1);
    let new = a.token().unwrap();
    assert_ne!(new, old);
    assert_eq!(a.secrets.get("connector:fake:refresh"), a.server.with(|s| s.refresh.clone()), "the rotated refresh token");
    assert_eq!(a.server.bearers("tools/call"), vec![Some(format!("Bearer {new}"))]);
}

/// doc29a (2): a token the server refuses before its end is refreshed and the call tried once more; a refusal that
/// outlives the refresh isn't tried again.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_token_is_refreshed_and_tried_once_more() {
    let a = app("refused").await;
    a.signed_in().await;
    let old = a.token().unwrap();
    a.server.with(|s| s.valid.clear());
    a.tool().await.unwrap_or_else(|e| panic!("the retry: {}", e.message));
    let new = a.token().unwrap();
    assert_eq!(a.server.refreshes(), 1);
    assert_eq!(a.server.bearers("initialize"), vec![Some(format!("Bearer {old}")), Some(format!("Bearer {new}"))]);
    assert_eq!(a.server.bearers("tools/call"), vec![Some(format!("Bearer {new}"))]);

    a.server.clear();
    a.server.with(|s| s.reject_all = true);
    let e = a.tool().await.expect_err("refused twice");
    assert_eq!(e.code, wisp_kernel::Code::Forbidden, "{}", e.message);
    assert_eq!((a.server.bearers("initialize").len(), a.server.refreshes()), (2, 1), "one refresh, one retry, no loop");
}

/// doc29a (3): two calls on a token that ran out make one refresh: the second waits and uses the new token.
#[tokio::test(flavor = "multi_thread")]
async fn two_calls_on_a_token_that_ran_out_refresh_once() {
    let a = app("twice").await;
    a.signed_in().await;
    a.clock.advance(3_600_000 + 1_000);
    let (x, y) = tokio::join!(a.tool(), a.tool());
    x.unwrap();
    y.unwrap();
    assert_eq!(a.server.refreshes(), 1);
    let new = a.token().unwrap();
    assert!(a.server.bearers("tools/call").iter().all(|b| b.as_deref() == Some(&format!("Bearer {new}"))));
}

/// doc29a (4) and `connectors.token`: after a refresh a new run gets the new bearer; the proxy's token is the
/// stored one while it's fresh, and a forced one is refreshed.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_run_gets_the_refreshed_bearer() {
    let a = app("runs").await;
    a.signed_in().await;
    let old = a.token().unwrap();
    assert_eq!(a.run_bearer(), Some(format!("Bearer {old}")));
    assert_eq!(a.ok("connectors.token", json!({ "id": "fake" })).await["token"], old);
    let forced = a.ok("connectors.token", json!({ "id": "fake", "force": true })).await;
    let new = a.token().unwrap();
    assert_ne!(new, old);
    assert_eq!(forced["token"], new);
    assert_eq!(a.run_bearer(), Some(format!("Bearer {new}")));
}

/// doc29a: the refresh runs ahead of the token's end by itself, on the plugin's clock.
#[tokio::test(flavor = "multi_thread")]
async fn a_token_is_refreshed_ahead_of_its_end() {
    let a = app("ahead").await;
    a.signed_in().await;
    let old = a.token().unwrap();
    eventually("a refresh is armed", async || a.clock.sleepers() > 0).await;
    a.clock.advance(3_600_000 - 60_000);
    eventually("the token was refreshed", async || a.token().is_some_and(|t| t != old)).await;
    assert_eq!(a.server.refreshes(), 1);
}

/// doc29b: a call refused after its refresh failed leaves the row asking for a sign-in, kept across a restart.
#[tokio::test(flavor = "multi_thread")]
async fn a_sign_in_that_cant_be_renewed_marks_the_row_and_outlives_a_restart() {
    let a = app("needs-signin").await;
    a.signed_in().await;
    a.server.with(|s| {
        s.reject_all = true;
        s.refresh_ok = false;
    });
    let e = a.tool().await.expect_err("refused");
    assert!(e.message.contains("needs sign-in"), "{}", e.message);
    let r = a.row().await;
    assert_eq!((r["test"]["ok"].clone(), r["test"]["auth"].clone()), (json!(false), json!(true)), "{r}");
    assert!(r["signedIn"].is_object(), "still signed in, the sign-in ran out: {r}");
    assert!(r["test"]["error"].as_str().unwrap().contains("invalid_grant"), "why, not hidden: {r}");

    let again = kernel(&a.dir, &a.secrets, &a.server, &a.clock).await;
    eventually("the restarted pane reads the row back", async || {
        let v = again.call(Caller::user(), "connectors.list", json!({})).await.unwrap();
        v["connectors"][0]["test"]["auth"] == true
    })
    .await;
}

/// new3 (1): a sign-in from a new place registers a new client; one registered without a secret leaves no old secret
/// behind, in the token request or in secrets.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_client_without_a_secret_drops_the_old_one() {
    let a = app("secret").await;
    a.server.with(|s| s.reg_secret = Some("s1".into()));
    a.signed_in().await;
    assert_eq!(a.secrets.get("connector:fake:client_secret").as_deref(), Some("s1"));
    a.server.with(|s| s.reg_secret = None);
    let state = a.start_signin("http://other.test:4800").await;
    a.finish(&state).await;
    let code = a.server.token_forms().into_iter().find(|f| f["grant_type"] == "authorization_code").unwrap();
    assert!(!code.contains_key("client_secret"), "{code:?}");
    assert_eq!(a.secrets.get("connector:fake:client_secret"), None);
}

/// new3 (2): a sign-in started and left in the browser stores nothing: the connector stays signed in as it was.
#[tokio::test(flavor = "multi_thread")]
async fn a_sign_in_left_unfinished_changes_nothing() {
    let a = app("abandoned").await;
    a.server.with(|s| s.reg_secret = Some("s1".into()));
    a.signed_in().await;
    let before = (a.secrets.get("connector:fake:oauth").unwrap(), a.secrets.get("connector:fake:client_secret"), a.token());
    a.server.with(|s| s.reg_secret = Some("s2".into()));
    let _ = a.start_signin("http://other.test:4800").await;
    assert_eq!((a.secrets.get("connector:fake:oauth").unwrap(), a.secrets.get("connector:fake:client_secret"), a.token()), before);
    assert!(a.row().await["signedIn"].is_object());
}

/// new1: signing in again from the same place (the client kept) is signed in now, so the row stops saying
/// "Signing in…" at once.
#[tokio::test(flavor = "multi_thread")]
async fn signing_in_again_with_the_kept_client_is_signed_in_now() {
    let a = app("again").await;
    a.signed_in().await;
    let at = a.row().await["signedIn"]["at"].as_i64().unwrap();
    a.clock.advance(5_000);
    let state = a.start_signin(HERE).await;
    a.finish(&state).await;
    assert_eq!(a.server.with(|s| s.log.iter().filter(|(_, u, _, _)| u.ends_with("/register")).count()), 0, "the client was kept");
    assert_eq!(a.row().await["signedIn"]["at"].as_i64(), Some(at + 5_000));
}
