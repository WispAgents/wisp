//! What the grid keeps of each cloud wisp (`instances`, v1 grid/server.js `hostd.on('change')`): its state, when it
//! last started and stopped and why, its last activity, and its next scheduled task. Home reads it; a restart reads it
//! back: nothing runs after one, but a box with a task scheduled still wakes for it.
use rusqlite::params;

use crate::boxhost::{BoxChange, BoxState};
use crate::db::Db;
use crate::error::Result;
use crate::rows::BoxUser;

/// One change, kept (a sandbox's box, or one whose account is gone, has no row: nothing to keep).
pub fn record(db: &Db, e: &BoxChange, now: i64) -> Result<()> {
    let stopped = e.state == BoxState::Stopped;
    db.with(|c| {
        c.prepare_cached(
            "INSERT INTO instances (user_id, state, created_at, started_at, stopped_at, last_active_at, next_wake_at, stop_reason)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8 WHERE EXISTS (SELECT 1 FROM users WHERE id = ?1)
             ON CONFLICT(user_id) DO UPDATE SET state = excluded.state, started_at = coalesce(excluded.started_at, started_at),
               stopped_at = coalesce(excluded.stopped_at, stopped_at), last_active_at = coalesce(excluded.last_active_at, last_active_at),
               next_wake_at = excluded.next_wake_at, stop_reason = excluded.stop_reason",
        )?
        .execute(params![
            e.user.id,
            e.state.as_str(),
            now,
            e.started_at,
            if stopped { Some(e.stopped_at.unwrap_or(now)) } else { None },
            e.last_active_at,
            e.next_wake_at,
            if stopped { Some(e.event.as_str()) } else { None },
        ])
    })?;
    Ok(())
}

/// The grid stopped its boxes and is going: every row still running says so now, rather than wait on events the
/// grid won't be there to read.
pub fn grid_stopped(db: &Db, now: i64) -> Result<()> {
    db.with(|c| {
        c.execute("UPDATE instances SET state = 'stopped', stopped_at = ?1, stop_reason = 'grid stopped' WHERE state != 'stopped'", [now])
    })?;
    Ok(())
}

/// After a restart nothing runs: every row says stopped, and these boxes wake for a task scheduled while it was down.
pub fn on_start(db: &Db) -> Result<Vec<(BoxUser, i64)>> {
    db.with(|c| {
        c.execute("UPDATE instances SET state = 'stopped' WHERE state != 'stopped'", [])?;
        let mut st = c.prepare(
            "SELECT u.id, u.handle, i.next_wake_at FROM instances i JOIN users u ON u.id = i.user_id WHERE i.next_wake_at IS NOT NULL AND u.disabled = 0",
        )?;
        let rows = st.query_map([], |r| {
            Ok((BoxUser { id: r.get(0)?, handle: r.get(1)?, sandbox: None, owner: None }, r.get::<_, i64>(2)?))
        })?;
        rows.collect()
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::boxhost::BoxUserWire;

    fn change(state: BoxState, event: &str, started: Option<i64>, stopped: Option<i64>, next: Option<i64>) -> BoxChange {
        BoxChange {
            user: BoxUserWire { id: "u1".into(), handle: "ada".into(), sandbox: None, owner: None },
            state,
            event: event.into(),
            started_at: started,
            stopped_at: stopped,
            next_wake_at: next,
            last_active_at: Some(150),
            error: String::new(),
        }
    }

    fn row(db: &Db) -> (String, Option<i64>, Option<i64>, Option<i64>, Option<String>) {
        db.with(|c| {
            c.query_row("SELECT state, started_at, stopped_at, next_wake_at, stop_reason FROM instances WHERE user_id = 'u1'", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
        })
        .unwrap()
    }

    #[test]
    fn a_box_s_start_and_sleep_are_kept_with_its_schedule() {
        let db = Db::memory().unwrap();
        // no account: nothing kept (a sandbox's box, or a user deleted while it stopped)
        record(&db, &change(BoxState::Running, "woke", Some(100), None, None), 100).unwrap();
        assert_eq!(db.with(|c| c.query_row("SELECT count(*) FROM instances", [], |r| r.get::<_, i64>(0))).unwrap(), 0);
        db.with(|c| c.execute("INSERT INTO users (id, handle, created_at) VALUES ('u1', 'ada', 0)", [])).unwrap();
        record(&db, &change(BoxState::Running, "woke", Some(100), None, None), 100).unwrap();
        assert_eq!(row(&db), ("running".into(), Some(100), None, None, None));
        // asleep: when and why, its schedule; when it started stays
        record(&db, &change(BoxState::Stopped, "idle", Some(100), Some(500), Some(9_000)), 500).unwrap();
        assert_eq!(row(&db), ("stopped".into(), Some(100), Some(500), Some(9_000), Some("idle".into())));
        // started again: the new start, the last stop kept, the schedule its own again
        record(&db, &change(BoxState::Running, "woke", Some(800), Some(500), None), 800).unwrap();
        assert_eq!(row(&db), ("running".into(), Some(800), Some(500), None, None));
    }

    #[test]
    fn a_grid_that_stops_says_its_boxes_stopped() {
        let db = Db::memory().unwrap();
        db.with(|c| c.execute("INSERT INTO users (id, handle, created_at) VALUES ('u1', 'ada', 0)", [])).unwrap();
        record(&db, &change(BoxState::Running, "woke", Some(100), None, Some(9_000)), 100).unwrap();
        grid_stopped(&db, 700).unwrap();
        assert_eq!(row(&db), ("stopped".into(), Some(100), Some(700), Some(9_000), Some("grid stopped".into())));
    }

    #[test]
    fn after_a_restart_nothing_runs_and_scheduled_boxes_wake() {
        let db = Db::memory().unwrap();
        db.with(|c| {
            c.execute("INSERT INTO users (id, handle, created_at) VALUES ('u1', 'ada', 0), ('u2', 'bob', 0)", [])?;
            c.execute("INSERT INTO instances (user_id, state, created_at, next_wake_at) VALUES ('u1', 'running', 0, 9000), ('u2', 'running', 0, NULL)", [])
        })
        .unwrap();
        let wakes = on_start(&db).unwrap();
        assert_eq!(wakes.iter().map(|(u, at)| (u.handle.as_str(), *at)).collect::<Vec<_>>(), vec![("ada", 9000)]);
        let states: Vec<String> = db.with(|c| c.prepare("SELECT state FROM instances")?.query_map([], |r| r.get(0))?.collect()).unwrap();
        assert!(states.iter().all(|s| s == "stopped"), "{states:?}");
    }
}
