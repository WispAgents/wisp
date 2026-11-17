//! Auto is switched off until the owner reworks it (owner, 2026-11-17; docs/v2-plan.md §3): one switch,
//! `runtime.auto` (default off). Off, a task that names no harness (the CLI, MCP, crew, automations) or names `auto`
//! never asks the router: it runs on the harness New task last picked (`runtime.launch`, v1 config.defaults.agent),
//! else the first one. On, Auto picks as before (lifecycle.rs `a_task_on_auto_runs_on_the_routers_pick_and_teaches_it`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};

use common::{Fake, rig_on};
use serde_json::json;
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::StartArgs;

/// A router that would always pick `two`, and counts how often it's asked.
#[derive(Default)]
struct Asked(Mutex<u32>);

impl wisp_router_api::service::RouterApi for Asked {
    fn pick(
        &self,
        _call: &wisp_kernel::Call,
        _req: wisp_router_api::service::PickRequest,
    ) -> wisp_kernel::Result<Option<wisp_router_api::Pick>> {
        *self.0.lock().unwrap() += 1;
        Ok(Some(wisp_router_api::Pick { agent: "two".into(), label: "Two".into(), ..wisp_router_api::Pick::default() }))
    }
    fn classify(
        &self,
        _call: &wisp_kernel::Call,
        _err: &wisp_router_api::service::RunError,
    ) -> wisp_kernel::Result<Option<wisp_router_api::Limit>> {
        Ok(None)
    }
    fn report(&self, _call: &wisp_kernel::Call, _o: wisp_router_api::service::Outcome) -> wisp_kernel::Result<()> {
        Ok(())
    }
}

fn start(agent: Option<&str>) -> StartArgs {
    StartArgs { prompt: "fix the login".into(), cwd: Some(common::tmp()), agent: agent.map(str::to_owned), ..StartArgs::default() }
}

async fn agent_of(settings: &[(&str, serde_json::Value)], agent: Option<&str>) -> (String, u32) {
    let router = Arc::new(Asked::default());
    let host = Recorder {
        router: Some(router.clone()),
        settings: settings.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect(),
        ..Recorder::default()
    };
    let r = rig_on(host, vec![Fake::harness(), Fake::named("two", "Two", &[]), Fake::named("three", "Three", &[])]).await;
    let id = r.rt.create(start(agent)).await.unwrap().id;
    let s = r.rt.get(id.as_str()).await.unwrap();
    (s.agent, *router.0.lock().unwrap())
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_is_off_by_default_a_task_with_no_harness_never_asks_the_router() {
    // nothing picked yet: the first harness
    assert_eq!(agent_of(&[], None).await, ("fake".to_owned(), 0));
    // `auto` by name (an old crew member, a formula): the same
    assert_eq!(agent_of(&[], Some("auto")).await, ("fake".to_owned(), 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn with_auto_off_no_harness_means_the_one_new_task_last_picked_v1_defaults_agent() {
    let launch = json!({ "agent": "three", "models": {}, "efforts": {} });
    assert_eq!(agent_of(&[("runtime.launch", launch.clone())], None).await, ("three".to_owned(), 0));
    // a last pick that isn't installed any more: the first one
    assert_eq!(agent_of(&[("runtime.launch", json!({ "agent": "gone" }))], None).await, ("fake".to_owned(), 0));
    // a harness named wins over the last pick
    assert_eq!(agent_of(&[("runtime.launch", launch)], Some("two")).await, ("two".to_owned(), 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_one_switch_turns_auto_back_on() {
    assert_eq!(agent_of(&[("runtime.auto", json!(true))], None).await, ("two".to_owned(), 1));
}
