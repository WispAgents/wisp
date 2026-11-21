//! The phone relay through the grid's own routes (ADR-0065 §3 and its edge cases): a phone registers its FCM or APNs
//! token with its device token and gets a handle; a wisp sends `{handle, sealed, collapse}` and nothing else; FCM and
//! APNs (a fake here: no real call leaves the test) get the ciphertext, the collapse key and APNs' fixed fallback; a
//! token they say is gone drops the handle and every later send answers `gone`, which wisp reads as "unpair it".
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_grid::accounts::Poll;
use wisp_grid::push::{Delivered, Outgoing, Platform, PushSender, Target, apns_payload, fcm_message};
use wisp_push_core::{Relayed, classify};

/// Runs while a send waits on FCM (the phone refreshing its token meanwhile).
type During = Box<dyn Fn() + Send + Sync>;

/// FCM and APNs: every note recorded; a token in `gone` is unregistered.
#[derive(Default)]
struct Fake {
    got: Mutex<Vec<(Target, Outgoing)>>,
    gone: Mutex<Vec<String>>,
    ios: bool,
    during: Mutex<Option<During>>,
}
#[async_trait]
impl PushSender for Fake {
    fn serves(&self, p: Platform) -> bool {
        p == Platform::Android || self.ios
    }
    async fn send(&self, to: &Target, msg: &Outgoing, _now_ms: i64) -> Delivered {
        self.got.lock().unwrap().push((to.clone(), msg.clone()));
        if let Some(f) = self.during.lock().unwrap().take() {
            f();
        }
        if self.gone.lock().unwrap().contains(&to.token) { Delivered::Gone } else { Delivered::Sent }
    }
}

