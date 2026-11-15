//! A lead that owes its delegates' reports (v1 runner.js `#awaiting`, `#delegateDone`, `#reportBack`). Each delegation
//! on the lead's turn (`ext["wisp.runtime"].delegations[]`) is `owed: true` until the lead has its report: read with
//! `subagent_results`, or sent to it by wisp. While one it owes still works, a lead whose turn ended stays in Working
//! (`land::Facts.owes`), unread and unannounced, and its slot names who it waits on (`waitsOn`, which wisp.later reads).
//! As one ends (done or failed) while the lead's own run is off, wisp sends the lead a follow-up at once with every owed
//! report that's in (`REPORTS_BACK`, `by: wisp`, `reports: true`): the ones still at work go on with that follow-up's turn
//! and come back after it. Those that end while the lead's run is on are collected by its end, in one follow-up. A
//! delegate the user stopped doesn't wake the lead alone: its `— stopped` line goes with the next report, or alone when
//! it was the last at work. Once: the owed marks go in the same write that queues it. A lead the user stopped is owed
//! nothing. The state is in the store (the turn's records, the task's queue), so a restart picks it
//! up where it was. Its children and crew jobs here are read here; a job on another wisp (`is_far`: a sandbox, another
//! machine, a crew job there) stands as that wisp answers (`far_open`).

use serde_json::Value;
use wisp_runtime_api::model::{SessionState, TurnStatus};

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
    /// its turn ended stopped (the user stopped it): reported, but it doesn't wake the lead by itself
    pub stopped: bool,
}

/// What the lead does about what it owes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// some still work: it waits on these (names, in order, once each)
    Hold(Vec<String>),
    /// send it these reports now (indexes into `owed`, the ended ones); any still at work go on with that follow-up
    Report(Vec<usize>),
    /// nothing to do: it owes nothing, it was stopped, or its own run is on (its end collects them)
    Nothing,
}

/// `owed`: the delegations it owes. `busy`: the lead's own run is on (starting, running or lingering): its end
/// decides. `stopped`: the user stopped the lead. v1 runner.js `#delegateDone` → `#reportBack`: what ended is sent as
/// soon as the lead's run is off, without waiting for the rest.
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
    let ended: Vec<usize> = owed.iter().enumerate().filter(|(_, d)| !d.running).map(|(i, _)| i).collect();
    // a report worth waking it for: one that finished, or the last ones ended (stopped included)
    let wakes = owed.iter().any(|d| !d.running && !d.stopped) || (at_work.is_empty() && !ended.is_empty());
    if !busy && wakes {
        return Next::Report(ended);
    }
    if at_work.is_empty() { Next::Nothing } else { Next::Hold(at_work) }
}

/// A record the lead still owes.
pub fn is_owed(d: &Value) -> bool {
    d.get(OWED).and_then(Value::as_bool).unwrap_or(false)
}

/// A record with a job to watch (one that didn't go has no session).
pub fn has_job(d: &Value) -> bool {
    d.get("sessionId").and_then(Value::as_str).is_some_and(|id| !id.is_empty())
}

/// A delegation whose job runs on another wisp (`<machine>|<task>`: a sandbox, another machine, a crew member's job
/// there): wisp reads how it stands there (`far_open`); the others (its own children, a crew job here) are read here.
pub fn is_far(d: &Value) -> bool {
    d.get("sessionId").and_then(Value::as_str).is_some_and(|id| id.contains('|'))
}

/// How a job on another wisp stands, from what that wisp answers (v1 relay.js `watch`): `state`, its task's (None: it
/// didn't answer, so it's still at work as far as the lead knows: Stop lets it go); `job`, the status of the turn the
/// delegation started there (`known`: its record names it), or of its task's last turn when it doesn't (None: not
/// shown there yet). Its own turn counts, not the task's last: a turn after it (a resume, the user typing there, its
/// own report-back) neither holds the lead nor ends its job.
pub fn far_open(name: &str, state: Option<SessionState>, job: Option<TurnStatus>, known: bool) -> Open {
    let at_work = matches!(job, None | Some(TurnStatus::Running | TurnStatus::Held));
    // not known which turn is its: its task still Running (on its own delegates there, say) is still at it
    let running = state.is_none() || at_work || (!known && state == Some(SessionState::Running));
    let stopped = !running && job == Some(TurnStatus::Stopped);
    Open { name: name.to_owned(), running, stopped }
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
