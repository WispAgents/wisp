//! A team hub's GitHub App through the real edge (ADR-0148 §8e), against a stand-in GitHub: connecting takes the
//! installation only when the person who installed it can see it, a hub's box gets an hour's token for its team (and
//! nothing for a team that hasn't connected), GitHub's signed webhooks go on to the hub's triggers' routes (a forged or
//! unsigned one doesn't), and an uninstall unties the team.
//!
//! The boxes are this test binary (`fake_box_serve`, as in hubs.rs): it says what it was sent.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::type_complexity
)]

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use hmac::Mac as _;
use rsa::pkcs1::EncodeRsaPrivateKey as _;
use serde_json::{Map, Value, json};

#[test]
fn fake_box_serve() {
    if std::env::var("WISP_BOX").as_deref() != Ok("1") {
        return;
    }
    let port: u16 = std::env::var("WISP_PORT").unwrap().parse().unwrap();
    let l = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    let bound = l.local_addr().unwrap().port();
    let home = PathBuf::from(std::env::var("WISP_HOME").unwrap());
    let file = json!({ "url": format!("http://127.0.0.1:{bound}"), "port": bound, "pid": std::process::id() });
    std::fs::write(home.join("server.json"), file.to_string()).unwrap();
    let env: Map<String, Value> =
        ["GH_TOKEN", "GITHUB_TOKEN"].iter().filter_map(|k| std::env::var(k).ok().map(|v| ((*k).to_owned(), json!(v)))).collect();
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
        let out =
            json!({ "ok": true, "line": line.trim(), "headers": got, "env": env, "body": String::from_utf8_lossy(&body) }).to_string();
        let mut s = r.into_inner();
        let _ = write!(
            s,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}",
            out.len()
        );
    }
}

const N: &str = "\
    cda108a9b580f5cd121e25765ffc6b998523fb1f5b91625c953e645669cf1a711c8beef47e9bf093f37bcbe323445f233de2\
    ea930ae1377de227701d02cdd2382adb549a3486e33f7d51e57ca36b8e5317b0e2d5c7459218cd4b620647acaa87dc01f1ae\
    712161b646fc6ab80e08d4edd0c14951dc182b7e33886627cb2cefbec3a3b0efa4c9df928fd955a46befbf385d1fa2cd00f3\
    99e60060170ea57929df414d9d0c43e445f1abe841d61d9033c8a48bb814038eb1d49eb62e2ffcf10067f29484cc23873429\
    541c3a691b69df67e68d136fc121aa8ba4977048fbffbc37c5f4d5d48b8ecd7d60203e7f68a050a08c5d144ce52a319f46e3\
    b501d6aa9cd1";
const D: &str = "\
    018cfe99fb0cb12dddfe5cb577155a016323e993abb6ddb94985da2f5da66a72626efabb1cce53c2e5e1efe07366635a4a6f\
    f0e9717a4fd712f2cdb16651438dd962e4913226b745860e2d4f22935fd4281855299c125dfef2636866983948e2f3e98ed7\
    4c329e8047aa452ac4ae53dec44584e64964e4451421e9f5505eb32327741f4a085ddae1ac20f344150c6c8139d53628be55\
    199d1d3b8f092f54244354cf48015da5b0f178663653226c7c89386e7827a936552b4f2baa354e773dfc125e3fe2903f85a9\
    59f70823eff45df38a48ddcd92fd3af20e14eb16c39e9923fc0db4e85973c0babde957c6a413a146ef85748bcc4c55c543cf\
    1c07365a5401";
const P: &str = "\
    e6740fc5561a8df0ce34869d68a21c44c4354255d3fc838f5d0c9abd55dc1068e725807e4b4c6f04370d2b6a4376ab7afef5\
    dad2d6cbfc5ae7a40cff7fc738809414a5b290146cab3e03ecf56d6c7bd1a20ec9449f5c2c1842f8bf9a5f30b3c82d08fe3a\
    6e351c69b2ed49247fe40ceb614f4d7fe8b67ad6172fe94714ac8bd1";
