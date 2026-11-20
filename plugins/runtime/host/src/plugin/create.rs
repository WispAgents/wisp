//! `runtime.create` (a worktree of its own and the project's setup, a terminal, the queued steps) and
//! `runtime.followUp` (a terminal task's follow-up goes to its terminal).

use std::sync::Arc;

use wisp_kernel::{Call, Ctx, Result as WireResult, Setup, WireError};
use wisp_proc::Proc;
use wisp_runtime_api::commands::{self as c, StartArgs};
use wisp_runtime_api::hooks::{BeforeCreate, BeforeFollowUp};
use wisp_store::Store;

use super::{Slot, rt};
use crate::kinds::Kinds;
use crate::runtime::Runtime;

/// Registers `runtime.create` and `runtime.followUp`.
pub(super) fn create_and_follow_up(s: &mut Setup, slot: &Slot, made_kinds: Arc<Kinds>) {
    let ctx = s.ctx();
    let (c1, c2) = (ctx.clone(), ctx);
    let start = slot.clone();
    s.command::<c::Create, _, _>(move |_ctx, call, a| {
        let (slot, ctx, kinds) = (start.clone(), c1.clone(), made_kinds.clone());
        // boxed: a create runs its first turn, whose future is large (clippy large_futures)
        async move { Box::pin(create(&ctx, &call, &slot, &kinds, a)).await }
    });
    let follow = slot.clone();
    s.command::<c::FollowUp, _, _>(move |_ctx, call, a| {
        let (slot, ctx) = (follow.clone(), c2.clone());
        async move {
            // an agent's own follow-up never lifts its task's mode or skips Ask near the limit (review r194 round 2 #2)
            if let Some(p) = agent_parent(&slot, &call).await? {
                wisp_runtime_core::child::follow_up(a.mode, a.confirm_limit, &p).map_err(WireError::forbidden)?;
            }
            let a = ctx.run_hook::<BeforeFollowUp>(&call, a).await?;
            let rt = rt(&slot)?;
            // a follow-up asking a capped task for more than its cap: refused, or lifted when it's the user
            if let Some(m) = a.mode {
                rt.mode_asked(a.id.as_str(), m, &call.caller().set).await.map_err(WireError::from)?;
            }
            if rt.repo.session(a.id.as_str()).await.is_ok_and(|s| crate::term::term_of(&s).is_some()) {
                return crate::term::follow_up(&ctx, &call, &rt, a.id.as_str(), &a.text).await;
            }
            rt.follow_up(a).await.map_err(WireError::from)
        }
    });
}

/// The asking agent's own task, when an agent's token starts this one (review r194 round 2 #2): what its child is held
/// to. None: the user, a plugin, a channel (each has its own rules).
async fn agent_parent(slot: &Slot, call: &Call) -> WireResult<Option<wisp_runtime_core::child::Parent>> {
    let c = call.caller();
    let Some(id) = c.session.as_deref().filter(|_| c.set.starts_with("agent:")) else { return Ok(None) };
    let s = rt(slot)?.repo.session(id).await.map_err(WireError::from)?;
    let spec = s.ext.get("wisp.runtime").and_then(|r| r.get("spec")).cloned().unwrap_or_default();
    let text = |k: &str| spec.get(k).and_then(serde_json::Value::as_str).unwrap_or_default().to_owned();
    Ok(Some(wisp_runtime_core::child::Parent {
        id: s.id.0.clone(),
        mode: s.mode,
        cap: crate::capped::cap_of(&s),
        crew: text("crew"),
        account: s.account.clone().unwrap_or_default(),
        connectors: spec
            .get("connectors")
            .and_then(serde_json::Value::as_array)
            .map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
        team: text("team"),
    }))
}

