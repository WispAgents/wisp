//! A thread's health at each turn's end (`turn.ended`), from runtime's own reads: its turns (compactions, failures,
//! context, edits) and the failed steps of the last three. Saved on the task's `session:<id>` topic; no model call.

use serde_json::{Value, json};
use wisp_kernel::{Call, Ctx, Result};
use wisp_runtime_api::commands::{ActivityArgs, ActivityOf, Turns, TurnsArgs};
use wisp_runtime_api::model::{Activity, FileChange, Turn, TurnStatus};
use wisp_threads_api::Health;
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
        fresh: t.ext.get(crate::PLUGIN_ID).and_then(|x| x.get("fresh")) == Some(&Value::Bool(true)),
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
    notes::count_turns(&st.store, task, total).await.map_err(|e| db(&e))?;
    let (_, m) = notes::load(&st.store, task).await.map_err(|e| db(&e))?;
    // the Notes' age: the task's turns now, less its turns when they were last written
    let h = health(task, &ins, (m.rev > 0).then(|| total.saturating_sub(m.turn)));
    save(st, &h).await?;
    Ok(h)
}

async fn save(st: &State, h: &Health) -> Result<()> {
    let row = store::row(json!({ "id": h.task, "body": h }));
    st.store.write(move |w| w.upsert(store::HEALTH.kind, row).map(drop)).await.map_err(|e| db(&e))
}

/// The health as last worked out; worked out now if it never was.
pub async fn read(ctx: &Ctx, call: &Call, st: &State, task: &str) -> Result<Health> {
    let t = task.to_owned();
    let kept = st.store.read_async(move |r| r.get(store::HEALTH.kind, t)).await.map_err(|e| db(&e))?;
    if let Some(h) = kept.and_then(|r| r.get("body").cloned()).and_then(|b| serde_json::from_value(b).ok()) {
        return Ok(h);
    }
    work_out(ctx, call, st, task).await
}
