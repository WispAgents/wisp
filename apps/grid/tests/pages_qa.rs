//! The grid's web pages as QA round 2 (2026-11-18, B2–B11) found them, each fixed: the images served, a page (not
//! JSON) for a browser that asked for one, the front page's way in, `/events`, Remove asking first and its note
//! said once, inputs that keep what was typed, how a dev account signs in, where API keys are, and the chrome.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::json;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

const PAGE: &str = "text/html,application/xhtml+xml";
const FORM: &str = "application/x-www-form-urlencoded";

struct Rig {
    grid: Arc<wisp_grid::edge::Grid>,
    c: reqwest::Client,
    root: String,
    port: u16,
    stop: CancellationToken,
    dir: std::path::PathBuf,
}

async fn rig(name: &str) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-qa-{name}-{}", std::process::id()));
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
    let c = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve("wisp.localhost", ([127, 0, 0, 1], port).into())
        .resolve("ada.wisp.localhost", ([127, 0, 0, 1], port).into())
        .build()
        .unwrap();
    Rig { grid, c, root: format!("http://wisp.localhost:{port}"), port, stop, dir }
}

impl Rig {
    fn session(&self, handle: &str) -> String {
        let u = self.grid.accounts.dev_user(handle).unwrap();
        format!("wisp_grid={}", self.grid.accounts.new_session(&u.id).unwrap())
    }
    async fn page(&self, path: &str, cookie: &str) -> String {
        self.c
            .get(format!("{}{path}", self.root))
            .header("cookie", cookie)
            .header("accept", PAGE)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    }
    fn done(self) {
        self.stop.cancel();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn csrf_in(page: &str) -> String {
    let at = page.find(r#"name="csrf" value=""#).expect("a form with its token");
    page[at + r#"name="csrf" value=""#.len()..].split('"').next().unwrap().to_owned()
}

fn cookie_set(r: &reqwest::Response, name: &str) -> Option<String> {
    r.headers().get_all("set-cookie").iter().map(|v| v.to_str().unwrap().to_owned()).find(|c| c.starts_with(&format!("{name}=")))
}

#[tokio::test(flavor = "multi_thread")]
async fn the_grid_s_images_are_served_on_every_address() {
    let g = rig("images").await;
    for (path, ct) in [
        ("/_grid/a/icon.svg", "image/svg+xml"),
        ("/_grid/a/light-app.webp", "image/webp"),
        ("/_grid/a/light-report.webp", "image/webp"),
        ("/_grid/a/claudecode-color.svg", "image/svg+xml"),
        ("/_grid/a/codex-color.svg", "image/svg+xml"),
        ("/_grid/a/opencode.svg", "image/svg+xml"),
        ("/_grid/a/cursor.svg", "image/svg+xml"),
        ("/favicon.svg", "image/svg+xml"),
    ] {
        let r = g.c.get(format!("{}{path}", g.root)).send().await.unwrap();
        assert_eq!((r.status().as_u16(), r.headers()["content-type"].to_str().unwrap()), (200, ct), "{path}");
        assert!(r.headers()["cache-control"].to_str().unwrap().contains("max-age"));
    }
    // at a wisp's own address too, without signing in (a page there shows them)
    g.session("ada");
    let r = g.c.get(format!("http://ada.wisp.localhost:{}/_grid/a/icon.svg", g.port)).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(g.c.get(format!("{}/_grid/a/nope.svg", g.root)).send().await.unwrap().status(), 404);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_browser_gets_a_page_for_what_isn_t_there_and_an_api_client_json() {
    let g = rig("notfound").await;
    for (path, says) in [("/no/such/page", "its link has a typo"), ("/s/doesnotexist", "shared any more, or its link has a typo")] {
        let r = g.c.get(format!("{}{path}", g.root)).header("accept", PAGE).send().await.unwrap();
        assert_eq!((r.status().as_u16(), r.headers()["content-type"].to_str().unwrap()), (404, "text/html; charset=utf-8"), "{path}");
        let html = r.text().await.unwrap();
        assert!(html.contains("<title>Nothing here · wisp Grid</title>") && html.contains(says) && html.contains(">Home<"), "{html}");
    }
    let r = g.c.get(format!("{}/no/such/page", g.root)).send().await.unwrap();
    assert_eq!(r.json::<serde_json::Value>().await.unwrap(), json!({ "error": "not found" }));
    let r = g.c.get(format!("{}/api/shares/doesnotexist", g.root)).send().await.unwrap();
    assert_eq!((r.status().as_u16(), r.headers()["content-type"].to_str().unwrap()), (404, "application/json"));
    // a handle nobody has: a page for a browser too
    let r = g.c.get(format!("http://ada.wisp.localhost:{}/", g.port)).header("accept", PAGE).send().await.unwrap();
    assert!(r.status() == 404 && r.text().await.unwrap().contains("nobody with that handle"));
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_front_page_signed_out_is_the_way_in() {
    let g = rig("front").await;
    let ada = g.session("ada");
    let home = g.page("/", &ada).await;
    let r =
        g.c.post(format!("{}/signout", g.root))
            .header("cookie", &ada)
            .header("content-type", FORM)
            .body(format!("csrf={}", csrf_in(&home)))
            .send()
            .await
            .unwrap();
    assert_eq!(r.headers()["location"], "/");
    let front = g.page("/", "").await;
    assert!(front.contains(r#"href="/auth/dev/start"#) && front.contains("Sign in"), "{front}");
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn signed_in_pages_hear_on_events_when_something_changed() {
    let g = rig("events").await;
    let ada = g.session("ada");
    assert_eq!(g.c.get(format!("{}/events", g.root)).send().await.unwrap().status(), 401);
    let mut r = g.c.get(format!("{}/events", g.root)).header("cookie", &ada).send().await.unwrap();
    assert_eq!((r.status().as_u16(), r.headers()["content-type"].to_str().unwrap()), (200, "text/event-stream"));
    assert_eq!(&r.chunk().await.unwrap().unwrap()[..], b"retry: 3000\n\n");
    // bob's change isn't ada's; hers is
    g.session("bob");
    let bob = g.grid.accounts.user_by_handle("bob").unwrap().unwrap().id;
    let uid = g.grid.accounts.user_by_handle("ada").unwrap().unwrap().id;
    g.grid.notify(&bob);
    g.grid.notify(&uid);
    let next = tokio::time::timeout(std::time::Duration::from_secs(5), r.chunk()).await.unwrap().unwrap().unwrap();
    assert_eq!(&next[..], b"event: change\ndata: {}\n\n");
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn remove_asks_first_and_says_what_it_did_once() {
    let g = rig("remove").await;
    let ada = g.session("ada");
    let uid = g.grid.accounts.user_by_handle("ada").unwrap().unwrap().id;
    let c = g.grid.accounts.device_code("studio").unwrap();
    g.grid.accounts.decide(c["user_code"].as_str().unwrap(), &uid, true).unwrap();
    let wisp_grid::accounts::Poll::Token { token, user } = g.grid.accounts.poll(c["device_code"].as_str().unwrap()).unwrap() else {
        panic!()
    };
    let (_, dev) = g.grid.accounts.device(&token).unwrap().unwrap();
    let comp = g.grid.accounts.add_computer(&user, &dev, "Studio").unwrap();

    let home = g.page("/", &ada).await;
    assert!(home.contains(&format!(r#"action="/computers/{}/remove" data-confirm="Remove it?""#, comp.id)), "{home}");
    assert!(home.contains("dataset.confirm"), "the confirm script");
    let r =
        g.c.post(format!("{}/computers/{}/remove", g.root, comp.id))
            .header("cookie", &ada)
            .header("content-type", FORM)
            .body(format!("csrf={}", csrf_in(&home)))
            .send()
            .await
            .unwrap();
    // back home with no note in the URL: it rides a cookie for a minute
    assert_eq!(r.headers()["location"], "/");
    let flash = cookie_set(&r, "wisp_flash").expect("a flash");
    assert!(flash.contains("Max-Age=60"), "{flash}");
    let flash = flash.split(';').next().unwrap().to_owned();
    // the next page says it and drops it; a reload doesn't say it again
    let r = g.c.get(format!("{}/", g.root)).header("cookie", format!("{ada}; {flash}")).send().await.unwrap();
    assert!(cookie_set(&r, "wisp_flash").is_some_and(|c| c.contains("Max-Age=0")));
    assert!(r.text().await.unwrap().contains("Taken off the grid"));
    assert!(!g.page("/", &ada).await.contains("Taken off the grid"));
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_pages_keep_what_people_type_and_say_the_right_things() {
    let g = rig("copy").await;
    let ada = g.session("ada");
    let home = g.page("/", &ada).await;
    // inputs show what was typed; only the device code is capitals
    assert!(home.contains(".input.code{font:15px var(--mono);letter-spacing:.08em;text-transform:uppercase}"));
    let activate = g.page("/activate", &ada).await;
    assert!(activate.contains(r#"<input class="input code" name="code""#), "{activate}");
    // keys are on Account (docs/ux/2026-11-18-grid.md #6), and Home's links lead there
    assert!(home.contains(r#"href="/account#keys""#) && !home.contains("Settings → Connect"));
    // a dev account says how it signs in, and its name isn't said twice
    let account = g.page("/account", &ada).await;
    assert!(account.contains("Development sign-in") && account.contains("By handle (ada)"), "{account}");
    assert!(account.contains(r#"<b title="ada">ada</b><small title=""></small>"#), "{account}");
    // the grid's name once in a title; Activate has Home and Account like the other signed-in pages
    for p in [&home, &account, &activate, &g.page("/no/such", "").await] {
        assert!(!p.contains("wisp Grid · wisp Grid"), "{p}");
    }
    assert!(
        activate.contains(r#"<nav class="navt" aria-label="Your grid"><a href="/">Home</a><a href="/account">Account</a></nav>"#),
        "{activate}"
    );
    // one Grid badge everywhere: the landing's look
    assert!(home.contains(r#"<span class="chipx">Grid</span>"#) && home.contains("background:var(--accent-soft)"), "{home}");
    g.done();
}

/// Audit B #10: Open on a sleeping cloud wisp showed nothing for ~1.3 s (the box wakes before the next page starts), and
/// Home's other links had no pending state. Every page link now changes the screen at once (PENDING), and the asleep
/// Open carries what its row says while it wakes.
#[tokio::test(flavor = "multi_thread")]
async fn every_page_link_shows_it_is_going_and_open_says_it_is_waking() {
    let g = rig("pending").await;
    let cookie = g.session("ada");
    let home = g.page("/", &cookie).await;
    // asleep: Open says it's waking, with the words its row shows once it is
    assert!(
        home.contains(r#"<a class="primary" href="/go" data-wake="Waking up · a few seconds: it opens as soon as it’s up">Open"#),
        "{home}"
    );
    // the pending script covers any link that loads a page, not only sign-in links, and Back undoes it
    for want in
        ["closest('a[href]')", "a.dataset.wake", "classList.add('going')", "addEventListener('pageshow'", "a.target!=='_self'", "e.metaKey"]
    {
        assert!(home.contains(want), "PENDING lacks {want}");
    }
    assert!(!home.contains(r#"closest('a[href^="/auth/"]')"#));
    // a dimmed link and a spinning button-like one are styled
    assert!(home.contains(".going{pointer-events:none;opacity:.55;cursor:progress}") && home.contains(".primary.wait>svg"));
    // the account page has it too (its tabs and links)
    let account = g.page("/account", &cookie).await;
    assert!(account.contains("closest('a[href]')"));
    g.done();
}
