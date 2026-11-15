//! Windows (ADR-0070): a long-lived process that starts others (`wisp serve`) joins its nursery, a `KILL_ON_JOB_CLOSE`
//! job, so every child is born in a job and ends with it however it ends. A per-child job is joined only after
//! `CreateProcess` returns: CI 708ba936's Windows job hung 26 minutes on a harness probe (`opencode`) whose server was
//! killed in that window; it stayed suspended outside every job and held the step's log pipe.
//!
//! The test runs this binary as the "server" (`ROLE=server`): it joins its nursery, starts a child the way a spawn
//! does (`CREATE_SUSPENDED`) and stops there, as if killed before the child's own job; it also checks that a per-child
//! job still nests inside the nursery and ends its tree. The test then kills the server hard and the child must go.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "a test: failing loudly is the point")]

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::windows::process::CommandExt as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const ROLE: &str = "WISP_NURSERY_TEST_ROLE";
const NAME: &str = "a_child_started_as_its_server_is_killed_ends_with_it";
const CREATE_SUSPENDED: u32 = 0x0000_0004;

/// This binary again, running only [`NAME`], as `role`.
fn me(role: &str) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", NAME, "--nocapture", "--test-threads=1"]).env(ROLE, role);
    c
}

/// The server: in its nursery, a per-child job still nests in it and ends its tree; then a child caught in the window
/// (started suspended, never put in a job of its own), its pid printed, and a wait to be killed.
fn server() -> ! {
    wisp_proc::nursery().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let proc = wisp_proc::local();
        let args: Vec<String> = ["/d", "/c", "ping -n 60 127.0.0.1 >nul"].iter().map(|s| (*s).to_owned()).collect();
        let (h, _out) = proc.spawn("cmd", &args, wisp_proc::SpawnOpts::default()).unwrap().split();
        let report = h.kill().await;
        assert_eq!(report.survivors, 0, "a per-child job in the nursery didn't end its tree: {report:?}");
        tokio::time::timeout(Duration::from_secs(5), h.exited()).await.expect("the killed child never exited");
    });
    // this process's stdout is the test's pipe: the child must not hold it
    wisp_proc::keep_std_handles();
    // never waited for: it is the child caught in the window, which only the nursery ends
    #[allow(clippy::zombie_processes)]
    let child =
        me("child").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(CREATE_SUSPENDED).spawn().unwrap();
    let mut out = std::io::stdout();
    writeln!(out, "child {}", child.id()).unwrap();
    out.flush().unwrap();
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

/// The server, killed however the test ends (a `Child` dropped is not killed): a failed assert must never leave it, or
/// what it holds, behind.
struct Killed(std::process::Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
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
    let lines = BufReader::new(server.0.stdout.take().unwrap()).lines();
    let pid: u32 = lines
        .map_while(Result::ok)
        .find_map(|l| l.strip_prefix("child ").and_then(|p| p.trim().parse().ok()))
        .unwrap_or_else(|| panic!("the server never started its child: {:?}", server.0.try_wait()));
    let running = wisp_proc::pid_alive(pid);
    drop(server);
    let at = Instant::now();
    while wisp_proc::pid_alive(pid) && at.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let left = wisp_proc::pid_alive(pid);
    if left {
        // never leave it behind (it would hold whatever it inherited): ended by its pid, ours
        let _ = Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output();
    }
    assert!(running, "the child (pid {pid}) didn't run while its server did");
    assert!(!left, "the child (pid {pid}) outlived its server, killed hard: it was in no job");
}
