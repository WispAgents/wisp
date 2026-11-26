//! A workflow's step is a child task with a goal of its own: its turn's end is judged (a child task's never was, so the
//! step's run waited on a goal nobody judged), while a subagent's task, which shares its lead's goal, still isn't. And a
//! restart in the moment between a round writing `judging` and its first check's record leaves no record to find: the
//! goal is found by its slot, reset and judged again (it said judging for ever, and its step with it). Against a
//! stand-in wisp.runtime.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::mpsc;
use wisp_goal::GoalPlugin;
use wisp_kernel::{Kernel, Manifest, Plugin, Result, Setup, WireError};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::model::{Session, Turn, TurnStatus};
use wisp_runtime_api::points::{AFTER, TurnEnd};
use wisp_testkit::TestApp;

#[derive(Clone)]
struct Rt {
    task: Arc<Mutex<Session>>,
    /// each goal state written, in order
    wrote: mpsc::UnboundedSender<String>,
}

struct Runtime(Rt);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let rt = self.0.clone();
        s.command::<r::Get, _, _>(move |_, _, _a| {
            let t = rt.task.lock().unwrap().clone();
            async move { Ok(t) }
        });
        s.command::<r::Turns, _, _>(|_, _, _a| async move {
            Ok(vec![Turn {
                id: "u1".into(),
                session_id: "t1".into(),
                status: TurnStatus::Done,
                text: Some("done".into()),
                ..Turn::default()
            }])
        });
        let rt = self.0.clone();
        s.command::<r::Slots, _, _>(move |_, _, _a: r::SlotsArgs| {
            let t = rt.task.lock().unwrap().clone();
            async move { Ok(vec![r::TaskSlots { id: t.id.clone(), ext: t.ext.clone().into_iter().collect() }]) }
        });
        s.command::<r::Profiles, _, _>(|_, _, _a: Value| async move { Ok(Vec::new()) });
        s.command::<r::Quick, _, _>(|_, _, _a: r::QuickArgs| async move {
            Ok(r::QuickResult {
                text: "MET".into(),
                json: None,
                agent: String::new(),
                model: String::new(),
                cost_usd: None,
                activity: Vec::new(),
            })
        });
        let rt = self.0.clone();
        s.command::<r::SetExt, _, _>(move |_, _, a: r::SetExtArgs| {
            let rt = rt.clone();
            async move {
                let state = a.value.get("state").and_then(Value::as_str).unwrap_or_default().to_owned();
                let went = {
                    let mut t = rt.task.lock().unwrap();
                    let now = t.ext.get(&a.plugin).cloned().unwrap_or(Value::Null);
                    let ok = a.expect.as_ref().is_none_or(|e| e.value == now);
                    if ok {
                        t.ext.insert(a.plugin.clone(), a.value.clone());
                    }
                    ok
                };
                if !went {
                    return Err(WireError::conflict("changed meanwhile"));
                }
                rt.wrote.send(state).unwrap();
                Ok(r::Ack {})
            }
        });
        s.command::<r::FollowUp, _, _>(|_, _, _a| async move { Ok(r::FollowedUp { turn_id: None, queued: None, interrupts: false }) });
        s.command::<r::Ask, _, _>(|_, _, _a| async move { Ok(r::Asked { ask_id: "q1".into() }) });
        Ok(())
    }
}

async fn start(task: Session) -> (Kernel, mpsc::UnboundedReceiver<String>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let k = TestApp::new().with(GoalPlugin).with(Runtime(Rt { task: Arc::new(Mutex::new(task)), wrote: tx })).start().await;
    (k, rx)
}

fn child(goal: Option<Value>) -> Session {
    let mut t =
        Session { id: "t1".into(), cwd: "/tmp".into(), title: "w · build".into(), parent_id: Some("lead".into()), ..Session::default() };
    let spec = goal.map_or_else(|| json!({}), |g| json!({ "goal": g }));
    t.ext.insert("wisp.runtime".into(), json!({ "spec": spec }));
    t
}

/// A child task's turn ended done: runtime asks its after-turn steps (`runtime.after@1`) whether one holds it, and runs
/// the one that does (the goal's round).
fn turn_ended(k: &Kernel, task: &Session) {
    let ctx = k.ctx("wisp.runtime").unwrap();
    let turn = Turn { id: "u1".into(), session_id: "t1".into(), status: TurnStatus::Done, ..Turn::default() };
    let end = TurnEnd { stopped: false, is_error: false };
    if let Some((_, step)) = ctx.contributions_by(&AFTER).into_iter().find(|(_, a)| a.wants(task, &turn, end)) {
        tokio::spawn(step.run(task.clone(), turn));
    }
}

async fn next(rx: &mut mpsc::UnboundedReceiver<String>) -> Option<String> {
    tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.ok().flatten()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_child_task_with_a_goal_of_its_own_is_judged() {
    let task = child(Some(json!({ "text": "the tests pass", "judges": ["claude"] })));
    let (k, mut rx) = start(task.clone()).await;
    turn_ended(&k, &task);
    assert_eq!(next(&mut rx).await.as_deref(), Some("judging"), "its step's goal is judged");
    assert_eq!(next(&mut rx).await.as_deref(), Some("met"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_subagents_task_without_a_goal_of_its_own_is_not_judged() {
    let task = child(None);
    let (k, mut rx) = start(task.clone()).await;
    turn_ended(&k, &task);
    assert_eq!(tokio::time::timeout(Duration::from_millis(300), rx.recv()).await.ok().flatten(), None, "nothing judged");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_goal_left_judging_with_no_check_record_is_judged_again_after_a_restart() {
    let mut t = Session { id: "t1".into(), cwd: "/tmp".into(), title: "t".into(), ..Session::default() };
    t.ext.insert("wisp.runtime".into(), json!({ "spec": { "goal": { "text": "a.txt says hello", "judges": ["claude"] } } }));
    t.ext.insert("wisp.goal".into(), json!({ "state": "judging", "rounds": 0, "running": [{ "run": "r0", "name": "claude", "at": 1 }] }));
    let (_k, mut rx) = start(t).await;
    assert_eq!(next(&mut rx).await.as_deref(), Some(""), "no longer judging");
    assert_eq!(next(&mut rx).await.as_deref(), Some("judging"), "its last answer judged again");
    assert_eq!(next(&mut rx).await.as_deref(), Some("met"));
}
