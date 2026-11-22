//! A crew job's turn end (v1 crew.js `#turnDone`, `#ended`, `#sendFollowUps`):
//! - `prepare` runs synchronously inside `runtime` before the turn is persisted: it strips `{"type":"crew"}` blocks
//!   out of `turn.presentation`, promotes `crew.topic`, and keeps the extracted `CrewOut` on `turn.ext["wisp.crew"]`
//!   (unless the job is capped: v1 crew.js:967).
//! - `outcome` tells `runtime` whether the job lands quiet (archived), filed (Done), or For later.
//! - `report` runs once the turn ended (locally on durable `TaskFinished`/`TaskFailed`, or from `remote.rs` for a
//!   sandbox job): it applies memory (`notebook`, `remember`, `wakeIn`), kit offers, the member's `log`, follow-ups
//!   to linked tasks (joining their delegates), the answer under any question that asked, `crew.reported`, and drains
//!   the member's queue.

use serde_json::{Value, json};
use wisp_crew_api::{CrewExt, Member, PLUGIN_ID, TaskRef, TeamRow};
use wisp_crew_core::filing::{TaskSide, file_turn, follow_up_targets, follow_up_text, join_team};
use wisp_crew_core::kit::Ids as _;
use wisp_crew_core::memory::{CrewOut, LogInput, apply_memory, log_entry, take_crew_block};
use wisp_crew_core::offers::offer_from_job;
use wisp_crew_core::queue::push_log;
use wisp_kernel::{Call, Ctx};
use wisp_runtime_api::model::{Presentation, Session, Turn, TurnStatus};
use wisp_runtime_api::points::Outcome;

use crate::{NewIds, State, answers, calls, jobs};

const CREW_OUT_KEY: &str = "crewOut";

/// Whether a task runs under a channel/lead cap (v1 `session.capped`): its crew block still goes,
/// but writes nothing to the member's memory, kit, follow-ups or filing.
fn is_capped_session(s: &Session) -> bool {
    s.ext.get("wisp.runtime").and_then(|r| r.get("capped")).is_some_and(|v| !v.is_null())
}

fn is_capped_json(s: &Value) -> bool {
    s.pointer("/ext/wisp.runtime/capped").is_some_and(|v| !v.is_null())
}

/// The task has follow-ups or a reminder still to come (wisp.later's mirror on the task).
fn scheduled(s: &Session) -> bool {
    // read by key, not by the full row: a mirror written by a newer wisp.later still counts
    let Some(l) = s.ext.get(wisp_later_api::PLUGIN_ID) else { return false };
    l.get("later").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) || l.get("reminder").is_some_and(|r| !r.is_null())
}

/// Extracts `CrewOut` from a turn: prefers the `crewOut` `prepare` kept on `turn.ext["wisp.crew"]`,
/// falling back to `take_crew_block` on a clone of `turn.presentation` (for turns saved before `prepare`).
pub(crate) fn crew_out_of(turn: &Turn, capped: bool) -> Option<CrewOut> {
    if capped {
        return None;
    }
    if let Some(v) = turn.ext.get(PLUGIN_ID).and_then(|slot| slot.get(CREW_OUT_KEY))
        && let Ok(out) = serde_json::from_value::<CrewOut>(v.clone())
    {
        return Some(out);
    }
    let mut p = turn.presentation.as_ref().and_then(|p| serde_json::to_value(p).ok())?;
    take_crew_block(&mut p)
}

/// Strips `{"type":"crew"}` blocks from `turn.presentation` in place and keeps `CrewOut` on
/// `turn.ext["wisp.crew"]["crewOut"]` (unless the job is capped). Runs on the job's own machine,
/// including sandboxes where the member itself doesn't live.
pub(crate) fn prepare(s: &Session, turn: &mut Turn) {
    if s.ext.get(PLUGIN_ID).and_then(|v| serde_json::from_value::<CrewExt>(v.clone()).ok()).is_none() {
        return;
    }
    let Some(pres) = turn.presentation.as_ref() else { return };
    let Ok(mut p) = serde_json::to_value(pres) else { return };
    let Some(data) = take_crew_block(&mut p) else { return };
    if let Ok(cleaned) = serde_json::from_value::<Presentation>(p) {
        turn.presentation = Some(cleaned);
    }
    if is_capped_session(s) {
        return;
    }
    if let Ok(val) = serde_json::to_value(&data) {
        let slot = turn.ext.entry(PLUGIN_ID.to_owned()).or_insert_with(|| json!({}));
        if !slot.is_object() {
            *slot = json!({});
        }
        if let Some(o) = slot.as_object_mut() {
            o.insert(CREW_OUT_KEY.to_owned(), val);
        }
    }
}

