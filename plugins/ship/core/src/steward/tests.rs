//! ADR-0132's edge cases: once per head, the limits, a failure that comes back, one on the base too, threads that
//! are new, the review's high findings only, green, and a PR that's gone.

use wisp_ship_api::{Checks, ConflictFix, PrSummary, ShipRepo, ShipTask, Steward, StewardPasses, Worktree};

use super::*;

fn pr(head: &str, fail: u32, pending: u32) -> PrSummary {
    PrSummary {
        number: 7,
        url: "https://github.com/acme/repo/pull/7".into(),
        state: "OPEN".into(),
        head: head.into(),
        base: "main".into(),
        merge_state: "BLOCKED".into(),
        checks: Checks { fail, pending, pass: 1 },
        ..PrSummary::default()
    }
}

fn task(p: PrSummary, mode: &str) -> ShipTask {
    let worktree = Worktree { branch: "wisp/x".into(), base: "main".into(), remote: Some("origin".into()), ..Worktree::default() };
    ShipTask {
        repos: vec![ShipRepo { name: "repo".into(), worktree, pr: Some(p) }],
        steward: Some(started(mode, 1, None)),
        ..ShipTask::default()
    }
}

fn failing(name: &str, log: &str) -> Failing {
    Failing { name: name.into(), url: String::new(), log: log.into(), on_base: false }
}

fn thread(id: &str, at: &str) -> OpenThread {
    OpenThread {
        id: id.into(),
        path: "src/a.rs".into(),
        line: Some(3),
        comments: vec![("ana".into(), "rename this".into())],
        last_at: at.into(),
    }
}

fn seen(failing: Vec<Failing>, threads: Vec<OpenThread>, review: Option<ReviewRead>) -> Seen {
    Seen { failing, threads, review }
}

fn pass(p: Plan) -> Pass {
    match p {
        Plan::Pass(x) => x,
        other => panic!("expected a pass, got {other:?}"),
    }
}

#[test]
fn steward_is_off_unless_its_mode_is_known() {
    let mut t = task(pr("h1", 1, 0), "fix");
    assert_eq!(gate(&t, Busy::default()), Gate::Look);
    t.steward = Some(started("maybe", 1, None));
    assert_eq!(gate(&t, Busy::default()), Gate::Skip);
    t.steward = None;
    assert_eq!(gate(&t, Busy::default()), Gate::Skip);
    assert_eq!(
        [mode_of("fix"), mode_of("review"), mode_of("fix+review"), mode_of("off"), mode_of("")],
        [Some(FIX), Some(REVIEW), Some(REVIEW), None, None]
    );
}

#[test]
fn steward_skips_pending_checks_busy_tasks_conflicts_and_a_running_pass() {
    assert_eq!(gate(&task(pr("h1", 1, 1), "fix"), Busy::default()), Gate::Skip, "a check still running: wait for all of them");
    let just_opened = PrSummary { checks: Checks::default(), waiting_checks: true, ..pr("h1", 0, 0) };
    assert_eq!(
        gate(&task(just_opened, "fix"), Busy::default()),
        Gate::Skip,
        "checks not listed yet after a push: not green, wait (PR #996)"
    );
    let t = task(pr("h1", 1, 0), "fix");
    for b in [Busy { live: true, ..Busy::default() }, Busy { ask: true, ..Busy::default() }, Busy { shipping: true, ..Busy::default() }] {
        assert_eq!(gate(&t, b), Gate::Skip);
    }
    let mut c = t.clone();
    c.conflict_fix = Some(ConflictFix { step: "fixing".into(), ..ConflictFix::default() });
    assert_eq!(gate(&c, Busy::default()), Gate::Skip, "the conflict fix goes first");
    let mut f = t;
    if let Some(s) = f.steward.as_mut() {
        s.step = "fixing".into();
    }
    assert_eq!(gate(&f, Busy::default()), Gate::Skip);
}

#[test]
fn a_closed_merged_or_draft_pr_stops_the_steward() {
    let mut p = pr("h1", 0, 0);
    p.state = "MERGED".into();
    assert!(matches!(gate(&task(p.clone(), "fix"), Busy::default()), Gate::Stop(w) if w.contains("merged")));
    p.state = "CLOSED".into();
    assert!(matches!(gate(&task(p.clone(), "fix"), Busy::default()), Gate::Stop(w) if w.contains("closed")));
    p.state = "OPEN".into();
    p.draft = true;
    assert!(matches!(gate(&task(p, "fix"), Busy::default()), Gate::Stop(w) if w.contains("draft")));
}

