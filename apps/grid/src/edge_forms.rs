//! The signed-in pages' forms for what you put out (docs/ux/2026-11-18-grid.md #5, #6): API keys made and revoked on
//! Account, a published app unpublished and a shared task stopped from Home. The same path as every page form
//! (`pages_route::form`): the session's own POST with its CSRF token, answered with a redirect back and a flash line,
//! but for a new key, which the page itself says once.

use http::HeaderMap;
use serde_json::Value;

use super::Grid;
use crate::error::Result;
use crate::http::Resp;
use crate::pages::ShareView;
use crate::rows::UserRow;

impl Grid {
    /// `/keys`, `/keys/:id/revoke`, `/sites/:key/remove`, `/shares/:id/revoke`; None for any other form.
    #[allow(clippy::many_single_char_names)] // u the user, q the query, h the headers, b the body, as `form` names them
    pub(super) fn link_form(
        &self,
        u: &UserRow,
        parts: &[&str],
        q: &str,
        h: &HeaderMap,
        b: &serde_json::Map<String, Value>,
    ) -> Option<Result<Resp>> {
        Some(match parts {
            ["sites", key, "remove"] => self.unpublish(u, key),
            ["keys"] => self.make_key(u, h, q, b.get("name")),
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
    /// the box sleeps; the page answers with the key said once (200, `no-store`), never a redirect that could keep it.
    fn make_key(&self, u: &UserRow, h: &HeaderMap, q: &str, name: Option<&Value>) -> Result<Resp> {
        self.limits.hit(&format!("apikey|{}", u.id), crate::keys::CREATES_PER_MINUTE, self.clock.now_ms())?;
        let name = match crate::keys::key_name(name) {
            Ok(n) => n,
            Err(e) => return Ok(self.back("/account#keys", &e.to_string(), true)),
        };
        match self.keys.create(&u.id, &name) {
            Ok((view, key)) => {
                tracing::info!("grid: {} made API key {}", u.handle, view.id);
                self.account_with(u, h, q, Some((view.name, key)))
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
