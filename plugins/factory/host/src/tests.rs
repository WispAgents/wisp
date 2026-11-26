//! ADR-0114's host edge cases that need no server: what `before:create` makes of a task that names a factory, and what
//! a restart leaves of a run it cut short. The driver is held end to end by `crates/wisp/tests/serve_factory.rs`.

use serde_json::json;
use wisp_factory_api::{FactorySpec, NamedFactory};
use wisp_runtime_api::commands::StartArgs;

use super::*;

fn declared() -> Vec<NamedFactory> {
    let spec: FactorySpec =
        serde_json::from_value(json!({ "stations": { "build": { "agent": "codex" }, "checks": { "run": ["npm test"] } } })).unwrap();
    vec![NamedFactory { name: "careful".into(), spec }]
}

#[test]
fn an_unknown_factory_is_refused_naming_the_ones_there_are() {
    let mut a = StartArgs { prompt: "x".into(), factory: Some("carefull".into()), ..StartArgs::default() };
    assert_eq!(prepare(&mut a, &declared(), false), Err("no factory named carefull: factories has default, careful".into()));
    // a task that names none is left alone
    let mut a = StartArgs { prompt: "x".into(), ..StartArgs::default() };
    assert_eq!(prepare(&mut a, &declared(), false), Ok(false));
    assert!(!a.hold && a.ext.is_empty());
    // the rows: the built-in default first, unless one is declared by its name
    assert_eq!(rows(&declared()).iter().map(|r| (r.name.as_str(), r.builtin)).collect::<Vec<_>>(), [("default", true), ("careful", false)]);
    let mine = vec![NamedFactory { name: "default".into(), spec: declared()[0].spec.clone() }];
    assert_eq!(rows(&mine).len(), 1);
    assert_eq!(resolve(&mine, "").unwrap().1.station_ids(), ["build", "checks"], "a declared default replaces the built-in one");
    let mut a = StartArgs { prompt: "x".into(), factory: Some("y".into()), terminal: true, ..StartArgs::default() };
    assert!(prepare(&mut a, &[], false).is_err_and(|e| e.contains("default")), "y names nothing");
}

#[test]
fn a_factory_task_is_made_held_with_no_worktree_and_keeps_its_factory() {
    let mut a = StartArgs {
        prompt: "Fix the add bug".into(),
        steps: vec!["and add a test".into()],
        factory: Some(String::new()),
        worktree: Some(true),
        agent: Some("claude".into()),
        goal: Some(json!({ "text": "tests pass", "judges": [{ "agent": "codex" }], "vote": "all" })),
        ..StartArgs::default()
    };
    assert_eq!(prepare(&mut a, &declared(), true), Ok(true));
    assert!(a.hold, "the run's own task never runs an agent");
    assert_eq!((a.worktree, a.factory.as_deref(), a.goal.as_ref(), a.steps.len()), (Some(false), Some("default"), None, 0));
    let slot = &a.ext[EXT];
    assert_eq!(slot["state"], "starting");
    let input = &slot["input"];
    assert_eq!(input["prompt"], "Fix the add bug\n\nThen: and add a test");
    assert_eq!(
        (input["worktree"].as_bool(), input["agent"].as_str(), input["byAgent"].as_bool()),
        (Some(true), Some("claude"), Some(true))
    );
    let goal = goal_of(&input["goal"]).unwrap();
    assert_eq!((goal.text.as_str(), goal.judges.as_slice(), goal.vote.as_str()), ("tests pass", ["codex".to_owned()].as_slice(), "all"));
    assert_eq!(input["spec"]["stations"]["ship"]["merge"], "human", "the built-in default's spec travels with the run");
    // a formula's factory (`--formula`, a task file's) is the run's, nothing applied
    let mut f = StartArgs {
        prompt: "x".into(),
        factory: Some("quick".into()),
        formula: Some(json!({ "factories": [{ "name": "quick", "stations": { "build": { "agent": "codex" } } }] })),
        ..StartArgs::default()
    };
    assert_eq!(prepare(&mut f, &declared(), false), Ok(true));
    assert_eq!(f.ext[EXT]["input"]["spec"]["stations"]["build"]["agent"], "codex");
    let mut bad = StartArgs {
        prompt: "x".into(),
        factory: Some("q".into()),
        formula: Some(json!({ "factories": [{ "name": "q", "stations": {} }] })),
        ..StartArgs::default()
    };
    assert!(prepare(&mut bad, &[], false).is_err_and(|e| e.contains("needs a build station")));
    // a terminal task can't be a run
    let mut t = StartArgs { prompt: "x".into(), factory: Some("default".into()), terminal: true, ..StartArgs::default() };
    assert!(prepare(&mut t, &[], false).is_err_and(|e| e.contains("terminal")));
}

#[test]
fn a_run_wisp_stopped_in_the_middle_of_says_so_and_ends() {
    let slot = json!({ "factory": "default", "state": "waiting", "station": "spec", "waiting": { "question": "Approve?" },
        "stations": [{ "id": "spec", "state": "asking" }, { "id": "build", "state": "waiting" }] });
    let v = interrupted(&slot).unwrap();
    assert_eq!((v["state"].as_str(), v["station"].as_str(), v["waiting"].is_null()), (Some("stopped"), Some(""), true));
    assert!(v["why"].as_str().unwrap().contains("wisp stopped"));
    assert_eq!((v["stations"][0]["state"].as_str(), v["stations"][1]["state"].as_str()), (Some("failed"), Some("waiting")));
    assert!(interrupted(&json!({ "state": "done" })).is_none(), "an ended run stays as it ended");
    assert!(interrupted(&json!({ "run": "r1", "station": "build" })).is_none(), "a station's task isn't a run");
}

