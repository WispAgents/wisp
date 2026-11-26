//! Live approvals (`runtime.approval`, Claude Code's permission prompt). Two in flight at once (parallel tool calls)
//! both wait for the user, one card at a time: the second took the first's place and the first was denied with "No
//! answer from the user in time". One nobody answers for 30 minutes is denied, and taken off the task: its card stayed
//! up, the task waited on it for good, and a late Allow wrote "You allowed" for a call that never ran.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{open, rig_timed, rig_with, tmp, until};
use serde_json::{Value, json};
use wisp_runtime::Recorder;
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{ApprovalArgs, StartArgs};
use wisp_runtime_api::model::{SessionState, TurnStatus};

const ASK_TIMEOUT_MS: u64 = 30 * 60_000;

fn approval(id: &SessionId, tool: &str) -> ApprovalArgs {
    ApprovalArgs { id: id.clone(), tool: tool.into(), input: json!({ "what": tool }), tool_use_id: None }
}

async fn open_ask(r: &common::Rig, id: &str) -> Option<String> {
    let s = r.rt.get(id).await.unwrap();
    s.ext.get("wisp.runtime").and_then(|o| o.pointer("/ask/id")).and_then(Value::as_str).map(str::to_owned)
}

async fn rows(r: &common::Rig, id: &str) -> Vec<String> {
    let t = r.rt.turns(id, None, None).await.unwrap().pop().unwrap();
    r.rt.activity(t.id.as_str(), None).await.unwrap().items.iter().map(|s| format!("{:?}", s.activity)).collect()
}

/// A task whose turn runs (its CLI waits at `gate <name>`) for approvals to be asked on.
async fn running(r: &common::Rig, gate: &str) -> SessionId {
    let id = r.rt.create(StartArgs { prompt: format!("gate {gate} work"), cwd: Some(tmp()), ..StartArgs::default() }).await.unwrap().id;
    until("the turn runs", || async {
        r.rt.turns(id.as_str(), None, None).await.unwrap().first().is_some_and(|t| t.status == TurnStatus::Running)
    })
    .await;
    id
}

