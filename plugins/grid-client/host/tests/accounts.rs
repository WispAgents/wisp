//! Several grid accounts, one current (ADR-0061, F-mach-21): the one-account shape of an older wisp moves over
//! with its token; signing in to two grids (and twice to one, as two people) lists each, each with its own token
//! in secrets and its own cloud box and computers in wisp.machines; switching and signing out of one touch only it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::many_single_char_names)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use wisp_grid_client::GridClientPlugin;
use wisp_kernel::services::{Paths, SecretsService};
use wisp_kernel::{BoxFuture, Caller, Code, Kernel, Result};
use wisp_machines::MachinesPlugin;
use wisp_store::{Col, EntityDef, Location, Migration, Row, Store, SystemClock};
use wisp_testkit::TestApp;

#[derive(Default)]
struct Secrets(Mutex<BTreeMap<String, String>>);
impl Secrets {
    fn all(&self) -> BTreeMap<String, String> {
        self.0.lock().unwrap().clone()
    }
    fn grid_tokens(&self) -> BTreeMap<String, String> {
        self.all().into_iter().filter(|(k, _)| k.starts_with("grid-client/")).collect()
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
        let v = self.0.lock().unwrap().keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        Box::pin(async move { Ok(v) })
    }
}

/// A grid as far as signing in goes: the device flow approves at once as `who` (the person signed in to the
/// grid's page in the browser), and `/api/computers` lists one computer of that token's person.
struct FakeGrid {
    url: String,
    who: Arc<Mutex<String>>,
}

async fn fake_grid(n: u8) -> FakeGrid {
    let who = Arc::new(Mutex::new(String::from("ada")));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let (kept, base) = (who.clone(), url.clone());
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = l.accept().await else { return };
            let (who, base) = (kept.clone(), base.clone());
            tokio::spawn(async move {
                let svc = hyper::service::service_fn(move |req: http::Request<hyper::body::Incoming>| {
                    let (who, base) = (who.clone(), base.clone());
                    async move {
                        let (m, p) = (req.method().to_string(), req.uri().path().to_owned());
                        let auth = req.headers().get("authorization").map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default();
                        let _ = req.into_body().collect().await;
                        let (status, v) = match (m.as_str(), p.as_str()) {
                            ("POST", "/api/device/code") => (
                                200,
                                json!({ "device_code": "dc", "user_code": "ABCD-EFGH", "verification_uri": format!("{base}/activate"), "interval": 1, "expires_in": 600 }),
                            ),
                            ("POST", "/api/device/token") => {
                                let h = who.lock().unwrap().clone();
                                (
                                    200,
                                    json!({ "token": format!("wgd_g{n}_{h}"), "user": { "handle": h }, "instance": { "url": format!("{base}/box/{h}") } }),
                                )
                            }
                            ("GET", "/api/computers") => match auth.strip_prefix(&format!("Bearer wgd_g{n}_")) {
                                Some(h) => (
                                    200,
                                    json!([{ "id": format!("c-{h}"), "url": base, "name": format!("studio of {h} on g{n}"), "label": "studio" }]),
                                ),
                                None => (401, json!({ "error": "who?" })),
                            },
                            _ => (404, json!({ "error": "not found" })),
                        };
                        let mut r = http::Response::new(Full::new(Bytes::from(v.to_string())));
                        *r.status_mut() = http::StatusCode::from_u16(status).unwrap();
                        Ok::<_, std::convert::Infallible>(r)
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(sock), svc).await;
            });
        }
    });
    FakeGrid { url, who }
}

