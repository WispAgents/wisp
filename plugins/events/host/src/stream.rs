//! MCP Events' push delivery (SEP-3415 `events/stream`; ADR-0145): one long-lived request a subscription, answered
//! with SSE by wisp.mcp. Opened with a poll's checks, then `active`, the backlog and each new event as the feed writes
//! it, and a heartbeat with the cursor in quiet stretches. A caller holds at most `STREAMS_PER_CALLER` of them.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_events_api as api;
use wisp_kernel::{Call, Ctx};
use wisp_mcp_api::MethodError;

use crate::mcp::{INTERNAL, MAX, poll, poll_args, refusal};

/// The heartbeat a stream gets: the client's suggestion within 5 s to 60 s, 30 s without one (SEP-3415's default;
/// under the 60 s idle timeout proxies commonly have).
pub fn heartbeat_ms(params: &Value) -> u64 {
    params.get("heartbeatMs").and_then(Value::as_u64).map_or(DEFAULT_HEARTBEAT_MS, |h| h.clamp(5_000, 60_000))
}
pub const DEFAULT_HEARTBEAT_MS: u64 = 30_000;

/// A `notifications/events/*` message of the stream whose request id is `sub`.
pub fn note(kind: &str, mut params: Value, sub: &Value) -> Value {
    if let Some(o) = params.as_object_mut() {
        let meta = o.entry("_meta").or_insert_with(|| json!({}));
        meta["io.modelcontextprotocol/subscriptionId"] = sub.clone();
    }
    json!({ "jsonrpc": "2.0", "method": format!("notifications/events/{kind}"), "params": params })
}

/// A poll's events as stream notifications, each with the cursor just past it (its seq).
fn events_of(poll: &Value, sub: &Value) -> Vec<Value> {
    poll.get("events")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|e| {
            let mut e = e.clone();
            e["cursor"] = e.get("eventId").cloned().unwrap_or(Value::Null);
            note("event", e, sub)
        })
        .collect()
}

/// Open streams a caller may hold, and in all (SEP-3415 `ResourceExhausted` past them).
pub const STREAMS_PER_CALLER: usize = 16;
pub const STREAMS: usize = 256;

/// The open streams, by caller.
#[derive(Default)]
pub struct Open(std::sync::Mutex<std::collections::HashMap<String, usize>>);

/// One open stream, counted until it ends.
pub struct Held(Arc<Open>, String);

impl Drop for Held {
    fn drop(&mut self) {
        let mut m = self.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(n) = m.get_mut(&self.1) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                m.remove(&self.1);
            }
        }
    }
}

impl Open {
    /// A place for one more of `who`'s streams, or the limit it hit.
    pub fn hold(self: &Arc<Self>, who: &str) -> std::result::Result<Held, MethodError> {
        let mut m = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (mine, all) = (m.get(who).copied().unwrap_or(0), m.values().sum::<usize>());
        let full = |max: usize| {
            MethodError::new(crate::webhook::RESOURCE_EXHAUSTED, format!("at most {max} open event streams"))
                .with_data(json!({ "limit": "streams", "max": max }))
        };
        if mine >= STREAMS_PER_CALLER {
            return Err(full(STREAMS_PER_CALLER));
        }
        if all >= STREAMS {
            return Err(full(STREAMS));
        }
        *m.entry(who.to_owned()).or_default() += 1;
        Ok(Held(self.clone(), who.to_owned()))
    }
}

/// `events/stream`: opened with `events.poll` run as the caller (so the type, the cursor, the capability and the
/// task scope are checked once, before any stream), then `active`, the backlog, and every new event as the feed
/// writes it (`newest_seq` wakes it: no stream polls the log). A heartbeat carries the cursor whenever `heartbeatMs`
/// passes without a message. The client gone ends it at the next send.
pub async fn open(
    ctx: Ctx,
    call: Call,
    req: wisp_mcp_api::MethodRequest,
    newest_seq: &tokio::sync::watch::Sender<u64>,
    open: &Arc<Open>,
) -> std::result::Result<wisp_mcp_api::MethodAnswer, MethodError> {
    let hb = heartbeat_ms(&req.params);
    let mut a = poll_args(req.params, "events/stream")?;
    a.max_events = Some(MAX);
    let first = ctx.run::<api::Poll>(&call, a.clone()).await.map_err(|e| refusal(&e))?;
    let held = open.hold(&crate::webhook::principal(&call))?;
    let (tx, rx) = tokio::sync::mpsc::channel::<Value>(64);
    let s = Stream { sub: req.id, hb, args: a, tx, woken: newest_seq.subscribe(), _held: held };
    ctx.clone().spawn(async move { s.run(&ctx, &call, &first).await });
    Ok(wisp_mcp_api::MethodAnswer::Stream(rx))
}

