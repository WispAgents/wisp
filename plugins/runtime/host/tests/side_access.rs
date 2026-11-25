//! A side discussion has its main task's access on every harness (audit user-side-full-access): the main task's mode,
//! never below Edits (it used to fall back to Plan on a harness that can't ask before a write), its commands run without
//! a card, Allow & retry works on it, and its first run gets the main task's MCP servers. Its write to a file the main
//! task changed isn't made: it becomes a proposal on the main task, applied or discarded there (audit
//! user-side-edits-for-approval).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use common::{Fake, Recording, rig_on, run_of, until};
use serde_json::{Value, json};
use wisp_runtime::Recorder;
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{ApprovalArgs, FollowUpArgs, StartArgs};
use wisp_runtime_api::model::{Mode, Session, TurnStatus};
use wisp_runtime_api::points::{McpQuery, RunPart};

/// Stands in for wisp.connectors: a run gets the connectors its task names and its overlay's, and the mode each run
/// asked them for is kept.
#[derive(Default)]
struct Conn(Mutex<Vec<(String, Mode)>>);

impl RunPart for Conn {
    fn id(&self) -> &'static str {
        "test.conn"
    }
    fn mcp_servers(&self, s: &Session, q: &McpQuery) -> BTreeMap<String, Value> {
        self.0.lock().unwrap().push((s.id.0.clone(), q.mode));
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
}

