//! A factory run's driver: what wisp-factory-core's machine says, done. Stations' tasks are made with `runtime.create`
//! as the run's children (`parentId`, `lineage.workflowRun`, `ext["wisp.factory"] = { run, station }`; runtime holds them
//! to the agent that started the run, if one did) and followed up with `runtime.followUp`; it waits on `turn.ended` (a
//! 2 s look for a Stop), asks on the parent with `runtime.ask` (its answer read with `runtime.pollAnswer`), runs checks
//! in the build's folder (wisp's own variables and every secret left out, the output masked, the process tree killed on
//! Stop), ships with `ship.ship` and merges with `ship.merge`. Where the run stands, the machine and the tasks it waits
//! on are written on the parent after every step (`runtime.setExt`, its one writer: this run's own loop), so a restart
//! picks it up where it was.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::broadcast;
use wisp_factory_api::{EXT, FactorySpec, RunState};
use wisp_factory_core::{CheckRun, Event, Input, Next, Resume, Run, TaskEnd, TaskStep, check_env, short, tail};
use wisp_kernel::{Call, Ctx, Result, WireError};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::spec::Lineage;
use wisp_runtime_api::{Mode, SessionId, SessionState, TurnStatus};

use crate::{State, goal_of, lock};

/// How often a run looks when nothing woke it (a Stop, a question's answer).
const LOOK_MS: u64 = 2_000;
/// How often a question's answer is looked for.
const ANSWER_MS: u64 = 500;

/// A task a station waits on: until `turn` (a follow-up's) has ended, or (None) its first turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Pending {
    task: String,
    turn: Option<String>,
}

fn mode_of(m: &str) -> Option<Mode> {
    if m.is_empty() { None } else { serde_json::from_value(json!(m)).ok() }
}

fn some(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_owned())
}

async fn get(ctx: &Ctx, call: &Call, id: &str) -> Result<wisp_runtime_api::model::Session> {
    ctx.run::<r::Get>(call, r::IdArgs { id: SessionId(id.to_owned()) }).await
}

async fn set_state(ctx: &Ctx, call: &Call, id: &str, state: SessionState) {
    if let Err(err) = ctx.run::<r::SetState>(call, r::SetStateArgs { id: SessionId(id.to_owned()), state }).await {
        tracing::warn!(error = %err.message, task = id, "factory: runtime.setState");
    }
}

/// The run's slot: its view, what it started from, and the machine with the tasks it waits on (what a restart picks up).
async fn report(ctx: &Ctx, call: &Call, id: &str, m: &Run, input: &Value, pending: &[Pending]) {
    let mut value = serde_json::to_value(m.view()).unwrap_or_else(|_| json!({}));
    value["input"] = input.clone();
    value["machine"] = serde_json::to_value(m).unwrap_or(Value::Null);
    value["pending"] = serde_json::to_value(pending).unwrap_or(Value::Null);
    // set-ext: whole one writer: the run's task is made by its create and only this run's loop writes this slot
    let a = r::SetExtArgs { id: SessionId(id.to_owned()), plugin: EXT.into(), value, expect: None };
    if let Err(err) = ctx.run::<r::SetExt>(call, a).await {
        tracing::warn!(error = %err.message, task = id, "factory: runtime.setExt");
    }
}

/// The project's own test command for a folder (the checks station's default), '' when none.
async fn project_test(ctx: &Ctx, call: &Call, cwd: &str) -> String {
    let a = wisp_resources_api::ExportArgs { kinds: vec!["project".into()], include: Vec::new() };
    let Ok(projects) = ctx.run::<wisp_resources_api::Export>(call, a).await else { return String::new() };
    let mut best: Option<(usize, String)> = None;
    for p in &projects {
        let (Some(path), Some(test)) = (p.spec.get("path").and_then(Value::as_str), p.spec.get("test").and_then(Value::as_str)) else {
            continue;
        };
        if !test.trim().is_empty() && wisp_runtime_api::paths::is_within(cwd, path) && best.as_ref().is_none_or(|(n, _)| path.len() > *n) {
            best = Some((path.len(), test.to_owned()));
        }
    }
    best.map(|(_, t)| t).unwrap_or_default()
}

