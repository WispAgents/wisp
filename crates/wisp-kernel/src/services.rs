//! The small kernel services an install provides at boot (plan §6.1): paths, secrets, net and notify,
//! beside `Clock` (lib.rs), the store (wisp-store) and proc (wisp-proc). Each is a trait object so an
//! install swaps it (the grid, the simulation, a test). `crates/wisp` provides the real ones.
//!
//! Additive (wisp-api/edge agent, 2026-11-08): a plugin reads them with the helpers at the bottom,
//! `services::paths(&ctx)?`, so no plugin names the `Arc<dyn …>` wrapping.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use crate::{BoxFuture, Ctx, Result};

/// Where wisp keeps its files: `~/.wisp`, or `WISP_HOME`.
#[derive(Debug, Clone)]
pub struct Paths {
    home: PathBuf,
}

impl Paths {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }
    pub fn home(&self) -> &Path {
        &self.home
    }
    /// `<home>/<rel>` (v1 paths keep their names: `stuff`, `uploads`, `worktrees`, …)
    pub fn join(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.home.join(rel)
    }
    /// A plugin's own folder: `<home>/plugins/<id>`.
    pub fn plugin_dir(&self, plugin: &str) -> PathBuf {
        self.home.join("plugins").join(plugin)
    }
}

/// `~/.wisp/secrets.json` (0600), never the database. Keys are `ns/key`; a key without `/` is in `wisp`.
pub trait Secrets: Send + Sync + 'static {
    fn get(&self, key: &str) -> BoxFuture<'static, Result<Option<String>>>;
    fn set(&self, key: &str, value: &str) -> BoxFuture<'static, Result<()>>;
    fn delete(&self, key: &str) -> BoxFuture<'static, Result<()>>;
    /// every key starting with `prefix`, sorted
    fn keys(&self, prefix: &str) -> BoxFuture<'static, Result<Vec<String>>>;
}

/// One HTTP request out. Plugins never open their own client (plan §3.1 rule 8: one HTTP client).
#[derive(Debug, Clone, Default)]
pub struct NetRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// 0: the service's default (30 s)
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NetResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl NetResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.body).map_err(|e| crate::WireError::invalid(format!("response body: {e}")))
    }
}

/// A response whose body is read as it arrives (an event stream that stays open): status and headers at once, then
/// the body in chunks. Dropping it closes the connection. ADR-0134.
pub struct NetStream {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// the body's chunks as they come; an `unavailable` error ends it
    pub body: futures::stream::BoxStream<'static, Result<Vec<u8>>>,
}

impl NetStream {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

impl std::fmt::Debug for NetStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetStream").field("status", &self.status).field("headers", &self.headers).finish_non_exhaustive()
    }
}

pub trait Net: Send + Sync + 'static {
    /// Transport failures are `unavailable` (retry: true); any HTTP status is an `Ok` answer.
    fn fetch(&self, req: NetRequest) -> BoxFuture<'static, Result<NetResponse>>;
    /// The same request with its body streamed: an SSE stream read while other requests go out. `timeout_ms` bounds
    /// the whole stream. The default reads the whole body with `fetch` and gives it as one chunk, so a `Net` that
    /// can't stream (a test's fake) still answers.
    /// A request to an address nobody vouched for (an MCP Events webhook's callback, SEP-3415): never redirected, and
    /// sent only to a globally routable address ([`is_global`]), resolved at send time and connected to directly, so
    /// the name can't point elsewhere between the check and the connection (DNS rebinding). `allow_private` lets a
    /// loopback or private address through (a LAN, a test). The default sends it with `fetch`: a `Net` that can't
    /// pin an address (a test's fake) still answers.
    fn fetch_untrusted(&self, req: NetRequest, allow_private: bool) -> BoxFuture<'static, Result<NetResponse>> {
        let _ = allow_private;
        self.fetch(req)
    }
    fn stream(&self, req: NetRequest) -> BoxFuture<'static, Result<NetStream>> {
        let f = self.fetch(req);
        Box::pin(async move {
            let r = f.await?;
            let body = r.body;
            Ok(NetStream { status: r.status, headers: r.headers, body: Box::pin(futures::stream::once(async move { Ok(body) })) })
        })
    }
}

/// Whether `ip` is globally routable: not loopback, private, link-local, shared (CGNAT), documentation, benchmarking,
/// unspecified, multicast or broadcast, by the IANA special-purpose registries (an IPv4-mapped IPv6 address as its
/// IPv4). What an untrusted callback may be sent to ([`Net::fetch_untrusted`]).
pub fn is_global(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_documentation()
                || a == 0
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0 && c == 0)
                || (a == 198 && (b == 18 || b == 19))
                || a >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_global(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || first == 0x2001 && v6.segments()[1] == 0x0db8
                || first == 0x0064 && v6.segments()[1] == 0xff9b)
        }
    }
}

/// Ephemeral notices for every client: a `notice` frame for v2 clients, an event of that name for v1
/// clients (`toast`, `finished`, `reminder`, `summary`, …). Never stored, never replayed.
pub trait Notify: Send + Sync + 'static {
    /// `topic`: only subscribers of that topic get it (v2); v1 clients get every notice.
    fn notice(&self, kind: &str, data: Value, topic: Option<&str>);
}

pub type SecretsService = Arc<dyn Secrets>;
pub type NetService = Arc<dyn Net>;
pub type NotifyService = Arc<dyn Notify>;

pub fn paths(ctx: &Ctx) -> Result<Arc<Paths>> {
    ctx.service::<Paths>()
}
pub fn secrets(ctx: &Ctx) -> Result<SecretsService> {
    ctx.service::<SecretsService>().map(|s| (*s).clone())
}
pub fn net(ctx: &Ctx) -> Result<NetService> {
    ctx.service::<NetService>().map(|s| (*s).clone())
}
pub fn notify(ctx: &Ctx) -> Result<NotifyService> {
    ctx.service::<NotifyService>().map(|s| (*s).clone())
}

/// The address of whoever last reached this wisp through a trusted proxy (wisp Grid's edge sends it as
/// `x-wisp-proxy-client-ip`; a box has one user). The edge writes it; usage stats send it as `$ip`, so a box's
/// events are placed where its user is, not where the grid runs. Empty on a computer: nothing proxies to it.
#[derive(Debug, Default)]
pub struct ClientAddr(std::sync::Mutex<Option<String>>);

impl ClientAddr {
    pub fn set(&self, ip: &str) {
        let mut g = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if g.as_deref() != Some(ip) {
            *g = Some(ip.to_owned());
        }
    }
    pub fn get(&self) -> Option<String> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }
}

pub type ClientAddrService = Arc<ClientAddr>;

/// The proxied user's address, when this wisp has one (see [`ClientAddr`]).
pub fn client_addr(ctx: &Ctx) -> Option<String> {
    ctx.service::<ClientAddrService>().ok().and_then(|c| c.get())
}

#[cfg(test)]
mod untrusted_tests {
    use super::is_global;

    #[test]
    fn only_globally_routable_addresses_are_global() {
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:10.0.0.1",
            "192.0.2.1",
            "240.0.0.1",
        ] {
            assert!(!is_global(private.parse().unwrap_or_else(|_| panic!("{private}"))), "{private}");
        }
        for public in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111", "::ffff:8.8.8.8"] {
            assert!(is_global(public.parse().unwrap_or_else(|_| panic!("{public}"))), "{public}");
        }
    }
}
