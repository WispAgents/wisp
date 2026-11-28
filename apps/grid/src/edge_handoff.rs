//! Into your own addresses from the grid's pages (v1 grid/server.js `/go`, `boxEdge`, `computerEdge`; `handoff.rs`):
//!
//! - `GET <domain>/go[?computer=<id>|?sandbox=<id>|?hub=<team>][&path=]`: signed in at the grid, a one-time ticket to that
//!   address, `302 <address>/_grid/auth?t=<ticket>&r=<path>`.
//! - `GET <address>/_grid/auth?t=&r=`: the ticket, once, becomes that address's own cookie, then on to `r` (a path
//!   on that address only). A spent, expired or someone else's ticket gets no cookie: the browser goes back to `/go`
//!   for a fresh one, which needs the grid's own sign-in.
//! - `GET <address>/_grid/signout`: that address's cookie ends; back to the grid.
//! - Signed out at an address, a page opened in a browser goes to `/go` for a ticket (and comes back to where it
//!   was); anything else hears 401.
use http::{HeaderMap, Method};
use hyper::body::Incoming;
use serde_json::json;

use super::{Grid, query};
use crate::error::{Result, bad};
use crate::handoff::COOKIE;
use crate::http::{CookieOptions, Resp, add_cookie, cookie, cookies, header_str, json, redirect, safe_path, wants_html};
use crate::rows::UserRow;

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// What an address answers on its own, before anyone is let through.
pub(super) enum Own {
    /// `/_grid/auth` or `/_grid/signout`: answered.
    Done(Resp),
    /// anything else: on to the address's sign-in check.
    Next(http::Request<Incoming>),
}

impl Grid {
    /// `/go`: a ticket to your cloud wisp, or `?computer=<id>` / `?sandbox=<id>`, one of your computers or sandboxes.
    pub(super) fn go(&self, u: &UserRow, q: &str) -> Result<Resp> {
        let label = if let Some(id) = query(q, "computer") {
            self.accounts.computer(&id)?.filter(|c| c.user_id == u.id).ok_or_else(|| bad("No such computer.", 404))?.label
        } else if let Some(id) = query(q, "sandbox") {
            self.sandboxes.get(&id)?.filter(|s| s.user_id == u.id).ok_or_else(|| bad("No such sandbox.", 404))?.label
        } else if let Some(slug) = query(q, "hub") {
            // a team's hub (ADR-0147): anyone in the team gets a ticket to it, as themselves
            match self.teams.hub(&slug)? {
                Some(h) if self.teams.role(&u.id, &slug)?.is_some() => h.slug,
                _ => return Err(bad("No such hub, or you aren't in its team.", 404)),
            }
        } else {
            u.handle.clone()
        };
        let t = self.accounts.address_ticket(&u.id, &label)?;
        let path = query(q, "path").unwrap_or_default();
        let to = format!("{}/_grid/auth?t={}&r={}", self.cfg.instance_url(&label), enc(&t), enc(safe_path(&path, "/")));
        Ok(redirect(&to, &[]))
    }

    /// Where an address sends a browser for a ticket: `/go` for it, back to `path`.
    pub(super) fn go_back(&self, go: &str, path: &str) -> String {
        let sep = if go.contains('?') { '&' } else { '?' };
        format!("{}{go}{sep}path={}", self.cfg.public_url, enc(path))
    }

    /// `/_grid/auth` and `/_grid/signout` at `label`'s address, owned by `owner`; `go`: its `/go` link. `offline`: a
    /// computer that isn't connected, so the sign-in lands on its waiting page (`/_grid/offline`), never on what a
    /// phone's service worker would answer from its cache (QA round 10 Grid #2).
    pub(super) fn address_own(&self, req: http::Request<Incoming>, label: &str, owner: &UserRow, go: &str, offline: bool) -> Own {
        let back = format!("{}{go}", self.cfg.public_url);
        match req.uri().path() {
            "/_grid/auth" => {
                Own::Done(self.address_auth(req.uri().query().unwrap_or_default(), req.headers(), label, owner, &back, offline))
            }
            "/_grid/signout" => {
                let c = cookies(req.headers()).get(COOKIE).cloned().unwrap_or_default();
                if let Err(e) = self.accounts.end_address_session(&c) {
                    tracing::warn!("grid: signing out of {label}: {e}");
                }
                let mut r = redirect(&self.cfg.public_url, &[]);
                add_cookie(&mut r, &cookie(COOKIE, "", self.address_cookie(Some(0))));
                Own::Done(r)
            }
            _ => Own::Next(req),
        }
    }

    fn address_cookie(&self, max_age: Option<i64>) -> CookieOptions<'static> {
        CookieOptions { secure: self.cfg.secure, max_age, path: None, framed: false }
    }

    /// `/_grid/auth?t=&r=`: the ticket becomes this address's cookie.
    fn address_auth(&self, q: &str, h: &HeaderMap, label: &str, owner: &UserRow, back: &str, offline: bool) -> Resp {
        let t = query(q, "t").unwrap_or_default();
        let spent = self.accounts.use_address_ticket(&t, label);
        if !matches!(&spent, Ok(Some(uid)) if self.holds_address(owner, uid)) {
            // spent, expired, for another address or another person: no cookie, a fresh ticket from the grid
            return redirect(back, &[("cache-control", "no-store".into())]);
        }
        // the person's own session at a hub's address; the owner's at anyone else's (they're the same there)
        let who = spent.ok().flatten().unwrap_or_default();
        let session = match self.accounts.new_address_session(&who, label, header_str(h, "user-agent")) {
            Ok(s) => s,
            Err(e) => return super::err(&e),
        };
        let next = query(q, "r").unwrap_or_default();
        let next = safe_path(&next, "/");
        let to = if offline { super::answer::waiting_path(next) } else { next.to_owned() };
        let mut r = redirect(&to, &[("cache-control", "no-store".into())]);
        add_cookie(&mut r, &cookie(COOKIE, &session, self.address_cookie(Some(crate::handoff::SESSION_MS / 1000))));
        r
    }

    /// Who this address's own cookie signs in (its owner only).
    pub(super) fn address_user(&self, h: &HeaderMap, label: &str, owner: &UserRow) -> Option<UserRow> {
        let c = cookies(h).get(COOKIE).cloned()?;
        self.accounts.address_session(&c, label).ok().flatten().filter(|u| self.holds_address(owner, &u.id))
    }

    /// Whether `uid` may be signed in at `owner`'s address: its owner, or anyone in the team whose hub it is (ADR-0147).
    fn holds_address(&self, owner: &UserRow, uid: &str) -> bool {
        if uid == owner.id {
            return true;
        }
        matches!(self.teams.hub_by_user(&owner.id), Ok(Some(h)) if matches!(self.teams.role(uid, &h.slug), Ok(Some(_))))
    }

    /// Nobody signed in at an address: a browser goes for a ticket, a fetch hears 401.
    pub(super) fn sign_in_at(&self, method: &Method, h: &HeaderMap, here: &str, go: &str) -> Resp {
        if wants_html(method, h) {
            return redirect(&self.go_back(go, here), &[("cache-control", "no-store".into())]);
        }
        json(401, &json!({ "error": "Sign in again." }))
    }

    /// A post to an address with its cookie must come from that address: a page elsewhere can't use the cookie.
    pub(super) fn foreign_origin(&self, method: &Method, h: &HeaderMap, label: &str) -> bool {
        !matches!(*method, Method::GET | Method::HEAD) && header_str(h, "origin") != self.cfg.instance_url(label)
    }
}
