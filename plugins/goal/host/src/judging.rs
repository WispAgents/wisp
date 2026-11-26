//! A goal's rounds (v1 server/goal.js): after a finished turn each judge answers in a quick side run, all at once,
//! recorded as it goes (`runs`); the votes decide met, another round told what's missing, or past the limit a question.

use std::sync::atomic::{AtomicBool, Ordering};

use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::{Value, json};
use wisp_goal_api::GoalState;
use wisp_goal_core::{Next, WakeGone, judge_prompt, next, verdict, waits_for_wake, wake_gone};
use wisp_kernel::Ctx;
use wisp_runtime_api::commands as rt;
use wisp_runtime_api::model::{Session, TurnStatus};

use crate::calls::{pick_of, put_if, quick, raw, raw_of, scheduled, spec_of, state_of, task_of, turns_of, warn_put};
use crate::inflight::{Inflight, was_cancelled};
use crate::{BY, Goal, goal_with, kinds, runs};

/// Where a round's judging stands: its task, the turn judged, the round's number.
struct Round<'a> {
    task: &'a Session,
    turn_id: &'a str,
    number: u32,
}

pub(crate) fn now(ctx: &Ctx) -> i64 {
    ctx.clock().map_or(0, |k| k.now_ms())
}

/// One judge's quick run, recorded as it starts and as it ends (`runs`): its verdict, and whether it was stopped
/// because the round's vote was already decided (`decided`) without it.
async fn one_judge(
    ctx: &Ctx,
    round: &Round<'_>,
    judge: &str,
    prompt: &str,
    place: Option<&(String, String)>,
    run: &str,
    decided: &AtomicBool,
) -> (wisp_goal_core::Verdict, bool) {
    let pick = pick_of(ctx, judge, round.task.id.as_str()).await;
    let at = now(ctx);
    let task = round.task.id.as_str();
    let check = runs::Check {
        id: run,
        task,
        turn_id: round.turn_id,
        round: round.number,
        name: judge,
        agent: pick.agent.as_deref().unwrap_or_default(),
        model: pick.model.as_deref().unwrap_or_default(),
        prompt,
        at,
    };
    // its task deleted meanwhile (`session.deleted`): no check starts, no record of it is left
    if !runs::put(ctx, runs::started(&check)).await {
        return (wisp_goal_core::Verdict::failed(judge, format!("{judge} didn't judge: the task is gone")), false);
    }
    // the others decided the round while this one's record was written: it never starts
    if decided.load(Ordering::SeqCst) {
        let v = not_needed(judge);
        runs::put(ctx, runs::ended(&check, &runs::Ended::NotNeeded { why: &v.why }, now(ctx))).await;
        return (v, true);
    }
    let args = rt::QuickArgs {
        prompt: prompt.to_owned(),
        cwd: place.map_or(round.task.cwd.as_str(), |p| p.1.as_str()).to_owned(),
        pick: Some(pick.clone()),
        timeout_ms: Some(300_000),
        json: false,
        run_id: Some(run.to_owned()),
    };
    match quick(ctx, place.map(|p| p.0.as_str()), args).await {
        Ok(out) => {
            let said = verdict(judge, &out.text);
            let activity: Vec<Value> = out.activity.iter().filter_map(|x| serde_json::to_value(x).ok()).collect();
            let ended = runs::Ended::Voted { met: said.met, why: &said.why, text: &out.text, activity: &activity, model: &out.model };
            runs::put(ctx, runs::ended(&check, &ended, now(ctx))).await;
            (said, false)
        }
        // stopped as the others' votes already decided the round: no vote, and not a failure
        Err(err) if was_cancelled(&err) && decided.load(Ordering::SeqCst) => {
            let v = not_needed(judge);
            runs::put(ctx, runs::ended(&check, &runs::Ended::NotNeeded { why: &v.why }, now(ctx))).await;
            (v, true)
        }
        // stopped as its goal was dropped, set again or its task deleted: no vote, and the record says so
        Err(err) if was_cancelled(&err) => {
            let why = format!("{judge} was stopped: the goal changed while it judged");
            runs::put(ctx, runs::ended(&check, &runs::Ended::Cancelled { why: &why }, now(ctx))).await;
            (wisp_goal_core::Verdict::failed(judge, why), false)
        }
        Err(err) => {
            let why = format!("{judge} couldn't judge: {}", err.message);
            runs::put(ctx, runs::ended(&check, &runs::Ended::Failed { why: &why }, now(ctx))).await;
            (wisp_goal_core::Verdict::failed(judge, why), false)
        }
    }
}

