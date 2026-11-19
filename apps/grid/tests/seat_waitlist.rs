//! The waitlist (v1 grid/server.js `POST /api/waitlist`, grid/accounts.js `waitFor`): trywisp.dev's form and an in-app
//! browser put an address on it, signing up with that address marks it joined, and `wisp-grid waitlist` lists them all.
//! No seat cap (the owner removed `maxUsers` on 2026-10-27), so no seat is ever held or refused.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

struct Rig {
    grid: Arc<wisp_grid::edge::Grid>,
    c: reqwest::Client,
    root: String,
    stop: CancellationToken,
    dir: std::path::PathBuf,
}

async fn rig(name: &str, auth: Value) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-seats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let file = json!({ "domain": "wisp.localhost", "listen": { "host": "127.0.0.1", "port": port }, "auth": auth });
    let cfg = wisp_grid::config::for_test(&dir, file).unwrap();
    let clock = wisp_grid::clock::system();
    let db = Arc::new(wisp_grid::db::Db::open(&dir, clock.now_ms()).unwrap());
    let url_cfg = cfg.clone();
    let boxes = wisp_grid::hostd::ProcessHost::new(
        wisp_grid::hostd::ProcessOptions {
            bin: "/nonexistent/wisp".into(),
            data: dir.clone(),
            idle_ms: 60_000,
            check_ms: 1000,
            start_ms: 2_000,
            max_running: 1,
            env: Vec::new(),
            url_of: Arc::new(move |h| url_cfg.instance_url(h)),
            boxd: None,
            previews: false,
        },
        clock.clone(),
    );
    let grid = wisp_grid::edge::Grid::new(cfg, db, clock, boxes);
    let stop = CancellationToken::new();
    tokio::spawn(wisp_grid::edge::serve(grid.clone(), listener, stop.clone()));
    let c = reqwest::Client::builder().no_proxy().resolve("wisp.localhost", ([127, 0, 0, 1], port).into()).build().unwrap();
    Rig { grid, c, root: format!("http://wisp.localhost:{port}"), stop, dir }
}

