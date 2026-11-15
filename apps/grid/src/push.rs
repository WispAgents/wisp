//! The phone relay (ADR-0065 §3): wisp seals each notification to the phone's own key, and the grid carries it to
//! FCM or APNs. The grid holds the FCM and APNs credentials and a handle → token map; it sees only ciphertext, a
//! handle and a collapse key, and so do Google and Apple.
//!
//! - `POST /api/push/register {platform, token, handle?}` (a device token: the phone's own sign-in) → `{handle}`.
//!   One handle per device: the same device registering again (a refreshed FCM/APNs token) names its handle and
//!   keeps it; without one, its old handle goes. Only the handle's hash is kept.
//! - `POST /api/push/send {handle, sealed, collapse}` (no other key: a body with anything else is refused, so a
//!   wisp can't leak plaintext through here by mistake). The handle is the capability, as a webhook's path is: a
//!   wisp on any computer the phone paired with may send to it, and only ciphertext gets through. At most
//!   [`SEND_PER_MINUTE`] a minute per handle.
//! - Answers: 200 sent; 410 `gone` (FCM/APNs said the token is gone, or the handle was dropped before: wisp
//!   unpairs the phone); 503 `unavailable` (no credentials for that platform: wisp keeps ntfy); 429; 502 (the
//!   service refused it for another reason).

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

use crate::clock::SharedClock;
use crate::db::Db;
use crate::error::{Result, bad};
use crate::secrets::{sha256, token};

pub mod providers;

pub const HANDLE_PREFIX: &str = "wph_";
pub const SEND_PER_MINUTE: usize = 30;
/// What APNs shows if the phone's extension can't open the note in time (FCM data messages show nothing).
pub const FALLBACK: &str = "A task needs you";
/// The most a sealed note may be: FCM's data payload is 4 KB, APNs' 4 KB.
pub const SEALED_MAX: usize = 3000;
const TOKEN_MAX: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Android,
    Ios,
}

impl Platform {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "android" => Some(Self::Android),
            "ios" => Some(Self::Ios),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Android => "android",
            Self::Ios => "ios",
        }
    }
}

/// Where a note goes: the platform and its token (only the relay and FCM/APNs ever hold the token).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub platform: Platform,
    pub token: String,
}

/// All a note is, past wisp: ciphertext and the key that collapses a newer note for the same task onto it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub sealed: String,
    pub collapse: String,
}

/// What FCM or APNs said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivered {
    Sent,
    /// the token is gone (the app removed, its notifications reset): FCM 404 `UNREGISTERED`, APNs 410
    Gone,
    Failed(String),
}

/// FCM and APNs, as the relay uses them (`providers::HttpPush` for real, a fake in tests).
#[async_trait]
pub trait PushSender: Send + Sync {
    /// it has credentials for this platform
    fn serves(&self, p: Platform) -> bool;
    async fn send(&self, to: &Target, msg: &Outgoing, now_ms: i64) -> Delivered;
}

/// FCM HTTP v1's message: a data message only (the phone's messaging service opens it and posts the notification),
/// collapsed per task.
pub fn fcm_message(to: &Target, msg: &Outgoing) -> Value {
    json!({ "message": {
        "token": to.token,
        "data": { "sealed": msg.sealed, "collapse": msg.collapse },
        "android": { "priority": "high", "collapse_key": msg.collapse }
    } })
}

/// APNs' payload: a `mutable-content` alert whose visible text is the fixed fallback; the Notification Service
/// Extension replaces it with what it opens. Its `apns-collapse-id` header is `msg.collapse`.
pub fn apns_payload(msg: &Outgoing) -> Value {
    json!({
        "aps": { "alert": { "title": "wisp", "body": FALLBACK }, "mutable-content": 1, "sound": "default" },
        "sealed": msg.sealed,
        "collapse": msg.collapse
    })
}

fn handle_ok(h: &str) -> bool {
    h.starts_with(HANDLE_PREFIX) && (20..=128).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn collapse_ok(c: &str) -> bool {
    (1..=64).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'@'))
}

fn sealed_ok(s: &str) -> bool {
    (44..=SEALED_MAX).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn text<'a>(b: &'a Map<String, Value>, k: &str) -> &'a str {
    b.get(k).and_then(Value::as_str).unwrap_or_default()
}

pub struct Relay {
    pub db: Arc<Db>,
    pub clock: SharedClock,
    sender: RwLock<Option<Arc<dyn PushSender>>>,
    limits: crate::http::Limiter,
    per_minute: usize,
}

impl std::fmt::Debug for Relay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Relay").field("per_minute", &self.per_minute).finish_non_exhaustive()
    }
}

impl Relay {
    pub fn new(db: Arc<Db>, clock: SharedClock, sender: Option<Arc<dyn PushSender>>, per_minute: Option<usize>) -> Self {
        Self {
            db,
            clock,
            sender: RwLock::new(sender),
            limits: crate::http::Limiter::default(),
            per_minute: per_minute.unwrap_or(SEND_PER_MINUTE),
        }
    }

    /// FCM and APNs as they are now (tests put a fake in).
    pub fn set_sender(&self, s: Option<Arc<dyn PushSender>>) {
        *self.sender.write().unwrap_or_else(std::sync::PoisonError::into_inner) = s;
    }