/// The parent's queued prompt (it was made held) goes: the run's own task never runs an agent.
async fn drop_queue(ctx: &Ctx, call: &Call, id: &str) {
    // the create puts its held prompt in the queue just after it says the task was made
    for _ in 0..40 {
        if let Ok(s) = get(ctx, call, id).await
            && let Some(item) = s.queue.first()
        {
            let a = r::QueueItemArgs { id: SessionId(id.to_owned()), item_id: item.id.clone() };
            if let Err(err) = ctx.run::<r::QueueCancel>(call, a).await {
                tracing::warn!(error = %err.message, task = id, "factory: runtime.queue.cancel");
            }
            return;
        }
        if let Ok(clock) = ctx.clock() {
            clock.sleep(25).await;
        }
    }
}

/// What the run's task was made with: the machine's input.
async fn input_of(ctx: &Ctx, call: &Call, parent: &wisp_runtime_api::model::Session, input_v: &Value) -> Input {
    let text = |k: &str| input_v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let spec: FactorySpec = serde_json::from_value(input_v.get("spec").cloned().unwrap_or_default()).unwrap_or_default();
    Input {
        factory: text("factory"),
        spec,
        prompt: text("prompt"),
        title: if parent.title.trim().is_empty() { short(&text("prompt"), 60) } else { parent.title.clone() },
        agent: text("agent"),
        // the run task's own mode as it was made (held to the agent that asked, ADR-0111): what no station goes above
        mode: serde_json::to_value(parent.mode).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_else(|| text("mode")),
        worktree: input_v.get("worktree").and_then(Value::as_bool),
        goal: input_v.get("goal").and_then(goal_of),
        test: project_test(ctx, call, &parent.cwd).await,
        by_agent: input_v.get("byAgent").and_then(Value::as_bool).unwrap_or(false),
        sandbox: parent.ext.contains_key("wisp.sandbox"),
    }
}

/// Drives the run on task `id` from its start.
pub(crate) async fn run(ctx: Ctx, st: Arc<State>, id: String) {
    let call = ctx.root_call();
    let Ok(parent) = get(&ctx, &call, &id).await else { return };
    let slot = parent.ext.get(EXT).cloned().unwrap_or_default();
    if slot.get("state").and_then(Value::as_str) != Some("starting") {
        return;
    }
    let input_v = slot.get("input").cloned().unwrap_or_default();
    let input = input_of(&ctx, &call, &parent, &input_v).await;
    drop_queue(&ctx, &call, &id).await;
    let mut m = Run::new(input);
    set_state(&ctx, &call, &id, SessionState::Running).await;
    let first = m.start();
    let place = (parent.cwd.clone(), parent.dirs.clone());
    drive(&ctx, &st, &id, m, &input_v, &place, first).await;
}

/// Picks up a run wisp was stopped in the middle of, where it was: the tasks its station waits on, the question it
/// asks, or the step to do again. False: there was no machine to pick up (a run from before it was kept).
pub(crate) async fn resume(ctx: Ctx, st: Arc<State>, id: String) -> bool {
    let call = ctx.root_call();
    let Ok(parent) = get(&ctx, &call, &id).await else { return false };
    let slot = parent.ext.get(EXT).cloned().unwrap_or_default();
    let Some(mut m) = slot.get("machine").and_then(|v| serde_json::from_value::<Run>(v.clone()).ok()) else { return false };
    let input_v = slot.get("input").cloned().unwrap_or_default();
    let place = (parent.cwd.clone(), parent.dirs.clone());
    let saved: Vec<Pending> = slot.get("pending").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    let mut wake = st.ended.subscribe();
    let event = match m.resume() {
        Resume::Over => return true,
        Resume::Redo(next) => return drive_on(&ctx, &st, &id, m, &input_v, &place, next).await,
        Resume::Tasks { tasks } => {
            // the turns it was waiting on, as they were saved; else the station's tasks' last turns
            let pending = if saved.is_empty() { tasks.into_iter().map(|task| Pending { task, turn: None }).collect() } else { saved };
            wait(&ctx, &call, &st, &mut wake, &id, &m, &pending).await
        }
        Resume::Ask { ask_id } if !ask_id.is_empty() => poll(&ctx, &call, &st, &id, &ask_id).await,
        Resume::Ask { .. } => {
            // asked but its id never saved: ask again
            let w = m.view().waiting.clone().unwrap_or_default();
            ask(&ctx, &call, &st, &id, &mut m, &input_v, w.question, w.options, w.context).await
        }
    };
    let next = m.on(event);
    drive_on(&ctx, &st, &id, m, &input_v, &place, next).await
}

