//! Every way a trigger's run ends clears it from the trigger's running list, so `concurrency` stops counting it: its
//! turn stopped (no `task.finished` comes), its task deleted, its workflow's end (`workflow.ended`: a workflow's held
//! task never runs a turn), or, heard by none of those, its task gone at a tick. Each left the trigger "skipped: already
//! running" for good. And a chain of triggers on each other's runs is cut at `MAX_DEPTH` (its depth was 0 at every
//! `trigger.fired`), as is a trigger on `task.finished` whose target is a workflow, on its own steps. Against stand-ins
//! for wisp.runtime and wisp.workflows.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_automations::AutomationsPlugin;
use wisp_automations_api::{PROVIDERS, Provided, TriggerProvider, TriggerSpec};
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup, WireError};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::events::{SessionDeleted, TaskFact, TaskFinished, TurnEnded};
use wisp_runtime_api::model::Session;
use wisp_runtime_api::spec::{Lineage, TaskSpec};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};
use wisp_workflows_api::{RunArgs, Started, WorkflowEnded};

const T0: i64 = 1_800_000_000_000;

fn manifest(id: &str) -> Manifest {
    Manifest::from_value(json!({ "wispPlugin": 1, "id": id, "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
}

#[derive(Clone, Default)]
struct World {
    made: Arc<Mutex<Vec<r::StartArgs>>>,
    /// tasks `runtime.get` no longer finds
    gone: Arc<Mutex<BTreeSet<String>>>,
    /// what `workflows.run` was asked
    runs: Arc<Mutex<Vec<RunArgs>>>,
}

impl World {
    /// how many workflow runs were asked for
    fn runs(&self) -> usize {
        self.runs.lock().unwrap().len()
    }
}

/// wisp.runtime: `runtime.create` makes t1, t2, …, each running; `runtime.spec` of a workflow step (`s1`) says its run
/// and the trigger that started it.
struct Runtime(World);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        manifest("wisp.runtime")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let w = self.0.clone();
        s.command::<r::Create, _, _>(move |_, _, a: r::StartArgs| {
            let mut made = w.made.lock().unwrap();
            made.push(a);
            let id = format!("t{}", made.len());
            async move { Ok(r::Created { id: id.as_str().into(), turn_id: None }) }
        });
        let w = self.0.clone();
        s.command::<r::Get, _, _>(move |_, _, a: r::IdArgs| {
            let gone = w.gone.lock().unwrap().contains(a.id.as_str());
            async move {
                if gone {
                    return Err(WireError::not_found(format!("no task {}", a.id.as_str())));
                }
                Ok(serde_json::from_value::<Session>(json!({ "id": a.id, "title": "t", "agent": "claude", "model": "", "effort": "",
                    "mode": "auto", "cwd": "/repo", "state": "running", "createdAt": 0, "updatedAt": 0 }))
                .unwrap())
            }
        });
        s.command::<r::Turns, _, _>(|_, _, _a: r::TurnsArgs| async move { Ok(Vec::new()) });
        s.command::<r::Stop, _, _>(|_, _, _a: r::StopArgs| async move { Ok(r::Ack {}) });
        s.command::<r::SpecOf, _, _>(|_, _, a: r::IdArgs| async move {
            let lineage = if a.id.as_str() == "s1" {
                Lineage { workflow_run: Some("w1".into()), triggered_by: Some("trigger:flow".into()), ..Lineage::default() }
            } else {
                Lineage::default()
            };
            Ok(TaskSpec { prompt: "p".into(), lineage, ..TaskSpec::default() })
        });
        Ok(())
    }
}

/// wisp.workflows: `workflows.run` answers runs `w1`, `w2`, … (each a held task of its own, as for a trigger's run).
struct Workflows(World);
impl Plugin for Workflows {
    fn manifest(&self) -> Manifest {
        manifest("wisp.workflows")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let w = self.0.clone();
        s.command::<wisp_workflows_api::Run, _, _>(move |_, _, a: RunArgs| {
            let mut runs = w.runs.lock().unwrap();
            runs.push(a);
            let id = format!("w{}", runs.len());
            async move { Ok(Started { id, said: String::new() }) }
        });
        Ok(())
    }
}

struct Brings(Vec<(String, TriggerSpec)>);
impl TriggerProvider for Brings {
    fn triggers(&self) -> Vec<Provided> {
        self.0.iter().map(|(name, spec)| Provided { name: name.clone(), spec: spec.clone(), paused: false, full_auto: false }).collect()
    }
}
struct Provider(Vec<(String, TriggerSpec)>);
impl Plugin for Provider {
    /// it offers chat messages, so a tick finds the triggers on them available
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.testprovider", "version": "1.0.0", "api": "^1",
            "contributes": { "triggerSources": ["chat.message"] } }))
        .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let p: Arc<dyn TriggerProvider> = Arc::new(Brings(self.0.clone()));
        s.contribute(&PROVIDERS, p);
        Ok(())
    }
}