/// How a member's job lands (v1 runner.js `#finishTurn` with `crewOut`): quiet (archived), For later, or
/// filed under Done, from its crew block. A quiet job that scheduled a follow-up is filed, not archived (#760).
pub(crate) fn outcome(s: &Session, turn: &Turn) -> Option<Outcome> {
    let job = s.ext.get(PLUGIN_ID).and_then(|v| serde_json::from_value::<CrewExt>(v.clone()).ok())?;
    if turn.is_error || matches!(turn.status, TurnStatus::Error | TurnStatus::Stopped) || turn.by.as_deref() == Some("compact") {
        return None;
    }
    let data = crew_out_of(turn, is_capped_session(s));
    let f = file_turn(job.job, turn.idx as usize + 1, data.as_ref()).keeping(scheduled(s));
    Some(Outcome { quiet: f.quiet, filed: f.filed, later: f.later })
}

/// A member's job's turn ended (here, or in a sandbox: `remote.rs`): updates the member's notebook, wake-up,
/// kit offers and log, sends any `followUp` messages to linked tasks, fills in any answer under the question
/// that asked, emits `crew.reported`, and drains the member's queue.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn report(ctx: &Ctx, call: &Call, st: &State, m: &Member, job: &str, last: Option<&Turn>, text: &str, failed: bool) {
    if last.is_some_and(|t| t.by.as_deref() == Some("compact")) {
        return;
    }
    let task_json = jobs::job_task(ctx, call, job).await.ok();
    let ext = task_json
        .as_ref()
        .and_then(|s| s.pointer(&format!("/ext/{PLUGIN_ID}")))
        .and_then(|v| serde_json::from_value::<CrewExt>(v.clone()).ok())
        .unwrap_or_else(|| CrewExt { id: m.id.clone(), ..CrewExt::default() });
    let capped = task_json.as_ref().is_some_and(is_capped_json);
    let archived = task_json.as_ref().and_then(|s| s.get("archived")).and_then(Value::as_bool).unwrap_or(false);
    let for_later = task_json.as_ref().and_then(|s| s.pointer("/ext/wisp.runtime/forLater/by")).and_then(Value::as_str) == Some("crew");

    let mut p = last.and_then(|t| t.presentation.as_ref()).and_then(|p| calls::to_json(p).ok()).unwrap_or_default();
    let from_block = take_crew_block(&mut p);
    let data = if failed || capped { None } else { last.and_then(|t| crew_out_of(t, false)).or(from_block) };
    let quiet = !failed && (data.as_ref().is_some_and(|d| d.quiet) || (archived && ext.job.nobody_waits())) && !for_later;
    let at = jobs::now(ctx);

    // Apply memory (notebook/remember/wakeIn), kit offers, and job log in one member update.
    let updated_member = {
        let Ok(mut member) = st.member(&m.id) else { return };
        if let Some(d) = &data {
            apply_memory(&mut member, d, at);
            if !d.kit.is_empty() {
                offer_from_job(&mut member, job, &d.kit, NewIds.next_id(), at);
            }
        }
        let turn_status = if failed {
            "error"
        } else if last.is_some_and(|t| t.status == TurnStatus::Stopped) {
            "stopped"
        } else {
            "done"
        };
        let entry = log_entry(&LogInput {
            session_id: job,
            job: ext.job,
            from: ext.from.as_ref(),
            turn_status,
            prompt: last.map_or("", |t| t.prompt.as_str()),
            presentation: &p,
            quiet: !failed && archived && ext.job.nobody_waits(),
            for_later,
            at: last.and_then(|t| t.ended_at).unwrap_or(at),
        });
        push_log(&mut member, entry);
        st.put(member.clone());
        member
    };
    st.save().await;
    ctx.emit(call, &wisp_crew_api::events::MemberChanged { crew_id: updated_member.id.clone() });

    if let Some(d) = data.as_ref().filter(|d| !d.follow_ups.is_empty()) {
        send_follow_ups(ctx, call, &updated_member, &ext, job, &d.follow_ups).await;
    }

    // asked in another task: the answer goes under the message that asked
    let said = last.and_then(|t| t.text.clone()).unwrap_or_else(|| text.to_owned());
    let said = wisp_crew_core::answers::answer_text(&said, &p);
    answers::answered(ctx, call, st, job, &said, failed).await;
    let status = if failed { "error" } else { "done" };
    ctx.emit(
        call,
        &wisp_crew_api::events::Reported {
            crew_id: updated_member.id.clone(),
            member: updated_member.name.clone(),
            session_id: job.to_owned(),
            status: status.into(),
            quiet,
            text: text.chars().take(500).collect(),
        },
    );
    Box::pin(jobs::drain_and_save(ctx, call, st, &updated_member.id)).await;
}