/// What a judge stopped once the vote was decided without it says.
pub const NOT_NEEDED: &str = "not needed: the vote was already decided";

/// Whether the votes in so far fix the round's outcome whatever the rest say: `any` with a yes, `all` with a no (or a
/// judge that couldn't judge). A majority waits for every vote.
pub fn decided(vote: &str, so_far: &[&wisp_goal_core::Verdict]) -> bool {
    match vote {
        "any" => so_far.iter().any(|v| v.met),
        "all" => so_far.iter().any(|v| !v.met),
        _ => false,
    }
}

/// Each judge's vote on the turn's answer, with the record of its run (`ids`: one per judge, in order), and whether it
/// was stopped as not needed; None for one that never started (the goal moved meanwhile). Every judge runs at once (a
/// round took the sum of their times, five minutes each at worst); with `any` or `all` the rest stop as soon as the
/// votes in decide the round. In `g.judges` order, whatever order they end in.
async fn judge(
    ctx: &Ctx,
    live: &Inflight,
    g: &Goal,
    r: &Round<'_>,
    answer: &str,
    ids: &[String],
) -> Vec<Option<(wisp_goal_core::Verdict, bool)>> {
    let task = r.task;
    let cwd = task.cwd.as_str();
    // judges in a sandbox: one for this task's judges (made once, reused every round), the task's folder in it
    let place = if g.sandbox {
        let a =
            wisp_sandbox_api::PlaceArgs { owner: json!({ "judges": task.id.as_str() }), cwd: (!cwd.is_empty()).then(|| cwd.to_owned()) };
        match ctx.run::<wisp_sandbox_api::Place>(&ctx.root_call(), a).await {
            Ok(p) => Some((p.machine, p.cwd)),
            Err(e) => {
                return g
                    .judges
                    .iter()
                    .map(|j| Some((wisp_goal_core::Verdict::failed(j, format!("{j} couldn't judge in a sandbox: {}", e.message)), false)))
                    .collect();
            }
        }
    } else {
        None
    };
    let prompt = judge_prompt(&g.text, &task.title, answer);
    let id = task.id.as_str();
    let done = AtomicBool::new(false);
    let mut going: FuturesUnordered<_> = g
        .judges
        .iter()
        .zip(ids)
        .enumerate()
        .map(|(i, (j, run))| {
            let (prompt, place, done) = (&prompt, place.as_ref(), &done);
            async move {
                // held before the goal is read again: a drop, a new goal or a delete either is seen here or stops this run
                let _held = live.hold(id, run, place.map(|p| p.0.as_str()));
                // dropped, set again, paused, deleted or halted since the round began: this judge isn't started
                if live.halted(id) || !fresh_state(ctx, &ctx.root_call(), id).await.is_some_and(|cur| still_ours(&cur, ids)) {
                    return (i, None);
                }
                if done.load(Ordering::SeqCst) {
                    return (i, Some((not_needed(j), true)));
                }
                (i, Some(one_judge(ctx, r, j, prompt, place, run, done).await))
            }
        })
        .collect();
    let mut out: Vec<Option<(wisp_goal_core::Verdict, bool)>> = vec![None; ids.len()];
    while let Some((i, v)) = going.next().await {
        if let Some(slot) = out.get_mut(i) {
            *slot = v;
        }
        let votes: Vec<&wisp_goal_core::Verdict> = out.iter().flatten().filter(|(_, skipped)| !skipped).map(|(v, _)| v).collect();
        if !done.load(Ordering::SeqCst) && decided(&g.vote, &votes) {
            done.store(true, Ordering::SeqCst);
            // the ones still judging stop now: their votes can't change the outcome
            let rest: Vec<String> = ids.iter().zip(&out).filter(|(_, v)| v.is_none()).map(|(run, _)| run.clone()).collect();
            live.stop(ctx, id, Some(&rest)).await;
        }
    }
    out
}