/// The limits a task an agent's token made carries (`ext["wisp.runtime"].heldTo`): what a task made under it is held to,
/// whoever makes it (a plugin's own call, as a factory's driver makes its stations: review r194 factories #1).
async fn inherited_parent(slot: &Slot, a: &StartArgs) -> WireResult<Option<wisp_runtime_core::child::Parent>> {
    let Some(pid) = a.parent_id.as_ref() else { return Ok(None) };
    let Ok(s) = rt(slot)?.repo.session(pid.as_str()).await else { return Ok(None) };
    Ok(s.ext.get("wisp.runtime").and_then(|r| r.get(HELD_TO)).and_then(|v| serde_json::from_value(v.clone()).ok()))
}

/// Whether `a` is work run for an agent's task (review r194 final pass #1): a plugin or the kernel making a step of the
/// parent's own run (`lineage.workflowRun`: a factory's station, a workflow's step). Never the user's own call (their
/// page, CLI, harness through `wisp mcp`, a linked channel, an API key: a Side discussion, a rerun on an agent's task
/// is theirs) and never an agent's (its own token is held to its own task already).
pub(crate) fn for_the_agents_run(set: &str, a: &StartArgs) -> bool {
    let plugin_or_kernel = set == "kernel" || set.starts_with("plugin:");
    plugin_or_kernel && a.parent_id.is_some() && a.lineage.workflow_run.as_ref() == a.parent_id.as_ref().map(|p| &p.0)
}

/// Where a task an agent's token made keeps that agent's limits.
const HELD_TO: &str = "heldTo";

/// The limits a task gets before anything is made (review r194 round 2 #2, factories #1): an agent's token is held to
/// its own task; a task a plugin makes under a task an agent's token made is held to that agent too (a factory run's
/// stations); either way the task keeps the limits it was held to (`heldTo`), for what is made under it in turn.
async fn held(slot: &Slot, call: &Call, a: &mut StartArgs) -> WireResult<Option<wisp_runtime_core::child::Parent>> {
    let parent = agent_parent(slot, call).await?;
    if let Some(p) = &parent {
        wisp_runtime_core::child::hold(a, p).map_err(WireError::forbidden)?;
        if let Some(f) = &a.formula {
            let why = wisp_resources_api::agent_overlay_problems(f);
            if !why.is_empty() {
                return Err(WireError::forbidden(why.join("; ")));
            }
        }
    }
    let inherited = if parent.is_none() && for_the_agents_run(&call.caller().set, a) { inherited_parent(slot, a).await? } else { None };
    if let Some(p) = &inherited {
        wisp_runtime_core::child::inherit(a, p).map_err(WireError::forbidden)?;
    }
    let parent = parent.or(inherited);
    if let Some(p) = &parent {
        let own = a.ext.entry("wisp.runtime".to_owned()).or_insert_with(|| serde_json::json!({}));
        if !own.is_object() {
            *own = serde_json::json!({});
        }
        if let (Some(o), Ok(v)) = (own.as_object_mut(), serde_json::to_value(p)) {
            o.insert(HELD_TO.into(), v);
        }
    }
    Ok(parent)
}

