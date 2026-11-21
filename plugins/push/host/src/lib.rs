//! wisp-push: the `push` transport for wisp.channels (ADR-0065 §3). A phone pairs itself (`push.pair`: the relay's
//! handle for its FCM/APNs token and its X25519 public key), which switches the one `push` channel on; the devices
//! are a list in `~/.wisp/secrets.json` (`push/devices`), never in the store. When channels sends (a task finished,
//! asks, waits for approval: the `phone-push` notification), each phone gets the note sealed to its own key and the
//! relay gets `{handle, sealed, collapse}` only. A choice or a reply comes back as `push.answer`, verified with the
//! hub's key and run once with the phone's own call, as the app's button would. A token the relay says is gone
//! unpairs that phone; the last one switches the channel off and `push.devices` says why.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_channels_api::views::{AskView, Msg, SessionView};
use wisp_channels_api::{ChannelHub, HttpRequest, Status, TRANSPORTS, Transport, TransportDef, TransportKind};
use wisp_kernel::{Call, CancellationToken, Ctx, Manifest, Plugin, Result, Setup, WireError};
use wisp_push_api::{Answer, Answered, Device, Devices, Kept, List, Pair, SECRET_KEY, SendBody, TRANSPORT, Unpair, Unpaired};
use wisp_push_core::devices::{self, drop_gone};
use wisp_push_core::{Act, Relayed, classify, decide, note, seal};
use wisp_resources_api::{Apply, Export, ExportArgs, PlanArgs, Resource};
use wisp_runtime_api::commands as rt;

/// How many answered taps are remembered, so a retried one doesn't run twice.
const RAN_KEPT: usize = 256;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug, Default)]
pub struct PushPlugin;

/// What the commands and the transport share.
#[derive(Default)]
struct Shared {
    ctx: OnceLock<Ctx>,
    /// the hub, once channels started the `push` channel (taps are verified with its key)
    hub: Mutex<Option<Arc<dyn ChannelHub>>>,
    /// one read-modify-write of the devices at a time, and one answer at a time
    devices_turn: tokio::sync::Mutex<()>,
    answer_turn: tokio::sync::Mutex<()>,
    /// taps that already ran (signature and text)
    ran: Mutex<VecDeque<String>>,
    /// devices paired now (the transport's `ready` can't wait on secrets)
    paired: std::sync::atomic::AtomicUsize,
}

impl Shared {
    fn ctx(&self) -> Result<Ctx> {
        self.ctx.get().cloned().ok_or_else(|| WireError::unavailable("wisp.push isn't set up yet"))
    }

    fn hub(&self) -> Result<Arc<dyn ChannelHub>> {
        lock(&self.hub).clone().ok_or_else(|| WireError::unavailable("Push is off on this wisp: pair a phone first."))
    }

    async fn load(&self, ctx: &Ctx) -> Result<Kept> {
        let text = wisp_kernel::services::secrets(ctx)?.get(SECRET_KEY).await?;
        let k = devices::read(text.as_deref());
        self.paired.store(k.devices.len(), std::sync::atomic::Ordering::Release);
        Ok(k)
    }

    async fn save(&self, ctx: &Ctx, k: &Kept) -> Result<()> {
        let sec = wisp_kernel::services::secrets(ctx)?;
        if k.devices.is_empty() && k.dropped.is_none() {
            sec.delete(SECRET_KEY).await?;
        } else {
            sec.set(SECRET_KEY, &serde_json::to_string(k).map_err(|e| WireError::internal(e.to_string()))?).await?;
        }
        self.paired.store(k.devices.len(), std::sync::atomic::Ordering::Release);
        Ok(())
    }
}

