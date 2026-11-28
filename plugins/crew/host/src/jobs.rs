//! The crew's commands and the scheduler (v1 crew.js `create`, `update`, `assign`, `checkin`, `tick`,
//! `drain`). A job is an ordinary task: `runtime.create` with the member's harness, folder and mode,
//! `ext["wisp.crew"]` saying whose job it is, `by: "crew"`.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_automations_api::TriggerSpec;
use wisp_crew_api::commands as c;
use wisp_crew_api::{CrewExt, JobKind, JobSummary, Member, MemberView, PLUGIN_ID, RunPick, TICK_MS, WaitingJob};
use wisp_crew_core::layout::{layout_of, normalize_layout};
use wisp_crew_core::note::checkin_prompt;
use wisp_crew_core::queue::{Placement, checkin_already, count_checkin, drain_count, place, place_delegated};
use wisp_crew_core::run::{at_cap, job_body, job_run};
use wisp_crew_core::schedule::due_now;
use wisp_crew_core::tick::{clear_wake, tick};
use wisp_kernel::{Call, Command as _, Ctx, Setup, WireError};
use wisp_runtime_api::commands as rt;

use crate::{Slot, State, calls, lock, wire};

pub(crate) fn now(ctx: &Ctx) -> i64 {
    ctx.clock().map_or(0, |c| c.now_ms())
}

/// A job's task: here, or on the machine (a sandbox) it runs on (`<machine>|<id>`).
pub(crate) async fn job_task(ctx: &Ctx, call: &Call, id: &str) -> Result<Value, WireError> {
    match id.split_once('|') {
        Some((machine, task)) => calls::routed::<rt::Get>(ctx, call, machine, json!({ "id": task })).await,
        None => calls::to_json(&calls::task(ctx, call, id).await?),
    }
}

/// A job's turns, here or in its sandbox.
pub(crate) async fn job_turns(ctx: &Ctx, call: &Call, id: &str) -> Result<Vec<wisp_runtime_api::model::Turn>, WireError> {
    match id.split_once('|') {
        Some((machine, task)) => {
            let v = calls::routed::<rt::Turns>(
                ctx,
                call,
                machine,
                calls::to_json(&rt::TurnsArgs { id: task.into(), before: None, limit: None, masked: None })?,
            )
            .await?;
            serde_json::from_value(v).map_err(|e| WireError::internal(format!("{}: {e}", rt::Turns::ID)))
        }
        None => calls::turns(ctx, call, id).await,
    }
}

/// A crew follow-up to a job, here or in its sandbox (queued behind a running turn).
pub(crate) async fn job_follow_up(ctx: &Ctx, call: &Call, id: &str, text: String) -> Result<(), WireError> {
    match id.split_once('|') {
        Some((machine, task)) => {
            let a = rt::FollowUpArgs { id: task.into(), text, queue: Some(true), by: Some("crew".into()), ..rt::FollowUpArgs::default() };
            calls::routed::<rt::FollowUp>(ctx, call, machine, calls::to_json(&a)?).await.map(|_| ())
        }
        None => calls::follow_up(ctx, call, id, text).await,
    }
}

/// A member's fallbacks as the runtime takes them (`runtime.create`'s `fallbacks`): where its job carries on when a
/// run stops at a limit.
pub(crate) fn fallbacks_of(m: &Member) -> Vec<wisp_runtime_api::commands::RunPick> {
    let some = |s: &str| (!s.is_empty()).then(|| s.to_owned());
    m.fallbacks
        .iter()
        .map(|f| wisp_runtime_api::commands::RunPick { agent: some(&f.agent), model: some(&f.model), effort: some(&f.effort) })
        .collect()
}

/// Where a member's job runs when it isn't here: on their machine (v1 crew.js `{ machine: m.machine }`, its folder
/// one there), a sandbox per job, or their own (ADR-0047). `(machine, cwd)`.
async fn sandbox_for(ctx: &Ctx, call: &Call, m: &Member) -> Result<Option<(String, String)>, WireError> {
    if !m.machine.is_empty() && m.sandbox.is_empty() {
        return Ok(Some((m.machine.clone(), m.cwd.clone())));
    }
    let owner = match m.sandbox.as_str() {
        "own" => json!({ "crew": m.id }),
        "job" => json!({ "crew": m.id, "job": "" }),
        _ => return Ok(None),
    };
    let cwd = (!m.cwd.is_empty()).then(|| m.cwd.clone());
    Ok(Some(calls::place(ctx, call, owner, cwd).await?))
}

/// The member's jobs as the runtime has them now.
async fn jobs_of(ctx: &Ctx, call: &Call, st: &State, m: &Member) -> Vec<JobSummary> {
    let ids = lock(&st.jobs).get(&m.id).cloned().unwrap_or_default();
    jobs_in(ctx, call, m, ids).await
}

