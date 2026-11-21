//! The compaction audit (ADR-0096, setting `threads.compactAudit`, off by default): after a turn that compacted (a
//! `/compact`, or the harness compacting on its own), a copy of the compacted conversation is asked what it still
//! remembers of the goal, constraints, decisions and what failed (`runtime.probe`: answering only, kept nowhere);
//! each Notes item is kept or lost by a word match (no model call); what was lost goes in the next follow-up's note.

use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use serde_json::{Value, json};
use wisp_kernel::{Call, Ctx, Result};
use wisp_runtime_api::commands::{Probe, ProbeArgs, Turns, TurnsArgs};
use wisp_runtime_api::model::{Turn, TurnStatus};
use wisp_store::Query;
use wisp_threads_api::{Audit, AuditStatus, SETTING_COMPACT_AUDIT};
use wisp_threads_core::audit::{RECALL, items, score};
use wisp_threads_core::notes::Shown;

use crate::{State, db, notes, store};

/// An audit still `running` this long after it started was cut off (a restart): it reads as skipped.
const STALE_MS: i64 = 10 * 60_000;

/// The tasks being audited now: one at a time each.
#[derive(Debug, Default)]
pub struct Busy(Mutex<HashSet<String>>);

impl Busy {
    fn take(&self, task: &str) -> bool {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).insert(task.to_owned())
    }
    fn give(&self, task: &str) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).remove(task);
    }
}

/// How many times the turn's conversation compacted: its harness's count (`stats.compactions`: Claude Code's
/// `compact_boundary`), else the count its compaction hook left (`ext["wisp.threads"].compactions`: Codex, ADR-0097).
pub fn compactions(t: &Turn) -> u32 {
    t.stats.as_ref().and_then(|s| s.compactions).filter(|n| *n > 0).unwrap_or_else(|| {
        t.ext
            .get(crate::PLUGIN_ID)
            .and_then(|x| x.get("compactions"))
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0)
    })
}

/// The turn compacted: a `/compact` that did, or a turn of the work the harness compacted inside.
pub fn compacted(t: &Turn) -> bool {
    t.status == TurnStatus::Done && compactions(t) > 0
}

/// The compactions a harness's hook counted during the turn (`wisp hook compacted` lines in the run's folder), kept
/// on the turn (`ext["wisp.threads"].compactions`, beside what the slot had) and the file removed. How many.
pub async fn take_marked(ctx: &Ctx, call: &Call, st: &State, task: &str, turn: &str) -> u32 {
    let file = st.out.join(crate::parts::folder_name(task)).join(wisp_threads_core::offload::COMPACTIONS_FILE);
    let Ok(text) = tokio::fs::read_to_string(&file).await else { return 0 };
    let _ = tokio::fs::remove_file(&file).await;
    let n = u32::try_from(text.lines().filter(|l| !l.trim().is_empty()).count()).unwrap_or(u32::MAX);
    if n == 0 {
        return 0;
    }
    let turns = ctx.run::<Turns>(call, TurnsArgs { id: task.into(), before: None, limit: Some(3), masked: None }).await.unwrap_or_default();
    let mut slot =
        turns.iter().find(|t| t.id.as_str() == turn).and_then(|t| t.ext.get(crate::PLUGIN_ID)).cloned().unwrap_or_else(|| json!({}));
    if let Some(o) = slot.as_object_mut() {
        o.insert("compactions".into(), json!(n));
    }
    let mark =
        wisp_runtime_api::commands::TurnExtArgs { id: task.into(), turn_id: turn.into(), plugin: crate::PLUGIN_ID.into(), value: slot };
    if let Err(e) = ctx.run::<wisp_runtime_api::commands::TurnSetExt>(call, mark).await {
        tracing::warn!(error = %e.message, "threads: the turn's compactions aren't kept");
    }
    n
}

pub fn on(ctx: &Ctx) -> bool {
    ctx.setting(SETTING_COMPACT_AUDIT).and_then(|v| v.as_bool()) == Some(true)
}

async fn save(st: &State, a: &Audit) -> Result<()> {
    let row = store::row(json!({ "id": format!("{}|{}", a.task, a.turn), "task_id": a.task, "at": a.at, "body": a }));
    st.store.write(move |w| w.upsert(store::AUDITS.kind, row).map(drop)).await.map_err(|e| db(&e))
}

/// A task's audits, newest first (a `running` one a restart cut off reads as skipped).
pub async fn list(st: &State, task: &str, now: i64) -> Result<Vec<Audit>> {
    let t = task.to_owned();
    let rows =
        st.store.read_async(move |r| r.query(store::AUDITS.kind, &Query::new().eq("task_id", t.as_str()))).await.map_err(|e| db(&e))?;
    let mut out: Vec<Audit> = rows.iter().filter_map(|r| serde_json::from_value(r.get("body")?.clone()).ok()).collect();
    for a in &mut out {
        if a.status == AuditStatus::Running && now - a.at > STALE_MS {
            a.status = AuditStatus::Skipped;
            a.why = Some("It didn’t finish.".into());
        }
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.at));
    Ok(out)
}

