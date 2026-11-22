//! wisp.budget's calls into other plugins, through its manifest (default deny, ADR-0008): Drain watch's phone note
//! (`channels.send`) and starting a held run (`runtime.release`) reach them; and an early reset that frees the plan
//! tells the phone by itself when wisp.later is off (later sends that note when it's on, so it's never sent twice).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use serde_json::json;
use wisp_budget::BudgetPlugin;
use wisp_channels_api as ch;
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::events::TurnEnded;
use wisp_runtime_api::hooks::{BeforeRun, BeforeRunCtx};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};

const NOW: i64 = 1_800_000_000_000;
const H: i64 = 3_600_000;

#[derive(Clone, Default)]
struct Seen {
    notes: Arc<Mutex<Vec<(String, String)>>>,
    released: Arc<Mutex<Vec<String>>>,
}
impl Seen {
    fn notes(&self) -> Vec<(String, String)> {
        self.notes.lock().unwrap().clone()
    }
    fn released(&self) -> Vec<String> {
        self.released.lock().unwrap().clone()
    }
}

fn manifest(id: &str) -> Manifest {
    Manifest::from_value(json!({ "wispPlugin": 1, "id": id, "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
}

/// wisp.runtime as far as budget calls it: `runtime.release` starts a held turn.
struct Runtime(Seen);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        manifest("wisp.runtime")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let seen = self.0.clone();
        s.command::<r::Release, _, _>(move |_, _, a: r::ReleaseArgs| {
            seen.released.lock().unwrap().push(a.turn_id.0);
            async move { Ok(r::Ack::default()) }
        });
        s.command::<r::TurnSetExt, _, _>(|_, _, _| async move { Ok(r::Ack::default()) });
        Ok(())
    }
}

/// wisp.channels: the phone's notes.
struct Channels(Seen);
impl Plugin for Channels {
    fn manifest(&self) -> Manifest {
        manifest("wisp.channels")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let seen = self.0.clone();
        s.command::<ch::SendNotice, _, _>(move |_, _, a: ch::SendArgs| {
            seen.notes.lock().unwrap().push((a.msg.title, a.msg.text));
            async move { Ok(ch::Sent { sent: 1 }) }
        });
        Ok(())
    }
}

/// wisp.later is on (it sends the early-reset note itself; this stand-in sends nothing).
struct Later;
impl Plugin for Later {
    fn manifest(&self) -> Manifest {
        manifest("wisp.later")
    }
    fn setup(&self, _: &mut Setup) -> Result<()> {
        Ok(())
    }
}

async fn kernel(seen: &Seen, clock: &FakeClock, later: bool) -> Kernel {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let app = TestApp::new().clock(clock).service(store).with(BudgetPlugin).with(Runtime(seen.clone())).with(Channels(seen.clone()));
    if later { app.with(Later).start().await } else { app.start().await }
}

async fn eventually(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..300 {
        if ok() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("{what}");
}

async fn settle() {
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
}

async fn record(k: &Kernel, window: &str, used: f64, resets_at: i64, ms: i64) {
    let a = json!({ "agent": "codex", "account": "", "report": { "windows": [{ "id": window, "used": used, "resetsAt": resets_at, "ms": ms }], "complete": true } });
    k.call(Caller::user(), "budget.record", a).await.unwrap();
}

fn ended(k: &Kernel, n: u32) {
    let e: TurnEnded = serde_json::from_value(json!({ "sessionId": format!("t{n}"), "turnId": format!("u{n}"), "status": "done",
        "interrupted": false, "costUsd": 1.0, "agent": "codex", "model": "gpt-5" }))
    .unwrap();
    let ctx = k.ctx("wisp.runtime").unwrap();
    ctx.emit(&ctx.root_call(), &e);
}

/// v1 index.js:145 `onDrain`: five runs took a point of the weekly window each, then two took five each: the plan moves
/// 5× faster for the same work, and with Drain watch on the phone hears it through `channels.send`.
#[tokio::test(flavor = "multi_thread")]
async fn drain_watchs_note_reaches_the_phone_through_channels_send() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let k = kernel(&seen, &clock, true).await;
    k.set_setting("budget.drainWatch", Some(json!(true))).unwrap();
    let (week, resets) = (7 * 24 * H, NOW + 6 * 24 * H);
    let mut used = 0.10;
    record(&k, "seven_day", used, resets, week).await;
    for (i, d) in [0.01, 0.01, 0.01, 0.01, 0.01, 0.05, 0.05].into_iter().enumerate() {
        if i == 5 {
            // the usual runs are hours ago; a fresh reading starts the recent ones
            clock.advance(3 * 3_600_000);
            record(&k, "seven_day", used, resets, week).await;
        }
        clock.advance(60_000);
        used += d;
        record(&k, "seven_day", used, resets, week).await;
        ended(&k, u32::try_from(i).unwrap());
        settle().await;
    }
    eventually("Drain watch tells the phone", || !seen.notes().is_empty()).await;
    let notes = seen.notes();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].0.contains("faster"), "{notes:?}");
}

