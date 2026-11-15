//! A lead's owed delegations (v1 runner.js `#awaiting`, `#reportBack`): when it waits, when it hears, and the records
//! a new turn takes over.
#![allow(clippy::unwrap_used)]

use serde_json::{Value, json};
use wisp_runtime_core::owed::{self, Next, Open};

fn open(name: &str, running: bool) -> Open {
    Open { name: name.into(), running }
}

#[test]
fn a_lead_waits_while_one_it_owes_works_and_hears_once_the_last_ends() {
    // nothing owed, or the user stopped it: nothing to do
    assert_eq!(owed::next(&[], false, false), Next::Nothing);
    assert_eq!(owed::next(&[open("a", true)], false, true), Next::Nothing);
    // one at work: it waits on it, by name, once each (two copies' tasks of one member)
    assert_eq!(owed::next(&[open("a", false), open("b", true), open("b", true)], false, false), Next::Hold(vec!["b".into()]));
    // its own run is on: it still says who works, but its end decides
    assert_eq!(owed::next(&[open("a", true)], true, false), Next::Hold(vec!["a".into()]));
    // the last ended: it hears now, unless its own run is on (that run's end collects them)
    assert_eq!(owed::next(&[open("a", false), open("b", false)], false, false), Next::Report);
    assert_eq!(owed::next(&[open("a", false)], true, false), Next::Nothing);
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
fn delegates_elsewhere_are_read_with_subagent_results_not_waited_on() {
    let jobs: serde_json::Map<String, Value> = [("job1".to_owned(), json!("quill"))].into_iter().collect();
    assert!(!owed::is_away(&json!({ "sessionId": "abc" }), Some(&jobs)));
    assert!(owed::is_away(&json!({ "sessionId": "m1|r1" }), None), "a sandbox's or another machine's");
    assert!(owed::is_away(&json!({ "sessionId": "x", "place": "studio" }), None));
    assert!(owed::is_away(&json!({ "sessionId": "job1" }), Some(&jobs)), "a crew member's job");
    assert!(owed::is_away(&json!({ "sessionId": "" }), None));
}

#[test]
fn the_report_back_text_is_the_header_then_each_report_apart() {
    let t = owed::report_text(&["### a".into(), "### b".into()]);
    assert_eq!(
        t,
        "Subagents you delegated to earlier have finished. Review their work below and carry on with the task.\n\n### a\n\n---\n\n### b"
    );
}
