//! Start now on a held turn after a Stop held the queue: the held turn starts (pressing it is the go-ahead for that
//! turn), and what's queued behind it stays held. It only went into the in-memory queue, which the Stop's hold kept
//! from starting, and the call answered Ok with nothing started. When it really can't start, it says so (Conflict), so
//! the page's optimistic start rolls back.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{rig_with, tmp, until};
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::{FollowUpArgs, StartArgs};
use wisp_runtime_api::model::TurnStatus;

/// A task whose turn waits (held), with a follow-up queued behind it and the queue held by a Stop.
async fn held_with_a_held_queue() -> (common::Rig, String, String) {
    let r = rig_with(Recorder { hold: Some("limit".into()), ..Recorder::default() }).await;
    let id = r.rt.create(StartArgs { prompt: "held work".into(), cwd: Some(tmp()), ..StartArgs::default() }).await.unwrap().id.0;
    let f = FollowUpArgs { id: id.as_str().into(), text: "after it".into(), ..FollowUpArgs::default() };
    r.rt.follow_up(f).await.unwrap();
    r.rt.stop(&id, false).await.unwrap();
    let s = r.rt.get(&id).await.unwrap();
    assert_eq!((s.queue.len(), s.queue_held), (1, true), "the follow-up waits, held: {:?}", s.queue);
    let t = r.rt.turns(&id, None, None).await.unwrap();
    assert_eq!((t.len(), t[0].status), (1, TurnStatus::Held), "{t:?}");
    let tid = t[0].id.0.clone();
    (r, id, tid)
}

#[tokio::test(flavor = "multi_thread")]
async fn start_now_starts_a_held_turn_past_a_stops_hold_and_keeps_the_rest_held() {
    let (r, id, tid) = held_with_a_held_queue().await;
    r.rt.release(&id, &tid, true, true).await.unwrap();
    until("the held turn runs and ends", || async {
        r.rt.turns(&id, None, None).await.unwrap().first().is_some_and(|t| t.status == TurnStatus::Done)
    })
    .await;
    assert!(r.sim.spawned().iter().any(|s| s.args.first().is_some_and(|p| p == "held work")), "its CLI ran");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let s = r.rt.get(&id).await.unwrap();
    assert_eq!((s.queue.len(), s.queue_held), (1, true), "the follow-up behind it stays held");
    assert_eq!(r.rt.turns(&id, None, None).await.unwrap().len(), 1, "only the released turn ran");
}

#[tokio::test(flavor = "multi_thread")]
async fn start_now_that_cant_start_says_so() {
    let (r, id, tid) = held_with_a_held_queue().await;
    let writes = vec![wisp_runtime_core::setup::Write {
        command: wisp_runtime_core::setup::PROJECT_ADD.into(),
        args: serde_json::json!({ "path": "/tmp/qa-proj", "name": "qa-proj", "sid": id }),
    }];
    r.rt.setup_ask(&id.as_str().into(), "Fake CLI", writes).await.unwrap();
    let e = r.rt.release(&id, &tid, true, true).await.unwrap_err();
    assert!(matches!(e, wisp_runtime::error::RuntimeError::Conflict(_)), "{e}");
    let t = r.rt.turns(&id, None, None).await.unwrap();
    assert_eq!(t[0].status, TurnStatus::Held, "still waiting: {t:?}");
}
