//! hostd's clock work (v1 grid/hostd.js `tick`, `#scheduleWake`, `restore`, `close`):
//!
//! - **tick:** every `checkMs`, each running box's `/api/status`: its next scheduled task (`nextWakeAt`) and its
//!   last activity are kept, and a box with nothing going on for `idleMs` is put to sleep.
//! - **wake:** a box asleep with a task scheduled is started a little before it (`WAKE_EARLY_MS`), as a scheduled
//!   wake (`ensure(…, wake: true)`: it doesn't count as use). After a restart the grid hands the times it kept back
//!   (`restore`). With no room it waits in line behind people (`wake queued`); a start that fails keeps the schedule
//!   (`wake failed`) and is tried again, 30 s, 1 min, 2 min… up to 10 min apart, until an hour past the task
//!   (`wake missed`).
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

/// A wake that keeps failing is tried until this long past its task, then given up (and said).
pub const WAKE_GIVE_UP_MS: i64 = 3_600_000;
/// The longest wait between a failed wake's tries.
pub const WAKE_RETRY_CAP_MS: i64 = 600_000;

/// How long to wait before waking for a task due `at` (now `now`): a little early, never negative.
pub fn wake_in(at: i64, now: i64) -> Duration {
    Duration::from_millis(u64::try_from(at - now - WAKE_EARLY_MS).unwrap_or(0))
}

/// The wait before a failed wake's next try (`attempt` 0 first): `base`, doubling, capped at 10 min.
pub fn retry_in(base: i64, attempt: u32) -> Duration {
    let ms = base.max(1).saturating_mul(1_i64 << attempt.min(20)).min(WAKE_RETRY_CAP_MS);
    Duration::from_millis(u64::try_from(ms).unwrap_or(0))
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
            if self.reap_if_exited(&id) {
                continue;
            }
            let Ok(r) = self.http.get(format!("http://127.0.0.1:{port}/api/status")).send().await else {
                let _ = self.reap_if_exited(&id);
                continue;
            };
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
        // the line moves on every tick too: who left it goes, and a start that missed its turn gets it
        self.drain();
    }

    /// Wakes the box for its next scheduled task, if it has one and sleeps (replacing an earlier timer).
    pub(super) fn schedule_wake(&self, id: &str) {
        let now = self.clock.now_ms();
        let at = lock(&self.inst).get(id).and_then(|i| i.next_wake_at);
        self.arm_wake(id, at.map_or(Duration::ZERO, |at| wake_in(at, now)), 0);
    }

    /// The wake timer: after `wait`, a scheduled wake (its `attempt`th try), replacing any earlier timer.
    fn arm_wake(&self, id: &str, wait: Duration, attempt: u32) {
        if self.is_closed() {
            return;
        }
        let mut m = lock(&self.inst);
        let Some(i) = m.get_mut(id) else { return };
        if let Some(w) = i.wake.take() {
            w.abort();
        }
        let (Some(_), Some(user), BoxState::Stopped) = (i.next_wake_at, i.user.clone(), i.state) else { return };
        let me = self.me.clone();
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
                match h.ensure(&user, true).await {
                    Ok(_) => {}
                    // no room: it waits in line behind people, and starts once there's room
                    Err(_) if h.state(&user.id) == BoxState::Queued => {}
                    Err(e) => {
                        tracing::warn!("hostd: wake {}: {e}", user.handle);
                        h.wake_failed(&user, attempt);
                    }
                }
            }
        });
        i.wake = Some(task.abort_handle());
    }

    /// A scheduled wake that didn't start (it said `wake failed`, its schedule kept): tried again, later each time,
    /// while its task is recent; past that the miss is said (`wake missed`) and it waits for the next open.
    pub(super) fn wake_failed(&self, user: &BoxUser, attempt: u32) {
        let now = self.clock.now_ms();
        let at = lock(&self.inst).get(&user.id).and_then(|i| i.next_wake_at);
        match at {
            Some(at) if now <= at + WAKE_GIVE_UP_MS => self.arm_wake(&user.id, retry_in(self.o.wake_retry_ms, attempt), attempt + 1),
            _ => {
                tracing::warn!("hostd: {} missed its scheduled wake", user.handle);
                self.change(user, BoxState::Stopped, "wake missed");
            }
        }
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
        lock(&self.queue).clear();
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

    #[test]
    fn a_failed_wake_tries_again_later_each_time_up_to_ten_minutes() {
        let s: Vec<u64> = (0..7).map(|n| retry_in(30_000, n).as_secs()).collect();
        assert_eq!(s, [30, 60, 120, 240, 480, 600, 600]);
        assert_eq!(retry_in(30_000, 99), Duration::from_secs(600));
    }
}
