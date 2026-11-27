//! MCP Events' webhook delivery (SEP-3415 `events/subscribe`, `events/unsubscribe`; ADR-0146): wisp POSTs its
//! durable events to a client's callback, signed per Standard Webhooks.
//!
//! - **Subscribe** is checked as a poll is (`events.poll` run as the caller: the type, the cursor, the capability,
//!   the task scope), then the secret (`whsec_` + 24 to 64 bytes) and the URL (`https`; `http` and private addresses
//!   only with `events.webhooks.allowPrivate`). The key is `(principal, url, name, arguments)` and its id a hash of
//!   it, so a refresh is an idempotent upsert: the TTL is granted again (1 h by default, 1 min to 24 h, never
//!   "no expiry"), the secret replaced, a suspended delivery resumed.
//! - **Verification** before the first delivery to a `(principal, url)`: a signed `verification` envelope whose
//!   challenge the endpoint echoes (compared in constant time), remembered for an hour.
//! - **Delivery**, one task a subscription: woken by the feed like a stream, each event sent in order through
//!   `Net::fetch_untrusted` (no redirects, a public address pinned at send time), retried at 2 s, 10 s, 1 min and
//!   5 min with a fresh timestamp and signature, abandoned on `410`/`413` or when the tries run out. The body's
//!   `cursor` is the watermark: everything before it was delivered or abandoned. Ten abandoned in a row suspend
//!   delivery until the next refresh. A cursor that fell out of retention sends a `gap` envelope.
//! - **State** is in memory, as short TTLs allow (SEP-3415): after a restart the client's next refresh brings the
//!   subscription back from its cursor.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use base64::Engine as _;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wisp_events_api as api;
use wisp_kernel::{Call, Ctx};
use wisp_mcp_api::{MethodAnswer, MethodError, MethodRequest};

use crate::mcp::{NOT_FOUND, iso, refusal};

const INVALID_PARAMS: i64 = -32602;
/// SEP-3415's `CallbackEndpointError`.
pub const CALLBACK_ERROR: i64 = -32027;
pub const RESOURCE_EXHAUSTED: i64 = -32025;
pub const ALLOW_PRIVATE: &str = "events.webhooks.allowPrivate";
const DEFAULT_TTL_MS: i64 = 60 * 60_000;
const MIN_TTL_MS: i64 = 60_000;
const MAX_TTL_MS: i64 = 24 * 60 * 60_000;
const VERIFIED_MS: i64 = 60 * 60_000;
/// Between tries of one delivery (the first goes at once): about six minutes in all.
pub const RETRY_MS: [u64; 4] = [2_000, 10_000, 60_000, 300_000];
pub(crate) const SUSPEND_AFTER: u32 = 10;
/// Subscriptions one caller may hold.
pub const MAX_PER_CALLER: usize = 32;

/// One subscription.
#[derive(Debug, Clone)]
pub(crate) struct Sub {
    pub(crate) principal: String,
    pub(crate) url: String,
    pub(crate) args: api::PollArgs,
    pub(crate) secret: Vec<u8>,
    /// past the last event delivered or abandoned (the log's seq)
    pub(crate) cursor: String,
    pub(crate) expires_at: i64,
    pub(crate) active: bool,
    pub(crate) abandoned_in_a_row: u32,
    pub(crate) last_delivery_at: Option<i64>,
    pub(crate) last_error: Option<&'static str>,
    pub(crate) failed_since: Option<i64>,
    /// a delivery task is running for it
    pub(crate) running: bool,
}

