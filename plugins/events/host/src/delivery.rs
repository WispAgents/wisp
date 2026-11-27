//! MCP Events' webhook delivery, sending (ADR-0146): each message signed per Standard Webhooks and sent through
//! `Net::fetch_untrusted`, retried with a fresh timestamp and signature, and a subscription's events delivered in order
//! with a watermark cursor (see `webhook.rs` for subscribing).

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_events_api as api;
use wisp_kernel::services::{NetRequest, net};
use wisp_kernel::{Call, Ctx};

use crate::mcp::{FORBIDDEN, NOT_FOUND, refusal};
use crate::webhook::{ALLOW_PRIVATE, Hooks, RETRY_MS, SUSPEND_AFTER, lock, sign};

/// A message's id: high entropy, header-safe, never the event's (SEP-3415).
pub(crate) fn message_id(kind: &str) -> String {
    use rand::Rng as _;
    let mut r = rand::rng();
    let tail: String =
        (0..22).map(|_| char::from(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"[r.random_range(0..62)])).collect();
    if kind.is_empty() { format!("msg_{tail}") } else { format!("msg_{kind}_{tail}") }
}

/// The category a failed attempt is reported under (never the endpoint's own words: no response oracle).
pub fn category(status: Option<u16>, err: Option<&str>) -> &'static str {
    match (status, err) {
        (Some(s), _) if (400..500).contains(&s) => "http_4xx",
        (Some(_), _) => "http_5xx",
        (None, Some(e)) if e.contains("timeout") => "timeout",
        (None, Some(e)) if e.contains("tls") || e.contains("certificate") => "tls_error",
        _ => "connection_refused",
    }
}

/// One signed POST: its status, or the failure's category.
async fn post(
    ctx: &Ctx,
    url: &str,
    sub_id: &str,
    secret: &[u8],
    msg_id: &str,
    body: &[u8],
) -> std::result::Result<(u16, Vec<u8>), &'static str> {
    let now = ctx.clock().map_or(0, |c| c.now_ms()) / 1000;
    let headers = vec![
        ("content-type".to_owned(), "application/json".to_owned()),
        ("webhook-id".to_owned(), msg_id.to_owned()),
        ("webhook-timestamp".to_owned(), now.to_string()),
        ("webhook-signature".to_owned(), sign(secret, msg_id, now, body)),
        ("x-mcp-subscription-id".to_owned(), sub_id.to_owned()),
    ];
    let allow = ctx.setting(ALLOW_PRIVATE) == Some(Value::Bool(true));
    let req = NetRequest { method: "POST".into(), url: url.to_owned(), headers, body: body.to_vec(), timeout_ms: 5_000 };
    let n = net(ctx).map_err(|_| "connection_refused")?;
    match n.fetch_untrusted(req, allow).await {
        Ok(r) => Ok((r.status, r.body)),
        Err(e) => Err(category(None, Some(&e.message))),
    }
}

/// The endpoint proves it wants deliveries: it echoes a signed challenge (SEP-3415 "Endpoint verification").
pub(crate) async fn verify(ctx: &Ctx, url: &str, id: &str, secret: &[u8]) -> std::result::Result<(), &'static str> {
    use rand::Rng as _;
    let nonce: String = {
        let mut r = rand::rng();
        (0..32).map(|_| char::from(b"0123456789abcdef"[r.random_range(0..16)])).collect()
    };
    let body = json!({ "type": "verification", "challenge": nonce }).to_string();
    let (status, answer) = post(ctx, url, id, secret, &message_id("verification"), body.as_bytes()).await?;
    if !(200..300).contains(&status) {
        return Err(category(Some(status), None));
    }
    let echoed = serde_json::from_slice::<Value>(&answer).ok().and_then(|v| v.get("challenge").and_then(Value::as_str).map(str::to_owned));
    let same = echoed.is_some_and(|e| subtle::ConstantTimeEq::ct_eq(e.as_bytes(), nonce.as_bytes()).into());
    if same { Ok(()) } else { Err("challenge_failed") }
}

/// What a delivery attempt series ended with.
#[derive(Debug, Clone, Copy)]
enum Sent {
    Delivered,
    Abandoned(&'static str),
    /// the subscription is gone (unsubscribed, expired)
    Gone,
}

/// One message to the subscription's endpoint, retried with a fresh timestamp and signature each time.
async fn send(ctx: &Ctx, hooks: &Hooks, id: &str, body: &[u8], kind: &str) -> Sent {
    let msg = message_id(kind);
    let mut last = "connection_refused";
    for wait in std::iter::once(0).chain(RETRY_MS) {
        if wait > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
        }
        let Some((url, secret)) = lock(&hooks.subs).get(id).map(|s| (s.url.clone(), s.secret.clone())) else { return Sent::Gone };
        match post(ctx, &url, id, &secret, &msg, body).await {
            Ok((s, _)) if (200..300).contains(&s) => return Sent::Delivered,
            // the receiver doesn't want it: never tried again (SEP-3415)
            Ok((410 | 413, _)) => return Sent::Abandoned("http_4xx"),
            Ok((s, _)) => last = category(Some(s), None),
            Err(c) => last = c,
        }
        if let Some(s) = lock(&hooks.subs).get_mut(id) {
            s.last_error = Some(last);
        }
    }
    Sent::Abandoned(last)
}