struct G {
    grid: Arc<wisp_grid::edge::Grid>,
    port: u16,
    dir: PathBuf,
    stop: tokio_util::sync::CancellationToken,
}
impl Drop for G {
    fn drop(&mut self) {
        self.stop.cancel();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn grid(name: &str, fake: Option<Arc<Fake>>) -> G {
    let dir = std::env::temp_dir().join(format!("wisp-grid-push-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (grid, port, stop) = wisp_grid::edge::dev(&dir, 0, PathBuf::from("/nonexistent/wisp"), Vec::new(), 600_000).await.unwrap();
    grid.push.set_sender(fake.map(|f| f as Arc<dyn PushSender>));
    G { grid, port, dir, stop }
}

impl G {
    fn url(&self, path: &str) -> String {
        format!("http://wisp.localhost:{}{path}", self.port)
    }
    fn web(&self) -> reqwest::Client {
        reqwest::Client::builder().no_proxy().resolve("wisp.localhost", ([127, 0, 0, 1], self.port).into()).build().unwrap()
    }
    /// A device token for `handle` (the phone's own sign-in, approved at once).
    fn device(&self, handle: &str, name: &str) -> String {
        let a = &self.grid.accounts;
        let u = a.dev_user(handle).unwrap();
        let c = a.device_code(name).unwrap();
        a.decide(c["user_code"].as_str().unwrap(), &u.id, true).unwrap();
        match a.poll(c["device_code"].as_str().unwrap()).unwrap() {
            Poll::Token { token, .. } => token,
            other => panic!("{other:?}"),
        }
    }
    async fn post(&self, path: &str, bearer: Option<&str>, body: &Value) -> (u16, Value) {
        let mut r = self.web().post(self.url(path)).json(body);
        if let Some(b) = bearer {
            r = r.bearer_auth(b);
        }
        let r = r.send().await.unwrap();
        (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
    }
    async fn register(&self, dev: &str, platform: &str, token: &str) -> String {
        let (s, v) = self.post("/api/push/register", Some(dev), &json!({ "platform": platform, "token": token })).await;
        assert_eq!(s, 200, "{v}");
        v["handle"].as_str().unwrap().to_owned()
    }
    fn on_disk(&self) -> Vec<u8> {
        std::fs::read_dir(&self.dir).unwrap().flat_map(|e| std::fs::read(e.unwrap().path()).unwrap_or_default()).collect()
    }
}

/// What wisp sends: base64url of 32 bytes of encapsulated key and the ciphertext (here random bytes stand in).
fn sealed() -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode((0..120u8).map(|i| i.wrapping_mul(37)).collect::<Vec<_>>())
}

fn holds(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

#[tokio::test(flavor = "multi_thread")]
async fn the_relay_sees_only_ciphertext() {
    let fake = Arc::new(Fake { ios: true, ..Fake::default() });
    let g = grid("cipher", Some(fake.clone())).await;
    let dev = g.device("kai", "Pixel 9 (wisp app)");
    let handle = g.register(&dev, "android", "fcm-token-1").await;
    assert!(handle.starts_with("wph_") && handle.len() == 47, "{handle}");

    // what wisp-push posts (its SendBody) goes through as is
    let body = serde_json::to_value(wisp_push_api::SendBody { handle: handle.clone(), sealed: sealed(), collapse: "t1".into() }).unwrap();
    let (s, v) = g.post("/api/push/send", None, &body).await;
    assert_eq!((s, v.clone()), (200, json!({ "ok": true })));
    assert_eq!(classify(s, v["error"].as_str()), Relayed::Sent);
    let got = fake.got.lock().unwrap().clone();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, Target { platform: Platform::Android, token: "fcm-token-1".into() });
    assert_eq!(got[0].1, Outgoing { sealed: sealed(), collapse: "t1".into() }, "ciphertext and the collapse key, nothing else");

    // what FCM and APNs are given: the same two strings (and APNs' fixed fallback), never a title or body of wisp's
    let fcm = fcm_message(&got[0].0, &got[0].1);
    assert_eq!(fcm["message"]["data"], json!({ "sealed": sealed(), "collapse": "t1" }));
    assert_eq!(fcm["message"]["android"]["collapse_key"], "t1");
    let apns = apns_payload(&got[0].1);
    assert_eq!(apns["aps"]["alert"]["body"], wisp_push_api::FALLBACK);
    assert_eq!(apns["aps"]["mutable-content"], 1);

    // a body with anything more is refused before it goes anywhere
    let mut leaky = body.clone();
    leaky["title"] = json!("Fix the build");
    let (s, v) = g.post("/api/push/send", None, &leaky).await;
    assert_eq!(s, 400, "{v}");
    for bad in [
        json!({ "handle": handle, "sealed": "not base64!", "collapse": "t1" }),
        json!({ "handle": handle, "sealed": sealed(), "collapse": "a b" }),
    ] {
        assert_eq!(g.post("/api/push/send", None, &bad).await.0, 400, "{bad}");
    }
    assert_eq!(fake.got.lock().unwrap().len(), 1, "nothing refused reached FCM");

    // the grid keeps the handle's hash and the token, never the handle or a note
    let disk = g.on_disk();
    assert!(!holds(&disk, &handle), "the handle itself isn't kept");
    assert!(!holds(&disk, &sealed()[..40]), "no note is kept");

    // only a signed-in device registers; the same device naming its handle keeps it
    let (s, _) = g.post("/api/push/register", None, &json!({ "platform": "android", "token": "x" })).await;
    assert_eq!(s, 401);
    let (s, v) =
        g.post("/api/push/register", Some(&dev), &json!({ "platform": "android", "token": "fcm-token-2", "handle": handle })).await;
    assert_eq!((s, v["handle"].as_str()), (200, Some(handle.as_str())), "a refreshed token keeps its handle");
    let other = g.device("ana", "Ana's phone");
    let (_, v) = g.post("/api/push/register", Some(&other), &json!({ "platform": "ios", "token": "apns-1", "handle": handle })).await;
    assert_ne!(v["handle"].as_str(), Some(handle.as_str()), "another device can't take a handle over");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_gone_token_turns_its_channel_off() {
    let fake = Arc::new(Fake::default());
    let g = grid("gone", Some(fake.clone())).await;
    let dev = g.device("kai", "Pixel (wisp app)");
    let handle = g.register(&dev, "android", "fcm-old").await;
    fake.gone.lock().unwrap().push("fcm-old".into());
    let body = json!({ "handle": handle, "sealed": sealed(), "collapse": "t1" });

    let (s, v) = g.post("/api/push/send", None, &body).await;
    assert_eq!((s, v.clone()), (410, json!({ "error": "gone" })));
    assert_eq!(classify(s, v["error"].as_str()), Relayed::Gone, "wisp unpairs the phone and, the last one, switches push off");
    let user = g.grid.accounts.user_by_handle("kai").unwrap().unwrap();
    assert_eq!(g.grid.push.handles_of(&user.id).unwrap(), 0, "the relay dropped the handle");

    // a later send for it is gone too, without asking FCM again
    let (s, _) = g.post("/api/push/send", None, &body).await;
    assert_eq!(s, 410);
    assert_eq!(fake.got.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_credentials_the_relay_is_unavailable_and_sends_are_limited_per_handle() {
    let none = grid("none", None).await;
    let dev = none.device("kai", "Pixel (wisp app)");
    let (s, v) = none.post("/api/push/register", Some(&dev), &json!({ "platform": "android", "token": "t" })).await;
    assert_eq!((s, v["error"].as_str()), (503, Some("unavailable")), "the app keeps ntfy");

    // FCM only: an iPhone can't register
    let fake = Arc::new(Fake::default());
    let g = grid("limit", Some(fake.clone())).await;
    let dev = g.device("kai", "Pixel (wisp app)");
    let (s, _) = g.post("/api/push/register", Some(&dev), &json!({ "platform": "ios", "token": "abcd" })).await;
    assert_eq!(s, 503);
    let handle = g.register(&dev, "android", "fcm-1").await;
    let body = json!({ "handle": handle, "sealed": sealed(), "collapse": "t1" });
    for _ in 0..wisp_grid::push::SEND_PER_MINUTE {
        assert_eq!(g.post("/api/push/send", None, &body).await.0, 200);
    }
    let (s, v) = g.post("/api/push/send", None, &body).await;
    assert_eq!(s, 429, "{v}");
    assert_eq!(classify(s, None), Relayed::Busy);
    // FCM credentials gone mid-way: unavailable, not gone (the phone stays paired, wisp keeps ntfy)
    let other = g.register(&g.device("ana", "Ana (wisp app)"), "android", "fcm-2").await;
    g.grid.push.set_sender(None);
    let (s, v) = g.post("/api/push/send", None, &json!({ "handle": other, "sealed": sealed(), "collapse": "t1" })).await;
    assert_eq!((s, v["error"].as_str()), (503, Some("unavailable")));
    assert!(matches!(classify(s, v["error"].as_str()), Relayed::Unavailable(_)));
    let user = g.grid.accounts.user_by_handle("ana").unwrap().unwrap();
    assert_eq!(g.grid.push.handles_of(&user.id).unwrap(), 1, "kept for when the relay is back");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_refreshed_while_its_old_one_is_said_gone_keeps_the_handle() {
    let fake = Arc::new(Fake::default());
    let g = grid("refresh", Some(fake.clone())).await;
    let dev = g.device("kai", "Pixel (wisp app)");
    let handle = g.register(&dev, "android", "fcm-old").await;
    fake.gone.lock().unwrap().push("fcm-old".into());
    let (user, device) = g.grid.accounts.device(&dev).unwrap().unwrap();
    let (grid, h) = (g.grid.clone(), handle.clone());
    *fake.during.lock().unwrap() = Some(Box::new(move || {
        let b = json!({ "platform": "android", "token": "fcm-new", "handle": h }).as_object().unwrap().clone();
        grid.push.register(&user.id, &device.id, &b).unwrap();
    }));
    let body = json!({ "handle": handle, "sealed": sealed(), "collapse": "t1" });
    let (s, _) = g.post("/api/push/send", None, &body).await;
    assert_eq!(s, 410, "the old token was gone");
    let (s, v) = g.post("/api/push/send", None, &body).await;
    assert_eq!((s, v), (200, json!({ "ok": true })), "the refreshed token kept its handle");
    assert_eq!(fake.got.lock().unwrap().last().unwrap().0.token, "fcm-new");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_apns_token_is_hex_and_another_device_cant_move_a_handle() {
    let fake = Arc::new(Fake { ios: true, ..Fake::default() });
    let g = grid("hex", Some(fake.clone())).await;
    let dev = g.device("kai", "iPhone (wisp app)");
    let (s, _) = g.post("/api/push/register", Some(&dev), &json!({ "platform": "ios", "token": "../../3/device/x" })).await;
    assert_eq!(s, 400, "a path in an APNs token is refused");
    let handle = g.register(&dev, "ios", "a1b2c3").await;
    let other = g.device("ana", "Ana's phone");
    g.post("/api/push/register", Some(&other), &json!({ "platform": "ios", "token": "ffff", "handle": handle })).await;
    let (s, _) = g.post("/api/push/send", None, &json!({ "handle": handle, "sealed": sealed(), "collapse": "t1" })).await;
    assert_eq!(s, 200);
    assert_eq!(fake.got.lock().unwrap()[0].0.token, "a1b2c3", "kai's handle still reaches kai's phone");
}
