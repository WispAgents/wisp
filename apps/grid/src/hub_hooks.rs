//! A team hub's webhook trigger URLs (ADR-0148 with ADR-0147's tokens): `https://<team>.<domain>/_hook/wgh_…` starts one
//! of the hub's webhook triggers from the internet (GitHub, Sentry, Linear…). As a computer's: the token is the only way
//! in, said once, kept as its SHA-256 and last four characters, at most [`crate::hooks::MAX_HOOKS`] live per hub, and
//! revoked at once. A hub's box sleeps but wakes for a call, so nothing is held: the call waits for it.
//!
//! An owner or admin makes one (the team's page, `wisp team url`), or the hub's own wisp does for its Automations page
//! (its box secret, which the grid takes for its own hub only): a hub's URL starts work on the hub, never on a laptop.
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use crate::error::{Result, bad};
use crate::hooks::{MAX_HOOKS, PREFIX};
use crate::hubs::Hub;
use crate::teams::Teams;

const USED_EVERY_MS: i64 = 60_000;

/// A live hub URL, checked: its hub and the trigger it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubHook {
    pub id: String,
    pub team_id: String,
    pub trigger: String,
}

impl Teams {
    /// A new URL for `trigger` on `hub` by `by` (a handle, or the hub itself): its view and the token, said once.
    pub fn hook_create(&self, hub: &Hub, by: &str, trigger: &str) -> Result<(Value, String)> {
        let token = crate::accounts::token(PREFIX);
        let id = crate::secrets::hex(&rand::random::<[u8; 6]>());
        let hint = token[token.len().saturating_sub(4)..].to_owned();
        let now = self.clock.now_ms();
        self.db.tx(|tx| {
            let live: i64 = tx.query_row(
                "SELECT count(*) FROM team_hooks WHERE team_id = ?1 AND revoked_at IS NULL",
                [&hub.team_id],
                |r| r.get(0),
            )?;
            if live >= MAX_HOOKS {
                return Err(bad(format!("{} has {MAX_HOOKS} trigger URLs, the most a hub keeps. Revoke one first.", hub.slug), 409));
            }
            tx.execute(
                "INSERT INTO team_hooks (id, team_id, trigger, token_hash, hint, created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, hub.team_id, trigger, crate::secrets::sha256(&token), hint, by, now],
            )?;
            crate::rows::find::audit(tx, None, "hubhook.create", Some(&json!({ "team": hub.slug, "id": id })), now)?;
            Ok(())
        })?;
        let view = json!({ "id": id, "trigger": trigger, "computer": hub.slug, "hint": hint, "createdAt": now, "lastUsedAt": null });
        Ok((view, token))
    }

    /// A hub's live URLs, newest first, as a computer's list says them (`computer` is the hub's team).
    pub fn hooks_of(&self, hub: &Hub) -> Result<Vec<Value>> {
        self.db.with(|c| {
            c.prepare_cached(
                "SELECT id, trigger, hint, created_at, last_used_at FROM team_hooks WHERE team_id = ?1 AND revoked_at IS NULL
                 ORDER BY created_at DESC, id",
            )?
            .query_map([&hub.team_id], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?, "trigger": r.get::<_, String>(1)?, "computer": hub.slug, "hint": r.get::<_, String>(2)?,
                    "createdAt": r.get::<_, i64>(3)?, "lastUsedAt": r.get::<_, Option<i64>>(4)?,
                }))
            })?
            .collect()
        })
    }

    /// Revokes one of `hub`'s URLs: false when it isn't a live one of this hub's.
    pub fn hook_revoke(&self, hub: &Hub, id: &str) -> Result<bool> {
        let now = self.clock.now_ms();
        let n = self.db.with(|c| {
            c.execute(
                "UPDATE team_hooks SET revoked_at = ?1 WHERE id = ?2 AND team_id = ?3 AND revoked_at IS NULL",
                params![now, id, hub.team_id],
            )
        })?;
        if n > 0 {
            self.db.with(|c| crate::rows::find::audit(c, None, "hubhook.revoke", Some(&json!({ "team": hub.slug, "id": id })), now))?;
        }
        Ok(n > 0)
    }

    /// The live URL a token is, for `hub`'s address: None for anything else (unknown, revoked, another hub's).
    pub fn hook_check(&self, hub: &Hub, token: &str) -> Result<Option<HubHook>> {
        if !crate::hooks::is_token(token) {
            return Ok(None);
        }
        let row: Option<(String, String, Option<i64>)> = self.db.with(|c| {
            c.prepare_cached(
                "SELECT id, trigger, last_used_at FROM team_hooks WHERE token_hash = ?1 AND team_id = ?2 AND revoked_at IS NULL",
            )?
            .query_row(params![crate::secrets::sha256(token), hub.team_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
        })?;
        let Some((id, trigger, used)) = row else { return Ok(None) };
        let now = self.clock.now_ms();
        // `last_used_at` at most once a minute (a busy sender's every call isn't a write of it); the count each time
        let stamp = used.is_none_or(|t| now - t > USED_EVERY_MS);
        self.db.with(|c| {
            c.execute(
                "UPDATE team_hooks SET calls = calls + 1, last_used_at = CASE WHEN ?2 THEN ?3 ELSE last_used_at END WHERE id = ?1",
                params![id, stamp, now],
            )
        })?;
        Ok(Some(HubHook { id, team_id: hub.team_id.clone(), trigger }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::accounts::Accounts;
    use crate::clock::ManualClock;
    use crate::db::Db;

    #[test]
    fn a_hubs_url_is_said_once_opens_its_own_hubs_trigger_and_nothing_once_revoked() {
        let db = Arc::new(Db::memory().unwrap());
        let clock = ManualClock::new(1_000);
        let a = Accounts { db: db.clone(), clock: clock.clone(), telemetry: None };
        let t = Teams { db: db.clone(), clock };
        let ada = a.dev_user("ada").unwrap();
        t.create(&ada, "acme", "").unwrap();
        t.create(&ada, "zeta", "").unwrap();
        let (acme, zeta) = (t.hub("acme").unwrap().unwrap(), t.hub("zeta").unwrap().unwrap());
        let (view, token) = t.hook_create(&acme, "ada", "deploy").unwrap();
        assert!(token.starts_with("wgh_") && view["hint"] == json!(&token[token.len() - 4..]));
        let kept: String = db.with(|c| c.query_row("SELECT token_hash FROM team_hooks", [], |r| r.get(0))).unwrap();
        assert_ne!(kept, token, "only its hash is kept");
        assert_eq!(t.hook_check(&acme, &token).unwrap().unwrap().trigger, "deploy");
        // a second call within the minute: counted, its last use kept (a sender calls often)
        assert!(t.hook_check(&acme, &token).unwrap().is_some());
        let calls: i64 = db.with(|c| c.query_row("SELECT calls FROM team_hooks", [], |r| r.get(0))).unwrap();
        assert_eq!(calls, 2);
        assert!(t.hook_check(&zeta, &token).unwrap().is_none(), "another hub's address opens nothing");
        assert!(t.hook_check(&acme, "deploy").unwrap().is_none(), "a trigger's name is never a token");
        assert_eq!(t.hooks_of(&acme).unwrap().len(), 1);
        assert!(!t.hook_revoke(&zeta, view["id"].as_str().unwrap()).unwrap(), "only its own hub revokes it");
        assert!(t.hook_revoke(&acme, view["id"].as_str().unwrap()).unwrap());
        assert!(t.hook_check(&acme, &token).unwrap().is_none() && t.hooks_of(&acme).unwrap().is_empty());
    }
}