/// The plan is past the stop line: a crew run is held. Then a reading shows the window reset early.
async fn held_then_reset_early(k: &Kernel) {
    k.call(Caller::user(), "budget.setStop", json!({ "key": "autoStopCodex", "value": "75" })).await.unwrap();
    record(k, "five_hour", 0.8, NOW + 3 * H, 5 * H).await;
    let c: BeforeRunCtx = serde_json::from_value(json!({ "sessionId": "t1", "turnId": "u1", "agent": "codex", "model": "", "effort": "",
        "mode": "auto", "cwd": "/repo", "prompt": "nightly tidy", "by": "crew", "kind": "first" }))
    .unwrap();
    let ctx = k.ctx("wisp.runtime").unwrap();
    let c = ctx.run_hook::<BeforeRun>(&ctx.root_call(), c).await.unwrap();
    assert_eq!(c.hold.as_ref().map(|h| h.kind.as_str()), Some("plan"), "{c:?}");
    // OpenAI banked a reset: 2% used, the next reset five hours away
    record(k, "five_hour", 0.02, NOW + 5 * H, 5 * H).await;
}

/// Start now on a crew run held at the stop line is a yes (ADR-0091 §5): the same run, sent again with `confirmLimit`
/// (`runtime.release` start), runs. It was held again at once, so Start now seemed to do nothing.
#[tokio::test(flavor = "multi_thread")]
async fn start_now_runs_a_crew_run_held_at_the_stop_line() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let k = kernel(&seen, &clock, false).await;
    k.call(Caller::user(), "budget.setStop", json!({ "key": "autoStopCodex", "value": "75" })).await.unwrap();
    record(&k, "five_hour", 0.8, NOW + 3 * H, 5 * H).await;
    let run = json!({ "sessionId": "t1", "turnId": "u1", "agent": "codex", "model": "", "effort": "",
        "mode": "auto", "cwd": "/repo", "prompt": "check in", "by": "crew", "kind": "first" });
    let ctx = k.ctx("wisp.runtime").unwrap();
    let held = ctx.run_hook::<BeforeRun>(&ctx.root_call(), serde_json::from_value(run.clone()).unwrap()).await.unwrap();
    assert_eq!(held.hold.as_ref().map(|h| h.kind.as_str()), Some("plan"), "{held:?}");
    let mut yes = run;
    yes["confirmLimit"] = json!(true);
    let c = ctx.run_hook::<BeforeRun>(&ctx.root_call(), serde_json::from_value(yes).unwrap()).await.unwrap();
    assert!(c.hold.is_none(), "Start now is held again: {c:?}");
}

/// F-auto-12 with wisp.later off: the held run starts (`runtime.release`) and budget itself tells the phone, in v1's
/// words; nobody else would.
#[tokio::test(flavor = "multi_thread")]
async fn with_later_off_an_early_reset_tells_the_phone_itself() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let k = kernel(&seen, &clock, false).await;
    held_then_reset_early(&k).await;
    eventually("the held run starts", || seen.released() == ["u1"]).await;
    eventually("the phone hears it", || !seen.notes().is_empty()).await;
    assert_eq!(
        seen.notes(),
        [("\u{21bb} codex reset early".to_owned(), "Its plan reset before its time. 1 task waiting on it starts now.".to_owned())]
    );
}

/// With wisp.later on, later sends that one note (counting its follow-ups too): budget sends none of its own.
#[tokio::test(flavor = "multi_thread")]
async fn with_later_on_budget_sends_no_second_note() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let k = kernel(&seen, &clock, true).await;
    held_then_reset_early(&k).await;
    eventually("the held run starts", || seen.released() == ["u1"]).await;
    settle().await;
    assert!(seen.notes().is_empty(), "later's to send: {:?}", seen.notes());
}

struct Heard;
impl wisp_kernel::Command for Heard {
    const ID: &'static str = "ear.heard";
    type Args = ();
    type Out = usize;
}

