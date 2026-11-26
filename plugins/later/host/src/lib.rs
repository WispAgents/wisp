//! wisp.later host: follow-ups and reminders scheduled on tasks, plans that start a task at a time
//! (or after another task is done), and the agent's own wake-ups (`resume_later`). The decisions are
//! `wisp-later-core`'s; this crate keeps the rows (store.rs), reads tasks from wisp.runtime, asks
//! wisp.budget what holds an account back, and looks again on the kernel's clock.
//!
//! Times are stored in UTC and written in the user's IANA zone (`later.zone`, sent by wisp.settings from
//! the client), else the server's own (ADR-0025).
//!
//! resources: data-only (its rows are follow-ups and reminders scheduled on tasks: task data, kept with history (ADR-0029)).

mod calls;
mod egress;
mod store;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use serde_json::{Map, Value, json};
use wisp_kernel::{Call, Ctx, Manifest, Plugin, Result, Setup, WireError};
use wisp_later_api::commands as c;
use wisp_later_api::{BY_AGENT, LaterEntry, LaterHeld, PER_TICK, Plan, PlanKind, SentMeta, TICK_MS};
use wisp_later_core::{Action, PlanAction, Task, TaskMeta, due, due_plans, next_at, pull_forward, rules};
use wisp_store::Store;

/// A zone's offset from UTC at `at`, in minutes: the zone named, else the server's, else UTC.
fn offset_min(zone: Option<&str>, at: i64) -> i32 {
    let tz = zone.and_then(|z| jiff::tz::TimeZone::get(z).ok()).unwrap_or_else(jiff::tz::TimeZone::system);
    jiff::Timestamp::from_millisecond(at).map_or(0, |ts| tz.to_offset(ts).seconds() / 60)
}

struct State {
    store: Store,
    /// its kernel handle: `runtime.envmask@1`, for the ranges each row keeps as it is written
    ctx: Ctx,
    entries: Mutex<Vec<LaterEntry>>,
    plans: Mutex<Vec<Plan>>,
    /// due plans whose start is in flight (`start_due`): Start now, an edit or Cancel is refused (conflict) until it lands,
    /// rather than starting it twice or being dropped with it (audit doc23b). Taken under `plans`' lock, in that order.
    starting: Mutex<BTreeSet<String>>,
    metas: Mutex<HashMap<String, TaskMeta>>,
    /// the user's IANA zone, as the client sent it
    zone: Mutex<Option<String>>,
    /// one look at a time: a `later.tick` that lands while the scheduler's own look runs waits for it, so when it
    /// returns everything that came due has gone (else it saw the entries the other look had taken, and returned
    /// before their sends landed)
    looking: tokio::sync::Mutex<()>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

type Slot = Arc<OnceLock<Arc<State>>>;

fn state(slot: &Slot) -> Result<Arc<State>> {
    slot.get().cloned().ok_or_else(|| WireError::unavailable("wisp.later is starting"))
}

fn now(ctx: &Ctx) -> i64 {
    ctx.clock().map_or(0, |c| c.now_ms())
}

fn wire(e: wisp_later_core::LaterError) -> WireError {
    match e {
        wisp_later_core::LaterError::Invalid(m) => WireError::invalid(m),
        wisp_later_core::LaterError::NotFound(m) => WireError::not_found(m),
        wisp_later_core::LaterError::Conflict(m) => WireError::conflict(m),
    }
}

/// What wisp.later needs of a task, as wisp.runtime has it; None: it's gone.
/// Entries that went without being sent, other than your own (`later.dropped`): an agent's wake-up a goal waits for
/// hears why (v1 #775 wakeGone: `cancelled` checks the goal now; `done` or `missed` pauses it).
fn dropped(ctx: &Ctx, call: &Call, session_id: &str, gone: &[LaterEntry], why: &str) {
    for l in gone.iter().filter(|l| l.by != "user") {
        ctx.emit(call, &wisp_later_api::events::Dropped { session_id: session_id.to_owned(), entry: l.into(), why: why.to_owned() });
    }
}

/// The task, as wisp.runtime has it; None: it's gone, or couldn't be read (logged).
async fn task_of(ctx: &Ctx, call: &Call, id: &str) -> Option<Task> {
    calls::task(ctx, call, id).await.unwrap_or_else(|err| {
        tracing::warn!(error = %err.message, task = id, "later: runtime.get");
        None
    })
}

/// The task for a command that needs it: not found when it's gone, runtime's own refusal otherwise.
async fn need_task(ctx: &Ctx, call: &Call, id: &str, gone: &str) -> Result<Task> {
    calls::task(ctx, call, id).await?.ok_or_else(|| WireError::not_found(gone))
}

/// A write a look makes as it goes (logged when refused: the next look sees the rows as they are).
fn logged(r: Result<()>, what: &str) {
    if let Err(err) = r {
        tracing::warn!(error = %err.message, "later: {what}");
    }
}

/// What a look may decide on: the entries and `/after` plans of tasks runtime answered for (gone or not).
/// One it couldn't read would otherwise look gone, and its follow-ups forgotten, its plans lost.
fn readable(entries: Vec<LaterEntry>, plans: Vec<Plan>, unread: &BTreeSet<String>) -> (Vec<LaterEntry>, Vec<Plan>) {
    if unread.is_empty() {
        return (entries, plans);
    }
    let entries = entries.into_iter().filter(|l| !unread.contains(&l.session_id)).collect();
    let plans = plans.into_iter().filter(|p| p.after.as_ref().is_none_or(|a| !unread.contains(&a.id))).collect();
    (entries, plans)
}

impl State {
    /// The user's offset now (formatting and reading times happen around now).
    fn tz(&self, at: i64) -> i32 {
        offset_min(lock(&self.zone).as_deref(), at)
    }

