//! wisp.goal host: a task with a goal keeps going until its judges say it's met (v1 goals: rounds,
//! verdicts, judges, stalls). After each finished turn, each judge (an agent profile or a harness, the
//! goal's own or the `goal` resource's defaults) answers MET or NOT MET in a quick side run; the vote
//! decides: met, another round told what's missing, or past the round limit a question to you. Never
//! for a task's own side runs, and a turn you started yourself after a stall begins the count again.

mod calls;
mod egress;
pub mod inflight;
mod judging;
use judging::{after, judge_turn, recover, wake_dropped};
mod kinds;
pub mod runs;
mod slash;

use std::sync::Arc;

use calls::{put, spec_of, state_of, task_of, turns_of};
use inflight::Inflight;
use serde_json::{Value, json};
use wisp_goal_api::{GoalState, PLUGIN};
use wisp_goal_core::MAX_ROUNDS;
use wisp_kernel::{Ctx, Manifest, Plugin, Result, Setup, WireError};
use wisp_runtime_api::events::TurnEnded;
use wisp_runtime_api::ext::update_ext;
use wisp_runtime_api::model::{Session, TurnStatus};

/// Who the goal's own follow-ups are by.
const BY: &str = "goal";

#[derive(Debug, Default)]
pub struct GoalPlugin;

/// The goal as a task's spec says it, with the defaults under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Goal {
    pub text: String,
    pub judges: Vec<String>,
    pub vote: String,
    pub max_rounds: u32,
    /// its judges run in a sandbox of their own, seeded with the task's folder (ADR-0047: `goal.sandbox`)
    pub sandbox: bool,
    /// the declared goal it is (ADR-0110), '' for a goal in words
    pub name: String,
}

pub fn goal_of(spec: &Value, d: &kinds::GoalSpec) -> Option<Goal> {
    let g = spec.get("goal")?;
    let text = g
        .as_str()
        .map(str::to_owned)
        .or_else(|| g.get("text").and_then(Value::as_str).map(str::to_owned))
        .filter(|t| !t.trim().is_empty())?;
    let list = |v: Option<&Value>| {
        v.and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>())
            .filter(|a| !a.is_empty())
    };
    // the task's own harness: its quick check, when no judge is named
    let own = || vec![spec.get("agent").and_then(Value::as_str).filter(|a| !a.is_empty()).unwrap_or("claude").to_owned()];
    // `judges: []` said on purpose is no judges (B49), not "the defaults"
    let none_on_purpose = g.get("judges").and_then(Value::as_array).is_some_and(Vec::is_empty);
    let judges = if none_on_purpose {
        own()
    } else {
        list(g.get("judges")).or_else(|| d.judges.clone().filter(|j| !j.is_empty())).unwrap_or_else(own)
    };
    let vote = g
        .get("vote")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| Some(d.vote.clone()).filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "majority".into());
    let max_rounds = g.get("maxRounds").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).or(d.max_rounds).unwrap_or(MAX_ROUNDS);
    let sandbox = g.get("sandbox").and_then(Value::as_bool).or(d.sandbox).unwrap_or(false);
    let name = g.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
    Some(Goal { text, judges, vote, max_rounds, sandbox, name })
}

/// A goal as a task's args give it, with a declared goal it names filled in (ADR-0110): `{ name: green-build, …}` is
/// that goal, its fields under the ones given beside the name; a string that is a declared goal's name is that goal
/// too (`goal: green-build` from `wisp_launch`), any other string is what done means. Ok(None): nothing to change.
///
/// # Errors
/// A name no goal has ("no goal named X: goals has a, b").
pub fn resolve(goal: &Value, named: &std::collections::BTreeMap<String, kinds::GoalSpec>) -> std::result::Result<Option<Value>, String> {
    let (name, over) = match goal {
        Value::Object(o) => match o.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => (n.to_owned(), o.clone()),
            None => return Ok(None),
        },
        Value::String(s) if named.contains_key(s.trim()) => (s.trim().to_owned(), serde_json::Map::new()),
        _ => return Ok(None),
    };
    let Some(spec) = named.get(&name) else {
        return Err(kinds::unknown(&name, &named.keys().cloned().collect::<Vec<_>>()));
    };
    let mut out = match serde_json::to_value(spec) {
        Ok(Value::Object(o)) => o,
        _ => serde_json::Map::new(),
    };
    for (k, v) in over {
        if k != "name" && !v.is_null() {
            out.insert(k, v);
        }
    }
    out.insert("name".into(), Value::String(name));
    Ok(Some(Value::Object(out)))
}

