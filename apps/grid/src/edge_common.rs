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

const ROBOTS_ROOT: &str = "User-agent: *\nDisallow: /api/\nDisallow: /auth/\nDisallow: /activate\nDisallow: /go\n";

impl Grid {
    /// The answer every address gives alike, or None to route the request.
    pub(super) fn common<B>(&self, req: &http::Request<B>, kind: &HostKind) -> Option<Resp> {
        let path = req.uri().path();
        let root = matches!(kind, HostKind::Root);
        // Caddy asks before it gets a certificate for a name (deploy/Caddyfile): only the grid itself and names that
        // exist. Loopback only: Caddy itself, never a request it passed on (that would tell anyone which exist).
        if root && path == TLS_ASK {
            let peer = req.extensions().get::<Peer>().map(|p| p.0.to_string()).unwrap_or_default();
            if is_loopback(&peer) {
                let ask = super::query(req.uri().query().unwrap_or_default(), "domain").unwrap_or_default();
                let ok = tls_allowed(&self.cfg.domain, &ask, self);
                return Some(json(if ok { 200 } else { 404 }, &json!({ "ok": ok })));
            }
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
