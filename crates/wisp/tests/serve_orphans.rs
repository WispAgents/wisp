//! Nothing `wisp serve` started outlives it (gate G12 for the server itself): a task's agent and what the agent
//! started (a tool, a dev server) end when the server ends, however it ends. Killed hard (SIGKILL, `TerminateProcess`)
//! it gets no chance to clean up, so the OS must do it: on Unix wisp-proc's guard (a process holding the read end of a
//! pipe only the server writes; at its EOF it kills every process group wisp-proc started), on Windows each child's
//! Job Object with `KILL_ON_JOB_CLOSE`, assigned before the child runs. Windows CI saw the miss: a restart in the same
//! folder couldn't replace `claude.exe` (os error 32) while a process of the killed server still ran it.
//! An exec plugin (ADR-0063) gets the same: the kernel starts it through wisp-proc's `contain` hook.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Api, install_fakes, start_with, temp};
use serde_json::json;

/// Whether `pid` is a process that can still run (a zombie, dead but not reaped yet, isn't).
fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let o = Command::new("ps").args(["-o", "stat=", "-p", &pid.to_string()]).output().unwrap();
        let stat = String::from_utf8_lossy(&o.stdout);
        o.status.success() && !stat.trim().is_empty() && !stat.trim_start().starts_with('Z')
    }
    #[cfg(windows)]
    {
        let o = Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"]).output().unwrap();
        String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\""))
    }
}

/// Ends `pid` (one this test's fake started, never anything found by name).
fn end(pid: u32) {
    #[cfg(unix)]
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
    #[cfg(windows)]
    let _ = Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output();
}

/// The agent's pid and its grandchild's, once the fake wrote them.
fn pids(file: &Path) -> Option<(u32, u32)> {
    let text = std::fs::read_to_string(file).ok()?;
    let line = text.lines().find(|l| !l.is_empty())?;
    let (a, b) = line.split_once(' ')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
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
    assert!(alive(agent) && alive(grandchild), "both run while the server does");
    stop(&mut serve.child);
    let _ = serve.child.wait();
    let at = Instant::now();
    let left = loop {
        let left: Vec<u32> = [agent, grandchild].into_iter().filter(|&p| alive(p)).collect();
        if left.is_empty() || at.elapsed() > Duration::from_secs(10) {
            break left;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    for &p in &left {
        end(p);
    }
    assert!(left.is_empty(), "outlived the server: {left:?} (agent {agent}, grandchild {grandchild})");
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
    while alive(grandchild) && at.elapsed() < Duration::from_secs(10) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let left = alive(grandchild);
    if left {
        end(grandchild);
    }
    assert!(!left, "the crashed plugin's grandchild {grandchild} (plugin {plugin}) still runs");
}

/// The harness probes the server runs (`agy --help`, `cursor-agent --list-models`, `opencode models --verbose`,
/// `copilot help config`, `grok models`, …), as `(argv, pid)`: every record of a fake with no prompt, from
/// `FAKE_RECORD`. Under `FAKE_HANG` a probe never answers.
fn probes(rec: &Path) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(rec).into_iter().flatten().flatten() {
        let name = e.path().file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned();
        for r in common::records(&e.path()) {
            if r["stdin"].as_str().is_some_and(|s| s.trim().is_empty())
                && let Some(pid) = r["pid"].as_u64().and_then(|p| u32::try_from(p).ok())
            {
                out.push((format!("{name} {}", common::argv(&r).join(" ")), pid));
            }
        }
    }
    out
}

/// The probes recorded in `rec` once their list stopped growing for `quiet` (at most `limit`), as `(argv, pid)`.
async fn settled(rec: &Path, quiet: Duration, limit: Duration) -> Vec<(String, u32)> {
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
) -> (common::Serve, Vec<(String, u32)>, Vec<(String, u32)>, tokio::task::JoinHandle<serde_json::Value>) {
    let dir = temp(name);
    let rec = dir.join("rec");
    let (serve, port) = start_with(&dir, &[("FAKE_HANG", "1"), ("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    // what the server probes by itself at start (its own detects time out at 15 s, so a few seconds hold still)
    let before: Vec<u32> = settled(&rec, Duration::from_secs(1), Duration::from_secs(8)).await.into_iter().map(|(_, p)| p).collect();
    let agents = tokio::spawn(async move { api.try_cmd("runtime.agents", json!({})).await });
    let at = Instant::now();
    let ours = loop {
        let all = settled(&rec, Duration::from_millis(500), Duration::from_secs(5)).await;
        let ours: Vec<_> = all.into_iter().filter(|(_, p)| !before.contains(p) && alive(*p)).collect();
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
    let all = probes(&rec).into_iter().filter(|&(_, p)| alive(p)).collect();
    (serve, ours, all, agents)
}

/// Waits up to `limit` for every probe in `running` to be gone; ends (by pid) and returns what is left.
async fn left_after(running: &[(String, u32)], limit: Duration) -> Vec<(String, u32)> {
    let at = Instant::now();
    loop {
        let left: Vec<_> = running.iter().filter(|&&(_, p)| alive(p)).cloned().collect();
        if left.is_empty() || at.elapsed() > limit {
            for (_, p) in &left {
                end(*p);
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
/// while its probes hang: the dropped command never reaches the probes' time limit, so dropping it must end them; before,
/// nothing did while the server ran, and they ran on with ppid 1 for an hour.
#[tokio::test(flavor = "multi_thread")]
async fn a_probe_whose_caller_hung_up_is_ended() {
    let (serve, running, _, _) = hung_probes("serve-orphans-probe-hangup", true).await;
    // well inside the probes' own 15 s limit: the drop ends them, not their timeout
    let left = left_after(&running, Duration::from_secs(8)).await;
    drop(serve);
    assert!(left.is_empty(), "probes ran on after their caller hung up: {left:?}");
}
