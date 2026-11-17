//! Delegate's team reaches the lead (v1 server/routes/sessions.js followUpTeam, launch.js `subagents`): New task's
//! inline team and a reply box's team become the task's `subagents`, the roster `wisp_delegate` reads, named and
//! checked as v1's normalizeTeam. A follow-up's team is the lead's from then on; `[]` removes it; one without a team
//! keeps what it had; a subagent can't be given one (r330 #27: F-comp-33 sent nothing).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Rig, rig, until};
use serde_json::{Value, json};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{FollowUpArgs, StartArgs};
use wisp_runtime_api::model::TurnStatus;

async fn subagents(r: &Rig, id: &str) -> Vec<Value> {
    let s = r.rt.get(id).await.unwrap();
    s.ext.get("wisp.runtime").and_then(|o| o.get("subagents")).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn names(team: &[Value]) -> Vec<String> {
    team.iter().map(|m| m["name"].as_str().unwrap_or_default().to_owned()).collect()
}

async fn idle(r: &Rig, id: &str) {
    until("its turn ended", || async {
        r.rt.turns(id, None, Some(1)).await.unwrap().pop().is_some_and(|t| t.status != TurnStatus::Running)
    })
    .await;
}

async fn task(r: &Rig, team: Option<Value>) -> SessionId {
    let id = r.rt.create(StartArgs { prompt: "hello".into(), cwd: Some(common::tmp()), team, ..StartArgs::default() }).await.unwrap().id;
    idle(r, id.as_str()).await;
    id
}

fn follow(id: &SessionId, text: &str, team: Option<Value>) -> FollowUpArgs {
    FollowUpArgs { id: id.clone(), text: text.into(), team, ..FollowUpArgs::default() }
}

#[tokio::test]
async fn new_tasks_inline_team_becomes_the_leads_subagents_v1_launch_subagents() {
    let r = rig().await;
    let id = task(&r, Some(json!([{ "agent": "fake" }, { "agent": "fake", "count": 3, "use": "  the tests " }]))).await;
    let team = subagents(&r, id.as_str()).await;
    assert_eq!(names(&team), ["fake", "fake-2"]);
    assert_eq!(team[1]["count"], 3);
    assert_eq!(team[1]["use"], "the tests");
}

#[tokio::test]
async fn a_follow_up_with_a_team_gives_the_lead_that_team_from_then_on_v1_follow_up_team() {
    let r = rig().await;
    let id = task(&r, None).await;
    assert_eq!(subagents(&r, id.as_str()).await, Vec::<Value>::new());
    r.rt.follow_up(follow(&id, "now with help", Some(json!([{ "agent": "fake", "name": "Tester" }])))).await.unwrap();
    idle(&r, id.as_str()).await;
    assert_eq!(names(&subagents(&r, id.as_str()).await), ["tester"]);
    // one without a team keeps it
    r.rt.follow_up(follow(&id, "carry on", None)).await.unwrap();
    idle(&r, id.as_str()).await;
    assert_eq!(names(&subagents(&r, id.as_str()).await), ["tester"]);
    // [] removes it
    r.rt.follow_up(follow(&id, "alone now", Some(json!([])))).await.unwrap();
    idle(&r, id.as_str()).await;
    assert_eq!(subagents(&r, id.as_str()).await, Vec::<Value>::new());
}

#[tokio::test]
async fn a_team_with_an_unknown_harness_is_refused_and_the_follow_up_not_sent() {
    let r = rig().await;
    let id = task(&r, None).await;
    let turns = r.rt.turns(id.as_str(), None, None).await.unwrap().len();
    let err = r.rt.follow_up(follow(&id, "with a ghost", Some(json!([{ "agent": "ghost" }])))).await.unwrap_err();
    assert!(err.to_string().contains("unknown subagent harness: ghost"), "{err}");
    assert_eq!(r.rt.turns(id.as_str(), None, None).await.unwrap().len(), turns);
    assert_eq!(subagents(&r, id.as_str()).await, Vec::<Value>::new());
}

#[tokio::test]
async fn a_subagent_cant_be_given_a_team_of_its_own() {
    let r = rig().await;
    let lead = task(&r, None).await;
    let sub =
        r.rt.create(StartArgs { prompt: "hello".into(), cwd: Some(common::tmp()), parent_id: Some(lead.clone()), ..StartArgs::default() })
            .await
            .unwrap()
            .id;
    idle(&r, sub.as_str()).await;
    let err = r.rt.follow_up(follow(&sub, "more", Some(json!([{ "agent": "fake" }])))).await.unwrap_err();
    assert!(err.to_string().contains("subagent can’t have subagents"), "{err}");
}
