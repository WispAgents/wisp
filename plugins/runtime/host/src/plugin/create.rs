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

async fn create(ctx: &Ctx, call: &Call, slot: &Slot, kinds: &Kinds, a: StartArgs) -> WireResult<c::Created> {
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
    let made = rt.create(args).await.map_err(WireError::from)?;
    let made = if wants_tree {
        let mode =
            mode.map(|m| serde_json::to_value(m).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()).unwrap_or_default();
        let wa = wisp_worktrees_api::CreateArgs { session_id: made.id.0.clone(), cwd, title, mode, from: None };
        in_worktree(ctx, &rt, &proc, made, wa, &setup).await?
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
    };
    let turn_id = rt.worktree_ready(made.id.as_str(), wt, failed).await.map_err(WireError::from)?;
    Ok(c::Created { id: made.id, turn_id })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
}
