//! A page in a box at its own address (ADR-0064, F-page-31, v1 grid/server.js `previewEdge`): `p<port>--<handle>`
//! is its owner's alone, through a one-time link from the box's own address that becomes a sign-in for that
//! address only; a dev server's ports only, never the grid's own; a box asleep stays asleep. No box runs here: a
//! signed-in request that gets as far as the box hears "asleep" (503), which is how these tests see it got past
//! every check.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use wisp_grid::boxhost::BoxHost as _;

struct Rig {
    grid: Arc<wisp_grid::edge::Grid>,
    port: u16,
    stop: CancellationToken,
    dir: std::path::PathBuf,
}

async fn rig(name: &str, previews: bool) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-preview-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let file = json!({ "domain": "wisp.localhost", "listen": { "host": "127.0.0.1", "port": port }, "auth": { "dev": true },
        "hostd": { "previews": previews } });
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
            start_ms: 30_000,
            max_running: 1,
            env: Vec::new(),
            url_of: Arc::new(move |h| url_cfg.instance_url(h)),
            boxd: None,
            previews,
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
    fn web(&self, labels: &[&str]) -> reqwest::Client {
        let mut b = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none());
        for l in labels {
            b = b.resolve(&format!("{l}.wisp.localhost"), ([127, 0, 0, 1], self.port).into());
        }
        b.resolve("wisp.localhost", ([127, 0, 0, 1], self.port).into()).build().unwrap()
    }
    /// The grid's own sign-in for `handle` (dev): `wisp_grid=<value>`.
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