const Q: &str = "\
    e46c7e40a0d6d0633536cc296c71fecfaef608f444398b06d2a12b63e5e9310b18f7218749c0d73b7d91088411331007bd50\
    27838ac5e0c7e14c341c11a97b9a02c8c5aff92eb6fbc135e6a74bf03ad892c0cd3e03f5f4ea3cfcffd5bd61bd28ecdc52d8\
    bb03e036b1f23f0af60fdc57649a7f0fe218c6eb96d73b0d901e4101";

/// GitHub as the App sees it: the installer's OAuth, their installations (77 only), installation 77 on `acme-org`,
/// and its tokens. Each request is kept: `(method path, authorization)`.
fn fake_github() -> (String, Arc<Mutex<Vec<(String, String)>>>) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s);
            let mut line = String::new();
            let _ = r.read_line(&mut line);
            let (mut auth, mut len) = (String::new(), 0usize);
            let mut h = String::new();
            while r.read_line(&mut h).is_ok_and(|n| n > 2) {
                if let Some((k, v)) = h.trim_end().split_once(':') {
                    match k.trim().to_ascii_lowercase().as_str() {
                        "authorization" => v.trim().clone_into(&mut auth),
                        "content-length" => len = v.trim().parse().unwrap_or(0),
                        _ => {}
                    }
                }
                h.clear();
            }
            let mut body = vec![0; len];
            let _ = r.read_exact(&mut body);
            let what: Vec<&str> = line.split_whitespace().take(2).collect();
            let what = what.join(" ");
            s2.lock().unwrap().push((what.clone(), auth.clone()));
            let jwt = auth.strip_prefix("Bearer ").is_some_and(|t| t.split('.').count() == 3);
            let (status, out) = match what.split('?').next().unwrap_or_default() {
                "POST /login/oauth/access_token" if String::from_utf8_lossy(&body).contains("\"code\":\"good\"") => {
                    (200, json!({ "access_token": "utok" }))
                }
                "POST /login/oauth/access_token" => (200, json!({ "error": "bad_verification_code" })),
                "GET /user/installations" if auth == "Bearer utok" => (200, json!({ "installations": [{ "id": 77 }] })),
                "GET /app/installations/77" if jwt => (200, json!({ "id": 77, "account": { "login": "acme-org" } })),
                "POST /app/installations/77/access_tokens" if jwt => {
                    (201, json!({ "token": "ghs_app77", "expires_at": "2026-11-28T10:00:00Z" }))
                }
                _ => (404, json!({ "message": "Not Found" })),
            };
            let out = out.to_string();
            let mut s = r.into_inner();
            let _ = write!(
                s,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}",
                out.len()
            );
        }
    });
    (url, seen)
}

struct Local;

impl reqwest::dns::Resolve for Local {
    fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async { Ok(Box::new(std::iter::once(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))) as reqwest::dns::Addrs) })
    }
}

fn signed(body: &str) -> String {
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(b"whsec").unwrap();
    mac.update(body.as_bytes());
    let bytes = mac.finalize().into_bytes();
    format!("sha256={}", wisp_grid::secrets::hex(&bytes))
}

