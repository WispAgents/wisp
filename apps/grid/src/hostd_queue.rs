//! hostd's line (v1 grid/hostd.js `#queue`): a start with no room waits here instead of failing.
//!
//! - **Room:** a box occupies a slot while it starts, runs or stops (its process or container is there); a start
//!   takes one only if fewer than `maxRunning` are taken by others and by the starts ahead of it in line. Counting
//!   and taking happen under one `inst` lock, so two cold starts never both see the last slot.
//! - **Per user:** a sandbox whose owner already has `maxRunningPerUser` boxes up (their own and their sandboxes)
//!   waits in line without holding anyone else up; the owner's own box is never held back by it.
//! - **Order:** people first, in the order they came; scheduled wakes behind them. A wake already waiting that a
//!   person then opens moves up with the people.
//! - **Moving:** whenever room frees (a stop, an exit, a failed start, a removal), and on every idle tick, each one
//!   in line that may start now is started, as its own caller would (`start` decides again under the lock). Who may
//!   start is counted the same way in both places (`starters`): walking the line in order, a sandbox takes one of
//!   its owner's places only while the owner has one left, counting the ones before it in line. So two slots freeing
//!   at once never pick two sandboxes of an owner with room for one and leave the person behind them waiting by an
//!   empty slot. A removed box leaves the line.
//! - **Giving up:** a person's start that no one asks after for `queueMs` (no waiting page polling, no caller
//!   retrying: a tab that was closed) leaves the line, so it never takes a slot later for no one. Scheduled wakes
//!   stay in line until there's room.
use std::collections::{HashMap, VecDeque};

use super::{Inst, ProcessHost, lock};
use crate::boxhost::BoxState;
use crate::error::{GridError, bad};
use crate::rows::BoxUser;

/// How long a failed start is told to a page that opens the box (rather than starting it again by itself).
const FAILED_LATELY_MS: i64 = 10 * 60_000;

/// One start waiting for room.
#[derive(Debug, Clone)]
pub(super) struct Waiting {
    pub user: BoxUser,
    /// a scheduled wake's: behind people, and it doesn't count as use
    pub wake: bool,
    /// when its person last asked after it (a start, a waiting page's poll): past `queueMs` it leaves the line
    pub seen: i64,
}

/// Holds a slot: its process or container is there.
fn occupies(i: &Inst) -> bool {
    matches!(i.state, BoxState::Starting | BoxState::Running | BoxState::Stopping)
}

impl ProcessHost {
    /// How many boxes of `owner` (their own and their sandboxes) hold a slot.
    fn owners_up(m: &HashMap<String, Inst>, owner: &str) -> usize {
        m.iter()
            .filter(|(id, i)| occupies(i) && (id.as_str() == owner || i.user.as_ref().is_some_and(|u| u.owner.as_deref() == Some(owner))))
            .count()
    }

