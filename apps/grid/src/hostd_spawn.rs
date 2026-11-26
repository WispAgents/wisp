//! Starting a box (v1 grid/hostd.js `spawnBox`, `untilHealthy`): the process (or boxd's container) with the environment
//! every box gets, and the wait until it answers.
use std::time::Duration;

use tokio::sync::{oneshot, watch};
use wisp_kernel::exec::windows_base_env;

use super::{ProcessHost, lock, preview_template};
use crate::error::{GridError, Result, bad};
use crate::rows::BoxUser;

/// A local child process wrapped like [`crate::boxd::BoxdRun`]: its exit is watched on a
/// `watch::Receiver`, and dropping it without `stop()` kills and reaps the child.
#[derive(Debug)]
pub(super) struct LocalRun {
    stop_tx: Option<oneshot::Sender<()>>,
    exit: watch::Receiver<Option<String>>,
}

impl LocalRun {
    fn spawn(mut child: tokio::process::Child) -> Self {
        let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
        let (tx, exit) = watch::channel(None);
        tokio::spawn(async move {
            let why = tokio::select! {
                st = child.wait() => match st {
                    Ok(s) => s.to_string(),
                    Err(e) => format!("wait error: {e}"),
                },
                _ = &mut stop_rx => {
                    let _ = child.start_kill();
                    match tokio::time::timeout(Duration::from_secs(10), child.wait()).await {
                        Ok(Ok(s)) => s.to_string(),
                        _ => "killed".into(),
                    }
                }
            };
            let _ = tx.send(Some(why));
        });
        Self { stop_tx: Some(stop_tx), exit }
    }

    fn exited(&self) -> Option<String> {
        self.exit.borrow().clone()
    }

    fn watch_exit(&self) -> watch::Receiver<Option<String>> {
        self.exit.clone()
    }

    async fn stop(mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        let _ = tokio::time::timeout(Duration::from_secs(11), self.exit.wait_for(Option::is_some)).await;
    }
}

/// A started box: a process here, or a container boxd runs.
#[derive(Debug)]
pub(super) enum BoxProc {
    Local(LocalRun),
    Boxd(crate::boxd::BoxdRun),
}

impl BoxProc {
    /// It ended, and how.
    pub(super) fn exited(&self) -> Option<String> {
        match self {
            Self::Local(c) => c.exited(),
            Self::Boxd(r) => r.exited().map(|e| {
                let why = r.error();
                if why.is_empty() { e } else { why }
            }),
        }
    }

    /// Fires when the box exits (for both process and boxd container drivers).
    pub(super) fn watch_exit(&self) -> watch::Receiver<Option<String>> {
        match self {
            Self::Local(c) => c.watch_exit(),
            Self::Boxd(r) => r.watch_exit(),
        }
    }

    pub(super) async fn stop(self) {
        match self {
            Self::Local(c) => c.stop().await,
            Self::Boxd(r) => r.stop(Duration::from_secs(10)).await,
        }
    }
}

impl ProcessHost {
    /// The environment every box gets (its secret goes in boxd's 0600 env file, never on a command line).
    fn box_env(&self, user: &BoxUser, port: u16, secret: &str) -> Vec<(String, String)> {
        let mut env = vec![
            ("WISP_PORT".to_owned(), port.to_string()),
            ("WISP_BOX".to_owned(), "1".to_owned()),
            ("WISP_TRUSTED_PROXY_SECRET".to_owned(), secret.to_owned()),
            ("WISP_PUBLIC_URL".to_owned(), (self.o.url_of)(&user.handle)),
        ];
        env.extend(self.o.env.iter().cloned());
        // where its pages show (ADR-0064): its own address with `p{port}--` before it; a sandbox's aren't shown
        if self.o.previews && user.sandbox.is_none() {
            env.push(("WISP_PREVIEW_URL".to_owned(), preview_template(&(self.o.url_of)(&user.handle))));
        }
        // a sandbox's wisp knows it is one: it takes a project in, and publishes, shares and makes nothing (ADR-0047)
        if user.sandbox.is_some() {
            env.push(("WISP_SANDBOX".to_owned(), "1".to_owned()));
        }
        let more = lock(&self.env_for.0).clone();
        if let Some(f) = more {
            env.extend(f(user));
        }
        env
    }

