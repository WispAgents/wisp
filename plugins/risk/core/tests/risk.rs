#![allow(clippy::unwrap_used)]
//! Merge risk's pure logic (ADR-0140): a task's open PRs, which heads are due, the slot's transitions, the view, who
//! judges and reading the answer.

use serde_json::json;
use wisp_risk_api::{Level, Pick, Slot, Status};
use wisp_risk_core::{self as core, Agent, Answer, OpenPr, Outcome, STALE_MS};

fn ship() -> serde_json::Value {
    json!({
        "repos": [
            { "name": "app", "pr": { "url": "u1", "number": 1, "title": "Own", "state": "OPEN", "head": "a1" } },
            { "name": "api", "pr": { "url": "u3", "number": 3, "state": "MERGED", "head": "c1" } }
        ],
        "prs": [
            { "repo": "lib", "pr": { "url": "u2", "number": 7, "state": "OPEN", "head": "b1" } },
            { "repo": "app", "pr": { "url": "u1", "number": 1, "state": "OPEN", "head": "a1" } },
            { "repo": "lib", "pr": { "url": "u4", "number": 9, "state": "OPEN", "head": "" } }
        ]
    })
}

fn pick() -> Pick {
    Pick { agent: "codex".into(), model: String::new() }
}

fn answer(level: Level) -> Outcome {
    Outcome::Answered(Answer { level, score: Some(70), summary: "touches auth".into(), reasons: vec!["src/auth.rs: token check".into()] })
}

#[test]
fn every_open_pr_with_a_head_counts_once_the_tasks_own_first() {
    let open = core::open_prs(&ship());
    let urls: Vec<&str> = open.iter().map(|p| p.url.as_str()).collect();
    assert_eq!(urls, ["u1", "u2"], "merged and headless PRs aren't judged; an attached copy of its own PR counts once");
    assert_eq!((open[1].repo.as_str(), open[1].number), ("lib", 7));
    assert_eq!(core::open_prs(&serde_json::Value::Null).len(), 0, "a missing PR list judges nothing");
}

#[test]
fn a_head_is_due_once_and_again_after_a_push() {
    let open = core::open_prs(&ship());
    let empty = Slot::default();
    assert_eq!(core::due(&open, &empty, 0, false, None).len(), 2);
    assert_eq!(core::due(&open, &empty, 0, false, Some("u2")).iter().map(|p| p.url.as_str()).collect::<Vec<_>>(), ["u2"]);
    let running = core::start(&empty, &open, &open, &pick(), 100);
    assert!(core::due(&open, &running, 200, false, None).is_empty(), "being judged: not due");
    assert!(core::due(&open, &running, 200, true, None).is_empty(), "Judge again never starts a second run of a head being judged");
    assert_eq!(core::due(&open, &running, 100 + STALE_MS + 1, false, None).len(), 2, "a run cut short is due again");
    let done = core::finish(&running, "u1", "a1", answer(Level::High), false, 300).unwrap();
    let done = core::finish(&done, "u2", "b1", answer(Level::Low), false, 300).unwrap();
    assert!(core::due(&open, &done, 400, false, None).is_empty(), "judged: not due");
    assert_eq!(core::due(&open, &done, 400, true, None).len(), 2, "Judge again re-judges");
    let mut pushed = open.clone();
    pushed[0].head = "a2".into();
    assert_eq!(
        core::due(&pushed, &done, 400, false, None).iter().map(|p| p.url.as_str()).collect::<Vec<_>>(),
        ["u1"],
        "a push: that PR only"
    );
}

#[test]
fn an_answer_lands_only_on_the_run_it_belongs_to() {
    let open = core::open_prs(&ship());
    let running = core::start(&Slot::default(), &open, &open, &pick(), 100);
    let v = &running.prs[0];
    assert_eq!((v.status, v.by.as_ref().map(|p| p.agent.as_str()), v.at), (Status::Running, Some("codex"), 100));
    assert!(core::finish(&running, "u1", "a0", answer(Level::High), false, 300).is_none(), "an older head's answer is dropped");
    assert!(core::finish(&running, "u9", "a1", answer(Level::High), false, 300).is_none());
    let done = core::finish(&running, "u1", "a1", answer(Level::High), true, 300).unwrap();
    let v = &done.prs[0];
    assert_eq!((v.status, v.level, v.score, v.partial, v.at), (Status::Done, Some(Level::High), Some(70), true, 300));
    assert!(core::finish(&done, "u1", "a1", answer(Level::Low), false, 400).is_none(), "a run answers once");
    let failed = core::finish(&done, "u2", "b1", Outcome::Failed("no diff".into()), false, 300).unwrap();
    assert_eq!((failed.prs[1].status, failed.prs[1].why.as_deref()), (Status::Failed, Some("no diff")));
}