    fn meta(&self, id: &str) -> TaskMeta {
        lock(&self.metas).get(id).cloned().unwrap_or_default()
    }

    fn of_task(&self, id: &str) -> Vec<LaterEntry> {
        lock(&self.entries).iter().filter(|l| l.session_id == id).cloned().collect()
    }

    async fn write(&self, ops: store::Ops) -> Result<()> {
        let m = self.ctx.use_service::<wisp_runtime_api::env::EnvMaskService>().ok();
        let rows = ops.rows(m.as_deref()).map_err(WireError::from)?;
        self.store.write(move |w| rows.apply(w)).await.map_err(WireError::from)
    }

    /// The task's ext slot, so its list row shows what is scheduled.
    async fn mirror(&self, ctx: &Ctx, id: &str) {
        let now = || match serde_json::to_value(self.meta(id).ext(&self.of_task(id))) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!(error = %e, task = id, "later: a task's ext");
                None
            }
        };
        calls::mirror(ctx, id, now).await;
    }

    async fn add(&self, ctx: &Ctx, call: &Call, a: &c::AddArgs) -> Result<LaterEntry> {
        // an agent's own follow-up is the agent's, whatever it says (audit I, smaller notes: an agent's `later.add` was
        // stored as the user's, so it read as theirs where it shows and fired as theirs)
        let mine;
        let a = if by_agent(call.caller()) && a.by.as_deref() != Some(BY_AGENT) {
            mine = c::AddArgs { by: Some(BY_AGENT.into()), ..a.clone() };
            &mine
        } else {
            a
        };
        let task = need_task(ctx, call, &a.session_id, "No such task.").await?;
        let pending = self.of_task(&a.session_id).len();
        let l = rules::new_entry(wisp_kernel::ids::uuid7(), &task, &self.meta(&a.session_id), pending, a, now(ctx), self.tz(now(ctx)))
            .map_err(wire)?;
        lock(&self.entries).push(l.clone());
        self.write(store::Ops { entries: vec![l.clone()], ..store::Ops::default() }).await?;
        self.mirror(ctx, &a.session_id).await;
        Ok(l)
    }

    fn take(&self, ids: &[String]) -> Vec<LaterEntry> {
        let mut all = lock(&self.entries);
        let (gone, keep): (Vec<LaterEntry>, Vec<LaterEntry>) = std::mem::take(&mut *all).into_iter().partition(|l| ids.contains(&l.id));
        *all = keep;
        gone
    }

    /// A follow-up goes into its task now. Sends before deleting the entry so a transient error never drops it.
    /// `sent` is recorded first: the turn the send starts (`turn.started`) can land before `follow_up` returns, and
    /// it must find the send to count as later's (audit doc23a: it found none, reset the chain, and the next turn,
    /// the user's, was counted as later's instead). A refused send takes it back.
    async fn fire(&self, ctx: &Ctx, call: &Call, l: &LaterEntry, late: bool) -> Result<wisp_runtime_api::commands::FollowedUp> {
        let sent = SentMeta { by: l.by.clone(), set_at: l.set_at, at: l.at, late };
        let note = rules::late_note(&sent, now(ctx), self.tz(now(ctx)));
        let text = format!("{note}\n\n{}", l.text);
        let sid = l.session_id.clone();
        let (was, m) = {
            let mut metas = lock(&self.metas);
            let m = metas.entry(sid.clone()).or_default();
            let was = m.sent.replace(sent.clone());
            (was, m.clone())
        };
        logged(self.write(store::Ops { metas: vec![(sid.clone(), m)], ..store::Ops::default() }).await, "a follow-up's send");
        let res = match calls::follow_up(ctx, call, &sid, text, &l.attachments).await {
            Ok(res) => res,
            Err(err) => {
                // only the send this made, and only if no turn took it meanwhile
                let back = lock(&self.metas).get_mut(&sid).filter(|m| m.sent.as_ref() == Some(&sent)).map(|m| {
                    m.sent = was;
                    m.clone()
                });
                if let Some(m) = back {
                    logged(self.write(store::Ops { metas: vec![(sid, m)], ..store::Ops::default() }).await, "a refused send");
                }
                return Err(err);
            }
        };
        // the entry only: the meta is as the turn left it (its reminder cleared, the send taken)
        logged(self.write(store::Ops { drop_entries: vec![l.id.clone()], ..store::Ops::default() }).await, "drop fired entry");
        Ok(res)
    }

    /// A reminder (or a missed follow-up) brings the task back to Needs you with its text.
    async fn remind(&self, ctx: &Ctx, call: &Call, task: &Task, l: &LaterEntry, text: String, missed: bool) -> Result<()> {
        let (reminder, new_state) = rules::reminder_of(task, l, text, missed);
        let mut m = self.meta(&l.session_id);
        m.reminder = Some(reminder);
        lock(&self.metas).insert(l.session_id.clone(), m.clone());
        self.write(store::Ops { drop_entries: vec![l.id.clone()], metas: vec![(l.session_id.clone(), m)], ..store::Ops::default() })
            .await?;
        if new_state != task.state {
            logged(calls::set_state(ctx, call, &l.session_id, new_state.as_str()).await, "a reminder's state");
        }
        calls::note(ctx, call, &l.session_id, l.text.clone()).await;
        Ok(())
    }

    /// Runs a due plan (task or reminder) or records a missed overdue task plan as a card instead of running it.
    async fn run_plan(&self, ctx: &Ctx, call: &Call, p: &Plan, missed: bool) -> Result<String> {
        let at = now(ctx);
        let tz = self.tz(at);
        if !missed && p.kind == PlanKind::Task {
            return calls::create(ctx, call, p.body.as_ref()).await;
        }
        let text = if missed && p.kind == PlanKind::Task {
            rules::missed_plan_text(p, at, tz)
        } else {
            p.text.clone().unwrap_or_else(|| p.title.clone().unwrap_or_default())
        };
        let title = rules::card_title(&text);
        let summary = rules::card_summary(p, missed && p.kind == PlanKind::Task, at, tz);
        let mut ext = BTreeMap::new();
        let meta = TaskMeta { card: true, origin: Some(rules::origin_of(p)), ..TaskMeta::default() };
        if let Ok(v) = serde_json::to_value(meta.ext(&[])) {
            ext.insert(wisp_later_api::PLUGIN_ID.to_owned(), v);
        }
        calls::card(ctx, call, title, summary, text, ext).await
    }

    /// A due plan starts (or, missed, becomes its card), and only then leaves, as `fire` does for a follow-up: it
    /// went first and was never run when the start failed (audit doc23b). A failed start keeps it, backing off
    /// (`rules::failed_start`), until it is lost to Drafts, where you see it.
    async fn start_due(&self, ctx: &Ctx, call: &Call, id: &str, missed: bool) -> bool {
        let p = {
            let plans = lock(&self.plans);
            let Some(p) = plans.iter().find(|p| p.id == id).cloned() else { return false };
            lock(&self.starting).insert(id.to_owned());
            p
        };
        let ran = self.run_plan(ctx, call, &p, missed).await;
        if let Err(err) = &ran {
            tracing::warn!(error = %err.message, plan = %id, tries = p.tries + 1, "later: plan run");
        }
        // what the store hears, decided under `plans` with the start mark cleared in the same hold
        let ops = {
            let mut plans = lock(&self.plans);
            lock(&self.starting).remove(id);
            if ran.is_ok() {
                plans.retain(|x| x.id != id);
                Some(store::Ops { drop_plans: vec![id.to_owned()], ..store::Ops::default() })
            } else {
                let next = rules::failed_start(&p, now(ctx));
                // gone or changed meanwhile (a restore, say): that one stands
                plans.iter_mut().find(|x| x.id == id && **x == p).map(|x| {
                    x.clone_from(&next);
                    store::Ops { plans: vec![next], ..store::Ops::default() }
                })
            }
        };
        if let Some(ops) = ops {
            logged(self.write(ops).await, if ran.is_ok() { "a plan" } else { "a failed plan" });
        }
        ran.is_ok()
    }

    /// `agent@account`'s plan reset early and nothing holds it (`budget.plan_free`): follow-ups held for it, or waiting
    /// at its limit, come due and go now; the phone hears how many tasks start, with the runs wisp.budget started.
    async fn plan_free(&self, ctx: &Ctx, e: &wisp_budget_api::PlanFree) -> u32 {
        let call = ctx.root_call();
        let mut ids: Vec<String> =
            lock(&self.entries).iter().filter(|l| l.held.is_some() || l.limit.is_some()).map(|l| l.session_id.clone()).collect();
        ids.sort();
        ids.dedup();
        let mut tasks = BTreeMap::new();
        for id in ids {
            match calls::task(ctx, &call, &id).await {
                Ok(Some(t)) => {
                    tasks.insert(id, t);
                }
                Ok(None) => {}
                Err(err) => tracing::warn!(error = %err.message, task = %id, "later: runtime.get"),
            }
        }
        let moved = {
            let mut all = lock(&self.entries);
            let moved = pull_forward(&all, &tasks, &e.agent, &e.account, now(ctx));
            for m in &moved {
                if let Some(l) = all.iter_mut().find(|l| l.id == m.id) {
                    *l = m.clone();
                }
            }
            moved
        };
        let n = u32::try_from(moved.len()).unwrap_or(u32::MAX);
        if !moved.is_empty() {
            let touched: BTreeSet<String> = moved.iter().map(|l| l.session_id.clone()).collect();
            logged(self.write(store::Ops { entries: moved, ..store::Ops::default() }).await, "an early reset");
            for id in &touched {
                self.mirror(ctx, id).await;
            }
            self.tick(ctx, &call).await;
        }
        calls::early_note(ctx, &e.agent, &e.account, e.started.saturating_add(n)).await;
        n
    }

    /// One look (v1 later.js tick): what came due goes, at most `PER_TICK` at once.
    #[allow(clippy::too_many_lines)] // one arm per decision, as v1's tick
    async fn tick(&self, ctx: &Ctx, call: &Call) -> u32 {
        let _one = self.looking.lock().await;
        let at = now(ctx);
        let entries = lock(&self.entries).clone();
        let plans = lock(&self.plans).clone();
        let mut ids: Vec<String> = entries.iter().map(|l| l.session_id.clone()).collect();
        ids.extend(plans.iter().filter_map(|p| p.after.as_ref().map(|a| a.id.clone())));
        ids.sort();
        ids.dedup();
        let mut tasks = BTreeMap::new();
        let mut holds: HashMap<(String, String), Option<LaterHeld>> = HashMap::new();
        // a task runtime couldn't read this look (starting, a failed read) isn't gone: what it has waits
        let mut unread = BTreeSet::new();
        for id in ids {
            let t = match calls::task(ctx, call, &id).await {
                Ok(Some(t)) => t,
                Ok(None) => continue,
                Err(err) => {
                    tracing::warn!(error = %err.message, task = %id, "later: runtime.get");
                    unread.insert(id);
                    continue;
                }
            };
            let key = (t.agent.clone(), t.account.clone());
            if let std::collections::hash_map::Entry::Vacant(slot) = holds.entry(key) {
                slot.insert(calls::hold(ctx, call, &t).await);
            }
            tasks.insert(id, t);
        }
        let (entries, plans) = readable(entries, plans, &unread);
        let d = due(&entries, &tasks, &mut |t| holds.get(&(t.agent.clone(), t.account.clone())).cloned().flatten(), at, PER_TICK);
        let mut fired = 0;
        let mut touched = Vec::new();
        for a in d.actions {
            match a {
                Action::Lapse { session_id, ids } => {
                    self.take(&ids);
                    logged(self.write(store::Ops { drop_entries: ids, ..store::Ops::default() }).await, "lapse");
                    touched.push(session_id);
                }
                Action::Forget { session_id } => {
                    let ids: Vec<String> = self.of_task(&session_id).into_iter().map(|l| l.id).collect();
                    self.take(&ids);
                    logged(self.write(store::Ops { drop_entries: ids, ..store::Ops::default() }).await, "forget");
                }
                Action::Hold { session_id, id, at: when, held } => {
                    let moved = {
                        let mut all = lock(&self.entries);
                        all.iter_mut().find(|l| l.id == id).map(|l| {
                            l.at = when;
                            l.held = Some(held);
                            l.clone()
                        })
                    };
                    if let Some(l) = moved {
                        logged(self.write(store::Ops { entries: vec![l], ..store::Ops::default() }).await, "hold");
                    }
                    touched.push(session_id);
                }
                Action::Fire { session_id, id, late } => {
                    if let Some(l) = self.take(std::slice::from_ref(&id)).pop() {
                        match self.fire(ctx, call, &l, late).await {
                            Ok(_) => fired += 1,
                            Err(err) => {
                                lock(&self.entries).push(l);
                                tracing::warn!(error = %err.message, task = %session_id, "later: a follow-up");
                            }
                        }
                        touched.push(session_id);
                    }
                }
                Action::Remind { session_id, id } => {
                    if let (Some(l), Some(t)) = (self.take(std::slice::from_ref(&id)).pop(), tasks.get(&session_id)) {
                        logged(self.remind(ctx, call, t, &l, l.text.clone(), false).await, "a reminder");
                        touched.push(session_id);
                    }
                }
                Action::Missed { session_id, id } => {
                    if let (Some(l), Some(t)) = (self.take(std::slice::from_ref(&id)).pop(), tasks.get(&session_id)) {
                        let text = rules::missed_text(&l, at, self.tz(at));
                        logged(self.remind(ctx, call, t, &l, text, true).await, "a missed follow-up");
                        dropped(ctx, call, &session_id, std::slice::from_ref(&l), "missed");
                        touched.push(session_id);
                    }
                }
            }
        }
        let pd = due_plans(&plans, &tasks, at, PER_TICK.saturating_sub(d.used));
        for a in pd.actions {
            match a {
                PlanAction::Run { id } => fired += u32::from(self.start_due(ctx, call, &id, false).await),
                PlanAction::Missed { id } => fired += u32::from(self.start_due(ctx, call, &id, true).await),
                PlanAction::Lose { id, why } => {
                    let lost = lock(&self.plans).iter().find(|p| p.id == id).map(|p| rules::lose(p, why));
                    if let Some(p) = lost {
                        lock(&self.plans).retain(|x| x.id != id);
                        lock(&self.plans).push(p.clone());
                        logged(self.write(store::Ops { plans: vec![p], ..store::Ops::default() }).await, "a lost plan");
                    }
                }
            }
        }
        touched.sort();
        touched.dedup();
        for id in touched {
            self.mirror(ctx, &id).await;
        }
        fired
    }
}

