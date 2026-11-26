//! A sign-in carried from the grid to one of your addresses (v1 grid/server.js `/go` and `/_grid/auth`):
//! your cloud wisp (`<handle>`), a computer (`<name>--<handle>`) or a sandbox (`sb-<id>--<handle>`).
//!
//! The grid's own cookie is host-only, so it never reaches those addresses (and must not: a page a box serves
//! could read it there). Instead `/go` makes a one-time ticket for that one address and sends the browser to
//! `<address>/_grid/auth?t=<ticket>&r=<path>`, which spends it for the address's own cookie ([`COOKIE`]), a
//! session scoped `i:<label>`.
//!
//! - A ticket lives [`crate::previews::TICKET_MS`] and is spent once. It is scoped to its address's label, so a
//!   ticket for your cloud wisp opens neither a computer nor a preview, and the other way round.
//! - An address session is good at its own label only. [`Accounts::session`] (the grid's pages) never takes one,
//!   and [`Accounts::address_session`] takes nothing else.
use rusqlite::{OptionalExtension, params};

use crate::accounts::{Accounts, sha256_hex, token};
use crate::error::Result;
use crate::rows::UserRow;

/// An address's own sign-in cookie (host-only, set from a ticket). Not the grid's cookie name, so a dev grid on
/// `*.localhost` never confuses the two.
pub const COOKIE: &str = "wisp_box";
/// An address session's life, as the grid's own (v1: `sessionDays`).
pub const SESSION_MS: i64 = 30 * 24 * 3_600_000;

/// A ticket's scope for one address.
pub fn ticket_scope(label: &str) -> String {
    format!("box:{label}")
}

/// An address session's scope (v1's `i:` sessions).
pub fn session_scope(label: &str) -> String {
    format!("i:{label}")
}

impl Accounts {
    /// A one-time ticket to `label`'s address, for its owner.
    pub fn address_ticket(&self, user_id: &str, label: &str) -> Result<String> {
        self.ticket(user_id, &ticket_scope(label))
    }

    /// Spends a ticket at `label`'s address: the user it was for, if it was for this address and is fresh.
    pub fn use_address_ticket(&self, t: &str, label: &str) -> Result<Option<String>> {
        self.use_ticket(t, &ticket_scope(label))
    }

    /// A sign-in at `label`'s address: the cookie's value (kept hashed).
    pub fn new_address_session(&self, user_id: &str, label: &str, ua: &str) -> Result<String> {
        let t = token("");
        let now = self.clock.now_ms();
        let ua: String = ua.chars().take(200).collect();
        self.db.with(|c| {
            c.execute(
                "INSERT INTO sessions (id_hash, user_id, scope, created_at, expires_at, last_seen_at, ua) VALUES (?1, ?2, ?3, ?4, ?5, ?4, ?6)",
                params![sha256_hex(&t), user_id, session_scope(label), now, now + SESSION_MS, ua],
            )
        })?;
        Ok(t)
    }

    /// Who this cookie signs in at `label`'s address: only a session made there, unexpired, of a user still allowed.
    pub fn address_session(&self, cookie: &str, label: &str) -> Result<Option<UserRow>> {
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

    /// Signs this cookie out of its address.
    pub fn end_address_session(&self, cookie: &str) -> Result<()> {
        if !cookie.is_empty() {
            self.db.with(|c| c.execute("DELETE FROM sessions WHERE id_hash = ?1 AND scope LIKE 'i:%'", [sha256_hex(cookie)]))?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::clock::ManualClock;
    use crate::db::Db;
    use crate::previews::TICKET_MS;

    fn accounts() -> (Accounts, Arc<ManualClock>) {
        let clock = ManualClock::new(1_000_000);
        (Accounts { db: Arc::new(Db::memory().unwrap()), clock: clock.clone(), telemetry: None }, clock)
    }

    #[test]
    fn an_address_ticket_is_spent_once_at_its_own_address() {
        let (a, _) = accounts();
        let ada = a.dev_user("ada").unwrap();
        let t = a.address_ticket(&ada.id, "ada").unwrap();
        // not at another address: and that try doesn't spend it
        assert_eq!(a.use_address_ticket(&t, "studio--ada").unwrap(), None);
        assert_eq!(a.use_ticket(&t, &crate::previews::ticket_scope("ada")).unwrap(), None, "never a preview's");
        assert_eq!(a.use_address_ticket(&t, "ada").unwrap().as_deref(), Some(ada.id.as_str()));
        // replayed
        assert_eq!(a.use_address_ticket(&t, "ada").unwrap(), None);
        assert_eq!(a.use_address_ticket("", "ada").unwrap(), None);
    }

    #[test]
    fn an_address_ticket_expires() {
        let (a, clock) = accounts();
        let ada = a.dev_user("ada").unwrap();
        let t = a.address_ticket(&ada.id, "ada").unwrap();
        clock.advance(TICKET_MS + 1);
        assert_eq!(a.use_address_ticket(&t, "ada").unwrap(), None);
    }

    #[test]
    fn an_address_session_signs_in_there_and_nowhere_else() {
        let (a, clock) = accounts();
        let ada = a.dev_user("ada").unwrap();
        let s = a.new_address_session(&ada.id, "ada", "test").unwrap();
        assert_eq!(a.address_session(&s, "ada").unwrap().map(|u| u.handle).as_deref(), Some("ada"));
        assert!(a.address_session(&s, "studio--ada").unwrap().is_none(), "another address");
        assert!(a.session(&s).unwrap().is_none(), "the grid's own pages");
        assert!(a.preview_session(&s, "ada").unwrap().is_none(), "a preview address");
        // and the grid's own session isn't one
        let g = a.new_session(&ada.id).unwrap();
        assert!(a.address_session(&g, "ada").unwrap().is_none());
        // signed out
        a.end_address_session(&s).unwrap();
        assert!(a.address_session(&s, "ada").unwrap().is_none());
        // expired
        let s = a.new_address_session(&ada.id, "ada", "").unwrap();
        clock.advance(SESSION_MS + 1);
        assert!(a.address_session(&s, "ada").unwrap().is_none());
    }
}
