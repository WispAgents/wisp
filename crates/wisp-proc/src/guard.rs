//! Unix: the children's guard, so a server killed hard (SIGKILL, an OOM kill, a crash) leaves nothing behind. A
//! signal handler can't catch SIGKILL, and `PR_SET_PDEATHSIG` is Linux-only and fires when the *thread* that forked
//! exits (tokio's blocking pool retires idle threads, which would kill healthy children), so neither does the job.
//!
//! The guard is one `sh` started on the first spawn, in a process group of its own, reading a pipe only this
//! process holds the write end of (std makes it close-on-exec, so no child inherits it). Each child's process group
//! goes in as `+ <pgid>` when it starts and out as `- <pgid>` once its leader is reaped. However this process ends,
//! the kernel closes the pipe: the guard reads EOF, sends SIGTERM to every group still listed, then SIGKILL a second
//! later, and exits. A clean stop ends the children first, so the guard finds nothing then.
//!
//! What it can't reach: a grandchild that left its group (setsid, a daemon) once the server is gone; a clean stop's
//! tree kill reaches those (tree.rs), a SIGKILL can't. A group whose leader exited while members run is let go when
//! the leader is reaped (its pgid may be reused by a stranger later).
//! Windows needs none of this: each child's Job Object has `KILL_ON_JOB_CLOSE`, and the handle closes with wisp.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, PoisonError};

/// The guard: `+ N` adds group N, `- N` drops it; at EOF every listed group gets SIGTERM, then SIGKILL.
/// Signals that reach the server's terminal group (Ctrl-C, a closed terminal) don't end it early.
const SCRIPT: &str = r#"trap '' INT HUP TERM QUIT PIPE
g=' '
while read -r op id; do
  case "$id" in ''|*[!0-9]*) continue;; esac
  case "$op" in
    +) g="$g$id ";;
    -) n=' '; for x in $g; do [ "$x" = "$id" ] || n="$n$x "; done; g=$n;;
  esac
done
set -- $g
[ $# -eq 0 ] && exit 0
for x in "$@"; do kill -s TERM -- "-$x" 2>/dev/null; done
sleep 1
for x in "$@"; do kill -s KILL -- "-$x" 2>/dev/null; done
exit 0
"#;

struct Guard {
    /// the write end; dropping it (or this process ending) is the guard's EOF
    pipe: ChildStdin,
    /// reaped by nobody until this process ends: it outlives us by design
    _child: Child,
}

#[derive(Default)]
struct State {
    guard: Option<Guard>,
    /// the groups listed, to hand a restarted guard (one someone killed)
    groups: BTreeSet<u32>,
}

static STATE: Mutex<State> = Mutex::new(State { guard: None, groups: BTreeSet::new() });

fn start() -> std::io::Result<Guard> {
    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", SCRIPT]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd.spawn()?;
    let pipe = child.stdin.take().ok_or_else(|| std::io::Error::other("the guard has no stdin"))?;
    tracing::debug!(pid = child.id(), "proc guard");
    Ok(Guard { pipe, _child: child })
}

/// One line to the guard (started, or started again after a failed write, with every group still listed).
fn send(state: &mut State, line: &str) {
    if let Some(g) = state.guard.as_mut()
        && g.pipe.write_all(line.as_bytes()).is_ok()
    {
        return;
    }
    // none yet, or it's gone (EPIPE: someone ended it): a fresh one learns every group, this one included
    state.guard = None;
    match start() {
        Ok(mut g) => {
            let all = state.groups.iter().fold(String::new(), |mut all, p| {
                let _ = writeln!(all, "+ {p}");
                all
            });
            if g.pipe.write_all(all.as_bytes()).is_ok() {
                state.guard = Some(g);
            }
        }
        Err(e) => tracing::warn!(error = %e, "no proc guard: children may outlive a hard kill"),
    }
}

/// A child that leads process group `pgid` started: the guard ends the group if this process dies first.
pub(crate) fn watch(pgid: u32) {
    if pgid <= 1 {
        return;
    }
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    if state.groups.insert(pgid) {
        send(&mut state, &format!("+ {pgid}\n"));
    }
}

/// The group's leader was reaped: its pgid may be a stranger's once the group empties, so the guard lets it go.
pub(crate) fn reaped(pgid: u32) {
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    if state.groups.remove(&pgid) && state.guard.is_some() {
        send(&mut state, &format!("- {pgid}\n"));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a test")]
mod tests {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::SCRIPT;

    fn alive(pid: u32) -> bool {
        let o = Command::new("ps").args(["-o", "stat=", "-p", &pid.to_string()]).output().unwrap();
        let s = String::from_utf8_lossy(&o.stdout);
        o.status.success() && !s.trim().is_empty() && !s.trim_start().starts_with('Z')
    }

    fn sleeper() -> std::process::Child {
        let mut c = Command::new("sleep");
        c.arg("30");
        std::os::unix::process::CommandExt::process_group(&mut c, 0);
        c.spawn().unwrap()
    }

    /// At EOF the guard ends the groups still listed, and only those: one dropped with `-` keeps running, and a
    /// malformed line is skipped.
    #[test]
    fn at_eof_the_guard_ends_the_listed_groups_and_only_those() {
        let (mut kept, mut ended) = (sleeper(), sleeper());
        let mut sh = Command::new("/bin/sh").args(["-c", SCRIPT]).stdin(Stdio::piped()).spawn().unwrap();
        let mut pipe = sh.stdin.take().unwrap();
        let lines = format!("+ {k}\n+ {e}\n+ 1;rm\n- {k}\n", k = kept.id(), e = ended.id());
        pipe.write_all(lines.as_bytes()).unwrap();
        drop(pipe);
        assert!(sh.wait().unwrap().success());
        let at = Instant::now();
        while ended.try_wait().unwrap().is_none() {
            assert!(at.elapsed() < Duration::from_secs(5), "the listed group still runs");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(alive(kept.id()), "the group dropped with `-` was let go");
        let _ = kept.kill();
        let _ = kept.wait();
    }
}
