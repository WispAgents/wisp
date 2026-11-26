//! Subagents on another machine (ADR-0059, v1 services/relay.js): the lead's work copied there (`machines.carry`,
//! a copy under that wisp's own folder, never the user's checkout), the task run there (`machines.route`), its report
//! read there and its changes brought into the lead's folder once per job (`machines.bring`). Each says where it ran.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{Delegation, Report};
use wisp_runtime_api::model::{Session, SessionState, TurnStatus};
use wisp_runtime_core::prompt::subagents::{AWAY_SUBAGENT, in_folder};

use super::{push, slot, str_at, with_slot};
use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;

/// The lead's delegations on other machines (`ext["wisp.runtime"].away`): key → the machine's name.
pub(super) const AWAY: &str = "away";
/// The jobs whose changes came back, as `<key>@<turn there>` (one bring per job, not per member): a machine member's
/// and a sandbox member's alike.
const BROUGHT: &str = "broughtJobs";
/// Before marks were per job, a sandbox member's were per member (`<key>`): read so that job isn't brought twice.
const BROUGHT_ONCE: &str = "brought";

/// A delegation that went to another machine (not a sandbox, not a crew job).
pub(super) fn on_machine(d: &Delegation) -> bool {
    d.error.is_none() && d.place.as_deref().is_some_and(|p| p != "sandbox" && p != "crew")
}

fn wire(e: wisp_kernel::WireError) -> RuntimeError {
    RuntimeError::Invalid(e.message)
}

/// What the machine says of a task there: its row, and the delegation's turn there (the one it started, or its last
/// when that isn't known; Null: not shown there yet).
#[derive(Clone)]
pub(crate) struct There {
    pub(crate) session: Value,
    pub(crate) state: SessionState,
    pub(crate) job: Value,
    /// the delegation's turn there is known (its record's `turnId`)
    pub(crate) known: bool,
}

impl There {
    /// The job's turn's status there (None: not shown yet), as `owed::far_open` reads it.
    pub(crate) fn job_status(&self) -> Option<TurnStatus> {
        serde_json::from_value(self.job.get("status")?.clone()).ok()
    }
}

/// What a bring answered (`machines.bring`, `sandbox.bring`): files touched, why they didn't go in, where they're kept.
struct Got {
    files: u32,
    error: Option<String>,
    kept: Option<String>,
}

/// Why a done far job's changes are held, not put in the lead's folder (None: they go in). A lead in Plan changes no
/// file, and only a job that finished has changes worth taking: a stopped or failed one's are kept for the user.
fn held_why(l: &Session, t: &There) -> Option<&'static str> {
    if l.mode == wisp_runtime_api::model::Mode::Plan {
        return Some("the lead is in Plan");
    }
    // a turn not shown there (an older wisp's answer) counts by the task's end, as its report does
    match t.job_status() {
        Some(TurnStatus::Stopped) => Some("the job stopped"),
        Some(TurnStatus::Error) => Some("the job failed"),
        Some(TurnStatus::Running | TurnStatus::Held) => Some("the job didn't finish"),
        Some(TurnStatus::Done) | None => None,
    }
}

/// The note a far job's report carries about its changes: how many came into the lead's folder, held, or kept on a
/// clash (with where, so the lead and the user can find them).
fn bring_note(got: &Got, held: Option<&str>) -> String {
    match (held, got) {
        (Some(why), Got { kept: Some(at), .. }) => format!("\n\n(Its changes are held: {why}. Kept at {at}.)"),
        (_, Got { kept: Some(at), .. }) => format!("\n\n(Its changes clash with yours: kept at {at}. Apply with git apply {at})"),
        (Some(why), Got { error: Some(e), .. }) => format!("\n\n(Its changes are held: {why}, but {e}.)"),
        (_, Got { error: Some(e), .. }) => format!("\n\n(Its changes didn't come back: {e})"),
        (None, Got { files, .. }) if *files > 0 => format!("\n\n(Its changes to {files} file(s) are in your folder now.)"),
        _ => String::new(),
    }
}