fn folder(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("wisp-side-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    wisp_proc::canonical_blocking(&d).unwrap().display().to_string()
}

async fn done(r: &common::Rig, id: &str, n: usize) {
    until("its turns", || async {
        let t = r.rt.turns(id, None, None).await.unwrap();
        t.len() == n && t.iter().all(|t| t.status != TurnStatus::Running)
    })
    .await;
}

/// A side discussion of `main`, as `runtime.create` makes one the user starts (`mark_side`, `side_of_main`).
fn side(main: &SessionId, prompt: &str, slot: &Value) -> StartArgs {
    let mut own = json!({ "sideOf": { "id": main.as_str(), "title": "The main task" } });
    for (k, v) in slot.as_object().cloned().unwrap_or_default() {
        own[k] = v;
    }
    let mut a = StartArgs { prompt: prompt.into(), parent_id: Some(main.clone()), mode: Some(Mode::Plan), ..StartArgs::default() };
    a.ext.insert("wisp.runtime".into(), own);
    a
}

#[tokio::test(flavor = "multi_thread")]
async fn a_side_of_a_full_auto_task_runs_in_full_auto_on_a_harness_without_a_gate_and_gets_its_mcp_servers() {
    let conn = Arc::new(Conn::default());
    let (h, runs) = Recording::of(Fake::harness());
    let r = rig_on(Recorder { parts: vec![conn.clone()], ..Recorder::default() }, vec![h]).await;
    let setup = json!({ "spec": { "connectors": ["linear"] }, "connectors": { "docs": { "command": "d" } }, "subagents": [{ "name": "rev", "agent": "fake" }] });
    let mut a = StartArgs { prompt: "main work".into(), cwd: Some(folder("auto")), mode: Some(Mode::Auto), ..StartArgs::default() };
    a.ext.insert("wisp.runtime".into(), setup.clone());
    let main = r.rt.create(a).await.unwrap().id;
    done(&r, main.as_str(), 1).await;
    let s = r.rt.create(side(&main, "side talk", &setup)).await.unwrap().id;
    let run = run_of(&runs, "side talk").await;
    assert_eq!(run.mode, Mode::Auto, "the main task's Full auto, not Plan");
    assert!(run.side && run.plan_commands, "it runs commands");
    let parent = run_of(&runs, "main work").await;
    assert_eq!(run.mcp_servers.keys().collect::<Vec<_>>(), parent.mcp_servers.keys().collect::<Vec<_>>());
    let asked = conn.0.lock().unwrap().iter().find(|(id, _)| *id == s.0).map(|(_, m)| *m);
    assert_eq!(asked, Some(Mode::Auto), "McpQuery.mode is the mode it runs in");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_side_of_a_plan_task_runs_in_edits_never_plan() {
    let (h, runs) = Recording::of(Fake::harness());
    let r = rig_on(Recorder::default(), vec![h]).await;
    let a = StartArgs { prompt: "plan it".into(), cwd: Some(folder("plan")), mode: Some(Mode::Plan), ..StartArgs::default() };
    let main = r.rt.create(a).await.unwrap().id;
    done(&r, main.as_str(), 1).await;
    r.rt.create(side(&main, "side of plan", &json!({}))).await.unwrap();
    assert_eq!(run_of(&runs, "side of plan").await.mode, Mode::Edits);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sides_command_runs_without_a_card_and_allow_and_retry_works_on_it() {
    let r = rig_on(Recorder::default(), vec![Fake::harness()]).await;
    // in Edits (a Full auto run's refusals are its harness's own: nothing to allow)
    let a = StartArgs { prompt: "main work".into(), cwd: Some(folder("cmd")), mode: Some(Mode::Edits), ..StartArgs::default() };
    let main = r.rt.create(a).await.unwrap().id;
    done(&r, main.as_str(), 1).await;
    let s = r.rt.create(side(&main, "blocked side", &json!({}))).await.unwrap().id;
    done(&r, s.as_str(), 1).await;
    let bash = ApprovalArgs { id: s.clone(), tool: "Bash".into(), input: json!({ "command": "npm test" }), tool_use_id: None };
    let out = r.rt.approval(bash).await.unwrap();
    assert_eq!(out["behavior"], "allow", "{out}");
    assert!(r.rt.get(s.as_str()).await.unwrap().ext["wisp.runtime"].get("ask").is_none(), "no card");
    // what its permissions blocked, allowed and retried there (was: "A side discussion can't allow this")
    let out = r.rt.approve_blocked(&s, None, true, "user:full").await.unwrap();
    assert!(out["turnId"].is_string() || out["queued"].is_string(), "{out}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sides_write_to_a_file_the_main_task_changed_is_proposed_there_and_applied_or_discarded() {
    let (fake, runs) = Recording::of(Fake::harness());
    let r = rig_on(Recorder::default(), vec![fake]).await;
    let dir = folder("propose");
    std::fs::write(format!("{dir}/a.ts"), "main's\n").unwrap();
    // its turn changed a.ts (the fake's change), never b.ts
    let main = r.rt.create(StartArgs { prompt: "main work".into(), cwd: Some(dir.clone()), ..StartArgs::default() }).await.unwrap().id;
    done(&r, main.as_str(), 1).await;
    let s = r.rt.create(side(&main, "side talk", &json!({}))).await.unwrap().id;
    done(&r, s.as_str(), 1).await;
    let write = |path: &str, content: &str| ApprovalArgs {
        id: s.clone(),
        tool: "Write".into(),
        input: json!({ "file_path": format!("{dir}/{path}"), "content": content }),
        tool_use_id: None,
    };
    let out = r.rt.approval(write("a.ts", "side's\n")).await.unwrap();
    assert_eq!(out["behavior"], "deny");
    assert!(out["message"].as_str().unwrap().contains("proposed to the main task"), "{out}");
    assert_eq!(std::fs::read_to_string(format!("{dir}/a.ts")).unwrap(), "main's\n", "nothing written");
    let m = r.rt.get(main.as_str()).await.unwrap();
    let p = m.ext["wisp.runtime"]["proposals"][0].clone();
    assert_eq!((p["side"].as_str(), p["path"].as_str(), p["after"].as_str()), (Some(s.as_str()), Some("a.ts"), Some("side's\n")));
    assert_eq!(p["before"], "main's\n");
    // a file the main task never touched: written, no card, no proposal
    let out = r.rt.approval(write("b.ts", "new\n")).await.unwrap();
    assert_eq!(out["behavior"], "allow", "{out}");
    let m = r.rt.get(main.as_str()).await.unwrap();
    assert_eq!(m.ext["wisp.runtime"]["proposals"].as_array().unwrap().len(), 1);
    assert!(r.rt.get(s.as_str()).await.unwrap().ext["wisp.runtime"].get("ask").is_none());

    // applied: written, gone, and the main task's agent hears of it
    let id = p["id"].as_str().unwrap();
    r.rt.apply_proposal(&main, id).await.unwrap();
    assert_eq!(std::fs::read_to_string(format!("{dir}/a.ts")).unwrap(), "side's\n");
    let m = r.rt.get(main.as_str()).await.unwrap();
    assert!(m.ext["wisp.runtime"].get("proposals").is_none(), "{:?}", m.ext);
    assert_eq!(m.ext["wisp.runtime"]["sideApplied"][0]["path"], "a.ts");
    let next = FollowUpArgs { id: main.clone(), text: "main next".into(), ..FollowUpArgs::default() };
    r.rt.follow_up(next).await.unwrap();
    let told = run_of(&runs, "main next").await.prompt;
    assert!(told.contains("applied a change a side discussion proposed") && told.contains("`a.ts`, from the side discussion"), "{told}");
    done(&r, main.as_str(), 2).await;
    assert!(r.rt.get(main.as_str()).await.unwrap().ext["wisp.runtime"].get("sideApplied").is_none(), "told once");

    // a stale one (the file changed since): a Conflict, nothing written
    r.rt.approval(write("a.ts", "again\n")).await.unwrap();
    std::fs::write(format!("{dir}/a.ts"), "edited meanwhile\n").unwrap();
    let m = r.rt.get(main.as_str()).await.unwrap();
    let stale = m.ext["wisp.runtime"]["proposals"][0]["id"].as_str().unwrap().to_owned();
    let err = r.rt.apply_proposal(&main, &stale).await.unwrap_err().to_string();
    assert!(err.contains("changed since"), "{err}");
    assert_eq!(std::fs::read_to_string(format!("{dir}/a.ts")).unwrap(), "edited meanwhile\n");
    // discarded: gone, nothing written
    r.rt.discard_proposal(&main, &stale).await.unwrap();
    assert!(r.rt.get(main.as_str()).await.unwrap().ext["wisp.runtime"].get("proposals").is_none());
    assert_eq!(std::fs::read_to_string(format!("{dir}/a.ts")).unwrap(), "edited meanwhile\n");
    std::fs::remove_dir_all(&dir).ok();
}
