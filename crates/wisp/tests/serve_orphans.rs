//! Nothing `wisp serve` started outlives it (gate G12 for the server itself): a task's agent and what the agent
//! started (a tool, a dev server) end when the server ends, however it ends. Killed hard (SIGKILL, `TerminateProcess`)
//! it gets no chance to clean up, so the OS must do it: on Unix wisp-proc's guard (a process holding the read end of a
//! pipe only the server writes; at its EOF it kills every process group wisp-proc started), on Windows each child's
//! Job Object with `KILL_ON_JOB_CLOSE`, assigned before the child runs. Windows CI saw the miss: a restart in the same
//! folder couldn't replace `claude.exe` (os error 32) while a process of the killed server still ran it.
//! An exec plugin (ADR-0063) gets the same: the kernel starts it through wisp-proc's `contain` hook.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Api, install_fakes, start_with, temp};
use serde_json::json;

/// A process a fake started that this test watches: its pid, and the file it holds locked for its whole life
/// (`FAKE_RECORD`'s `live`, `FAKE_GRANDCHILD`'s `<file>.<pid>.live`). A pid alone can't say whether it still runs:
/// Windows hands a freed pid to the next process at once. CI db8f01ad (windows, node 24) saw it: the start's hung
/// probes ended at their 15 s limit, wisp.accounts' sign-in check (`agy models`, a new hung probe) and the other tests'
/// probes started right then, one of them got a freed pid, and the probe "outlived its time limit" for 25 s.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Watched {
    pid: u32,
    live: PathBuf,
}

impl Watched {
    /// Whether it still runs: its lock is held (the OS drops it with the process, however it ends; a zombie, dead but
    /// not reaped yet, holds none).
    fn running(&self) -> bool {
        let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(&self.live) else {
            return false;
        };
        match f.try_lock() {
            Ok(()) => false,
            Err(std::fs::TryLockError::WouldBlock) => true,
            Err(std::fs::TryLockError::Error(e)) => panic!("reading the lock {}: {e}", self.live.display()),
        }
    }

    /// Ends it (by pid) if it still runs, so a stranger that got the pid since is never ended.
    fn end(&self) {
        if !self.running() {
            return;
        }
        #[cfg(unix)]
        let _ = Command::new("kill").args(["-9", &self.pid.to_string()]).output();
        #[cfg(windows)]
        let _ = Command::new("taskkill").args(["/F", "/PID", &self.pid.to_string()]).output();
    }
}

/// The agent and its grandchild, once the agent wrote their pids (after both took their locks).
fn pids(file: &Path) -> Option<(Watched, Watched)> {
    let text = std::fs::read_to_string(file).ok()?;
    let line = text.lines().find(|l| !l.is_empty())?;
    let (a, b) = line.split_once(' ')?;
    let watched = |p: &str| -> Option<Watched> {
        let pid: u32 = p.trim().parse().ok()?;
        Some(Watched { pid, live: PathBuf::from(format!("{}.{pid}.live", file.display())) })
    };
    Some((watched(a)?, watched(b)?))
}

/// What starts the process under test: a task's agent, or an exec plugin.
#[derive(Clone, Copy)]
enum Via {
    Agent,
    ExecPlugin,
}

/// A trusted user plugin (so it gets the server's environment, `FAKE_GRANDCHILD` included) whose exec server is the
/// fake's `exec-plugin` role, under `<home>/plugins/orphan`.
fn add_exec_plugin(dir: &Path) {
    let folder = dir.join("home/plugins/orphan");
    install_fakes(&folder, &["exec-plugin"]);
    let bin = folder.join(format!("exec-plugin{}", std::env::consts::EXE_SUFFIX));
    let m = json!({ "wispPlugin": 1, "id": "user.orphan", "version": "1.0.0", "api": "^1", "server": { "exec": [bin] } });
    std::fs::write(folder.join("wisp-plugin.json"), m.to_string()).unwrap();
    std::fs::write(folder.join(".wisp-trusted"), "").unwrap();
}

