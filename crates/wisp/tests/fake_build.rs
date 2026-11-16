//! The fake harness build in `common` is safe when several test processes start it at once, also while
//! a fake in place is running (Windows can't rename over a running executable).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime};

const CHILD: &str = "WISP_FAKE_BUILD_OUT";
const HOLD: &str = "WISP_FAKE_BUILD_HOLD";

/// One of the racing processes: builds into the shared `out`, then runs the agent fake.
#[test]
fn fake_build_child() {
    let Some(out) = std::env::var_os(CHILD) else { return };
    let (agent, gh) = common::fake_bins_into(Path::new(&out));
    assert!(gh.is_file(), "{}", gh.display());
    let o = Command::new(&agent).arg("--version").stdin(Stdio::null()).output().unwrap();
    assert!(o.status.success() && String::from_utf8_lossy(&o.stdout).contains("9.9.9"), "{} --version: {o:?}", agent.display());
}

/// A copy of this test binary placed as a stale fake: runs until its stdin closes.
#[test]
fn hold_child() {
    if std::env::var_os(HOLD).is_none() {
        return;
    }
    let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut Vec::new());
}

/// Runs `exe` until the returned child's stdin is dropped: the fake waits for its prompt, this binary in
/// [`hold_child`]. Either way the file is a running executable for as long as the race lasts.
fn hold(exe: &Path) -> Child {
    Command::new(exe)
        .args(["--exact", "hold_child", "--nocapture"])
        .env(HOLD, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

fn release(mut c: Child) {
    assert!(c.try_wait().unwrap().is_none(), "the held executable ran for the whole race");
    drop(c.stdin.take());
    let _ = c.kill();
    let _ = c.wait();
}

/// Six processes build and place the fakes into `out` at once; each must pass.
fn race(out: &Path) {
    let me = std::env::current_exe().unwrap();
    let kids: Vec<_> = (0..6)
        .map(|_| {
            Command::new(&me)
                .args(["--exact", "fake_build_child", "--nocapture", "--test-threads=1"])
                .env(CHILD, out)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for k in kids {
        let o = k.wait_with_output().unwrap();
        assert!(
            o.status.success(),
            "a racing build failed:\n{}\n{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(String::from_utf8_lossy(&o.stdout).contains("1 passed"), "the child ran no test: {}", String::from_utf8_lossy(&o.stdout));
    }
}

fn names(out: &Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(out).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

fn modified(p: &Path) -> SystemTime {
    std::fs::metadata(p).unwrap().modified().unwrap()
}

#[test]
fn several_processes_build_the_fakes_at_once() {
    let out = std::env::temp_dir().join(format!("wisp-fake-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let x = std::env::consts::EXE_SUFFIX;
    let (agent, gh) = (out.join(format!("wisp-fake-agent{x}")), out.join(format!("wisp-fake-gh{x}")));
    let placed = [".lock".to_owned(), format!("wisp-fake-agent{x}"), format!("wisp-fake-gh{x}")];
    let built = common::bin_dir().join(format!("wisp-fake-agent{x}"));

    // 1. an empty folder: one process places the fakes, the others find them
    race(&out);
    assert_eq!(names(&out), placed, "no temp file left behind, the two fakes and the lock in place");

    // 2. the fakes in place and one of them running (a server's harness): nobody replaces it, so the
    // race passes on Windows too; a stamped mtime that survives shows no process renamed over it, on every OS
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    for f in [&agent, &gh] {
        std::fs::File::options().write(true).open(f).unwrap().set_modified(stamp).unwrap();
    }
    let running = hold(&agent);
    race(&out);
    release(running);
    assert_eq!((modified(&agent), modified(&gh)), (stamp, stamp), "a fake already in place with the build's bytes is kept");
    assert_eq!(names(&out), placed);

    // 3. a different build in place and running: it is moved aside and the build's bytes put in its place
    std::fs::remove_file(&agent).unwrap();
    std::fs::copy(std::env::current_exe().unwrap(), &agent).unwrap();
    let stale = hold(&agent);
    race(&out);
    assert!(wisp_testkit::same_bytes(&built, &agent), "{} holds the build's bytes after the race", agent.display());
    release(stale);
    // the copy moved aside goes at the next placement once nothing runs it
    common::fake_bins_into(&out);
    assert_eq!(names(&out), placed);
    std::fs::remove_dir_all(&out).unwrap();
}
