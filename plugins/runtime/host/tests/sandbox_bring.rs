//! A sandbox member's changes come back into the lead's folder once per job (ADR-0047): a bring that didn't answer
//! marks nothing and is tried again, a clash says where the patch is kept, each follow-up job brings its own, a lead in
//! Plan or a job that didn't finish has its changes held (kept, the folder untouched), and a member whose box is gone
//! gets a new one. The sandbox and the wisp there are answers the fake host gives (`sandbox.*`, `machines.route …`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;

use common::{Rig, until};
use serde_json::{Value, json};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{DelegateArgs, DelegateTask, ResultsArgs, StartArgs};
use wisp_runtime_api::model::{Mode, TurnStatus};

/// A host for a sandbox member: the sandbox placed (`sb1`), its task there (`r1`, turn `rt1`), and `bring` as
/// `sandbox.bring`'s answer (None: it doesn't answer).
fn host(bring: Option<Value>) -> wisp_runtime::Recorder {
    let mut answers = BTreeMap::new();
    answers.insert(
        "sandbox.place".to_owned(),
        json!({ "machine": "sb1", "peer": "p", "sandbox": "x", "cwd": "/home/u/code/x", "made": true }),
    );
    answers.insert("machines.route runtime.create".to_owned(), json!({ "id": "r1", "turnId": "rt1" }));
    if let Some(b) = bring {
        answers.insert("sandbox.bring".to_owned(), b);
    }
    wisp_runtime::Recorder { answers, ..wisp_runtime::Recorder::default() }
}

/// The task there is done with these turns (`(id, status)`).
fn done_there(r: &Rig, turns: &[(&str, &str)]) {
    let list: Vec<Value> = turns.iter().map(|(id, s)| json!({ "id": id, "status": s, "text": format!("did {id}") })).collect();
    let mut live = r.host.live.lock().unwrap();
    live.insert(
        "machines.route runtime.get".to_owned(),
        json!({ "id": "r1", "state": "review", "cwd": "/home/u/code/x", "title": "far · lead" }),
    );
    live.insert("machines.route runtime.turns".to_owned(), json!(list));
}

/// A lead (in `mode`) whose member `far` works in a sandbox, its own turn held at gate `gate` (it never ends here).
async fn lead(r: &Rig, gate: &str, mode: Option<Mode>) -> SessionId {
    let mut ext = BTreeMap::new();
    ext.insert("wisp.runtime".to_owned(), json!({ "subagents": [{ "name": "far", "agent": "codex", "machine": "sandbox" }] }));
    let start = StartArgs { prompt: format!("gate {gate} lead"), cwd: Some(common::tmp()), mode, ext, ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    until("the lead runs", || async {
        r.rt.turns(id.as_str(), None, Some(1)).await.unwrap().pop().is_some_and(|t| t.status == TurnStatus::Running)
    })
    .await;
    id
}

async fn delegate(r: &Rig, id: &SessionId) -> String {
    let task = DelegateTask { subagent: "far".into(), prompt: "fix it".into(), ..Default::default() };
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task] }).await.unwrap();
    assert!(out[0].error.is_none(), "{out:?}");
    out[0].id.0.clone()
}

/// `subagent_results` on the member's job: its report's text.
async fn report(r: &Rig, id: &SessionId, key: &str) -> String {
    let rep = r.rt.results(ResultsArgs { id: id.clone(), ids: vec![SessionId(key.into())], wait: Some(0) }).await.unwrap();
    assert!(rep.reports[0].done, "{rep:?}");
    rep.reports[0].text.clone()
}

fn asked(r: &Rig, cmd: &str) -> Vec<Value> {
    r.host
        .asked
        .lock()
        .unwrap()
        .iter()
        .filter(|(c, a)| c == cmd || format!("{c} {}", a["cmd"].as_str().unwrap_or_default()) == cmd)
        .map(|(_, a)| a.clone())
        .collect()
}

