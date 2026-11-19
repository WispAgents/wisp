//! "Add a cloud machine" (v1 grid/accounts.js `wishMachines`, `machineWish`, `machineWishes`): each account has one
//! cloud wisp for now, so asking for more puts the person on a list for what's next: more machines, a bigger one,
//! GPUs, with a line about what for. Kept in `machine_wishes`, one row per user, audited; the operator reads the list
//! with `wisp-grid waitlist`. An empty pick takes them off it.
use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::accounts::Accounts;
use crate::error::{Result, bad};

/// What there is to wait for, in v1's order.
pub const MACHINE_WANTS: [&str; 3] = ["more", "bigger", "gpu"];

/// What someone is waiting for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MachineWish {
    pub wants: Vec<String>,
    pub note: String,
    /// when they first asked
    pub since: i64,
}

/// One line of the operator's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WishRow {
    pub handle: String,
    pub email: Option<String>,
    pub wants: String,
    pub note: Option<String>,
    pub created_at: i64,
}

/// The picks as v1 took them: a list, or a comma-separated string; trimmed, without repeats, in the order given.
pub fn wants_of(v: &serde_json::Value) -> Vec<String> {
    let items: Vec<String> = match v {
        serde_json::Value::Array(a) => a.iter().map(|x| x.as_str().map_or_else(|| x.to_string(), str::to_owned)).collect(),
        serde_json::Value::String(s) => s.split(',').map(str::to_owned).collect(),
        _ => Vec::new(),
    };
    let mut out: Vec<String> = Vec::new();
    for w in items.iter().map(|x| x.trim()).filter(|x| !x.is_empty()) {
        if !out.iter().any(|o| o == w) {
            out.push(w.to_owned());
        }
    }
    out
}

impl Accounts {
    /// On the list for `wants` with `note`, or off it when `wants` is empty. What they now wait for.
    pub fn wish_machines(&self, user_id: &str, wants: &[String], note: &str) -> Result<Option<MachineWish>> {
        if let Some(odd) = wants.iter().find(|w| !MACHINE_WANTS.contains(&w.as_str())) {
            return Err(bad(format!("\"{odd}\" isn’t something to wait for: {}.", MACHINE_WANTS.join(", ")), 400));
        }
        let now = self.clock.now_ms();
        if wants.is_empty() {
            self.db.with(|c| {
                c.execute("DELETE FROM machine_wishes WHERE user_id = ?1", [user_id])?;
                crate::rows::find::audit(c, Some(user_id), "machines.unwish", None, now)
            })?;
            return Ok(None);
        }
        let note: String = note.trim().chars().take(500).collect();
        let joined = wants.join(",");
        self.db.with(|c| {
            c.execute(
                "INSERT INTO machine_wishes (user_id, wants, note, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(user_id) DO UPDATE SET wants = excluded.wants, note = excluded.note, updated_at = excluded.updated_at",
                params![user_id, joined, (!note.is_empty()).then_some(&note), now],
            )?;
            crate::rows::find::audit(c, Some(user_id), "machines.wish", Some(&serde_json::json!({ "wants": wants })), now)
        })?;
        self.machine_wish(user_id)
    }

    /// What this account is waiting for, if anything.
    pub fn machine_wish(&self, user_id: &str) -> Result<Option<MachineWish>> {
        self.db.with(|c| {
            c.query_row("SELECT wants, note, created_at FROM machine_wishes WHERE user_id = ?1", [user_id], |r| {
                Ok(MachineWish {
                    wants: r.get::<_, String>(0)?.split(',').map(str::to_owned).collect(),
                    note: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    since: r.get(2)?,
                })
            })
            .optional()
        })
    }

    /// Everyone waiting for more, oldest first (the operator's list).
    pub fn machine_wishes(&self) -> Result<Vec<WishRow>> {
        self.db.with(|c| {
            let mut st = c.prepare(
                "SELECT u.handle, u.email, m.wants, m.note, m.created_at FROM machine_wishes m JOIN users u ON u.id = m.user_id ORDER BY m.created_at",
            )?;
            let rows = st.query_map([], |r| {
                Ok(WishRow { handle: r.get(0)?, email: r.get(1)?, wants: r.get(2)?, note: r.get(3)?, created_at: r.get(4)? })
            })?;
            rows.collect()
        })
    }
}

/// A seat waitlist row: email, where they signed up, when, invited, joined.
type SeatRow = (String, String, i64, Option<i64>, Option<i64>);

