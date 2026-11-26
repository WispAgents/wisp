//! The grid's one HTTP server (v1 grid/server.js, the parts ported so far): every address of `hosts`, on
//! one port.
//!
//! - `<domain>`: the device API (RFC 8628 sign-in, computers and their tunnels), the shares API and their
//!   pages (`/s/<id>`), the gallery, teams, sign-in with GitHub, Google or OIDC (`edge_signin.rs`), and the dev
//!   sign-in when `auth.dev` is on.
//! - `<handle>.<domain>`: that user's wisp in its box, started by hostd on a real request; a passive one
//!   (`x-wisp-passive: 1`, or v1's `x-wisp-relay: 1`) to a stopped box answers 503 asleep and wakes
//!   nothing. `/_grid/state` says how the box is without waking it, `/_grid/stop` puts it to sleep.
//! - `<name>--<handle>.<domain>` and `p<port>--<name>--<handle>`: a computer, and a page on it, through
//!   the tunnel it holds open (a box's credential never reaches one: `trust`).
//!
//! Who's asking is a device token (`Authorization: Bearer wgd_…`) or the grid's session cookie; each check
//! names its reach in `trust`, so adding a route means choosing who reaches it.
use std::collections::HashSet;
use std::sync::Arc;

use http::HeaderMap;
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use serde_json::{Value, json};

use crate::accounts::Accounts;
use crate::clock::SharedClock;
use crate::config::GridConfig;
use crate::db::Db;
use crate::error::{GridError, Result};
use crate::hostd::ProcessHost;
use crate::hosts::{HostKind, HostLookups, PreviewOf, host_kind};
use crate::http::{Body, Resp, cookies, header_str, json, parse_body, read_bytes};
use crate::pages::PageCfg;
use crate::rows::{ComputerRow, SandboxRow, SiteRow, UserRow};
use crate::shares::Shares;
use crate::teams::Teams;
use crate::trust::{Credential, Verdict};
use crate::tunnels::Tunnels;

#[path = "edge_answer.rs"]
mod answer;
#[path = "edge_build.rs"]
mod build;
#[path = "edge_common.rs"]
mod common;
#[path = "edge_device.rs"]
mod device;
#[path = "edge_events.rs"]
mod events;
#[path = "edge_forms.rs"]
mod forms;
#[path = "edge_handoff.rs"]
mod handoff;
#[path = "edge_inbox.rs"]
mod inbox;
#[path = "edge_keys.rs"]
mod keys;
#[path = "edge_mail.rs"]
mod mail;
#[path = "edge_pages.rs"]
mod pages_route;
#[path = "edge_preview.rs"]
mod preview;
#[path = "edge_push.rs"]
mod push_route;
#[path = "edge_reach.rs"]
mod reach;
#[path = "edge_route.rs"]
mod route;
#[path = "edge_sandboxes.rs"]
mod sandboxes;
#[path = "edge_serve.rs"]
mod serve;
#[path = "edge_shares.rs"]
mod shares_route;
#[path = "edge_signin.rs"]
mod signin;
#[path = "edge_sites.rs"]
mod sites;
#[path = "edge_waitlist.rs"]
mod waitlist;
pub use serve::{bytes_body, dev, serve};
pub use signin::{Reqwest, SignIn};

/// The grid's session cookie.
pub const COOKIE: &str = "wisp_grid";
/// A request that mustn't count as use of a box (a watching peer's sync stream).
pub const PASSIVE: &str = "x-wisp-passive";

