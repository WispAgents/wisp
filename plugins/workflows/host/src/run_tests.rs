//! Runs driven against a stand-in wisp.runtime (tasks and turns in memory) on a fake clock: what a step cost is read
//! from its task's turns, a step stopped for its timeout counts its stopped turn, a lagged wake loses nothing, and a
//! task held in Working on a run a restart cut lands.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::watch;
use wisp_kernel::{Declarative, Manifest, Plugin, Setup};
use wisp_runtime_api::model::TurnStats;
use wisp_testkit::{FakeClock, TestApp};

use super::*;
use crate::WorkflowsPlugin;

const T0: i64 = 1_700_000_000_000;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// wisp.runtime with tasks and their turns in memory. A step's task is made running, mid-turn; `runtime.stop` ends
/// its turn stopped, costing `stop_cost`. A step's `runtime.get` waits while `open` is false.
#[derive(Clone)]
struct Rt {
    tasks: Arc<Mutex<BTreeMap<String, Session>>>,
    turns: Arc<Mutex<BTreeMap<String, Vec<Turn>>>>,
    stop_cost: f64,
    stopped: Arc<Mutex<Vec<String>>>,
    open: watch::Sender<bool>,
    /// step reads waiting on `open`
    waiting: Arc<Mutex<u32>>,
}

impl Rt {
    fn new(stop_cost: f64) -> Self {
        Self {
            tasks: Arc::default(),
            turns: Arc::default(),
            stop_cost,
            stopped: Arc::default(),
            open: watch::channel(true).0,
            waiting: Arc::default(),
        }
    }

    fn put(&self, id: &str, state: SessionState, ext: Value) {
        let mut s = Session { id: id.into(), state, ..Session::default() };
        if let Value::Object(m) = ext {
            s.ext = m.into_iter().collect();
        }
        lock(&self.tasks).insert(id.to_owned(), s);
    }

    fn turn(&self, id: &str, status: TurnStatus, cost: Option<f64>) {
        let mut turns = lock(&self.turns);
        let list = turns.entry(id.to_owned()).or_default();
        let t = Turn {
            id: format!("{id}-u{}", list.len()).into(),
            session_id: id.into(),
            status,
            text: Some("done".into()),
            stats: cost.map(|c| TurnStats { cost_usd: Some(c), ..TurnStats::default() }),
            ..Turn::default()
        };
        list.push(t);
    }

    /// The step's task ends: its last turn `status`, costing `cost`, and the task in Needs review.
    fn end(&self, id: &str, status: TurnStatus, cost: f64) {
        if let Some(t) = lock(&self.turns).get_mut(id).and_then(|l| l.last_mut()) {
            t.status = status;
            t.stats = Some(TurnStats { cost_usd: Some(cost), ..TurnStats::default() });
        }
        if let Some(s) = lock(&self.tasks).get_mut(id) {
            s.state = SessionState::Review;
        }
    }

    fn get(&self, id: &str) -> Session {
        lock(&self.tasks).get(id).cloned().unwrap_or_else(|| panic!("no task {id}"))
    }

    fn steps(&self) -> Vec<String> {
        lock(&self.tasks).keys().filter(|k| k.starts_with("step-")).cloned().collect()
    }

    /// The owner's view of its first run.
    fn run(&self) -> Value {
        self.get("owner").ext.get(EXT).and_then(|v| v.pointer("/runs/0")).cloned().unwrap_or(Value::Null)
    }
}

