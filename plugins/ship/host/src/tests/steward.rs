//! ADR-0132's Steward end to end on the fake git and gh: a red check goes back to the task with its log and the fix
//! is pushed, then green; a check that fails on the base too sends nothing; a thread is sent once; a pass with no
//! change stops; `review` sends another vendor's high findings; off clears it.

use std::sync::Arc;

use serde_json::json;
use wisp_review_api::{Finding, Pick, ReviewView, Severity, Status};

use super::{H, h};
use crate::follow::TurnEnd;
use crate::steward::ReviewSaid;

const JOB: &str = "https://github.com/acme/repo/actions/runs/11/job/22";

async fn shipped(h: &H) -> wisp_ship_api::Worktree {
    let wts = h.track("t", &[("repo", None)]);
    h.dirty(&wts[0], 1);
    h.ship.ship_all(&h.call, "t", "feat: x", "", false).await.unwrap_or_else(|e| panic!("{e}"));
    h.dirty(&wts[0], 0);
    wts[0].clone()
}

fn red(h: &H) {
    h.gh.lock().checks = Some(json!([{ "__typename": "CheckRun", "name": "test", "conclusion": "FAILURE", "detailsUrl": JOB }]));
    h.gh.lock().run_log = Some("2026-11-26T10:00:00.0000000Z error: expected 3, got 4\n".into());
}

fn steward(h: &H) -> wisp_ship_api::Steward {
    h.task("t").steward.unwrap_or_default()
}

#[tokio::test]
async fn a_red_check_goes_back_with_its_log_then_the_fix_is_pushed_then_green() {
    let h = h();
    let wt = shipped(&h).await;
    red(&h);
    h.ship.set_steward("t", "fix").unwrap_or_else(|e| panic!("{e}"));
    h.ship.refresh(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    let ups = h.follow_ups("t");
    assert_eq!(ups.len(), 1, "one pass");
    assert!(
        ups[0].contains("## Failing checks") && ups[0].contains("error: expected 3, got 4") && ups[0].contains("Don't push"),
        "{}",
        ups[0]
    );
    assert_eq!((steward(&h).step.as_str(), steward(&h).passes.checks), ("fixing", 1));
    // a second refresh while the pass runs sends nothing more
    h.ship.refresh(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(h.follow_ups("t").len(), 1);
    // the turn fixed it: wisp pushes
    h.dirty(&wt, 1);
    h.ship.finish_steward(&h.call, "t", &TurnEnd::default()).await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(steward(&h).step, "watching");
    assert!(h.task("t").shipping.is_none());
    assert!(h.git.lock().log.iter().any(|l| l.starts_with("push")), "{:?}", h.git.lock().log);
    // CI passes on the new head: green, nothing merged
    h.dirty(&wt, 0);
    h.gh.lock().checks = Some(json!([{ "__typename": "CheckRun", "name": "test", "conclusion": "SUCCESS" }]));
    h.ship.refresh(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(steward(&h).step, "green");
    assert_eq!(h.gh.calls("merge").len(), 0, "the Steward never merges");
}

#[tokio::test]
async fn a_check_that_fails_on_the_base_too_sends_nothing() {
    let h = h();
    shipped(&h).await;
    red(&h);
    h.gh.lock().base_runs = json!({ "check_runs": [{ "conclusion": "failure" }] });
    h.ship.set_steward("t", "fix").unwrap_or_else(|e| panic!("{e}"));
    h.ship.refresh(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(h.follow_ups("t").len(), 0, "nothing sent: it fails on the base too");
    let s = steward(&h);
    assert_eq!(s.step, "stopped");
    assert!(s.why.unwrap_or_default().contains("fails on `main` too"));
}

#[tokio::test]
async fn a_thread_is_sent_once_and_a_pass_with_no_change_stops() {
    let h = h();
    shipped(&h).await;
    h.gh.lock().threads = json!({ "data": { "repository": { "pullRequest": { "reviewThreads": { "nodes": [
        { "id": "T1", "isResolved": false, "isOutdated": false, "path": "src/a.rs", "line": 4, "comments": { "nodes": [
            { "author": { "login": "ana", "__typename": "User" }, "body": "This leaks the handle.", "createdAt": "t1" } ] } } ] } } } } });
    h.ship.set_steward("t", "fix").unwrap_or_else(|e| panic!("{e}"));
    h.ship.refresh(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    let ups = h.follow_ups("t");
    assert_eq!(ups.len(), 1);
    assert!(ups[0].contains("src/a.rs:4") && ups[0].contains("> This leaks the handle."), "{}", ups[0]);
    // the agent only answered: nothing to push, the Steward says so and stops
    h.ship.finish_steward(&h.call, "t", &TurnEnd::default()).await.unwrap_or_else(|e| panic!("{e}"));
    let s = steward(&h);
    assert_eq!(s.step, "stopped");
    assert!(s.why.unwrap_or_default().contains("without a change"));
}

#[tokio::test]
async fn with_review_another_vendors_high_findings_go_back_and_medium_ones_dont() {
    let h = h();
    shipped(&h).await;
    let head = h.task("t").pr().map(|p| p.head.clone()).unwrap_or_default();
    let view = |findings: Vec<Finding>| ReviewView {
        status: Status::Done,
        head: head.clone(),
        current: true,
        by: Some(Pick { agent: "codex".into(), model: "gpt-6.2".into() }),
        findings,
        ..ReviewView::default()
    };
    let finding = |severity, text: &str| Finding {
        severity,
        file: Some("src/b.rs".into()),
        line: Some(9),
        text: text.into(),
        constraint: None,
        rule: None,
    };
    let said =
        Arc::new(std::sync::Mutex::new(ReviewSaid { view: Some(view(vec![finding(Severity::Medium, "naming")])), why: String::new() }));
    let s2 = said.clone();
    h.ship.set_review_port(Arc::new(move |_c, _id, _head| {
        let s = s2.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        Box::pin(async move { s })
    }));
    h.ship.set_steward("t", "review").unwrap_or_else(|e| panic!("{e}"));
    h.ship.refresh(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(h.follow_ups("t").len(), 0, "a medium finding stays on the review card");
    assert_eq!((steward(&h).step.as_str(), steward(&h).reviewer.as_deref()), ("green", Some("codex · gpt-6.2")));
    // a new head, and this time a high finding
    h.ship.set_steward("t", "off").unwrap_or_else(|e| panic!("{e}"));
    assert!(h.task("t").steward.is_none(), "off clears it");
    said.lock().unwrap_or_else(std::sync::PoisonError::into_inner).view = Some(view(vec![finding(Severity::High, "drops the error")]));
    h.ship.set_steward("t", "review").unwrap_or_else(|e| panic!("{e}"));
    h.ship.refresh(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    let ups = h.follow_ups("t");
    assert_eq!(ups.len(), 1);
    assert!(ups[0].contains("src/b.rs:9: drops the error"), "{}", ups[0]);
}

#[tokio::test]
async fn an_unknown_mode_is_refused() {
    let h = h();
    shipped(&h).await;
    assert!(h.ship.set_steward("t", "sometimes").is_err());
}
