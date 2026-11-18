//! Open on Home (v1 grid/server.js `/go`, `boxEdge`, `computerEdge`; QA round 2 B1): the grid's cookie is host-only,
//! so `/go` carries the sign-in to your cloud wisp or computer as a one-time ticket, which that address spends for a
//! cookie of its own. No box runs here (its binary doesn't exist): a request that gets past the address's sign-in
//! hears the box fail to start (5xx), which is how these tests see it was let in.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::json;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

struct Rig {
    grid: Arc<wisp_grid::edge::Grid>,
    port: u16,
    stop: CancellationToken,
    dir: std::path::PathBuf,
}

async fn rig(name: &str) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-go-{name}-{}", std::process::id()));
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
    Rig { grid, port, stop, dir }
}

impl Rig {
    fn at(&self, label: &str) -> String {
        format!("http://{label}.wisp.localhost:{}", self.port)
    }
    fn root(&self) -> String {
        format!("http://wisp.localhost:{}", self.port)
    }
    fn web(&self, labels: &[&str]) -> reqwest::Client {
        let mut b = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none());
        for l in labels {
            b = b.resolve(&format!("{l}.wisp.localhost"), ([127, 0, 0, 1], self.port).into());
        }
        b.resolve("wisp.localhost", ([127, 0, 0, 1], self.port).into()).build().unwrap()
    }
    fn session(&self, handle: &str) -> String {
        let u = self.grid.accounts.dev_user(handle).unwrap();
        format!("wisp_grid={}", self.grid.accounts.new_session(&u.id).unwrap())
    }
    fn done(self) {
        self.stop.cancel();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn location(r: &reqwest::Response) -> String {
    r.headers().get("location").map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default()
}

fn query(url: &str, k: &str) -> String {
    let u = reqwest::Url::parse(url).unwrap();
    u.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.into_owned()).unwrap_or_default()
}

/// The cookie a response sets, as `name=value`.
fn set_cookie(r: &reqwest::Response) -> String {
    r.headers().get("set-cookie").map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned()).unwrap_or_default()
}

const PAGE: &str = "text/html,application/xhtml+xml";

