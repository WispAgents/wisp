//! Audit C18: copilot's `/btw` passed the task's id as `--session-id`. A side run (a /btw, a quick run, a goal's
//! judge) carried no turn id, so a harness that names a new session up front (copilot: `--session-id`, the turn's
//! id) fell back to the task's id: every side question of the task, and every judge, went into one session that is
//! no run's. Each side run gets an id of its own, never the task's (it can't resume the task's conversation without a
//! fork: copilot has none, so it gets the task's story in its prompt instead, as v1 did).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};

use common::{rig_on, until};
use wisp_kernel::BoxFuture;
use wisp_runtime::Recorder;
use wisp_runtime_api::Harness;
use wisp_runtime_api::commands::{BtwArgs, StartArgs};
use wisp_runtime_api::harness::{HarnessFound, HarnessInfo, HarnessRun, HarnessSys, Launch, RunDriver};
use wisp_runtime_api::model::TurnStatus;

/// The fake harness, recording the turn id each launch was handed.
struct Seen(Arc<dyn Harness>, Arc<Mutex<Vec<String>>>);

impl Harness for Seen {
    fn info(&self) -> &HarnessInfo {
        self.0.info()
    }
    fn detect<'a>(&'a self, sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        self.0.detect(sys)
    }
    fn launch(&self, run: &HarnessRun) -> wisp_kernel::Result<Launch> {
        self.1.lock().unwrap().push(run.turn_id.clone());
        self.0.launch(run)
    }
    fn driver(&self, run: &HarnessRun) -> Box<dyn RunDriver> {
        self.0.driver(run)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn each_side_question_runs_in_a_session_of_its_own_never_the_tasks_id_audit_c18() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = rig_on(Recorder::default(), vec![Arc::new(Seen(common::Fake::harness(), seen.clone()))]).await;
    let id = r.rt.create(StartArgs { prompt: "fix it".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id;
    until("the turn", || async { r.rt.turns(id.as_str(), None, None).await.unwrap().iter().all(|t| t.status == TurnStatus::Done) }).await;
    for q in ["why?", "and how?"] {
        r.rt.btw(BtwArgs { id: id.clone(), question: q.into(), attachments: Vec::new() }).await.unwrap();
    }
    until("both side runs", || async { seen.lock().unwrap().len() == 3 }).await;
    let ids = seen.lock().unwrap().clone();
    let turn = r.rt.turns(id.as_str(), None, None).await.unwrap()[0].id.0.clone();
    assert_eq!(ids[0], turn, "a turn runs as its own id");
    for side in &ids[1..] {
        assert!(!side.is_empty() && *side != id.0 && *side != turn, "a side run's own id, not the task's: {ids:?}");
    }
    assert_ne!(ids[1], ids[2], "each side question its own: {ids:?}");
}