/// A judge the vote was decided without, never started.
fn not_needed(judge: &str) -> wisp_goal_core::Verdict {
    wisp_goal_core::Verdict { judge: judge.to_owned(), met: false, why: format!("{judge}: {NOT_NEEDED}"), failed: false }
}

/// The wake-up a goal waited for went without going (`later.dropped`, v1 goal.js wakeGone): cancelled, the round's
/// report is checked now; otherwise the goal pauses and waits for you.
pub(crate) async fn wake_dropped(ctx: &Ctx, defaults: &kinds::Defaults, live: &Inflight, e: &wisp_later_api::Dropped) {
    let call = ctx.root_call();
    let Ok(task) = task_of(ctx, &call, &e.session_id).await else { return };
    let was = raw_of(&task);
    let mut st = state_of(&task).unwrap_or_default();
    if st.wake.is_empty() || st.wake != e.entry.id {
        return;
    }
    st.wake.clear();
    match wake_gone(&e.why) {
        WakeGone::CheckNow => {
            // a goal moved since it was read (dropped, set again): theirs stands, nothing is checked for it
            if put_if(ctx, &call, &e.session_id, &st, &was).await.is_err() {
                return;
            }
            let turns = turns_of(ctx, &call, &e.session_id).await.unwrap_or_default();
            if let Some(last) = turns.last() {
                judge_turn(ctx, defaults, live, &e.session_id, last.id.as_str()).await;
            }
        }
        WakeGone::Pause => {
            "paused".clone_into(&mut st.state);
            "Its agent’s wake-up didn’t go".clone_into(&mut st.why);
            warn_put(&e.session_id, put_if(ctx, &call, &e.session_id, &st, &was).await);
        }
    }
}

/// Whether a task's goal is judged: a task of its own, or a child task whose spec gives it a goal (a workflow's step);
/// a subagent's task shares its lead's goal and isn't judged on its own.
pub fn judged(task: &Session, spec: &Value) -> bool {
    task.parent_id.is_none() || spec.get("goal").is_some_and(|g| !g.is_null())
}

