//! size-ok: one run from start to its end (start, drive, its view on the task, status, stop) with its tests
//! A run (ADR-0028, ADR-0119): starting it, driving it until every step ended, stopping it, and what it reads back.
//!
//! A run belongs to a task, its owner: the task whose agent started it (an agent's run is always its own task's),
//! the task the user named, or else a held task made for it (`wisp run <name>` from a shell). Its steps are the
//! owner's children; where it stands is written to the owner's `ext["wisp.workflows"].runs` (one view per run, the
//! last few kept), so the task's page shows it live and every frontend reads it the same way. When it ends, the owner
//! is sent every step's result as a follow-up (queued behind a turn that's running): the agent that started it picks
//! up from there, as Claude Code's workflows tell their lead.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Map, Value, json};
use tokio::sync::{Mutex as AsyncMutex, broadcast, mpsc, oneshot};
use wisp_kernel::{Call, Ctx, Result, WireError};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::spec::Lineage;
use wisp_runtime_api::{Mode, Session, SessionId, SessionState, Turn, TurnStatus};
use wisp_store::Store;
use wisp_workflows_api::{EXT, EditArgs, RunArgs, Started, StepState, WorkflowEnded, WorkflowSpec, WorkflowStarted};
use wisp_workflows_core::{Action, Run, check, output_of};

/// How often a run looks at its steps when nothing woke it (timeouts).
const LOOK_MS: u64 = 2_000;
/// The runs a task keeps in its view (the newest).
const KEEP: usize = 8;
/// How much of a step's answer the owner is sent when the run ends (its task holds the rest).
const SAY_CHARS: usize = 700;
/// How much of a step's output the task's view carries.
const VIEW_CHARS: usize = 240;

/// `(task, cost)` of every turn that ended, for the runs to wake on.
pub(crate) type Ended = broadcast::Sender<(String, f64)>;

/// What a run's driver is told while it goes: stop, or a change (ADR-0121) with who made it, the mode its steps
/// may run at (an agent's: its task's), and where to say whether it took.
pub(crate) enum Ctl {
    Stop,
    Edit(Box<EditArgs>, String, Option<Mode>, oneshot::Sender<std::result::Result<(), String>>),
}

/// A run that is going: whose it is, and how to reach its driver.
struct Live {
    owner: String,
    ctl: mpsc::UnboundedSender<Ctl>,
}

/// The runs ended here kept to start again (a retry or a step added after the end), the newest few.
const KEEP_ENDED: usize = 16;

pub(crate) struct State {
    pub(crate) ended: Ended,
    live: Mutex<BTreeMap<String, Live>>,
    /// ended runs that a retry or an added step can start again (ADR-0121)
    done: Mutex<Vec<(RunCtx, Run)>>,
    /// one writer of a task's `runs` at a time: each write reads the slot, changes its run, and writes it back
    slots: AsyncMutex<()>,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            ended: broadcast::channel(256).0,
            live: Mutex::new(BTreeMap::new()),
            done: Mutex::new(Vec::new()),
            slots: AsyncMutex::new(()),
        }
    }

    /// Whether a run of `task`'s is going here (its task stays in Working on it).
    pub(crate) fn owns_live(&self, task: &str) -> bool {
        self.live.lock().unwrap_or_else(PoisonError::into_inner).values().any(|l| l.owner == task)
    }

    /// Stops every run of `task`'s going here (you stopped the task); true when there was one.
    pub(crate) fn stop_owned(&self, task: &str) -> bool {
        let live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        let mut any = false;
        for l in live.values().filter(|l| l.owner == task) {
            let _ = l.ctl.send(Ctl::Stop);
            any = true;
        }
        any
    }

    fn owner_of(&self, run: &str) -> Option<String> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner).get(run).map(|l| l.owner.clone())
    }

    /// A run going here: its task, and how to reach its driver.
    pub(crate) fn live_run(&self, run: &str) -> Option<(String, mpsc::UnboundedSender<Ctl>)> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner).get(run).map(|l| (l.owner.clone(), l.ctl.clone()))
    }

    /// The task of a run that ended here and is kept to start again.
    pub(crate) fn ended_owner(&self, run: &str) -> Option<String> {
        self.done.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(rc, _)| rc.id == run).map(|(rc, _)| rc.owner.clone())
    }

    fn keep_ended(&self, rc: RunCtx, run: Run) {
        let mut done = self.done.lock().unwrap_or_else(PoisonError::into_inner);
        done.retain(|(x, _)| x.id != rc.id);
        done.push((rc, run));
        if done.len() > KEEP_ENDED {
            done.remove(0);
        }
    }

    fn take_ended(&self, run: &str) -> Option<(RunCtx, Run)> {
        let mut done = self.done.lock().unwrap_or_else(PoisonError::into_inner);
        let at = done.iter().position(|(rc, _)| rc.id == run)?;
        Some(done.remove(at))
    }
}

/// A node's `mode:` as runtime names it (`plan`, `edits`, `auto`); a wrong one fails the step, as runtime's own
/// check of the args did.
pub(crate) fn mode_of(m: &str) -> std::result::Result<Option<Mode>, String> {
    if m.is_empty() {
        return Ok(None);
    }
    serde_json::from_value(json!(m)).map(Some).map_err(|_| format!("{m:?} isn't a mode (plan, edits or auto)"))
}

fn some(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_owned())
}

