//! Windows (ADR-0070): a long-lived process that starts others (`wisp serve`) joins its nursery, a `KILL_ON_JOB_CLOSE`
//! job, so every child is born in a job and ends with it however it ends. A per-child job is joined only after
//! `CreateProcess` returns: CI 708ba936's Windows job hung 26 minutes on a harness probe (`opencode`) whose server was
//! killed in that window; it stayed suspended outside every job and held the step's log pipe.
//!
//! The test runs this binary as the "server" (`ROLE=server`): it joins its nursery, starts a child the way a spawn
//! does (`CREATE_SUSPENDED`) and stops there, as if killed before the child's own job; it also checks that a per-child
//! job still nests inside the nursery and ends its tree. The test then kills the server hard and the child must go.
//!
//! Nothing here may block for ever (CI 8bb8ccd0 hung 26 minutes on this test): the server's lines are read on a thread
//! of their own and waited for with a deadline, every wait for an exit polls with one, and the server ends itself
//! after [`SERVER_LIFE`] whoever forgets it. A failure says what the server printed.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "a test: failing loudly is the point")]
#![cfg(windows)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::windows::process::CommandExt as _;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const ROLE: &str = "WISP_NURSERY_TEST_ROLE";
const NAME: &str = "a_child_started_as_its_server_is_killed_ends_with_it";
const CREATE_SUSPENDED: u32 = 0x0000_0004;
/// What the server prints before the child's pid. Looked for anywhere in a line: with `--test-threads=1` libtest
/// prints `test <name> ... ` before the test runs, so the server's first line of its own is that line's tail (CI
/// 8bb8ccd0: a `strip_prefix("child ")` never matched, and the test read on for ever).
const MARK: &str = "WISP_NURSERY_CHILD_PID=";
/// The most the server's pid line may take (it starts and kills one `cmd` first: seconds).
const SERVER_START: Duration = Duration::from_secs(30);
/// The most a process killed hard may take to be gone.
const EXIT: Duration = Duration::from_secs(10);
/// The server ends itself after this, so a test that died never leaves it (or the pipes it holds) behind for long.
const SERVER_LIFE: Duration = Duration::from_secs(90);

/// This binary again, running only [`NAME`], as `role`.
fn me(role: &str) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", NAME, "--nocapture", "--test-threads=1"]).env(ROLE, role);
    c
}

/// The server: in its nursery, a per-child job still nests in it and ends its tree; then a child caught in the window
/// (started suspended, never put in a job of its own), its pid printed, and a wait to be killed.
fn server() -> ! {
    // this process's stdout is the test's pipe and its stderr CI's log: nothing it starts may hold them
    wisp_proc::keep_std_handles();
    wisp_proc::nursery().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let proc = wisp_proc::local();
        let args: Vec<String> = ["/d", "/c", "ping -n 60 127.0.0.1 >nul"].iter().map(|s| (*s).to_owned()).collect();
        let (h, _out) = proc.spawn("cmd", &args, wisp_proc::SpawnOpts::default()).unwrap().split();
        let report = tokio::time::timeout(EXIT, h.kill()).await.expect("the kill of a per-child job never finished");
        assert_eq!(report.survivors, 0, "a per-child job in the nursery didn't end its tree: {report:?}");
        tokio::time::timeout(EXIT, h.exited()).await.expect("the killed child never exited");
    });
    // never waited for: it is the child caught in the window, which only the nursery ends
    #[allow(clippy::zombie_processes)]
    let child =
        me("child").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(CREATE_SUSPENDED).spawn().unwrap();
    let mut out = std::io::stdout();
    // on a line of its own (libtest's `test <name> ... ` is before it)
    writeln!(out, "\n{MARK}{}", child.id()).unwrap();
    out.flush().unwrap();
    std::thread::sleep(SERVER_LIFE);
    // forgotten: the test is gone (or hung); end, and the nursery ends the child
    std::process::exit(3);
}

/// Polls until `done` or `limit` passes; whether it was done.
fn within(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let at = Instant::now();
    loop {
        if done() {
            return true;
        }
        if at.elapsed() >= limit {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The server, killed however the test ends (a `Child` dropped is not killed): a failed assert must never leave it, or
/// what it holds, behind. No wait for its exit is unbounded.
struct Killed(std::process::Child);

impl Killed {
    /// Kill it and wait (at most [`EXIT`]) for it to be gone; whether it is.
    fn end(&mut self) -> bool {
        let _ = self.0.kill();
        within(EXIT, || !matches!(self.0.try_wait(), Ok(None)))
    }
}

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.end();
    }
}

/// End `pid` by its pid (a child left behind would hold whatever it inherited), waiting at most [`EXIT`] for taskkill.
fn taskkill(pid: u32) {
    let started = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if let Ok(mut t) = started
        && !within(EXIT, || !matches!(t.try_wait(), Ok(None)))
    {
        let _ = t.kill();
    }
}

#[test]
fn a_child_started_as_its_server_is_killed_ends_with_it() {
    match std::env::var(ROLE).as_deref() {
        Ok("server") => server(),
        // started suspended: never runs
        Ok(_) => return,
        Err(_) => {}
    }
    // this test's own stdout is cargo's (CI's log pipe): neither the server nor its child may hold it
    wisp_proc::keep_std_handles();
    let mut server = Killed(me("server").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().unwrap());
    // its lines on a thread of their own: a read can block for as long as anything holds the pipe, the wait for the
    // pid can't
    let (tx, lines) = mpsc::channel::<String>();
    let stdout = server.0.stdout.take().unwrap();
    std::thread::spawn(move || {
        for l in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(l).is_err() {
                return;
            }
        }
    });
    let deadline = Instant::now() + SERVER_START;
    let mut seen = Vec::new();
    let pid: u32 = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match lines.recv_timeout(left) {
            Ok(l) => {
                let pid = l.split_once(MARK).and_then(|(_, p)| p.trim().parse().ok());
                seen.push(l);
                if let Some(pid) = pid {
                    break pid;
                }
            }
            Err(why) => {
                let gone = server.end();
                panic!(
                    "the server never printed its child's pid ({why:?} after {SERVER_START:?}; killed, gone: {gone}; exit: {:?}); \
                     it printed:\n{}",
                    server.0.try_wait(),
                    seen.join("\n")
                );
            }
        }
    };
    let running = wisp_proc::pid_alive(pid);
    let server_gone = server.end();
    let child_gone = within(EXIT, || !wisp_proc::pid_alive(pid));
    if !child_gone {
        taskkill(pid);
    }
    assert!(server_gone, "the server, killed hard, didn't exit within {EXIT:?}");
    assert!(running, "the child (pid {pid}) didn't run while its server did");
    assert!(child_gone, "the child (pid {pid}) outlived its server, killed hard: it was in no job");
}