/// The `push` channel on or off (`resources.apply`). Switched on the first time, it also says what it sends:
/// questions and finished tasks (`phone-push`), as Settings → Phone's Send me does for ntfy.
async fn switch(ctx: &Ctx, on: bool, first: bool) -> Result<()> {
    let mut resources = vec![Resource::new("channel", TRANSPORT, json!({ "transport": TRANSPORT, "enabled": on }))];
    if on && first {
        let events: Vec<&str> = wisp_channels_api::NOTIFY_EVENTS.to_vec();
        resources.push(Resource::new("notification", format!("phone-{TRANSPORT}"), json!({ "on": events, "to": TRANSPORT })));
    }
    ctx.run::<Apply>(&ctx.root_call(), PlanArgs { resources, ..PlanArgs::default() }).await.map(|_| ())
}

/// Whether a `phone-push` notification is declared already (a person may have changed what it sends). A list that
/// can't be read counts as declared: never overwrite what someone set.
async fn told(ctx: &Ctx) -> bool {
    let args = ExportArgs { kinds: vec!["notification".into()], include: Vec::new() };
    let name = format!("phone-{TRANSPORT}");
    ctx.run::<Export>(&ctx.root_call(), args).await.map_or(true, |list| list.iter().any(|r| r.kind == "notification" && r.name == name))
}

/// A task as `runtime.get` answers it, as JSON (its open ask is in its `wisp.runtime` slot).
async fn task(ctx: &Ctx, call: &Call, id: &str) -> Result<Value> {
    let t = ctx.run::<rt::Get>(call, rt::IdArgs { id: id.into() }).await?;
    serde_json::to_value(t).map_err(|e| WireError::internal(e.to_string()))
}

/// The task's open ask, as `runtime.get` keeps it.
fn ask_of(task: &Value) -> Option<AskView> {
    task.pointer("/ext/wisp.runtime/ask").cloned().and_then(|a| serde_json::from_value(a).ok())
}

struct Def(Arc<Shared>);
impl TransportDef for Def {
    fn name(&self) -> &'static str {
        TRANSPORT
    }
    fn create(&self, hub: Arc<dyn ChannelHub>) -> Arc<dyn Transport> {
        *lock(&self.0.hub) = Some(hub.clone());
        Arc::new(PushTransport { hub, sh: self.0.clone() })
    }
    /// the phone pairs itself (`push.pair`): nothing to set up in Settings → Phone or `channels status`
    fn listed(&self) -> bool {
        false
    }
}

struct PushTransport {
    hub: Arc<dyn ChannelHub>,
    sh: Arc<Shared>,
}

impl PushTransport {
    fn mark(&self, error: Option<String>) {
        self.hub.mark(TRANSPORT, Status { ok: Some(error.is_none()), error, at: Some(self.hub.now_ms()), ..Status::default() });
    }

    /// One phone's note through its relay. → what the relay said.
    async fn send_one(&self, d: &Device, n: &wisp_push_api::Note, collapse: &str) -> Relayed {
        let sealed = match seal(n, &d.public_key, collapse, &mut rand::rng()) {
            Ok(s) => s,
            Err(e) => return Relayed::Failed(e.to_string()),
        };
        let body = SendBody { handle: d.handle.clone(), sealed, collapse: collapse.to_owned() };
        let Ok(body) = serde_json::to_value(&body) else { return Relayed::Failed("unsendable".into()) };
        let req = HttpRequest::post_json(format!("{}/api/push/send", d.relay), &body).timeout(10_000);
        match self.hub.net().fetch(req).await {
            Ok(r) => {
                let v = r.json();
                classify(r.status, v.get("error").and_then(Value::as_str))
            }
            Err(e) => Relayed::Unavailable(e.to_string()),
        }
    }

    /// The phones the relay said are gone, unpaired; the last one switches the channel off, saying why.
    async fn drop_handles(&self, ctx: &Ctx, gone: &[String]) {
        let _turn = self.sh.devices_turn.lock().await;
        let Ok(mut k) = self.sh.load(ctx).await else { return };
        for h in gone {
            drop_gone(&mut k, h);
        }
        if let Err(e) = self.sh.save(ctx, &k).await {
            tracing::warn!("push: a gone phone couldn't be unpaired: {}", e.message);
            return;
        }
        if k.devices.is_empty() {
            self.mark(k.dropped.clone());
            if let Err(e) = switch(ctx, false, false).await {
                tracing::warn!("push: the channel couldn't be switched off: {}", e.message);
            }
        }
    }
}