    /// The ones in `line` (in order) that could start if there were room: a sandbox whose owner is at
    /// `maxRunningPerUser` (counting the owner's boxes up and their sandboxes before it here) holds no one up.
    fn starters<'a>(&self, m: &HashMap<String, Inst>, line: impl IntoIterator<Item = &'a Waiting>) -> Vec<&'a Waiting> {
        let mut up: HashMap<&'a str, usize> = HashMap::new();
        line.into_iter()
            .filter(|w| {
                let Some(o) = w.user.owner.as_deref() else { return true };
                let n = up.entry(o).or_insert_with(|| Self::owners_up(m, o));
                let room = *n < self.o.max_running_per_user;
                *n += usize::from(room);
                room
            })
            .collect()
    }

    /// Takes out of line the people's starts no one asked after for `queueMs` (asleep again) → who left.
    fn expire(&self, m: &mut HashMap<String, Inst>, q: &mut VecDeque<Waiting>, now: i64) -> Vec<BoxUser> {
        let mut gone = Vec::new();
        q.retain(|w| {
            let keep = w.wake || now - w.seen <= self.o.queue_ms;
            if !keep {
                gone.push(w.user.clone());
            }
            keep
        });
        for u in &gone {
            if let Some(i) = m.get_mut(&u.id).filter(|i| i.state == BoxState::Queued) {
                i.state = BoxState::Stopped;
            }
        }
        gone
    }

    /// Says who left the line (no locks held): asleep again, its schedule's wake armed.
    fn left(&self, gone: Vec<BoxUser>) {
        for u in gone {
            tracing::info!("hostd: {} left the line: no one waited for it", u.handle);
            self.change(&u, BoxState::Stopped, "left the line");
            self.schedule_wake(&u.id);
        }
    }

    /// Someone still waits for it (its waiting page polled): it keeps its place in line.
    pub fn still_waiting(&self, user_id: &str) {
        let now = self.clock.now_ms();
        if let Some(w) = lock(&self.queue).iter_mut().find(|w| w.user.id == user_id) {
            w.seen = now;
        }
    }

    /// Marks `user` Starting if there's room for it, else puts it in line (Queued) and says where: the 503 its
    /// caller hears.
    pub(super) fn admit_or_queue(&self, user: &BoxUser, wake: bool) -> Option<GridError> {
        let now = self.clock.now_ms();
        let mut m = lock(&self.inst);
        let mut q = lock(&self.queue);
        let gone = self.expire(&mut m, &mut q, now);
        let taken = m.iter().filter(|(id, i)| id.as_str() != user.id && occupies(i)).count();
        let mine = q.iter().position(|w| w.user.id == user.id);
        // the ones ahead that could start now: a sandbox held back by its owner's limit holds no one up
        // in line: the ones before it; new (or a wake a person now opens): every person, and for a wake everyone
        let moving_up = mine.is_some_and(|p| q[p].wake && !wake);
        let (ahead, held) = {
            let line = q.iter().enumerate().filter(|(at, w)| {
                w.user.id != user.id
                    && match mine {
                        Some(p) if !moving_up => *at < p,
                        _ => !w.wake || wake,
                    }
            });
            let ahead = self.starters(&m, line.map(|(_, w)| w));
            // a sandbox: its owner's boxes up, and its owner's sandboxes ahead of it in line, take its owner's places
            let held = user.owner.as_deref().is_some_and(|o| {
                Self::owners_up(&m, o) + ahead.iter().filter(|w| w.user.owner.as_deref() == Some(o)).count() >= self.o.max_running_per_user
            });
            (ahead.len(), held)
        };
        if !held && taken + ahead < self.o.max_running {
            q.retain(|w| w.user.id != user.id);
            let i = m.entry(user.id.clone()).or_default();
            i.state = BoxState::Starting;
            i.user = Some(user.clone());
            i.error.clear();
            i.failed_at = None;
            drop(q);
            drop(m);
            self.left(gone);
            return None;
        }
        let new = mine.is_none();
        let at = enqueue(&mut q, user, wake, now);
        let i = m.entry(user.id.clone()).or_default();
        i.state = BoxState::Queued;
        i.user = Some(user.clone());
        drop(q);
        drop(m);
        self.left(gone);
        if new {
            lock(&self.peak).1 += 1;
            self.change(user, BoxState::Queued, if wake { "wake queued" } else { "queued" });
        }
        let place = if at == 1 { "You’re next in line".to_owned() } else { format!("You’re number {at} in line") };
        Some(bad(format!("This grid is full right now. {place}: it starts as soon as there’s room."), 503))
    }

    /// Room freed: each one in line that may start now is started (in order, as many as there's room for, never
    /// more of one owner's sandboxes than the owner has room for).
    pub(super) fn drain(&self) {
        if self.is_closed() {
            return;
        }
        let Some(me) = self.me.upgrade() else { return };
        let now = self.clock.now_ms();
        let (ready, gone): (Vec<Waiting>, _) = {
            let mut m = lock(&self.inst);
            let mut q = lock(&self.queue);
            let gone = self.expire(&mut m, &mut q, now);
            let free = self.o.max_running.saturating_sub(m.values().filter(|i| occupies(i)).count());
            let ready = self.starters(&m, q.iter()).into_iter().take(free).cloned().collect();
            (ready, gone)
        };
        self.left(gone);
        for w in ready {
            let h = me.clone();
            tokio::spawn(async move { h.kick(w).await });
        }
    }

    /// Starts one from the line, under its gate, if it's still waiting (not removed, not started by its own caller).
    async fn kick(&self, w: Waiting) {
        let gate = self.gate(&w.user.id);
        let _one = gate.lock().await;
        if !lock(&self.queue).iter().any(|q| q.user.id == w.user.id) {
            return;
        }
        match self.start(&w.user, w.wake).await {
            // up: the ones behind it are counted again (one it held a place ahead of may start now)
            Ok(_) => self.drain(),
            // still no room: still in line, where it was
            Err(_) if self.state_of(&w.user.id) == BoxState::Queued => {}
            Err(e) => {
                tracing::warn!("hostd: {} from the line: {e}", w.user.handle);
                if w.wake {
                    self.wake_failed(&w.user, 0);
                }
            }
        }
    }

    fn state_of(&self, id: &str) -> BoxState {
        lock(&self.inst).get(id).map_or(BoxState::Stopped, |i| i.state)
    }

    /// Its place in line (1: next), while it waits for room.
    pub fn queued_at(&self, user_id: &str) -> Option<usize> {
        lock(&self.queue).iter().position(|w| w.user.id == user_id).map(|p| p + 1)
    }

    /// Why its last start didn't work, while it's asleep since (its waiting page says it, with Try again).
    pub fn last_error(&self, user_id: &str) -> Option<String> {
        lock(&self.inst).get(user_id).filter(|i| i.state == BoxState::Stopped && !i.error.is_empty()).map(|i| i.error.clone())
    }

    /// Why its start failed, if that was in the last `FAILED_LATELY_MS`: a page opened then says it with Try again
    /// (and doesn't start it again by itself: a box that can't start isn't restarted on every reload). Longer ago,
    /// opening it simply starts it again.
    pub fn failed_lately(&self, user_id: &str) -> Option<String> {
        let now = self.clock.now_ms();
        let m = lock(&self.inst);
        let i = m.get(user_id).filter(|i| i.failed_at.is_some_and(|at| now - at <= FAILED_LATELY_MS))?;
        (i.state == BoxState::Stopped && !i.error.is_empty()).then(|| i.error.clone())
    }

    /// Try again: its last failure forgotten at once (so a page polling `/_grid/state` while the new start waits for
    /// its gate never reads the old one back), before that start is asked for.
    pub fn forget_failure(&self, user_id: &str) {
        if let Some(i) = lock(&self.inst).get_mut(user_id).filter(|i| i.state == BoxState::Stopped) {
            i.error.clear();
            i.failed_at = None;
        }
    }
}

