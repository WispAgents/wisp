//! v1 #794 (test/plan-mode.test.js, test/side.test.js): a task you start in Plan runs commands (`HarnessRun::plan_commands`,
//! which the harness turns into its own flags); a channel's, a trigger's, a capped task's, their subagents' and a side
//! discussion's Plan stays read-only. A side discussion's action buttons (`runtime.exec`) run in the main task's folder.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{rig, until};
use serde_json::json;
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{FollowUpArgs, StartArgs};
use wisp_runtime_api::model::Mode;

fn plan(prompt: &str) -> StartArgs {
    StartArgs { prompt: prompt.into(), cwd: Some(common::tmp()), mode: Some(Mode::Plan), ..StartArgs::default() }
}

fn with_slot(mut a: StartArgs, slot: serde_json::Value) -> StartArgs {
    a.ext.insert("wisp.runtime".into(), slot);
    a
}

/// The flags the fake CLI was started with for the run whose prompt says `prompt`.
async fn ran_with(r: &common::Rig, prompt: &str) -> Vec<String> {
    until(prompt, || async { r.sim.spawned().iter().any(|s| s.args.first().is_some_and(|p| p.contains(prompt))) }).await;
    r.sim.spawned().into_iter().find(|s| s.args.first().is_some_and(|p| p.contains(prompt))).unwrap().args
}

fn runs_commands(args: &[String]) -> bool {
    args.iter().any(|a| a == "--plan-commands")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plan_task_you_start_runs_commands_and_one_from_outside_stays_read_only_v1_794() {
    let r = rig().await;
    r.rt.create(plan("plan yours")).await.unwrap();
    assert!(runs_commands(&ran_with(&r, "plan yours").await), "a Plan task you start runs commands");
    let auto = StartArgs { mode: Some(Mode::Auto), ..plan("auto yours") };
    r.rt.create(auto).await.unwrap();
    assert!(!runs_commands(&ran_with(&r, "auto yours").await), "only Plan");
    let sched = with_slot(plan("plan scheduled"), json!({ "spec": { "lineage": { "triggeredBy": "schedule:nightly" } } }));
    r.rt.create(sched).await.unwrap();
    assert!(runs_commands(&ran_with(&r, "plan scheduled").await), "your own schedule is yours");

    // a channel's task carries its cap; a trigger's says so in its lineage
    let channel = StartArgs { cap: Some(Mode::Plan), by: Some("channel:slack".into()), ..plan("plan channel") };
    let lead = r.rt.create(channel).await.unwrap().id;
    assert!(!runs_commands(&ran_with(&r, "plan channel").await), "a channel's task stays read-only");
    // the user lifting its cap doesn't make what the channel wrote yours
    r.rt.mode_asked(lead.as_str(), Mode::Auto, "user:full").await.unwrap();
    let again = FollowUpArgs { id: lead.clone(), text: "plan channel again".into(), mode: Some(Mode::Plan), ..FollowUpArgs::default() };
    r.rt.follow_up(again).await.unwrap();
    assert!(!runs_commands(&ran_with(&r, "plan channel again").await), "a channel's task, its cap lifted, stays read-only");
    let trig = with_slot(plan("plan triggered"), json!({ "spec": { "lineage": { "triggeredBy": "trigger:on-issue" } } }));
    r.rt.create(trig).await.unwrap();
    assert!(!runs_commands(&ran_with(&r, "plan triggered").await), "a trigger's task stays read-only");

    // a subagent goes by its lead (its own row carries no cap)
    let sub = StartArgs { parent_id: Some(lead), ..plan("plan subagent") };
    r.rt.create(sub).await.unwrap();
    assert!(!runs_commands(&ran_with(&r, "plan subagent").await), "a channel task's subagent stays read-only");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_side_discussions_plan_only_reads_and_its_action_buttons_run_in_the_main_tasks_folder_v1_794() {
    let r = rig().await;
    let main_dir = std::env::temp_dir().join(format!("wisp-794-main-{}", std::process::id()));
    std::fs::create_dir_all(&main_dir).unwrap();
    let main_dir = wisp_proc::canonical_blocking(&main_dir).unwrap().display().to_string();
    let main = StartArgs { cwd: Some(main_dir.clone()), mode: Some(Mode::Auto), ..plan("the main task") };
    let main = r.rt.create(main).await.unwrap().id;
    // what runtime.create keeps for a side discussion the user starts (plugin/create.rs `mark_side`): no folder of its own
    let side = StartArgs {
        cwd: None,
        parent_id: Some(main.clone()),
        ..with_slot(plan("plan side"), json!({ "sideOf": { "id": main.as_str(), "title": "The main task" } }))
    };
    let side = r.rt.create(side).await.unwrap().id;
    assert!(!runs_commands(&ran_with(&r, "plan side").await), "a side discussion's Plan only reads");

    // its actions block's buttons run, where the main task works (v1: was refused)
    let (code, out) = r.rt.exec(side.as_str(), "touch from-side").await.unwrap();
    assert_eq!((code, out.trim()), (0, "ran"));
    let ran = r.sim.spawned().into_iter().rfind(|s| s.bin == "sh").unwrap();
    assert_eq!(ran.args, ["-c", "touch from-side"]);
    assert_eq!(ran.cwd.map(|c| c.display().to_string()), Some(main_dir.clone()), "run where the main task works");

    // a task with a parent but no sideOf (a subagent) runs in its own folder, as before
    let sub = StartArgs { cwd: Some(common::tmp()), parent_id: Some(SessionId(main.0.clone())), ..plan("plan sub of main") };
    let sub = r.rt.create(sub).await.unwrap().id;
    r.rt.exec(sub.as_str(), "true").await.unwrap();
    let ran = r.sim.spawned().into_iter().rfind(|s| s.bin == "sh").unwrap();
    assert_ne!(ran.cwd.map(|c| c.display().to_string()), Some(main_dir.clone()));
    std::fs::remove_dir_all(&main_dir).ok();
}