struct App {
    k: Kernel,
    secrets: Arc<Secrets>,
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wisp-grid-accounts-{name}-{}", wisp_kernel::ids::uuid7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

async fn app(dir: PathBuf, secrets: Arc<Secrets>) -> App {
    let store = Store::open(Location::Path(dir.join("wisp.db")), Arc::new(SystemClock)).unwrap();
    let sec: SecretsService = secrets.clone();
    let k = TestApp::new()
        .service(store)
        .service(sec)
        .service(Paths::new(&dir))
        .service(wisp_proc::local())
        .with(MachinesPlugin)
        .with(GridClientPlugin)
        .start()
        .await;
    App { k, secrets }
}

impl App {
    async fn call(&self, id: &str, args: Value) -> Value {
        self.k.call(Caller::user(), id, args).await.unwrap_or_else(|e| panic!("{id}: {}", e.message))
    }
    /// Signs in to `g` as `who`, through the device flow, and waits for it to be done.
    async fn sign_in(&self, g: &FakeGrid, who: &str) {
        who.clone_into(&mut g.who.lock().unwrap());
        let f = self.call("grid-client.signin", json!({ "grid": g.url })).await;
        for _ in 0..100 {
            let s = self.call("grid-client.signinStatus", json!({ "id": f["id"] })).await;
            if s["state"] != "waiting" {
                assert_eq!((s["state"].as_str(), s["handle"].as_str()), (Some("done"), Some(who)), "{s}");
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("the sign-in to {} as {who} never ended", g.url);
    }
    async fn peers(&self) -> Vec<Value> {
        self.call("machines.list", json!({})).await.as_array().cloned().unwrap_or_default()
    }
    /// The peers that came from `grid` as `handle`, by name.
    async fn names_from(&self, grid: &str, handle: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .peers()
            .await
            .iter()
            .filter(|p| p["grid"] == grid && p["handle"] == handle)
            .map(|p| p["name"].as_str().unwrap_or_default().to_owned())
            .collect();
        out.sort();
        out
    }
    async fn eventually_names(&self, grid: &str, handle: &str, want: &[&str]) {
        for _ in 0..100 {
            if self.names_from(grid, handle).await == want {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(self.names_from(grid, handle).await, want, "{grid} as {handle}");
    }
}

fn keys(v: &Value) -> Vec<(String, bool)> {
    v.as_array().unwrap().iter().map(|a| (a["key"].as_str().unwrap().to_owned(), a["current"] == true)).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn signed_in_to_two_grids_and_twice_to_one_each_is_listed_with_its_own_token_and_peers() {
    let (g1, g2) = (fake_grid(1).await, fake_grid(2).await);
    let d = dir("several");
    let a = app(d.clone(), Arc::default()).await;

    a.sign_in(&g1, "ada").await;
    a.sign_in(&g2, "ada").await;
    a.sign_in(&g1, "bea").await;
    let (k1a, k2a, k1b) = (format!("{}#ada", g1.url), format!("{}#ada", g2.url), format!("{}#bea", g1.url));
    let list = a.call("grid-client.accounts", json!({})).await;
    assert_eq!(keys(&list), [(k1a.clone(), false), (k2a.clone(), false), (k1b.clone(), true)], "each listed, the newest current");

    // secrets_are_keyed_per_grid_and_account: three tokens, none in the old one-account place
    let want: BTreeMap<String, String> = [
        (format!("grid-client/token/{k1a}"), "wgd_g1_ada".to_owned()),
        (format!("grid-client/token/{k2a}"), "wgd_g2_ada".to_owned()),
        (format!("grid-client/token/{k1b}"), "wgd_g1_bea".to_owned()),
    ]
    .into();
    assert_eq!(a.secrets.grid_tokens(), want);

    // each account's cloud box and computer, with the grid and handle it came from; the second "Cloud" by its handle
    a.eventually_names(&g1.url, "ada", &["Cloud", "studio of ada on g1"]).await;
    a.eventually_names(&g2.url, "ada", &["Cloud (ada)", "studio of ada on g2"]).await;
    a.eventually_names(&g1.url, "bea", &["Cloud (bea)", "studio of bea on g1"]).await;

    // switch: shares, keys and sandboxes go through the one switched to
    let st = a.call("grid-client.accountSwitch", json!({ "key": k2a })).await;
    assert_eq!((st["grid"].as_str(), st["handle"].as_str()), (Some(g2.url.as_str()), Some("ada")));
    assert_eq!(keys(&st["accounts"]), [(k1a.clone(), false), (k2a.clone(), true), (k1b.clone(), false)]);
    let nope = a.k.call(Caller::user(), "grid-client.accountSwitch", json!({ "key": "https://elsewhere#zed" })).await.unwrap_err();
    assert_eq!(nope.code, Code::NotFound);

    // sign out of one (not current): only its token and peers go; ada's on g1 keep theirs though bea's shared the grid
    let st = a.call("grid-client.accountRemove", json!({ "key": k1b })).await;
    assert_eq!(keys(&st["accounts"]), [(k1a.clone(), false), (k2a.clone(), true)]);
    assert!(!a.secrets.grid_tokens().contains_key(&format!("grid-client/token/{k1b}")));
    a.eventually_names(&g1.url, "bea", &[]).await;
    a.eventually_names(&g1.url, "ada", &["Cloud", "studio of ada on g1"]).await;

    // sign out of the current one: the first left is current
    let st = a.call("grid-client.signout", json!({})).await;
    assert_eq!((st["handle"].as_str(), st["grid"].as_str()), (Some("ada"), Some(g1.url.as_str())));
    assert_eq!(keys(&st["accounts"]), [(k1a.clone(), true)]);
    a.eventually_names(&g2.url, "ada", &[]).await;

    // the_tokens_are_in_no_row: not in the database, its log or the Settings → Cloud row
    let disk: Vec<u8> = std::fs::read_dir(&d).unwrap().flat_map(|e| std::fs::read(e.unwrap().path()).unwrap_or_default()).collect();
    assert!(!disk.windows(4).any(|w| w == b"wgd_"), "a device token reached the store");

    // and it holds across a restart: the accounts, and which is current
    let kept = a.secrets.all();
    drop(a);
    let b = app(d.clone(), Arc::new(Secrets(Mutex::new(kept)))).await;
    let list = b.call("grid-client.accounts", json!({})).await;
    assert_eq!(keys(&list), [(k1a, true)]);
    drop(b);
    let _ = std::fs::remove_dir_all(&d);
}

/// The doc table as an older wisp (one account) made it.
const OLD_DOC: EntityDef = EntityDef {
    kind: "grid_client_doc",
    table: "grid_client_doc",
    key: "id",
    columns: &[("id", Col::Text), ("body", Col::Text)],
    json_columns: &["body"],
    parent: None,
    seq: None,
    topics: |_| vec!["plugin:wisp.grid-client:account".to_owned()],
};

#[tokio::test(flavor = "multi_thread")]
async fn the_one_account_shape_moves_to_several_with_its_token() {
    let d = dir("migrate");
    {
        let old = Store::open(Location::Path(d.join("wisp.db")), Arc::new(SystemClock)).unwrap();
        old.migrate("grid-client", &[Migration::sql(1, "CREATE TABLE grid_client_doc (id TEXT PRIMARY KEY, body TEXT NOT NULL) STRICT;")])
            .unwrap();
        old.define_entity(OLD_DOC).unwrap();
        let mut r = Row::new();
        r.insert("id".into(), json!("account"));
        r.insert(
            "body".into(),
            json!({ "grid": "https://g.test", "handle": "ada", "machine": "m1", "computerId": "c1", "computerOn": false }),
        );
        let mut i = Row::new();
        i.insert("id".into(), json!("import:s1"));
        i.insert("body".into(), json!({ "id": "s1", "grid": "https://g.test", "url": "u", "title": "t", "kind": "setup", "version": 2 }));
        old.write(move |w| {
            w.upsert("grid_client_doc", r)?;
            w.upsert("grid_client_doc", i).map(|_| ())
        })
        .await
        .unwrap();
    }
    let secrets = Arc::new(Secrets::default());
    secrets.0.lock().unwrap().insert("grid-client/token".into(), "wgd_old".into());
    let a = app(d.clone(), secrets).await;

    let st = a.call("grid-client.status", json!({})).await;
    assert_eq!((st["signedIn"].clone(), st["handle"].clone(), st["machine"].clone()), (json!(true), json!("ada"), json!("m1")));
    assert_eq!(keys(&st["accounts"]), [("https://g.test#ada".to_owned(), true)]);
    let want: BTreeMap<String, String> = [("grid-client/token/https://g.test#ada".to_owned(), "wgd_old".to_owned())].into();
    assert_eq!(a.secrets.grid_tokens(), want, "the token moved to its account's place, the old one gone");

    // the docs: one per account, current, the import untouched, no `account`
    let ids: Vec<String> = {
        let s = Store::open(Location::Path(d.join("wisp.db")), Arc::new(SystemClock)).unwrap();
        s.define_entity(OLD_DOC).unwrap();
        let rows = s.read(|r| r.query("grid_client_doc", &wisp_store::Query::new())).unwrap();
        let mut ids: Vec<String> = rows.iter().map(|r| r["id"].as_str().unwrap().to_owned()).collect();
        ids.sort();
        ids
    };
    assert_eq!(ids, ["account:https://g.test#ada", "current", "import:s1"]);
    drop(a);
    let _ = std::fs::remove_dir_all(&d);
}
