//! TRY-23, silent runs, through wisp.budget's manifest (default deny): a run with no sign of life for 30 minutes
//! while its plan moves is told to the phone once, with Stop, about its task (`channels.send`); a step starts its
//! silence again; with *Stop a run silent for* on, it's stopped (`runtime.stop`) and the phone hears why.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use serde_json::json;
use wisp_budget::BudgetPlugin;
use wisp_channels_api as ch;
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::events::{TurnActivity, TurnStarted};
use wisp_runtime_api::hooks::{BeforeRun, BeforeRunCtx};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};

const NOW: i64 = 1_800_000_000_000;
const MIN: u64 = 60_000;
const WEEK: i64 = 7 * 24 * 3_600_000;

/// A note the phone got: (title, text, button kinds, task).
type Note = (String, String, Vec<ch::views::ActionKind>, Option<String>);

#[derive(Clone, Default)]
struct Seen {
    notes: Arc<Mutex<Vec<Note>>>,
    stopped: Arc<Mutex<Vec<String>>>,
}

fn manifest(id: &str) -> Manifest {
    Manifest::from_value(json!({ "wispPlugin": 1, "id": id, "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
}

/// wisp.runtime as far as the watch asks it: the task's title, and Stop.
struct Runtime(Seen);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        manifest("wisp.runtime")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.command::<r::Get, _, _>(|_, _, a: r::IdArgs| async move {
            Ok(wisp_runtime_api::model::Session { id: a.id, title: "Fix the flaky login test".into(), ..Default::default() })
        });
        let seen = self.0.clone();
        s.command::<r::Stop, _, _>(move |_, _, a: r::StopArgs| {
            seen.stopped.lock().unwrap().push(a.id.0);
            async move { Ok(r::Ack::default()) }
        });
        s.command::<r::TurnSetExt, _, _>(|_, _, _| async move { Ok(r::Ack::default()) });
        Ok(())
    }
}

struct Channels(Seen);
impl Plugin for Channels {
    fn manifest(&self) -> Manifest {
        manifest("wisp.channels")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let seen = self.0.clone();
        s.command::<ch::SendNotice, _, _>(move |_, _, a: ch::SendArgs| {
            let kinds = a.msg.actions.iter().map(|x| x.kind).collect();
            seen.notes.lock().unwrap().push((a.msg.title, a.msg.text, kinds, a.session_id));
            async move { Ok(ch::Sent { sent: 1 }) }
        });
        Ok(())
    }
}

async fn kernel(seen: &Seen, clock: &FakeClock) -> Kernel {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    TestApp::new().clock(clock).service(store).with(BudgetPlugin).with(Runtime(seen.clone())).with(Channels(seen.clone())).start().await
}

async fn weekly(k: &Kernel, used: f64) {
    let a = json!({ "agent": "codex", "account": "", "report": { "windows": [{ "id": "seven_day", "used": used, "resetsAt": NOW + 6 * 24 * 3_600_000, "ms": WEEK }], "complete": true } });
    k.call(Caller::user(), "budget.record", a).await.unwrap();
}

/// t1's turn u1 starts on codex (before:run tells budget its account, then it starts).
async fn start(k: &Kernel) {
    let c: BeforeRunCtx = serde_json::from_value(json!({ "sessionId": "t1", "turnId": "u1", "agent": "codex", "model": "", "effort": "",
        "mode": "auto", "cwd": "/repo", "prompt": "fix it", "by": "user", "kind": "first" }))
    .unwrap();
    let ctx = k.ctx("wisp.runtime").unwrap();
    ctx.run_hook::<BeforeRun>(&ctx.root_call(), c).await.unwrap();
    let e: TurnStarted = serde_json::from_value(json!({ "sessionId": "t1", "turnId": "u1" })).unwrap();
    ctx.emit(&ctx.root_call(), &e);
    settle().await;
}

fn step(k: &Kernel, seq: u64, at: i64) {
    let e: TurnActivity = serde_json::from_value(
        json!({ "sessionId": "t1", "turnId": "u1", "items": [{ "seq": seq, "t": at, "kind": "text", "text": "still here" }] }),
    )
    .unwrap();
    let ctx = k.ctx("wisp.runtime").unwrap();
    ctx.emit(&ctx.root_call(), &e);
}

async fn settle() {
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
}

/// Minute by minute: the watch looks once a minute.
async fn minutes(clock: &FakeClock, n: u64) {
    for _ in 0..n {
        clock.advance(MIN);
        settle().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_silent_30_minutes_while_its_plan_moves_is_told_once_with_stop() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let k = kernel(&seen, &clock).await;
    weekly(&k, 0.10).await;
    start(&k).await;
    minutes(&clock, 10).await;
    weekly(&k, 0.14).await;
    minutes(&clock, 19).await;
    assert!(seen.notes.lock().unwrap().is_empty(), "29 minutes isn't silent yet");
    minutes(&clock, 3).await;
    let notes = seen.notes.lock().unwrap().clone();
    assert_eq!(notes.len(), 1, "{notes:?}");
    let (title, text, kinds, task) = &notes[0];
    assert_eq!(title, "Fix the flaky login test");
    assert!(text.starts_with("Silent 3") && text.ends_with("spent 4% of weekly"), "{text}");
    assert_eq!((kinds.as_slice(), task.as_deref()), ([ch::views::ActionKind::Stop].as_slice(), Some("t1")));
    minutes(&clock, 20).await;
    assert_eq!(seen.notes.lock().unwrap().len(), 1, "once per silent stretch");
    assert!(seen.stopped.lock().unwrap().is_empty(), "Stop a run silent for is off by default");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_step_is_a_sign_of_life() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let k = kernel(&seen, &clock).await;
    weekly(&k, 0.10).await;
    start(&k).await;
    minutes(&clock, 20).await;
    step(&k, 1, NOW + 20 * 60_000);
    settle().await;
    weekly(&k, 0.20).await;
    minutes(&clock, 15).await;
    assert!(seen.notes.lock().unwrap().is_empty(), "35 minutes in, but its last step was 15 minutes ago");
}

#[tokio::test(flavor = "multi_thread")]
async fn with_stop_after_an_hour_a_silent_run_is_stopped_and_the_phone_hears_why() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let k = kernel(&seen, &clock).await;
    k.set_setting("budget.silentStop", Some(json!("1"))).unwrap();
    weekly(&k, 0.10).await;
    start(&k).await;
    minutes(&clock, 61).await;
    assert_eq!(*seen.stopped.lock().unwrap(), ["t1"]);
    let notes = seen.notes.lock().unwrap().clone();
    assert!(notes.iter().any(|n| n.1 == "Stopped after 1 h silent"), "{notes:?}");
    minutes(&clock, 5).await;
    assert_eq!(seen.stopped.lock().unwrap().len(), 1, "stopped once");
}