/// How many of the member's jobs are live, and which jobs that read (for `State::taken`, which adds the ones started
/// since).
async fn busy_of(ctx: &Ctx, call: &Call, st: &State, m: &Member) -> (u32, Vec<String>) {
    let ids = lock(&st.jobs).get(&m.id).cloned().unwrap_or_default();
    let live = jobs_in(ctx, call, m, ids.clone()).await.iter().filter(|j| j.live()).count();
    (u32::try_from(live).unwrap_or(u32::MAX), ids)
}

async fn jobs_in(ctx: &Ctx, call: &Call, m: &Member, ids: Vec<String>) -> Vec<JobSummary> {
    let mut out = Vec::new();
    for id in ids {
        let Ok(s) = job_task(ctx, call, &id).await else { continue };
        let job = s.pointer("/ext/wisp.crew/job").and_then(|j| serde_json::from_value::<JobKind>(j.clone()).ok()).unwrap_or_default();
        let state = s.get("state").and_then(Value::as_str).unwrap_or("done").to_owned();
        out.push(JobSummary { session_id: id, crew_id: m.id.clone(), job, state });
    }
    out
}

pub(crate) async fn view(ctx: &Ctx, call: &Call, st: &State, m: &Member) -> MemberView {
    let live = jobs_of(ctx, call, st, m).await.iter().filter(|j| j.live()).count();
    st.view(m, u32::try_from(live).unwrap_or(u32::MAX), now(ctx))
}

/// A job's worktree never piles up (ADR-0108, audit C3: 25 → 65 worktrees in 40 min): every job's goes when its turn
/// ends with nothing changed (v1 done-folders `tidy` on a crew job ending), and a member's check-ins reuse one folder
/// per project, on `wisp/<member>-check-in`.
pub(crate) fn tree_reuse(args: &mut Value, m: &Member, job: JobKind) {
    args["worktreeTidy"] = json!(true);
    if job == JobKind::Checkin {
        args["worktreeKey"] = json!(format!("{}-check-in", if m.name.trim().is_empty() { m.id.as_str() } else { m.name.as_str() }));
    }
}

/// Starts one job as a task, in the place `slot` took under the member's limit (given back once the job is recorded,
/// or when it fails to start).
/// `w.run`: this job's own pick (`wisp_crew_core::run`), already checked. `capped`: a capped lead's cap
/// (`{ mode }`), which a job it delegated runs under (v1 `atCap`, and the job keeps it as a child would).
/// The member as this job sees them: in the job's own folder (a trigger's project, ADR-0148) when it has one.
fn in_its_folder<'a>(m: &'a Member, w: &WaitingJob) -> std::borrow::Cow<'a, Member> {
    match &w.cwd {
        Some(c) if *c != m.cwd => std::borrow::Cow::Owned(Member { cwd: c.clone(), ..m.clone() }),
        _ => std::borrow::Cow::Borrowed(m),
    }
}

