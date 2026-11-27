//! A connector's MCP Events by webhook (SEP-3415 `events/subscribe`; ADR-0145), as wisp receives them: the subscribe
//! params, checking a delivery's Standard Webhooks signature and freshness, and what its body is. Pure: the host
//! makes the secret, holds the route and calls the server.

use base64::Engine as _;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::events::{Occurrence, Subscription, page};

/// The TTL wisp asks for; it refreshes at 80 % of what it's granted.
pub const TTL_MS: i64 = 60 * 60_000;
/// A delivery older (or newer) than this is refused (SEP-3415: 5 minutes).
pub const FRESH_S: i64 = 300;

/// A Standard Webhooks secret from random bytes: `whsec_` + base64.
pub fn secret(bytes: &[u8]) -> String {
    format!("whsec_{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// `events/subscribe`'s params.
pub fn subscribe_params(s: &Subscription, url: &str, secret: &str, cursor: &Value) -> Value {
    json!({ "name": s.event, "arguments": s.arguments, "delivery": { "mode": "webhook", "url": url, "secret": secret }, "cursor": cursor, "ttlMs": TTL_MS })
}

/// `events/unsubscribe`'s params.
pub fn unsubscribe_params(s: &Subscription, url: &str) -> Value {
    json!({ "name": s.event, "arguments": s.arguments, "delivery": { "url": url } })
}

/// When to refresh, from the grant: at 80 % of the way to `refreshBefore` (an hour when it can't be read, so a
/// grant of no expiry is still refreshed now and then, as SEP-3415 asks).
pub fn refresh_at(now: i64, refresh_before: Option<&str>) -> i64 {
    let until = refresh_before.and_then(crate::events::iso_ms).unwrap_or(now + TTL_MS);
    now + (until - now).max(0) * 4 / 5
}

/// Whether a delivery is the server's: its `webhook-signature` (any of several, space-separated, `v1,`) is the HMAC
/// of `id.timestamp.body` under `secret`, and its timestamp is within five minutes of `now_s`.
pub fn verified(secret: &str, id: &str, timestamp: &str, signatures: &str, body: &[u8], now_s: i64) -> bool {
    let Ok(ts) = timestamp.trim().parse::<i64>() else { return false };
    if (now_s - ts).abs() > FRESH_S {
        return false;
    }
    let Some(key) = secret.strip_prefix("whsec_").and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok()) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(&key) else { return false };
    mac.update(format!("{id}.{ts}.").as_bytes());
    mac.update(body);
    let want = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    signatures
        .split_whitespace()
        .filter_map(|s| s.strip_prefix("v1,"))
        .any(|got| subtle::ConstantTimeEq::ct_eq(got.as_bytes(), want.as_bytes()).into())
}

/// What a delivery's body is.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    /// echo the challenge in the answer
    Verification(String),
    /// one event, and the watermark past it
    Event {
        occurrence: Occurrence,
        cursor: Value,
    },
    /// events were skipped: keep this cursor
    Gap(Value),
    /// the server ended the subscription
    Terminated(String),
    Other,
}

pub fn body(s: &Subscription, v: &Value) -> Body {
    match v.get("type").and_then(Value::as_str) {
        Some("verification") => Body::Verification(v.get("challenge").and_then(Value::as_str).unwrap_or_default().to_owned()),
        Some("gap") => Body::Gap(v.get("cursor").cloned().unwrap_or(Value::Null)),
        Some("terminated") => {
            Body::Terminated(v.pointer("/error/message").and_then(Value::as_str).unwrap_or("the server ended it").to_owned())
        }
        Some(_) => Body::Other,
        None => match page(s, &json!({ "events": [v] })).and_then(|p| p.events.into_iter().next()) {
            Some(occurrence) => Body::Event { occurrence, cursor: v.get("cursor").cloned().unwrap_or(Value::Null) },
            None => Body::Other,
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::events::subscription;

    fn sub() -> Subscription {
        subscription(&json!({ "match": { "connector": "pd", "event": "incident.created" } })).unwrap()
    }

    #[test]
    fn a_standard_webhooks_signature_verifies_and_a_stale_or_forged_one_does_not() {
        // Standard Webhooks' published test vector
        let (secret, id, ts, body) =
            ("whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw", "msg_p5jXN8AQM9LWM0D4loKWxJek", "1614265330", br#"{"test": 2432232314}"#);
        let sig = "v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=";
        assert!(verified(secret, id, ts, sig, body, 1_614_265_330));
        assert!(verified(secret, id, ts, &format!("v1,bm9wZQ== {sig}"), body, 1_614_265_400), "one of several, during a rotation");
        assert!(!verified(secret, id, ts, sig, body, 1_614_265_330 + 301), "over five minutes old");
        assert!(!verified(secret, id, ts, sig, br#"{"test": 1}"#, 1_614_265_330), "another body");
        assert!(!verified("whsec_bm9wZQ==", id, ts, sig, body, 1_614_265_330), "another secret");
        assert!(!verified(secret, id, "soon", sig, body, 1_614_265_330));
    }

    #[test]
    fn bodies_read_as_what_they_are() {
        assert_eq!(body(&sub(), &json!({ "type": "verification", "challenge": "n1" })), Body::Verification("n1".into()));
        assert_eq!(body(&sub(), &json!({ "type": "gap", "cursor": "c9" })), Body::Gap(json!("c9")));
        assert!(
            matches!(body(&sub(), &json!({ "type": "terminated", "error": { "message": "Forbidden" } })), Body::Terminated(m) if m == "Forbidden")
        );
        let Body::Event { occurrence, cursor } =
            body(&sub(), &json!({ "eventId": "e1", "name": "incident.created", "data": { "title": "Down" }, "cursor": "c1" }))
        else {
            panic!()
        };
        assert_eq!((occurrence.key.as_str(), cursor), ("pd:incident.created:e1", json!("c1")));
        assert_eq!(body(&sub(), &json!({ "type": "future" })), Body::Other);
    }

    #[test]
    fn refresh_comes_at_four_fifths_of_the_grant() {
        assert_eq!(refresh_at(0, Some("1970-01-01T01:00:00Z")), 2_880_000);
        assert_eq!(refresh_at(0, None), TTL_MS * 4 / 5);
        assert_eq!(subscribe_params(&sub(), "https://h/x", "whsec_a", &Value::Null)["delivery"]["mode"], "webhook");
        assert!(secret(&[1u8; 32]).starts_with("whsec_"));
    }
}
