//! MCP Events (SEP-3415, `io.modelcontextprotocol/events`; ADR-0146) as an `mcp.event` trigger reads them: the
//! `events/poll` params from a trigger's `match` (`connector`, `event`, `arguments`), the cursor wisp keeps between
//! polls, and each `EventOccurrence` as a trigger event. Pure: the host does the call.
//!
//! A trigger:
//!
//! ```yaml
//! on: mcp.event
//! match: { connector: pagerduty, event: incident.created, arguments: { severity: P1 } }
//! prompt: "Triage this: {event.summary}"
//! ```
//!
//! The payload echoes `connector`, `event` and `arguments`, so the engine's `match` filter passes them, and any other
//! key (`data.service`) filters on the event's data. An event's data is the server's, so untrusted: it reaches a
//! prompt only through the placeholders the trigger's author wrote.

use serde_json::{Map, Value, json};

pub const EXTENSION: &str = "io.modelcontextprotocol/events";
pub const ON: &str = "mcp.event";
/// The modern protocol, for a server that refuses `initialize`.
pub const MODERN: &str = "2026-07-28";
/// How many pages a poll drains at once when the server says `hasMore`.
pub const MAX_PAGES: usize = 10;

/// What a trigger subscribes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub connector: String,
    pub event: String,
    pub arguments: Value,
    /// `match.delivery` (`poll` keeps it on polling), echoed in each payload so the engine's `match` passes it
    pub delivery: Option<String>,
}

/// The subscription in a trigger's spec (`match.connector`, `match.event`, `match.arguments`).
pub fn subscription(spec: &Value) -> Result<Subscription, String> {
    let m = spec.get("match").cloned().unwrap_or(Value::Null);
    let text = |k: &str| m.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    let (Some(connector), Some(event)) = (text("connector"), text("event")) else {
        return Err("mcp.event needs match.connector (a remote connector) and match.event (one of its event types)".into());
    };
    let arguments = match m.get("arguments") {
        None | Some(Value::Null) => json!({}),
        Some(Value::Object(o)) => Value::Object(o.clone()),
        Some(_) => return Err("mcp.event: match.arguments is an object (the event type's inputSchema)".into()),
    };
    Ok(Subscription { connector, event, arguments, delivery: text("delivery") })
}

/// The `_meta` every events request carries: the client says it speaks the extension; a modern request names its
/// version and who it is too.
pub fn meta(modern: Option<&str>) -> Value {
    let mut m = json!({ "io.modelcontextprotocol/clientCapabilities": { "extensions": { EXTENSION: {} } } });
    if let Some(version) = modern {
        m["io.modelcontextprotocol/protocolVersion"] = json!(MODERN);
        m["io.modelcontextprotocol/clientInfo"] = json!({ "name": "wisp", "version": version });
    }
    m
}

/// What wisp keeps between polls: `{"c": <the server's cursor>}`. Nothing kept yet (`null`): the first poll, which
/// starts from now. A server that keeps no replay answers `c: null`, and every poll then sends null.
pub fn server_cursor(kept: &Value) -> Value {
    kept.get("c").cloned().unwrap_or(Value::Null)
}

pub fn kept(server: &Value) -> Value {
    json!({ "c": server })
}

/// `events/poll`'s params.
pub fn poll_params(s: &Subscription, kept: &Value) -> Value {
    json!({ "name": s.event, "arguments": s.arguments, "cursor": server_cursor(kept), "maxEvents": 50 })
}

/// A poll's answer, read.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub events: Vec<Occurrence>,
    pub cursor: Value,
    pub has_more: bool,
    pub truncated: bool,
}

/// One event, as the trigger engine gets it.
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    /// `<connector>:<event>:<eventId>`: the engine fires once per key
    pub key: String,
    pub at: Option<i64>,
    pub summary: String,
    pub payload: Value,
}

