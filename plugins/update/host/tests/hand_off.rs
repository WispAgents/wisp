//! Windows `wisp update` (`wisp_update_core::deferred`): the start after a hand-off reads what npm did and forgets it
//! (every OS), and on Windows the real `PowerShell` waiter stops "wisp", runs a fake npm and restarts it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests may fail loudly

use std::path::{Path, PathBuf};

use wisp_update::read_hand_off;
use wisp_update_core::deferred::{self, Outcome, Pending};

/// A fresh folder whose path has a space and a quote, as `C:\Users\Ann O'Brien` has.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wisp update O'Brien {name} {}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_pending(dir: &Path) {
    let p = Pending { from: "2.0.0".into(), to: "2.1.0".into(), cmd: "npm install -g wisp-agents@latest".into() };
    std::fs::write(dir.join(deferred::PENDING), serde_json::to_vec(&p).unwrap()).unwrap();
}

#[tokio::test]
async fn the_start_after_a_failed_hand_off_says_why_once() {
    let dir = scratch("failed");
    write_pending(&dir);
    std::fs::write(dir.join(deferred::EXIT), "1\r\n").unwrap();
    std::fs::write(dir.join(deferred::LOG), b"npm error code EBUSY\r\nnpm error syscall rename\r\n\xff\r\n").unwrap();
    let o = read_hand_off(&dir, "2.0.0").await;
    assert_eq!(
        o,
        Some(Outcome::Failed("`npm install -g wisp-agents@latest` failed: npm error code EBUSY npm error syscall rename \u{fffd}".into())),
        "npm's last lines, read lossily"
    );
    for f in [deferred::PENDING, deferred::EXIT, deferred::LOG] {
        assert!(!dir.join(f).exists(), "{f} is read once");
    }
    assert_eq!(read_hand_off(&dir, "2.0.0").await, None, "the start after that has nothing to say");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn the_start_after_a_good_hand_off_is_the_new_version() {
    let dir = scratch("updated");
    write_pending(&dir);
    std::fs::write(dir.join(deferred::EXIT), "0").unwrap();
    assert_eq!(read_hand_off(&dir, "2.1.0").await, Some(Outcome::Updated { from: "2.0.0".into(), to: "2.1.0".into() }));
    assert_eq!(read_hand_off(&dir, "2.1.0").await, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn no_hand_off_no_word() {
    let dir = scratch("none");
    assert_eq!(read_hand_off(&dir, "2.0.0").await, None);
    std::fs::write(dir.join(deferred::PENDING), "not json").unwrap();
    assert_eq!(read_hand_off(&dir, "2.0.0").await, None, "a torn pending file is no update");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The real waiter on Windows: "wisp" (a sleeping process) is stopped and waited for, npm (a fake `npm.cmd` first on
/// PATH) runs once it's gone and fails, and the old copy restarts through the launcher with the same args; the next
/// start reads npm's error.
#[cfg(windows)]
#[tokio::test]
async fn the_windows_waiter_installs_after_wisp_exits_and_restarts_it_when_npm_fails() {
    use std::time::{Duration, Instant};
    use wisp_update_core::deferred::Files;
    use wisp_update_core::{Env, Platform, Updater, detect};

    let base = scratch("waiter");
    let root = base.join("npm").join("node_modules").join("wisp-agents");
    let bin = root.join("node_modules").join("@wisp").join("core-win32-x64").join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("wisp.exe");
    std::fs::write(&exe, b"old").unwrap();
    let plugin = base.join("home").join("plugins").join("wisp.update");
    std::fs::create_dir_all(&plugin).unwrap();
    let tools = base.join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    // npm: fails the way it does when wisp.exe is locked, after checking "wisp" is gone
    let marker = base.join("npm-ran.txt");
    std::fs::write(
        tools.join("npm.cmd"),
        format!(
            "@echo off\r\necho %* > \"{}\"\r\necho npm error code EBUSY 1>&2\r\necho npm error syscall rename 1>&2\r\nexit /b 1\r\n",
            marker.display()
        ),
    )
    .unwrap();
    // node: the relaunch, which records its arguments
    let restarted = base.join("restarted.txt");
    let node = tools.join("node.cmd");
    std::fs::write(&node, format!("@echo off\r\necho %* > \"{}\"\r\n", restarted.display())).unwrap();

    let win = Platform::new("win32", "x64");
    let env = Env { node: Some(node.display().to_string()), ..Env::default() };
    let mut u = Updater::new(detect(&exe.display().to_string(), &win, None, false, None), win, "2.0.0", 4477, env);
    u.latest = Some("2.1.0".into());
    u.begin_install().unwrap();
    u.install_done();
    std::fs::write(plugin.join(deferred::PENDING), serde_json::to_vec(&u.pending().unwrap()).unwrap()).unwrap();

    let mut wisp = std::process::Command::new("powershell").args(["-NoProfile", "-Command", "Start-Sleep -Seconds 120"]).spawn().unwrap();
    let files = Files { dir: plugin.display().to_string(), exe: exe.display().to_string() };
    let h = u.hand_off(wisp.id(), &files).unwrap();
    let path = format!("{};{}", tools.display(), std::env::var("PATH").unwrap_or_default());
    let mut waiter = std::process::Command::new(h.bin).args(&h.args).env("PATH", path).spawn().unwrap();

    let start = Instant::now();
    // node.cmd may still be writing when the file appears
    while !std::fs::read_to_string(&restarted).is_ok_and(|t| t.contains("4477")) && start.elapsed() < Duration::from_secs(90) {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let _ = waiter.wait();
    assert!(wisp.try_wait().unwrap().is_some(), "the waiter stopped wisp");
    assert!(marker.exists(), "npm ran");
    let args = std::fs::read_to_string(&restarted).unwrap_or_else(|_| panic!("wisp was restarted (script: {})", h.script));
    assert!(args.contains("wisp.js") && args.contains("--browser --no-open --port 4477"), "same args: {args}");
    assert!(exe.exists() && !plugin.join(deferred::BACKUP).exists(), "the copy set aside goes once wisp.exe is still there");
    let o = read_hand_off(&plugin, "2.0.0").await.unwrap();
    assert_eq!(o, Outcome::Failed("`npm install -g wisp-agents@latest` failed: npm error code EBUSY npm error syscall rename".into()));
    let _ = std::fs::remove_dir_all(&base);
}
