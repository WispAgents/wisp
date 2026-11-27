//! A connector's MCP Events by webhook (SEP-3415; ADR-0145): with this wisp's public address set
//! (`connectors.events.callback`), an `mcp.event` trigger whose connector offers `webhook` subscribes with
//! `events/subscribe` and its events come to `POST /api/connectors/events/:token`.
//!
//! - **The secret** is made here (32 random bytes, `whsec_`), never by the server; the route's token (random) says
//!   which subscription a delivery is for before anything is parsed.
//! - **Each delivery** is checked before it's read: the Standard Webhooks signature over the raw body, a timestamp
//!   within five minutes, and a `webhook-id` not seen before. The event goes to the engine
//!   (`automations.trigger.push`) before the answer, so a `2xx` means it's taken; a push that fails answers `503`
//!   and the server tries again.
//! - **The engine's poll keeps it alive**, as for a stream: a poll while subscribed asks nothing (it returns the
//!   watermark the deliveries carried), refreshes at 80 % of the grant, and a subscription no trigger polled for three
//!   intervals is unsubscribed. A refused subscribe (the server can't reach the address, or refuses webhooks) leaves
//!   the trigger to push or poll for an hour.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_automations_api::commands::TriggerPush;
use wisp_connectors_core::events::{self as ev, Subscription};
use wisp_connectors_core::hook::{self, Body};
use wisp_kernel::http::{Request, Response};
use wisp_kernel::{Ctx, Result};

pub const CALLBACK: &str = "connectors.events.callback";
pub const ROUTE: &str = "/api/connectors/events/:token";
/// A subscription the server refused is not asked for again before this.
const REFUSED_MS: i64 = 60 * 60_000;
/// The `webhook-id`s remembered per subscription, to drop a retried delivery.
const SEEN: usize = 256;

struct Hook {
    token: String,
    secret: String,
    url: String,
    sub: Subscription,
    /// the watermark: everything before it was delivered (or abandoned by the server)
    cursor: Value,
    refresh_at: i64,
    touched: i64,
    idle_ms: i64,
    seen: VecDeque<String>,
}

