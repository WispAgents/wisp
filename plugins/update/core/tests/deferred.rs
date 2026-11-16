//! Windows `wisp update`: npm runs in the hand-off once wisp.exe has exited (`wisp_update_core::deferred`), and the
//! next start says what it did. The script is a golden: changing it is a deliberate act,
//! `UPDATE_GOLDENS=1 cargo test -p wisp-update-core --test deferred`. The run itself is
//! `wisp-update`'s `hand_off` test (Windows CI).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests may fail loudly
use std::path::Path;

use serde_json::json;
use wisp_update_core::deferred::{self, Files, Outcome, Pending, ps_quote};
use wisp_update_core::{Env, InstallStep, Method, Platform, Status, Updater, detect};

const ROOT: &str = "C:\\Users\\Ann O'Brien\\AppData\\Roaming\\npm\\node_modules\\wisp-agents";

fn win() -> Platform {
    Platform::new("win32", "x64")
}

fn exe() -> String {
    format!("{ROOT}\\node_modules\\@wisp\\core-win32-x64\\bin\\wisp.exe")
}

fn files() -> Files {
    Files { dir: "C:\\Users\\Ann O'Brien\\.wisp\\plugins\\wisp.update".into(), exe: exe() }
}

fn windows_npm(latest: &str) -> Updater {
    let mut u = Updater::new(detect(&exe(), &win(), None, false, None), win(), "2.0.0", 4477, Env::default());
    assert_eq!(u.method(), Method::Global);
    let req = u.begin_check(0, "stable");
    u.end_check(&req, Some(&json!({ "latest": latest })));
    u
}

fn pending() -> Pending {
    Pending { from: "2.0.0".into(), to: "2.1.0".into(), cmd: "npm install -g wisp-agents@latest".into() }
}

/// npm swaps the folder wisp.exe runs from, and Windows locks a running exe: the install waits for the hand-off.
#[test]
fn a_windows_npm_install_waits_for_wisp_to_exit() {
    let mut u = windows_npm("2.1.0");
    let step = u.begin_install().unwrap();
    assert_eq!(step, InstallStep::AfterExit { cmd: "npm install -g wisp-agents@latest".into() });
    assert_eq!(u.status, Status::Installing);
    assert_eq!(u.pending(), Some(pending()), "what the next start compares with");
    assert_eq!(u.hand_off(9, &files()), None, "no relaunch before the install is handed off");
    u.install_done();
    let h = u.hand_off(9, &files()).expect("a hand-off");
    assert_eq!(h.bin, "powershell");
    assert_eq!(h.args[5], wisp_update_core::encode_ps(&h.script), "the script travels encoded");
    // in order: stop, wait (bounded), give up if it's still there, set the exe aside, npm, its exit, restart
    let at = |needle: &str| h.script.find(needle).unwrap_or_else(|| panic!("no {needle:?} in {}", h.script));
    let steps = [
        "Stop-Process -Id 9 -Force",
        "Wait-Process -Id 9 -Timeout 30",
        "if (Get-Process -Id 9",
        "Copy-Item -LiteralPath",
        "npm install -g wisp-agents@latest > ",
        "-Wait -PassThru",
        "Set-Content -LiteralPath 'C:\\Users\\Ann O''Brien\\.wisp\\plugins\\wisp.update\\update-exit.txt' -Value $code",
        "if (Test-Path -LiteralPath",
    ];
    let order: Vec<usize> = steps.iter().map(|s| at(s)).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{steps:?} out of order in {}", h.script);
    assert!(h.script.contains(&u.relaunch_command(false).replace('\'', "''")), "the same relaunch as after any update: {}", h.script);
}

/// The other installs, and every install off Windows, run while wisp does (a running binary on macOS and Linux is
/// an open inode npm's rename leaves alone): only a Windows npm install is deferred.
#[test]
fn only_a_windows_npm_install_is_deferred() {
    let mut mac = Updater::new(
        detect(
            "/usr/local/lib/node_modules/wisp-agents/node_modules/@wisp/core-darwin-arm64/bin/wisp",
            &Platform::new("darwin", "arm64"),
            None,
            false,
            None,
        ),
        Platform::new("darwin", "arm64"),
        "2.0.0",
        4477,
        Env::default(),
    );
    mac.latest = Some("2.1.0".into());
    assert!(matches!(mac.begin_install(), Ok(InstallStep::Run { .. })));
    assert_eq!(mac.pending(), None);
    mac.install_done();
    assert_eq!(mac.hand_off(9, &files()).unwrap().bin, "sh");
    let mut npx = windows_npm("2.1.0");
    npx.install.method = Method::Npx;
    assert_eq!(npx.begin_install(), Ok(InstallStep::Nothing), "npx installs on relaunch, after wisp is gone");
    assert_eq!(npx.pending(), None);
}

