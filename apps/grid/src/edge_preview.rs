//! A page in a box at its own address (ADR-0064, v1 grid/server.js `previewLink`, `previewAuth`, `previewEdge`):
//!
//! - `GET <handle>.<domain>/_grid/preview?port=&path=`: the box's owner, signed in there, is sent to that page's
//!   preview address with a one-time ticket (`previews.rs`). A key or a box's own secret never gets one.
//! - `p<port>--<handle>.<domain>/_grid/auth?t=&r=`: the ticket becomes the address's own sign-in (host-only,
//!   framed: wisp shows the page in a frame), then on to `r`.
//! - anything else there: with that sign-in, to the box's wisp (its proxy secret, `x-wisp-preview: <port>`), which
//!   passes it to the port on its loopback (`wisp-api` preview.rs); websockets (live reload) too. Without it, a
//!   page opened in a browser goes to the box's address to get one; anything else is 401.
//!
//! Never an open proxy: one box, its owner alone, a port in `previews::PORTS` (never the grid's own), a box that's
//! running (a preview never wakes one: whatever ran there stopped with it), and a post from elsewhere is refused.
use http::HeaderMap;
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use serde_json::json;

use super::{Grid, incoming, no, query};
use crate::boxhost::BoxHost as _;
use crate::error::{Result, bad};
use crate::http::{CookieOptions, Resp, add_cookie, cookie, cookies, header_str, json, redirect, safe_path, wants_html};
use crate::previews::{COOKIE, SESSION_MS, allowed_port, ticket_scope};
use crate::rows::UserRow;
use crate::trust::{Credential, Reach, may};

const HOP: [&str; 9] =
    ["connection", "keep-alive", "proxy-connection", "transfer-encoding", "upgrade", "te", "trailer", "host", "http2-settings"];

fn port_of(q: &str) -> Option<u32> {
    query(q, "port").filter(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit())).and_then(|p| p.parse().ok())
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// The request's cookies, minus the preview's own: the page keeps its cookies, never the grid's sign-in.
fn page_cookies(h: &HeaderMap) -> Option<String> {
    let raw = header_str(h, "cookie");
    let rest: Vec<&str> = raw.split(';').map(str::trim).filter(|c| !c.is_empty() && !c.starts_with(&format!("{COOKIE}="))).collect();
    (!rest.is_empty()).then(|| rest.join("; "))
}

impl Grid {
    /// The preview address of `label`'s page on `port`.
    pub fn preview_origin(&self, label: &str, port: impl std::fmt::Display) -> String {
        self.cfg.instance_url(&format!("p{port}--{label}"))
    }

    /// `GET <handle>/_grid/preview?port=&path=` by its owner (`cred` already let into the box): on to the page with
    /// a ticket.
    pub(super) fn preview_link(&self, q: &str, user: &UserRow, cred: Credential) -> Result<Resp> {
        if let Some(r) = no(&may(cred, Reach::Preview)).filter(|_| cred == Credential::Key) {
            return Ok(r);
        }
        let Some(port) = port_of(q).filter(|p| self.cfg.previews() && allowed_port(*p, self.cfg.listen.port)) else {
            return Ok(json(400, &json!({ "error": "No preview for that port here." })));
        };
        let t = self.accounts.ticket(&user.id, &ticket_scope(&user.handle))?;
        let path = query(q, "path").unwrap_or_default();
        let to = format!("{}/_grid/auth?t={}&r={}", self.preview_origin(&user.handle, port), enc(&t), enc(safe_path(&path, "/")));
        Ok(redirect(&to, &[("cache-control", "no-store".into())]))
    }

    /// `p<port>--<handle>`: the page on `port` in that user's box, to them alone.
    pub(super) async fn box_preview(&self, mut req: http::Request<Incoming>, port: u16, label: &str) -> Result<Resp> {
        let Some(user) = self.accounts.user_by_handle(label)?.filter(|u| u.disabled == 0) else {
            return Ok(json(404, &json!({ "error": "Nothing answers at this address." })));
        };
        if !allowed_port(u32::from(port), self.cfg.listen.port) {
            return Ok(json(404, &json!({ "error": "No preview for that port here." })));
        }
        if req.uri().path() == "/_grid/auth" {
            return self.preview_sign_in(&req, &user, label);
        }
        if let Some(r) = self.preview_refused(&req, &user, label, port)? {
            return Ok(r);
        }
        // its server runs in the box: asleep, there's nothing to show (waking it wouldn't start it)
        let Some(box_port) = self.boxes.port(&user.id) else {
            let why = format!("Port {port} lives in your wisp, which is asleep. Open your wisp and ask the agent to start it again.");
            return Ok(crate::http::json_with(503, &json!({ "error": why, "asleep": true }), &[("retry-after", "10".into())]));
        };
        let busy = std::sync::Mutex::new(self.boxes.begin(&user.id, true));
        let upgrade = !header_str(req.headers(), "upgrade").is_empty();
        let down = upgrade.then(|| hyper::upgrade::on(&mut req));
        let (mut head, body) = req.into_parts();
        head.headers = self.preview_headers(&head.headers, &user.id, label, port, box_port);
        let pq = head.uri.path_and_query().map_or("/", |p| p.as_str()).to_owned();
        head.uri = format!("http://127.0.0.1:{box_port}{pq}").parse().map_err(|_| bad("Bad path.", 400))?;
        let mut up = self
            .client
            .request(http::Request::from_parts(head, incoming(body)))
            .await
            .map_err(|e| bad(format!("Your wisp didn't answer: {e}"), 502))?;
        host_only_cookies(up.headers_mut());
        if let (Some(down), http::StatusCode::SWITCHING_PROTOCOLS) = (down, up.status()) {
            let theirs = hyper::upgrade::on(&mut up);
            tokio::spawn(async move {
                let _busy = busy;
                if let (Ok(a), Ok(b)) = (down.await, theirs.await) {
                    let _ = tokio::io::copy_bidirectional(&mut TokioIo::new(a), &mut TokioIo::new(b)).await;
                }
            });
            let (parts, _) = up.into_parts();
            return Ok(http::Response::from_parts(parts, crate::http::empty()));
        }
        let (parts, b) = up.into_parts();
        // the request counts as use until its answer has been read
        let b = b.map_err(std::io::Error::other).map_frame(move |f| {
            let _ = &busy;
            f
        });
        Ok(http::Response::from_parts(parts, b.boxed()))
    }
}