async fn create(ctx: &Ctx, call: &Call, slot: &Slot, kinds: &Kinds, mut a: StartArgs) -> WireResult<c::Created> {
    // a task an agent starts (or a plugin makes under one) gets nothing that agent's task may not have
    let parent = held(slot, call, &mut a).await?;
    // a project by name (a declared task's `project: web`) is its folder before anything reads the folder
    crate::specs::project_by_name(&mut a, Some(kinds)).await?;
    let mut a = ctx.run_hook::<BeforeCreate>(call, a).await?;
    if let Some(rest) = crate::term::slash(&a.prompt) {
        a.prompt = rest;
        a.terminal = true;
    }
    // the client's id for it (the row and page it shows): checked before anything is made; one that exists is this
    // create sent again (after a restart), answered with the task it made, nothing made twice
    if let Some(id) = a.id.clone()
        && rt(slot)?.claimed(&id).await.map_err(WireError::from)?
    {
        return Ok(c::Created { id, turn_id: None });
    }
    // a task you start under another is a side discussion (`/side`, a row's Side discussion…): v1's `side_task`
    let side_of = a.parent_id.clone().filter(|_| call.caller().set.starts_with("user"));
    let main = match &side_of {
        Some(main) => rt(slot)?.repo.session(main.as_str()).await.ok(),
        None => None,
    };
    let main_title = side_of.as_ref().map(|_| main.as_ref().map(|m| m.title.clone()).unwrap_or_default());
    mark_side(&mut a, side_of.as_ref().zip(main_title.as_deref()));
    // a side discussion works in the main task's folder, on its harness unless one was picked (v1 side_task); never
    // in a worktree of its own: it talks the main task's work over (orch #5)
    if let Some(m) = &main {
        if a.cwd.as_deref().is_none_or(str::is_empty) {
            a.cwd = Some(m.cwd.clone());
            a.dirs.clone_from(&m.dirs);
        }
        if a.agent.as_deref().is_none_or(str::is_empty) {
            a.agent = Some(m.agent.clone());
            a.model.get_or_insert_with(|| m.model.clone());
        }
        a.worktree = None;
    }
    let (store, proc) = (ctx.service::<Store>()?, ctx.service::<Proc>()?);
    let home = wisp_kernel::services::paths(ctx).ok().map(|p| p.home().to_owned());
    // what the caller said about a worktree, before the project's default fills it in
    let asked_tree = a.worktree;
    // a worktree of its own: made before the first turn, the project's setup run in it
    let p = crate::specs::prepare(&store, &proc, home.as_deref(), Some(kinds), a).await?;
    let rt = rt(slot)?;
    let mut args = p.args;
    // the mode its profile or the user's default gave it, held to the parent's
    if let Some(parent) = &parent {
        args.mode = Some(wisp_runtime_core::child::held_mode(args.mode, parent));
    }
    if own_tree(&rt, &args, asked_tree).await {
        args.worktree = Some(true);
    }
    let on = ctx.setting("terminal.terminalSessions").is_some_and(|v| v == true || v == "on");
    args.terminal = crate::term::starts_in_terminal(&args, on)
        && rt
            .harness(args.agent.as_deref().unwrap_or("claude"))
            .is_ok_and(|h| h.resume_command(&wisp_runtime_api::harness::ResumeOptions::default()).is_some());
    let terminal = args.terminal;
    let wants_tree = args.worktree == Some(true) && !args.hold && args.cwd.as_deref().is_some_and(|c| !c.is_empty());
    let setup = if wants_tree { crate::specs::project_setup(Some(kinds), &args).await } else { Vec::new() };
    args.hold |= wants_tree;
    let (cwd, title, mode) = (args.cwd.clone().unwrap_or_default(), args.title.clone().unwrap_or_default(), args.mode);
    let (reuse, tidy) = (args.worktree_key.clone(), args.worktree_tidy);
    let made = rt.create(args).await.map_err(WireError::from)?;
    let made = if wants_tree {
        // the mode the run has (none asked is Full auto, a cap applied), not the argument: `wisp "…" --mode auto` sends
        // none, so the worktree was made for mode "" and its setup waited for trust that the auto run never asks (audit E D5)
        let mode = rt.get(made.id.as_str()).await.map_or_else(|_| mode.unwrap_or_default(), |s| s.mode);
        let mode = serde_json::to_value(mode).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        // a rerun at an older commit: its worktree's branch starts at that commit (the detached folder made for it)
        let from = p.detached.as_ref().map(|(_, path)| path.clone());
        let wa = wisp_worktrees_api::CreateArgs { session_id: made.id.0.clone(), cwd, title, mode, from, reuse, tidy };
        let made = in_worktree(ctx, &rt, &proc, made, wa, &setup).await?;
        // it runs in its own worktree now: the detached folder goes (audit C8); one it still runs in stays
        if let Some((project, path)) = &p.detached
            && rt.get(made.id.as_str()).await.is_ok_and(|s| !std::path::Path::new(&s.cwd).starts_with(path))
        {
            crate::specs::drop_detached(&proc, project, path).await;
        }
        made
    } else {
        made
    };
    if terminal {
        crate::term::open(ctx, call, &rt, made.id.as_str(), None, (None, None)).await?;
    }
    // the steps after the first go in the queue, in order: each starts when the turn before ends
    for text in p.steps {
        let f = c::FollowUpArgs { id: made.id.clone(), text, queue: Some(true), by: Some("step".into()), ..c::FollowUpArgs::default() };
        rt.follow_up(f).await.map_err(WireError::from)?;
    }
    if let Some(main) = side_of {
        let running = !matches!(rt.run_state(main.as_str()), wisp_runtime_core::queue::Run::Idle);
        let agent = rt.get(made.id.as_str()).await.map(|x| x.agent).unwrap_or_default();
        super::track(ctx, "side_task", serde_json::json!({ "agent": agent, "running": running })).await;
    }
    Ok(made)
}

