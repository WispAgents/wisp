//! Webhook calls held for an offline computer (ADR-0147): a call to `<computer>--<handle>/_hook/<token>` that finds the
//! computer offline is kept here and answered 202 `{held: true}`; once its tunnel is back, the held calls go to it.
//!
//! - **Sealed:** the call (its path, headers and body) is sealed under the user's data key (`Secrets::seal_bytes`,
//!   bound to the computer), since it carries the sender's signature and maybe `x-wisp-secret`. Without a master key the
//!   grid holds nothing: 503, as before.
//! - **Strictly in order:** while anything is held for a computer, a new call is held behind it, even with the computer
//!   online, and nothing passes a call being retried.
//! - **What ends a call:** the computer's 2xx or 4xx (it read the call and said yes or no: retrying can't help). A 5xx
//!   or no answer is tried again with backoff ([`RETRY_MS`] doubling, at most [`RETRY_MAX_MS`]), and dropped after
//!   [`MAX_TRIES`]. The tunnel gone mid-way isn't a try: the next tunnel picks up from there.
//! - **Caps:** [`MAX_ROWS`] calls and [`MAX_BYTES`] held per computer, [`TTL_MS`] at most; over a cap the call isn't
//!   held but answered 503 with `Retry-After`, so the sender keeps it (nothing is silently dropped on arrival).
//! - **Once:** a call with a delivery id (`x-wisp-delivery`, `x-github-delivery`) is held once per URL; a sender's
//!   retry of it is answered 202 and not held again.
//! - **Revoked or removed:** revoking a URL drops what's held for it; removing the computer or the URL removes it too
//!   (`ON DELETE CASCADE`).
//! - Nothing a call said goes to a log: its URL's id and size only.
use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use bytes::Bytes;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::clock::SharedClock;
use crate::db::Db;
use crate::error::{Result, bad};
use crate::secrets::Secrets;
use crate::tunnels::{AskOptions, Tunnels};

/// How long a call is held at most: a laptop asleep overnight still gets the morning's calls.
pub const TTL_MS: i64 = 24 * 3_600_000;
/// Calls held per computer at most.
pub const MAX_ROWS: i64 = 200;
/// Bytes held per computer at most (sealed bodies count as their size).
pub const MAX_BYTES: i64 = 20 * 1024 * 1024;
/// Tries before a call the computer keeps failing (5xx) is dropped.
pub const MAX_TRIES: i64 = 8;
/// The first retry's wait; each next one doubles, up to [`RETRY_MAX_MS`].
pub const RETRY_MS: i64 = 2_000;
pub const RETRY_MAX_MS: i64 = 5 * 60_000;
/// How long the computer may take to take one held call.
const DELIVER_MS: u64 = 30_000;

/// One call, as sealed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub path: String,
    pub headers: Vec<(String, String)>,
    /// base64
    pub body: String,
}

/// What became of a call the grid was asked to hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// held (or held already, the same delivery id)
    Yes,
    /// a cap is reached, or the grid has no master key: the sender keeps it
    Full,
}

/// One held call, as drained.
struct Row {
    id: i64,
    user_id: String,
    sealed: String,
    tries: i64,
    next_at: i64,
}

pub struct HookHeld {
    pub db: Arc<Db>,
    pub clock: SharedClock,
    pub secrets: Option<Arc<Secrets>>,
    pub tunnels: Arc<Tunnels>,
    draining: Mutex<HashSet<String>>,
}

impl std::fmt::Debug for HookHeld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookHeld").finish_non_exhaustive()
    }
}

/// The wait before try number `tries` (1 for the first retry).
pub fn backoff(tries: i64) -> i64 {
    let shift = u32::try_from(tries.saturating_sub(1).clamp(0, 20)).unwrap_or(20);
    RETRY_MS.saturating_mul(1_i64 << shift).min(RETRY_MAX_MS)
}

