//! v1 test/ship.test.js: "a conflicting PR is left alone by default; with Auto-fix on …", "a conflict fix
//! that leaves conflicts is not pushed; Fix conflicts retries; the setting turns it off", and the
//! unread rules of "ship it: … checks are followed".

use wisp_ship_api::{Checks, ConflictFix, PrSummary, ShipRepo, ShipTask, Worktree};

use super::*;

fn task(pr: PrSummary) -> ShipTask {
    let worktree = Worktree { branch: "wisp/x".into(), base: "main".into(), remote: Some("origin".into()), ..Worktree::default() };
    ShipTask { repos: vec![ShipRepo { name: "repo".into(), worktree, pr: Some(pr) }], ..ShipTask::default() }
}

fn open(head: &str, conflict: bool) -> PrSummary {
    PrSummary { number: 1, state: "OPEN".into(), head: head.into(), conflict, ..PrSummary::default() }
}

#[test]
fn a_conflicting_pr_is_left_alone_by_default() {
    let t = task(open("h1", true));
    assert_eq!(should_fix(&t, false, false, Busy::default()), Fix::Skip);
    assert_eq!(should_fix(&t, true, false, Busy::default()), Fix::CheckBase, "auto: git is asked first");
    assert_eq!(should_fix(&t, false, true, Busy::default()), Fix::CheckBase, "the user clicked: git is asked first too (audit A #21)");
}

#[test]
fn auto_fix_runs_once_per_head_and_at_most_three_times() {
    let mut t = task(open("h1", true));
    t.conflict_fix = Some(fix_started(&t, 5));
    assert_eq!(t.conflict_fix.as_ref().map(|f| (f.tries, f.head.as_str(), f.step.as_str())), Some((1, "h1", "fixing")));
    // still CONFLICTING on the stale GitHub side, same head: no second follow-up
    assert_eq!(should_fix(&t, true, false, Busy::default()), Fix::Skip);
    // ...but Fix conflicts retries
    assert_eq!(should_fix(&t, true, true, Busy::default()), Fix::CheckBase);
    // a new head may try again, up to three
    t.repos[0].pr = Some(open("h2", true));
    assert_eq!(should_fix(&t, true, false, Busy::default()), Fix::CheckBase);
    t.conflict_fix = Some(ConflictFix { tries: MAX_CONFLICT_FIXES, head: "h0".into(), ..ConflictFix::default() });
    assert_eq!(should_fix(&t, true, false, Busy::default()), Fix::Skip);
}

#[test]
fn never_while_busy_or_without_a_conflict() {
    let t = task(open("h", true));
    for busy in [
        Busy { live: true, ..Busy::default() },
        Busy { ask: true, ..Busy::default() },
        Busy { shipping: true, ..Busy::default() },
        Busy { archived: true, ..Busy::default() },
        Busy { starting: true, ..Busy::default() },
    ] {
        assert_eq!(should_fix(&t, true, true, busy), Fix::Skip);
    }
    assert_eq!(should_fix(&task(open("h", false)), true, true, Busy::default()), Fix::Skip);
    assert_eq!(should_fix(&task(PrSummary { state: "MERGED".into(), ..open("h", true) }), true, true, Busy::default()), Fix::Skip);
    let mut no_remote = task(open("h", true));
    no_remote.repos[0].worktree.remote = None;
    assert_eq!(should_fix(&no_remote, true, true, Busy::default()), Fix::Skip);
    assert_eq!(why_not_fixing(&no_remote, Busy::default()), "This repo has no origin remote to push to.");
    assert_eq!(why_not_fixing(&task(open("h", false)), Busy::default()), "The PR has no merge conflicts now.");
    assert_eq!(why_not_fixing(&t, Busy { ask: true, ..Busy::default() }), "The task is waiting for your answer first.");
    assert_eq!(why_not_fixing(&t, Busy { shipping: true, ..Busy::default() }), "Already shipping: wait for it to land.");
}

#[test]
fn the_fix_turn_ending() {
    assert_eq!(fix_ended(true, false, None).as_deref(), Some("Stopped: the task moved on before the conflicts were fixed."));
    assert_eq!(fix_ended(false, true, None).as_deref(), Some("Stopped: the task moved on before the conflicts were fixed."));
    assert_eq!(fix_ended(false, false, Some("boom")).as_deref(), Some("The agent run failed: boom"));
    assert_eq!(fix_ended(false, false, Some(&"x".repeat(999))).map(|s| s.chars().count()), Some(300));
    assert_eq!(fix_ended(false, false, None), None);
}

#[test]
fn a_clean_pr_clears_a_finished_fix() {
    let mut t = task(open("h", false));
    t.conflict_fix = Some(ConflictFix { step: "done".into(), ..ConflictFix::default() });
    clear_fix(&mut t);
    assert_eq!(t.conflict_fix, None);
    t.conflict_fix = Some(ConflictFix { step: "fixing".into(), ..ConflictFix::default() });
    clear_fix(&mut t);
    assert!(t.conflict_fix.is_some(), "a fix under way stays");
    let mut c = task(open("h", true));
    c.conflict_fix = Some(ConflictFix { step: "failed".into(), ..ConflictFix::default() });
    clear_fix(&mut c);
    assert!(c.conflict_fix.is_some(), "still conflicting: the failure stays");
}

