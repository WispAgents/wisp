//! wisp.later against a stand-in wisp.runtime whose answers a test sets, on a fake clock: a due plan whose start
//! fails stays, backs off, and is lost to Drafts after its tries, never deleted unseen (audit doc23b); a follow-up's
//! turn that starts before `runtime.followUp` returns counts as later's, and the user's next turn as theirs (doc23a).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup, WireError};
use wisp_later::LaterPlugin;
use wisp_runtime_api::commands as r;
use wisp_runtime_api::events::TurnStarted;
use wisp_runtime_api::model::Session;
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};

const T0: i64 = 1_800_000_000_000;

#[derive(Clone, Default)]
struct Rt {
    /// `runtime.create` refuses this many more times (usize::MAX: always)
    fails: Arc<AtomicUsize>,
    creates: Arc<AtomicUsize>,
    sent: Arc<Mutex<Vec<String>>>,
    /// `runtime.setExt` calls so far
    exts: Arc<AtomicUsize>,
    /// `runtime.followUp` starts the task's turn (and waits for later to hear it) before it answers
    turn_in_send: Arc<Mutex<bool>>,
}

struct Runtime(Rt);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.command::<r::Get, _, _>(move |_, _, a: r::IdArgs| {
            let out = if a.id.0 == "t1" {
                Ok(serde_json::from_value::<Session>(json!({ "id": "t1", "title": "Ship it", "agent": "claude", "model": "", "effort": "",
                    "mode": "auto", "cwd": "/repo", "state": "review", "createdAt": 0, "updatedAt": 0 }))
                .unwrap())
            } else {
                Err(WireError::not_found("No such task."))
            };
            async move { out }
        });
        let rt = self.0.clone();
        s.command::<r::Create, _, _>(move |_, _, _a: r::StartArgs| {
            rt.creates.fetch_add(1, Ordering::SeqCst);
            let failed =
                rt.fails.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| if n == usize::MAX { Some(n) } else { n.checked_sub(1) });
            async move {
                if failed.is_ok() {
                    return Err(WireError::unavailable("wisp.runtime is restarting"));
                }
                Ok(r::Created { id: "t9".into(), turn_id: None })
            }
        });
        let rt = self.0.clone();
        s.command::<r::FollowUp, _, _>(move |ctx, _, a: r::FollowUpArgs| {
            rt.sent.lock().unwrap().push(a.text);
            let rt = rt.clone();
            async move {
                if *rt.turn_in_send.lock().unwrap() {
                    let before = rt.exts.load(Ordering::SeqCst);
                    ctx.emit(&ctx.root_call(), &TurnStarted { session_id: "t1".into(), turn_id: "u1".into() });
                    until(|| rt.exts.load(Ordering::SeqCst) > before).await;
                }
                Ok(r::FollowedUp { turn_id: None, queued: None, interrupts: false })
            }
        });
        s.command::<r::Note, _, _>(move |_, _, _a: r::NoteArgs| async move { Ok(r::Ack::default()) });
        let rt = self.0.clone();
        s.command::<r::SetExt, _, _>(move |_, _, _a: r::SetExtArgs| {
            rt.exts.fetch_add(1, Ordering::SeqCst);
            async move { Ok(r::Ack::default()) }
        });
        Ok(())
    }
}

