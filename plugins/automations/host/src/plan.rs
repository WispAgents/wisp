//! Runs a trigger started that wait for their plan (wisp.budget held the turn, `turn.held` by `wisp.budget`, kind
//! `plan`): the trigger's row says so (`held`, with why) until the plan frees (`budget.plan_free`, or
//! `automations.release` by hand; v1 index.js `app.triggers.release(agent)`), its held run starts (an on-time reset:
//! `turn.started`), or the run ends.

use wisp_automations_core::plan::{self, PlanWait};
use wisp_kernel::Ctx;
use wisp_runtime_api::commands as rt;

use crate::engine::{Engine, Shared};

/// A held run starts (an on-time reset, Start now): its row stops saying held; any other start is a set lookup.
pub fn on_started(s: &mut wisp_kernel::Setup, engine: &Shared) {
    let e = engine.clone();
    s.on::<wisp_runtime_api::events::TurnStarted, _, _>(move |_, t| {
        let e = e.clone();
        async move {
            e.plan_started(t.session_id.as_str()).await;
        }
    });
}

/// The plugin whose hold is the plan's, and its kind.
const BUDGET: (&str, &str) = ("wisp.budget", "plan");

impl Engine {
    /// `turn.held`: a run one of the triggers started waits for its plan; that trigger's row says so.
    pub async fn plan_held(&self, ctx: &Ctx, session: &str, by: &str, kind: &str, now: i64) {
        if (by, kind) != BUDGET {
            return;
        }
        let names: Vec<String> =
            self.states().await.into_iter().filter(|(_, st)| st.running.iter().any(|s| s == session)).map(|(n, _)| n).collect();
        if names.is_empty() {
            return; // not a trigger's run: no read
        }
        let (agent, account) = match ctx.run::<rt::Get>(&ctx.root_call(), rt::IdArgs { id: session.into() }).await {
            Ok(s) => (s.agent, s.account.unwrap_or_default()),
            Err(e) => {
                tracing::warn!(task = %session, "automations: runtime.get: {}", e.message);
                return;
            }
        };
        let why = format!("waits for {agent}'s plan (Automations stop at)");
        for name in names {
            let _one = self.gate.lock().await;
            let mut st = self.load(&name).await;
            let w = PlanWait { at: now, session: session.to_owned(), agent: agent.clone(), account: account.clone(), why: why.clone() };
            if plan::held(&mut st, w) {
                self.plan_runs().insert(session.to_owned());
                if let Err(e) = self.save(&name, &st).await {
                    tracing::warn!(trigger = %name, "state not saved: {}", e.message);
                }
            }
        }
    }

    pub(crate) fn plan_runs(&self) -> std::sync::MutexGuard<'_, std::collections::BTreeSet<String>> {
        self.plan_runs.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// At start: the runs rows already say held for (a restart keeps the rows, not the set).
    pub async fn seed_plan_runs(&self) {
        let held: Vec<String> = self.states().await.into_iter().filter_map(|(_, st)| st.plan.map(|w| w.session)).collect();
        self.plan_runs().extend(held);
    }

    /// `turn.started`: a held run that starts (its plan reset on time, or Start now) clears its row. Any other run's
    /// start is one set lookup, no row read. → how many rows.
    pub async fn plan_started(&self, session: &str) -> u32 {
        if !self.plan_runs().remove(session) {
            return 0;
        }
        let mut n = 0;
        for (name, st) in self.states().await {
            if st.plan.as_ref().is_none_or(|w| w.session != session) {
                continue;
            }
            let _one = self.gate.lock().await;
            let mut st = self.load(&name).await;
            if plan::started(&mut st, session) {
                n += 1;
                if let Err(e) = self.save(&name, &st).await {
                    tracing::warn!(trigger = %name, "state not saved: {}", e.message);
                }
            }
        }
        n
    }

    /// `agent@account`'s plan is free (an empty account: all of `agent`'s): the rows that waited for it stop saying
    /// so. The run itself is wisp.budget's to start. → how many rows.
    pub async fn release(&self, agent: &str, account: &str) -> u32 {
        let mut n = 0;
        for (name, st) in self.states().await {
            if st.plan.is_none() {
                continue;
            }
            let _one = self.gate.lock().await;
            let mut st = self.load(&name).await;
            let session = st.plan.as_ref().map(|w| w.session.clone());
            if plan::release(&mut st, agent, account) {
                n += 1;
                if let Some(s) = session {
                    self.plan_runs().remove(&s);
                }
                if let Err(e) = self.save(&name, &st).await {
                    tracing::warn!(trigger = %name, "state not saved: {}", e.message);
                }
            }
        }
        n
    }
}
