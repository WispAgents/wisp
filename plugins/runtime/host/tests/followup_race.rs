//! Audit C2: quick follow-ups raced the queue. `wisp reply <id> ".." --queue` ten times back to back lost some
//! (`UNIQUE constraint failed: runtime_turn.session_id, runtime_turn.idx`) and ran the rest out of order: a follow-up
//! that found the task idle (its last turn just let go, or the one before it still starting) started a turn of its
//! own beside the queue's, both on the same turn index. Every follow-up is queued or started, in the order sent.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{rig, until};
use wisp_runtime_api::commands::{FollowUpArgs, StartArgs};
use wisp_runtime_api::model::TurnStatus;

fn follow(id: &wisp_runtime_api::SessionId, text: String) -> FollowUpArgs {
    FollowUpArgs { id: id.clone(), text, queue: Some(true), ..FollowUpArgs::default() }
}

async fn settled(r: &common::Rig, id: &str, n: usize) -> bool {
    let t = r.rt.turns(id, None, None).await.unwrap();
    t.len() == n && t.iter().all(|t| t.status == TurnStatus::Done) && r.rt.get(id).await.unwrap().queue.is_empty()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn many_quick_follow_ups_are_all_queued_or_started_in_the_order_sent_audit_c2() {
    let r = rig().await;
    let id = r.rt.create(StartArgs { prompt: "fix it".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id;
    until("the first turn", || async { settled(&r, id.as_str(), 1).await }).await;

    // back to back, as `wisp reply --queue` sends them: each answered before the next goes
    let sent: Vec<String> = (1..=25).map(|n| format!("C46 turn {n}")).collect();
    for t in &sent {
        r.rt.follow_up(follow(&id, t.clone())).await.unwrap_or_else(|e| panic!("{t}: {e}"));
    }
    until("every follow-up's turn", || async { settled(&r, id.as_str(), 26).await }).await;
    let prompts: Vec<String> = r.rt.turns(id.as_str(), None, None).await.unwrap().into_iter().skip(1).map(|t| t.prompt).collect();
    assert_eq!(prompts, sent, "none lost, in the order sent");

    // all at once, from many callers: none lost, each once, and the turns' indexes one after another
    let burst: Vec<String> = (1..=25).map(|n| format!("burst {n}")).collect();
    let calls: Vec<_> = burst
        .iter()
        .map(|t| {
            tokio::spawn({
                let (rt, f) = (r.rt.clone(), follow(&id, t.clone()));
                async move { rt.follow_up(f).await }
            })
        })
        .collect();
    for c in calls {
        c.await.unwrap().unwrap_or_else(|e| panic!("a follow-up refused: {e}"));
    }
    until("every burst turn", || async { settled(&r, id.as_str(), 51).await }).await;
    let turns = r.rt.turns(id.as_str(), None, None).await.unwrap();
    let mut got: Vec<String> = turns.iter().skip(26).map(|t| t.prompt.clone()).collect();
    got.sort();
    let mut want = burst.clone();
    want.sort();
    assert_eq!(got, want, "each burst follow-up ran once");
    assert_eq!(turns.iter().map(|t| t.idx).collect::<Vec<_>>(), (0..51).collect::<Vec<u32>>(), "one turn per index");
}

/// Audit I3: 25 quick `wisp reply`s (no `--queue`: the default follow-up setting, interrupt) each answered "queued",
/// yet three of them were stopped after ~50 ms and the rest ran out of order (I, 1, 9, 15, 21, 2, 3 …): each one sent
/// while a queued follow-up's turn ran interrupted that turn and went ahead of the ones waiting. The first may interrupt
/// the task's own running turn (the setting says so); every queued follow-up's turn then runs to its end, once, in the
/// order sent.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn quick_follow_ups_without_queue_each_run_to_the_end_in_the_order_sent_audit_i3() {
    let r = rig().await;
    // the task's own turn has its conversation and works on (gated), as the audit's did while the replies came
    let id =
        r.rt.create(StartArgs { prompt: "gate i3-first first".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id;
    until("the first turn running", || async {
        r.rt.turns(id.as_str(), None, None).await.unwrap().first().is_some_and(|t| t.status == TurnStatus::Running)
    })
    .await;
    let sent: Vec<String> = (1..=25).map(|n| format!("gate i3-{n} follow-up {n}")).collect();
    let plain = |t: &String| FollowUpArgs { id: id.clone(), text: t.clone(), ..FollowUpArgs::default() };
    // the first may interrupt the task's own turn (the setting says so); if it waits instead, that turn ends now
    r.rt.follow_up(plain(&sent[0])).await.unwrap();
    common::open("i3-first");
    // the queue starts it: a queued follow-up's turn, with the task's conversation to resume
    until("the first follow-up's turn running", || async {
        r.rt.turns(id.as_str(), None, None).await.unwrap().get(1).is_some_and(|t| t.status == TurnStatus::Running)
    })
    .await;
    // the rest come while it works: none may cut it short or go ahead of another
    for t in &sent[1..] {
        r.rt.follow_up(plain(t)).await.unwrap_or_else(|e| panic!("{t}: {e}"));
    }
    for n in 1..=25 {
        common::open(&format!("i3-{n}"));
    }
    until("every follow-up's turn", || async {
        let t = r.rt.turns(id.as_str(), None, None).await.unwrap();
        t.len() == 26 && t.iter().skip(1).all(|t| t.status == TurnStatus::Done) && r.rt.get(id.as_str()).await.unwrap().queue.is_empty()
    })
    .await;
    let turns = r.rt.turns(id.as_str(), None, None).await.unwrap();
    let prompts: Vec<String> = turns.iter().skip(1).map(|t| t.prompt.clone()).collect();
    assert_eq!(prompts, sent, "each once, in the order sent");
    assert!(
        turns.iter().skip(1).all(|t| t.status == TurnStatus::Done),
        "no follow-up's turn stopped: {:?}",
        turns.iter().map(|t| t.status).collect::<Vec<_>>()
    );
}
