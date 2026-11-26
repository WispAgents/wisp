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
/// Writes its pid to `$HOME/fakebox.pid` (hostd's HOME for a box is its own folder), and, as `wisp serve` does, the port
/// it bound to `$WISP_HOME/server.json`.
#[test]
fn fake_box_serve() {
    if std::env::var("WISP_BOX").as_deref() != Ok("1") {
        return;
    }
    let port: u16 = std::env::var("WISP_PORT").unwrap().parse().unwrap();
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap());
    std::fs::write(home.join("fakebox.pid"), std::process::id().to_string()).unwrap();
    let next = std::env::var("WISP_FAKE_NEXT_WAKE").ok().and_then(|v| v.parse::<i64>().ok());
    // as `wisp serve` does: `WISP_PORT=0` binds a port of its own, and `server.json` in its `WISP_HOME` says which
    let l = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap_or_else(|e| panic!("the box couldn't bind {port}: {e}"));
    let bound = l.local_addr().unwrap().port();
    let wisp_home = std::path::PathBuf::from(std::env::var("WISP_HOME").unwrap());
    let file = json!({ "url": format!("http://127.0.0.1:{bound}"), "port": bound, "pid": std::process::id() });
    std::fs::write(wisp_home.join("server.json"), file.to_string()).unwrap();
    for s in l.incoming().flatten() {
        let mut r = BufReader::new(s);
        let mut line = String::new();
        let _ = r.read_line(&mut line);
        let mut h = String::new();
        while r.read_line(&mut h).is_ok_and(|n| n > 2) {
            h.clear();
        }
        if line.contains("/_die") {
            let mut s = r.into_inner();
            let _ = write!(s, "HTTP/1.1 200 OK\r\ncontent-length: 3\r\nconnection: close\r\n\r\nbye");
            let _ = s.flush();
            std::process::exit(137);
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

/// Whether the process `pid` runs, asked of the OS itself. Not `kill -0` on Windows: there `kill` is Git Bash's, which
/// takes MSYS process ids, not Windows ones, so a live box read as dead (and every "it's gone" passed unchecked).
fn alive(pid: &str) -> bool {
    let pid = pid.trim();
    if cfg!(windows) {
        // one CSV row, `"image.exe","1234",…`, for a running process; an INFO line for none
        let out = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]).output();
        return out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\",\"{pid}\",")));
    }
    std::process::Command::new("kill").args(["-0", pid]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

#[test]
fn the_liveness_probe_sees_a_running_process_and_not_a_finished_one() {
    let me = std::process::id().to_string();
    assert!(alive(&me), "this test's own process runs");
    let mut done =
        std::process::Command::new(std::env::current_exe().unwrap()).arg("--list").stdout(std::process::Stdio::null()).spawn().unwrap();
    let gone = done.id().to_string();
    done.wait().unwrap();
    assert!(!alive(&gone), "a process that has exited (and been waited for)");
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
    rig_with(name, next_wake, |_| {}).await
}

/// `rig` with hostd's options changed first (room on the grid, the box's binary).
async fn rig_with(name: &str, next_wake: Option<i64>, tune: impl FnOnce(&mut wisp_grid::hostd::ProcessOptions)) -> Rig {
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
    let mut o = wisp_grid::hostd::ProcessOptions {
        bin: std::env::current_exe().unwrap(),
        data: dir.clone(),
        idle_ms: 600_000,
        check_ms: 200,
        start_ms: 20_000,
        max_running: 2,
        max_running_per_user: 4,
        wake_retry_ms: 30_000,
        queue_ms: 120_000,
        env,
        url_of: Arc::new(move |h| url_cfg.instance_url(h)),
        boxd: None,
        previews: false,
    };
    tune(&mut o);
    let boxes = wisp_grid::hostd::ProcessHost::new(o, clock.clone());
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
        let cookie = self.box_cookie(grid_cookie).await;
        let r = self.c.get(format!("http://ada.wisp.localhost:{}/x", self.port)).header("cookie", &cookie).send().await.unwrap();
        r.text().await.unwrap()
    }
    /// ada's cookie at her box's address, as `/go` and its ticket leave it.
    async fn box_cookie(&self, grid_cookie: &str) -> String {
        let root = format!("http://wisp.localhost:{}", self.port);
        let r = self.c.get(format!("{root}/go")).header("cookie", grid_cookie).send().await.unwrap();
        let r = self.c.get(r.headers()["location"].to_str().unwrap()).send().await.unwrap();
        r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned()
    }
    /// `path` at ada's box, asked for as `accept` (a browser navigating: `text/html`).
    async fn at_box(&self, path: &str, cookie: &str, accept: &str) -> reqwest::Response {
        let mut r = self.c.get(format!("http://ada.wisp.localhost:{}{path}", self.port)).header("cookie", cookie).header("accept", accept);
        if accept.contains("text/html") {
            r = r.header("sec-fetch-mode", "navigate");
        }
        r.send().await.unwrap()
    }
    async fn state(&self, cookie: &str) -> serde_json::Value {
        self.at_box("/_grid/state", cookie, "application/json").await.json().await.unwrap()
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
    // the cloud wisp's row in Your machines (docs/ux/2026-11-18-grid.md #1) says when it started
    assert!(home.contains("Running · started just now") && !home.contains("started never"), "{home}");
    // a tick reads its schedule, then Sleep: when, why, and what wakes it next, on Home
    tokio::time::sleep(Duration::from_millis(600)).await;
    let pid = g.pid();
    assert!(alive(&pid), "the box runs before it sleeps");
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
    // asleep since when, and what wakes it next: on the row, and in its ⋯ with the machine
    assert!(home.contains("Asleep · since just now · opens in a few seconds · next schedule "), "{home}");
    assert!(home.contains("<small>Next schedule: ") && !home.contains("Next schedule: nothing scheduled"), "{home}");
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

/// The macOS flake (CI 44759143055: "wisp in the box stopped as it started (exit status: 101)"): hostd read a free port,
/// closed it and handed it to the box, which binds it only once it has started; a port another bind took in that window
/// fails the box's bind, and it exits 101. Shown here with a port held on purpose. So no port is handed any more: a
/// process box binds one of its own (`WISP_PORT=0`) and says which in its `server.json`, as `wisp serve` does.
#[test]
fn a_box_handed_a_port_someone_took_stops_as_it_started_so_none_is_handed() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-life-taken-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let taken = held.local_addr().unwrap().port();
    let start = |port: u16| {
        std::process::Command::new(std::env::current_exe().unwrap())
            .arg("serve")
            .env("WISP_BOX", "1")
            .env("WISP_PORT", port.to_string())
            .env("HOME", &dir)
            .env("WISP_HOME", &dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap()
    };
    // handed a port someone took: it stops as it starts, as on CI
    let status = start(taken).wait().unwrap();
    assert_eq!(status.code(), Some(101), "{status}");
    // its own port: it starts, and says which
    let mut own = start(0);
    let mut said = None;
    for _ in 0..200 {
        if let Some(p) = std::fs::read(dir.join("server.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
            said = p["port"].as_u64();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = own.kill();
    let _ = own.wait();
    let said = said.expect("it wrote server.json");
    assert!(said != 0 && said != u64::from(taken), "a port of its own: {said}");
    drop(held);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The grid starts a box on the port the box says it bound (its `server.json`, for its pid), never one picked for it.
#[tokio::test(flavor = "multi_thread")]
async fn the_grid_reaches_a_box_on_the_port_the_box_bound() {
    let g = rig("own-port", None).await;
    assert_eq!(g.open(&g.session()).await, "fake box");
    let home = g.dir.join("users").join(g.uid()).join("home");
    let said: serde_json::Value = serde_json::from_slice(&std::fs::read(home.join("server.json")).unwrap()).unwrap();
    assert_eq!(g.grid.boxes.port(&g.uid()).map(u64::from), said["port"].as_u64());
    g.done().await;
}

/// Regression: a running box whose process exits unexpectedly (crash or OOM) must be reaped to `stopped`
/// immediately so `/_grid/state` stops reporting `"running"` and the next request restarts it instead of
/// returning `502 Your wisp didn't answer: client error (Connect)` forever.
#[tokio::test(flavor = "multi_thread")]
async fn a_box_that_exits_unexpectedly_is_reaped_and_restarts_on_next_request() {
    let g = rig("crash-reap", None).await;
    let cookie = g.session();
    assert_eq!(g.open(&cookie).await, "fake box");
    let first_pid = g.pid();
    let port = g.grid.boxes.port(&g.uid()).expect("running port");
    // Ask the fake box to exit 137 (simulating an OOM kill after startup).
    let _ = g.c.get(format!("http://127.0.0.1:{port}/_die")).send().await;
    until("crashed box is reaped to stopped", || g.grid.boxes.state(&g.uid()) == BoxState::Stopped).await;
    until("crash recorded in instances", || g.row().0 == "stopped" && g.row().4.as_deref() == Some("out of memory")).await;
    assert_eq!(g.grid.boxes.port(&g.uid()), None);
    // The next request wakes a fresh box instead of hitting the dead port with 502.
    assert_eq!(g.open(&cookie).await, "fake box");
    assert_ne!(g.pid(), first_pid, "restarted as a new process");
    g.done().await;
}

fn bob() -> wisp_grid::rows::BoxUser {
    wisp_grid::rows::BoxUser { id: "u-bob".into(), handle: "bob".into(), sandbox: None, owner: None }
}

/// grid-r2-1: a browser opening a sleeping box gets its waiting page at once (not a blank tab until the box is up,
/// up to `startMs`), the box starts behind it, and the page's next load is the wisp.
#[tokio::test(flavor = "multi_thread")]
async fn a_browser_opening_a_sleeping_box_gets_its_waiting_page_at_once_then_the_wisp() {
    let g = rig("waiting-page", None).await;
    let cookie = g.box_cookie(&g.session()).await;
    let t = std::time::Instant::now();
    let r = g.at_box("/x", &cookie, "text/html,application/xhtml+xml").await;
    let took = t.elapsed();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["cache-control"], "no-store");
    let page = r.text().await.unwrap();
    assert!(page.contains("Waking your wisp") && page.contains("fetch('/_grid/state'"), "{page}");
    assert!(took < Duration::from_millis(1500), "answered before the box was up: {took:?}");
    until("the box started behind the page", || g.grid.boxes.state(&g.uid()) == BoxState::Running).await;
    assert_eq!(g.state(&cookie).await["state"], "running");
    let r = g.at_box("/x", &cookie, "text/html").await;
    assert_eq!(r.text().await.unwrap(), "fake box", "once it runs, the page's reload is the wisp");
    g.done().await;
}

/// grid-r2-1: a full grid puts a start in line. A caller that isn't a page hears 503 with when to ask again and its
/// place; `/_grid/state` says queued and where; once room frees, the box starts by itself.
#[tokio::test(flavor = "multi_thread")]
async fn a_full_grid_queues_a_start_and_starts_it_when_room_frees() {
    let g = rig_with("full", None, |o| o.max_running = 1).await;
    g.grid.boxes.ensure(&bob(), false).await.unwrap();
    let cookie = g.box_cookie(&g.session()).await;
    let r = g.at_box("/api/x", &cookie, "application/json").await;
    assert_eq!(r.status(), 503);
    assert_eq!(r.headers()["retry-after"], "10");
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!((body["queued"].as_bool(), body["position"].as_u64()), (Some(true), Some(1)), "{body}");
    let s = g.state(&cookie).await;
    assert_eq!((s["state"].as_str(), s["position"].as_u64()), (Some("queued"), Some(1)), "{s}");
    // a page says its place in line
    let page = g.at_box("/", &cookie, "text/html").await.text().await.unwrap();
    assert!(page.contains("Waiting for room") && page.contains("You’re next in line"), "{page}");
    g.grid.boxes.stop("u-bob", "idle").await.unwrap();
    until("ada's box starts once there's room", || g.grid.boxes.state(&g.uid()) == BoxState::Running).await;
    g.done().await;
}

/// grid-r2-1: a start that fails says why on the waiting page, with Try again (never the old dead end).
#[tokio::test(flavor = "multi_thread")]
async fn a_start_that_fails_says_why_on_the_waiting_page_with_try_again() {
    let g = rig_with("spawn-fails", None, |o| o.bin = "/nonexistent/wisp".into()).await;
    let cookie = g.box_cookie(&g.session()).await;
    let r = g.at_box("/", &cookie, "text/html").await;
    assert_eq!(r.status(), 200);
    let page = r.text().await.unwrap();
    assert!(page.contains(r#"id="again" type="button">Try again</button>"#) && !page.contains(">Back<"), "{page}");
    let mut s = serde_json::Value::Null;
    for _ in 0..100 {
        s = g.state(&cookie).await;
        if s["error"].is_string() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(s["state"], "stopped", "{s}");
    assert!(s["error"].as_str().unwrap().contains("wisp didn't start in the box"), "{s}");
    // opened again: the served page itself says why, with Try again showing, and doesn't start it again by itself
    let r = g.at_box("/", &cookie, "text/html").await;
    assert_eq!((r.status().as_u16(), r.headers()["cache-control"].to_str().unwrap()), (200, "no-store"));
    let page = r.text().await.unwrap();
    assert!(page.contains("Your wisp didn’t start") && page.contains("wisp didn&#39;t start in the box"), "{page}");
    assert!(page.contains(r#"id="acts" style="justify-content:center"><button class="primary tall" id="again""#), "{page}");
    assert_eq!(g.grid.boxes.state(&g.uid()), BoxState::Stopped, "no start of its own");
    // Try again: the old failure is gone at once (a poll never reads it back while the new start waits for its
    // gate), and a new start runs
    let mut changes = g.grid.boxes.changes();
    let r =
        g.c.post(format!("http://ada.wisp.localhost:{}/_grid/start", g.port))
            .header("cookie", &cookie)
            .header("origin", g.grid.cfg.instance_url("ada"))
            .send()
            .await
            .unwrap();
    assert_eq!(r.status(), 202);
    let started = tokio::time::timeout(Duration::from_secs(5), async {
        while let Ok(c) = changes.recv().await {
            if c.user.id == g.uid() && c.state == BoxState::Starting {
                return true;
            }
        }
        false
    });
    assert!(started.await.unwrap_or(false), "Try again started it again");
    until("the new start failed again", || g.grid.boxes.last_error(&g.uid()).is_some()).await;
    g.grid.boxes.forget_failure(&g.uid());
    assert!(g.state(&cookie).await["error"].is_null(), "a forgotten failure isn't read back");
    g.done().await;
}

/// hostd against a fake boxd whose boxes take their time (to answer, to stop) and can refuse to start: the races
/// between stops, starts and wakes, and the line for room. Unix only: boxd serves a Unix socket.
#[cfg(unix)]
mod races {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use bytes::Bytes;
    use http_body_util::Full;
    use hyper_util::rt::TokioIo;
    use serde_json::Value;
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
    use tokio::net::{TcpListener, UnixListener};
    use wisp_grid::boxhost::{BoxHost as _, BoxState};
    use wisp_grid::hostd::{ProcessHost, ProcessOptions};
    use wisp_grid::rows::BoxUser;

    #[derive(Default)]
    struct Fake {
        /// before a box answers
        health_ms: u64,
        /// between SIGTERM and its exit
        stop_ms: u64,
        /// runs refused (boxd's `error`) before one starts
        fail: AtomicUsize,
        /// boxes up now
        live: AtomicUsize,
    }

    fn fake_boxd(sock: &std::path::Path, fake: Arc<Fake>) {
        let l = UnixListener::bind(sock).unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((conn, _)) = l.accept().await else { return };
                let f = fake.clone();
                tokio::spawn(async move {
                    let (rd, mut wr) = conn.into_split();
                    let mut lines = BufReader::new(rd).lines();
                    let Ok(Some(first)) = lines.next_line().await else { return };
                    let m: Value = serde_json::from_str(&first).unwrap();
                    if m["op"] == "remove" {
                        let _ = wr.write_all(b"{\"ok\":true}\n").await;
                        return;
                    }
                    if f.fail.load(Ordering::SeqCst) > 0 {
                        f.fail.fetch_sub(1, Ordering::SeqCst);
                        let _ = wr.write_all(b"{\"error\":\"the host is out of memory\"}\n").await;
                        return;
                    }
                    let port = u16::try_from(m["port"].as_u64().unwrap()).unwrap();
                    tokio::time::sleep(Duration::from_millis(f.health_ms)).await;
                    let http = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
                    f.live.fetch_add(1, Ordering::SeqCst);
                    let server = tokio::spawn(async move {
                        loop {
                            let Ok((s, _)) = http.accept().await else { return };
                            tokio::spawn(async move {
                                let svc = hyper::service::service_fn(|_r| async {
                                    Ok::<_, std::convert::Infallible>(http::Response::new(Full::new(Bytes::from_static(b"{\"ok\":true}"))))
                                });
                                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(s), svc).await;
                            });
                        }
                    });
                    while let Ok(Some(l)) = lines.next_line().await {
                        if l.contains("SIGTERM") {
                            break;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(f.stop_ms)).await;
                    server.abort();
                    f.live.fetch_sub(1, Ordering::SeqCst);
                    let _ = wr.write_all(b"{\"exit\":0}\n").await;
                });
            }
        });
    }

    struct Host {
        h: Arc<ProcessHost>,
        f: Arc<Fake>,
        dir: std::path::PathBuf,
    }

    fn host(name: &str, f: Fake, tune: impl FnOnce(&mut ProcessOptions)) -> Host {
        let dir = std::env::temp_dir().join(format!("wisp-grid-race-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("box.sock");
        let f = Arc::new(f);
        fake_boxd(&sock, f.clone());
        let mut o = ProcessOptions {
            bin: "/nonexistent/wisp".into(),
            data: dir.clone(),
            idle_ms: 600_000,
            check_ms: 60_000,
            start_ms: 10_000,
            max_running: 4,
            max_running_per_user: 4,
            wake_retry_ms: 30_000,
            queue_ms: 120_000,
            env: Vec::new(),
            url_of: Arc::new(|h| format!("https://{h}.grid.test")),
            boxd: Some(wisp_grid::boxd::Driver { socket: sock, limits: Value::Null, dns: None }),
            previews: false,
        };
        tune(&mut o);
        Host { h: ProcessHost::new(o, wisp_grid::clock::system()), f, dir }
    }

    impl Host {
        async fn done(self) {
            self.h.close_all().await;
            let _ = std::fs::remove_dir_all(&self.dir);
        }
        fn live(&self) -> usize {
            self.f.live.load(Ordering::SeqCst)
        }
        async fn answers(&self, id: &str) -> bool {
            let Some(port) = self.h.port(id) else { return false };
            let c = reqwest::Client::builder().no_proxy().build().unwrap();
            c.get(format!("http://127.0.0.1:{port}/api/health")).send().await.is_ok_and(|r| r.status().is_success())
        }
    }

    fn user(id: &str) -> BoxUser {
        BoxUser { id: id.into(), handle: id.into(), sandbox: None, owner: None }
    }

    fn sandbox(owner: &str, n: u32) -> BoxUser {
        BoxUser {
            id: format!("{owner}-sb-{n}"),
            handle: format!("sb-{n}--{owner}"),
            sandbox: Some(n.to_string()),
            owner: Some(owner.into()),
        }
    }

    async fn until(what: &str, mut ok: impl FnMut() -> bool) {
        for _ in 0..200 {
            if ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("never: {what}");
    }

    /// grid-r2-3: a start asked for while the box is still stopping waits for the stop, and the stop's end never
    /// marks the new run stopped: the box ends up, on a port that answers, with one box and none orphaned. Before,
    /// the start began at once and the slow stop's tail wrote Stopped over it, its container still running.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_start_racing_a_slow_stop_waits_for_it_and_ends_up_running() {
        let x = host("stop-start", Fake { stop_ms: 800, ..Fake::default() }, |_| {});
        let ada = user("u1");
        x.h.ensure(&ada, false).await.unwrap();
        let (stopped, started) = tokio::join!(x.h.stop("u1", "idle"), async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            x.h.ensure(&ada, false).await
        });
        assert!(stopped.unwrap());
        started.unwrap();
        assert_eq!(x.h.state("u1"), BoxState::Running);
        assert!(x.answers("u1").await, "its port answers");
        assert!(x.h.secret("u1").is_some_and(|s| !s.is_empty()), "the edge still has its secret");
        assert_eq!(x.live(), 1, "the old box stopped and the new one isn't orphaned");
        x.done().await;
    }

    /// grid-r2-3: the idle tick's stop racing a scheduled wake (`ensure(…, wake: true)`) ends running, not stopped
    /// with a box still up.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_idle_stop_racing_a_scheduled_wake_ends_running() {
        let x = host("idle-wake", Fake { stop_ms: 800, ..Fake::default() }, |o| {
            o.idle_ms = 1_500;
            o.check_ms = 200;
        });
        let ada = user("u1");
        x.h.ensure(&ada, false).await.unwrap();
        until("the idle tick stops it", || x.h.state("u1") == BoxState::Stopping).await;
        let t = std::time::Instant::now();
        x.h.ensure(&ada, true).await.unwrap();
        // past the slow stop's end, well before the next idle stop
        if let Some(rest) = Duration::from_millis(1_000).checked_sub(t.elapsed()) {
            tokio::time::sleep(rest).await;
        }
        assert_eq!(x.h.state("u1"), BoxState::Running);
        assert!(x.answers("u1").await);
        assert_eq!(x.live(), 1);
        x.done().await;
    }

    /// grid-r2-6: room is counted and taken at once. Two cold starts for the last slot: one runs, the other waits in
    /// line (503). Before, both counted the box up (not the one starting) and both ran.
    #[tokio::test(flavor = "multi_thread")]
    async fn two_cold_starts_for_the_last_slot_run_one_and_queue_the_other() {
        let x = host("last-slot", Fake { health_ms: 400, ..Fake::default() }, |o| o.max_running = 1);
        let (ua, ub) = (user("a"), user("b"));
        let (a, b) = tokio::join!(x.h.ensure(&ua, false), x.h.ensure(&ub, false));
        assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1, "exactly one starts");
        let e = a.err().or(b.err()).unwrap();
        assert_eq!(e.status(), 503);
        let mut states = [x.h.state("a"), x.h.state("b")];
        states.sort_by_key(|s| s.as_str());
        assert_eq!(states, [BoxState::Queued, BoxState::Running]);
        assert_eq!(x.live(), 1);
        x.done().await;
    }

    /// grid-r2-6: `maxRunningPerUser` holds one person's sandboxes back without holding anyone else, or their own
    /// box, back.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_sandbox_past_its_owners_limit_waits_while_others_and_the_owners_box_start() {
        let x = host("per-user", Fake::default(), |o| {
            o.max_running = 10;
            o.max_running_per_user = 2;
        });
        x.h.ensure(&sandbox("o1", 1), false).await.unwrap();
        x.h.ensure(&sandbox("o1", 2), false).await.unwrap();
        let e = x.h.ensure(&sandbox("o1", 3), false).await.unwrap_err();
        assert_eq!(e.status(), 503);
        assert_eq!(x.h.state("o1-sb-3"), BoxState::Queued);
        x.h.ensure(&user("bob"), false).await.expect("another user still starts");
        x.h.ensure(&user("o1"), false).await.expect("the owner's own box is never held back");
        assert_eq!(x.h.state("o1-sb-3"), BoxState::Queued, "still waiting: the owner has 3 up");
        x.h.stop("o1-sb-1", "idle").await.unwrap();
        x.h.stop("o1", "idle").await.unwrap();
        until("the sandbox starts once its owner has room", || x.h.state("o1-sb-3") == BoxState::Running).await;
        x.done().await;
    }

    /// grid-r2-1/-6: two slots freeing at once start the ones in line that may start, never two sandboxes of an owner
    /// with room for one: the person behind them starts too. Before, drain picked sb-2 and sb-3 (counting the owner
    /// against the map only), sb-3 went back in line, and bob waited by an empty slot until some other box stopped.
    #[tokio::test(flavor = "multi_thread")]
    async fn two_slots_freeing_at_once_skip_an_owner_at_its_limit_and_start_the_person_behind() {
        let x = host("drain-owner", Fake::default(), |o| {
            o.max_running = 3;
            o.max_running_per_user = 2;
        });
        x.h.ensure(&sandbox("o1", 1), false).await.unwrap();
        x.h.ensure(&user("z1"), false).await.unwrap();
        x.h.ensure(&user("z2"), false).await.unwrap();
        for u in [sandbox("o1", 2), sandbox("o1", 3), user("bob")] {
            assert_eq!(x.h.ensure(&u, false).await.unwrap_err().status(), 503);
        }
        assert_eq!(x.h.queued_at("bob"), Some(3));
        let (a, b) = tokio::join!(x.h.stop("z1", "idle"), x.h.stop("z2", "idle"));
        assert!(a.unwrap() && b.unwrap(), "both were up");
        until("sb-2 and bob both start", || x.h.state("o1-sb-2") == BoxState::Running && x.h.state("bob") == BoxState::Running).await;
        assert_eq!((x.h.state("o1-sb-3"), x.h.queued_at("o1-sb-3")), (BoxState::Queued, Some(1)), "its owner is at 2");
        x.done().await;
    }

    /// The line forgets a start no one waits for any more (`queueMs` with no poll, no retry: a closed tab), so it never
    /// takes a slot later; one whose page still polls keeps its place.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_start_no_one_waits_for_leaves_the_line_and_one_still_polled_keeps_its_place() {
        let x = host("line-expiry", Fake::default(), |o| {
            o.max_running = 1;
            o.queue_ms = 400;
            o.check_ms = 200;
        });
        x.h.ensure(&user("a"), false).await.unwrap();
        x.h.ensure(&user("gone"), false).await.unwrap_err();
        x.h.ensure(&user("here"), false).await.unwrap_err();
        for _ in 0..12 {
            x.h.still_waiting("here");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!((x.h.state("gone"), x.h.queued_at("gone")), (BoxState::Stopped, None), "a closed tab left the line");
        assert_eq!(x.h.queued_at("here"), Some(1), "a page still polling moved up");
        x.h.stop("a", "idle").await.unwrap();
        until("the one still waited for starts", || x.h.state("here") == BoxState::Running).await;
        assert_eq!(x.h.state("gone"), BoxState::Stopped);
        x.done().await;
    }

    /// grid-r2-1: with no room the second box waits at place 1 in line, and starts once the first stops.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_box_in_line_starts_when_the_one_running_stops() {
        let x = host("line", Fake::default(), |o| o.max_running = 1);
        x.h.ensure(&user("a"), false).await.unwrap();
        let e = x.h.ensure(&user("b"), false).await.unwrap_err();
        assert!(e.to_string().contains("You’re next in line"), "{e}");
        assert_eq!((x.h.state("b"), x.h.queued_at("b")), (BoxState::Queued, Some(1)));
        x.h.stop("a", "idle").await.unwrap();
        until("b starts", || x.h.state("b") == BoxState::Running).await;
        assert_eq!(x.h.queued_at("b"), None);
        x.done().await;
    }

    /// grid-r2-2: a scheduled wake with no room waits in line (`wake queued`) while the grid is full, and starts once
    /// there's room. Before, it failed, was logged, and the box slept through its task.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_scheduled_wake_on_a_full_grid_waits_in_line_and_wakes_once_theres_room() {
        let x = host("wake-full", Fake::default(), |o| o.max_running = 1);
        let mut changes = x.h.changes();
        x.h.ensure(&user("a"), false).await.unwrap();
        x.h.restore(&user("b"), Some(wisp_grid::clock::system().now_ms()));
        until("b's wake waits in line", || x.h.state("b") == BoxState::Queued).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(x.h.state("b"), BoxState::Queued, "not while a is up");
        let mut said = Vec::new();
        while let Ok(c) = changes.try_recv() {
            said.push((c.user.id, c.event));
        }
        assert!(said.contains(&("b".into(), "wake queued".into())), "{said:?}");
        x.h.stop("a", "idle").await.unwrap();
        until("b wakes once a stops", || x.h.state("b") == BoxState::Running).await;
        x.done().await;
    }

    /// grid-r2-2: a wake whose start fails says so (`wake failed`, with why, its schedule kept) and is tried again,
    /// and then runs. Before, its schedule was cleared as it began and nothing tried again.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_scheduled_wake_that_fails_keeps_its_schedule_and_is_tried_again() {
        let fake = Fake { fail: AtomicUsize::new(1), ..Fake::default() };
        let x = host("wake-fails", fake, |o| o.wake_retry_ms = 200);
        let mut changes = x.h.changes();
        let at = wisp_grid::clock::system().now_ms();
        x.h.restore(&user("b"), Some(at));
        until("b wakes on its second try", || x.h.state("b") == BoxState::Running).await;
        let mut failed = None;
        while let Ok(c) = changes.try_recv() {
            if c.event == "wake failed" {
                failed = Some(c);
            }
        }
        let c = failed.expect("a `wake failed` change");
        assert_eq!((c.state, c.next_wake_at), (BoxState::Stopped, Some(at)), "its schedule kept through the failure");
        assert!(c.error.contains("out of memory"), "{}", c.error);
        x.done().await;
    }
}
