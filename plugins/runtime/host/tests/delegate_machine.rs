//! A subagent on another machine (ADR-0059, F-mach-7, v1 services/relay.js): `runtime.delegate` copies the lead's work
//! there (`machines.carry`, typed, through the machines api), runs the task there (`machines.route`), and the lead's
//! row, its turn's record and the report name the machine; once the job is done its changes come into the lead's
//! folder (`machines.bring`), once per job.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;

use common::{rig_with, until};
use serde_json::{Value, json};
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::{DelegateArgs, DelegateTask, ResultsArgs, StartArgs};
use wisp_runtime_api::model::TurnStatus;

fn lead(capped: bool) -> StartArgs {
    let mut slot = json!({ "subagents": [{ "name": "far", "agent": "codex", "machine": "m1", "place": "studio" }] });
    if capped {
        slot["capped"] = json!({ "mode": "edits" });
    }
    let mut ext = BTreeMap::new();
    ext.insert("wisp.runtime".to_owned(), slot);
    StartArgs { prompt: "slow lead".into(), cwd: Some(common::tmp()), ext, ..StartArgs::default() }
}

fn answers() -> Recorder {
    let mut answers = BTreeMap::new();
    answers.insert("machines.carry".to_owned(), json!({ "cwd": "/there/copies/k/repo", "machine": "studio" }));
    // what the machine answers for runtime.create / runtime.get / runtime.turns there
    answers.insert(
        "machines.route".to_owned(),
        json!({ "id": "r1", "state": "review", "cwd": "/there/copies/k/repo", "title": "far · lead", "text": "fixed it" }),
    );
    answers.insert("machines.bring".to_owned(), json!({ "files": 2 }));
    answers.insert("machines.uncarry".to_owned(), json!({ "done": true }));
    Recorder { answers, ..Recorder::default() }
}

fn asked(r: &common::Rig, cmd: &str) -> Vec<Value> {
    r.host.asked.lock().unwrap().iter().filter(|(c, _)| c == cmd).map(|(_, a)| a.clone()).collect()
}