#[derive(Default)]
pub struct Hooks {
    by_key: Mutex<HashMap<String, Hook>>,
    refused_until: Mutex<HashMap<String, i64>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// This wisp's public address, when one is set.
pub fn base(ctx: &Ctx) -> Option<String> {
    let v = ctx.setting(CALLBACK)?;
    let s = v.as_str()?.trim().trim_end_matches('/');
    (!s.is_empty()).then(|| s.to_owned())
}

fn random(n: usize) -> String {
    use rand::Rng as _;
    let mut r = rand::rng();
    (0..n).map(|_| char::from(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"[r.random_range(0..62)])).collect()
}

fn secret() -> String {
    use rand::Rng as _;
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    hook::secret(&bytes)
}

impl Hooks {
    /// The subscription's watermark while it's subscribed and not due for a refresh (nothing to ask), touched.
    pub fn current(&self, key: &str, idle_ms: i64, now: i64) -> Option<(Value, bool)> {
        let mut by = lock(&self.by_key);
        let h = by.get_mut(key)?;
        h.touched = now;
        h.idle_ms = idle_ms;
        Some((h.cursor.clone(), now >= h.refresh_at))
    }

    pub fn refused(&self, key: &str, now: i64) -> bool {
        lock(&self.refused_until).get(key).is_some_and(|u| *u > now)
    }

    pub fn refuse(&self, key: &str, now: i64) {
        lock(&self.by_key).remove(key);
        lock(&self.refused_until).insert(key.to_owned(), now + REFUSED_MS);
    }

    /// A new subscription's route and secret, kept before `events/subscribe` is sent: the server's verification
    /// arrives while it is still answering.
    pub fn open(&self, key: &str, sub: &Subscription, base: &str, cursor: Value, idle_ms: i64, now: i64) -> (String, String) {
        let token = random(24);
        let url = format!("{base}{}", ROUTE.replace(":token", &token));
        let s = secret();
        let h = Hook {
            token,
            secret: s.clone(),
            url: url.clone(),
            sub: sub.clone(),
            cursor,
            refresh_at: i64::MAX,
            touched: now,
            idle_ms,
            seen: VecDeque::new(),
        };
        lock(&self.by_key).insert(key.to_owned(), h);
        (url, s)
    }

    /// What `events/subscribe` granted.
    pub fn granted(&self, key: &str, result: &Value, now: i64) -> Value {
        let mut by = lock(&self.by_key);
        let Some(h) = by.get_mut(key) else { return Value::Null };
        if let Some(c) = result.get("cursor") {
            h.cursor = c.clone();
        }
        h.refresh_at = hook::refresh_at(now, result.get("refreshBefore").and_then(Value::as_str));
        h.cursor.clone()
    }

    /// The url, secret and cursor a refresh sends.
    pub fn refresh_of(&self, key: &str) -> Option<(String, String, Value)> {
        lock(&self.by_key).get(key).map(|h| (h.url.clone(), h.secret.clone(), h.cursor.clone()))
    }

    /// The subscriptions no trigger polled lately, taken out (to unsubscribe).
    pub fn idle(&self, now: i64) -> Vec<(Subscription, String)> {
        let mut by = lock(&self.by_key);
        let gone: Vec<String> = by.iter().filter(|(_, h)| now - h.touched > h.idle_ms).map(|(k, _)| k.clone()).collect();
        gone.into_iter().filter_map(|k| by.remove(&k)).map(|h| (h.sub, h.url)).collect()
    }
}

fn answer(status: u16, body: Value) -> Response {
    Response::Json { status, body }
}

/// One delivered event to the engine, before the answer (a `2xx` says it's taken); whether it got there.
async fn to_triggers(ctx: &Ctx, sub: &Subscription, o: wisp_connectors_core::events::Occurrence, now: i64) -> bool {
    let ev = wisp_automations_api::TriggerEvent {
        on: ev::ON.into(),
        key: o.key,
        at: o.at.unwrap_or(now),
        payload: o.payload,
        summary: o.summary,
        ..wisp_automations_api::TriggerEvent::default()
    };
    match ctx.run::<TriggerPush>(&ctx.root_call(), ev).await {
        Ok(_) => true,
        Err(e) => {
            tracing::warn!(connector = %sub.connector, "an event didn't reach the triggers: {}", e.message);
            false
        }
    }
}

/// `POST /api/connectors/events/:token`: one delivery.
pub async fn receive(ctx: Ctx, hooks: Arc<Hooks>, req: Request) -> Result<Response> {
    let token = req.param("token").unwrap_or_default().to_owned();
    let found = lock(&hooks.by_key).iter().find(|(_, h)| h.token == token).map(|(k, h)| (k.clone(), h.secret.clone(), h.sub.clone()));
    // gone: the server needn't try it again
    let Some((key, secret, sub)) = found else { return Ok(answer(410, json!({ "error": "no such subscription" }))) };
    let (id, ts, sig) = (
        req.header("webhook-id").unwrap_or_default(),
        req.header("webhook-timestamp").unwrap_or_default(),
        req.header("webhook-signature").unwrap_or_default(),
    );
    let now = ctx.clock()?.now_ms();
    if id.is_empty() || !hook::verified(&secret, id, ts, sig, &req.body, now / 1000) {
        return Ok(answer(401, json!({ "error": "the signature doesn't verify" })));
    }
    {
        let mut by = lock(&hooks.by_key);
        let Some(h) = by.get_mut(&key) else { return Ok(answer(410, json!({}))) };
        if h.seen.iter().any(|s| s == id) {
            return Ok(answer(200, json!({})));
        }
    }
    let Ok(v) = serde_json::from_slice::<Value>(&req.body) else { return Ok(answer(400, json!({ "error": "not JSON" }))) };
    let cursor = match hook::body(&sub, &v) {
        Body::Verification(c) => return Ok(answer(200, json!({ "challenge": c }))),
        Body::Event { occurrence, cursor } => {
            if !to_triggers(&ctx, &sub, occurrence, now).await {
                return Ok(answer(503, json!({ "error": "try again" })));
            }
            Some(cursor)
        }
        Body::Gap(c) => {
            tracing::warn!(connector = %sub.connector, event = %sub.event, "events were skipped: the server says a gap");
            Some(c)
        }
        Body::Terminated(why) => {
            tracing::warn!(connector = %sub.connector, event = %sub.event, "its webhook ended, polling for now: {why}");
            hooks.refuse(&key, now);
            return Ok(answer(200, json!({})));
        }
        Body::Other => None,
    };
    let mut by = lock(&hooks.by_key);
    if let Some(h) = by.get_mut(&key) {
        if let Some(c) = cursor {
            h.cursor = c;
        }
        h.seen.push_back(id.to_owned());
        while h.seen.len() > SEEN {
            h.seen.pop_front();
        }
    }
    Ok(answer(200, json!({})))
}
