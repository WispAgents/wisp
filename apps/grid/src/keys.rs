//! API keys (v1 grid/accounts.js "API keys", ADR-0058): a script's way in to its user's own wisp, `Authorization:
//! Bearer wgk_…` at `https://<handle>.<domain>/api/…`, and nowhere else (`trust`: a key reaches `box` only).
//!
//! Apart from devices on purpose: `Accounts::device` never takes a key, so a key opens only what the edge lets
//! it, and none of the grid's own API. A key is said once, in the answer that made it; the grid keeps its
//! SHA-256 (base64url, as v1 did, so a live grid.db's keys keep working) and its last four characters (`hint`,
//! so a person can tell keys apart). Neither the key nor its hash is in any answer after that, nor in `audit`.
use std::sync::Arc;

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::json;

use crate::clock::SharedClock;
use crate::db::Db;
use crate::error::{Result, bad};
use crate::rows::UserRow;

/// Live keys a user may hold at once (v1 `MAX_API_KEYS`).
pub const MAX_KEYS: i64 = 20;
/// A key's name, in characters (v1 counted UTF-16 units).
pub const NAME_MAX: usize = 60;
/// Creates a user may ask for in a minute (v1 `limit('apikey|<user>', 30)`).
pub const CREATES_PER_MINUTE: usize = 30;
pub const PREFIX: &str = "wgk_";
/// `last_used_at` is written at most once a minute per key (a script's every call isn't a write).
const USED_EVERY_MS: i64 = 60_000;

/// A key as every answer shows it: never the key, never its hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyView {
    pub id: String,
    pub name: String,
    pub hint: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ApiKeys {
    pub db: Arc<Db>,
    pub clock: SharedClock,
}

/// The key a `Bearer wgk_…` header (any case of `Bearer`) carries; the bare key too, as `Accounts::device` takes.
pub fn bearer_key(auth: &str) -> Option<&str> {
    let a = auth.trim();
    let t = match a.split_once(char::is_whitespace) {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("bearer") => rest.trim(),
        Some(_) => return None,
        None => a,
    };
    t.starts_with(PREFIX).then_some(t)
}

/// A key's id as the grid makes them (12 hex): anything else names no key, without asking the db.
pub fn is_key_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A name as given: trimmed, 1–60 characters, no control characters.
pub fn key_name(name: Option<&serde_json::Value>) -> Result<String> {
    let n = name.and_then(serde_json::Value::as_str).map(str::trim).unwrap_or_default();
    if n.is_empty() {
        return Err(bad("Give the key a name.", 400));
    }
    if n.chars().count() > NAME_MAX {
        return Err(bad("Keep a key’s name to 60 characters.", 400));
    }
    if n.chars().any(char::is_control) {
        return Err(bad("A key’s name is one line of text.", 400));
    }
    Ok(n.to_owned())
}

fn hex_id() -> String {
    crate::secrets::hex(&rand::random::<[u8; 6]>())
}

impl ApiKeys {
    fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    /// A new key for `user_id`: its view and the key itself, which is never seen again. 409 past [`MAX_KEYS`] live.
    pub fn create(&self, user_id: &str, name: &str) -> Result<(KeyView, String)> {
        let key = crate::accounts::token(PREFIX);
        let view = KeyView {
            id: hex_id(),
            name: name.to_owned(),
            hint: key[key.len().saturating_sub(4)..].to_owned(),
            created_at: self.now(),
            last_used_at: None,
        };
        self.db.tx(|tx| {
            // counted and made in one transaction: two at once can't both take the 20th place
            let live: i64 =
                tx.query_row("SELECT COUNT(*) FROM api_keys WHERE user_id = ?1 AND revoked_at IS NULL", [user_id], |r| r.get(0))?;
            if live >= MAX_KEYS {
                return Err(bad(format!("You have {MAX_KEYS} API keys, the most a grid keeps. Revoke one first."), 409));
            }
            tx.execute(
                "INSERT INTO api_keys (id, user_id, name, token_hash, hint, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![view.id, user_id, view.name, crate::secrets::sha256(&key), view.hint, view.created_at],
            )?;
            // its id, not its name: a name may say more than the audit needs
            crate::rows::find::audit(tx, Some(user_id), "apikey.create", Some(&json!({ "id": view.id })), view.created_at)?;
            Ok(())
        })?;
        Ok((view, key))
    }

