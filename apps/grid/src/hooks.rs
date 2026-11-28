//! A computer's webhook trigger URLs (ADR-0147): `https://<computer>--<handle>.<domain>/_hook/<token>`, made by wisp on
//! that computer for one of its webhook triggers. The URL names the computer that runs the trigger; the token, 256
//! random bits, is the only way in (a trigger's name is never enough).
//!
//! Like an API key (`keys.rs`): the token is said once, in the answer that made it; the grid keeps its SHA-256 and its
//! last four characters (`hint`), never the token, and neither is in any later answer or in `audit`. A revoked token
//! answers the same 404 as one that never was; removing the computer removes its tokens (`ON DELETE CASCADE`).
use std::sync::Arc;

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::json;

use crate::clock::SharedClock;
use crate::db::Db;
use crate::error::{Result, bad};

/// Live URLs one computer may hold at once.
pub const MAX_HOOKS: i64 = 50;
/// Makes a user may ask for in a minute.
pub const CREATES_PER_MINUTE: usize = 30;
pub const PREFIX: &str = "wgh_";
/// A trigger's name, as the URL may name it: what wisp.automations takes as a resource name, without a slash.
pub const TRIGGER_MAX: usize = 128;
/// `last_used_at` and `calls` are written at most once a minute per URL (a busy sender's every call isn't a write).
const USED_EVERY_MS: i64 = 60_000;

/// A URL as every answer shows it: never the token, never its hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookView {
    pub id: String,
    pub trigger: String,
    pub computer: String,
    pub hint: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

/// What a token opens: that computer's trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hook {
    pub id: String,
    pub user_id: String,
    pub computer_id: String,
    pub trigger: String,
}

#[derive(Debug, Clone)]
pub struct Hooks {
    pub db: Arc<Db>,
    pub clock: SharedClock,
}

/// A trigger's name as given: trimmed, 1–128 bytes, no slash, no control or space characters.
pub fn trigger_name(name: Option<&serde_json::Value>) -> Result<String> {
    let n = name.and_then(serde_json::Value::as_str).map(str::trim).unwrap_or_default();
    if n.is_empty() {
        return Err(bad("Name the webhook trigger this URL starts.", 400));
    }
    if n.len() > TRIGGER_MAX || n.contains('/') || n.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(bad("That isn’t a trigger’s name.", 400));
    }
    Ok(n.to_owned())
}

