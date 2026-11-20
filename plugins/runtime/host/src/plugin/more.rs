//! asks, approvals, /btw, compact, rerun, fork, subagents, quick, sign-in, overage, projects, harness
//! sessions, a task's setup and the agents (the `more/` surface of the runtime).

use std::sync::Arc;

use wisp_kernel::{Setup, WireError};
use wisp_resources_api::StoredKind;
use wisp_runtime_api::commands as c;

use super::{Slot, rt, track};

/// asks, approvals, /btw, compact, rerun, fork, subagents, quick, sign-in, overage, agents (more.rs)
pub(super) fn more(s: &mut Setup, slot: &Slot, harness_kind: Arc<StoredKind>, kinds_for_run: Arc<StoredKind>) {
    cmd!(s, slot, c::Ask, |r, call, a| r.ask(a).await.map(|ask_id| c::Asked { ask_id }));
    setup_cards(s, slot);
    cmd!(s, slot, c::Unask, |r, call, a| r.unask(&a.id, &a.question).await);
    cmd!(s, slot, c::PollAnswerOf, |r, call, a| Ok(r.poll_answer(&a.ask_id)));
    cmd!(s, slot, c::Approval, |r, call, a| r.approval(a).await);
    cmd!(s, slot, c::ApproveBlocked, |r, call, a| r.approve_blocked(&a.id, a.turn_id.as_ref(), a.allow, &call.caller().set).await);
    cmd!(s, slot, c::Btw, |r, call, a| r.btw(a).await);
    cmd!(s, slot, c::Probe, |r, call, a| r.probe(a).await);
    cmd!(s, slot, c::BtwDrop, |r, call, a| r.btw_drop(a.id.as_str(), &a.btw_id).await.map(|()| c::Ack {}));
    cmd!(s, slot, c::Reveal, |r, call, a| r.reveal(a.id.as_str(), &a.path).await);
    counted(s, slot);
    cmd!(s, slot, c::Rerun, |r, call, a| r.rerun(a).await);
    fork(s, slot);
    cmd!(s, slot, c::Adopt, |r, call, a| r.adopt(a).await);
    cmd!(s, slot, c::Results, |r, call, a| r.results(a).await);
    cmd!(s, slot, c::Quick, |r, call, a| r.quick(a).await);
    cmd!(s, slot, c::QuickCancel, |r, call, a| Ok(c::QuickCancelled { stopped: r.quick_cancel(&a.run_id) }));
    cmd!(s, slot, c::SignIn, |r, call, a| r.sign_in(a.id.as_str(), &a.turn_id).await.map(|_| c::Ack {}));
    cmd!(s, slot, c::SignInDone, |r, call, a| r.sign_in_done(&a.agent, &a.account).await.map(|()| c::Ack {}));
    cmd!(s, slot, c::Overage, |r, call, a| r.overage(a.id.as_str(), &a.turn_id, &a.kind).await.map(|()| c::Ack {}));
    super::projects::project_add(s, kinds_for_run.clone());
    super::projects::project_test(s, kinds_for_run.clone());
    super::projects::discover(s, kinds_for_run.clone());
    super::projects::folders(s);
    super::projects::project_run(s, kinds_for_run);
    super::sessions::sessions(s, slot);
    task_setup(s, slot);
    agents(s, slot, harness_kind);
    cmd!(s, slot, c::DefaultModel, |r, call, a| Ok(r.default_model(&a).await));
}

/// `runtime.taskSetup`: what a task had set up when it ran, and what changed since.
fn task_setup(s: &mut Setup, slot: &Slot) {
    let setups = slot.clone();
    s.command::<c::TaskSetup, _, _>(move |_ctx, _call, a| {
        let slot = setups.clone();
        async move {
            let rt = rt(&slot)?;
            let id = a.id.as_str();
            let s = rt.repo.session(id).await.map_err(WireError::from)?;
            let mut steps = Vec::new();
            for t in rt.repo.turns(id, None, None).await.map_err(WireError::from)? {
                steps.extend(rt.repo.steps(t.id.as_str(), None).await.map_err(WireError::from)?);
            }
            let available =
                crate::setup::load(&rt.repo.store, id).await.map(|a| crate::setup::changed(a, &crate::setup::latest_versions()));
            let note = if available.is_some() {
                serde_json::Value::Null
            } else if s.ext.contains_key("wisp.adopted") {
                serde_json::json!("it ran outside wisp: what it had isn't known")
            } else {
                serde_json::json!("not recorded (it ran before wisp kept setups)")
            };
            Ok(serde_json::json!({ "id": id, "available": available, "note": note, "used": crate::setup::used(&steps) }))
        }
    });
}

