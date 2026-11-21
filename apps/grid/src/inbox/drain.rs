//! Draining a user's held messages into their box: strictly in order, waking it as its wakes are earned, retrying
//! with backoff, and what's taken told to `delivered`.
use std::collections::HashMap;
use std::sync::PoisonError;

use rusqlite::{OptionalExtension, params};

use super::{Delivery, Inbox, MAX_TRIES, Strikes};
use crate::boxhost::BoxState;
use crate::error::Result;
use crate::rows::{BoxUser, UserRow};

/// One held message, as drained.
pub(super) struct Row {
    pub(super) id: i64,
    pub(super) channel: String,
    pub(super) key: String,
    pub(super) headers: String,
    pub(super) body: Vec<u8>,
    pub(super) at: i64,
    pub(super) tries: i64,
    pub(super) next_at: i64,
    pub(super) verified: bool,
}

impl Inbox {
    /// The next row to deliver for a user (the oldest not done).
    fn next_row(&self, user_id: &str) -> Result<Option<Row>> {
        self.db.with(|c| {
            c.query_row("SELECT id, channel, event_key, headers, body, at, tries, next_at, verified FROM channel_inbox WHERE user_id = ?1 AND done_at IS NULL ORDER BY id LIMIT 1", [user_id], |r| {
                Ok(Row { id: r.get(0)?, channel: r.get(1)?, key: r.get(2)?, headers: r.get(3)?, body: r.get(4)?, at: r.get(5)?, tries: r.get(6)?, next_at: r.get(7)?, verified: r.get::<_, i64>(8)? != 0 })
            })
            .optional()
        })
    }

    fn finish(&self, id: i64) -> Result<()> {
        let now = self.clock.now_ms();
        self.db.with(|c| c.execute("UPDATE channel_inbox SET done_at = ?1, body = x'', headers = '[]' WHERE id = ?2", params![now, id]))?;
        Ok(())
    }

    fn strikes(&self) -> std::sync::MutexGuard<'_, HashMap<String, Strikes>> {
        self.wakes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Deliver what's held for `user`, in order, until it's all in or one must wait. → how long until the next try
    /// (None: nothing left, or the account's gone).
    pub async fn drain(&self, user: &UserRow) -> Result<Option<i64>> {
        if !self.opts.enabled || user.disabled != 0 {
            return Ok(None);
        }
        let _one = self.busy.lock().await;
        loop {
            let Some(row) = self.next_row(&user.id)? else { return Ok(None) };
            let now = self.clock.now_ms();
            if row.next_at > now {
                return Ok(Some(row.next_at - now)); // strictly in order: nothing passes a row being retried
            }
            if self.boxes.state(&user.id) != BoxState::Running {
                match self.wake(user, now).await {
                    Some(wait) => return Ok(Some(wait)),
                    None => continue,
                }
            }
            let (Some(port), Some(secret)) = (self.boxes.port(&user.id), self.boxes.secret(&user.id)) else {
                return Ok(Some(self.opts.retry_ms));
            };
            let status = self.deliver_row(user, &row, port, &secret).await?;
            // 4xx: the box read it and said no (a bad signature, a stranger, the channel off): retrying can't help
            if (200..500).contains(&status) {
                self.finish(row.id)?;
                continue;
            }
            let tries = row.tries + 1;
            if tries >= MAX_TRIES {
                self.finish(row.id)?;
                self.count(&user.id, &row.channel)?;
                continue;
            }
            let next = self.opts.retry_ms * 2i64.pow(u32::try_from(tries - 1).unwrap_or(0));
            self.db.with(|c| {
                c.execute("UPDATE channel_inbox SET tries = ?1, next_at = ?2 WHERE id = ?3", params![tries, now + next, row.id])
            })?;
            return Ok(Some(next));
        }
    }

    /// The box is asleep: wake it (a scheduled wake once its wakes stopped earning anything). → how long to wait first,
    /// or None once it's up.
    async fn wake(&self, user: &UserRow, now: i64) -> Option<i64> {
        let w = self.strikes().get(&user.id).copied().unwrap_or_default();
        let cold = w.n >= self.opts.wake_strikes;
        let wait = if cold { w.at + self.opts.wake_gap_ms - now } else { 0 };
        if wait > 0 {
            return Some(wait);
        }
        if self.boxes.ensure(&BoxUser::user(user), cold).await.is_err() {
            return Some(self.opts.wake_retry_ms); // full, or it wouldn't start: not the message's fault
        }
        self.strikes().insert(user.id.clone(), Strikes { n: w.n + 1, at: now });
        None
    }

    /// One held row to the box's `channels.inbound`, with how many were dropped since: the box's status (0 when it
    /// didn't answer). Taken (2xx): the strikes clear, `delivered` hears it, and the dropped count it was told goes.
    async fn deliver_row(&self, user: &UserRow, row: &Row, port: u16, secret: &str) -> Result<u16> {
        let dropped: i64 = self
            .db
            .with(|c| {
                c.query_row("SELECT dropped FROM channel_urls WHERE user_id = ?1 AND channel = ?2", params![user.id, row.channel], |r| {
                    r.get(0)
                })
                .optional()
            })?
            .unwrap_or(0);
        let d = Delivery {
            channel: row.channel.clone(),
            headers: serde_json::from_str(&row.headers).unwrap_or_default(),
            body: row.body.clone(),
            at: row.at,
            dropped,
            verified: row.verified,
            key: row.key.clone(),
        };
        let status = {
            let _busy = self.boxes.begin(&user.id); // not stopped under it, and it isn't use
            self.deliver.deliver(port, secret, &d, 30_000).await.map_or(0, |(s, _)| s)
        };
        if (200..300).contains(&status) {
            self.strikes().remove(&user.id);
            if let Some(f) = self.delivered.get() {
                f(&user.id, &d);
            }
            if dropped > 0 {
                self.db.with(|c| {
                    c.execute(
                        "UPDATE channel_urls SET dropped = max(0, dropped - ?1) WHERE user_id = ?2 AND channel = ?3",
                        params![dropped, user.id, row.channel],
                    )
                })?;
            }
        }
        Ok(status)
    }
}