async fn start(
    ctx: &Ctx,
    call: &Call,
    st: &State,
    m: &Member,
    w: &WaitingJob,
    capped: Option<&Value>,
    slot: Slot<'_>,
) -> Result<Value, WireError> {
    let m = &*in_its_folder(m, w);
    let (prompt, job, attachments) = (&w.prompt, w.job, &w.attachments);
    // what it was asked, for its page (a check-in's prompt is the crew's own)
    let asked = (job != JobKind::Checkin && !prompt.trim().is_empty()).then(|| prompt.chars().take(2000).collect::<String>());
    // on the member's machine, or in a sandbox (a fresh one per job, or the member's own): the job's task runs there
    let place = sandbox_for(ctx, call, m).await?;
    let mut ext = CrewExt { id: m.id.clone(), job, prompt: asked, from: w.from.clone(), reply: w.reply.clone(), ..CrewExt::default() };
    if place.is_some() {
        // no such member over there: the note goes with the job (`Note::part` reads it)
        ext.note = Some(wisp_crew_core::note::crew_note(m, &ext, None, &st.local()));
    }
    let (agent, model, effort, mode) = job_body(m, w.run.as_ref());
    let mode = at_cap(&mode, capped.map(|c| c.get("mode").and_then(Value::as_str).unwrap_or("edits")));
    let agent = (!agent.is_empty() && agent != "auto").then_some(agent);
    let mut args = json!({
        "prompt": prompt, "agent": agent, "model": (!model.is_empty()).then_some(model), "effort": (!effort.is_empty()).then_some(effort),
        "mode": mode, "cwd": (!m.cwd.is_empty()).then(|| m.cwd.clone()), "attachments": attachments, "by": "crew", "title": format!("{} · {}", m.name, if job == JobKind::Checkin { "Check-in" } else { "Job" }),
        "ext": { PLUGIN_ID: ext },
    });
    if let Some(l) = outside_lineage(w.origin.as_ref()) {
        args["lineage"] = l;
    }
    // their other folders (paths on their computer: a sandbox has its own copy of `cwd` only)
    if m.sandbox.is_empty() && !m.cwd.is_empty() && !m.dirs.is_empty() {
        args["dirs"] = json!(m.dirs);
    }
    if !m.fallbacks.is_empty() {
        args["fallbacks"] = json!(fallbacks_of(m));
    }
    // here, in its project: a worktree of its own, as every launch was in v1 (jobs and check-ins side by side edited
    // your checkout and each other: audit B #7); a sandbox's or another machine's folder is theirs
    if place.is_none() && !m.cwd.is_empty() {
        let set = ctx.setting(wisp_runtime_api::commands::WORKTREE_SETTING);
        args["worktree"] = json!(wisp_runtime_api::commands::worktree_by_default(set.as_ref(), mode == "plan"));
        tree_reuse(&mut args, m, job);
    }
    // the job's delegates (v1 crew.js): the member's own (Org chart links, "Make Bob Ada's delegate") and the
    // members tagged in what it was asked; the runtime makes them the lead's roster (orch #2)
    // a linked row names only the member (`{ crew }`): the lead hears who it is, as v1's roster said (its name, role,
    // harness), read from the crew now
    let team: Vec<Value> = m
        .team
        .iter()
        .chain(w.team.iter())
        .filter_map(|r| {
            let mut v = serde_json::to_value(r).ok()?;
            if let Some(who) = r.crew.as_deref().and_then(|id| st.member(id).ok()) {
                v["name"] = json!(who.name);
                v["label"] = json!(who.name);
                v["role"] = json!(who.role);
                v["agent"] = json!(who.agent);
            }
            Some(v)
        })
        .collect();
    if !team.is_empty() {
        args["team"] = Value::Array(team);
    }
    let mut rt_ext = json!({ "titleAuto": true });
    if let Some(cap) = capped {
        rt_ext["capped"] = cap.clone();
    }
    args["ext"][wisp_runtime_api::PLUGIN_ID] = rt_ext;
    let made = match &place {
        Some((machine, cwd)) => {
            // no folder: the machine's own default (a member's there may have none)
            args["cwd"] = if cwd.is_empty() { Value::Null } else { json!(cwd) };
            // a sandbox of the job's own goes when its start fails half way: never named as the job's, it would hold
            // one of the user's few sandboxes, `{crew, job: ""}`, until the sweep's half hour
            let made = match calls::routed::<rt::Create>(ctx, call, machine, args).await {
                Ok(made) => made,
                Err(e) => {
                    if m.sandbox == "job" {
                        calls::warn("dropping a job's sandbox", calls::drop_sandbox(ctx, call, machine).await);
                    }
                    return Err(e);
                }
            };
            let id = format!("{machine}|{}", made.get("id").and_then(Value::as_str).unwrap_or_default());
            if m.sandbox == "job"
                && let Err(e) = calls::own(ctx, call, machine, json!({ "crew": m.id, "job": id })).await
            {
                tracing::warn!(error = %e.message, machine, "crew: a job's sandbox couldn't be named the job's; dropped");
                calls::warn("dropping a job's sandbox", calls::drop_sandbox(ctx, call, machine).await);
                return Err(e);
            }
            let mut made = made;
            made["id"] = json!(id);
            made
        }
        None => calls::to_json(&ctx.run::<rt::Create>(call, calls::args_of::<rt::Create>(args)?).await?)?,
    };
    if let Some(id) = made.get("id").and_then(Value::as_str) {
        // the entry names its job before the job is kept: a sandbox's row seen in between finds whom it answers
        if let Some(r) = &w.reply {
            crate::answers::started(ctx, call, st, r, id).await;
        }
        st.add_job(slot, id, now(ctx)).await;
        if let Some((machine, task)) = id.split_once('|') {
            crate::remote::check(ctx, st, machine, task).await;
        }
    }
    let id = made.get("id").cloned().unwrap_or_default();
    let mut session = job_task(ctx, call, id.as_str().unwrap_or_default()).await.unwrap_or(made);
    // a sandbox's job is named as it's kept here (`<machine>|<id>`), not by its id over there
    session["id"] = id;
    Ok(session)
}

/// A job to start or queue.
/// A wake's origin (`{ kind: "trigger", by }`), from what woke it (`trigger:<name>`, an event like `github.comment`).
fn woken_by(source: Option<&String>) -> Option<Value> {
    source.map(|s| json!({ "kind": "trigger", "by": s }))
}

