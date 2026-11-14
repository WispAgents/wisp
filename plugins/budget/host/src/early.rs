//! A banked (early) reset (v1 index.js `app.resetEarly`, F-auto-12): a reading shows a window reset before its time, so
//! what waits on that account goes now, not at the old reset: the runs held here start, and `budget.plan_free` tells
//! wisp.later to bring due the follow-ups it holds (it sends the phone's one note, with the count; with later off,
//! budget sends it for the runs it started). Starting a held
//! turn is tried again on the next looks when the runtime refuses it.

use wisp_budget_api::PlanFree;
use wisp_budget_api::events::Reset;
use wisp_kernel::Ctx;
use wisp_runtime_api::commands as rt;
use wisp_runtime_api::{SessionId, TurnId};

use crate::{State, account_of, lock, now};

/// The plugin that sends the early-reset note when it's on (it counts its follow-ups in it).
const LATER: &str = "wisp.later";

/// wisp.later is there to hear `budget.plan_free` (on, or waiting to wake on an event).
fn later_on(ctx: &Ctx) -> bool {
    ctx.plugin_status().iter().any(|p| p.id == LATER && (p.state == "active" || p.state == "resolved"))
}

/// The phone hears that the runs held for `agent@account` start now (v1 `app.resetEarly`), when later can't say it.
async fn early_note(ctx: &Ctx, agent: &str, account: &str, n: u32) {
    let name = ctx
        .contributions(&wisp_runtime_api::harness::HARNESS)
        .iter()
        .find(|h| h.info().id == agent)
        .map_or_else(|| agent.to_owned(), |h| h.info().name.clone());
    let Some((title, text)) = wisp_budget_api::early_note(&name, account, n) else { return };
    let msg = wisp_channels_api::Msg { title, text, actions: Vec::new() };
    if let Err(err) = ctx.run::<wisp_channels_api::SendNotice>(&ctx.root_call(), wisp_channels_api::SendArgs { msg }).await {
        tracing::debug!(error = %err.message, "budget: an early reset, no phone to tell");
    }
}

/// A held turn to start: ((agent, account), (task, turn), the list it came from).
pub(crate) type Due = ((String, String), (String, String), Origin);

/// Which hold a turn to start came from, so one the runtime refuses goes back on that list: a stop line's waits for
/// its account to be free, Ask near the limit's for its window's reset (`until`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Stop,
    Ask { until: i64 },
}

/// A turn the runtime refused to start goes back where it came from.
pub(crate) fn put_back(due: Due, held: &mut crate::HeldTurns, asked: &mut Vec<crate::ask::Asked>) {
    let ((agent, account), (task, turn), from) = due;
    match from {
        Origin::Stop => held.entry((agent, account)).or_default().push((task, turn)),
        Origin::Ask { until } => asked.push(crate::ask::Asked { task, turn, until, agent, account }),
    }
}

/// How many looks a held turn the runtime refuses to start is tried again.
const RELEASE_TRIES: u8 = 5;

/// Held turns the runtime refused to start: tries per turn, and those to try again at the next look.
#[derive(Debug, Default)]
pub(crate) struct Retry {
    tries: std::collections::BTreeMap<String, u8>,
    again: Vec<Due>,
}

impl State {
    /// The turns `start` couldn't start wait again, from this look on (not the same look's `release`, which would
    /// spend their tries before the runtime could have written them held).
    pub(crate) fn requeue(&self) {
        let again = std::mem::take(&mut lock(&self.retry).again);
        let (mut held, mut asked) = (lock(&self.held), lock(&self.asked));
        for due in again {
            put_back(due, &mut held, &mut asked);
        }
    }

    /// Held turns start: → how many did. One the runtime refuses goes back to wait for the next look, a few times:
    /// a look can come between `before:run` holding a turn and the runtime writing it held ("that turn isn't
    /// waiting"), and dropping it then left it held for good.
    pub(crate) async fn start(&self, ctx: &Ctx, due: Vec<Due>) -> u32 {
        let mut n = 0;
        for (key, (id, turn), from) in due {
            let a = rt::ReleaseArgs { id: SessionId(id.clone()), turn_id: TurnId(turn.clone()), action: rt::ReleaseAction::Start };
            match ctx.run::<rt::Release>(&ctx.root_call(), a).await {
                Ok(_) => {
                    lock(&self.retry).tries.remove(&turn);
                    n += 1;
                }
                Err(err) => {
                    let again = {
                        let mut r = lock(&self.retry);
                        let tries = r.tries.entry(turn.clone()).or_insert(0);
                        *tries += 1;
                        let again = *tries <= RELEASE_TRIES;
                        if again {
                            r.again.push((key, (id.clone(), turn.clone()), from));
                        } else {
                            r.tries.remove(&turn);
                        }
                        again
                    };
                    tracing::warn!(error = %err.message, task = %id, turn = %turn, again, "budget: runtime.release");
                }
            }
        }
        n
    }

