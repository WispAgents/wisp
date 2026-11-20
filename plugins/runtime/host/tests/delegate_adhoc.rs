//! A lead with no team makes a subagent on the fly (v1 control.js `adhocTeam`; audit H1): a task that names a
//! harness (`agent`, with an optional `name`) and no `subagent` joins the lead's team and runs; the same name again
//! continues that subagent; an unknown harness is that task's error, never the whole call's.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Rig, open, rig, until};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{DelegateArgs, DelegateTask, StartArgs};
use wisp_runtime_api::model::{Turn, TurnStatus};

async fn last(r: &Rig, id: &str) -> Option<Turn> {
    r.rt.turns(id, None, Some(1)).await.unwrap().pop()
}

/// A lead on the fake harness with no team, its first turn held at gate `gate`.
async fn lone_lead(r: &Rig, gate: &str) -> SessionId {
    let id =
        r.rt.create(StartArgs { prompt: format!("gate {gate} lead"), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id;
    until("the lead runs", || async { last(r, id.as_str()).await.is_some_and(|t| t.status == TurnStatus::Running) }).await;
    id
}

fn task(agent: Option<&str>, name: Option<&str>, prompt: &str) -> DelegateTask {
    DelegateTask { agent: agent.map(str::to_owned), name: name.map(str::to_owned), prompt: prompt.into(), ..DelegateTask::default() }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lead_with_no_team_makes_a_subagent_on_the_fly_audit_h1() {
    let r = rig().await;
    let id = lone_lead(&r, "adhoc-l").await;

    // an_agent_and_a_name_make_a_subagent_and_run_it
    let out =
        r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task(Some("fake"), Some("Tester"), "gate adhoc-k run the tests")] })
            .await
            .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].error.clone(), out[0].subagent.as_str(), out[0].agent.as_str()), (None, "tester", "fake"), "{out:?}");
    let child = out[0].id.clone();
    until("the subagent runs", || async { last(&r, child.as_str()).await.is_some_and(|t| t.status == TurnStatus::Running) }).await;
    let lead = r.rt.get(id.as_str()).await.unwrap();
    let team = lead.ext_of("wisp.runtime")["subagents"].as_array().cloned().unwrap_or_default();
    assert_eq!(team.iter().map(|m| m["name"].as_str().unwrap_or_default()).collect::<Vec<_>>(), ["tester"], "it joined the team");
    assert_eq!(r.rt.get(child.as_str()).await.unwrap().parent_id.as_ref(), Some(&id));

    // the_same_name_again_continues_it
    open("adhoc-k");
    until("its task ended", || async { last(&r, child.as_str()).await.is_some_and(|t| t.status != TurnStatus::Running) }).await;
    let again = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task(None, Some("tester"), "and the lint")] }).await.unwrap();
    assert_eq!((again[0].error.clone(), again[0].id.clone()), (None, child.clone()), "{again:?}");

    // an_unknown_harness_is_that_tasks_error_only, and a harness alone names it
    let mixed =
        r.rt.delegate(DelegateArgs {
            id: id.clone(),
            tasks: vec![task(Some("nosuch"), None, "x"), task(Some("fake"), None, "gate adhoc-f go")],
        })
        .await
        .unwrap();
    assert!(mixed[0].error.as_deref().is_some_and(|e| e.contains("nosuch")), "{mixed:?}");
    assert_eq!((mixed[1].error.clone(), mixed[1].subagent.as_str()), (None, "fake"), "{mixed:?}");
    let team = r.rt.get(id.as_str()).await.unwrap().ext_of("wisp.runtime")["subagents"].clone();
    assert_eq!(team.as_array().map(Vec::len), Some(2), "the unknown one didn't join: {team}");
    // the same harness again with no name is the same subagent, not fake-2 (review of audit H1)
    open("adhoc-f");
    until("its task ended", || async { last(&r, mixed[1].id.as_str()).await.is_some_and(|t| t.status != TurnStatus::Running) }).await;
    let same = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task(Some("fake"), None, "and once more")] }).await.unwrap();
    assert_eq!((same[0].error.clone(), same[0].id.clone()), (None, mixed[1].id.clone()), "{same:?}");
    let team = r.rt.get(id.as_str()).await.unwrap().ext_of("wisp.runtime")["subagents"].clone();
    assert_eq!(team.as_array().map(Vec::len), Some(2), "{team}");
    assert!(team.as_array().unwrap().iter().all(|m| m["agent"] != "nosuch"));
    open("adhoc-l");
}

/// Audit H HS11: `delegate {agent: "frob"}` from a lead with no team came back `{"reports":[]}`: the task's error
/// names the harnesses there are, as `--agent frob` does (the MCP tool says it: `wisp_mcp_core::words::delegated`).
#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_harness_alone_is_refused_naming_the_harnesses_audit_h_hs11() {
    let r = rig().await;
    let id = lone_lead(&r, "adhoc-u").await;
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task(Some("frob"), None, "x")] }).await.unwrap();
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!((out[0].subagent.as_str(), out[0].error.as_deref()), ("frob", Some("no harness frob: name one of fake")), "{out:?}");
    open("adhoc-u");
}
