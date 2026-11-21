//! ADR-0083: every running turn's activity goes through one coalescer: their lines commit together once a window
//! (`ACTIVITY_WINDOW_MS` after the first one buffered), and a tool's result or a turn's end commits at once. The
//! store runs on a clock the test moves, so the window passes only when the test says.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};

use common::{open, rig_clocked, until};
use serde_json::Value;
use wisp_runtime::Emitted;
use wisp_runtime_api::commands::StartArgs;
use wisp_runtime_api::model::TurnStatus;
use wisp_store::ManualClock;

fn start(prompt: &str) -> StartArgs {
    StartArgs { prompt: prompt.into(), cwd: Some(common::tmp()), ..StartArgs::default() }
}

/// Each commit's activity rows, as (task, kind, seq), in the order the commit holds them.
type Commits = Arc<Mutex<Vec<Vec<(String, String, u64)>>>>;

fn activity_of(commits: &Commits) -> Vec<Vec<(String, String, u64)>> {
    commits.lock().unwrap().iter().filter(|c| !c.is_empty()).cloned().collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn running_tasks_lines_commit_together_and_a_tool_result_or_a_turns_end_commits_at_once() {
    let clock = Arc::new(ManualClock::new(1_700_000_000_000));
    let r = rig_clocked(clock.clone()).await;
    let commits: Commits = Arc::default();
    let seen = commits.clone();
    let _sub = r.store.on_commit(move |rows| {
        let mut one = Vec::new();
        for row in rows.iter().filter(|row| row.entity == "event") {
            for e in row.patch.as_array().into_iter().flatten() {
                let field = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
                one.push((field("session_id"), field("kind"), e.get("seq").and_then(Value::as_u64).unwrap_or_default()));
            }
        }
        seen.lock().unwrap().push(one);
    });
    let a = r.rt.create(start("stream gc-a")).await.unwrap().id.0;
    let b = r.rt.create(start("stream gc-b")).await.unwrap().id.0;
    until("both runs to start", || async { r.sim.spawned().len() == 2 }).await;
    let before = activity_of(&commits).len();
    open("gc-a");
    open("gc-b");
    // published (so buffered) for both, never committed while the store's clock stands still
    let texts = |id: &str| {
        r.host
            .events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                Emitted::Activity(act) if act.session_id.0 == id => Some(act.items.len()),
                _ => None,
            })
            .sum::<usize>()
    };
    until("both tasks' three lines to be buffered", || async { texts(&a) == 3 && texts(&b) == 3 }).await;
    assert_eq!(activity_of(&commits).len(), before, "no line commits before the window: {:?}", activity_of(&commits));

    clock.advance(i64::try_from(wisp_runtime::ACTIVITY_WINDOW_MS).unwrap());
    until("the window's commit", || async { activity_of(&commits).len() == before + 1 }).await;
    let window = activity_of(&commits).pop().unwrap();
    for id in [&a, &b] {
        let mine: Vec<_> = window.iter().filter(|(s, ..)| s == id).collect();
        assert_eq!(mine.len(), 3, "both tasks' lines in the one commit: {window:?}");
        assert!(mine.iter().all(|(_, k, _)| k == "text"));
        assert!(mine.windows(2).all(|w| w[0].2 < w[1].2), "each task's lines in order: {window:?}");
    }

    // a tool's result: committed with no window, the clock still where it was
    open("gc-a.tool");
    until("a's tool result to commit at once", || async {
        activity_of(&commits).iter().flatten().any(|(s, k, _)| *s == a && k == "tool_output")
    })
    .await;

    // a turn's end: its last lines are in before it ends, with no window either
    open("gc-b.tool");
    open("gc-a.end");
    open("gc-b.end");
    for id in [&a, &b] {
        until("the turn to end", || async {
            r.rt.turns(id, None, Some(1)).await.unwrap().pop().map(|t| t.status) == Some(TurnStatus::Done)
        })
        .await;
        let turn = r.rt.turns(id, None, Some(1)).await.unwrap().pop().unwrap();
        let steps = r.rt.activity(turn.id.as_str(), None).await.unwrap().items;
        assert_eq!(steps.len(), 5, "three lines, the tool and its result, all kept: {steps:?}");
        assert!(steps.windows(2).all(|w| w[0].seq < w[1].seq));
    }
}
