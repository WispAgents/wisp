//! API keys through the grid's own routes (ADR-0058, v1 test/grid.test.js "API keys"): made with a device, said
//! once, listed without the key or its hash; a key opens its user's own wisp (`/_grid/state` answers without
//! waking the box) and nothing else on the grid; revoked, it's 401. The box itself, with a key, is in
//! `crates/wisp/tests/serve_grid.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::many_single_char_names)]

use std::path::PathBuf;

use serde_json::{Value, json};
use wisp_grid::accounts::Poll;

struct G {
    grid: std::sync::Arc<wisp_grid::edge::Grid>,
    port: u16,
    stop: tokio_util::sync::CancellationToken,
    _dir: PathBuf,
}

async fn grid(name: &str) -> G {
    let dir = std::env::temp_dir().join(format!("wisp-grid-keys-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // no box is started here: /_grid/state answers from hostd's state alone
    let (grid, port, stop) = wisp_grid::edge::dev(&dir, 0, PathBuf::from("/nonexistent/wisp"), Vec::new(), 600_000).await.unwrap();
    G { grid, port, stop, _dir: dir }
}

impl G {
    fn root(&self) -> String {
        format!("http://wisp.localhost:{}", self.port)
    }
    fn at(&self, label: &str) -> String {
        format!("http://{label}.wisp.localhost:{}", self.port)
    }
    fn web(&self, names: &[&str]) -> reqwest::Client {
        let mut b = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none());
        for n in ["wisp.localhost"].iter().chain(names) {
            b = b.resolve(n, ([127, 0, 0, 1], self.port).into());
        }
        b.build().unwrap()
    }
    /// A device token for `handle` (the device flow, approved at once).
    fn device(&self, handle: &str) -> String {
        let a = &self.grid.accounts;
        let u = a.dev_user(handle).unwrap();
        let c = a.device_code("Laptop (wisp)").unwrap();
        a.decide(c["user_code"].as_str().unwrap(), &u.id, true).unwrap();
        match a.poll(c["device_code"].as_str().unwrap()).unwrap() {
            Poll::Token { token, .. } => token,
            other => panic!("{other:?}"),
        }
    }
}

async fn send(r: reqwest::RequestBuilder) -> (u16, Value, String) {
    let r = r.send().await.unwrap();
    let status = r.status().as_u16();
    let cache = r.headers().get("cache-control").map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default();
    let v = r.json::<Value>().await.unwrap_or(Value::Null);
    (status, v, cache)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_key_is_made_once_listed_without_its_secret_and_revoked() {
    let g = grid("make").await;
    let web = g.web(&["kai.wisp.localhost"]);
    let dev = g.device("kai");
    let keys = format!("{}/api/keys", g.root());

    // create_returns_the_key_once
    let (s, made, cache) = send(web.post(&keys).bearer_auth(&dev).json(&json!({ "name": "  Deploy script  " }))).await;
    assert_eq!((s, cache.as_str()), (201, "no-store"), "{made}");
    let key = made["key"].as_str().unwrap().to_owned();
    assert!(key.starts_with("wgk_") && key.len() == 47, "{key}");
    let mut fields: Vec<&str> = made.as_object().unwrap().keys().map(String::as_str).collect();
    fields.sort_unstable();
    assert_eq!(fields, ["createdAt", "hint", "id", "key", "lastUsedAt", "name"]);
    assert_eq!(
        (made["name"].clone(), made["hint"].clone(), made["lastUsedAt"].clone()),
        (json!("Deploy script"), json!(&key[43..]), Value::Null)
    );
    send(web.post(&keys).bearer_auth(&dev).json(&json!({ "name": "Newer" }))).await;

    // lists_never_carry_the_key_or_its_hash
    let r = web.get(&keys).bearer_auth(&dev).send().await.unwrap();
    assert_eq!(r.headers()["cache-control"], "no-store");
    let text = r.text().await.unwrap();
    let list: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        list["keys"].as_array().unwrap().iter().map(|k| k["name"].as_str().unwrap()).collect::<Vec<_>>(),
        ["Newer", "Deploy script"]
    );
    assert_eq!(list["address"], json!(g.at("kai")));
    let hash = wisp_grid::secrets::sha256(&key);
    assert!(!text.contains(&key[4..]) && !text.contains(&hash) && !text.contains("token_hash") && !text.contains("wgk_"), "{text}");
    // only the hash is at rest, and the audit has its id alone
    let at_rest: String =
        g.grid.db.with(|c| c.query_row("SELECT group_concat(name || hint || token_hash, '|') FROM api_keys", [], |r| r.get(0))).unwrap();
    let audit: String = g.grid.db.with(|c| c.query_row("SELECT group_concat(action || meta, '|') FROM audit", [], |r| r.get(0))).unwrap();
    assert!(at_rest.contains(&hash) && !at_rest.contains(&key[4..]) && !audit.contains(&key[4..]) && !audit.contains("Deploy"), "{audit}");

    // a_name_is_needed_and_60_characters_at_most
    for name in [json!(""), json!("   "), json!("x".repeat(61)), Value::Null, json!(7)] {
        let (s, v, _) = send(web.post(&keys).bearer_auth(&dev).json(&json!({ "name": name }))).await;
        assert_eq!(s, 400, "{name}: {v}");
        assert!(v["error"].as_str().unwrap().contains("name"), "{v}");
    }

    // revoke: another user's id is a 404 and leaves it; then it's gone, once
    let id = made["id"].as_str().unwrap();
    let other = g.device("kay");
    assert_eq!(send(web.delete(format!("{keys}/{id}")).bearer_auth(&other)).await.0, 404);
    assert_eq!(send(web.delete(format!("{keys}/abcdef012345")).bearer_auth(&dev)).await.0, 404);
    assert_eq!(send(web.get(format!("{}/_grid/state", g.at("kai"))).bearer_auth(&key)).await.0, 200, "still works");
    let (s, v, cache) = send(web.delete(format!("{keys}/{id}")).bearer_auth(&dev)).await;
    assert_eq!((s, v, cache.as_str()), (200, json!({ "ok": true }), "no-store"));
    assert_eq!(send(web.delete(format!("{keys}/{id}")).bearer_auth(&dev)).await.0, 404, "revoked once");
    let (_, list, _) = send(web.get(&keys).bearer_auth(&dev)).await;
    assert_eq!(list["keys"].as_array().unwrap().len(), 1, "a revoked key isn't listed");
    g.stop.cancel();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_key_opens_its_own_wisp_only_and_is_401_once_revoked() {
    let g = grid("reach").await;
    let dev = g.device("kor");
    let web = g.web(&["kor.wisp.localhost", "desk--kor.wisp.localhost", "kim.wisp.localhost"]);
    let keys = format!("{}/api/keys", g.root());
    let (_, made, _) = send(web.post(&keys).bearer_auth(&dev).json(&json!({ "name": "ci" }))).await;
    let key = made["key"].as_str().unwrap().to_owned();
    let state = format!("{}/_grid/state", g.at("kor"));

    // a_key_reaches_its_own_wisp: no cookie, no device, the key alone; and it's marked used
    let (s, v, _) = send(web.get(&state).bearer_auth(&key)).await;
    assert_eq!((s, v["state"].as_str()), (200, Some("stopped")), "{v}");
    assert!(g.grid.keys.list(&g.grid.accounts.user_by_handle("kor").unwrap().unwrap().id).unwrap()[0].last_used_at.is_some());
    // none, a typo, or the key without Bearer's word is nobody; never from the query string
    for auth in [None, Some("Bearer wgk_typo".to_owned()), Some(format!("Bearer {}", &key[..key.len() - 1]))] {
        let mut r = web.get(&state);
        if let Some(a) = &auth {
            r = r.header("authorization", a);
        }
        assert_eq!(send(r).await.0, 401, "{auth:?}");
    }
    assert_eq!(send(web.get(format!("{state}?key={key}"))).await.0, 401);

    // another_users_wisp_refuses_it
    g.grid.accounts.dev_user("kim").unwrap();
    assert_eq!(send(web.get(format!("{}/_grid/state", g.at("kim"))).bearer_auth(&key)).await.0, 403);

    // a_key_never_reaches_a_computer: a key is nobody there (as v1), and the trust table denies it anyway
    let (u, d) = g.grid.accounts.device(&dev).unwrap().unwrap();
    g.grid.accounts.add_computer(&u, &d, "desk").unwrap();
    for p in ["/api/health", "/_grid/state"] {
        assert_eq!(send(web.get(format!("{}{p}", g.at("desk--kor"))).bearer_auth(&key)).await.0, 401, "{p}");
    }
    assert!(!wisp_grid::trust::may(wisp_grid::trust::Credential::Key, wisp_grid::trust::Reach::Computer).ok());

    // a_key_reaches_none_of_the_grids_own_api, and can't manage keys
    for (m, p) in
        [("GET", "/api/me"), ("GET", "/api/computers"), ("POST", "/api/computers"), ("GET", "/api/sandboxes"), ("GET", "/api/teams")]
    {
        let r = web.request(m.parse().unwrap(), format!("{}{p}", g.root())).bearer_auth(&key).json(&json!({}));
        assert_eq!(send(r).await.0, 401, "{m} {p}");
    }
    for (m, p) in [
        ("GET", "/api/keys".to_owned()),
        ("POST", "/api/keys".to_owned()),
        ("DELETE", format!("/api/keys/{}", made["id"].as_str().unwrap())),
    ] {
        let (s, v, _) =
            send(web.request(m.parse().unwrap(), format!("{}{p}", g.root())).bearer_auth(&key).json(&json!({ "name": "second" }))).await;
        assert_eq!((s, v["error"].as_str()), (403, Some("a key can’t manage keys")), "{m} {p}");
    }
    assert_eq!(g.grid.keys.list(&u.id).unwrap().len(), 1, "nothing made or revoked by a key");

    // the box's secret and the grid's own cookie make no key either; nobody is 401
    let (s, v, _) = send(web.post(&keys).header("authorization", "Box kor nope").json(&json!({ "name": "x" }))).await;
    assert_eq!(s, 401, "{v}");
    let session = g.grid.accounts.new_session(&u.id).unwrap();
    let (s, _, _) =
        send(web.post(&keys).header("cookie", format!("{}={session}", wisp_grid::edge::COOKIE)).json(&json!({ "name": "x" }))).await;
    assert_eq!(s, 403, "the grid's pages don't make keys: wisp on a computer does");
    assert_eq!(send(web.get(&keys)).await.0, 401);

    // revoked: the same call is 401
    send(web.delete(format!("{keys}/{}", made["id"].as_str().unwrap())).bearer_auth(&dev)).await;
    assert_eq!(send(web.get(&state).bearer_auth(&key)).await.0, 401);
    g.stop.cancel();
}

#[tokio::test(flavor = "multi_thread")]
async fn making_keys_is_rate_limited_per_user() {
    let g = grid("limit").await;
    let web = g.web(&[]);
    let (dev, other) = (g.device("kip"), g.device("kit"));
    let keys = format!("{}/api/keys", g.root());
    let mut made = 0;
    for i in 0..wisp_grid::keys::CREATES_PER_MINUTE {
        let (s, v, _) = send(web.post(&keys).bearer_auth(&dev).json(&json!({ "name": format!("k{i}") }))).await;
        // 20 live, then 409 for the rest of the minute's 30
        assert!(s == 201 || (s == 409 && v["error"].as_str().unwrap().contains("Revoke one first")), "{s} {v}");
        made += usize::from(s == 201);
    }
    assert_eq!(made, 20);
    let (s, v, _) = send(web.post(&keys).bearer_auth(&dev).json(&json!({ "name": "flood" }))).await;
    assert_eq!((s, v["error"].as_str()), (429, Some("Too many requests. Wait a minute.")));
    // per user: someone else isn't held back
    assert_eq!(send(web.post(&keys).bearer_auth(&other).json(&json!({ "name": "mine" }))).await.0, 201);
    g.stop.cancel();
}
