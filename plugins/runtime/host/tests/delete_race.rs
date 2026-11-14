//! A delete that lands while a run is still starting (claimed, not spawned: its folder made ready, `before:run`, the
//! run parts) stops it there: its CLI never spawns, and nothing is written for the deleted task (a start's write put
//! the task's row back, and its turn's rows failed with 'no task'). The same for a /btw (a delete stops it and
//! waits) and a named `runtime.quick` cancelled before it spawned (a goal's judge).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};

use common::{rig, rig_with, tmp, until};
use tokio::sync::oneshot;
use wisp_kernel::BoxFuture;
use wisp_runtime::Recorder;
use wisp_runtime_api::TurnId;
use wisp_runtime_api::commands::{BtwArgs, ListArgs, QuickArgs, StartArgs};
use wisp_runtime_api::model::{Session, TurnStatus};
use wisp_runtime_api::points::RunPart;

type Pair = (oneshot::Sender<()>, oneshot::Receiver<()>);

/// Holds the first run's `ready` (its folder made ready) until the test lets it go; says when it's there.
struct Gate(Mutex<Option<Pair>>);
impl RunPart for Gate {
    fn id(&self) -> &'static str {
        "test.gate"
    }
    fn ready(&self, _s: &Session, _turn: Option<&TurnId>) -> BoxFuture<'static, Result<(), String>> {
        let once = self.0.lock().unwrap().take();
        Box::pin(async move {
            if let Some((entered, release)) = once {
                entered.send(()).unwrap();
                release.await.unwrap();
            }
            Ok(())
        })
    }
}

/// A gate, the signal that a run is held in it, and what lets it go.
fn gate() -> (Arc<Gate>, oneshot::Receiver<()>, oneshot::Sender<()>) {
    let (entered_tx, entered) = oneshot::channel();
    let (release, release_rx) = oneshot::channel();
    (Arc::new(Gate(Mutex::new(Some((entered_tx, release_rx))))), entered, release)
}

fn start(prompt: &str) -> StartArgs {
    StartArgs { prompt: prompt.into(), agent: Some("fake".into()), cwd: Some(tmp()), ..StartArgs::default() }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delete_while_a_turn_starts_spawns_no_cli_and_writes_nothing_for_the_task() {
    let (g, entered, release) = gate();
    let r = rig_with(Recorder { parts: vec![g], ..Recorder::default() }).await;
    let rt = r.rt.clone();
    let create = tokio::spawn(async move { rt.create(start("x")).await });
    entered.await.unwrap();
    let id = r.rt.list(&ListArgs::default()).pop().expect("the task is made before its first turn starts").id.0.clone();
    let (rt, del_id) = (r.rt.clone(), id.clone());
    let delete = tokio::spawn(async move { rt.delete(&del_id).await });
    until("the task's row to go", || async { r.rt.get(&id).await.is_err() }).await;
    release.send(()).unwrap();
    delete.await.unwrap().unwrap();
    let created = create.await.unwrap();
    assert!(created.is_err(), "the start refused for a deleted task: {created:?}");
    assert!(r.sim.spawned().is_empty(), "its CLI never spawned: {:?}", r.sim.spawned());
    assert!(r.rt.get(&id).await.is_err(), "the start didn't put the deleted task's row back");
    assert!(r.rt.turns(&id, None, None).await.unwrap_or_default().is_empty(), "no turn row for a deleted task");
    assert_eq!(r.rt.list(&ListArgs::default()).len(), 0, "no list row either");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delete_stops_the_tasks_btw_and_answers_once_it_let_go() {
    let r = rig().await;
    let id = r.rt.create(start("x")).await.unwrap().id.0;
    until("the turn to end", || async {
        r.rt.turns(&id, None, Some(1)).await.unwrap().pop().is_some_and(|t| t.status == TurnStatus::Done)
    })
    .await;
    r.rt.btw(BtwArgs { id: id.clone().into(), question: "hang on".into(), attachments: Vec::new() }).await.unwrap();
    until("the /btw to spawn", || async { r.sim.spawned().len() == 2 }).await;
    r.rt.delete(&id).await.unwrap();
    assert!(common::HANG_GONE.load(std::sync::atomic::Ordering::SeqCst), "the delete answered with its /btw's CLI still running");
    assert!(r.rt.get(&id).await.is_err(), "the /btw's end didn't put the deleted task's row back");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_quick_run_cancelled_while_it_starts_never_spawns() {
    let (g, entered, release) = gate();
    let r = rig_with(Recorder { parts: vec![g], ..Recorder::default() }).await;
    let q =
        QuickArgs { prompt: "is it met?".into(), cwd: tmp(), pick: None, timeout_ms: Some(60_000), json: false, run_id: Some("j1".into()) };
    let rt = r.rt.clone();
    let run = tokio::spawn(async move { rt.quick(q).await });
    entered.await.unwrap();
    assert!(r.rt.quick_cancel("j1"), "under way: it's told to stop");
    release.send(()).unwrap();
    let err = wisp_kernel::WireError::from(run.await.unwrap().expect_err("a cancelled run gives no answer"));
    assert_eq!(err.details.as_ref().and_then(|d| d["cancelled"].as_bool()), Some(true), "{err:?}");
    assert!(r.sim.spawned().is_empty(), "its CLI never spawned: {:?}", r.sim.spawned());
}
