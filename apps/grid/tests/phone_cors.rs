//! The phone app signs in from another origin (its webview is `https://localhost`), so the grid's `/api/` answers
//! v1's open CORS (grid/server.js `DEVICE_CORS`): the preflight a JSON or bearer call sends gets a 204 with the headers,
//! and every answer, refusals included, carries `allow-origin: *`. Without it the v2 grid 404'd the preflight and no
//! phone could sign in or reach its account (2026-11-21).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

async fn grid(name: &str) -> (u16, tokio_util::sync::CancellationToken, PathBuf) {
    let dir = std::env::temp_dir().join(format!("wisp-grid-cors-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (_grid, port, stop) = wisp_grid::edge::dev(&dir, 0, PathBuf::from("/nonexistent/wisp"), Vec::new(), 600_000).await.unwrap();
    (port, stop, dir)
}

fn web(port: u16) -> reqwest::Client {
    reqwest::Client::builder().no_proxy().resolve("wisp.localhost", ([127, 0, 0, 1], port).into()).build().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_phone_apps_preflight_and_calls_get_cors() {
    let (port, _stop, _dir) = grid("phone").await;
    let url = |p: &str| format!("http://wisp.localhost:{port}{p}");
    let c = web(port);
    for path in ["/api/device/code", "/api/device/token", "/api/me", "/api/computers"] {
        let r = c
            .request(reqwest::Method::OPTIONS, url(path))
            .header("origin", "https://localhost")
            .header("access-control-request-method", "POST")
            .header("access-control-request-headers", "authorization, content-type")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status().as_u16(), 204, "{path}");
        assert_eq!(r.headers()["access-control-allow-origin"], "*", "{path}");
        let allowed = r.headers()["access-control-allow-headers"].to_str().unwrap();
        assert!(allowed.contains("authorization") && allowed.contains("content-type"), "{path}: {allowed}");
    }
    let code = c
        .post(url("/api/device/code"))
        .header("origin", "https://localhost")
        .json(&serde_json::json!({ "name": "Phone (wisp app)" }))
        .send()
        .await
        .unwrap();
    assert_eq!((code.status().as_u16(), code.headers()["access-control-allow-origin"].to_str().unwrap()), (200, "*"));
    let me = c.get(url("/api/me")).header("origin", "https://localhost").bearer_auth("wgd_nope").send().await.unwrap();
    assert_eq!(
        (me.status().as_u16(), me.headers()["access-control-allow-origin"].to_str().unwrap()),
        (401, "*"),
        "a refusal is readable too"
    );
    // never on the grid's pages: only /api/
    let page = c.get(url("/signin")).header("origin", "https://localhost").send().await.unwrap();
    assert!(page.headers().get("access-control-allow-origin").is_none());
}
