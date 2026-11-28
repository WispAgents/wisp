//! The kernel services this install provides (plan §6.1): paths (`~/.wisp` or `WISP_HOME`), secrets
//! (`secrets.json`, 0600, atomic), net (one reqwest client), the clock, notify (the edge's hub), and,
//! as their crates land, proc (wisp-proc) and the store (wisp-store). The grid and tests swap any.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Mutex;
use wisp_kernel::services::{Net, NetRequest, NetResponse, NetService, NetStream, NotifyService, Paths, Secrets, SecretsService};
use wisp_kernel::{App, BoxFuture, Clock, Result, WireError};

/// `WISP_HOME`, else `~/.wisp`; spelled once, here, as every path in wisp is ([`plain`]).
pub fn home(env_home: Option<&str>, user_home: Option<PathBuf>) -> PathBuf {
    plain(match env_home.filter(|h| !h.is_empty()) {
        Some(h) => PathBuf::from(h),
        None => user_home.unwrap_or_else(|| PathBuf::from(".")).join(".wisp"),
    })
}

/// `HOME`, else `USERPROFILE`, spelled as every path in wisp is ([`plain`]).
pub fn user_home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(|h| plain(PathBuf::from(h)))
}

/// One folder, one spelling (ADR-0071): a path that enters wisp (its home, the user's, the CLI's folder) with
/// Windows' 8.3 short names spelled out (`%TEMP%` is often `C:\Users\RUNNER~1\…`), so every folder wisp derives
/// from it (task copies, worktrees, scratch folders) and every folder a task stores compare as one. Startup only
/// (blocking: one file-system call on Windows; none elsewhere).
pub fn plain(p: PathBuf) -> PathBuf {
    #[cfg(feature = "proc")]
    let p = wisp_proc::plain_path_blocking(p);
    p
}

type Data = BTreeMap<String, BTreeMap<String, String>>;

/// `secrets.json` in v1's shape `{ns: {key: value}}`, so a v1 install's secrets read as they are:
/// `ns/key` is `data[ns][key]`, a key without `/` lives in `wisp`. Writes are serialised, go to a tmp
/// file made 0600 from the start (and chmod'ed in case it existed), then renamed over.
#[derive(Debug, Clone)]
pub struct FileSecrets {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    file: PathBuf,
    data: Mutex<Option<Data>>,
    /// a team hub's secrets (ADR-0148): set on the grid, given to its box as env vars, named in `WISP_HUB_SECRETS`.
    /// They answer as `${secret:NAME}` when the file has no such key (read once: a box's env doesn't change).
    team: std::collections::BTreeMap<String, String>,
}

fn split(key: &str) -> (String, String) {
    match key.split_once('/') {
        Some((ns, k)) => (ns.to_owned(), k.to_owned()),
        None => ("wisp".to_owned(), key.to_owned()),
    }
}

fn io(e: &std::io::Error) -> WireError {
    WireError::internal(format!("secrets.json: {e}"))
}

/// `secrets.json`'s bytes as [`Data`]. A document that isn't a JSON object fails (the caller then leaves the file
/// alone); inside it, a namespace that isn't an object or a value that isn't a string (a v1 bug, a hand edit) is
/// dropped with a warning and the rest kept.
fn parse(b: &[u8]) -> Result<Data> {
    let bad = |why: String| WireError::internal(format!("secrets.json: {why}; fix or move the file, wisp won't write over it"));
    let Value::Object(top) = serde_json::from_slice::<Value>(b).map_err(|e| bad(e.to_string()))? else {
        return Err(bad("not a JSON object".to_owned()));
    };
    let mut d = Data::new();
    for (ns, kv) in top {
        let Value::Object(kv) = kv else {
            tracing::warn!(namespace = %ns, "secrets.json: namespace isn't an object, skipped");
            continue;
        };
        let m = d.entry(ns.clone()).or_default();
        for (k, v) in kv {
            if let Value::String(s) = v {
                m.insert(k, s);
            } else {
                tracing::warn!(namespace = %ns, key = %k, "secrets.json: value isn't a string, skipped");
            }
        }
    }
    Ok(d)
}

