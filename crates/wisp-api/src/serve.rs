//! Listening (edge cases 8 and 10): HTTP/1.1 with upgrades (previews' websockets) and h2c, keep-alive
//! idle and header reads bounded by `keep_alive` (65 s: longer than the 60 s idle of the proxies in
//! front, so they never reuse a socket wisp is closing). `server.json` is written once listening, so
//! integrations (CLI, MCP, editors) find wisp without knowing the port, and removed on exit only if its
//! pid is ours (a second wisp that failed to bind must not delete the first one's).
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use hyper_util::server::conn::auto::Builder;
use hyper_util::server::graceful::GracefulShutdown;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::edge::{API_VERSION, Edge, now_ms};

/// How long open connections get to finish once wisp stops.
const DRAIN: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub struct Server {
    pub addr: SocketAddr,
    edge: Arc<Edge>,
    task: JoinHandle<()>,
    advertised: Option<PathBuf>,
}

/// The URL this computer reaches a server bound at `addr` on: any IPv4 address (`0.0.0.0`) or a loopback one as
/// `127.0.0.1`, any IPv6 address (`::`) or its loopback as `[::1]`, and an address of its own as itself. It was always
/// `127.0.0.1`, so `wisp serve --host ::1` advertised a URL nothing answered on.
pub fn advertised_url(addr: SocketAddr) -> String {
    let port = addr.port();
    match addr.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() || ip.is_loopback() => format!("http://127.0.0.1:{port}"),
        IpAddr::V6(ip) if ip.is_unspecified() || ip.is_loopback() => format!("http://[::1]:{port}"),
        IpAddr::V4(ip) => format!("http://{ip}:{port}"),
        IpAddr::V6(ip) => format!("http://[{ip}]:{port}"),
    }
}

impl Server {
    pub fn url(&self) -> String {
        advertised_url(self.addr)
    }

    pub fn edge(&self) -> &Arc<Edge> {
        &self.edge
    }

    /// Stops accepting, ends the streams, lets requests in flight finish (at most 2 s), removes server.json.
    pub async fn shutdown(self) {
        self.edge.shutdown();
        let _ = self.task.await;
        if let Some(f) = &self.advertised {
            remove_if_ours(f).await;
        }
    }
}

/// Serves `edge` on `listener` until `edge.shutdown()`.
pub async fn serve(edge: Arc<Edge>, listener: TcpListener) -> std::io::Result<Server> {
    let addr = listener.local_addr()?;
    edge.port.store(addr.port(), Ordering::Relaxed);
    let server_json = match &edge.cfg.home {
        Some(home) => {
            tokio::fs::create_dir_all(home).await?;
            let id = edge.ident.get_or_init(|| Ident::new(&edge.cfg.master, home));
            Some(write_server_json(home, addr, &edge.cfg.version, id).await?)
        }
        None => None,
    };
    if let Some(s) = edge.statics.clone() {
        tokio::spawn(async move { s.warm().await });
    }
    let task = tokio::spawn(accept_loop(edge.clone(), listener));
    Ok(Server { addr, edge, task, advertised: server_json })
}

async fn accept_loop(edge: Arc<Edge>, listener: TcpListener) {
    let mut builder = Builder::new(TokioExecutor::new());
    builder.http1().timer(TokioTimer::new()).keep_alive(true).header_read_timeout(edge.cfg.keep_alive);
    let graceful = GracefulShutdown::new();
    let stop = edge.stop.clone();
    loop {
        let (stream, _) = tokio::select! {
            () = stop.cancelled() => break,
            r = listener.accept() => match r {
                Ok(s) => s,
                Err(e) => {
                    // EMFILE and friends: back off a little instead of spinning
                    tracing::warn!("accept: {e}");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
        };
        let _ = stream.set_nodelay(true);
        let e = edge.clone();
        let svc = service_fn(move |req| {
            let e = e.clone();
            async move { Ok::<_, std::convert::Infallible>(e.handle(req).await) }
        });
        let conn = builder.serve_connection_with_upgrades(TokioIo::new(stream), svc).into_owned();
        let conn = graceful.watch(conn);
        tokio::spawn(async move {
            let _ = conn.await;
        });
    }
    drop(listener);
    let _ = tokio::time::timeout(DRAIN, graceful.shutdown()).await;
}

/// Who a server is, in `server.json` and `/api/health`: an id new at each boot, and the hash of the canonical home it
/// serves. A CLI on another home (another `WISP_HOME` on the same default port) compares the hash with its own and
/// refuses that wisp, instead of running its commands on someone else's tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub instance: String,
    pub home_hash: String,
}

impl Ident {
    /// The instance id is derived from this boot's master token (random): new each boot, and it tells nothing of it.
    pub fn new(master: &str, home: &Path) -> Self {
        let id = blake3::derive_key("wisp server instance id", master.as_bytes());
        Self { instance: crate::auth::hex(&id[..12]), home_hash: home_hash(home) }
    }
}

/// A stable hash of `home` as the file system resolves it (links, `/var` and `/private/var`, case on a Mac), never the
/// path itself (`/api/health` is open). A home that isn't there yet hashes as spelled. The server and the CLI both
/// hash through this one function, so they agree on every OS.
pub fn home_hash(home: &Path) -> String {
    // canonicalize: only hashed and compared with another hash from here, never handed to git or a shell
    let canon = std::fs::canonicalize(home).unwrap_or_else(|_| home.to_owned());
    let digest = <sha2::Sha256 as sha2::Digest>::digest(canon.as_os_str().as_encoded_bytes());
    crate::auth::hex(&digest[..16])
}

async fn write_server_json(home: &Path, addr: SocketAddr, version: &str, id: &Ident) -> std::io::Result<PathBuf> {
    let file = home.join("server.json");
    let port = addr.port();
    let body = json!({"url": advertised_url(addr), "port": port, "pid": std::process::id(), "version": version, "apiVersion": API_VERSION, "startedAt": now_ms(), "instance": id.instance, "homeHash": id.home_hash});
    let tmp = home.join(format!("server.json.{}.tmp", std::process::id()));
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(&body).unwrap_or_default()).await?;
    tokio::fs::rename(&tmp, &file).await?;
    Ok(file)
}

/// Removes server.json only if it still names this process.
pub async fn remove_if_ours(file: &Path) {
    let Ok(text) = tokio::fs::read(file).await else {
        return;
    };
    let pid = serde_json::from_slice::<serde_json::Value>(&text).ok().and_then(|v| v.get("pid").and_then(serde_json::Value::as_u64));
    if pid == Some(u64::from(std::process::id())) {
        let _ = tokio::fs::remove_file(file).await;
    }
}

#[cfg(test)]
mod tests {
    use super::advertised_url;

    /// cli-r6 M4: the URL server.json, `Server::url` and the agents' access say is the one this computer reaches.
    #[test]
    fn the_advertised_url_is_the_bound_address_this_computer_reaches() {
        let at = |a: &str| advertised_url(a.parse().unwrap_or_else(|e| panic!("{a}: {e}")));
        assert_eq!(at("0.0.0.0:4477"), "http://127.0.0.1:4477");
        assert_eq!(at("127.0.0.1:4477"), "http://127.0.0.1:4477");
        assert_eq!(at("127.0.0.2:4477"), "http://127.0.0.1:4477");
        assert_eq!(at("[::]:4477"), "http://[::1]:4477");
        assert_eq!(at("[::1]:4477"), "http://[::1]:4477");
        assert_eq!(at("192.168.1.5:4477"), "http://192.168.1.5:4477");
        assert_eq!(at("[fe80::1]:80"), "http://[fe80::1]:80");
    }
}
