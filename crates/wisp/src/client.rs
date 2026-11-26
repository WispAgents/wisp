//! The CLI's way to the running wisp (v1 server/client.js): `server.json` names its port, `GET
//! /api/health` says it is up, and a scripted command with none running starts a headless one
//! (`wisp --no-open --port N`, detached) and waits for it.
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Said(String),
    /// an HTTP refusal or error from wisp, with its status
    #[error("{message}")]
    Http { status: u16, message: String, body: Value },
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl CliError {
    pub fn said(m: impl Into<String>) -> Self {
        Self::Said(m.into())
    }
    pub fn status(&self) -> u16 {
        if let Self::Http { status, .. } = self { *status } else { 0 }
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

#[derive(Debug, Clone)]
pub struct Client {
    pub url: String,
    http: reqwest::Client,
    token: Option<String>,
    /// `--on <machine>`: commands go to that peer through this wisp (`machines.route`, same key)
    on: Option<String>,
    peer: std::sync::Arc<tokio::sync::OnceCell<(String, String)>>,
}

static ON: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// `--on <machine>`: every client made from now on runs its commands there (ADR-0041).
/// How long a command for a waking box is waited for (a cloud computer's cold boot), past route's own 30 s each time.
const WAKE_WAIT: Duration = Duration::from_secs(300);

/// machines.route's "waiting for <box> to wake" (`retry: true`): the command is still queued for the box.
fn still_waking(body: &Value) -> bool {
    body.pointer("/error/retry") == Some(&Value::Bool(true))
        && body.pointer("/error/message").and_then(Value::as_str).is_some_and(|m| m.ends_with(" to wake"))
}

pub fn set_on(machine: &str) {
    let _ = ON.set(machine.to_owned());
}

/// Commands that are always this wisp's own, even with `--on`: the machines themselves.
fn local_only(cmd: &str) -> bool {
    cmd.starts_with("machines.") || cmd.starts_with("grid-client.") || cmd == "cli.catalogue"
}

/// The peer `on` names in `machines.list`: its id, or its name (any case). The words otherwise.
pub fn find_peer(list: &Value, on: &str) -> std::result::Result<(String, String), String> {
    let all = list.as_array().cloned().unwrap_or_default();
    let s = |p: &Value, k: &str| p.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let hit = all.iter().find(|p| s(p, "id") == on).or_else(|| all.iter().find(|p| s(p, "name").eq_ignore_ascii_case(on)));
    match hit {
        Some(p) => Ok((s(p, "id"), s(p, "name"))),
        None if all.is_empty() => {
            Err(format!("there's no machine {on}: this wisp has none yet (wisp cloud signin, wisp computer on, or Machines)"))
        }
        None => Err(format!("there's no machine {on}; there's {}", all.iter().map(|p| s(p, "name")).collect::<Vec<_>>().join(", "))),
    }
}

/// The port `server.json` advertises, if any.
pub fn discover(home: &Path) -> Option<u16> {
    let text = std::fs::read(home.join("server.json")).ok()?;
    let v: Value = serde_json::from_slice(&text).ok()?;
    v.get("port")?.as_u64().and_then(|p| u16::try_from(p).ok())
}

impl Client {
    pub fn new(port: u16) -> Self {
        let http = reqwest::Client::builder().connect_timeout(Duration::from_secs(3)).build().unwrap_or_default();
        Self {
            url: format!("http://127.0.0.1:{port}"),
            http,
            token: std::env::var("WISP_TOKEN").ok().filter(|t| !t.is_empty()),
            on: ON.get().cloned(),
            peer: std::sync::Arc::default(),
        }
    }

    /// The port this client reaches wisp on (the one `connect` found, not the default), so a child `wisp` it starts
    /// (the TUI's Open in) talks to the same server.
    pub fn port(&self) -> Option<u16> {
        self.url.rsplit_once(':').and_then(|(_, p)| p.parse().ok())
    }

    /// `--on <machine>`: the peer its commands run on, if any.
    pub fn on(&self) -> Option<&str> {
        self.on.as_deref()
    }

    pub async fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        let m = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| CliError::said(e.to_string()))?;
        let read = m == reqwest::Method::GET || m == reqwest::Method::HEAD;
        // wisp takes only JSON on anything but a read, a body-less DELETE included
        let body = if read { body } else { Some(body.unwrap_or_else(|| serde_json::json!({}))) };
        let mut b = self.http.request(m, format!("{}/api{path}", self.url));
        if let Some(t) = &self.token {
            b = b.header("x-wisp-token", t);
        }
        if let Some(body) = body {
            b = b.json(&body);
        }
        let r = b.send().await.map_err(|e| CliError::said(format!("wisp at {}: {e}", self.url)))?;
        let status = r.status().as_u16();
        let text = r.text().await.unwrap_or_default();
        let json: Value = if text.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({ "text": text }))
        };
        if status >= 400 {
            let message = json
                .get("error")
                .and_then(|e| e.as_str().map(str::to_owned).or_else(|| e.get("message").and_then(Value::as_str).map(str::to_owned)))
                .unwrap_or_else(|| format!("HTTP {status}"));
            let message = if status == 404 && message == "not found" { format!("{method} /api{path}: not in this build") } else { message };
            return Err(CliError::Http { status, message, body: json });
        }
        Ok(json)
    }

    /// The peer `--on` names, once: its id and name (it says which machine answers, on stderr).
    async fn peer(&self, on: &str) -> Result<(String, String)> {
        let c = self.clone();
        let on = on.to_owned();
        self.peer
            .get_or_try_init(|| async move {
                let list = c
                    .local_cmd(
                        "machines.list",
                        serde_json::json!({}),
                        &format!("cli-{}", crate::services::hex(&rand::random::<[u8; 12]>())),
                    )
                    .await?;
                let (id, name) = find_peer(&list, &on).map_err(CliError::said)?;
                eprintln!("  ◌ on {name}");
                Ok::<_, CliError>((id, name))
            })
            .await
            .cloned()
    }

    /// Runs a command through `POST /api/cmd` (once per key), its result or the wire error's message. With
    /// `--on`, on that machine: through `machines.route` with the same key.
    pub async fn cmd(&self, cmd: &str, args: Value) -> Result<Value> {
        let key = format!("cli-{}", crate::services::hex(&rand::random::<[u8; 12]>()));
        match self.on.as_deref() {
            Some(on) if !local_only(cmd) => {
                let (id, _) = self.peer(on).await?;
                let route = serde_json::json!({ "id": id, "cmd": cmd, "args": args, "mid": key });
                // A box still waking: machines.route answers "waiting for <box> to wake" (retry) after 30 s while the
                // command stays queued on the link under `mid`. Ask again with the same mid (a fresh HTTP key, so the
                // edge doesn't replay the refusal): it runs once, and its answer comes back here (B51).
                let deadline = Instant::now() + WAKE_WAIT;
                let mut attempt = 0u32;
                loop {
                    let http_key = if attempt == 0 { key.clone() } else { format!("{key}-{attempt}") };
                    match self.local_cmd("machines.route", route.clone(), &http_key).await {
                        Err(CliError::Http { body, message, .. }) if still_waking(&body) && Instant::now() < deadline => {
                            if attempt == 0 {
                                eprintln!("  ◌ {message}…");
                            }
                            attempt += 1;
                        }
                        other => return other,
                    }
                }
            }
            _ => self.local_cmd(cmd, args, &key).await,
        }
    }

    async fn local_cmd(&self, cmd: &str, args: Value, key: &str) -> Result<Value> {
        let mut b = self
            .http
            .post(format!("{}/api/cmd", self.url))
            .header("idempotency-key", key)
            .json(&serde_json::json!({ "cmd": cmd, "args": args }));
        if let Some(t) = &self.token {
            b = b.header("x-wisp-token", t);
        }
        let r = b.send().await.map_err(|e| CliError::said(format!("wisp at {}: {e}", self.url)))?;
        let status = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or_default();
        if v.get("ok") == Some(&Value::Bool(true)) {
            return Ok(v.get("result").cloned().unwrap_or(Value::Null));
        }
        let message = v.pointer("/error/message").and_then(Value::as_str).map_or_else(|| format!("{cmd}: HTTP {status}"), str::to_owned);
        Err(CliError::Http { status, message, body: v })
    }

    /// The token this client sends (`WISP_TOKEN`), for the stream it opens itself.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        self.request("GET", path, None).await
    }

    /// One JSON-RPC message to `/api/mcp` as the user's editor working in `cwd` (ADR-0040): its answer, as it came.
    pub async fn mcp(&self, msg: &Value, cwd: &Path) -> Result<Value> {
        let mut b = self.http.post(format!("{}/api/mcp", self.url)).header("x-wisp-cwd", cwd.display().to_string()).json(msg);
        if let Some(t) = &self.token {
            b = b.header("x-wisp-token", t);
        }
        let r = b.send().await.map_err(|e| CliError::said(format!("wisp at {}: {e}", self.url)))?;
        r.json().await.map_err(|e| CliError::said(format!("wisp at {}: /api/mcp: {e}", self.url)))
    }

    /// Whether a wisp answers here, bounded: something that takes the connection and never answers isn't a wisp.
    pub async fn ping(&self) -> bool {
        let health = tokio::time::timeout(Duration::from_secs(2), self.get("/health")).await;
        health.ok().and_then(Result::ok).and_then(|v| v.get("ok").and_then(Value::as_bool)) == Some(true)
    }

    /// The running wisp, else a headless one started now (v1 `WispClient.connect`).
    pub async fn connect(port: u16, start: bool) -> Result<Self> {
        let c = Self::new(port);
        if c.ping().await {
            return Ok(c);
        }
        // a server already holds this home (still starting, or on a port server.json didn't say yet): that one, never a
        // second headless server on the same home (it would refuse to start)
        let home = crate::services::home(std::env::var("WISP_HOME").ok().as_deref(), crate::services::user_home());
        if crate::serve::home_held(&home) {
            let end = Instant::now() + Duration::from_secs(15);
            while Instant::now() < end {
                if let Some(at) = discover(&home).map(Self::new)
                    && at.ping().await
                {
                    return Ok(at);
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            return Err(CliError::said(format!("wisp is running for {} but does not answer", home.display())));
        }
        if !start {
            return Err(CliError::said(format!("wisp is not running at {}", c.url)));
        }
        let exe = std::env::current_exe()?;
        let mut cmd = std::process::Command::new(exe);
        cmd.args(["--no-open", "--port", &port.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        // the server outlives this command: it must not hold the pipes our caller reads (as desktop::launch)
        #[cfg(feature = "proc")]
        wisp_proc::keep_std_handles();
        cmd.spawn()?;
        let end = Instant::now() + Duration::from_secs(15);
        while Instant::now() < end {
            if c.ping().await {
                return Ok(c);
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        Err(CliError::said(format!("wisp did not start at {}", c.url)))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::still_waking;

    // B51: only route's "waiting for <box> to wake" (retry) is waited out; any other refusal is the answer
    #[test]
    fn a_waking_box_is_waited_for_and_nothing_else() {
        assert!(still_waking(
            &json!({ "ok": false, "error": { "code": "waiting", "message": "waiting for Cloud to wake", "retry": true } })
        ));
        assert!(!still_waking(&json!({ "ok": false, "error": { "message": "waiting for Cloud to wake", "retry": false } })));
        assert!(!still_waking(&json!({ "ok": false, "error": { "message": "Cloud is offline", "retry": true } })));
        assert!(!still_waking(&json!({})));
    }
}