impl FileSecrets {
    pub fn new(home: &Path) -> Self {
        Self::with_team(home, team_secrets(&|k| std::env::var(k).ok()))
    }

    /// With a team hub's secrets beside the file's (tests give their own).
    pub fn with_team(home: &Path, team: std::collections::BTreeMap<String, String>) -> Self {
        Self { inner: Arc::new(Inner { file: home.join("secrets.json"), data: Mutex::new(None), team }) }
    }
}

/// A team hub's secrets from its environment: the names in `WISP_HUB_SECRETS` (comma-separated, set by the grid for a
/// hub's box only, `WISP_HUB=1`), each with its env var's value. Anywhere else, none.
pub fn team_secrets(env: &dyn Fn(&str) -> Option<String>) -> std::collections::BTreeMap<String, String> {
    if env("WISP_HUB").as_deref() != Some("1") {
        return std::collections::BTreeMap::new();
    }
    let names = env("WISP_HUB_SECRETS").unwrap_or_default();
    names.split(',').map(str::trim).filter(|n| !n.is_empty()).filter_map(|n| Some((n.to_owned(), env(n)?))).collect()
}

impl Inner {
    /// The file's secrets, cached once read. Only a missing file reads as empty: a file that can't be read
    /// (permissions, a disk error) is an error and isn't cached, so the next call tries again, and a file
    /// that isn't a JSON object is an error too. Either way nothing is written over it, so a bad moment
    /// never costs every key.
    async fn load(&self, slot: &mut Option<Data>) -> Result<Data> {
        if let Some(d) = slot {
            return Ok(d.clone());
        }
        let d = match tokio::fs::read(&self.file).await {
            Ok(b) => parse(&b)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data::new(),
            Err(e) => return Err(io(&e)),
        };
        *slot = Some(d.clone());
        Ok(d)
    }

    async fn save(&self, d: &Data) -> Result<()> {
        let dir = self.file.parent().unwrap_or(Path::new("."));
        tokio::fs::create_dir_all(dir).await.map_err(|e| io(&e))?;
        let tmp = self.file.with_extension("json.tmp");
        let text = serde_json::to_vec_pretty(d).map_err(|e| WireError::internal(e.to_string()))?;
        write_private(&tmp, &text).await.map_err(|e| io(&e))?;
        rename_retrying(|| tokio::fs::rename(&tmp, &self.file)).await.map_err(|e| io(&e))
    }

    async fn update(&self, f: impl FnOnce(&mut Data) -> bool) -> Result<()> {
        let mut slot = self.data.lock().await;
        let mut d = self.load(&mut slot).await?;
        if f(&mut d) {
            self.save(&d).await?;
            *slot = Some(d);
        }
        Ok(())
    }
}

/// A rename over a file something holds open for a moment fails on Windows (antivirus, the search indexer, a
/// reader): a new connector's token or a sign-in was refused with "permission denied". Waits it out, ~0.5 s at
/// most, as v1's `writeJsonAtomic` did (#567); any other error fails at once.
pub async fn rename_retrying<F, Fut>(mut rename: F) -> std::io::Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::io::Result<()>>,
{
    const TRIES: u64 = 10;
    let mut i = 1;
    loop {
        match rename().await {
            Err(e) if i < TRIES && held(&e) => tokio::time::sleep(Duration::from_millis(i * 10)).await,
            r => return r,
        }
        i += 1;
    }
}

/// The errors a briefly held file gives: access denied (EPERM/EACCES), busy, and Windows' sharing and lock
/// violations (`ERROR_SHARING_VIOLATION` 32, `ERROR_LOCK_VIOLATION` 33), which have no `ErrorKind` of their own.
fn held(e: &std::io::Error) -> bool {
    use std::io::ErrorKind as K;
    matches!(e.kind(), K::PermissionDenied | K::ResourceBusy) || (cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)))
}