/// Puts `user` in line (people before wakes; a wake a person now asks for moves up with the people) → its place.
/// A person asking again keeps it in line (`seen`).
fn enqueue(q: &mut VecDeque<Waiting>, user: &BoxUser, wake: bool, now: i64) -> usize {
    if let Some(p) = q.iter().position(|w| w.user.id == user.id) {
        if !wake {
            q[p].seen = now;
        }
        if wake || !q[p].wake {
            return p + 1;
        }
        q.remove(p);
    }
    let at = if wake { q.len() } else { q.iter().position(|w| w.wake).unwrap_or(q.len()) };
    q.insert(at, Waiting { user: user.clone(), wake, seen: now });
    at + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(id: &str) -> BoxUser {
        BoxUser { id: id.into(), handle: id.into(), sandbox: None, owner: None }
    }

    #[test]
    fn people_wait_ahead_of_scheduled_wakes_and_a_wake_someone_opens_moves_up() {
        let mut q = VecDeque::new();
        assert_eq!(enqueue(&mut q, &u("w1"), true, 0), 1);
        assert_eq!(enqueue(&mut q, &u("p1"), false, 0), 1, "a person goes ahead of a wake");
        assert_eq!(enqueue(&mut q, &u("p2"), false, 0), 2);
        assert_eq!(enqueue(&mut q, &u("w2"), true, 0), 4);
        assert_eq!(enqueue(&mut q, &u("p1"), false, 0), 1, "asking again keeps its place");
        assert_eq!(enqueue(&mut q, &u("w2"), false, 0), 3, "opened by its person: up with the people");
        let order: Vec<_> = q.iter().map(|w| (w.user.id.as_str(), w.wake)).collect();
        assert_eq!(order, [("p1", false), ("p2", false), ("w2", false), ("w1", true)]);
    }
}