/// The lead's marks of jobs brought (both lists, the per-job one and the old per-member one).
async fn marks(r: &Rig, id: &SessionId) -> Vec<String> {
    let s = r.rt.get(id.as_str()).await.unwrap().ext_of("wisp.runtime").clone();
    ["broughtJobs", "brought"]
        .iter()
        .flat_map(|k| s[*k].as_array().cloned().unwrap_or_default())
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// The sandbox asleep or unreachable (the bring fails): no mark is written, the note says it's tried again, and the
/// next look brings it. (Before, the mark went on anyway: the job's changes never came back.)
#[tokio::test(flavor = "multi_thread")]
async fn a_bring_that_fails_marks_nothing_and_is_tried_again() {
    let r = common::rig_with(host(None)).await;
    let id = Box::pin(lead(&r, "sbx-err-l", None)).await;
    let key = delegate(&r, &id).await;
    done_there(&r, &[("rt1", "done")]);
    let text = report(&r, &id, &key).await;
    assert!(text.contains("didn't come back yet") && text.contains("tried again"), "{text}");
    assert!(marks(&r, &id).await.is_empty(), "nothing marked brought: {:?}", marks(&r, &id).await);
    report(&r, &id, &key).await;
    assert_eq!(asked(&r, "sandbox.bring").len(), 2, "tried again");
    // it answers now: brought, and marked
    r.host.live.lock().unwrap().insert("sandbox.bring".to_owned(), json!({ "files": 1 }));
    let text = report(&r, &id, &key).await;
    assert!(text.contains("(Its changes to 1 file(s) are in your folder now.)"), "{text}");
    assert_eq!(marks(&r, &id).await, ["sb1|r1@rt1"]);
    report(&r, &id, &key).await;
    assert_eq!(asked(&r, "sandbox.bring").len(), 3, "once brought, never again");
}

/// A clash: the patch was kept in a file, the job is marked brought, and the note says where and how to apply it.
#[tokio::test(flavor = "multi_thread")]
async fn a_bring_that_clashes_is_marked_and_says_where_its_changes_are_kept() {
    let clash = json!({ "files": 1, "error": "Its changes clash with yours here", "kept": "/w/plugins/wisp.sandbox/patches/p1.patch" });
    let r = common::rig_with(host(Some(clash))).await;
    let id = Box::pin(lead(&r, "sbx-clash-l", None)).await;
    let key = delegate(&r, &id).await;
    done_there(&r, &[("rt1", "done")]);
    let text = report(&r, &id, &key).await;
    let at = "/w/plugins/wisp.sandbox/patches/p1.patch";
    assert!(text.contains(&format!("Its changes clash with yours: kept at {at}. Apply with git apply {at}")), "{text}");
    assert_eq!(marks(&r, &id).await, ["sb1|r1@rt1"]);
    let bring = &asked(&r, "sandbox.bring")[0];
    assert!(bring.get("keep").is_none(), "a finished job for a lead in Auto goes in: {bring}");
}

/// Two follow-up jobs on one sandbox member (one key, turns rt1 and rt2): each brings its own changes, and each note
/// reaches the lead. (Before, the member was marked once: the second job's changes never came back.)
#[tokio::test(flavor = "multi_thread")]
async fn every_job_of_a_sandbox_member_brings_its_own_changes() {
    let r = common::rig_with(host(Some(json!({ "files": 1 })))).await;
    r.host.live.lock().unwrap().insert("machines.route runtime.followUp".to_owned(), json!({ "turnId": "rt2" }));
    let id = Box::pin(lead(&r, "sbx-two-l", None)).await;
    let key = delegate(&r, &id).await;
    done_there(&r, &[("rt1", "done")]);
    assert!(report(&r, &id, &key).await.contains("(Its changes to 1 file(s)"));
    assert_eq!(delegate(&r, &id).await, key, "the same member there, followed up");
    r.host.live.lock().unwrap().insert("sandbox.bring".to_owned(), json!({ "files": 3 }));
    done_there(&r, &[("rt1", "done"), ("rt2", "done")]);
    let text = report(&r, &id, &key).await;
    assert!(text.contains("did rt2") && text.contains("(Its changes to 3 file(s)"), "{text}");
    assert_eq!(asked(&r, "sandbox.bring").len(), 2, "once per job");
    let mut m = marks(&r, &id).await;
    m.sort();
    assert_eq!(m, ["sb1|r1@rt1", "sb1|r1@rt2"]);
}

/// A job brought under the old per-member mark (`brought: [key]`) isn't brought again; the mark moves to that job, so
/// the member's next job comes back.
#[tokio::test(flavor = "multi_thread")]
async fn a_job_brought_under_the_old_mark_isnt_brought_again() {
    let r = common::rig_with(host(Some(json!({ "files": 1 })))).await;
    let id = Box::pin(lead(&r, "sbx-old-l", None)).await;
    let key = delegate(&r, &id).await;
    let mut slot = r.rt.get(id.as_str()).await.unwrap().ext_of("wisp.runtime").clone();
    slot["brought"] = json!([key]);
    r.rt.set_ext(id.as_str(), "wisp.runtime", slot, None).await.unwrap();
    done_there(&r, &[("rt1", "done")]);
    report(&r, &id, &key).await;
    assert!(asked(&r, "sandbox.bring").is_empty(), "brought before");
    assert_eq!(marks(&r, &id).await, ["sb1|r1@rt1"]);
}

/// A lead in Plan changes no file: a finished sandbox job's changes are held (kept in a file, `keep`), marked, and
/// the note says so and where. (Before, they went into the Plan lead's checkout.)
#[tokio::test(flavor = "multi_thread")]
async fn a_plan_lead_s_folder_never_gets_a_delegate_s_changes() {
    let r = common::rig_with(host(Some(json!({ "files": 1, "kept": "/w/patches/h1.patch" })))).await;
    let id = Box::pin(lead(&r, "sbx-plan-l", Some(Mode::Plan))).await;
    let key = delegate(&r, &id).await;
    done_there(&r, &[("rt1", "done")]);
    let text = report(&r, &id, &key).await;
    assert!(text.contains("(Its changes are held: the lead is in Plan. Kept at /w/patches/h1.patch.)"), "{text}");
    assert_eq!(asked(&r, "sandbox.bring")[0]["keep"], json!(true), "kept, not applied");
    assert_eq!(marks(&r, &id).await, ["sb1|r1@rt1"]);
}

/// A job that failed there: its changes are held too, for a lead in Edits.
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_job_s_changes_are_held() {
    let r = common::rig_with(host(Some(json!({ "files": 1, "kept": "/w/patches/h2.patch" })))).await;
    let id = Box::pin(lead(&r, "sbx-fail-l", Some(Mode::Edits))).await;
    let key = delegate(&r, &id).await;
    done_there(&r, &[("rt1", "error")]);
    let text = report(&r, &id, &key).await;
    assert!(text.contains("(Its changes are held: the job failed. Kept at /w/patches/h2.patch.)"), "{text}");
    assert_eq!(asked(&r, "sandbox.bring")[0]["keep"], json!(true));
}

/// The member's box is gone (its task there not found): the next delegation places a new sandbox and starts its task
/// there, and the lead's child for the member is the new key. (Before, the delegation failed for good.)
#[tokio::test(flavor = "multi_thread")]
async fn a_sandbox_member_whose_box_is_gone_gets_a_new_one() {
    let r = common::rig_with(host(Some(json!({ "files": 0 })))).await;
    let id = Box::pin(lead(&r, "sbx-gone-l", None)).await;
    assert_eq!(delegate(&r, &id).await, "sb1|r1");
    {
        let mut live = r.host.live.lock().unwrap();
        live.insert(
            "machines.route runtime.followUp".to_owned(),
            json!({ "$error": { "code": "not_found", "message": "That task is gone." } }),
        );
        live.insert(
            "sandbox.place".to_owned(),
            json!({ "machine": "sb2", "peer": "p", "sandbox": "y", "cwd": "/home/u/code/x", "made": true }),
        );
        live.insert("machines.route runtime.create".to_owned(), json!({ "id": "r2", "turnId": "rt9" }));
    }
    assert_eq!(delegate(&r, &id).await, "sb2|r2");
    assert_eq!(asked(&r, "sandbox.place").len(), 2);
    assert_eq!(asked(&r, "machines.route runtime.create").len(), 2);
    let s = r.rt.get(id.as_str()).await.unwrap().ext_of("wisp.runtime").clone();
    assert_eq!(s["children"]["far"], json!("sb2|r2"));
    // one still at work there is said as it is, and nothing new is made
    r.host
        .live
        .lock()
        .unwrap()
        .insert("machines.route runtime.followUp".to_owned(), json!({ "$error": { "code": "conflict", "message": "Still working." } }));
    let task = DelegateTask { subagent: "far".into(), prompt: "more".into(), ..Default::default() };
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task] }).await.unwrap();
    assert!(out[0].error.as_deref().is_some_and(|e| e.contains("Still working")), "{out:?}");
    assert_eq!(asked(&r, "sandbox.place").len(), 2);
}