async fn drive_on(ctx: &Ctx, st: &Arc<State>, id: &str, m: Run, input_v: &Value, place: &(String, Vec<String>), next: Next) -> bool {
    drive(ctx, st, id, m, input_v, place, next).await;
    true
}

/// Does what the machine says until the run ends.
async fn drive(ctx: &Ctx, st: &Arc<State>, id: &str, mut m: Run, input_v: &Value, place: &(String, Vec<String>), first: Next) {
    let call = ctx.root_call();
    let mut wake = st.ended.subscribe();
    let mut next = first;
    loop {
        report(ctx, &call, id, &m, input_v, &[]).await;
        let event = match next {
            Next::End { state, .. } => {
                if state == RunState::Stopped {
                    stop_stations(ctx, &call, &m).await;
                }
                let s = if state == RunState::Failed { SessionState::Error } else { SessionState::Review };
                set_state(ctx, &call, id, s).await;
                lock(&st.stops).remove(id);
                lock(&st.cancels).remove(id);
                return;
            }
            Next::Tasks(steps) => match make(ctx, &call, id, &mut m, place, steps).await {
                Ok(pending) => {
                    report(ctx, &call, id, &m, input_v, &pending).await;
                    wait(ctx, &call, st, &mut wake, id, &m, &pending).await
                }
                Err(e) => e,
            },
            Next::Ask { question, options, context } => ask(ctx, &call, st, id, &mut m, input_v, question, options, context).await,
            Next::Checks { task, commands, minutes } => match stopped(st, id) {
                Some(why) => Event::Stop(why),
                None => checks(ctx, &call, st, id, &task, &commands, minutes).await,
            },
            Next::Ship { task, title, body, draft } => {
                let a = wisp_ship_api::ShipArgs { session: task, title, body, draft };
                match ctx.run::<wisp_ship_api::ShipIt>(&call, a).await {
                    Ok(s) => Event::Shipped { ok: !s.pr.url.is_empty(), pr: s.pr.url.clone(), why: "no PR came back".into() },
                    Err(e) => Event::Shipped { ok: false, pr: String::new(), why: e.message },
                }
            }
            Next::Merge { task } => match ctx.run::<wisp_ship_api::Merge>(&call, wisp_ship_api::SessionArgs { session: task }).await {
                Ok(m) => Event::Merged { ok: true, why: String::new(), local: if m.sync.ok { String::new() } else { m.sync.text } },
                Err(e) => Event::Merged { ok: false, why: e.message, local: String::new() },
            },
        };
        next = m.on(event);
    }
}

/// Every station's task still running stops (a Stop ends the run, its stations with it: review r194 factories #4).
async fn stop_stations(ctx: &Ctx, call: &Call, m: &Run) {
    for task in m.view().stations.iter().flat_map(|s| s.tasks.iter()) {
        if let Err(err) = ctx.run::<r::Stop>(call, r::StopArgs { id: SessionId(task.clone()), clear_queue: true }).await {
            tracing::debug!(error = %err.message, task, "factory: runtime.stop of a station");
        }
    }
}

fn stopped(st: &State, id: &str) -> Option<String> {
    lock(&st.stops).remove(id)
}

