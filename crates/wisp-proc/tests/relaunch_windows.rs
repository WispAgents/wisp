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
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "a test: failing loudly is the point")]
#![cfg(windows)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const ROLE: &str = "WISP_RELAUNCH_TEST_ROLE";
/// `1`: the server's detached start asks to leave every job (the update's relaunch helper); else an ordinary one
const LEAVE: &str = "WISP_RELAUNCH_TEST_LEAVE";
/// the test each role runs as (the roles are this binary again, filtered to one test)
const NAME_VAR: &str = "WISP_RELAUNCH_TEST_NAME";
/// Looked for anywhere in a line (libtest prints `test <name> ... ` before the test runs).
const MARK: &str = "WISP_RELAUNCH_HELPER_PID=";
const START: Duration = Duration::from_secs(30);
const EXIT: Duration = Duration::from_secs(10);

/// This binary again, running only the test named in [`NAME_VAR`], as `role`, its stdout piped.
fn me(role: &str) -> Child {
    let name = std::env::var(NAME_VAR).unwrap();
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture", "--test-threads=1"])
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
        let leave = std::env::var(LEAVE).is_ok_and(|v| v == "1");
        let opts = wisp_proc::SpawnOpts { no_stdin: true, leave_every_job: leave, ..wisp_proc::SpawnOpts::default() };
        wisp_proc::local().spawn_detached("cmd", &args, opts).unwrap()
    });
    say(pid);
    std::process::exit(0);
}

/// Runs the shell (which runs the server, which starts a detached `ping`), and says whether that `ping` was still
/// running once the shell and its job were gone. The `ping` is killed either way.
fn helper_outlives_the_shell(name: &str, leave: bool) -> bool {
    match std::env::var(ROLE).as_deref() {
        Ok("shell") => shell(),
        Ok("server") => server(),
        _ => {}
    }
    wisp_proc::keep_std_handles();
    let mut shell = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(ROLE, "shell")
        .env(NAME_VAR, name)
        .env(LEAVE, if leave { "1" } else { "0" })
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let pid = pid_from(&mut shell);
    let shell_gone = exited(&mut shell);
    // the job's processes end as its last handle closes; give it a moment either way
    std::thread::sleep(Duration::from_millis(500));
    let alive = wisp_proc::pid_alive(pid);
    if alive {
        let _ = Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    assert!(shell_gone, "the shell didn't exit");
    alive
}

#[test]
fn the_relaunch_helper_outlives_the_shells_job() {
    assert!(
        helper_outlives_the_shell("the_relaunch_helper_outlives_the_shells_job", true),
        "the helper ended with the shell's job: it was never let out"
    );
}

/// DESK2-05: anything else the server starts detached (a terminal, a browser) stays in the shell's job and ends when
/// the window closes; since d4c98dac every detached start left the job, and the app's quit stopped none of them.
#[test]
fn an_ordinary_detached_start_ends_with_the_shells_job() {
    assert!(
        !helper_outlives_the_shell("an_ordinary_detached_start_ends_with_the_shells_job", false),
        "a detached start that didn't ask to leave every job outlived the shell's job"
    );
}