    /// Windows of `agent@account` reset before their time (a banked reset, v1 `app.resetEarly`): `budget.reset`, and
    /// when no window holds the account any more, what waits on it goes now: the runs held here start, and
    /// `budget.plan_free` brings due the follow-ups later holds for it (it sends the one phone note, with the count).
    pub(crate) async fn reset_early(&self, ctx: &Ctx, agent: &str, account: &str, ids: Vec<String>) {
        let account = account_of(account);
        let free = self.free(agent, &account, now(ctx));
        tracing::info!(agent, %account, ?ids, free, "budget: a window reset early");
        ctx.emit(&ctx.root_call(), &Reset { agent: agent.to_owned(), account: account.clone(), ids, free });
        if !free {
            return; // another window still holds it (the weekly one, say)
        }
        let key = (agent.to_owned(), account.clone());
        lock(&self.limited).remove(&key);
        let mut due: Vec<Due> =
            lock(&self.held).remove(&key).unwrap_or_default().into_iter().map(|t| (key.clone(), t, Origin::Stop)).collect();
        due.extend(self.ask_freed(agent, &account));
        let started = self.start(ctx, due).await;
        ctx.emit(&ctx.root_call(), &PlanFree { agent: agent.to_owned(), account: account.clone(), started });
        // wisp.later sends the one note (it adds the follow-ups it brings due); with later off, nobody would
        if !later_on(ctx) {
            early_note(ctx, agent, &account, started).await;
        }
    }

    /// Codex accounts work waits on (v1 `waitingOn`): runs held here, and accounts a run found at their limit.
    pub(crate) fn waiting(&self, at: i64) -> Vec<String> {
        let mut out: std::collections::BTreeSet<String> =
            lock(&self.held).keys().filter(|(g, _)| g == "codex").map(|(_, a)| a.clone()).collect();
        out.extend(lock(&self.asked).iter().filter(|a| a.agent == "codex").map(|a| a.account.clone()));
        let limited: Vec<String> = lock(&self.limited).iter().filter(|(g, _)| g == "codex").map(|(_, a)| a.clone()).collect();
        let (free, held): (Vec<String>, Vec<String>) = limited.into_iter().partition(|a| self.free("codex", a, at));
        {
            let mut l = lock(&self.limited);
            for a in free {
                l.remove(&("codex".to_owned(), a));
            }
        }
        out.extend(held);
        out.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round 177's review: a refused release of a run Ask near the limit held went back on the stop line's list, so
    /// it waited for the stop line (maybe never set) instead of its window's reset, and Check Codex lost it. It goes
    /// back where it came from; a stop line's goes back on its own list.
    #[test]
    fn a_refused_release_goes_back_on_the_list_it_came_from() {
        let key = ("codex".to_owned(), "work".to_owned());
        let (mut held, mut asked) = (crate::HeldTurns::new(), Vec::new());
        put_back((key.clone(), ("t1".into(), "u1".into()), Origin::Ask { until: 42 }), &mut held, &mut asked);
        assert!(held.is_empty(), "not on the stop line's list");
        assert_eq!(
            asked,
            vec![crate::ask::Asked { task: "t1".into(), turn: "u1".into(), until: 42, agent: "codex".into(), account: "work".into() }]
        );
        put_back((key.clone(), ("t2".into(), "u2".into()), Origin::Stop), &mut held, &mut asked);
        assert_eq!(held[&key], vec![("t2".to_owned(), "u2".to_owned())]);
        assert_eq!(asked.len(), 1);
        // taken off the Ask list again, it says where it came from
        let back = crate::ask::take(&mut asked, |_| true);
        assert_eq!(back, vec![(key, ("t1".to_owned(), "u1".to_owned()), Origin::Ask { until: 42 })]);
    }
}