/// Makes or follows up a station's tasks: the turns to wait on.
async fn make(
    ctx: &Ctx,
    call: &Call,
    run: &str,
    m: &mut Run,
    place: &(String, Vec<String>),
    steps: Vec<TaskStep>,
) -> std::result::Result<Vec<Pending>, Event> {
    let mut pending = Vec::new();
    for step in steps {
        match step {
            TaskStep::Start { station, index, agent, title, prompt, mode, worktree, folder_of } => {
                let (cwd, dirs) = match folder_of {
                    Some(t) => match get(ctx, call, &t).await {
                        Ok(s) => (s.cwd, s.dirs),
                        Err(e) => return Err(Event::Failed(format!("{station}: its build's folder can't be read: {}", e.message))),
                    },
                    None => place.clone(),
                };
                let a = r::StartArgs {
                    prompt,
                    title: Some(title),
                    parent_id: Some(SessionId(run.to_owned())),
                    lineage: Lineage { workflow_run: Some(run.to_owned()), ..Lineage::default() },
                    ext: [(EXT.to_owned(), json!({ "run": run, "station": station }))].into(),
                    agent: some(&agent),
                    mode: mode_of(&mode),
                    cwd: some(&cwd),
                    dirs,
                    worktree,
                    ..r::StartArgs::default()
                };
                match ctx.run::<r::Create>(call, a).await {
                    Ok(made) => {
                        m.made(station, index, made.id.as_str());
                        pending.push(Pending { task: made.id.0, turn: made.turn_id.map(|t| t.0) });
                    }
                    Err(e) => return Err(Event::Failed(format!("the {station} station's task couldn't start: {}", e.message))),
                }
            }
            TaskStep::FollowUp { task, text } => {
                let a = r::FollowUpArgs {
                    id: SessionId(task.clone()),
                    text,
                    queue: Some(true),
                    by: Some("factory".into()),
                    ..r::FollowUpArgs::default()
                };
                match ctx.run::<r::FollowUp>(call, a).await {
                    Ok(f) => pending.push(Pending { task, turn: f.turn_id.map(|t| t.0).or(f.queued) }),
                    Err(e) => return Err(Event::Failed(format!("a follow-up couldn't be sent: {}", e.message))),
                }
            }
        }
    }
    Ok(pending)
}

/// Waits until each pending turn has ended; a Stop stops them, and the run's total passing its budget stops them too.
async fn wait(
    ctx: &Ctx,
    call: &Call,
    st: &State,
    wake: &mut broadcast::Receiver<(String, f64, String)>,
    run: &str,
    m: &Run,
    pending: &[Pending],
) -> Event {
    let mut ends: Vec<Option<TaskEnd>> = pending.iter().map(|_| None).collect();
    let mut costs: BTreeMap<String, f64> = BTreeMap::new();
    let Ok(clock) = ctx.clock() else { return Event::Failed("no clock".into()) };
    let stop_all = |why: String| async move {
        for p in pending {
            let _ = ctx.run::<r::Stop>(call, r::StopArgs { id: SessionId(p.task.clone()), clear_queue: true }).await;
        }
        why
    };
    loop {
        if let Some(why) = stopped(st, run) {
            return Event::Stop(stop_all(why).await);
        }
        for (i, p) in pending.iter().enumerate() {
            if ends[i].is_none()
                && let Some(mut e) = ended(ctx, call, p).await
            {
                e.cost = costs.remove(&p.task).unwrap_or(0.0);
                ends[i] = Some(e);
            }
        }
        if ends.iter().all(Option::is_some) {
            return Event::Ended(ends.into_iter().flatten().collect());
        }
        // a station's turn that ended took the run past its budget: the rest of the station stops now (review #6)
        let spent: f64 = costs.values().sum::<f64>() + ends.iter().flatten().map(|e| e.cost).sum::<f64>();
        if let Some(why) = m.over_budget(spent) {
            return Event::Failed(stop_all(why).await);
        }
        tokio::select! {
            got = wake.recv() => {
                if let Ok((task, cost, _)) = got
                    && pending.iter().any(|p| p.task == task)
                {
                    *costs.entry(task).or_default() += cost;
                }
            }
            () = clock.sleep(LOOK_MS) => {}
        }
    }
}