/// A task that must run in a worktree of its own whatever Worktree per task says (audit A, 2026-11-18): one of
/// Compare's runs (a group: side by side in one folder they edit each other's files, orch #7), and a rerun of a task
/// that ran in its own worktree when the rerun didn't say (it ran in the main checkout, tasks #3).
async fn own_tree(rt: &crate::runtime::Runtime, a: &StartArgs, asked: Option<bool>) -> bool {
    if a.ext.get("wisp.runtime").and_then(|o| o.get("group")).and_then(serde_json::Value::as_str).is_some_and(|g| !g.is_empty()) {
        return true;
    }
    match (asked, a.lineage.rerun_of.as_deref()) {
        (None, Some(of)) => rt
            .repo
            .session(of)
            .await
            .is_ok_and(|s| s.ext.get("wisp.worktrees").and_then(|w| w.get("path")).is_some_and(serde_json::Value::is_string)),
        _ => false,
    }
}

/// v1 `sideOf: { id, title }` in runtime's slot: only the user's create makes a side discussion, so one a caller wrote
/// itself is dropped (a forged one would run its action buttons in another task's folder).
pub(crate) fn mark_side(a: &mut StartArgs, main: Option<(&wisp_runtime_api::SessionId, &str)>) {
    let own = a.ext.entry("wisp.runtime".to_owned()).or_insert_with(|| serde_json::json!({}));
    if let Some(o) = own.as_object_mut() {
        o.remove("sideOf");
        if let Some((id, title)) = main {
            o.insert("sideOf".into(), serde_json::json!({ "id": id.as_str(), "title": title }));
        }
    }
    if a.ext.get("wisp.runtime").is_some_and(|o| o.as_object().is_some_and(serde_json::Map::is_empty)) {
        a.ext.remove("wisp.runtime");
    }
}

/// The held task's worktree, made and set up; then its first turn starts (in its folder if that failed).
async fn in_worktree(
    ctx: &Ctx,
    rt: &Arc<Runtime>,
    proc: &Proc,
    made: c::Created,
    wa: wisp_worktrees_api::CreateArgs,
    setup: &[String],
) -> WireResult<c::Created> {
    // runtime makes the folder of a task it was allowed to create, on its own grant (`worktrees:create` in its
    // manifest): under an agent's call the agent's set refused it, and a task an agent launched with `worktree: true`
    // ran in the user's main checkout (audit B #5)
    let wt = match ctx.run::<wisp_worktrees_api::Create>(&ctx.root_call(), wa).await {
        Ok(w) => w.and_then(|w| serde_json::to_value(w).ok()),
        Err(err) => {
            tracing::warn!(error = %err.message, task = made.id.as_str(), "runtime: worktrees.create (it runs in its folder)");
            None
        }
    };
    let failed = match wt.as_ref().and_then(|w| w.get("path")).and_then(serde_json::Value::as_str) {
        Some(dir) if !setup.is_empty() => crate::specs::run_setup(proc, dir, setup).await,
        _ => None,
    }
    // the repo's own setup (`.wisp/worktree.json`, Cursor's, a source's) failed in the new worktree: the turn waits with
    // why, as a declared project's does (audit D5: it ran straight to the harness)
    .or_else(|| wt.as_ref().and_then(setup_problem));
    let turn_id = rt.worktree_ready(made.id.as_str(), wt, failed).await.map_err(WireError::from)?;
    Ok(c::Created { id: made.id, turn_id })
}

