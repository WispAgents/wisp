//! `GET /api/shares/by/<handle>/<key>` (ADR-0130): an unlisted share opens by name only with its cap (`?cap=` or
//! `x-wisp-cap`) or for its sharer, and a name refused answers byte for byte as a name nobody has.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signer as _, SigningKey};
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

async fn rig(name: &str) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-byname-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let file = json!({ "domain": "wisp.localhost", "listen": { "host": "127.0.0.1", "port": port }, "auth": { "dev": true } });
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
    async fn get(&self, path: &str, headers: &[(&str, &str)]) -> (u16, String) {
        let mut r = self.c.get(format!("{}{path}", self.root));
        for (k, v) in headers {
            r = r.header(*k, *v);
        }
        let r = r.send().await.unwrap();
        (r.status().as_u16(), r.text().await.unwrap())
    }
    fn done(self) {
        self.stop.cancel();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn put(key: &str, publish: bool) -> wisp_grid::shares::Put {
    let k = SigningKey::from_bytes(&[1; 32]);
    let body = "{\"wispCrew\":1}";
    wisp_grid::shares::Put {
        kind: "crew".into(),
        key: key.into(),
        title: "Crew".into(),
        body: B64.encode(body),
        signer: B64.encode(k.verifying_key().to_bytes()),
        sig: B64.encode(k.sign(body.as_bytes()).to_bytes()),
        publish: Some(publish),
        ..wisp_grid::shares::Put::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn unlisted_shares_answer_by_name_as_if_missing_without_their_cap() {
    let g = rig("unlisted").await;
    let ada = g.grid.accounts.dev_user("ada").unwrap();
    let v = g.grid.shares.put(&ada, &put("crew", false)).unwrap();
    let source = v["source"].as_str().unwrap().to_owned();
    let cap = source.split_once('#').map(|(_, c)| c.to_owned()).expect("an unlisted share's source carries its cap");
    // no cap: the very answer a missing name gets (`crew` renamed to a key nobody has, the rest the same)
    let (status, body) = g.get("/api/shares/by/ada/crew", &[]).await;
    let (missing_status, missing) = g.get("/api/shares/by/ada/nope", &[]).await;
    assert_eq!((status, missing_status), (404, 404));
    assert_eq!(body.replace("ada/crew", "ada/nope"), missing);
    // a wrong cap: the same
    assert_eq!(g.get(&format!("/api/shares/by/ada/crew?cap={}", "0".repeat(32)), &[]).await, (status, body.clone()));
    // the right cap, as a query or a header
    let (ok, content) = g.get(&format!("/api/shares/by/ada/crew?cap={cap}"), &[]).await;
    assert_eq!(ok, 200, "{content}");
    let content: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(content["card"]["source"], json!(source));
    assert_eq!(g.get("/api/shares/by/ada/crew", &[("x-wisp-cap", cap.as_str())]).await.0, 200);
    // its sharer, signed in, needs no cap
    let cookie = format!("wisp_grid={}", g.grid.accounts.new_session(&ada.id).unwrap());
    assert_eq!(g.get("/api/shares/by/ada/crew", &[("cookie", cookie.as_str())]).await.0, 200);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn published_shares_open_by_their_plain_name() {
    let g = rig("published").await;
    let ada = g.grid.accounts.dev_user("ada").unwrap();
    let v = g.grid.shares.put(&ada, &put("review", true)).unwrap();
    assert_eq!(v["source"], json!("grid://ada/review"));
    let (status, body) = g.get("/api/shares/by/ada/review", &[]).await;
    assert_eq!(status, 200, "{body}");
    g.done();
}
