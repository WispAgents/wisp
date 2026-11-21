//! A Stop that lands while a turn is starting (saved as running, its CLI not spawned yet) stops it: Windows CI's
//! `serve_task follow_up_queue_and_stop_keep_the_queue` saw the second turn running, queued a third, pressed Stop, and
//! the turn ran on forever. Stop only reached a run already in `Live`; a slow spawn came after it and nothing told it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};

use common::{rig_with, tmp, until};
use tokio::sync::oneshot;
use wisp_kernel::BoxFuture;
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::{FollowUpArgs, StartArgs};
use wisp_runtime_api::model::{Session, TurnStatus};
use wisp_runtime_api::points::{PromptPart, TurnNote};

type Pair = (oneshot::Sender<()>, oneshot::Receiver<()>);

/// Holds the turn whose prompt is `gate` between its save as running and its spawn (a turn note is read there).
struct Gate(Mutex<Option<Pair>>);
impl PromptPart for Gate {
    fn id(&self) -> &'static str {
        "test.gate"
    }
    fn part(&self, _s: &Session) -> String {
        String::new()
    }
    fn before(&self, note: TurnNote) -> BoxFuture<'static, String> {
        let once = (note.turn.prompt == "gate").then(|| self.0.lock().unwrap().take()).flatten();
        Box::pin(async move {
            if let Some((entered, release)) = once {
                entered.send(()).unwrap();
                release.await.unwrap();
            }
            String::new()
        })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stop_while_the_turn_starts_stops_it_once_spawned_and_keeps_the_queue() {
    let (entered_tx, entered) = oneshot::channel();
    let (release, release_rx) = oneshot::channel();
    let gate = Arc::new(Gate(Mutex::new(Some((entered_tx, release_rx)))));
    let r = rig_with(Recorder { prompts: vec![gate], ..Recorder::default() }).await;
    let id = r.rt.create(StartArgs { prompt: "first".into(), cwd: Some(tmp()), ..StartArgs::default() }).await.unwrap().id;
    let done = |n: usize| {
        let rt = r.rt.clone();
        let id = id.0.clone();
        async move { rt.turns(&id, None, None).await.unwrap().get(n).is_some_and(|t| t.status != TurnStatus::Running) }
    };
    until("the first turn", || done(0)).await;
    let rt = r.rt.clone();
    let fid = id.clone();
    let second = tokio::spawn(async move { rt.follow_up(FollowUpArgs { id: fid, text: "gate".into(), ..FollowUpArgs::default() }).await });
    entered.await.unwrap();
    // the second turn is saved as running and its CLI on its way: a follow-up queues, then Stop
    let t = r.rt.turns(id.as_str(), None, None).await.unwrap();
    assert_eq!((t.len(), t[1].status), (2, TurnStatus::Running), "{t:?}");
    let q = r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "third".into(), queue: Some(true), ..FollowUpArgs::default() }).await;
    assert!(q.unwrap().queued.is_some(), "queued behind the starting turn");
    r.rt.stop(id.as_str(), false).await.unwrap();
    release.send(()).unwrap();
    second.await.unwrap().unwrap();
    until("the second turn ends", || done(1)).await;
    let t = r.rt.turns(id.as_str(), None, None).await.unwrap();
    assert_eq!(t[1].status, TurnStatus::Stopped, "Stop reached the run that spawned after it: {:?}", t[1]);
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert_eq!((s.queue.len(), s.queue_held), (1, true), "the queue kept and held");
    assert_eq!(t.len(), 2, "nothing more started");
}