/// The lineage a job woken from outside (a trigger, a pushed comment, a chat mention) runs with: `triggeredBy: trigger:…`, which keeps its Plan read-only.
fn outside_lineage(origin: Option<&Value>) -> Option<Value> {
    let by = origin.filter(|o| o.get("kind").and_then(Value::as_str) == Some("trigger"))?.get("by").and_then(Value::as_str)?;
    let by = if by.starts_with("trigger:") { by.to_owned() } else { format!("trigger:{by}") };
    Some(json!({ "triggeredBy": by }))
}

fn work(prompt: String, job: JobKind, attachments: Vec<Value>, run: Option<RunPick>) -> WaitingJob {
    WaitingJob { prompt, job, attachments, run, ..WaitingJob::default() }
}

/// Records a link to the task that tagged, delegated to or watched the member, so it may follow up there later
/// (v1 crew.js:630 `#link`).
async fn record_link(ctx: &Ctx, st: &State, id: &str, w: &WaitingJob) {
    let Some(from) = w.from.as_ref().filter(|f| !f.id.is_empty()) else { return };
    if matches!(w.job, JobKind::Mention | JobKind::Delegated | JobKind::Event) || w.reply.is_some() {
        let at = now(ctx);
        st.update(id, |m| wisp_crew_core::queue::link(m, from, at));
        st.save().await;
    }
}

pub(crate) fn gone() -> WireError {
    WireError::not_found("No such crew member.")
}

/// A job for `m`: now when a slot is free, else in its queue (v1 `assign`).
/// Placed against the member as they are after the awaits, not the copy `m`: two jobs given at once both wait (each
/// wrote back its own copy's queue, and one was lost), and a member removed meanwhile isn't brought back. The place a
/// job that starts takes is reserved as it's decided: two given at once to an idle member both read it idle, and both
/// started past its limit. A job that fails to start gives its place back and waits at the head of the queue.
pub(crate) async fn assign(ctx: &Ctx, call: &Call, st: &State, m: &Member, w: WaitingJob) -> Result<c::Started, WireError> {
    record_link(ctx, st, &m.id, &w).await;
    let (busy, seen) = busy_of(ctx, call, st, m).await;
    let (id, at) = (wisp_kernel::ids::uuid7(), now(ctx));
    let placed = st
        .update(&m.id, |live| -> wisp_crew_core::Result<(Option<Slot<'_>>, u32)> {
            let p = place(live, st.taken(&live.id, busy, &seen), w.job)?;
            if p == Placement::Queue {
                live.queue.push(WaitingJob { id: id.clone(), at, ..w.clone() });
            }
            Ok(((p == Placement::Start).then(|| st.reserve(&live.id)), u32::try_from(live.queue.len()).unwrap_or(u32::MAX)))
        })
        .ok_or_else(gone)?
        .map_err(wire)?;
    match placed {
        (Some(slot), _) => {
            let m = st.member(&m.id)?;
            match start(ctx, call, st, &m, &w, None, slot).await {
                Ok(session) => Ok(c::Started { session: Some(session), ..c::Started::default() }),
                // its place is given back (the slot dropped) and the job waits first in line, as a drain's does on a
                // full grid: it's never lost, and the next drain tries it again
                Err(e) => {
                    tracing::warn!(member = %m.name, error = %e.message, "crew job didn't start; it waits first in line");
                    let back = WaitingJob { id: id.clone(), at, ..w };
                    st.update(&m.id, |live| live.queue.insert(0, back)).ok_or_else(gone)?;
                    st.save().await;
                    Ok(c::Started { queued: Some(id), position: Some(1), session: None })
                }
            }
        }
        (None, position) => {
            st.save().await;
            Ok(c::Started { queued: Some(id), position: Some(position), session: None })
        }
    }
}

/// A job a lead task's agent handed out (`crew.assign` with `from`, v1 crew.js `#delegate`): it starts now, as a
/// `delegated` job from that task, or is refused (never queued: the lead waits on its report).
async fn delegated(ctx: &Ctx, call: &Call, st: &State, m: &Member, w: WaitingJob, capped: Option<&Value>) -> Result<c::Started, WireError> {
    if capped.is_some() && m.elsewhere() {
        return Err(WireError::conflict(wisp_kernel::caps::capped_refusal("send work to another machine")));
    }
    let (busy, seen) = busy_of(ctx, call, st, m).await;
    // decided and reserved at once, against the jobs starting meanwhile (two handed out at once both started)
    let slot = st
        .update(&m.id, |live| place_delegated(live, st.taken(&live.id, busy, &seen)).map(|()| st.reserve(&live.id)))
        .ok_or_else(gone)?
        .map_err(wire)?;
    record_link(ctx, st, &m.id, &w).await;
    let m = st.member(&m.id)?;
    Ok(c::Started { session: Some(start(ctx, call, st, &m, &w, capped, slot).await?), ..c::Started::default() })
}

