//! The hosted grid's sign-up funnel (v1 grid/telemetry.js, lost in the v2 port: nothing from the grid reached
//! `PostHog` after 2026-11-21). Its sign-in page loads `PostHog`; the server's steps go as that page's anonymous
//! visitor: off to the provider, back signed up or in, or turned away and why; sign-ups and sign-ins as hashed users.
//! A grid that isn't the hosted one, or has telemetry off, loads no script and counts no step.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use wisp_grid::telemetry::{GridTelemetry, HOSTED, KEY};

/// A tiny HTTP server: `answer(path, body)` → JSON; every request's path and body are kept in `seen`.
async fn fake(seen: Arc<Mutex<Vec<(String, String)>>>, answer: fn(&str, &str) -> Value) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = l.accept().await else { return };
            let seen = seen.clone();
            tokio::spawn(async move {
                let svc = hyper::service::service_fn(move |req: http::Request<hyper::body::Incoming>| {
                    let seen = seen.clone();
                    async move {
                        let path = req.uri().path().to_owned();
                        let body = req
                            .into_body()
                            .collect()
                            .await
                            .map(|b| String::from_utf8_lossy(&b.to_bytes()).into_owned())
                            .unwrap_or_default();
                        let v = answer(&path, &body);
                        seen.lock().unwrap().push((path, body));
                        Ok::<_, std::convert::Infallible>(http::Response::new(Full::new(Bytes::from(v.to_string()))))
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(sock), svc).await;
            });
        }
    });
    port
}

fn github(path: &str, body: &str) -> Value {
    match path {
        "/login/oauth/access_token" if body.contains("code_verifier") => json!({ "access_token": "gho_test" }),
        "/user" => json!({ "id": 4242, "login": "ada", "name": "Ada" }),
        "/user/emails" => json!([{ "email": "ada@example.com", "primary": true, "verified": true }]),
        _ => json!({ "message": "Not Found" }),
    }
}

struct Run {
    grid: Arc<wisp_grid::edge::Grid>,
    c: reqwest::Client,
    root: String,
    stop: CancellationToken,
    posthog: Arc<Mutex<Vec<(String, String)>>>,
}

async fn grid(domain: &str, telemetry: Option<bool>, tag: &str) -> Run {
    let gh = fake(Arc::new(Mutex::new(Vec::new())), github).await;
    let posthog = Arc::new(Mutex::new(Vec::new()));
    let ph = fake(posthog.clone(), |_, _| json!({ "status": 1 })).await;
    let dir = std::env::temp_dir().join(format!("wisp-grid-funnel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{gh}");
    let cfg = wisp_grid::config::for_test(&dir, json!({ "domain": domain, "listen": { "host": "127.0.0.1", "port": port }, "auth": { "providers": { "github": { "clientId": "cid", "clientSecret": "cs", "authBase": base, "apiBase": base } } } })).unwrap();
    let clock = wisp_grid::clock::system();
    let db = Arc::new(wisp_grid::db::Db::open(&dir, clock.now_ms()).unwrap());
    let url_cfg = cfg.clone();
    let boxes = wisp_grid::hostd::ProcessHost::new(
        wisp_grid::hostd::ProcessOptions {
            bin: "wisp".into(),
            data: dir.clone(),
            idle_ms: 60_000,
            check_ms: 1000,
            start_ms: 30_000,
            max_running: 1,
            max_running_per_user: 4,
            wake_retry_ms: 30_000,
            queue_ms: 120_000,
            env: Vec::new(),
            url_of: Arc::new(move |h| url_cfg.instance_url(h)),
            boxd: None,
            previews: false,
        },
        clock.clone(),
    );
    let env = [("WISP_POSTHOG_HOST".to_owned(), format!("http://127.0.0.1:{ph}"))].into_iter().collect();
    let t = GridTelemetry::new(telemetry, domain, &env);
    let grid = wisp_grid::edge::Grid::with_telemetry(cfg, db, clock, boxes, t);
    let stop = CancellationToken::new();
    tokio::spawn(wisp_grid::edge::serve(grid.clone(), listener, stop.clone()));
    let c = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(domain, ([127, 0, 0, 1], port).into())
        .build()
        .unwrap();
    Run { grid, c, root: format!("http://{domain}:{port}"), stop, posthog }
}

/// The page's own `PostHog` cookie, as posthog-js writes it (URL-encoded JSON).
fn visitor_cookie(id: &str) -> String {
    format!(
        "ph_{KEY}_posthog={}",
        url::form_urlencoded::byte_serialize(json!({ "distinct_id": id }).to_string().as_bytes()).collect::<String>()
    )
}

/// Out to GitHub as `visitor`: the flow's cookie and state.
async fn start(r: &Run, visitor: &str) -> (String, String) {
    let f = r.c.get(format!("{}/auth/github/start?return=/", r.root)).header("cookie", visitor_cookie(visitor)).send().await.unwrap();
    assert_eq!(f.status(), 302);
    let to = url::Url::parse(f.headers()["location"].to_str().unwrap()).unwrap();
    let state = to.query_pairs().find(|(k, _)| k == "state").unwrap().1.into_owned();
    let flow = f
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().to_owned())
        .find(|c| c.starts_with("wisp_grid_oauth="))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    (flow, state)
}

