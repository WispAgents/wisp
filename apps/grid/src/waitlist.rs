//! The waitlist (v1 grid/accounts.js `waitFor`, `waitlist`): trywisp.dev's "get the next seat" form and an in-app
//! browser's "email me the link" put an address on the list (`POST /api/waitlist`); whoever then signs up with that
//! verified email is marked joined. The operator reads it with `wisp-grid waitlist` (and writes to the people on it).
//!
//! No seat cap, so no seats handed out or held: the owner removed `maxUsers` on 2026-10-27 and every grid takes
//! everyone its sign-up rules let in. `invited` stays in the answer for rows a v1 grid invited.
use rusqlite::params;
use serde::Serialize;

use crate::accounts::Accounts;
use crate::error::{Result, bad};

/// Where an address stands, as v1 answered it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Waiting {
    pub joined: bool,
    /// a seat is held for it
    pub invited: bool,
    /// how many are waiting up to it (0 once invited or joined)
    pub position: i64,
}

/// v1's `EMAIL`: `^[^@\s<>"']{1,64}@[^@\s<>"']+\.[^@\s<>"']{2,}$`, at most 254 characters.
pub fn is_email(s: &str) -> bool {
    let ok = |c: char| !(c == '@' || c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\''));
    let Some((local, domain)) = s.split_once('@') else { return false };
    let n = local.chars().count();
    let chars: Vec<char> = domain.chars().collect();
    // a dot after at least one character, with two or more after it
    let dotted = chars.iter().enumerate().any(|(i, c)| *c == '.' && i >= 1 && chars.len() - i > 2);
    s.chars().count() <= 254 && (1..=64).contains(&n) && local.chars().all(ok) && chars.iter().all(|c| ok(*c)) && dotted
}

impl Accounts {
    /// `email` on the list (once is enough); where it stands.
    pub fn wait_for(&self, email: &str, source: &str) -> Result<Waiting> {
        let email = email.trim().to_lowercase();
        if !is_email(&email) {
            return Err(bad("An email address, please.", 400));
        }
        let source = if source == "in-app" { "in-app" } else { "site" };
        let now = self.clock.now_ms();
        self.db.with(|c| {
            c.execute("INSERT OR IGNORE INTO waitlist (email, source, created_at) VALUES (?1, ?2, ?3)", params![email, source, now])
        })?;
        self.db.with(|c| {
            let (at, invited, joined): (i64, Option<i64>, Option<i64>) =
                c.query_row("SELECT created_at, invited_at, joined_at FROM waitlist WHERE email = ?1", [&email], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?;
            let ahead: i64 = c.query_row(
                "SELECT COUNT(*) FROM waitlist WHERE joined_at IS NULL AND invited_at IS NULL AND (created_at < ?1 OR (created_at = ?1 AND email <= ?2))",
                params![at, email],
                |r| r.get(0),
            )?;
            let waiting = invited.is_none() && joined.is_none();
            Ok(Waiting { joined: joined.is_some(), invited: invited.is_some() && joined.is_none(), position: if waiting { ahead } else { 0 } })
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::clock::ManualClock;
    use crate::db::Db;

    #[test]
    fn email_addresses_as_v1_took_them() {
        for ok in ["ada@x.dev", "a.b+c@sub.example.co", "ü@exämple.com"] {
            assert!(is_email(ok), "{ok}");
        }
        for bad in ["", "ada", "ada@x", "ada@x.d", "@x.dev", "a da@x.dev", "<a>@x.dev", "a@b@x.dev", "ada@.dev"] {
            assert!(!is_email(bad), "{bad}");
        }
        assert!(!is_email(&format!("{}@x.dev", "a".repeat(65))));
    }

    #[test]
    fn once_on_the_list_with_its_place() {
        let clock = ManualClock::new(1_000);
        let a = Accounts { db: Arc::new(Db::memory().unwrap()), clock: clock.clone() };
        assert_eq!(a.wait_for(" Ada@X.dev ", "site").unwrap(), Waiting { joined: false, invited: false, position: 1 });
        clock.advance(10);
        assert_eq!(a.wait_for("bob@x.dev", "in-app").unwrap().position, 2);
        // again: same place, kept as it was first said
        assert_eq!(a.wait_for("ada@x.dev", "in-app").unwrap().position, 1);
        let rows: Vec<(String, String)> =
            a.db.with(|c| {
                c.prepare("SELECT email, source FROM waitlist ORDER BY created_at")?
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect()
            })
            .unwrap();
        assert_eq!(rows, [("ada@x.dev".to_owned(), "site".to_owned()), ("bob@x.dev".to_owned(), "in-app".to_owned())]);
        // anything else as a source is the site
        a.wait_for("cyd@x.dev", "evil").unwrap();
        let source: String = a.db.with(|c| c.query_row("SELECT source FROM waitlist WHERE email = 'cyd@x.dev'", [], |r| r.get(0))).unwrap();
        assert_eq!(source, "site");
        assert_eq!(a.wait_for("nope", "site").unwrap_err().to_string(), "An email address, please.");
    }

    #[test]
    fn nobody_is_invited_and_signing_up_needs_no_seat() {
        let a = Accounts { db: Arc::new(Db::memory().unwrap()), clock: ManualClock::new(1_000) };
        for h in ["ada", "bob", "cyd", "dan", "eve", "fay"] {
            a.dev_user(h).unwrap();
        }
        // with any number of users there's no seat to hand out: on the list, waiting, in order
        assert_eq!(a.wait_for("x@x.dev", "site").unwrap(), Waiting { joined: false, invited: false, position: 1 });
        assert_eq!(a.wait_for("y@x.dev", "site").unwrap().position, 2);
    }
}