/// `crew.checkin`: one check-in at a time per member (v1 `checkin`).
pub(crate) async fn checkin(ctx: &Ctx, call: &Call, st: &State, id: &str, wake: bool) -> Result<c::Started, WireError> {
    let m = st.member(id)?;
    // paused members skip check-ins (v1): Check in now said nothing, queued one, then "already checking in" (audit E7)
    if m.paused {
        return Err(WireError::conflict(format!("{} is paused. Resume them to check in.", m.name)));
    }
    let live = jobs_of(ctx, call, st, &m).await.iter().any(|j| j.job == JobKind::Checkin && j.live());
    if checkin_already(&m, live) {
        return Err(WireError::conflict(format!("{} is already checking in.", m.name)));
    }
    let prompt = checkin_prompt(&m, wake);
    let at = now(ctx);
    st.update(&m.id, |live| count_checkin(live, at)).ok_or_else(gone)?;
    st.save().await;
    let m = st.member(&m.id)?;
    assign(ctx, call, st, &m, work(prompt, JobKind::Checkin, Vec::new(), None)).await
}

macro_rules! on {
    ($s:ident, $st:ident, $C:ty, |$state:ident, $ctx:ident, $call:ident, $a:ident| $body:expr) => {{
        let st = $st.clone();
        $s.command::<$C, _, _>(move |$ctx, $call, $a| {
            let $state = st.clone();
            async move { $body }
        });
    }};
}

/// `crew.wake` and `crew.wakeMentions` (ADR-0043).
fn adopt(s: &mut Setup, st: &Arc<State>) {
    // Add to crew… on a task (v1 POST /sessions/:id/crew): a new member, and the task becomes its first job
    on!(s, st, c::Adopt, |st, ctx, call, a| {
        let task = calls::task(&ctx, &call, &a.session_id).await.map_err(|_| WireError::not_found("That task is gone."))?;
        if calls::crew_of(&task).get("id").and_then(Value::as_str).is_some() {
            return Err(WireError::invalid("That task is already a crew member's job."));
        }
        let m = st.normalize(
            &a.member,
            None,
            now(&ctx),
            &profile_names(&ctx, &call).await,
            &crate::calls::machine_ids(&ctx, &call).await,
            &crate::calls::default_agent(&ctx),
        )?;
        // claimed before the member is kept: of two adds at once, only one makes a member (the other is refused)
        let ext = CrewExt { id: m.id.clone(), job: JobKind::Task, ..CrewExt::default() };
        if !calls::claim(&ctx, &call, &a.session_id, &calls::to_json(&ext)?).await? {
            return Err(WireError::invalid("That task is already a crew member's job."));
        }
        st.insert(m.clone());
        st.save().await;
        ctx.emit(&call, &wisp_crew_api::events::Joined { crew_id: m.id.clone(), name: m.name.clone(), how: "task".into() });
        Ok(view(&ctx, &call, &st, &m).await)
    });
}

fn wakes(s: &mut Setup, st: &Arc<State>) {
    // a wake (ADR-0043): what happened is the job; where it came from is said in it
    on!(s, st, c::Wake, |st, ctx, call, a| {
        let m = st.member(&a.id)?;
        let why = if a.why.trim().is_empty() { "You were woken: see what needs doing.".to_owned() } else { a.why.trim().to_owned() };
        let prompt = match &a.source {
            Some(src) => format!("{why}\n\n(woken by {src})"),
            None => why,
        };
        // a trigger's project: the job works there, not in the member's own folder (ADR-0148)
        let cwd = a.cwd.as_deref().map(str::trim).filter(|c| !c.is_empty()).map(str::to_owned);
        let w = WaitingJob { origin: woken_by(a.source.as_ref()), cwd, ..work(prompt, JobKind::Task, Vec::new(), None) };
        assign(&ctx, &call, &st, &m, w).await
    });
    on!(s, st, c::WakeMentions, |st, ctx, call, a| Ok(wake_mentions(&ctx, &call, &st, &a).await));
}

