//! The phone app reaches its cloud wisp with a device token from a webview whose Origin is `https://localhost`. The
//! box's wisp refuses a write with a foreign Origin (`forbidden origin`, 403), so before this every sync POST the phone
//! sent through the grid failed, its client reopened the stream for each, and the box flipped online/offline about
//! four times a second (bouraouisana60, 2026-11-23). A bearer token can't be sent cross-site by a browser on its own,
//! so the grid forwards a device token's (or an API key's) request without the caller's Origin; a cookie keeps it.
//!
//! The box is this test binary: hostd runs `<bin> serve` with `WISP_BOX=1`, so `fake_box_serve` (the only test here
//! whose name holds "serve") answers as a box's wisp would. Run normally, it does nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::PathBuf;

use serde_json::json;
use wisp_grid::accounts::Poll;

/// The stand-in box: `/api/health`; a POST with an Origin header gets the box's 403 `forbidden origin`, without one
/// `{"ok":true}`, as `crates/wisp-api/src/guard.rs` answers a proxied write.
#[test]
fn fake_box_serve() {
    if std::env::var("WISP_BOX").as_deref() != Ok("1") {
        return;
    }
    let port: u16 = std::env::var("WISP_PORT").unwrap().parse().unwrap();
    let l = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    let bound = l.local_addr().unwrap().port();
    let wisp_home = PathBuf::from(std::env::var("WISP_HOME").unwrap());
    let file = json!({ "url": format!("http://127.0.0.1:{bound}"), "port": bound, "pid": std::process::id() });
    std::fs::write(wisp_home.join("server.json"), file.to_string()).unwrap();
    for s in l.incoming().flatten() {
        let mut r = BufReader::new(s);
        let mut line = String::new();
        let _ = r.read_line(&mut line);
        let (mut origin, mut len) = (false, 0usize);
        let mut h = String::new();
        while r.read_line(&mut h).is_ok_and(|n| n > 2) {
            let lower = h.to_ascii_lowercase();
            origin |= lower.starts_with("origin:");
            if let Some(v) = lower.strip_prefix("content-length:") {
                len = v.trim().parse().unwrap_or(0);
            }
            h.clear();
        }
        let mut body = vec![0; len];
        let _ = r.read_exact(&mut body);
        let (status, out) = if line.starts_with("POST") && origin {
            ("403 Forbidden", r#"{"code":"forbidden","error":"forbidden origin"}"#)
        } else {
            ("200 OK", r#"{"ok":true}"#)
        };
        let mut s = r.into_inner();
        let _ = write!(
            s,
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}",
            out.len()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_phones_sync_posts_reach_its_box_with_a_device_token() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-origin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (grid, port, stop) = wisp_grid::edge::dev(&dir, 0, std::env::current_exe().unwrap(), Vec::new(), 600_000).await.unwrap();
    let a = &grid.accounts;
    let u = a.dev_user("kai").unwrap();
    let c = a.device_code("Phone (wisp app)").unwrap();
    a.decide(c["user_code"].as_str().unwrap(), &u.id, true).unwrap();
    let token = match a.poll(c["device_code"].as_str().unwrap()).unwrap() {
        Poll::Token { token, .. } => token,
        other => panic!("{other:?}"),
    };
    let host = format!("kai.{}", grid.cfg.domain);
    let web = reqwest::Client::builder().no_proxy().resolve(&host, ([127, 0, 0, 1], port).into()).build().unwrap();
    let post = |origin: Option<&str>| {
        let mut r = web
            .post(format!("http://{host}:{port}/api/events"))
            .bearer_auth(&token)
            .header("x-wisp-client", "phone-1")
            .json(&json!({ "t": "sub", "topic": "list" }));
        if let Some(o) = origin {
            r = r.header("origin", o);
        }
        r.send()
    };
    // the phone app's webview, as Capacitor sends it
    let r = post(Some("https://localhost")).await.unwrap();
    let status = r.status().as_u16();
    assert_eq!((status, r.text().await.unwrap()), (200, r#"{"ok":true}"#.to_owned()), "the phone's write reached the box");
    // and without one, as before
    assert_eq!(post(None).await.unwrap().status().as_u16(), 200);
    stop.cancel();
    let _ = std::fs::remove_dir_all(&dir);
}
