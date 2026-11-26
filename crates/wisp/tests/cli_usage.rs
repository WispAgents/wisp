//! The CLI's usage with nothing running (audit of 2026-11-26, H1 and M3): `--help` after a built-in command, and a
//! group's line typed wrong, say the usage and exit; they never run the command nor start a background wisp.
//!   - H1: `wisp destroy --help` planned a destroy, `wisp graph --help` printed the digraph, `wisp workspace new --help`
//!     made `.wisp/workspaces/--help.vars.yml`; each started a wisp to do it.
//!   - M3: `wisp serve --port N --bogus` started a daemon on N to read the catalogue, then refused the line.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{isolate, temp};

fn wisp(dir: &Path, port: u16, args: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_wisp"));
    isolate(&mut cmd, dir);
    let o = cmd
        .args(args)
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .env("NO_COLOR", "1")
        .env("WISP_UPDATE_CHECK", "0")
        .env_remove("WISP_TASK")
        .current_dir(dir.join("proj"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// A port nothing listens on (taken from the OS, then let go).
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn nothing_started(dir: &Path, port: u16, what: &str) {
    assert!(!dir.join("home").join("server.json").exists(), "{what}: a wisp was started (server.json)");
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "{what}: something listens on {port}");
}

#[test]
fn serve_bogus_and_help_after_a_built_in_start_nothing_and_run_nothing() {
    let dir = temp("cli-usage");
    std::fs::create_dir_all(dir.join("home")).unwrap();
    // a declared folder, so destroy, graph and workspace have something to act on
    std::fs::create_dir_all(dir.join("proj/.wisp")).unwrap();
    std::fs::write(dir.join("proj/.wisp/wisp.yml"), "agents: {}\n").unwrap();
    let port = free_port();
    let p = port.to_string();

    // M3: a mistyped group line is refused with its usage, exit 2
    for args in [&["serve", "--port", &p, "--bogus"][..], &["serve", "x"]] {
        let (code, out, err) = wisp(&dir, port, args);
        assert!(code == 2 && err.contains("wisp serve"), "{args:?} → {code}: {out}{err}");
        nothing_started(&dir, port, &args.join(" "));
    }

    // H1: the command's usage, exit 0, whatever follows
    for (args, line) in [
        (&["destroy", "--help"][..], "wisp plan|destroy"),
        (&["validate", "--help"], "wisp diff|validate"),
        (&["graph", "--help"], "wisp graph"),
        (&["mv", "--help"], "wisp mv FROM TO"),
        (&["workspace", "new", "--help"], "wisp workspace"),
        (&["run", "-f", "x.task.yml", "--help"], "wisp run -f x.task.yml"),
        (&["apply", "-f", "x.yml", "--help"], "wisp apply -f wisp.yml"),
        (&["secret", "set", "FOO", "--help"], "wisp secret set|rm NAME"),
        (&["tui", "--help"], "wisp tui"),
        (&["schema", "--help"], "wisp schema"),
        (&["list", "--help"], "wisp ls"),
        (&["ship", "--help"], "wisp ship <id>"),
    ] {
        let (code, out, err) = wisp(&dir, port, args);
        assert!(
            code == 0 && out.contains(line) && !out.contains("digraph") && !out.contains("needs more"),
            "{args:?} → {code}: {out}{err}"
        );
        nothing_started(&dir, port, &args.join(" "));
    }
    assert!(!dir.join("proj/.wisp/workspaces/--help.vars.yml").exists(), "workspace new --help made a workspace");
    let (code, out, _) = wisp(&dir, port, &["--help"]);
    assert!(code == 0 && out.contains("\n  wisp tui "), "wisp --help names wisp tui: {out}");
}