/// wisp.runtime's `runtime.envmask@1`: a list command's answer is masked as its rows are (ADR-0055).
fn mask(ctx: &Ctx) -> Option<Arc<dyn wisp_runtime_api::env::EnvMask>> {
    ctx.use_service::<wisp_runtime_api::env::EnvMaskService>().ok()
}

macro_rules! on {
    ($s:ident, $slot:ident, $C:ty, |$st:ident, $ctx:ident, $call:ident, $a:ident| $body:expr) => {{
        let slot = $slot.clone();
        $s.command::<$C, _, _>(move |$ctx, $call, $a| {
            let slot = slot.clone();
            async move {
                let $st = state(&slot)?;
                $body
            }
        });
    }};
}

#[allow(clippy::too_many_lines)] // one handler per v1 route
fn commands(s: &mut Setup, slot: &Slot) {
    on!(s, slot, c::Add, |st, ctx, call, a| st.add(&ctx, &call, &a).await);
    on!(s, slot, c::List, |st, ctx, _call, a| egress::masked(mask(&ctx).as_deref(), store::ENTRY.kind, st.of_task(&a.session_id)));
    on!(s, slot, c::Edit, |st, ctx, call, a| {
        let l = st
            .of_task(&a.session_id)
            .into_iter()
            .find(|l| l.id == a.item)
            .ok_or_else(|| WireError::not_found("Nothing scheduled with that id."))?;
        if a.now {
            st.take(std::slice::from_ref(&l.id));
            let res = st.fire(&ctx, &call, &l, false).await;
            if res.is_err() {
                lock(&st.entries).push(l);
            }
            st.mirror(&ctx, &a.session_id).await;
            let sent = res?;
            return serde_json::to_value(sent).map(c::Edited::Sent).map_err(|e| WireError::internal(e.to_string()));
        }
        let next = rules::edit_entry(&l, &a, now(&ctx), st.tz(now(&ctx))).map_err(wire)?;
        if let Some(x) = lock(&st.entries).iter_mut().find(|x| x.id == next.id) {
            *x = next.clone();
        }
        st.write(store::Ops { entries: vec![next.clone()], ..store::Ops::default() }).await?;
        st.mirror(&ctx, &a.session_id).await;
        Ok(c::Edited::Entry(next))
    });
    on!(s, slot, c::Cancel, |st, ctx, call, a| {
        let ids: Vec<String> = st
            .of_task(&a.session_id)
            .into_iter()
            .filter(|l| a.item.as_deref().is_none_or(|i| i == l.id) && (!a.goal_round || l.goal.is_some()))
            .map(|l| l.id)
            .collect();
        // `wisp later cancel <x>` for nothing said "ok true" (audit H7): a plan's id takes that plan back, anything
        // else is refused. A stale item from the app (already fired) still answers ok: its row is gone either way.
        if ids.is_empty() && a.item.is_none() && !a.goal_round {
            let plan = lock(&st.plans).iter().find(|p| p.id == a.session_id).map(|p| p.id.clone());
            let Some(plan) = plan else {
                return Err(WireError::not_found(format!(
                    "Nothing is scheduled on {}: wisp later plans lists the plans (wisp later cancel-plan <id> takes one back)",
                    a.session_id
                )));
            };
            lock(&st.plans).retain(|p| p.id != plan);
            st.write(store::Ops { drop_plans: vec![plan], ..store::Ops::default() }).await?;
            return Ok(c::Cancelled { ok: true, entry: None });
        }
        let gone = st.take(&ids);
        st.write(store::Ops { drop_entries: ids, ..store::Ops::default() }).await?;
        st.mirror(&ctx, &a.session_id).await;
        dropped(&ctx, &call, &a.session_id, &gone, "cancelled");
        Ok(c::Cancelled { ok: true, entry: gone.into_iter().next() })
    });
    on!(s, slot, c::Done, |st, ctx, call, a| {
        let (keep, gone) = rules::done_split(st.of_task(&a.session_id));
        let ids: Vec<String> = gone.into_iter().map(|l| l.id).collect();
        let taken = st.take(&ids);
        st.write(store::Ops { drop_entries: ids, ..store::Ops::default() }).await?;
        let _ = keep;
        st.mirror(&ctx, &a.session_id).await;
        dropped(&ctx, &call, &a.session_id, &taken, "done");
        Ok(c::Empty {})
    });
    on!(s, slot, c::Park, |_st, ctx, call, a| {
        let (on, at) = rules::park_of(a.for_later.as_ref(), now(&ctx));
        calls::for_later(&ctx, &call, &a.session_id, on, at).await?;
        if let Some(state) = &a.state {
            // Undo after Mark done puts the task's state back
            calls::set_state(&ctx, &call, &a.session_id, state).await?;
        }
        Ok(c::Parked {
            for_later: at.filter(|_| on).map(|at| wisp_later_api::ForLater { at, by: a.by.clone().unwrap_or_else(|| "you".into()) }),
        })
    });
    on!(s, slot, c::Plans, |st, ctx, _call, _a| egress::masked(mask(&ctx).as_deref(), store::PLAN.kind, lock(&st.plans).clone()));
    on!(s, slot, c::GetPlan, |st, ctx, _call, a| {
        let p = lock(&st.plans).iter().find(|p| p.id == a.id).cloned().ok_or_else(|| WireError::not_found("No such plan."))?;
        egress::masked(mask(&ctx).as_deref(), store::PLAN.kind, p)
    });
    on!(s, slot, c::MakePlan, |st, ctx, call, a| {
        let after = match &a.after {
            Some(v) => {
                let id = rules::after_id(v).map_err(wire)?;
                Some(need_task(&ctx, &call, &id, "No such task to wait for.").await?)
            }
            None => None,
        };
        let p =
            rules::new_plan(wisp_kernel::ids::uuid7(), &a, after.as_ref().map(|t| (t, st.meta(&t.id).card)), now(&ctx), st.tz(now(&ctx)))
                .map_err(wire)?;
        lock(&st.plans).push(p.clone());
        st.write(store::Ops { plans: vec![p.clone()], ..store::Ops::default() }).await?;
        Ok(p)
    });
    on!(s, slot, c::EditPlan, |st, ctx, call, a| {
        let p = {
            let mut plans = lock(&st.plans);
            let p = plans.iter().find(|p| p.id == a.id).cloned().ok_or_else(|| WireError::not_found("No such plan."))?;
            if lock(&st.starting).contains(&p.id) {
                return Err(WireError::conflict("This plan is starting now."));
            }
            if a.now {
                // taken before the await, so a look that comes due meanwhile doesn't find it and start it too
                plans.retain(|x| x.id != p.id);
            }
            p
        };
        if a.now {
            let started = match st.run_plan(&ctx, &call, &p, false).await {
                Ok(id) => id,
                Err(err) => {
                    lock(&st.plans).push(p);
                    return Err(err);
                }
            };
            logged(st.write(store::Ops { drop_plans: vec![p.id], ..store::Ops::default() }).await, "start plan now");
            return Ok(c::PlanEdited::Started(c::Started { session: c::StartedRef { id: started } }));
        }
        let next = rules::edit_plan(&p, &a, now(&ctx), st.tz(now(&ctx))).map_err(wire)?;
        if let Some(x) = lock(&st.plans).iter_mut().find(|x| x.id == next.id) {
            *x = next.clone();
        }
        st.write(store::Ops { plans: vec![next.clone()], ..store::Ops::default() }).await?;
        Ok(c::PlanEdited::Plan(Box::new(next)))
    });
    on!(s, slot, c::CancelPlan, |st, _ctx, _call, a| {
        {
            let mut plans = lock(&st.plans);
            if !plans.iter().any(|p| p.id == a.id) {
                return Err(WireError::not_found(format!("No plan has the id {}: wisp later plans lists them", a.id)));
            }
            if lock(&st.starting).contains(&a.id) {
                return Err(WireError::conflict("This plan is starting now."));
            }
            plans.retain(|p| p.id != a.id);
        }
        st.write(store::Ops { drop_plans: vec![a.id], ..store::Ops::default() }).await?;
        Ok(c::Okay { ok: true })
    });
    on!(s, slot, c::NextAtOf, |st, ctx, call, _a| {
        let entries = lock(&st.entries).clone();
        let plans = lock(&st.plans).clone();
        let mut tasks = BTreeMap::new();
        for l in &entries {
            if !tasks.contains_key(&l.session_id)
                && let Some(t) = task_of(&ctx, &call, &l.session_id).await
            {
                tasks.insert(l.session_id.clone(), t);
            }
        }
        Ok(c::NextAt { at: next_at(&entries, &plans, &tasks, now(&ctx)) })
    });
    on!(s, slot, c::Zone, |st, _ctx, _call, a| {
        let z = a.zone.trim();
        if !z.is_empty() && jiff::tz::TimeZone::get(z).is_err() {
            return Err(WireError::invalid(format!("{z} isn't a time zone wisp knows (an IANA name, like Europe/Zurich).")));
        }
        let zone = (!z.is_empty()).then(|| z.to_owned());
        lock(&st.zone).clone_from(&zone);
        let v = json!({ "zone": zone });
        st.store.write(move |w| store::put_conf(w, &v)).await.map_err(WireError::from)?;
        Ok(a)
    });
    on!(s, slot, c::Tick, |st, ctx, call, _a| Ok(c::Ticked { fired: st.tick(&ctx, &call).await }));
    on!(s, slot, c::WaitAtLimit, |st, ctx, call, a| {
        // the carry-on at a usage limit: a follow-up of wisp's own at the reset
        let add = c::AddArgs {
            session_id: a.session_id.clone(),
            text: Some(wisp_later_api::LIMIT_CARRY_ON.into()),
            when: c::When { at: Some(json!(a.at.max(now(&ctx) + wisp_later_api::MIN_MS))), in_min: None },
            by: Some(wisp_later_api::BY_WISP.into()),
            limit: a.agent.clone().map(|agent| wisp_later_api::LimitRef { agent, account: String::new() }),
            ..c::AddArgs::default()
        };
        Ok(st.add(&ctx, &call, &add).await.ok())
    });
    on!(s, slot, c::AgentSchedule, |st, ctx, call, a| {
        // the `resume_later` tool: the agent's own wake-up, or a cancel
        if let Some(id) = a.cancel.as_deref().filter(|c| !c.is_empty()) {
            let ids: Vec<String> = st.of_task(&a.session).into_iter().filter(|l| l.id == id && l.by == BY_AGENT).map(|l| l.id).collect();
            if ids.is_empty() {
                return Ok(c::ScheduleOut { text: None, error: Some(format!("Nothing of yours is scheduled with id {id}.")) });
            }
            st.take(&ids);
            st.write(store::Ops { drop_entries: ids, ..store::Ops::default() }).await?;
            st.mirror(&ctx, &a.session).await;
            return Ok(c::ScheduleOut { text: Some(format!("Cancelled {id}.")), error: None });
        }
        if let Some(at) = a.at.as_deref()
            && !rules::agent_at_ok(at)
        {
            return Ok(c::ScheduleOut {
                text: None,
                error: Some("`at` must be an ISO time with its offset (2026-11-08T17:00:00Z).".into()),
            });
        }
        let add = c::AddArgs {
            session_id: a.session.clone(),
            text: a.message.clone(),
            when: c::When { at: a.at.clone().map(Value::String), in_min: a.minutes.clone() },
            by: Some(BY_AGENT.into()),
            ..c::AddArgs::default()
        };
        match st.add(&ctx, &call, &add).await {
            Ok(l) => {
                let task = task_of(&ctx, &call, &a.session).await;
                let minutes = (l.at - now(&ctx)) / 60_000;
                let props = Map::from_iter([("agent".to_owned(), json!(task.map(|t| t.agent))), ("minutes".to_owned(), json!(minutes))]);
                calls::track(&ctx, "resume_later", props).await;
                let others: Vec<LaterEntry> = st.of_task(&a.session).into_iter().filter(|x| x.id != l.id).collect();
                Ok(c::ScheduleOut { text: Some(rules::scheduled_text(&l, &others, now(&ctx), st.tz(now(&ctx)))), error: None })
            }
            Err(e) => Ok(c::ScheduleOut { text: None, error: Some(e.message) }),
        }
    });
}