/// Writes `bytes` to `file`, readable by its owner only.
pub async fn write_private(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut o = tokio::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    o.mode(0o600);
    let mut f = o.open(file).await?;
    tokio::io::AsyncWriteExt::write_all(&mut f, bytes).await?;
    f.sync_all().await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600)).await?;
    }
    Ok(())
}

impl Secrets for FileSecrets {
    fn get(&self, key: &str) -> BoxFuture<'static, Result<Option<String>>> {
        let (me, (ns, k)) = (self.inner.clone(), split(key));
        Box::pin(async move {
            let mut slot = me.data.lock().await;
            let own = me.load(&mut slot).await?.get(&ns).and_then(|m| m.get(&k)).cloned();
            Ok(own.or_else(|| if ns == "wisp" { me.team.get(&k).cloned() } else { None }))
        })
    }
    fn set(&self, key: &str, value: &str) -> BoxFuture<'static, Result<()>> {
        let (me, (ns, k), v) = (self.inner.clone(), split(key), value.to_owned());
        Box::pin(async move {
            me.update(|d| {
                let slot = d.entry(ns).or_default();
                slot.insert(k, v.clone()) != Some(v)
            })
            .await
        })
    }
    fn delete(&self, key: &str) -> BoxFuture<'static, Result<()>> {
        let (me, (ns, k)) = (self.inner.clone(), split(key));
        Box::pin(async move {
            me.update(|d| {
                let Some(b) = d.get_mut(&ns) else { return false };
                let had = b.remove(&k).is_some();
                if b.is_empty() {
                    d.remove(&ns);
                }
                had
            })
            .await
        })
    }
    fn keys(&self, prefix: &str) -> BoxFuture<'static, Result<Vec<String>>> {
        let (me, prefix) = (self.inner.clone(), prefix.to_owned());
        Box::pin(async move {
            let mut slot = me.data.lock().await;
            let d = me.load(&mut slot).await?;
            let mut out: Vec<String> = d
                .iter()
                .flat_map(|(ns, kv)| kv.keys().map(move |k| if ns == "wisp" { k.clone() } else { format!("{ns}/{k}") }))
                .chain(me.team.keys().cloned())
                .filter(|k| k.starts_with(&prefix))
                .collect();
            out.sort();
            out.dedup();
            Ok(out)
        })
    }
}

/// HTTP out through one reqwest client (rustls), with a timeout on every request.
#[derive(Debug, Clone)]
pub struct ReqwestNet {
    client: reqwest::Client,
    version: String,
}

impl ReqwestNet {
    pub fn new(version: &str) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(format!("wisp/{version}"))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self { client, version: version.to_owned() }
    }
}