/// A poll result into what the engine reads; `None` when it isn't one.
pub fn page(s: &Subscription, result: &Value) -> Option<Page> {
    let list = result.get("events")?.as_array()?;
    let events = list
        .iter()
        .filter_map(|e| {
            let id = match e.get("eventId")? {
                Value::String(s) if !s.is_empty() => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            let name = e.get("name").and_then(Value::as_str).unwrap_or(&s.event);
            let data = e.get("data").cloned().unwrap_or_else(|| Value::Object(Map::new()));
            let payload =
                json!({ "connector": s.connector, "event": s.event, "arguments": s.arguments, "eventId": id, "name": name, "data": data });
            let mut payload = payload;
            if let Some(d) = &s.delivery {
                payload["delivery"] = json!(d);
            }
            Some(Occurrence {
                key: format!("{}:{}:{id}", s.connector, s.event),
                at: e.get("timestamp").and_then(Value::as_str).and_then(iso_ms),
                summary: summary(&s.connector, name, &data),
                payload,
            })
        })
        .collect();
    Some(Page {
        events,
        cursor: result.get("cursor").cloned().unwrap_or(Value::Null),
        has_more: result.get("hasMore") == Some(&Value::Bool(true)),
        truncated: result.get("truncated") == Some(&Value::Bool(true)),
    })
}

/// `pagerduty: incident.created: Database down`: the event, and its data's title if it has one (one line, short).
fn summary(connector: &str, name: &str, data: &Value) -> String {
    let title = ["title", "subject", "summary", "name"].iter().find_map(|k| data.get(*k).and_then(Value::as_str)).unwrap_or_default();
    let title: String = title.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect();
    if title.is_empty() { format!("{connector}: {name}") } else { format!("{connector}: {name}: {title}") }
}

/// Why a JSON-RPC error refused an events request, in words (SEP-3415's codes, and a server with no events).
pub fn refusal(connector: &str, event: &str, error: &Value) -> String {
    let message = error.get("message").and_then(Value::as_str).unwrap_or("it refused");
    match error.get("code").and_then(Value::as_i64) {
        Some(-32601) => format!("{connector} serves no MCP events"),
        Some(-32023) => format!("{connector} has no event {event} ({message})"),
        Some(-32024) => format!("{connector} doesn't let this sign-in watch {event} ({message})"),
        Some(-32602) => format!("{connector} refused the arguments for {event} ({message})"),
        Some(-32026) => format!("{connector} doesn't offer {event} by poll ({message})"),
        _ => format!("{connector}: {message}"),
    }
}

/// The subscription as one key: its connector, event and arguments (a trigger's stream, shared by every trigger that
/// names the same three).
pub fn sub_key(s: &Subscription) -> String {
    format!("{}\u{1f}{}\u{1f}{}", s.connector, s.event, s.arguments)
}

/// An RFC 3339 time (`2026-02-19T15:30:00Z`, `…+02:00`, with or without fractions) in ms.
pub fn iso_ms(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') || !matches!(bytes.get(10), Some(b'T' | b't' | b' '))
    {
        return None;
    }
    let num = |from: usize, to: usize| text.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut at = 19;
    let mut millis = 0;
    if bytes.get(at) == Some(&b'.') {
        let start = at + 1;
        at = start;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        let three: String = text.get(start..at)?.chars().chain("000".chars()).take(3).collect();
        millis = three.parse::<i64>().ok()?;
    }
    let offset_min = match bytes.get(at)? {
        b'Z' | b'z' => 0,
        sign @ (b'+' | b'-') => {
            let total = num(at + 1, at + 3)? * 60 + num(at + 4, at + 6)?;
            if *sign == b'+' { total } else { -total }
        }
        _ => return None,
    };
    // Howard Hinnant's days_from_civil
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let yoe = shifted - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400 + hour * 3600 + minute * 60 + second) - offset_min * 60) * 1000 + millis)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sub() -> Subscription {
        subscription(
            &json!({ "on": "mcp.event", "match": { "connector": "pd", "event": "incident.created", "arguments": { "severity": "P1" } } }),
        )
        .unwrap()
    }

    #[test]
    fn a_trigger_names_its_connector_and_event_in_match() {
        let s = sub();
        assert_eq!((s.connector.as_str(), s.event.as_str()), ("pd", "incident.created"));
        assert_eq!(s.arguments, json!({ "severity": "P1" }));
        assert!(subscription(&json!({ "match": { "connector": "pd" } })).is_err());
        assert!(subscription(&json!({ "match": { "connector": "pd", "event": "x", "arguments": [1] } })).is_err());
        assert_eq!(subscription(&json!({ "match": { "connector": "pd", "event": "x" } })).unwrap().arguments, json!({}));
    }

    #[test]
    fn the_first_poll_sends_no_cursor_and_later_ones_the_servers() {
        let s = sub();
        assert_eq!(poll_params(&s, &Value::Null)["cursor"], Value::Null);
        assert_eq!(poll_params(&s, &kept(&json!("c_9")))["cursor"], "c_9");
        // a server that keeps no replay: null is kept, and null is sent again
        assert_eq!(poll_params(&s, &kept(&Value::Null))["cursor"], Value::Null);
        assert_eq!(poll_params(&s, &Value::Null)["name"], "incident.created");
    }

    #[test]
    fn occurrences_become_events_keyed_per_subscription_and_echo_the_match() {
        let r = json!({ "events": [
            { "eventId": "evt_1", "name": "incident.created", "timestamp": "2026-02-19T15:30:00Z", "data": { "title": "DB  down\nnow", "service": "api" } },
            { "name": "incident.created", "data": {} },
            { "eventId": 7, "name": "incident.created", "timestamp": "nope", "data": {} }
        ], "cursor": "c_2", "hasMore": true, "truncated": false });
        let p = page(&sub(), &r).unwrap();
        assert_eq!(p.events.len(), 2, "an occurrence without an id can't be deduped: left out");
        assert_eq!(p.events[0].key, "pd:incident.created:evt_1");
        assert_eq!(p.events[0].at, Some(1_771_515_000_000));
        assert_eq!(p.events[0].summary, "pd: incident.created: DB down now");
        assert_eq!(p.events[0].payload["data"]["service"], "api");
        assert_eq!(p.events[0].payload["arguments"], json!({ "severity": "P1" }));
        assert_eq!(p.events[1].key, "pd:incident.created:7");
        assert_eq!(p.events[1].at, None);
        assert_eq!(p.events[1].summary, "pd: incident.created");
        assert!(p.has_more && !p.truncated);
        assert_eq!(p.cursor, "c_2");
        assert!(page(&sub(), &json!({ "nope": 1 })).is_none());
    }

    #[test]
    fn refusals_say_which_code_in_words() {
        assert_eq!(refusal("pd", "x", &json!({ "code": -32601, "message": "Method not found" })), "pd serves no MCP events");
        assert!(refusal("pd", "x", &json!({ "code": -32023, "message": "NotFound" })).contains("has no event x"));
    }

    #[test]
    fn iso_times_with_offsets_and_fractions() {
        assert_eq!(iso_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_ms("1970-01-01T01:00:00.5+01:00"), Some(500));
        assert_eq!(iso_ms("2026-11-27T19:16:51.123Z"), Some(1_795_807_011_123));
        assert_eq!(iso_ms("2026-13-01T00:00:00Z"), None);
        assert_eq!(iso_ms("yesterday"), None);
    }

    #[test]
    fn meta_declares_the_extension_and_a_modern_request_its_version() {
        assert!(meta(None)["io.modelcontextprotocol/clientCapabilities"]["extensions"][EXTENSION].is_object());
        assert!(meta(None).get("io.modelcontextprotocol/protocolVersion").is_none());
        assert_eq!(meta(Some("2.0.0"))["io.modelcontextprotocol/protocolVersion"], MODERN);
    }
}
