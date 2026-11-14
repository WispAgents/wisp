//! `wisp` with no command (what `npx wisp-agents` runs), `--browser` and `wisp desktop` (F-cli-9): the real
//! binary with a fake app (`WISP_DESKTOP_APP`) and a fake browser (`WISP_BROWSER`), so nothing real opens. The
//! running wisp is found from server.json and shown in the app, `--browser` forces a tab, no app means a tab and a
//! line saying how to get it, and a port held by something else starts nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{script, start, temp};

struct Fakes {
    app: PathBuf,
    browser: PathBuf,
    app_log: PathBuf,
    browser_log: PathBuf,
}

/// The app records the port it was handed and whether it was given a server binary, then (with `FAKE_WISP` set)
/// runs that wisp's server as the real app does when none answers. The browser records the address.
fn fakes(dir: &Path) -> Fakes {
    let app_log = dir.join("app.log");
    let browser_log = dir.join("browser.log");
    let app = script(
        &dir.join("fake-app"),
        &format!(
            "#!/bin/sh\necho \"port=$WISP_PORT bin=${{WISP_BIN:-}}\" >> '{}'\nif [ -n \"$FAKE_WISP\" ]; then exec \"$FAKE_WISP\" serve >/dev/null 2>&1; fi\n",
            app_log.display()
        ),
        &format!(
            "echo port=%WISP_PORT% bin=%WISP_BIN%>> \"{}\"\nif defined FAKE_WISP \"%FAKE_WISP%\" serve >NUL 2>&1\n",
            app_log.display()
        ),
    );
    let browser = script(
        &dir.join("fake-browser"),
        &format!("#!/bin/sh\necho \"$1\" >> '{}'\n", browser_log.display()),
        &format!("echo %~1>> \"{}\"\n", browser_log.display()),
    );
    Fakes { app, browser, app_log, browser_log }
}