/// `crew.wakeMentions`: every member the text tags gets a job. One who can't take it (their queue full, a start that
/// failed) is said in the log and the rest still wake: `@ada @bob` with Ada's queue full woke no one.
async fn wake_mentions(ctx: &Ctx, call: &Call, st: &State, a: &c::MentionArgs) -> Vec<c::Started> {
    let crew = lock(&st.saved).crew.clone();
    let found = wisp_crew_core::mentions::crew_mentions(&a.text, &crew);
    let mut out = Vec::new();
    for m in found.all {
        let prompt = match &a.source {
            Some(src) => format!("You were mentioned ({src}):\n\n{}", a.text.trim()),
            None => format!("You were mentioned:\n\n{}", a.text.trim()),
        };
        let w = WaitingJob { origin: woken_by(a.source.as_ref()), ..work(prompt, JobKind::Task, Vec::new(), None) };
        match assign(ctx, call, st, m, w).await {
            Ok(s) => out.push(s),
            Err(e) => tracing::warn!(member = %m.name, error = %e.message, "crew: a mention didn't wake them"),
        }
    }
    out
}

pub(crate) fn register(s: &mut Setup, st: &Arc<State>) {
    adopt(s, st);
    on!(s, st, c::List, |st, ctx, call, _a| {
        let crew = lock(&st.saved).crew.clone();
        let mut out = Vec::new();
        for m in &crew {
            out.push(view(&ctx, &call, &st, m).await);
        }
        Ok(out)
    });
    on!(s, st, c::Get, |st, ctx, call, a| {
        let m = st.member(&a.id)?;
        let mut job_list = jobs_of(&ctx, &call, &st, &m).await;
        job_list.reverse();
        job_list.truncate(50);
        Ok(c::MemberDetail { member: view(&ctx, &call, &st, &m).await, job_list })
    });
    on!(s, st, c::GetLayout, |st, _ctx, _call, _a| {
        let saved = lock(&st.saved).clone();
        Ok(layout_of(&saved.crew_layout, &saved.crew))
    });
    on!(s, st, c::SetLayout, |st, _ctx, _call, a| {
        let crew = lock(&st.saved).crew.clone();
        let out = normalize_layout(&a.layout, &crew).map_err(wire)?;
        st.set_layout(out.clone());
        st.save().await;
        Ok(out)
    });
    on!(s, st, c::Create, |st, ctx, call, a| create(&ctx, &call, &st, a).await);
    on!(s, st, c::Update, |st, ctx, call, a| update(&ctx, &call, &st, &a).await);
    on!(s, st, c::Remove, |st, ctx, call, a| {
        let m = st.member(&a.id)?;
        st.remove(&m.id);
        st.save().await;
        ctx.emit(&call, &wisp_crew_api::events::Left { crew_id: m.id });
        Ok(c::Ok { ok: true })
    });
    on!(s, st, c::Detach, |st, ctx, call, a| {
        let mut m = st.member(&a.id)?;
        if m.spell.is_none() {
            return Err(WireError::invalid(format!("{} is already yours.", m.name)));
        }
        m.spell = None;
        m.changed = None;
        st.put(m.clone());
        st.save().await;
        Ok(view(&ctx, &call, &st, &m).await)
    });
    work_commands(s, st);
}

/// `crew.create`: a new member, and their first check-in if asked.
async fn create(ctx: &Ctx, call: &Call, st: &State, a: c::CreateArgs) -> Result<c::Created, WireError> {
    if let Some(w) = &a.member.wakes {
        unknown_sources(ctx, call, w, &[]).await?;
    }
    let mut m = st.normalize(
        &a.member,
        None,
        now(ctx),
        &profile_names(ctx, call).await,
        &crate::calls::machine_ids(ctx, call).await,
        &crate::calls::default_agent(ctx),
    )?;
    // the id the client drew it under (B27: the rail showed the member twice for a frame)
    if let Some(id) = a.id.filter(|id| wisp_crew_core::kit::is_id(id) && st.member(id).is_err()) {
        m.id = id;
    }
    st.insert(m.clone());
    st.save().await;
    ctx.emit(call, &wisp_crew_api::events::Joined { crew_id: m.id.clone(), name: m.name.clone(), how: "create".into() });
    let session = if a.checkin == Some(true) { checkin(ctx, call, st, &m.id, false).await.ok().and_then(|s| s.session) } else { None };
    Ok(c::Created { member: view(ctx, call, st, &m).await, session })
}

