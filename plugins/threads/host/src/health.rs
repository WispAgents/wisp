//! A thread's health at each turn's end (`turn.ended`), from runtime's own reads: its turns (compactions, failures,
//! context, edits) and the failed steps of the last three. Saved on the task's `session:<id>` topic; no model call.

use serde_json::json;
use wisp_kernel::{Call, Ctx, Result};
use wisp_runtime_api::commands::{ActivityArgs, ActivityOf, Turns, TurnsArgs};
use wisp_runtime_api::model::{Activity, FileChange, Turn, TurnStatus};
use wisp_threads_api::{Health, Level, Repeat};
use wisp_threads_core::health::{EditIn, TurnIn, health};

use crate::store;
use crate::{State, db, notes};

/// Turns read back for health: enough for the thresholds (60 turns is the most any counts).
const LOOK_BACK: u32 = 80;

fn edits(t: &Turn) -> Vec<EditIn> {
    let changes: Vec<FileChange> =
        t.ext.get("wisp.runtime").and_then(|r| r.get("changes")).cloned().and_then(|c| serde_json::from_value(c).ok()).unwrap_or_default();
    changes
        .iter()
        .flat_map(|c| c.edits.iter().map(|e| EditIn { file: c.file.clone(), before: e.before.clone(), after: e.after.clone() }))
        .collect()
}

/// Its answer says it didn't get there (a blocked or partial hero), or it ended in an error.
fn failed(t: &Turn) -> bool {
    t.status == TurnStatus::Error
        || t.presentation
            .as_ref()
            .and_then(|p| p.blocks.iter().find(|b| b["type"] == "hero"))
            .is_some_and(|h| matches!(h["status"].as_str(), Some("blocked" | "partial")))
}

pub fn turn_in(t: &Turn, errors: Vec<String>) -> TurnIn {
    let stats = t.stats.as_ref();
    TurnIn {
        compact: t.by.as_deref() == Some("compact"),
        held: t.status == TurnStatus::Held,
        failed: failed(t),
        fresh: crate::audit::fresh_mark(t),
        // a `/compact` turn is counted as one already
        compacted: if t.by.as_deref() == Some("compact") { 0 } else { crate::audit::compactions(t) },
        context: stats.and_then(|s| Some((s.context_tokens?, s.context_window?))),
        errors,
        edits: edits(t),
    }
}

/// Works the task's health out again and saves it.
pub async fn work_out(ctx: &Ctx, call: &Call, st: &State, task: &str) -> Result<Health> {
    let mut turns = ctx.run::<Turns>(call, TurnsArgs { id: task.into(), before: None, limit: Some(LOOK_BACK), masked: None }).await?;
    turns.sort_by_key(|t| t.idx);
    let total = turns.last().map_or(0, |t| t.idx + 1);
    let mut ins = Vec::with_capacity(turns.len());
    let last_work: Vec<usize> =
        turns.iter().enumerate().filter(|(_, t)| t.by.as_deref() != Some("compact")).map(|(i, _)| i).rev().take(3).collect();
    for (i, t) in turns.iter().enumerate() {
        let mut errors = Vec::new();
        if last_work.contains(&i)
            && let Ok(page) =
                ctx.run::<ActivityOf>(call, ActivityArgs { id: task.into(), turn_id: t.id.clone(), since_seq: None, masked: None }).await
        {
            errors = page
                .items
                .into_iter()
                .filter_map(|s| {
                    if let Activity::ToolError { name, text, .. } = s.activity {
                        Some(format!("{name} failed: {}", text.lines().next().unwrap_or_default()))
                    } else {
                        None
                    }
                })
                .collect();
        }
        ins.push(turn_in(t, errors));
    }
    let (_, m) = notes::load(&st.store, task).await.map_err(|e| db(&e))?;
    // the Notes' age: the task's turns now, less its turns when they were last written
    let h = health(task, &ins, (m.rev > 0).then(|| total.saturating_sub(m.turn)));
    save(st, &h).await?;
    Ok(h)
}

/// What the Thread tab and the TUI show of a thread's health: its level, what made it so, the error it repeats. The
/// counts behind them (turns, the context's share) move every turn; a row is written only when this changes, and a
/// thread with no row is Fine (ADR-0100: a write at every turn's end cost the stream's write amplification).
fn shown(h: &Health) -> (Level, &[String], Option<&Repeat>) {
    (h.level, &h.reasons, h.repeat.as_ref())
}

/// Saves `h` when what it shows differs from the kept row's (none kept: Fine, nothing to say), in one read-and-write.
async fn save(st: &State, h: &Health) -> Result<()> {
    let h = h.clone();
    st.store
        .write(move |w| {
            let kept: Option<Health> = w
                .get(store::HEALTH.kind, h.task.clone())?
                .and_then(|r| r.get("body").cloned())
                .and_then(|b| serde_json::from_value(b).ok());
            let was = kept.as_ref().map_or((Level::Ok, &[][..], None), shown);
            if was == shown(&h) {
                return Ok(());
            }
            w.upsert(store::HEALTH.kind, store::row(json!({ "id": h.task, "body": h }))).map(drop)
        })
        .await
        .map_err(|e| db(&e))
}

/// The health now, worked out from the task's turns (the kept row may be behind on the counts).
pub async fn read(ctx: &Ctx, call: &Call, st: &State, task: &str) -> Result<Health> {
    work_out(ctx, call, st, task).await
}
