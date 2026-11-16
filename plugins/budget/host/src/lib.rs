//! wisp.budget host: the usage windows each account reported (a run's `turn.limits`, a live read),
//! Automations stop at, and the `before:run` guard: unattended work (crew, triggers, goals, agents)
//! on an account past its stop line is held, and starts once the account is free again (its windows
//! reset, or a reading says they reset early). The user's own runs are never held.

mod ask;
mod drain;
mod early;
mod kinds;
mod live;
mod store;
mod usage;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use serde_json::{Value, json};
use wisp_budget_api::commands as c;
use wisp_budget_api::{PlanHold, settings};
use wisp_budget_core::Budget;
use wisp_budget_core::limits::report_from_value;
use wisp_kernel::services::Paths;
use wisp_kernel::{Ctx, Failure, InterceptOpts, Manifest, Plugin, Result, Setup, Verdict, WireError};
use wisp_runtime_api::commands as rt;
use wisp_runtime_api::events::{TurnEnded, TurnLimits, TurnStarted};
use wisp_runtime_api::hooks::{BeforeCreate, BeforeFollowUp, BeforeRun, BeforeRunCtx};
use wisp_runtime_api::model::Held;
use wisp_runtime_api::{SessionId, TurnId};
use wisp_store::Store;

pub const PLUGIN_ID: &str = "wisp.budget";
/// The `held.kind` of a turn the plan holds.
pub const HOLD_KIND: &str = "plan";
const STOP_AT: [&str; 4] = ["off", "50", "75", "90"];

/// Each turn's plan shares so far: turn → (last touched, task, shares).
type TurnPlans = BTreeMap<String, (u64, String, Vec<wisp_budget_api::Share>)>;

/// Turns held per (agent, account): (task, turn).
pub(crate) type HeldTurns = BTreeMap<(String, String), Vec<(String, String)>>;

struct State {
    store: Store,
    budget: Mutex<Budget>,
    settings: Mutex<Value>,
    held: Mutex<HeldTurns>,
    /// held turns the runtime refused to start: how often, and those to try at the next look
    retry: Mutex<early::Retry>,
    /// Drain watch's notes sent, per window
    notes: Mutex<wisp_budget_core::drain::DrainNotes>,
    /// Ask near the limit: the runs it learns from, per harness
    runs: Mutex<ask::Runs>,
    /// `runs` learned something not saved yet: the next save writes them with the spend (one commit a turn's end)
    runs_dirty: std::sync::atomic::AtomicBool,
    /// runs going now: turn → (agent, account, their likely units)
    running: Mutex<BTreeMap<String, (String, String, f64)>>,
    /// automations Ask near the limit holds
    asked: Mutex<Vec<ask::Asked>>,
    /// accounts a run found at their usage limit (`budget.resetAt` said when): a task may wait for them (Wait for the
    /// reset), so Check Codex reads them too, until they're free
    limited: Mutex<std::collections::BTreeSet<(String, String)>>,
    /// the account each turn is about to run on (`before:run`), until it starts: the ledger begins it then
    starting: Mutex<BTreeMap<String, (String, String)>>,
    /// each turn's plan shares so far (v1 turn.plan), the newest `MAX_PLANS` turns: written to its `wisp.budget` slot
    plans: Mutex<TurnPlans>,
    plan_seq: std::sync::atomic::AtomicU64,
}

/// turns whose plan shares are kept in memory to add to (a share lands during the run or with the next report)
const MAX_PLANS: usize = 500;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

type Slot = Arc<OnceLock<Arc<State>>>;

fn state(slot: &Slot) -> Result<Arc<State>> {
    slot.get().cloned().ok_or_else(|| WireError::unavailable("wisp.budget is starting"))
}

fn now(ctx: &Ctx) -> i64 {
    ctx.clock().map_or(0, |c| c.now_ms())
}

fn account_of(a: &str) -> String {
    if a.is_empty() { "default".into() } else { a.to_owned() }
}

