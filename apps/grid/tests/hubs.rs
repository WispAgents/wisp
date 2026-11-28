//! A team's hub through the real edge (ADR-0147): made with its team, opened by the team's people by role (and what the
//! hub's wisp hears of each), invite links, deploy keys that open their own hub only, deploys kept in a history, the
//! hub's secrets in its box's environment, and the hub gone with its team.
//!
//! The box is this test binary: hostd runs `<bin> serve` with `WISP_BOX=1`, so `fake_box_serve` (the only test here
//! whose name holds "serve") answers as a box's wisp would: the headers it was sent, a few env vars, and for a
//! `resources.apply` an answer that created two resources. Run normally, it does nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines, clippy::many_single_char_names)]

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine as _;
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;
use wisp_grid::accounts::Poll;

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
        ["GH_TOKEN", "WISP_SANDBOX"].iter().filter_map(|k| std::env::var(k).ok().map(|v| ((*k).to_owned(), json!(v)))).collect();
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
        let sent: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let out = if sent["cmd"] == "resources.apply" {
            let ops = [json!({ "op": "create", "kind": "trigger", "name": "a" }), json!({ "op": "create", "kind": "crew", "name": "b" })];
            json!({ "ok": true, "seq": 1, "result": { "plan": { "ops": ops, "unchanged": 0 }, "applied": 2 } })
        } else {
            json!({ "ok": true, "headers": got, "env": env })
        };
        let out = out.to_string();
        let mut s = r.into_inner();
        let _ = write!(
            s,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}",
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

/// Every name under wisp.localhost is this machine.
struct Local;

impl reqwest::dns::Resolve for Local {
    fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async { Ok(Box::new(std::iter::once(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))) as reqwest::dns::Addrs) })
    }
}

async fn rig(name: &str) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-hubs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // a master key, so the grid seals a hub's secrets
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

