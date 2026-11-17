//! A fresh conversation on the same task (ADR-0096, `FollowUpArgs::fresh`, a long thread's Fresh thread): its turn
//! resumes nothing and hears nothing of the turns before it (no hand-off note, even after a harness switch), it is
//! marked with the conversation it left, the turn after it resumes the new one, and it's refused while a turn runs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Fake, rig_on, until};
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::{FollowUpArgs, ProbeArgs, RunPick, StartArgs};
use wisp_runtime_api::model::TurnStatus;

#[tokio::test(flavor = "multi_thread")]
async fn a_fresh_follow_up_starts_a_new_conversation_linked_to_the_old_one() {
    let r = rig_on(Recorder::default(), vec![Fake::named("fake", "Fake CLI", &["low"]), Fake::named("other", "Other CLI", &["low"])]).await;
    let start = StartArgs { prompt: "fix the login".into(), agent: Some("fake".into()), cwd: Some(common::tmp()), ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    let done = |n: usize| {
        let (rt, id) = (r.rt.clone(), id.0.clone());
        async move { rt.turns(&id, None, None).await.unwrap().iter().filter(|t| t.status == TurnStatus::Done).count() == n }
    };
    until("the first turn", || done(1)).await;
    // a turn on another harness, so an ordinary follow-up back would carry a hand-off note
    let other = RunPick { agent: Some("other".into()), ..RunPick::default() };
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "now the signup".into(), run: Some(other), ..FollowUpArgs::default() })
        .await
        .unwrap();
    until("the second turn", || done(2)).await;
    let back = RunPick { agent: Some("fake".into()), ..RunPick::default() };
    let brief = "# Fresh thread\nCarry on with the signup.";
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: brief.into(), run: Some(back), fresh: true, ..FollowUpArgs::default() })
        .await
        .unwrap();
    until("the fresh turn", || done(3)).await;
    let spawned = r.sim.spawned();
    assert_eq!(spawned[2].args[0], brief, "no hand-off note: the brief alone");
    assert!(!spawned[2].args.iter().any(|a| a.starts_with("--resume")), "a new conversation: {:?}", spawned[2].args);
    let fresh = r.rt.turns(id.as_str(), None, Some(1)).await.unwrap().pop().unwrap();
    assert_eq!(fresh.ext["wisp.runtime"]["fresh"], true);
    assert_eq!(fresh.ext["wisp.runtime"]["after"], "conv-1", "the old conversation stays linked");
    // the next follow-up resumes the new conversation, and still hears nothing from before the fresh one
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "and the tests".into(), ..FollowUpArgs::default() }).await.unwrap();
    until("the turn after", || done(4)).await;
    let next = &r.sim.spawned()[3].args;
    assert_eq!(next[0], "and the tests");
    assert!(next.iter().any(|a| a == "--resume=conv-1"), "{next:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fresh_follow_up_is_refused_while_a_turn_runs() {
    let r = rig_on(Recorder::default(), vec![Fake::named("fake", "Fake CLI", &["low"])]).await;
    let start = StartArgs {
        prompt: "gate fresh-running hold".into(),
        agent: Some("fake".into()),
        cwd: Some(common::tmp()),
        ..StartArgs::default()
    };
    let id = r.rt.create(start).await.unwrap().id;
    until("the turn runs", || {
        let (rt, id) = (r.rt.clone(), id.0.clone());
        async move { rt.get(&id).await.unwrap().state == wisp_runtime_api::model::SessionState::Running }
    })
    .await;
    let e =
        r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "brief".into(), fresh: true, ..FollowUpArgs::default() }).await.unwrap_err();
    assert!(e.to_string().contains("fresh thread once it’s idle"), "{e}");
    assert!(r.rt.get(id.as_str()).await.unwrap().queue.is_empty(), "nothing was queued");
    common::open("fresh-running");
}

/// `runtime.probe` (the compaction audit's question): asked of a fork of the conversation, answering only, kept
/// nowhere (no turn, no /btw); a harness that can't fork is refused.
#[tokio::test(flavor = "multi_thread")]
async fn a_probe_asks_a_copy_of_the_conversation_and_keeps_nothing() {
    let r = rig_on(Recorder::default(), vec![Fake::forks("fake", "Fake CLI"), Fake::named("other", "Other CLI", &["low"])]).await;
    let start = StartArgs { prompt: "fix the login".into(), agent: Some("fake".into()), cwd: Some(common::tmp()), ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    until("the first turn", || {
        let (rt, id) = (r.rt.clone(), id.0.clone());
        async move { rt.turns(&id, None, None).await.unwrap().iter().any(|t| t.status == TurnStatus::Done) }
    })
    .await;
    let a = r.rt.probe(ProbeArgs { id: id.clone(), question: "what do you remember?".into(), timeout_ms: Some(10_000) }).await.unwrap();
    assert_ne!(a.text, "");
    let asked = r.sim.spawned().pop().unwrap();
    assert_eq!(asked.args[0], "what do you remember?");
    assert!(asked.args.iter().any(|x| x == "--resume=conv-1"), "on the task's conversation: {:?}", asked.args);
    assert_eq!(r.rt.turns(id.as_str(), None, None).await.unwrap().len(), 1, "no turn");
    assert!(r.rt.get(id.as_str()).await.unwrap().ext["wisp.runtime"].get("btw").is_none(), "not a /btw");
    let start =
        StartArgs { prompt: "fix the signup".into(), agent: Some("other".into()), cwd: Some(common::tmp()), ..StartArgs::default() };
    let other = r.rt.create(start).await.unwrap().id;
    until("its turn", || {
        let (rt, id) = (r.rt.clone(), other.0.clone());
        async move { rt.turns(&id, None, None).await.unwrap().iter().any(|t| t.status == TurnStatus::Done) }
    })
    .await;
    let e = r.rt.probe(ProbeArgs { id: other, question: "what do you remember?".into(), timeout_ms: None }).await.unwrap_err();
    assert!(e.to_string().contains("copy"), "{e}");
}
