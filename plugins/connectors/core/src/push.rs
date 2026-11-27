//! Push (`events/stream`, SEP-3415; ADR-0146) as an `mcp.event` trigger reads it: whether a connector offers it and
//! the trigger wants it, the stream's params, each message off the stream as a [`Frame`], and the backoff before a
//! stream is opened again. Pure: the host holds the stream.

use serde_json::{Value, json};

use crate::events::{Occurrence, Subscription, page, refusal};

/// Whether the connector's listing offers `event` by `mode` (`push`, `poll`). A listing that doesn't name the event
/// offers nothing; one that names no modes is read as poll (SEP-3415 asks for a non-empty subset).
pub fn offers(listing: &[Value], event: &str, mode: &str) -> bool {
    listing.iter().filter(|d| d.get("name").and_then(Value::as_str) == Some(event)).any(|d| {
        match d.get("delivery").and_then(Value::as_array) {
            Some(modes) if !modes.is_empty() => modes.iter().any(|m| m.as_str() == Some(mode)),
            _ => mode == "poll",
        }
    })
}

/// Whether a trigger takes `mode` (`webhook`, `push`) when it's offered: `match.delivery` names the one mode it wants
/// (`poll` keeps it on polling); without it, each is taken in turn, webhook first (SEP-3415's SDK guidance).
pub fn wants(spec: &Value, mode: &str) -> bool {
    spec.pointer("/match/delivery").and_then(Value::as_str).is_none_or(|d| d == mode)
}

pub fn wants_push(spec: &Value) -> bool {
    wants(spec, "push")
}

/// The heartbeat wisp asks for: under the 60 s idle limit of common proxies.
pub const HEARTBEAT_MS: u64 = 25_000;

/// `events/stream`'s params.
pub fn stream_params(s: &Subscription, cursor: &Value) -> Value {
    json!({ "name": s.event, "arguments": s.arguments, "cursor": cursor, "heartbeatMs": HEARTBEAT_MS })
}

/// One message off a push stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// the subscription is on: where it starts, the heartbeat it was granted, and whether events were skipped
    Active {
        cursor: Value,
        heartbeat_ms: Option<u64>,
        truncated: bool,
    },
    /// one event, and the cursor past it
    Event {
        occurrence: Occurrence,
        cursor: Value,
    },
    Heartbeat {
        cursor: Value,
    },
    /// recoverable: the stream stays open
    Error(String),
    /// the server ended the subscription (access revoked, the type removed): don't open it again soon
    Terminated(String),
    /// the server closed the stream (its final response): open it again with the cursor
    Closed,
    /// another message (not this extension's)
    Other,
}

/// A stream message (an SSE frame's `data`) as a [`Frame`].
pub fn frame(s: &Subscription, msg: &Value) -> Frame {
    let p = msg.get("params").cloned().unwrap_or(Value::Null);
    let cursor = p.get("cursor").cloned().unwrap_or(Value::Null);
    let says = |p: &Value| p.pointer("/error/message").and_then(Value::as_str).unwrap_or("it failed").to_owned();
    match msg.get("method").and_then(Value::as_str) {
        Some("notifications/events/active") => Frame::Active {
            cursor,
            heartbeat_ms: p.get("heartbeatMs").and_then(Value::as_u64),
            truncated: p.get("truncated") == Some(&Value::Bool(true)),
        },
        Some("notifications/events/event") => match page(s, &json!({ "events": [p] })).and_then(|pg| pg.events.into_iter().next()) {
            Some(occurrence) => Frame::Event { occurrence, cursor },
            None => Frame::Other,
        },
        Some("notifications/events/heartbeat") => Frame::Heartbeat { cursor },
        Some("notifications/events/error") => Frame::Error(says(&p)),
        Some("notifications/events/terminated") => Frame::Terminated(says(&p)),
        None if msg.get("result").is_some() || msg.get("error").is_some() => {
            if let Some(e) = msg.get("error") {
                return Frame::Terminated(refusal(&s.connector, &s.event, e));
            }
            Frame::Closed
        }
        _ => Frame::Other,
    }
}