impl Net for ReqwestNet {
    fn fetch(&self, req: NetRequest) -> BoxFuture<'static, Result<NetResponse>> {
        let client = self.client.clone();
        Box::pin(async move {
            let method =
                reqwest::Method::from_bytes(req.method.to_ascii_uppercase().as_bytes()).map_err(|e| WireError::invalid(e.to_string()))?;
            let timeout = Duration::from_millis(if req.timeout_ms == 0 { 30_000 } else { req.timeout_ms });
            let mut b = client.request(method, &req.url).timeout(timeout).body(req.body);
            for (k, v) in &req.headers {
                b = b.header(k, v);
            }
            let r = b.send().await.map_err(|e| WireError::unavailable(format!("{}: {e}", req.url)))?;
            let status = r.status().as_u16();
            let headers = r.headers().iter().filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_owned(), v.to_owned()))).collect();
            let body = r.bytes().await.map_err(|e| WireError::unavailable(e.to_string()))?.to_vec();
            Ok(NetResponse { status, headers, body })
        })
    }

    fn fetch_untrusted(&self, req: NetRequest, allow_private: bool) -> BoxFuture<'static, Result<NetResponse>> {
        let version = self.version.clone();
        Box::pin(async move {
            let url = reqwest::Url::parse(&req.url).map_err(|e| WireError::invalid(format!("{}: {e}", req.url)))?;
            let host = url.host_str().ok_or_else(|| WireError::invalid(format!("{} names no host", req.url)))?.to_owned();
            let port = url.port_or_known_default().unwrap_or(443);
            let bare = host.trim_start_matches('[').trim_end_matches(']').to_owned();
            // resolved now, and connected to as resolved: the name can't move between the check and the connection
            let addr = match bare.parse::<std::net::IpAddr>() {
                Ok(ip) => std::net::SocketAddr::new(ip, port),
                Err(_) => tokio::net::lookup_host((bare.as_str(), port))
                    .await
                    .map_err(|e| WireError::unavailable(format!("{host}: {e}")))?
                    .next()
                    .ok_or_else(|| WireError::unavailable(format!("{host} doesn't resolve")))?,
            };
            if !allow_private && !wisp_kernel::services::is_global(addr.ip()) {
                return Err(WireError::invalid(format!("{host} is not a public address ({})", addr.ip())));
            }
            let client = reqwest::Client::builder()
                .user_agent(format!("wisp/{version}"))
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .resolve(&bare, addr)
                .build()
                .map_err(|e| WireError::internal(e.to_string()))?;
            let method =
                reqwest::Method::from_bytes(req.method.to_ascii_uppercase().as_bytes()).map_err(|e| WireError::invalid(e.to_string()))?;
            let timeout = Duration::from_millis(if req.timeout_ms == 0 { 10_000 } else { req.timeout_ms });
            let mut b = client.request(method, url).timeout(timeout).body(req.body);
            for (k, v) in &req.headers {
                b = b.header(k, v);
            }
            let r = b.send().await.map_err(|e| {
                WireError::unavailable(if e.is_timeout() {
                    "timeout".to_owned()
                } else if e.is_connect() {
                    "connection_refused".to_owned()
                } else {
                    e.to_string()
                })
            })?;
            let status = r.status().as_u16();
            let headers = r.headers().iter().filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_owned(), v.to_owned()))).collect();
            let body = r.bytes().await.map_err(|e| WireError::unavailable(e.to_string()))?.to_vec();
            Ok(NetResponse { status, headers, body })
        })
    }

    fn stream(&self, req: NetRequest) -> BoxFuture<'static, Result<NetStream>> {
        use futures::StreamExt as _;
        let client = self.client.clone();
        Box::pin(async move {
            let method =
                reqwest::Method::from_bytes(req.method.to_ascii_uppercase().as_bytes()).map_err(|e| WireError::invalid(e.to_string()))?;
            let timeout = Duration::from_millis(if req.timeout_ms == 0 { 30_000 } else { req.timeout_ms });
            let mut b = client.request(method, &req.url).timeout(timeout).body(req.body);
            for (k, v) in &req.headers {
                b = b.header(k, v);
            }
            let r = b.send().await.map_err(|e| WireError::unavailable(format!("{}: {e}", req.url)))?;
            let status = r.status().as_u16();
            let headers = r.headers().iter().filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_owned(), v.to_owned()))).collect();
            let body = r.bytes_stream().map(|c| c.map(|b| b.to_vec()).map_err(|e| WireError::unavailable(e.to_string()))).boxed();
            Ok(NetStream { status, headers, body })
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
    }
    fn sleep(&self, ms: u64) -> BoxFuture<'static, ()> {
        Box::pin(tokio::time::sleep(Duration::from_millis(ms)))
    }
}

/// Every service this install provides, on `app`.
pub fn provide(app: App, home: &Path, version: &str, notify: NotifyService) -> App {
    let secrets: SecretsService = Arc::new(FileSecrets::new(home));
    let net: NetService = Arc::new(ReqwestNet::new(version));
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let app = app.service(Paths::new(home)).service(secrets).service(net).service(clock).service(notify);
    #[cfg(feature = "proc")]
    let app = app.service::<wisp_proc::Proc>(wisp_proc::local());
    app
}

#[cfg(test)]
#[path = "services_tests.rs"]
mod tests;

pub fn hex(b: &[u8]) -> String {
    b.iter().fold(String::with_capacity(b.len() * 2), |mut s, x| {
        use std::fmt::Write;
        let _ = write!(s, "{x:02x}");
        s
    })
}