/// The goal as it stands: `goal.set`'s text over the spec's; none once it was cleared.
pub fn goal_with(spec: &Value, st: Option<&GoalState>, d: &kinds::GoalSpec) -> Option<Goal> {
    match st {
        Some(s) if s.state == "cleared" => None,
        Some(s) if !s.text.is_empty() => {
            let mut spec = spec.clone();
            let mut goal = spec.get("goal").filter(|g| g.is_object()).cloned().unwrap_or_else(|| json!({}));
            goal["text"] = Value::String(s.text.clone());
            // the judges and vote it was set with, over the spec's (B44)
            if let Some(j) = &s.judges {
                goal["judges"] = json!(j);
            }
            if !s.vote.is_empty() {
                goal["vote"] = Value::String(s.vote.clone());
            }
            // a declared goal set by name: its round limit too (ADR-0110)
            if let Some(n) = s.max_rounds {
                goal["maxRounds"] = json!(n);
            }
            if let Some(b) = s.sandbox {
                goal["sandbox"] = json!(b);
            }
            // the goal set after the start is its own: a goal in words set over a declared one names none
            if let Some(o) = goal.as_object_mut() {
                if s.name.is_empty() {
                    o.remove("name");
                } else {
                    o.insert("name".into(), Value::String(s.name.clone()));
                }
            }
            spec["goal"] = goal;
            goal_of(&spec, d)
        }
        _ => goal_of(spec, d),
    }
}

/// What `goal.set` says of a goal set on a task that isn't running (and wasn't checked now).
pub const NEXT_WORDS: &str = "Checked when the task's next turn ends; --kick checks its last turn now.";

/// `goal.set`: the task's goal set, changed (its count starts again) or cleared. `kick`: a task whose last turn
/// is done is judged on it now (v1 Goals#set: it may already hold); a running one when its turn ends.
async fn set(ctx: &Ctx, defaults: &kinds::Defaults, named: &kinds::Named, live: &Inflight, a: &wisp_goal_api::SetArgs) -> Result<Value> {
    let call = ctx.root_call();
    let task = task_of(ctx, &call, &a.id).await?;
    let a = &by_name(a, &named.lock().unwrap_or_else(std::sync::PoisonError::into_inner))?;
    let text = a.text.trim();
    if !text.is_empty() {
        refuse_goal_on(&task)?;
    }
    let st = if text.is_empty() {
        GoalState { state: "cleared".into(), ..GoalState::default() }
    } else {
        let (max_rounds, sandbox) = if a.name.is_empty() { (None, None) } else { named_limits(named, &a.name) };
        GoalState {
            text: text.to_owned(),
            judges: a.judges.clone(),
            vote: a.vote.clone(),
            max_rounds,
            sandbox,
            name: a.name.clone(),
            ..GoalState::default()
        }
    };
    // the goal it replaced (or cleared) is no longer judged: its judges still running stop (only theirs: a round of
    // the new goal that starts meanwhile goes on)
    let old: Vec<String> = state_of(&task)
        .map(|s| s.running.iter().filter_map(|r| r.get("run").and_then(Value::as_str).map(str::to_owned)).collect())
        .unwrap_or_default();
    // the user's new goal (or none) replaces the slot whole: a round that read the old one loses at its `put_if`
    put(ctx, &call, &a.id, &st).await?;
    live.stop(ctx, &a.id, Some(&old)).await;
    let mut kicked = false;
    if a.kick && !text.is_empty() {
        let turns = turns_of(ctx, &call, &a.id).await.unwrap_or_default();
        if let Some(last) = turns.last().filter(|t| t.status == TurnStatus::Done) {
            let (c, d, l, id, turn) = (ctx.clone(), defaults.clone(), live.clone(), a.id.clone(), last.id.as_str().to_owned());
            ctx.spawn(async move { judge_turn(&c, &d, &l, &id, &turn).await });
            kicked = true;
        }
    }
    let mut view =
        view_of(&spec_of(&task), Some(&st), task.created_at, task.updated_at, &kinds::GoalSpec::default()).unwrap_or(Value::Null);
    // set on a task that isn't running, without kick: nothing judges it until its next turn ends, said so (audit D10:
    // `wisp goal set` answered `status working`, as if it were being worked on)
    if !kicked && !text.is_empty() && task.state != wisp_runtime_api::model::SessionState::Running {
        view["next"] = json!(NEXT_WORDS);
    }
    Ok(view)
}

/// `goal.set`'s args with a declared goal it names (`name`, or a `text` that is exactly one's name) filled in: that
/// goal's text, and its judges and vote unless these args give their own; `name` says which it was. ADR-0110.
///
/// # Errors
/// A `name` no goal has.
pub fn by_name(a: &wisp_goal_api::SetArgs, named: &std::collections::BTreeMap<String, kinds::GoalSpec>) -> Result<wisp_goal_api::SetArgs> {
    let asked = if a.name.trim().is_empty() { a.text.trim() } else { a.name.trim() };
    let Some(spec) = named.get(asked) else {
        if a.name.trim().is_empty() {
            return Ok(a.clone());
        }
        return Err(WireError::not_found(kinds::unknown(asked, &named.keys().cloned().collect::<Vec<_>>())));
    };
    let mut out = a.clone();
    asked.clone_into(&mut out.name);
    out.text.clone_from(&spec.text);
    // `judges: []` declared on purpose is no judges (B49), kept as such
    if out.judges.is_none() {
        out.judges.clone_from(&spec.judges);
    }
    if out.vote.is_empty() {
        out.vote.clone_from(&spec.vote);
    }
    Ok(out)
}