#[async_trait]
impl Transport for PushTransport {
    fn name(&self) -> &'static str {
        TRANSPORT
    }
    fn ready(&self) -> bool {
        self.sh.paired.load(std::sync::atomic::Ordering::Acquire) > 0
    }
    fn kind(&self) -> TransportKind {
        TransportKind::Phone
    }
    async fn start(&self, stop: CancellationToken) {
        // nothing to listen to: answers come back as `push.answer`
        if let Ok(ctx) = self.sh.ctx() {
            let _ = self.sh.load(&ctx).await;
        }
        stop.cancelled().await;
    }
    async fn publish(&self, msg: &Msg, s: Option<&SessionView>) -> Result<()> {
        let ctx = self.sh.ctx()?;
        let kept = self.sh.load(&ctx).await?;
        if kept.devices.is_empty() {
            return Err(WireError::unavailable("no phone is paired for push"));
        }
        let ask = match s {
            Some(s) => task(&ctx, &ctx.root_call(), &s.id).await.ok().as_ref().and_then(ask_of),
            None => None,
        };
        let hub = self.hub.clone();
        let sign = move |sid: &str, k, r: &str, i| hub.sign(sid, k, r, i);
        let n = note(msg, s, ask.as_ref(), &sign);
        // a notice about no task collapses with the other notices
        let collapse = s.map_or_else(|| "wisp".to_owned(), |s| s.id.clone());
        let (mut sent, mut gone, mut why) = (0, Vec::new(), None);
        for d in &kept.devices {
            match self.send_one(d, &n, &collapse).await {
                Relayed::Sent => sent += 1,
                Relayed::Gone => gone.push(d.handle.clone()),
                Relayed::Busy => why = Some("the relay is busy: too many notifications this minute".to_owned()),
                Relayed::Unavailable(m) => why = Some(format!("the push relay is unavailable ({m}); ntfy and chat channels still work")),
                Relayed::Failed(m) => why = Some(m),
            }
        }
        if !gone.is_empty() {
            self.drop_handles(&ctx, &gone).await;
        }
        if sent > 0 {
            self.mark(why);
            return Ok(());
        }
        let Some(why) = why else {
            // every phone was gone: `drop_handles` said why on the channel
            return Err(WireError::unavailable("every paired phone is gone"));
        };
        self.mark(Some(why.clone()));
        Err(WireError::unavailable(why))
    }
    fn view(&self) -> Option<Value> {
        Some(json!({ "devices": self.sh.paired.load(std::sync::atomic::Ordering::Acquire) }))
    }
}

