//! Pages in a box at their own address (ADR-0064, v1 grid/server.js `previewEdge`): `p<port>--<handle>.<domain>`
//! shows what listens on `<port>` inside that user's box, to that user alone.
//!
//! The address has a sign-in of its own, so neither it nor the user's other addresses can stand in for the
//! other: a one-time ticket scoped `preview:<label>` (60 s, used once), asked for at the box's own address by its
//! owner, becomes a host-only session scoped `p:<label>` (cookie [`COOKIE`]). [`Accounts::session`] never accepts
//! one of those, and [`Accounts::preview_session`] accepts nothing else.
//!
//! Which ports: [`PORTS`] (an app's dev server, never a system port), never the grid's own.
use std::ops::RangeInclusive;

use rusqlite::{OptionalExtension, params};

use crate::accounts::{Accounts, sha256_hex, token};
use crate::error::Result;
use crate::rows::UserRow;

/// The preview address's own cookie (host-only, set from a ticket).
pub const COOKIE: &str = "wisp_preview";
/// A ticket's life: long enough for a redirect, never for a link to be passed around.
pub const TICKET_MS: i64 = 60_000;
/// A preview sign-in's life.
pub const SESSION_MS: i64 = 7 * 24 * 3_600_000;
/// Ports a page in a box may be shown from: a dev server's, never a system one (`< 1024`).
pub const PORTS: RangeInclusive<u32> = 1024..=65_535;

/// Whether `port` may be shown: in [`PORTS`] and not the grid's own (whose loopback a process box shares).
pub fn allowed_port(port: u32, grid_port: u16) -> bool {
    PORTS.contains(&port) && port != u32::from(grid_port)
}

/// A ticket's scope for one label's preview addresses.
pub fn ticket_scope(label: &str) -> String {
    format!("preview:{label}")
}

/// A preview session's scope.
pub fn session_scope(label: &str) -> String {
    format!("p:{label}")
}

impl Accounts {
    /// A one-time ticket for `scope`, good for [`TICKET_MS`].
    pub fn ticket(&self, user_id: &str, scope: &str) -> Result<String> {
        let t = token("wgt_");
        let now = self.clock.now_ms();
        self.db.with(|c| {
            c.execute(
                "INSERT INTO tickets (hash, user_id, expires_at, scope) VALUES (?1, ?2, ?3, ?4)",
                params![sha256_hex(&t), user_id, now + TICKET_MS, scope],
            )
        })?;
        Ok(t)
    }

    /// The user a ticket was for, once: unused, unexpired and of exactly this `scope` (a ticket for another label,
    /// or for the user's own addresses, is no ticket here). Spent either way it's looked up with the right scope.
    pub fn use_ticket(&self, t: &str, scope: &str) -> Result<Option<String>> {
        if t.is_empty() {
            return Ok(None);
        }
        let now = self.clock.now_ms();
        self.db.with(|c| {
            c.query_row(
                "UPDATE tickets SET used_at = ?3 WHERE hash = ?1 AND scope = ?2 AND used_at IS NULL AND expires_at > ?3 RETURNING user_id",
                params![sha256_hex(t), scope, now],
                |r| r.get::<_, String>(0),
            )
            .optional()
        })
    }

    /// A preview sign-in at `label`'s preview addresses: the cookie's value (kept hashed).
    pub fn new_preview_session(&self, user_id: &str, label: &str, ua: &str) -> Result<String> {
        let t = token("");
        let now = self.clock.now_ms();
        self.db.with(|c| {
            c.execute(
                "INSERT INTO sessions (id_hash, user_id, scope, created_at, expires_at, last_seen_at, ua) VALUES (?1, ?2, ?3, ?4, ?5, ?4, ?6)",
                params![sha256_hex(&t), user_id, session_scope(label), now, now + SESSION_MS, ua],
            )
        })?;
        Ok(t)
    }

    /// The user signed in at `label`'s preview addresses by this cookie, if it's that label's and theirs to use.
    pub fn preview_session(&self, cookie: &str, label: &str) -> Result<Option<UserRow>> {
        if cookie.is_empty() {
            return Ok(None);
        }
        let now = self.clock.now_ms();
        let uid: Option<String> = self.db.with(|c| {
            c.query_row(
                "SELECT user_id FROM sessions WHERE id_hash = ?1 AND scope = ?2 AND expires_at > ?3",
                params![sha256_hex(cookie), session_scope(label), now],
                |r| r.get(0),
            )
            .optional()
        })?;
        match uid {
            Some(u) => Ok(self.user_by_id(&u)?.filter(|u| u.disabled == 0)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn only_a_dev_server_s_ports_and_never_the_grid_s_own() {
        assert!(allowed_port(5173, 8080) && allowed_port(1024, 8080) && allowed_port(65_535, 8080));
        assert!(!allowed_port(80, 8080) && !allowed_port(22, 8080) && !allowed_port(0, 8080) && !allowed_port(65_536, 8080));
        assert!(!allowed_port(8080, 8080), "the grid's own port, which a process box's loopback shares");
    }
}