/// A token as the grid makes them: anything else opens nothing, without asking the db.
pub fn is_token(t: &str) -> bool {
    t.len() == PREFIX.len() + 43 && t.starts_with(PREFIX) && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Hooks {
    fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    /// A new URL's token for `trigger` on `computer_id` (one of `user_id`'s): its view and the token, never seen again.
    /// 409 past [`MAX_HOOKS`] live on that computer.
    pub fn create(&self, user_id: &str, computer_id: &str, computer: &str, trigger: &str) -> Result<(HookView, String)> {
        let token = crate::accounts::token(PREFIX);
        let view = HookView {
            id: crate::secrets::hex(&rand::random::<[u8; 6]>()),
            trigger: trigger.to_owned(),
            computer: computer.to_owned(),
            hint: token[token.len().saturating_sub(4)..].to_owned(),
            created_at: self.now(),
            last_used_at: None,
        };
        self.db.tx(|tx| {
            let live: i64 =
                tx.query_row("SELECT COUNT(*) FROM hook_tokens WHERE computer_id = ?1 AND revoked_at IS NULL", [computer_id], |r| {
                    r.get(0)
                })?;
            if live >= MAX_HOOKS {
                return Err(bad(format!("This computer has {MAX_HOOKS} trigger URLs, the most a grid keeps. Revoke one first."), 409));
            }
            tx.execute(
                "INSERT INTO hook_tokens (id, user_id, computer_id, trigger, token_hash, hint, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![view.id, user_id, computer_id, view.trigger, crate::secrets::sha256(&token), view.hint, view.created_at],
            )?;
            // its id, not its trigger: a name may say more than the audit needs
            crate::rows::find::audit(tx, Some(user_id), "hook.create", Some(&json!({ "id": view.id })), view.created_at)?;
            Ok(())
        })?;
        Ok((view, token))
    }

    /// What a live token opens, its use counted; None for anything else (unknown, revoked, its user disabled).
    pub fn check(&self, token: &str) -> Result<Option<Hook>> {
        type Row = (String, String, String, String, Option<i64>, Option<i64>);
        if !is_token(token) {
            return Ok(None);
        }
        let row: Option<Row> = self.db.with(|c| {
            c.query_row(
                "SELECT h.id, h.user_id, h.computer_id, h.trigger, h.last_used_at, h.revoked_at
                 FROM hook_tokens h JOIN users u ON u.id = h.user_id WHERE h.token_hash = ?1 AND u.disabled = 0",
                [crate::secrets::sha256(token)],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .optional()
        })?;
        let Some((id, user_id, computer_id, trigger, used, None)) = row else { return Ok(None) };
        let now = self.now();
        if used.is_none_or(|t| now - t > USED_EVERY_MS) {
            self.db.with(|c| c.execute("UPDATE hook_tokens SET last_used_at = ?1, calls = calls + 1 WHERE id = ?2", params![now, id]))?;
        }
        Ok(Some(Hook { id, user_id, computer_id, trigger }))
    }

    /// A computer's live URLs, newest first.
    pub fn list(&self, computer_id: &str, computer: &str) -> Result<Vec<HookView>> {
        self.db.with(|c| {
            let mut s = c.prepare_cached(
                "SELECT id, trigger, hint, created_at, last_used_at FROM hook_tokens
                 WHERE computer_id = ?1 AND revoked_at IS NULL ORDER BY created_at DESC, rowid DESC",
            )?;
            let rows = s.query_map([computer_id], |r| {
                Ok(HookView {
                    id: r.get(0)?,
                    trigger: r.get(1)?,
                    computer: computer.to_owned(),
                    hint: r.get(2)?,
                    created_at: r.get(3)?,
                    last_used_at: r.get(4)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Revokes one of a computer's live URLs: false when there's no such URL on it (or it's revoked already).
    pub fn revoke(&self, user_id: &str, computer_id: &str, id: &str) -> Result<bool> {
        if !is_id(id) {
            return Ok(false);
        }
        let now = self.now();
        self.db.tx(|tx| {
            let n = tx.execute(
                "UPDATE hook_tokens SET revoked_at = ?1 WHERE id = ?2 AND computer_id = ?3 AND user_id = ?4 AND revoked_at IS NULL",
                params![now, id, computer_id, user_id],
            )?;
            if n > 0 {
                crate::rows::find::audit(tx, Some(user_id), "hook.revoke", Some(&json!({ "id": id })), now)?;
            }
            Ok(n > 0)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::accounts::{Accounts, Poll};
    use crate::clock::ManualClock;

    fn setup() -> (Hooks, Accounts, Arc<ManualClock>, String, String) {
        let db = Arc::new(Db::memory().unwrap());
        let clock = ManualClock::new(1_000_000);
        let accounts = Accounts { db: db.clone(), clock: clock.clone(), telemetry: None };
        let ada = accounts.dev_user("ada").unwrap();
        let code = accounts.device_code("studio").unwrap();
        accounts.decide(code["user_code"].as_str().unwrap(), &ada.id, true).unwrap();
        let Poll::Token { token, .. } = accounts.poll(code["device_code"].as_str().unwrap()).unwrap() else { unreachable!() };
        let (_, dev) = accounts.device(&token).unwrap().unwrap();
        let c = accounts.add_computer(&ada, &dev, "studio").unwrap();
        (Hooks { db, clock: clock.clone() }, accounts, clock, ada.id, c.id)
    }

    #[test]
    fn a_token_is_said_once_kept_as_its_hash_and_opens_its_computers_trigger() {
        let (h, _, clock, ada, comp) = setup();
        let (v, token) = h.create(&ada, &comp, "studio", "deploy").unwrap();
        assert!(is_token(&token), "{token}");
        assert_eq!(v.hint, token[token.len() - 4..]);
        let kept: String = h.db.with(|c| c.query_row("SELECT token_hash FROM hook_tokens", [], |r| r.get(0))).unwrap();
        assert_eq!(kept, crate::secrets::sha256(&token));
        let audit: Vec<String> =
            h.db.with(|c| c.prepare("SELECT action || ' ' || meta FROM audit")?.query_map([], |r| r.get(0))?.collect()).unwrap();
        assert!(!audit.concat().contains(&token[4..]) && !audit.concat().contains("deploy"), "{audit:?}");
        let got = h.check(&token).unwrap().unwrap();
        assert_eq!((got.computer_id.as_str(), got.trigger.as_str(), got.user_id.as_str()), (comp.as_str(), "deploy", ada.as_str()));
        assert_eq!(h.list(&comp, "studio").unwrap()[0].last_used_at, Some(1_000_000));
        clock.advance(61_000);
        h.check(&token).unwrap();
        assert_eq!(h.list(&comp, "studio").unwrap()[0].last_used_at, Some(1_061_000));
        let flipped = format!("{}{}", &token[..token.len() - 1], if token.ends_with('A') { 'B' } else { 'A' });
        assert!(h.check(&flipped).unwrap().is_none(), "one character off opens nothing");
        assert!(h.check("deploy").unwrap().is_none(), "a trigger's name is never a token");
    }

    #[test]
    fn a_revoked_token_opens_nothing_and_only_its_computer_revokes_it() {
        let (h, _, _, ada, comp) = setup();
        let (v, token) = h.create(&ada, &comp, "studio", "deploy").unwrap();
        assert!(!h.revoke(&ada, "other-computer", &v.id).unwrap(), "not that computer's");
        assert!(!h.revoke("someone", &comp, &v.id).unwrap(), "not their computer");
        assert!(h.revoke(&ada, &comp, &v.id).unwrap());
        assert!(!h.revoke(&ada, &comp, &v.id).unwrap(), "once");
        assert!(h.check(&token).unwrap().is_none());
        assert_eq!(h.list(&comp, "studio").unwrap().len(), 0);
    }

    #[test]
    fn a_computer_holds_at_most_fifty_live_urls() {
        let (h, _, _, ada, comp) = setup();
        let first = h.create(&ada, &comp, "studio", "t0").unwrap().0;
        for i in 1..MAX_HOOKS {
            h.create(&ada, &comp, "studio", &format!("t{i}")).unwrap();
        }
        assert_eq!(h.create(&ada, &comp, "studio", "one too many").map_err(|e| e.status()).err(), Some(409));
        assert!(h.revoke(&ada, &comp, &first.id).unwrap());
        assert!(h.create(&ada, &comp, "studio", "room again").is_ok());
    }

    #[test]
    fn removing_the_computer_removes_its_urls() {
        let (h, accounts, _, ada, comp) = setup();
        let (_, token) = h.create(&ada, &comp, "studio", "deploy").unwrap();
        h.db.with(|c| c.execute("DELETE FROM computers WHERE id = ?1", [&comp])).unwrap();
        assert!(h.check(&token).unwrap().is_none());
        let _ = accounts;
    }

    #[test]
    fn a_trigger_name_is_one_path_segment() {
        let n = |v: serde_json::Value| trigger_name(Some(&v));
        assert_eq!(n(json!("  deploy ")).unwrap(), "deploy");
        for bad in [json!(""), json!("a/b"), json!("a b"), json!("x".repeat(129)), json!(3)] {
            assert_eq!(n(bad.clone()).map_err(|e| e.status()), Err(400), "{bad}");
        }
    }
}
