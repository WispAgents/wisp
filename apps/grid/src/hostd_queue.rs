//! hostd's line (v1 grid/hostd.js `#queue`): a start with no room waits here instead of failing.
//!
//! - **Room:** a box occupies a slot while it starts, runs or stops (its process or container is there); a start
//!   takes one only if fewer than `maxRunning` are taken by others and by the starts ahead of it in line. Counting
//!   and taking happen under one `inst` lock, so two cold starts never both see the last slot.
//! - **Per user:** a sandbox whose owner already has `maxRunningPerUser` boxes up (their own and their sandboxes)
//!   waits in line without holding anyone else up; the owner's own box is never held back by it.
//! - **Order:** people first, in the order they came; scheduled wakes behind them. A wake already waiting that a
//!   person then opens moves up with the people.
//! - **Moving:** whenever room frees (a stop, an exit, a failed start, a removal), each one in line that may start
//!   now is started, as its own caller would (`start` decides again under the lock). A removed box leaves the line.
use std::collections::{HashMap, VecDeque};

use super::{Inst, ProcessHost, lock};
use crate::boxhost::BoxState;
use crate::error::{GridError, bad};
use crate::rows::BoxUser;

/// One start waiting for room.
#[derive(Debug, Clone)]
pub(super) struct Waiting {
    pub user: BoxUser,
    /// a scheduled wake's: behind people, and it doesn't count as use
    pub wake: bool,
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

    /// A sandbox whose owner is at `maxRunningPerUser`.
    fn owner_full(&self, m: &HashMap<String, Inst>, u: &BoxUser) -> bool {
        u.owner.as_deref().is_some_and(|o| Self::owners_up(m, o) >= self.o.max_running_per_user)
    }

    /// Marks `user` Starting if there's room for it, else puts it in line (Queued) and says where: the 503 its
    /// caller hears.
    pub(super) fn admit_or_queue(&self, user: &BoxUser, wake: bool) -> Option<GridError> {
        let mut m = lock(&self.inst);
        let mut q = lock(&self.queue);
        let taken = m.iter().filter(|(id, i)| id.as_str() != user.id && occupies(i)).count();
        let mine = q.iter().position(|w| w.user.id == user.id);
        // the ones ahead that could start now: a sandbox held back by its owner's limit holds no one up
        // in line: the ones before it; new (or a wake a person now opens): every person, and for a wake everyone
        let moving_up = mine.is_some_and(|p| q[p].wake && !wake);
        let ahead = q
            .iter()
            .enumerate()
            .filter(|(at, w)| match mine {
                Some(p) if !moving_up => *at < p,
                _ => !w.wake || wake,
            })
            .filter(|(_, w)| w.user.id != user.id && !self.owner_full(&m, &w.user))
            .count();
        if !self.owner_full(&m, user) && taken + ahead < self.o.max_running {
            q.retain(|w| w.user.id != user.id);
            let i = m.entry(user.id.clone()).or_default();
            i.state = BoxState::Starting;
            i.user = Some(user.clone());
            i.error.clear();
            return None;
        }
        let new = mine.is_none();
        let at = enqueue(&mut q, user, wake);
        let i = m.entry(user.id.clone()).or_default();
        i.state = BoxState::Queued;
        i.user = Some(user.clone());
        drop(q);
        drop(m);
        if new {
            lock(&self.peak).1 += 1;
            self.change(user, BoxState::Queued, if wake { "wake queued" } else { "queued" });
        }
        let place = if at == 1 { "You’re next in line".to_owned() } else { format!("You’re number {at} in line") };
        Some(bad(format!("This grid is full right now. {place}: it starts as soon as there’s room."), 503))
    }

    /// Room freed: each one in line that may start now is started (in order, as many as there's room for).
    pub(super) fn drain(&self) {
        if self.is_closed() {
            return;
        }
        let Some(me) = self.me.upgrade() else { return };
        let ready: Vec<Waiting> = {
            let m = lock(&self.inst);
            let q = lock(&self.queue);
            let free = self.o.max_running.saturating_sub(m.values().filter(|i| occupies(i)).count());
            q.iter().filter(|w| !self.owner_full(&m, &w.user)).take(free).cloned().collect()
        };
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
        if let Err(e) = self.start(&w.user, w.wake).await {
            if self.state_of(&w.user.id) == BoxState::Queued {
                return; // still no room: still in line, where it was
            }
            tracing::warn!("hostd: {} from the line: {e}", w.user.handle);
            if w.wake {
                self.wake_failed(&w.user, 0);
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
}

/// Puts `user` in line (people before wakes; a wake a person now asks for moves up with the people) → its place.
fn enqueue(q: &mut VecDeque<Waiting>, user: &BoxUser, wake: bool) -> usize {
    if let Some(p) = q.iter().position(|w| w.user.id == user.id) {
        if wake || !q[p].wake {
            return p + 1;
        }
        q.remove(p);
    }
    let at = if wake { q.len() } else { q.iter().position(|w| w.wake).unwrap_or(q.len()) };
    q.insert(at, Waiting { user: user.clone(), wake });
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
        assert_eq!(enqueue(&mut q, &u("w1"), true), 1);
        assert_eq!(enqueue(&mut q, &u("p1"), false), 1, "a person goes ahead of a wake");
        assert_eq!(enqueue(&mut q, &u("p2"), false), 2);
        assert_eq!(enqueue(&mut q, &u("w2"), true), 4);
        assert_eq!(enqueue(&mut q, &u("p1"), false), 1, "asking again keeps its place");
        assert_eq!(enqueue(&mut q, &u("w2"), false), 3, "opened by its person: up with the people");
        let order: Vec<_> = q.iter().map(|w| (w.user.id.as_str(), w.wake)).collect();
        assert_eq!(order, [("p1", false), ("p2", false), ("w2", false), ("w1", true)]);
    }
}