#[tokio::test(flavor = "multi_thread")]
async fn two_approvals_at_once_both_wait_and_each_answer_resolves_only_its_own() {
    let r = rig_with(Recorder::default()).await;
    let id = running(&r, "approvals-two").await;
    let rt = r.rt.clone();
    let a = approval(&id, "Bash");
    let first = tokio::spawn(async move { rt.approval(a).await.unwrap() });
    until("the first approval's card", || async { open_ask(&r, id.as_str()).await.is_some() }).await;
    let first_id = open_ask(&r, id.as_str()).await.unwrap();
    let rt = r.rt.clone();
    let b = approval(&id, "Write");
    let second = tokio::spawn(async move { rt.approval(b).await.unwrap() });
    until("the second approval is asked", || async {
        rows(&r, id.as_str()).await.iter().any(|x| x.contains("Needs your OK to use Write"))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(open_ask(&r, id.as_str()).await.as_deref(), Some(first_id.as_str()), "the first stays on the card");
    assert!(!first.is_finished() && !second.is_finished(), "both still wait for the user");

    r.rt.answer(&id, "Allow", Some(&first_id)).await.unwrap();
    let got = first.await.unwrap();
    assert_eq!(got["behavior"], "allow", "{got}");
    let second_id = open_ask(&r, id.as_str()).await.expect("the second comes onto the card");
    assert_ne!(second_id, first_id);
    assert_eq!(r.rt.get(id.as_str()).await.unwrap().state, SessionState::Waiting, "still waits on the second");
    assert!(!second.is_finished());

    r.rt.answer(&id, "Deny", Some(&second_id)).await.unwrap();
    let got = second.await.unwrap();
    assert_eq!(got["behavior"], "deny", "{got}");
    assert!(!got["message"].as_str().unwrap().contains("No answer"), "{got}");
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert_eq!((open_ask(&r, id.as_str()).await, s.state), (None, SessionState::Running), "nothing asked: running again");
    open("approvals-two");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_approval_nobody_answers_is_denied_and_taken_off_the_task() {
    let clock = wisp_testkit::FakeClock::at(1_700_000_000_000);
    let r = rig_timed(Recorder::default(), clock.service()).await;
    let id = running(&r, "approvals-late").await;
    let rt = r.rt.clone();
    let a = approval(&id, "Bash");
    let call = tokio::spawn(async move { rt.approval(a).await.unwrap() });
    until("its card", || async { open_ask(&r, id.as_str()).await.is_some() }).await;
    let old = open_ask(&r, id.as_str()).await.unwrap();
    // its wait polls on the kernel's clock
    until("the wait sleeps", || async { clock.sleepers() > 0 }).await;
    clock.advance(ASK_TIMEOUT_MS + 1_000);
    let got = call.await.unwrap();
    assert_eq!(got["behavior"], "deny", "{got}");
    assert!(got["message"].as_str().unwrap().contains("No answer from the user in 30 minutes"), "{got}");
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert_eq!((open_ask(&r, id.as_str()).await, s.state), (None, SessionState::Running), "no card, and it runs on");
    let before = rows(&r, id.as_str()).await;
    assert!(before.iter().any(|x| x.contains("No answer in 30 min: denied use Bash")), "{before:?}");
    // a late Allow finds nothing to allow, and writes nothing
    assert!(r.rt.answer(&id, "Allow", Some(&old)).await.is_err());
    assert_eq!(rows(&r, id.as_str()).await, before, "no \"You allowed\" for a call that never ran");
    open("approvals-late");
}

/// A newer plain question replaces the one still waiting, and the turn says so (its agent reads it was replaced);
/// an approval open on the card is never replaced by one.
#[tokio::test(flavor = "multi_thread")]
async fn a_newer_question_replaces_the_waiting_one_and_says_so_but_never_an_approval() {
    let r = rig_with(Recorder::default()).await;
    let id = running(&r, "approvals-questions").await;
    let ask = |q: &str| wisp_runtime_api::commands::AskArgs { id: id.clone(), question: q.into(), options: Vec::new(), details: None };
    let rt = r.rt.clone();
    let a = approval(&id, "Bash");
    let call = tokio::spawn(async move { rt.approval(a).await.unwrap() });
    until("the approval's card", || async { open_ask(&r, id.as_str()).await.is_some() }).await;
    let approval_id = open_ask(&r, id.as_str()).await.unwrap();
    let first = r.rt.ask(ask("Which port?")).await.unwrap();
    let second = r.rt.ask(ask("Which port, 3000 or 8080?")).await.unwrap();
    assert_eq!(open_ask(&r, id.as_str()).await.as_deref(), Some(approval_id.as_str()), "the approval stays on the card");
    // the replaced question's agent reads that it was replaced, never the false "the user did not answer (cancelled)"
    match r.rt.poll_answer(&first) {
        wisp_runtime_api::commands::PollAnswer::Answered { answer } => {
            assert!(answer.starts_with("Replaced by a newer question: “Which port, 3000 or 8080?”"), "{answer}");
        }
        other => panic!("the replaced question reads {other:?}, not that it was replaced"),
    }
    assert_eq!(r.rt.poll_answer(&second), wisp_runtime_api::commands::PollAnswer::Pending);
    assert!(rows(&r, id.as_str()).await.iter().any(|x| x.contains("Replaced by a newer question: Which port?")));
    r.rt.answer(&id, "Allow", Some(&approval_id)).await.unwrap();
    assert_eq!(call.await.unwrap()["behavior"], "allow");
    assert_eq!(open_ask(&r, id.as_str()).await.as_deref(), Some(second.as_str()), "the question comes onto the card next");
    open("approvals-questions");
}
