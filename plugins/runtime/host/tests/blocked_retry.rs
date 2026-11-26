//! Audit doc19a: Allow & retry wrote its grants (or Full auto) on the task (`next`), then queued its retry at the back.
//! The next turn to start took them, so a prompt queued before the retry ran with the grants, or in Full auto, and the
//! retry ran without. They ride on the retry's own queue item now.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Fake, Recording, rig_on, run_of, until};
use serde_json::json;
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::{FollowUpArgs, StartArgs};
use wisp_runtime_api::model::{Mode, TurnStatus};
use wisp_runtime_core::setup::{PROJECT_ADD, Write};

#[tokio::test(flavor = "multi_thread")]
async fn allow_and_retrys_grants_go_to_the_retry_never_to_a_prompt_queued_before_it_doc19a() {
    let (fake, runs) = Recording::of(Fake::harness());
    let r = rig_on(Recorder::default(), vec![fake]).await;
    let start = StartArgs { prompt: "blocked first".into(), cwd: Some(common::tmp()), mode: Some(Mode::Edits), ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    until("the blocked turn", || async { r.rt.turns(id.as_str(), None, None).await.unwrap().iter().all(|t| t.status == TurnStatus::Done) })
        .await;
    // a card the user hasn't answered yet: what is sent now waits behind it, in the order sent
    let card = Write { command: PROJECT_ADD.into(), args: json!({ "path": "/tmp/qa-proj", "name": "qa-proj", "sid": id.as_str() }) };
    let asked = r.rt.setup_ask(&id, "Fake CLI", vec![card]).await.unwrap();
    let before = FollowUpArgs { id: id.clone(), text: "queued before".into(), queue: Some(true), ..FollowUpArgs::default() };
    r.rt.follow_up(before).await.unwrap();
    let out = r.rt.approve_blocked(&id, None, true, "user:full").await.unwrap();
    assert_eq!(out["scoped"], json!(true), "{out}");
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert!(s.ext["wisp.runtime"].get("next").is_none(), "nothing on the task for whichever turn starts first: {:?}", s.ext);
    assert_eq!(s.queue.len(), 2);
    r.rt.answer_setup(&id, "Deny", Some(&asked.ask_id), |_c, _a| async { Ok(json!({})) }).await.unwrap();

    let first = run_of(&runs, "queued before").await;
    assert_eq!(first.mode, Mode::Edits, "the prompt queued before runs in the task's mode");
    assert!(first.grants.is_empty(), "and without the retry's grants: {:?}", first.grants);
    let retry = run_of(&runs, "were blocked by your permissions").await;
    assert_eq!(retry.grants, ["Bash(npm publish)"], "the retry has them");
}

/// An unscoped Allow & retry (a refusal with no rule) runs its retry, and only its retry, in Full auto.
#[tokio::test(flavor = "multi_thread")]
async fn an_unscoped_retry_runs_in_full_auto_and_the_prompt_before_it_doesnt_doc19a() {
    let (fake, runs) = Recording::of(Fake::harness());
    let r = rig_on(Recorder::default(), vec![fake]).await;
    // the refusal names no rule wisp can grant: the retry goes in Full auto
    let start = StartArgs { prompt: "blockedwide first".into(), cwd: Some(common::tmp()), mode: Some(Mode::Edits), ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    until("the blocked turn", || async { r.rt.turns(id.as_str(), None, None).await.unwrap().iter().all(|t| t.status == TurnStatus::Done) })
        .await;
    let card = Write { command: PROJECT_ADD.into(), args: json!({ "path": "/tmp/qa-proj", "name": "qa-proj", "sid": id.as_str() }) };
    let asked = r.rt.setup_ask(&id, "Fake CLI", vec![card]).await.unwrap();
    let before = FollowUpArgs { id: id.clone(), text: "queued before".into(), queue: Some(true), ..FollowUpArgs::default() };
    r.rt.follow_up(before).await.unwrap();
    let out = r.rt.approve_blocked(&id, None, true, "user:full").await.unwrap();
    assert_eq!(out["scoped"], json!(false), "{out}");
    r.rt.answer_setup(&id, "Deny", Some(&asked.ask_id), |_c, _a| async { Ok(json!({})) }).await.unwrap();

    let first = run_of(&runs, "queued before").await;
    assert_eq!(first.mode, Mode::Edits, "the prompt queued before isn't run in Full auto");
    let retry = run_of(&runs, "were blocked by your permissions").await;
    assert_eq!(retry.mode, Mode::Auto, "the retry is");
}
