//! API keys on the edge (v1 grid/server.js `manageKeys`, ADR-0058):
//!
//! - `GET /api/keys` → `{keys: [{id, name, hint, createdAt, lastUsedAt}], address}` (newest first; `address`: the
//!   user's own wisp, where a key is sent);
//! - `POST /api/keys {name}` → 201 `{id, name, hint, createdAt, lastUsedAt, key}`: the only time the key is said;
//!   at most [`CREATES_PER_MINUTE`] a minute per user (429), and [`crate::keys::MAX_KEYS`] live (409);
//! - `DELETE /api/keys/<id>` → `{ok: true}`, or 404 `No such key.` (not theirs, unknown, or revoked already).
//!
//! Only a device reaches them (`grid:keys`). A key, a box's secret and the grid's own cookie are refused with the
//! trust table's word; nothing at all is 401. Every answer is `no-store` (`http::respond`). Nothing here logs a
//! body: the POST's answer holds the key.

use http::{HeaderMap, Method};
use serde_json::{Value, json};

use super::{Grid, Who, no};
use crate::error::{Result, bad};
use crate::http::{Resp, header_str, json, parse_body};
use crate::keys::{CREATES_PER_MINUTE, key_name};
use crate::trust::{Credential, Reach, may};

impl Grid {
    /// `/api/keys` and `/api/keys/<id>` (`id`).
    pub(super) fn keys_route(&self, method: &Method, id: Option<&str>, h: &HeaderMap, bytes: &[u8], who: &Who) -> Result<Resp> {
        // a key is nobody to `who` (it reaches no grid API); here it's named, so it hears why
        let cred = match who.credential() {
            Some(c) => c,
            None if self.keys.check(header_str(h, "authorization"))?.is_some() => Credential::Key,
            None => return Err(bad("Sign in first.", 401)),
        };
        if let Some(r) = no(&may(cred, Reach::GridKeys)) {
            return Ok(r);
        }
        let Who::Device(user, _) = who else { return Err(bad("Sign in with wisp on a computer first.", 401)) };
        match (method.as_str(), id) {
            ("GET", None) => {
                let keys = self.keys.list(&user.id)?;
                Ok(json(200, &json!({ "keys": keys, "address": self.cfg.instance_url(&user.handle) })))
            }
            ("POST", None) => {
                self.limits.hit(&format!("apikey|{}", user.id), CREATES_PER_MINUTE, self.clock.now_ms())?;
                let name = key_name(parse_body(h, bytes)?.get("name"))?;
                let (view, key) = self.keys.create(&user.id, &name)?;
                tracing::info!("grid: {} made API key {}", user.handle, view.id);
                let mut v = serde_json::to_value(&view)?;
                v["key"] = Value::String(key);
                Ok(json(201, &v))
            }
            ("DELETE", Some(id)) => {
                if self.keys.revoke(&user.id, id)? {
                    tracing::info!("grid: {} revoked API key {id}", user.handle);
                    Ok(json(200, &json!({ "ok": true })))
                } else {
                    Ok(json(404, &json!({ "error": "No such key." })))
                }
            }
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }
}