/// `wisp …` as a user types it, without `WISP_PORT`: the port comes from the home's server.json.
fn wisp(dir: &Path, home: &str, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env_remove("WISP_PORT")
        .env_remove("FAKE_WISP")
        .env("WISP_HOME", dir.join(home))
        .env("NO_COLOR", "1")
        .env("WISP_WEB_DIR", dir.join("no-web"))
        // no release to fetch a shell from: refused at once
        .env("WISP_SHELL_URL", "http://127.0.0.1:9/none")
        .envs(env.iter().copied())
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// What a detached fake wrote, once it has (it outlives `wisp`).
fn written(path: &Path) -> String {
    let at = Instant::now();
    while at.elapsed() < Duration::from_secs(10) {
        if let Ok(s) = std::fs::read_to_string(path).map(|s| s.trim().to_owned())
            && !s.is_empty()
        {
            return s;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{} was never written", path.display())
}

/// Ends the server the fake app started.
struct Stop(Option<u64>);

impl Drop for Stop {
    fn drop(&mut self) {
        let Some(pid) = self.0.map(|p| p.to_string()) else { return };
        let _ = if cfg!(windows) {
            Command::new("taskkill").args(["/PID", &pid, "/T", "/F"]).output()
        } else {
            Command::new("kill").arg(&pid).output()
        };
    }
}

fn settle() {
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn wisp_shows_the_running_wisp_in_the_app_or_a_tab() {
    let dir = temp("serve-desktop");
    let (serve, port) = start(&dir);
    let f = fakes(&dir);
    let (app, browser) = (f.app.to_str().unwrap(), f.browser.to_str().unwrap());

    // the_running_wisp_opens_in_the_app: the port is server.json's (not 4477), and no second server starts
    let (code, out, err) = wisp(&dir, "home", &[], &[("WISP_DESKTOP_APP", app), ("WISP_BROWSER", browser)]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains("wisp is open"), "{err}");
    assert_eq!(written(&f.app_log), format!("port={port} bin="), "an installed app runs its own server, never this binary");
    settle();
    assert!(!f.browser_log.exists(), "no tab when the app opens");

    // browser_forces_a_tab
    std::fs::remove_file(&f.app_log).unwrap();
    let (code, out, err) = wisp(&dir, "home", &["--browser"], &[("WISP_DESKTOP_APP", app), ("WISP_BROWSER", browser)]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(written(&f.browser_log), format!("http://127.0.0.1:{port}/"));
    settle();
    assert!(!f.app_log.exists(), "--browser never opens the app");

    // no_app_opens_a_tab_and_says_where_the_app_is
    std::fs::remove_file(&f.browser_log).unwrap();
    let (code, out, err) = wisp(&dir, "home", &[], &[("WISP_DESKTOP_APP", "none"), ("WISP_BROWSER", browser)]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains("no wisp app here") && err.contains("opening a browser tab") && err.contains("trywisp.dev"), "{err}");
    assert_eq!(written(&f.browser_log), format!("http://127.0.0.1:{port}/"));

    // no_open_opens_nothing
    std::fs::remove_file(&f.browser_log).unwrap();
    let (code, out, err) = wisp(&dir, "home", &["--no-open"], &[("WISP_DESKTOP_APP", app), ("WISP_BROWSER", browser)]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains(&format!("wisp is open at http://127.0.0.1:{port}")), "{err}");
    settle();
    assert!(!f.app_log.exists() && !f.browser_log.exists(), "--no-open opens nothing");

    // desktop_opens_the_app
    let (code, out, err) = wisp(&dir, "home", &["desktop"], &[("WISP_DESKTOP_APP", app), ("WISP_BROWSER", browser)]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(written(&f.app_log), format!("port={port} bin="));

    // desktop_without_an_app_says_how_to_get_it, and opens no tab
    let (code, out, err) = wisp(&dir, "home", &["desktop"], &[("WISP_DESKTOP_APP", "none"), ("WISP_BROWSER", browser)]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("no wisp app here") && err.contains("Get it at https://trywisp.dev/#install") && err.contains("wisp --browser"),
        "{err}"
    );
    settle();
    assert!(!f.browser_log.exists(), "wisp desktop never falls back to a tab");
    drop(serve);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_port_held_by_something_else_starts_nothing() {
    let dir = temp("serve-desktop-taken");
    let f = fakes(&dir);
    // listening, but no wisp: every connection is taken and never answered
    let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port().to_string();
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        for c in held.incoming() {
            kept.push(c);
        }
    });
    let env = [("WISP_PORT", port.as_str()), ("WISP_DESKTOP_APP", f.app.to_str().unwrap()), ("WISP_BROWSER", f.browser.to_str().unwrap())];
    for args in [&[][..], &["--browser"], &["desktop"]] {
        let (code, out, err) = wisp(&dir, "home", args, &env);
        assert_eq!(code, 1, "{args:?}: {out}{err}");
        assert!(err.contains(&format!("port {port} is taken by something that isn't wisp")), "{args:?}: {err}");
    }
    settle();
    assert!(!f.app_log.exists() && !f.browser_log.exists(), "nothing opened");
    assert!(!dir.join("home/server.json").exists(), "no second server");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_no_wisp_running_the_app_starts_one() {
    let dir = temp("serve-desktop-start");
    let f = fakes(&dir);
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port().to_string();
    let (code, out, err) = wisp(
        &dir,
        "home",
        &[],
        &[
            ("WISP_PORT", port.as_str()),
            ("WISP_DESKTOP_APP", f.app.to_str().unwrap()),
            ("WISP_BROWSER", f.browser.to_str().unwrap()),
            ("FAKE_WISP", env!("CARGO_BIN_EXE_wisp")),
        ],
    );
    let info: serde_json::Value =
        std::fs::read(dir.join("home/server.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    // the app's server outlives `wisp`: stopped when the test ends, pass or fail
    let _stop = Stop(info["pid"].as_u64());
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains("wisp is open"), "it waited for the app's server: {err}");
    assert_eq!(info["port"].to_string(), port, "the app's server is on the port asked for: {info}");
    assert_eq!(written(&f.app_log), format!("port={port} bin="));
    assert!(!f.browser_log.exists(), "no tab");
    let _ = std::fs::remove_dir_all(&dir);
}
