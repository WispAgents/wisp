//! A lead that delegated stays in Working while its delegates work, and hears each report as soon as its own run is off
//! (v1 runner.js `#awaiting`, `#delegateDone`, `#reportBack`; test/subagents.test.js): a follow-up from wisp, once,
//! never to a lead the user stopped, never for a report it already read with `subagent_results`, and still after a
//! restart. The delegates here are fake CLIs held at a gate (`gate <name> …`) until the test lets them answer.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::{Rig, open, rig, until};
use serde_json::{Value, json};
use wisp_runtime::{Emitted, Runtime, repo::Repo};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{DelegateArgs, DelegateTask, FollowUpArgs, ResultsArgs, StartArgs};
use wisp_runtime_api::model::{SessionState, Turn, TurnStatus};
use wisp_store::SystemClock;

const REPORTS_BACK: &str = "Subagents you delegated to earlier have finished.";

/// A lead on the fake harness with these members (`fake` each), its first turn held at gate `lead`.
async fn lead(r: &Rig, lead: &str, members: &[&str]) -> SessionId {
    let team: Vec<Value> = members.iter().map(|m| json!({ "name": m, "agent": "fake" })).collect();
    let mut ext = BTreeMap::new();
    ext.insert("wisp.runtime".to_owned(), json!({ "subagents": team }));
    let id =
        r.rt.create(StartArgs { prompt: format!("gate {lead} lead"), cwd: Some(common::tmp()), ext, ..StartArgs::default() })
            .await
            .unwrap()
            .id;
    until("the lead runs", || async { last(r, id.as_str()).await.is_some_and(|t| t.status == TurnStatus::Running) }).await;
    id
}

async fn last(r: &Rig, id: &str) -> Option<Turn> {
    r.rt.turns(id, None, Some(1)).await.unwrap().pop()
}

async fn turns(r: &Rig, id: &str) -> Vec<Turn> {
    r.rt.turns(id, None, None).await.unwrap()
}

/// Hands each `(member, gate)` a task held at that gate; the delegations' task ids.
async fn delegate(r: &Rig, id: &SessionId, tasks: &[(&str, &str)]) -> Vec<String> {
    let tasks = tasks.iter().map(|(m, g)| DelegateTask { subagent: (*m).into(), prompt: format!("gate {g} work") }).collect();
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks }).await.unwrap();
    assert!(out.iter().all(|d| d.error.is_none()), "{out:?}");
    for d in &out {
        until("the delegate runs", || async { last(r, d.id.as_str()).await.is_some_and(|t| t.status == TurnStatus::Running) }).await;
    }
    out.into_iter().map(|d| d.id.0).collect()
}

async fn ended(r: &Rig, id: &str) {
    until("its turn ended", || async { last(r, id).await.is_some_and(|t| t.status != TurnStatus::Running) }).await;
}

fn records(t: &Turn) -> Vec<Value> {
    t.ext.get("wisp.runtime").and_then(|x| x.get("delegations")).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn finished(r: &Rig, id: &str) -> usize {
    r.host.events.lock().unwrap().iter().filter(|e| matches!(e, Emitted::Finished(f) if f.session_id.as_str() == id)).count()
}

fn waits_on(s: &wisp_runtime_api::model::Session) -> Value {
    s.ext_of("wisp.runtime")["waitsOn"].clone()
}

/// The lead's turn ends while its delegate works: it stays in Working, unread, nobody pinged, and says who it waits on.
#[tokio::test(flavor = "multi_thread")]
async fn a_lead_whose_turn_ends_while_its_delegate_works_stays_working_unread_and_unannounced() {
    let r = rig().await;
    let id = lead(&r, "hold-l", &["kid"]).await;
    delegate(&r, &id, &[("kid", "hold-k")]).await;
    open("hold-l");
    ended(&r, id.as_str()).await;
    until("its run let go", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| !waits_on(&s).is_null()) }).await;
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert_eq!(s.state, SessionState::Running, "not finished while a delegate owes its report");
    assert!(!s.unread);
    assert_eq!(waits_on(&s), json!(["kid"]));
    assert_eq!(finished(&r, id.as_str()), 0, "nobody pinged");
    assert_eq!(turns(&r, id.as_str()).await.len(), 1);
    open("hold-k");
}

