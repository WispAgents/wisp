//! A cloud wisp's life as the grid keeps it (v1 grid/server.js `hostd.on('change')`, grid/hostd.js `tick`,
//! `#scheduleWake`, `close`; grid/cli.js `serve`): its start and sleep are recorded where Home reads them, its next
//! scheduled task wakes it, and no box outlives the grid that started it.
//!
//! The box is this test binary: hostd runs `<bin> serve` with `WISP_BOX=1`, so `fake_box_serve` (the only test here
//! whose name holds "serve") answers as a box's wisp would. Run normally, it does nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use wisp_grid::boxhost::{BoxHost as _, BoxState};

const FORM: &str = "application/x-www-form-urlencoded";

/// The stand-in box: `/api/health`, `/api/status` (its next wake from `WISP_FAKE_NEXT_WAKE`), anything else "fake box".
/// Writes its pid to `$HOME/fakebox.pid` (hostd's HOME for a box is its own folder).
#[test]
fn fake_box_serve() {
    if std::env::var("WISP_BOX").as_deref() != Ok("1") {
        return;
    }
    let port: u16 = std::env::var("WISP_PORT").unwrap().parse().unwrap();
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap());
    std::fs::write(home.join("fakebox.pid"), std::process::id().to_string()).unwrap();
    let next = std::env::var("WISP_FAKE_NEXT_WAKE").ok().and_then(|v| v.parse::<i64>().ok());
    let l = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    for s in l.incoming().flatten() {
        let mut r = BufReader::new(s);
        let mut line = String::new();
        let _ = r.read_line(&mut line);
        let mut h = String::new();
        while r.read_line(&mut h).is_ok_and(|n| n > 2) {
            h.clear();
        }
        let body = if line.contains("/api/status") {
            json!({ "live": 0, "sides": 0, "terms": 0, "channels": 0, "nextWakeAt": next, "lastActivityAt": null }).to_string()
        } else if line.contains("/api/health") {
            "{\"ok\":true}".to_owned()
        } else {
            "fake box".to_owned()
        };
        let mut s = r.into_inner();
        let _ = write!(s, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    }
}

