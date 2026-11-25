//! `workflows.edit` (ADR-0121): a run changed while it goes, by its task's agent (`workflows_edit`) or by you (the
//! board's step cards). The change goes to the run's driver, which applies it to a copy and keeps it only when every
//! part took; a run that ended here and is kept starts again when a retry or an added step gives it work. An agent's
//! change is held to its task: its own runs, steps in its folder, at its mode or lower. And a step that fails for good
//! while the run goes on is told to the run's agent at once, so it can retry or skip it.

use std::fmt::Write as _;
use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::oneshot;
use wisp_kernel::{Call, Ctx, Result, WireError};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::{Mode, SessionId};
use wisp_workflows_api::{EditArgs, StepSpec};
use wisp_workflows_core::Run;

use crate::run::{Ctl, RunCtx, State, cap_step, output_line, put_back, revive, take_ended};

/// Applies `a` to `run`, in order: change, add, limits, skip, retry. The tasks of the steps it skipped are returned
/// to stop. `by`: who, in the skipped steps' words; `cap`: the mode an agent's steps may run at.
pub(crate) fn apply(run: &mut Run, a: &EditArgs, by: &str, cap: Option<Mode>, now: i64) -> std::result::Result<Vec<String>, String> {
    for (name, patch) in &a.change {
        let mut patch = patch.clone();
        if let Some(cap) = cap {
            if ["project", "on"].iter().any(|k| patch.get(*k).is_some_and(|v| !v.is_null() && v != "")) {
                return Err(format!("{name}: an agent's steps run in its task's folder on this computer: leave out project and on"));
            }
            // the mode it will run at, held to the task's
            let mut s = StepSpec {
                mode: patch.get("mode").and_then(Value::as_str).map_or_else(|| mode_now(run, name), str::to_owned),
                ..StepSpec::default()
            };
            cap_step(&mut s, cap);
            if let Some(o) = patch.as_object_mut() {
                o.insert("mode".into(), json!(s.mode));
            }
        }
        run.change(name, &patch)?;
    }
    for (name, spec) in &a.add {
        let mut spec = spec.clone();
        if let Some(cap) = cap {
            if !spec.project.is_empty() || !spec.on.is_empty() {
                return Err(format!("{name}: an agent's steps run in its task's folder on this computer: leave out project and on"));
            }
            cap_step(&mut spec, cap);
        }
        run.add(name, spec)?;
    }
    run.limits(a.concurrency, a.budget.as_ref());
    let mut stop = Vec::new();
    for t in &a.skip {
        stop.extend(run.skip(t, &format!("skipped by {by}"), now)?);
    }
    for t in &a.retry {
        run.retry(t)?;
    }
    Ok(stop)
}

/// The mode step `name` says now ('' when none).
fn mode_now(run: &Run, name: &str) -> String {
    run.spec.tasks.get(name).map(|s| s.mode.clone()).unwrap_or_default()
}

/// What the run's agent is told when steps fail for good while the run goes on.
pub(crate) fn failed_message(rc_name: &str, run_id: &str, run: &Run, failed: &[String]) -> String {
    let mut out = format!("wisp: in the workflow {rc_name}, ");
    let lines: Vec<String> = failed
        .iter()
        .map(|id| {
            let r = run.runs.get(id);
            let why = r.map(|r| r.why.clone()).unwrap_or_default();
            let task = r.and_then(|r| r.task.clone()).map(|t| format!(" (task {t})")).unwrap_or_default();
            let said = r.map(|r| output_line(&r.output, 300)).unwrap_or_default();
            format!(
                "{id} failed{task}: {}{}",
                if why.is_empty() { "it ended with an error" } else { &why },
                if said.is_empty() { String::new() } else { format!(" — it said: {said}") }
            )
        })
        .collect();
    out.push_str(&lines.join("; "));
    let ids = json!(failed);
    let _ = write!(
        out,
        ". The steps that need it won't run; the rest go on. workflows_edit {{\"id\": \"{run_id}\", \"retry\": {ids}}} tries it again \
         (change its prompt first with \"change\" if it hasn't a chance as written), {{\"id\": \"{run_id}\", \"skip\": {ids}}} lets the \
         steps after it run without it. Or end your turn: wisp sends you every result when the run ends."
    );
    out
}

/// Tells the run's agent which steps failed (a follow-up, queued behind a turn that's running).
pub(crate) async fn tell_failed(ctx: &Ctx, call: &Call, rc: &RunCtx, run: &Run, failed: &[String]) {
    let text = failed_message(&rc.name, &rc.id, run, failed);
    let a = r::FollowUpArgs {
        id: SessionId(rc.owner.clone()),
        text,
        queue: Some(true),
        by: Some("workflow".into()),
        ..r::FollowUpArgs::default()
    };
    if let Err(err) = ctx.run::<r::FollowUp>(call, a).await {
        tracing::warn!(error = %err.message, task = rc.owner, "workflows: telling the agent a step failed");
    }
}

