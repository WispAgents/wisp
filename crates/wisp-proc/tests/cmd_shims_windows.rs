//! Windows: a CLI installed as a `.cmd` file (npm's shims, a test's fake) starts through `spawn` and through
//! `terminal_command` (`wisp open`'s harness, `$EDITOR`), found without its extension by PATHEXT, with a prompt full
//! of cmd metacharacters arriving as text and never running a command. Every wait has a deadline ([`LIMIT`]): a start that
//! never ends fails the test instead of hanging the suite.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "a test: failing loudly is the point")]

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use wisp_proc::ExecOpts;

/// The most one start of the fake may take (it writes one file: well under a second).
const LIMIT: Duration = Duration::from_secs(30);

const PROMPT: &str = "fix it & echo pwned> pwned.txt | more";

/// `dir\fake.cmd`, which writes its first argument (as cmd passed it, quotes kept) to `dir\got.txt`.
fn fake(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("wisp-proc-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("fake.cmd"), "@echo off\r\n> \"%~dp0got.txt\" echo(%1\r\n").unwrap();
    dir
}

fn check(dir: &std::path::Path) {
    let got = std::fs::read_to_string(dir.join("got.txt")).unwrap();
    assert!(got.contains("fix it"), "the .cmd ran with its argument: {got:?}");
    assert!(!dir.join("pwned.txt").exists() && !std::path::Path::new("pwned.txt").exists(), "the prompt ran a command");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cmd_file_named_without_its_extension_runs_through_spawn_with_its_prompt_as_text() {
    let dir = fake("spawn");
    let bin = dir.join("fake").display().to_string();
    let opts = ExecOpts { cwd: Some(dir.clone()), timeout: Some(LIMIT), ..ExecOpts::default() };
    let out = tokio::time::timeout(LIMIT * 2, wisp_proc::local().exec(&bin, &[PROMPT.to_owned()], opts))
        .await
        .unwrap_or_else(|_| panic!("exec of the .cmd never finished, its timeout ({LIMIT:?}) included"))
        .unwrap();
    assert!(!out.timed_out, "the .cmd ran past {LIMIT:?}: {out:?}");
    assert!(out.exit.success(), "{out:?}");
    check(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_cmd_file_runs_through_terminal_command_with_its_prompt_as_text() {
    let dir = fake("terminal");
    let bin = dir.join("fake").display().to_string();
    let mut child = wisp_proc::terminal_command(&bin, &[PROMPT.to_owned()])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let at = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if at.elapsed() > LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the .cmd through terminal_command ran past {LIMIT:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "{status:?}");
    check(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}