async fn running(r: &common::Rig, id: &str) {
    until("the lead runs", || async {
        r.rt.turns(id, None, Some(1)).await.unwrap().pop().is_some_and(|t| t.status == TurnStatus::Running)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delegate_on_another_machine_runs_on_a_copy_there_and_says_where() {
    let r = rig_with(answers()).await;
    let id = r.rt.create(lead(false)).await.unwrap().id;
    running(&r, id.as_str()).await;
    // the lead's folder as the prompt names it, `/` after it even on Windows (`C:\…\Temp/src/a.ts`)
    let task = DelegateTask { subagent: "far".into(), prompt: format!("fix {}/src/a.ts", common::tmp()) };
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task] }).await.unwrap();
    assert!(out[0].error.is_none(), "{out:?}");
    assert_eq!(out[0].id.0, "m1|r1", "kept as <machine>|<task there>");
    assert_eq!(out[0].place.as_deref(), Some("studio"), "the delegation names the machine");

    // the lead's work goes to the machine, one copy per lead and member
    let carry = asked(&r, "machines.carry");
    assert_eq!(carry.len(), 1, "{carry:?}");
    assert_eq!((carry[0]["id"].as_str(), carry[0]["cwd"].as_str()), (Some("m1"), Some(common::tmp().as_str())));
    assert_eq!(carry[0]["key"], format!("{}-far", id.0));
    // the task runs there, in the copy, with the lead's paths as paths in it, told it's elsewhere
    let routed = asked(&r, "machines.route");
    let create = routed.iter().find(|a| a["cmd"] == "runtime.create").unwrap();
    assert_eq!(create["id"], "m1");
    assert_eq!(create["args"]["cwd"], "/there/copies/k/repo");
    assert_eq!(create["args"]["agent"], "codex");
    let prompt = create["args"]["prompt"].as_str().unwrap();
    assert!(prompt.starts_with("fix src/a.ts"), "{prompt}");
    assert!(prompt.contains("another computer than the lead"), "{prompt}");

    // says where: the lead's row, its turn's record
    let l = r.rt.repo.session(id.as_str()).await.unwrap();
    assert_eq!(l.ext["wisp.runtime"]["away"]["m1|r1"], "studio");
    let t = r.rt.turns(id.as_str(), None, Some(1)).await.unwrap().pop().unwrap();
    assert_eq!(t.ext["wisp.runtime"]["delegations"][0]["place"], "studio");
    let rows = serde_json::to_string(&r.rt.activity(t.id.as_str(), None).await.unwrap()).unwrap();
    assert!(rows.contains("Delegated to far on studio"), "{rows}");

    // its report says where it ran, and its changes come into the lead's folder, once per job
    let rep = r.rt.results(ResultsArgs { id: id.clone(), ids: Vec::new(), wait: Some(0) }).await.unwrap();
    let text = &rep.reports[0].text;
    assert!(rep.reports[0].done, "{rep:?}");
    assert!(text.contains("It ran on studio"), "{text}");
    assert!(text.contains("2 file(s) are in your folder"), "{text}");
    let bring = asked(&r, "machines.bring");
    assert_eq!(bring.len(), 1, "{bring:?}");
    assert_eq!(
        (bring[0]["id"].as_str(), bring[0]["from"].as_str(), bring[0]["into"].as_str()),
        (Some("m1"), Some("/there/copies/k/repo"), Some(common::tmp().as_str()))
    );
    let again = r.rt.results(ResultsArgs { id: id.clone(), ids: Vec::new(), wait: Some(0) }).await.unwrap();
    assert!(!again.reports[0].text.contains("in your folder"), "{again:?}");
    assert_eq!(asked(&r, "machines.bring").len(), 1, "brought once");

    // the next job: the copy is made afresh from the lead's folder, and the same task there is followed up
    let task = DelegateTask { subagent: "far".into(), prompt: "and the tests".into() };
    let next = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task] }).await.unwrap();
    assert_eq!((next[0].id.0.as_str(), next[0].place.as_deref()), ("m1|r1", Some("studio")), "{next:?}");
    assert_eq!(asked(&r, "machines.carry").len(), 2);
    assert!(asked(&r, "machines.route").iter().any(|a| a["cmd"] == "runtime.followUp" && a["args"]["id"] == "r1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_capped_leads_work_never_leaves_this_computer() {
    let r = rig_with(answers()).await;
    let id = r.rt.create(lead(true)).await.unwrap().id;
    running(&r, id.as_str()).await;
    let task = DelegateTask { subagent: "far".into(), prompt: "go".into() };
    let out = r.rt.delegate(DelegateArgs { id, tasks: vec![task] }).await.unwrap();
    assert!(out[0].error.as_deref().is_some_and(|e| e.contains("send work to another machine")), "{out:?}");
    assert!(asked(&r, "machines.carry").is_empty(), "nothing carried");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_machine_that_refuses_the_copy_is_that_tasks_error() {
    // no machines.carry answer: the machines plugin is unavailable, and the lead reads why
    let mut rec = answers();
    rec.answers.remove("machines.carry");
    let r = rig_with(rec).await;
    let id = r.rt.create(lead(false)).await.unwrap().id;
    running(&r, id.as_str()).await;
    let task = DelegateTask { subagent: "far".into(), prompt: "go".into() };
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task] }).await.unwrap();
    assert!(out[0].error.as_deref().is_some_and(|e| e.contains("machines.carry")), "{out:?}");
    assert!(asked(&r, "machines.route").is_empty(), "nothing runs there without its copy");
    let l = r.rt.repo.session(id.as_str()).await.unwrap();
    assert!(l.ext["wisp.runtime"].get("away").is_none(), "nothing went: nothing kept");
}

/// ADR-0060: deleting the lead lets each of its delegates on another machine go there (its task and its copy, through
/// `machines.uncarry`, typed), and nothing for a lead that never delegated elsewhere.
#[tokio::test(flavor = "multi_thread")]
async fn deleting_the_lead_lets_its_delegates_elsewhere_go() {
    let r = rig_with(answers()).await;
    let id = r.rt.create(lead(false)).await.unwrap().id;
    running(&r, id.as_str()).await;
    let task = DelegateTask { subagent: "far".into(), prompt: "go".into() };
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task] }).await.unwrap();
    assert_eq!(out[0].id.0, "m1|r1", "{out:?}");
    assert!(asked(&r, "machines.uncarry").is_empty(), "nothing goes while the lead is here");
    // a task that never delegated elsewhere, deleted first: it lets nothing go
    let other = r.rt.create(StartArgs { prompt: "alone".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id;
    r.rt.delete(other.as_str()).await.unwrap();
    r.rt.delete(id.as_str()).await.unwrap();
    until("the delegate there goes", || async { !asked(&r, "machines.uncarry").is_empty() }).await;
    let gone = asked(&r, "machines.uncarry");
    assert_eq!(gone, [json!({ "id": "m1", "lead": id.0, "task": "r1" })], "{gone:?}");
}