/// A turn's answer judged against the task's goal: met, another round, or past the limit a question.
pub(crate) async fn judge_turn(ctx: &Ctx, defaults: &kinds::Defaults, live: &Inflight, id: &str, turn_id: &str) -> Moved {
    let call = ctx.root_call();
    let Ok(task) = task_of(ctx, &call, id).await else { return Moved::Not };
    let spec = spec_of(&task);
    if !judged(&task, &spec) {
        return Moved::Not;
    }
    let d = defaults.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    // every write below is conditional on the slot as read here (or as this round last wrote it), in the write's
    // own transaction: a goal set, paused or dropped meanwhile always wins over this round
    let was = raw_of(&task);
    let mut st = state_of(&task).unwrap_or_default();
    let Some(g) = goal_with(&spec, Some(&st), &d) else { return Moved::Not };
    // met, paused, or a round already being judged (TurnEnded and a kick or Check now on one turn): nothing twice
    if matches!(st.state.as_str(), "met" | "paused") || judging_now(&st) {
        return Moved::Not;
    }
    // a turn you started after a stall: a new count
    let turns = turns_of(ctx, &call, id).await.unwrap_or_default();
    let last = turns.iter().find(|t| t.id.as_str() == turn_id);
    if st.state == "stalled" && last.and_then(|t| t.by.as_deref()) != Some(BY) {
        st.rounds = 0;
    }
    // its agent scheduled its own wake-up this turn: it's waiting, not done; the goal is checked after that turn (v1 #775)
    let started = last.map_or(0, |t| t.started_at);
    if let Some(wake) = waits_for_wake(&scheduled(ctx, id).await, started) {
        st.wake = wake;
        warn_put(id, put_if(ctx, &call, id, &st, &was).await);
        return Moved::Not;
    }
    st.wake.clear();
    let answer = last.and_then(|t| t.text.clone()).unwrap_or_default();
    // checking: the band says so and each judge's run opens while it goes (v1 goalView `running`)
    let ids: Vec<String> = g.judges.iter().map(|_| wisp_kernel::ids::uuid7()).collect();
    "judging".clone_into(&mut st.state);
    st.running = g.judges.iter().zip(&ids).map(|(j, run)| json!({ "run": run, "name": j, "at": now(ctx) })).collect();
    // the goal moved while this read the turns (set, paused, dropped, another round started): theirs stands
    if let Err(err) = put_if(ctx, &call, id, &st, &was).await {
        tracing::debug!(task = %id, "goal: the round isn't started: {}", err.message);
        return Moved::Not;
    }
    let ours = raw(&st);
    let r = Round { task: &task, turn_id, number: st.rounds + 1 };
    let verdicts = judge(ctx, live, &g, &r, &answer, &ids).await;
    st.running.clear();
    // you stopped the task this round held in Working: its votes, cut short, decide nothing (a stopped judge would read
    // as one that couldn't judge, and the goal asked you why); it goes back to waiting for the next turn
    if live.take_halt(id) {
        st.state.clear();
        warn_put(id, put_if(ctx, &call, id, &st, &ours).await);
        return Moved::Not;
    }
    st.rounds += 1;
    st.verdicts = verdicts
        .iter()
        .zip(&ids)
        .filter_map(|(v, run)| v.as_ref().map(|v| (v, run)))
        .map(|((v, skipped), run)| {
            let mut out = json!({ "judge": v.judge, "met": v.met, "why": v.why, "run": run });
            if *skipped {
                out["skipped"] = json!(true);
            }
            out
        })
        .collect();
    // a judge stopped because the vote was decided without it has no vote
    let votes: Vec<wisp_goal_core::Verdict> = verdicts.into_iter().flatten().filter(|(_, skipped)| !skipped).map(|(v, _)| v).collect();
    let step = next(&g.vote, &votes, st.rounds, g.max_rounds);
    match &step {
        Next::Met => "met".clone_into(&mut st.state),
        Next::Again(_) => "again".clone_into(&mut st.state),
        Next::Stall(why) => {
            "stalled".clone_into(&mut st.state);
            why.clone_into(&mut st.why);
        }
    }
    // set again, paused, dropped or resumed while it judged (even between a read and this write): the votes stay
    // on their records, the goal where you put it, and nothing follows from them
    if let Err(err) = put_if(ctx, &call, id, &st, &ours).await {
        tracing::debug!(task = %id, "goal: the round's votes are not the goal's any more: {}", err.message);
        return Moved::Not;
    }
    // the verdict as a durable fact (ADR-0043: `goal.met` and `goal.failed` start triggers and wake crew)
    match &step {
        Next::Met => ctx.emit(&call, &wisp_goal_api::GoalMet { session_id: id.to_owned(), goal: g.text.clone(), rounds: st.rounds }),
        Next::Stall(why) => ctx.emit(
            &call,
            &wisp_goal_api::GoalFailed { session_id: id.to_owned(), goal: g.text.clone(), rounds: st.rounds, why: why.clone() },
        ),
        Next::Again(_) => {}
    }
    let out = match step {
        Next::Met => Ok(Moved::Not),
        Next::Again(text) => ctx
            .run::<rt::FollowUp>(
                &call,
                rt::FollowUpArgs { id: id.into(), text, by: Some(BY.into()), queue: Some(true), ..rt::FollowUpArgs::default() },
            )
            .await
            .map(|_| Moved::Again),
        Next::Stall(q) => ctx
            .run::<rt::Ask>(&call, rt::AskArgs { id: id.into(), question: q, options: Vec::new(), details: None })
            .await
            .map(|_| Moved::Asked),
    };
    out.unwrap_or_else(|err| {
        tracing::warn!(task = %id, "goal: {}", err.message);
        Moved::Not
    })
}