/// v1 `stopFor`: the harness's own line, else the one for every harness; 'off' (or none) is no line.
fn stop_for(settings: &Value, agent: &str) -> Option<f64> {
    let own = match agent {
        "claude" => settings.get(settings::AUTO_STOP_CLAUDE),
        "codex" => settings.get(settings::AUTO_STOP_CODEX),
        _ => None,
    }
    .and_then(Value::as_str)
    .filter(|v| STOP_AT.contains(v));
    let v = own.or_else(|| settings.get(settings::AUTO_STOP).and_then(Value::as_str))?;
    (v != "off").then(|| v.parse::<f64>().ok().map(|p| p / 100.0)).flatten()
}

impl State {
    fn hold(&self, agent: &str, account: &str, at: i64) -> Option<PlanHold> {
        let stop = stop_for(&lock(&self.settings), agent);
        lock(&self.budget).hold(agent, &account_of(account), stop, at)
    }

    fn free(&self, agent: &str, account: &str, at: i64) -> bool {
        let stop = stop_for(&lock(&self.settings), agent);
        lock(&self.budget).free(agent, &account_of(account), stop, at)
    }

    async fn save(&self) {
        self.save_with(None).await;
    }

    /// `save`, and `ob`'s events go out with its commit (ADR-0074): whoever reads the saved spend finds them queued.
    /// With nothing to save they still go out, with an empty write; a save that fails publishes none of them.
    async fn save_with(&self, ob: Option<wisp_kernel::Outbox>) {
        use std::sync::atomic::Ordering::SeqCst;
        let dirty = lock(&self.budget).take_dirty();
        let ob = ob.filter(|o| !o.is_empty());
        // the run costs Ask near the limit learned ride the same write (ADR-0088): a turn's end commits once
        let runs = self.runs_dirty.swap(false, SeqCst).then(|| lock(&self.runs).clone());
        if dirty.is_empty() && ob.is_none() && runs.is_none() {
            return;
        }
        let (n, learned) = (dirty.len(), runs.is_some());
        let job = move |w: &mut wisp_store::Writer<'_>| {
            dirty.iter().try_for_each(|(g, a, acc)| store::put_acc(w, g, a, acc))?;
            runs.as_ref().map_or(Ok(()), |r| store::put_runs(w, r))
        };
        let r = match ob {
            Some(ob) => self.store.write_with(ob, job).await,
            None => self.store.write(job).await,
        };
        if let Err(e) = r {
            if learned {
                // tried again with the next save
                self.runs_dirty.store(true, SeqCst);
            }
            tracing::warn!(accounts = n, learned, "couldn't save the budget's spend per account: {e}");
        }
    }

    /// Held work whose account is free starts now.
    async fn release(&self, ctx: &Ctx) {
        let at = now(ctx);
        let due: Vec<early::Due> = {
            let mut held = lock(&self.held);
            let keys: Vec<(String, String)> = held.keys().filter(|(g, a)| self.free(g, a, at)).cloned().collect();
            let stop = |k: &(String, String), t| (k.clone(), t, early::Origin::Stop);
            let mut due: Vec<early::Due> =
                keys.iter().filter_map(|k| held.remove(k).map(|v| v.into_iter().map(|t| stop(k, t)))).flatten().collect();
            due.extend(self.ask_due(at));
            due
        };
        self.start(ctx, due).await;
    }

    /// Where a window's move went (v1 budget.js addShare → turn.plan): each turn's shares, added to what it has, on
    /// its own turn (`ext["wisp.budget"].plan`), so Compare's Plan row and the receipt read them.
    async fn keep_shares(&self, ctx: &Ctx, shares: &[(String, String, wisp_budget_api::Share)]) {
        let mut writes = Vec::new();
        {
            let mut plans = lock(&self.plans);
            for (sid, tid, share) in shares {
                let seq = self.plan_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let e = plans.entry(tid.clone()).or_insert_with(|| (seq, sid.clone(), Vec::new()));
                e.0 = seq;
                wisp_budget_core::where_went::add_share(&mut e.2, share);
                writes.push((sid.clone(), tid.clone(), e.2.clone()));
            }
            while plans.len() > MAX_PLANS {
                let Some(old) = plans.iter().min_by_key(|(_, v)| v.0).map(|(k, _)| k.clone()) else { break };
                plans.remove(&old);
            }
        }
        writes.dedup_by(|a, b| a.1 == b.1);
        for (sid, tid, plan) in writes {
            let a = rt::TurnExtArgs {
                id: SessionId(sid.clone()),
                turn_id: TurnId(tid.clone()),
                plugin: PLUGIN_ID.into(),
                value: json!({ "plan": plan }),
            };
            if let Err(err) = ctx.run::<rt::TurnSetExt>(&ctx.root_call(), a).await {
                tracing::warn!(error = %err.message, task = %sid, turn = %tid, "budget: runtime.turn.setExt");
            }
        }
    }

    async fn record(&self, ctx: &Ctx, agent: &str, account: &str, report: &wisp_budget_api::LimitsReport) -> c::RecordOut {
        let r = lock(&self.budget).record(agent, &account_of(account), report, now(ctx));
        // an early reset goes out with the save that records it (ADR-0074)
        let ob = ctx.outbox();
        let early = (!r.early.is_empty()).then(|| self.reset_event(ctx, agent, account, r.early.clone()));
        if let Some(e) = &early {
            ob.emit(e);
        }
        self.save_with(Some(ob)).await;
        self.keep_shares(ctx, &r.shares).await;
        if let Some(e) = early {
            self.reset_early(ctx, &e).await;
        }
        self.release(ctx).await;
        c::RecordOut { past: r.past }
    }
}