pub struct Grid {
    pub cfg: GridConfig,
    pub db: Arc<Db>,
    pub clock: SharedClock,
    pub accounts: Accounts,
    /// API keys (`wgk_…`): a script's way in to its user's own wisp (`keys.rs`)
    pub keys: crate::keys::ApiKeys,
    /// per-user and per-address rate limits (a minute's window)
    limits: crate::http::Limiter,
    /// share passwords' scrypt, a few at a time on the blocking pool (`edge_shares.rs` `off_the_workers`); public so
    /// a test can hold them all
    pub scrypt: Arc<tokio::sync::Semaphore>,
    /// a key just made, held for the one Account page that says it (`keys_fresh.rs`)
    fresh_keys: crate::keys_fresh::FreshKeys,
    pub shares: Shares,
    pub teams: Teams,
    pub tunnels: Arc<Tunnels>,
    pub boxes: Arc<ProcessHost>,
    roots: HashSet<String>,
    page: PageCfg,
    client: Client<HttpConnector, Body>,
    /// sign-in with GitHub, Google or an OIDC issuer
    pub signin: SignIn,
    pub sandboxes: crate::sandboxes::Sandboxes,
    /// apps published from My Stuff (`sites.rs`)
    pub sites: crate::sites::Sites,
    /// channels' messages held for sleeping boxes (`inbox.rs`)
    pub inbox: crate::inbox::Inbox,
    /// `on laptop` through the grid's own bot (`route.rs`)
    pub routes: crate::route::Routes,
    /// mail to a box's secret address (`mail/service.rs`), when grid.json has `mail`
    pub mail: Option<Arc<crate::mail::service::MailService>>,
    /// the phone relay: a handle → FCM/APNs token, ciphertext through (`push.rs`, ADR-0065 §3)
    pub push: crate::push::Relay,
    /// whose pages changed, for their open `/events` streams (`edge_events.rs`)
    notes: tokio::sync::broadcast::Sender<String>,
}

impl std::fmt::Debug for Grid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grid").field("domain", &self.cfg.domain).finish_non_exhaustive()
    }
}

impl HostLookups for Grid {
    fn domain(&self) -> &str {
        &self.cfg.domain
    }
    fn root_hosts(&self) -> &HashSet<String> {
        &self.roots
    }
    fn previews(&self) -> bool {
        self.cfg.previews()
    }
    fn site(&self, label: &str) -> Option<SiteRow> {
        self.sites.by_label(label).ok().flatten()
    }
    fn sandbox(&self, label: &str) -> Option<SandboxRow> {
        self.sandboxes.by_label(label).ok().flatten()
    }
    fn computer(&self, label: &str) -> Option<ComputerRow> {
        self.accounts.computer_by_label(label).ok().flatten()
    }
    fn handle(&self, label: &str) -> bool {
        self.accounts.user_by_handle(label).ok().flatten().is_some()
    }
}

/// How the edge let a request into a box, when the box's wisp must know (`x-wisp-proxy-via`, set only here).
#[derive(Debug, Clone, Copy)]
struct Via(&'static str);

/// Who's asking.
#[derive(Debug, Clone)]
enum Who {
    Nobody,
    /// a device token (wisp on a computer)
    Device(UserRow, crate::rows::DeviceRow),
    /// the grid's own cookie (a browser)
    Session(UserRow),
    /// `Box <handle> <secret>`: a box asking for itself (its user, the secret checked against the running box's)
    Box(UserRow),
}

impl Who {
    fn user(&self) -> Option<&UserRow> {
        match self {
            Self::Device(u, _) | Self::Session(u) | Self::Box(u) => Some(u),
            Self::Nobody => None,
        }
    }
    fn credential(&self) -> Option<Credential> {
        match self {
            Self::Device(..) => Some(Credential::Device),
            Self::Session(_) => Some(Credential::Session),
            Self::Box(_) => Some(Credential::Box),
            Self::Nobody => None,
        }
    }
}

fn err(e: &GridError) -> Resp {
    json(e.status(), &json!({ "error": e.to_string() }))
}

fn no(v: &Verdict) -> Option<Resp> {
    match v {
        Verdict::Ok => None,
        Verdict::No(why) => Some(json(403, &json!({ "error": why }))),
    }
}

/// `/a/b/c?x` → (`["a","b","c"]`, `"x"`).
fn split(path_q: &str) -> (Vec<&str>, &str) {
    let (p, q) = path_q.split_once('?').unwrap_or((path_q, ""));
    (p.split('/').filter(|s| !s.is_empty()).collect(), q)
}

fn query(q: &str, k: &str) -> Option<String> {
    q.split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(a, _)| *a == k)
        .map(|(_, v)| url::form_urlencoded::parse(format!("x={v}").as_bytes()).next().map(|(_, v)| v.into_owned()).unwrap_or_default())
}

