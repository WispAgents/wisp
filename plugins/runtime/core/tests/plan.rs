//! Plan that runs commands (v1 #794, test/plan-mode.test.js `outsideTask`): a task you start in Plan may run commands,
//! and so may a side discussion; a channel's, a trigger's or a capped task's (and their subagents') Plan stays read-only.
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
fn plan_on_a_task_of_your_own_and_any_side_discussion_runs_commands_v1_794() {
    assert!(runs_commands(Mode::Plan, false, false));
    // audit user-side-full-access: a side discussion has its main task's access, commands included
    assert!(runs_commands(Mode::Plan, true, false), "a side discussion runs commands");
    assert!(runs_commands(Mode::Edits, true, false));
    assert!(!runs_commands(Mode::Plan, false, true), "a channel's or trigger's Plan only reads");
    assert!(!runs_commands(Mode::Edits, false, false), "only Plan");
    assert!(!runs_commands(Mode::Auto, false, false));
}

/// v1 #794 side.js: a side discussion is told it can do anything a task can (run commands, start tasks, use the
/// connectors) and to run what it would suggest. Audit user-side-edits-for-approval: what it hears about its writes is
/// what wisp does: a write to a file the main task changed is proposed there; no clash card, no checks that don't exist.
#[test]
fn a_side_discussion_is_told_it_has_the_main_tasks_access_and_where_its_writes_go() {
    use wisp_runtime_core::prompt::side::{SideOf, side_note};
    let n = side_note(SideOf { id: "t1", title: "Fix login" }, "mcp__wisp__wisp_session", true);
    assert!(n.contains(
        "You have the main task's access: run commands, tests, builds and the app, start tasks, use wisp's tools and the connectors"
    ));
    assert!(n.contains(
        "Never hand the user a command to run, or tell them to ask the main task for something, that you could do here yourself"
    ));
    assert!(n.contains("your change goes to the main task as a proposal") && n.contains("proposed to the main task"), "{n}");
    assert!(!n.contains("clash") && !n.contains("card here") && !n.contains("with a warning"), "nothing wisp doesn't do: {n}");
    // a harness that can't let wisp see a write first: its writes land, and it is told so (no more read-only side)
    let open = side_note(SideOf { id: "t1", title: "Fix login" }, "", false);
    assert!(!open.contains("Change nothing") && open.contains("nothing checks them first"), "{open}");
    assert!(open.contains("You have the main task's access"));
}