/// Starts a task whose agent (or an exec plugin that) starts a grandchild and hangs; stops the server with `stop`;
/// asserts that within a few seconds neither it nor its grandchild runs. Leftovers are ended (by pid) before the
/// assert fails.
async fn nothing_outlives(name: &str, via: Via, stop: impl FnOnce(&mut std::process::Child)) {
    let dir = temp(name);
    let file = dir.join("pids.txt");
    if matches!(via, Via::ExecPlugin) {
        add_exec_plugin(&dir);
    }
    let (mut serve, port) = start_with(&dir, &[("FAKE_HANG", "1"), ("FAKE_GRANDCHILD", file.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    match via {
        Via::Agent => {
            api.cmd("runtime.create", json!({ "prompt": "fix the add bug", "cwd": dir.join("proj"), "agent": "claude" })).await;
        }
        Via::ExecPlugin => {
            api.cmd("plugins.setEnabled", json!({ "id": "user.orphan", "enabled": true })).await;
        }
    }
    let at = Instant::now();
    let (agent, grandchild) = loop {
        if let Some(p) = pids(&file) {
            break p;
        }
        assert!(
            at.elapsed() < Duration::from_secs(30),
            "it never started its grandchild: {}",
            std::fs::read_to_string(dir.join("serve.log")).unwrap_or_default()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(agent.running() && grandchild.running(), "both run while the server does");
    stop(&mut serve.child);
    let _ = serve.child.wait();
    let at = Instant::now();
    let left = loop {
        let left: Vec<Watched> = [&agent, &grandchild].into_iter().filter(|w| w.running()).cloned().collect();
        if left.is_empty() || at.elapsed() > Duration::from_secs(10) {
            break left;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    for w in &left {
        w.end();
    }
    assert!(left.is_empty(), "outlived the server: {left:?} (agent {}, grandchild {})", agent.pid, grandchild.pid);
}

/// SIGKILL on Unix, `TerminateProcess` on Windows: `Child::kill`.
#[tokio::test(flavor = "multi_thread")]
async fn a_server_killed_hard_leaves_no_agent_and_no_grandchild() {
    nothing_outlives("serve-orphans-kill", Via::Agent, |c| {
        let _ = c.kill();
    })
    .await;
}

/// SIGTERM: the clean stop (every plugin off, then the guard's sweep).
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_server_stopped_with_sigterm_leaves_no_agent_and_no_grandchild() {
    nothing_outlives("serve-orphans-term", Via::Agent, |c| {
        let _ = Command::new("kill").args(["-TERM", &c.id().to_string()]).output();
    })
    .await;
}

/// ADR-0063: an exec plugin that starts a grandchild and ignores its input closing; the server killed hard (SIGKILL,
/// `TerminateProcess`) leaves neither (Unix: the guard ends its group; Windows: it joined its job before it ran).
#[tokio::test(flavor = "multi_thread")]
async fn a_server_killed_hard_leaves_no_exec_plugin_and_no_grandchild() {
    nothing_outlives("serve-orphans-exec", Via::ExecPlugin, |c| {
        let _ = c.kill();
    })
    .await;
}

/// ADR-0063: an exec plugin that crashes leaves no grandchild behind while the server runs on (Unix: its group is
/// ended before the guard lets it go; Windows: its job closes).
#[tokio::test(flavor = "multi_thread")]
async fn an_exec_plugin_that_crashes_leaves_no_grandchild() {
    let dir = temp("serve-orphans-exec-crash");
    let file = dir.join("pids.txt");
    add_exec_plugin(&dir);
    let (_serve, port) = start_with(&dir, &[("FAKE_EXEC_CRASH", "1"), ("FAKE_GRANDCHILD", file.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    // it may crash before or after the switch answers: either way it was started
    let _ = api.try_cmd("plugins.setEnabled", json!({ "id": "user.orphan", "enabled": true })).await;
    let at = Instant::now();
    let (plugin, grandchild) = loop {
        if let Some(p) = pids(&file) {
            break p;
        }
        assert!(at.elapsed() < Duration::from_secs(30), "it never started its grandchild");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let at = Instant::now();
    while grandchild.running() && at.elapsed() < Duration::from_secs(10) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let left = grandchild.running();
    grandchild.end();
    assert!(!left, "the crashed plugin's grandchild {grandchild:?} (plugin {plugin:?}) still runs");
}

/// The harness probes the server runs (`agy --help`, `cursor-agent --list-models`, `opencode models --verbose`,
/// `copilot help config`, `grok models`, …), as `(argv, process)`: every record of a fake with no prompt, from
/// `FAKE_RECORD`. Under `FAKE_HANG` a probe never answers.
fn probes(rec: &Path) -> Vec<(String, Watched)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(rec).into_iter().flatten().flatten() {
        let name = e.path().file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned();
        for r in common::records(&e.path()) {
            if r["stdin"].as_str().is_some_and(|s| s.trim().is_empty())
                && let Some(pid) = r["pid"].as_u64().and_then(|p| u32::try_from(p).ok())
                && let Some(live) = r["live"].as_str()
            {
                out.push((format!("{name} {}", common::argv(&r).join(" ")), Watched { pid, live: live.into() }));
            }
        }
    }
    out
}

/// The probes recorded in `rec` once their list stopped growing for `quiet` (at most `limit`), as `(argv, process)`.
async fn settled(rec: &Path, quiet: Duration, limit: Duration) -> Vec<(String, Watched)> {
    let at = Instant::now();
    let (mut last, mut since) = (probes(rec), Instant::now());
    while since.elapsed() < quiet && at.elapsed() < limit {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let now = probes(rec);
        if now.len() != last.len() {
            (last, since) = (now, Instant::now());
        }
    }
    last
}

/// Hung probes on a live server whose fakes never answer one, in two sets: the ones started at the server's start, and
/// the ones one `runtime.agents` request started (`hang_up`: that request is then closed, as a client that gives up
/// does). Returns the server, the request's probes still running, every probe running, and the request.
async fn hung_probes(
    name: &str,
    hang_up: bool,
) -> (common::Serve, Vec<(String, Watched)>, Vec<(String, Watched)>, tokio::task::JoinHandle<serde_json::Value>) {
    let dir = temp(name);
    let rec = dir.join("rec");
    let (serve, port) = start_with(&dir, &[("FAKE_HANG", "1"), ("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    // what the server probes by itself at start (its own detects time out at 15 s, so a few seconds hold still)
    let before: Vec<Watched> = settled(&rec, Duration::from_secs(1), Duration::from_secs(8)).await.into_iter().map(|(_, p)| p).collect();
    // a probe runs once however many ask (round 212): a request made while those run joins them, and after they timed
    // out it answers with their hang. So the request's own probes are a Re-check (`fresh`) once the start's ended.
    let at = Instant::now();
    while before.iter().any(Watched::running) {
        assert!(at.elapsed() < Duration::from_secs(25), "the start's probes outlived their time limit");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let agents = tokio::spawn(async move { api.try_cmd("runtime.agents", json!({ "fresh": "all" })).await });
    let at = Instant::now();
    let ours = loop {
        let all = settled(&rec, Duration::from_millis(500), Duration::from_secs(5)).await;
        let ours: Vec<_> = all.into_iter().filter(|(_, p)| !before.contains(p) && p.running()).collect();
        if !ours.is_empty() {
            break ours;
        }
        assert!(
            at.elapsed() < Duration::from_secs(10),
            "runtime.agents ran no probe: {:?} {}",
            probes(&rec),
            std::fs::read_to_string(dir.join("serve.log")).unwrap_or_default()
        );
    };
    if hang_up {
        agents.abort();
    }
    let all = probes(&rec).into_iter().filter(|(_, p)| p.running()).collect();
    (serve, ours, all, agents)
}

/// Waits up to `limit` for every probe in `running` to be gone; ends and returns what is left.
async fn left_after(running: &[(String, Watched)], limit: Duration) -> Vec<(String, Watched)> {
    let at = Instant::now();
    loop {
        let left: Vec<_> = running.iter().filter(|(_, p)| p.running()).cloned().collect();
        if left.is_empty() || at.elapsed() > limit {
            for (_, p) in &left {
                p.end();
            }
            return left;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Round 211: a server killed hard (SIGKILL, `TerminateProcess`) while its harness probes still run leaves no probe
/// (they lead groups of their own, so only the guard / their job can end them).
#[tokio::test(flavor = "multi_thread")]
async fn a_server_killed_hard_while_probes_run_leaves_no_probe() {
    let (mut serve, _, running, _) = hung_probes("serve-orphans-probe-kill", false).await;
    let _ = serve.child.kill();
    let _ = serve.child.wait();
    let left = left_after(&running, Duration::from_secs(10)).await;
    assert!(left.is_empty(), "probes outlived the server: {left:?}");
}

/// Round 211: a probe that never answers is ended (its whole group) by its time limit (15 s at most) while the
/// server runs on and its caller still waits.
#[tokio::test(flavor = "multi_thread")]
async fn a_hung_harness_probe_is_ended_by_its_timeout() {
    let (serve, running, _, agents) = hung_probes("serve-orphans-probe-timeout", false).await;
    let left = left_after(&running, Duration::from_secs(25)).await;
    agents.abort();
    drop(serve);
    assert!(left.is_empty(), "hung probes outlived their time limit: {left:?}");
}

/// Round 211 (the leak seen): the caller of `runtime.agents` gives up (a closed request, an MCP client's own timeout)
/// while its probes hang; before, nothing ended them while the server ran, and they ran on with ppid 1 for an hour.
/// Since ADR-0066 the detection round runs on its own task (a caller that hangs up must not leave the harnesses topic
/// `checking`: `plugins/runtime/host/tests/detect.rs` `a_caller_that_hangs_up_never_leaves_rows_checking`), so the
/// round still waits on the probes: they end, each with its group, at their own time limit (15 s), never later, and
/// the round lands their answer (kept by the memo: a plain ask afterwards answers from it and starts no probe). A
/// probe nobody waits on at all ends at once (`plugins/runtime/host/src/probes.rs` `a_probe_nobody_waits_for_is_dropped`).
#[tokio::test(flavor = "multi_thread")]
async fn a_probe_whose_caller_hung_up_ends_at_its_time_limit() {
    let (serve, running, _, _) = hung_probes("serve-orphans-probe-hangup", true).await;
    // the probes started at most ~6 s ago; their own limit is 15 s from their start
    let left = left_after(&running, Duration::from_secs(25)).await;
    assert!(left.is_empty(), "probes ran on past their time limit after their caller hung up: {left:?}");
    let rec = serve.dir.join("rec");
    let n = settled(&rec, Duration::from_secs(1), Duration::from_secs(5)).await.len();
    let api = Api { http: reqwest::Client::new(), port: port_of(&serve), n: 0.into() };
    let answer = tokio::time::timeout(Duration::from_secs(5), api.cmd("runtime.agents", json!({})))
        .await
        .expect("a plain runtime.agents waited: the hung-up round never landed its answers");
    assert!(answer.as_array().is_some_and(|a| !a.is_empty()), "{answer}");
    let after = probes(&rec);
    assert_eq!(after.len(), n, "the hung-up round's answers weren't kept: a plain ask probed again: {after:?}");
    drop(serve);
}

/// The port `serve` advertised.
fn port_of(serve: &common::Serve) -> u16 {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(serve.dir.join("home/server.json")).unwrap()).unwrap();
    u16::try_from(v["port"].as_u64().unwrap()).unwrap()
}

/// CI db8f01ad (windows, node 24): a probe that ended reads as ended even when its pid is another running process's
/// now (Windows reuses a freed pid at once). Before, the watch was by pid (`ps`, `tasklist`), so a stranger with the
/// pid kept a dead probe "alive"; here the stranger is this test process, so the old check fails on every OS.
#[tokio::test(flavor = "multi_thread")]
async fn a_probe_that_ended_is_told_apart_from_a_process_that_got_its_pid() {
    let dir = temp("serve-orphans-pid-reuse");
    let (bin, rec) = (dir.join("bin"), dir.join("rec"));
    install_fakes(&bin, &["grok"]);
    let mut probe = Command::new(bin.join(format!("grok{}", std::env::consts::EXE_SUFFIX)))
        .arg("models")
        .env("FAKE_HANG", "1")
        .env("FAKE_RECORD", &rec)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let at = Instant::now();
    let (_, watched) = loop {
        if let Some(p) = probes(&rec).into_iter().next() {
            break p;
        }
        assert!(at.elapsed() < Duration::from_secs(10), "the probe never recorded itself");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(watched.pid, probe.id());
    assert!(watched.running(), "a hung probe runs");
    let _ = probe.kill();
    let _ = probe.wait();
    let at = Instant::now();
    while watched.running() {
        assert!(at.elapsed() < Duration::from_secs(5), "a killed probe still reads as running");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // its pid handed to another process (this one stands for it): the record still reads as ended
    let line = json!({ "argv": ["models"], "stdin": "", "pid": std::process::id(), "live": watched.live });
    std::fs::write(rec.join("grok.jsonl"), format!("{line}\n")).unwrap();
    let after = probes(&rec);
    assert_eq!(after.len(), 1, "{after:?}");
    assert!(after.iter().all(|(_, p)| !p.running()), "a dead probe read as running through a pid someone else has: {after:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
