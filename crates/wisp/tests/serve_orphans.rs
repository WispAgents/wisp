//! Nothing `wisp serve` started outlives it (gate G12 for the server itself): a task's agent and what the agent
//! started (a tool, a dev server) end when the server ends, however it ends. Killed hard (SIGKILL, `TerminateProcess`)
//! it gets no chance to clean up, so the OS must do it: on Unix wisp-proc's guard (a process holding the read end of a
//! pipe only the server writes; at its EOF it kills every process group wisp-proc started), on Windows each child's
//! Job Object with `KILL_ON_JOB_CLOSE`, assigned before the child runs. Windows CI saw the miss: a restart in the same
//! folder couldn't replace `claude.exe` (os error 32) while a process of the killed server still ran it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Api, start_with, temp};
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

/// Starts a task whose agent starts a grandchild and hangs; stops the server with `stop`; asserts that within a few
/// seconds neither the agent nor its grandchild runs. Leftovers are ended (by pid) before the assert fails.
async fn nothing_outlives(name: &str, stop: impl FnOnce(&mut std::process::Child)) {
    let dir = temp(name);
    let file = dir.join("pids.txt");
    let (mut serve, port) = start_with(&dir, &[("FAKE_HANG", "1"), ("FAKE_GRANDCHILD", file.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    api.cmd("runtime.create", json!({ "prompt": "fix the add bug", "cwd": dir.join("proj"), "agent": "claude" })).await;
    let at = Instant::now();
    let (agent, grandchild) = loop {
        if let Some(p) = pids(&file) {
            break p;
        }
        assert!(at.elapsed() < Duration::from_secs(30), "the agent never started its grandchild");
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
    nothing_outlives("serve-orphans-kill", |c| {
        let _ = c.kill();
    })
    .await;
}

/// SIGTERM: the clean stop (every plugin off, then the guard's sweep).
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_server_stopped_with_sigterm_leaves_no_agent_and_no_grandchild() {
    nothing_outlives("serve-orphans-term", |c| {
        let _ = Command::new("kill").args(["-TERM", &c.id().to_string()]).output();
    })
    .await;
}
