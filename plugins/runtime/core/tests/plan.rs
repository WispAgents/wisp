//! Plan that runs commands (v1 #794, test/plan-mode.test.js `outsideTask`): a task you start in Plan may run commands;
//! a side discussion's, a channel's, a trigger's or a capped task's (and their subagents') Plan stays read-only.
#![allow(clippy::unwrap_used)]

use wisp_runtime_api::model::Mode;
use wisp_runtime_core::plan::{MAX_LEADS, Origin, outside, runs_commands};

const YOURS: Origin<'static> = Origin { capped: false, triggered_by: None, started_by: None };

fn by(b: &'static str) -> Origin<'static> {
    Origin { triggered_by: Some(b), ..YOURS }
}

#[test]
fn a_channels_a_triggers_or_a_capped_task_and_its_subagents_count_as_outside_v1_794() {
    assert!(!outside(&[YOURS]), "yours, from the page");
    assert!(!outside(&[by("hand")]), "your declared task, run by hand");
    assert!(!outside(&[by("schedule:nightly")]), "your schedule");
    assert!(!outside(&[by("pipeline:review")]), "your pipeline");
    assert!(outside(&[by("trigger:on-issue")]), "a trigger's");
    assert!(outside(&[Origin { capped: true, ..YOURS }]), "a channel's (capped)");
    assert!(outside(&[YOURS, Origin { capped: true, ..YOURS }]), "a subagent goes by its lead");
    assert!(outside(&[YOURS, YOURS, by("trigger:t")]), "and its lead's lead");
    assert!(outside(&[Origin { started_by: Some("channel:slack"), ..YOURS }]), "a channel's, its cap lifted");
    // a chain deeper than v1's walk: the far end isn't read
    let mut deep = vec![YOURS; MAX_LEADS + 1];
    deep.push(by("trigger:t"));
    assert!(!outside(&deep));
}

#[test]
fn only_plan_on_a_task_of_your_own_that_is_no_side_discussion_runs_commands_v1_794() {
    assert!(runs_commands(Mode::Plan, false, false));
    assert!(!runs_commands(Mode::Plan, true, false), "a side discussion's Plan only reads");
    assert!(!runs_commands(Mode::Plan, false, true), "a channel's or trigger's Plan only reads");
    assert!(!runs_commands(Mode::Edits, false, false), "only Plan");
    assert!(!runs_commands(Mode::Auto, false, false));
}

/// v1 #794 side.js: a side discussion behind the gate is told it can do anything a task can (run commands, start
/// tasks, use the connectors) and to run what it would suggest, not that it can't start tasks.
#[test]
fn a_side_discussion_behind_the_gate_is_told_it_runs_commands_and_starts_tasks_v1_794() {
    use wisp_runtime_core::prompt::side::{SideOf, side_note};
    let n = side_note(SideOf { id: "t1", title: "Fix login" }, "mcp__wisp__wisp_session", true, "Claude Code");
    assert!(n.contains(
        "You can do anything a task can: run commands, tests, builds and the app, start tasks, use wisp's tools and the connectors"
    ));
    assert!(n.contains(
        "Never hand the user a command to run, or tell them to ask the main task for something, that you could do here yourself"
    ));
    assert!(!n.contains("Don't start changes of your own") && !n.contains("can't start tasks"), "{n}");
    assert!(n.contains("Lines it changed are a clash"), "writes still go through the clash check");
    let ro = side_note(SideOf { id: "t1", title: "Fix login" }, "", false, "Codex");
    assert!(ro.contains("Change nothing"), "a harness that can't ask before a write still only reads");
}
