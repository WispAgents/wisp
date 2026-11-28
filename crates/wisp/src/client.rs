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
    /// the command can't run as typed or where it was typed: exits `cli::USAGE_EXIT`, not 1
    #[error("{0}")]
    Usage(String),
    /// the task the command names isn't there: exits `cli::NOT_FOUND`, so a script tells a mistyped id from a refusal
    #[error("{0}")]
    NotFound(String),
}

impl CliError {
    pub fn said(m: impl Into<String>) -> Self {
        Self::Said(m.into())
    }
    /// A command line that doesn't fit: exits `cli::USAGE_EXIT`.
    pub fn usage(m: impl Into<String>) -> Self {
        Self::Usage(m.into())
    }
    /// What `wisp` exits with when the command ends on this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => crate::cli::USAGE_EXIT,
            Self::NotFound(_) => crate::cli::NOT_FOUND,
            _ => 1,
        }
    }
    /// wisp's 404 for the task a command named (`runtime.get` of an id that isn't there) as [`Self::NotFound`]; any
    /// other error as it was.
    #[must_use]
    pub fn not_found(self) -> Self {
        match self {
            Self::Http { status: 404, message, .. } => Self::NotFound(message),
            e => e,
        }
    }
    pub fn status(&self) -> u16 {
        if let Self::Http { status, .. } = self { *status } else { 0 }
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

#[path = "client_hub.rs"]
mod hub;
pub use hub::grid_http;

#[derive(Debug, Clone)]
pub struct Client {
    pub url: String,
    http: reqwest::Client,
    token: Option<String>,
    /// `--on <machine>`: commands go to that peer through this wisp (`machines.route`, same key)
    on: Option<String>,
    peer: std::sync::Arc<tokio::sync::OnceCell<(String, String)>>,
    /// a team's hub (ADR-0147, `wisp grid plan|deploy`): every command goes there instead
    hub: Option<hub::Hub>,
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

/// Whether the wisp whose `/api/health` this is serves the home hashed `mine` (`wisp_api::home_hash`). One with no
/// `homeHash` is an older wisp, from before it said: taken as ours, so an upgrade keeps working with it.
pub fn serves_home(health: &Value, mine: &str) -> bool {
    health.get("homeHash").and_then(Value::as_str).is_none_or(|h| h == mine)
}

/// The one line for a wisp on the port that serves another home (two `WISP_HOME`s, one default port).
pub fn other_home(url: &str, home: &Path) -> CliError {
    let port = url.rsplit(':').next().unwrap_or(url);
    CliError::said(format!("port {port} is the wisp for another home, not {}: pass --port or run wisp serve", home.display()))
}

/// The port `server.json` advertises, if any.
pub fn discover(home: &Path) -> Option<u16> {
    let text = std::fs::read(home.join("server.json")).ok()?;
    let v: Value = serde_json::from_slice(&text).ok()?;
    v.get("port")?.as_u64().and_then(|p| u16::try_from(p).ok())
}

/// Where to reach wisp on `port`: the URL `server.json` advertises when it names that port (a `wisp serve --host ::1`
/// answers on `http://[::1]:P`, never on 127.0.0.1), else `127.0.0.1` on it.
pub fn url_for(server_json: Option<&Value>, port: u16) -> String {
    server_json
        .filter(|v| v.get("port").and_then(Value::as_u64) == Some(u64::from(port)))
        .and_then(|v| v.get("url").and_then(Value::as_str))
        .filter(|u| u.starts_with("http://"))
        .map_or_else(|| format!("http://127.0.0.1:{port}"), |u| u.trim_end_matches('/').to_owned())
}

impl Client {
    pub fn new(port: u16) -> Self {
        let http = reqwest::Client::builder().connect_timeout(Duration::from_secs(3)).build().unwrap_or_default();
        let home = crate::services::home(std::env::var("WISP_HOME").ok().as_deref(), crate::services::user_home());
        let advertised = std::fs::read(home.join("server.json")).ok().and_then(|t| serde_json::from_slice::<Value>(&t).ok());
        Self {
            url: url_for(advertised.as_ref(), port),
            http,
            token: std::env::var("WISP_TOKEN").ok().filter(|t| !t.is_empty()),
            on: ON.get().cloned(),
            peer: std::sync::Arc::default(),
            hub: None,
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
        match &self.hub {
            Some(hub::Hub::Key { url, key: bearer, source }) => return self.hub_cmd(url, bearer, source, cmd, args, &key).await,
            Some(hub::Hub::Via { team, source }) => {
                let call = serde_json::json!({ "team": team, "cmd": cmd, "args": args, "source": source });
                return self.local_cmd("grid-client.hubCall", call, &key).await;
            }
            None => {}
        }
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
        let r = self.mcp_post(msg, cwd).await?;
        r.json().await.map_err(|e| CliError::said(format!("wisp at {}: /api/mcp: {e}", self.url)))
    }

    /// One message to `/api/mcp`, its response as it comes (a stream's is read by `mcp_stdio::forward`).
    pub async fn mcp_post(&self, msg: &Value, cwd: &Path) -> Result<reqwest::Response> {
        let mut b = self.http.post(format!("{}/api/mcp", self.url)).header("x-wisp-cwd", cwd.display().to_string()).json(msg);
        if let Some(t) = &self.token {
            b = b.header("x-wisp-token", t);
        }
        b.send().await.map_err(|e| CliError::said(format!("wisp at {}: {e}", self.url)))
    }

    /// The wisp answering here (its `/api/health`), bounded: something that takes the connection and never answers
    /// isn't a wisp.
    pub async fn health(&self) -> Option<Value> {
        let health = tokio::time::timeout(Duration::from_secs(2), self.get("/health")).await;
        health.ok().and_then(Result::ok).filter(|v| v.get("ok").and_then(Value::as_bool) == Some(true))
    }

    /// Whether a wisp answers here, whichever home it serves.
    pub async fn ping(&self) -> bool {
        self.health().await.is_some()
    }

    /// What answers on this port for `home`: None (no wisp), Ok (this home's), or the refusal for another home's.
    pub async fn found(&self, home: &Path) -> Option<Result<()>> {
        let h = self.health().await?;
        Some(if serves_home(&h, &wisp_api::home_hash(home)) { Ok(()) } else { Err(other_home(&self.url, home)) })
    }

    /// Whether this home's own wisp answers here (another home's is refused by the command itself, `connect`).
    pub async fn serves(&self, home: &Path) -> bool {
        matches!(self.found(home).await, Some(Ok(())))
    }

    /// The running wisp, else a headless one started now (v1 `WispClient.connect`). A wisp for another home on the
    /// port is refused, never used and never raced by a second server.
    pub async fn connect(port: u16, start: bool) -> Result<Self> {
        let c = Self::new(port);
        let home = crate::services::home(std::env::var("WISP_HOME").ok().as_deref(), crate::services::user_home());
        if let Some(found) = c.found(&home).await {
            return found.map(|()| c);
        }
        // a server already holds this home (still starting, or on a port server.json didn't say yet): that one, never a
        // second headless server on the same home (it would refuse to start)
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

    use super::{serves_home, still_waking, url_for};

    /// cli-r6 M4: `wisp ls` reaches a `wisp serve --host ::1` through the URL its server.json says; a server.json for
    /// another port, or none, is 127.0.0.1 as before.
    #[test]
    fn the_client_goes_where_server_json_says_for_its_port() {
        let v6 = json!({ "url": "http://[::1]:4477", "port": 4477 });
        assert_eq!(url_for(Some(&v6), 4477), "http://[::1]:4477");
        assert_eq!(url_for(Some(&v6), 4500), "http://127.0.0.1:4500", "another port's");
        assert_eq!(url_for(None, 4477), "http://127.0.0.1:4477");
        assert_eq!(url_for(Some(&json!({ "port": 4477 })), 4477), "http://127.0.0.1:4477", "an older one with no url");
        assert_eq!(url_for(Some(&json!({ "url": "file:///x", "port": 4477 })), 4477), "http://127.0.0.1:4477");
    }

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

    // two homes, one default port: a wisp is used only for the home it serves; one too old to say is taken as ours
    #[test]
    fn a_wisp_is_ours_only_for_the_home_it_serves() {
        let dir = std::env::temp_dir().join(format!("wisp-home-hash-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        let a = wisp_api::home_hash(&dir.join("a"));
        assert_eq!(a, wisp_api::home_hash(&dir.join("b/../a")), "one home however it's spelled");
        assert!(serves_home(&json!({ "ok": true, "homeHash": a }), &a), "a match");
        assert!(!serves_home(&json!({ "ok": true, "homeHash": wisp_api::home_hash(&dir.join("b")) }), &a), "a mismatch");
        assert!(serves_home(&json!({ "ok": true, "name": "wisp" }), &a), "an older wisp, no homeHash");
        assert!(!a.contains("wisp-home-hash"), "a hash, never the path");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