/// What the next follow-up's note says was lost (the latest done audit not told yet), marked told.
pub async fn take_lost(st: &State, task: &str, now: i64) -> Vec<String> {
    let Ok(all) = list(st, task, now).await else { return Vec::new() };
    let Some(mut a) = all.into_iter().find(|a| a.status == AuditStatus::Done) else { return Vec::new() };
    if a.told || a.lost.is_empty() {
        return Vec::new();
    }
    a.told = true;
    if let Err(e) = save(st, &a).await {
        tracing::warn!(error = %e.message, "threads: audit not marked told");
    }
    a.lost
}

/// At a turn's end: when it compacted and the audit is on, audit it (in the background, one at a time per task).
/// `marked`: compactions the harness's hook counted this turn ([`take_marked`]).
pub async fn after_turn(ctx: &Ctx, call: &Call, st: &std::sync::Arc<State>, task: &str, turn: &str, marked: u32) {
    if !on(ctx) {
        return;
    }
    let Ok(turns) = ctx.run::<Turns>(call, TurnsArgs { id: task.into(), before: None, limit: Some(3), masked: None }).await else { return };
    let Some(t) = turns.iter().find(|t| t.id.as_str() == turn) else { return };
    if !(compacted(t) || (marked > 0 && t.status == TurnStatus::Done)) {
        return;
    }
    let (st, ctx2, task, turn) = (st.clone(), ctx.clone(), task.to_owned(), turn.to_owned());
    ctx.spawn(async move {
        if !st.busy.take(&task) {
            return;
        }
        let call = ctx2.root_call();
        if let Err(e) = run(&ctx2, &call, &st, &task, &turn).await {
            tracing::warn!(error = %e.message, task = %task, "threads: compaction audit");
        }
        st.busy.give(&task);
    });
}

/// One audit: ask, score, save.
pub async fn run(ctx: &Ctx, call: &Call, st: &State, task: &str, turn: &str) -> Result<Audit> {
    let (n, _) = notes::load(&st.store, task).await.map_err(|e| db(&e))?;
    let shown: Vec<Shown<'_>> = n.sections.iter().map(|s| Shown { id: &s.id, text: &s.text }).collect();
    let list = items(&shown);
    let mut a = Audit {
        task: task.to_owned(),
        turn: turn.to_owned(),
        total: u32::try_from(list.len()).unwrap_or(0),
        at: crate::now(ctx),
        ..Audit::default()
    };
    if list.is_empty() {
        return Ok(a); // nothing to check: no row
    }
    save(st, &a).await?;
    match ctx.run::<Probe>(call, ProbeArgs { id: task.into(), question: RECALL.into(), timeout_ms: Some(120_000) }).await {
        Ok(p) => {
            let (kept, lost) = score(&list, &p.text);
            a.status = AuditStatus::Done;
            a.kept = kept;
            a.lost = lost;
        }
        Err(e) => {
            a.status = AuditStatus::Skipped;
            a.why = Some(e.message);
        }
    }
    save(st, &a).await?;
    // it ran in the background: say it's over (a watcher, a test, never a poll)
    ctx.emit(call, &wisp_threads_api::Audited { audit: a.clone() });
    Ok(a)
}

/// The value a turn's `ext` keeps of a fresh thread's start (either plugin's mark).
pub fn fresh_mark(t: &Turn) -> bool {
    [crate::PLUGIN_ID, "wisp.runtime"].iter().any(|p| t.ext.get(*p).and_then(|x| x.get("fresh")) == Some(&Value::Bool(true)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// v1 set a cut-off audit to skipped only in memory: a restart left it `running` for good (docs/v1-bugs-fixed.md).
    #[tokio::test]
    async fn an_audit_a_restart_cut_off_reads_as_skipped() {
        let store = wisp_store::Store::open(wisp_store::Location::Memory, std::sync::Arc::new(wisp_store::SystemClock)).unwrap();
        store::install(&store).unwrap();
        let st = State { store, out: std::env::temp_dir(), busy: Busy::default(), windows: crate::health::Windows::default() };
        let a = Audit { task: "t1".into(), turn: "u1".into(), total: 3, at: 1_000, ..Audit::default() };
        save(&st, &a).await.unwrap();
        assert_eq!(list(&st, "t1", 1_000 + 60_000).await.unwrap()[0].status, AuditStatus::Running, "still asking a minute on");
        let late = list(&st, "t1", 1_000 + STALE_MS + 1).await.unwrap();
        assert_eq!((late[0].status, late[0].why.as_deref()), (AuditStatus::Skipped, Some("It didn’t finish.")));
        assert!(take_lost(&st, "t1", 1_000).await.is_empty(), "nothing done: nothing to tell");
    }
}
