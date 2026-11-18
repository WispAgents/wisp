//! hostd's clock work (v1 grid/hostd.js `tick`, `#scheduleWake`, `restore`, `close`):
//!
//! - **tick:** every `checkMs`, each running box's `/api/status`: its next scheduled task (`nextWakeAt`) and its
//!   last activity are kept, and a box with nothing going on for `idleMs` is put to sleep.
//! - **wake:** a box asleep with a task scheduled is started a little before it (`WAKE_EARLY_MS`), as a scheduled
//!   wake (`ensure(…, wake: true)`: it doesn't count as use). After a restart the grid hands the times it kept back
//!   (`restore`).
//! - **close:** the grid is stopping: nothing more starts, wake timers end, a start under way gives up and kills its
//!   process, and every running box is stopped, so no box outlives the grid.
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::{Inst, ProcessHost, lock};
use crate::boxhost::{BoxHost as _, BoxState};
use crate::rows::BoxUser;

/// A scheduled wake starts the box this long before its task, so it's up when the task is due (v1).
pub const WAKE_EARLY_MS: i64 = 30_000;

/// What a box's `/api/status` says about being busy, its schedule and its last activity.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Status {
    pub busy: bool,
    pub next_wake_at: Option<i64>,
    pub last_activity_at: Option<i64>,
}

impl Status {
    /// v1's shape: `{live, sides, terms, channels, lastActivityAt, nextWakeAt}`.
    pub fn of(s: &serde_json::Value) -> Self {
        let n = |k: &str| s.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0);
        let t = |k: &str| s.get(k).and_then(serde_json::Value::as_i64).filter(|t| *t > 0);
        Self {
            busy: n("live") + n("sides") + n("terms") + n("channels") > 0,
            next_wake_at: t("nextWakeAt"),
            last_activity_at: t("lastActivityAt"),
        }
    }
}

/// How long to wait before waking for a task due `at` (now `now`): a little early, never negative.
pub fn wake_in(at: i64, now: i64) -> Duration {
    Duration::from_millis(u64::try_from(at - now - WAKE_EARLY_MS).unwrap_or(0))
}

impl ProcessHost {
    pub(super) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Every running box's status, kept; the quiet ones put to sleep.
    pub(super) async fn tick(&self) {
        let running: Vec<(String, u16)> = lock(&self.inst)
            .iter()
            .filter(|(_, i)| i.state == BoxState::Running)
            .filter_map(|(id, i)| i.port.map(|p| (id.clone(), p)))
            .collect();
        for (id, port) in running {
            let Ok(r) = self.http.get(format!("http://127.0.0.1:{port}/api/status")).send().await else { continue };
            let s = Status::of(&r.json::<serde_json::Value>().await.unwrap_or_default());
            let now = self.clock.now_ms();
            let quiet = {
                let mut m = lock(&self.inst);
                let Some(i) = m.get_mut(&id).filter(|i| i.state == BoxState::Running) else { continue };
                i.next_wake_at = s.next_wake_at;
                i.last_active_at = s.last_activity_at.max(i.last_active_at);
                // checked after the await: a request may have come in meanwhile
                let last = s.last_activity_at.unwrap_or(0).max(i.last_touch);
                i.inflight == 0 && !s.busy && now - last > self.o.idle_ms
            };
            if quiet {
                tracing::info!("hostd: {id} idle, stopping");
                let _ = self.stop(&id, "idle").await;
            }
        }
    }

    /// Wakes the box for its next scheduled task, if it has one and sleeps (replacing an earlier timer).
    pub(super) fn schedule_wake(&self, id: &str) {
        if self.is_closed() {
            return;
        }
        let now = self.clock.now_ms();
        let mut m = lock(&self.inst);
        let Some(i) = m.get_mut(id) else { return };
        if let Some(w) = i.wake.take() {
            w.abort();
        }
        let (Some(at), Some(user), BoxState::Stopped) = (i.next_wake_at, i.user.clone(), i.state) else { return };
        let me = self.me.clone();
        let wait = wake_in(at, now);
        let task = tokio::spawn(async move {
            tokio::time::sleep(wait).await;
            let Some(h) = me.upgrade() else { return };
            // still asleep, still someone's (a deleted account's box is gone from the map), the grid still up
            // this timer has fired: off the box, so the start below (which ends any timer) doesn't end itself
            let asleep = lock(&h.inst).get_mut(&user.id).is_some_and(|i| {
                i.wake = None;
                i.state == BoxState::Stopped
            });
            if asleep && !h.is_closed() {
                tracing::info!("hostd: {} wakes for its schedule", user.handle);
                if let Err(e) = h.ensure(&user, true).await {
                    tracing::warn!("hostd: wake {}: {e}", user.handle);
                }
            }
        });
        i.wake = Some(task.abort_handle());
    }

    /// After a restart: the box's next scheduled task, as the grid kept it, wakes it.
    pub(super) fn restore_wake(&self, user: &BoxUser, next_wake_at: Option<i64>) {
        {
            let mut m = lock(&self.inst);
            let i: &mut Inst = m.entry(user.id.clone()).or_default();
            i.user = Some(user.clone());
            i.next_wake_at = next_wake_at;
        }
        self.schedule_wake(&user.id);
    }

    /// The grid is stopping: no box outlives it.
    pub(super) async fn close_boxes(&self) {
        self.closed.store(true, Ordering::SeqCst);
        for i in lock(&self.inst).values_mut() {
            if let Some(w) = i.wake.take() {
                w.abort();
            }
        }
        // a start under way sees `closed`, kills its process and lets go of its gate
        let gates: Vec<_> = lock(&self.starts).values().cloned().collect();
        for g in gates {
            drop(g.lock().await);
        }
        let ids: Vec<String> = lock(&self.inst).keys().cloned().collect();
        for id in ids {
            let _ = self.stop(&id, "grid stopped").await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_says_busy_schedule_and_activity_like_v1() {
        let s =
            Status::of(&serde_json::json!({ "live": 0, "sides": 0, "terms": 1, "channels": 0, "nextWakeAt": 900, "lastActivityAt": 50 }));
        assert_eq!(s, Status { busy: true, next_wake_at: Some(900), last_activity_at: Some(50) });
        assert_eq!(Status::of(&serde_json::json!({ "nextWakeAt": null })), Status::default());
        assert_eq!(Status::of(&serde_json::Value::Null), Status::default());
    }

    #[test]
    fn a_scheduled_wake_comes_a_little_early_and_never_late() {
        assert_eq!(wake_in(100_000, 0), Duration::from_secs(70));
        assert_eq!(wake_in(10_000, 0), Duration::ZERO, "due within the early margin: now");
        assert_eq!(wake_in(0, 50_000), Duration::ZERO, "already past");
    }
}
