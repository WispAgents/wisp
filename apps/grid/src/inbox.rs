//! Channels reaching a sleeping box (v1 grid/inbox.js). A channel posts to `<handle>.<domain>/_in/<token>`; the
//! grid keeps the request, answers 200 at once, wakes the box and delivers it to the box's `channels.inbound`, in
//! order and one at a time. The grid reads nothing a person said and checks no signature: the box verifies each
//! message itself. A row is held for an hour at most; once delivered it keeps only its event key, which stops a
//! sender's retry.
//!
//! - **Wakes are earned:** a channel never keeps a box awake. After `wake_strikes` wakes in a row with nothing the
//!   box accepted (a 2xx), the next is a scheduled wake (behind people, evicting nobody), at most one per `wake_gap_ms`.
//! - **Strictly in order:** nothing passes a row being retried; a 4xx is the box saying no (finished), anything
//!   else is retried with backoff, then dropped and counted after `MAX_TRIES`.
//! - **Caps:** `max_rows` and `max_bytes` held per user, `BODY_MAX` a body; over them, dropped and counted
//!   (`channel_urls.dropped`, which the box is told with its next delivery), still a 200 to the sender.
//! - **Live:** a request the sender wants answered in the same call (Slack's URL check) goes through only to a box
//!   already up, and never wakes one; asleep, 503.
//! - Nothing a message said goes to a log: channel and size only.
//!
//! - **Discord's three seconds:** `/wisp` and a button press are held and answered at once ("thinking", "seen"); the
//!   box goes on from that answer and checks Discord's signature itself. A ping (the Portal's check) and an
//!   autocomplete keystroke go live, to a box already up.
//!
//! - **Discord is checked at the edge:** its Ed25519 signature, by the application's public key the box set
//!   (`/api/channels/key`), before anything is held, answered or woken. No key, unsigned or forged: 401.
//!
//! Not here yet: linked people at the edge (a linked stranger's `/wisp` still wakes the box, which refuses it with a
//! 403 and a strike), mail's provider webhook (`mail.rs`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use crate::boxhost::{BoxHost, BoxState, Busy, Running};
use crate::clock::SharedClock;
use crate::db::Db;
use crate::error::{Result, bad};
use crate::rows::{BoxUser, UserRow};
use crate::secrets::{Secrets, sha256, token};

mod channels;
mod deliver;
mod drain;
pub use channels::{CHANNELS, ChannelDef, channel, discord_signed, event_key};
pub use deliver::HttpDeliver;

pub const BODY_MAX: usize = 256 * 1024;
const TTL_MS: i64 = 60 * 60_000;
const MAX_TRIES: i64 = 5;

/// A held message its box accepted (a 2xx): `(user id, the delivery)`. Mail keeps the thread then.
pub type Delivered = Arc<dyn Fn(&str, &Delivery) + Send + Sync>;

#[derive(Debug, Clone, Copy)]
pub struct Opts {
    pub enabled: bool,
    pub max_rows: i64,
    pub max_bytes: i64,
    pub ttl_ms: i64,
    pub retry_ms: i64,
    pub wake_retry_ms: i64,
    pub wake_strikes: u32,
    pub wake_gap_ms: i64,
    pub live_ms: u64,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            enabled: false,
            max_rows: 200,
            max_bytes: 2 * 1024 * 1024,
            ttl_ms: TTL_MS,
            retry_ms: 2000,
            wake_retry_ms: 30_000,
            wake_strikes: 3,
            wake_gap_ms: 10 * 60_000,
            live_ms: 2500,
        }
    }
}

/// One message on its way into a box.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    pub channel: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub at: i64,
    pub dropped: i64,
    pub verified: bool,
    pub key: String,
}

/// The box's side: what it answered (its transport's status and body).
#[async_trait::async_trait]
pub trait Deliver: Send + Sync {
    async fn deliver(&self, port: u16, secret: &str, d: &Delivery, timeout_ms: u64) -> std::result::Result<(u16, Value), String>;
}

