//! The grid's side of a computer's tunnel (ADR-0010): a computer that dials again replaces its old tunnel,
//! and requests go to the new one.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::Full;
use hyper_util::rt::{TokioExecutor, TokioIo};
use wisp_grid::accounts::Poll;

/// A computer holding a tunnel that answers every request with `who`. Resolves when it's up.
async fn computer(port: u16, id: &str, token: &str, who: &'static str) -> tokio::task::JoinHandle<()> {
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
        let svc = hyper::service::service_fn(move |_req| async move {
            Ok::<_, std::convert::Infallible>(http::Response::new(Full::new(Bytes::from_static(who.as_bytes()))))
        });
        let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new()).serve_connection(TokioIo::new(up), svc).await;
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_tunnel_for_the_same_computer_wins() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-tunnel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (g, port, stop) = wisp_grid::edge::dev(&dir, 0, "wisp".into(), Vec::new(), 600_000).await.unwrap();
    let ada = g.accounts.dev_user("ada").unwrap();
    let code = g.accounts.device_code("studio").unwrap();
    g.accounts.decide(code["user_code"].as_str().unwrap(), &ada.id, true).unwrap();
    let Poll::Token { token, .. } = g.accounts.poll(code["device_code"].as_str().unwrap()).unwrap() else { panic!("no token") };
    let (_, dev) = g.accounts.device(&token).unwrap().unwrap();
    let c = g.accounts.add_computer(&ada, &dev, "studio").unwrap();
    let ask = |label: String| {
        let token = token.clone();
        async move {
            let w = reqwest::Client::builder()
                .no_proxy()
                .resolve(&format!("{label}.wisp.localhost"), ([127, 0, 0, 1], port).into())
                .build()
                .unwrap();
            let r = w.get(format!("http://{label}.wisp.localhost:{port}/")).bearer_auth(token).send().await.unwrap();
            (r.status().as_u16(), r.text().await.unwrap())
        }
    };
    assert_eq!(ask(c.label.clone()).await.0, 503, "offline before it dials");
    let first = computer(port, &c.id, &token, "one").await;
    let at = Instant::now();
    while !g.tunnels.online(&c.id) {
        assert!(at.elapsed() < Duration::from_secs(5));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(ask(c.label.clone()).await, (200, "one".to_owned()));
    let _second = computer(port, &c.id, &token, "two").await;
    let at = Instant::now();
    loop {
        if ask(c.label.clone()).await == (200, "two".to_owned()) {
            break;
        }
        assert!(at.elapsed() < Duration::from_secs(5), "the new tunnel never took over");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(g.tunnels.count(), 1);
    // a device that isn't this computer's can't hold its tunnel
    let other =
        reqwest::Client::builder().http1_only().no_proxy().resolve("wisp.localhost", ([127, 0, 0, 1], port).into()).build().unwrap();
    let r = other
        .get(format!("http://wisp.localhost:{port}/api/computers/{}/tunnel", c.id))
        .header("connection", "Upgrade")
        .header("upgrade", "wisp-tunnel")
        .bearer_auth("wgd_nope")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    first.abort();
    stop.cancel();
    let _ = std::fs::remove_dir_all(&dir);
}

/// QA round 10 Grid #2: at phone width, reopening an offline computer showed the raw JSON (`WebKit`) or the stale app
/// (Chrome): the wisp app's service worker (phones only) answers navigations from its cache, and its own fetch of the
/// page isn't a navigation. The grid's offline answer names its waiting page (`x-wisp-offline`, under `/_grid/`,
/// which the worker never answers), is never cached, and is the page for any navigation; the waiting page opens what
/// was asked for once the computer is back.
#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::many_single_char_names)]
async fn an_offline_computer_answers_every_navigation_with_its_waiting_page_and_names_it_to_the_worker() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-tunnel-offline-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (g, port, stop) = wisp_grid::edge::dev(&dir, 0, "wisp".into(), Vec::new(), 600_000).await.unwrap();
    let ada = g.accounts.dev_user("ada").unwrap();
    let code = g.accounts.device_code("studio").unwrap();
    g.accounts.decide(code["user_code"].as_str().unwrap(), &ada.id, true).unwrap();
    let Poll::Token { token, .. } = g.accounts.poll(code["device_code"].as_str().unwrap()).unwrap() else { panic!("no token") };
    let (_, dev) = g.accounts.device(&token).unwrap().unwrap();
    let c = g.accounts.add_computer(&ada, &dev, "Studio").unwrap();
    let w = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(&format!("{}.wisp.localhost", c.label), ([127, 0, 0, 1], port).into())
        .resolve(&format!("p3000--{}.wisp.localhost", c.label), ([127, 0, 0, 1], port).into())
        .build()
        .unwrap();
    let at = |p: &str| format!("http://{}.wisp.localhost:{port}{p}", c.label);
    let preview = |p: &str| format!("http://p3000--{}.wisp.localhost:{port}{p}", c.label);
    let head = |r: &reqwest::Response, k: &str| r.headers().get(k).map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default();

    // the worker's own fetch of the page (no navigation, `*/*`): JSON, never cached, naming the waiting page
    let r = w.get(at("/tasks?x=1")).bearer_auth(&token).header("x-wisp-sw", "1").send().await.unwrap();
    assert_eq!(r.status(), 503);
    assert_eq!(head(&r, "x-wisp-offline"), "/_grid/offline?r=%2Ftasks%3Fx%3D1");
    assert!(head(&r, "cache-control").contains("no-store"), "{:?}", r.headers());
    // a navigation whatever its Accept says (`sec-fetch-mode: navigate`): on to the waiting page, never cached
    let r = w.get(at("/tasks")).bearer_auth(&token).header("accept", "*/*").header("sec-fetch-mode", "navigate").send().await.unwrap();
    assert_eq!((r.status().as_u16(), head(&r, "location")), (302, "/_grid/offline?r=%2Ftasks".to_owned()));
    assert!(head(&r, "cache-control").contains("no-store"));
    // the waiting page: a page that's there (200: no failed load in the console), never cached
    let r = w.get(at("/_grid/offline?r=%2Ftasks")).bearer_auth(&token).header("accept", "text/html").send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert!(head(&r, "cache-control").contains("no-store"));
    let page = r.text().await.unwrap();
    assert!(page.contains("Studio is offline") && page.contains("Waiting for it"), "{page}");
    // its poll asks the same address, and hears 200 {online} (not a 503 a minute in the console)
    assert!(page.contains("if(j.online)return location.reload()"), "{page}");
    let poll = |w: reqwest::Client| {
        let u = at("/_grid/offline?r=%2Ftasks");
        let token = token.clone();
        async move {
            let r = w.get(u).bearer_auth(token).header("accept", "application/json").send().await.unwrap();
            (r.status().as_u16(), r.json::<serde_json::Value>().await.unwrap())
        }
    };
    assert_eq!(poll(w.clone()).await, (200, serde_json::json!({ "online": false })));
    // only that exact page waits: a path under it is the computer's, waited for like any other
    let r = w.get(at("/_grid/offline/x")).bearer_auth(&token).header("accept", "text/html").send().await.unwrap();
    assert_eq!(head(&r, "location"), "/_grid/offline?r=%2F_grid%2Foffline%2Fx");
    // a preview of a page on it waits the same way, on its own address
    let nav = |u: String| w.get(u).bearer_auth(&token).header("accept", "text/html");
    let r = nav(preview("/app")).send().await.unwrap();
    assert_eq!((r.status().as_u16(), head(&r, "location")), (302, "/_grid/offline?r=%2Fapp".to_owned()));
    let r = nav(preview("/_grid/offline?r=%2Fapp")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert!(r.text().await.unwrap().contains("Studio is offline"));
    let r = w.get(preview("/_grid/offline?r=%2Fapp")).bearer_auth(&token).header("accept", "application/json").send().await.unwrap();
    assert_eq!(r.json::<serde_json::Value>().await.unwrap(), serde_json::json!({ "online": false }));

    // back: the waiting page opens what was asked for (only a path here)
    let _up = computer(port, &c.id, &token, "one").await;
    let t = Instant::now();
    while !g.tunnels.online(&c.id) {
        assert!(t.elapsed() < Duration::from_secs(5));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(poll(w.clone()).await, (200, serde_json::json!({ "online": true })));
    let r = w.get(at("/_grid/offline?r=%2Ftasks")).bearer_auth(&token).header("accept", "text/html").send().await.unwrap();
    assert_eq!((r.status().as_u16(), head(&r, "location")), (302, "/tasks".to_owned()));
    let r =
        w.get(at("/_grid/offline?r=https%3A%2F%2Fevil.example")).bearer_auth(&token).header("accept", "text/html").send().await.unwrap();
    assert_eq!(head(&r, "location"), "/");
    stop.cancel();
}