/// The tap a reply or a choice came with, run once with the phone's own call.
async fn answer(sh: &Shared, ctx: &Ctx, call: &Call, a: wisp_push_api::AnswerArgs) -> Result<Answered> {
    let hub = sh.hub()?;
    if !hub.verify(&a.signed()) {
        return Err(WireError::forbidden("That answer isn't signed by this wisp."));
    }
    let _turn = sh.answer_turn.lock().await;
    let key = format!("{}\n{}", a.sig, a.text.as_deref().unwrap_or_default().trim());
    if lock(&sh.ran).contains(&key) {
        return Err(WireError::conflict("That was sent already."));
    }
    let t = task(ctx, call, &a.s).await?;
    let act = decide(&a.k, &a.r, a.i, a.text.as_deref(), ask_of(&t).as_ref())?;
    let id: wisp_runtime_api::SessionId = a.s.as_str().into();
    let line = match act {
        Act::Answer(text) => {
            // the question the tap answered: refused once it isn't the open one (a newer one replaced it)
            ctx.run::<rt::Answer>(call, rt::AnswerArgs { id, text, ask_id: Some(a.r.clone()).filter(|r| !r.is_empty()) }).await?;
            "Answered."
        }
        Act::FollowUp(text) => {
            ctx.run::<rt::FollowUp>(call, rt::FollowUpArgs { id, text, ..rt::FollowUpArgs::default() }).await?;
            "Sent."
        }
        Act::Done => {
            ctx.run::<rt::SetState>(call, rt::SetStateArgs { id, state: wisp_runtime_api::model::SessionState::Done }).await?;
            "Marked done."
        }
        Act::Stop => {
            ctx.run::<rt::Stop>(call, rt::StopArgs { id, clear_queue: false }).await?;
            "Stopped."
        }
    };
    let mut ran = lock(&sh.ran);
    ran.push_back(key);
    while ran.len() > RAN_KEPT {
        ran.pop_front();
    }
    Ok(Answered { line: line.to_owned() })
}

fn commands(s: &mut Setup, sh: &Arc<Shared>) {
    let h = sh.clone();
    s.command::<Pair, _, _>(move |ctx, _call, a| {
        let sh = h.clone();
        async move {
            devices::check_pair(&a).map_err(WireError::from)?;
            let _turn = sh.devices_turn.lock().await;
            let before = sh.load(&ctx).await?;
            let mut k = before.clone();
            let now = ctx.clock().map(|c| c.now_ms()).unwrap_or_default();
            let v = devices::pair(&mut k, &a, &wisp_kernel::ids::uuid7(), now).map_err(WireError::from)?;
            // what it sends is declared once; a person's own `phone-push` (or an unreadable list) is left alone
            let first = !told(&ctx).await;
            sh.save(&ctx, &k).await?;
            if let Err(e) = switch(&ctx, true, first).await {
                // the phone isn't paired if its channel isn't on: put the list back, so a retry starts over
                if let Err(back) = sh.save(&ctx, &before).await {
                    tracing::warn!("push: a refused pairing couldn't be undone: {}", back.message);
                }
                return Err(e);
            }
            Ok(v)
        }
    });
    let h = sh.clone();
    s.command::<Unpair, _, _>(move |ctx, _call, a| {
        let sh = h.clone();
        async move {
            let _turn = sh.devices_turn.lock().await;
            let mut k = sh.load(&ctx).await?;
            if !devices::unpair(&mut k, &a.id) {
                return Err(WireError::not_found("No such phone is paired."));
            }
            if k.devices.is_empty() {
                // off first: if that's refused, the phone is still paired and a retry finds it
                switch(&ctx, false, false).await?;
                k.dropped = None;
            }
            sh.save(&ctx, &k).await?;
            Ok(Unpaired { removed: true })
        }
    });
    let h = sh.clone();
    s.command::<List, _, _>(move |ctx, _call, _a| {
        let sh = h.clone();
        async move {
            let k = sh.load(&ctx).await?;
            Ok(Devices { devices: devices::views(&k), dropped: k.dropped })
        }
    });
    let h = sh.clone();
    s.command::<Answer, _, _>(move |ctx, call, a| {
        let sh = h.clone();
        async move { answer(&sh, &ctx, &call, a).await }
    });
}

impl Plugin for PushPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let sh = Arc::new(Shared::default());
        let _ = sh.ctx.set(s.ctx());
        let d: Arc<dyn TransportDef> = Arc::new(Def(sh.clone()));
        s.contribute(&TRANSPORTS, d);
        commands(s, &sh);
        let h = sh;
        s.on_start(move |ctx| async move {
            let _ = h.load(&ctx).await;
            Ok(())
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        let m = PushPlugin.manifest();
        assert_eq!(m.id, "wisp.push");
        assert!(m.validate().is_ok(), "{:?}", m.validate());
    }
}