impl Rig {
    async fn wait(&self, body: &Value, ip: &str) -> reqwest::Response {
        self.c
            .post(format!("{}/api/waitlist", self.root))
            .header("origin", "https://trywisp.dev")
            .header("x-forwarded-for", ip)
            .json(body)
            .send()
            .await
            .unwrap()
    }
    fn done(self) {
        self.stop.cancel();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_site_s_form_puts_an_address_on_the_list_and_the_operator_sees_it() {
    let g = rig("form", json!({ "dev": true })).await;
    // the site's preflight, then its post: both readable from another origin
    let r =
        g.c.request(reqwest::Method::OPTIONS, format!("{}/api/waitlist", g.root))
            .header("origin", "https://trywisp.dev")
            .header("access-control-request-method", "POST")
            .send()
            .await
            .unwrap();
    assert_eq!((r.status().as_u16(), r.headers()["access-control-allow-origin"].to_str().unwrap()), (204, "*"));
    assert!(r.headers()["access-control-allow-headers"].to_str().unwrap().contains("content-type"));
    let r = g.wait(&json!({ "email": " Ada@Example.com " }), "1.1.1.1").await;
    assert_eq!(r.headers()["access-control-allow-origin"], "*");
    assert_eq!(r.json::<Value>().await.unwrap(), json!({ "ok": true, "joined": false, "invited": false, "position": 1 }));
    let r = g.wait(&json!({ "email": "bob@example.com", "source": "in-app" }), "1.1.1.2").await;
    assert_eq!(r.json::<Value>().await.unwrap()["position"], 2);
    // not an address: said, readable by the site too
    let r = g.wait(&json!({ "email": "nope" }), "1.1.1.3").await;
    assert_eq!((r.status().as_u16(), r.headers()["access-control-allow-origin"].to_str().unwrap()), (400, "*"));
    assert_eq!(r.json::<Value>().await.unwrap(), json!({ "error": "An email address, please." }));

    // the operator's list, from the command line as from the library
    let out =
        std::process::Command::new(env!("CARGO_BIN_EXE_wisp-grid")).args(["waitlist", "--data", g.dir.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.starts_with("ada@example.com") && text.contains(" site ") && text.contains("bob@example.com") && text.contains(" in-app "),
        "{text}"
    );
    assert!(text.contains("waiting") && !text.contains("Nobody is waiting"), "{text}");
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn ten_a_minute_from_one_address() {
    let g = rig("limit", json!({})).await;
    for i in 0..10 {
        assert_eq!(g.wait(&json!({ "email": format!("p{i}@example.com") }), "9.9.9.9").await.status(), 200);
    }
    let r = g.wait(&json!({ "email": "p10@example.com" }), "9.9.9.9").await;
    assert_eq!((r.status().as_u16(), r.headers()["access-control-allow-origin"].to_str().unwrap()), (429, "*"));
    // another address isn't held up
    assert_eq!(g.wait(&json!({ "email": "q@example.com" }), "8.8.8.8").await.status(), 200);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_seat_cap_left_in_grid_json_caps_nothing_and_signing_up_joins() {
    // the owner removed maxUsers (2026-10-27): an old grid.json that still says 1 hands out no seats and refuses no one
    let g = rig("nocap", json!({ "maxUsers": 1, "seats": { "min": 1, "cap": 1, "holdDays": 7 } })).await;
    let r = g.wait(&json!({ "email": "ada@example.com" }), "1.1.1.1").await;
    assert_eq!(r.json::<Value>().await.unwrap(), json!({ "ok": true, "joined": false, "invited": false, "position": 1 }));
    let idn = |n: &str, email: &str| wisp_grid::auth::Identity {
        provider: "github".into(),
        subject: n.into(),
        email: email.into(),
        email_verified: true,
        login: format!("u{n}"),
        ..wisp_grid::auth::Identity::default()
    };
    // more people than the old cap sign up
    for n in ["1", "2", "3"] {
        g.grid.accounts.sign_in(&g.grid.cfg.auth, &idn(n, ""), None).unwrap();
    }
    // ada signs up with the address she left: joined
    g.grid.accounts.sign_in(&g.grid.cfg.auth, &idn("4", "ada@example.com"), None).unwrap();
    let r = g.wait(&json!({ "email": "ada@example.com" }), "1.1.1.1").await;
    assert_eq!(r.json::<Value>().await.unwrap(), json!({ "ok": true, "joined": true, "invited": false, "position": 0 }));
    let report = wisp_grid::wishes::report(&g.grid.accounts).unwrap();
    assert!(report.starts_with("ada@example.com") && report.contains("joined "), "{report}");
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn get_the_app_puts_an_address_on_the_phone_app_list_apart_from_the_seats() {
    let g = rig("phone-app", json!({ "dev": true })).await;
    // trywisp.dev's Get the app: the same endpoint, with the app it's for; readable from the site
    let r = g.wait(&json!({ "email": " Ada@Example.com ", "kind": "phone-app", "platform": "android" }), "1.1.1.1").await;
    assert_eq!((r.status().as_u16(), r.headers()["access-control-allow-origin"].to_str().unwrap()), (200, "*"));
    assert_eq!(r.json::<Value>().await.unwrap(), json!({ "ok": true, "kind": "phone-app", "platform": "android" }));
    // again: the same answer, so nobody can find out whether an address is on the list
    let r = g.wait(&json!({ "email": "ada@example.com", "kind": "phone-app", "platform": "both" }), "1.1.1.1").await;
    assert_eq!(r.json::<Value>().await.unwrap(), json!({ "ok": true, "kind": "phone-app", "platform": "both" }));
    // an app that doesn't exist, or no address: refused, readable by the site
    let r = g.wait(&json!({ "email": "bob@example.com", "kind": "toaster" }), "1.1.1.2").await;
    assert_eq!((r.status().as_u16(), r.headers()["access-control-allow-origin"].to_str().unwrap()), (400, "*"));
    assert_eq!(r.json::<Value>().await.unwrap(), json!({ "error": "There is no app by that name to wait for." }));
    let r = g.wait(&json!({ "email": "nope", "kind": "phone-app" }), "1.1.1.3").await;
    assert_eq!(r.json::<Value>().await.unwrap(), json!({ "error": "An email address, please." }));
    // the seat list is untouched: the operator's report shows the app list under its own heading
    let report = wisp_grid::wishes::report(&g.grid.accounts).unwrap();
    assert!(report.starts_with("Nobody is waiting."), "{report}");
    assert!(
        report.contains("waiting for an app:")
            && report.contains("ada@example.com")
            && report.contains("phone-app")
            && report.contains("both"),
        "{report}"
    );
    g.done();
}