/// `workflows.edit`.
pub(crate) async fn edit(ctx: &Ctx, call: &Call, st: &Arc<State>, a: EditArgs) -> Result<Value> {
    let agent = call.caller().session.clone();
    let owner = st.live_run(&a.id).map(|(o, _)| o).or_else(|| st.ended_owner(&a.id));
    let Some(owner) = owner else {
        return Err(WireError::not_found(format!("no workflow run {} here (it ended before wisp last started: run it again)", a.id)));
    };
    if agent.as_ref().is_some_and(|s| *s != owner) {
        return Err(WireError::forbidden("an agent changes only its own task's workflow runs"));
    }
    // an agent's steps run at its task's mode or lower
    let cap = match &agent {
        Some(own) => Some(ctx.run::<r::Get>(call, r::IdArgs { id: SessionId(own.clone()) }).await?.mode),
        None => None,
    };
    let by = if agent.is_some() { "the agent" } else { "the user" }.to_owned();
    if let Some((_, ctl)) = st.live_run(&a.id) {
        let (tx, rx) = oneshot::channel();
        if ctl.send(Ctl::Edit(Box::new(a.clone()), by.clone(), cap, tx)).is_ok()
            && let Ok(answer) = rx.await
        {
            return answer.map(|()| said(&a)).map_err(WireError::invalid);
        }
        // it ended in between: kept, it's changed below
    }
    let Some((rc, run)) = take_ended(st, &a.id) else {
        return Err(WireError::not_found(format!("workflow run {} just ended: try again", a.id)));
    };
    if !a.skip.is_empty() || !a.change.is_empty() && a.add.is_empty() && a.retry.is_empty() {
        put_back(st, rc, run);
        return Err(WireError::invalid(
            "the run ended: retry a step or add one to start it again (skip and change are for a run that goes)",
        ));
    }
    let mut next = run.clone();
    let now = ctx.clock()?.now_ms();
    match apply(&mut next, &a, &by, cap, now) {
        Ok(_) if !next.over() => {
            revive(ctx, st, rc, next);
            Ok(said(&a))
        }
        Ok(_) => {
            put_back(st, rc, run);
            Err(WireError::invalid("that gives the ended run nothing to do"))
        }
        Err(why) => {
            put_back(st, rc, run);
            Err(WireError::invalid(why))
        }
    }
}

fn said(a: &EditArgs) -> Value {
    let mut parts: Vec<String> = Vec::new();
    if !a.change.is_empty() {
        parts.push(format!("changed {}", a.change.keys().cloned().collect::<Vec<_>>().join(", ")));
    }
    if !a.add.is_empty() {
        parts.push(format!("added {}", a.add.keys().cloned().collect::<Vec<_>>().join(", ")));
    }
    if a.concurrency.is_some() || a.budget.is_some() {
        parts.push("set its limits".to_owned());
    }
    if !a.skip.is_empty() {
        parts.push(format!("skipped {}", a.skip.join(", ")));
    }
    if !a.retry.is_empty() {
        parts.push(format!("retrying {}", a.retry.join(", ")));
    }
    json!({ "ok": true, "id": a.id, "said": format!("Done: {}. workflows_status says where the run is.", parts.join("; ")) })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use wisp_workflows_api::WorkflowSpec;

    use super::*;

    fn run() -> Run {
        let w = WorkflowSpec {
            tasks: [
                ("a".to_owned(), StepSpec { prompt: "a".into(), ..StepSpec::default() }),
                ("b".to_owned(), StepSpec { prompt: "b".into(), needs: vec!["a".into()], ..StepSpec::default() }),
            ]
            .into(),
            ..WorkflowSpec::default()
        };
        Run::new(w, &BTreeMap::new())
    }

    #[test]
    fn an_agents_change_stays_at_its_mode_and_in_its_folder() {
        let mut r = run();
        let a = EditArgs {
            id: "r".into(),
            add: [("c".to_owned(), StepSpec { prompt: "c".into(), mode: "auto".into(), ..StepSpec::default() })].into(),
            change: [("b".to_owned(), json!({ "mode": "auto" }))].into(),
            ..EditArgs::default()
        };
        apply(&mut r, &a, "the agent", Some(Mode::Plan), 0).unwrap();
        assert_eq!(r.spec.tasks["c"].mode, "plan");
        assert_eq!(r.spec.tasks["b"].mode, "plan");
        let far = EditArgs {
            id: "r".into(),
            add: [("d".to_owned(), StepSpec { prompt: "d".into(), project: "/etc".into(), ..StepSpec::default() })].into(),
            ..EditArgs::default()
        };
        assert!(apply(&mut run(), &far, "the agent", Some(Mode::Auto), 0).is_err_and(|e| e.contains("leave out project")));
        // yours: as written
        let mut r = run();
        apply(&mut r, &a, "you", None, 0).unwrap();
        assert_eq!(r.spec.tasks["c"].mode, "auto");
    }

    #[test]
    fn a_skip_says_who_and_a_failure_message_says_how_to_act() {
        let mut r = run();
        r.next(0);
        r.started("a", "T1");
        let stop = apply(&mut r, &EditArgs { id: "r".into(), skip: vec!["a".into()], ..EditArgs::default() }, "the user", None, 1).unwrap();
        assert_eq!(stop, ["T1"]);
        assert_eq!(r.runs["a"].why, "skipped by the user");
        let mut r = run();
        r.next(0);
        r.started("a", "T1");
        r.finished("a", false, Value::Null, "it ended error", 0.0);
        let m = failed_message("audit", "r9", &r, &["a".to_owned()]);
        assert!(m.starts_with("wisp: in the workflow audit, a failed (task T1): it ended error."), "{m}");
        assert!(m.contains(r#"workflows_edit {"id": "r9", "retry": ["a"]}"#), "{m}");
    }
}