async fn kernel(w: &World, clock: &FakeClock, triggers: Vec<(String, TriggerSpec)>) -> Kernel {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let log = wisp_kernel::durable::EventLogService(Arc::new(store.clone()));
    TestApp::new()
        .clock(clock)
        .service(store)
        .service(log)
        .with(AutomationsPlugin)
        .with(Runtime(w.clone()))
        .with(Workflows(w.clone()))
        .with(Provider(triggers))
        .start()
        .await
}

fn chat() -> TriggerSpec {
    TriggerSpec {
        on: "chat.message".into(),
        prompt: "Deploy it".into(),
        project: "/repo".into(),
        users: vec!["U1".into()],
        ..TriggerSpec::default()
    }
}

async fn push(k: &Kernel, n: u32) -> u64 {
    let ev = json!({ "on": "chat.message", "key": format!("chat:slack:U1:{n}"), "at": T0, "summary": "deploy",
        "payload": { "channel": "slack", "user": "U1", "text": "deploy", "trusted": true } });
    k.call(Caller::user(), "automations.trigger.push", ev).await.unwrap()["n"].as_u64().unwrap()
}

async fn log(k: &Kernel, name: &str) -> Value {
    k.call(Caller::user(), "automations.trigger.log", json!({ "name": name })).await.unwrap()
}

