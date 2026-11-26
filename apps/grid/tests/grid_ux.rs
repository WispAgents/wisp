//! The grid's pages after the 2026-11-18 UX proposals (docs/ux/2026-11-18-grid.md), through the edge: API keys made and
//! revoked on Account with the session's own form (#6), shared links listed on Home and stopped from there (#5), the
//! way you signed in remembered for the sign-in page (#10), and Home's links into the wisp landing on their place (#3).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

const PAGE: &str = "text/html,application/xhtml+xml";
const FORM: &str = "application/x-www-form-urlencoded";

struct Rig {
    grid: Arc<wisp_grid::edge::Grid>,
    c: reqwest::Client,
    root: String,
    stop: CancellationToken,
    dir: std::path::PathBuf,
}

async fn rig(name: &str) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-ux-{name}-{}", std::process::id()));
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
            max_running_per_user: 4,
            wake_retry_ms: 30_000,
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
        .build()
        .unwrap();
    Rig { grid, c, root: format!("http://wisp.localhost:{port}"), stop, dir }
}

impl Rig {
    fn session(&self, handle: &str) -> String {
        let u = self.grid.accounts.dev_user(handle).unwrap();
        format!("wisp_grid={}", self.grid.accounts.new_session(&u.id).unwrap())
    }
    async fn page(&self, path: &str, cookie: &str) -> String {
        let r = self.c.get(format!("{}{path}", self.root)).header("cookie", cookie).header("accept", PAGE).send().await.unwrap();
        r.text().await.unwrap()
    }
    async fn form(&self, path: &str, cookie: &str, body: &str) -> reqwest::Response {
        self.c
            .post(format!("{}{path}", self.root))
            .header("cookie", cookie)
            .header("content-type", FORM)
            .body(body.to_owned())
            .send()
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

#[tokio::test(flavor = "multi_thread")]
async fn api_keys_are_made_once_and_revoked_from_account() {
    let g = rig("keys").await;
    let ada = g.session("ada");
    let account = g.page("/account", &ada).await;
    assert!(account.contains("No keys yet") && account.contains(r#"action="/keys""#), "{account}");
    let csrf = csrf_in(&account);
    // without the page's token nothing is made
    assert_eq!(g.form("/keys", &ada, "csrf=stale&name=deploy").await.status(), 403);
    // made (QA round 6 #3): post, then a redirect, then the page: the key is said once, from a one-time token in a
    // short-lived cookie (never the key itself), and a reload neither says it again nor makes another
    let r = g.form("/keys", &ada, &format!("csrf={csrf}&name=deploy+bot")).await;
    assert_eq!((r.status().as_u16(), r.headers()["location"].to_str().unwrap()), (303, "/account#keys"));
    let set: Vec<String> = r.headers().get_all("set-cookie").iter().map(|v| v.to_str().unwrap().to_owned()).collect();
    let once = set.iter().find(|c| c.starts_with("wisp_new_key=")).expect("the one-time token").clone();
    assert!(once.contains("HttpOnly") && once.contains("Max-Age=60") && !once.contains("wgk_"), "{once}");
    let token = once.trim_start_matches("wisp_new_key=").split(';').next().unwrap().to_owned();
    let with = format!("{ada}; wisp_new_key={token}");
    let r = g.c.get(format!("{}/account", g.root)).header("cookie", &with).header("accept", PAGE).send().await.unwrap();
    assert_eq!(r.headers()["cache-control"].to_str().unwrap(), "no-store");
    let cleared = r.headers().get_all("set-cookie").iter().any(|v| v.to_str().unwrap().starts_with("wisp_new_key=;"));
    assert!(cleared, "the token is dropped once shown");
    let made = r.text().await.unwrap();
    assert!(made.contains("deploy bot is made. Copy it now: it won’t be shown again."), "{made}");
    let key = made.split(r#"data-copy=""#).find(|s| s.starts_with("wgk_")).unwrap().split('"').next().unwrap().to_owned();
    // it opens the cloud wisp's API (the keys table checks it), and a reload with the same token says nothing
    assert!(g.grid.keys.check(&format!("Bearer {key}")).unwrap().is_some());
    let again = g.page("/account", &with).await;
    assert!(!again.contains(&key) && !again.contains("is made") && again.contains("<b>deploy bot</b>"), "{again}");
    let uid = g.grid.accounts.dev_user("ada").unwrap().id;
    assert_eq!(g.grid.keys.list(&uid).unwrap().len(), 1, "a reload makes no second key");
    // someone else's token shows them nothing
    let bob0 = g.session("bob");
    let r = g.form("/keys", &ada, &format!("csrf={csrf}&name=second")).await;
    let t2 =
        r.headers().get_all("set-cookie").iter().map(|v| v.to_str().unwrap().to_owned()).find(|c| c.starts_with("wisp_new_key=")).unwrap();
    let t2 = t2.trim_start_matches("wisp_new_key=").split(';').next().unwrap().to_owned();
    assert!(!g.page("/account", &format!("{bob0}; wisp_new_key={t2}")).await.contains("wgk_"));
    let id2 = g.grid.keys.list(&uid).unwrap()[0].id.clone();
    assert_eq!(g.form(&format!("/keys/{id2}/revoke"), &ada, &format!("csrf={csrf}")).await.status(), 302);
    // a name it can't take is said on the page
    let r = g.form("/keys", &ada, &format!("csrf={csrf}&name=")).await;
    assert_eq!(r.headers()["location"].to_str().unwrap(), "/account#keys");
    // revoked: refused from now on, and gone from the list
    let id = g.grid.keys.list(&uid).unwrap()[0].id.clone();
    let r = g.form(&format!("/keys/{id}/revoke"), &ada, &format!("csrf={csrf}")).await;
    assert_eq!((r.status().as_u16(), r.headers()["location"].to_str().unwrap()), (302, "/account#keys"));
    assert!(g.grid.keys.check(&format!("Bearer {key}")).unwrap().is_none());
    assert!(g.page("/account", &ada).await.contains("No keys yet"));
    // someone else's key isn't theirs to revoke
    let bob = g.session("bob");
    let bcsrf = csrf_in(&g.page("/account", &bob).await);
    let (v, _) = g.grid.keys.create(&uid, "mine").unwrap();
    let r = g.form(&format!("/keys/{}/revoke", v.id), &bob, &format!("csrf={bcsrf}")).await;
    assert_eq!(r.headers()["location"].to_str().unwrap(), "/account#keys");
    assert_eq!(g.grid.keys.list(&uid).unwrap().len(), 1);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn shared_links_show_on_home_and_stop_from_there() {
    let g = rig("shares").await;
    let ada = g.session("ada");
    let user = g.grid.accounts.dev_user("ada").unwrap();
    let signer = SigningKey::from_bytes(&[7; 32]);
    let body = serde_json::to_vec(&json!({ "spec": { "prompt": "fix it" } })).unwrap();
    let put: wisp_grid::shares::Put = serde_json::from_value(json!({
        "kind": "task", "key": "t-1", "title": "Fix the flaky test", "body": B64.encode(&body),
        "signer": B64.encode(signer.verifying_key().to_bytes()), "sig": B64.encode(signer.sign(&body).to_bytes()),
    }))
    .unwrap();
    let v = g.grid.shares.put(&user, &put).unwrap();
    let id = v["id"].as_str().unwrap().to_owned();
    let home = g.page("/", &ada).await;
    assert!(home.contains("Shared tasks <span class=\"n\">1</span>") && home.contains("Fix the flaky test"), "{home}");
    assert!(home.contains("task · anyone with the link · 0 views"), "{home}");
    let csrf = csrf_in(&home);
    let r = g.form(&format!("/shares/{id}/revoke"), &ada, &format!("csrf={csrf}")).await;
    assert_eq!((r.status().as_u16(), r.headers()["location"].to_str().unwrap()), (302, "/#shares"));
    assert!(!g.page("/", &ada).await.contains("Fix the flaky test"));
    assert_eq!(g.c.get(format!("{}/s/{id}", g.root)).header("accept", PAGE).send().await.unwrap().status(), 410);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_way_you_signed_in_comes_first_next_time() {
    let g = rig("last").await;
    let r = g.c.get(format!("{}/auth/dev/start", g.root)).send().await.unwrap();
    let set: Vec<String> = r.headers().get_all("set-cookie").iter().map(|v| v.to_str().unwrap().to_owned()).collect();
    let last = set.iter().find(|c| c.starts_with("wisp_last_way=")).expect("remembered");
    assert!(last.starts_with("wisp_last_way=dev;") && last.contains("Max-Age=31536000") && last.contains("HttpOnly"), "{last}");
    // one way in (this dev grid): nothing to tell apart, so no tag; the page lands on the way in
    let page = g.page("/signin", "wisp_last_way=dev").await;
    assert!(!page.contains(r#"<span class="last">"#) && page.contains("querySelector('#join .btn')"), "{page}");
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn home_links_land_on_their_place_in_the_wisp() {
    let g = rig("go").await;
    let ada = g.session("ada");
    let home = g.page("/", &ada).await;
    let agents = wisp_grid::pages::wisp_settings("agents");
    assert!(home.contains(&wisp_grid::pages::esc(&agents)), "{home}");
    // /go hands the browser to the cloud wisp with the place kept: the ticket's `r` is the wisp's own URL
    let r = g.c.get(format!("{}{agents}", g.root)).header("cookie", &ada).send().await.unwrap();
    let to = r.headers()["location"].to_str().unwrap().to_owned();
    let r_at = to.split("&r=").nth(1).unwrap();
    assert_eq!(r_at, "%2F%3Fo%3Dviewer%253Ashell-settings.sheet%253A%257B%2522pane%2522%253A%2522agents%2522%257D", "{to}");
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn grid_pages_say_who_is_signed_in() {
    // QA round 6 #4 (docs/ux/2026-11-18-grid.md #7): a page that says one thing, the share pages and the gallery carry
    // Home, Account and you, as Home does, once you're signed in; signed out they don't
    let g = rig("header").await;
    let ada = g.session("ada");
    let nav = r#"<nav class="navt" aria-label="Your grid"><a href="/">Home</a><a href="/account">Account</a></nav>"#;
    for path in ["/nope", "/s/doesnotexist0000", "/gallery"] {
        let page = g.page(path, &ada).await;
        assert!(page.contains(nav) && page.contains(r#"<span class="who">ada</span>"#), "{path}: {page}");
        assert!(!g.page(path, "").await.contains(nav), "{path} signed out");
    }
    g.done();
}

/// Setup audit F17: on a device's sign-in page, "Not you? Use another account" signed out and landed on Home: the code
/// was gone, so the sign-in in wisp couldn't finish. It signs out, then asks for a sign-in that comes back to the code.
#[tokio::test(flavor = "multi_thread")]
async fn use_another_account_comes_back_to_the_device_code() {
    let g = rig("another").await;
    let ada = g.session("ada");
    let code = g.grid.accounts.device_code("studio").unwrap();
    let user_code = code["user_code"].as_str().unwrap().to_owned();
    let page = g.page(&format!("/activate?code={user_code}"), &ada).await;
    assert!(page.contains("Use another account"), "{page}");
    let action = page.split(r#"action="/signout?return="#).nth(1).unwrap().split('"').next().unwrap().replace("&amp;", "&");
    let r = g.form(&format!("/signout?return={action}"), &ada, &format!("csrf={}", csrf_in(&page))).await;
    assert!(r.status().is_redirection(), "{}", r.status());
    let to = r.headers()["location"].to_str().unwrap().to_owned();
    assert!(to.starts_with("/signin?return=") && to.contains(&user_code), "back to the code after signing in: {to}");
    // only a path here: anything else goes Home
    let bea = g.session("bea");
    let page = g.page("/account", &bea).await;
    let r = g.form("/signout?return=%2F%2Fevil.example", &bea, &format!("csrf={}", csrf_in(&page))).await;
    assert_eq!(r.headers()["location"].to_str().unwrap(), "/");
    g.done();
}
