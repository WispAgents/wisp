//! What the grid lets through into a box, and what it lets back out (audit grid-r1):
//!
//! - a client's own copy of one of the grid's marks (`trust::MARKS`: x-wisp-preview, x-wisp-route, x-wisp-upgrade,
//!   x-wisp-tunnel, x-wisp-proxy-*, x-forwarded-*) never reaches the box's wisp, at a user's box or a sandbox's. The
//!   box's wisp believes x-wisp-preview from behind its proxy secret and passes the request to that port on its
//!   loopback, so before this `x-wisp-preview: 22` reached the box's sshd;
//! - a box's answer never sets a `Domain=` cookie, which would reach the grid's other names (cookie tossing);
//! - a write to the grid's API with its session cookie comes from the grid's own origin, or is refused: a published
//!   app is same-site, so its page could post a form there;
//! - a box's secret never reaches the teams API or the list of computers;
//! - a grid cookie sent twice (a sibling page's `Domain=` copy beside the grid's own) signs nobody in.
//!
//! The box is this test binary: hostd runs `<bin> serve` with `WISP_BOX=1`, so `fake_box_serve` (the only test here
//! whose name holds "serve") answers as a box's wisp would. Run normally, it does nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine as _;
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;
use wisp_grid::accounts::Poll;
use wisp_grid::boxhost::BoxHost as _;

/// The stand-in box: answers anything with the headers it was sent (`{"headers": {name: value}}`), and sets two
/// cookies, one host-only and one for the whole domain.
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
        let (mut got, mut len) = (Map::new(), 0usize);
        let mut h = String::new();
        while r.read_line(&mut h).is_ok_and(|n| n > 2) {
            if let Some((k, v)) = h.trim_end().split_once(':') {
                let k = k.trim().to_ascii_lowercase();
                if k == "content-length" {
                    len = v.trim().parse().unwrap_or(0);
                }
                got.insert(k, json!(v.trim()));
            }
            h.clear();
        }
        let mut body = vec![0; len];
        let _ = r.read_exact(&mut body);
        let out = json!({ "headers": got }).to_string();
        let mut s = r.into_inner();
        let _ = write!(
            s,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nset-cookie: host=1; Path=/; HttpOnly\r\nset-cookie: tossed=1; Domain=wisp.localhost; Path=/\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}",
            out.len()
        );
    }
}

struct Rig {
    grid: Arc<wisp_grid::edge::Grid>,
    port: u16,
    stop: CancellationToken,
    dir: PathBuf,
    web: reqwest::Client,
}

async fn rig(name: &str) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-marks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // a master key, so the grid keeps sandboxes and their env
    let secrets = dir.join("secrets");
    std::fs::create_dir_all(&secrets).unwrap();
    let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
    let master = secrets.join("master.json");
    std::fs::write(&master, json!({ "current": "k1", "keys": { "k1": key } }).to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&master, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let (grid, port, stop) = wisp_grid::edge::dev(&dir, 0, std::env::current_exe().unwrap(), Vec::new(), 600_000).await.unwrap();
    let web =
        reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).dns_resolver(Arc::new(Local)).build().unwrap();
    Rig { grid, port, stop, dir, web }
}

/// Every name under wisp.localhost is this machine.
struct Local;

impl reqwest::dns::Resolve for Local {
    fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async { Ok(Box::new(std::iter::once(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))) as reqwest::dns::Addrs) })
    }
}

