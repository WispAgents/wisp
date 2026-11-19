//! Windows: the update's relaunch helper (`spawn_detached`) outlives the desktop shell. The shell puts `wisp serve` in
//! a `KILL_ON_JOB_CLOSE` job (desktop/src-tauri/src/server.rs), the hand-off stops the server, the shell quits with it
//! and its job ends everything still inside. A helper born in that job died before `setup.exe /S /R` ran, so wisp
//! never came back: `spawn_detached` broke away only from wisp's own nursery, never from a job it was started in.
//!
//! Three processes, this binary in three roles: the test starts the "shell", which joins a `KILL_ON_JOB_CLOSE |
//! BREAKAWAY_OK` job (its nursery: the limits the shell's job has) and starts the "server" in it. The server has no
//! nursery of its own (in a job, but not its nursery: the case that went wrong), starts the helper with
//! `spawn_detached`, prints its pid and exits; the shell passes the pid on and exits, closing its job. The helper must
//! still be running. Every wait has a deadline and the helper is killed at the end whatever happens.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "a test: failing loudly is the point")]

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const ROLE: &str = "WISP_RELAUNCH_TEST_ROLE";
const NAME: &str = "the_relaunch_helper_outlives_the_shells_job";
/// Looked for anywhere in a line (libtest prints `test <name> ... ` before the test runs).
const MARK: &str = "WISP_RELAUNCH_HELPER_PID=";
const START: Duration = Duration::from_secs(30);
const EXIT: Duration = Duration::from_secs(10);

/// This binary again, running only [`NAME`], as `role`, its stdout piped.
fn me(role: &str) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
        .env(ROLE, role)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
}

/// The pid `child` prints after [`MARK`], read on a thread of its own and waited for at most [`START`].
fn pid_from(child: &mut Child) -> u32 {
    let (tx, lines) = mpsc::channel::<String>();
    let out = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for l in BufReader::new(out).lines().map_while(Result::ok) {
            if tx.send(l).is_err() {
                return;
            }
        }
    });
    let deadline = Instant::now() + START;
    let mut seen = Vec::new();
    loop {
        match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(l) => {
                if let Some(pid) = l.split_once(MARK).and_then(|(_, p)| p.trim().parse().ok()) {
                    return pid;
                }
                seen.push(l);
            }
            Err(why) => {
                let _ = child.kill();
                panic!("no helper pid ({why:?} after {START:?}); it printed:\n{}", seen.join("\n"));
            }
        }
    }
}

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

fn exited(child: &mut Child) -> bool {
    within(EXIT, || !matches!(child.try_wait(), Ok(None)))
}

fn say(pid: u32) {
    let mut out = std::io::stdout();
    writeln!(out, "\n{MARK}{pid}").unwrap();
    out.flush().unwrap();
}

/// The desktop shell: its job (`KILL_ON_JOB_CLOSE | BREAKAWAY_OK`), the server in it, then gone.
fn shell() -> ! {
    wisp_proc::keep_std_handles();
    wisp_proc::nursery().unwrap();
    let mut server = me("server");
    let pid = pid_from(&mut server);
    assert!(exited(&mut server), "the server didn't exit");
    say(pid);
    // closing the last handle to its job ends whatever is still in it
    std::process::exit(0);
}

/// `wisp serve` in the shell's job, with no nursery of its own: the hand-off's helper, then gone.
fn server() -> ! {
    wisp_proc::keep_std_handles();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let pid = rt.block_on(async {
        let args: Vec<String> = ["/d", "/c", "ping -n 30 127.0.0.1 >nul"].iter().map(|s| (*s).to_owned()).collect();
        wisp_proc::local().spawn_detached("cmd", &args, wisp_proc::SpawnOpts { no_stdin: true, ..wisp_proc::SpawnOpts::default() }).unwrap()
    });
    say(pid);
    std::process::exit(0);
}

#[test]
fn the_relaunch_helper_outlives_the_shells_job() {
    match std::env::var(ROLE).as_deref() {
        Ok("shell") => shell(),
        Ok("server") => server(),
        _ => {}
    }
    wisp_proc::keep_std_handles();
    let mut shell = me("shell");
    let pid = pid_from(&mut shell);
    let shell_gone = exited(&mut shell);
    // the job's processes end as its last handle closes; give it a moment either way
    std::thread::sleep(Duration::from_millis(500));
    let alive = wisp_proc::pid_alive(pid);
    if alive {
        let _ = Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    assert!(shell_gone, "the shell didn't exit");
    assert!(alive, "the helper (pid {pid}) ended with the shell's job: it was never let out");
}