#[test]
fn a_closed_prs_verdict_goes_when_the_next_run_starts() {
    let open = core::open_prs(&ship());
    let both = core::start(&Slot::default(), &open, &open, &pick(), 100);
    let only_one = &open[..1];
    let next = core::start(&both, only_one, &[], &pick(), 200);
    assert_eq!(next.prs.iter().map(|v| v.url.as_str()).collect::<Vec<_>>(), ["u1"]);
}

#[test]
fn the_view_says_which_verdicts_are_current_and_a_lost_run_failed() {
    let open = core::open_prs(&ship());
    let running = core::start(&Slot::default(), &open, &open, &pick(), 100);
    let done = core::finish(&running, "u2", "b1", answer(Level::Medium), false, 300).unwrap();
    let mut pushed = open.clone();
    pushed[1].head = "b2".into();
    let v = core::view(&done, &pushed, 400);
    assert_eq!(v.iter().map(|r| (r.verdict.url.as_str(), r.current)).collect::<Vec<_>>(), [("u1", true), ("u2", false)]);
    let late = core::view(&done, &open, 100 + STALE_MS + 1);
    assert_eq!((late[0].verdict.status, late[0].verdict.why.is_some()), (Status::Failed, true), "running past its time reads as failed");
    assert_eq!(late[1].verdict.status, Status::Done);
}

#[test]
fn the_judge_is_another_harness_than_the_author_or_the_one_picked() {
    let agents = vec![
        Agent { id: "claude".into(), name: "Claude Code".into(), available: true },
        Agent { id: "codex".into(), name: "Codex".into(), available: true },
        Agent { id: "grok".into(), name: "Grok".into(), available: false },
    ];
    let authors = core::authors_of("claude", ["auto", "claude", "wisp"]);
    assert_eq!(authors, ["claude"]);
    assert_eq!(core::pick_judge("other", &authors, &agents).unwrap().agent, "codex");
    assert_eq!(core::pick_judge("claude", &authors, &agents).unwrap().agent, "claude", "picked by name: even the author");
    assert_eq!(core::pick_judge("grok", &authors, &agents).unwrap_err(), "Grok isn't installed here.");
    assert_eq!(core::pick_judge("off", &authors, &agents).unwrap_err(), "Merge risk is off.");
    assert!(core::pick_judge("", &authors, &agents).is_err(), "unset is off (off by default)");
    let both = core::authors_of("claude", ["codex"]);
    assert!(core::pick_judge("other", &both, &agents).is_err(), "every installed harness wrote it");
}

#[test]
fn the_answer_is_read_from_json_or_from_words_around_it() {
    let a = core::read_answer(&json!({ "risk": "HIGH", "score": 88.6, "summary": "drops a column", "reasons": ["migrations/0042.sql drops users.email", { "text": "no backfill" }, ""] })).unwrap();
    assert_eq!((a.level, a.score, a.summary.as_str(), a.reasons.len()), (Level::High, Some(89), "drops a column", 2));
    let t = core::read_answer(&json!("Here it is:\n```json\n{\"level\": \"moderate\", \"score\": \"40\", \"reasons\": []}\n```")).unwrap();
    assert_eq!((t.level, t.score), (Level::Medium, Some(40)));
    let s = core::read_answer(&json!({ "score": 150 })).unwrap();
    assert_eq!((s.level, s.score), (Level::High, Some(100)), "a level from the score, clamped");
    assert_eq!(core::read_answer(&json!({ "score": 10 })).unwrap().level, Level::Low);
    assert!(core::read_answer(&json!({ "summary": "looks fine" })).is_none(), "no level and no score: not a verdict");
    assert!(core::read_answer(&json!("I can't read the diff.")).is_none());
    assert!(core::read_answer(&json!(42)).is_none());
}

#[test]
fn a_long_diff_is_cut_at_a_lines_end_and_the_prompt_says_so() {
    let line = "+ let x = \"é\";\n";
    let big = line.repeat(core::MAX_DIFF / line.len() + 10);
    let (cut, partial) = core::fit_diff(&big);
    assert!(partial && cut.len() <= core::MAX_DIFF && cut.ends_with('\n'));
    let (same, p) = core::fit_diff("+a\n");
    assert_eq!((same.as_str(), p), ("+a\n", false));
    let pr = OpenPr { url: "https://github.com/o/r/pull/7".into(), repo: "lib".into(), number: 7, head: "b1".into(), title: "Fix".into() };
    let prompt = core::prompt("Fix login", "make login faster", &pr, &cut, true);
    assert!(prompt.contains("lib #7 Fix") && prompt.contains("its start only") && prompt.contains("\"risk\""));
    assert!(!core::prompt("t", "", &pr, "+a", false).contains("its start only"));
}
