//! wisp.push end to end in one process (ADR-0065 §3 and its edge cases): a phone pairs with its own key, the
//! `push` channel switches on, a task's notification reaches the relay as `{handle, sealed, collapse}` only and
//! opens with that phone's key only, a choice from it lands once, as the app's own button would, and a token the
//! relay says is gone unpairs its phone and switches the channel off with the reason. The relay is a fake `Net`; the
//! runtime a stand-in holding one task.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_kernel::services::{NetRequest, NetResponse};
use wisp_kernel::{BoxFuture, Caller, Code, Kernel, Manifest, Plugin, Result, Setup};
use wisp_push_api::Note;
use wisp_push_core::seal::{Keys, open};
use wisp_runtime_api::commands::{Ack, Answer, AnswerArgs, FollowUp, FollowUpArgs, FollowedUp, SetState, SetStateArgs};
use wisp_runtime_api::model::Session;
use wisp_runtime_api::testing::SlotRuntime;
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

#[derive(Default)]
struct Secrets(Mutex<BTreeMap<String, String>>);
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

/// The grid relay: every send recorded; a handle in `gone` answers 410 as the relay does once FCM/APNs say so.
#[derive(Default)]
struct Relay {
    sends: Mutex<Vec<(String, Value)>>,
    gone: Mutex<Vec<String>>,
}
impl wisp_kernel::services::Net for Relay {
    fn fetch(&self, req: NetRequest) -> BoxFuture<'static, Result<NetResponse>> {
        let body: Value = serde_json::from_slice(&req.body).unwrap_or_default();
        let gone = body["handle"].as_str().is_some_and(|h| self.gone.lock().unwrap().iter().any(|g| g == h));
        self.sends.lock().unwrap().push((req.url.clone(), body));
        let (status, answer) = if gone { (410, json!({ "error": "gone" })) } else { (200, json!({ "ok": true })) };
        Box::pin(async move { Ok(NetResponse { status, headers: Vec::new(), body: answer.to_string().into_bytes() }) })
    }
}

/// wisp.runtime holding task `t1`: `runtime.answer` closes its ask; every command that came in is recorded.
#[derive(Clone)]
struct Runtime {
    slot: SlotRuntime,
    ran: Arc<Mutex<Vec<(String, String)>>>,
}
impl Runtime {
    fn new() -> Self {
        let t = Session { id: "t1".into(), title: "Fix the build".into(), cwd: "/w/api".into(), ..Session::default() };
        Self { slot: SlotRuntime::new(t), ran: Arc::default() }
    }
    fn ask(&self, id: &str, options: &[&str]) {
        self.slot.set("wisp.runtime", json!({ "ask": { "id": id, "question": "Which branch?", "options": options } }));
    }
    fn ran(&self) -> Vec<(String, String)> {
        self.ran.lock().unwrap().clone()
    }
}
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(
            json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1", "activation": ["onStartup"] }),
        )
        .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        self.slot.register(s);
        let me = self.clone();
        s.command::<Answer, _, _>(move |_c, _call, a: AnswerArgs| {
            // with the question it answers (review r194 row 66 #1): `text@askId`
            me.ran.lock().unwrap().push(("answer".into(), format!("{}@{}", a.text, a.ask_id.unwrap_or_default())));
            me.slot.set("wisp.runtime", json!({}));
            async { Ok(json!({ "ok": true })) }
        });
        let me = self.clone();
        s.command::<FollowUp, _, _>(move |_c, _call, a: FollowUpArgs| {
            me.ran.lock().unwrap().push(("followUp".into(), a.text));
            async { Ok(serde_json::from_value::<FollowedUp>(json!({})).unwrap()) }
        });
        let me = self.clone();
        s.command::<SetState, _, _>(move |_c, _call, a: SetStateArgs| {
            me.ran
                .lock()
                .unwrap()
                .push(("setState".into(), serde_json::to_value(a.state).unwrap().as_str().unwrap_or_default().to_owned()));
            async { Ok(Ack {}) }
        });
        Ok(())
    }
}