/// The record's note of what came back (`far_ahead` keeps it there for the report the lead gets).
pub(crate) const BROUGHT_NOTE: &str = "broughtNote";

impl Runtime {
    /// A lead deleted (ADR-0060): each of its delegates on another machine goes there too, its task and its copy
    /// (`machines.uncarry`; a machine that can't be reached keeps it for machines' sweep). Doesn't hold up the delete.
    pub(crate) fn let_away_go(self: &Arc<Self>, lead: &Session) {
        let Some(away) = slot(lead).and_then(|m| m.get(AWAY)).and_then(Value::as_object) else { return };
        for key in away.keys() {
            let Some((machine, task)) = key.split_once('|') else { continue };
            let a = wisp_machines_api::UncarryArgs { id: machine.to_owned(), lead: lead.id.0.clone(), task: task.to_owned() };
            let rt = self.clone();
            self.host.spawn(Box::pin(async move {
                if let Err(err) = crate::calls::run::<wisp_machines_api::Uncarry>(&*rt.host, a).await {
                    tracing::warn!(error = %err.message, "runtime: a deleted lead's delegate on another machine");
                }
            }));
        }
    }

    /// What machine `machine` says of task `id` there; `turn`: the delegation's turn there ("" = its last). None: it
    /// didn't answer (both asks must: a job read without its turns would look ended with no report).
    pub(crate) async fn there(&self, machine: &str, id: &str, turn: &str) -> Option<There> {
        let session = self.route(machine, "runtime.get", json!({ "id": id })).await.ok()?;
        let state = serde_json::from_value(session.get("state").cloned().unwrap_or_default()).unwrap_or(SessionState::Running);
        let turns = self.route(machine, "runtime.turns", json!({ "id": id })).await.ok()?;
        let list = turns.as_array().cloned().or_else(|| turns.get("turns").and_then(Value::as_array).cloned()).unwrap_or_default();
        let job = if turn.is_empty() { list.last() } else { list.iter().find(|t| str_at(t, "id") == turn) };
        Some(There { session, state, job: job.cloned().unwrap_or_default(), known: !turn.is_empty() })
    }

    /// A member whose place is machine `machine`: the lead's work carried there (afresh each job, after the last job's
    /// changes came back), and its task there, followed up or made. → (`<machine>|<task>`, the machine's name).
    pub(super) async fn machine_member(
        self: &Arc<Self>,
        lead: &Session,
        m: &Value,
        machine: &str,
        slot_name: &str,
        existing: Option<String>,
        prompt: &str,
    ) -> Result<(String, String, String)> {
        // a capped lead's work stays on this computer, under its cap (a sandbox refuses it the same way)
        if crate::capped::cap_of(lead).is_some() {
            return Err(RuntimeError::Wire(wisp_kernel::WireError::forbidden(wisp_kernel::caps::capped_refusal(
                "send work to another machine",
            ))));
        }
        let before = existing.filter(|k| k.split_once('|').is_some_and(|(on, _)| on == machine));
        if let Some(key) = &before {
            let place = slot(lead).and_then(|x| x.get(AWAY)).and_then(|a| a.get(key)).and_then(Value::as_str).unwrap_or(machine);
            // still at its last job: refused; done: its changes come back before the copy is made afresh
            if let Some(r) = self.far_report(lead, key).await
                && !r.done
            {
                return Err(RuntimeError::Conflict(format!(
                    "{slot_name} is still working on an earlier task on {place}. Wait with subagent_results, then delegate again."
                )));
            }
        }
        let carry =
            wisp_machines_api::CarryArgs { id: machine.to_owned(), cwd: lead.cwd.clone(), key: format!("{}-{slot_name}", lead.id.0) };
        let carried = crate::calls::run::<wisp_machines_api::Carry>(&*self.host, carry).await.map_err(wire)?;
        // its folder has another path there: the lead's paths as paths in the folder, and it's told where it is
        let text = format!("{}\n{AWAY_SUBAGENT}", in_folder(prompt, &lead.cwd));
        if let Some((_, id)) = before.as_deref().and_then(|k| k.split_once('|'))
            && let Ok(f) = self.route(machine, "runtime.followUp", json!({ "id": id, "text": text, "queue": true })).await
        {
            return Ok((format!("{machine}|{id}"), carried.machine, str_at(&f, "turnId")));
        }
        let mut args = json!({ "prompt": text, "cwd": carried.cwd, "mode": lead.mode, "title": format!("{slot_name} · {}", lead.title) });
        for k in ["agent", "model", "effort"] {
            let v = str_at(m, k);
            if !v.is_empty() && v != "auto" {
                args[k] = json!(v);
            }
        }
        let made = self.route(machine, "runtime.create", args).await.map_err(wire)?;
        Ok((format!("{machine}|{}", str_at(&made, "id")), carried.machine, str_at(&made, "turnId")))
    }