impl Plugin for Rt {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(
            json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1", "activation": ["onStartup"] }),
        )
        .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> wisp_kernel::Result<()> {
        let me = self.clone();
        s.command::<r::Get, _, _>(move |_c, _call, a: r::IdArgs| {
            let me = me.clone();
            async move {
                if a.id.as_str().starts_with("step-") {
                    *lock(&me.waiting) += 1;
                    let mut rx = me.open.subscribe();
                    let _ = rx.wait_for(|open| *open).await;
                    *lock(&me.waiting) -= 1;
                }
                lock(&me.tasks).get(a.id.as_str()).cloned().ok_or_else(|| WireError::not_found("no such task"))
            }
        });
        let me = self.clone();
        s.command::<r::Turns, _, _>(move |_c, _call, a: r::TurnsArgs| {
            let me = me.clone();
            async move { Ok(lock(&me.turns).get(a.id.as_str()).cloned().unwrap_or_default()) }
        });
        let me = self.clone();
        s.command::<r::Create, _, _>(move |_c, _call, a: r::StartArgs| {
            let me = me.clone();
            async move {
                let id = format!("step-{}", me.steps().len());
                let mut s = Session { id: id.as_str().into(), state: SessionState::Running, ..Session::default() };
                s.ext = a.ext;
                lock(&me.tasks).insert(id.clone(), s);
                me.turn(&id, TurnStatus::Running, None);
                Ok(r::Created { id: id.as_str().into(), turn_id: None })
            }
        });
        let me = self.clone();
        s.command::<r::Stop, _, _>(move |_c, _call, a: r::StopArgs| {
            let me = me.clone();
            async move {
                lock(&me.stopped).push(a.id.as_str().to_owned());
                me.end(a.id.as_str(), TurnStatus::Stopped, me.stop_cost);
                Ok(r::Ack {})
            }
        });
        let me = self.clone();
        s.command::<r::SetExt, _, _>(move |_c, _call, a: r::SetExtArgs| {
            let me = me.clone();
            async move {
                let mut tasks = lock(&me.tasks);
                let t = tasks.get_mut(a.id.as_str()).ok_or_else(|| WireError::not_found("no such task"))?;
                if a.expect.as_ref().is_some_and(|e| t.ext.get(&a.plugin).unwrap_or(&Value::Null) != &e.value) {
                    return Err(WireError::conflict("changed meanwhile"));
                }
                t.ext.insert(a.plugin, a.value);
                Ok(r::Ack {})
            }
        });
        let me = self.clone();
        s.command::<r::Slots, _, _>(move |_c, _call, a: r::SlotsArgs| {
            let me = me.clone();
            async move {
                let tasks = lock(&me.tasks);
                Ok(tasks
                    .values()
                    .filter_map(|t| {
                        let ext: BTreeMap<String, Value> =
                            t.ext.iter().filter(|(k, _)| a.plugins.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
                        (!ext.is_empty()).then(|| r::TaskSlots { id: t.id.clone(), ext })
                    })
                    .collect::<Vec<_>>())
            }
        });
        // as runtime.settle: a task held in Working with no turn running lands in Needs review, unread
        let me = self.clone();
        s.command::<r::Settle, _, _>(move |_c, _call, a: r::SettleArgs| {
            let me = me.clone();
            async move {
                if let Some(t) = lock(&me.tasks).get_mut(a.id.as_str()).filter(|t| t.state == SessionState::Running) {
                    t.state = SessionState::Review;
                    t.unread = true;
                }
                Ok(r::Ack {})
            }
        });
        Ok(())
    }
}

async fn kernel(rt: &Rt, clock: &FakeClock) -> (wisp_kernel::Kernel, Ctx) {
    let store = Store::open(wisp_store::Location::Memory, Arc::new(wisp_store::SystemClock)).unwrap();
    let k = TestApp::new().clock(clock).service(store).with(rt.clone()).with(Declarative(WorkflowsPlugin.manifest())).start().await;
    let ctx = k.ctx(crate::PLUGIN_ID).unwrap();
    (k, ctx)
}

/// Starts `spec` under the task `owner` (made here, idle), telling it nothing at the end.
async fn run_it(ctx: &Ctx, st: &Arc<State>, rt: &Rt, spec: Value) {
    rt.put("owner", SessionState::Review, json!({}));
    let a: RunArgs = serde_json::from_value(json!({ "name": "w", "spec": spec, "task": "owner", "notify": false })).unwrap();
    start(ctx, &ctx.root_call(), st, a).await.unwrap();
}

/// Waits (on the wall clock) until `f` holds, moving the fake clock on a look at a time meanwhile.
async fn until(clock: Option<&FakeClock>, what: &str, f: impl Fn() -> bool) {
    let at = Instant::now();
    while !f() {
        assert!(at.elapsed() < Duration::from_secs(20), "{what}");
        if let Some(c) = clock {
            c.advance(LOOK_MS);
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn ended(rt: &Rt) -> bool {
    rt.run().get("state").and_then(Value::as_str).is_some_and(|s| s != "running")
}

/// A step stopped for its timeout was finished before its stopped turn ended, with the cost counted so far (none):
/// what that turn cost never reached the step nor the run.
#[tokio::test(flavor = "multi_thread")]
async fn a_step_stopped_for_its_timeout_counts_its_stopped_turn() {
    let (rt, clock) = (Rt::new(0.5), FakeClock::at(T0));
    let (_k, ctx) = kernel(&rt, &clock).await;
    let st = Arc::new(State::new());
    run_it(&ctx, &st, &rt, json!({ "tasks": { "a": { "prompt": "x", "timeout": 1 } } })).await;
    until(Some(&clock), "the run ends", || ended(&rt)).await;
    assert_eq!(lock(&rt.stopped).len(), 1, "stopped once");
    let run = rt.run();
    assert_eq!(run["state"], "failed", "{run}");
    assert_eq!(run["steps"][0]["why"], "it ran past its timeout", "{run}");
    assert_eq!(run["steps"][0]["cost"], 0.5, "the step: {run}");
    assert_eq!(run["cost"], 0.5, "the run: {run}");
}

/// The budget read only what its steps' events said: two steps stopped for their timeout cost nothing to it, and the
/// third ran past a spent budget. It's skipped.
#[tokio::test(flavor = "multi_thread")]
async fn steps_stopped_for_their_timeout_spend_the_budget() {
    let (rt, clock) = (Rt::new(0.5), FakeClock::at(T0));
    let (_k, ctx) = kernel(&rt, &clock).await;
    let st = Arc::new(State::new());
    let spec = json!({ "concurrency": 1, "budget": { "usd": 1 }, "tasks": {
        "a": { "prompt": "x", "timeout": 1 }, "b": { "prompt": "y", "timeout": 1 }, "c": { "prompt": "z" },
    } });
    run_it(&ctx, &st, &rt, spec).await;
    until(Some(&clock), "the run ends", || ended(&rt)).await;
    let run = rt.run();
    assert_eq!(run["cost"], 1.0, "{run}");
    let c = &run["steps"][2];
    assert_eq!((c["id"].as_str(), c["state"].as_str()), (Some("c"), Some("skipped")), "{run}");
    assert!(c["why"].as_str().is_some_and(|w| w.contains("budget")), "{c}");
    assert_eq!(rt.steps().len(), 2, "c never started");
}

/// A run's wakes are a broadcast: one that fell behind (`Lagged`) dropped every cost it had missed, so a step that ended
/// then counted nothing. Each step's cost is read from its task's turns.
#[tokio::test(flavor = "multi_thread")]
async fn a_lagged_wake_still_counts_every_steps_cost() {
    let (rt, clock) = (Rt::new(0.0), FakeClock::at(T0));
    let (_k, ctx) = kernel(&rt, &clock).await;
    let st = Arc::new(State::new());
    run_it(&ctx, &st, &rt, json!({ "tasks": { "a": { "prompt": "x" }, "b": { "prompt": "y" } } })).await;
    until(None, "both steps start", || rt.run()["steps"].as_array().is_some_and(|s| s.iter().all(|s| s["task"].is_string()))).await;
    // the run looks at its steps and is held there while step a ends and its wake is lost behind many others
    rt.open.send_replace(false);
    let _ = st.ended.send("someone".into());
    until(None, "the run is reading its steps", || *lock(&rt.waiting) > 0).await;
    rt.end("step-0", TurnStatus::Done, 0.25);
    let _ = st.ended.send("step-0".into());
    for i in 0..300 {
        let _ = st.ended.send(format!("other-{i}"));
    }
    rt.open.send_replace(true);
    until(None, "a is done", || rt.run()["steps"][0]["state"] == "done").await;
    rt.end("step-1", TurnStatus::Done, 0.25);
    let _ = st.ended.send("step-1".into());
    until(Some(&clock), "the run ends", || ended(&rt)).await;
    let run = rt.run();
    assert_eq!(run["state"], "done", "{run}");
    assert_eq!((run["steps"][0]["cost"].as_f64(), run["steps"][1]["cost"].as_f64()), (Some(0.25), Some(0.25)), "{run}");
    assert_eq!(run["cost"], 0.5, "{run}");
}

/// A task held in Working on its run (a held run task, or a lead whose turn ended while it ran) said Working for ever
/// after a restart cut that run: the run said stopped, and nothing landed the task. It lands, unread; a task with a turn
/// still running, or waiting on you, is left be.
#[tokio::test(flavor = "multi_thread")]
async fn a_task_held_on_a_run_a_restart_cut_lands() {
    let (rt, clock) = (Rt::new(0.0), FakeClock::at(T0));
    let (_k, ctx) = kernel(&rt, &clock).await;
    let cut = || json!({ EXT: { "runs": [{ "id": "r1", "state": "running", "steps": [{ "id": "a", "state": "running" }] }] } });
    rt.put("held", SessionState::Running, cut());
    rt.put("lead", SessionState::Running, cut());
    rt.turn("lead", TurnStatus::Done, None);
    rt.put("busy", SessionState::Running, cut());
    rt.turn("busy", TurnStatus::Running, None);
    rt.put("asks", SessionState::Waiting, cut());
    crate::recover(&ctx).await;
    for id in ["held", "lead", "busy", "asks"] {
        let v = rt.get(id).ext.get(EXT).cloned().unwrap_or_default();
        assert_eq!(v["runs"][0]["state"], "stopped", "{id}: {v}");
    }
    for id in ["held", "lead"] {
        let t = rt.get(id);
        assert_eq!((t.state, t.unread), (SessionState::Review, true), "{id} lands, unread");
    }
    assert_eq!(rt.get("busy").state, SessionState::Running, "its own turn lands it");
    assert_eq!(rt.get("asks").state, SessionState::Waiting, "it waits on you");
}