/// The boxes as the inbox sees them (`hostd`), with the secret the box takes from the grid.
pub trait Boxes: Send + Sync {
    fn state(&self, user_id: &str) -> BoxState;
    fn port(&self, user_id: &str) -> Option<u16>;
    fn begin(&self, user_id: &str) -> Busy;
    fn ensure<'a>(&'a self, user: &'a BoxUser, wake: bool) -> futures::future::BoxFuture<'a, Result<Running>>;
    fn secret(&self, user_id: &str) -> Option<String>;
}

impl Boxes for crate::hostd::ProcessHost {
    fn state(&self, user_id: &str) -> BoxState {
        BoxHost::state(self, user_id)
    }
    fn port(&self, user_id: &str) -> Option<u16> {
        BoxHost::port(self, user_id)
    }
    fn begin(&self, user_id: &str) -> Busy {
        BoxHost::begin(self, user_id, false)
    }
    fn ensure<'a>(&'a self, user: &'a BoxUser, wake: bool) -> futures::future::BoxFuture<'a, Result<Running>> {
        BoxHost::ensure(self, user, wake)
    }
    fn secret(&self, user_id: &str) -> Option<String> {
        crate::hostd::ProcessHost::secret(self, user_id)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Strikes {
    n: u32,
    at: i64,
}

pub struct Inbox {
    pub db: Arc<Db>,
    pub clock: SharedClock,
    pub secrets: Option<Arc<Secrets>>,
    pub boxes: Arc<dyn Boxes>,
    pub deliver: Arc<dyn Deliver>,
    pub opts: Opts,
    /// a user's handle → its box's public URL
    pub url_of: Arc<dyn Fn(&str) -> String + Send + Sync>,
    wakes: Mutex<HashMap<String, Strikes>>,
    busy: tokio::sync::Mutex<()>,
    /// told of each delivery a box accepted (set once, by mail)
    pub delivered: std::sync::OnceLock<Delivered>,
}

impl std::fmt::Debug for Inbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inbox").field("opts", &self.opts).finish_non_exhaustive()
    }
}

/// What a sender gets beyond the 200: the box's own answer to a live request.
pub type Answer = Option<(u16, Value)>;

impl Inbox {
    pub fn new(
        db: Arc<Db>,
        clock: SharedClock,
        secrets: Option<Arc<Secrets>>,
        boxes: Arc<dyn Boxes>,
        deliver: Arc<dyn Deliver>,
        opts: Opts,
        url_of: Arc<dyn Fn(&str) -> String + Send + Sync>,
    ) -> Self {
        Self {
            db,
            clock,
            secrets,
            boxes,
            deliver,
            opts,
            url_of,
            wakes: Mutex::default(),
            busy: tokio::sync::Mutex::new(()),
            delivered: std::sync::OnceLock::new(),
        }
    }

    fn secrets(&self) -> Result<&Secrets> {
        self.secrets.as_deref().ok_or_else(|| bad("This grid has no master key, so it holds no channels.", 503))
    }

    fn count(&self, user_id: &str, ch: &str) -> Result<()> {
        self.db.with(|c| {
            c.execute("UPDATE channel_urls SET dropped = dropped + 1 WHERE user_id = ?1 AND channel = ?2", params![user_id, ch])
        })?;
        tracing::info!(channel = ch, "grid: inbox dropped a message");
        Ok(())
    }

    /// `{url, dropped}`: this user's own URL, made the first time; dropped: lost messages it hasn't been told of
    /// (this tells it). The token is kept sealed, so asking again gives the same URL.
    pub fn url_for(&self, user: &UserRow, ch: &str) -> Result<Value> {
        channel(ch).ok_or_else(|| bad("No such channel here.", 404))?;
        let sealed = self.secrets()?.ensure(&user.id, &format!("channel:{ch}"))?;
        let now = self.clock.now_ms();
        let dropped: i64 = self.db.with(|c| {
            c.execute("INSERT INTO channel_urls (user_id, channel, token_hash, created_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(user_id, channel) DO UPDATE SET token_hash = excluded.token_hash", params![user.id, ch, sha256(&sealed), now])?;
            let d: i64 = c.query_row("SELECT dropped FROM channel_urls WHERE user_id = ?1 AND channel = ?2", params![user.id, ch], |r| r.get(0))?;
            if d > 0 {
                c.execute("UPDATE channel_urls SET dropped = 0 WHERE user_id = ?1 AND channel = ?2", params![user.id, ch])?;
            }
            Ok(d)
        })?;
        Ok(json!({ "url": format!("{}/_in/{sealed}", (self.url_of)(&user.handle)), "dropped": dropped }))
    }