    /// A job on another wisp, for `subagent_results` and the next delegation (`<machine>|<task>`: a machine member,
    /// a sandbox member, a crew job there): read there; once it's done, its changes come into the lead's folder once
    /// (`bring_far`).
    pub(crate) async fn far_report(self: &Arc<Self>, lead: &Session, key: &str) -> Option<Report> {
        let (machine, id) = key.split_once('|')?;
        let turn = self.far_turn_of(lead.id.as_str(), key).await;
        let t = self.there(machine, id, &turn).await?;
        let done = t.state != SessionState::Running;
        let note = if done { self.bring_far(lead.id.as_str(), key, &t).await.unwrap_or_default() } else { String::new() };
        Some(Report {
            id: SessionId(key.to_owned()),
            subagent: str_at(&t.session, "title").split(" · ").next().unwrap_or_default().to_owned(),
            state: t.state,
            done,
            text: far_text(lead, key, &t, &note),
            cost_usd: t.job.pointer("/stats/costUsd").and_then(Value::as_f64),
        })
    }

    /// The turn there the lead's latest record of job `key` names ("" when none does: its last turn counts).
    async fn far_turn_of(&self, lead: &str, key: &str) -> String {
        let mut recent = self.repo.turns(lead, None, Some(8)).await.unwrap_or_default();
        recent.sort_by_key(|t| std::cmp::Reverse(t.idx));
        // the latest record within a turn too: a member followed up in the lead's same turn has two, its last is the job
        recent
            .iter()
            .flat_map(|t| super::report_back::records(t).iter().rev())
            .find(|d| str_at(d, "sessionId") == key)
            .map(|d| str_at(d, "turnId"))
            .unwrap_or_default()
    }