/// A failed or refused hand-off forgets the deferred install, so a retry starts clean.
#[test]
fn a_failed_install_forgets_what_it_deferred() {
    let mut u = windows_npm("2.1.0");
    let _ = u.begin_install().unwrap();
    u.install_failed("The update couldn't start.");
    assert_eq!((u.status, u.pending()), (Status::Idle, None));
}

#[test]
fn the_script_is_the_golden() {
    let mut u = windows_npm("2.1.0");
    let _ = u.begin_install().unwrap();
    u.install_done();
    let script = u.hand_off(4242, &files()).unwrap().script;
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/windows-hand-off.ps1");
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, format!("{script}\n")).unwrap();
    }
    let golden = std::fs::read_to_string(&file).unwrap_or_default();
    assert_eq!(golden.trim_end(), script, "the hand-off script changed (UPDATE_GOLDENS=1 to accept)");
}

/// Paths with spaces and quotes: inside `PowerShell` single quotes (nothing expands, `'` doubled), and inside the
/// `cmd /S /C "…"` line in double quotes (Windows paths can't hold a `"`).
#[test]
fn paths_with_spaces_and_quotes_survive_both_shells() {
    assert_eq!(ps_quote("C:\\Ann O'Brien\\$env:x"), "'C:\\Ann O''Brien\\$env:x'");
    assert_eq!(ps_quote("C:\\Ann O\u{2019}Brien"), "'C:\\Ann O\u{2019}\u{2019}Brien'", "PowerShell ends a string at ’ too");
    let mut u = windows_npm("2.1.0");
    let _ = u.begin_install().unwrap();
    u.install_done();
    let s = u.hand_off(1, &files()).unwrap().script;
    assert!(
        s.contains("-ArgumentList '/d /s /c \"npm install -g wisp-agents@latest > \"C:\\Users\\Ann O''Brien\\.wisp\\plugins\\wisp.update\\update-npm.log\" 2>&1\"'"),
        "{s}"
    );
    assert!(s.contains(&format!("Copy-Item -LiteralPath {} -Destination", ps_quote(&exe()))), "{s}");
    assert!(s.contains("Start-Process -FilePath 'C:\\Users\\Ann O''Brien\\.wisp\\plugins\\wisp.update\\wisp-old.exe' -ArgumentList '--browser --no-open --port 4477'"), "{s}");
    assert!(!s.contains("O'B"), "every quote doubled: {s}");
}

#[test]
fn the_next_start_says_what_the_install_did() {
    let p = pending();
    assert_eq!(deferred::outcome(&p, Some("0\r\n"), "", "2.1.0"), Outcome::Updated { from: "2.0.0".into(), to: "2.1.0".into() });
    assert_eq!(
        deferred::outcome(&p, Some("0"), "", "2.0.0"),
        Outcome::Failed("npm installed wisp somewhere else: this copy is still 2.0.0.".into()),
        "npm said yes, but this is still the old copy"
    );
    assert_eq!(
        deferred::outcome(&p, Some("1"), "npm warn x\nnpm error code EBUSY\nnpm error syscall rename\nnpm error path C:\\x\n", "2.0.0"),
        Outcome::Failed(
            "`npm install -g wisp-agents@latest` failed: npm error code EBUSY npm error syscall rename npm error path C:\\x".into()
        ),
        "npm's last lines, as when it runs while wisp does"
    );
    assert_eq!(
        deferred::outcome(&p, Some("4294963238"), " \n", "2.0.0"),
        Outcome::Failed("`npm install -g wisp-agents@latest` failed: exit code 4294963238".into()),
        "no output: the exit code"
    );
    assert_eq!(
        deferred::outcome(&p, Some("busy"), "", "2.0.0"),
        Outcome::Failed("wisp didn't stop within 30 seconds, so `npm install -g wisp-agents@latest` didn't run. Try again.".into())
    );
    assert_eq!(
        deferred::outcome(&p, None, "", "2.0.0"),
        Outcome::Failed("The update to 2.1.0 didn't finish. Run `npm install -g wisp-agents@latest` yourself, then start wisp.".into()),
        "the waiter never wrote its answer"
    );
    assert_eq!(
        deferred::outcome(&p, Some("start"), "", "2.0.0"),
        Outcome::Failed("The update couldn't start `npm install -g wisp-agents@latest` (cmd.exe didn't run). Run it yourself.".into()),
        "cmd.exe itself didn't start"
    );
}

/// The start that reads a failure puts it on the row: idle, with the reason, so the page toasts it.
#[test]
fn a_failure_read_at_start_is_the_rows_error() {
    let mut u = windows_npm("2.1.0");
    u.resume(&Outcome::Failed("`npm` failed: EBUSY".into()));
    let v = u.view("stable");
    assert_eq!((v.status, v.error.as_deref()), (Status::Idle, Some("`npm` failed: EBUSY")));
    let mut ok = windows_npm("2.1.0");
    ok.resume(&Outcome::Updated { from: "1.9.0".into(), to: "2.0.0".into() });
    assert_eq!(ok.view("stable").error, None);
}