/// `runtime.agents`: the harnesses as Settings → Agents shows them, with the declared ones; each lands on the
/// `harnesses` topic as it settles (ADR-0066).
fn agents(s: &mut Setup, slot: &Slot, harness_kind: Arc<StoredKind>) {
    let (agents, harnesses) = (slot.clone(), harness_kind);
    s.command::<c::Agents, _, _>(move |_ctx, _call, a| {
        let (slot, harnesses) = (agents.clone(), harnesses.clone());
        async move {
            let rt = rt(&slot)?;
            Ok(detect(&rt, &harnesses, &a).await)
        }
    });
}

/// Every harness detected, with what `harnesses:` declares applied to each as it lands (the topic and the answer
/// agree). At start too (`plugin/mod.rs`), so the topic is filled before anyone asks.
pub(super) async fn detect(rt: &Arc<crate::runtime::Runtime>, harnesses: &StoredKind, a: &c::AgentsArgs) -> Vec<c::AgentStatus> {
    let specs = harnesses.specs().await.unwrap_or_default();
    rt.agents_as(a, Arc::new(move |s: &mut c::AgentStatus| crate::kinds::declared(std::slice::from_mut(s), &specs))).await
}

/// `runtime.compact` and `runtime.delegate`, counted as v1's `compact` and `delegate` (agent, how many tasks, which
/// harnesses: never what they were asked).
fn counted(s: &mut Setup, slot: &Slot) {
    let sl = slot.clone();
    s.command::<c::Compact, _, _>(move |ctx, _call, a| {
        let slot = sl.clone();
        async move {
            let r = rt(&slot)?;
            let out = r.compact(&a.id).await.map_err(WireError::from)?;
            let agent = r.get(a.id.as_str()).await.map(|x| x.agent).unwrap_or_default();
            track(&ctx, "compact", serde_json::json!({ "agent": agent })).await;
            Ok(out)
        }
    });
    let sl = slot.clone();
    s.command::<c::Delegate, _, _>(move |ctx, _call, a| {
        let slot = sl.clone();
        async move {
            let r = rt(&slot)?;
            let lead = a.id.clone();
            let out = r.delegate(a).await.map_err(WireError::from)?;
            let agent = r.get(lead.as_str()).await.map(|x| x.agent).unwrap_or_default();
            track(&ctx, "delegate", delegate_props(&agent, &out)).await;
            Ok(out)
        }
    });
}

/// v1 internal.js's `delegate`: the lead's harness, how many it started (refusals too), and the harnesses they run on.
pub(crate) fn delegate_props(agent: &str, out: &[c::Delegation]) -> serde_json::Value {
    let mut harnesses: Vec<&str> = Vec::new();
    for d in out.iter().filter(|d| !d.agent.is_empty()) {
        if !harnesses.contains(&d.agent.as_str()) {
            harnesses.push(&d.agent);
        }
    }
    serde_json::json!({ "agent": agent, "tasks": out.len(), "harnesses": harnesses })
}

/// `runtime.fork`, counted as `fork_task`.
fn fork(s: &mut Setup, slot: &Slot) {
    let sl = slot.clone();
    s.command::<c::Fork, _, _>(move |ctx, _call, a| {
        let slot = sl.clone();
        async move {
            let r = rt(&slot)?;
            let (id, latest) = (a.id.clone(), a.turn_id.is_none());
            let out = r.fork(a).await.map_err(WireError::from)?;
            own_folder(&ctx, &r, id.as_str(), out.id.as_str()).await;
            let agent = r.get(id.as_str()).await.map(|x| x.agent).unwrap_or_default();
            track(&ctx, "fork_task", serde_json::json!({ "agent": agent, "switched": false, "latest": latest })).await;
            Ok(out)
        }
    });
}

