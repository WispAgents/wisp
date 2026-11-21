//! A lead whose turn ended while its delegates still work stays in Working on them; the check-in it scheduled goes
//! anyway (v1 #795: it is often how the lead watches them, and their report still comes after). A task in Working
//! for any other reason (its run is live, an after-turn step checks its work) or asking you still holds it.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use wisp_later_api::LaterEntry;
use wisp_later_core::{Action, Task, TaskState, due};

const NOW: i64 = 1_000_000;

fn lead(state: TaskState, busy: bool, delegates: bool) -> BTreeMap<String, Task> {
    let t = Task {
        id: "lead".into(),
        title: "lead".into(),
        agent: "claude".into(),
        account: "default".into(),
        state,
        busy,
        delegates,
        ..Task::default()
    };
    [("lead".to_owned(), t)].into()
}

fn check_in() -> Vec<LaterEntry> {
    vec![LaterEntry {
        id: "l1".into(),
        session_id: "lead".into(),
        at: NOW - 1_000,
        text: "check on the delegate".into(),
        by: "agent".into(),
        set_at: 0,
        remind: false,
        goal: None,
        held: None,
        limit: None,
        attachments: Vec::new(),
    }]
}

fn fires(tasks: &BTreeMap<String, Task>) -> bool {
    let d = due(&check_in(), tasks, &mut |_| None, NOW, 10);
    d.actions.iter().any(|a| matches!(a, Action::Fire { id, .. } if id == "l1"))
}

#[test]
fn a_lead_waiting_on_its_delegates_gets_the_check_in_it_scheduled_v1_795() {
    let waiting = lead(TaskState::Running, false, true);
    assert!(waiting["lead"].free(), "a lead in Working only on its delegates is free");
    assert!(fires(&waiting), "the check-in went, not held behind the delegates");
}

#[test]
fn a_task_in_working_for_any_other_reason_still_holds_its_check_in_v1_795() {
    // its run is live (delegates out or not)
    assert!(!fires(&lead(TaskState::Running, true, false)));
    assert!(!fires(&lead(TaskState::Running, true, true)));
    // Working with no run and nothing owed: an after-turn step (Auto's or the goal's check)
    assert!(!fires(&lead(TaskState::Running, false, false)));
    // asking you: an answer would take the follow-up's place
    let mut asking = lead(TaskState::Running, false, true);
    asking.get_mut("lead").unwrap().asking = true;
    assert!(!fires(&asking));
    // done with its turn: free, as before
    assert!(fires(&lead(TaskState::Review, false, false)));
}