/// Waits (real time, bounded) for `f`.
async fn until(f: impl Fn() -> bool) {
    for _ in 0..400 {
        if f() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("never happened");
}

async fn kernel(rt: &Rt, clock: &FakeClock) -> (Kernel, Store) {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new().clock(clock).service(store.clone()).with(LaterPlugin).with(Runtime(rt.clone())).start().await;
    (k, store)
}

async fn armed(clock: &FakeClock) {
    while clock.sleepers() == 0 {
        tokio::task::yield_now().await;
    }
}

/// Moves the clock on and waits for the scheduler's look that woke to end.
async fn advance(clock: &FakeClock, ms: u64) {
    armed(clock).await;
    clock.advance(ms);
    armed(clock).await;
}

async fn row(store: &Store, kind: &'static str, id: &str) -> Option<Value> {
    let id = id.to_owned();
    store.read_async(move |r| r.get(kind, id.as_str())).await.unwrap().and_then(|r| r.get("body").cloned())
}

async fn plans(k: &Kernel) -> Vec<Value> {
    k.call(Caller::user(), "later.plans", json!({})).await.unwrap().as_array().unwrap().clone()
}

async fn plan_in_a_minute(k: &Kernel) -> String {
    let p = k
        .call(Caller::user(), "later.plan", json!({ "kind": "task", "in": 1, "prompt": "fix the flaky test", "folder": "/repo" }))
        .await
        .unwrap();
    p["id"].as_str().unwrap().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_due_plan_whose_start_fails_stays_backs_off_and_runs_on_a_later_look() {
    let (rt, clock) = (Rt::default(), FakeClock::at(T0));
    rt.fails.store(1, Ordering::SeqCst);
    let (k, store) = kernel(&rt, &clock).await;
    let id = plan_in_a_minute(&k).await;
    advance(&clock, 90_000).await;
    assert_eq!(rt.creates.load(Ordering::SeqCst), 1, "due: tried");
    // the start failed: still here and in the store (it was dropped before it ran), with its try
    assert_eq!(plans(&k).await.len(), 1, "a failed start lost the plan");
    let kept = row(&store, "later_plan", &id).await.expect("still in the store");
    assert_eq!((kept["tries"].as_u64(), kept["retryAt"].as_i64()), (Some(1), Some(T0 + 90_000 + 60_000)), "{kept}");
    // not again before its backoff, however often later looks
    k.call(Caller::user(), "later.tick", json!({})).await.unwrap();
    advance(&clock, 20_000).await;
    assert_eq!(rt.creates.load(Ordering::SeqCst), 1, "tried again inside its backoff");
    // past it: runs, then goes
    advance(&clock, 60_000).await;
    assert_eq!(rt.creates.load(Ordering::SeqCst), 2);
    assert!(plans(&k).await.is_empty());
    assert_eq!(row(&store, "later_plan", &id).await, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plan_that_never_starts_is_lost_to_drafts_not_silently_deleted() {
    let (rt, clock) = (Rt::default(), FakeClock::at(T0));
    rt.fails.store(usize::MAX, Ordering::SeqCst);
    let (k, store) = kernel(&rt, &clock).await;
    let id = plan_in_a_minute(&k).await;
    advance(&clock, 90_000).await; // 1st try; again after a minute
    advance(&clock, 70_000).await; // 2nd; again after five
    advance(&clock, 310_000).await; // 3rd: lost
    assert_eq!(rt.creates.load(Ordering::SeqCst), 3);
    let all = plans(&k).await;
    assert_eq!(all.len(), 1, "{all:?}");
    let p = &all[0];
    assert_eq!((p["draft"].as_bool(), p["lost"]["why"].as_str(), p.get("at")), (Some(true), Some("failed"), None), "{p}");
    assert_eq!(p["lost"]["title"], "fix the flaky test");
    assert_eq!(row(&store, "later_plan", &id).await.unwrap()["lost"]["why"], "failed");
    advance(&clock, 3_600_000).await;
    assert_eq!(rt.creates.load(Ordering::SeqCst), 3, "a lost plan waits for you, it isn't tried again");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_follow_ups_turn_starting_inside_the_send_counts_as_laters_and_the_users_next_turn_resets_the_chain() {
    let (rt, clock) = (Rt::default(), FakeClock::at(T0));
    let (k, store) = kernel(&rt, &clock).await;
    // a reminder due first (the turn clears it), then the agent's own wake-up
    k.call(Caller::user(), "later.add", json!({ "sessionId": "t1", "text": "call the client", "in": 1, "remind": true })).await.unwrap();
    k.call(Caller::user(), "later.agent.schedule", json!({ "session": "t1", "minutes": 2, "message": "see if CI passed" })).await.unwrap();
    advance(&clock, 90_000).await;
    assert!(row(&store, "later_meta", "t1").await.unwrap()["reminder"].is_object(), "the reminder came due");
    *rt.turn_in_send.lock().unwrap() = true;
    advance(&clock, 60_000).await;
    assert_eq!(rt.sent.lock().unwrap().len(), 1);
    let m = row(&store, "later_meta", "t1").await.unwrap();
    assert_eq!(m["chain"], 1, "the follow-up's turn is later's: {m}");
    assert!(m["sent"].is_null(), "the turn took the send; the write after followUp put it back: {m}");
    assert!(m["reminder"].is_null(), "the turn cleared the reminder and it stays cleared: {m}");
    // the user's own turn next: no send waits, the chain starts again
    let ctx = k.ctx("wisp.runtime").unwrap();
    ctx.emit(&ctx.root_call(), &TurnStarted { session_id: "t1".into(), turn_id: "u2".into() });
    for _ in 0..400 {
        if row(&store, "later_meta", "t1").await.unwrap()["chain"] == 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("the user's turn was counted as later's: {}", row(&store, "later_meta", "t1").await.unwrap());
}