/// A declared goal's round limit and sandbox, which `goal.set` by name carries beside its text, judges and vote.
fn named_limits(named: &kinds::Named, name: &str) -> (Option<u32>, Option<bool>) {
    named.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(name).map_or((None, None), |g| (g.max_rounds, g.sandbox))
}

/// `goal.declared`: each declared goal as stored, with its name.
fn declared(named: &kinds::Named) -> Value {
    let named = named.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    Value::Array(
        named
            .iter()
            .map(|(name, g)| {
                let mut v = serde_json::to_value(g).unwrap_or_else(|_| json!({}));
                v["name"] = json!(name);
                v
            })
            .collect(),
    )
}

/// v1 Goals#set and replyTo: a goal is for a task of your own, never a side discussion's or a subagent's.
pub fn refuse_goal_on(task: &Session) -> Result<()> {
    if task.ext.get("wisp.runtime").and_then(|r| r.get("sideOf")).is_some_and(|v| !v.is_null()) {
        return Err(WireError::conflict("A side discussion only talks the work over: set the goal on the main task."));
    }
    if task.parent_id.is_some() {
        return Err(WireError::conflict("Only a task of your own can have a goal."));
    }
    Ok(())
}

/// v1 Goals#pause / resume / drop: there has to be a goal to move (a typed `/goal drop` says so).
pub fn refuse_no_goal(had: bool, to: &str) -> Result<()> {
    match (had, to) {
        (true, _) => Ok(()),
        (false, "cleared") => Err(WireError::not_found("This task has no goal to drop.")),
        (false, _) => Err(WireError::not_found("This task has no goal.")),
    }
}

/// The new state of a task's goal for pause / resume / drop (pure: the handlers write what this gives).
pub fn moved(st: Option<&GoalState>, to: &str, why: Option<&str>) -> GoalState {
    let mut st = st.cloned().unwrap_or_default();
    match to {
        "paused" => {
            // a round being judged is let go: its runs finish on their records, the goal waits for you
            st.running.clear();
            "paused".clone_into(&mut st.state);
            why.unwrap_or_default().clone_into(&mut st.why);
        }
        "cleared" => st = GoalState { state: "cleared".into(), ..GoalState::default() },
        // going again: where it left off, without the pause's note
        _ => {
            st.state.clear();
            st.why.clear();
            st.running.clear();
        }
    }
    st
}

/// `goal.pause` / `goal.resume` / `goal.drop`: the goal's state written, the band's view given back.
async fn move_goal(ctx: &Ctx, live: &Inflight, a: &wisp_goal_api::TaskArgs, to: &str) -> Result<Value> {
    let call = ctx.root_call();
    // read, decided and written in one compare-and-set: a round's result (its count, its verdicts) or a drop that
    // lands between the read and the write is moved on from, never written back over as it was read
    let mut out = None;
    let mut asked = None;
    update_ext(ctx, &call, &a.id, PLUGIN, |_, task| {
        let had = goal_with(&spec_of(task), state_of(task).as_ref(), &kinds::GoalSpec::default()).is_some();
        refuse_no_goal(had, to)?;
        // a stalled goal asked "Keep going?" (its `why`): dropped, paused or resumed, that question is answered
        asked = state_of(task).filter(|st| st.state == "stalled" && !st.why.is_empty()).map(|st| st.why);
        let st = moved(state_of(task).as_ref(), to, a.why.as_deref());
        let v = serde_json::to_value(&st).map_err(|e| WireError::internal(e.to_string()))?;
        out = Some((st, task.clone()));
        Ok(Some(v))
    })
    .await?;
    let Some((st, task)) = out else { return Err(WireError::internal("goal: nothing was written")) };
    // dropped: its judges still running stop (a pause lets them finish on their records, as v1 did)
    if to == "cleared" {
        live.stop(ctx, &a.id, None).await;
    }
    // its question goes with it, unless something else was asked since: the task stops waiting (audit A tasks #15:
    // after Drop goal "Keep going?" stayed pinned, the row said Asking and Save refused "the agent is working")
    if let Some(question) = asked
        && to != "stalled"
        && let Err(e) = ctx
            .run::<wisp_runtime_api::commands::Unask>(&call, wisp_runtime_api::commands::UnaskArgs { id: a.id.as_str().into(), question })
            .await
    {
        tracing::warn!(task = %a.id, "goal: its question stays: {}", e.message);
    }
    Ok(view_of(&spec_of(&task), Some(&st), task.created_at, task.updated_at, &kinds::GoalSpec::default()).unwrap_or(Value::Null))
}

