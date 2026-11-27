//! What every address answers the same way, before routing (v1 grid/server.js `common`): the grid's images,
//! robots.txt, and Caddy's on-demand TLS check.
use http::Method;
use serde_json::json;

use super::Grid;
use crate::hosts::{HostKind, TLS_ASK, tls_allowed};
use crate::http::{Resp, full, is_loopback, json, respond};

/// The peer a request came from (set by `serve`).
#[derive(Debug, Clone, Copy)]
pub(super) struct Peer(pub std::net::IpAddr);

/// v1's `DEVICE_CORS` (grid/server.js): the phone app's webview is another origin (`https://localhost`) and calls the
/// grid's `/api/` (device sign-in, `/api/me`, computers) and a box's or computer's `/api/` with its device token. Any
/// origin, never credentials: with `*` a browser sends no cookie, so a signed-in page elsewhere still can't read them.
const API_CORS: [(&str, &str); 4] = [
    ("access-control-allow-origin", "*"),
    (
        "access-control-allow-headers",
        "authorization, content-type, x-wisp-passkey, x-wisp-client, x-wisp-plugin, idempotency-key, last-event-id",
    ),
    ("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS"),
    ("access-control-max-age", "600"),
];

/// Whether a request gets [`API_CORS`]: `/api/` (and the phone's `/_grid/media`) on the grid itself, a user's box or a
/// computer (v1 had it on all three).
pub(super) fn api_cors(kind: &HostKind, path: &str) -> bool {
    (path.starts_with("/api/") || path == super::media_route::MEDIA)
        && matches!(kind, HostKind::Root | HostKind::Edge { .. } | HostKind::Computer(_))
}

/// The preflight for [`api_cors`] requests: answered here, never routed (a stopped box isn't woken for it).
pub(super) fn api_preflight() -> Resp {
    with_api_cors(respond(204, "", crate::http::empty(), &[]))
}

/// Puts [`API_CORS`] on a response, replacing any the box or computer sent (two `allow-origin` values are refused).
pub(super) fn with_api_cors(mut r: Resp) -> Resp {
    let h = r.headers_mut();
    for (k, v) in API_CORS {
        h.insert(k, http::HeaderValue::from_static(v));
    }
    r
}

const ROBOTS_ROOT: &str = "User-agent: *\nDisallow: /api/\nDisallow: /auth/\nDisallow: /activate\nDisallow: /go\n";

/// Whether this is Caddy's own TLS check: from loopback, on the grid's Host or a loopback one, and never a request
/// Caddy passed on. Every proxied request also comes from loopback, so the peer alone tells nothing; Caddy's proxy
/// always adds X-Forwarded-For and its ask never does (that would tell anyone which names exist).
fn caddy_asks<B>(req: &http::Request<B>, root: bool) -> bool {
    let peer = req.extensions().get::<Peer>().map(|p| p.0.to_string()).unwrap_or_default();
    if !is_loopback(&peer) || req.headers().contains_key("x-forwarded-for") {
        return false;
    }
    let host = req.headers().get("host").and_then(|h| h.to_str().ok()).unwrap_or_default().to_ascii_lowercase();
    let bare = host.rsplit_once(':').filter(|(_, p)| p.bytes().all(|b| b.is_ascii_digit())).map_or(host.as_str(), |(h, _)| h);
    root || matches!(bare, "127.0.0.1" | "localhost" | "[::1]")
}

impl Grid {
    /// The answer every address gives alike, or None to route the request.
    pub(super) fn common<B>(&self, req: &http::Request<B>, kind: &HostKind) -> Option<Resp> {
        let path = req.uri().path();
        let root = matches!(kind, HostKind::Root);
        // Caddy asks before it gets a certificate for a name (deploy/Caddyfile): only the grid itself and names that
        // exist. Caddy's ask is `http://127.0.0.1:<port>/_grid/tls-ask`, so its Host is the loopback address, not the
        // grid's: answering only on the grid's own Host refused every new name with a 421 (no certificate for any
        // account made after the v2 cutover, 2026-11-23).
        if path == TLS_ASK && caddy_asks(req, root) {
            let ask = super::query(req.uri().query().unwrap_or_default(), "domain").unwrap_or_default();
            let ok = tls_allowed(&self.cfg.domain, &ask, self);
            return Some(json(if ok { 200 } else { 404 }, &json!({ "ok": ok })));
        }
        if !matches!(*req.method(), Method::GET | Method::HEAD) {
            return None;
        }
        // the favicon on the grid's own address; a box's wisp has its own
        if (root || !path.starts_with("/favicon"))
            && let Some((bytes, ct)) = crate::assets::by_path(path)
        {
            return Some(respond(
                200,
                ct,
                full(bytes),
                &[("cache-control", "public, max-age=86400".into()), ("x-content-type-options", "nosniff".into())],
            ));
        }
        // People's wisps and computers are private: keep crawlers out of everything but the grid's own pages.
        if path == "/robots.txt" {
            let body = if root { ROBOTS_ROOT } else { "User-agent: *\nDisallow: /\n" };
            return Some(respond(200, "text/plain; charset=utf-8", full(body), &[("cache-control", "public, max-age=3600".into())]));
        }
        None
    }
}