/// One open stream.
struct Stream {
    sub: Value,
    hb: u64,
    args: api::PollArgs,
    tx: tokio::sync::mpsc::Sender<Value>,
    woken: tokio::sync::watch::Receiver<u64>,
    _held: Held,
}

impl Stream {
    async fn run(mut self, ctx: &Ctx, call: &Call, first: &Value) {
        let sub = self.sub.clone();
        let gap = first.get("truncated").cloned().unwrap_or(json!(false));
        let mut cursor = first.get("cursor").cloned().unwrap_or(Value::Null);
        let active = note("active", json!({ "cursor": cursor, "truncated": gap, "heartbeatMs": self.hb }), &sub);
        let mut out: Vec<Value> = std::iter::once(active).chain(events_of(first, &sub)).collect();
        let mut more = first.get("hasMore") == Some(&Value::Bool(true));
        let quiet = std::time::Duration::from_millis(self.hb);
        // the heartbeat counts from the last message sent, not the last wake: other types' events wake the stream too
        let mut sent = tokio::time::Instant::now();
        loop {
            for m in out.drain(..) {
                if self.tx.send(m).await.is_err() {
                    return;
                }
                sent = tokio::time::Instant::now();
            }
            if !more {
                tokio::select! {
                    changed = self.woken.changed() => if changed.is_err() { return },
                    () = tokio::time::sleep_until(sent + quiet) => {
                        out.push(note("heartbeat", json!({ "cursor": cursor }), &sub));
                        continue;
                    }
                }
            }
            self.args.cursor = cursor.as_str().map(str::to_owned);
            more = self.read(ctx, call, &mut cursor, &mut out).await;
        }
    }

    /// What's new past `cursor`, onto `out`; whether more is waiting. A failed read is recoverable: said, and tried
    /// again at the next wake.
    async fn read(&self, ctx: &Ctx, call: &Call, cursor: &mut Value, out: &mut Vec<Value>) -> bool {
        match poll(ctx, call, self.args.clone()).await {
            Ok(p) => {
                out.extend(events_of(&p, &self.sub));
                *cursor = p.get("cursor").cloned().unwrap_or_else(|| cursor.clone());
                if p.get("truncated") == Some(&Value::Bool(true)) {
                    out.push(note("active", json!({ "cursor": cursor, "truncated": true, "heartbeatMs": self.hb }), &self.sub));
                }
                p.get("hasMore") == Some(&Value::Bool(true))
            }
            Err(e) => {
                out.push(note("error", json!({ "error": { "code": INTERNAL, "message": e.message } }), &self.sub));
                false
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_caller_holds_at_most_its_share_of_streams_and_each_frees_its_place() {
        let open = Arc::new(Open::default());
        let mine: Vec<Held> = (0..STREAMS_PER_CALLER).map(|_| open.hold("a").unwrap()).collect();
        let e = open.hold("a").err().unwrap();
        assert_eq!((e.code, e.data.clone().unwrap()["max"].clone()), (crate::webhook::RESOURCE_EXHAUSTED, json!(STREAMS_PER_CALLER)));
        assert!(open.hold("b").is_ok(), "another caller isn't held up");
        drop(mine);
        assert!(open.hold("a").is_ok(), "closed streams free their places");
    }

    #[test]
    fn stream_notes_carry_the_subscription_id_and_events_their_cursor() {
        let poll = json!({ "events": [{ "eventId": "41", "name": "task.finished", "data": {} }] });
        let ev = events_of(&poll, &json!(7));
        assert_eq!(ev[0]["method"], "notifications/events/event");
        assert_eq!(ev[0]["params"]["cursor"], "41");
        assert_eq!(ev[0]["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"], 7);
        let hb = note("heartbeat", json!({ "cursor": "41" }), &json!("s"));
        assert_eq!(hb["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"], "s");
        assert_eq!(heartbeat_ms(&json!({})), DEFAULT_HEARTBEAT_MS);
        assert_eq!(heartbeat_ms(&json!({ "heartbeatMs": 10 })), 5_000);
        assert_eq!(heartbeat_ms(&json!({ "heartbeatMs": 600_000 })), 60_000);
    }
}