/// `goal.check`: the last finished turn judged now (in the background; the band shows it as it lands).
async fn check_now(ctx: &Ctx, defaults: &kinds::Defaults, live: &Inflight, id: &str) -> Result<Value> {
    let turns = turns_of(ctx, &ctx.root_call(), id).await?;
    let Some(turn) = turns.iter().rev().find(|t| t.status == TurnStatus::Done).map(|t| t.id.as_str().to_owned()) else {
        return Err(WireError::invalid("No finished turn to check yet."));
    };
    let (c, d, l, id) = (ctx.clone(), defaults.clone(), live.clone(), id.to_owned());
    ctx.spawn(async move { judge_turn(&c, &d, &l, &id, &turn).await });
    Ok(json!({}))
}

/// What a task's list row says about its goal (`summary.goal`, the GUI's `GoalView`: the band over the task,
/// the list's pill): the goal as its spec has it, where its judging stands (`ext["wisp.goal"]`) and the last
/// round's verdicts. None for a task without a goal.
pub fn view_of(spec: &Value, st: Option<&GoalState>, set_at: i64, at: i64, d: &kinds::GoalSpec) -> Option<Value> {
    let g = goal_with(spec, st, d)?;
    let st = st.cloned().unwrap_or_default();
    let status = match st.state.as_str() {
        "met" => "met",
        "paused" => "paused",
        "stalled" => "out",
        "judging" => "checking",
        _ => "working",
    };
    let met = |v: &Value| v.get("met").and_then(Value::as_bool) == Some(true);
    let said = |v: &Value| if met(v) { "met" } else { "not_yet" };
    let why_of = |v: &Value| v.get("why").and_then(Value::as_str).unwrap_or_default().to_owned();
    // the last round: one check with each judge's vote, each opening its run (v1 goalView checks[].votes)
    let votes: Vec<Value> = st
        .verdicts
        .iter()
        .map(|v| {
            let name = v.get("judge").and_then(Value::as_str).unwrap_or_default();
            let mut vote = json!({ "name": name, "agent": name, "verdict": said(v), "why": why_of(v) });
            if let Some(run) = v.get("run").and_then(Value::as_str) {
                vote["run"] = json!(run);
            }
            vote
        })
        .collect();
    let all_met = !st.verdicts.is_empty() && st.verdicts.iter().all(met);
    let tally = format!("{}/{}", st.verdicts.iter().filter(|v| met(v)).count(), st.verdicts.len());
    let joined = st.verdicts.iter().map(why_of).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" · ");
    let checks: Vec<Value> = if votes.is_empty() {
        Vec::new()
    } else {
        vec![
            json!({ "round": st.rounds, "at": at, "verdict": if all_met { "met" } else { "not_yet" }, "why": joined, "tally": tally, "votes": votes }),
        ]
    };
    let mut view = json!({
        "text": g.text,
        "judges": g.judges.iter().map(|j| json!({ "agent": j, "model": "", "effort": "", "name": j })).collect::<Vec<_>>(),
        "vote": g.vote,
        "status": status,
        "round": st.rounds,
        "setAt": set_at,
        "checks": checks,
    });
    if status == "checking" && !st.running.is_empty() {
        view["running"] = Value::Array(st.running.clone());
    }
    if !st.why.is_empty() {
        view["why"] = Value::String(st.why.clone());
    }
    // a declared goal by name (ADR-0110): the list row says which, for Goal…'s recent first (owner-list #63)
    if !g.name.is_empty() {
        view["name"] = Value::String(g.name.clone());
    }
    if !st.verdicts.is_empty() {
        view["last"] = json!({ "verdict": if all_met { "met" } else { "not_yet" }, "why": joined, "tally": tally, "votes": votes });
    }
    Some(view)
}

/// `runtime.summary@1`: the goal on each task's list row.
struct Summary {
    defaults: kinds::Defaults,
}

impl wisp_runtime_api::points::SummaryField for Summary {
    fn plugin(&self) -> &str {
        PLUGIN
    }
    fn fields(
        &self,
        session: &wisp_runtime_api::model::Session,
        _last: Option<&wisp_runtime_api::model::Turn>,
        out: &mut std::collections::BTreeMap<String, Value>,
    ) {
        let spec = session.ext.get("wisp.runtime").and_then(|r| r.get("spec")).cloned().unwrap_or_default();
        let st: Option<GoalState> = session.ext.get(PLUGIN).and_then(|v| serde_json::from_value(v.clone()).ok());
        let d = self.defaults.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        if let Some(v) = view_of(&spec, st.as_ref(), session.created_at, session.updated_at, &d) {
            out.insert("goal".into(), v);
        }
    }
}