    /// A done far job's changes into lead `lead`'s folder, once per job (v1 relay.js `return`), with the note its
    /// report carries; None: nothing to bring (a crew job there worked in the member's folder), or it was brought
    /// already. Held instead (kept in a file, the folder left as it is) for a lead in Plan or a job that didn't finish.
    /// A bring that didn't answer (the sandbox asleep, the machine unreachable) marks nothing: the next look tries again.
    /// Off the lead's lock (git and the machine's link can take long): bringers of one lead go one at a time under a
    /// lock of their own, and the mark (on the lead, the note on the owed record) is written before the next looks.
    pub(crate) async fn bring_far(self: &Arc<Self>, lead: &str, key: &str, t: &There) -> Option<String> {
        let (machine, _) = key.split_once('|')?;
        let _one = self.task_lock(&format!("{lead} bring")).await;
        let l = self.repo.session(lead).await.ok()?;
        let own = slot(&l);
        if own.and_then(|m| m.get(super::delegate::CREW_JOBS)).and_then(|j| j.get(key)).is_some() {
            return None;
        }
        let has = |list: &str, mark: &str| {
            own.and_then(|m| m.get(list)).and_then(Value::as_array).is_some_and(|b| b.iter().any(|x| x.as_str() == Some(mark)))
        };
        let machine_member = own.and_then(|m| m.get(AWAY)).and_then(|a| a.get(key)).is_some();
        let mark = format!("{key}@{}", str_at(&t.job, "id"));
        if has(BROUGHT, &mark) {
            return None;
        }
        // a sandbox member's job brought under the old per-member mark: that one counts as this job, brought; the mark
        // moves to this job, so the member's next job comes back
        let once = !machine_member && has(BROUGHT_ONCE, key);
        let note = if once {
            String::new()
        } else {
            match self.bring_one(&l, machine, machine_member, t).await {
                Ok(n) => n,
                Err(n) => {
                    self.note_not_back(lead, key, &n).await;
                    return Some(n);
                }
            }
        };
        let _g = self.task_lock(lead).await;
        let mut l = self.repo.session(lead).await.ok()?;
        with_slot(&mut l.ext, |x| {
            if let Some(Value::Array(old)) = x.get_mut(BROUGHT_ONCE) {
                old.retain(|v| v.as_str() != Some(key));
            }
            push(x, BROUGHT, json!(mark));
        });
        let mut turns = Vec::new();
        if let Ok(Some(mut owing)) = self.owed_turn(lead, None).await
            && with_slot(&mut owing.ext, |x| super::report_back::note_brought(x, key, &note))
        {
            turns.push(owing);
        }
        if let Err(err) = self.persist(&mut l, turns).await {
            tracing::warn!(error = %err, task = lead, "runtime: a delegate's changes elsewhere, marked brought");
        }
        Some(note)
    }

    /// One far job's changes brought (or held) for lead `l`: the note its report carries. Err: the bring didn't
    /// answer, and the note says they're tried again.
    async fn bring_one(&self, l: &Session, machine: &str, machine_member: bool, t: &There) -> std::result::Result<String, String> {
        let held = held_why(l, t);
        let (from, into, keep) = (str_at(&t.session, "cwd"), l.cwd.clone(), held.is_some());
        let got = if machine_member {
            let a = wisp_machines_api::BringArgs { id: machine.to_owned(), from, into, keep };
            crate::calls::run::<wisp_machines_api::Bring>(&*self.host, a).await.map(|b| Got {
                files: b.files,
                error: b.error,
                kept: b.kept,
            })
        } else {
            let a = wisp_sandbox_api::BringArgs { machine: machine.to_owned(), from, into, keep };
            crate::calls::run::<wisp_sandbox_api::Bring>(&*self.host, a).await.map(|b| Got { files: b.files, error: b.error, kept: b.kept })
        };
        got.map(|g| bring_note(&g, held)).map_err(|e| {
            tracing::info!(error = %e.message, task = %l.id, "runtime: a delegate's changes elsewhere didn't come back yet");
            format!("\n\n(Its changes didn't come back yet: {}. They'll be tried again.)", e.message)
        })
    }

    /// A bring that didn't answer: its note on the owed record (the report says so), and no mark.
    async fn note_not_back(self: &Arc<Self>, lead: &str, key: &str, note: &str) {
        let _g = self.task_lock(lead).await;
        let (Ok(mut l), Ok(Some(mut owing))) = (self.repo.session(lead).await, self.owed_turn(lead, None).await) else { return };
        if with_slot(&mut owing.ext, |x| super::report_back::note_brought(x, key, note))
            && let Err(err) = self.persist(&mut l, vec![owing]).await
        {
            tracing::warn!(error = %err, task = lead, "runtime: a delegate's changes elsewhere, noted not back yet");
        }
    }
}

/// A far job's report text: its turn's answer, `note` (what came back), and where a machine member ran.
pub(crate) fn far_text(lead: &Session, key: &str, t: &There, note: &str) -> String {
    let mut text = str_at(&t.job, "text");
    text.push_str(note);
    if let Some(place) = slot(lead).and_then(|m| m.get(AWAY)).and_then(|a| a.get(key)).and_then(Value::as_str) {
        text.push_str("\n\n(It ran on ");
        text.push_str(place);
        text.push_str(", on a copy of your folder.)");
    }
    text
}
