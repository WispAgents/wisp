//! Audit F2/F3: a failed call's error as a transcript step. It kept only its parser's first 400 characters (a test
//! run's failure is at its end) and named no call, so the app showed it as a row of its own after a call that looked
//! fine. Now it is kept like an output (start and end, `clip_output`) and carries the call it ends.
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value, json};
use wisp_runtime_api::harness::HarnessEvent;
use wisp_runtime_api::model::Activity;
use wisp_runtime_core::activity::{self, Effect, Head};

fn step(v: Value) -> Activity {
    let mut h = Head::default();
    match activity::apply(&mut h, &HarnessEvent::from_v1(v).unwrap(), false, &|_| false) {
        Effect::Append(a) => a,
        e => panic!("a step, not {e:?}"),
    }
}

#[test]
fn a_long_error_keeps_its_head_and_tail_and_a_short_one_stays_as_it_is() {
    let long = format!("HEAD_LINE\n{}assertion failed: TAIL_MARKER", "running test …\n".repeat(350));
    let Activity::ToolError { text, .. } = step(json!({"type":"tool_error","name":"Bash","text":long,"id":"c1"})) else {
        panic!("an error")
    };
    assert!(text.starts_with("HEAD_LINE\n"), "{text}");
    assert!(text.contains(" lines cut …\n"), "{text}");
    assert!(text.ends_with("assertion failed: TAIL_MARKER"), "{text}");
    let short = "exit 1\n  at a.rs:3\n";
    let Activity::ToolError { text, .. } = step(json!({"type":"tool_error","name":"Bash","text":short})) else { panic!() };
    assert_eq!(text, short);
}

#[test]
fn an_error_names_the_call_it_ends() {
    let a = step(json!({"type":"tool_error","name":"Bash","text":"exit 1","id":"toolu_1"}));
    assert!(matches!(&a, Activity::ToolError { call_id: Some(c), .. } if c == "toolu_1"), "{a:?}");
    assert_eq!(serde_json::to_value(&a).unwrap()["callId"], "toolu_1");
}

#[test]
fn an_error_stored_before_it_named_its_call_still_reads() {
    let old: Activity = serde_json::from_value(json!({"kind":"tool_error","name":"Bash","text":"exit 1"})).unwrap();
    assert!(matches!(old, Activity::ToolError { call_id: None, .. }));
    assert!(serde_json::to_value(&old).unwrap().get("callId").is_none(), "nothing new written for it");
}