fn purpose(computer_id: &str) -> String {
    format!("hook:{computer_id}")
}

/// Whether a computer's drain is under way (one at a time); dropping it lets the next one start.
struct Draining<'a>(&'a HookHeld, String);
impl Drop for Draining<'_> {
    fn drop(&mut self) {
        self.0.draining.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.1);
    }
}

impl HookHeld {
    pub fn new(db: Arc<Db>, clock: SharedClock, secrets: Option<Arc<Secrets>>, tunnels: Arc<Tunnels>) -> Self {
        Self { db, clock, secrets, tunnels, draining: Mutex::default() }
    }

    fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    fn expire(&self) -> Result<()> {
        let old = self.now() - TTL_MS;
        self.db.with(|c| c.execute("DELETE FROM hook_held WHERE at < ?1", [old]))?;
        Ok(())
    }

    /// Whether anything is held for the computer (a new call then waits behind it).
    pub fn any(&self, computer_id: &str) -> Result<bool> {
        self.expire()?;
        self.db.with(|c| c.query_row("SELECT EXISTS(SELECT 1 FROM hook_held WHERE computer_id = ?1)", [computer_id], |r| r.get(0)))
    }

    /// Holds a call to `computer_id` made through URL `hook_id` of `user_id`.
    pub fn hold(&self, user_id: &str, computer_id: &str, hook_id: &str, delivery: Option<&str>, call: &Call) -> Result<Held> {
        let Some(secrets) = self.secrets.as_deref() else { return Ok(Held::Full) };
        self.expire()?;
        let plain = serde_json::to_vec(call)?;
        let size = i64::try_from(plain.len()).unwrap_or(i64::MAX);
        let sealed = secrets.seal_bytes(user_id, &purpose(computer_id), &plain)?;
        let now = self.now();
        self.db.tx(|tx| {
            if let Some(d) = delivery {
                let seen: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM hook_held WHERE hook_id = ?1 AND delivery = ?2)",
                    params![hook_id, d],
                    |r| r.get(0),
                )?;
                if seen {
                    return Ok(Held::Yes);
                }
            }
            let (n, bytes): (i64, i64) =
                tx.query_row("SELECT COUNT(*), COALESCE(SUM(size), 0) FROM hook_held WHERE computer_id = ?1", [computer_id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
            if n >= MAX_ROWS || bytes + size > MAX_BYTES {
                return Ok(Held::Full);
            }
            tx.execute(
                "INSERT INTO hook_held (hook_id, computer_id, user_id, delivery, sealed, size, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![hook_id, computer_id, user_id, delivery, sealed, size, now],
            )?;
            Ok(Held::Yes)
        })
    }

    /// Drops what's held for a URL (it was revoked).
    pub fn drop_hook(&self, hook_id: &str) -> Result<()> {
        self.db.with(|c| c.execute("DELETE FROM hook_held WHERE hook_id = ?1", [hook_id]))?;
        Ok(())
    }

    /// How many calls are held for a computer (for the tests and the logs).
    pub fn count(&self, computer_id: &str) -> Result<i64> {
        self.db.with(|c| c.query_row("SELECT COUNT(*) FROM hook_held WHERE computer_id = ?1", [computer_id], |r| r.get(0)))
    }

    fn next_row(&self, computer_id: &str) -> Result<Option<Row>> {
        self.expire()?;
        self.db.with(|c| {
            c.query_row(
                "SELECT id, user_id, sealed, tries, next_at FROM hook_held WHERE computer_id = ?1 ORDER BY id LIMIT 1",
                [computer_id],
                |r| Ok(Row { id: r.get(0)?, user_id: r.get(1)?, sealed: r.get(2)?, tries: r.get(3)?, next_at: r.get(4)? }),
            )
            .optional()
        })
    }

    fn done(&self, id: i64) -> Result<()> {
        self.db.with(|c| c.execute("DELETE FROM hook_held WHERE id = ?1", [id]))?;
        Ok(())
    }

