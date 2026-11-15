//! A lead that owes its delegates' reports (v1 runner.js `#awaiting`, `#delegateDone`, `#reportBack`). Each delegation
//! on the lead's turn (`ext["wisp.runtime"].delegations[]`) is `owed: true` until the lead has its report: read with
//! `subagent_results`, or sent to it by wisp. While one it owes still works, a lead whose turn ended stays in Working
//! (`land::Facts.owes`), unread and unannounced, and its slot names who it waits on (`waitsOn`, which wisp.later reads).
//! When the last one ends (done, failed or stopped), wisp sends the lead one follow-up with every owed report
//! (`REPORTS_BACK`, `by: wisp`, `reports: true`), once: the owed marks go in the same write that queues it. A lead the
//! user stopped is owed nothing. The state is in the store (the turn's records, the task's queue), so a restart picks it
//! up where it was.

use serde_json::Value;

/// A delegation's mark on the lead's turn: the lead hasn't had its report yet.
pub const OWED: &str = "owed";
/// The lead's slot: the members it waits on while it stays in Working on them.
pub const WAITS_ON: &str = "waitsOn";
/// The lead's turn's delegation records.
pub const DELEGATIONS: &str = "delegations";

/// One delegation the lead owes, as the host found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    /// the member (or copy) it went to
    pub name: String,
    /// its turn still runs
    pub running: bool,
}

/// What the lead does about what it owes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// some still work: it waits on these (names, in order, once each)
    Hold(Vec<String>),
    /// every one it owes ended: send it their reports now
    Report,
    /// nothing to do: it owes nothing, it was stopped, or its own run is on (its end collects them)
    Nothing,
}

/// `owed`: the delegations it owes. `busy`: the lead's own run is on (starting, running or lingering): its end
/// decides. `stopped`: the user stopped the lead.
pub fn next(owed: &[Open], busy: bool, stopped: bool) -> Next {
    if stopped || owed.is_empty() {
        return Next::Nothing;
    }
    let mut at_work: Vec<String> = Vec::new();
    for d in owed.iter().filter(|d| d.running) {
        if !at_work.contains(&d.name) {
            at_work.push(d.name.clone());
        }
    }
    if !at_work.is_empty() {
        return Next::Hold(at_work);
    }
    if busy { Next::Nothing } else { Next::Report }
}

/// A record the lead still owes.
pub fn is_owed(d: &Value) -> bool {
    d.get(OWED).and_then(Value::as_bool).unwrap_or(false)
}

/// A delegation that went elsewhere (a sandbox or another machine, `<machine>|<task>`) or to a crew member: wisp
/// doesn't watch its run from here, so the lead doesn't wait on it (it reads it with `subagent_results`).
pub fn is_away(d: &Value, crew_jobs: Option<&serde_json::Map<String, Value>>) -> bool {
    let id = d.get("sessionId").and_then(Value::as_str).unwrap_or_default();
    id.is_empty() || id.contains('|') || d.get("place").is_some_and(|p| !p.is_null()) || crew_jobs.is_some_and(|j| j.contains_key(id))
}

/// The records a new turn of the lead takes over from its last one (v1 `#startTurn`: an interrupting follow-up's turn
/// waits on them, `subagent_results` finds them, their edits count on it): the owed ones, taken out of `prev`.
pub fn take_owed(prev: &mut serde_json::Map<String, Value>) -> Vec<Value> {
    let Some(Value::Array(list)) = prev.get_mut(DELEGATIONS) else { return Vec::new() };
    let (owed, kept): (Vec<Value>, Vec<Value>) = std::mem::take(list).into_iter().partition(is_owed);
    if kept.is_empty() {
        prev.remove(DELEGATIONS);
    } else {
        *list = kept;
    }
    owed
}

/// Marks these records (by session id) as read; true when one changed.
pub fn mark_read(slot: &mut serde_json::Map<String, Value>, ids: &[String]) -> bool {
    let Some(Value::Array(list)) = slot.get_mut(DELEGATIONS) else { return false };
    let mut changed = false;
    for d in list.iter_mut() {
        let id = d.get("sessionId").and_then(Value::as_str).unwrap_or_default().to_owned();
        if is_owed(d)
            && ids.contains(&id)
            && let Some(o) = d.as_object_mut()
        {
            o.insert(OWED.into(), Value::Bool(false));
            changed = true;
        }
    }
    changed
}

/// The follow-up's text: the header, then each report, apart.
pub fn report_text(reports: &[String]) -> String {
    format!("{}\n\n{}", crate::prompt::subagents::REPORTS_BACK, reports.join("\n\n---\n\n"))
}