/// A fork works in a worktree and branch of its own, holding a copy of its parent's folder as it is (audit A orch #4,
/// tasks #2: it ran in its parent's worktree, so both edited one folder). Its harness conversation is copied there too,
/// so the fork's first turn resumes it. Outside a repo (or if git fails) it keeps its parent's folder, as before.
async fn own_folder(ctx: &wisp_kernel::Ctx, r: &Arc<crate::runtime::Runtime>, parent: &str, fork: &str) {
    let (Ok(src), Ok(made)) = (r.get(parent).await, r.get(fork).await) else { return };
    if src.cwd.is_empty() {
        return;
    }
    // made from the project's own checkout (its base, its folder for worktrees), the parent's folder copied in
    let parent_tree =
        src.ext.get(wisp_worktrees_api::EXT).and_then(|w| serde_json::from_value::<wisp_worktrees_api::Worktree>(w.clone()).ok());
    let project = match &parent_tree {
        Some(w) if !w.root.is_empty() => match src.cwd.strip_prefix(w.path.as_str()) {
            Some(sub) => format!("{}{sub}", w.root),
            None => w.root.clone(),
        },
        _ => src.cwd.clone(),
    };
    let wa = wisp_worktrees_api::CreateArgs {
        session_id: fork.to_owned(),
        cwd: project,
        title: made.title.clone(),
        mode: serde_json::to_value(made.mode).ok().and_then(|m| m.as_str().map(str::to_owned)).unwrap_or_default(),
        // the parent's folder gone (its worktree went with its merge, audit F orchestration F2): a fresh branch from the
        // project's base, which holds the merged work, rather than a fork in a folder that isn't there
        from: wisp_proc::canonical(&src.cwd).await.is_ok().then(|| src.cwd.clone()),
        ..wisp_worktrees_api::CreateArgs::default()
    };
    // on runtime's own grant, as a new task's (create.rs `in_worktree`): an agent's set has no worktrees:create
    let wt = match ctx.run::<wisp_worktrees_api::Create>(&ctx.root_call(), wa).await {
        Ok(Some(w)) => w,
        Ok(None) => return,
        Err(err) => {
            tracing::warn!(error = %err.message, task = fork, "runtime: a fork's worktree (it runs in its parent's folder)");
            return;
        }
    };
    if let Some(resume) = made.resume_id.as_deref() {
        for h in ctx.contributions(&wisp_runtime_api::harness::sessions::SESSIONS).into_iter().filter(|h| h.harness() == made.agent) {
            if let Some(raw) = h.raw(resume).await {
                let _ = h.restore(resume, &wt.cwd, raw).await;
            }
        }
    }
    if let Err(err) = r.worktree_ready(fork, serde_json::to_value(&wt).ok(), None).await {
        tracing::warn!(error = %err, task = fork, "runtime: a fork's worktree not saved");
    }
}

/// `runtime.setup.ask` and `runtime.answer` (ADR-0113): a setup card is answered by the user, never an agent, and its
/// Allow runs the stored writes with wisp.runtime's own call (its manifest grants just those writes), so a channel's or
/// the phone's tap allows it as the app's does.
fn setup_cards(s: &mut Setup, slot: &Slot) {
    use wisp_runtime_core::setup::{self as su, Normalized};
    let sl = slot.clone();
    s.command::<c::SetupAsk, _, _>(move |ctx, call, a| {
        let slot = sl.clone();
        async move {
            let r = rt(&slot)?;
            // v1 offered none of these to a capped task (mcp-ask.js CAPPED_OFF)
            if call.caller().set.starts_with("agent:capped") {
                return Err(WireError::forbidden(wisp_kernel::caps::capped_refusal("change the user's setup")));
            }
            let task = r.get(a.id.as_str()).await.map_err(WireError::from)?;
            let t = su::Task { session: a.id.as_str(), cwd: &task.cwd };
            let writes = match su::normalize(&a.command, &a.args, t).map_err(WireError::invalid)? {
                Normalized::Writes(w) => w,
                Normalized::Import { from } => vec![read_crew(&ctx, &from, &task.cwd).await?],
            };
            let who = asker(&ctx, &r, &a.id).await;
            r.setup_ask(&a.id, &who, writes).await.map_err(WireError::from)
        }
    });
    let sl = slot.clone();
    s.command::<c::Answer, _, _>(move |ctx, call, a| {
        let slot = sl.clone();
        async move {
            let r = rt(&slot)?;
            if !r.asks_setup(&a.id).await {
                return r.answer(&a.id, &a.text, a.ask_id.as_deref()).await.map_err(WireError::from);
            }
            let who = call.caller();
            let relay = who
                .set
                .strip_prefix("plugin:")
                .or(who.plugin.as_deref())
                .is_some_and(|p| ctx.manifests().iter().any(|m| m.id == p && m.permissions.iter().any(|x| x == crate::more::RELAY)));
            let keys: Vec<String> = ctx.setting(crate::more::KEYS_SETTING).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
            if !crate::more::may_answer(&who.set, relay, &keys) {
                return Err(WireError::forbidden("This card is the user's to answer: an agent can't allow a change to the user's setup."));
            }
            let run = move |cmd: String, args: serde_json::Value| {
                let ctx = ctx.clone();
                async move { ctx.run_json(&ctx.root_call(), &cmd, args).await.map_err(|e| e.message) }
            };
            r.answer_setup(&a.id, &a.text, a.ask_id.as_deref(), run).await.map_err(WireError::from)
        }
    });
}

