//! A turn whose answer leaves a background task up (a dev server). Chat mode (the default) and blocks off ask for
//! prose, so the prose answer ends the turn: the drive read every such answer as the blocks protocol's fallback and
//! waited on the dev server for good, the follow-ups queued behind it never starting. The blocks protocol's fallback
//! still waits ("I'll report once it's done"), but not past `BACKGROUND_QUIET_MS` of silence.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{rig_timed, rig_with, tmp, until};
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::{FollowUpArgs, StartArgs};
use wisp_runtime_api::model::TurnStatus;

fn ended(r: &common::Rig, id: &str, n: usize) -> impl std::future::Future<Output = bool> {
    let (rt, id) = (r.rt.clone(), id.to_owned());
    async move { rt.turns(&id, None, None).await.unwrap().get(n).is_some_and(|t| t.status != TurnStatus::Running) }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_mode_answer_that_leaves_a_dev_server_up_ends_its_turn_and_the_follow_up_runs() {
    let r = rig_with(Recorder::default()).await;
    let id = r.rt.create(StartArgs { prompt: "devserver start it".into(), cwd: Some(tmp()), ..StartArgs::default() }).await.unwrap().id;
    until("the turn with the dev server up ends", || ended(&r, id.as_str(), 0)).await;
    let t = r.rt.turns(id.as_str(), None, None).await.unwrap();
    assert_eq!(t[0].status, TurnStatus::Done, "{:?}", t[0]);
    // its CLI lingers, then is let go; a follow-up runs after it
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "next".into(), queue: Some(true), ..FollowUpArgs::default() }).await.unwrap();
    until("the follow-up's turn ends", || ended(&r, id.as_str(), 1)).await;
    let t = r.rt.turns(id.as_str(), None, None).await.unwrap();
    assert_eq!((t.len(), t[1].status), (2, TurnStatus::Done), "{t:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_blocks_protocol_prose_answer_waits_on_its_background_shell_until_the_quiet_deadline() {
    let clock = wisp_testkit::FakeClock::at(1_700_000_000_000);
    let settings = [("settings.transcript".to_owned(), serde_json::Value::Bool(false))].into();
    let r = rig_timed(Recorder { settings, ..Recorder::default() }, clock.service()).await;
    let id = r.rt.create(StartArgs { prompt: "devserver start it".into(), cwd: Some(tmp()), ..StartArgs::default() }).await.unwrap().id;
    // the waiting line is in, and the deadline armed on the kernel's clock
    until("the turn waits on its dev server", || {
        let rt = r.rt.clone();
        let id = id.0.clone();
        async move {
            let Some(t) = rt.turns(&id, None, None).await.unwrap().pop() else { return false };
            let page = rt.activity(t.id.as_str(), None).await.unwrap();
            page.items.iter().any(|s| format!("{:?}", s.activity).contains("Waiting on background task"))
        }
    })
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(r.rt.turns(id.as_str(), None, None).await.unwrap()[0].status, TurnStatus::Running, "the blocks protocol's fallback waits");
    clock.advance(u64::try_from(wisp_runtime_core::activity::BACKGROUND_QUIET_MS).unwrap() - 60_000);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(r.rt.turns(id.as_str(), None, None).await.unwrap()[0].status, TurnStatus::Running, "not before its deadline");
    clock.advance(60_001);
    until("the quiet deadline ends the turn", || ended(&r, id.as_str(), 0)).await;
    let t = r.rt.turns(id.as_str(), None, None).await.unwrap();
    assert_eq!(t[0].status, TurnStatus::Done, "it ends with the answer it gave, not an error: {:?}", t[0]);
    assert_eq!(t[0].text.as_deref(), Some("The dev server is up on http://localhost:3000."));
}