    /// Who a live key belongs to, and the key's id; None when it's unknown, revoked, or its user is disabled.
    pub fn check(&self, auth: &str) -> Result<Option<(UserRow, String)>> {
        let Some(key) = bearer_key(auth) else { return Ok(None) };
        let row: Option<(String, String, Option<i64>, Option<i64>)> = self.db.with(|c| {
            c.query_row(
                "SELECT id, user_id, last_used_at, revoked_at FROM api_keys WHERE token_hash = ?1",
                [crate::secrets::sha256(key)],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
        })?;
        let Some((id, user_id, used, None)) = row else { return Ok(None) };
        let Some(user) = self.db.with(|c| crate::rows::find::user_by_id(c, &user_id))? else { return Ok(None) };
        if user.disabled != 0 {
            return Ok(None);
        }
        let now = self.now();
        if used.is_none_or(|t| now - t > USED_EVERY_MS) {
            self.db.with(|c| c.execute("UPDATE api_keys SET last_used_at = ?1 WHERE id = ?2", params![now, id]))?;
        }
        Ok(Some((user, id)))
    }

    /// A user's live keys, newest first.
    pub fn list(&self, user_id: &str) -> Result<Vec<KeyView>> {
        self.db.with(|c| {
            let mut s = c.prepare_cached(
                "SELECT id, name, hint, created_at, last_used_at FROM api_keys
                 WHERE user_id = ?1 AND revoked_at IS NULL ORDER BY created_at DESC, rowid DESC",
            )?;
            let rows = s.query_map([user_id], |r| {
                Ok(KeyView { id: r.get(0)?, name: r.get(1)?, hint: r.get(2)?, created_at: r.get(3)?, last_used_at: r.get(4)? })
            })?;
            rows.collect()
        })
    }

    /// Revokes one of `user_id`'s live keys: false when there's no such key of theirs (or it's revoked already).
    pub fn revoke(&self, user_id: &str, id: &str) -> Result<bool> {
        if !is_key_id(id) {
            return Ok(false);
        }
        let now = self.now();
        self.db.tx(|tx| {
            let n = tx.execute(
                "UPDATE api_keys SET revoked_at = ?1 WHERE id = ?2 AND user_id = ?3 AND revoked_at IS NULL",
                params![now, id, user_id],
            )?;
            if n > 0 {
                crate::rows::find::audit(tx, Some(user_id), "apikey.revoke", Some(&json!({ "id": id })), now)?;
            }
            Ok(n > 0)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::accounts::Accounts;
    use crate::clock::ManualClock;

    fn keys() -> (ApiKeys, Accounts, Arc<ManualClock>) {
        let clock = ManualClock::new(1_000_000);
        let db = Arc::new(Db::memory().unwrap());
        (ApiKeys { db: db.clone(), clock: clock.clone() }, Accounts { db, clock: clock.clone() }, clock)
    }

    #[test]
    fn a_bearer_names_a_key_only_with_its_prefix() {
        assert_eq!(bearer_key("Bearer wgk_abc"), Some("wgk_abc"));
        assert_eq!(bearer_key("bearer  wgk_abc "), Some("wgk_abc"));
        assert_eq!(bearer_key("wgk_abc"), Some("wgk_abc"));
        for no in ["", "Bearer", "Bearer wgd_abc", "Basic wgk_abc", "Box ada wgk_abc"] {
            assert_eq!(bearer_key(no), None, "{no}");
        }
    }

    #[test]
    fn names_are_one_trimmed_line_of_at_most_60_characters() {
        let n = |v: serde_json::Value| key_name(Some(&v));
        assert_eq!(n(json!("  Deploy script  ")).unwrap(), "Deploy script");
        // 60 characters of 2 UTF-16 units each still fit (v1 counted units: 30 emoji filled it)
        assert!(n(json!("🔑".repeat(60))).is_ok());
        for bad in [json!(""), json!("   "), json!("x".repeat(61)), json!(7), json!(null), json!("a\nb")] {
            assert_eq!(n(bad.clone()).map_err(|e| e.status()), Err(400), "{bad}");
        }
        assert!(key_name(None).is_err());
    }

    #[test]
    fn a_key_is_kept_as_its_hash_and_last_four_and_said_once() {
        let (k, a, clock) = keys();
        let ada = a.dev_user("ada").unwrap();
        let (v, key) = k.create(&ada.id, "ci").unwrap();
        assert!(key.starts_with("wgk_") && key.len() == 47, "{key}");
        assert_eq!(v.hint, key[key.len() - 4..]);
        let row: (String, String) =
            k.db.with(|c| c.query_row("SELECT token_hash, hint FROM api_keys", [], |r| Ok((r.get(0)?, r.get(1)?)))).unwrap();
        assert_eq!(row.0, crate::secrets::sha256(&key));
        let audit: Vec<String> =
            k.db.with(|c| c.prepare("SELECT action || ' ' || meta FROM audit")?.query_map([], |r| r.get(0))?.collect()).unwrap();
        assert_eq!(audit, vec![format!("apikey.create {{\"id\":\"{}\"}}", v.id)]);
        assert!(!audit.concat().contains(&key[4..]) && !audit.concat().contains("ci"));
        // used: last_used_at set, then held for a minute
        assert_eq!(k.check(&format!("Bearer {key}")).unwrap().map(|(u, id)| (u.handle, id)), Some(("ada".into(), v.id.clone())));
        assert_eq!(k.list(&ada.id).unwrap()[0].last_used_at, Some(1_000_000));
        clock.advance(30_000);
        k.check(&key).unwrap();
        assert_eq!(k.list(&ada.id).unwrap()[0].last_used_at, Some(1_000_000));
        clock.advance(31_000);
        k.check(&key).unwrap();
        assert_eq!(k.list(&ada.id).unwrap()[0].last_used_at, Some(1_061_000));
        // a typo, the key without its last character: nobody
        assert!(k.check(&format!("Bearer {}", &key[..key.len() - 1])).unwrap().is_none());
    }

    #[test]
    fn twenty_live_keys_at_most_and_a_revoke_frees_a_place() {
        let (k, a, _) = keys();
        let ada = a.dev_user("ada").unwrap();
        let first = k.create(&ada.id, "k0").unwrap().0;
        for i in 1..MAX_KEYS {
            k.create(&ada.id, &format!("k{i}")).unwrap();
        }
        assert_eq!(k.create(&ada.id, "one too many").map_err(|e| e.status()).err(), Some(409));
        assert!(k.revoke(&ada.id, &first.id).unwrap());
        assert!(!k.revoke(&ada.id, &first.id).unwrap(), "once");
        assert!(k.create(&ada.id, "room again").is_ok());
        assert_eq!(k.list(&ada.id).unwrap().len(), 20);
        assert_eq!(k.list(&ada.id).unwrap()[0].name, "room again", "newest first");
    }

    #[test]
    fn a_revoked_key_or_a_disabled_user_opens_nothing_and_another_user_cant_revoke() {
        let (k, a, _) = keys();
        let ada = a.dev_user("ada").unwrap();
        let bo = a.dev_user("bo").unwrap();
        let (v, key) = k.create(&ada.id, "ci").unwrap();
        assert!(!k.revoke(&bo.id, &v.id).unwrap(), "not bo's");
        assert!(!k.revoke(&ada.id, "../x").unwrap());
        assert!(k.check(&key).unwrap().is_some());
        k.db.with(|c| c.execute("UPDATE users SET disabled = 1 WHERE id = ?1", [&ada.id])).unwrap();
        assert!(k.check(&key).unwrap().is_none(), "disabled");
        k.db.with(|c| c.execute("UPDATE users SET disabled = 0 WHERE id = ?1", [&ada.id])).unwrap();
        assert!(k.revoke(&ada.id, &v.id).unwrap());
        assert!(k.check(&key).unwrap().is_none(), "revoked");
        assert_eq!(k.list(&ada.id).unwrap().len(), 0);
    }
}
