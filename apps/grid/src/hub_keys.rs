//! A team's deploy keys (ADR-0148 §4): `Authorization: Bearer wgt_…` at the hub's own address, for CI and scripts that
//! plan and apply the team's folder. Like an API key (`keys.rs`): said once, kept as its SHA-256 and last four
//! characters, `last_used_at` at most once a minute, revoked at once. A key opens its own team's hub and nothing else
//! (`trust`: `HubKey` reaches `box` and `grid:hub`), and the hub's wisp gives it `hub:deploy` (resources only).
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::json;

use crate::error::{Result, bad};
use crate::hubs::Hub;
use crate::rows::UserRow;
use crate::teams::Teams;

pub const PREFIX: &str = "wgt_";
/// Live keys a team may hold at once.
pub const MAX_KEYS: i64 = 20;
const USED_EVERY_MS: i64 = 60_000;

/// What a key may do: `plan` (pull requests) or `deploy` (plan and apply).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Rights {
    Plan,
    Deploy,
}

impl Rights {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "plan" => Some(Self::Plan),
            "deploy" | "" => Some(Self::Deploy),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Deploy => "deploy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubKeyView {
    pub id: String,
    pub name: String,
    pub hint: String,
    pub rights: Rights,
    pub created_by: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

/// A live key, checked: its team's hub, and what it may do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubKey {
    pub id: String,
    pub name: String,
    pub rights: Rights,
    pub hub: Hub,
}

/// The key a `Bearer wgt_…` header carries.
pub fn bearer(auth: &str) -> Option<&str> {
    let a = auth.trim();
    let t = match a.split_once(char::is_whitespace) {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("bearer") => rest.trim(),
        Some(_) => return None,
        None => a,
    };
    t.starts_with(PREFIX).then_some(t)
}

fn view(r: &rusqlite::Row<'_>) -> rusqlite::Result<HubKeyView> {
    let rights: String = r.get(3)?;
    Ok(HubKeyView {
        id: r.get(0)?,
        name: r.get(1)?,
        hint: r.get(2)?,
        rights: Rights::parse(&rights).unwrap_or(Rights::Plan),
        created_by: r.get(4)?,
        created_at: r.get(5)?,
        last_used_at: r.get(6)?,
    })
}

impl Teams {
    fn key_manager(&self, actor: &UserRow, slug: &str) -> Result<Hub> {
        let mine = self.role(&actor.id, slug)?.ok_or_else(|| bad(format!("You aren't in {slug}."), 403))?;
        if !mine.manages() {
            return Err(bad(format!("Only an owner or admin of {slug} manages its deploy keys."), 403));
        }
        self.hub(slug)?.ok_or_else(|| bad(format!("{slug} has no hub yet: wisp team hub {slug}"), 409))
    }

    /// A new deploy key (said once) and its view.
    pub fn key_create(&self, actor: &UserRow, slug: &str, name: &str, rights: Rights) -> Result<(HubKeyView, String)> {
        let hub = self.key_manager(actor, slug)?;
        let key = crate::accounts::token(PREFIX);
        let v = HubKeyView {
            id: crate::secrets::hex(&rand::random::<[u8; 6]>()),
            name: name.to_owned(),
            hint: key[key.len().saturating_sub(4)..].to_owned(),
            rights,
            created_by: actor.handle.clone(),
            created_at: self.clock.now_ms(),
            last_used_at: None,
        };
        self.db.tx(|tx| {
            let live: i64 =
                tx.query_row("SELECT count(*) FROM team_keys WHERE team_id = ?1 AND revoked_at IS NULL", [&hub.team_id], |r| r.get(0))?;
            if live >= MAX_KEYS {
                return Err(bad(format!("{slug} has {MAX_KEYS} deploy keys, the most a team keeps. Revoke one first."), 409));
            }
            tx.execute(
                "INSERT INTO team_keys (id, team_id, name, token_hash, hint, rights, created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![v.id, hub.team_id, v.name, crate::secrets::sha256(&key), v.hint, rights.as_str(), v.created_by, v.created_at],
            )?;
            crate::rows::find::audit(tx, Some(&actor.id), "hubkey.create", Some(&json!({ "team": slug, "id": v.id })), v.created_at)?;
            Ok(())
        })?;
        Ok((v, key))
    }

    /// A team's live keys, newest first (managers only).
    pub fn keys(&self, actor: &UserRow, slug: &str) -> Result<Vec<HubKeyView>> {
        let hub = self.key_manager(actor, slug)?;
        self.db.with(|c| {
            c.prepare_cached(
                "SELECT id, name, hint, rights, created_by, created_at, last_used_at FROM team_keys
                 WHERE team_id = ?1 AND revoked_at IS NULL ORDER BY created_at DESC, id",
            )?
            .query_map([&hub.team_id], view)?
            .collect()
        })
    }

    /// A key gone at once: its next call is a 401. 404 for one that isn't this team's live key.
    pub fn key_revoke(&self, actor: &UserRow, slug: &str, id: &str) -> Result<()> {
        let hub = self.key_manager(actor, slug)?;
        let now = self.clock.now_ms();
        let n = self.db.with(|c| {
            c.execute(
                "UPDATE team_keys SET revoked_at = ?1 WHERE id = ?2 AND team_id = ?3 AND revoked_at IS NULL",
                params![now, id, hub.team_id],
            )
        })?;
        if n == 0 {
            return Err(bad("No such key.", 404));
        }
        self.db.with(|c| crate::rows::find::audit(c, Some(&actor.id), "hubkey.revoke", Some(&json!({ "team": slug, "id": id })), now))?;
        Ok(())
    }

    /// The live key a header carries, with its team's hub; None for no `wgt_` key, an unknown or revoked one.
    pub fn key_check(&self, auth: &str) -> Result<Option<HubKey>> {
        let Some(key) = bearer(auth) else { return Ok(None) };
        let row: Option<(String, String, String, Option<i64>, String)> = self.db.with(|c| {
            c.prepare_cached("SELECT id, name, rights, last_used_at, team_id FROM team_keys WHERE token_hash = ?1 AND revoked_at IS NULL")?
                .query_row([crate::secrets::sha256(key)], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
                .optional()
        })?;
        let Some((id, name, rights, used, tid)) = row else { return Ok(None) };
        let slug: Option<String> =
            self.db.with(|c| c.query_row("SELECT slug FROM teams WHERE id = ?1", [&tid], |r| r.get(0)).optional())?;
        let Some(hub) = slug.map(|s| self.hub(&s)).transpose()?.flatten() else { return Ok(None) };
        let now = self.clock.now_ms();
        if used.is_none_or(|t| now - t > USED_EVERY_MS) {
            self.db.with(|c| c.execute("UPDATE team_keys SET last_used_at = ?1 WHERE id = ?2", params![now, id]))?;
        }
        Ok(Some(HubKey { id, name, rights: Rights::parse(&rights).unwrap_or(Rights::Plan), hub }))
    }
}