/// A wire name (`running`, `done`, …) of a runtime enum.
fn wire_name<T: serde::Serialize>(v: T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

/// Every step at most `cap` (ADR-0112: an agent never raises a mode): a step that says none, or more, runs at `cap`.
pub(crate) fn cap_modes(spec: &mut WorkflowSpec, cap: Mode) {
    for s in spec.tasks.values_mut() {
        cap_step(s, cap);
    }
}

/// One step at most `cap`.
pub(crate) fn cap_step(s: &mut wisp_workflows_api::StepSpec, cap: Mode) {
    let over = match mode_of(&s.mode) {
        Ok(Some(m)) => m.rank() > cap.rank(),
        Ok(None) => true,
        // a wrong one fails its step, as it does in any run
        Err(_) => false,
    };
    if over {
        s.mode = wire_name(cap);
    }
}

/// What a run's steps are made with: its id, the task they go under, and where they run.
#[derive(Debug, Clone)]
pub(crate) struct RunCtx {
    pub(crate) id: String,
    pub(crate) owner: String,
    /// the owner is a held task made for this run (it ends with the run, and nothing is sent to it)
    held: bool,
    pub(crate) name: String,
    project: String,
    notify: bool,
    started_at: i64,
    /// the workflow as it was given (the view's Copy as workflow)
    spec: Value,
    /// the trigger that started the run: its steps carry it, so their events never fire it again
    triggered_by: Option<String>,
}

/// The child task's args for a node (Err: why it can't start).
fn child_args(run: &Run, rc: &RunCtx, node: &str, prompt: String, steps: Vec<String>) -> std::result::Result<r::StartArgs, String> {
    let n = run.nodes.iter().find(|n| n.id == node).map(|n| n.spec.clone()).unwrap_or_default();
    let cwd = if n.project.is_empty() { rc.project.as_str() } else { n.project.as_str() };
    let what = if n.title.is_empty() { node.to_owned() } else { format!("{} · {node}", n.title) };
    Ok(r::StartArgs {
        prompt,
        title: Some(format!("{} · {what}", rc.name)),
        parent_id: Some(SessionId(rc.owner.clone())),
        // the run's task, as runtime reads it: a step made under a task an agent was held to is held to it too
        lineage: Lineage { workflow_run: Some(rc.owner.clone()), triggered_by: rc.triggered_by.clone(), ..Lineage::default() },
        ext: [(EXT.to_owned(), json!({ "run": rc.id, "node": node }))].into(),
        agent: some(&n.agent),
        model: some(&n.model),
        effort: some(&n.effort),
        mode: mode_of(&n.mode)?,
        cwd: some(cwd),
        steps,
        goal: n.goal.clone(),
        worktree: n.worktree,
        ..r::StartArgs::default()
    })
}

/// At most `n` characters of `s`, with … when it was cut.
fn short(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(n).collect();
    out.push('…');
    out
}

/// A step's output in a line: its text, else its fields as JSON.
pub(crate) fn output_line(output: &Value, n: usize) -> String {
    match output {
        Value::Null => String::new(),
        Value::String(s) => short(s, n),
        Value::Object(o) if o.contains_key("text") && o.len() <= 4 => {
            let lead = ["title", "summary"].iter().find_map(|k| o.get(*k).and_then(Value::as_str));
            short(lead.unwrap_or_else(|| o.get("text").and_then(Value::as_str).unwrap_or_default()), n)
        }
        // a structured output: its fields in words (`file: one.txt · lines: 3`), a nested value as JSON
        Value::Object(o) => short(
            &o.iter()
                .map(|(k, v)| match v {
                    Value::String(s) => format!("{k}: {s}"),
                    Value::Array(a) => format!("{k}: {} items", a.len()),
                    other => format!("{k}: {other}"),
                })
                .collect::<Vec<_>>()
                .join(" · "),
            n,
        ),
        other => short(&other.to_string(), n),
    }
}

/// The run as its task's view shows it (`ext["wisp.workflows"].runs[]`): each step with its phase, agent, state, task,
/// times, cost and a line of its output.
/// A step whose turn is held before it starts carries why (`held`): the board shows it waiting, not working.
fn view(run: &Run, rc: &RunCtx, state: &str, notes: &[String], ended_at: Option<i64>, holds: &BTreeMap<String, String>) -> Value {
    let mut steps = run.report();
    for s in steps.as_array_mut().into_iter().flatten() {
        let line = output_line(s.get("output").unwrap_or(&Value::Null), VIEW_CHARS);
        if let Some(o) = s.as_object_mut() {
            o.remove("output");
            o.insert("said".into(), json!(line));
            if let Some(why) = o.get("id").and_then(Value::as_str).and_then(|id| holds.get(id))
                && o.get("state").and_then(Value::as_str) == Some("running")
            {
                o.insert("held".into(), json!(why));
            }
            if let Some(item) = o.get("item").filter(|v| !v.is_null() && !v.is_string()).cloned() {
                o.insert("item".into(), json!(short(&item.to_string(), 80)));
            }
        }
    }
    let total = run.nodes.len();
    let done = run.runs.values().filter(|r| matches!(r.state, StepState::Done | StepState::Failed | StepState::Skipped)).count();
    let mut phases: Vec<String> = Vec::new();
    for n in &run.nodes {
        if !phases.contains(&n.spec.phase) {
            phases.push(n.spec.phase.clone());
        }
    }
    json!({
        "id": rc.id, "workflow": rc.name, "state": state, "cost": run.cost(), "budget": run.spec.budget.as_ref().and_then(|b| b.get("usd")),
        "description": run.spec.description, "steps": steps, "phases": phases, "notes": notes, "done": done, "total": total,
        "startedAt": rc.started_at, "endedAt": ended_at, "held": rc.held, "spec": rc.spec,
    })
}

/// Writes the run's view into its owner's slot (the other runs kept; a held run task also carries it at the top, as
/// `wisp run --wait` reads it).
async fn write_view(ctx: &Ctx, call: &Call, st: &State, rc: &RunCtx, v: Value) {
    let _one = st.slots.lock().await;
    let id = SessionId(rc.owner.clone());
    // a write over a slot that changed since it was read (another plugin's step slot, a restart's fix) reads it again
    for _ in 0..3 {
        let Ok(t) = ctx.run::<r::Get>(call, r::IdArgs { id: id.clone() }).await else { return };
        let was = t.ext.get(EXT).cloned().unwrap_or(Value::Null);
        let slot = with_run(&was, &v, rc.held);
        let a = r::SetExtArgs { id: id.clone(), plugin: EXT.into(), value: slot, expect: Some(r::ExtExpect { value: was }) };
        match ctx.run::<r::SetExt>(call, a).await {
            Ok(_) => return,
            Err(err) if err.code == wisp_kernel::Code::Conflict => {}
            Err(err) => {
                tracing::warn!(error = %err.message, task = rc.owner, "workflows: runtime.setExt");
                return;
            }
        }
    }
}

/// `was` (a task's slot) with run `v` in its `runs`: replaced, or added (the newest few kept; one still going never
/// dropped). A held run task also carries the view at the top.
fn with_run(was: &Value, v: &Value, held: bool) -> Value {
    let mut slot = Some(was.clone()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    let id = v.get("id").and_then(Value::as_str);
    let mut runs: Vec<Value> = slot.get("runs").and_then(Value::as_array).cloned().unwrap_or_default();
    match runs.iter_mut().find(|x| x.get("id").and_then(Value::as_str) == id) {
        Some(x) => x.clone_from(v),
        None => runs.push(v.clone()),
    }
    while runs.len() > KEEP {
        let Some(i) = runs.iter().position(|x| x.get("state").and_then(Value::as_str) != Some("running")) else { break };
        runs.remove(i);
    }
    if let Some(o) = slot.as_object_mut() {
        o.insert("runs".into(), Value::Array(runs));
        if held {
            for k in ["workflow", "state", "cost", "steps", "notes"] {
                o.insert(k.into(), v.get(k).cloned().unwrap_or(Value::Null));
            }
        }
    }
    slot
}

pub(crate) async fn set_state(ctx: &Ctx, call: &Call, id: &str, state: SessionState) {
    if let Err(err) = ctx.run::<r::SetState>(call, r::SetStateArgs { id: SessionId(id.to_owned()), state }).await {
        tracing::warn!(error = %err.message, task = id, "workflows: runtime.setState");
    }
}

/// How a step's task stands at a look.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Look {
    /// still working, or waiting on you; `held`: its turn waits before it starts (a limit, trust, the plan), and why
    Going {
        held: Option<String>,
    },
    Ended {
        ok: bool,
        output: Value,
        why: String,
    },
    /// it couldn't be read just now: looked at again next time
    Retry,
}

/// How the node's task stands (`goal`: the step has a goal of its own, judged before the step counts as done).
async fn ended(ctx: &Ctx, call: &Call, task: &str, goal: bool) -> Look {
    let id = SessionId(task.to_owned());
    let t = match ctx.run::<r::Get>(call, r::IdArgs { id: id.clone() }).await {
        Ok(t) => t,
        Err(e) => return unread(&e),
    };
    if let Some(going) = busy(&t) {
        return going;
    }
    match ctx.run::<r::Turns>(call, r::TurnsArgs { id, before: None, limit: None, masked: None }).await {
        Ok(turns) => look(&t, turns.last(), goal),
        Err(e) => unread(&e),
    }
}

/// A step's task that couldn't be read: gone (deleted) ends the step failed, or the run waited on it for ever;
/// anything else is looked at again.
pub(crate) fn unread(e: &WireError) -> Look {
    if e.code == wisp_kernel::Code::NotFound {
        Look::Ended { ok: false, output: Value::Null, why: "its task was deleted".into() }
    } else {
        Look::Retry
    }
}

/// Running, waiting on you, or something queued (a held queue waits to be started): still going.
fn busy(t: &Session) -> Option<Look> {
    if matches!(t.state, SessionState::Running | SessionState::Waiting) || (!t.queue.is_empty() && !t.queue_held) {
        return Some(Look::Going { held: None });
    }
    (!t.queue.is_empty()).then(|| Look::Going { held: Some("it waits to be started".into()) })
}

/// Why a held turn waits, in a few words: what its holder said, else its kind.
fn held_why(l: &Turn) -> String {
    let Some(h) = &l.held else { return "it waits before it starts".into() };
    ["why", "line", "label"]
        .iter()
        .find_map(|k| h.data.get(*k).and_then(Value::as_str).filter(|s| !s.trim().is_empty()))
        .map_or_else(|| h.kind.clone(), str::to_owned)
}

/// How a step stands from its task and last turn. A held turn (a limit, trust, the plan) waits, as does a step whose
/// goal its judges haven't met yet (working, judging, another round); met is done, stalled is failed with why.
pub(crate) fn look(t: &Session, last: Option<&Turn>, goal: bool) -> Look {
    if let Some(going) = busy(t) {
        return going;
    }
    if let Some(l) = last.filter(|l| l.status.is_live()) {
        let held = (l.status == TurnStatus::Held).then(|| held_why(l));
        return Look::Going { held };
    }
    let (ok, output, why) = outcome(t.state, last);
    if !(ok && goal) {
        return Look::Ended { ok, output, why };
    }
    let slot = t.ext.get(wisp_goal_api::PLUGIN).and_then(|v| serde_json::from_value::<wisp_goal_api::GoalState>(v.clone()).ok());
    let st = slot.unwrap_or_default();
    match st.state.as_str() {
        // dropped: the turn's answer stands, as a step without a goal
        "met" | "cleared" => Look::Ended { ok, output, why },
        "stalled" | "failed" | "limit" => {
            let why = if st.why.is_empty() { format!("its goal ended {}", st.state) } else { format!("its goal wasn't met: {}", st.why) };
            Look::Ended { ok: false, output, why }
        }
        _ => Look::Going { held: None },
    }
}

/// How a step ended, from its task's state and its last turn: done with no error is ok.
pub(crate) fn outcome(state: SessionState, last: Option<&Turn>) -> (bool, Value, String) {
    let ok = last.is_some_and(|l| l.status == TurnStatus::Done && !l.is_error);
    let why = if ok { String::new() } else { format!("it ended {}", last.map_or_else(|| wire_name(state), |l| wire_name(l.status))) };
    let output = last.and_then(|l| serde_json::to_value(l).ok()).map_or(Value::Null, |l| output_of(&l));
    (ok, output, why)
}

/// What the owner is sent when the run ended: how, then each step with what it gave or why not.
pub(crate) fn ended_message(name: &str, state: &str, run: &Run, secs: i64) -> String {
    let mut out = format!(
        "wisp: the workflow {name} ended {state} ({} of {} steps done, {}s, ${:.2}). Each step's task holds its whole answer.",
        run.runs.values().filter(|r| r.state == StepState::Done).count(),
        run.nodes.len(),
        secs.max(0),
        run.cost()
    );
    for n in &run.nodes {
        let r = &run.runs[&n.id];
        let mark = match r.state {
            StepState::Done => "done",
            StepState::Failed => "failed",
            StepState::Skipped => "skipped",
            StepState::Running | StepState::Waiting => "not run",
        };
        // the agent reads a structured output exactly (JSON); a text answer as its line
        let said = match &r.output {
            Value::Object(o) if !o.contains_key("text") => short(&r.output.to_string(), SAY_CHARS),
            other => output_line(other, SAY_CHARS),
        };
        let tail = match (said.is_empty(), r.why.is_empty()) {
            (false, _) if r.state == StepState::Done => format!(": {said}"),
            (_, false) => format!(": {}", r.why),
            _ => String::new(),
        };
        let task = r.task.as_deref().map(|t| format!(" (task {t})")).unwrap_or_default();
        let _ = write!(out, "\n- {} [{mark}]{task}{tail}", n.id);
    }
    out
}

/// Each running step's task looked at: the ones that ended are finished (with what they cost), a held one says why.
/// The steps that failed for good.
async fn look_at_steps(
    ctx: &Ctx,
    call: &Call,
    run: &mut Run,
    costs: &mut BTreeMap<String, f64>,
    holds: &mut BTreeMap<String, String>,
    now: i64,
) -> Vec<String> {
    let mut failed = Vec::new();
    let running: Vec<(String, String)> = run
        .runs
        .iter()
        .filter_map(|(id, r)| (r.state == StepState::Running).then(|| r.task.clone().map(|t| (id.clone(), t))).flatten())
        .collect();
    for (node, task) in running {
        let goal = run.nodes.iter().find(|n| n.id == node).is_some_and(|n| n.spec.goal.as_ref().is_some_and(|g| !g.is_null()));
        match ended(ctx, call, &task, goal).await {
            Look::Ended { ok, output, why } => {
                holds.remove(&node);
                run.finished(&node, ok, output, &why, costs.remove(&task).unwrap_or(0.0));
                run.ended_at(&node, now);
                if run.runs.get(&node).is_some_and(|r| r.state == StepState::Failed) {
                    failed.push(node);
                }
            }
            Look::Going { held: Some(why) } => {
                holds.insert(node, why);
            }
            Look::Going { held: None } => {
                holds.remove(&node);
            }
            Look::Retry => {}
        }
    }
    failed
}

/// Runs the workflow under its owner until every step has ended, or it's stopped.
async fn drive(
    ctx: Ctx,
    st: Arc<State>,
    mut wake: broadcast::Receiver<(String, f64)>,
    mut ctl: mpsc::UnboundedReceiver<Ctl>,
    rc: RunCtx,
    mut run: Run,
) {
    let call = ctx.root_call();
    let Ok(clock) = ctx.clock() else { return };
    let mut notes: Vec<String> = Vec::new();
    for n in &run.nodes {
        if !n.spec.on.is_empty() && !notes.iter().any(|x| x.contains(&n.step)) {
            notes.push(format!("{} runs on this computer: steps on another machine ({}) aren't wired yet", n.step, n.spec.on));
        }
    }
    let mut costs: BTreeMap<String, f64> = BTreeMap::new();
    let mut holds: BTreeMap<String, String> = BTreeMap::new();
    let mut last = Value::Null;
    let mut stopped = false;
    loop {
        for a in run.next(clock.now_ms()) {
            match a {
                Action::Start { node, prompt, steps } => match child_args(&run, &rc, &node, prompt, steps) {
                    Ok(args) => match ctx.run::<r::Create>(&call, args).await {
                        Ok(made) => run.started(&node, &made.id.0),
                        Err(e) => run.finished(&node, false, Value::Null, &e.message, 0.0),
                    },
                    Err(why) => run.finished(&node, false, Value::Null, &why, 0.0),
                },
                Action::Stop { node, task } => {
                    if let Err(err) = ctx.run::<r::Stop>(&call, r::StopArgs { id: SessionId(task.clone()), clear_queue: false }).await {
                        tracing::warn!(error = %err.message, task, "workflows: runtime.stop");
                    }
                    run.finished(&node, false, Value::Null, "it ran past its timeout", costs.remove(&task).unwrap_or(0.0));
                    run.ended_at(&node, clock.now_ms());
                }
            }
        }
        // written when it changed, not on every look
        let v = view(&run, &rc, "running", &notes, None, &holds);
        if v != last {
            write_view(&ctx, &call, &st, &rc, v.clone()).await;
            last = v;
        }
        if run.over() {
            break;
        }
        // a turn ended somewhere, you stopped it, or it's time to look at the timeouts
        tokio::select! {
            got = wake.recv() => {
                if let Ok((task, cost)) = got {
                    *costs.entry(task).or_default() += cost;
                }
            }
            got = ctl.recv() => match got {
                Some(Ctl::Edit(a, by, cap, reply)) => {
                    // all or nothing: changed on a copy, kept only when every part took
                    let mut next = run.clone();
                    match crate::edit::apply(&mut next, &a, &by, cap, clock.now_ms()) {
                        Ok(stop) => {
                            run = next;
                            stop_tasks(&ctx, &call, stop).await;
                            let _ = reply.send(Ok(()));
                        }
                        Err(why) => {
                            let _ = reply.send(Err(why));
                        }
                    }
                    continue;
                }
                Some(Ctl::Stop) | None => {
                    stop_tasks(&ctx, &call, run.stop(clock.now_ms())).await;
                    stopped = true;
                    break;
                }
            },
            () = clock.sleep(LOOK_MS) => {}
        }
        let failed = look_at_steps(&ctx, &call, &mut run, &mut costs, &mut holds, clock.now_ms()).await;
        // a step failed for good while the run goes on: its agent hears now, so it can retry or skip it (the run's end
        // says the rest)
        if !failed.is_empty() && !run.over() && rc.notify && !rc.held {
            crate::edit::tell_failed(&ctx, &call, &rc, &run, &failed).await;
        }
    }
    finish(&ctx, &st, &call, &rc, &run, stopped, &notes).await;
    st.keep_ended(rc, run);
}

/// Stops these steps' tasks (skipped, or the run stopped).
async fn stop_tasks(ctx: &Ctx, call: &Call, tasks: Vec<String>) {
    for task in tasks {
        if let Err(err) = ctx.run::<r::Stop>(call, r::StopArgs { id: SessionId(task.clone()), clear_queue: true }).await {
            tracing::warn!(error = %err.message, task, "workflows: runtime.stop");
        }
    }
}

/// Starts an ended run again under its task (a retry, a step added): its driver picks up where it was, and the task
/// is in Working on it again.
pub(crate) fn revive(ctx: &Ctx, st: &Arc<State>, rc: RunCtx, run: Run) {
    let (tx, rx) = mpsc::unbounded_channel();
    st.live.lock().unwrap_or_else(PoisonError::into_inner).insert(rc.id.clone(), Live { owner: rc.owner.clone(), ctl: tx });
    let (c, owner) = (ctx.clone(), rc.owner.clone());
    ctx.spawn(async move { set_state(&c, &c.root_call(), &owner, SessionState::Running).await });
    ctx.spawn(drive(ctx.clone(), st.clone(), st.ended.subscribe(), rx, rc, run));
}

/// An ended run kept here, taken to change and start again (`None`: none by that id; put it back with `keep_ended`
/// when the change is refused).
pub(crate) fn take_ended(st: &State, id: &str) -> Option<(RunCtx, Run)> {
    st.take_ended(id)
}

pub(crate) fn put_back(st: &State, rc: RunCtx, run: Run) {
    st.keep_ended(rc, run);
}

/// The run ended (or you stopped it): its view says how, `workflow.ended` tells, and its task lands or hears.
async fn finish(ctx: &Ctx, st: &State, call: &Call, rc: &RunCtx, run: &Run, stopped: bool, notes: &[String]) {
    let Ok(clock) = ctx.clock() else { return };
    let state = if stopped {
        "stopped"
    } else if run.ok() {
        "done"
    } else {
        "failed"
    };
    let now = clock.now_ms();
    write_view(ctx, call, st, rc, view(run, rc, state, notes, Some(now), &BTreeMap::new())).await;
    st.live.lock().unwrap_or_else(PoisonError::into_inner).remove(&rc.id);
    let total = u32::try_from(run.nodes.len()).unwrap_or(u32::MAX);
    let done = u32::try_from(run.runs.values().filter(|r| r.state == StepState::Done).count()).unwrap_or(u32::MAX);
    let ended = WorkflowEnded {
        session_id: rc.owner.clone(),
        run: rc.id.clone(),
        workflow: rc.name.clone(),
        state: state.to_owned(),
        done,
        total,
        cost_usd: run.cost(),
    };
    ctx.emit(call, &ended);
    if rc.held {
        set_state(ctx, call, &rc.owner, if run.ok() && !stopped { SessionState::Review } else { SessionState::Error }).await;
        return;
    }
    // the owner was held in Working on this run (Hold): wisp's message with the results starts its agent's next turn;
    // with none to send (stopped, notify off), it lands, unless another of its runs still goes
    if rc.notify && !stopped {
        let text = ended_message(&rc.name, state, run, (now - rc.started_at) / 1000);
        let a = r::FollowUpArgs {
            id: SessionId(rc.owner.clone()),
            text,
            queue: Some(true),
            by: Some("workflow".into()),
            ..r::FollowUpArgs::default()
        };
        if let Err(err) = ctx.run::<r::FollowUp>(call, a).await {
            tracing::warn!(error = %err.message, task = rc.owner, "workflows: runtime.followUp");
        }
    } else if !st.owns_live(&rc.owner) {
        settle(ctx, call, &rc.owner).await;
    }
}

/// `runtime.settle`: the task the runs held in Working lands (a turn of it running is left to its own end).
pub(crate) async fn settle(ctx: &Ctx, call: &Call, task: &str) {
    if let Err(err) = ctx.run::<r::Settle>(call, r::SettleArgs { id: SessionId(task.to_owned()), quiet: false }).await {
        tracing::warn!(error = %err.message, task, "workflows: runtime.settle");
    }
}

/// The workflow a run runs: the one it was given, else the saved one by name.
async fn resolve(store: &Store, a: &RunArgs) -> Result<(String, WorkflowSpec)> {
    if let Some(spec) = a.spec.clone() {
        let name = if a.name.trim().is_empty() { "workflow".to_owned() } else { a.name.trim().to_owned() };
        let problems = check(&spec);
        if !problems.is_empty() {
            return Err(WireError::invalid(format!("the workflow can't run: {}", problems.join("; "))));
        }
        return Ok((name, spec));
    }
    let all = crate::runnable(store).await?;
    let Some(w) = all.into_iter().find(|w| w.name == a.name) else {
        return Err(WireError::not_found(format!(
            "no workflow named {} (pass `spec` to run one written now; workflows.list has yours, workflows.starters the prebuilt ones)",
            a.name
        )));
    };
    Ok((w.name, w.spec))
}

/// `workflows.run`.
pub(crate) async fn start(ctx: &Ctx, call: &Call, st: &Arc<State>, a: RunArgs) -> Result<Started> {
    let store = ctx.service::<Store>()?;
    let (name, mut spec) = resolve(&store, &a).await?;
    let missing: Vec<&String> = spec.inputs.iter().filter(|(k, v)| v.is_null() && !a.inputs.contains_key(*k)).map(|(k, _)| k).collect();
    if !missing.is_empty() {
        return Err(WireError::invalid(format!("{name} needs {}", missing.iter().map(|m| format!("{m}=…")).collect::<Vec<_>>().join(" "))));
    }
    // an agent's run is its own task's, in its folder, at its mode or lower (ADR-0112)
    let agent = call.caller().session.clone();
    if let Some(own) = &agent {
        if a.task.as_ref().is_some_and(|t| t != own) {
            return Err(WireError::forbidden("an agent's workflow runs under its own task: leave out task"));
        }
        if a.project.is_some() || spec.tasks.values().any(|s| !s.project.is_empty() || !s.on.is_empty()) || !spec.project.is_empty() {
            return Err(WireError::invalid("an agent's workflow runs in its task's folder on this computer: leave out project and on"));
        }
    }
    let owner = agent.clone().or_else(|| a.task.clone());
    let owner_task = match &owner {
        Some(o) => Some(ctx.run::<r::Get>(call, r::IdArgs { id: SessionId(o.clone()) }).await?),
        None => None,
    };
    if let (Some(_), Some(t)) = (&agent, &owner_task) {
        cap_modes(&mut spec, t.mode);
    }
    let project =
        a.project.clone().or_else(|| some(&spec.project)).or_else(|| owner_task.as_ref().map(|t| t.cwd.clone())).unwrap_or_default();
    let clock = ctx.clock()?;
    let (id, owner, held) = if let Some(o) = owner {
        (wisp_kernel::ids::uuid7(), o, false)
    } else {
        {
            let steps: Vec<String> = spec.tasks.keys().cloned().collect();
            let made = r::StartArgs {
                prompt: format!("Run the workflow {name}: {}", steps.join(", ")),
                title: Some(format!("{name} (workflow)")),
                hold: true,
                cwd: some(&project),
                lineage: Lineage { triggered_by: a.triggered_by.clone(), ..Lineage::default() },
                ext: [(EXT.to_owned(), json!({ "workflow": name, "state": "starting" }))].into(),
                ..r::StartArgs::default()
            };
            let id = ctx.run::<r::Create>(call, made).await?.id.0;
            set_state(ctx, call, &id, SessionState::Running).await;
            (id.clone(), id, true)
        }
    };
    let total = spec.tasks.len();
    let rc = RunCtx {
        id: id.clone(),
        owner: owner.clone(),
        held,
        name: name.clone(),
        project,
        notify: a.notify.unwrap_or(true),
        started_at: clock.now_ms(),
        spec: serde_json::to_value(&spec).unwrap_or(Value::Null),
        // a trigger names itself; an agent names none (its steps are its own task's)
        triggered_by: a.triggered_by.clone().filter(|_| agent.is_none()),
    };
    let (tx, rx) = mpsc::unbounded_channel();
    st.live.lock().unwrap_or_else(PoisonError::into_inner).insert(id.clone(), Live { owner, ctl: tx });
    let run = Run::new(spec, &a.inputs);
    let started = WorkflowStarted {
        session_id: rc.owner.clone(),
        run: id.clone(),
        workflow: name.clone(),
        steps: u32::try_from(run.nodes.len()).unwrap_or(u32::MAX),
    };
    ctx.emit(call, &started);
    ctx.spawn(drive(ctx.clone(), st.clone(), st.ended.subscribe(), rx, rc, run));
    let said = if held {
        format!("Started the workflow {name} ({total} steps) as task {id}.")
    } else {
        format!(
            "Started the workflow {name} ({total} steps), run {id}. It runs in the background and the user watches it live on this task. \
             Don't do its steps yourself: end your turn, and wisp sends you every step's result as a message when the run ends. \
             wisp also tells you if a step fails, so you can retry or skip it with workflows_edit. \
             workflows_status {{\"id\":\"{id}\"}} says where it is; workflows_stop stops it."
        )
    };
    Ok(Started { id, said })
}

/// The run's owner, for status: a run going here, else the agent's own task (an ended run's view stays there), else
/// the id as a task's (whose view holds its runs).
fn owner_for(st: &State, call: &Call, id: &str) -> String {
    st.owner_of(id).or_else(|| call.caller().session.clone()).unwrap_or_else(|| id.to_owned())
}

/// `workflows.status`: the run's view (or, given a task, every run it keeps).
pub(crate) async fn status(ctx: &Ctx, call: &Call, st: &State, id: &str) -> Result<Value> {
    let owner = owner_for(st, call, id);
    let t = ctx.run::<r::Get>(call, r::IdArgs { id: SessionId(owner) }).await?;
    let slot = t.ext.get(EXT).cloned().unwrap_or(Value::Null);
    let runs = slot.get("runs").and_then(Value::as_array).cloned().unwrap_or_default();
    // what an agent reads: where the run stands, not the spec it was given (it wrote it)
    let lean = |v: &Value| {
        let mut v = v.clone();
        if let Some(o) = v.as_object_mut() {
            o.remove("spec");
        }
        v
    };
    if let Some(v) = runs.iter().find(|x| x.get("id").and_then(Value::as_str) == Some(id)) {
        return Ok(lean(v));
    }
    let runs: Vec<Value> = runs.iter().map(lean).collect();
    if runs.is_empty() {
        return Err(WireError::not_found(format!("no workflow run {id}")));
    }
    let mut m = Map::new();
    m.insert("runs".into(), Value::Array(runs));
    Ok(Value::Object(m))
}

/// `workflows.stop`: the run's working steps stop and nothing new starts (an agent stops only its own task's runs).
pub(crate) fn stop(call: &Call, st: &State, id: &str) -> Result<Value> {
    let live = st.live.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(l) = live.get(id) else {
        return Err(WireError::not_found(format!("no workflow run {id} is going")));
    };
    if call.caller().session.as_ref().is_some_and(|s| *s != l.owner) {
        return Err(WireError::forbidden("an agent stops only its own task's workflow runs"));
    }
    let _ = l.ctl.send(Ctl::Stop);
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use wisp_workflows_api::StepSpec;

    #[test]
    fn an_agents_steps_never_run_above_its_mode() {
        let step = |mode: &str| StepSpec { prompt: "p".into(), mode: mode.into(), ..StepSpec::default() };
        let mut w = WorkflowSpec {
            tasks: [("a".to_owned(), step("")), ("b".to_owned(), step("auto")), ("c".to_owned(), step("plan"))].into(),
            ..WorkflowSpec::default()
        };
        cap_modes(&mut w, Mode::Edits);
        assert_eq!(w.tasks["a"].mode, "edits", "none said: the task's own");
        assert_eq!(w.tasks["b"].mode, "edits", "above: down to the task's");
        assert_eq!(w.tasks["c"].mode, "plan", "lower stays");
    }

    #[test]
    fn the_owner_is_told_each_steps_result() {
        let w = WorkflowSpec {
            tasks: [
                ("a".to_owned(), StepSpec { prompt: "a".into(), ..StepSpec::default() }),
                ("b".to_owned(), StepSpec { prompt: "b".into(), needs: vec!["a".into()], ..StepSpec::default() }),
            ]
            .into(),
            ..WorkflowSpec::default()
        };
        let mut run = Run::new(w, &BTreeMap::new());
        run.next(0);
        run.started("a", "T1");
        run.finished("a", true, json!({ "text": "found 3 bugs" }), "", 0.25);
        run.ended_at("a", 1);
        run.next(1);
        run.finished("b", false, Value::Null, "it ended error", 0.0);
        let m = ended_message("audit", "failed", &run, 42);
        assert!(m.starts_with("wisp: the workflow audit ended failed (1 of 2 steps done, 42s, $0.25)"), "{m}");
        assert!(m.contains("- a [done] (task T1): found 3 bugs"), "{m}");
        assert!(m.contains("- b [failed]: it ended error"), "{m}");
    }

    #[test]
    fn a_run_is_put_in_its_tasks_slot_beside_whats_there() {
        let was = json!({ "run": "r0", "node": "a", "runs": [{ "id": "r1", "state": "done" }] });
        let s = with_run(&was, &json!({ "id": "r2", "state": "running", "workflow": "w" }), false);
        assert_eq!(s["node"], "a", "a step's own fields stay");
        assert_eq!(s["runs"].as_array().map(Vec::len), Some(2));
        let s = with_run(&s, &json!({ "id": "r2", "state": "done" }), true);
        assert_eq!(s["runs"][1]["state"], "done");
        assert_eq!(s["state"], "done", "a held run task carries it at the top");
        let mut many = Value::Null;
        for i in 0..12 {
            many = with_run(&many, &json!({ "id": format!("r{i}"), "state": if i == 0 { "running" } else { "done" } }), false);
        }
        let runs = many["runs"].as_array().cloned().unwrap_or_default();
        assert_eq!(runs.len(), KEEP);
        assert_eq!(runs[0]["id"], "r0", "the one still going stays");
    }

    fn turn(status: TurnStatus) -> Turn {
        Turn { id: "u1".into(), session_id: "s1".into(), status, text: Some("done".into()), ..Turn::default() }
    }

    fn task(state: SessionState) -> Session {
        Session { id: "s1".into(), state, ..Session::default() }
    }

    /// A step whose turn waits for a limit's reset (or trust, or the plan) failed with "it ended held"; the board now
    /// shows it waiting, with why.
    #[test]
    fn a_held_turn_is_still_going_and_says_why() {
        let mut held = turn(TurnStatus::Held);
        held.held = Some(wisp_runtime_api::model::Held {
            by: "wisp.accounts".into(),
            kind: "limit".into(),
            until: None,
            data: json!({ "line": "claude's limit resets at 14:00" }),
        });
        assert_eq!(
            look(&task(SessionState::Review), Some(&held), false),
            Look::Going { held: Some("claude's limit resets at 14:00".into()) }
        );
        held.held = None;
        assert!(matches!(look(&task(SessionState::Review), Some(&held), false), Look::Going { held: Some(_) }));
        let mut queued = task(SessionState::Review);
        queued.queue.push(wisp_runtime_api::QueueItem::default());
        queued.queue_held = true;
        assert_eq!(look(&queued, None, false), Look::Going { held: Some("it waits to be started".into()) }, "a held queue waits");
        assert!(matches!(look(&task(SessionState::Review), Some(&turn(TurnStatus::Done)), false), Look::Ended { ok: true, .. }));
    }

    /// A deleted step's task was read as "can't be read" forever: the run never ended.
    #[test]
    fn a_deleted_steps_task_fails_it_and_a_passing_error_looks_again() {
        assert_eq!(
            unread(&WireError::not_found("no task s1")),
            Look::Ended { ok: false, output: Value::Null, why: "its task was deleted".into() }
        );
        assert_eq!(unread(&WireError::unavailable("busy")), Look::Retry);
    }

    /// A step with a goal of its own counts as done once its judges met it, not when its first turn ended.
    #[test]
    fn a_steps_goal_is_judged_before_it_counts_as_done() {
        let with_goal = |state: &str, why: &str| {
            let mut t = task(SessionState::Review);
            t.ext.insert(wisp_goal_api::PLUGIN.into(), json!({ "state": state, "rounds": 1, "why": why }));
            t
        };
        let done = turn(TurnStatus::Done);
        for going in ["", "judging", "again"] {
            assert_eq!(look(&with_goal(going, ""), Some(&done), true), Look::Going { held: None }, "{going:?}");
        }
        assert_eq!(look(&task(SessionState::Review), Some(&done), true), Look::Going { held: None }, "not judged yet");
        assert!(matches!(look(&with_goal("met", ""), Some(&done), true), Look::Ended { ok: true, .. }));
        let Look::Ended { ok, why, .. } = look(&with_goal("stalled", "3 rounds and still no test"), Some(&done), true) else { panic!() };
        assert!(!ok);
        assert_eq!(why, "its goal wasn't met: 3 rounds and still no test");
        assert!(matches!(look(&with_goal("judging", ""), Some(&done), false), Look::Ended { ok: true, .. }), "a step without a goal");
        assert!(matches!(look(&with_goal("judging", ""), Some(&turn(TurnStatus::Error)), true), Look::Ended { ok: false, .. }));
    }

    /// A trigger's workflow run: its steps carry the trigger as their lineage, so their own `task.finished` never fires it.
    #[test]
    fn a_step_carries_the_trigger_that_started_its_run() {
        let w = WorkflowSpec {
            tasks: [("a".to_owned(), StepSpec { prompt: "a".into(), ..StepSpec::default() })].into(),
            ..WorkflowSpec::default()
        };
        let run = Run::new(w, &BTreeMap::new());
        let rc = RunCtx {
            id: "r1".into(),
            owner: "o1".into(),
            held: true,
            name: "w".into(),
            project: "/repo".into(),
            notify: false,
            started_at: 0,
            spec: Value::Null,
            triggered_by: Some("trigger:nightly".into()),
        };
        let a = child_args(&run, &rc, "a", "a".into(), Vec::new()).unwrap();
        assert_eq!((a.lineage.workflow_run.as_deref(), a.lineage.triggered_by.as_deref()), (Some("o1"), Some("trigger:nightly")));
    }

    #[test]
    fn an_output_line_is_its_text_or_its_fields() {
        assert_eq!(output_line(&json!({ "text": "ok", "status": "done" }), 10), "ok");
        assert_eq!(output_line(&json!({ "text": "long", "title": "Tests pass" }), 40), "Tests pass");
        assert_eq!(output_line(&json!({ "file": "one.txt", "lines": 3 }), 40), "file: one.txt · lines: 3");
        assert_eq!(output_line(&json!({ "files": ["a", "b"] }), 40), "files: 2 items");
        assert_eq!(output_line(&json!("abcdef"), 3), "abc…");
    }
}