/// The turn's end writes its changes (the fake CLI edits `a.ts`) beside what the turn already kept in runtime's slot:
/// its delegation records stay (the slot was replaced, and the lead forgot whom it owed).
#[tokio::test(flavor = "multi_thread")]
async fn a_turns_end_keeps_its_delegation_records_beside_its_changes() {
    let r = rig().await;
    let id = lead(&r, "merge-l", &["kid"]).await;
    delegate(&r, &id, &[("kid", "merge-k")]).await;
    open("merge-l");
    ended(&r, id.as_str()).await;
    let t = last(&r, id.as_str()).await.unwrap();
    assert_eq!(t.ext["wisp.runtime"]["changes"][0]["file"], "a.ts", "{:?}", t.ext);
    let recs = records(&t);
    assert_eq!(recs.len(), 1, "{:?}", t.ext);
    assert_eq!(recs[0]["owed"], true);
    open("merge-k");
}

/// Two delegates (v1 runner.js `#delegateDone` → `#reportBack`): the first to end goes to the waiting lead at once, in
/// a follow-up from wisp whose turn takes the other over; that turn ends while b works, so the lead waits on b; b's
/// report comes in a second follow-up, and the lead lands then, with one notification. (Round 247 held a's report until
/// b ended too.)
#[tokio::test(flavor = "multi_thread")]
async fn each_delegate_that_ends_while_the_lead_waits_is_reported_at_once() {
    let r = rig().await;
    let id = lead(&r, "two-l", &["a", "b"]).await;
    let kids = delegate(&r, &id, &[("a", "two-a"), ("b", "two-b")]).await;
    open("two-l");
    ended(&r, id.as_str()).await;
    until("it waits on both", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| waits_on(&s) == json!(["a", "b"])) }).await;
    open("two-a");
    ended(&r, &kids[0]).await;
    until("a's report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let t = turns(&r, id.as_str()).await;
    assert_eq!(t[1].by.as_deref(), Some("wisp"));
    assert!(t[1].prompt.starts_with(REPORTS_BACK), "{}", t[1].prompt);
    assert!(t[1].prompt.contains("### a (Fake CLI · m1) — done"), "{}", t[1].prompt);
    assert!(t[1].prompt.contains("Files it changed: `a.ts`"), "{}", t[1].prompt);
    assert!(!t[1].prompt.contains("### b"), "b still works: {}", t[1].prompt);
    assert_eq!(records(&t[0]).len(), 1, "b moved on with the follow-up: {:?}", records(&t[0]));
    assert_eq!(records(&t[0])[0]["owed"], false);
    assert_eq!(records(&t[1])[0]["sessionId"], kids[1].as_str());
    until("it waits on b alone", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| waits_on(&s) == json!(["b"])) }).await;
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert_eq!(s.state, SessionState::Running);
    assert_eq!(finished(&r, id.as_str()), 0, "nobody pinged while b works");
    open("two-b");
    until("b's report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 3 && t[2].status == TurnStatus::Done
    })
    .await;
    let t = turns(&r, id.as_str()).await;
    assert!(t[2].prompt.contains("### b (Fake CLI · m1) — done"), "{}", t[2].prompt);
    assert!(!t[2].prompt.contains("### a"), "a's report went once: {}", t[2].prompt);
    assert!(t.iter().flat_map(records).all(|d| d["owed"] == false), "nothing left owed");
    until("it lands", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| s.state == SessionState::Review) }).await;
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert!(s.unread);
    assert!(waits_on(&s).is_null());
    assert_eq!(finished(&r, id.as_str()), 1, "one notification, after the last report-back turn");
}

