//! Forks (audit user-fork-full-access, fork-claude-session-lost, fork-turn-includes-later): a fork keeps its task's
//! setup (connectors, team, variant, lean, limits) and none of its runs' state, so its first run has the same MCP
//! servers; its first run keeps its copy of the conversation (a /btw's is thrown away); and a fork from an earlier turn
//! goes on from that turn, never with what came after it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use common::{Fake, Recording, rig_on, run_of, until};
use serde_json::{Value, json};
use wisp_kernel::BoxFuture;
use wisp_runtime::Recorder;
use wisp_runtime_api::commands::{BtwArgs, FollowUpArgs, ForkArgs, StartArgs};
use wisp_runtime_api::model::{Session, TurnStatus};
use wisp_runtime_api::points::{McpQuery, RunPart};

/// Stands in for wisp.connectors and git: a run gets the connectors its task names (`spec.connectors`) and its overlay's
/// (`connectors`), each team it was asked for is kept, and each turn keeps a checkpoint of its own.
#[derive(Default)]
struct Parts {
    teams: Mutex<Vec<(String, Vec<Value>)>>,
    n: AtomicU32,
}

impl RunPart for Parts {
    fn id(&self) -> &'static str {
        "test.parts"
    }
    fn mcp_servers(&self, s: &Session, q: &McpQuery) -> BTreeMap<String, Value> {
        self.teams.lock().unwrap().push((s.id.0.clone(), q.team.clone()));
        let own = s.ext.get("wisp.runtime");
        let named = own.and_then(|o| o.pointer("/spec/connectors")).and_then(Value::as_array).cloned().unwrap_or_default();
        let overlay = own.and_then(|o| o.get("connectors")).and_then(Value::as_object).cloned().unwrap_or_default();
        named
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .chain(overlay.keys().cloned())
            .map(|n| (n, json!({ "command": "x" })))
            .collect()
    }
    fn before_turn(&self, _s: &Session) -> BoxFuture<'static, Option<(String, Value)>> {
        let n = self.n.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            Some(("wisp.git".to_owned(), json!({ "checkpoint": { "to": format!("snap-{n}"), "head": format!("head-{n}") } })))
        })
    }
}

