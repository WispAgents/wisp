//! A resumed conversation's usage counters are its running total (Claude Code's `total_cost_usd`, Codex's
//! `turn.completed` usage): each run is told where they stood after the conversation's last turn here
//! (`HarnessRun::prior`, from the turn's stored `stats.totals`), so a resumed turn counts only its own. Another
//! harness, a fresh conversation or nothing to resume starts from zero; a chat brought in, whose counters wisp never
//! saw, takes its first result as the baseline (`unknown_prior`). Also: the CLI's last stderr line reaches the parser
//! even without a newline after it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::float_cmp)]

mod common;

use common::{Fake, Rig, rig_on, until};
use wisp_runtime::Recorder;
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{FollowUpArgs, RunPick, StartArgs};
use wisp_runtime_api::model::{Turn, TurnStatus};

async fn rig() -> Rig {
    rig_on(Recorder::default(), vec![Fake::named("fake", "Fake CLI", &["low"]), Fake::named("other", "Other CLI", &["low"])]).await
}

async fn ended(r: &Rig, id: &SessionId, n: usize) -> Vec<Turn> {
    let (rt, sid) = (r.rt.clone(), id.0.clone());
    until(&format!("{n} turns"), || {
        let (rt, sid) = (rt.clone(), sid.clone());
        async move { rt.turns(&sid, None, None).await.unwrap().iter().filter(|t| t.status != TurnStatus::Running).count() == n }
    })
    .await;
    r.rt.turns(id.as_str(), None, None).await.unwrap()
}

fn spent(t: &Turn) -> (Option<f64>, Option<u64>) {
    let st = t.stats.as_ref().unwrap();
    (st.cost_usd, st.input_tokens)
}

fn arg(r: &Rig, n: usize, prefix: &str) -> Option<String> {
    r.sim.spawned()[n].args.iter().find(|a| a.starts_with(prefix)).cloned()
}

async fn follow(r: &Rig, id: &SessionId, text: &str, run: Option<RunPick>, fresh: bool) {
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: text.into(), run, fresh, ..FollowUpArgs::default() }).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_resumed_turn_counts_only_its_own_cost_and_tokens() {
    let r = rig().await;
    let start = StartArgs { prompt: "cumul 1.0 1000".into(), agent: Some("fake".into()), cwd: Some(common::tmp()), ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    ended(&r, &id, 1).await;
    follow(&r, &id, "cumul 1.5 1600", None, false).await;
    let turns = ended(&r, &id, 2).await;
    assert_eq!(arg(&r, 0, "--prior"), None, "a new conversation starts from zero");
    assert_eq!(arg(&r, 1, "--prior").as_deref(), Some(r#"--prior={"costUsd":1.0,"inputTokens":1000}"#), "the first turn's totals");
    // Claude Code's cost and Codex's tokens alike: 1.0 then 0.5, 1000 then 600 (not 1.5 and 1600)
    assert_eq!((spent(&turns[0]), spent(&turns[1])), ((Some(1.0), Some(1000)), (Some(0.5), Some(600))));
    assert_eq!(turns[1].stats.as_ref().unwrap().totals, Some(serde_json::json!({ "costUsd": 1.5, "inputTokens": 1600 })));
}

#[tokio::test(flavor = "multi_thread")]
async fn another_harness_or_a_fresh_conversation_starts_from_zero() {
    let r = rig().await;
    let start = StartArgs { prompt: "cumul 1.0 1000".into(), agent: Some("fake".into()), cwd: Some(common::tmp()), ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    ended(&r, &id, 1).await;
    follow(&r, &id, "cumul 0.25 300", Some(RunPick { agent: Some("other".into()), ..RunPick::default() }), false).await;
    let turns = ended(&r, &id, 2).await;
    assert_eq!((arg(&r, 1, "--prior"), arg(&r, 1, "--unknown-prior")), (None, None), "the other harness's own conversation");
    assert_eq!(spent(&turns[1]), (Some(0.25), Some(300)));
    follow(&r, &id, "cumul 0.5 400", None, true).await;
    let turns = ended(&r, &id, 3).await;
    assert_eq!((arg(&r, 2, "--prior"), arg(&r, 2, "--unknown-prior")), (None, None), "a fresh conversation");
    assert_eq!(spent(&turns[2]), (Some(0.5), Some(400)));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_adopted_chats_first_turn_is_its_baseline() {
    let r = rig().await;
    let a = wisp_runtime_api::spec::AdoptArgs {
        title: "brought in".into(),
        agent: "fake".into(),
        cwd: common::tmp(),
        resume_id: Some("outside-1".into()),
        fresh: true,
        ..wisp_runtime_api::spec::AdoptArgs::default()
    };
    let id = r.rt.adopt(a).await.unwrap().id;
    follow(&r, &id, "cumul 3.0 5000", None, false).await;
    let turns = ended(&r, &id, 1).await;
    assert_eq!((arg(&r, 0, "--prior"), arg(&r, 0, "--unknown-prior").as_deref()), (None, Some("--unknown-prior")));
    assert_eq!(spent(&turns[0]), (Some(0.0), Some(0)), "all it ever spent elsewhere isn't this turn's");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_clis_last_stderr_line_without_a_newline_reaches_the_turn() {
    let r = rig().await;
    let start = StartArgs { prompt: "errtail".into(), agent: Some("fake".into()), cwd: Some(common::tmp()), ..StartArgs::default() };
    let id = r.rt.create(start).await.unwrap().id;
    let t = ended(&r, &id, 1).await.pop().unwrap();
    assert!(t.is_error);
    let said = serde_json::to_string(&t).unwrap();
    assert!(said.contains("fatal: the last words"), "{said}");
}
