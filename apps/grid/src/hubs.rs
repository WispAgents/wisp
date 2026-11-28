//! A team's hub (ADR-0148): an account of the team's own, a `users` row with the team's slug as its handle and no
//! identity, so nobody signs in as it and it gets a box like anyone's. `teams.hub_user_id` points at it.
//!
//! The team's slug and a person's handle share one namespace (both are `<label>.<domain>`), so a slug someone holds as
//! a handle, or a reserved word, gets no hub. A team made before hubs gets its hub on first asking (`ensure`).
use rusqlite::{OptionalExtension, Transaction, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::error::{Result, bad};
use crate::rows::UserRow;
use crate::teams::{Role, Teams};

/// A team's hub as the edge needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hub {
    pub team_id: String,
    pub slug: String,
    pub name: String,
    /// the hub's own account (its box runs for it)
    pub user_id: String,
}

/// Whether `slug` can be a hub's handle: not reserved, and no account holds it.
fn free(tx: &Transaction<'_>, slug: &str) -> Result<()> {
    let held = tx.query_row("SELECT 1 FROM users WHERE handle = ?1", [slug], |_| Ok(())).optional()?.is_some();
    if held || crate::config::RESERVED.contains(&slug) {
        return Err(bad(format!("{slug} is taken on this grid: pick another name for the team."), 409));
    }
    Ok(())
}

/// The hub's account for team `tid`, made in `tx`; its id.
pub(crate) fn make(tx: &Transaction<'_>, tid: &str, slug: &str, name: &str, now: i64) -> Result<String> {
    free(tx, slug)?;
    let uid = format!("h_{}", crate::secrets::hex(&rand::random::<[u8; 8]>()));
    tx.execute("INSERT INTO users (id, handle, name, created_at) VALUES (?1, ?2, ?3, ?4)", params![uid, slug, name, now])?;
    tx.execute("UPDATE teams SET hub_user_id = ?1 WHERE id = ?2", params![uid, tid])?;
    crate::rows::find::audit(tx, None, "hub.create", Some(&json!({ "team": slug })), now)?;
    Ok(uid)
}

fn hub_of(r: &rusqlite::Row<'_>) -> rusqlite::Result<Hub> {
    Ok(Hub { team_id: r.get(0)?, slug: r.get(1)?, name: r.get(2)?, user_id: r.get(3)? })
}

impl Teams {
    /// The hub whose account is `user_id`: what the edge asks of every `<handle>` it opens.
    pub fn hub_by_user(&self, user_id: &str) -> Result<Option<Hub>> {
        self.db.with(|c| {
            c.prepare_cached("SELECT id, slug, name, hub_user_id FROM teams WHERE hub_user_id = ?1")?
                .query_row([user_id], hub_of)
                .optional()
        })
    }

    /// Team `slug`'s hub, if it has one.
    pub fn hub(&self, slug: &str) -> Result<Option<Hub>> {
        self.db.with(|c| {
            c.prepare_cached("SELECT id, slug, name, hub_user_id FROM teams WHERE slug = ?1 AND hub_user_id IS NOT NULL")?
                .query_row([slug], hub_of)
                .optional()
        })
    }

    /// Whether an account is a hub's (it never signs in, never counts as a person).
    pub fn is_hub(&self, user_id: &str) -> Result<bool> {
        Ok(self.hub_by_user(user_id)?.is_some())
    }

    /// Team `slug`'s hub, made now for a team from before hubs (an owner or admin asks).
    pub fn ensure_hub(&self, actor: &UserRow, slug: &str) -> Result<Hub> {
        if let Some(h) = self.hub(slug)? {
            return Ok(h);
        }
        let mine = self.role(&actor.id, slug)?.ok_or_else(|| bad(format!("You aren't in {slug}."), 403))?;
        if !mine.manages() {
            return Err(bad(format!("Only an owner or admin of {slug} turns its hub on."), 403));
        }
        let now = self.clock.now_ms();
        self.db.tx(|tx| {
            let (tid, name): (String, String) =
                tx.query_row("SELECT id, name FROM teams WHERE slug = ?1", [slug], |r| Ok((r.get(0)?, r.get(1)?)))?;
            make(tx, &tid, slug, &name, now)?;
            Ok(())
        })?;
        self.hub(slug)?.ok_or_else(|| bad("The hub wasn't made.", 500))
    }

    /// The hubs a person may open, with their role and the hub's address: `/api/me`'s `hubs`, the machines list.
    pub fn hubs_of(&self, user_id: &str, address: impl Fn(&str) -> String) -> Result<Vec<Value>> {
        let rows: Vec<(String, String, String)> = self.db.with(|c| {
            c.prepare_cached(
                "SELECT t.slug, t.name, m.role FROM team_members m JOIN teams t ON t.id = m.team_id
                 WHERE m.user_id = ?1 AND t.hub_user_id IS NOT NULL ORDER BY t.slug",
            )?
            .query_map([user_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect()
        })?;
        Ok(rows.into_iter().map(|(slug, name, role)| json!({ "slug": slug, "name": name, "role": role, "url": address(&slug) })).collect())
    }

    /// What `actor` may do at hub `h`'s address: their role, or None (not in the team).
    pub fn hub_role(&self, actor_id: &str, h: &Hub) -> Result<Option<Role>> {
        self.role(actor_id, &h.slug)
    }

    /// The teams `user_id` is the only owner of: an account with one can't be deleted until it has another owner.
    pub fn sole_owner_of(&self, user_id: &str) -> Result<Vec<String>> {
        self.db.with(|c| {
            c.prepare_cached(
                "SELECT t.slug FROM team_members m JOIN teams t ON t.id = m.team_id WHERE m.user_id = ?1 AND m.role = 'owner'
                 AND (SELECT count(*) FROM team_members o WHERE o.team_id = m.team_id AND o.role = 'owner') = 1 ORDER BY t.slug",
            )?
            .query_map([user_id], |r| r.get(0))?
            .collect()
        })
    }

    /// Team `slug` and everything it has, for its owner: the team's row (members, invites, keys and deploys go with it).
    /// The hub's account is the caller's to delete with `delete_user` (its box, home and secrets): returned here.
    pub fn delete(&self, actor: &UserRow, slug: &str, confirm: &str) -> Result<Option<UserRow>> {
        if self.role(&actor.id, slug)? != Some(Role::Owner) {
            return Err(bad(format!("Only an owner of {slug} deletes it."), 403));
        }
        if confirm.trim() != slug {
            return Err(bad(format!("Type {slug} to delete the team and its hub."), 400));
        }
        let hub = self.hub(slug)?;
        let now = self.clock.now_ms();
        self.db.tx(|tx| {
            tx.execute("DELETE FROM teams WHERE slug = ?1", [slug])?;
            crate::rows::find::audit(tx, Some(&actor.id), "team.delete", Some(&json!({ "team": slug })), now)?;
            Ok(())
        })?;
        match hub {
            Some(h) => self.db.with(|c| crate::rows::find::user_by_id(c, &h.user_id)),
            None => Ok(None),
        }
    }
}