/// Two delegates end while the lead's turn still runs and a third works on (v1 `#reportBack` with `ending`): the lead's
/// end sends the two in one follow-up at once, without waiting on the third, whose report comes after.
#[tokio::test(flavor = "multi_thread")]
async fn delegates_that_end_while_the_lead_runs_come_back_in_one_follow_up_and_the_rest_after() {
    let r = rig().await;
    let id = lead(&r, "batch-l", &["a", "b", "c"]).await;
    let kids = delegate(&r, &id, &[("a", "batch-a"), ("b", "batch-b"), ("c", "batch-c")]).await;
    open("batch-a");
    open("batch-b");
    ended(&r, &kids[0]).await;
    ended(&r, &kids[1]).await;
    open("batch-l");
    until("the report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let t = turns(&r, id.as_str()).await;
    let p = &t[1].prompt;
    assert!(p.starts_with(REPORTS_BACK), "{p}");
    assert!(p.contains("### a (Fake CLI · m1) — done") && p.contains("### b (Fake CLI · m1) — done"), "{p}");
    assert!(!p.contains("### c"), "c still works: {p}");
    assert_eq!(p.matches(REPORTS_BACK).count(), 1, "one follow-up: {p}");
    until("it waits on c", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| waits_on(&s) == json!(["c"])) }).await;
    open("batch-c");
    until("c's report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 3 && t[2].status == TurnStatus::Done
    })
    .await;
    assert!(turns(&r, id.as_str()).await[2].prompt.contains("### c (Fake CLI · m1) — done"));
    until("it lands", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| s.state == SessionState::Review) }).await;
    assert_eq!(finished(&r, id.as_str()), 1);
}

/// A delegate that finished while the lead still ran, its report never read: the lead's end queues it at once.
#[tokio::test(flavor = "multi_thread")]
async fn a_delegate_that_ended_before_its_lead_unread_comes_back_when_the_lead_ends() {
    let r = rig().await;
    let id = lead(&r, "early-l", &["kid"]).await;
    let kids = delegate(&r, &id, &[("kid", "early-k")]).await;
    open("early-k");
    ended(&r, &kids[0]).await;
    open("early-l");
    until("the report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let t = turns(&r, id.as_str()).await;
    assert!(t[1].prompt.starts_with(REPORTS_BACK), "{}", t[1].prompt);
    assert_eq!(finished(&r, id.as_str()), 1, "no flash through Needs you before it");
}

/// What `subagent_results` handed the lead it has: its end sends nothing more, and it lands.
#[tokio::test(flavor = "multi_thread")]
async fn reports_read_with_subagent_results_are_not_sent_again() {
    let r = rig().await;
    let id = lead(&r, "read-l", &["kid"]).await;
    let kids = delegate(&r, &id, &[("kid", "read-k")]).await;
    open("read-k");
    let rep = r.rt.results(ResultsArgs { id: id.clone(), ids: Vec::new(), wait: Some(5) }).await.unwrap();
    assert!(rep.reports[0].done, "{rep:?}");
    assert_eq!(rep.reports[0].id.0, kids[0]);
    let recs = records(&last(&r, id.as_str()).await.unwrap());
    assert_eq!(recs[0]["owed"], false, "the lead has it now: {recs:?}");
    open("read-l");
    until("it lands", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| s.state == SessionState::Review) }).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(turns(&r, id.as_str()).await.len(), 1, "no report-back for what it read");
    assert_eq!(finished(&r, id.as_str()), 1);
}

/// Stop on a lead waiting on its delegate: it lands, its delegate stops with it, and nothing comes back.
#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_lead_gets_no_report_back_and_its_delegates_stop() {
    let r = rig().await;
    let id = lead(&r, "stop-l", &["kid"]).await;
    let kids = delegate(&r, &id, &[("kid", "stop-k")]).await;
    open("stop-l");
    until("it waits", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| !waits_on(&s).is_null()) }).await;
    r.rt.stop(id.as_str(), false).await.unwrap();
    ended(&r, &kids[0]).await;
    assert_eq!(last(&r, &kids[0]).await.unwrap().status, TurnStatus::Stopped, "the delegate stopped with it");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert_eq!(s.state, SessionState::Review);
    assert!(waits_on(&s).is_null());
    assert_eq!(turns(&r, id.as_str()).await.len(), 1, "nothing sent back to a stopped lead");
}