/// How long to wait before opening a stream again after `failures` in a row: 1 s doubling to 30 s, plus a jitter of
/// up to half of it (`per_mille` of that half, 0 to 1000) so a server's restart doesn't bring every client back at once.
pub fn backoff_ms(failures: u32, per_mille: u64) -> u64 {
    let base = 1_000u64.saturating_mul(1u64 << failures.min(5)).min(30_000);
    base + base * per_mille.min(1000) / 2000
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::events::{sub_key, subscription};

    fn sub() -> Subscription {
        subscription(
            &json!({ "on": "mcp.event", "match": { "connector": "pd", "event": "incident.created", "arguments": { "severity": "P1" } } }),
        )
        .unwrap()
    }

    #[test]
    fn push_is_taken_only_where_offered_and_wanted() {
        let listing = vec![
            json!({ "name": "incident.created", "delivery": ["webhook", "push", "poll"] }),
            json!({ "name": "email.received", "delivery": ["poll"] }),
            json!({ "name": "old.kind" }),
        ];
        assert!(offers(&listing, "incident.created", "push"));
        assert!(!offers(&listing, "email.received", "push"));
        assert!(offers(&listing, "old.kind", "poll") && !offers(&listing, "old.kind", "push"));
        assert!(!offers(&listing, "nope", "poll"));
        assert!(wants_push(&json!({ "match": { "connector": "pd" } })));
        assert!(!wants_push(&json!({ "match": { "delivery": "poll" } })));
        assert!(!wants_push(&json!({ "match": { "delivery": "webhook" } })));
        assert!(wants(&json!({ "match": {} }), "webhook") && wants(&json!({ "match": { "delivery": "webhook" } }), "webhook"));
        // the engine matches every `match` key on the payload: `delivery` is echoed, or the trigger would never fire
        let s = subscription(&json!({ "match": { "connector": "pd", "event": "x", "delivery": "poll" } })).unwrap();
        let p = page(&s, &json!({ "events": [{ "eventId": "1", "name": "x", "data": {} }] })).unwrap();
        assert_eq!(p.events[0].payload["delivery"], "poll");
        assert!(page(&sub(), &json!({ "events": [{ "eventId": "1", "data": {} }] })).unwrap().events[0].payload.get("delivery").is_none());
        assert_ne!(sub_key(&sub()), sub_key(&Subscription { arguments: json!({}), ..sub() }));
    }

    #[test]
    fn stream_frames_read_as_what_they_say() {
        let s = sub();
        let f = |m: Value| frame(&s, &m);
        assert_eq!(
            f(json!({ "method": "notifications/events/active", "params": { "cursor": "c1", "heartbeatMs": 15000, "truncated": true } })),
            Frame::Active { cursor: json!("c1"), heartbeat_ms: Some(15000), truncated: true }
        );
        let ev = f(
            json!({ "method": "notifications/events/event", "params": { "eventId": "e9", "name": "incident.created", "timestamp": "2026-02-19T15:30:00Z", "data": { "title": "Down" }, "cursor": "c2" } }),
        );
        let Frame::Event { occurrence, cursor } = ev else { panic!("{ev:?}") };
        assert_eq!((occurrence.key.as_str(), cursor), ("pd:incident.created:e9", json!("c2")));
        assert_eq!(
            f(json!({ "method": "notifications/events/heartbeat", "params": { "cursor": "c3" } })),
            Frame::Heartbeat { cursor: json!("c3") }
        );
        assert_eq!(
            f(json!({ "method": "notifications/events/error", "params": { "error": { "message": "Gmail 503" } } })),
            Frame::Error("Gmail 503".into())
        );
        assert_eq!(
            f(json!({ "method": "notifications/events/terminated", "params": { "error": { "message": "Forbidden" } } })),
            Frame::Terminated("Forbidden".into())
        );
        assert_eq!(f(json!({ "id": 2, "result": { "resultType": "complete" } })), Frame::Closed);
        assert!(matches!(f(json!({ "id": 2, "error": { "code": -32026, "message": "push" } })), Frame::Terminated(_)));
        assert_eq!(f(json!({ "method": "notifications/progress" })), Frame::Other);
    }

    #[test]
    fn backoff_doubles_to_thirty_seconds_with_jitter() {
        assert_eq!(backoff_ms(0, 0), 1_000);
        assert_eq!(backoff_ms(3, 0), 8_000);
        assert_eq!(backoff_ms(9, 0), 30_000);
        assert_eq!(backoff_ms(0, 1000), 1_500);
        assert_eq!(backoff_ms(0, 5000), 1_500);
    }
}
