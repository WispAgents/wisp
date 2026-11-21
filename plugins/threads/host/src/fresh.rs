//! Fresh thread (ADR-0096): a new conversation on the same task, from a brief you can read and edit first
//! (`threads.freshBrief`), started by runtime's own `runtime.followUp { fresh: true }` (any plugin has it: no private
//! API). The old conversation stays linked on the new turn (`ext["wisp.runtime"].after`); health counts from it.

use serde_json::{Value, json};
use wisp_kernel::{Call, Ctx, Result, WireError};
use wisp_runtime_api::commands::{FollowUpArgs, Get, IdArgs, TurnExtArgs, TurnSetExt, Turns, TurnsArgs};
use wisp_runtime_api::model::{FileChange, Session, Turn, TurnStatus};
use wisp_threads_api::{Brief, Fresh, FreshArgs};
use wisp_threads_core::fresh::{BRIEF_MAX, BriefIn, Recent, brief, clip};

use crate::{State, db, notes};

/// The turns read for a brief.
const LOOK_BACK: u32 = 200;

fn changed_files(turns: &[Turn]) -> Vec<String> {
    turns
        .iter()
        .flat_map(|t| {
            let changes: Vec<FileChange> = t
                .ext
                .get("wisp.runtime")
                .and_then(|r| r.get("changes"))
                .cloned()
                .and_then(|c| serde_json::from_value(c).ok())
                .unwrap_or_default();
            changes.into_iter().map(|c| c.file)
        })
        .collect()
}

fn branch(s: &Session) -> Option<String> {
    s.ext.get("wisp.worktrees").and_then(|w| w.get("branch")).and_then(Value::as_str).map(str::to_owned)
}

/// A subagent's thread is its lead's.
async fn own_task(ctx: &Ctx, call: &Call, task: &str) -> Result<Session> {
    let s = ctx.run::<Get>(call, IdArgs { id: task.into() }).await?;
    if s.parent_id.is_some() {
        return Err(WireError::invalid("A subagent’s thread is its lead’s: start a fresh thread there."));
    }
    Ok(s)
}

/// `threads.freshBrief`: the brief, made now; nothing is written.
pub async fn make_brief(ctx: &Ctx, call: &Call, st: &State, task: &str) -> Result<Brief> {
    let s = own_task(ctx, call, task).await?;
    let mut turns = ctx.run::<Turns>(call, TurnsArgs { id: task.into(), before: None, limit: Some(LOOK_BACK), masked: None }).await?;
    turns.sort_by_key(|t| t.idx);
    let done: Vec<&Turn> = turns
        .iter()
        .filter(|t| t.status != TurnStatus::Running && t.status != TurnStatus::Held && t.by.as_deref() != Some("compact"))
        .collect();
    let (n, _) = notes::load(&st.store, task).await.map_err(|e| db(&e))?;
    let md = notes::md(&n, &[], usize::MAX, crate::parts::TOOL);
    let first = turns.first().map(|t| t.prompt.clone()).unwrap_or_default();
    let files = changed_files(&turns);
    let recent: Vec<Recent<'_>> = done
        .iter()
        .rev()
        .take(2)
        .rev()
        .map(|t| Recent { n: t.idx + 1, prompt: &t.prompt, answer: t.text.as_deref().unwrap_or_default() })
        .collect();
    let br = branch(&s);
    let text =
        brief(&BriefIn { notes: &md, first: &first, files: &files, branch: br.as_deref(), recent: &recent, tool: crate::parts::TOOL });
    let mut listed: Vec<&str> = Vec::new();
    for f in &files {
        if !listed.contains(&f.as_str()) {
            listed.push(f);
        }
    }
    Ok(Brief { task: task.to_owned(), text, notes_rev: n.rev, diff: (!listed.is_empty()).then(|| listed.join("\n")) })
}

/// `threads.fresh`: the brief (yours, or made now) as the first message of a new conversation on the task.
pub async fn start(ctx: &Ctx, call: &Call, st: &State, a: FreshArgs) -> Result<Fresh> {
    own_task(ctx, call, &a.id).await?;
    let text = match a.brief.as_deref().map(str::trim) {
        Some(b) if !b.is_empty() => clip(b, BRIEF_MAX),
        Some(_) => return Err(WireError::invalid("A fresh thread needs a brief: what the new conversation starts from.")),
        None => make_brief(ctx, call, st, &a.id).await?.text,
    };
    let (n, _) = notes::load(&st.store, &a.id).await.map_err(|e| db(&e))?;
    let out = ctx
        .run::<wisp_runtime_api::commands::FollowUp>(
            call,
            FollowUpArgs { id: a.id.as_str().into(), text, fresh: true, ..FollowUpArgs::default() },
        )
        .await?;
    // the brief has the Notes: the agent has read them
    if let Err(e) = notes::mark_seen(&st.store, &a.id, n.rev).await {
        tracing::warn!(error = %e, "threads: seen not kept");
    }
    if let Some(turn) = &out.turn_id {
        let mark = TurnExtArgs {
            id: a.id.as_str().into(),
            turn_id: turn.clone(),
            plugin: crate::PLUGIN_ID.into(),
            value: json!({ "fresh": true, "notesRev": n.rev }),
        };
        if let Err(e) = ctx.run::<TurnSetExt>(call, mark).await {
            tracing::warn!(error = %e.message, "threads: the fresh turn isn't marked (runtime's own mark stands)");
        }
    }
    Ok(Fresh { task: a.id, turn_id: out.turn_id.map(|t| t.0), queued: out.queued })
}