#[tokio::test(flavor = "multi_thread")]
async fn open_hands_the_sign_in_to_the_cloud_wisp_with_a_one_time_ticket() {
    let g = rig("box").await;
    let web = g.web(&["ada", "bob"]);
    let ada = g.session("ada");
    g.session("bob");

    // /go: a ticket to ada's own address, with the path she asked for
    let r = web.get(format!("{}/go?path=%2Fsettings%3Fpane%3Dcloud", g.root())).header("cookie", &ada).send().await.unwrap();
    assert_eq!(r.status(), 302);
    let to = location(&r);
    assert!(to.starts_with(&format!("{}/_grid/auth?t=wgt_", g.at("ada"))), "{to}");
    assert_eq!(query(&to, "r"), "/settings?pane=cloud");

    // a ticket for ada's wisp is nothing at bob's: no cookie, back to /go for one of his own
    let t = query(&to, "t");
    let r = web.get(format!("{}/_grid/auth?t={t}&r=/", g.at("bob"))).send().await.unwrap();
    assert_eq!((r.status().as_u16(), set_cookie(&r)), (302, String::new()));
    assert_eq!(location(&r), format!("{}/go", g.root()));

    // spent at ada's: her address's own cookie, host-only, then on to the path
    let r = web.get(&to).send().await.unwrap();
    assert_eq!((r.status().as_u16(), location(&r)), (302, "/settings?pane=cloud".to_owned()));
    let raw = r.headers()["set-cookie"].to_str().unwrap().to_owned();
    assert!(raw.starts_with("wisp_box=") && raw.contains("HttpOnly") && raw.contains("SameSite=Lax"), "{raw}");
    assert!(!raw.to_ascii_lowercase().contains("domain="), "host-only: {raw}");
    let cookie = set_cookie(&r);

    // with it, the address lets her in (the box behind it fails to start here: not a 401, not a redirect)
    let r = web.get(format!("{}/api/status", g.at("ada"))).header("cookie", &cookie).send().await.unwrap();
    assert!(r.status().is_server_error(), "let in: {}", r.status());
    // it's ada's address alone: at bob's it's nobody, and the grid's own pages don't take it
    let r = web.get(format!("{}/api/status", g.at("bob"))).header("cookie", &cookie).send().await.unwrap();
    assert_eq!(r.status(), 401);
    let r = web.get(format!("{}/account", g.root())).header("cookie", cookie.replace("wisp_box", "wisp_grid")).send().await.unwrap();
    assert!(location(&r).starts_with("/signin"), "{}", location(&r));

    // a post with it from another page is refused
    let r = web
        .post(format!("{}/api/tasks", g.at("ada")))
        .header("cookie", &cookie)
        .header("origin", "http://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);

    // signing out there ends it
    let r = web.get(format!("{}/_grid/signout", g.at("ada"))).header("cookie", &cookie).send().await.unwrap();
    assert_eq!((r.status().as_u16(), location(&r)), (302, g.root()));
    assert_eq!(web.get(format!("{}/api/status", g.at("ada"))).header("cookie", &cookie).send().await.unwrap().status(), 401);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_replayed_or_expired_ticket_sets_no_cookie() {
    let g = rig("replay").await;
    let web = g.web(&["ada"]);
    let ada = g.session("ada");
    let to = location(&web.get(format!("{}/go", g.root())).header("cookie", &ada).send().await.unwrap());
    assert!(!set_cookie(&web.get(&to).send().await.unwrap()).is_empty(), "first use");
    // replayed: no cookie, back to /go
    let r = web.get(&to).send().await.unwrap();
    assert_eq!((set_cookie(&r), location(&r)), (String::new(), format!("{}/go", g.root())));

    // expired: a ticket past its minute
    let uid = g.grid.accounts.user_by_handle("ada").unwrap().unwrap().id;
    let t = g.grid.accounts.address_ticket(&uid, "ada").unwrap();
    g.grid.db.with(|c| c.execute("UPDATE tickets SET expires_at = 0 WHERE hash = ?1", [wisp_grid::accounts::sha256_hex(&t)])).unwrap();
    let r = web.get(format!("{}/_grid/auth?t={t}&r=/", g.at("ada"))).send().await.unwrap();
    assert_eq!((set_cookie(&r), location(&r)), (String::new(), format!("{}/go", g.root())));
    // no ticket at all, and a forged one
    for q in ["", "?t=wgt_forged&r=/"] {
        assert_eq!(set_cookie(&web.get(format!("{}/_grid/auth{q}", g.at("ada"))).send().await.unwrap()), "");
    }
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn signed_out_at_the_wisp_a_browser_goes_for_a_ticket_and_a_fetch_hears_401() {
    let g = rig("signedout").await;
    let web = g.web(&["ada"]);
    g.session("ada");
    // a page opened in a browser: to /go, back to where it was
    let r = web.get(format!("{}/tasks/1?x=2", g.at("ada"))).header("accept", PAGE).send().await.unwrap();
    assert_eq!(r.status(), 302);
    assert_eq!(location(&r), format!("{}/go?path=%2Ftasks%2F1%3Fx%3D2", g.root()));
    // a fetch: 401, as JSON
    let r = web.get(format!("{}/api/status", g.at("ada"))).send().await.unwrap();
    assert_eq!(r.status(), 401);
    assert_eq!(r.json::<serde_json::Value>().await.unwrap()["error"], "Sign in again.");
    // signed out at the grid as well: /go asks for a sign-in first, then comes back to the same /go
    let r = web.get(format!("{}/go?path=%2Ftasks", g.root())).send().await.unwrap();
    assert_eq!(location(&r), "/signin?return=%2Fgo%3Fpath%3D%252Ftasks");
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn open_on_a_computer_hands_the_sign_in_to_that_computer_only() {
    let g = rig("computer").await;
    let web = g.web(&["ada", "studio--ada"]);
    let ada = g.session("ada");
    let uid = g.grid.accounts.user_by_handle("ada").unwrap().unwrap().id;
    let c = g.grid.accounts.device_code("studio").unwrap();
    g.grid.accounts.decide(c["user_code"].as_str().unwrap(), &uid, true).unwrap();
    let wisp_grid::accounts::Poll::Token { token, user } = g.grid.accounts.poll(c["device_code"].as_str().unwrap()).unwrap() else {
        panic!("no token")
    };
    let (_, dev) = g.grid.accounts.device(&token).unwrap().unwrap();
    let comp = g.grid.accounts.add_computer(&user, &dev, "Studio").unwrap();

    let to = location(&web.get(format!("{}/go?computer={}", g.root(), comp.id)).header("cookie", &ada).send().await.unwrap());
    assert!(to.starts_with(&format!("{}/_grid/auth?t=", g.at("studio--ada"))), "{to}");
    // not spendable at the cloud wisp
    let t = query(&to, "t");
    assert_eq!(set_cookie(&web.get(format!("{}/_grid/auth?t={t}&r=/", g.at("ada"))).send().await.unwrap()), "");
    let r = web.get(&to).send().await.unwrap();
    let cookie = set_cookie(&r);
    assert!(cookie.starts_with("wisp_box="), "{cookie}");
    // offline: on to its waiting page (under /_grid/, never the phone's cached app: QA round 10 Grid #2)
    assert_eq!(location(&r), "/_grid/offline?r=%2F");
    // let in (offline here: 503 from the tunnel, not 401)
    let r = web.get(format!("{}/", g.at("studio--ada"))).header("cookie", &cookie).send().await.unwrap();
    assert_eq!(r.status(), 503);
    // a browser hears it in a page, as v1 did: its waiting page (QA round 10 Grid #2)
    let r = web.get(format!("{}/", g.at("studio--ada"))).header("cookie", &cookie).header("accept", PAGE).send().await.unwrap();
    assert_eq!(location(&r), "/_grid/offline?r=%2F");
    let r = web
        .get(format!("{}/_grid/offline?r=%2F", g.at("studio--ada")))
        .header("cookie", &cookie)
        .header("accept", PAGE)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let page = r.text().await.unwrap();
    assert!(page.contains("Studio is offline"));
    // not a dead end (docs/ux/2026-11-18-grid.md #7): it waits and opens it once it connects, and offers the cloud wisp
    assert!(page.contains("It hasn’t connected yet.") && page.contains("this page opens it as soon as it connects"), "{page}");
    assert!(page.contains(&format!(r#"href="{}/go">Open your cloud wisp instead"#, g.root())), "{page}");
    // signed out there, a browser goes to /go?computer=… for one
    let r = web.get(format!("{}/x", g.at("studio--ada"))).header("accept", PAGE).send().await.unwrap();
    assert_eq!(location(&r), format!("{}/go?computer={}&path=%2Fx", g.root(), comp.id));
    // someone else's computer isn't on offer
    let bob = g.session("bob");
    assert_eq!(web.get(format!("{}/go?computer={}", g.root(), comp.id)).header("cookie", &bob).send().await.unwrap().status(), 404);
    g.done();
}