async fn back(r: &Run, visitor: &str, flow: &str, query: &str) -> u16 {
    let cookie = format!("{flow}; {}", visitor_cookie(visitor));
    r.c.get(format!("{}/auth/github/callback?{query}", r.root)).header("cookie", cookie).send().await.unwrap().status().as_u16()
}

/// Every event the grid made: what reached the fake `PostHog`, and what's still waiting.
fn events(r: &Run) -> Vec<Value> {
    let mut out: Vec<Value> = r
        .posthog
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(_, b)| serde_json::from_str::<Value>(b).ok()?.get("batch")?.as_array().cloned())
        .flatten()
        .collect();
    out.extend(r.grid.telemetry.take());
    out
}

fn named<'a>(all: &'a [Value], e: &str) -> Vec<&'a Value> {
    all.iter().filter(|v| v["event"] == e).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_hosted_grid_counts_each_sign_in_step_as_the_pages_visitor() {
    let r = grid(HOSTED, Some(true), "hosted").await;

    // the sign-in page loads PostHog and counts its provider clicks
    let page = r.c.get(format!("{}/signin", r.root)).send().await.unwrap().text().await.unwrap();
    assert!(page.contains(&format!("posthog.init(\"{KEY}\"")) && page.contains("grid_signin_clicked"), "{page}");

    // turned back at GitHub ("Cancel"): started, then failed because of the provider
    let (flow, state) = start(&r, "visitor-abc123").await;
    assert_eq!(back(&r, "visitor-abc123", &flow, &format!("error=access_denied&state={state}")).await, 400);
    // a stale flow (another tab used it): failed, expired
    assert_eq!(back(&r, "visitor-abc123", &flow, &format!("code=abc&state={state}")).await, 400);

    // signed up, then (another day) signed in
    let (flow, state) = start(&r, "visitor-abc123").await;
    assert_eq!(back(&r, "visitor-abc123", &flow, &format!("code=abc&state={state}")).await, 302);
    let (flow, state) = start(&r, "visitor-abc123").await;
    assert_eq!(back(&r, "visitor-abc123", &flow, &format!("code=abc&state={state}")).await, 302);

    let all = events(&r);
    let started = named(&all, "grid_auth_started");
    assert_eq!(started.len(), 3, "{all:#?}");
    assert!(started.iter().all(|e| e["properties"]["distinct_id"] == "visitor-abc123" && e["properties"]["provider"] == "github"));
    let failed: Vec<(&Value, &Value)> =
        named(&all, "grid_auth_failed").iter().map(|e| (&e["properties"]["reason"], &e["properties"]["distinct_id"])).collect();
    assert_eq!(failed, [(&json!("provider"), &json!("visitor-abc123")), (&json!("expired"), &json!("visitor-abc123"))]);
    let finished: Vec<&Value> = named(&all, "grid_auth_finished").iter().map(|e| &e["properties"]["signup"]).collect();
    assert_eq!(finished, [&json!(true), &json!(false)]);

    // the account's own events: a hash of its id, never its handle or email
    let ada = r.grid.accounts.user_by_handle("ada").unwrap().unwrap();
    let who = GridTelemetry::who(Some(&ada.id));
    for e in ["grid_signup", "grid_signin"] {
        let v = named(&all, e);
        assert_eq!(v.len(), 1, "{e}: {all:#?}");
        assert_eq!(v[0]["properties"]["distinct_id"], who.as_str());
        assert_eq!(v[0]["properties"]["$lib"], "wisp-grid");
    }
    let text = serde_json::to_string(&all).unwrap();
    assert!(!text.contains("ada@example.com") && !text.contains("\"ada\"") && !text.contains(&ada.id), "{text}");
    r.stop.cancel();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_self_hosted_grid_or_telemetry_off_loads_no_script_and_counts_no_step() {
    for (domain, telemetry, tag) in [("wisp.localhost", Some(true), "self"), (HOSTED, None, "off")] {
        let r = grid(domain, telemetry, tag).await;
        let page = r.c.get(format!("{}/signin", r.root)).send().await.unwrap().text().await.unwrap();
        assert!(!page.contains("posthog"), "{tag}: {page}");
        let (flow, state) = start(&r, "visitor-abc123").await;
        assert_eq!(back(&r, "visitor-abc123", &flow, &format!("code=abc&state={state}")).await, 302);
        let all = events(&r);
        assert!(all.iter().all(|e| !e["event"].as_str().unwrap().starts_with("grid_auth_")), "{tag}: {all:#?}");
        // a self-hosted grid with telemetry on still counts its sign-ups (as hashed users), as v1 did
        assert_eq!(named(&all, "grid_signup").len(), usize::from(telemetry == Some(true)), "{tag}: {all:#?}");
        r.stop.cancel();
    }
}

#[test]
fn opting_out_and_odd_cookies() {
    let env = |k: &str, v: &str| [(k.to_owned(), v.to_owned())].into_iter().collect();
    assert!(GridTelemetry::new(Some(true), HOSTED, &wisp_grid::config::Env::new()).pages);
    assert!(!GridTelemetry::new(Some(true), HOSTED, &env("DO_NOT_TRACK", "1")).on);
    assert!(!GridTelemetry::new(Some(true), HOSTED, &env("WISP_TELEMETRY", "off")).on);
    assert!(GridTelemetry::new(Some(true), HOSTED, &env("DO_NOT_TRACK", "0")).on);
    assert!(!GridTelemetry::new(Some(false), HOSTED, &wisp_grid::config::Env::new()).on);
    let t = GridTelemetry::new(Some(true), HOSTED, &wisp_grid::config::Env::new());
    let name = format!("ph_{KEY}_posthog");
    for (v, want) in [
        (r#"{"distinct_id":"019a-visitor_1"}"#, Some("019a-visitor_1")),
        (r#"{"distinct_id":"<script>"}"#, None),
        (r#"{"distinct_id":"short"}"#, None),
        ("not json", None),
    ] {
        assert_eq!(t.visitor(&[(name.clone(), v.to_owned())].into_iter().collect()).as_deref(), want, "{v}");
    }
    assert_eq!(GridTelemetry::who(None), "grid-anonymous");
    assert!(GridTelemetry::who(Some("u1")).starts_with("grid-") && GridTelemetry::who(Some("u1")).len() == 29);
}

#[test]
fn box_changes_become_started_stopped_and_failed() {
    use wisp_grid::boxhost::{BoxChange, BoxState, BoxUserWire};
    let t = GridTelemetry::new(Some(true), "grid.example.com", &wisp_grid::config::Env::new());
    let change = |state, event: &str, started_at| BoxChange {
        user: BoxUserWire { id: "u1".into(), handle: "ada".into(), sandbox: None, owner: None },
        state,
        event: event.into(),
        started_at,
        stopped_at: None,
        next_wake_at: None,
        last_active_at: None,
        error: "secret error text".into(),
    };
    t.box_change(&change(BoxState::Starting, "starting", None), 1_000);
    t.box_change(&change(BoxState::Running, "woke", Some(3_500)), 3_500);
    t.box_change(&change(BoxState::Stopped, "idle", Some(3_500)), 3_500 + 30 * 60_000);
    t.box_change(&change(BoxState::Starting, "starting", None), 10);
    t.box_change(&change(BoxState::Stopped, "failed", None), 20);
    let all = t.take();
    let got: Vec<(&str, &Value)> = all.iter().map(|e| (e["event"].as_str().unwrap(), &e["properties"])).collect();
    assert_eq!(got.len(), 3, "{all:#?}");
    assert_eq!((got[0].0, &got[0].1["ms"]), ("grid_box_started", &json!(2_500)));
    assert_eq!((got[1].0, &got[1].1["reason"], &got[1].1["minutes"]), ("grid_box_stopped", &json!("idle"), &json!(30)));
    assert_eq!(got[2].0, "grid_box_failed");
    assert!(!serde_json::to_string(&all).unwrap().contains("secret error text"));
}
