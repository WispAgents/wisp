//! The grid's signed-in pages and their forms through the grid's own routes (`edge_pages.rs`): home and account
//! for a signed-in person, a form with a stale CSRF token refused, one with the page's token done, sign-out
//! dropping the session, and the pages sending a signed-out browser to sign in.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::json;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

const FORM: &str = "application/x-www-form-urlencoded";

fn csrf_in(page: &str) -> String {
    let at = page.find(r#"name="csrf" value=""#).expect("a form with its token");
    page[at + r#"name="csrf" value=""#.len()..].split('"').next().unwrap().to_owned()
}

struct Rig {
    grid: Arc<wisp_grid::edge::Grid>,
    c: reqwest::Client,
    root: String,
    stop: CancellationToken,
    dir: std::path::PathBuf,
}

async fn rig(name: &str, extra: serde_json::Value) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // a grid that keeps sandboxes needs a master key (they're sealed)
    if extra.get("sandboxes").is_some() {
        use base64::Engine as _;
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
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut file = json!({ "domain": "wisp.localhost", "listen": { "host": "127.0.0.1", "port": port }, "auth": { "dev": true } });
    if let (Some(f), Some(e)) = (file.as_object_mut(), extra.as_object()) {
        f.extend(e.clone());
    }
    let cfg = wisp_grid::config::for_test(&dir, file).unwrap();
    let clock = wisp_grid::clock::system();
    let db = Arc::new(wisp_grid::db::Db::open(&dir, clock.now_ms()).unwrap());
    let url_cfg = cfg.clone();
    let boxes = wisp_grid::hostd::ProcessHost::new(
        wisp_grid::hostd::ProcessOptions {
            bin: "wisp".into(),
            data: dir.clone(),
            idle_ms: 60_000,
            check_ms: 1000,
            start_ms: 30_000,
            max_running: 1,
            max_running_per_user: 4,
            wake_retry_ms: 30_000,
            queue_ms: 120_000,
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

/// What a form said it did: the one-time flash cookie (`!` first when it went wrong).
fn flash(r: &reqwest::Response) -> String {
    let v = r.headers().get_all("set-cookie").iter().map(|v| v.to_str().unwrap().to_owned()).find(|c| c.starts_with("wisp_flash="));
    let v = v.unwrap_or_default();
    let raw = v.split(';').next().unwrap_or_default().trim_start_matches("wisp_flash=");
    url::form_urlencoded::parse(format!("x={}", raw.replace('+', "%2B")).as_bytes()).next().map(|(_, v)| v.into_owned()).unwrap_or_default()
}

/// Signs in by handle (dev): the session cookie.
async fn sign_in(r: &Rig, handle: &str) -> String {
    let res = r.c.get(format!("{}/auth/dev?handle={handle}&next=/", r.root)).send().await.unwrap();
    assert_eq!(res.status(), 302);
    let session =
        res.headers().get_all("set-cookie").iter().map(|v| v.to_str().unwrap().to_owned()).find(|c| c.starts_with("wisp_grid=")).unwrap();
    session.split(';').next().unwrap().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_signed_in_person_sees_home_and_account_and_their_forms_work() {
    let rig = rig("pages", json!({})).await;
    let (c, root) = (&rig.c, &rig.root);
    // signed out: the landing with its way in (QA round 2 B4), and account sends you to sign in
    let r = c.get(format!("{root}/")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert!(r.text().await.unwrap().contains(r#"href="/auth/dev/start"#));
    let r = c.get(format!("{root}/account")).send().await.unwrap();
    assert_eq!(r.status(), 302);
    assert!(r.headers()["location"].to_str().unwrap().starts_with("/signin"));

    // signed in (dev): home says hi, account lists how you sign in and offers sign-out
    let cookie = sign_in(&rig, "ada").await;
    let home = c.get(format!("{root}/")).header("cookie", &cookie).send().await.unwrap();
    assert_eq!(home.status(), 200);
    let home = home.text().await.unwrap();
    assert!(home.contains("Hi ada"), "{home}");
    let account = c.get(format!("{root}/account")).header("cookie", &cookie).send().await.unwrap().text().await.unwrap();
    assert!(account.contains("Signed-in devices") && account.contains("Sign out everywhere"), "{account}");
    // this grid routes no channels: no Telegram section
    assert!(!account.contains("Telegram to your computers"));
    let csrf = csrf_in(&account);

    // a stale form is refused; deleting with the wrong handle says what to type
    let r =
        c.post(format!("{root}/signout")).header("cookie", &cookie).header("content-type", FORM).body("csrf=stale").send().await.unwrap();
    assert_eq!(r.status(), 403);
    let r = c
        .post(format!("{root}/account/delete"))
        .header("cookie", &cookie)
        .header("content-type", FORM)
        .body(format!("csrf={csrf}&confirm=bob"))
        .send()
        .await
        .unwrap();
    assert_eq!((r.status().as_u16(), r.headers()["location"].to_str().unwrap()), (302, "/account"));
    assert_eq!(flash(&r), "!Type ada to delete your account.");
    // the cloud wisp asleep already: Sleep says so
    let r = c
        .post(format!("{root}/instance/stop"))
        .header("cookie", &cookie)
        .header("content-type", FORM)
        .body(format!("csrf={csrf}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 302);
    assert!(flash(&r).contains("already asleep"), "{}", flash(&r));

    // sign out: the session goes and its cookie is cleared; the pages send you to sign in again
    let r = c
        .post(format!("{root}/signout"))
        .header("cookie", &cookie)
        .header("content-type", FORM)
        .body(format!("csrf={csrf}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 302);
    assert!(
        r.headers()
            .get_all("set-cookie")
            .iter()
            .any(|v| v.to_str().unwrap().starts_with("wisp_grid=;") && v.to_str().unwrap().contains("Max-Age=0"))
    );
    let r = c.get(format!("{root}/account")).header("cookie", &cookie).send().await.unwrap();
    assert_eq!(r.status(), 302);
    rig.stop.cancel();
    let _ = std::fs::remove_dir_all(&rig.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_account_page_offers_the_telegram_routing_form_and_says_what_was_wrong() {
    let rig = rig("route", json!({ "channels": { "enabled": true, "route": true } })).await;
    let (c, root) = (&rig.c, &rig.root);
    let cookie = sign_in(&rig, "ada").await;
    let account = c.get(format!("{root}/account")).header("cookie", &cookie).send().await.unwrap().text().await.unwrap();
    assert!(
        account.contains("Telegram to your computers")
            && account.contains(r#"action="/channels/route""#)
            && account.contains("Who may send"),
        "{account}"
    );
    let csrf = csrf_in(&account);
    // not a bot token: back to the form with BotFather's shape, nothing kept
    let r = c
        .post(format!("{root}/channels/route"))
        .header("cookie", &cookie)
        .header("content-type", FORM)
        .body(format!("csrf={csrf}&token=nope&senders=4242"))
        .send()
        .await
        .unwrap();
    assert_eq!((r.status().as_u16(), r.headers()["location"].to_str().unwrap()), (302, "/account"));
    let f = flash(&r);
    assert!(f.starts_with('!') && f.contains("BotFather"), "{f}");
    rig.stop.cancel();
    let _ = std::fs::remove_dir_all(&rig.dir);
}

/// Deleting an account takes everything of it (grid-r2-4): the user's own box and each sandbox's box and home
/// (`<uid>-sb-<id>`), and every sealed secret (the sandbox's token, its sign-in and env) with the data key that
/// sealed them. Before, only the user's own box and apps went: the sandboxes' homes and `user_secrets`/`keys` stayed.
#[tokio::test(flavor = "multi_thread")]
async fn deleting_an_account_removes_its_sandboxes_and_shreds_its_secrets() {
    let rig = rig("delete-all", json!({ "sandboxes": { "enabled": true } })).await;
    let (c, root) = (&rig.c, &rig.root);
    let cookie = sign_in(&rig, "ada").await;
    let g = &rig.grid;
    let u = g.accounts.user_by_handle("ada").unwrap().unwrap();
    let (sb, _bearer) = g.sandboxes.create(&u, "task", &|_| false).unwrap();
    g.sandboxes.set_login(&u.id, "claude", &format!("sk-ant-oat01-{}", "a".repeat(40))).unwrap();
    g.sandboxes.set_env(&u.id, "DATABASE_URL", "postgres://x").unwrap();
    // the homes the boxes would have (the process driver deletes each box's folder)
    let users = rig.dir.join("users");
    let own = users.join(&u.id);
    let sandbox = users.join(format!("{}-sb-{}", u.id, sb.id));
    for d in [&own, &sandbox] {
        std::fs::create_dir_all(d.join("home")).unwrap();
    }
    let count = |table: &str| -> i64 {
        g.db.with(|c| c.query_row(&format!("SELECT count(*) FROM {table} WHERE user_id = ?1"), [&u.id], |r| r.get(0))).unwrap()
    };
    assert!(count("user_secrets") >= 3 && count("keys") == 1, "sealed before");
    let account = c.get(format!("{root}/account")).header("cookie", &cookie).send().await.unwrap().text().await.unwrap();
    let r = c
        .post(format!("{root}/account/delete"))
        .header("cookie", &cookie)
        .header("content-type", FORM)
        .body(format!("csrf={}&confirm=ada", csrf_in(&account)))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 302);
    assert!(r.headers()["location"].to_str().unwrap().starts_with("/signin?m="));
    assert!(g.accounts.user_by_handle("ada").unwrap().is_none());
    assert!(!own.exists(), "the user's own box is removed");
    assert!(!sandbox.exists(), "the sandbox's box and home are removed too");
    assert_eq!((count("user_secrets"), count("keys")), (0, 0), "every sealed secret and the data key are gone");
    rig.stop.cancel();
    let _ = std::fs::remove_dir_all(&rig.dir);
}
