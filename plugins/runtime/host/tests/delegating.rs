//! A lead told to delegate through wisp (the team note in its system prompt) runs with its harness's own subagent
//! tool off (`HarnessRun::delegating`): one predicate decides both. A side discussion has its main task's team
//! (ADR-0123) and gets both; a subagent, a host without wisp's MCP server and a harness that can't take wisp's tools
//! get neither.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;

use common::{Fake, Rig, rig_on, until};
use serde_json::json;
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::StartArgs;
use wisp_runtime_api::points::AgentAccess;

struct Access;
impl AgentAccess for Access {
    fn url(&self) -> String {
        "http://127.0.0.1:1".into()
    }
    fn token(&self, session: &str) -> String {
        format!("t-{session}")
    }
}

async fn ran_with(r: &Rig, prompt: &str) -> Vec<String> {
    until(prompt, || async { r.sim.spawned().iter().any(|s| s.args.first().is_some_and(|p| p.contains(prompt))) }).await;
    r.sim.spawned().into_iter().find(|s| s.args.first().is_some_and(|p| p.contains(prompt))).unwrap().args
}

fn told(args: &[String]) -> (bool, bool) {
    (args.iter().any(|a| a == "--subagents-note"), args.iter().any(|a| a == "--delegating"))
}

fn start(prompt: &str) -> StartArgs {
    StartArgs { prompt: prompt.into(), agent: Some("fake".into()), cwd: Some(common::tmp()), ..StartArgs::default() }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_top_level_lead_or_its_side_discussion_delegates_through_wisp_and_a_subagent_does_not() {
    let r = rig_on(Recorder { access: Some(Arc::new(Access)), ..Recorder::default() }, vec![Fake::leads("fake", "Fake CLI")]).await;
    let lead = r.rt.create(start("the lead")).await.unwrap().id;
    assert_eq!(told(&ran_with(&r, "the lead").await), (true, true), "the note and the flag together");
    r.rt.create(StartArgs { parent_id: Some(lead.clone()), ..start("a subagent") }).await.unwrap();
    assert_eq!(told(&ran_with(&r, "a subagent").await), (false, false));
    let mut side = StartArgs { parent_id: Some(lead.clone()), cwd: None, ..start("a side discussion") };
    side.ext.insert("wisp.runtime".into(), json!({ "sideOf": { "id": lead.as_str(), "title": "The lead" } }));
    r.rt.create(side).await.unwrap();
    assert_eq!(told(&ran_with(&r, "a side discussion").await), (true, true), "a side has its main task's team");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_wisps_mcp_server_or_its_tools_a_lead_keeps_its_own_subagents() {
    let r = rig_on(Recorder::default(), vec![Fake::leads("fake", "Fake CLI")]).await;
    r.rt.create(start("no access")).await.unwrap();
    assert_eq!(told(&ran_with(&r, "no access").await), (false, false));
    let r =
        rig_on(Recorder { access: Some(Arc::new(Access)), ..Recorder::default() }, vec![Fake::named("fake", "Fake CLI", &["low"])]).await;
    r.rt.create(start("no ask tool")).await.unwrap();
    assert_eq!(told(&ran_with(&r, "no ask tool").await), (false, false), "a harness that can't take wisp's tools");
}