impl Rig {
    fn at(&self, label: &str) -> String {
        format!("http://{label}.wisp.localhost:{}", self.port)
    }
    fn root(&self) -> String {
        self.grid.cfg.public_url.clone()
    }
    fn uid(&self, handle: &str) -> String {
        self.grid.accounts.dev_user(handle).unwrap().id
    }
    fn session(&self, handle: &str) -> String {
        let u = self.grid.accounts.dev_user(handle).unwrap();
        format!("wisp_grid={}", self.grid.accounts.new_session(&u.id).unwrap())
    }
    fn device(&self, handle: &str) -> String {
        let a = &self.grid.accounts;
        let u = a.dev_user(handle).unwrap();
        let c = a.device_code("laptop").unwrap();
        a.decide(c["user_code"].as_str().unwrap(), &u.id, true).unwrap();
        match a.poll(c["device_code"].as_str().unwrap()).unwrap() {
            Poll::Token { token, .. } => token,
            other => panic!("{other:?}"),
        }
    }
    fn done(self) {
        self.stop.cancel();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The headers the box was sent, and the cookies its answer set.
async fn through(r: reqwest::RequestBuilder) -> (Map<String, Value>, Vec<String>) {
    let r = r.send().await.unwrap();
    assert_eq!(r.status(), 200);
    let set: Vec<String> = r.headers().get_all("set-cookie").iter().map(|v| v.to_str().unwrap().to_owned()).collect();
    let v: Value = r.json().await.unwrap();
    (v["headers"].as_object().unwrap().clone(), set)
}

const CLIENT_MARKS: [(&str, &str); 9] = [
    ("x-wisp-preview", "22"),
    ("x-wisp-route", "grid"),
    ("x-wisp-upgrade", "websocket"),
    ("x-wisp-tunnel", "1"),
    ("x-wisp-proxy-via", "inbox"),
    ("x-wisp-proxy-secret", "guess"),
    ("x-wisp-proxy-anything", "1"),
    ("x-forwarded-for", "10.0.0.1"),
    ("x-wisp-proxy-client-ip", "6.6.6.6"),
];

fn with_marks(mut r: reqwest::RequestBuilder, preview: &str) -> reqwest::RequestBuilder {
    for (k, v) in CLIENT_MARKS {
        r = r.header(k, if k == "x-wisp-preview" { preview } else { v });
    }
    r
}

fn no_client_marks(got: &Map<String, Value>, secret: &str) {
    for (k, _) in CLIENT_MARKS {
        if k == "x-wisp-proxy-secret" || k == "x-wisp-proxy-client-ip" {
            continue;
        }
        assert!(!got.contains_key(k), "{k} reached the box: {got:?}");
    }
    // the grid's own marks are still there, and its secret is the box's, never the client's guess
    assert_eq!(got["x-wisp-proxy-secret"], json!(secret));
    // who is asking is the address Caddy put last in X-Forwarded-For (here the test's), never the client's claim
    assert_eq!(got["x-wisp-proxy-client-ip"], json!("10.0.0.1"), "{got:?}");
    assert!(got.contains_key("x-forwarded-host"), "{got:?}");
    assert!(!got.contains_key("x-wisp-proxy-anything"));
}

fn host_only(set: &[String]) {
    assert!(set.iter().any(|c| c.starts_with("host=1")), "the box's own cookie: {set:?}");
    assert!(!set.iter().any(|c| c.to_ascii_lowercase().contains("domain=")), "a Domain= cookie got out: {set:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_clients_marks_never_reach_a_box_and_its_domain_cookies_never_leave_it() {
    let g = rig("box").await;
    let token = g.device("ada");
    let ada = g.uid("ada");
    for preview in ["22", "65000"] {
        let (got, set) = through(with_marks(g.web.get(format!("{}/api/x", g.at("ada"))).bearer_auth(&token), preview)).await;
        no_client_marks(&got, &g.grid.boxes.secret(&ada).unwrap());
        // the box's wisp answered at its own port, not the one the client named
        assert_eq!(got["host"], json!(format!("127.0.0.1:{}", g.grid.boxes.port(&ada).unwrap())));
        host_only(&set);
    }
    // an API key's request still carries the grid's own via mark, and only that
    let (_, key) = g.grid.keys.create(&ada, "script").unwrap();
    let (got, _) = through(with_marks(g.web.get(format!("{}/api/x", g.at("ada"))).bearer_auth(&key), "22")).await;
    assert_eq!(got["x-wisp-proxy-via"], json!("key"));
    assert!(!got.contains_key("x-wisp-preview"));

    // a sandbox, with its own token
    let r = g.web.post(format!("{}/api/sandboxes", g.root())).bearer_auth(&token).json(&json!({ "name": "sb" })).send().await.unwrap();
    assert_eq!(r.status(), 201);
    let sb: Value = r.json().await.unwrap();
    let label = sb["url"].as_str().unwrap().split("://").nth(1).unwrap().split('.').next().unwrap().to_owned();
    let sb_token = sb["token"].as_str().unwrap();
    let (got, set) = through(with_marks(g.web.get(format!("{}/api/x", g.at(&label))).bearer_auth(sb_token), "22")).await;
    let secret = g.grid.boxes.secret(&format!("{ada}-sb-{}", sb["id"].as_str().unwrap())).unwrap();
    no_client_marks(&got, &secret);
    host_only(&set);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cookie_write_to_the_grids_api_from_another_origin_is_refused() {
    let g = rig("origin").await;
    let token = g.device("ada");
    let ada = g.uid("ada");
    g.grid.accounts.dev_user("mallory").unwrap();
    let cookie = g.session("ada");
    let form = "application/x-www-form-urlencoded";
    let app = g.at("x--app--ada");
    let post = |path: &str, origin: Option<&str>, body: &str| {
        let mut r = g.web.post(format!("{}{path}", g.root())).header("cookie", &cookie).header("content-type", form).body(body.to_owned());
        if let Some(o) = origin {
            r = r.header("origin", o);
        }
        r.send()
    };
    let r = g.web.post(format!("{}/api/teams", g.root())).bearer_auth(&token).json(&json!({ "slug": "crew" })).send().await.unwrap();
    assert_eq!(r.status(), 200);

    for origin in [Some(app.as_str()), None] {
        let r = post("/api/sandboxes/env", origin, "key=HTTPS_PROXY&value=http://evil.example").await.unwrap();
        assert_eq!(r.status(), 403, "{origin:?}");
        assert!(g.grid.sandboxes.env_keys(&ada).is_empty(), "{origin:?}");
        let r = post("/api/teams/crew/members", origin, "handle=mallory&role=admin").await.unwrap();
        assert_eq!(r.status(), 403, "{origin:?}");
        assert_eq!(g.grid.teams.members("crew").unwrap().len(), 1, "{origin:?}");
    }
    // the grid's own pages (a same-origin fetch sends its Origin) still write
    let own = g.root();
    assert_eq!(post("/api/sandboxes/env", Some(&own), "key=HTTPS_PROXY&value=x").await.unwrap().status(), 200);
    assert_eq!(g.grid.sandboxes.env_keys(&ada), vec!["HTTPS_PROXY".to_owned()]);
    assert_eq!(post("/api/teams/crew/members", Some(&own), "handle=mallory&role=member").await.unwrap().status(), 200);
    assert_eq!(g.grid.teams.members("crew").unwrap().len(), 2);
    // a device token has no Origin to check, and needs none
    let r = g.web.delete(format!("{}/api/teams/crew/members/mallory", g.root())).bearer_auth(&token).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let r = g
        .web
        .post(format!("{}/api/sandboxes/env", g.root()))
        .bearer_auth(&token)
        .json(&json!({ "key": "B", "value": "1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    // a read with the cookie, from anywhere, is a read
    assert_eq!(g.web.get(format!("{}/api/teams", g.root())).header("cookie", &cookie).send().await.unwrap().status(), 200);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_boxs_secret_reaches_neither_teams_nor_computers() {
    let g = rig("boxcred").await;
    let token = g.device("ada");
    let ada = g.uid("ada");
    // wake the box, so its secret is a running box's
    through(g.web.get(format!("{}/api/x", g.at("ada"))).bearer_auth(&token)).await;
    let as_box = format!("Box ada {}", g.grid.boxes.secret(&ada).unwrap());
    let r = g.web.post(format!("{}/api/teams", g.root())).bearer_auth(&token).json(&json!({ "slug": "crew" })).send().await.unwrap();
    assert_eq!(r.status(), 200);
    g.grid.accounts.dev_user("mallory").unwrap();

    let calls: [(reqwest::Method, &str, Value); 4] = [
        (reqwest::Method::GET, "/api/teams", Value::Null),
        (reqwest::Method::POST, "/api/teams", json!({ "slug": "boxcrew" })),
        (reqwest::Method::POST, "/api/teams/crew/members", json!({ "handle": "mallory", "role": "admin" })),
        (reqwest::Method::GET, "/api/computers", Value::Null),
    ];
    for (m, path, body) in &calls {
        let mut r = g.web.request(m.clone(), format!("{}{path}", g.root())).header("authorization", &as_box);
        if !body.is_null() {
            r = r.json(body);
        }
        let r = r.send().await.unwrap();
        assert_eq!(r.status(), 403, "{m} {path}");
    }
    assert_eq!(g.grid.teams.members("crew").unwrap().len(), 1);
    assert_eq!(g.grid.teams.of(&ada).unwrap().len(), 1, "no team made by the box");
    // a person's own, with a device token or a session, still is
    let cookie = g.session("ada");
    for path in ["/api/teams", "/api/computers"] {
        assert_eq!(g.web.get(format!("{}{path}", g.root())).bearer_auth(&token).send().await.unwrap().status(), 200, "{path}");
        assert_eq!(g.web.get(format!("{}{path}", g.root())).header("cookie", &cookie).send().await.unwrap().status(), 200, "{path}");
    }
    let r = g
        .web
        .post(format!("{}/api/teams/crew/members", g.root()))
        .bearer_auth(&token)
        .json(&json!({ "handle": "mallory", "role": "member" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_grid_cookie_sent_twice_signs_nobody_in() {
    let g = rig("twice").await;
    let (ada, mallory) = (g.session("ada"), g.session("mallory"));
    let get = |c: String| g.web.get(format!("{}/api/teams", g.root())).header("cookie", c).send();
    assert_eq!(get(ada.clone()).await.unwrap().status(), 200);
    // mallory's copy, tossed for the whole domain from a sibling page, beside ada's own: signed out, never mallory
    assert_eq!(get(format!("{mallory}; {ada}")).await.unwrap().status(), 401);
    assert_eq!(get(format!("{ada}; {mallory}")).await.unwrap().status(), 401);
    g.done();
}
