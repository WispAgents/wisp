//! Starting a box (v1 grid/hostd.js `spawnBox`, `untilHealthy`): the process (or boxd's container) with the environment
//! every box gets, and the wait until it answers.
use std::time::Duration;

use wisp_kernel::exec::windows_base_env;

use super::{ProcessHost, lock, preview_template};
use crate::boxhost::BoxState;
use crate::error::{Result, bad};
use crate::rows::BoxUser;

/// A started box: a process here, or a container boxd runs.
#[derive(Debug)]
pub(super) enum BoxProc {
    /// boxed: a Child is 272 bytes on Windows, a [`crate::boxd::BoxdRun`] 32
    Local(Box<tokio::process::Child>),
    Boxd(crate::boxd::BoxdRun),
}

impl BoxProc {
    /// It ended, and how.
    pub(super) fn exited(&mut self) -> Option<String> {
        match self {
            Self::Local(c) => c.try_wait().ok().flatten().map(|s| s.to_string()),
            Self::Boxd(r) => r.exited().map(|e| {
                let why = r.error();
                if why.is_empty() { e } else { why }
            }),
        }
    }

    pub(super) async fn stop(self) {
        match self {
            Self::Local(mut c) => {
                let _ = c.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(10), c.wait()).await;
            }
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
        Ok(BoxProc::Local(Box::new(cmd.spawn().map_err(|e| bad(format!("wisp didn't start in the box: {e}"), 503))?)))
    }

    /// Waits for the started box to answer `/api/health`: its process, or why it never came up (it stopped as it
    /// started, it took longer than `startMs`, the grid began stopping). Either way but up, it's marked stopped.
    pub(super) async fn healthy(&self, user: &BoxUser, port: u16, child: BoxProc) -> Result<BoxProc> {
        let deadline = self.clock.now_ms() + self.o.start_ms;
        let mut child = child;
        let failed = |event: &str| {
            lock(&self.inst).entry(user.id.clone()).or_default().state = BoxState::Stopped;
            self.change(user, BoxState::Stopped, event);
        };
        loop {
            if self.is_closed() {
                // the grid is stopping: this box doesn't outlive it
                child.stop().await;
                failed("grid stopped");
                return Err(bad("The grid is shutting down.", 503));
            }
            if self.http.get(format!("http://127.0.0.1:{port}/api/health")).send().await.is_ok_and(|r| r.status().is_success()) {
                return Ok(child);
            }
            if let Some(st) = child.exited() {
                failed("failed");
                return Err(bad(format!("wisp in the box stopped as it started ({st})."), 503));
            }
            if self.clock.now_ms() > deadline {
                failed("failed");
                return Err(bad("wisp in the box didn't answer in time.", 504));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use super::*;

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
