//! ADR-0147: a computer's webhook trigger URL, end to end through the grid. wisp on the computer makes it with its device
//! token (`POST /api/hooks`), the URL names that computer and carries a token, and a call to it reaches
//! `POST /api/triggers/<trigger>` through the tunnel marked `x-wisp-route: hook`, with none of the grid's credentials.
//! Nothing else opens a trigger: not its name, not a revoked token, not another computer's token.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::many_single_char_names)]

use std::time::{Duration, Instant};

use base64::Engine as _;
use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper_util::rt::{TokioExecutor, TokioIo};
use serde_json::{Value, json};
use wisp_grid::accounts::Poll;

type Got = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

/// A computer whose wisp answers each request with what it got (method, path, the headers that matter, the body) and
/// records the body in `got`; the first `fail` calls it answers 503.
async fn computer(port: u16, id: &str, token: &str, got: Got, fail: usize) -> tokio::task::JoinHandle<()> {
    let c = reqwest::Client::builder().http1_only().no_proxy().resolve("wisp.localhost", ([127, 0, 0, 1], port).into()).build().unwrap();
    let r = c
        .get(format!("http://wisp.localhost:{port}/api/computers/{id}/tunnel"))
        .header("connection", "Upgrade")
        .header("upgrade", "wisp-tunnel")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 101);
    let up = r.upgrade().await.unwrap();
    let failed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    tokio::spawn(async move {
        let svc = hyper::service::service_fn(move |req: http::Request<hyper::body::Incoming>| {
            let (got, failed) = (got.clone(), failed.clone());
            async move {
                let (p, b) = req.into_parts();
                let body = b.collect().await.map(http_body_util::Collected::to_bytes).unwrap_or_default();
                let h = |k: &str| p.headers.get(k).and_then(|v| v.to_str().ok()).unwrap_or_default().to_owned();
                let text = String::from_utf8_lossy(&body).into_owned();
                let status = if failed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < fail {
                    503
                } else {
                    got.lock().unwrap().push(text.clone());
                    202
                };
                let echo = json!({
                    "method": p.method.as_str(),
                    "path": p.uri.path_and_query().map(http::uri::PathAndQuery::as_str).unwrap_or_default(),
                    "route": h("x-wisp-route"),
                    "cookie": h("cookie"),
                    "authorization": h("authorization"),
                    "signature": h("x-hub-signature-256"),
                    "body": text,
                });
                let mut r = http::Response::new(Full::new(Bytes::from(echo.to_string())));
                *r.status_mut() = http::StatusCode::from_u16(status).unwrap();
                r.headers_mut().insert("content-type", http::HeaderValue::from_static("application/json"));
                Ok::<_, std::convert::Infallible>(r)
            }
        });
        let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new()).serve_connection(TokioIo::new(up), svc).await;
    })
}

