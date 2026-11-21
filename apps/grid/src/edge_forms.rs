//! The signed-in pages' forms for what you put out (docs/ux/2026-11-18-grid.md #5, #6): API keys made and revoked on
//! Account, a published app unpublished and a shared task stopped from Home. The same path as every page form
//! (`pages_route::form`): the session's own POST with its CSRF token, answered with a redirect back and a flash line,
//! but for a new key, which the page itself says once.

use serde_json::Value;

use super::Grid;
use crate::error::Result;
use crate::http::{CookieOptions, Resp, add_cookie, cookie, redirect};

/// The cookie that names a key just made, for Account to say once (never the key itself).
pub(super) const NEW_KEY: &str = "wisp_new_key";
use crate::pages::ShareView;
use crate::rows::UserRow;

impl Grid {
    /// `/keys`, `/keys/:id/revoke`, `/sites/:key/remove`, `/shares/:id/revoke`; None for any other form.
    pub(super) fn link_form(&self, u: &UserRow, parts: &[&str], b: &serde_json::Map<String, Value>) -> Option<Result<Resp>> {
        Some(match parts {
            ["sites", key, "remove"] => self.unpublish(u, key),
            ["keys"] => self.make_key(u, b.get("name")),
            ["keys", id, "revoke"] => self.revoke_key(u, id),
            ["shares", id, "revoke"] => self
                .shares
                .revoke(u, id)
                .map(|_| self.back("/#shares", "Stopped sharing: that link no longer opens. Sharing it again brings it back.", false)),
            _ => return None,
        })
    }

    fn unpublish(&self, u: &UserRow, key: &str) -> Result<Resp> {
        let gone = self.sites.remove(&u.id, key)?;
        Ok(self.back("/", if gone { "Unpublished: that link no longer works." } else { "That app was already unpublished." }, !gone))
    }

    fn revoke_key(&self, u: &UserRow, id: &str) -> Result<Resp> {
        let gone = self.keys.revoke(&u.id, id)?;
        if gone {
            tracing::info!("grid: {} revoked API key {id}", u.handle);
        }
        Ok(self.back(
            "/account#keys",
            if gone { "Revoked: scripts using it are refused from now on." } else { "That key was already revoked." },
            !gone,
        ))
    }

    /// `POST /keys` from Account (#6): the session's own form (CSRF checked by `form`), so the browser makes keys while
    /// the box sleeps; a 303 to Account, which says the key once (`fresh_keys`).
    fn make_key(&self, user: &UserRow, name: Option<&Value>) -> Result<Resp> {
        let u = user;
        self.limits.hit(&format!("apikey|{}", u.id), crate::keys::CREATES_PER_MINUTE, self.clock.now_ms())?;
        let name = match crate::keys::key_name(name) {
            Ok(n) => n,
            Err(e) => return Ok(self.back("/account#keys", &e.to_string(), true)),
        };
        match self.keys.create(&u.id, &name) {
            Ok((view, key)) => {
                tracing::info!("grid: {} made API key {}", u.handle, view.id);
                // post, redirect, get (QA round 6 #3): a reload of Account never posts again; the key waits for that
                // one page under a token, and the cookie carries only the token
                let token = self.fresh_keys.put(&u.id, &view.name, &key, self.clock.now_ms());
                let mut to_account = redirect("/account#keys", &[]);
                *to_account.status_mut() = http::StatusCode::SEE_OTHER;
                let held =
                    CookieOptions { secure: self.cfg.secure, max_age: Some(crate::keys_fresh::FRESH_MS / 1000), path: None, framed: false };
                add_cookie(&mut to_account, &cookie(NEW_KEY, &token, held));
                Ok(to_account)
            }
            Err(e) if e.status() < 500 => Ok(self.back("/account#keys", &e.to_string(), true)),
            Err(e) => Err(e),
        }
    }

    /// Home's shared tasks (#5): the sharer's own list, minus the ones stopped.
    pub(super) fn shares_of(&self, uid: &str) -> Result<Vec<ShareView>> {
        let s = |v: &Value, k: &str| v[k].as_str().unwrap_or_default().to_owned();
        Ok(self
            .shares
            .mine(uid)?
            .iter()
            .filter(|v| !v["revoked"].as_bool().unwrap_or(false))
            .map(|v| ShareView {
                id: s(v, "id"),
                title: s(v, "title"),
                kind: s(v, "kind"),
                url: s(v, "url"),
                access: v["access"]["kind"].as_str().unwrap_or_default().to_owned(),
                views: v["views"].as_i64().unwrap_or(0),
                updated_at: v["updatedAt"].as_i64(),
                expires_at: v["expiresAt"].as_i64(),
            })
            .collect())
    }
}
