//! `edge`'s tests.
#![allow(clippy::unwrap_used)]
use std::net::IpAddr;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signer as _, SigningKey};
use http::Method;
use http::header::HeaderValue;

use super::*;
use crate::clock::ManualClock;
use crate::shares::{Access, Put, derived};

#[test]
fn queries_decode() {
    assert_eq!(query("a=1&next=%2Factivate%3Fcode%3DX", "next").as_deref(), Some("/activate?code=X"));
    assert_eq!(split("/api/shares/x?ref=v2"), (vec!["api", "shares", "x"], "ref=v2"));
}

/// A grid on an in-memory db and a hand-moved clock, its routes called directly.
fn grid(name: &str) -> (Arc<Grid>, Arc<ManualClock>) {
    let dir = std::env::temp_dir().join(format!("wisp-grid-edge-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = crate::config::for_test(&dir, json!({ "domain": "wisp.localhost", "auth": { "dev": true } })).unwrap();
    let clock = ManualClock::new(1_000_000);
    let shared: crate::clock::SharedClock = clock.clone();
    let url_cfg = cfg.clone();
    let boxes = ProcessHost::new(
        crate::hostd::ProcessOptions {
            bin: "/nonexistent/wisp".into(),
            data: dir,
            idle_ms: 60_000,
            check_ms: 1000,
            start_ms: 2_000,
            max_running: 1,
            env: Vec::new(),
            url_of: Arc::new(move |h| url_cfg.instance_url(h)),
            boxd: None,
            previews: false,
            max_running_per_user: 4,
            wake_retry_ms: 30_000,
            queue_ms: 120_000,
        },
        shared.clone(),
    );
    (Grid::new(cfg, Arc::new(Db::memory().unwrap()), shared, boxes), clock)
}

/// ada's share of `bodies` (one version each), behind `password`.
fn locked_share(g: &Grid, password: &str, bodies: &[&str]) -> (String, UserRow) {
    let ada = g.accounts.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[3; 32]);
    let mut id = String::new();
    for b in bodies {
        let body = json!({ "markdown": b }).to_string();
        let p = Put {
            kind: "doc".into(),
            key: "notes".into(),
            title: "Notes".into(),
            body: B64.encode(&body),
            signer: B64.encode(k.verifying_key().to_bytes()),
            sig: B64.encode(k.sign(body.as_bytes()).to_bytes()),
            access: Some(Access { kind: "password".into(), password: Some(password.into()), ..Access::default() }),
            ..Put::default()
        };
        id = g.shares.put(&ada, &p).unwrap()["id"].as_str().unwrap().to_owned();
    }
    (id, ada)
}

fn ip(s: &str) -> Option<IpAddr> {
    s.parse().ok()
}

async fn text(r: Resp) -> (u16, String) {
    let status = r.status().as_u16();
    (status, String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

/// The password form on `/s/<id>?<q>`, from `from`.
async fn unlock(g: &Grid, id: &str, q: &str, password: &str, from: &str) -> (u16, String) {
    let mut h = HeaderMap::new();
    h.insert("content-type", HeaderValue::from_static("application/x-www-form-urlencoded"));
    let body = format!("password={}", crate::http::encode_component(password));
    text(g.shares_route(&Method::POST, &["s", id], q, &h, ip(from), body.as_bytes(), &Who::Nobody).await.unwrap()).await
}

/// `GET /api/shares/<id>` with `x-wisp-share-password`.
async fn api_read(g: &Grid, id: &str, password: &str, from: &str) -> Resp {
    let mut h = HeaderMap::new();
    h.insert("x-wisp-share-password", HeaderValue::from_str(password).unwrap());
    g.shares_route(&Method::GET, &["api", "shares", id], "", &h, ip(from), b"", &Who::Nobody).await.unwrap()
}

/// Nothing counted guesses at a share's password, and each ran scrypt on an async worker: a script could try
/// passwords as fast as the grid hashed them, and stall every other request while it did.
#[tokio::test(flavor = "multi_thread")]
async fn share_password_guesses_are_rate_limited() {
    let (g, clock) = grid("guesses");
    let (id, ada) = locked_share(&g, "right-horse", &["the-unlocked-note"]);
    let guess = "wrong-guess-rate-limited";
    for _ in 0..10 {
        let (status, page) = unlock(&g, &id, "", guess, "203.0.113.9").await;
        assert!(status == 403 && page.contains("That password isn’t right."), "{status}");
    }
    let (status, page) = unlock(&g, &id, "", guess, "203.0.113.9").await;
    assert_eq!(status, 429);
    assert!(page.contains("Too many tries. Wait a minute and try again.") && !page.contains("isn’t right"), "{page}");
    assert_eq!(derived(guess), 10, "the 11th guess hashed nothing");
    // the right password from there too, and the API: refused, unhashed
    assert_eq!(unlock(&g, &id, "", "right-horse", "203.0.113.9").await.0, 429);
    let (status, body) = text(api_read(&g, &id, guess, "203.0.113.9").await).await;
    assert!(status == 429 && body.contains("Too many tries"), "{body}");
    assert_eq!(derived(guess), 10);
    // another address gets the usual wrong-password page, until the share's own cap (60 a minute, all addresses)
    assert_eq!(unlock(&g, &id, "", guess, "198.51.100.1").await.0, 403);
    for n in 0..49 {
        assert_eq!(unlock(&g, &id, "", guess, &format!("10.0.{n}.1")).await.0, 403);
    }
    assert_eq!(unlock(&g, &id, "", "right-horse", "192.0.2.77").await.0, 429);
    // the sharer reads it whatever the guesses
    let mine =
        g.shares_route(&Method::GET, &["api", "shares", &id], "", &HeaderMap::new(), ip("203.0.113.9"), b"", &Who::Session(ada)).await;
    assert_eq!(mine.unwrap().status(), 200);
    // a minute on, the right password opens it
    clock.advance(crate::http::LIMIT_WINDOW);
    let (status, page) = unlock(&g, &id, "", "right-horse", "203.0.113.9").await;
    assert!(status == 200 && page.contains("the-unlocked-note"), "{status}");
}

#[tokio::test(flavor = "multi_thread")]
async fn overlong_share_password_is_refused_without_hashing() {
    let (g, _) = grid("overlong");
    let (id, _) = locked_share(&g, "right-horse", &["hi"]);
    let long = "overlong-guess-at-the-edge".repeat(6);
    assert!(long.len() > crate::shares::PASSWORD_MAX);
    let (status, page) = unlock(&g, &id, "", &long, "203.0.113.9").await;
    assert!(status == 403 && page.contains("That password isn’t right."), "{status}");
    assert_eq!(api_read(&g, &id, &long, "203.0.113.9").await.status(), 403);
    assert_eq!(derived(&long), 0);
}

/// With every scrypt permit taken a password read waits for none: 503 with `Retry-After`, and a new password too.
#[tokio::test(flavor = "multi_thread")]
async fn hashing_waits_on_the_blocking_pool() {
    let (g, _) = grid("busy");
    let (id, _) = locked_share(&g, "right-horse", &["hi"]);
    let all = g.scrypt.clone().try_acquire_many_owned(4).unwrap();
    let r = api_read(&g, &id, "right-horse", "203.0.113.9").await;
    assert_eq!((r.status().as_u16(), r.headers()["retry-after"].to_str().unwrap()), (503, "1"));
    let r = unlock(&g, &id, "", "right-horse", "203.0.113.9").await;
    assert_eq!(r.0, 503);
    drop(all);
    assert_eq!(api_read(&g, &id, "right-horse", "203.0.113.9").await.status(), 200);
}

/// The password form posted to `/s/<id>` with no query, so unlocking `/s/<id>?ref=v1` opened the latest version.
#[tokio::test(flavor = "multi_thread")]
async fn unlocking_a_versioned_link_opens_that_version() {
    let (g, _) = grid("ref");
    let (id, _) = locked_share(&g, "right-horse", &["the first draft", "the second draft"]);
    let (status, page) =
        text(g.shares_route(&Method::GET, &["s", &id], "ref=v1", &HeaderMap::new(), None, b"", &Who::Nobody).await.unwrap()).await;
    assert!(status == 403 && page.contains(&format!(r#"action="/s/{id}?ref=v1""#)), "{page}");
    let (status, page) = unlock(&g, &id, "ref=v1", "right-horse", "203.0.113.9").await;
    assert!(status == 200 && page.contains("the first draft") && !page.contains("the second draft"), "{page}");
}

/// `POST /api/device/code` from `from`.
async fn new_code(g: &Grid, from: &str) -> u16 {
    let mut h = HeaderMap::new();
    h.insert("content-type", HeaderValue::from_static("application/json"));
    match g
        .account_routes(&Method::POST, &["api", "device", "code"], "", &h, ip(from), br#"{"name":"laptop"}"#, &Who::Nobody)
        .await
        .unwrap()
    {
        Ok(r) => r.status().as_u16(),
        Err(e) => e.status(),
    }
}

/// Anyone could ask for device codes as fast as they liked: 10 a minute from one address now.
#[tokio::test(flavor = "multi_thread")]
async fn device_code_is_rate_limited_per_ip() {
    let (g, _) = grid("devcode");
    for _ in 0..10 {
        assert_eq!(new_code(&g, "203.0.113.9").await, 200);
    }
    assert_eq!(new_code(&g, "203.0.113.9").await, 429);
    assert_eq!(new_code(&g, "198.51.100.7").await, 200);
}

/// `/activate?code=…` could be walked through every code: 30 look-ups a minute, per person or per address.
#[tokio::test(flavor = "multi_thread")]
async fn activate_lookups_are_rate_limited() {
    let (g, _) = grid("activate");
    let look = |from: &'static str| {
        let g = g.clone();
        async move {
            match g
                .account_routes(&Method::GET, &["activate"], "code=BCDF-GHJK", &HeaderMap::new(), ip(from), b"", &Who::Nobody)
                .await
                .unwrap()
            {
                Ok(r) => r.status().as_u16(),
                Err(e) => e.status(),
            }
        }
    };
    for _ in 0..30 {
        assert_eq!(look("203.0.113.9").await, 401);
    }
    assert_eq!(look("203.0.113.9").await, 429);
    assert_eq!(look("198.51.100.7").await, 401);
}
