//! A tap answers the question it was made for (review r194 row 66 #1, #8): on a kernel with a stand-in wisp.runtime,
//! the hub sends `runtime.answer` the option the button stands for and the id of the question it answers (so an answer
//! to a question that's no longer open is refused there), and a setup card (ADR-0113) is answered only by a tap the
//! transport checked came from the linked person.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_channels::ChannelsPlugin;
use wisp_channels_api::views::{ActArgs, ActKind, Msg, SessionView};
use wisp_channels_api::{ChannelHub, TRANSPORTS, Transport, TransportDef, TransportKind};
use wisp_kernel::{BoxFuture, Caller, CancellationToken, Kernel, Manifest, Plugin, Result, Setup};
use wisp_runtime_api::commands as r;
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

type HubSlot = Arc<Mutex<Option<Arc<dyn ChannelHub>>>>;
type Answers = Arc<Mutex<Vec<Value>>>;

struct Fake;
#[async_trait]
impl Transport for Fake {
    fn name(&self) -> &'static str {
        "fake"
    }
    fn ready(&self) -> bool {
        true
    }
    fn kind(&self) -> TransportKind {
        TransportKind::Thread
    }
    async fn start(&self, _stop: CancellationToken) {}
    async fn publish(&self, _msg: &Msg, _s: Option<&SessionView>) -> Result<()> {
        Ok(())
    }
}
struct Def(HubSlot);
impl TransportDef for Def {
    fn name(&self) -> &'static str {
        "fake"
    }
    fn create(&self, hub: Arc<dyn ChannelHub>) -> Arc<dyn Transport> {
        *self.0.lock().unwrap() = Some(hub);
        Arc::new(Fake)
    }
}

/// wisp.runtime with `q1` (a plain question) open on `s1` and a setup card open on `s2`; `runtime.answer` is recorded.
struct Runtime(HubSlot, Answers);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "2.0.0", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.command::<r::Get, _, _>(move |_, _, a: r::IdArgs| {
            let ask = if a.id.0 == "s2" {
                json!({ "id": "c2", "question": "Ada wants to add the connector `x`", "options": ["Allow", "Deny"], "setup": { "headline": "…" } })
            } else {
                json!({ "id": "q1", "question": "Which?", "options": ["Yes", "No"] })
            };
            let t = json!({ "id": a.id.0, "title": "t", "agent": "claude", "model": "", "effort": "", "mode": "edits", "state": "waiting",
                "createdAt": 0, "updatedAt": 0, "cwd": "/w", "ext": { "wisp.runtime": { "ask": ask } } });
            async move { Ok(serde_json::from_value(t).unwrap()) }
        });
        let answers = self.1.clone();
        s.command::<r::Answer, _, _>(move |_, _, a: r::AnswerArgs| {
            answers.lock().unwrap().push(serde_json::to_value(&a).unwrap());
            async move { Ok(json!({ "answered": true })) }
        });
        s.contribute(&TRANSPORTS, Arc::new(Def(self.0.clone())) as Arc<dyn TransportDef>);
        Ok(())
    }
}

#[derive(Default)]
struct Secrets(Mutex<std::collections::BTreeMap<String, String>>);
impl wisp_kernel::services::Secrets for Secrets {
    fn get(&self, key: &str) -> BoxFuture<'static, Result<Option<String>>> {
        let v = self.0.lock().unwrap().get(key).cloned();
        Box::pin(async move { Ok(v) })
    }
    fn set(&self, key: &str, value: &str) -> BoxFuture<'static, Result<()>> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Box::pin(async { Ok(()) })
    }
    fn delete(&self, key: &str) -> BoxFuture<'static, Result<()>> {
        self.0.lock().unwrap().remove(key);
        Box::pin(async { Ok(()) })
    }
    fn keys(&self, _prefix: &str) -> BoxFuture<'static, Result<Vec<String>>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

async fn hub() -> (Kernel, Arc<dyn ChannelHub>, Answers) {
    let (slot, answers) = (HubSlot::default(), Answers::default());
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let secrets: wisp_kernel::services::SecretsService = Arc::new(Secrets::default());
    let k = TestApp::new()
        .service(store)
        .service(secrets)
        .with(wisp_resources::ResourcesPlugin)
        .with(ChannelsPlugin)
        .with(Runtime(slot.clone(), answers.clone()))
        .start()
        .await;
    k.call(
        Caller::user(),
        "resources.apply",
        json!({ "resources": [{ "kind": "channel", "name": "fake", "spec": { "transport": "fake", "enabled": true } }] }),
    )
    .await
    .unwrap();
    for _ in 0..200 {
        if slot.lock().unwrap().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let h = slot.lock().unwrap().clone().expect("the channel started");
    (k, h, answers)
}

fn tap(session: &str, reference: &str, index: u32, person: bool) -> ActArgs {
    ActArgs {
        session_id: session.into(),
        kind: ActKind::Answer,
        reference: Some(reference.into()),
        index: Some(index),
        text: None,
        approvals: false,
        person,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tap_answers_with_its_option_and_names_the_question_it_answers() {
    let (_k, h, answers) = hub().await;
    h.act(&tap("s1", "q1", 1, false)).await.unwrap();
    assert_eq!(answers.lock().unwrap().clone(), vec![json!({ "id": "s1", "text": "No", "askId": "q1" })]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_setup_card_is_answered_from_a_channel_only_by_the_linked_person() {
    let (_k, h, answers) = hub().await;
    let said = h.act(&tap("s2", "c2", 0, false)).await.unwrap();
    assert!(said.contains("answer it in wisp"), "{said}");
    assert_eq!(answers.lock().unwrap().len(), 0, "nothing answered for someone the channel can't vouch for");
    h.act(&tap("s2", "c2", 0, true)).await.unwrap();
    assert_eq!(answers.lock().unwrap().clone(), vec![json!({ "id": "s2", "text": "Allow", "askId": "c2" })]);
}