/// How the task stands: None while it runs (or waits on someone), else how its last turn (the one waited on) ended.
async fn ended(ctx: &Ctx, call: &Call, p: &Pending) -> Option<TaskEnd> {
    let t = get(ctx, call, &p.task).await.ok()?;
    let busy = matches!(t.state, SessionState::Running | SessionState::Waiting) || !t.queue.is_empty() && !t.queue_held;
    if busy {
        return None;
    }
    let turns =
        ctx.run::<r::Turns>(call, r::TurnsArgs { id: SessionId(p.task.clone()), before: None, limit: Some(3), masked: None }).await.ok()?;
    let last = turns.last();
    // a follow-up still queued (no turn of its id yet) hasn't run
    if let Some(want) = &p.turn
        && !turns.iter().any(|x| &x.id.0 == want)
        && t.queue.iter().any(|q| &q.id == want)
    {
        return None;
    }
    let Some(last) = last else {
        // held before its first turn: its worktree's setup failed, or it's still to start
        let why = t.ext.get("wisp.runtime").and_then(|r| r.get("setupFailed")).and_then(Value::as_str).map(str::to_owned);
        if why.is_none() && !t.queue.is_empty() {
            return None;
        }
        return Some(TaskEnd { task: p.task.clone(), ok: false, text: why.unwrap_or_else(|| "it never ran".into()), cost: 0.0 });
    };
    if last.status == TurnStatus::Running {
        return None;
    }
    let ok = last.status == TurnStatus::Done && !last.is_error;
    let text = last.text.clone().unwrap_or_default();
    let text = if ok || !text.is_empty() {
        text
    } else {
        format!("it ended {}", serde_json::to_value(last.status).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default())
    };
    Some(TaskEnd { task: p.task.clone(), ok, text, cost: 0.0 })
}

/// Asks on the run's task and waits for the answer.
#[allow(clippy::too_many_arguments)] // the run's place (ctx, call, state, id, run, input) and the question's three parts
async fn ask(
    ctx: &Ctx,
    call: &Call,
    st: &State,
    run: &str,
    m: &mut Run,
    input: &Value,
    question: String,
    options: Vec<String>,
    context: String,
) -> Event {
    let details = (!context.is_empty()).then(|| serde_json::json!({ "context": context }));
    let a = r::AskArgs { id: SessionId(run.to_owned()), question, options, details };
    let asked = match ctx.run::<r::Ask>(call, a).await {
        Ok(x) => x.ask_id,
        Err(e) => return Event::Failed(format!("couldn't ask: {}", e.message)),
    };
    m.asked(&asked);
    report(ctx, call, run, m, input, &[]).await;
    poll(ctx, call, st, run, &asked).await
}

/// The answer to the question `asked` on the run's task, once there is one.
async fn poll(ctx: &Ctx, call: &Call, st: &State, run: &str, asked: &str) -> Event {
    let Ok(clock) = ctx.clock() else { return Event::Failed("no clock".into()) };
    loop {
        if let Some(why) = stopped(st, run) {
            return Event::Stop(why);
        }
        match ctx.run::<r::PollAnswerOf>(call, r::PollAnswerArgs { ask_id: asked.to_owned() }).await {
            Ok(r::PollAnswer::Answered { answer }) => {
                set_state(ctx, call, run, SessionState::Running).await;
                return Event::Answered(answer);
            }
            Ok(r::PollAnswer::Pending) => clock.sleep(ANSWER_MS).await,
            // taken back: its task was stopped (`wisp stop <run>`), as `factory.stop` says it (audit G15)
            Ok(r::PollAnswer::Cancelled) => return Event::Stop("you stopped it".into()),
            Err(e) => return Event::Failed(format!("couldn't read the answer: {}", e.message)),
        }
    }
}

