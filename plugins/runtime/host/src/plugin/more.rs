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
    cmd!(s, slot, c::Answer, |r, call, a| r.answer(&a.id, &a.text).await);
    cmd!(s, slot, c::PollAnswerOf, |r, call, a| Ok(r.poll_answer(&a.ask_id)));
    cmd!(s, slot, c::Approval, |r, call, a| r.approval(a).await);
    cmd!(s, slot, c::ApproveBlocked, |r, call, a| r.approve_blocked(&a.id, a.turn_id.as_ref(), a.allow, &call.caller().set).await);
    cmd!(s, slot, c::Btw, |r, call, a| r.btw(a).await);
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
    super::projects::project_test(s, kinds_for_run.clone());
    super::projects::discover(s, kinds_for_run.clone());
    super::projects::folders(s);
    super::projects::project_run(s, kinds_for_run);
    super::sessions::sessions(s, slot);
    task_setup(s, slot);
    agents(s, slot, harness_kind);
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

/// `runtime.agents`: the harnesses as Settings → Agents shows them, with the declared ones.
fn agents(s: &mut Setup, slot: &Slot, harness_kind: Arc<StoredKind>) {
    let (agents, harnesses) = (slot.clone(), harness_kind);
    s.command::<c::Agents, _, _>(move |_ctx, _call, a| {
        let (slot, harnesses) = (agents.clone(), harnesses.clone());
        async move {
            let mut list = rt(&slot)?.agents(&a).await;
            crate::kinds::declared(&mut list, &harnesses.specs().await.unwrap_or_default());
            Ok(list)
        }
    });
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
            let agent = r.get(id.as_str()).await.map(|x| x.agent).unwrap_or_default();
            track(&ctx, "fork_task", serde_json::json!({ "agent": agent, "switched": false, "latest": latest })).await;
            Ok(out)
        }
    });
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