fn incoming(b: Incoming) -> Body {
    b.map_err(std::io::Error::other).boxed()
}

impl Grid {
    fn who(&self, h: &HeaderMap) -> Who {
        let auth = header_str(h, "authorization");
        if let Some(rest) = auth.strip_prefix("Box ") {
            // the running box's own secret, never a stopped one's: a box asks only for itself
            let (handle, secret) = rest.trim().split_once(' ').unwrap_or((rest.trim(), ""));
            return match self.accounts.user_by_handle(handle).ok().flatten() {
                Some(u) if !secret.is_empty() && self.boxes.secret(&u.id).is_some_and(|s| crate::secrets::safe_equal(&s, secret)) => {
                    Who::Box(u)
                }
                _ => Who::Nobody,
            };
        }
        if let Ok(Some((u, d))) = self.accounts.device(auth) {
            return Who::Device(u, d);
        }
        if let Some(c) = cookies(h).get(COOKIE)
            && let Ok(Some(u)) = self.accounts.session(c)
        {
            return Who::Session(u);
        }
        Who::Nobody
    }

    /// A write to the grid's API with its session cookie must come from the grid's own origin. The cookie is
    /// SameSite=Lax, and a published app (`<slug>--app--<handle>`) is same-site, so its page could otherwise post a
    /// form here (`parse_body` reads urlencoded too) and change the visitor's sandboxes, teams or keys. A bearer
    /// (device, key, Box) can't be sent by a browser on its own, and the pages' forms have their csrf token; the
    /// waitlist reads no sign-in at all, and trywisp.dev posts to it.
    fn cross_site_api(&self, method: &http::Method, parts: &[&str], h: &HeaderMap, who: &Who) -> Option<Resp> {
        let safe = matches!(*method, http::Method::GET | http::Method::HEAD | http::Method::OPTIONS);
        if safe || !matches!(who, Who::Session(_)) || parts.first() != Some(&"api") || parts == ["api", "waitlist"] {
            return None;
        }
        (header_str(h, "origin") != self.cfg.public_url).then(|| json(403, &json!({ "error": "forbidden origin" })))
    }

    fn csrf(cookie: &str) -> String {
        crate::accounts::sha256_hex(&format!("csrf:{cookie}")).chars().take(24).collect()
    }

    fn instance(&self, u: &UserRow) -> Value {
        json!({ "url": self.cfg.instance_url(&u.handle), "name": "Cloud", "handle": u.handle })
    }

    /// One request, whatever its address.
    pub async fn handle(self: &Arc<Self>, req: http::Request<Incoming>) -> Resp {
        let host = req.headers().get("host").and_then(|h| h.to_str().ok()).unwrap_or_default().to_owned();
        let kind = host_kind(&host, self.as_ref());
        let asker = self.asker(req.method(), req.headers());
        let cors = common::api_cors(&kind, req.uri().path());
        if cors && req.method() == http::Method::OPTIONS {
            return common::api_preflight();
        }
        if let Some(r) = self.common(&req, &kind) {
            return r;
        }
        let out = match kind {
            HostKind::Root => self.root(req).await,
            HostKind::Edge { handle } => match req.uri().path().strip_prefix("/_in/").map(str::to_owned) {
                Some(tok) => self.inbox_in(req, &handle, &tok).await,
                None => self.edge(req, &handle).await,
            },
            HostKind::Computer(c) => self.computer_reach(req, &c, None).await,
            HostKind::Preview(p) => match p.of {
                PreviewOf::Computer(c) => self.computer_reach(req, &c, Some(p.port)).await,
                PreviewOf::Handle(h) => self.box_preview(req, p.port, &h).await,
                PreviewOf::Sandbox(_) => Ok(json(404, &json!({ "error": "Pages in sandboxes aren't shown on this grid." }))),
            },
            HostKind::Sandbox(sb) => self.sandbox_reach(req, &sb).await,
            HostKind::Site(site) => Ok(self.site_reach(&req, site.as_ref())),
            HostKind::Unknown => Ok(json(421, &json!({ "error": "This grid doesn't serve that address." }))),
        };
        let out = out.unwrap_or_else(|e| self.failed(asker, &e));
        if cors { common::with_api_cors(out) } else { out }
    }