/// What a check printed, as it may be kept and shown: the build task's .env values and .env-shaped lines hidden
/// (wisp.runtime's mask; hidden whole when it isn't there).
fn masked(ctx: &Ctx, task: &str, text: String) -> String {
    let mut v = Value::String(text);
    match ctx.use_service::<wisp_runtime_api::env::EnvMaskService>() {
        Ok(m) => {
            m.mask(Some(task), &mut v);
        }
        Err(_) => {
            wisp_runtime_api::env::hide(&mut v, &[]);
        }
    }
    v.as_str().unwrap_or_default().to_owned()
}

/// Runs the check commands in the task's folder, in order, stopping at the first that fails. They see wisp's
/// environment without its own variables, tokens, secrets or harness keys; a Stop kills the running one's tree at once.
#[allow(clippy::too_many_arguments)]
async fn checks(ctx: &Ctx, call: &Call, st: &State, run: &str, task: &str, commands: &[String], minutes: u64) -> Event {
    let cwd = match get(ctx, call, task).await {
        Ok(s) => s.cwd,
        Err(e) => return Event::Failed(format!("checks: the build's folder can't be read: {}", e.message)),
    };
    let Ok(dir) = wisp_runtime_api::paths::Folder::of(&cwd) else {
        return Event::Failed("checks: the build has no folder to run them in".into());
    };
    let proc = match ctx.service::<wisp_proc::Proc>() {
        Ok(p) => p,
        Err(e) => return Event::Failed(e.message),
    };
    let scope = wisp_proc::Scope::new();
    lock(&st.cancels).insert(run.to_owned(), scope.clone());
    let env = wisp_proc::Env::exactly(check_env(wisp_proc::vars()));
    let mut out = Vec::new();
    for c in commands {
        let opts = wisp_proc::ExecOpts {
            cwd: Some(dir.clone().into_path()),
            env: env.clone(),
            timeout: Some(Duration::from_secs(minutes.saturating_mul(60))),
            scope: Some(scope.clone()),
            ..wisp_proc::ExecOpts::default()
        };
        let ran = proc.exec("sh", &["-c".to_owned(), c.clone()], opts).await;
        if scope.is_cancelled() {
            lock(&st.cancels).remove(run);
            return Event::Stop(stopped(st, run).unwrap_or_else(|| "stopped".into()));
        }
        let ran = match ran {
            Ok(r) => {
                let mut text = r.stdout_text();
                text.push_str(&r.stderr_text());
                CheckRun { command: c.clone(), code: r.exit.shell_code(), tail: masked(ctx, task, tail(&text, 30)), timed_out: r.timed_out }
            }
            Err(e) => CheckRun { command: c.clone(), code: 127, tail: e.to_string(), timed_out: false },
        };
        let failed = ran.code != 0 || ran.timed_out;
        out.push(ran);
        if failed {
            break;
        }
    }
    lock(&st.cancels).remove(run);
    Event::Checked(out)
}

/// `factory.stop`: the run's running check is killed and its stations stop; it ends stopped.
pub(crate) async fn stop(ctx: &Ctx, call: &Call, st: &State, id: &str, why: &str) -> Result<Value> {
    let s = get(ctx, call, id).await?;
    let state = s.ext.get(EXT).and_then(|v| v.get("state")).and_then(Value::as_str).unwrap_or_default();
    if !matches!(state, "starting" | "running" | "waiting") {
        return Err(WireError::conflict("This factory run isn't running."));
    }
    halt(st, id, why);
    Ok(json!({ "stopping": true }))
}

/// The run `id` stops: its loop sees why, and a check it's running is killed now.
pub(crate) fn halt(st: &State, id: &str, why: &str) {
    lock(&st.stops).insert(id.to_owned(), why.to_owned());
    if let Some(scope) = lock(&st.cancels).remove(id) {
        scope.cancel();
    }
}