async fn send(r: reqwest::RequestBuilder) -> (u16, Value) {
    let r = r.send().await.unwrap();
    let s = r.status().as_u16();
    (s, r.json::<Value>().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_team_connects_its_github_app_and_its_hub_gets_tokens_and_webhooks() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-hub-github-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("secrets")).unwrap();
    let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
    let master = dir.join("secrets/master.json");
    std::fs::write(&master, json!({ "current": "k1", "keys": { "k1": key } }).to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&master, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let big = |h: &str| rsa::BigUint::parse_bytes(h.as_bytes(), 16).unwrap();
    let pem = rsa::RsaPrivateKey::from_components(big(N), big("10001"), big(D), vec![big(P), big(Q)])
        .unwrap()
        .to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
        .unwrap();
    for (f, v) in [("app.pem", pem.as_str()), ("client", "csecret"), ("webhook", "whsec")] {
        std::fs::write(dir.join(f), v).unwrap();
    }
    let (gh, seen) = fake_github();
    let p = |f: &str| dir.join(f).display().to_string();
    let mut more = Map::new();
    more.insert(
        "githubApp".into(),
        json!({ "id": "42", "slug": "wisp-test", "clientId": "Iv1", "clientSecretFile": p("client"), "privateKeyFile": p("app.pem"),
                "webhookSecretFile": p("webhook"), "api": gh, "web": gh }),
    );
    let (grid, port, stop) = wisp_grid::edge::dev_with(&dir, 0, std::env::current_exe().unwrap(), Vec::new(), 600_000, more).await.unwrap();
    let web =
        reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).dns_resolver(Arc::new(Local)).build().unwrap();
    let api = |path: &str| format!("{}{path}", grid.cfg.public_url);
    let a = &grid.accounts;
    let ada = a.dev_user("ada").unwrap();
    let bo = a.dev_user("bo").unwrap();
    let cookie = |u: &wisp_grid::rows::UserRow| format!("wisp_grid={}", a.new_session(&u.id).unwrap());
    let (ada_c, bo_c) = (cookie(&ada), cookie(&bo));
    grid.teams.create(&ada, "acme", "Acme").unwrap();
    grid.teams.add(&ada, "acme", "bo", "member").unwrap();

    // not connected: the grid has an App, the team none, and its box gets no token
    let (s, v) = send(web.get(api("/api/teams/acme/github")).header("cookie", &ada_c)).await;
    assert_eq!((s, v["app"]["slug"].clone(), v["connected"].clone()), (200, json!("wisp-test"), Value::Null), "{v}");
    // only an owner or admin starts connecting
    assert_eq!(
        send(web.post(api("/api/teams/acme/github")).header("cookie", &bo_c).header("origin", grid.cfg.public_url.clone())).await.0,
        403
    );
    let (s, v) = send(web.post(api("/api/teams/acme/github")).header("cookie", &ada_c).header("origin", grid.cfg.public_url.clone())).await;
    assert_eq!(s, 200, "{v}");
    let url = v["url"].as_str().unwrap();
    assert!(url.starts_with(&format!("{gh}/apps/wisp-test/installations/new?state=ghs_")), "{url}");
    let state = url.rsplit('=').next().unwrap().to_owned();
    let setup = |q: String, c: &str| web.get(api(&format!("/github/setup?{q}"))).header("cookie", c.to_owned()).send();
    let flash = |r: &reqwest::Response| r.headers().get("location").map(|l| l.to_str().unwrap().to_owned()).unwrap_or_default();

    // someone else can't finish it, and a used or made-up state is refused
    let r = setup(format!("installation_id=77&code=good&setup_action=install&state={state}"), &bo_c).await.unwrap();
    assert!(flash(&r).starts_with("/teams/acme"), "{r:?}");
    assert!(grid.teams.github_of(&grid.teams.hub("acme").unwrap().unwrap().team_id).unwrap().is_none());
    // (the state went with that try: begin again)
    let (_, v) = send(web.post(api("/api/teams/acme/github")).header("cookie", &ada_c).header("origin", grid.cfg.public_url.clone())).await;
    let state = v["url"].as_str().unwrap().rsplit('=').next().unwrap().to_owned();
    // an installation the installer can't see is never tied, whatever the URL says
    let r = setup(format!("installation_id=78&code=good&setup_action=install&state={state}"), &ada_c).await.unwrap();
    assert_eq!(r.status().as_u16(), 302);
    let team = grid.teams.hub("acme").unwrap().unwrap().team_id;
    assert!(grid.teams.github_of(&team).unwrap().is_none(), "78 isn't ada's");
    let (_, v) = send(web.post(api("/api/teams/acme/github")).header("cookie", &ada_c).header("origin", grid.cfg.public_url.clone())).await;
    let state = v["url"].as_str().unwrap().rsplit('=').next().unwrap().to_owned();
    let r = setup(format!("installation_id=77&code=good&setup_action=install&state={state}"), &ada_c).await.unwrap();
    assert_eq!(r.status().as_u16(), 302, "{r:?}");
    let c = grid.teams.github_of(&team).unwrap().unwrap();
    assert_eq!((c.installation_id, c.account.as_str(), c.connected_by.as_str()), (77, "acme-org", "ada"));
    assert!(setup(format!("installation_id=77&code=good&state={state}"), &ada_c).await.unwrap().status().as_u16() == 302, "a state once");
    let (_, v) = send(web.get(api("/api/teams/acme/github")).header("cookie", &bo_c)).await;
    assert_eq!(v["connected"]["account"], "acme-org", "{v}");

    // a signed webhook goes on to the hub (waking it), to the route of the plugin that makes its events
    let issue = json!({ "action": "opened", "installation": { "id": 77 }, "repository": { "full_name": "acme-org/app" },
                        "issue": { "number": 3, "title": "slug keeps a dash" } })
    .to_string();
    let hook = |body: &str, sig: &str, event: &str| {
        web.post(api("/github/webhook"))
            .header("x-github-event", event)
            .header("x-hub-signature-256", sig)
            .header("x-github-delivery", "d1")
            .body(body.to_owned())
    };
    assert_eq!(send(hook(&issue, "sha256=00", "issues")).await.0, 401, "not the App's signature");
    let (s, v) = send(hook(&issue, &signed(&issue), "issues")).await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["line"], "POST /api/issues/webhook HTTP/1.1", "{v}");
    assert_eq!(
        (v["headers"]["x-wisp-proxy-via"].clone(), v["headers"]["x-github-event"].clone()),
        (json!("github"), json!("issues")),
        "{v}"
    );
    assert!(v["body"].as_str().unwrap().contains("slug keeps a dash"));
    let pr = json!({ "action": "review_requested", "installation": { "id": 77 }, "pull_request": {} }).to_string();
    assert_eq!(send(hook(&pr, &signed(&pr), "pull_request")).await.1["line"], "POST /api/github/webhook HTTP/1.1");
    // anything else isn't worth a wake
    let push = json!({ "installation": { "id": 77 } }).to_string();
    assert_eq!(send(hook(&push, &signed(&push), "push")).await.0, 202);

    // the hub's box (up now) asks for its token as itself; a person or another box can't
    let hub = grid.teams.hub("acme").unwrap().unwrap();
    let secret = grid.boxes.secret(&hub.user_id).expect("the hub's box is up");
    let (s, v) = send(web.get(api("/api/github/token")).header("authorization", format!("Box acme {secret}"))).await;
    assert_eq!((s, v["token"].clone(), v["account"].clone()), (200, json!("ghs_app77"), json!("acme-org")), "{v}");
    // twice: one mint (kept until near its hour's end)
    send(web.get(api("/api/github/token")).header("authorization", format!("Box acme {secret}"))).await;
    let mints = seen.lock().unwrap().iter().filter(|(w, _)| w == "POST /app/installations/77/access_tokens").count();
    assert_eq!(mints, 1);
    assert_eq!(send(web.get(api("/api/github/token")).header("cookie", &ada_c)).await.0, 401);
    // the hub, restarted with GitHub connected, has no GH_TOKEN of the team's in its env (its own renews)
    send(
        web.put(api("/api/teams/acme/secrets"))
            .header("cookie", &ada_c)
            .header("origin", grid.cfg.public_url.clone())
            .json(&json!({ "key": "GH_TOKEN", "value": "ghp_team" })),
    )
    .await;
    send(web.post(api("/api/teams/acme/hub/restart")).header("cookie", &ada_c).header("origin", grid.cfg.public_url.clone())).await;
    let (_, v) = send(hook(&issue, &signed(&issue), "issues")).await;
    assert_eq!(v["env"], json!({}), "{v}");

    // GitHub removes the App: the team is untied, and its hub gets no token
    let gone = json!({ "action": "deleted", "installation": { "id": 77 } }).to_string();
    assert_eq!(send(hook(&gone, &signed(&gone), "installation")).await.0, 200);
    assert!(grid.teams.github_of(&team).unwrap().is_none());
    let secret = grid.boxes.secret(&hub.user_id).unwrap();
    assert_eq!(send(web.get(api("/api/github/token")).header("authorization", format!("Box acme {secret}"))).await.0, 404);
    let _ = port;
    stop.cancel();
    let _ = std::fs::remove_dir_all(&dir);
}