fn alive(pid: &str) -> bool {
    std::process::Command::new("kill").args(["-0", pid]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

async fn until(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..200 {
        if ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("never: {what}");
}

struct Rig {
    grid: Arc<wisp_grid::edge::Grid>,
    c: reqwest::Client,
    port: u16,
    stop: CancellationToken,
    dir: std::path::PathBuf,
}

async fn rig(name: &str, next_wake: Option<i64>) -> Rig {
    let dir = std::env::temp_dir().join(format!("wisp-grid-life-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let file = json!({ "domain": "wisp.localhost", "listen": { "host": "127.0.0.1", "port": port }, "auth": { "dev": true } });
    let cfg = wisp_grid::config::for_test(&dir, file).unwrap();
    let clock = wisp_grid::clock::system();
    let db = Arc::new(wisp_grid::db::Db::open(&dir, clock.now_ms()).unwrap());
    let url_cfg = cfg.clone();
    let env = next_wake.map(|t| vec![("WISP_FAKE_NEXT_WAKE".to_owned(), t.to_string())]).unwrap_or_default();
    let boxes = wisp_grid::hostd::ProcessHost::new(
        wisp_grid::hostd::ProcessOptions {
            bin: std::env::current_exe().unwrap(),
            data: dir.clone(),
            idle_ms: 600_000,
            check_ms: 200,
            start_ms: 20_000,
            max_running: 2,
            env,
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
    Rig { grid, c, port, stop, dir }
}

impl Rig {
    fn session(&self) -> String {
        let u = self.grid.accounts.dev_user("ada").unwrap();
        format!("wisp_grid={}", self.grid.accounts.new_session(&u.id).unwrap())
    }
    fn uid(&self) -> String {
        self.grid.accounts.user_by_handle("ada").unwrap().unwrap().id
    }
    fn pid(&self) -> String {
        std::fs::read_to_string(self.dir.join("users").join(self.uid()).join("fakebox.pid")).unwrap()
    }
    /// Opens ada's cloud wisp as the browser does (`/go`, its ticket), and asks it something.
    async fn open(&self, grid_cookie: &str) -> String {
        let root = format!("http://wisp.localhost:{}", self.port);
        let r = self.c.get(format!("{root}/go")).header("cookie", grid_cookie).send().await.unwrap();
        let r = self.c.get(r.headers()["location"].to_str().unwrap()).send().await.unwrap();
        let cookie = r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
        let r = self.c.get(format!("http://ada.wisp.localhost:{}/x", self.port)).header("cookie", &cookie).send().await.unwrap();
        r.text().await.unwrap()
    }
    async fn home(&self, cookie: &str) -> String {
        let root = format!("http://wisp.localhost:{}", self.port);
        self.c.get(format!("{root}/")).header("cookie", cookie).send().await.unwrap().text().await.unwrap()
    }
    fn row(&self) -> (String, Option<i64>, Option<i64>, Option<i64>, Option<String>) {
        // the id first: grid.db's connection is held while the closure runs
        let uid = self.uid();
        self.grid
            .db
            .with(|c| {
                c.query_row(
                    "SELECT state, started_at, stopped_at, next_wake_at, stop_reason FROM instances WHERE user_id = ?1",
                    [uid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
            })
            .unwrap_or_default()
    }
    async fn done(self) {
        self.grid.boxes.close_all().await;
        self.stop.cancel();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn csrf_in(page: &str) -> String {
    let at = page.find(r#"name="csrf" value=""#).expect("a form with its token");
    page[at + r#"name="csrf" value=""#.len()..].split('"').next().unwrap().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn home_says_when_the_cloud_wisp_started_slept_and_wakes_next() {
    let next = wisp_grid::clock::system().now_ms() + 3 * 3_600_000;
    let g = rig("home", Some(next)).await;
    let cookie = g.session();
    assert_eq!(g.open(&cookie).await, "fake box");
    until("its start is recorded", || g.row().0 == "running" && g.row().1.is_some()).await;
    let home = g.home(&cookie).await;
    assert!(home.contains("Started just now.") && !home.contains("Started never"), "{home}");
    // a tick reads its schedule, then Sleep: when, why, and what wakes it next, on Home
    tokio::time::sleep(Duration::from_millis(600)).await;
    let pid = g.pid();
    let r =
        g.c.post(format!("http://wisp.localhost:{}/instance/stop", g.port))
            .header("cookie", &cookie)
            .header("content-type", FORM)
            .body(format!("csrf={}", csrf_in(&home)))
            .send()
            .await
            .unwrap();
    assert_eq!(r.status(), 302);
    until("its sleep is recorded", || g.row().0 == "stopped").await;
    let (_, started, stopped, wake, why) = g.row();
    assert!(started.is_some() && stopped.is_some(), "{started:?} {stopped:?}");
    assert_eq!((wake, why.as_deref()), (Some(next), Some("user")));
    assert!(!alive(&pid), "the box's process is gone");
    let home = g.home(&cookie).await;
    assert!(!home.contains("Nothing scheduled"), "Next schedule has its time: {home}");
    g.done().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_box_asleep_wakes_for_its_scheduled_task() {
    // due within the early margin: it wakes as soon as it sleeps
    let next = wisp_grid::clock::system().now_ms() + 5_000;
    let g = rig("wake", Some(next)).await;
    assert_eq!(g.open(&g.session()).await, "fake box");
    tokio::time::sleep(Duration::from_millis(600)).await;
    let first = g.pid();
    g.grid.boxes.stop(&g.uid(), "idle").await.unwrap();
    until("it wakes for its task", || g.grid.boxes.state(&g.uid()) == BoxState::Running).await;
    assert_ne!(g.pid(), first, "a new start");
    g.done().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_the_grid_stops_its_boxes_and_starts_no_more() {
    let g = rig("close", None).await;
    let cookie = g.session();
    assert_eq!(g.open(&cookie).await, "fake box");
    let pid = g.pid();
    assert!(alive(&pid));
    g.grid.boxes.close_all().await;
    assert!(!alive(&pid), "the box went with the grid");
    let bu = wisp_grid::rows::BoxUser { id: g.uid(), handle: "ada".into(), sandbox: None, owner: None };
    let e = g.grid.boxes.ensure(&bu, false).await.unwrap_err();
    assert_eq!((e.status(), e.to_string()), (503, "The grid is shutting down.".to_owned()));
    g.done().await;
}

/// The binary: `wisp-grid dev` told to stop by SIGTERM (as `kill`, systemd or a supervisor does) stops its boxes first.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn wisp_grid_dev_stops_its_boxes_on_sigterm() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-life-bin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let mut grid = std::process::Command::new(env!("CARGO_BIN_EXE_wisp-grid"))
        .args([
            "dev",
            "--port",
            &port.to_string(),
            "--data",
            dir.to_str().unwrap(),
            "--wisp",
            std::env::current_exe().unwrap().to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let c = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve("wisp.localhost", ([127, 0, 0, 1], port).into())
        .resolve("ada.wisp.localhost", ([127, 0, 0, 1], port).into())
        .build()
        .unwrap();
    let root = format!("http://wisp.localhost:{port}");
    let mut signed = None;
    for _ in 0..200 {
        if let Ok(r) = c.get(format!("{root}/auth/dev?handle=ada&next=/")).send().await {
            signed = r.headers().get("set-cookie").map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned());
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let cookie = signed.expect("the dev grid answers");
    let r = c.get(format!("{root}/go")).header("cookie", &cookie).send().await.unwrap();
    let r = c.get(r.headers()["location"].to_str().unwrap()).send().await.unwrap();
    let boxc = r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
    let r = c.get(format!("http://ada.wisp.localhost:{port}/x")).header("cookie", &boxc).send().await.unwrap();
    assert_eq!(r.text().await.unwrap(), "fake box");
    let users = std::fs::read_dir(dir.join("users")).unwrap().flatten().next().unwrap().path();
    let pid = std::fs::read_to_string(users.join("fakebox.pid")).unwrap();
    assert!(alive(&pid));

    assert!(std::process::Command::new("kill").args(["-TERM", &grid.id().to_string()]).status().unwrap().success());
    let mut exited = None;
    for _ in 0..300 {
        if let Some(s) = grid.try_wait().unwrap() {
            exited = Some(s);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let s = exited.expect("wisp-grid stopped");
    assert!(s.success(), "{s}");
    assert!(!alive(&pid), "its box stopped with it");
    // and the grid kept that it stopped
    let db = wisp_grid::db::Db::open(&dir, 0).unwrap();
    let st: Vec<(String, Option<String>)> = db
        .with(|c| c.prepare("SELECT state, stop_reason FROM instances")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect())
        .unwrap();
    assert_eq!(st, vec![("stopped".to_owned(), Some("grid stopped".to_owned()))]);
    let _ = std::fs::remove_dir_all(&dir);
}
