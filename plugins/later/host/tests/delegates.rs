//! v1 #795 against a stand-in wisp.runtime, in the states v2's runtime gives a lead: one whose turn ended while its
//! delegate still works lands in Review (runtime doesn't hold it in Working: `land::Facts.owes` is unset), and the
//! check-in it scheduled goes; one in Working after its turn ended is an after-turn step (Auto's or its goal's check)
//! checking its work, and the check-in waits for it (it would cancel the check mid-run).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup, WireError};
use wisp_later::LaterPlugin;
use wisp_runtime_api::commands as r;
use wisp_runtime_api::model::Session;
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::{FakeClock, TestApp};

/// The lead `lead` (its state) and its delegate `kid`, still at work.
#[derive(Clone)]
struct Rt {
    lead: &'static str,
    sent: Arc<Mutex<Vec<String>>>,
}

fn session(id: &str, state: &str, parent: Option<&str>) -> Session {
    serde_json::from_value(json!({ "id": id, "title": id, "agent": "claude", "model": "", "effort": "", "mode": "auto",
        "cwd": "/repo", "state": state, "parentId": parent, "createdAt": 0, "updatedAt": 0 }))
    .unwrap()
}

struct Runtime(Rt);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "2.0.0", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let lead = self.0.lead;
        s.command::<r::Get, _, _>(move |_, _, a: r::IdArgs| {
            let out = match a.id.0.as_str() {
                "lead" => Ok(session("lead", lead, None)),
                "kid" => Ok(session("kid", "running", Some("lead"))),
                _ => Err(WireError::not_found("No such task.")),
            };
            async move { out }
        });
        let rt = self.0.clone();
        s.command::<r::FollowUp, _, _>(move |_, _, a: r::FollowUpArgs| {
            rt.sent.lock().unwrap().push(a.text);
            async move { Ok(r::FollowedUp { turn_id: None, queued: None }) }
        });
        s.command::<r::SetExt, _, _>(|_, _, _| async move { Ok(r::Ack::default()) });
        Ok(())
    }
}

async fn armed(clock: &FakeClock) {
    while clock.sleepers() == 0 {
        tokio::task::yield_now().await;
    }
}

/// The lead's agent schedules a check-in in a minute; the clock moves past it and later looks. → what was sent.
async fn due_check_in(lead: &'static str) -> Vec<String> {
    let rt = Rt { lead, sent: Arc::default() };
    let clock = FakeClock::at(1_800_000_000_000);
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k: Kernel = TestApp::new().clock(&clock).service(store).with(LaterPlugin).with(Runtime(rt.clone())).start().await;
    let out: Value = k
        .call(Caller::user(), "later.agent.schedule", json!({ "session": "lead", "minutes": 1, "message": "check on the delegate" }))
        .await
        .unwrap();
    assert!(out["text"].as_str().unwrap().starts_with("Scheduled (id "), "{out}");
    armed(&clock).await;
    clock.advance(90_000);
    armed(&clock).await;
    k.call(Caller::user(), "later.tick", json!({})).await.unwrap();
    rt.sent.lock().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lead_whose_turn_ended_with_its_delegate_at_work_gets_the_check_in_it_scheduled_v1_795() {
    let sent = due_check_in("review").await;
    assert_eq!(sent.len(), 1, "the check-in was held behind the delegate: {sent:?}");
    assert!(sent[0].contains("check on the delegate"), "{sent:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lead_in_working_after_its_turn_ended_holds_its_check_in_for_the_after_turn_step_v1_795() {
    assert_eq!(due_check_in("running").await, Vec::<String>::new());
}