/// Waits until the trigger's log has a line with this result.
async fn until_logged(k: &Kernel, name: &str, result: &str) {
    for _ in 0..300 {
        if log(k, name).await.as_array().unwrap().iter().any(|l| l["result"] == result) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("no {result} in {name}'s log: {}", log(k, name).await);
}

fn emit_runtime<E: wisp_kernel::Event>(k: &Kernel, e: &E) {
    let ctx = k.ctx("wisp.runtime").unwrap();
    ctx.emit(&ctx.root_call(), e);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_run_lets_the_next_event_start() {
    let (w, clock) = (World::default(), FakeClock::at(T0));
    let k = kernel(&w, &clock, vec![("deploys".into(), chat())]).await;
    assert_eq!(push(&k, 1).await, 1);
    assert_eq!(push(&k, 2).await, 0, "one runs: skipped");
    let e: TurnEnded = serde_json::from_value(json!({ "sessionId": "t1", "turnId": "u1", "status": "stopped", "interrupted": false,
        "agent": "claude", "model": "" }))
    .unwrap();
    emit_runtime(&k, &e);
    until_logged(&k, "deploys", "stopped").await;
    assert_eq!(push(&k, 3).await, 1, "{}", log(&k, "deploys").await);
    assert!(!log(&k, "deploys").await.as_array().unwrap().iter().any(|l| l["result"] == "failed"), "a stop isn't a failure");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_deleted_runs_task_lets_the_next_event_start() {
    let (w, clock) = (World::default(), FakeClock::at(T0));
    let k = kernel(&w, &clock, vec![("deploys".into(), chat())]).await;
    assert_eq!(push(&k, 1).await, 1);
    emit_runtime(&k, &SessionDeleted { id: "t1".into() });
    until_logged(&k, "deploys", "stopped").await;
    assert_eq!(push(&k, 2).await, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workflow_targets_run_ends_with_the_workflow() {
    let (w, clock) = (World::default(), FakeClock::at(T0));
    let k = kernel(&w, &clock, vec![("flow".into(), TriggerSpec { workflow: "release".into(), ..chat() })]).await;
    assert_eq!(push(&k, 1).await, 1);
    assert_eq!(w.runs.lock().unwrap()[0].triggered_by.as_deref(), Some("trigger:flow"), "its run carries the trigger");
    assert_eq!(push(&k, 2).await, 0, "its run goes on: skipped");
    let ctx = k.ctx("wisp.workflows").unwrap();
    let e = WorkflowEnded {
        session_id: "w1".into(),
        run: "w1".into(),
        workflow: "release".into(),
        state: "done".into(),
        done: 1,
        total: 1,
        cost_usd: 0.5,
    };
    ctx.emit(&ctx.root_call(), &e);
    until_logged(&k, "flow", "done").await;
    assert_eq!(push(&k, 3).await, 1);
}

/// Moves the clock on by the engine's ticks, letting each tick's work finish.
async fn ticks(clock: &FakeClock, n: u32) {
    for _ in 0..n {
        while clock.sleepers() < 2 {
            tokio::task::yield_now().await;
        }
        clock.advance(15_000);
        for _ in 0..200 {
            tokio::task::yield_now().await;
        }
    }
    while clock.sleepers() < 2 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tick_drops_a_run_whose_task_is_gone() {
    let (w, clock) = (World::default(), FakeClock::at(T0));
    let k = kernel(&w, &clock, vec![("deploys".into(), chat())]).await;
    assert_eq!(push(&k, 1).await, 1);
    // gone without a word (deleted while wisp was off)
    w.gone.lock().unwrap().insert("t1".into());
    ticks(&clock, 2).await;
    until_logged(&k, "deploys", "stopped").await;
    assert_eq!(push(&k, 2).await, 1, "{}", log(&k, "deploys").await);
}

/// Two triggers on each other's runs (`trigger.fired`, started): each `trigger.fired` said depth 0, so they started
/// tasks for each other without end. They stop at three causes deep.
#[tokio::test(flavor = "multi_thread")]
async fn two_triggers_on_each_others_runs_stop_at_max_depth() {
    let (w, clock) = (World::default(), FakeClock::at(T0));
    let on_fired = |other: &str| {
        // a workflow each (a prompt's task needs a project, and a project would filter `trigger.fired`, which names none)
        let mut s = TriggerSpec {
            on: "trigger.fired".into(),
            workflow: "again".into(),
            concurrency: "parallel:99".into(),
            ..TriggerSpec::default()
        };
        s.filters.insert("trigger".into(), json!(other));
        s.filters.insert("result".into(), json!("started"));
        s
    };
    let k = kernel(&w, &clock, vec![("a".into(), on_fired("b")), ("b".into(), on_fired("a"))]).await;
    k.call(Caller::user(), "automations.trigger.fire", json!({ "name": "a" })).await.unwrap();
    for _ in 0..100 {
        if log(&k, "b")
            .await
            .as_array()
            .unwrap()
            .iter()
            .chain(log(&k, "a").await.as_array().unwrap())
            .any(|l| l["result"] == "skipped" && l["why"].as_str().is_some_and(|y| y.contains("causes deep")))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(w.runs(), 3, "a by hand, b, a again; then cut: a {} b {}", log(&k, "a").await, log(&k, "b").await);
}

/// A trigger on `task.finished` whose target is a workflow: its own steps' `task.finished` name it (their lineage),
/// so it never fires on them.
#[tokio::test(flavor = "multi_thread")]
async fn a_workflow_trigger_doesnt_fire_on_its_own_steps() {
    let (w, clock) = (World::default(), FakeClock::at(T0));
    let spec =
        TriggerSpec { on: "task.finished".into(), workflow: "release".into(), concurrency: "parallel:9".into(), ..TriggerSpec::default() };
    let k = kernel(&w, &clock, vec![("flow".into(), spec)]).await;
    let fact =
        |id: &str| TaskFinished(TaskFact { session_id: id.into(), status: "done".into(), cwd: "/repo".into(), ..TaskFact::default() });
    emit_runtime(&k, &fact("x1"));
    until_logged(&k, "flow", "started").await;
    emit_runtime(&k, &fact("s1"));
    until_logged(&k, "flow", "skipped").await;
    let l = log(&k, "flow").await;
    assert!(l.as_array().unwrap().iter().any(|x| x["why"] == "its own task's event (no loops)"), "{l}");
    assert_eq!(w.runs(), 1, "its step started no run");
}