async fn done(r: &common::Rig, id: &str, n: usize) {
    until("its turns", || async {
        let t = r.rt.turns(id, None, None).await.unwrap();
        t.len() == n && t.iter().all(|t| t.status == TurnStatus::Done)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fork_keeps_its_tasks_connectors_team_variant_and_lean_and_none_of_its_runs_state() {
    let parts = Arc::new(Parts::default());
    let (h, runs) = Recording::of(Fake::forks("fake", "Fake CLI"));
    let r = rig_on(Recorder { parts: vec![parts.clone()], ..Recorder::default() }, vec![h]).await;
    let id = r.rt.create(StartArgs { prompt: "first".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id;
    done(&r, id.as_str(), 1).await;
    let mut own = r.rt.get(id.as_str()).await.unwrap().ext["wisp.runtime"].clone();
    for (k, v) in [
        ("spec", json!({ "agent": "reviewer", "connectors": ["linear"] })),
        ("connectors", json!({ "docs": { "command": "docs-mcp" } })),
        ("subagents", json!([{ "name": "rev", "agent": "fake" }])),
        ("variant", json!({ "skills": "off" })),
        ("heldTo", json!({ "id": "lead" })),
        ("startedBy", json!("hand")),
        // a run's and a turn's state: never the fork's
        ("next", json!({ "once": "auto" })),
        ("btw", json!([{ "id": "b1", "question": "why?" }])),
        ("blocked", json!([{ "tool": "Bash" }])),
        ("grants", json!([{ "tool": "Bash", "detail": "npm publish" }])),
        ("forLater", json!({ "at": 1 })),
    ] {
        own[k] = v;
    }
    r.rt.set_ext(id.as_str(), "wisp.runtime", own, None).await.unwrap();
    r.rt.set_lean(id.as_str(), Some("on".into())).await.unwrap();
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "parent again".into(), ..FollowUpArgs::default() }).await.unwrap();
    done(&r, id.as_str(), 2).await;

    let fork = r.rt.fork(ForkArgs { id: id.clone(), new_id: None, turn_id: None, title: None }).await.unwrap().id;
    let s = r.rt.get(fork.as_str()).await.unwrap();
    let o = &s.ext["wisp.runtime"];
    for k in ["spec", "connectors", "subagents", "variant", "heldTo", "startedBy"] {
        assert_eq!(o[k], r.rt.get(id.as_str()).await.unwrap().ext["wisp.runtime"][k], "{k} carries over: {o}");
    }
    for k in ["btw", "blocked", "grants", "forLater"] {
        assert!(o.get(k).is_none(), "{k} is the parent's run's, not the fork's: {o}");
    }
    assert_eq!(o["next"], json!({ "fork": true }), "only the fork's own first-run extras");
    assert_eq!(s.lean.as_deref(), Some("on"));

    r.rt.follow_up(FollowUpArgs { id: fork.clone(), text: "fork first".into(), ..FollowUpArgs::default() }).await.unwrap();
    let (parent, first) = (run_of(&runs, "parent again").await, run_of(&runs, "fork first").await);
    assert_eq!(first.mcp_servers.keys().collect::<Vec<_>>(), parent.mcp_servers.keys().collect::<Vec<_>>());
    assert!(first.mcp_servers.contains_key("linear") && first.mcp_servers.contains_key("docs"), "{:?}", first.mcp_servers.keys());
    let team = |of: &str| parts.teams.lock().unwrap().iter().rev().find(|(s, _)| s == of).map(|(_, t)| t.clone()).unwrap();
    assert_eq!(team(fork.as_str()), team(id.as_str()), "McpQuery.team is the parent's");
    assert_eq!(first.mode, parent.mode, "not the parent's leftover once:auto");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forks_first_run_keeps_its_copy_of_the_conversation_and_a_btws_is_thrown_away() {
    let (h, runs) = Recording::of(Fake::forks("fake", "Fake CLI"));
    let r = rig_on(Recorder::default(), vec![h]).await;
    let id = r.rt.create(StartArgs { prompt: "first".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id;
    done(&r, id.as_str(), 1).await;
    r.rt.btw(BtwArgs { id: id.clone(), question: "why that?".into(), attachments: Vec::new() }).await.unwrap();
    let btw = run_of(&runs, "why that?").await;
    assert!(btw.fork && !btw.fork_keep, "a /btw works on a copy it throws away");
    let fork = r.rt.fork(ForkArgs { id: id.clone(), new_id: None, turn_id: None, title: None }).await.unwrap().id;
    r.rt.follow_up(FollowUpArgs { id: fork, text: "go on".into(), ..FollowUpArgs::default() }).await.unwrap();
    let first = run_of(&runs, "go on").await;
    assert!(first.fork && first.fork_keep, "the fork's copy is its own conversation from now on");
    assert_eq!(first.resume_at, None, "from the latest turn: the whole conversation");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fork_from_an_earlier_turn_goes_on_from_that_turn_and_its_folder_then() {
    let parts = Arc::new(Parts::default());
    let (h, runs) = Recording::of(Fake::forks("fake", "Fake CLI"));
    let r = rig_on(Recorder { parts: vec![parts], ..Recorder::default() }, vec![h]).await;
    let id =
        r.rt.create(StartArgs { prompt: "first mark:msg-1".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id;
    done(&r, id.as_str(), 1).await;
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "second mark:msg-2".into(), ..FollowUpArgs::default() }).await.unwrap();
    done(&r, id.as_str(), 2).await;
    let turns = r.rt.turns(id.as_str(), None, None).await.unwrap();
    assert_eq!(turns[0].ext["wisp.runtime"]["resumeAt"], json!("msg-1"), "each turn keeps where it left the conversation");
    let a = ForkArgs { id: id.clone(), new_id: None, turn_id: Some(turns[0].id.clone()), title: None };
    let fork = r.rt.fork(a).await.unwrap().id;
    let s = r.rt.get(fork.as_str()).await.unwrap();
    assert_eq!(s.ext["wisp.runtime"]["forkOf"]["checkpoint"], turns[1].ext["wisp.git"]["checkpoint"], "its folder as before turn 2");
    assert!(s.title.ends_with("(fork at turn 1)"), "{}", s.title);
    assert_eq!(r.rt.turns(fork.as_str(), None, None).await.unwrap().len(), 1);
    r.rt.follow_up(FollowUpArgs { id: fork, text: "from there".into(), ..FollowUpArgs::default() }).await.unwrap();
    let first = run_of(&runs, "from there").await;
    assert_eq!(first.resume_at.as_deref(), Some("msg-1"), "the copy ends where turn 1 did");
}

/// A harness that can't end its copy at an earlier turn (Codex, and any without the turn's point) refuses the fork,
/// never forks it with the later turns' context.
#[tokio::test(flavor = "multi_thread")]
async fn a_fork_from_an_earlier_turn_on_a_harness_that_cant_go_back_is_refused() {
    let r = rig_on(Recorder::default(), vec![Fake::harness()]).await;
    let id = r.rt.create(StartArgs { prompt: "first".into(), cwd: Some(common::tmp()), ..StartArgs::default() }).await.unwrap().id;
    done(&r, id.as_str(), 1).await;
    r.rt.follow_up(FollowUpArgs { id: id.clone(), text: "second".into(), ..FollowUpArgs::default() }).await.unwrap();
    done(&r, id.as_str(), 2).await;
    let turns = r.rt.turns(id.as_str(), None, None).await.unwrap();
    let a = ForkArgs { id: id.clone(), new_id: None, turn_id: Some(turns[0].id.clone()), title: None };
    let err = r.rt.fork(a).await.unwrap_err().to_string();
    assert!(err.contains("can’t go back to an earlier turn"), "{err}");
    // from its latest turn it forks as before
    r.rt.fork(ForkArgs { id, new_id: None, turn_id: None, title: None }).await.unwrap();
}
