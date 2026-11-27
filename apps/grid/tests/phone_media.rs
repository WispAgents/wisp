//! The phone app's pictures, videos and attachments (ADR-0122): its page is `https://localhost`, so a plain `<img>`
//! can send neither the device's token nor a cookie. It asks `POST <address>/_grid/media` for signed links and the
//! edge lets exactly those files through. v1's grid had it (#921); the v2 grid that replaced it on wisp Cloud didn't,
//! so the POST 404'd and every one said "This picture can't be shown here." (2026-11-27).
//!
//! The box is this test binary: hostd runs `<bin> serve` with `WISP_BOX=1`, so `fake_box_serve` (the only test here
//! whose name holds "serve") answers as a box's wisp would, with the path it was asked for. Run normally, it does
//! nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::PathBuf;

use serde_json::{Value, json};
use wisp_grid::accounts::Poll;

/// The stand-in box: answers every request with the path and query it got.
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
        let mut h = String::new();
        while r.read_line(&mut h).is_ok_and(|n| n > 2) {
            h.clear();
        }
        let out = line.split(' ').nth(1).unwrap_or_default().to_owned();
        let mut s = r.into_inner();
        let _ = write!(s, "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}", out.len());
    }
}

fn token_for(a: &wisp_grid::accounts::Accounts, user_id: &str) -> String {
    let c = a.device_code("Phone (wisp app)").unwrap();
    a.decide(c["user_code"].as_str().unwrap(), user_id, true).unwrap();
    match a.poll(c["device_code"].as_str().unwrap()).unwrap() {
        Poll::Token { token, .. } => token,
        other => panic!("{other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::many_single_char_names)]
async fn the_phone_loads_an_answers_files_through_links_the_grid_signed() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-media-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (grid, port, stop) = wisp_grid::edge::dev(&dir, 0, std::env::current_exe().unwrap(), Vec::new(), 600_000).await.unwrap();
    let a = &grid.accounts;
    let mia = a.dev_user("mia").unwrap();
    let ned = a.dev_user("ned").unwrap();
    let token = token_for(a, &mia.id);
    let neds = token_for(a, &ned.id);
    let (host, other) = (format!("mia.{}", grid.cfg.domain), format!("ned.{}", grid.cfg.domain));
    let web = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(&host, ([127, 0, 0, 1], port).into())
        .resolve(&other, ([127, 0, 0, 1], port).into())
        .build()
        .unwrap();
    let at = |h: &str, p: &str| format!("http://{h}:{port}{p}");
    let img = |u: String| web.get(u).header("accept", "image/*").header("origin", "https://localhost").send();
    let sign = |bearer: String, paths: Value| {
        web.post(at(&host, "/_grid/media"))
            .bearer_auth(bearer)
            .header("origin", "https://localhost")
            .json(&json!({ "paths": paths }))
            .send()
    };
    let file = "/api/sessions/s1/file?path=%2Ftmp%2Fa.png";
    let upload = "/api/peer/my-mac/api/uploads/u1";

    // the phone's preflight, answered at the address without waking anything
    let pre = web
        .request(reqwest::Method::OPTIONS, at(&host, "/_grid/media"))
        .header("origin", "https://localhost")
        .header("access-control-request-method", "POST")
        .header("access-control-request-headers", "authorization, content-type")
        .send()
        .await
        .unwrap();
    assert_eq!((pre.status().as_u16(), pre.headers()["access-control-allow-origin"].to_str().unwrap()), (204, "*"));

    let r = sign(token.clone(), json!([file, upload])).await.unwrap();
    assert_eq!(r.headers()["access-control-allow-origin"], "*", "the phone can read the answer");
    assert_eq!(r.status().as_u16(), 200);
    let body: Value = r.json().await.unwrap();
    let urls: Vec<String> = body["urls"].as_array().unwrap().iter().map(|u| u.as_str().unwrap().to_owned()).collect();
    let (f, u) = (&urls[0], &urls[1]);
    assert!(f.starts_with(&format!("{file}&_ge=")) && u.starts_with(&format!("{upload}?_ge=")), "{urls:?}");

    assert_ne!(img(at(&host, file)).await.unwrap().status().as_u16(), 200, "no token, no signature: the grid refuses");
    let through = img(at(&host, f)).await.unwrap();
    assert_eq!(through.status().as_u16(), 200, "signed: on to her wisp");
    assert_eq!(through.text().await.unwrap(), file, "the wisp gets the path without the signature");
    assert_eq!(img(at(&host, u)).await.unwrap().text().await.unwrap(), upload);
    let head = web.head(at(&host, f)).send().await.unwrap();
    assert_eq!(head.status().as_u16(), 200, "HEAD too (a <video> asks)");

    assert_ne!(img(at(&host, &f.replace("a.png", "b.png"))).await.unwrap().status().as_u16(), 200, "another file");
    let forged = format!("{}x", &f[..f.len() - 1]);
    assert_ne!(img(at(&host, &forged)).await.unwrap().status().as_u16(), 200, "a forged signature");
    assert_ne!(img(at(&other, f)).await.unwrap().status().as_u16(), 200, "another address");
    let post = web.post(at(&host, f)).header("origin", "https://localhost").json(&json!({})).send().await.unwrap();
    assert_ne!(post.status().as_u16(), 200, "GET and HEAD only");

    assert_eq!(sign(token.clone(), json!(["/api/sessions/s1"])).await.unwrap().status().as_u16(), 400, "only a file or an upload");
    assert_eq!(sign(token.clone(), json!([])).await.unwrap().status().as_u16(), 400);
    let many: Vec<&str> = std::iter::repeat_n(file, 101).collect();
    assert_eq!(sign(token.clone(), json!(many)).await.unwrap().status().as_u16(), 400, "up to 100");
    assert_eq!(sign(neds, json!([file])).await.unwrap().status().as_u16(), 403, "another account's phone");

    let (_, d) = a.device(&format!("Bearer {token}")).unwrap().unwrap();
    a.revoke_device(&mia.id, &d.id).unwrap();
    assert_ne!(img(at(&host, f)).await.unwrap().status().as_u16(), 200, "the device revoked: its links die with it");

    stop.cancel();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A computer holding its tunnel, answering every request with the path and query it got.
async fn computer(port: u16, id: &str, token: &str) -> tokio::task::JoinHandle<()> {
    use hyper_util::rt::{TokioExecutor, TokioIo};
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
    tokio::spawn(async move {
        let svc = hyper::service::service_fn(|req: http::Request<hyper::body::Incoming>| async move {
            let pq = req.uri().path_and_query().map_or("/", |p| p.as_str()).to_owned();
            Ok::<_, std::convert::Infallible>(http::Response::new(http_body_util::Full::new(bytes::Bytes::from(pq))))
        });
        let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new()).serve_connection(TokioIo::new(up), svc).await;
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_computer_through_the_grid_takes_the_phones_signed_links_too() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-media-computer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (g, port, stop) = wisp_grid::edge::dev(&dir, 0, "wisp".into(), Vec::new(), 600_000).await.unwrap();
    let ada = g.accounts.dev_user("ada").unwrap();
    let laptop = token_for(&g.accounts, &ada.id);
    let phone = token_for(&g.accounts, &ada.id);
    let (_, dev) = g.accounts.device(&laptop).unwrap().unwrap();
    let c = g.accounts.add_computer(&ada, &dev, "studio").unwrap();
    let tunnel = computer(port, &c.id, &laptop).await;
    let at = std::time::Instant::now();
    while !g.tunnels.online(&c.id) {
        assert!(at.elapsed() < std::time::Duration::from_secs(5));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let host = format!("{}.wisp.localhost", c.label);
    let web = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(&host, ([127, 0, 0, 1], port).into())
        .build()
        .unwrap();
    let url = |p: &str| format!("http://{host}:{port}{p}");
    let file = "/api/sessions/s1/file?path=shot.png";
    let r = web.post(url("/_grid/media")).bearer_auth(&phone).json(&json!({ "paths": [file] })).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 200);
    let body: Value = r.json().await.unwrap();
    let f = body["urls"][0].as_str().unwrap().to_owned();
    let got = web.get(url(&f)).header("accept", "image/*").send().await.unwrap();
    assert_eq!(got.status().as_u16(), 200);
    assert_eq!(got.text().await.unwrap(), file, "the computer gets the path without the signature");
    assert_ne!(web.get(url(file)).header("accept", "image/*").send().await.unwrap().status().as_u16(), 200, "unsigned: refused");
    tunnel.abort();
    stop.cancel();
    let _ = std::fs::remove_dir_all(&dir);
}
