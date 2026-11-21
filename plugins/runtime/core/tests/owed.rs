//! A lead's owed delegations (v1 runner.js `#awaiting`, `#reportBack`): when it waits, when it hears, and the records
//! a new turn takes over.
#![allow(clippy::unwrap_used)]

use serde_json::json;
use wisp_runtime_core::owed::{self, Next, Open};

fn open(name: &str, running: bool) -> Open {
    Open { name: name.into(), running, stopped: false }
}

fn stopped(name: &str) -> Open {
    Open { name: name.into(), running: false, stopped: true }
}

#[test]
fn a_lead_waits_while_one_it_owes_works_and_hears_once_the_last_ends() {
    // nothing owed, or the user stopped it: nothing to do
    assert_eq!(owed::next(&[], false, false), Next::Nothing);
    assert_eq!(owed::next(&[open("a", true)], false, true), Next::Nothing);
    // at work: it waits on them, by name, once each (two copies' tasks of one member)
    assert_eq!(owed::next(&[open("b", true), open("b", true)], false, false), Next::Hold(vec!["b".into()]));
    // its own run is on: it still says who works, but its end decides
    assert_eq!(owed::next(&[open("a", true)], true, false), Next::Hold(vec!["a".into()]));
    // the last ended: it hears now, unless its own run is on (that run's end collects them)
    assert_eq!(owed::next(&[open("a", false), open("b", false)], false, false), Next::Report(vec![0, 1]));
    assert_eq!(owed::next(&[open("a", false)], true, false), Next::Nothing);
}

/// v1 runner.js `#delegateDone` → `#reportBack`: one that ended goes to the lead at once, while others still work
/// (round 247 held it until the last one ended).
#[test]
fn one_that_ends_while_others_work_is_reported_at_once() {
    assert_eq!(owed::next(&[open("a", false), open("b", true), open("b", true)], false, false), Next::Report(vec![0]));
    assert_eq!(owed::next(&[open("a", true), open("b", false), open("c", false)], false, false), Next::Report(vec![1, 2]));
    // the lead's run is on: its end collects them, in one
    assert_eq!(owed::next(&[open("a", false), open("b", true)], true, false), Next::Hold(vec!["b".into()]));
}

/// v1: a stopped run owes nothing. wisp keeps its `— stopped` line, but it doesn't wake the lead on its own: it goes
/// with the next report, or alone when it was the last at work.
#[test]
fn a_stopped_delegate_rides_with_the_next_report_and_doesnt_wake_the_lead_alone() {
    assert_eq!(owed::next(&[stopped("a"), open("b", true)], false, false), Next::Hold(vec!["b".into()]));
    assert_eq!(owed::next(&[stopped("a"), open("b", false), open("c", true)], false, false), Next::Report(vec![0, 1]));
    assert_eq!(owed::next(&[stopped("a")], false, false), Next::Report(vec![0]));
}

#[test]
fn a_new_turn_takes_over_the_owed_records_and_read_ones_stay() {
    let mut prev = json!({ "delegations": [
        { "name": "a", "sessionId": "s1", "owed": true },
        { "name": "b", "sessionId": "s2", "owed": false },
    ], "changes": [] });
    let taken = owed::take_owed(prev.as_object_mut().unwrap());
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0]["sessionId"], "s1");
    assert_eq!(prev["delegations"], json!([{ "name": "b", "sessionId": "s2", "owed": false }]));
    // all owed: the list goes, the rest of the slot stays
    let mut all = json!({ "delegations": [{ "sessionId": "s1", "owed": true }], "changes": [] });
    assert_eq!(owed::take_owed(all.as_object_mut().unwrap()).len(), 1);
    assert_eq!(all, json!({ "changes": [] }));
}

#[test]
fn marking_read_changes_only_the_owed_records_named() {
    let mut slot = json!({ "delegations": [
        { "sessionId": "s1", "owed": true },
        { "sessionId": "s2", "owed": true },
    ] });
    let m = slot.as_object_mut().unwrap();
    assert!(owed::mark_read(m, &["s1".into()]));
    assert!(!owed::mark_read(m, &["s1".into()]), "once");
    assert_eq!(slot["delegations"][0]["owed"], false);
    assert_eq!(slot["delegations"][1]["owed"], true);
}

#[test]
fn delegates_elsewhere_are_watched_there_and_local_crew_jobs_here() {
    assert!(!owed::is_far(&json!({ "sessionId": "abc" })), "a child, or a crew job here: read here");
    assert!(owed::is_far(&json!({ "sessionId": "m1|r1" })), "a sandbox's, another machine's, a crew job there");
    assert!(owed::has_job(&json!({ "sessionId": "job1" })));
    assert!(!owed::has_job(&json!({ "sessionId": "" })), "one that didn't go");
}

#[test]
fn a_job_elsewhere_stands_as_its_machine_answers() {
    use wisp_runtime_api::model::{SessionState as S, TurnStatus as T};
    let at = |state, job, known| {
        let o = owed::far_open("far", state, job, known);
        (o.running, o.stopped)
    };
    assert_eq!(at(None, Some(T::Done), true), (true, false), "no answer: still at work as far as the lead knows");
    assert_eq!(at(Some(S::Running), Some(T::Running), true), (true, false));
    assert_eq!(at(Some(S::Review), Some(T::Done), true), (false, false), "done");
    assert_eq!(at(Some(S::Review), Some(T::Stopped), true), (false, true), "stopped there");
    assert_eq!(at(Some(S::Review), None, true), (true, false), "its turn isn't shown there yet");
    assert_eq!(at(Some(S::Review), Some(T::Error), true), (false, false));
    assert_eq!(at(Some(S::Running), Some(T::Done), true), (false, false), "its turn ended: a later one there doesn't hold it");
    assert_eq!(at(Some(S::Running), Some(T::Done), false), (true, false), "turn not known: its task still at it holds");
    assert_eq!(at(Some(S::Review), Some(T::Done), false), (false, false), "turn not known: its last turn counts");
}

#[test]
fn the_report_back_text_is_the_header_then_each_report_apart() {
    let t = owed::report_text(&["### a".into(), "### b".into()]);
    assert_eq!(
        t,
        "Subagents you delegated to earlier have finished. Review their work below and carry on with the task.\n\n### a\n\n---\n\n### b"
    );
}