/// The harness Auto would run a New task on (`router.preview`: the same pick every time), or None when it can't say
/// (no router, nothing to pick): the send goes on to its turn's own guard.
async fn auto_pick(ctx: &Ctx, a: &wisp_runtime_api::commands::StartArgs) -> Option<String> {
    let goal = a.goal.as_ref().and_then(|g| g.as_str().or_else(|| g.get("text").and_then(Value::as_str))).map(str::to_owned);
    let args = wisp_router_api::view::PreviewArgs {
        prompt: Some(a.prompt.clone()),
        #[allow(clippy::cast_precision_loss)]
        attachments: Some(a.attachments.len() as f64),
        goal,
        team: Some(a.team.as_ref().is_some_and(|t| !t.is_null())),
        machine: None,
    };
    match ctx.run::<wisp_router_api::commands::Preview>(&ctx.root_call(), args).await.ok()? {
        wisp_router_api::view::PreviewResult::Pick(p) => Some(p.pick.agent).filter(|x| !x.is_empty() && x != "auto"),
        wisp_router_api::view::PreviewResult::Nothing(_) => None,
    }
}

/// `before:run`: unattended work on an account past its stop line is held until it resets.
/// Ask near the limit before a send is made (v1: the page's `holdNow`, the server's 409 `hold`): a New task or a
/// follow-up you send that would take its window past your line is refused with `budget_hold` before the task or turn
/// exists, so nothing shows as failed and the box keeps your words; the composer asks `budget.ask` for the row. A
/// send by an automation, a subagent or an answer to the task's question goes on to the turn's own guard (held there
/// until the reset); one wisp routes (`auto`) is asked about the harness Auto's preview picks (`auto_pick`).
fn sends(s: &mut Setup, slot: &Slot) {
    let opts = InterceptOpts { priority: 30, failure: Failure::Open, timeout_ms: 500 };
    let create = slot.clone();
    s.intercept::<BeforeCreate, _, _>(opts, move |ctx, _call, a| {
        let slot = create.clone();
        async move {
            let Ok(st) = state(&slot) else { return Verdict::Continue };
            // a subagent's, an automation's, or one made held (it starts later: its turn's own guard asks then)
            if a.parent_id.is_some() || a.hold || wisp_budget_core::windows::unattended(a.by.as_deref()) {
                return Verdict::Continue;
            }
            let mut agent = a.agent.clone().unwrap_or_default();
            // Auto: the harness its preview picks (the composer asks the same), as v1 resolved Auto before it asked;
            // its first turn picks again, and a yes here (confirmLimit) goes with it (B15)
            if agent == "auto" {
                agent = auto_pick(&ctx, &a).await.unwrap_or_default();
            }
            let account = a.account.clone().unwrap_or_default();
            let send = ask::Send { agent: &agent, account: &account, session: "", first: true, together: 1, confirm: a.confirm_limit };
            match st.ask_send(&ctx, &send) {
                Some(h) => Verdict::Veto { code: wisp_budget_api::VETO_CODE.into(), message: h.why, then: Vec::new() },
                None => Verdict::Continue,
            }
        }
    });
    let follow = slot.clone();
    s.intercept::<BeforeFollowUp, _, _>(opts, move |ctx, _call, a| {
        let slot = follow.clone();
        async move {
            let Ok(st) = state(&slot) else { return Verdict::Continue };
            if wisp_budget_core::windows::unattended(a.by.as_deref()) {
                return Verdict::Continue;
            }
            let Ok(task) =
                ctx.run::<wisp_runtime_api::commands::Get>(&ctx.root_call(), wisp_runtime_api::commands::IdArgs { id: a.id.clone() }).await
            else {
                return Verdict::Continue;
            };
            // an answer to what the task asks, or a run inside a task you already said yes to
            if task.parent_id.is_some() || task.state == wisp_runtime_api::model::SessionState::Waiting {
                return Verdict::Continue;
            }
            let agent = a.run.as_ref().and_then(|r| r.agent.clone()).unwrap_or(task.agent);
            let account = task.account.unwrap_or_default();
            let send =
                ask::Send { agent: &agent, account: &account, session: a.id.as_str(), first: false, together: 1, confirm: a.confirm_limit };
            match st.ask_send(&ctx, &send) {
                Some(h) => Verdict::Veto { code: wisp_budget_api::VETO_CODE.into(), message: h.why, then: Vec::new() },
                None => Verdict::Continue,
            }
        }
    });
}

