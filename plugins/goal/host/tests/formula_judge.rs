//! Audit G2: a goal whose judge is an agent the task's own formula declares (`wisp run release --formula team.yml`,
//! never applied) is judged by that agent, resolved as the task's was (from the task's formula snapshot); a judge that
//! can't judge at all never sends the work back: the goal asks once, saying why, and no follow-up goes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_goal::GoalPlugin;
use wisp_kernel::{Caller, Manifest, Plugin, Result, Setup, WireError};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::model::{Session, Turn, TurnStatus};
use wisp_runtime_api::spec::{HashArgs, Snapshot};
use wisp_testkit::TestApp;

#[derive(Default)]
struct Seen {
    /// each judge run's harness
    picks: Vec<String>,
    follow_ups: Vec<String>,
    asks: Vec<String>,
}

#[derive(Clone)]
struct Rt {
    task: Arc<Mutex<Session>>,
    /// the task's formula snapshot (`agents`)
    formula: Value,
    seen: Arc<Mutex<Seen>>,
}

struct Runtime(Rt);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "2.0.0", "api": "^1" })).unwrap()
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
        s.command::<r::Profiles, _, _>(|_, _, _a: Value| async move { Ok(Vec::new()) });
        let rt = self.0.clone();
        s.command::<r::SnapshotOf, _, _>(move |_, _, a: HashArgs| {
            let f = rt.formula.clone();
            async move { Ok(Snapshot { hash: a.of, formula: f }) }
        });
        let rt = self.0.clone();
        s.command::<r::Quick, _, _>(move |_, _, a: r::QuickArgs| {
            let rt = rt.clone();
            async move {
                let agent = a.pick.and_then(|p| p.agent).unwrap_or_default();
                rt.seen.lock().unwrap().picks.push(agent.clone());
                // the harnesses this computer has: a formula agent's name is none of them
                if agent != "codex" {
                    return Err(WireError::invalid(format!("no harness {agent}")));
                }
                Ok(r::QuickResult { text: "MET".into(), json: None, agent, model: String::new(), cost_usd: None, activity: Vec::new() })
            }
        });
        s.command::<r::QuickCancel, _, _>(|_, _, _a: r::QuickCancelArgs| async move { Ok(r::QuickCancelled { stopped: true }) });
        let rt = self.0.clone();
        s.command::<r::SetExt, _, _>(move |_, _, a: r::SetExtArgs| {
            let rt = rt.clone();
            async move {
                let mut t = rt.task.lock().unwrap();
                let now = t.ext.get(&a.plugin).cloned().unwrap_or(Value::Null);
                if a.expect.as_ref().is_some_and(|e| e.value != now) {
                    return Err(WireError::conflict("changed meanwhile"));
                }
                t.ext.insert(a.plugin.clone(), a.value.clone());
                Ok(r::Ack {})
            }
        });
        let rt = self.0.clone();
        s.command::<r::FollowUp, _, _>(move |_, _, a: r::FollowUpArgs| {
            rt.seen.lock().unwrap().follow_ups.push(a.text);
            async move { Ok(r::FollowedUp { turn_id: None, queued: None }) }
        });
        let rt = self.0.clone();
        s.command::<r::Ask, _, _>(move |_, _, a: r::AskArgs| {
            rt.seen.lock().unwrap().asks.push(a.question);
            async move { Ok(r::Asked { ask_id: "q1".into() }) }
        });
        Ok(())
    }
}

/// A task whose goal's judge is `critic`, under a formula snapshot of `formula`; the goal checked once, settled.
async fn checked(formula: Value) -> (Arc<Mutex<Seen>>, Value) {
    let mut task = Session { id: "t1".into(), cwd: "/tmp".into(), title: "t".into(), ..Session::default() };
    task.ext
        .insert("wisp.runtime".into(), json!({ "spec": { "agent": "critic", "goal": { "text": "tests pass", "judges": ["critic"] } } }));
    let seen = Arc::new(Mutex::new(Seen::default()));
    let rt = Rt { task: Arc::new(Mutex::new(task)), formula, seen: seen.clone() };
    let k = TestApp::new().with(GoalPlugin).with(Runtime(rt.clone())).start().await;
    k.call(Caller::user(), "goal.check", json!({ "id": "t1" })).await.unwrap();
    for _ in 0..200 {
        let st = rt.task.lock().unwrap().ext.get("wisp.goal").cloned().unwrap_or_default();
        if matches!(st.get("state").and_then(Value::as_str), Some("met" | "stalled" | "again")) {
            return (seen, st);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the goal never settled: {:?}", rt.task.lock().unwrap().ext.get("wisp.goal"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_judge_the_tasks_formula_declares_judges_on_its_harness() {
    let (seen, st) = checked(json!({ "agents": [{ "name": "critic", "harness": "codex", "model": "o3" }] })).await;
    assert_eq!(seen.lock().unwrap().picks, ["codex"], "critic resolves as the task's agent did, not as a harness named critic");
    assert_eq!(st["state"], "met");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_judge_that_cant_judge_asks_once_and_never_sends_the_work_back() {
    let (seen, st) = checked(json!({})).await;
    let seen = seen.lock().unwrap();
    assert_eq!(st["state"], "stalled", "{st}");
    assert!(seen.follow_ups.is_empty(), "no round spent on a judge that never ran: {:?}", seen.follow_ups);
    assert_eq!(seen.asks.len(), 1);
    assert!(seen.asks[0].starts_with("The goal can't be judged: critic couldn't judge: no harness critic"), "{}", seen.asks[0]);
}
