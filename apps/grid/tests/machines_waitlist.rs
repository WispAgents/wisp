//! Add a cloud machine (v1 grid/server.js `/machines/waitlist`, grid/accounts.js `wishMachines`): the dialog on Home
//! puts you on the list for more machines, a bigger one or GPUs, and Home says so; a form without script works too;
//! the operator reads the list with `wisp-grid waitlist`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

const FORM: &str = "application/x-www-form-urlencoded";

struct Rig {
    grid: Arc<wisp_grid::edge::Grid>,
    c: reqwest::Client,
    root: String,
    stop: CancellationToken,
    dir: std::path::PathBuf,
}

async fn rig(name: &str, hostd: Value) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-wish-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let file =
        json!({ "domain": "wisp.localhost", "listen": { "host": "127.0.0.1", "port": port }, "auth": { "dev": true }, "hostd": hostd });
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
        .build()
        .unwrap();
    Rig { grid, c, root: format!("http://wisp.localhost:{port}"), stop, dir }
}

impl Rig {
    fn session(&self) -> String {
        let u = self.grid.accounts.dev_user("ada").unwrap();
        format!("wisp_grid={}", self.grid.accounts.new_session(&u.id).unwrap())
    }
    async fn home(&self, cookie: &str) -> String {
        self.c.get(format!("{}/", self.root)).header("cookie", cookie).send().await.unwrap().text().await.unwrap()
    }
    async fn wish(&self, cookie: &str, body: &Value) -> reqwest::Response {
        self.c
            .post(format!("{}/machines/waitlist", self.root))
            .header("cookie", cookie)
            .header("accept", "application/json")
            .json(body)
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
async fn the_dialog_puts_you_on_the_list_and_home_says_so() {
    let g = rig("dialog", json!({})).await;
    let cookie = g.session();
    let home = g.home(&cookie).await;
    let csrf = csrf_in(&home);

    // the dialog's own call: JSON in, JSON out
    let r = g.wish(&cookie, &json!({ "csrf": csrf, "wants": ["gpu", "bigger"], "note": "a 70B model" })).await;
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    assert_eq!(
        (v["ok"].clone(), v["wish"]["wants"].clone(), v["wish"]["note"].clone()),
        (json!(true), json!(["gpu", "bigger"]), json!("a 70B model"))
    );
    // Home says it, ticked in the dialog, the note kept
    let home = g.home(&cookie).await;
    assert!(home.contains("On the waitlist for a bigger machine and GPUs."), "{home}");
    assert!(home.contains(r#"name="want_gpu" value="1" checked"#) && home.contains(">a 70B model</textarea>"), "{home}");
    assert!(home.contains(r#"data-state="done""#));

    // something else is refused; a stale form too; nothing changes
    let r = g.wish(&cookie, &json!({ "csrf": csrf, "wants": ["tpu"] })).await;
    assert_eq!(
        (r.status().as_u16(), r.json::<Value>().await.unwrap()["error"].clone()),
        (400, json!("\"tpu\" isn’t something to wait for: more, bigger, gpu."))
    );
    assert_eq!(g.wish(&cookie, &json!({ "csrf": "stale", "wants": [] })).await.status(), 403);
    assert!(g.home(&cookie).await.contains("On the waitlist for a bigger machine and GPUs."));

    // the operator's list
    let report = wisp_grid::wishes::report(&g.grid.accounts).unwrap();
    assert!(report.contains("wanting more machines:") && report.contains("gpu,bigger") && report.contains("“a 70B model”"), "{report}");

    // Leave the list: nothing picked
    let r = g.wish(&cookie, &json!({ "csrf": csrf, "wants": [] })).await;
    assert_eq!(r.json::<Value>().await.unwrap(), json!({ "ok": true, "wish": null }));
    let home = g.home(&cookie).await;
    assert!(home.contains(r#"data-state="pick""#) && home.contains(r#"<span class="pill soon">Waitlist</span>"#), "{home}");
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_form_without_script_and_a_signed_out_post_work_as_v1() {
    let g = rig("form", json!({})).await;
    let cookie = g.session();
    let csrf = csrf_in(&g.home(&cookie).await);
    let r =
        g.c.post(format!("{}/machines/waitlist", g.root))
            .header("cookie", &cookie)
            .header("content-type", FORM)
            .body(format!("csrf={csrf}&want_more=1&note=three+projects"))
            .send()
            .await
            .unwrap();
    assert_eq!((r.status().as_u16(), r.headers()["location"].to_str().unwrap()), (302, "/"));
    let flash =
        r.headers().get_all("set-cookie").iter().map(|v| v.to_str().unwrap().to_owned()).find(|c| c.starts_with("wisp_flash=")).unwrap();
    assert!(flash.contains("on%20the%20list"), "{flash}");
    let uid = g.grid.accounts.user_by_handle("ada").unwrap().unwrap().id;
    let w = g.grid.accounts.machine_wish(&uid).unwrap().unwrap();
    assert_eq!((w.wants, w.note.as_str()), (vec!["more".to_owned()], "three projects"));
    // signed out: to sign in, nothing kept
    let r = g.c.post(format!("{}/machines/waitlist", g.root)).header("content-type", FORM).body("want_gpu=1").send().await.unwrap();
    assert!(r.headers()["location"].to_str().unwrap().starts_with("/signin"));
    g.done();
}

#[tokio::test(flavor = "multi_thread")]
async fn home_says_what_the_cloud_machine_is() {
    // containers: their limits (boxd's when grid.json sets none, as v1 showed)
    let g = rig("spec", json!({ "driver": "container", "container": { "boxd": "/run/wisp-grid/box.sock" } })).await;
    let home = g.home(&g.session()).await;
    assert!(home.contains("1.5 vCPU · 1.5 GB · sleeps when idle"), "{home}");
    g.done();
    let g =
        rig("spec2", json!({ "driver": "container", "container": { "boxd": "/run/wisp-grid/box.sock", "cpus": 2, "memory": "4g" } })).await;
    assert!(g.home(&g.session()).await.contains("2 vCPU · 4 GB · sleeps when idle"));
    g.done();
    // a process box has no limits of its own: it says none
    let g = rig("spec3", json!({})).await;
    let home = g.home(&g.session()).await;
    assert!(home.contains("<small>Machine: sleeps when idle</small>"), "{home}");
    g.done();
}
