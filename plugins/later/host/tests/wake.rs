//! wisp.later's scheduler against a stand-in wisp.runtime, on a fake clock: a wake-up the agent sets during its own
//! running turn is held while the turn runs and fires once it ends (v1's bug: one scheduled while a turn ran never
//! fired); a look that can't read a task keeps what it has scheduled; a reminder's note reaches the task.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup, WireError};
use wisp_later::LaterPlugin;
use wisp_runtime_api::commands as r;
use wisp_runtime_api::model::{Session, SessionState};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};

#[derive(Clone, Default)]
struct Rt {
    state: Arc<Mutex<Option<SessionState>>>,
    /// runtime.get refuses (it's starting) while set
    down: Arc<Mutex<bool>>,
    sent: Arc<Mutex<Vec<String>>>,
    notes: Arc<Mutex<Vec<String>>>,
    ext: Arc<Mutex<serde_json::Map<String, Value>>>,
}

impl Rt {
    fn set(&self, s: SessionState) {
        *self.state.lock().unwrap() = Some(s);
    }
    fn later_ext(&self) -> Value {
        self.ext.lock().unwrap().get("wisp.later").cloned().unwrap_or(Value::Null)
    }
    fn session(&self) -> Session {
        let state = self.state.lock().unwrap().unwrap_or_default();
        let ext = self.ext.lock().unwrap().clone();
        serde_json::from_value(json!({ "id": "t1", "title": "Ship it", "agent": "claude", "model": "", "effort": "", "mode": "auto",
            "cwd": "/repo", "state": state, "createdAt": 0, "updatedAt": 0, "ext": ext }))
        .unwrap()
    }
}

/// wisp.runtime as far as later calls it: one task, `t1`.
struct Runtime(Rt);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let rt = self.0.clone();
        s.command::<r::Get, _, _>(move |_, _, a: r::IdArgs| {
            let out = if *rt.down.lock().unwrap() {
                Err(WireError::unavailable("wisp.runtime is starting"))
            } else if a.id.0 == "t1" {
                Ok(rt.session())
            } else {
                Err(WireError::not_found("No such task."))
            };
            async move { out }
        });
        let rt = self.0.clone();
        s.command::<r::FollowUp, _, _>(move |_, _, a: r::FollowUpArgs| {
            assert_eq!(a.by.as_deref(), Some("later"));
            rt.sent.lock().unwrap().push(a.text);
            async move { Ok(r::FollowedUp { turn_id: None, queued: None, interrupts: false }) }
        });
        let rt = self.0.clone();
        s.command::<r::Note, _, _>(move |_, _, a: r::NoteArgs| {
            rt.notes.lock().unwrap().push(a.label);
            async move { Ok(r::Ack::default()) }
        });
        let rt = self.0.clone();
        s.command::<r::SetExt, _, _>(move |_, call, a: r::SetExtArgs| {
            let caller = call.caller();
            let ok = matches!(caller.set.as_str(), "user:full" | "kernel") || caller.plugin.as_deref() == Some(a.plugin.as_str());
            let out = if ok {
                let mut ext = rt.ext.lock().unwrap();
                let cur = ext.get(&a.plugin).unwrap_or(&Value::Null);
                if a.expect.as_ref().is_some_and(|e| &e.value != cur) {
                    Err(WireError::conflict("changed meanwhile"))
                } else {
                    if a.value.is_null() {
                        ext.remove(&a.plugin);
                    } else {
                        ext.insert(a.plugin, a.value);
                    }
                    Ok(r::Ack::default())
                }
            } else {
                Err(WireError::forbidden(format!("{} may not write {}'s slot", caller.set, a.plugin)))
            };
            async move { out }
        });
        Ok(())
    }
}

async fn kernel(rt: &Rt, clock: &FakeClock) -> Kernel {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    TestApp::new().clock(clock).service(store).with(LaterPlugin).with(Runtime(rt.clone())).start().await
}

/// The scheduler's own timer armed: its look (at start, or the one an advance woke) is over.
async fn armed(clock: &FakeClock) {
    while clock.sleepers() == 0 {
        tokio::task::yield_now().await;
    }
}

/// Moves the clock past the scheduler's next look and waits for that look to end, so the test's own look (and
/// what it asserts) never runs beside it.
async fn advance(clock: &FakeClock, ms: u64) {
    armed(clock).await;
    clock.advance(ms);
    armed(clock).await;
}

async fn tick(k: &Kernel) -> Value {
    k.call(Caller::user(), "later.tick", json!({})).await.unwrap()
}