/// `crew.update`: the edit is applied to the member as they are after the awaits (the harnesses, the machines), not to
/// a copy read before them: a job that started from the queue meanwhile isn't put back, nor one queued lost.
async fn update(ctx: &Ctx, call: &Call, st: &State, a: &c::UpdateArgs) -> Result<MemberView, WireError> {
    let before = st.member(&a.id)?;
    if let Some(w) = &a.member.wakes {
        unknown_sources(ctx, call, w, &before.wakes).await?;
    }
    let (profiles, machines) = (profile_names(ctx, call).await, crate::calls::machine_ids(ctx, call).await);
    // read again, and written with no await between
    let mut prev = st.member(&before.id)?;
    if let Some(keep) = &a.queue {
        prev.queue.retain(|w| keep.contains(&w.id)); // drop waiting jobs
    }
    let mut m = st.normalize(&a.member, Some(&prev), now(ctx), &profiles, &machines, &crate::calls::default_agent(ctx))?;
    crate::offers::settle_all(&mut m, a.offers.as_ref(), now(ctx));
    st.update(&m.id, |live| *live = m.clone()).ok_or_else(gone)?;
    st.save().await;
    ctx.emit(call, &wisp_crew_api::events::MemberChanged { crew_id: m.id.clone() });
    Ok(view(ctx, call, st, &m).await)
}

/// A wake whose `on` no trigger source offers never fires: refused in the words the triggers kind uses
/// (`wakes[i].on: …`), so the app, the CLI and `wisp apply` agree. Only a wake added or changed (`had`: the member's
/// own): one whose source has gone since (its plugin removed) doesn't lock every other edit. With automations off,
/// there is nothing to check against.
pub(crate) async fn unknown_sources(ctx: &Ctx, call: &Call, wakes: &[TriggerSpec], had: &[TriggerSpec]) -> Result<(), WireError> {
    if wakes.iter().all(|w| had.contains(w)) {
        return Ok(());
    }
    let sources = match ctx.run::<wisp_automations_api::commands::Sources>(call, wisp_automations_api::commands::Empty {}).await {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(error = %e.message, "crew: no trigger sources to check wakes against");
            return Ok(());
        }
    };
    for (i, w) in wakes.iter().enumerate().filter(|(_, w)| !had.contains(w)) {
        if let Some(p) = wisp_automations_api::spec::no_source(&w.on, &sources) {
            return Err(WireError::invalid(format!("wakes[{i}].{p}")));
        }
    }
    Ok(())
}

/// Giving a member work: assign, check in, wake, and the per-member stats.
fn work_commands(s: &mut Setup, st: &Arc<State>) {
    on!(s, st, c::Assign, |st, ctx, call, a| {
        let m = st.member(&a.id)?;
        let prompt = if a.prompt.trim().is_empty() && a.kit {
            format!(
                "Build your kit: make yourself excellent{}.",
                if m.role.is_empty() { " at your job".to_owned() } else { format!(" as our {}", m.role) }
            )
        } else {
            a.prompt.trim().to_owned()
        };
        if prompt.is_empty() {
            return Err(WireError::invalid("Tell them what to do."));
        }
        let harnesses = if a.run.as_ref().is_some_and(|r| r.agent.is_some()) { profile_names(&ctx, &call).await } else { Vec::new() };
        let run = job_run(&m, a.run.as_ref(), a.picked, &|h| harnesses.iter().any(|p| p == h)).map_err(wire)?;
        match a.from {
            Some(from) => {
                delegated(
                    &ctx,
                    &call,
                    &st,
                    &m,
                    WaitingJob { from: Some(from), ..work(prompt, JobKind::Delegated, a.attachments, run) },
                    a.capped.as_ref(),
                )
                .await
            }
            None => assign(&ctx, &call, &st, &m, work(prompt, JobKind::Task, a.attachments, run)).await,
        }
    });
    on!(s, st, c::Checkin, |st, ctx, call, a| checkin(&ctx, &call, &st, &a.id, false).await);
    wakes(s, st);
    crate::stats::register(s, st);
}

/// Every 20 s (v1 `tick`): the check-ins that are due, then each queue with room.
pub(crate) fn start_ticks(ctx: &Ctx, st: Arc<State>) {
    // the zone the client sent with its settings, and each change of it
    let zone_of = |v: Option<Value>| v.and_then(|v| v.as_str().map(str::to_owned)).filter(|z| !z.is_empty());
    *lock(&st.zone) = zone_of(ctx.setting("settings.timeZone"));
    let st2 = st.clone();
    ctx.watch_setting("settings.timeZone", move |v| *lock(&st2.zone) = zone_of(v));
    let scope = ctx.scope().clone();
    let c2 = ctx.clone();
    let Ok(clock) = ctx.clock() else { return };
    ctx.spawn(async move {
        loop {
            run_tick(&c2, &st).await;
            // on the kernel's clock, so a test's FakeClock drives the scheduler
            tokio::select! {
                () = scope.cancelled() => return,
                () = clock.sleep(TICK_MS) => {}
            }
        }
    });
}