/// A follow-up to a lead waiting on its delegate starts at once: the new turn takes the delegation over (the old one
/// no longer owes it), and the lead hears it's still at work; its end waits on it, and the report comes after.
#[tokio::test(flavor = "multi_thread")]
async fn a_follow_up_to_a_waiting_lead_takes_its_delegates_over_and_hears_who_still_works() {
    let r = rig().await;
    let id = lead(&r, "over-l", &["kid"]).await;
    let kids = delegate(&r, &id, &[("kid", "over-k")]).await;
    open("over-l");
    until("it waits", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| !waits_on(&s).is_null()) }).await;
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "gate over-l2 meanwhile".into(), ..FollowUpArgs::default() }).await.unwrap();
    until("the follow-up runs", || async { turns(&r, id.as_str()).await.len() == 2 }).await;
    let t = turns(&r, id.as_str()).await;
    assert!(records(&t[0]).is_empty(), "moved off the old turn: {:?}", records(&t[0]));
    assert_eq!(records(&t[1])[0]["sessionId"], kids[0].as_str());
    let told = r.sim.spawned().into_iter().find(|s| s.args.first().is_some_and(|a| a.contains("gate over-l2"))).unwrap();
    assert!(
        told.args[0].starts_with(&format!(
            "Subagents you delegated to before this message are still on it, or done and not yet read: kid (id {})",
            kids[0]
        )),
        "{}",
        told.args[0]
    );
    open("over-l2");
    ended(&r, id.as_str()).await;
    until("it waits again", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| !waits_on(&s).is_null()) }).await;
    open("over-k");
    until("the report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 3 && t[2].status == TurnStatus::Done
    })
    .await;
    assert!(turns(&r, id.as_str()).await[2].prompt.starts_with(REPORTS_BACK));
}

/// wisp restarts while a lead waits on its delegate: the restart stops the delegate, and the lead hears it (once),
/// from what the store kept; another restart sends nothing more.
#[tokio::test(flavor = "multi_thread")]
async fn a_lead_waiting_on_its_delegate_hears_once_after_a_restart() {
    let r = rig().await;
    let id = lead(&r, "boot-l", &["kid"]).await;
    delegate(&r, &id, &[("kid", "boot-k")]).await;
    open("boot-l");
    until("it waits", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| !waits_on(&s).is_null()) }).await;
    // a second wisp on the same store (the first's runs are as good as gone: their gate never opens)
    let again = || async {
        let rt = Runtime::new(Repo::new(r.store.clone()), r.host.clone(), Arc::new(r.sim.clone()), Arc::new(SystemClock));
        rt.start().await.unwrap();
        rt
    };
    let rt2 = again().await;
    until("the report-back turn ran", || async {
        let t = rt2.turns(id.as_str(), None, None).await.unwrap();
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let t = rt2.turns(id.as_str(), None, None).await.unwrap();
    assert!(t[1].prompt.starts_with(REPORTS_BACK), "{}", t[1].prompt);
    assert!(t[1].prompt.contains("### kid (Fake CLI · m1) — stopped"), "{}", t[1].prompt);
    until("it lands", || async { rt2.get(id.as_str()).await.is_ok_and(|s| s.state == SessionState::Review) }).await;
    let rt3 = again().await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(rt3.turns(id.as_str(), None, None).await.unwrap().len(), 2, "heard once");
}

/// A follow-up to a waiting lead that never ran (its folder went, so it was refused) comes after the turn that owes:
/// the delegate's end still finds what the lead is owed there and sends it (looked at only on the last turn, the
/// report was lost).
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_follow_up_between_doesnt_lose_what_the_lead_is_owed() {
    let r = rig().await;
    let dir = std::env::temp_dir().join(format!("wisp-rb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ext = BTreeMap::new();
    ext.insert("wisp.runtime".to_owned(), json!({ "subagents": [{ "name": "kid", "agent": "fake" }] }));
    let start = StartArgs { prompt: "gate gone-l lead".into(), cwd: Some(dir.display().to_string()), ext, ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    until("the lead runs", || async { last(&r, id.as_str()).await.is_some_and(|t| t.status == TurnStatus::Running) }).await;
    let kids = delegate(&r, &id, &[("kid", "gone-k")]).await;
    open("gone-l");
    until("it waits", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| !waits_on(&s).is_null()) }).await;
    std::fs::remove_dir_all(&dir).unwrap();
    let _ = r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "and this".into(), ..FollowUpArgs::default() }).await;
    until("the follow-up was refused", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 2 && t[1].status != TurnStatus::Running
    })
    .await;
    open("gone-k");
    ended(&r, &kids[0]).await;
    until("the report went to it", || async {
        turns(&r, id.as_str()).await.iter().any(|t| t.by.as_deref() == Some("wisp") && t.prompt.starts_with(REPORTS_BACK))
    })
    .await;
    assert!(records(&turns(&r, id.as_str()).await[0]).iter().all(|d| d["owed"] == false));
}