async fn error(r: reqwest::Response) -> (u16, String) {
    let s = r.status().as_u16();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    (s, v["error"].as_str().unwrap_or_default().to_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_box_s_page_is_its_owner_s_alone_through_a_one_time_link_grid_preview_owner_only() {
    let g = rig("owner", true).await;
    let web = g.web(&["ada", "bob", "p5173--ada", "p5173--bob", "p3000--ada"]);
    let (ada, bob) = (g.session("ada"), g.session("bob"));
    let link = format!("{}/_grid/preview?port=5173&path=%2Fsettings%3Ftab%3D1", g.at("ada"));

    // the link: only the box's owner, signed in at its address; never someone else, nobody, or a key
    let r = web.get(&link).header("cookie", &ada).send().await.unwrap();
    assert_eq!((r.status().as_u16(), r.headers()["cache-control"].to_str().unwrap()), (302, "no-store"));
    let to = location(&r);
    assert!(to.starts_with(&format!("{}/_grid/auth?t=wgt_", g.at("p5173--ada"))), "{to}");
    assert_eq!(query(&to, "r"), "/settings?tab=1");
    assert_eq!(error(web.get(&link).header("cookie", &bob).send().await.unwrap()).await.0, 403, "someone else's box");
    assert_eq!(error(web.get(&link).send().await.unwrap()).await.0, 401, "nobody");
    let ada_id = g.grid.accounts.user_by_handle("ada").unwrap().unwrap().id;
    let (_, key) = g.grid.keys.create(&ada_id, "script").unwrap();
    let (s, why) = error(web.get(&link).bearer_auth(&key).send().await.unwrap()).await;
    assert_eq!((s, why.contains("never a preview")), (403, true), "{why}");

    // the ticket: once, at its own label's addresses only (another user's, or after use, it's nothing)
    let t = query(&to, "t");
    let at_bob = format!("{}/_grid/auth?t={t}&r=/", g.at("p5173--bob"));
    assert_eq!(error(web.get(&at_bob).send().await.unwrap()).await.0, 401, "a ticket for ada's label isn't bob's");
    let r = web.get(&to).send().await.unwrap();
    assert_eq!((r.status().as_u16(), location(&r)), (302, "/settings?tab=1".to_owned()));
    let set = r.headers()["set-cookie"].to_str().unwrap().to_owned();
    for part in ["wisp_preview=", "HttpOnly", "SameSite=None", "Secure", "Partitioned"] {
        assert!(set.contains(part), "{part} in {set}");
    }
    assert!(!set.to_ascii_lowercase().contains("domain="), "host-only: {set}");
    assert_eq!(error(web.get(&to).send().await.unwrap()).await.0, 401, "a ticket is used once");
    let pv = set.split(';').next().unwrap().to_owned();

    // signed in there: through to the box (asleep here, so 503, and a preview never wakes it)
    let page = format!("{}/settings", g.at("p5173--ada"));
    let r = web.get(&page).header("cookie", &pv).send().await.unwrap();
    assert_eq!(r.status(), 503);
    assert_eq!(r.json::<Value>().await.unwrap()["asleep"], true);
    assert_eq!(g.grid.boxes.port(&ada_id), None, "still asleep");
    // the same label's other ports too; the grid's own sign-in, or bob's, is nothing there
    assert_eq!(web.get(format!("{}/", g.at("p3000--ada"))).header("cookie", &pv).send().await.unwrap().status(), 503);
    assert_eq!(error(web.get(&page).header("cookie", &ada).send().await.unwrap()).await.0, 401, "the grid's cookie isn't the preview's");
    let bob_pv = format!("wisp_preview={}", &bob["wisp_grid=".len()..]);
    assert_eq!(error(web.get(&page).header("cookie", &bob_pv).send().await.unwrap()).await.0, 401);
    assert_eq!(
        error(web.get(format!("{}/", g.at("p5173--bob"))).header("cookie", &pv).send().await.unwrap()).await.0,
        401,
        "ada's at bob's"
    );

    // ...and the preview's sign-in is never a session anywhere else: not at the box, not at the grid
    let as_grid = format!("wisp_grid={}", &pv["wisp_preview=".len()..]);
    assert_eq!(error(web.get(format!("{}/", g.at("ada"))).header("cookie", &as_grid).send().await.unwrap()).await.0, 401);
    let r = web.get(format!("http://wisp.localhost:{}/account", g.port)).header("cookie", &as_grid).send().await.unwrap();
    assert!(location(&r).starts_with("/signin"), "{}", location(&r));

    // a page elsewhere can't post to it with the cookie; the page itself can
    let r = web.post(&page).header("cookie", &pv).header("origin", "http://evil.example").send().await.unwrap();
    assert_eq!(error(r).await, (403, "forbidden origin".to_owned()));
    let r = web.post(&page).header("cookie", &pv).header("origin", g.at("p5173--ada")).send().await.unwrap();
    assert_eq!(r.status(), 503);

    // signed out: a browser goes to the box's address for a link (back to the same page); a fetch hears 401
    let r = web.get(format!("{page}?x=1")).header("accept", "text/html").send().await.unwrap();
    assert_eq!(r.status(), 302);
    let back = location(&r);
    assert!(back.starts_with(&format!("{}/_grid/preview?port=5173&path=", g.at("ada"))), "{back}");
    assert_eq!(query(&back, "path"), "/settings?x=1");
    assert_eq!(error(web.get(&page).send().await.unwrap()).await.0, 401);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn only_a_dev_server_s_ports_are_shown_never_the_grid_s_own_grid_preview_port_allowlist() {
    let g = rig("ports", true).await;
    let own = format!("p{}--ada", g.port);
    let web = g.web(&["ada", "p80--ada", "p1023--ada", "p1024--ada", own.as_str()]);
    let ada = g.session("ada");
    for port in ["22", "80", "1023", "0", "65536", "99999", "abc", "", "5173x", &g.port.to_string()] {
        let r = web.get(format!("{}/_grid/preview?port={port}", g.at("ada"))).header("cookie", &ada).send().await.unwrap();
        assert_eq!(error(r).await, (400, "No preview for that port here.".to_owned()), "port {port:?}");
    }
    let r = web.get(format!("{}/_grid/preview?port=1024", g.at("ada"))).header("cookie", &ada).send().await.unwrap();
    assert_eq!(r.status(), 302, "1024 is the first port shown");
    // the addresses themselves: a system port, or the grid's own, is no page (before any sign-in is read)
    for label in ["p80--ada", "p1023--ada", own.as_str()] {
        assert_eq!(error(web.get(format!("{}/", g.at(label))).send().await.unwrap()).await.0, 404, "{label}");
    }
    assert_eq!(error(web.get(format!("{}/", g.at("p1024--ada"))).send().await.unwrap()).await.0, 401, "shown, once signed in");
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn with_previews_off_there_is_no_preview_address_and_no_link_grid_preview_off() {
    let g = rig("off", false).await;
    let web = g.web(&["ada", "p5173--ada"]);
    let ada = g.session("ada");
    let r = web.get(format!("{}/_grid/preview?port=5173", g.at("ada"))).header("cookie", &ada).send().await.unwrap();
    assert_eq!(error(r).await.0, 400);
    // not a preview address: read as a handle nobody has
    let (s, why) = error(web.get(format!("{}/", g.at("p5173--ada"))).send().await.unwrap()).await;
    assert_eq!((s, why.as_str()), (404, "There's nobody with that handle here."));
    g.done();
}

#[test]
fn a_box_is_told_its_preview_template_grid_preview_template() {
    assert_eq!(wisp_grid::hostd::preview_template("https://ada.grid.example"), "https://p{port}--ada.grid.example");
    assert_eq!(wisp_grid::hostd::preview_template("http://ada.wisp.localhost:8080/"), "http://p{port}--ada.wisp.localhost:8080");
    assert_eq!(wisp_grid::hostd::preview_template("nonsense"), "");
}