/// Sends each permitted `followUp` to its linked task, joining the member to that task's delegates
/// and recording the target in the job's `ext["wisp.crew"].sent` (v1 crew.js `#sendFollowUps`).
async fn send_follow_ups(ctx: &Ctx, call: &Call, m: &Member, ext: &CrewExt, job: &str, list: &[wisp_crew_core::memory::FollowUp]) {
    let targets = follow_up_targets(m, ext, list);
    for f in targets {
        let Ok(target) = calls::task(ctx, call, &f.task).await else { continue };
        let have: Vec<TeamRow> = target
            .ext
            .get("wisp.runtime")
            .and_then(|r| r.get("subagents"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let target_crew = target.ext.get(PLUGIN_ID);
        let helper = target.parent_id.is_some() || target_crew.and_then(|c| c.get("job")).and_then(Value::as_str) == Some("delegated");
        let crew_id = target_crew.and_then(|c| c.get("id")).and_then(Value::as_str);
        let joined = join_team(&have, TaskSide { helper, crew: crew_id }, m).and_then(|t| serde_json::to_value(t).ok());
        let text = follow_up_text(m, &f.text);
        if let Err(err) = calls::follow_up_with_team(ctx, call, &target.id.0, text, joined).await {
            tracing::warn!(task = %target.id, error = %err.message, "crew: follow-up");
            continue;
        }
        if !job.contains('|') {
            calls::warn(
                "recording crew follow-up target",
                calls::record_sent(ctx, call, job, TaskRef { id: target.id.0, title: target.title }).await,
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use wisp_crew_api::JobKind;

    fn crew_session(job: JobKind, capped: bool) -> Session {
        let mut s = Session::default();
        let ext = CrewExt { id: "m1".into(), job, ..CrewExt::default() };
        s.ext.insert(PLUGIN_ID.into(), serde_json::to_value(&ext).unwrap());
        if capped {
            s.ext.insert("wisp.runtime".into(), json!({ "capped": { "mode": "edits" } }));
        }
        s
    }

    #[test]
    fn prepare_strips_crew_block_promotes_topic_and_keeps_crew_out() {
        let s = crew_session(JobKind::Checkin, false);
        let mut turn = Turn {
            status: TurnStatus::Done,
            presentation: Some(
                serde_json::from_value(json!({
                    "v": 1,
                    "blocks": [
                        { "type": "markdown", "text": "Quiet day." },
                        { "type": "crew", "forLater": true, "topic": "Daily pain check", "remember": ["saw v1"] }
                    ]
                }))
                .unwrap(),
            ),
            ..Turn::default()
        };
        prepare(&s, &mut turn);
        let p = turn.presentation.as_ref().unwrap();
        assert_eq!(p.blocks.len(), 1, "crew block stripped: {:?}", p.blocks);
        assert_eq!(p.blocks[0]["type"], "markdown");
        assert_eq!(p.topic.as_deref(), Some("Daily pain check"));
        let out = crew_out_of(&turn, false).unwrap();
        assert!(out.for_later);
        assert_eq!(out.remember, vec!["saw v1"]);
        assert_eq!(outcome(&s, &turn), Some(Outcome { quiet: false, filed: false, later: true }));
    }

    #[test]
    fn capped_job_strips_crew_block_but_keeps_no_crew_out_and_files_normally() {
        let s = crew_session(JobKind::Checkin, true);
        let mut turn = Turn {
            status: TurnStatus::Done,
            presentation: Some(
                serde_json::from_value(json!({
                    "v": 1,
                    "blocks": [{ "type": "crew", "quiet": true, "remember": ["secret"] }]
                }))
                .unwrap(),
            ),
            ..Turn::default()
        };
        prepare(&s, &mut turn);
        let p = turn.presentation.as_ref().unwrap();
        assert_eq!(p.blocks.len(), 1);
        assert_eq!(p.blocks[0]["type"], "hero", "falls back to Nothing new hero");
        assert!(!turn.ext.contains_key(PLUGIN_ID), "capped writes no crewOut");
        assert_eq!(outcome(&s, &turn), Some(Outcome { quiet: false, filed: false, later: false }));
    }
}