#[test]
fn one_follow_up_holds_every_cause_in_order_checks_threads_review() {
    let t = task(pr("h1", 1, 0), "review");
    let high = vec![HighFinding { file: Some("src/b.rs".into()), line: Some(9), text: "drops the error".into() }];
    let p = pass(plan(
        &t,
        &seen(vec![failing("test", "error: boom")], vec![thread("T1", "a")], Some(ReviewRead::Done { by: "Codex".into(), high })),
    ));
    assert_eq!(p.causes, ["checks", "threads", "review"]);
    let (c, th, r) = (p.prompt.find("## Failing checks"), p.prompt.find("## Review comments"), p.prompt.find("## High findings"));
    assert!(c < th && th < r && c.is_some(), "{}", p.prompt);
    assert!(p.prompt.contains("error: boom") && p.prompt.contains("src/a.rs:3") && p.prompt.contains("src/b.rs:9: drops the error"));
    assert!(p.prompt.contains("Don't push"));
}

#[test]
fn steward_runs_once_per_cause_and_stops_at_the_limits() {
    let mut t = task(pr("h1", 1, 0), "fix");
    let mut s = t.steward.clone().unwrap_or_default();
    s.passes = StewardPasses { checks: MAX_PER_CAUSE, ..StewardPasses::default() };
    t.steward = Some(s.clone());
    assert!(
        matches!(plan(&t, &seen(vec![failing("test", "error: x")], vec![], None)), Plan::Stop(w) if w.contains("failing checks 3 times"))
    );
    // another cause still has room: it goes alone
    let p = pass(plan(&t, &seen(vec![failing("test", "error: x")], vec![thread("T1", "a")], None)));
    assert_eq!(p.causes, ["threads"]);
    // six passes in all, conflict fixes counted
    s.passes = StewardPasses { checks: 2, threads: 1, review: 0 };
    t.steward = Some(s);
    t.conflict_fix = Some(ConflictFix { tries: 3, ..ConflictFix::default() });
    assert!(matches!(plan(&t, &seen(vec![failing("test", "error: y")], vec![], None)), Plan::Stop(w) if w.contains("6 passes")));
}

#[test]
fn steward_stops_when_the_same_failure_comes_back() {
    let mut t = task(pr("h1", 1, 0), "fix");
    let first = pass(plan(&t, &seen(vec![failing("test", "2026-11-26T10:00:00.1234567Z error: test x failed at line 41")], vec![], None)));
    if let Some(s) = t.steward.as_mut() {
        sent(s, "h1", &first);
        s.step = "watching".into();
    }
    // the new head fails the same way: other timestamps, other line numbers
    t.repos[0].pr = Some(pr("h2", 1, 0));
    let again = plan(&t, &seen(vec![failing("test", "2026-11-26T11:02:00.7654321Z error: test x failed at line 44")], vec![], None));
    assert!(matches!(&again, Plan::Stop(w) if w.contains("same way")), "{again:?}");
    // a different failure is a new pass
    let other = plan(&t, &seen(vec![failing("test", "error: type mismatch in y")], vec![], None));
    assert!(matches!(other, Plan::Pass(_)));
}

#[test]
fn steward_sends_nothing_when_the_check_fails_on_the_base_too() {
    let t = task(pr("h1", 1, 0), "fix");
    let mut f = failing("lint", "error: unused");
    f.on_base = true;
    assert!(matches!(plan(&t, &seen(vec![f], vec![], None)), Plan::Stop(w) if w.contains("fails on `main` too")));
}

#[test]
fn threads_count_only_when_new_or_answered_since_the_last_pass() {
    let mut t = task(pr("h1", 0, 0), "fix");
    let p = pass(plan(&t, &seen(vec![], vec![thread("T1", "a")], None)));
    if let Some(s) = t.steward.as_mut() {
        sent(s, "h1", &p);
        s.step = "watching".into();
    }
    // still unresolved (only a person resolves it): not sent twice, and green otherwise
    assert_eq!(plan(&t, &seen(vec![], vec![thread("T1", "a")], None)), Plan::Green { reviewer: None });
    // someone answered in it: news again
    assert_eq!(pass(plan(&t, &seen(vec![], vec![thread("T1", "b")], None))).causes, ["threads"]);
}

#[test]
fn with_review_it_waits_for_the_review_and_goes_green_with_who_read_it() {
    let t = task(pr("h1", 0, 0), "review");
    assert_eq!(plan(&t, &seen(vec![], vec![], Some(ReviewRead::Pending))), Plan::Wait(Some("Waiting for the review.".into())));
    assert_eq!(
        plan(&t, &seen(vec![], vec![], Some(ReviewRead::Done { by: "Codex · gpt-6.2".into(), high: vec![] }))),
        Plan::Green { reviewer: Some("Codex · gpt-6.2".into()) }
    );
    assert_eq!(plan(&t, &seen(vec![], vec![], Some(ReviewRead::Skipped("none".into())))), Plan::Green { reviewer: None });
}

