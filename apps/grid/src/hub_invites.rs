//! Invite links to a team (ADR-0148 §3): `<grid>/join/<code>`. The code is said once, in the answer that made it; the
//! grid keeps its SHA-256. Opening it signed in adds you with the invite's role. An invite never makes an owner, an
//! admin's never makes an admin, and a spent, expired or revoked one adds nobody.
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::json;

use crate::error::{Result, bad};
use crate::rows::UserRow;
use crate::teams::{Role, Teams};

pub const PREFIX: &str = "wgi_";
/// Open invites a team may hold at once.
pub const MAX_OPEN: i64 = 20;
/// The longest an invite lasts, and how long one lasts unless told: 30 days, 7 days.
pub const MAX_TTL_MS: i64 = 30 * 86_400_000;
pub const TTL_MS: i64 = 7 * 86_400_000;
/// The most people one link adds.
pub const MAX_USES: i64 = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteView {
    pub id: String,
    pub role: String,
    pub created_by: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub uses_left: i64,
}

/// What an invite asks for, before it's made.
#[derive(Debug, Clone, Copy)]
pub struct Ask {
    pub role: Role,
    pub ttl_ms: i64,
    pub uses: i64,
}

impl Teams {
    fn manager(&self, actor: &UserRow, slug: &str) -> Result<Role> {
        let mine = self.role(&actor.id, slug)?.ok_or_else(|| bad(format!("You aren't in {slug}."), 403))?;
        if !mine.manages() {
            return Err(bad(format!("Only an owner or admin of {slug} invites people."), 403));
        }
        Ok(mine)
    }

    fn team_row(&self, slug: &str) -> Result<String> {
        self.db
            .with(|c| c.query_row("SELECT id FROM teams WHERE slug = ?1", [slug], |r| r.get(0)).optional())?
            .ok_or_else(|| bad(format!("There's no team {slug}."), 404))
    }

    /// A new invite link's code (said once) and its view.
    pub fn invite(&self, actor: &UserRow, slug: &str, ask: Ask) -> Result<(InviteView, String)> {
        let mine = self.manager(actor, slug)?;
        if ask.role == Role::Owner || (ask.role == Role::Admin && mine != Role::Owner) {
            return Err(bad("An invite makes members, viewers or (an owner's) admins: make an owner by hand.", 403));
        }
        if !(60_000..=MAX_TTL_MS).contains(&ask.ttl_ms) {
            return Err(bad("An invite lasts from a minute to 30 days.", 400));
        }
        if !(1..=MAX_USES).contains(&ask.uses) {
            return Err(bad("An invite adds 1 to 100 people.", 400));
        }
        let tid = self.team_row(slug)?;
        let code = crate::accounts::token(PREFIX);
        let now = self.clock.now_ms();
        let view = InviteView {
            id: crate::secrets::hex(&rand::random::<[u8; 6]>()),
            role: ask.role.as_str().into(),
            created_by: actor.handle.clone(),
            created_at: now,
            expires_at: now + ask.ttl_ms,
            uses_left: ask.uses,
        };
        self.db.tx(|tx| {
            let open: i64 = tx.query_row(
                "SELECT count(*) FROM team_invites WHERE team_id = ?1 AND revoked_at IS NULL AND uses_left > 0 AND expires_at > ?2",
                params![tid, now],
                |r| r.get(0),
            )?;
            if open >= MAX_OPEN {
                return Err(bad(format!("{slug} has {MAX_OPEN} open invites: revoke one first."), 409));
            }
            tx.execute(
                "INSERT INTO team_invites (id, team_id, code_hash, role, created_by, created_at, expires_at, uses_left)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![view.id, tid, crate::secrets::sha256(&code), view.role, view.created_by, now, view.expires_at, view.uses_left],
            )?;
            crate::rows::find::audit(tx, Some(&actor.id), "invite.create", Some(&json!({ "team": slug, "id": view.id })), now)?;
            Ok(())
        })?;
        Ok((view, code))
    }