/// Review r194 factories #5: the run task's turn guard fails closed. A run's own task known to this wisp is refused at
/// once; before the runs are known (at start), the task is read, and a task that can't be read is refused too.
#[test]
fn a_turn_on_a_run_is_refused_and_an_unreadable_task_fails_closed() {
    assert!(refuse_turn(true, true, || None), "a known run");
    assert!(!refuse_turn(false, true, || None), "once the runs are known, any other task runs");
    assert!(refuse_turn(false, false, || Some(Err("store".into()))), "can't read it: refused");
    assert!(refuse_turn(false, false, || Some(Ok(true))), "read: a run");
    assert!(!refuse_turn(false, false, || Some(Ok(false))), "read: another task");
}

/// Review r194 final pass #3: the factory's `before:create` fails closed: a task that names a factory is never made as
/// a plain agent task because the factory couldn't read it, and what went wrong is said in plain words.
#[test]
fn the_factory_create_guard_fails_closed_with_a_plain_reason() {
    assert_eq!(CREATE_GUARD.failure, wisp_kernel::Failure::Closed);
    let a = StartArgs { prompt: "x".into(), factory: Some("default".into()), ..StartArgs::default() };
    match guarded(|_: &mut StartArgs| -> std::result::Result<bool, String> { panic!("boom") }, a) {
        wisp_kernel::Verdict::Veto { message, .. } => assert_eq!(message, CREATE_FAILED),
        _ => panic!("refused"),
    }
}

/// Audit I5: a follow-up to a run's own task with nothing asked was taken ("✓ sent") and then not started; it's refused
/// in words. I12: `wisp factory stop` on a task that isn't a run said "This factory run isn't running".
#[test]
fn a_follow_up_or_a_stop_with_nothing_there_says_why_audit_i5_i12() {
    assert_eq!(follow_up_refusal("stopped"), "This factory run is stopped: nothing was sent.");
    assert_eq!(follow_up_refusal("done"), "This factory run is done: nothing was sent.");
    assert!(follow_up_refusal("running").starts_with("This factory run's stations do the work: nothing was sent."));
    assert!(drive::stop_refusal(false, "").starts_with("That task isn't a factory run"));
    assert_eq!(drive::stop_refusal(true, "stopped"), "This factory run is stopped already: nothing to stop.");
}

fn station_turn(status: wisp_runtime_api::TurnStatus) -> wisp_runtime_api::model::Turn {
    wisp_runtime_api::model::Turn { id: "u1".into(), session_id: "s1".into(), status, text: Some("built".into()), ..Default::default() }
}

/// A new station task's wait: its first turn.
fn first() -> wisp_factory_core::Pending {
    wisp_factory_core::Pending { task: "s1".into(), ..Default::default() }
}

/// doc23a: a follow-up's wait read the task's last turn, so after a restart (no turn id kept) round 1's review was
/// taken as round 2's. Only a turn after `since` (the last one when it was sent) ends it.
#[test]
fn a_follow_ups_wait_never_takes_a_turn_from_before_it() {
    let t = wisp_runtime_api::model::Session { state: wisp_runtime_api::SessionState::Review, ..Default::default() };
    let turn = |idx: u32, text: &str| wisp_runtime_api::model::Turn {
        idx,
        text: Some(text.into()),
        ..station_turn(wisp_runtime_api::TurnStatus::Done)
    };
    let sent = wisp_factory_core::Pending { task: "s1".into(), turn: None, since: Some(1) };
    let round1 = [turn(0, "x"), turn(1, "CHANGES: add a test")];
    assert_eq!(drive::end_of(&t, &round1, &sent), None, "round 1's turn isn't this round's: still to run");
    let round2 = [turn(1, "CHANGES: add a test"), turn(2, "APPROVE")];
    assert_eq!(drive::end_of(&t, &round2, &sent).map(|e| e.text), Some("APPROVE".into()));
    assert_eq!(drive::end_of(&t, &round1, &first()).map(|e| e.text), Some("CHANGES: add a test".into()), "no since: its last turn");
}

/// A station whose turn waits for a limit's reset (or trust, or the plan) failed the run as "it ended held": it's
/// still going.
#[test]
fn a_held_station_turn_is_still_going() {
    let t = wisp_runtime_api::model::Session { state: wisp_runtime_api::SessionState::Review, ..Default::default() };
    assert_eq!(drive::end_of(&t, &[station_turn(wisp_runtime_api::TurnStatus::Held)], &first()), None);
    let done = drive::end_of(&t, &[station_turn(wisp_runtime_api::TurnStatus::Done)], &first()).unwrap();
    assert!(done.ok && done.text == "built");
    let mut queued = t.clone();
    queued.queue.push(wisp_runtime_api::QueueItem::default());
    queued.queue_held = true;
    assert_eq!(drive::end_of(&queued, &[station_turn(wisp_runtime_api::TurnStatus::Done)], &first()), None, "a held queue waits");
}

/// A station's task deleted while the run waited on it was "can't be read" on every look: the run never ended.
#[test]
fn a_deleted_station_task_fails_and_a_passing_error_looks_again() {
    let gone = drive::unread("s1", &WireError::not_found("no task s1")).unwrap();
    assert!(!gone.ok);
    assert_eq!(gone.text, "its task was deleted");
    assert_eq!(drive::unread("s1", &WireError::unavailable("busy")), None);
}