    /// Delivers what's held for the computer in the background, unless a drain is under way already.
    pub fn kick(self: &Arc<Self>, computer_id: &str) {
        {
            let mut d = self.draining.lock().unwrap_or_else(PoisonError::into_inner);
            if !d.insert(computer_id.to_owned()) {
                return;
            }
        }
        let me = self.clone();
        let id = computer_id.to_owned();
        tokio::spawn(async move {
            let _one = Draining(&me, id.clone());
            if let Err(e) = me.drain(&id).await {
                tracing::warn!("grid: held webhook calls for {id}: {e}");
            }
        });
    }

    /// Delivers the computer's held calls in order while its tunnel is up; returns when nothing's left or it's gone.
    pub async fn drain(&self, computer_id: &str) -> Result<()> {
        loop {
            let Some(row) = self.next_row(computer_id)? else { return Ok(()) };
            if !self.tunnels.online(computer_id) {
                return Ok(()); // the next tunnel picks up from here
            }
            let wait = row.next_at - self.now();
            if wait > 0 {
                tokio::time::sleep(Duration::from_millis(u64::try_from(wait).unwrap_or(0))).await;
                continue;
            }
            let call = match self.open(&row, computer_id) {
                Ok(c) => c,
                Err(e) => {
                    // a call that won't open (its key gone) can never be delivered
                    tracing::warn!("grid: a held webhook call for {computer_id} won't open: {e}");
                    self.done(row.id)?;
                    continue;
                }
            };
            let body = B64.decode(call.body.as_bytes()).unwrap_or_default();
            let opts = AskOptions {
                method: "POST".into(),
                path: call.path,
                headers: call.headers,
                body: Some(Bytes::from(body)),
                timeout_ms: Some(DELIVER_MS),
            };
            let status = match self.tunnels.ask(computer_id, opts).await {
                Ok(a) => a.status,
                Err(e) if e.status() == 502 && !self.tunnels.online(computer_id) => return Ok(()),
                Err(e) => e.status(),
            };
            if (200..500).contains(&status) {
                self.done(row.id)?;
                continue;
            }
            let tries = row.tries + 1;
            if tries >= MAX_TRIES {
                tracing::warn!("grid: dropped a held webhook call for {computer_id} after {tries} tries ({status})");
                self.done(row.id)?;
                continue;
            }
            let next = self.now() + backoff(tries);
            self.db.with(|c| c.execute("UPDATE hook_held SET tries = ?1, next_at = ?2 WHERE id = ?3", params![tries, next, row.id]))?;
        }
    }