struct App {
    k: Kernel,
    secrets: Arc<Secrets>,
    relay: Arc<Relay>,
    rt: Runtime,
    dir: PathBuf,
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("wisp-push-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::open(Location::Path(dir.join("wisp.db")), Arc::new(SystemClock)).unwrap();
    let (secrets, relay, rt) = (Arc::new(Secrets::default()), Arc::new(Relay::default()), Runtime::new());
    let (sec, net): (wisp_kernel::services::SecretsService, wisp_kernel::services::NetService) = (secrets.clone(), relay.clone());
    let k = TestApp::new()
        .service(store)
        .service(sec)
        .service(net)
        .with(rt.clone())
        .with(wisp_resources::ResourcesPlugin)
        .with(wisp_channels::ChannelsPlugin)
        .with(wisp_push::PushPlugin)
        .start()
        .await;
    App { k, secrets, relay, rt, dir }
}

async fn eventually(what: &str, f: impl AsyncFn() -> bool) {
    for _ in 0..400 {
        if f().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("never: {what}");
}

fn handle(n: u8) -> String {
    format!("wph_{}", char::from(b'a' + n).to_string().repeat(43))
}

impl App {
    async fn call(&self, id: &str, args: Value) -> Value {
        self.k.call(Caller::user(), id, args).await.unwrap_or_else(|e| panic!("{id}: {}", e.message))
    }
    async fn pair(&self, n: u8, keys: &Keys, device: &str) -> Value {
        let a = json!({ "handle": handle(n), "publicKey": keys.public, "platform": "android", "device": device, "relay": "http://wisp.localhost:8080" });
        self.call("push.pair", a).await
    }
    async fn push_on(&self) -> bool {
        let v = self.call("channels.view", json!({})).await;
        v["channels"].as_array().unwrap().iter().any(|c| c["transport"] == "push" && c["running"] == true)
    }
    async fn channel_spec(&self) -> Value {
        let v = self.call("resources.export", json!({ "kinds": ["channel"] })).await;
        v.as_array().unwrap().iter().find(|r| r["name"] == "push").map(|r| r["spec"].clone()).unwrap_or_default()
    }
    fn sends(&self) -> Vec<(String, Value)> {
        self.relay.sends.lock().unwrap().clone()
    }
    /// A notification about task `t1`, as channels sends a finished or asking task.
    async fn notify(&self) {
        self.call("channels.notify", json!({ "sessionId": "t1" })).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn pairing_keeps_the_phone_in_secrets_and_switches_the_push_channel_on() {
    let a = app("pair").await;
    let phone = Keys::generate(&mut rand::rng());
    let d = a.pair(0, &phone, "Pixel 9").await;
    assert_eq!((d["device"].clone(), d["platform"].clone()), (json!("Pixel 9"), json!("android")), "{d}");
    let kept = a.secrets.0.lock().unwrap().get("push/devices").cloned().unwrap();
    assert!(kept.contains(&handle(0)) && kept.contains(&phone.public), "the handle and key are in secrets.json: {kept}");
    eventually("the push channel runs", async || a.push_on().await).await;
    assert_eq!(a.channel_spec().await["enabled"], true);
    let notes = a.call("resources.export", json!({ "kinds": ["notification"] })).await.to_string();
    assert!(notes.contains("phone-push") && notes.contains("asks") && notes.contains("finished"), "{notes}");

    // push.devices says which phones, never their handle or key
    let list = a.call("push.devices", json!({})).await;
    assert_eq!(list["devices"].as_array().unwrap().len(), 1);
    assert!(!list.to_string().contains(&handle(0)) && !list.to_string().contains(&phone.public), "{list}");
    // pairing the same handle again replaces it
    let again = a.pair(0, &phone, "Work phone").await;
    assert_eq!(again["id"], d["id"]);
    assert_eq!(a.call("push.devices", json!({})).await["devices"].as_array().unwrap().len(), 1);
    // nothing about the phone reached the store
    let disk: Vec<u8> = std::fs::read_dir(&a.dir).unwrap().flat_map(|e| std::fs::read(e.unwrap().path()).unwrap_or_default()).collect();
    assert!(!disk.windows(handle(0).len()).any(|w| w == handle(0).as_bytes()), "the handle isn't in the store");
}

#[tokio::test(flavor = "multi_thread")]
async fn only_the_phone_pairs_or_unpairs_and_an_agent_may_list() {
    let a = app("caps").await;
    let phone = Keys::generate(&mut rand::rng());
    let args = json!({ "handle": handle(0), "publicKey": phone.public, "platform": "ios", "device": "iPhone" });
    let agent = Caller::agent("t1", false);
    for (id, args) in [("push.pair", args.clone()), ("push.unpair", json!({ "id": "x" }))] {
        let e = a.k.call(agent.clone(), id, args).await.unwrap_err();
        assert_eq!(e.code, Code::Forbidden, "{id}: {}", e.message);
    }
    a.call("push.pair", args).await;
    let list = a.k.call(agent, "push.devices", json!({})).await.unwrap();
    assert_eq!(list["devices"][0]["device"], "iPhone", "an agent can say which phones get alerts");
    let e =
        a.k.call(Caller::user(), "push.pair", json!({ "handle": "x", "publicKey": phone.public, "platform": "ios", "device": "a" }))
            .await
            .unwrap_err();
    assert!(e.message.starts_with("handle:"), "{}", e.message);
}

#[tokio::test(flavor = "multi_thread")]
async fn unpairing_the_last_phone_switches_the_channel_off() {
    let a = app("unpair").await;
    let (p1, p2) = (Keys::generate(&mut rand::rng()), Keys::generate(&mut rand::rng()));
    let d1 = a.pair(0, &p1, "Pixel").await;
    let d2 = a.pair(1, &p2, "iPad").await;
    eventually("the push channel runs", async || a.push_on().await).await;
    assert_eq!(a.call("push.unpair", json!({ "id": d1["id"] })).await["removed"], true);
    assert_eq!(a.call("push.devices", json!({})).await["devices"][0]["id"], d2["id"]);
    assert_eq!(a.channel_spec().await["enabled"], true, "one phone left: still on");
    let e = a.k.call(Caller::user(), "push.unpair", json!({ "id": d1["id"] })).await.unwrap_err();
    assert_eq!(e.code, Code::NotFound);
    a.call("push.unpair", json!({ "id": d2["id"] })).await;
    assert_eq!(a.channel_spec().await["enabled"], false);
    eventually("the push channel stopped", async || !a.push_on().await).await;
    assert!(a.secrets.0.lock().unwrap().get("push/devices").is_none(), "nothing kept for no phones");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_notification_opens_with_its_device_key_only() {
    let a = app("open").await;
    let (p1, p2, stranger) = (Keys::generate(&mut rand::rng()), Keys::generate(&mut rand::rng()), Keys::generate(&mut rand::rng()));
    a.pair(0, &p1, "Pixel").await;
    a.pair(1, &p2, "iPad").await;
    eventually("the push channel runs", async || a.push_on().await).await;
    a.rt.ask("q1", &["main", "dev"]);
    a.notify().await;
    eventually("both phones were sent one", async || a.sends().len() == 2).await;
    for (url, body) in a.sends() {
        assert_eq!(url, "http://wisp.localhost:8080/api/push/send");
        let mut keys: Vec<&str> = body.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["collapse", "handle", "sealed"], "the relay sees these only: {body}");
        assert_eq!(body["collapse"], "t1", "a newer note for the task replaces this one");
        assert!(!body.to_string().contains("Fix the build") && !body.to_string().contains("Which branch"), "no plaintext: {body}");
    }
    let sends = a.sends();
    let of = |h: &str| sends.iter().find(|(_, b)| b["handle"] == h).map(|(_, b)| b["sealed"].as_str().unwrap().to_owned()).unwrap();
    let (s1, s2) = (of(&handle(0)), of(&handle(1)));
    let n: Note = open(&s1, &p1.secret, "t1").unwrap();
    assert_eq!(n.target, json!({ "task": "t1" }));
    assert!(n.title.contains("Fix the build"), "{n:?}");
    assert_eq!(n.choices.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["main", "dev"]);
    assert!(open(&s2, &p2.secret, "t1").is_ok());
    assert!(open(&s1, &p2.secret, "t1").is_err(), "the other phone's key opens nothing");
    assert!(open(&s1, &stranger.secret, "t1").is_err());
    assert!(open(&s2, &p1.secret, "t1").is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_notification_answer_lands_once() {
    let a = app("answer").await;
    let phone = Keys::generate(&mut rand::rng());
    a.pair(0, &phone, "Pixel").await;
    eventually("the push channel runs", async || a.push_on().await).await;
    a.rt.ask("q1", &["main", "dev"]);
    a.notify().await;
    eventually("the phone was sent one", async || a.sends().len() == 1).await;
    let n: Note = open(a.sends()[0].1["sealed"].as_str().unwrap(), &phone.secret, "t1").unwrap();
    let tap = serde_json::to_value(&n.choices[1].tap).unwrap();

    let out = a.call("push.answer", tap.clone()).await;
    assert_eq!(out["line"], "Answered.");
    for again in [tap.clone(), tap.clone()] {
        let e = a.k.call(Caller::user(), "push.answer", again).await.unwrap_err();
        assert_eq!(e.code, Code::Conflict, "{}", e.message);
    }
    // another choice from the same note: the question is answered already
    let other = serde_json::to_value(&n.choices[0].tap).unwrap();
    assert_eq!(a.k.call(Caller::user(), "push.answer", other).await.unwrap_err().code, Code::Conflict);
    assert_eq!(a.rt.ran(), [("answer".to_owned(), "dev@q1".to_owned())], "it landed once, with the choice's label and its question");

    // a tap this wisp didn't sign is refused, and so is a signed one with its index changed
    let mut forged = tap.clone();
    forged["i"] = json!(0);
    assert_eq!(a.k.call(Caller::user(), "push.answer", forged).await.unwrap_err().code, Code::Forbidden);
    // an agent holds no phone
    assert_eq!(a.k.call(Caller::agent("t1", false), "push.answer", tap).await.unwrap_err().code, Code::Forbidden);

    // a reply to a finished task's note is a follow-up, sent once
    a.notify().await;
    eventually("a second note", async || a.sends().len() == 2).await;
    let n: Note = open(a.sends()[1].1["sealed"].as_str().unwrap(), &phone.secret, "t1").unwrap();
    let mut reply = serde_json::to_value(n.reply.as_ref().unwrap()).unwrap();
    reply["text"] = json!("and run the tests");
    a.call("push.answer", reply.clone()).await;
    assert_eq!(a.k.call(Caller::user(), "push.answer", reply).await.unwrap_err().code, Code::Conflict);
    assert_eq!(a.rt.ran().last().unwrap(), &("followUp".to_owned(), "and run the tests".to_owned()));
    assert_eq!(a.rt.ran().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_gone_token_unpairs_its_phone_and_the_last_switches_the_channel_off_with_the_reason() {
    let a = app("gone").await;
    let (p1, p2) = (Keys::generate(&mut rand::rng()), Keys::generate(&mut rand::rng()));
    a.pair(0, &p1, "Pixel").await;
    a.pair(1, &p2, "Old iPhone").await;
    eventually("the push channel runs", async || a.push_on().await).await;
    a.relay.gone.lock().unwrap().push(handle(1));
    a.notify().await;
    eventually("the gone phone is unpaired", async || a.call("push.devices", json!({})).await["devices"].as_array().unwrap().len() == 1)
        .await;
    assert_eq!(a.channel_spec().await["enabled"], true, "the other phone still gets them");

    a.relay.gone.lock().unwrap().push(handle(0));
    a.notify().await;
    eventually("the channel is off", async || a.channel_spec().await["enabled"] == false).await;
    let list = a.call("push.devices", json!({})).await;
    assert_eq!(list["devices"], json!([]));
    assert!(list["dropped"].as_str().unwrap().starts_with("Pixel stopped getting notifications"), "{list}");
}