impl Rig {
    fn at(&self, label: &str) -> String {
        format!("http://{label}.wisp.localhost:{}", self.port)
    }
    fn api(&self, path: &str) -> String {
        format!("{}{path}", self.grid.cfg.public_url)
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

async fn send(r: reqwest::RequestBuilder) -> (u16, Value) {
    let r = r.send().await.unwrap();
    let s = r.status().as_u16();
    (s, r.json::<Value>().await.unwrap_or(Value::Null))
}

/// What the hub's wisp heard as `x-wisp-proxy-via` from this caller ("" for none), or the refusal's status.
async fn via(g: &Rig, auth: &str) -> Result<String, (u16, Value)> {
    let (s, v) = send(g.web.get(format!("{}/api/whoami", g.at("acme"))).bearer_auth(auth)).await;
    if s != 200 {
        return Err((s, v));
    }
    Ok(v["headers"]["x-wisp-proxy-via"].as_str().unwrap_or_default().to_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hub_lets_its_team_in_by_role_and_its_keys_deploy() {
    let g = rig("roles").await;
    let (ada, bo, cy) = (g.device("ada"), g.device("bo"), g.device("cy"));
    let (s, v) = send(g.web.post(g.api("/api/teams")).bearer_auth(&ada).json(&json!({ "slug": "acme", "name": "Acme" }))).await;
    assert_eq!((s, v["hub"].clone()), (200, json!(true)), "{v}");
    // the hub's secrets, set before its box first starts, are in its environment (and it isn't a sandbox)
    let (s, v) =
        send(g.web.put(g.api("/api/teams/acme/secrets")).bearer_auth(&ada).json(&json!({ "key": "GH_TOKEN", "value": "ghp_x" }))).await;
    assert_eq!((s, v["env"].clone()), (200, json!(["GH_TOKEN"])), "{v}");
    assert_eq!(send(g.web.put(g.api("/api/teams/acme/secrets")).bearer_auth(&bo).json(&json!({ "key": "A", "value": "b" }))).await.0, 403);
    // /api/me lists the hubs you can open
    let (_, me) = send(g.web.get(g.api("/api/me")).bearer_auth(&ada)).await;
    assert_eq!(me["hubs"][0]["url"], json!(g.at("acme")), "{me}");

    // the owner runs the hub; an outsider is refused
    assert_eq!(via(&g, &ada).await.unwrap(), "owner");
    let (_, env) = send(g.web.get(format!("{}/api/whoami", g.at("acme"))).bearer_auth(&ada)).await;
    assert_eq!(env["env"], json!({ "GH_TOKEN": "ghp_x" }), "{env}");
    let (s, v) = via(&g, &bo).await.unwrap_err();
    assert!(s == 403 && v["error"].as_str().unwrap().contains("That hub is acme's"), "{s} {v}");

    // a member and a viewer, each heard as such
    for (who, role) in [("bo", "member"), ("cy", "viewer")] {
        let (s, _) =
            send(g.web.post(g.api("/api/teams/acme/members")).bearer_auth(&ada).json(&json!({ "handle": who, "role": role }))).await;
        assert_eq!(s, 200);
    }
    assert_eq!(via(&g, &bo).await.unwrap(), "member");
    assert_eq!(via(&g, &cy).await.unwrap(), "viewer");
    // a member can't make keys, invite or set secrets
    assert_eq!(send(g.web.post(g.api("/api/teams/acme/keys")).bearer_auth(&bo).json(&json!({ "name": "x" }))).await.0, 403);
    assert_eq!(send(g.web.post(g.api("/api/teams/acme/invites")).bearer_auth(&bo).json(&json!({}))).await.0, 403);

    // an invite link adds di as a member, once
    let (s, inv) = send(g.web.post(g.api("/api/teams/acme/invites")).bearer_auth(&ada).json(&json!({ "role": "member", "uses": 1 }))).await;
    assert_eq!(s, 201, "{inv}");
    assert!(inv["url"].as_str().unwrap().contains("/join/wgi_"), "{inv}");
    let di = g.device("di");
    let (s, j) = send(g.web.post(g.api("/api/join")).bearer_auth(&di).json(&json!({ "code": inv["code"] }))).await;
    assert_eq!((s, j["slug"].clone(), j["url"].clone()), (200, json!("acme"), json!(g.at("acme"))), "{j}");
    assert_eq!(via(&g, &di).await.unwrap(), "member");
    let ed = g.device("ed");
    assert_eq!(send(g.web.post(g.api("/api/join")).bearer_auth(&ed).json(&json!({ "code": inv["code"] }))).await.0, 410);

    // a deploy key: said once, opens its own hub as `deploy`, kept as a deploy when it applies
    let (s, k) = send(g.web.post(g.api("/api/teams/acme/keys")).bearer_auth(&ada).json(&json!({ "name": "ci-main" }))).await;
    assert_eq!(s, 201, "{k}");
    let key = k["key"].as_str().unwrap().to_owned();
    assert!(key.starts_with("wgh_"), "{key}");
    assert_eq!(via(&g, &key).await.unwrap(), "deploy");
    let (_, plan) =
        send(g.web.post(g.api("/api/teams/acme/keys")).bearer_auth(&ada).json(&json!({ "name": "prs", "rights": "plan" }))).await;
    assert_eq!(via(&g, plan["key"].as_str().unwrap()).await.unwrap(), "plan");
    let (_, list) = send(g.web.get(g.api("/api/teams/acme/keys")).bearer_auth(&ada)).await;
    assert!(!list.to_string().contains(&key), "a key is said once");
    let apply = json!({ "cmd": "resources.apply", "args": { "resources": [] } });
    let (s, v) = send(
        g.web
            .post(format!("{}/api/cmd", g.at("acme")))
            .bearer_auth(&key)
            .header("idempotency-key", "k1")
            .header("x-wisp-deploy-source", "acme/agents@a1b2c3")
            .json(&apply),
    )
    .await;
    assert_eq!((s, v["ok"].clone()), (200, json!(true)), "{v}");
    let (s, d) = send(g.web.get(format!("{}/_grid/deploys", g.at("acme"))).bearer_auth(&key)).await;
    assert_eq!(s, 200, "{d}");
    let first = &d["deploys"][0];
    assert_eq!(
        (first["n"].clone(), first["byName"].clone(), first["source"].clone()),
        (json!(1), json!("ci-main"), json!("acme/agents@a1b2c3"))
    );
    assert_eq!(first["summary"], json!({ "created": 2, "updated": 0, "deleted": 0 }));
    let (_, d2) = send(g.web.get(g.api("/api/teams/acme/deploys")).bearer_auth(&bo)).await;
    assert_eq!(d2["deploys"].as_array().unwrap().len(), 1, "anyone in the team reads the history");

    // a key opens its own team's hub only, and none of the grid's API
    let (_, z) = send(g.web.post(g.api("/api/teams")).bearer_auth(&bo).json(&json!({ "slug": "zeta" }))).await;
    assert_eq!(z["hub"], json!(true));
    let (s, v) = send(g.web.get(format!("{}/api/whoami", g.at("zeta"))).bearer_auth(&key)).await;
    assert!(s == 403 && v["error"].as_str().unwrap().contains("another team's"), "{s} {v}");
    assert_eq!(send(g.web.get(g.api("/api/teams")).bearer_auth(&key)).await.0, 401);
    // a person's API key never opens a hub
    let (_, wgk) = send(g.web.post(g.api("/api/keys")).bearer_auth(&ada).json(&json!({ "name": "script" }))).await;
    let (s, _) = via(&g, wgk["key"].as_str().unwrap()).await.unwrap_err();
    assert_eq!(s, 403);
    // revoked: nobody
    let (s, _) = send(g.web.delete(g.api(&format!("/api/teams/acme/keys/{}", k["id"].as_str().unwrap()))).bearer_auth(&ada)).await;
    assert_eq!(s, 200);
    assert_eq!(via(&g, &key).await.unwrap_err().0, 401);

    // gone with its team: only an owner, typing its name
    assert_eq!(send(g.web.delete(g.api("/api/teams/acme")).bearer_auth(&bo).json(&json!({ "confirm": "acme" }))).await.0, 403);
    assert_eq!(send(g.web.delete(g.api("/api/teams/acme")).bearer_auth(&ada).json(&json!({ "confirm": "acme" }))).await.0, 200);
    assert_eq!(send(g.web.get(format!("{}/api/whoami", g.at("acme"))).bearer_auth(&ada)).await.0, 404);
    let (_, me) = send(g.web.get(g.api("/api/me")).bearer_auth(&ada)).await;
    assert_eq!(me["hubs"], json!([]), "{me}");
    g.done();
}

/// A browser signs in at a hub's address from the grid (`/go?hub=`), as itself: a member's cookie there is a member's.
#[tokio::test(flavor = "multi_thread")]
async fn a_member_signs_in_at_the_hub_from_the_grid() {
    let g = rig("go").await;
    let ada = g.device("ada");
    g.grid.accounts.dev_user("bo").unwrap();
    send(g.web.post(g.api("/api/teams")).bearer_auth(&ada).json(&json!({ "slug": "acme" }))).await;
    send(g.web.post(g.api("/api/teams/acme/members")).bearer_auth(&ada).json(&json!({ "handle": "bo", "role": "member" }))).await;
    let session = |h: &str| {
        let u = g.grid.accounts.dev_user(h).unwrap();
        format!("wisp_grid={}", g.grid.accounts.new_session(&u.id).unwrap())
    };
    // signed out at the hub, a page goes to the grid's /go for it
    let r = g.web.get(format!("{}/tasks", g.at("acme"))).header("accept", "text/html").send().await.unwrap();
    assert_eq!(r.status(), 302);
    assert!(r.headers()["location"].to_str().unwrap().contains("/go?hub=acme"), "{:?}", r.headers()["location"]);
    // an outsider gets no ticket
    let r = g.web.get(g.api("/go?hub=acme")).header("cookie", session("cy")).send().await.unwrap();
    assert_eq!(r.status(), 404);
    let r = g.web.get(g.api("/go?hub=acme&path=/tasks")).header("cookie", session("bo")).send().await.unwrap();
    assert_eq!(r.status(), 302);
    let to = r.headers()["location"].to_str().unwrap().to_owned();
    assert!(to.starts_with(&format!("{}/_grid/auth?t=", g.at("acme"))), "{to}");
    let r = g.web.get(&to).send().await.unwrap();
    assert_eq!(r.status(), 302);
    let cookie = r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
    let (s, v) = send(g.web.get(format!("{}/api/whoami", g.at("acme"))).header("cookie", &cookie)).await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["headers"]["x-wisp-proxy-via"], "member");
    // taken out of the team, the cookie opens nothing
    send(g.web.delete(g.api("/api/teams/acme/members/bo")).bearer_auth(&ada)).await;
    let (s, _) = send(g.web.get(format!("{}/api/whoami", g.at("acme"))).header("cookie", &cookie)).await;
    assert_eq!(s, 401);
    g.done();
}

/// The Teams pages' forms (ADR-0147): a person makes a team and a deploy key in the browser, with the page's CSRF token,
/// and the key is said once; a form without the token, or someone outside the team, changes nothing.
#[tokio::test(flavor = "multi_thread")]
async fn the_teams_pages_make_a_team_and_say_a_key_once() {
    let g = rig("pages").await;
    let u = g.grid.accounts.dev_user("ada").unwrap();
    let cookie = format!("wisp_grid={}", g.grid.accounts.new_session(&u.id).unwrap());
    let page = |path: &str| g.web.get(g.api(path)).header("cookie", &cookie).send();
    let csrf_of = |html: &str| html.split(r#"name="csrf" value=""#).nth(1).unwrap().split('"').next().unwrap().to_owned();
    let html = page("/teams").await.unwrap().text().await.unwrap();
    assert!(html.contains("No teams yet"), "{html}");
    let csrf = csrf_of(&html);
    let post = |path: &str, form: Vec<(&str, String)>| {
        let body: String =
            url::form_urlencoded::Serializer::new(String::new()).extend_pairs(form.iter().map(|(k, v)| (*k, v.as_str()))).finish();
        g.web
            .post(g.api(path))
            .header("cookie", &cookie)
            .header("origin", g.grid.cfg.public_url.clone())
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
    };
    let r = post("/teams", vec![("csrf", csrf.clone()), ("slug", "acme".into()), ("name", "Acme".into())]).await.unwrap();
    assert_eq!(r.status(), 302);
    assert_eq!(r.headers()["location"], "/teams/acme");
    // stale form: nothing made
    let r = post("/teams/acme/keys", vec![("csrf", "nope".into()), ("name", "ci".into())]).await.unwrap();
    assert_eq!(r.status(), 403);
    let r = post("/teams/acme/keys", vec![("csrf", csrf.clone()), ("name", "ci-main".into()), ("rights", "deploy".into())]).await.unwrap();
    assert_eq!(r.status(), 303);
    let held = r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
    let both = format!("{cookie}; {held}");
    let html = g.web.get(g.api("/teams/acme")).header("cookie", &both).send().await.unwrap().text().await.unwrap();
    let key = html.split(r#"<code class="key">"#).nth(1).unwrap().split('<').next().unwrap().to_owned();
    assert!(key.starts_with("wgh_"), "{key}");
    let again = g.web.get(g.api("/teams/acme")).header("cookie", &both).send().await.unwrap().text().await.unwrap();
    assert!(!again.contains(&key), "said once");
    assert!(again.contains("ci-main"));
    // someone outside the team sees nothing of it
    let bo = g.grid.accounts.dev_user("bo").unwrap();
    let bo_cookie = format!("wisp_grid={}", g.grid.accounts.new_session(&bo.id).unwrap());
    let r = g.web.get(g.api("/teams/acme")).header("cookie", &bo_cookie).send().await.unwrap();
    assert_eq!(r.status(), 302, "back to /teams with why");
    g.done();
}