/// A crew job here (F-crew-11: `crew.assign` made it, `ext["wisp.crew"].from` = the lead) is waited on as a child is
/// (v1 `#awaiting` counts crew relays): the lead's end holds it in Working on the member, and the job's end sends its
/// report back. (Before: it wasn't owed past the lead's turn, and the lead landed at once.)
#[tokio::test(flavor = "multi_thread")]
async fn a_crew_job_here_holds_its_lead_and_its_report_comes_back_when_it_ends() {
    let r = rig().await;
    let mut ext = BTreeMap::new();
    ext.insert("wisp.runtime".to_owned(), json!({ "subagents": [{ "name": "ada", "label": "Ada", "crew": "crew-ada" }] }));
    let start = StartArgs { prompt: "gate crewh-l lead".into(), cwd: Some(common::tmp()), ext, ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    until("the lead runs", || async { last(&r, id.as_str()).await.is_some_and(|t| t.status == TurnStatus::Running) }).await;
    // the job crew.assign makes: a task of its own here, the lead as `from`
    let mut job_ext = BTreeMap::new();
    job_ext.insert("wisp.crew".to_owned(), json!({ "id": "crew-ada", "from": { "id": id.as_str(), "title": "lead" } }));
    let job_start = StartArgs { prompt: "gate crewh-k work".into(), cwd: Some(common::tmp()), ext: job_ext, ..StartArgs::default() };
    let job = r.rt.create(job_start).await.unwrap().id;
    until("the job runs", || async { last(&r, job.as_str()).await.is_some_and(|t| t.status == TurnStatus::Running) }).await;
    r.host.live.lock().unwrap().insert("crew.assign".into(), json!({ "session": { "id": job.as_str(), "agent": "fake", "model": "m1" } }));
    let task = DelegateTask { subagent: "ada".into(), prompt: "fix it".into() };
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task] }).await.unwrap();
    assert_eq!(out[0].id, job, "{out:?}");
    open("crewh-l");
    ended(&r, id.as_str()).await;
    until("it waits on the member", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| waits_on(&s) == json!(["ada"])) }).await;
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert_eq!(s.state, SessionState::Running, "held on its crew job");
    assert_eq!(finished(&r, id.as_str()), 0);
    open("crewh-k");
    until("the report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let t = turns(&r, id.as_str()).await;
    assert!(t[1].prompt.starts_with(REPORTS_BACK), "{}", t[1].prompt);
    assert!(t[1].prompt.contains("### ada (") && t[1].prompt.contains("— done"), "{}", t[1].prompt);
    assert_eq!(records(&t[0])[0]["owed"], false);
    until("it lands", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| s.state == SessionState::Review) }).await;
    assert_eq!(finished(&r, id.as_str()), 1);
}

/// What a machine answers for the job `r1` (turn `rt1`) there: still at work, or done with `fixed it`.
fn far_answers(state: &str) -> Vec<(&'static str, Value)> {
    let status = if state == "running" { "running" } else { "done" };
    vec![
        ("machines.route runtime.get", json!({ "id": "r1", "state": state, "cwd": "/there/copies/k/repo", "title": "far · lead" })),
        ("machines.route runtime.turns", json!([{ "id": "rt1", "status": status, "text": "fixed it" }])),
    ]
}