/// A subscriber that answers from what it heard of `budget.drain`, after its handlers took what came before (ADR-0073).
struct Ear(Arc<Mutex<usize>>);
impl Plugin for Ear {
    fn manifest(&self) -> Manifest {
        manifest("wisp.ear")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let n = self.0.clone();
        s.on::<wisp_budget_api::events::DrainNoted, _, _>(move |_, _| {
            let n = n.clone();
            async move { *n.lock().unwrap() += 1 }
        });
        let n = self.0.clone();
        s.command::<Heard, _, _>(move |ctx, _, ()| {
            let n = n.clone();
            async move {
                ctx.events_delivered(std::time::Duration::from_secs(10)).await;
                Ok(*n.lock().unwrap())
            }
        });
        Ok(())
    }
}

/// ADR-0074: Drain watch's note goes out with the save of the spend that drained. The store's commit listener is the
/// reader (the row is visible, the save hasn't returned): on the first commit whose account row shows the drain, it
/// asks a subscriber that answers after its handlers took what came before, and the note is in the answer. (Saved,
/// then emitted, the answer there was 0.)
#[tokio::test(flavor = "multi_thread")]
async fn drain_watchs_note_is_queued_before_the_drained_spend_can_be_read() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new()
        .clock(&clock)
        .service(store.clone())
        .with(BudgetPlugin)
        .with(Runtime(seen.clone()))
        .with(Channels(seen.clone()))
        .with(Later)
        .with(Ear(Arc::default()))
        .start()
        .await;
    k.set_setting("budget.drainWatch", Some(json!(true))).unwrap();
    let answers = Arc::new(Mutex::new(Vec::<usize>::new()));
    let (kk, a2, c2, st2, handle) =
        (Arc::new(Mutex::new(Some(k.clone()))), answers.clone(), clock.clone(), store.clone(), tokio::runtime::Handle::current());
    let _sub = store.on_commit(move |rows| {
        if !a2.lock().unwrap().is_empty() {
            return; // the first drained commit only
        }
        let now = wisp_kernel::Clock::now_ms(&c2);
        let drained = rows.iter().filter(|r| r.entity == "budget_acc").any(|r| {
            let body = st2.get("budget_acc", r.id.clone()).unwrap().and_then(|row| row.get("body").cloned());
            body.and_then(|b| serde_json::from_value::<wisp_budget_core::state::Acc>(b).ok())
                .is_some_and(|acc| wisp_budget_core::drain::drain_of(&acc, now).is_some())
        });
        let Some(k) = kk.lock().unwrap().clone().filter(|_| drained) else { return };
        let n = handle.block_on(k.call(Caller::user(), "ear.heard", json!(null))).unwrap();
        a2.lock().unwrap().push(usize::try_from(n.as_u64().unwrap()).unwrap());
    });
    let (week, resets) = (7 * 24 * H, NOW + 6 * 24 * H);
    let mut used = 0.10;
    record(&k, "seven_day", used, resets, week).await;
    for (i, d) in [0.01, 0.01, 0.01, 0.01, 0.01, 0.05, 0.05].into_iter().enumerate() {
        if i == 5 {
            clock.advance(3 * 3_600_000);
            record(&k, "seven_day", used, resets, week).await;
        }
        clock.advance(60_000);
        used += d;
        record(&k, "seven_day", used, resets, week).await;
        ended(&k, u32::try_from(i).unwrap());
        settle().await;
    }
    eventually("Drain watch tells the phone", || !seen.notes().is_empty()).await;
    assert_eq!(*answers.lock().unwrap(), vec![1], "the reader of the drained spend heard the note");
}

/// ADR-0088: a turn's end saves its spend and the run cost Ask near the limit learned in one commit, not two (each
/// commit pays the change feed's pages again: two per turn end were a tenth of the stream's other commits).
#[tokio::test(flavor = "multi_thread")]
async fn a_turns_end_saves_its_spend_and_the_learned_run_in_one_commit() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let commits: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
    let heard = commits.clone();
    let _sub = store.on_commit(move |rows| {
        let mut kinds: Vec<String> = rows.iter().map(|r| r.entity.clone()).filter(|e| e.starts_with("budget_")).collect();
        kinds.dedup();
        if !kinds.is_empty() {
            heard.lock().unwrap().push(kinds);
        }
    });
    let k = TestApp::new()
        .clock(&clock)
        .service(store.clone())
        .with(BudgetPlugin)
        .with(Runtime(seen.clone()))
        .with(Channels(seen.clone()))
        .start()
        .await;
    record(&k, "five_hour", 0.10, NOW + 3 * H, 5 * H).await;
    commits.lock().unwrap().clear();
    ended(&k, 1);
    eventually("the learned run is saved", || commits.lock().unwrap().iter().any(|c| c.iter().any(|e| e == "budget_meta"))).await;
    settle().await;
    let c = commits.lock().unwrap().clone();
    assert_eq!(c, vec![vec!["budget_acc".to_owned(), "budget_meta".to_owned()]], "one commit: the spend and the runs");
    let runs: String =
        store.read(|r| r.get("budget_meta", "runs")).unwrap().and_then(|row| row.get("body").map(ToString::to_string)).unwrap_or_default();
    assert!(runs.contains("\"t1\""), "the run is kept: {runs}");
}