/// What a round did with its task: nothing that moves it on (met, not judged, the goal moved), another round sent
/// (its follow-up starts the next turn), or a question put (it waits on you).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Moved {
    Not,
    Again,
    Asked,
}

/// A round the turn's end held its task in Working for (`GoalHold`): judged, then the task lands unless the round moved
/// it on, once, in Needs review and unread, as any turn's end lands (met: one `turn.finished`, one notification).
pub(crate) async fn held_round(ctx: &Ctx, defaults: &kinds::Defaults, live: &Inflight, hold: &Holds, id: &str, turn_id: &str) {
    // a halt left from a stop between rounds isn't this round's
    live.take_halt(id);
    let moved = {
        let _round = hold.round(id);
        judge_turn(ctx, defaults, live, id, turn_id).await
    };
    if moved == Moved::Not {
        let _landing = hold.landing(id);
        let a = rt::SettleArgs { id: id.into(), quiet: false };
        if let Err(err) = ctx.run::<rt::Settle>(&ctx.root_call(), a).await {
            tracing::warn!(task = %id, "goal: runtime.settle: {}", err.message);
        }
    }
}

/// The tasks a goal round holds in Working (`round`), and those it is landing (`landing`: `runtime.settle` asks the
/// after-turn steps again, and this one must not take the task back).
#[derive(Debug, Clone, Default)]
pub struct Holds(std::sync::Arc<std::sync::Mutex<(std::collections::HashSet<String>, std::collections::HashSet<String>)>>);

/// A task's place in [`Holds`], gone with the round (or its landing) however it ends.
pub(crate) struct Mark {
    all: Holds,
    task: String,
    landing: bool,
}

impl Drop for Mark {
    fn drop(&mut self) {
        let mut m = self.all.lock();
        if self.landing {
            m.1.remove(&self.task)
        } else {
            m.0.remove(&self.task)
        };
    }
}

impl Holds {
    fn lock(&self) -> std::sync::MutexGuard<'_, (std::collections::HashSet<String>, std::collections::HashSet<String>)> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn mark(&self, task: &str, landing: bool) -> Mark {
        let mut m = self.lock();
        if landing {
            m.1.insert(task.to_owned())
        } else {
            m.0.insert(task.to_owned())
        };
        Mark { all: self.clone(), task: task.to_owned(), landing }
    }

    pub(crate) fn round(&self, task: &str) -> Mark {
        self.mark(task, false)
    }

    pub(crate) fn landing(&self, task: &str) -> Mark {
        self.mark(task, true)
    }

    /// A round holds `task` in Working now.
    pub fn holds(&self, task: &str) -> bool {
        self.lock().0.contains(task)
    }

    /// The round is landing `task`.
    pub fn lands(&self, task: &str) -> bool {
        self.lock().1.contains(task)
    }
}

/// The tasks whose goal slot says `judging` (`runtime.slots`, only bodies that name it): at start, none is judged.
async fn judging_slots(ctx: &Ctx, call: &wisp_kernel::Call) -> Vec<String> {
    let a = rt::SlotsArgs { plugins: vec![wisp_goal_api::PLUGIN.into()], all: false, having: vec!["judging".into()] };
    let found = ctx.run::<rt::Slots>(call, a).await.unwrap_or_else(|e| {
        tracing::warn!(error = %e.message, "goal: the goals left judging weren't read");
        Vec::new()
    });
    judging_in(&found)
}