/// The first setup step that failed in a new worktree (`prepared.problems`, wisp.worktrees' `setup … failed: …`); a
/// file that couldn't be copied isn't one.
fn setup_problem(wt: &serde_json::Value) -> Option<String> {
    wt.pointer("/prepared/problems")?
        .as_array()?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .find(|p| p.starts_with("setup `"))
        .map(str::to_owned)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Audit D5: a failed step of the repo's own setup holds the turn; a copy problem doesn't.
    #[test]
    fn a_failed_setup_step_is_what_holds_a_copy_problem_is_not() {
        let wt = |p: serde_json::Value| serde_json::json!({ "path": "/w", "prepared": { "copied": [], "ran": [], "problems": p } });
        assert_eq!(
            setup_problem(&wt(serde_json::json!(["couldn't copy .env: denied", "setup `npm ci` failed: exit 3"]))).as_deref(),
            Some("setup `npm ci` failed: exit 3")
        );
        assert_eq!(setup_problem(&wt(serde_json::json!(["couldn't copy .env: denied"]))), None);
        assert_eq!(setup_problem(&serde_json::json!({ "path": "/w" })), None);
    }

    /// v1 #794: a side discussion's action buttons run in the main task's folder, so only the user's create may say
    /// which task that is; a `sideOf` another caller wrote is dropped, the rest of its slot kept.
    #[test]
    fn only_the_users_create_marks_a_side_discussion_v1_794() {
        let forged = || {
            let mut a = StartArgs::default();
            a.ext.insert("wisp.runtime".into(), serde_json::json!({ "sideOf": { "id": "other", "title": "x" }, "team": [] }));
            a
        };
        let mut a = forged();
        mark_side(&mut a, None);
        assert_eq!(a.ext["wisp.runtime"], serde_json::json!({ "team": [] }));
        let mut a = forged();
        mark_side(&mut a, Some((&"main".into(), "Fix login")));
        assert_eq!(a.ext["wisp.runtime"]["sideOf"], serde_json::json!({ "id": "main", "title": "Fix login" }));
        let mut a = StartArgs::default();
        mark_side(&mut a, None);
        assert!(!a.ext.contains_key("wisp.runtime"), "nothing added to a plain create");
    }

    /// Review r194 final pass #1: only a plugin's or the kernel's step of the parent's own run inherits its hold.
    #[test]
    fn only_a_step_of_the_agents_run_inherits_its_hold() {
        use wisp_runtime_api::spec::Lineage;
        let step = StartArgs {
            parent_id: Some("run".into()),
            lineage: Lineage { workflow_run: Some("run".into()), ..Lineage::default() },
            ..StartArgs::default()
        };
        assert!(for_the_agents_run("plugin:wisp.factory", &step));
        assert!(for_the_agents_run("kernel", &step));
        for user in ["user:full", "user:channel", "apikey:ci"] {
            assert!(!for_the_agents_run(user, &step), "{user}");
        }
        let side = StartArgs { parent_id: Some("run".into()), ..StartArgs::default() };
        assert!(!for_the_agents_run("plugin:wisp.channels", &side), "a plugin acting for the user (no run step)");
        let other = StartArgs { lineage: Lineage { workflow_run: Some("x".into()), ..Lineage::default() }, ..step.clone() };
        assert!(!for_the_agents_run("plugin:wisp.factory", &other), "a step of another run");
    }
}