impl Plugin for GoalPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let (defaults, named) = crate::kinds::register(s);
        let summary: Arc<dyn wisp_runtime_api::points::SummaryField> = Arc::new(Summary { defaults: Arc::clone(&defaults) });
        s.contribute(&wisp_runtime_api::points::SUMMARY, summary);
        // its check records leave masked (ADR-0054)
        s.contribute(&wisp_kernel::egress::VIEWS, Arc::new(crate::egress::RunView(s.ctx())) as Arc<dyn wisp_kernel::egress::RowView>);
        let live = Inflight::default();
        let (ds, ns, ls) = (Arc::clone(&defaults), Arc::clone(&named), live.clone());
        s.command::<wisp_goal_api::Set, _, _>(move |ctx, _call, a| {
            let (d, n, l) = (ds.clone(), ns.clone(), ls.clone());
            async move { set(&ctx, &d, &n, &l, &a).await }
        });
        let nd = Arc::clone(&named);
        s.command::<wisp_goal_api::Declared, _, _>(move |_ctx, _call, _a| {
            let n = nd.clone();
            async move { Ok(declared(&n)) }
        });
        crate::slash::register(s, named);
        let (dr, lr) = (Arc::clone(&defaults), live.clone());
        s.on_start(|ctx| async move {
            if let Ok(store) = ctx.service::<wisp_store::Store>() {
                runs::install(&store).map_err(|e| WireError::internal(e.to_string()))?;
            }
            // checks a restart cut short: ended, and their goals judged again
            let c = ctx.clone();
            ctx.spawn(async move { recover(&c, &dr, &lr).await });
            Ok(())
        });
        let mover = |to: &'static str| {
            let l = live.clone();
            move |ctx: Ctx, _call: wisp_kernel::Call, a: wisp_goal_api::TaskArgs| {
                let l = l.clone();
                async move { move_goal(&ctx, &l, &a, to).await }
            }
        };
        s.command::<wisp_goal_api::Pause, _, _>(mover("paused"));
        s.command::<wisp_goal_api::Resume, _, _>(mover("working"));
        s.command::<wisp_goal_api::Drop, _, _>(mover("cleared"));
        let (dc, lc) = (Arc::clone(&defaults), live.clone());
        s.command::<wisp_goal_api::Check, _, _>(move |ctx, _call, a| {
            let (d, l) = (dc.clone(), lc.clone());
            async move { check_now(&ctx, &d, &l, &a.id).await }
        });
        let (dt, lt) = (Arc::clone(&defaults), live.clone());
        s.on::<TurnEnded, _, _>(move |ctx, ev| {
            let (d, l, ev, c) = (dt.clone(), lt.clone(), ev.clone(), ctx.clone());
            ctx.spawn(async move { after(&c, &d, &l, &ev).await });
            async {}
        });
        // a deleted task's goal checks go with it, and its judges still running stop
        let ld = live.clone();
        s.on::<wisp_runtime_api::events::SessionDeleted, _, _>(move |ctx, e| {
            let (id, c, l) = (e.id.as_str().to_owned(), ctx.clone(), ld.clone());
            ctx.spawn(async move {
                runs::forget(&c, &id).await;
                l.stop(&c, &id, None).await;
            });
            async {}
        });
        let (dw, lw) = (Arc::clone(&defaults), live);
        s.on::<wisp_later_api::Dropped, _, _>(move |ctx, e| {
            let (d, l, ev, c) = (dw.clone(), lw.clone(), e.clone(), ctx.clone());
            ctx.spawn(async move { wake_dropped(&c, &d, &l, &ev).await });
            async {}
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared_goals() -> std::collections::BTreeMap<String, kinds::GoalSpec> {
        [(
            "green-build".to_owned(),
            kinds::GoalSpec {
                text: "npm test passes".into(),
                judges: Some(vec!["claude".into()]),
                max_rounds: Some(3),
                ..kinds::GoalSpec::default()
            },
        )]
        .into()
    }

    /// ADR-0110: a goal named is the declared one, its fields under the ones given beside the name; the defaults
    /// still fill what neither says; a name no goal has is refused, naming what there is.
    #[test]
    fn a_named_goal_is_the_declared_one_and_an_unknown_name_is_refused() {
        let named = declared_goals();
        let g = resolve(&json!({ "name": "green-build", "vote": "all" }), &named).unwrap().unwrap();
        assert_eq!(g, json!({ "name": "green-build", "text": "npm test passes", "judges": ["claude"], "maxRounds": 3, "vote": "all" }));
        let d = kinds::GoalSpec { vote: "majority".into(), max_rounds: Some(6), sandbox: Some(true), ..kinds::GoalSpec::default() };
        let spec = json!({ "goal": resolve(&json!({ "name": "green-build" }), &named).unwrap().unwrap() });
        let goal = goal_of(&spec, &d).unwrap();
        assert_eq!((goal.text.as_str(), goal.vote.as_str(), goal.max_rounds, goal.sandbox), ("npm test passes", "majority", 3, true));
        // a string that is a declared name is that goal; any other string is what done means
        assert_eq!(resolve(&json!("green-build"), &named).unwrap().unwrap()["text"], "npm test passes");
        assert_eq!(resolve(&json!("tests pass"), &named).unwrap(), None);
        assert_eq!(resolve(&json!({ "text": "inline" }), &named).unwrap(), None);
        assert_eq!(resolve(&json!({ "name": "nope" }), &named).unwrap_err(), "no goal named nope: goals has green-build");
    }

    /// Review of ADR-0110: a declared goal's `judges: []` is no judges on purpose (B49), never the defaults'; and a
    /// goal set by name keeps the declared sandbox and round limit.
    #[test]
    fn a_declared_goals_empty_judges_and_sandbox_are_kept() {
        let (_, named) = kinds::split(vec![("quick".into(), json!({ "text": "x", "judges": [] }))]);
        let d = kinds::GoalSpec { judges: Some(vec!["reviewer".into()]), ..kinds::GoalSpec::default() };
        let spec = json!({ "agent": "codex", "goal": resolve(&json!("quick"), &named).unwrap().unwrap() });
        assert_eq!(goal_of(&spec, &d).unwrap().judges, ["codex"], "its own quick check, not goals.default's judges");
        let by = by_name(&wisp_goal_api::SetArgs { id: "t".into(), name: "quick".into(), ..Default::default() }, &named).unwrap();
        assert_eq!(by.judges, Some(vec![]));
        let st = GoalState { text: "x".into(), sandbox: Some(true), max_rounds: Some(2), ..GoalState::default() };
        let g = goal_with(&json!({ "agent": "codex" }), Some(&st), &kinds::GoalSpec::default()).unwrap();
        assert_eq!((g.sandbox, g.max_rounds), (true, 2));
    }

    #[test]
    fn goal_set_takes_a_declared_goal_by_name_or_by_its_name_as_text() {
        let named = declared_goals();
        let a =
            |text: &str, name: &str| wisp_goal_api::SetArgs { id: "t".into(), text: text.into(), name: name.into(), ..Default::default() };
        let by = by_name(&a("", "green-build"), &named).unwrap();
        assert_eq!(
            (by.text.as_str(), by.judges.clone(), by.name.as_str()),
            ("npm test passes", Some(vec!["claude".to_owned()]), "green-build")
        );
        assert_eq!(by_name(&a("green-build", ""), &named).unwrap().text, "npm test passes", "`wisp goal set <task> green-build`");
        assert_eq!(by_name(&a("the tests pass", ""), &named).unwrap().text, "the tests pass", "words stay words");
        assert!(by_name(&a("", "nope"), &named).unwrap_err().message.contains("no goal named nope"));
        let own = wisp_goal_api::SetArgs { judges: Some(vec![]), vote: "any".into(), ..a("", "green-build") };
        let by = by_name(&own, &named).unwrap();
        assert_eq!((by.judges, by.vote.as_str()), (Some(vec![]), "any"), "the judges and vote given win");
    }

    #[test]
    fn manifest_is_valid() {
        assert_eq!(GoalPlugin.manifest().id, "wisp.goal");
    }

    /// B44: the judges and vote `goal.set` chose are the goal's (they were sent and dropped); without them the spec's
    #[test]
    fn a_goal_set_with_judges_and_a_vote_is_judged_by_them() {
        let d = kinds::GoalSpec::default();
        let spec = json!({ "agent": "codex", "goal": { "text": "tests pass", "judges": ["claude"], "vote": "majority" } });
        let set = GoalState {
            text: "it works on a phone".into(),
            judges: Some(vec!["codex/gpt-5/high".into(), "Ada".into()]),
            vote: "all".into(),
            ..GoalState::default()
        };
        let g = goal_with(&spec, Some(&set), &d).unwrap();
        assert_eq!(
            (g.text.as_str(), g.judges.clone(), g.vote.as_str()),
            ("it works on a phone", vec!["codex/gpt-5/high".to_owned(), "Ada".to_owned()], "all")
        );
        let plain = GoalState { text: "it works on a phone".into(), ..GoalState::default() };
        let g = goal_with(&spec, Some(&plain), &d).unwrap();
        assert_eq!((g.judges.clone(), g.vote.as_str()), (vec!["claude".to_owned()], "majority"), "the spec's when none were chosen");
        // B49: cleared on purpose, the task's own quick check, not the spec's or the defaults
        let d = kinds::GoalSpec { judges: Some(vec!["reviewer".into()]), ..kinds::GoalSpec::default() };
        let none = GoalState { text: "it works".into(), judges: Some(Vec::new()), ..GoalState::default() };
        assert_eq!(goal_with(&spec, Some(&none), &d).unwrap().judges, vec!["codex".to_owned()], "no judges: the agent's own check");
    }

    /// ADR-0047 (new in v2): a goal may ask for its judges in a sandbox; a goal that doesn't, judges here.
    #[test]
    fn a_goal_may_ask_for_its_judges_in_a_sandbox() {
        let d = kinds::GoalSpec::default();
        let boxed = goal_of(&json!({ "goal": { "text": "tests pass", "judges": ["claude"], "sandbox": true } }), &d).unwrap();
        assert!(boxed.sandbox);
        assert!(!goal_of(&json!({ "goal": "tests pass" }), &d).unwrap().sandbox);
    }

    /// Owner-list #63: a task's list row names the declared goal it was given (by name at its start, or by `goal.set`),
    /// so New task's Goal… can put the ones its project used most recently first; a goal in words set over it names
    /// none.
    #[test]
    fn goal_summary_names_the_declared_goal_it_was_given() {
        let d = kinds::GoalSpec::default();
        let started = json!({ "goal": { "name": "green-build", "text": "npm test passes" }, "agent": "claude" });
        assert_eq!(view_of(&started, None, 10, 20, &d).unwrap_or_default()["name"], "green-build", "started by name");
        let set = GoalState { text: "the docs build".into(), name: "docs-clean".into(), ..GoalState::default() };
        assert_eq!(view_of(&started, Some(&set), 10, 20, &d).unwrap_or_default()["name"], "docs-clean", "set by name after");
        let words = GoalState { text: "it works on a phone".into(), ..GoalState::default() };
        let v = view_of(&started, Some(&words), 10, 20, &d).unwrap_or_default();
        assert_eq!((v["text"].clone(), v.get("name")), (json!("it works on a phone"), None), "a goal in words names none");
        assert_eq!(view_of(&json!({ "goal": "tests pass" }), None, 1, 2, &d).unwrap_or_default().get("name"), None);
    }

    /// The GUI's goal band reads `summary.goal`: a task with a goal says where it stands, judged or not.
    #[test]
    fn goal_summary_shows_where_the_goal_stands() {
        let d = kinds::GoalSpec::default();
        assert_eq!(view_of(&json!({ "prompt": "x" }), None, 1, 2, &d), None, "no goal: no field");
        let spec = json!({ "goal": { "text": "a.txt says hello!" }, "agent": "claude" });
        let fresh = view_of(&spec, None, 10, 20, &d).unwrap_or_default();
        assert_eq!(
            (fresh["text"].clone(), fresh["status"].clone(), fresh["round"].clone(), fresh["setAt"].clone()),
            (json!("a.txt says hello!"), json!("working"), json!(0), json!(10))
        );
        assert_eq!(fresh["judges"][0]["name"], "claude");
        let st = GoalState {
            state: "again".into(),
            rounds: 2,
            verdicts: vec![json!({ "judge": "claude", "met": false, "why": "no exclamation mark" })],
            ..GoalState::default()
        };
        let again = view_of(&spec, Some(&st), 10, 20, &d).unwrap_or_default();
        assert_eq!(
            (again["round"].clone(), again["checks"][0]["verdict"].clone(), again["last"]["why"].clone(), again["last"]["tally"].clone()),
            (json!(2), json!("not_yet"), json!("no exclamation mark"), json!("0/1"))
        );
        let met =
            view_of(&spec, Some(&GoalState { state: "met".into(), rounds: 3, ..GoalState::default() }), 10, 20, &d).unwrap_or_default();
        assert_eq!(met["status"], "met");
        let out = view_of(
            &spec,
            Some(&GoalState { state: "stalled".into(), rounds: 5, why: "past 5 rounds".into(), ..GoalState::default() }),
            10,
            20,
            &d,
        )
        .unwrap_or_default();
        assert_eq!((out["status"].clone(), out["why"].clone()), (json!("out"), json!("past 5 rounds")));
    }

    /// `goal.set` after the start: its text wins over the spec's and the count starts again; empty clears it.
    #[test]
    fn goal_set_changes_or_clears_the_goal() {
        let d = kinds::GoalSpec::default();
        let spec = json!({ "goal": { "text": "tests pass", "judges": ["reviewer"] } });
        let changed = GoalState { text: "tests and lint pass".into(), ..GoalState::default() };
        let g = goal_with(&spec, Some(&changed), &d).unwrap_or_else(|| unreachable!());
        assert_eq!((g.text.as_str(), g.judges), ("tests and lint pass", vec!["reviewer".to_owned()]), "the judges stay the spec's");
        let set_later = goal_with(&json!({ "agent": "codex" }), Some(&changed), &d).unwrap_or_else(|| unreachable!());
        assert_eq!(
            (set_later.text.as_str(), set_later.judges),
            ("tests and lint pass", vec!["codex".to_owned()]),
            "a task without a goal gets one"
        );
        let cleared = GoalState { state: "cleared".into(), ..GoalState::default() };
        assert_eq!(goal_with(&spec, Some(&cleared), &d), None);
        assert_eq!(view_of(&spec, Some(&cleared), 1, 2, &d), None, "the band goes");
        assert_eq!(view_of(&spec, Some(&changed), 1, 2, &d).unwrap_or_default()["status"], "working");
    }

    /// Pause holds the goal with its note (no judging while paused), resume goes on where it was, drop clears it.
    #[test]
    fn goal_pause_resume_and_drop_move_its_state() {
        let d = kinds::GoalSpec::default();
        let spec = json!({ "goal": "tests pass", "agent": "claude" });
        let was = GoalState { state: "again".into(), rounds: 2, ..GoalState::default() };
        let paused = moved(Some(&was), "paused", Some("waiting on the API"));
        assert_eq!((paused.state.as_str(), paused.rounds, paused.why.as_str()), ("paused", 2, "waiting on the API"));
        assert_eq!(view_of(&spec, Some(&paused), 1, 2, &d).unwrap_or_default()["status"], "paused");
        let resumed = moved(Some(&paused), "working", None);
        assert_eq!((resumed.state.as_str(), resumed.rounds, resumed.why.as_str()), ("", 2, ""));
        assert_eq!(view_of(&spec, Some(&resumed), 1, 2, &d).unwrap_or_default()["status"], "working");
        assert_eq!(view_of(&spec, Some(&moved(Some(&resumed), "cleared", None)), 1, 2, &d), None);
    }

    /// v1 replyTo / Goals#drop: `/goal …` on a side discussion or a subagent is refused, `/goal drop` without one says so.
    #[test]
    fn a_goal_is_refused_where_v1_refused_it() {
        let mut task = Session::default();
        assert!(refuse_goal_on(&task).is_ok());
        task.ext.insert("wisp.runtime".into(), json!({ "sideOf": { "id": "t0", "title": "Fix login" } }));
        assert!(refuse_goal_on(&task).unwrap_err().message.starts_with("A side discussion only talks the work over"));
        let sub = Session { parent_id: Some("lead".into()), ..Session::default() };
        assert_eq!(refuse_goal_on(&sub).unwrap_err().message, "Only a task of your own can have a goal.");
        assert_eq!(refuse_no_goal(false, "cleared").unwrap_err().message, "This task has no goal to drop.");
        assert_eq!(refuse_no_goal(false, "paused").unwrap_err().message, "This task has no goal.");
        assert!(refuse_no_goal(true, "cleared").is_ok());
    }

    /// A round's result is written only while the goal is still at that round: a goal set, paused, dropped or
    /// judged again meanwhile keeps what was done to it (and one turn is never judged twice at once).
    #[test]
    fn a_rounds_votes_land_only_on_the_round_they_judged() {
        let ids = vec!["r1".to_owned(), "r2".to_owned()];
        let judging =
            GoalState { state: "judging".into(), running: vec![json!({ "run": "r1" }), json!({ "run": "r2" })], ..GoalState::default() };
        assert!(judging::still_ours(&judging, &ids));
        assert!(judging::judging_now(&judging), "a second judge_turn on it stops");
        let set_again = GoalState { text: "X2".into(), ..GoalState::default() };
        assert!(!judging::still_ours(&set_again, &ids), "goal.set meanwhile: its new goal stands");
        assert!(!judging::still_ours(&moved(Some(&judging), "paused", None), &ids), "paused meanwhile");
        let resumed = moved(Some(&moved(Some(&judging), "paused", None)), "working", None);
        assert!(!judging::still_ours(&resumed, &ids) && resumed.running.is_empty(), "paused and resumed meanwhile");
        let other = GoalState { running: vec![json!({ "run": "r9" })], ..judging.clone() };
        assert!(!judging::still_ours(&other, &ids), "another round started meanwhile");
    }

    #[test]
    fn the_goal_takes_the_defaults_under_its_own() {
        let d = kinds::GoalSpec {
            judges: Some(vec!["reviewer".into()]),
            vote: "all".into(),
            max_rounds: Some(2),
            ..kinds::GoalSpec::default()
        };
        assert_eq!(goal_of(&json!({ "agent": "fixer" }), &d), None);
        let g = goal_of(&json!({ "goal": { "text": "tests pass" } }), &d).unwrap_or_else(|| unreachable!());
        assert_eq!((g.judges, g.vote.as_str(), g.max_rounds), (vec!["reviewer".to_owned()], "all", 2));
        let g = goal_of(&json!({ "goal": "ship it", "agent": "fixer" }), &kinds::GoalSpec::default()).unwrap_or_else(|| unreachable!());
        assert_eq!((g.judges, g.max_rounds), (vec!["fixer".to_owned()], MAX_ROUNDS));
    }
}