/// Of these tasks' slots, the ones whose goal says `judging`.
pub fn judging_in(found: &[rt::TaskSlots]) -> Vec<String> {
    found
        .iter()
        .filter(|t| t.ext.get(wisp_goal_api::PLUGIN).and_then(|v| v.get("state")).and_then(Value::as_str) == Some("judging"))
        .map(|t| t.id.as_str().to_owned())
        .collect()
}

/// The task's goal state as it is now (None: the task is gone).
async fn fresh_state(ctx: &Ctx, call: &wisp_kernel::Call, id: &str) -> Option<GoalState> {
    task_of(ctx, call, id).await.ok().map(|t| state_of(&t).unwrap_or_default())
}

/// A round is being judged (its judges' runs under way).
pub fn judging_now(st: &GoalState) -> bool {
    st.state == "judging" && !st.running.is_empty()
}

/// The goal is still at the round these runs judge: nothing set, paused, dropped or started another since.
pub fn still_ours(cur: &GoalState, ids: &[String]) -> bool {
    let runs: Vec<&str> = cur.running.iter().filter_map(|r| r.get("run").and_then(Value::as_str)).collect();
    cur.state == "judging" && runs == ids.iter().map(String::as_str).collect::<Vec<_>>()
}

/// After a restart (v1 `interrupted`): checks that were under way never vote; their records say so, and their
/// tasks' goals go back to working and their last answers are judged again. A round cut before its first check's record
/// was written (`judging` is written first) has no record: its task's slot says `judging`, and is found by it. A task
/// its round held in Working is held again, and lands after it. A task left in Working by a round cut with no such
/// mark (`hold::stranded`) is judged again or landed.
pub(crate) async fn recover(ctx: &Ctx, defaults: &kinds::Defaults, live: &Inflight, hold: &Holds) {
    let call = ctx.root_call();
    let mut ids = runs::interrupted(ctx, now(ctx)).await;
    for id in judging_slots(ctx, &call).await {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let mut done: Vec<String> = Vec::new();
    for id in ids {
        let Ok(task) = task_of(ctx, &call, &id).await else { continue };
        let was = raw_of(&task);
        let Some(mut st) = state_of(&task).filter(|s| s.state == "judging") else { continue };
        st.state.clear();
        st.running.clear();
        if put_if(ctx, &call, &id, &st, &was).await.is_err() {
            continue;
        }
        let turns = turns_of(ctx, &call, &id).await.unwrap_or_default();
        let Some(last) = turns.last().filter(|t| t.status == TurnStatus::Done) else { continue };
        if task.state == wisp_runtime_api::model::SessionState::Running {
            held_round(ctx, defaults, live, hold, &id, last.id.as_str()).await;
        } else {
            judge_turn(ctx, defaults, live, &id, last.id.as_str()).await;
        }
        done.push(id);
    }
    // a round cut where nothing above shows it (after the hold took the task, before `judging` was written; or after
    // the verdict, before its landing): the task sat in Working for ever
    let rows = ctx.run::<rt::List>(&call, rt::ListArgs { archived: Some(false), limit: None }).await.unwrap_or_default();
    for row in rows.iter().filter(|r| r.state == wisp_runtime_api::model::SessionState::Running) {
        let id = row.id.as_str();
        if done.iter().any(|d| d == id) {
            continue;
        }
        let Ok(task) = task_of(ctx, &call, id).await else { continue };
        let turns = turns_of(ctx, &call, id).await.unwrap_or_default();
        let Some(last) = turns.last() else { continue };
        match crate::hold::stranded(&task, last) {
            Some(crate::hold::Stranded::Judge) => held_round(ctx, defaults, live, hold, id, last.id.as_str()).await,
            Some(crate::hold::Stranded::Land) => {
                let _landing = hold.landing(id);
                if let Err(err) = ctx.run::<rt::Settle>(&call, rt::SettleArgs { id: id.into(), quiet: false }).await {
                    tracing::warn!(task = %id, "goal: runtime.settle: {}", err.message);
                }
            }
            None => {}
        }
    }
}