/// A deleted run's stations go with it: their tasks, and their worktrees unless one holds work (review r194 factories #4).
pub(crate) async fn delete_stations(ctx: &Ctx, run: &str) {
    let call = ctx.root_call();
    let a = r::SlotsArgs { plugins: vec![EXT.into()], all: false, having: vec![run.to_owned()] };
    let Ok(found) = ctx.run::<r::Slots>(&call, a).await else { return };
    for t in found.into_iter().filter(|t| t.ext.get(EXT).and_then(|v| v.get("run")).and_then(Value::as_str) == Some(run)) {
        // its worktree first (a build's): removed as the worktrees plugin removes one, never with work in it (that one
        // stays, as Merge keeps it, for Worktrees to show)
        if let Ok(s) = get(ctx, &call, t.id.as_str()).await
            && let Some(w) =
                s.ext.get("wisp.worktrees").and_then(|w| serde_json::from_value::<wisp_worktrees_api::Worktree>(w.clone()).ok())
        {
            let a = wisp_worktrees_api::RemoveArgs { worktree: w, opts: wisp_worktrees_api::RemoveOpts::default() };
            match ctx.run::<wisp_worktrees_api::Remove>(&call, a).await {
                Ok(r) if !r.removed => {
                    tracing::info!(task = t.id.as_str(), dirty = r.dirty, "factory: a deleted run's worktree kept: it holds work");
                }
                Ok(_) => {}
                Err(err) => tracing::warn!(error = %err.message, task = t.id.as_str(), "factory: worktrees.remove"),
            }
        }
        if let Err(err) = ctx.run::<r::Delete>(&call, r::IdArgs { id: t.id.clone() }).await {
            tracing::warn!(error = %err.message, task = t.id.as_str(), "factory: deleting a deleted run's station");
        }
    }
}

/// At start: every run's own task is known (none of them runs an agent), and each run wisp was stopped in the middle
/// of picks up where it was; one made before its machine was kept says so and ends.
pub(crate) async fn recover(ctx: &Ctx, st: &Arc<State>) {
    let call = ctx.root_call();
    let a = r::SlotsArgs { plugins: vec![EXT.into()], all: false, having: vec![EXT.into()] };
    // wisp.runtime starts first (the manifest requires it), so its tasks can be read here
    let found = match ctx.run::<r::Slots>(&call, a).await {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(error = %e.message, "factory: the runs couldn't be read at start");
            st.ready.store(true, std::sync::atomic::Ordering::SeqCst);
            return;
        }
    };
    let mut todo = Vec::new();
    for t in found {
        let Some(slot) = t.ext.get(EXT) else { continue };
        if slot.get("factory").is_none() {
            continue;
        }
        lock(&st.runs).insert(t.id.as_str().to_owned());
        match slot.get("state").and_then(Value::as_str) {
            Some("starting") => todo.push((t.id.0.clone(), false)),
            Some("running" | "waiting") => todo.push((t.id.0.clone(), true)),
            _ => {}
        }
    }
    st.ready.store(true, std::sync::atomic::Ordering::SeqCst);
    for (id, going) in todo {
        let (c, s) = (ctx.clone(), st.clone());
        ctx.spawn(async move {
            if !going {
                run(c, s, id).await;
            } else if !resume(c.clone(), s, id.clone()).await {
                ended_by_restart(&c, &id).await;
            }
        });
    }
}

/// A run from before its machine was kept: it says wisp stopped and ends.
async fn ended_by_restart(ctx: &Ctx, id: &str) {
    let call = ctx.root_call();
    let Ok(t) = get(ctx, &call, id).await else { return };
    let Some(v) = t.ext.get(EXT).and_then(crate::interrupted) else { return };
    // set-ext: whole one writer: no run's loop drives this one after a restart, so nothing else writes this slot now
    let a = r::SetExtArgs { id: t.id.clone(), plugin: EXT.into(), value: v, expect: None };
    if ctx.run::<r::SetExt>(&call, a).await.is_ok() {
        set_state(ctx, &call, id, SessionState::Review).await;
    }
}