/// `wisp-grid waitlist` (v1 `wisp grid waitlist`): who's waiting for a seat (and who has one held, to write to), then
/// the people with a seat who want more machines.
pub fn report(a: &Accounts) -> Result<String> {
    use std::fmt::Write as _;
    let day = |t: Option<i64>| crate::pages::iso_day(t);
    let seats: Vec<SeatRow> = a.db.with(|c| {
        let mut st = c.prepare("SELECT email, source, created_at, invited_at, joined_at FROM waitlist ORDER BY created_at")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        rows.collect()
    })?;
    let mut out = String::new();
    for (email, source, at, invited, joined) in &seats {
        let state = match (joined, invited) {
            (Some(j), _) => format!("joined {}", day(Some(*j))),
            (None, Some(i)) => format!("invited {}", day(Some(*i))),
            _ => "waiting".to_owned(),
        };
        let _ = writeln!(out, "{email:<36} {source:<7} {}  {state}", day(Some(*at)));
    }
    if seats.is_empty() {
        out.push_str("Nobody is waiting.\n");
    }
    let apps = a.app_waitlist()?;
    if !apps.is_empty() {
        out.push_str("\n  waiting for an app:\n");
        for (email, kind, platform, source, at) in &apps {
            let _ = writeln!(out, "{email:<36} {kind:<10} {platform:<8} {source:<7} {}", day(Some(*at)));
        }
    }
    let more = a.machine_wishes()?;
    if !more.is_empty() {
        out.push_str("\n  wanting more machines:\n");
        for w in &more {
            let note = w.note.as_deref().map(|n| format!("  “{n}”")).unwrap_or_default();
            let _ = writeln!(
                out,
                "{:<20} {:<30} {:<17} {}{note}",
                w.handle,
                w.email.as_deref().unwrap_or(""),
                w.wants,
                day(Some(w.created_at))
            );
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::clock::ManualClock;
    use crate::db::Db;

    #[test]
    fn a_wish_is_kept_changed_and_dropped_and_audited() {
        let clock = ManualClock::new(1_000);
        let a = Accounts { db: Arc::new(Db::memory().unwrap()), clock: clock.clone() };
        let ada = a.dev_user("ada").unwrap();
        assert_eq!(a.machine_wish(&ada.id).unwrap(), None);
        let w = a.wish_machines(&ada.id, &["gpu".into(), "more".into()], "  a 70B model  ").unwrap().unwrap();
        assert_eq!(w, MachineWish { wants: vec!["gpu".into(), "more".into()], note: "a 70B model".into(), since: 1_000 });
        // changed later: what they want now, since when they first asked
        clock.advance(5_000);
        let w = a.wish_machines(&ada.id, &["bigger".into()], "").unwrap().unwrap();
        assert_eq!((w.wants, w.note, w.since), (vec!["bigger".to_owned()], String::new(), 1_000));
        let list = a.machine_wishes().unwrap();
        assert_eq!((list.len(), list[0].handle.as_str(), list[0].wants.as_str()), (1, "ada", "bigger"));
        // something else is refused, and nothing changes
        let e = a.wish_machines(&ada.id, &["tpu".into()], "").unwrap_err();
        assert_eq!((e.status(), e.to_string()), (400, "\"tpu\" isn’t something to wait for: more, bigger, gpu.".to_owned()));
        // off the list
        assert_eq!(a.wish_machines(&ada.id, &[], "").unwrap(), None);
        assert_eq!(a.machine_wishes().unwrap(), Vec::new());
        let actions: Vec<String> =
            a.db.with(|c| c.prepare("SELECT action FROM audit ORDER BY id")?.query_map([], |r| r.get(0))?.collect()).unwrap();
        assert_eq!(actions, ["machines.wish", "machines.wish", "machines.unwish"]);
    }

    #[test]
    fn the_operator_s_list_shows_seats_waiting_and_who_wants_more() {
        let a = Accounts { db: Arc::new(Db::memory().unwrap()), clock: ManualClock::new(1_792_920_800_000) };
        assert_eq!(report(&a).unwrap(), "Nobody is waiting.\n");
        let ada = a.dev_user("ada").unwrap();
        a.db.with(|c| {
            c.execute("UPDATE users SET email = 'ada@x.dev' WHERE id = ?1", [&ada.id])?;
            c.execute(
                "INSERT INTO waitlist (email, source, created_at, invited_at) VALUES ('bob@x.dev', 'site', 1792920800000, 1793000000000)",
                [],
            )
        })
        .unwrap();
        a.wish_machines(&ada.id, &["gpu".into()], "a 70B model").unwrap();
        let r = report(&a).unwrap();
        assert!(r.starts_with("bob@x.dev                            site    2026-10-25  invited 2026-10-26\n"), "{r}");
        assert!(r.contains("\n  wanting more machines:\nada                  ada@x.dev                      gpu               2026-10-25  “a 70B model”\n"), "{r}");
    }

    #[test]
    fn picks_come_as_a_list_or_a_comma_string() {
        assert_eq!(wants_of(&serde_json::json!(["gpu", " more", "gpu", ""])), ["gpu", "more"]);
        assert_eq!(wants_of(&serde_json::json!("bigger, gpu")), ["bigger", "gpu"]);
        assert_eq!(wants_of(&serde_json::Value::Null), Vec::<String>::new());
    }
}