#[test]
fn a_green_head_is_left_alone_until_a_push() {
    let mut t = task(pr("h1", 0, 0), "fix");
    if let Some(s) = t.steward.as_mut() {
        s.step = "green".into();
        s.head = "h1".into();
    }
    assert_eq!(gate(&t, Busy::default()), Gate::Skip);
    t.repos[0].pr = Some(pr("h2", 0, 0));
    assert_eq!(gate(&t, Busy::default()), Gate::Look);
}

#[test]
fn log_tail_strips_ansi_and_stamps_and_keeps_the_end() {
    let raw = (0..300).map(|i| format!("2026-11-26T10:00:00.0000000Z \u{1b}[31mline {i}\u{1b}[0m")).collect::<Vec<_>>().join("\n");
    let t = log_tail(&raw);
    assert_eq!(t.lines().count(), LOG_LINES);
    assert_eq!(t.lines().last(), Some("line 299"));
    assert!(!t.contains('\u{1b}'));
    // the prompt holds at most LOG_BYTES of logs in all
    let big = "error: x\n".repeat(5_000);
    let tk = task(pr("h1", 1, 0), "fix");
    let p = pass(plan(&tk, &seen(vec![failing("a", &big), failing("b", &big)], vec![], None)));
    assert!(p.prompt.len() < LOG_BYTES + 2_000, "{}", p.prompt.len());
}

#[test]
fn after_restart_a_pushing_pass_is_stopped() {
    let mut t = task(pr("h1", 0, 0), "fix");
    if let Some(s) = t.steward.as_mut() {
        s.step = "pushing".into();
    }
    after_restart(&mut t);
    assert_eq!(t.steward.as_ref().map(|s| s.step.as_str()), Some("stopped"));
}

#[test]
fn switching_it_back_on_after_a_stop_starts_the_counts_again_but_keeps_sent_threads() {
    let was = Steward {
        mode: "fix".into(),
        step: "stopped".into(),
        passes: StewardPasses { checks: 3, ..StewardPasses::default() },
        prints: vec!["p".into()],
        seen: vec!["T1@a".into()],
        why: Some("x".into()),
        started_at: 1,
        ..Steward::default()
    };
    let s = started("fix", 9, Some(&was));
    assert_eq!(
        (s.step.as_str(), s.passes.checks, s.prints.len(), s.seen.len(), s.why.is_none(), s.started_at),
        ("watching", 0, 0, 1, true, 9)
    );
    // fix → review while it runs: nothing starts again
    let running = Steward { step: "fixing".into(), ..s };
    let r = started("review", 20, Some(&running));
    assert_eq!((r.mode.as_str(), r.step.as_str(), r.started_at), ("review", "fixing", 9));
}

#[test]
fn threads_count_only_unresolved_not_outdated_with_a_person_in_them() {
    let j = serde_json::json!({ "data": { "repository": { "pullRequest": { "reviewThreads": { "nodes": [
        { "id": "A", "isResolved": false, "isOutdated": false, "path": "a.rs", "line": 4, "comments": { "nodes": [
            { "author": { "login": "ana", "__typename": "User" }, "body": "rename", "createdAt": "t1" },
            { "author": { "login": "ci-bot", "__typename": "Bot" }, "body": "noise", "createdAt": "t2" } ] } },
        { "id": "B", "isResolved": true, "isOutdated": false, "path": "b.rs", "line": 1, "comments": { "nodes": [ { "author": { "login": "ana", "__typename": "User" }, "body": "x", "createdAt": "t" } ] } },
        { "id": "C", "isResolved": false, "isOutdated": true, "path": "c.rs", "line": 1, "comments": { "nodes": [ { "author": { "login": "ana", "__typename": "User" }, "body": "x", "createdAt": "t" } ] } },
        { "id": "D", "isResolved": false, "isOutdated": false, "path": "d.rs", "line": null, "comments": { "nodes": [ { "author": { "login": "lint", "__typename": "Bot" }, "body": "x", "createdAt": "t" } ] } }
    ] } } } } });
    let t = threads_from(&j);
    assert_eq!(t.len(), 1);
    assert_eq!((t[0].id.as_str(), t[0].line, t[0].comments.len(), t[0].last_at.as_str()), ("A", Some(4), 1, "t2"));
    assert!(threads_from(&serde_json::Value::Null).is_empty());
}

#[test]
fn a_check_fails_on_the_base_only_when_its_latest_run_there_failed() {
    assert!(base_failed(&serde_json::json!({ "check_runs": [{ "conclusion": "failure" }] })));
    assert!(!base_failed(&serde_json::json!({ "check_runs": [{ "conclusion": "success" }] })));
    assert!(!base_failed(&serde_json::json!({ "check_runs": [] })));
    assert_eq!(encode("test (ubuntu, 22)"), "test%20%28ubuntu%2C%2022%29");
}