    // --- <domain> -------------------------------------------------------------------------------------

    async fn root(self: &Arc<Self>, req: http::Request<Incoming>) -> Result<Resp> {
        let method = req.method().clone();
        let path_q = req.uri().path_and_query().map(|p| p.as_str().to_owned()).unwrap_or_default();
        let (parts, q) = split(&path_q);
        let who = self.who(req.headers());
        if let Some(r) = self.cross_site_api(&method, &parts, req.headers(), &who) {
            return Ok(r);
        }
        if parts.first() == Some(&"api") && parts.get(1) == Some(&"computers") && parts.len() == 4 && parts[3] == "tunnel" {
            return self.tunnel(req, &who, parts[2]);
        }
        // an app's bundle may be bigger than any other body: read up to the sites' own cap
        if parts.first() == Some(&"api") && parts.get(1) == Some(&"sites") && parts.len() <= 3 {
            let key = parts.get(2).map(|k| (*k).to_owned());
            let q = q.to_owned();
            return self.sites_route(req, &who, key.as_deref(), &q).await;
        }
        let (head, body) = req.into_parts();
        let bytes = read_bytes(body, crate::shares::MAX_BYTES * 2).await?;
        let peer = head.extensions.get::<common::Peer>().map(|p| p.0);
        // the signed-in pages and their forms (edge_pages.rs)
        if let Some(r) = self.pages_route(method.as_str(), parts.as_slice(), q, &head.headers, &bytes, &who).await {
            return r;
        }
        // signing in: the device API, `/activate`, the providers' and the dev sign-in (edge_device.rs)
        if let Some(r) = self.account_routes(&method, &parts, q, &head.headers, peer, &bytes, &who).await {
            return r;
        }
        match (method.as_str(), parts.as_slice()) {
            ("POST" | "OPTIONS", ["api", "waitlist"]) => Ok(self.waitlist_in(method.as_str(), &head.headers, peer, &bytes)),
            (_, ["api", "shares", ..] | ["api", "gallery"] | ["s", ..]) | ("GET", ["gallery"]) => {
                self.shares_route(&method, &parts, q, &head.headers, peer, &bytes, &who).await
            }
            (_, ["api", "keys"]) => self.keys_route(&method, None, &head.headers, &bytes, &who),
            (_, ["api", "keys", id]) => self.keys_route(&method, Some(id), &head.headers, &bytes, &who),
            (_, ["api", "sandboxes", ..]) => self.sandboxes_route(&method, &parts, &head.headers, &bytes, &who).await,
            (_, ["api", "teams", ..]) => self.teams_route(&method, &parts, &head.headers, &bytes, &who),
            (_, ["api", "channels", which @ ("url" | "rotate" | "key")]) => {
                self.channels_route(&method, which, q, &head.headers, &bytes, &who)
            }
            (_, ["api", "channels", which @ ("route" | "reply")]) => self.route_api(&method, which, &head.headers, &bytes, &who).await,
            (_, ["api", "push", which @ ("register" | "send")]) => self.push_route(&method, which, &head.headers, &bytes, &who).await,
            ("POST", ["_route", tok]) => self.route_in(&head.headers, tok, &bytes),
            ("POST", ["_mail", provider]) => self.mail_in(provider, &bytes).await,
            ("POST", ["api", "channels", "mail", "reply"]) => {
                let b = parse_body(&head.headers, &bytes)?;
                self.mail_reply(&Value::Object(b), &who).await
            }
            _ => {
                let asker = self.asker(&method, &head.headers);
                Ok(if asker.page {
                    self.not_found_for(asker, "Nothing here", "This page doesn’t exist, or its link has a typo.")
                } else {
                    json(404, &json!({ "error": "not found" }))
                })
            }
        }
    }

    // --- shares, the gallery, teams -------------------------------------------------------------------

    // --- tunnels and computers ------------------------------------------------------------------------

    // --- <handle>.<domain>: a user's box -----------------------------------------------------------
}

#[cfg(test)]
#[path = "edge_tests.rs"]
mod tests;