#[derive(Debug, Default)]
pub struct LaterPlugin;

impl Plugin for LaterPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let slot: Slot = Arc::new(OnceLock::new());
        commands(s, &slot);
        s.contribute(&wisp_kernel::egress::VIEWS, Arc::new(egress::View(s.ctx())) as Arc<dyn wisp_kernel::egress::RowView>);
        // an early (banked) reset freed a plan: what waits on it comes due now (v1 app.resetEarly)
        let freed = slot.clone();
        s.on::<wisp_budget_api::PlanFree, _, _>(move |ctx, e| {
            let slot = freed.clone();
            async move {
                if let Ok(st) = state(&slot) {
                    st.plan_free(&ctx, &e).await;
                }
            }
        });
        // A new turn accounts for a fired follow-up (chain count), clears reminder & for_later (rules::on_turn).
        let started = slot.clone();
        s.on::<wisp_runtime_api::events::TurnStarted, _, _>(move |ctx, e| {
            let slot = started.clone();
            async move {
                let Ok(st) = state(&slot) else { return };
                let sid = e.session_id.as_str();
                let prev = st.meta(sid);
                let by = prev.sent.as_ref().map(|_| "later");
                let (next, _) = rules::on_turn(&prev, by);
                if next != prev {
                    lock(&st.metas).insert(sid.to_owned(), next.clone());
                    logged(st.write(store::Ops { metas: vec![(sid.to_owned(), next)], ..store::Ops::default() }).await, "on turn");
                    st.mirror(&ctx, sid).await;
                }
            }
        });
        // A deleted task drops its scheduled entries and meta immediately rather than waiting for the next tick.
        let deleted = slot.clone();
        s.on::<wisp_runtime_api::events::SessionDeleted, _, _>(move |_ctx, e| {
            let slot = deleted.clone();
            async move {
                let Ok(st) = state(&slot) else { return };
                let sid = e.id.as_str();
                let ids: Vec<String> = st.of_task(sid).into_iter().map(|l| l.id).collect();
                st.take(&ids);
                lock(&st.metas).remove(sid);
                if !ids.is_empty() {
                    logged(st.write(store::Ops { drop_entries: ids, ..store::Ops::default() }).await, "session deleted");
                }
            }
        });
        let start = slot;
        s.on_start(move |ctx| async move {
            let store = ctx.service::<Store>()?.as_ref().clone();
            store::install(&store).map_err(WireError::from)?;
            let (entries, plans, metas) = store::load(&store).await.map_err(WireError::from)?;
            let zone = store::load_conf(&store).await.map_err(WireError::from)?.get("zone").and_then(Value::as_str).map(str::to_owned);
            let st = Arc::new(State {
                store,
                ctx: ctx.clone(),
                entries: Mutex::new(entries),
                plans: Mutex::new(plans),
                metas: Mutex::new(metas.into_iter().collect()),
                zone: Mutex::new(zone),
                looking: tokio::sync::Mutex::new(()),
                starting: Mutex::new(BTreeSet::new()),
            });
            let _ = start.set(st.clone());
            // the zone the client sent with its settings wins over the one later kept
            let zone_of = |v: Option<Value>| v.and_then(|v| v.as_str().map(str::to_owned)).filter(|z| !z.is_empty());
            if let Some(z) = zone_of(ctx.setting("settings.timeZone")) {
                *lock(&st.zone) = Some(z);
            }
            let st2 = st.clone();
            ctx.watch_setting("settings.timeZone", move |v| {
                if let Some(z) = zone_of(v) {
                    *lock(&st2.zone) = Some(z);
                }
            });
            // the scheduler: a look every `TICK_MS` on the kernel's clock (a test's FakeClock drives it)
            if let Ok(clock) = ctx.clock() {
                let (scope, c2) = (ctx.scope().clone(), ctx.clone());
                ctx.spawn(async move {
                    loop {
                        st.tick(&c2, &c2.root_call()).await;
                        tokio::select! {
                            () = scope.cancelled() => return,
                            () = clock.sleep(u64::try_from(TICK_MS).unwrap_or(20_000)) => {}
                        }
                    }
                });
            }
            Ok(())
        });
        Ok(())
    }
}

/// Whether a call comes from a task's agent (its token: `agent:session…`, `agent:capped…`, a crew member's).
fn by_agent(c: &wisp_kernel::Caller) -> bool {
    c.session.is_some() && (c.set.starts_with("agent:") || c.set.starts_with("crew:"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_follow_the_zone_and_its_daylight_saving() {
        // 2026-01-15 and 2026-07-15, noon UTC
        let (winter, summer) = (1_768_478_400_000, 1_784_116_800_000);
        assert_eq!((offset_min(Some("Europe/Zurich"), winter), offset_min(Some("Europe/Zurich"), summer)), (60, 120));
        assert_eq!(offset_min(Some("America/New_York"), winter), -300);
        assert_eq!(offset_min(Some("UTC"), summer), 0);
        assert_eq!(offset_min(Some("Not/AZone"), winter), offset_min(None, winter), "an unknown zone falls back to the server's");
    }

    #[test]
    fn manifest_is_valid() {
        assert_eq!(LaterPlugin.manifest().id, "wisp.later");
    }
}
