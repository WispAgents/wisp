//! Two homes, one port (audit cli-r3): a CLI for home B finding home A's wisp on the port refuses it, in one line, and
//! starts nothing; the CLI for home A uses it as before.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{isolate, start, temp};

/// `wisp …` for `home`, pointed at `port` through `WISP_PORT` (not `--port`), stdin closed.
fn wisp(dir: &Path, home: &Path, port: u16, args: &[&str]) -> (i32, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_wisp"));
    isolate(&mut cmd, dir);
    let o = cmd
        .args(args)
        .env("WISP_HOME", home)
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stderr).into_owned())
}

#[test]
fn a_wisp_for_another_home_on_the_port_is_refused_and_ours_is_used() {
    let dir = temp("cross-home");
    let (_serve, port) = start(&dir);
    let other = dir.join("home-b");
    std::fs::create_dir_all(&other).unwrap();

    let (code, err) = wisp(&dir, &other, port, &["ls", "--json"]);
    assert_ne!(code, 0, "home B ran its command on home A's wisp: {err}");
    assert!(err.contains(&format!("port {port} is the wisp for another home, not {}", other.display())), "{err}");
    assert_eq!(err.trim().lines().count(), 1, "one clear line: {err}");
    assert!(!other.join("server.json").exists(), "no second server for home B");

    // an explicit --port is no way around it: the home still doesn't match
    let (code, err) = wisp(&dir, &other, port, &["ls", "--json", "--port", &port.to_string()]);
    assert_ne!(code, 0, "{err}");
    assert!(err.contains("is the wisp for another home"), "{err}");

    let (code, err) = wisp(&dir, &dir.join("home"), port, &["ls", "--json"]);
    assert_eq!(code, 0, "home A's own wisp: {err}");
}