/// Fake `wisp.accounts` returning a sequence of `accounts.limits` responses for Antigravity.
struct Accounts {
    calls: Arc<Mutex<Vec<(String, String)>>>,
    answers: Arc<Mutex<std::collections::VecDeque<serde_json::Value>>>,
}
impl Plugin for Accounts {
    fn manifest(&self) -> Manifest {
        manifest("wisp.accounts")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let (calls, answers) = (self.calls.clone(), self.answers.clone());
        s.command::<wisp_accounts_api::commands::Limits, _, _>(move |_, _, a| {
            calls.lock().unwrap().push((a.agent, a.account));
            let v = answers.lock().unwrap().pop_front().unwrap_or(serde_json::Value::Null);
            async move { Ok(v) }
        });
        Ok(())
    }
}

/// Antigravity has no in-stream rate-limit event: `turn.ended` refreshes quota asynchronously via `accounts.limits`,
/// populates `budget.view`, and `autoStopAntigravity` holds unattended runs past the stop line.
#[tokio::test(flavor = "multi_thread")]
async fn antigravity_turn_end_refreshes_limits_and_stop_line_holds_unattended_work() {
    let (seen, clock) = (Seen::default(), FakeClock::at(NOW));
    let calls: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let answers = Arc::new(Mutex::new(std::collections::VecDeque::from([json!({
        "windows": [
            { "id": "five_hour", "used": 0.25, "resetsAt": NOW + 4 * H, "ms": 5 * H },
            { "id": "seven_day", "used": 0.80, "resetsAt": NOW + 5 * 24 * H, "ms": 7 * 24 * H }
        ],
        "complete": true
    })])));
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new()
        .clock(&clock)
        .service(store)
        .with(BudgetPlugin)
        .with(Runtime(seen.clone()))
        .with(Channels(seen.clone()))
        .with(Accounts { calls: calls.clone(), answers })
        .start()
        .await;

    let e: TurnEnded = serde_json::from_value(json!({
        "sessionId": "ag1", "turnId": "u1", "status": "done", "interrupted": false,
        "agent": "antigravity", "model": "gemini-4-neon-high", "tokens": 12000
    }))
    .unwrap();
    let ctx = k.ctx("wisp.runtime").unwrap();
    ctx.emit(&ctx.root_call(), &e);

    eventually("antigravity live limits recorded", || !calls.lock().unwrap().is_empty()).await;
    eventually("budget.view returns antigravity windows", || {
        let v = futures_view(&k);
        v.as_array().is_some_and(|a| !a.is_empty())
    })
    .await;
    let view = k.call(Caller::user(), "budget.view", json!({ "agent": "antigravity" })).await.unwrap();
    let ws = view[0]["windows"].as_array().unwrap();
    assert_eq!(ws.len(), 2, "{view}");
    assert_eq!(ws[0]["id"], "five_hour");
    assert_eq!(ws[1]["id"], "seven_day");

    k.call(Caller::user(), "budget.setStop", json!({ "key": "autoStopAntigravity", "value": "75" })).await.unwrap();
    let run: BeforeRunCtx = serde_json::from_value(json!({
        "sessionId": "ag2", "turnId": "u2", "agent": "antigravity", "model": "", "effort": "",
        "mode": "auto", "cwd": "/repo", "prompt": "check", "by": "crew", "kind": "first"
    }))
    .unwrap();
    let held = ctx.run_hook::<BeforeRun>(&ctx.root_call(), run).await.unwrap();
    assert_eq!(held.hold.as_ref().map(|h| h.kind.as_str()), Some("plan"), "{held:?}");
}

fn futures_view(k: &Kernel) -> serde_json::Value {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(k.call(Caller::user(), "budget.view", json!({ "agent": "antigravity" })))
    })
    .unwrap_or(serde_json::Value::Null)
}