    fn sender(&self) -> Option<Arc<dyn PushSender>> {
        self.sender.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn serves(&self, p: Platform) -> bool {
        self.sender().is_some_and(|s| s.serves(p))
    }

    /// `POST /api/push/register` for `device` of `user`. → `{handle}`.
    pub fn register(&self, user_id: &str, device_id: &str, b: &Map<String, Value>) -> Result<Value> {
        let Some(platform) = Platform::parse(text(b, "platform")) else { return Err(bad("platform: android or ios", 400)) };
        let tok = text(b, "token");
        if tok.is_empty() || tok.len() > TOKEN_MAX || tok.bytes().any(|c| !c.is_ascii_graphic()) {
            return Err(bad("token: the FCM or APNs token", 400));
        }
        if platform == Platform::Ios && !tok.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(bad("token: an APNs device token is hex", 400));
        }
        if !self.serves(platform) {
            return Err(bad("unavailable", 503));
        }
        let now = self.clock.now_ms();
        let kept = text(b, "handle");
        let handle = self.db.tx(|t| {
            if handle_ok(kept) {
                let n = t.execute(
                    "UPDATE push_handles SET token = ?1, platform = ?2 WHERE handle_hash = ?3 AND user_id = ?4 AND device_id = ?5",
                    params![tok, platform.as_str(), sha256(kept), user_id, device_id],
                )?;
                if n == 1 {
                    return Ok(kept.to_owned());
                }
            }
            // one handle per device: a new one replaces the old
            t.execute("DELETE FROM push_handles WHERE device_id = ?1", params![device_id])?;
            let h = token(HANDLE_PREFIX, 32);
            t.execute(
                "INSERT INTO push_handles (handle_hash, user_id, device_id, platform, token, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![sha256(&h), user_id, device_id, platform.as_str(), tok, now],
            )?;
            Ok(h)
        })?;
        Ok(json!({ "handle": handle }))
    }

    /// `POST /api/push/send`. → (status, body).
    pub async fn send(&self, b: &Map<String, Value>) -> (u16, Value) {
        let refuse = |s: u16, m: &str| (s, json!({ "error": m }));
        if b.keys().any(|k| !matches!(k.as_str(), "handle" | "sealed" | "collapse")) {
            return refuse(400, "A push carries handle, sealed and collapse only.");
        }
        let (handle, sealed, collapse) = (text(b, "handle"), text(b, "sealed"), text(b, "collapse"));
        if !handle_ok(handle) {
            return refuse(400, "handle: not a push handle");
        }
        if !sealed_ok(sealed) {
            return refuse(400, "sealed: base64url ciphertext, at most 3000 characters");
        }
        if !collapse_ok(collapse) {
            return refuse(400, "collapse: 1 to 64 letters, digits or -_.:@");
        }
        let hash = sha256(handle);
        let row = self.db.with(|c| {
            c.query_row("SELECT platform, token FROM push_handles WHERE handle_hash = ?1", params![hash], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .optional()
        });
        let target = match row {
            Ok(Some((p, token))) => match Platform::parse(&p) {
                Some(platform) => Target { platform, token },
                None => return refuse(410, "gone"),
            },
            Ok(None) => return refuse(410, "gone"),
            Err(e) => {
                tracing::warn!("grid: push: {e}");
                return refuse(500, "internal error");
            }
        };
        // only a known handle is counted: random ones never grow the limiter's map
        if self.limits.hit(&format!("push|{hash}"), self.per_minute, self.clock.now_ms()).is_err() {
            return refuse(429, "Too many notifications to this phone. Wait a minute.");
        }
        let Some(sender) = self.sender().filter(|s| s.serves(target.platform)) else { return refuse(503, "unavailable") };
        let msg = Outgoing { sealed: sealed.to_owned(), collapse: collapse.to_owned() };
        let now = self.clock.now_ms();
        match sender.send(&target, &msg, now).await {
            Delivered::Sent => {
                let _ = self.db.with(|c| c.execute("UPDATE push_handles SET last_sent_at = ?1 WHERE handle_hash = ?2", params![now, hash]));
                (200, json!({ "ok": true }))
            }
            Delivered::Gone => {
                // only the token that was sent: one the phone registered meanwhile (a refresh) stays
                let _ = self
                    .db
                    .with(|c| c.execute("DELETE FROM push_handles WHERE handle_hash = ?1 AND token = ?2", params![hash, target.token]));
                tracing::info!("grid: push: a {} token is gone, its handle dropped", target.platform.as_str());
                refuse(410, "gone")
            }
            Delivered::Failed(why) => {
                tracing::warn!("grid: push: {} refused a note: {why}", target.platform.as_str());
                refuse(502, "The push service refused it.")
            }
        }
    }

    /// How many handles a user has (account pages, tests).
    pub fn handles_of(&self, user_id: &str) -> Result<i64> {
        self.db.with(|c| c.query_row("SELECT COUNT(*) FROM push_handles WHERE user_id = ?1", params![user_id], |r| r.get(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_fcm_and_apns_are_given_is_the_ciphertext_and_the_collapse_key() {
        let m = Outgoing { sealed: "c2VhbGVk".into(), collapse: "t1".into() };
        let f = fcm_message(&Target { platform: Platform::Android, token: "tok".into() }, &m);
        assert_eq!(f["message"]["data"], json!({ "sealed": "c2VhbGVk", "collapse": "t1" }));
        assert!(f["message"].get("notification").is_none(), "a data message: the phone shows what it opens");
        let a = apns_payload(&m);
        assert_eq!((a["aps"]["mutable-content"].clone(), a["aps"]["alert"]["body"].clone()), (json!(1), json!(FALLBACK)));
        assert!(collapse_ok("t1") && !collapse_ok("a b") && handle_ok("wph_abcdefghijklmnopqrstu") && !handle_ok("abc"));
    }
}