async fn until(what: &str, mut ok: impl FnMut() -> bool) {
    let at = Instant::now();
    while !ok() {
        assert!(at.elapsed() < Duration::from_secs(10), "never {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines)]
async fn a_trigger_url_reaches_its_computers_trigger_by_its_token_and_nothing_else_does() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-hooks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // a master key, so the grid can hold calls (sealed) for an offline computer
    let secrets = dir.join("secrets");
    std::fs::create_dir_all(&secrets).unwrap();
    let master = secrets.join("master.json");
    let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
    std::fs::write(&master, json!({ "current": "k1", "keys": { "k1": key } }).to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&master, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let (g, port, stop) = wisp_grid::edge::dev(&dir, 0, "wisp".into(), Vec::new(), 600_000).await.unwrap();
    let ada = g.accounts.dev_user("ada").unwrap();
    let device = |name: &str| {
        let code = g.accounts.device_code(name).unwrap();
        g.accounts.decide(code["user_code"].as_str().unwrap(), &ada.id, true).unwrap();
        let Poll::Token { token, .. } = g.accounts.poll(code["device_code"].as_str().unwrap()).unwrap() else { panic!("no token") };
        let (_, dev) = g.accounts.device(&token).unwrap().unwrap();
        (token, dev)
    };
    let (token, dev) = device("studio");
    let (other_token, other_dev) = device("laptop");
    let studio = g.accounts.add_computer(&ada, &dev, "studio").unwrap();
    let laptop = g.accounts.add_computer(&ada, &other_dev, "laptop").unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve("wisp.localhost", ([127, 0, 0, 1], port).into())
        .resolve(&format!("{}.wisp.localhost", studio.label), ([127, 0, 0, 1], port).into())
        .resolve(&format!("{}.wisp.localhost", laptop.label), ([127, 0, 0, 1], port).into())
        .build()
        .unwrap();
    let api = |p: &str| format!("http://wisp.localhost:{port}{p}");

    // made by wisp on the computer, for that computer: the URL names it and is said once
    let r = client.post(api("/api/hooks")).bearer_auth(&token).json(&json!({ "trigger": "deploy" })).send().await.unwrap();
    assert_eq!(r.status(), 201);
    let made: Value = r.json().await.unwrap();
    let url = made["url"].as_str().unwrap().to_owned();
    let tok = made["token"].as_str().unwrap().to_owned();
    assert_eq!(url, format!("http://{}.wisp.localhost:{port}/_hook/{tok}", studio.label));
    assert!(tok.starts_with("wgh_") && made["hint"].as_str() == Some(&tok[tok.len() - 4..]));
    let listed: Value = client.get(api("/api/hooks")).bearer_auth(&token).send().await.unwrap().json().await.unwrap();
    assert_eq!(listed["hooks"].as_array().unwrap().len(), 1);
    assert!(!listed.to_string().contains(&tok[4..]), "a list never says the token: {listed}");
    let none: Value = client.get(api("/api/hooks")).bearer_auth(&other_token).send().await.unwrap().json().await.unwrap();
    assert_eq!(none["hooks"].as_array().unwrap().len(), 0, "another computer's list is its own");
    // nobody signed in makes one
    assert_eq!(client.post(api("/api/hooks")).json(&json!({ "trigger": "x" })).send().await.unwrap().status(), 401);

    // offline: held (202), in order, a sender's retry of one delivery held once
    for (body, delivery) in [("one", "d1"), ("two", "d2"), ("one again", "d1"), ("three", "d3")] {
        let r = client.post(&url).header("x-github-delivery", delivery).body(body).send().await.unwrap();
        assert_eq!(r.status(), 202, "{body}");
        assert_eq!(r.json::<Value>().await.unwrap()["held"], true);
    }
    assert_eq!(g.held.count(&studio.id).unwrap(), 3);
    let kept: Vec<String> = g.db.with(|c| c.prepare("SELECT sealed FROM hook_held")?.query_map([], |r| r.get(0))?.collect()).unwrap();
    assert!(kept.iter().all(|k| !k.contains("one") && !k.contains("triggers")), "held sealed: {kept:?}");

    // back: the held calls go to it in order; it fails the first try (503), which is tried again
    let got: Got = Got::default();
    let up = computer(port, &studio.id, &token, got.clone(), 1).await;
    until("online", || g.tunnels.online(&studio.id)).await;
    until("delivered", || got.lock().unwrap().len() == 3).await;
    assert_eq!(*got.lock().unwrap(), ["one", "two", "three"]);
    until("emptied", || g.held.count(&studio.id).unwrap() == 0).await;
    got.lock().unwrap().clear();
    // the call: to the trigger, marked, its signature kept, the grid's cookie and a grid bearer dropped
    let r = client
        .post(format!("{url}?secret=s"))
        .header("content-type", "application/json")
        .header("x-hub-signature-256", "sha256=ab")
        .header("cookie", "wisp_grid=abc")
        .header("x-wisp-route", "grid")
        .bearer_auth("wgd_someone")
        .body(r#"{"ref":"main"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    let got: Value = r.json().await.unwrap();
    assert_eq!(got["method"], "POST");
    assert_eq!(got["path"], "/api/triggers/deploy?secret=s");
    assert_eq!(got["route"], "hook");
    assert_eq!(got["signature"], "sha256=ab");
    assert_eq!((got["cookie"].as_str(), got["authorization"].as_str()), (Some(""), Some("")));
    assert_eq!(got["body"], r#"{"ref":"main"}"#);

    // a trigger's name is no URL: the computer's own address still asks for a sign-in
    let r = client.post(format!("http://{}.wisp.localhost:{port}/api/triggers/deploy", studio.label)).body("{}").send().await.unwrap();
    assert!(matches!(r.status().as_u16(), 302 | 401), "{}", r.status());
    // a wrong token, or this token at another computer's address: the same 404
    let wrong = format!("http://{}.wisp.localhost:{port}/_hook/wgh_{}", studio.label, "A".repeat(43));
    assert_eq!(client.post(&wrong).body("{}").send().await.unwrap().status(), 404);
    let elsewhere = format!("http://{}.wisp.localhost:{port}/_hook/{tok}", laptop.label);
    assert_eq!(client.post(&elsewhere).body("{}").send().await.unwrap().status(), 404);
    // over 1 MB: 413 before the tunnel
    assert_eq!(client.post(&url).body(vec![b'x'; 1024 * 1024 + 1]).send().await.unwrap().status(), 413);

    // revoked (by its own computer only): the same 404
    let id = made["id"].as_str().unwrap();
    assert_eq!(client.delete(api(&format!("/api/hooks/{id}"))).bearer_auth(&other_token).send().await.unwrap().status(), 404);
    assert_eq!(client.delete(api(&format!("/api/hooks/{id}"))).bearer_auth(&token).send().await.unwrap().status(), 200);
    assert_eq!(client.post(&url).body("{}").send().await.unwrap().status(), 404);

    // revoking drops what's held for a URL
    up.abort();
    until("offline", || !g.tunnels.online(&studio.id)).await;
    let r = client.post(api("/api/hooks")).bearer_auth(&token).json(&json!({ "trigger": "deploy" })).send().await.unwrap();
    let again: Value = r.json().await.unwrap();
    let url2 = again["url"].as_str().unwrap().to_owned();
    assert_eq!(client.post(&url2).body("held").send().await.unwrap().status(), 202);
    assert_eq!(g.held.count(&studio.id).unwrap(), 1);
    let id2 = again["id"].as_str().unwrap();
    assert_eq!(client.delete(api(&format!("/api/hooks/{id2}"))).bearer_auth(&token).send().await.unwrap().status(), 200);
    assert_eq!(g.held.count(&studio.id).unwrap(), 0);

    stop.cancel();
    let _ = std::fs::remove_dir_all(&dir);
}