#[test]
fn a_restart_ends_ships_and_fails_pushes() {
    let mut t = task(open("h", true));
    t.shipping = Some(wisp_ship_api::Shipping { step: "push".into(), ..wisp_ship_api::Shipping::default() });
    t.conflict_fix = Some(ConflictFix { step: "pushing".into(), ..ConflictFix::default() });
    after_restart(&mut t);
    assert_eq!(t.shipping, None);
    assert_eq!(t.conflict_fix.as_ref().and_then(|f| f.why.as_deref()), Some("wisp restarted while pushing the fix."));
}

#[test]
fn news_is_what_is_worth_coming_back_for() {
    let pending = PrSummary { checks: Checks { pass: 0, fail: 0, pending: 1 }, ..open("h", false) };
    let green = PrSummary { checks: Checks { pass: 2, fail: 0, pending: 0 }, ..open("h", false) };
    let red = PrSummary { checks: Checks { pass: 1, fail: 1, pending: 0 }, ..open("h", false) };
    assert!(is_news(&pending, &green), "CI finished");
    assert!(is_news(&green, &red), "went red");
    assert!(!is_news(&green, &pending), "running again is not news");
    assert!(!is_news(&green, &green));
    assert!(is_news(&green, &PrSummary { review: "APPROVED".into(), ..green.clone() }));
    assert!(is_news(&green, &PrSummary { state: "MERGED".into(), ..green.clone() }));
    assert!(is_also_news(&pending, &green));
    assert!(!is_also_news(&green, &red), "another repo's PR: only checks finishing");
}

#[test]
fn polls_fast_while_settling() {
    let mut t = task(PrSummary { checked_at: 0, merge_state: "CLEAN".into(), ..open("h", false) });
    t.pushed_at = -10 * 60_000;
    assert!(!due(&t, 30_000));
    assert!(due(&t, 60_000));
    t.pushed_at = 0;
    assert!(due(&t, 10_000), "a push just landed: every 10 s");
    t.pushed_at = -10 * 60_000;
    t.repos[0].pr = Some(PrSummary { merge_state: "UNKNOWN".into(), ..open("h", false) });
    assert!(due(&t, 10_000));
    t.repos[0].pr = Some(PrSummary { state: "MERGED".into(), ..open("h", false) });
    assert!(!due(&t, 1_000_000), "nothing open: nothing to poll");
    t.prs.push(wisp_ship_api::AttachedPr { repo: "docs".into(), pr: open("x", false) });
    assert_eq!(lead(&t).map(|p| p.head.as_str()), Some("x"), "an attached PR polls on its own");
}

/// Edge case Git 9: merge-tree's exit 1 is a conflict, other exits are "can't tell" and trust GitHub.
#[test]
fn only_a_clean_merge_tree_overrules_github() {
    assert!(conflict_free(Some(false)));
    assert!(!conflict_free(Some(true)));
    assert!(!conflict_free(None));
}

#[test]
fn a_pr_read_the_moment_it_opens_waits_for_its_checks_instead_of_passing() {
    // PR #996: read 3 s after the push, GitHub listed no checks yet; the card went green and the Steward said green
    let mut p = open("h1", false);
    mark_waiting(&mut p, 1_000, 4_000);
    assert!(p.waiting_checks, "no checks yet, just pushed: waiting");
    assert_eq!(settled(&p), "pending");
    let mut listed = PrSummary { checks: Checks { pass: 1, fail: 0, pending: 5 }, ..p.clone() };
    mark_waiting(&mut listed, 1_000, 15_000);
    assert!(!listed.waiting_checks, "checks listed: they speak for themselves");
    assert!(!is_news(&p, &listed), "waiting → running is not CI finishing");
    let mut no_ci = open("h1", false);
    mark_waiting(&mut no_ci, 1_000, 1_000 + CHECKS_GRACE_MS);
    assert!(!no_ci.waiting_checks, "past the grace, no checks means a repo without CI");
    let mut never_pushed = open("h1", false);
    mark_waiting(&mut never_pushed, 0, 5_000);
    assert!(!never_pushed.waiting_checks, "an attached PR wisp never pushed");
    let mut merged = PrSummary { state: "MERGED".into(), ..open("h1", false) };
    mark_waiting(&mut merged, 1_000, 2_000);
    assert!(!merged.waiting_checks);
}

#[test]
fn a_pr_waiting_for_its_checks_is_asked_about_every_tick() {
    let mut p = open("h1", false);
    p.checked_at = 10_000;
    let mut t = task(p.clone());
    t.pushed_at = 9_000;
    assert!(!due(&t, 10_000 + PR_FAST_POLL_MS - 1), "just pushed, checks known: every 10 s");
    p.waiting_checks = true;
    t.repos[0].pr = Some(p);
    assert!(due(&t, 10_000 + PR_TICK_MS.cast_signed()), "checks still to show: every tick");
    mark_task(&mut t, 9_000 + CHECKS_GRACE_MS);
    assert!(t.pr().is_some_and(|p| !p.waiting_checks), "mark_task clears it on the task's own PR once the grace runs out");
}
