//! A new task's first turn is starting from the moment the task exists: while its folder is made ready (a worktree
//! brought back, `before:run`), a Stop stops it and a follow-up waits behind it. `create` started its turn with no
//! Starting mark, so the queue read the task as idle then: the Stop reached nothing and the turn ran, and a follow-up
//! started a second turn 0 beside the first.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};

use common::{rig_with, tmp, until};
use tokio::sync::oneshot;
use wisp_kernel::BoxFuture;
use wisp_runtime::Recorder;
use wisp_runtime_api::TurnId;
use wisp_runtime_api::commands::{FollowUpArgs, StartArgs};
use wisp_runtime_api::model::{Session, SessionState, TurnStatus};
use wisp_runtime_api::points::RunPart;

type Pair = (oneshot::Sender<()>, oneshot::Receiver<()>);

/// Holds the first turn's folder check until the test lets it go (a worktree slow to come back).
struct SlowFolder(Mutex<Option<Pair>>);
impl RunPart for SlowFolder {
    fn id(&self) -> &'static str {
        "test.slow"
    }
    fn ready(&self, _s: &Session, turn: Option<&TurnId>) -> BoxFuture<'static, Result<(), String>> {
        let once = turn.and_then(|_| self.0.lock().unwrap().take());
        Box::pin(async move {
            if let Some((entered, release)) = once {
                entered.send(()).unwrap();
                release.await.unwrap();
            }
            Ok(())
        })
    }
}

async fn gated(prompt: &str) -> (common::Rig, oneshot::Receiver<()>, oneshot::Sender<()>, tokio::task::JoinHandle<String>) {
    let (entered_tx, entered) = oneshot::channel();
    let (release, release_rx) = oneshot::channel();
    let slow = Arc::new(SlowFolder(Mutex::new(Some((entered_tx, release_rx)))));
    let r = rig_with(Recorder { parts: vec![slow], ..Recorder::default() }).await;
    let rt = r.rt.clone();
    let prompt = prompt.to_owned();
    let create = tokio::spawn(async move { rt.create(StartArgs { prompt, cwd: Some(tmp()), ..StartArgs::default() }).await.unwrap().id.0 });
    (r, entered, release, create)
}

fn the_task(r: &common::Rig) -> String {
    r.rt.list(&wisp_runtime_api::commands::ListArgs::default())[0].id.0.clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stop_while_a_new_tasks_first_turn_starts_stops_it() {
    let (r, entered, release, create) = gated("slow work").await;
    entered.await.unwrap();
    let id = the_task(&r);
    r.rt.stop(&id, false).await.unwrap();
    release.send(()).unwrap();
    create.await.unwrap();
    until("the first turn ends", || async {
        r.rt.turns(&id, None, None).await.unwrap().first().is_some_and(|t| t.status != TurnStatus::Running)
    })
    .await;
    let t = r.rt.turns(&id, None, None).await.unwrap();
    assert_eq!((t.len(), t[0].status), (1, TurnStatus::Stopped), "the Stop reached the starting turn: {t:?}");
    until("the task lands", || async { r.rt.get(&id).await.unwrap().state == SessionState::Review }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_follow_up_while_a_new_tasks_first_turn_starts_waits_behind_it() {
    let (r, entered, release, create) = gated("first").await;
    entered.await.unwrap();
    let id = the_task(&r);
    let f = FollowUpArgs { id: id.as_str().into(), text: "second".into(), queue: Some(true), ..FollowUpArgs::default() };
    let out = r.rt.follow_up(f).await.unwrap();
    assert!(out.turn_id.is_none() && out.queued.is_some(), "queued behind the starting turn: {out:?}");
    release.send(()).unwrap();
    create.await.unwrap();
    until("both turns end", || async {
        let t = r.rt.turns(&id, None, None).await.unwrap();
        t.len() == 2 && t.iter().all(|t| t.status == TurnStatus::Done)
    })
    .await;
    let t = r.rt.turns(&id, None, None).await.unwrap();
    assert_eq!(
        t.iter().map(|t| (t.idx, t.prompt.as_str())).collect::<Vec<_>>(),
        [(0, "first"), (1, "second")],
        "one turn 0, then the follow-up"
    );
}