    /// A team's open invites, newest first (managers only).
    pub fn invites(&self, actor: &UserRow, slug: &str) -> Result<Vec<InviteView>> {
        self.manager(actor, slug)?;
        let tid = self.team_row(slug)?;
        let now = self.clock.now_ms();
        self.db.with(|c| {
            c.prepare_cached(
                "SELECT id, role, created_by, created_at, expires_at, uses_left FROM team_invites
                 WHERE team_id = ?1 AND revoked_at IS NULL AND uses_left > 0 AND expires_at > ?2 ORDER BY created_at DESC, id",
            )?
            .query_map(params![tid, now], |r| {
                Ok(InviteView {
                    id: r.get(0)?,
                    role: r.get(1)?,
                    created_by: r.get(2)?,
                    created_at: r.get(3)?,
                    expires_at: r.get(4)?,
                    uses_left: r.get(5)?,
                })
            })?
            .collect()
        })
    }

    /// An invite gone at once. 404 for one that isn't this team's open invite.
    pub fn revoke_invite(&self, actor: &UserRow, slug: &str, id: &str) -> Result<()> {
        self.manager(actor, slug)?;
        let tid = self.team_row(slug)?;
        let now = self.clock.now_ms();
        let n = self.db.with(|c| {
            c.execute(
                "UPDATE team_invites SET revoked_at = ?1 WHERE id = ?2 AND team_id = ?3 AND revoked_at IS NULL",
                params![now, id, tid],
            )
        })?;
        if n == 0 {
            return Err(bad("No such invite.", 404));
        }
        Ok(())
    }

    /// `user` takes invite `code`: added with its role (someone already in keeps the higher of the two). The team's slug.
    pub fn join(&self, user: &UserRow, code: &str) -> Result<serde_json::Value> {
        if self.is_hub(&user.id)? {
            return Err(bad("A hub doesn't join teams.", 403));
        }
        let now = self.clock.now_ms();
        let hash = crate::secrets::sha256(code.trim());
        self.db.tx(|tx| {
            let row: Option<(String, String, String, i64, i64, Option<i64>)> = tx
                .query_row(
                    "SELECT i.id, i.team_id, i.role, i.expires_at, i.uses_left, i.revoked_at FROM team_invites i WHERE i.code_hash = ?1",
                    [&hash],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
                )
                .optional()?;
            let Some((id, tid, role, expires, left, revoked)) = row else {
                return Err(bad("That invite link isn't one this grid made.", 404));
            };
            if revoked.is_some() || left <= 0 || expires <= now {
                return Err(bad("That invite link is used up or expired: ask for a new one.", 410));
            }
            let slug: String = tx.query_row("SELECT slug FROM teams WHERE id = ?1", [&tid], |r| r.get(0))?;
            let had: Option<String> = tx
                .query_row("SELECT role FROM team_members WHERE team_id = ?1 AND user_id = ?2", params![tid, user.id], |r| r.get(0))
                .optional()?;
            let give = Role::parse(&role).unwrap_or(Role::Viewer);
            let keep = had.as_deref().and_then(Role::parse).map_or(give, |h| h.max(give));
            tx.execute(
                "INSERT INTO team_members (team_id, user_id, role, created_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(team_id, user_id) DO UPDATE SET role = excluded.role",
                params![tid, user.id, keep.as_str(), now],
            )?;
            if had.is_none() {
                tx.execute("UPDATE team_invites SET uses_left = uses_left - 1 WHERE id = ?1", [&id])?;
            }
            crate::rows::find::audit(tx, Some(&user.id), "invite.join", Some(&json!({ "team": slug, "id": id })), now)?;
            Ok(json!({ "slug": slug, "role": keep.as_str() }))
        })
    }
}

#[cfg(test)]
#[path = "hub_invites_tests.rs"]
mod tests;