#[derive(Default)]
pub struct Hooks {
    pub(crate) subs: Mutex<HashMap<String, Sub>>,
    /// `(principal, url)` → verified until
    verified: Mutex<HashMap<(String, String), i64>>,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Who asks: the caller's capability set and, for a task's agent, its task.
pub fn principal(call: &Call) -> String {
    let c = call.caller();
    match c.session.as_deref() {
        Some(s) => format!("{}:{s}", c.set),
        None => c.set.clone(),
    }
}

/// The subscription's id: a hash of its key, stable across refreshes and restarts (SEP-3415 "Derived id").
pub fn sub_id(principal: &str, url: &str, name: &str, arguments: &Value) -> String {
    let key = json!([principal, url, name, canonical(arguments)]).to_string();
    let h = Sha256::digest(key.as_bytes());
    let hex = |b: &u8| [char::from(b"0123456789abcdef"[usize::from(b >> 4)]), char::from(b"0123456789abcdef"[usize::from(b & 15)])];
    format!("sub_{}", h.iter().take(8).flat_map(hex).collect::<String>())
}

/// JSON with every object's keys sorted, for equality of `arguments` regardless of their order.
pub fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            Value::Object(keys.into_iter().map(|k| (k.clone(), canonical(&o[k]))).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// A Standard Webhooks secret's key bytes: `whsec_` + base64 of 24 to 64 bytes, or why not.
pub fn secret_bytes(secret: &str) -> std::result::Result<Vec<u8>, String> {
    let b64 = secret.strip_prefix("whsec_").ok_or("delivery.secret: a Standard Webhooks secret starts with whsec_")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|_| "delivery.secret: whsec_ then base64".to_owned())?;
    if !(24..=64).contains(&bytes.len()) {
        return Err(format!("delivery.secret: 24 to 64 bytes, not {}", bytes.len()));
    }
    Ok(bytes)
}

/// `v1,<base64 HMAC-SHA256(secret, id.timestamp.body)>`.
pub fn sign(secret: &[u8], id: &str, ts: i64, body: &[u8]) -> String {
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else { return String::new() };
    mac.update(format!("{id}.{ts}.").as_bytes());
    mac.update(body);
    format!("v1,{}", base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
}

/// The URL a callback may have: `https`, or (allowed) `http`.
pub fn check_url(url: &str, allow_private: bool) -> std::result::Result<(), String> {
    let u = url::Url::parse(url).map_err(|e| format!("delivery.url: {e}"))?;
    match u.scheme() {
        "https" => Ok(()),
        "http" if allow_private => Ok(()),
        s => Err(format!("delivery.url: https only ({s})")),
    }?;
    if u.host_str().is_none_or(str::is_empty) {
        return Err("delivery.url: no host".into());
    }
    Ok(())
}

/// The TTL granted for a suggestion: 1 min to 24 h, 1 h by default; `null` (no expiry) is granted the most.
pub fn grant(ttl: Option<&Value>) -> i64 {
    match ttl {
        None => DEFAULT_TTL_MS,
        Some(Value::Null) => MAX_TTL_MS,
        Some(v) => v.as_i64().unwrap_or(DEFAULT_TTL_MS).clamp(MIN_TTL_MS, MAX_TTL_MS),
    }
}

fn invalid(m: impl Into<String>) -> MethodError {
    MethodError::new(INVALID_PARAMS, m)
}

/// The endpoint's intent, confirmed once an hour per `(principal, url)` (SEP-3415 "Endpoint verification").
async fn verified(
    ctx: &Ctx,
    hooks: &Hooks,
    key: (String, String),
    id: &str,
    secret: &[u8],
    now: i64,
) -> std::result::Result<(), MethodError> {
    if lock(&hooks.verified).get(&key).is_some_and(|until| *until > now) {
        return Ok(());
    }
    crate::delivery::verify(ctx, &key.1, id, secret)
        .await
        .map_err(|why| MethodError::new(CALLBACK_ERROR, "CallbackEndpointError").with_data(json!({ "reason": why })))?;
    lock(&hooks.verified).insert(key, now + VERIFIED_MS);
    Ok(())
}

/// How a live subscription's delivery is doing (a refresh's `deliveryStatus`).
fn status_of(s: &Sub) -> Value {
    let mut st = json!({ "active": s.active, "lastError": s.last_error });
    if let Some(at) = s.last_delivery_at {
        st["lastDeliveryAt"] = json!(iso(at));
    }
    if let Some(at) = s.failed_since {
        st["failedSince"] = json!(iso(at));
    }
    st
}

/// A subscribe as an upsert: a live one is refreshed in place (its TTL, secret, delivery resumed) and says how it's
/// doing; a new one is kept. Whether its delivery task must start.
fn upsert(hooks: &Hooks, id: &str, fresh: Sub, now: i64) -> (Option<Value>, bool) {
    let mut subs = lock(&hooks.subs);
    if let Some(s) = subs.get_mut(id).filter(|s| s.expires_at > now) {
        let status = status_of(s);
        s.expires_at = fresh.expires_at;
        s.secret = fresh.secret;
        s.active = true;
        s.abandoned_in_a_row = 0;
        let spawn = !s.running;
        s.running = true;
        return (Some(status), spawn);
    }
    subs.insert(id.to_owned(), fresh);
    (None, true)
}

/// Where delivery starts: a live subscription where it is (the client's cursor is behind it: a no-op), else the
/// client's cursor, else now; and whether that skipped events (the cursor fell out of retention).
fn start_of(live: Option<&str>, asked: Option<&str>, first: &Value) -> (String, bool) {
    match (live, asked) {
        (Some(c), _) => (c.to_owned(), false),
        (None, None) => (first.get("cursor").and_then(Value::as_str).unwrap_or("0").to_owned(), false),
        (None, Some(c)) => (c.to_owned(), first.get("truncated") == Some(&Value::Bool(true))),
    }
}

/// Room for one more of `principal`'s subscriptions, or the limit.
fn room(hooks: &Hooks, principal: &str, now: i64) -> std::result::Result<(), MethodError> {
    let mine = lock(&hooks.subs).values().filter(|s| s.principal == principal && s.expires_at > now).count();
    if mine >= MAX_PER_CALLER {
        return Err(MethodError::new(RESOURCE_EXHAUSTED, format!("at most {MAX_PER_CALLER} webhook subscriptions a caller"))
            .with_data(json!({ "limit": "subscriptions", "max": MAX_PER_CALLER })));
    }
    Ok(())
}

/// `events/subscribe`.
pub async fn subscribe(
    ctx: Ctx,
    call: Call,
    req: MethodRequest,
    hooks: Arc<Hooks>,
    newest_seq: Arc<tokio::sync::watch::Sender<u64>>,
) -> std::result::Result<MethodAnswer, MethodError> {
    let p = &req.params;
    let delivery = p.get("delivery").cloned().unwrap_or(Value::Null);
    let url = delivery.get("url").and_then(Value::as_str).ok_or_else(|| invalid("events/subscribe: delivery.url"))?.to_owned();
    let secret = secret_bytes(delivery.get("secret").and_then(Value::as_str).unwrap_or_default()).map_err(invalid)?;
    let allow = ctx.setting(ALLOW_PRIVATE) == Some(Value::Bool(true));
    check_url(&url, allow).map_err(invalid)?;
    let ttl = grant(p.get("ttlMs"));
    let mut args = crate::mcp::poll_args(p.clone(), "events/subscribe")?;
    let principal = principal(&call);
    let arguments = canonical(&args.arguments);
    let id = sub_id(&principal, &url, &args.name, &arguments);
    let now = ctx.clock().map_or(0, |c| c.now_ms());
    // the checks a poll makes, and where delivery starts
    args.max_events = Some(1);
    let live_cursor = lock(&hooks.subs).get(&id).filter(|s| s.expires_at > now).map(|s| s.cursor.clone());
    let first = ctx
        .run::<api::Poll>(&call, api::PollArgs { cursor: live_cursor.clone().or(args.cursor.clone()), ..args.clone() })
        .await
        .map_err(|e| refusal(&e))?;
    let (start, truncated) = start_of(live_cursor.as_deref(), args.cursor.as_deref(), &first);
    if live_cursor.is_none() {
        room(&hooks, &principal, now)?;
    }
    verified(&ctx, &hooks, (principal.clone(), url.clone()), &id, &secret, now).await?;
    args.max_events = None;
    let fresh = Sub {
        principal,
        url,
        args,
        secret,
        cursor: start.clone(),
        expires_at: now + ttl,
        active: true,
        abandoned_in_a_row: 0,
        last_delivery_at: None,
        last_error: None,
        failed_since: None,
        running: true,
    };
    let (status, spawn) = upsert(&hooks, &id, fresh, now);
    if spawn {
        let (c, h, id2) = (ctx.clone(), hooks.clone(), id.clone());
        let woken = newest_seq.subscribe();
        ctx.spawn(async move { crate::delivery::deliver(c, call, h, id2, woken).await });
    }
    let cursor = lock(&hooks.subs).get(&id).map_or(start, |s| s.cursor.clone());
    let mut out = json!({ "resultType": "complete", "id": id, "refreshBefore": iso(now + ttl), "cursor": cursor, "truncated": truncated });
    if let Some(st) = status {
        out["deliveryStatus"] = st;
    }
    Ok(MethodAnswer::Result(out))
}

/// `events.webhooks`: the live subscriptions, the caller's own (the user's: every one); never their secrets.
pub fn list(call: &Call, hooks: &Hooks, now: i64) -> Value {
    let me = principal(call);
    let everyone = call.caller().session.is_none() && call.caller().set.starts_with("user");
    let mut out: Vec<Value> = lock(&hooks.subs)
        .iter()
        .filter(|(_, s)| s.expires_at > now && (everyone || s.principal == me))
        .map(|(id, s)| {
            json!({
                "id": id, "principal": s.principal, "url": s.url, "name": s.args.name, "arguments": s.args.arguments,
                "cursor": s.cursor, "refreshBefore": iso(s.expires_at), "active": s.active, "lastError": s.last_error,
                "lastDeliveryAt": s.last_delivery_at.map(iso),
            })
        })
        .collect();
    out.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Value::Array(out)
}

/// `events/unsubscribe`: by the same key; the derived id is never taken as input.
pub fn unsubscribe(call: &Call, req: &MethodRequest, hooks: &Hooks) -> std::result::Result<MethodAnswer, MethodError> {
    let p = &req.params;
    let url = p.pointer("/delivery/url").and_then(Value::as_str).ok_or_else(|| invalid("events/unsubscribe: delivery.url"))?;
    let name = p.get("name").and_then(Value::as_str).ok_or_else(|| invalid("events/unsubscribe: name"))?;
    let arguments = canonical(p.get("arguments").filter(|a| !a.is_null()).unwrap_or(&json!({})));
    let id = sub_id(&principal(call), url, name, &arguments);
    match lock(&hooks.subs).remove(&id) {
        Some(_) => Ok(MethodAnswer::Result(json!({ "resultType": "complete" }))),
        None => Err(MethodError::new(NOT_FOUND, "No such subscription").with_data(json!({ "kind": "subscription" }))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::delivery::{category, message_id};

    #[test]
    fn the_key_is_order_free_and_the_id_stable() {
        let a = sub_id("user:full", "https://h/x", "task.finished", &canonical(&json!({ "b": 1, "a": [{ "y": 2, "x": 1 }] })));
        let b = sub_id("user:full", "https://h/x", "task.finished", &canonical(&json!({ "a": [{ "x": 1, "y": 2 }], "b": 1 })));
        assert_eq!(a, b);
        assert!(a.starts_with("sub_") && a.len() == 20);
        assert_ne!(a, sub_id("agent:session:t1", "https://h/x", "task.finished", &json!({ "a": [{ "x": 1, "y": 2 }], "b": 1 })));
    }

    #[test]
    fn secrets_are_standard_webhooks_ones() {
        let ok = format!("whsec_{}", base64::engine::general_purpose::STANDARD.encode([7u8; 32]));
        assert_eq!(secret_bytes(&ok).unwrap().len(), 32);
        assert!(secret_bytes("abc").is_err());
        assert!(secret_bytes(&format!("whsec_{}", base64::engine::general_purpose::STANDARD.encode([7u8; 8]))).is_err());
        assert!(secret_bytes("whsec_!!!").is_err());
    }

    #[test]
    fn the_signature_is_standard_webhooks_v1() {
        // Standard Webhooks' published test vector
        let secret = secret_bytes("whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw").unwrap();
        let sig = sign(&secret, "msg_p5jXN8AQM9LWM0D4loKWxJek", 1_614_265_330, br#"{"test": 2432232314}"#);
        assert_eq!(sig, "v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=");
    }

    #[test]
    fn urls_ttls_and_categories() {
        assert!(check_url("https://h.example/hook", false).is_ok());
        assert!(check_url("http://127.0.0.1:9/hook", false).is_err());
        assert!(check_url("http://127.0.0.1:9/hook", true).is_ok());
        assert!(check_url("ftp://h/x", true).is_err());
        assert_eq!(grant(None), DEFAULT_TTL_MS);
        assert_eq!(grant(Some(&json!(5))), MIN_TTL_MS);
        assert_eq!(grant(Some(&json!(i64::MAX))), MAX_TTL_MS);
        assert_eq!(grant(Some(&Value::Null)), MAX_TTL_MS, "no expiry is never granted");
        assert_eq!(category(Some(404), None), "http_4xx");
        assert_eq!(category(Some(502), None), "http_5xx");
        assert_eq!(category(None, Some("timeout")), "timeout");
        assert_eq!(category(None, Some("refused")), "connection_refused");
        assert!(message_id("gap").starts_with("msg_gap_"));
        assert!(message_id("").chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }
}
