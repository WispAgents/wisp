//! Subagents on another machine (ADR-0059, v1 services/relay.js): the lead's work copied there (`machines.carry`,
//! a copy under that wisp's own folder, never the user's checkout), the task run there (`machines.route`), its report
//! read there and its changes brought into the lead's folder once per job (`machines.bring`). Each says where it ran.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{Delegation, Report};
use wisp_runtime_api::model::{Session, SessionState};
use wisp_runtime_core::prompt::subagents::{AWAY_SUBAGENT, in_folder};

use super::{push, slot, str_at, with_slot};
use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;

/// The lead's delegations on other machines (`ext["wisp.runtime"].away`): key → the machine's name.
pub(super) const AWAY: &str = "away";
/// The jobs whose changes came back, as `<key>@<turn there>` (one bring per job, not per member).
const BROUGHT: &str = "broughtJobs";

/// A delegation that went to another machine (not a sandbox, not a crew job).
pub(super) fn on_machine(d: &Delegation) -> bool {
    d.error.is_none() && d.place.as_deref().is_some_and(|p| p != "sandbox" && p != "crew")
}

fn wire(e: wisp_kernel::WireError) -> RuntimeError {
    RuntimeError::Invalid(e.message)
}

/// What the machine says of a task there: its row and its last turn.
struct There {
    session: Value,
    state: SessionState,
    last: Value,
}

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

    async fn there(&self, machine: &str, id: &str) -> Option<There> {
        let session = self.route(machine, "runtime.get", json!({ "id": id })).await.ok()?;
        let state = serde_json::from_value(session.get("state").cloned().unwrap_or_default()).unwrap_or(SessionState::Running);
        let turns = self.route(machine, "runtime.turns", json!({ "id": id })).await.ok().unwrap_or_default();
        let list = turns.as_array().cloned().or_else(|| turns.get("turns").and_then(Value::as_array).cloned()).unwrap_or_default();
        Some(There { session, state, last: list.last().cloned().unwrap_or_default() })
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
    ) -> Result<(String, String)> {
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
            if let Some(r) = self.machine_report(lead, key, place).await
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
            && self.route(machine, "runtime.followUp", json!({ "id": id, "text": text, "queue": true })).await.is_ok()
        {
            return Ok((format!("{machine}|{id}"), carried.machine));
        }
        let mut args = json!({ "prompt": text, "cwd": carried.cwd, "mode": lead.mode, "title": format!("{slot_name} · {}", lead.title) });
        for k in ["agent", "model", "effort"] {
            let v = str_at(m, k);
            if !v.is_empty() && v != "auto" {
                args[k] = json!(v);
            }
        }
        let made = self.route(machine, "runtime.create", args).await.map_err(wire)?;
        Ok((format!("{machine}|{}", str_at(&made, "id")), carried.machine))
    }

    /// A machine member's report, read there; once its job is done, the job's changes come into the lead's folder
    /// (once per job). The text says where it ran.
    pub(super) async fn machine_report(self: &Arc<Self>, lead: &Session, key: &str, place: &str) -> Option<Report> {
        let (machine, id) = key.split_once('|')?;
        let t = self.there(machine, id).await?;
        let done = t.state != SessionState::Running;
        let mut text = str_at(&t.last, "text");
        let job = format!("{key}@{}", str_at(&t.last, "id"));
        let lead = self.repo.session(lead.id.as_str()).await.ok()?;
        let brought = slot(&lead)
            .and_then(|m| m.get(BROUGHT))
            .and_then(Value::as_array)
            .is_some_and(|b| b.iter().any(|x| x.as_str() == Some(job.as_str())));
        if done && !brought {
            let bring = wisp_machines_api::BringArgs { id: machine.to_owned(), from: str_at(&t.session, "cwd"), into: lead.cwd.clone() };
            let note = match crate::calls::run::<wisp_machines_api::Bring>(&*self.host, bring).await {
                Ok(wisp_machines_api::Brought { error: Some(e), .. }) => format!("\n\n(Its changes didn't come back: {e})"),
                Ok(b) if b.files > 0 => format!("\n\n(Its changes to {} file(s) are in your folder now.)", b.files),
                Ok(_) => String::new(),
                Err(e) => format!("\n\n(Its changes didn't come back: {})", e.message),
            };
            text.push_str(&note);
            let _g = self.task_lock(lead.id.as_str()).await;
            if let Ok(mut l) = self.repo.session(lead.id.as_str()).await {
                with_slot(&mut l.ext, |x| push(x, BROUGHT, json!(job)));
                if let Err(err) = self.persist(&mut l, Vec::new()).await {
                    tracing::warn!(error = %err, task = lead.id.as_str(), "runtime: a machine member's changes, marked brought");
                }
            }
        }
        text.push_str("\n\n(It ran on ");
        text.push_str(place);
        text.push_str(", on a copy of your folder.)");
        Some(Report {
            id: SessionId(key.to_owned()),
            subagent: str_at(&t.session, "title").split(" · ").next().unwrap_or_default().to_owned(),
            state: t.state,
            done,
            text,
            cost_usd: t.last.pointer("/stats/costUsd").and_then(Value::as_f64),
        })
    }
}
