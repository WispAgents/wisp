//! A tap answers the question it was made for (review r194 row 66 #1, #8): on a kernel with a stand-in wisp.runtime,
//! the hub sends `runtime.answer` the option the button stands for and the id of the question it answers (so an answer
//! to a question that's no longer open is refused there), and a setup card (ADR-0113) is answered only by a tap the
//! transport checked came from the linked person. A reply button sends the option it shows as the follow-up, while the
//! turn it was made under is still the newest (doc26b).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_channels::ChannelsPlugin;
use wisp_channels_api::views::{ActArgs, ActKind, Acted, Msg, SessionView};
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

/// wisp.runtime with `q1` (a plain question) open on `s1`, a setup card open on `s2` and a guard question (ADR-0115) on `s3`; `runtime.answer` is recorded.
/// `s4` asks nothing: its newest turn `u2` ended with a question block (`Ship it`, `Revise`); `s5`'s newest, `u5`, with
/// an option that is an object (`{label, reply}`). `runtime.followUp` is recorded.
struct Runtime(HubSlot, Answers, Answers);

/// The newest turn of `s4` or `s5`, as `runtime.turns` answers it.
fn newest_turn(session: &str) -> Vec<Value> {
    let (id, options) = if session == "s5" {
        ("u5", json!([{ "label": "Ship", "reply": "Ship it and open a PR" }, "Wait"]))
    } else {
        ("u2", json!(["Ship it", "Revise"]))
    };
    let mut t = serde_json::to_value(wisp_runtime_api::model::Turn::default()).unwrap();
    t["id"] = json!(id);
    t["sessionId"] = json!(session);
    t["status"] = json!("done");
    t["presentation"] = json!({ "v": 1, "blocks": [{ "type": "hero", "status": "done", "title": "Ready" }, { "type": "question", "question": "Ship?", "options": options }] });
    vec![t]
}
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.command::<r::Get, _, _>(move |_, _, a: r::IdArgs| {
            let ask = if a.id.0 == "s2" {
                json!({ "id": "c2", "question": "Ada wants to add the connector “x”", "options": ["Allow", "Deny"], "setup": { "headline": "…" } })
            } else if a.id.0 == "s3" {
                json!({ "id": "g3", "question": "Your harness wants to turn off …", "options": ["Allow", "Deny"], "guard": true })
            } else if a.id.0 == "s4" || a.id.0 == "s5" {
                Value::Null
            } else {
                json!({ "id": "q1", "question": "Which?", "options": ["Yes", "No"] })
            };
            let slot = if ask.is_null() { json!({}) } else { json!({ "ask": ask }) };
            let t = json!({ "id": a.id.0, "title": "t", "agent": "claude", "model": "", "effort": "", "mode": "edits", "state": "waiting",
                "createdAt": 0, "updatedAt": 0, "cwd": "/w", "ext": { "wisp.runtime": slot } });
            async move { Ok(serde_json::from_value(t).unwrap()) }
        });
        let answers = self.1.clone();
        s.command::<r::Answer, _, _>(move |_, _, a: r::AnswerArgs| {
            answers.lock().unwrap().push(serde_json::to_value(&a).unwrap());
            async move { Ok(json!({ "answered": true })) }
        });
        s.command::<r::Turns, _, _>(move |_, _, a: r::TurnsArgs| {
            let turns = newest_turn(a.id.as_str());
            async move { Ok(turns.into_iter().map(|t| serde_json::from_value(t).unwrap()).collect()) }
        });
        let follow_ups = self.2.clone();
        s.command::<r::FollowUp, _, _>(move |_, _, a: r::FollowUpArgs| {
            follow_ups.lock().unwrap().push(json!({ "id": a.id.as_str(), "text": a.text }));
            async move { Ok(serde_json::from_value(json!({})).unwrap()) }
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

async fn hub() -> (Kernel, Arc<dyn ChannelHub>, Answers, Answers) {
    let (slot, answers, follow_ups) = (HubSlot::default(), Answers::default(), Answers::default());
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let secrets: wisp_kernel::services::SecretsService = Arc::new(Secrets::default());
    let k = TestApp::new()
        .service(store)
        .service(secrets)
        .with(wisp_resources::ResourcesPlugin)
        .with(ChannelsPlugin)
        .with(Runtime(slot.clone(), answers.clone(), follow_ups.clone()))
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
    (k, h, answers, follow_ups)
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
    let (_k, h, answers, _) = hub().await;
    h.act(&tap("s1", "q1", 1, false)).await.unwrap();
    assert_eq!(answers.lock().unwrap().clone(), vec![json!({ "id": "s1", "text": "No", "askId": "q1" })]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_setup_card_is_answered_from_a_channel_only_by_the_linked_person() {
    let (_k, h, answers, _) = hub().await;
    let said = h.act(&tap("s2", "c2", 0, false)).await.unwrap();
    assert!(!said.done && said.words.contains("answer it in wisp"), "{said:?}");
    assert_eq!(answers.lock().unwrap().len(), 0, "nothing answered for someone the channel can't vouch for");
    h.act(&tap("s2", "c2", 0, true)).await.unwrap();
    assert_eq!(answers.lock().unwrap().clone(), vec![json!({ "id": "s2", "text": "Allow", "askId": "c2" })]);
}

/// Audit G4: "Allow" typed in a chat was sent as a follow-up ("Sent."): it queued behind the card and later ran as the
/// agent's prompt. A typed reply from the linked person answers the open card, naming it (words are the Deny with
/// them, ADR-0113 rule 4); from anyone the channel can't vouch for, it says to answer in wisp and sends nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_typed_reply_under_a_setup_card_answers_it_only_for_the_linked_person_audit_g4() {
    let (_k, h, answers, _) = hub().await;
    let typed = |text: &str, person: bool| ActArgs {
        session_id: "s2".into(),
        kind: ActKind::Text,
        reference: None,
        index: None,
        text: Some(text.into()),
        approvals: false,
        person,
    };
    let said = h.act(&typed("Allow", false)).await.unwrap();
    assert!(!said.done && said.words.contains("answer it in wisp"), "{said:?}");
    assert!(answers.lock().unwrap().is_empty());
    h.act(&typed("Allow", true)).await.unwrap();
    h.act(&typed("not that one", true)).await.unwrap();
    assert_eq!(
        answers.lock().unwrap().clone(),
        vec![json!({ "id": "s2", "text": "Allow", "askId": "c2" }), json!({ "id": "s2", "text": "not that one", "askId": "c2" })]
    );
}

/// Audit I IS1: a guard question (ADR-0115 §4, the user's harness lowering a safety guard) took a replayed ntfy action and
/// another Telegram member's tap. It's answered as a setup card is: a tap or a typed reply the channel can't vouch is the
/// linked person's answers nothing and says to answer in wisp; the linked person's does.
#[tokio::test(flavor = "multi_thread")]
async fn a_guard_question_is_answered_from_a_channel_only_by_the_linked_person_audit_i_is1() {
    let (_k, h, answers, _) = hub().await;
    // a replayed ntfy action (ntfy never vouches) and another member's tap in a linked group chat: both `person: false`
    let said = h.act(&tap("s3", "g3", 0, false)).await.unwrap();
    assert_eq!(said, Acted::refused(wisp_channels_core::SETUP_ELSEWHERE));
    let typed = |text: &str, person: bool| ActArgs {
        session_id: "s3".into(),
        kind: ActKind::Reply,
        reference: None,
        index: None,
        text: Some(text.into()),
        approvals: false,
        person,
    };
    assert_eq!(h.act(&typed("Allow", false)).await.unwrap(), Acted::refused(wisp_channels_core::SETUP_ELSEWHERE));
    assert!(answers.lock().unwrap().is_empty(), "nothing answered: {:?}", answers.lock().unwrap());
    h.act(&tap("s3", "g3", 1, true)).await.unwrap();
    h.act(&typed("Allow", true)).await.unwrap();
    assert_eq!(
        answers.lock().unwrap().clone(),
        vec![json!({ "id": "s3", "text": "Deny", "askId": "g3" }), json!({ "id": "s3", "text": "Allow", "askId": "g3" })]
    );
}

fn reply_tap(session: &str, turn: &str, index: u32) -> ActArgs {
    ActArgs { kind: ActKind::Reply, ..tap(session, turn, index, true) }
}

/// doc26b: a reply button's tap was sent as an empty follow-up (its index never read), which the runtime refuses ("a
/// follow-up needs text"). It sends the option it shows, of the turn it was made under.
#[tokio::test(flavor = "multi_thread")]
async fn a_reply_button_sends_its_option_as_the_follow_up_doc26b() {
    let (_k, h, answers, follow_ups) = hub().await;
    assert_eq!(h.act(&reply_tap("s4", "u2", 1)).await.unwrap(), Acted::done("Sent."));
    assert_eq!(follow_ups.lock().unwrap().clone(), vec![json!({ "id": "s4", "text": "Revise" })]);
    assert!(answers.lock().unwrap().is_empty());
}

/// A button under a turn that isn't the newest any more (or an option that isn't there) sends nothing and says so.
#[tokio::test(flavor = "multi_thread")]
async fn a_reply_button_under_an_older_turn_or_past_its_options_sends_nothing_doc26b() {
    let (_k, h, _, follow_ups) = hub().await;
    assert_eq!(h.act(&reply_tap("s4", "u1", 0)).await.unwrap(), Acted::refused(wisp_channels_api::OUT_OF_DATE));
    assert_eq!(h.act(&reply_tap("s4", "u2", 2)).await.unwrap(), Acted::refused(wisp_channels_api::OUT_OF_DATE));
    assert!(follow_ups.lock().unwrap().is_empty(), "{:?}", follow_ups.lock().unwrap());
}

/// An option written as `{label, reply}` shows its label and sends its reply (the full message the agent asked for).
#[tokio::test(flavor = "multi_thread")]
async fn a_reply_buttons_object_option_sends_its_reply_doc26b() {
    let (_k, h, _, follow_ups) = hub().await;
    h.act(&reply_tap("s5", "u5", 0)).await.unwrap();
    h.act(&reply_tap("s5", "u5", 1)).await.unwrap();
    assert_eq!(
        follow_ups.lock().unwrap().clone(),
        vec![json!({ "id": "s5", "text": "Ship it and open a PR" }), json!({ "id": "s5", "text": "Wait" })]
    );
}