async fn listed(k: &Kernel) -> usize {
    k.call(Caller::user(), "later.list", json!({ "sessionId": "t1" })).await.unwrap().as_array().unwrap().len()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wake_set_during_a_running_turn_fires_once_the_turn_ends_v1_never_fired() {
    let (rt, clock) = (Rt::default(), FakeClock::at(1_800_000_000_000));
    rt.set(SessionState::Running);
    let k = kernel(&rt, &clock).await;
    // the agent's own wake-up, set from inside its running turn
    let out = k
        .call(Caller::user(), "later.agent.schedule", json!({ "session": "t1", "minutes": 1, "message": "see if CI passed" }))
        .await
        .unwrap();
    assert!(out["text"].as_str().unwrap().starts_with("Scheduled (id "), "{out}");
    advance(&clock, 90_000).await;
    // due, but the turn still runs: held, not dropped
    tick(&k).await;
    assert!(rt.sent.lock().unwrap().is_empty(), "nothing goes into a running turn");
    assert_eq!(listed(&k).await, 1, "still scheduled while the turn runs");
    // the turn ends: the next look sends it
    rt.set(SessionState::Review);
    advance(&clock, 20_000).await;
    tick(&k).await;
    let sent = rt.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(sent[0].contains("see if CI passed"), "{sent:?}");
    assert_eq!(listed(&k).await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_look_that_cannot_read_a_task_keeps_what_it_has_scheduled() {
    let (rt, clock) = (Rt::default(), FakeClock::at(1_800_000_000_000));
    rt.set(SessionState::Review);
    let k = kernel(&rt, &clock).await;
    k.call(Caller::user(), "later.add", json!({ "sessionId": "t1", "text": "check the deploy", "in": 1 })).await.unwrap();
    // runtime refuses the read (starting, a failed read): the task isn't gone, its follow-up stays
    *rt.down.lock().unwrap() = true;
    advance(&clock, 90_000).await;
    tick(&k).await;
    assert_eq!(listed(&k).await, 1, "an unreadable task's follow-up was forgotten as if the task were gone");
    *rt.down.lock().unwrap() = false;
    tick(&k).await;
    assert_eq!(rt.sent.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reminder_coming_due_notes_its_text_on_the_task() {
    let (rt, clock) = (Rt::default(), FakeClock::at(1_800_000_000_000));
    rt.set(SessionState::Review);
    let k = kernel(&rt, &clock).await;
    k.call(Caller::user(), "later.add", json!({ "sessionId": "t1", "text": "call the client", "in": 1, "remind": true })).await.unwrap();
    advance(&clock, 90_000).await;
    tick(&k).await;
    assert_eq!(rt.notes.lock().unwrap().clone(), ["call the client"], "runtime.note got v1's {{text}} and refused it");
    assert!(rt.sent.lock().unwrap().is_empty(), "a reminder isn't sent to the agent");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agents_resume_later_mirrors_to_ext_and_clears_on_cancel_and_send_now() {
    let (rt, clock) = (Rt::default(), FakeClock::at(1_800_000_000_000));
    rt.set(SessionState::Running);
    let k = kernel(&rt, &clock).await;
    let agent = Caller::agent("t1", false);
    // scheduling as an agent (`agent:session`) must mirror into `ext["wisp.later"]` so the sidebar moves it to Scheduled
    let out = k
        .call(agent.clone(), "later.agent.schedule", json!({ "session": "t1", "minutes": 15, "message": "check the release" }))
        .await
        .unwrap();
    assert!(out["text"].as_str().unwrap().starts_with("Scheduled (id "), "{out}");
    let mirrored = rt.later_ext();
    let entries = mirrored["later"].as_array().expect("agent resume_later must mirror into ext[\"wisp.later\"]");
    assert_eq!(entries.len(), 1, "{mirrored}");
    assert_eq!(entries[0]["text"], "check the release");
    assert_eq!(entries[0]["by"], "agent");
    let id = entries[0]["id"].as_str().unwrap().to_owned();
    // cancelling as an agent clears the mirror
    let cancelled = k.call(agent.clone(), "later.agent.schedule", json!({ "session": "t1", "cancel": id })).await.unwrap();
    assert!(cancelled["text"].as_str().unwrap().starts_with("Cancelled "), "{cancelled}");
    assert!(rt.later_ext().is_object() && rt.later_ext().get("later").is_none(), "{}", rt.later_ext());
    // scheduling again and sending now (`later.edit` with `now: true`) clears the mirror immediately
    k.call(agent, "later.agent.schedule", json!({ "session": "t1", "minutes": 15, "message": "deploy the grid" })).await.unwrap();
    let id = rt.later_ext()["later"][0]["id"].as_str().unwrap().to_owned();
    rt.set(SessionState::Review);
    k.call(Caller::user(), "later.edit", json!({ "sessionId": "t1", "item": id, "now": true })).await.unwrap();
    assert_eq!(rt.sent.lock().unwrap().len(), 1);
    assert!(
        rt.later_ext().is_object() && rt.later_ext().get("later").is_none(),
        "Send now must clear the task's mirror: {}",
        rt.later_ext()
    );
}