fn set_live(host: &wisp_runtime::Recorder, answers: Vec<(&'static str, Value)>) {
    let mut live = host.live.lock().unwrap();
    for (k, v) in answers {
        live.insert(k.to_owned(), v);
    }
}

/// A host that answers for a member on machine `m1` (named studio) and a sandbox member: the copy, the task there,
/// the bring.
fn far_host(scope: Option<wisp_proc::Scope>) -> wisp_runtime::Recorder {
    let mut answers = BTreeMap::new();
    answers.insert("machines.carry".to_owned(), json!({ "cwd": "/there/copies/k/repo", "machine": "studio" }));
    answers.insert("machines.route runtime.create".to_owned(), json!({ "id": "r1", "turnId": "rt1" }));
    answers.insert("machines.route runtime.stop".to_owned(), json!({}));
    answers.insert("machines.bring".to_owned(), json!({ "files": 2 }));
    answers.insert(
        "sandbox.place".to_owned(),
        json!({ "machine": "sb1", "peer": "p", "sandbox": "x", "cwd": "/home/u/code/x", "made": true }),
    );
    answers.insert("sandbox.bring".to_owned(), json!({ "files": 1 }));
    let host = wisp_runtime::Recorder { answers, scope, ..wisp_runtime::Recorder::default() };
    set_live(&host, far_answers("running"));
    host
}

/// A lead whose one member works `machine` (`m1`: another machine; `sandbox`), its turn held at gate `gate`, and its
/// task handed there: the lead's id.
async fn far_lead(r: &Rig, gate: &str, machine: &str) -> SessionId {
    let mut ext = BTreeMap::new();
    ext.insert("wisp.runtime".to_owned(), json!({ "subagents": [{ "name": "far", "agent": "codex", "machine": machine }] }));
    let start = StartArgs { prompt: format!("gate {gate} lead"), cwd: Some(common::tmp()), ext, ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    until("the lead runs", || async { last(r, id.as_str()).await.is_some_and(|t| t.status == TurnStatus::Running) }).await;
    let task = DelegateTask { subagent: "far".into(), prompt: "fix it".into() };
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task] }).await.unwrap();
    assert!(out[0].error.is_none() && out[0].id.0.ends_with("|r1"), "{out:?}");
    assert_eq!(out[0].turn_id.0, "rt1", "the turn it started there");
    id
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

/// A delegate on another machine (ADR-0059) holds its lead in Working as one here does (v1 `#awaiting` counts
/// `child.relay`); wisp watches it there (`machines.route`), and when it ends its changes come back once, before its
/// report, which says so and where it ran; `subagent_results` after that brings nothing again. (Before: the lead's end
/// let it go, and it was read only with `subagent_results`.)
#[tokio::test(flavor = "multi_thread")]
async fn a_delegate_on_another_machine_holds_its_lead_and_comes_back_once_with_its_changes() {
    let r = common::rig_with(far_host(None)).await;
    let id = far_lead(&r, "far-l", "m1").await;
    open("far-l");
    ended(&r, id.as_str()).await;
    until("it waits on far", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| waits_on(&s) == json!(["far"])) }).await;
    assert_eq!(r.rt.get(id.as_str()).await.unwrap().state, SessionState::Running);
    assert_eq!(asked(&r, "machines.bring").len(), 0, "nothing comes back while it works");
    set_live(&r.host, far_answers("review"));
    until("the report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let p = turns(&r, id.as_str()).await[1].prompt.clone();
    assert!(p.starts_with(REPORTS_BACK), "{p}");
    assert!(p.contains("### far (") && p.contains("— done"), "{p}");
    assert!(p.contains("fixed it"), "{p}");
    assert!(p.contains("(Its changes to 2 file(s) are in your folder now.)"), "{p}");
    assert!(p.contains("(It ran on studio, on a copy of your folder.)"), "{p}");
    until("it lands", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| s.state == SessionState::Review) }).await;
    let rep = r.rt.results(ResultsArgs { id: id.clone(), ids: vec![SessionId("m1|r1".into())], wait: Some(0) }).await.unwrap();
    assert!(rep.reports[0].done, "{rep:?}");
    assert_eq!(asked(&r, "machines.bring").len(), 1, "its changes came back once");
    assert_eq!(finished(&r, id.as_str()), 1);
}

/// A sandbox delegate (ADR-0047) is waited on the same way; its changes come back once through `sandbox.bring`.
#[tokio::test(flavor = "multi_thread")]
async fn a_sandbox_delegate_holds_its_lead_and_its_changes_come_back_once() {
    let r = common::rig_with(far_host(None)).await;
    let id = far_lead(&r, "box-l", "sandbox").await;
    open("box-l");
    until("it waits on far", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| waits_on(&s) == json!(["far"])) }).await;
    set_live(&r.host, far_answers("review"));
    until("the report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let p = turns(&r, id.as_str()).await[1].prompt.clone();
    assert!(p.contains("fixed it") && p.contains("(Its changes to 1 file(s) are in your folder now.)"), "{p}");
    until("it lands", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| s.state == SessionState::Review) }).await;
    r.rt.results(ResultsArgs { id: id.clone(), ids: vec![SessionId("sb1|r1".into())], wait: Some(0) }).await.unwrap();
    assert_eq!(asked(&r, "sandbox.bring").len(), 1, "once");
}

