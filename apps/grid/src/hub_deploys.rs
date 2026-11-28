//! A hub's deploys (ADR-0147 §5): one at a time, each kept with who made it, from where, what it changed and how it
//! ended. The edge opens one when `resources.apply` reaches a hub's wisp and closes it with the box's answer.
//!
//! The lock is the row itself: a deploy still `running` holds the hub; a second is refused with 409 and the first's
//! number and author. One that never closed (the grid restarted, the box died) gives the hub back after
//! [`STALE_MS`] and is kept as `abandoned`.
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::error::{Result, bad};
use crate::teams::Teams;

/// A deploy that hasn't closed in this long gives the hub back.
pub const STALE_MS: i64 = 10 * 60_000;
/// The source a client names (`x-wisp-deploy-source`: a repo and commit), kept to this many characters.
pub const SOURCE_MAX: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployView {
    pub n: i64,
    /// `key` or `person`
    pub by_kind: String,
    pub by_name: String,
    pub source: String,
    /// `running`, `done`, `failed` or `abandoned`
    pub state: String,
    pub summary: Value,
    pub error: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

/// Who is deploying.
#[derive(Debug, Clone)]
pub struct By {
    pub kind: &'static str,
    pub name: String,
}

/// A source as a client named it: one line, trimmed, capped.
pub fn clean_source(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(SOURCE_MAX).collect::<String>().trim().to_owned()
}

/// What an apply's answer changed, counted: `{created, updated, deleted}` from `/api/cmd`'s
/// `{ok, result: {plan: {ops: [{op: create|update|delete}]}, applied}}` (wisp-resources-api `Applied`).
pub fn counts(answer: &Value) -> Value {
    let ops = answer.pointer("/result/plan/ops").and_then(Value::as_array);
    let n = |op: &str| ops.map_or(0, |o| o.iter().filter(|x| x.get("op").and_then(Value::as_str) == Some(op)).count());
    json!({ "created": n("create"), "updated": n("update"), "deleted": n("delete") })
}

fn view(r: &rusqlite::Row<'_>) -> rusqlite::Result<DeployView> {
    let summary: String = r.get(5)?;
    Ok(DeployView {
        n: r.get(0)?,
        by_kind: r.get(1)?,
        by_name: r.get(2)?,
        source: r.get(3)?,
        state: r.get(4)?,
        summary: serde_json::from_str(&summary).unwrap_or(Value::Null),
        error: r.get(6)?,
        started_at: r.get(7)?,
        finished_at: r.get(8)?,
    })
}

const COLS: &str = "n, by_kind, by_name, source, state, summary, error, started_at, finished_at";

impl Teams {
    /// Opens deploy n+1 for the team, or 409 while another runs. A stale one is closed as `abandoned` first.
    pub fn deploy_begin(&self, team_id: &str, by: &By, source: &str) -> Result<i64> {
        let now = self.clock.now_ms();
        self.db.tx(|tx| {
            tx.execute(
                "UPDATE team_deploys SET state = 'abandoned', finished_at = ?2 WHERE team_id = ?1 AND state = 'running' AND started_at < ?3",
                params![team_id, now, now - STALE_MS],
            )?;
            let running: Option<(i64, String)> = tx
                .query_row("SELECT n, by_name FROM team_deploys WHERE team_id = ?1 AND state = 'running'", [team_id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?;
            if let Some((n, who)) = running {
                return Err(bad(format!("Deploy #{n} by {who} is running: try again when it's done."), 409));
            }
            let n: i64 = tx.query_row("SELECT coalesce(max(n), 0) + 1 FROM team_deploys WHERE team_id = ?1", [team_id], |r| r.get(0))?;
            tx.execute(
                "INSERT INTO team_deploys (team_id, n, by_kind, by_name, source, state, started_at) VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6)",
                params![team_id, n, by.kind, by.name, clean_source(source), now],
            )?;
            Ok(n)
        })
    }

    /// Closes deploy `n`: `done` with what it changed, or `failed` with why. A deploy that changed nothing is dropped,
    /// so the history lists what changed the hub (a plan's dry `apply`, a CI run on an unchanged folder).
    pub fn deploy_end(&self, team_id: &str, n: i64, outcome: std::result::Result<Value, String>) -> Result<()> {
        let now = self.clock.now_ms();
        self.db.with(|c| match outcome {
            Ok(summary) if summary.as_object().is_some_and(|o| o.values().all(|v| v.as_i64() == Some(0))) => {
                c.execute("DELETE FROM team_deploys WHERE team_id = ?1 AND n = ?2 AND state = 'running'", params![team_id, n])
            }
            Ok(summary) => c.execute(
                "UPDATE team_deploys SET state = 'done', summary = ?3, finished_at = ?4 WHERE team_id = ?1 AND n = ?2",
                params![team_id, n, summary.to_string(), now],
            ),
            Err(why) => c.execute(
                "UPDATE team_deploys SET state = 'failed', error = ?3, finished_at = ?4 WHERE team_id = ?1 AND n = ?2",
                params![team_id, n, why.chars().take(500).collect::<String>(), now],
            ),
        })?;
        Ok(())
    }

    /// A team's deploys, newest first (at most `limit`, up to 100): anyone in the team reads them.
    pub fn deploys(&self, team_id: &str, limit: i64) -> Result<Vec<DeployView>> {
        self.db.with(|c| {
            c.prepare_cached(&format!("SELECT {COLS} FROM team_deploys WHERE team_id = ?1 ORDER BY n DESC LIMIT ?2"))?
                .query_map(params![team_id, limit.clamp(1, 100)], view)?
                .collect()
        })
    }
}

#[cfg(test)]
#[path = "hub_deploys_tests.rs"]
mod tests;