    fn open(&self, row: &Row, computer_id: &str) -> Result<Call> {
        let secrets = self.secrets.as_deref().ok_or_else(|| bad("This grid has no master key.", 503))?;
        let plain = secrets.open_bytes(&row.user_id, &purpose(computer_id), &row.sealed)?;
        Ok(serde_json::from_slice(&plain)?)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::accounts::{Accounts, Poll};
    use crate::clock::ManualClock;
    use crate::hooks::Hooks;

    struct Setup {
        held: HookHeld,
        clock: Arc<ManualClock>,
        user: String,
        computer: String,
        hook: String,
    }

    fn setup() -> Setup {
        let db = Arc::new(Db::memory().unwrap());
        let clock = ManualClock::new(1_000_000);
        let accounts = Accounts { db: db.clone(), clock: clock.clone(), telemetry: None };
        let ada = accounts.dev_user("ada").unwrap();
        let code = accounts.device_code("studio").unwrap();
        accounts.decide(code["user_code"].as_str().unwrap(), &ada.id, true).unwrap();
        let Poll::Token { token, .. } = accounts.poll(code["device_code"].as_str().unwrap()).unwrap() else { unreachable!() };
        let (_, dev) = accounts.device(&token).unwrap().unwrap();
        let c = accounts.add_computer(&ada, &dev, "studio").unwrap();
        let hooks = Hooks { db: db.clone(), clock: clock.clone() };
        let (v, _) = hooks.create(&ada.id, &c.id, "studio", "deploy").unwrap();
        let secrets = Some(Arc::new(Secrets::for_test(db.clone(), [7; 32]).unwrap()));
        let held = HookHeld::new(db, clock.clone(), secrets, Arc::new(Tunnels::default()));
        Setup { held, clock, user: ada.id, computer: c.id, hook: v.id }
    }

    fn call(body: &str) -> Call {
        Call { path: "/api/triggers/deploy".into(), headers: vec![("x-wisp-secret".into(), "s3cret".into())], body: B64.encode(body) }
    }

    #[test]
    fn a_held_call_is_sealed_and_opens_for_its_user_and_computer_only() {
        let s = setup();
        assert_eq!(s.held.hold(&s.user, &s.computer, &s.hook, None, &call("{\"ref\":\"main\"}")).unwrap(), Held::Yes);
        let sealed: String = s.held.db.with(|c| c.query_row("SELECT sealed FROM hook_held", [], |r| r.get(0))).unwrap();
        assert!(!sealed.contains("s3cret") && !sealed.contains("main"), "kept sealed: {sealed}");
        let row = s.held.next_row(&s.computer).unwrap().unwrap();
        assert_eq!(s.held.open(&row, &s.computer).unwrap(), call("{\"ref\":\"main\"}"));
        assert!(s.held.open(&row, "another-computer").is_err(), "bound to its computer");
    }

    #[test]
    fn a_delivery_id_is_held_once_and_a_call_without_one_each_time() {
        let s = setup();
        for _ in 0..2 {
            assert_eq!(s.held.hold(&s.user, &s.computer, &s.hook, Some("d1"), &call("a")).unwrap(), Held::Yes);
            assert_eq!(s.held.hold(&s.user, &s.computer, &s.hook, None, &call("b")).unwrap(), Held::Yes);
        }
        assert_eq!(s.held.count(&s.computer).unwrap(), 3);
    }

    #[test]
    fn past_a_cap_a_call_is_not_held_and_a_day_old_one_is_gone() {
        let s = setup();
        for _ in 0..MAX_ROWS {
            assert_eq!(s.held.hold(&s.user, &s.computer, &s.hook, None, &call("x")).unwrap(), Held::Yes);
        }
        assert_eq!(s.held.hold(&s.user, &s.computer, &s.hook, None, &call("x")).unwrap(), Held::Full);
        s.clock.advance(TTL_MS + 1);
        assert!(!s.held.any(&s.computer).unwrap(), "held a day at most");
        let big = "y".repeat(usize::try_from(MAX_BYTES).unwrap());
        assert_eq!(s.held.hold(&s.user, &s.computer, &s.hook, None, &call(&big)).unwrap(), Held::Full, "over the bytes");
    }

    #[test]
    fn revoking_a_url_drops_what_it_held_and_no_master_key_holds_nothing() {
        let s = setup();
        s.held.hold(&s.user, &s.computer, &s.hook, None, &call("x")).unwrap();
        s.held.drop_hook(&s.hook).unwrap();
        assert_eq!(s.held.count(&s.computer).unwrap(), 0);
        let none = HookHeld::new(s.held.db.clone(), s.clock.clone(), None, Arc::new(Tunnels::default()));
        assert_eq!(none.hold(&s.user, &s.computer, &s.hook, None, &call("x")).unwrap(), Held::Full);
    }

    #[test]
    fn retries_wait_longer_each_time_up_to_five_minutes() {
        let waits: Vec<i64> = (1..=10).map(backoff).collect();
        assert_eq!(waits[..5], [2_000, 4_000, 8_000, 16_000, 32_000]);
        assert_eq!(waits[9], RETRY_MAX_MS);
    }
}