    /// A new URL for a leaked one: the old token stops here; what's held stays.
    pub fn rotate(&self, user: &UserRow, ch: &str) -> Result<Value> {
        channel(ch).ok_or_else(|| bad("No such channel here.", 404))?;
        let fresh = token("", 32);
        self.secrets()?.seal(&user.id, &format!("channel:{ch}"), &fresh)?;
        self.db.with(|c| {
            c.execute("UPDATE channel_urls SET token_hash = ?1 WHERE user_id = ?2 AND channel = ?3", params![sha256(&fresh), user.id, ch])
        })?;
        Ok(json!({ "url": format!("{}/_in/{fresh}", (self.url_of)(&user.handle)) }))
    }

    /// `(user id, channel)` for a URL's token.
    pub fn by_token(&self, t: &str) -> Result<Option<(String, String)>> {
        self.db.with(|c| {
            c.query_row("SELECT user_id, channel FROM channel_urls WHERE token_hash = ?1 AND channel != 'email'", [sha256(t)], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()
        })
    }

    fn held(&self, user_id: &str) -> Result<(i64, i64)> {
        self.db.with(|c| {
            c.query_row(
                "SELECT count(*), coalesce(sum(size), 0) FROM channel_inbox WHERE user_id = ?1 AND done_at IS NULL",
                [user_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })
    }

    /// The key a channel's requests are checked with at the edge (Discord's application public key, hex), set by
    /// the box; empty: none (its requests are refused, and wake nothing).
    pub fn set_key(&self, user: &UserRow, ch: &str, key: &str) -> Result<()> {
        if ch != "discord" {
            return Err(bad("Only Discord's requests are checked with a key here.", 400));
        }
        let key = key.trim().to_lowercase();
        if !key.is_empty() && (key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err(bad("That isn't a Discord public key (64 hex characters).", 400));
        }
        self.url_for(user, ch)?;
        self.db.with(|c| {
            c.execute(
                "UPDATE channel_urls SET channel_key = ?1 WHERE user_id = ?2 AND channel = ?3",
                params![Some(key).filter(|k| !k.is_empty()), user.id, ch],
            )
        })?;
        Ok(())
    }

    fn key_of(&self, user_id: &str, ch: &str) -> Result<Option<String>> {
        Ok(self
            .db
            .with(|c| {
                c.query_row("SELECT channel_key FROM channel_urls WHERE user_id = ?1 AND channel = ?2", params![user_id, ch], |r| {
                    r.get::<_, Option<String>>(0)
                })
                .optional()
            })?
            .flatten())
    }

    /// Keep what came. `body`: None when it was over `BODY_MAX`. → the sender's answer beyond the 200, if any.
    pub async fn accept(&self, user: &UserRow, ch: &str, headers: &[(String, String)], body: Option<&[u8]>) -> Result<Answer> {
        let def = channel(ch).ok_or_else(|| bad("No such channel here.", 404))?;
        let Some(body) = body else {
            self.count(&user.id, ch)?;
            return Ok(None);
        };
        // Discord: checked here, before anything is held, answered or woken (a forged one costs nothing)
        if ch == "discord" {
            let h = |k: &str| headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map_or("", |(_, v)| v.as_str());
            let key = self.key_of(&user.id, ch)?.unwrap_or_default();
            if !discord_signed(&key, h("x-signature-timestamp"), body, h("x-signature-ed25519")) {
                tracing::info!(channel = ch, "grid: inbox refused an unsigned or forged interaction");
                return Ok(Some((401, json!({ "error": "Bad signature." }))));
            }
        }
        let kept: Vec<(String, String)> = def
            .headers
            .iter()
            .filter_map(|h| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(h)).map(|(_, v)| ((*h).to_owned(), v.clone())))
            .collect();
        let j: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
        if (def.live)(&j) {
            return Ok(Some(self.live(user, ch, kept, body).await));
        }
        if (def.skip)(&j) {
            return Ok(None);
        }
        self.hold(user, def, body, &kept, false, &j)?;
        Ok((def.early)(&j).map(|a| (200, a)))
    }

    /// A request that can't wait: the box's answer if it's up; asleep or starting, 503 (never woken).
    async fn live(&self, user: &UserRow, ch: &str, headers: Vec<(String, String)>, body: &[u8]) -> (u16, Value) {
        let asleep = (503, json!({ "error": "wisp is asleep: try again with it open." }));
        let (Some(port), Some(secret)) =
            (self.boxes.port(&user.id).filter(|_| self.boxes.state(&user.id) == BoxState::Running), self.boxes.secret(&user.id))
        else {
            return asleep;
        };
        let _busy = self.boxes.begin(&user.id);
        let d = Delivery {
            channel: ch.into(),
            headers,
            body: body.to_vec(),
            at: self.clock.now_ms(),
            dropped: 0,
            verified: false,
            key: format!("live:{}", sha256(body)),
        };
        match self.deliver.deliver(port, &secret, &d, self.opts.live_ms).await {
            Ok((s, v)) if s < 500 => (s, v),
            _ => asleep,
        }
    }

    /// Hold one message. `verified`: the grid checked it itself (a route); nothing from a request sets it. → false
    /// when it was dropped (over the caps).
    pub fn hold(
        &self,
        user: &UserRow,
        def: &ChannelDef,
        body: &[u8],
        headers: &[(String, String)],
        verified: bool,
        j: &Value,
    ) -> Result<bool> {
        let hj = serde_json::to_string(headers).unwrap_or_default();
        let size = i64::try_from(body.len() + hj.len()).unwrap_or(i64::MAX);
        let (n, bytes) = self.held(&user.id)?;
        if n >= self.opts.max_rows || bytes + size > self.opts.max_bytes {
            self.count(&user.id, def.name)?;
            return Ok(false);
        }
        let key = event_key(def, j, body, &hj);
        let now = self.clock.now_ms();
        self.db.with(|c| {
            c.execute(
                "INSERT INTO channel_inbox (user_id, channel, event_key, headers, body, size, at, verified) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(user_id, event_key) DO NOTHING",
                params![user.id, def.name, key, hj, body, size, now, i64::from(verified)],
            )
        })?;
        Ok(true)
    }

    /// Past their hour: gone. One that never got there counts as lost; a delivered one just stops guarding.
    pub fn sweep(&self) -> Result<usize> {
        let before = self.clock.now_ms() - self.opts.ttl_ms;
        let lost: Vec<(String, String)> = self.db.with(|c| {
            c.prepare_cached("SELECT user_id, channel FROM channel_inbox WHERE at < ?1 AND done_at IS NULL")?
                .query_map([before], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })?;
        for (u, ch) in &lost {
            self.count(u, ch)?;
        }
        let n = self.db.with(|c| c.execute("DELETE FROM channel_inbox WHERE at < ?1", [before]))?;
        Ok(n)
    }

    /// True the first time a key comes by, for the hour a row lasts: a sender's retry stops here (a route's).
    pub fn first(&self, user_id: &str, key: &str) -> bool {
        let now = self.clock.now_ms();
        self.db
            .with(|c| c.execute("INSERT INTO channel_inbox (user_id, channel, event_key, headers, body, size, at, done_at) VALUES (?1, 'seen', ?2, '[]', x'', 0, ?3, ?3) ON CONFLICT(user_id, event_key) DO NOTHING", params![user_id, key, now]))
            .is_ok_and(|n| n > 0)
    }

    /// The users with something held (after a restart, each is drained).
    pub fn waiting(&self) -> Result<Vec<String>> {
        self.db.with(|c| {
            c.prepare_cached("SELECT DISTINCT user_id FROM channel_inbox WHERE done_at IS NULL")?.query_map([], |r| r.get(0))?.collect()
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