/// How a batch went: carry on (and whether more is waiting), or the subscription is over.
enum Batch {
    Done { more: bool },
    Over,
}

/// One event's outcome, kept on its subscription: the watermark moves past it, delivered or abandoned.
fn record(hooks: &Hooks, id: &str, seq: String, out: Sent, now: i64) -> bool {
    let mut subs = lock(&hooks.subs);
    let Some(s) = subs.get_mut(id) else { return false };
    s.cursor = seq;
    match out {
        Sent::Delivered => {
            s.last_delivery_at = Some(now);
            s.abandoned_in_a_row = 0;
            s.failed_since = None;
        }
        Sent::Abandoned(why) => {
            s.last_error = Some(why);
            s.failed_since.get_or_insert(now);
            s.abandoned_in_a_row += 1;
            if s.abandoned_in_a_row >= SUSPEND_AFTER {
                s.active = false;
            }
        }
        Sent::Gone => return false,
    }
    true
}

/// What's new past the subscription's cursor, each event sent in turn.
async fn batch(ctx: &Ctx, call: &Call, hooks: &Hooks, id: &str, sub: &crate::webhook::Sub) -> Batch {
    let args = api::PollArgs { cursor: Some(sub.cursor.clone()), max_events: Some(100), ..sub.args.clone() };
    let p = match crate::mcp::poll(ctx, call, args).await {
        Ok(p) => p,
        Err(e) if matches!(refusal(&e).code, FORBIDDEN | NOT_FOUND) => {
            // access gone (or the type): the subscription ends, said to the endpoint
            let body = json!({ "type": "terminated", "error": { "code": refusal(&e).code, "message": e.message } }).to_string();
            let _ = send(ctx, hooks, id, body.as_bytes(), "terminated").await;
            lock(&hooks.subs).remove(id);
            return Batch::Over;
        }
        Err(e) => {
            tracing::warn!(subscription = %id, "a webhook's read failed: {}", e.message);
            return Batch::Done { more: false };
        }
    };
    if p.get("truncated") == Some(&Value::Bool(true)) {
        let body = json!({ "type": "gap", "cursor": p.get("cursor").cloned().unwrap_or(Value::Null) }).to_string();
        if matches!(send(ctx, hooks, id, body.as_bytes(), "gap").await, Sent::Gone) {
            return Batch::Over;
        }
    }
    for e in p.get("events").and_then(Value::as_array).cloned().unwrap_or_default() {
        let seq = e.get("eventId").and_then(Value::as_str).unwrap_or_default().to_owned();
        let mut body = e.clone();
        body["cursor"] = json!(seq);
        let out = send(ctx, hooks, id, body.to_string().as_bytes(), "").await;
        if !record(hooks, id, seq, out, ctx.clock().map_or(0, |c| c.now_ms())) {
            return Batch::Over;
        }
    }
    if let (Some(c), Some(s)) = (p.get("cursor").and_then(Value::as_str), lock(&hooks.subs).get_mut(id)) {
        c.clone_into(&mut s.cursor);
    }
    Batch::Done { more: p.get("hasMore") == Some(&Value::Bool(true)) }
}

/// A subscription's deliveries, in order, until it expires or is unsubscribed.
pub(crate) async fn deliver(ctx: Ctx, call: Call, hooks: Arc<Hooks>, id: String, mut woken: tokio::sync::watch::Receiver<u64>) {
    loop {
        let now = ctx.clock().map_or(0, |c| c.now_ms());
        let Some(sub) = lock(&hooks.subs).get(&id).cloned() else { return };
        if sub.expires_at <= now {
            lock(&hooks.subs).remove(&id);
            return;
        }
        if sub.active {
            match batch(&ctx, &call, &hooks, &id, &sub).await {
                Batch::Over => return,
                Batch::Done { more: true } => continue,
                Batch::Done { more: false } => {}
            }
        }
        let until = u64::try_from((sub.expires_at - now).max(0)).unwrap_or(0);
        tokio::select! {
            changed = woken.changed() => if changed.is_err() { return },
            () = tokio::time::sleep(std::time::Duration::from_millis(until.min(60_000))) => {}
        }
    }
}