/// Stop on a lead waiting on a delegate elsewhere reaches it there (`runtime.stop` routed, v1: the relay stops it), and
/// nothing comes back to the stopped lead when it ends there.
#[tokio::test(flavor = "multi_thread")]
async fn stopping_a_lead_stops_its_delegate_elsewhere_there() {
    let r = common::rig_with(far_host(None)).await;
    let id = far_lead(&r, "farstop-l", "m1").await;
    open("farstop-l");
    until("it waits on far", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| !waits_on(&s).is_null()) }).await;
    r.rt.stop(id.as_str(), false).await.unwrap();
    until("the stop reached it there", || async { !asked(&r, "machines.route runtime.stop").is_empty() }).await;
    let stop = &asked(&r, "machines.route runtime.stop")[0];
    assert_eq!((stop["id"].as_str(), stop["args"]["id"].as_str()), (Some("m1"), Some("r1")), "{stop}");
    set_live(&r.host, far_answers("review"));
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert_eq!(s.state, SessionState::Review);
    assert_eq!(turns(&r, id.as_str()).await.len(), 1, "nothing sent back to a stopped lead");
    assert_eq!(asked(&r, "machines.bring").len(), 0, "nothing brought for a stopped lead");
}

/// wisp restarts while a lead waits on a delegate on another machine: the new wisp watches it again (from the store's
/// owed record), and its report comes back once it ends there.
#[tokio::test(flavor = "multi_thread")]
async fn a_lead_waiting_on_a_delegate_elsewhere_watches_it_again_after_a_restart() {
    let scope = wisp_proc::Scope::new();
    let r = common::rig_with(far_host(Some(scope.clone()))).await;
    let id = far_lead(&r, "farboot-l", "m1").await;
    open("farboot-l");
    until("it waits on far", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| !waits_on(&s).is_null()) }).await;
    // the first wisp goes (its scope, and the watch on it, with it); a second one on the same store
    scope.cancel();
    let host = Arc::new(wisp_runtime::Recorder { harnesses: r.host.harnesses.clone(), ..far_host(None) });
    let rt2 = Runtime::new(Repo::new(r.store.clone()), host.clone(), Arc::new(r.sim.clone()), Arc::new(SystemClock));
    rt2.start().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(rt2.turns(id.as_str(), None, None).await.unwrap().len(), 1, "still at work there: nothing yet");
    set_live(&host, far_answers("review"));
    until("the report-back turn ran", || async {
        let t = rt2.turns(id.as_str(), None, None).await.unwrap();
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let t = rt2.turns(id.as_str(), None, None).await.unwrap();
    assert!(t[1].prompt.contains("fixed it") && t[1].prompt.contains("(It ran on studio"), "{}", t[1].prompt);
    assert_eq!(host.asked.lock().unwrap().iter().filter(|(c, _)| c == "machines.bring").count(), 1);
}

/// The delegation's own turn there counts, not its task's last: after `rt1` ended, a newer turn runs on the task there
/// (wisp.later resumed it, the user typed into it): the lead still hears `rt1`'s report. (Read off the last turn, the
/// lead stayed in Working until Stop.)
#[tokio::test(flavor = "multi_thread")]
async fn a_later_turn_on_the_task_there_doesnt_hold_the_lead_on_its_job() {
    let r = common::rig_with(far_host(None)).await;
    let id = far_lead(&r, "later-l", "m1").await;
    open("later-l");
    until("it waits on far", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| waits_on(&s) == json!(["far"])) }).await;
    set_live(
        &r.host,
        vec![
            ("machines.route runtime.get", json!({ "id": "r1", "state": "running", "cwd": "/there/copies/k/repo", "title": "far · lead" })),
            (
                "machines.route runtime.turns",
                json!([{ "id": "rt1", "status": "done", "text": "fixed it" }, { "id": "rt2", "status": "running", "text": "" }]),
            ),
        ],
    );
    until("the report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let p = turns(&r, id.as_str()).await[1].prompt.clone();
    assert!(p.contains("### far (") && p.contains("— done") && p.contains("fixed it"), "{p}");
    assert_eq!(asked(&r, "machines.bring").len(), 1);
}