/// A crew `from` a link or a pasted file, read before the card (ADR-0113): crew's own dry run, whose bundle is what
/// Allow imports, so what the card lists is what joins (no second fetch).
async fn read_crew(ctx: &wisp_kernel::Ctx, from: &str, cwd: &str) -> wisp_kernel::Result<wisp_runtime_core::setup::Write> {
    use wisp_crew_api::commands as cc;
    let link = from.starts_with("https://") || from.starts_with("http://") || from.starts_with("gist:");
    let src = cc::ImportArgs {
        url: link.then(|| from.to_owned()),
        text: (!link).then(|| from.to_owned()),
        dry_run: true,
        ..cc::ImportArgs::default()
    };
    let review = ctx.run::<cc::Import>(&ctx.root_call(), src).await?;
    if review.locked.is_some() {
        return Err(WireError::invalid("That crew is locked with a password: the user imports it from Crew → Import."));
    }
    if review.spell.is_some() {
        return Err(WireError::invalid("That's a spell, not a crew."));
    }
    let bundle = review.bundle.filter(|b| !b.is_null()).ok_or_else(|| WireError::invalid("No crew to read there."))?;
    let keys: Vec<String> =
        review.members.iter().filter_map(|m| m.get("key").and_then(serde_json::Value::as_str).map(str::to_owned)).collect();
    Ok(wisp_runtime_core::setup::import_write(bundle, &keys, cwd))
}

/// Who asks, as the card names them: the crew member whose job it is, else the task's harness.
async fn asker(ctx: &wisp_kernel::Ctx, r: &Arc<crate::Runtime>, id: &wisp_runtime_api::SessionId) -> String {
    let Ok(s) = r.get(id.as_str()).await else { return String::new() };
    if let Some(member) = s.ext.get("wisp.crew").and_then(|c| c.get("id")).and_then(serde_json::Value::as_str) {
        let args = wisp_crew_api::commands::IdArgs { id: member.to_owned() };
        if let Ok(m) = ctx.run::<wisp_crew_api::commands::Get>(&ctx.root_call(), args).await {
            let name = serde_json::to_value(&m).ok().and_then(|v| v.get("name").and_then(serde_json::Value::as_str).map(str::to_owned));
            if let Some(n) = name.filter(|n| !n.is_empty()) {
                return n;
            }
        }
    }
    r.harness(&s.agent).map(|h| h.info().name.clone()).unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::delegate_props;

    /// v1's `delegate`: every task it started counts (a refused one too), each harness once, never what was asked.
    #[test]
    fn a_delegation_counts_its_tasks_and_names_each_harness_once() {
        let d = |agent: &str| {
            serde_json::from_value::<wisp_runtime_api::commands::Delegation>(serde_json::json!({
                "id": "s1", "subagent": "Reviewer", "agent": agent, "model": "", "effort": "", "turnId": "t1"
            }))
            .unwrap()
        };
        let p = delegate_props("claude", &[d("codex"), d("codex"), d(""), d("claude")]);
        assert_eq!(p, serde_json::json!({ "agent": "claude", "tasks": 4, "harnesses": ["codex", "claude"] }));
    }
}
