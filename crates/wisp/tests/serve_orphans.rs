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