impl Grid {
    /// `/_grid/auth?t=&r=` at a preview address: the ticket, once, becomes that address's own sign-in.
    fn preview_sign_in(&self, req: &http::Request<Incoming>, user: &UserRow, label: &str) -> Result<Resp> {
        let q = req.uri().query().unwrap_or_default();
        let ticket = query(q, "t").unwrap_or_default();
        if self.accounts.use_ticket(&ticket, &ticket_scope(label))?.is_none_or(|u| u != user.id) {
            return Ok(json(401, &json!({ "error": "That link has expired. Open the page from wisp again." })));
        }
        let session = self.accounts.new_preview_session(&user.id, label, header_str(req.headers(), "user-agent"))?;
        let next = query(q, "r").unwrap_or_default();
        let mut out = redirect(safe_path(&next, "/"), &[("cache-control", "no-store".into())]);
        let o = CookieOptions { secure: true, max_age: Some(SESSION_MS / 1000), path: None, framed: true };
        add_cookie(&mut out, &cookie(COOKIE, &session, o));
        Ok(out)
    }

    /// Why a request at a preview address goes no further: not its owner's preview sign-in (a page opened in a
    /// browser is sent for one), or a post from elsewhere.
    fn preview_refused(&self, req: &http::Request<Incoming>, user: &UserRow, label: &str, port: u16) -> Result<Option<Resp>> {
        let h = req.headers();
        let signed = cookies(h).get(COOKIE).map(|c| self.accounts.preview_session(c, label)).transpose()?.flatten();
        if signed.is_none_or(|u| u.id != user.id) {
            if wants_html(req.method(), h) {
                let here = req.uri().path_and_query().map_or("/", |p| p.as_str());
                let back = format!("{}/_grid/preview?port={port}&path={}", self.cfg.instance_url(label), enc(here));
                return Ok(Some(redirect(&back, &[("cache-control", "no-store".into())])));
            }
            return Err(bad("Sign in again.", 401));
        }
        // a page elsewhere can't post to it with the cookie
        let origin = header_str(h, "origin");
        let safe = matches!(*req.method(), http::Method::GET | http::Method::HEAD);
        if !safe && !origin.is_empty() && origin != self.preview_origin(label, port) {
            return Err(bad("forbidden origin", 403));
        }
        Ok(None)
    }

    /// What the box's wisp gets: the page's own headers and cookies, never the grid's sign-in; its proxy secret and
    /// the port; who asked where.
    fn preview_headers(&self, h: &HeaderMap, user_id: &str, label: &str, port: u16, box_port: u16) -> HeaderMap {
        let mut out = HeaderMap::with_capacity(h.len() + 6);
        for (k, v) in h {
            let n = k.as_str();
            if !(HOP.contains(&n) || n == "cookie" || n.starts_with("x-wisp-") || n.starts_with("x-forwarded-")) {
                out.append(k.clone(), v.clone());
            }
        }
        let origin = self.preview_origin(label, port);
        let mut set = |k: &'static str, v: &str| {
            if let Ok(v) = v.parse() {
                out.insert(k, v);
            }
        };
        if let Some(c) = page_cookies(h) {
            set("cookie", &c);
        }
        set("x-wisp-proxy-secret", &self.boxes.secret(user_id).unwrap_or_default());
        set("x-wisp-preview", &port.to_string());
        set("x-forwarded-host", origin.split("://").nth(1).unwrap_or_default());
        set("x-forwarded-proto", &self.cfg.scheme);
        set("host", &format!("127.0.0.1:{box_port}"));
        if let Some(u) = h.get("upgrade").and_then(|u| u.to_str().ok()) {
            set("connection", "Upgrade");
            set("upgrade", u);
        }
        out
    }
}

/// A cookie the page sets stays on its own address: one with `Domain=` would reach the grid's other names.
fn host_only_cookies(h: &mut HeaderMap) {
    let all: Vec<_> = h.get_all("set-cookie").iter().cloned().collect();
    if all.iter().all(|v| v.to_str().is_ok_and(crate::tunnels::host_only)) {
        return;
    }
    h.remove("set-cookie");
    for v in all.into_iter().filter(|v| v.to_str().is_ok_and(crate::tunnels::host_only)) {
        h.append("set-cookie", v);
    }
}