    pub(super) async fn spawn(&self, user: &BoxUser, port: u16, secret: &str) -> Result<BoxProc> {
        let logs = self.o.data.join("users").join(&user.id);
        std::fs::create_dir_all(&logs).map_err(|e| bad(format!("the box's folder: {e}"), 500))?;
        if let Some(d) = &self.o.boxd {
            let env: serde_json::Map<String, serde_json::Value> =
                self.box_env(user, port, secret).into_iter().map(|(k, v)| (k, serde_json::Value::String(v))).collect();
            let mut req = serde_json::json!({ "id": user.id, "port": port, "env": env, "sandbox": user.sandbox.is_some() });
            if !d.limits.is_null() {
                req["limits"] = d.limits.clone();
            }
            if let Some(dns) = &d.dns {
                req["dns"] = serde_json::json!(dns);
            }
            return Ok(BoxProc::Boxd(crate::boxd::BoxdRun::start(&d.socket, req, logs.join("wisp.log")).await?));
        }
        let home = logs.join("home");
        std::fs::create_dir_all(&home).map_err(|e| bad(format!("the box's home: {e}"), 500))?;
        // its last run's `server.json` isn't this one's port: gone before it starts, so the one `healthy` reads is new
        match std::fs::remove_file(home.join("server.json")) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(bad(format!("the box's last server.json: {e}"), 500)),
            _ => {}
        }
        let mut cmd = tokio::process::Command::new(&self.o.bin);
        cmd.arg("serve")
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default()) // path: inherited the grid is a server, no desktop app hands it one
            .env("HOME", &logs)
            .env("WISP_HOME", &home);
        // what Windows can't start without (`SystemRoot` for Winsock, …), the box's folder as its profile
        let profile = logs.display().to_string();
        for (k, v) in windows_base_env(cfg!(windows), Some(&profile), |k| std::env::var(k).ok()) {
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::fs::File::create(logs.join("wisp.log")).map_or_else(|_| std::process::Stdio::null(), std::process::Stdio::from))
            .kill_on_drop(true);
        for (k, v) in self.box_env(user, port, secret) {
            cmd.env(k, v);
        }
        let child = cmd.spawn().map_err(|e| bad(format!("wisp didn't start in the box: {e}"), 503))?;
        Ok(BoxProc::Local(LocalRun::spawn(child)))
    }

    /// Waits for the started box to answer `/api/health`: its process and the port it answers on, or why it never came
    /// up (it stopped as it started, it took longer than `startMs`, the grid began stopping) with the event that says
    /// so; `start` marks it stopped then. `port` 0: a process box binds its own, read from its `server.json` once it's
    /// written.
    pub(super) async fn healthy(
        &self,
        user: &BoxUser,
        port: u16,
        child: BoxProc,
    ) -> std::result::Result<(BoxProc, u16), (&'static str, GridError)> {
        let deadline = self.clock.now_ms() + self.o.start_ms;
        let mut port = port;
        let home = self.o.data.join("users").join(&user.id).join("home");
        loop {
            if self.is_closed() {
                // the grid is stopping: this box doesn't outlive it
                child.stop().await;
                return Err(("grid stopped", bad("The grid is shutting down.", 503)));
            }
            if port == 0 {
                port = tokio::fs::read(home.join("server.json")).await.ok().and_then(|b| bound_port(&b)).unwrap_or(0);
            }
            if port != 0 && self.http.get(format!("http://127.0.0.1:{port}/api/health")).send().await.is_ok_and(|r| r.status().is_success())
            {
                return Ok((child, port));
            }
            if let Some(st) = child.exited() {
                return Err(("failed", bad(format!("wisp in the box stopped as it started ({st})."), 503)));
            }
            if self.clock.now_ms() > deadline {
                child.stop().await;
                return Err(("failed", bad("wisp in the box didn't answer in time.", 504)));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// The port a process box bound, from the `server.json` it writes in its `WISP_HOME` (as every `wisp serve` does; the
/// last run's is removed before it starts, so whatever process wrote it, a launcher's wisp included, it's this run's).
/// None: not written yet, or half written.
pub(super) fn bound_port(server_json: &[u8]) -> Option<u16> {
    let v: serde_json::Value = serde_json::from_slice(server_json).ok()?;
    v.get("port").and_then(serde_json::Value::as_u64).and_then(|p| u16::try_from(p).ok()).filter(|p| *p != 0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// A process box's port is the one it bound and wrote in its `server.json`, never one read before it started (the
    /// macOS flake: a port closed by the grid was taken before the box bound it).
    #[test]
    fn a_process_box_s_port_is_the_one_its_server_json_names() {
        assert_eq!(bound_port(br#"{"url":"http://127.0.0.1:51234","port":51234,"pid":7}"#), Some(51234));
        assert_eq!(bound_port(br#"{"port":0}"#), None);
        assert_eq!(bound_port(br#"{"url":"#), None, "half written");
        assert_eq!(bound_port(b""), None);
    }

    /// A box's cleared environment on Windows (a runner's values): `SystemRoot` and the rest a process needs come
    /// with it, its profile is its own folder, and a variable the grid doesn't have isn't made up. Off Windows: none.
    #[test]
    fn a_process_box_on_windows_keeps_what_windows_needs_to_start() {
        let grid = |k: &str| match k {
            "SystemRoot" | "windir" => Some(r"C:\Windows".to_owned()),
            "ComSpec" => Some(r"C:\Windows\system32\cmd.exe".to_owned()),
            "PATHEXT" => Some(".COM;.EXE;.BAT;.CMD".to_owned()),
            "TEMP" | "TMP" => Some(r"C:\Users\RUNNER~1\AppData\Local\Temp".to_owned()),
            "USERPROFILE" => Some(r"C:\Users\runneradmin".to_owned()),
            _ => None,
        };
        let env: HashMap<String, String> = windows_base_env(true, Some(r"D:\grid\users\u1"), grid).into_iter().collect();
        assert_eq!(env.get("SystemRoot").map(String::as_str), Some(r"C:\Windows"), "Winsock loads");
        assert_eq!(env.get("ComSpec").map(String::as_str), Some(r"C:\Windows\system32\cmd.exe"));
        assert_eq!(env.get("TEMP").map(String::as_str), Some(r"C:\Users\RUNNER~1\AppData\Local\Temp"));
        assert_eq!(env.get("USERPROFILE").map(String::as_str), Some(r"D:\grid\users\u1"), "the box's own, not the grid's");
        assert!(!env.contains_key("SystemDrive"), "only what the grid has");
        assert_eq!(windows_base_env(false, Some("/g/users/u1"), grid), Vec::new());
    }
}