fn guard(ctx: &Ctx, st: &State, mut c: BeforeRunCtx) -> Verdict<BeforeRunCtx> {
    lock(&st.starting).insert(c.turn_id.0.clone(), (c.agent.clone(), account_of(c.account.as_deref().unwrap_or(""))));
    let subject = wisp_budget_core::subject_of(&c.agent, c.account.as_deref(), c.by.as_deref());
    let at = now(ctx);
    let Some(h) = wisp_budget_core::decide(&subject, |g, a| st.hold(g, a, at)) else { return st.ask(ctx, c).unwrap_or(Verdict::Continue) };
    let why = format!("{} is past {:.0}% of its {} window: unattended work waits for it to reset.", c.agent, h.stop * 100.0, h.label);
    lock(&st.held).entry((h.agent.clone(), h.account.clone())).or_default().push((c.session_id.0.clone(), c.turn_id.0.clone()));
    c.hold = Some(Held { by: PLUGIN_ID.into(), kind: HOLD_KIND.into(), until: Some(h.until), data: json!({ "hold": h, "why": why }) });
    Verdict::Modify { ctx: c }
}

/// A turn's start and end: the ledger knows which turns ran on an account while its windows moved (they share the
/// move, v1 #sharers; the account is the one `before:run` saw); an end also feeds Drain watch and Ask near the limit.
fn on_turns(s: &mut Setup, slot: &Slot) {
    // the ledger knows which turns ran on an account while its windows moved: they share the move (v1 #sharers)
    let started = slot.clone();
    s.on::<TurnStarted, _, _>(move |ctx, e| {
        let slot = started.clone();
        async move {
            if let Ok(st) = state(&slot)
                && let Some((agent, account)) = lock(&st.starting).remove(e.turn_id.as_str())
            {
                lock(&st.budget).begin(&agent, &account, e.session_id.as_str(), e.turn_id.as_str(), now(&ctx));
            }
        }
    });
    let ended = slot.clone();
    s.on::<TurnEnded, _, _>(move |ctx, e| {
        let slot = ended.clone();
        async move {
            if let Ok(st) = state(&slot) {
                lock(&st.budget).end(e.session_id.as_str(), e.turn_id.as_str(), now(&ctx));
                usage::note_turn(&ctx, &st, &e);
                // learned first, so the spend's save below writes the runs too: one commit, not two (ADR-0088)
                if ask::learn(&mut lock(&st.runs), &e, now(&ctx)) {
                    st.runs_dirty.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                st.ended(&ctx, &e).await;
                lock(&st.running).remove(e.turn_id.as_str());
                // a turn `ended` saved nothing for (no spend) learned nothing either; anything left goes now
                if st.runs_dirty.load(std::sync::atomic::Ordering::SeqCst) {
                    st.save().await;
                }
            }
        }
    });
}

macro_rules! on {
    ($s:ident, $slot:ident, $C:ty, |$st:ident, $ctx:ident, $a:ident| $body:expr) => {{
        let slot = $slot.clone();
        $s.command::<$C, _, _>(move |$ctx, _call, $a| {
            let slot = slot.clone();
            async move {
                let $st = state(&slot)?;
                $body
            }
        });
    }};
}

/// How long a read waits for the turns that ended before it to be counted (ADR-0073): the handler is a few store
/// writes; one stuck on a Drain watch notice doesn't hold Settings → Usage or the picker longer than this.
const EVENTS_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// The budget's commands: view, hold, reset, record, dismiss and the auto-stop lines.
fn commands(s: &mut Setup, slot: &Slot) {
    on!(s, slot, c::View, |st, ctx, a| {
        // the turns a client saw end are in what it reads (their drain, their Where it went): ADR-0073
        ctx.events_delivered(EVENTS_WAIT).await;
        let mut v = lock(&st.budget).view(&a.agent, now(&ctx));
        // v1 meta.js:63: the drain card with Drain watch on, the notices unless they're switched off
        let (drain, notices) = (
            drain::watch_on(ctx.setting(drain::WATCH).as_ref(), &lock(&st.settings)),
            ctx.setting(drain::NOTICE).and_then(|v| v.as_bool()).unwrap_or(true),
        );
        for acc in &mut v {
            drain::shown(acc, drain, notices);
        }
        if a.named {
            usage::named(&ctx, &mut v).await;
        } else {
            for x in &mut v {
                x.where_went = None;
            }
        }
        Ok(v)
    });
    // tokens per turn by account counts every turn a client saw end, the last one too (ADR-0073; v1 read the sessions)
    on!(s, slot, c::Compare, |st, ctx, a| {
        ctx.events_delivered(EVENTS_WAIT).await;
        Ok(lock(&st.budget).compare_accounts(&a.agent, now(&ctx)))
    });
    on!(s, slot, c::PlanWindows, |st, ctx, a| {
        let only: Option<std::collections::BTreeSet<String>> = a.tasks.map(|t| t.into_iter().collect());
        Ok(lock(&st.budget).plan_windows(only.as_ref(), now(&ctx)))
    });
    on!(s, slot, c::Hold, |st, ctx, a| Ok(st.hold(&a.agent, &a.account, now(&ctx))));
    // Ask near the limit on a send not made yet: the hold row's numbers (the composer, after a `budget_hold` refusal)
    on!(s, slot, c::Ask, |st, ctx, a| {
        let first = a.session.is_none();
        let session = a.session.unwrap_or_default();
        // a follow-up runs on its task's account unless the box picked one
        let mut account = a.account;
        if account.is_empty() && !first {
            let get = wisp_runtime_api::commands::IdArgs { id: wisp_runtime_api::SessionId(session.clone()) };
            if let Ok(task) = ctx.run::<wisp_runtime_api::commands::Get>(&ctx.root_call(), get).await {
                account = task.account.unwrap_or_default();
            }
        }
        let send = ask::Send { agent: &a.agent, account: &account, session: &session, first, together: 1, confirm: false };
        Ok(st.ask_send(&ctx, &send))
    });
    // when the account can run again after a limit (v1 budget.resetAt): the runtime's limit card
    on!(s, slot, c::ResetAt, |st, ctx, a| {
        let account = account_of(&a.account);
        let at = lock(&st.budget).reset_at(&a.agent, &account, now(&ctx));
        if at.is_some() {
            lock(&st.limited).insert((a.agent.clone(), account));
        }
        Ok(at)
    });
    on!(s, slot, c::Record, |st, ctx, a| Ok(st.record(&ctx, &a.agent, &a.account, &a.report).await));
    on!(s, slot, c::DismissDrain, |st, ctx, a| {
        let ok = lock(&st.budget).dismiss_drain(&a.agent, &account_of(&a.account), &a.window, now(&ctx));
        st.save().await;
        Ok(c::Done { ok })
    });
    on!(s, slot, c::DismissNotice, |st, _ctx, a| {
        let ok = lock(&st.budget).dismiss_notice(&a.agent, &account_of(&a.account), &a.window);
        st.save().await;
        Ok(c::Done { ok })
    });
    on!(s, slot, c::SetStop, |st, ctx, a| {
        let keys = [settings::AUTO_STOP, settings::AUTO_STOP_CLAUDE, settings::AUTO_STOP_CODEX];
        if !keys.contains(&a.key.as_str()) || !(STOP_AT.contains(&a.value.as_str()) || (a.value == "same" && a.key != settings::AUTO_STOP))
        {
            return Err(WireError::invalid("Automations stop at: off, 50, 75 or 90 (or same)."));
        }
        let v = {
            let mut set = lock(&st.settings);
            if let Some(o) = set.as_object_mut() {
                o.insert(a.key.clone(), json!(a.value));
            }
            set.clone()
        };
        st.store.write(move |w| store::put_settings(w, &v)).await.map_err(WireError::from)?;
        // a line raised or turned off frees held work now
        st.release(&ctx).await;
        Ok(c::Done { ok: true })
    });
}

#[derive(Debug, Default)]
pub struct BudgetPlugin;

impl Plugin for BudgetPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        crate::kinds::register(s);
        let slot: Slot = Arc::new(OnceLock::new());
        commands(s, &slot);
        let hook = slot.clone();
        // after the account is picked (wisp.accounts, 20), before trust (50)
        s.intercept::<BeforeRun, _, _>(InterceptOpts { priority: 30, failure: Failure::Open, timeout_ms: 500 }, move |ctx, _call, c| {
            let slot = hook.clone();
            async move {
                match state(&slot) {
                    Ok(st) => guard(&ctx, &st, c),
                    Err(_) => Verdict::Continue,
                }
            }
        });
        sends(s, &slot);
        let ev = slot.clone();
        s.on::<TurnLimits, _, _>(move |ctx, e| {
            let slot = ev.clone();
            async move {
                let (Ok(st), Some(report)) = (state(&slot), report_from_value(&e.limits)) else { return };
                st.record(&ctx, &e.agent, e.account.as_deref().unwrap_or(""), &report).await;
            }
        });
        on_turns(s, &slot);
        let start = slot;
        s.on_start(move |ctx| async move {
            let store = ctx.service::<Store>()?.as_ref().clone();
            store::install(&store).map_err(WireError::from)?;
            if let Ok(p) = ctx.service::<Paths>() {
                store::import_v1(&store, p.home()).await.map_err(WireError::from)?;
            }
            let (accs, settings, runs) = store::load(&store).await.map_err(WireError::from)?;
            let st = Arc::new(State {
                store,
                budget: Mutex::new(Budget::load(accs)),
                settings: Mutex::new(settings),
                held: Mutex::new(BTreeMap::new()),
                retry: Mutex::default(),
                notes: Mutex::default(),
                runs: Mutex::new(runs),
                running: Mutex::default(),
                asked: Mutex::default(),
                limited: Mutex::default(),
                starting: Mutex::default(),
                plans: Mutex::default(),
                plan_seq: std::sync::atomic::AtomicU64::new(0),
                runs_dirty: std::sync::atomic::AtomicBool::new(false),
            });
            let _ = start.set(st.clone());
            // held work starts when its windows reset on their own, or a live read says they reset early
            if let Ok(clock) = ctx.clock() {
                let (scope, c2, every) = (ctx.scope().clone(), ctx.clone(), live::tick_ms());
                ctx.spawn(async move {
                    let mut due = BTreeMap::new();
                    loop {
                        tokio::select! {
                            () = scope.cancelled() => return,
                            () = clock.sleep(every) => {
                                st.requeue();
                                st.live(&c2, &mut due).await;
                                st.release(&c2).await;
                            }
                        }
                    }
                });
            }
            Ok(())
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        assert_eq!(BudgetPlugin.manifest().id, "wisp.budget");
    }

    #[test]
    fn stop_lines_as_v1() {
        let s = json!({ "autoStop": "90", "autoStopCodex": "50", "autoStopClaude": "same" });
        assert_eq!((stop_for(&s, "codex"), stop_for(&s, "claude"), stop_for(&s, "cursor")), (Some(0.5), Some(0.9), Some(0.9)));
        assert_eq!(stop_for(&json!({ "autoStop": "off" }), "codex"), None);
    }
}