async fn run_tick(ctx: &Ctx, st: &State) {
    let call = ctx.root_call();
    let ids: Vec<String> = lock(&st.saved).crew.iter().map(|m| m.id.clone()).collect();
    let t = now(ctx);
    let zone = st.local();
    for id in ids {
        // each member as they are now (the one before awaited), the minute marked on their live row
        let Ok(m) = st.member(&id) else { continue };
        if due_now(&m, t, &zone)
            && let Some(fire) = st.update(&id, |live| tick(live, t, &zone)).flatten()
        {
            match checkin(ctx, &call, st, &id, fire.wake).await {
                // a wake-up is used up only once its check-in started or queued
                Ok(_) if fire.wake => {
                    st.update(&id, |live| clear_wake(live, t));
                }
                Ok(_) => {}
                // a wake-up stays: the next minute's tick asks again (already checking in, a full queue)
                Err(e) => tracing::warn!(member = %m.name, error = %e.message, "crew check-in"),
            }
        }
        drain(ctx, &call, st, &id).await;
    }
    st.save().await;
}

/// Starts waiting jobs for `id` that now have room and saves any queue change (called when a job finishes).
pub(crate) async fn drain_and_save(ctx: &Ctx, call: &Call, st: &State, id: &str) {
    drain(ctx, call, st, id).await;
    st.save().await;
}

/// The member's waiting jobs that may start now (room, and not paused) start (v1 `drain`); the caller saves.
async fn drain(ctx: &Ctx, call: &Call, st: &State, id: &str) {
    let Ok(m) = st.member(id) else { return };
    let (busy, seen) = busy_of(ctx, call, st, &m).await;
    if drain_count(&m, busy) == 0 {
        return;
    }
    // the head of the queue as it is now: a job queued or dropped while the jobs were read is neither lost nor run;
    // each one taken reserves its place, as a job given meanwhile does
    let Some(due) = st.update(id, |live| {
        let n = drain_count(live, st.taken(&live.id, busy, &seen));
        live.queue.drain(..n).map(|w| (w, st.reserve(id))).collect::<Vec<(WaitingJob, Slot<'_>)>>()
    }) else {
        return;
    };
    for (w, slot) in due {
        let Ok(m) = st.member(id) else { return };
        if let Err(e) = start(ctx, call, st, &m, &w, None, slot).await {
            tracing::warn!(member = %m.name, error = %e.message, "crew job");
            // a full grid (its sandboxes): the job waits its turn again instead of being lost (ADR-0047)
            if wisp_sandbox_api::full(&e) {
                st.update(id, |again| again.queue.insert(0, w));
            }
        }
    }
}

/// The agent profiles a member may name (wisp.runtime's `runtime.profiles`).
async fn profile_names(ctx: &wisp_kernel::Ctx, call: &wisp_kernel::Call) -> Vec<String> {
    calls::profile_names(ctx, call).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::assert_is_empty)]
#[path = "jobs_tests.rs"]
mod races;

#[cfg(test)]
mod tests {
    use super::*;

    /// Audit C3 (ADR-0108): a member's check-ins reuse one worktree (`worktreeKey`, branch `wisp/<member>-check-in`)
    /// and every job's worktree goes when it changed nothing (`worktreeTidy`); a job you gave it has a folder of its own.
    #[test]
    fn a_check_in_reuses_the_members_worktree_and_every_job_tidies_its_own() {
        let m = Member { id: "c1".into(), name: "Ada".into(), ..Member::default() };
        let mut checkin = json!({});
        tree_reuse(&mut checkin, &m, JobKind::Checkin);
        assert_eq!(checkin, json!({ "worktreeTidy": true, "worktreeKey": "Ada-check-in" }));
        let mut job = json!({});
        tree_reuse(&mut job, &m, JobKind::Task);
        assert_eq!(job, json!({ "worktreeTidy": true }));
        let a: wisp_runtime_api::commands::StartArgs =
            serde_json::from_value(json!({ "prompt": "x", "worktreeKey": "Ada-check-in", "worktreeTidy": true })).unwrap();
        assert_eq!((a.worktree_key.as_deref(), a.worktree_tidy), (Some("Ada-check-in"), true), "runtime.create reads them");
    }

    /// v1 #794: a crew job a trigger or a pushed mention woke carries the waker as its lineage, so its Plan stays
    /// read-only; one you gave it (no source) has none.
    #[test]
    fn a_job_woken_from_outside_runs_with_a_trigger_lineage_v1_794() {
        let by = |s: &str| outside_lineage(woken_by(Some(&s.to_owned())).as_ref());
        assert_eq!(by("trigger:on-issue"), Some(json!({ "triggeredBy": "trigger:on-issue" })));
        assert_eq!(by("github.comment"), Some(json!({ "triggeredBy": "trigger:github.comment" })));
        assert_eq!(outside_lineage(woken_by(None).as_ref()), None);
        assert_eq!(outside_lineage(Some(&json!({ "kind": "task" }))), None);
    }
}
