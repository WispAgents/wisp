//! A lead that delegated stays in Working while its delegates work, and hears their reports when the last one ends
//! (v1 runner.js `#awaiting`, `#delegateDone`, `#reportBack`; test/subagents.test.js): one follow-up from wisp, once,
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

/// Two delegates: the first to end changes nothing but who it waits on; the last sends one follow-up from wisp with
/// both reports, which runs, and the lead lands then, with one notification.
#[tokio::test(flavor = "multi_thread")]
async fn the_last_delegate_to_end_sends_the_lead_one_report_back_turn() {
    let r = rig().await;
    let id = lead(&r, "two-l", &["a", "b"]).await;
    let kids = delegate(&r, &id, &[("a", "two-a"), ("b", "two-b")]).await;
    open("two-l");
    ended(&r, id.as_str()).await;
    until("it waits on both", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| waits_on(&s) == json!(["a", "b"])) }).await;
    open("two-a");
    ended(&r, &kids[0]).await;
    until("it waits on b alone", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| waits_on(&s) == json!(["b"])) }).await;
    assert_eq!(turns(&r, id.as_str()).await.len(), 1, "no report while b works");
    open("two-b");
    until("the report-back turn ran", || async {
        let t = turns(&r, id.as_str()).await;
        t.len() == 2 && t[1].status == TurnStatus::Done
    })
    .await;
    let t = turns(&r, id.as_str()).await;
    let back = &t[1];
    assert_eq!(back.by.as_deref(), Some("wisp"));
    assert!(back.prompt.starts_with(REPORTS_BACK), "{}", back.prompt);
    assert!(back.prompt.contains("### a (Fake CLI · m1) — done"), "{}", back.prompt);
    assert!(back.prompt.contains("### b (Fake CLI · m1) — done"), "{}", back.prompt);
    assert!(back.prompt.contains("Files it changed: `a.ts`"), "{}", back.prompt);
    assert!(records(&t[0]).iter().all(|d| d["owed"] == false), "nothing left owed: {:?}", records(&t[0]));
    until("it lands", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| s.state == SessionState::Review) }).await;
    let s = r.rt.get(id.as_str()).await.unwrap();
    assert!(s.unread);
    assert!(waits_on(&s).is_null());
    assert_eq!(finished(&r, id.as_str()), 1, "one notification, after the report-back turn");
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

/// A delegate on another machine isn't waited on (it's read with `subagent_results`): the lead's end lands it, and the
/// record stops being owed, so its next turns aren't told it's still at work.
#[tokio::test(flavor = "multi_thread")]
async fn a_delegate_elsewhere_isnt_waited_on_nor_carried_into_the_leads_next_turn() {
    let mut answers = BTreeMap::new();
    answers.insert("machines.carry".to_owned(), json!({ "cwd": "/there/copies/k/repo", "machine": "studio" }));
    answers.insert("machines.route".to_owned(), json!({ "id": "r1", "state": "running", "cwd": "/there/copies/k/repo", "title": "far" }));
    let r = common::rig_with(wisp_runtime::Recorder { answers, ..wisp_runtime::Recorder::default() }).await;
    let mut ext = BTreeMap::new();
    ext.insert(
        "wisp.runtime".to_owned(),
        json!({ "subagents": [{ "name": "far", "agent": "codex", "machine": "m1", "place": "studio" }] }),
    );
    let start = StartArgs { prompt: "gate away-l lead".into(), cwd: Some(common::tmp()), ext, ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    until("the lead runs", || async { last(&r, id.as_str()).await.is_some_and(|t| t.status == TurnStatus::Running) }).await;
    let task = DelegateTask { subagent: "far".into(), prompt: "fix it".into() };
    let out = r.rt.delegate(DelegateArgs { id: id.clone(), tasks: vec![task] }).await.unwrap();
    assert_eq!(out[0].id.0, "m1|r1", "{out:?}");
    open("away-l");
    until("it lands", || async { r.rt.get(id.as_str()).await.is_ok_and(|s| s.state == SessionState::Review) }).await;
    assert_eq!(records(&last(&r, id.as_str()).await.unwrap())[0]["owed"], false);
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "gate away-l2 next".into(), ..FollowUpArgs::default() }).await.unwrap();
    until("the follow-up runs", || async { turns(&r, id.as_str()).await.len() == 2 }).await;
    let told = r.sim.spawned().into_iter().find(|s| s.args.first().is_some_and(|a| a.contains("gate away-l2"))).unwrap();
    assert!(told.args[0].starts_with("gate away-l2"), "no still-working note: {}", told.args[0]);
    open("away-l2");
}
