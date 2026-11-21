//! ADR-0096 through wisp.threads' manifest: Fresh thread starts a new conversation on the same task from a brief (the
//! Notes, the files changed, the last turns) through runtime's `followUp { fresh }`; the compaction audit, when on,
//! asks a copy of the compacted conversation (`runtime.probe`), scores it without a model, and tells the next
//! follow-up what was lost, once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::services::Paths;
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup, WireError};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::model::{Session, Turn, TurnStats, TurnStatus};
use wisp_runtime_api::points::{PROMPT, TurnNote};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;
use wisp_threads::ThreadsPlugin;

/// wisp.runtime as far as Fresh thread and the audit ask it.
#[derive(Clone, Default)]
struct Runtime {
    tasks: Arc<Mutex<BTreeMap<String, Session>>>,
    turns: Arc<Mutex<Vec<Turn>>>,
    follow_ups: Arc<Mutex<Vec<r::FollowUpArgs>>>,
    marks: Arc<Mutex<Vec<r::TurnExtArgs>>>,
    probes: Arc<Mutex<Vec<String>>>,
    /// what the copy answers (Err: it can't)
    answer: Arc<Mutex<Option<std::result::Result<String, String>>>>,
    /// the task's spec (`runtime.spec`)
    spec: Arc<Mutex<wisp_runtime_api::spec::TaskSpec>>,
}

impl Runtime {
    fn task(&self, id: &str, lead: Option<&str>) {
        let s = Session { id: id.into(), parent_id: lead.map(Into::into), ..Default::default() };
        self.tasks.lock().unwrap().insert(id.to_owned(), s);
    }
}

impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let me = self.clone();
        s.command::<r::Get, _, _>(move |_, _, a: r::IdArgs| {
            let t = me.tasks.lock().unwrap().get(a.id.as_str()).cloned();
            async move { t.ok_or_else(|| WireError::not_found("That task is gone.")) }
        });
        let me = self.clone();
        s.command::<r::Turns, _, _>(move |_, _, _a: r::TurnsArgs| {
            let t = me.turns.lock().unwrap().clone();
            async move { Ok(t) }
        });
        s.command::<r::ActivityOf, _, _>(|_, _, _| async move { Ok(r::ActivityPage { seq: 0, items: Vec::new() }) });
        let me = self.clone();
        s.command::<r::SpecOf, _, _>(move |_, _, _a: r::IdArgs| {
            let spec = me.spec.lock().unwrap().clone();
            async move { Ok(spec) }
        });
        let me = self.clone();
        s.command::<r::FollowUp, _, _>(move |_, _, a: r::FollowUpArgs| {
            me.follow_ups.lock().unwrap().push(a);
            async move { Ok(r::FollowedUp { turn_id: Some("u9".into()), queued: None, interrupts: false }) }
        });
        let me = self.clone();
        s.command::<r::TurnSetExt, _, _>(move |_, _, a: r::TurnExtArgs| {
            me.marks.lock().unwrap().push(a);
            async move { Ok(r::Ack::default()) }
        });
        let me = self.clone();
        s.command::<r::Probe, _, _>(move |_, _, a: r::ProbeArgs| {
            me.probes.lock().unwrap().push(a.question);
            let out = me.answer.lock().unwrap().clone().unwrap_or_else(|| Ok(String::new()));
            async move {
                // a copy of the conversation takes a while to answer: a test must wait on the audit's end, not on luck
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                out.map(|text| r::ProbeAnswer { text, cost_usd: None }).map_err(WireError::conflict)
            }
        });
        Ok(())
    }
}

/// Hears each `threads.audited`: the audit runs in the background, and its event is the sign it ended.
struct Watch(tokio::sync::mpsc::UnboundedSender<wisp_threads_api::Audit>);

impl Plugin for Watch {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "test.watch", "version": "1.0.0", "api": "^1", "activation": ["onStartup"] }))
            .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let tx = self.0.clone();
        s.on::<wisp_threads_api::Audited, _, _>(move |_ctx, e| {
            let _ = tx.send(e.audit);
            async {}
        });
        Ok(())
    }
}

struct Fixture {
    k: Kernel,
    rt: Runtime,
    home: std::path::PathBuf,
    audited: tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<wisp_threads_api::Audit>>,
}

impl Fixture {
    /// The next audit to end (its `threads.audited`). The bound only turns a lost event into a failure, not a hang.
    async fn audited(&self) -> wisp_threads_api::Audit {
        let mut rx = self.audited.lock().await;
        tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await.expect("the audit ended").expect("the watch is on")
    }
}

async fn kernel(name: &str) -> Fixture {
    let home = std::env::temp_dir().join(format!("wisp-threads-fresh-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let rt = Runtime::default();
    rt.task("t1", None);
    rt.task("c1", Some("t1"));
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let k = TestApp::new().service(store).service(Paths::new(&home)).with(ThreadsPlugin).with(rt.clone()).with(Watch(tx)).start().await;
    Fixture { k, rt, home, audited: tokio::sync::Mutex::new(rx) }
}

async fn call(k: &Kernel, cmd: &str, args: Value) -> std::result::Result<Value, WireError> {
    k.call(Caller::user(), cmd, args).await
}

async fn set(k: &Kernel, section: &str, text: &str) {
    call(k, "threads.notesUpdate", json!({ "id": "t1", "section": section, "text": text, "by": "agent" })).await.unwrap();
}

fn turn(idx: u32, prompt: &str, answer: &str) -> Turn {
    Turn {
        id: format!("u{idx}").as_str().into(),
        idx,
        prompt: prompt.into(),
        text: Some(answer.into()),
        status: TurnStatus::Done,
        ..Default::default()
    }
}

#[tokio::test]
async fn the_brief_has_the_notes_the_files_changed_and_the_last_turns() {
    let f = kernel("brief").await;
    set(&f.k, "goal", "Import CSV files").await;
    let mut edited = turn(1, "fix the parser", "Fixed; tests pass.");
    edited.ext.insert("wisp.runtime".into(), json!({ "changes": [{ "file": "src/parse.rs", "op": "modify", "edits": [] }] }));
    *f.rt.turns.lock().unwrap() = vec![turn(0, "build an importer", "Built."), edited, turn(2, "and the tests", "Added three.")];
    let b = call(&f.k, "threads.freshBrief", json!({ "id": "t1" })).await.unwrap();
    let text = b["text"].as_str().unwrap();
    assert!(text.starts_with("# Fresh thread") && text.contains("### Goal\nImport CSV files"), "{text}");
    assert!(
        text.contains("`src/parse.rs`") && text.contains("### Turn 2") && text.contains("### Turn 3") && !text.contains("### Turn 1\n"),
        "{text}"
    );
    assert_eq!((b["notesRev"].as_u64(), b["diff"].as_str()), (Some(1), Some("src/parse.rs")));
    assert!(f.rt.follow_ups.lock().unwrap().is_empty(), "a brief starts nothing");
    let _ = std::fs::remove_dir_all(&f.home);
}

#[tokio::test]
async fn fresh_starts_a_new_conversation_from_the_brief_and_marks_its_turn() {
    let f = kernel("start").await;
    set(&f.k, "goal", "Import CSV files").await;
    *f.rt.turns.lock().unwrap() = vec![turn(0, "build an importer", "Built.")];
    let out = call(&f.k, "threads.fresh", json!({ "id": "t1", "brief": "  # My brief\nCarry on with CSV.  " })).await.unwrap();
    assert_eq!(out["turnId"], "u9");
    let sent = f.rt.follow_ups.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].fresh, "a new conversation, not a follow-up in the old one");
    assert_eq!(sent[0].text, "# My brief\nCarry on with CSV.", "your edited brief goes as you wrote it");
    let marks = f.rt.marks.lock().unwrap().clone();
    assert_eq!((marks[0].turn_id.as_str(), marks[0].plugin.as_str(), &marks[0].value["fresh"]), ("u9", "wisp.threads", &json!(true)));
    // without a brief: the one made now
    call(&f.k, "threads.fresh", json!({ "id": "t1" })).await.unwrap();
    assert!(f.rt.follow_ups.lock().unwrap()[1].text.contains("Import CSV files"));
    let e = call(&f.k, "threads.fresh", json!({ "id": "t1", "brief": "  " })).await.unwrap_err();
    assert!(e.message.contains("needs a brief"), "{}", e.message);
    let e = call(&f.k, "threads.fresh", json!({ "id": "c1" })).await.unwrap_err();
    assert!(e.message.contains("lead"), "a subagent's thread is its lead's: {}", e.message);
    assert_eq!(f.rt.follow_ups.lock().unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(&f.home);
}

#[tokio::test]
async fn a_fresh_turn_hears_how_to_keep_the_notes_and_health_counts_from_it() {
    let f = kernel("note").await;
    set(&f.k, "goal", "Import CSV files").await;
    let part = f.k.ctx("wisp.runtime").unwrap().contributions(&PROMPT).into_iter().next().unwrap();
    let task = f.rt.tasks.lock().unwrap()["t1"].clone();
    let mut fresh = Turn { idx: 40, prompt: "# Fresh thread …".into(), ..Default::default() };
    fresh.ext.insert("wisp.runtime".into(), json!({ "fresh": true, "after": "old-conversation" }));
    let note = part.before(TurnNote { session: task, turn: fresh.clone(), tools: Some("mcp__wisp__".into()) }).await;
    assert!(note.contains("mcp__wisp__wisp_notes") && !note.contains("<task-notes"), "the brief has the Notes already: {note}");
    let compact = |idx: u32| Turn {
        id: format!("c{idx}").as_str().into(),
        idx,
        by: Some("compact".into()),
        status: TurnStatus::Done,
        ..Default::default()
    };
    *f.rt.turns.lock().unwrap() = vec![turn(0, "a", "b"), compact(1), compact(2), compact(3), Turn { id: "u4".into(), ..fresh }];
    let h = call(&f.k, "threads.health", json!({ "id": "t1" })).await.unwrap();
    assert_eq!((h["compactions"].as_u64(), h["level"].as_str()), (Some(0), Some("ok")), "the old thread's compactions stay with it: {h}");
    let _ = std::fs::remove_dir_all(&f.home);
}

async fn end(f: &Fixture, turn_id: &str) {
    let e: wisp_runtime_api::events::TurnEnded = serde_json::from_value(
        json!({ "sessionId": "t1", "turnId": turn_id, "status": "done", "interrupted": false, "agent": "claude", "model": "" }),
    )
    .unwrap();
    let ctx = f.k.ctx("wisp.runtime").unwrap();
    ctx.emit(&ctx.root_call(), &e);
    // the turn's end is handled (health, the hook's count, whether to audit); an audit then runs in the background,
    // and a test that wants it waits on its end (`Fixture::audited`)
    f.k.idle().await;
}

fn compacted(id: &str, idx: u32) -> Turn {
    Turn {
        id: id.into(),
        idx,
        status: TurnStatus::Done,
        stats: Some(TurnStats { compactions: Some(1), ..Default::default() }),
        ..Default::default()
    }
}

#[tokio::test]
async fn the_audit_is_off_by_default_and_on_says_what_the_compaction_lost_once() {
    let f = kernel("audit").await;
    set(&f.k, "goal", "Import CSV files").await;
    set(&f.k, "constraints", "No Redis\nKeep the public API").await;
    *f.rt.turns.lock().unwrap() = vec![turn(0, "a", "b"), compacted("u1", 1)];
    *f.rt.answer.lock().unwrap() =
        Some(Ok("- goal: import the CSV files\n- constraints: keep the public API\n- decisions: don't know".into()));
    end(&f, "u1").await;
    assert!(f.rt.probes.lock().unwrap().is_empty(), "off by default: no extra run");
    f.k.set_setting("threads.compactAudit", Some(json!(true))).unwrap();
    // a turn that didn't compact isn't audited
    *f.rt.turns.lock().unwrap() = vec![turn(0, "a", "b"), compacted("u1", 1), turn(2, "c", "d")];
    end(&f, "u2").await;
    assert!(f.rt.probes.lock().unwrap().is_empty());
    end(&f, "u1").await;
    let done = f.audited().await;
    assert_eq!((done.turn.as_str(), done.kept, done.total), ("u1", 2, 3));
    let probes = f.rt.probes.lock().unwrap().clone();
    assert_eq!(probes.len(), 1);
    assert!(probes[0].contains("Do not use tools"), "{}", probes[0]);
    let a = call(&f.k, "threads.audits", json!({ "id": "t1" })).await.unwrap();
    assert_eq!((a[0]["status"].as_str(), a[0]["kept"].as_u64(), a[0]["total"].as_u64()), (Some("done"), Some(2), Some(3)), "{a}");
    assert_eq!(a[0]["lost"], json!(["Constraints: No Redis"]));
    let part = f.k.ctx("wisp.runtime").unwrap().contributions(&PROMPT).into_iter().next().unwrap();
    let task = f.rt.tasks.lock().unwrap()["t1"].clone();
    let q = |idx: u32| TurnNote {
        session: task.clone(),
        turn: Turn { idx, prompt: "go on".into(), ..Default::default() },
        tools: Some("mcp__wisp__".into()),
    };
    let note = part.before(q(3)).await;
    assert!(note.contains("<compaction-lost>") && note.contains("- Constraints: No Redis"), "{note}");
    assert!(!part.before(q(4)).await.contains("<compaction-lost>"), "told once");
    // a copy that can't answer: skipped, with why
    *f.rt.answer.lock().unwrap() = Some(Err("This task's agent can't answer on a copy of its conversation.".into()));
    *f.rt.turns.lock().unwrap() = vec![compacted("u5", 5)];
    end(&f, "u5").await;
    assert_eq!(f.audited().await.status, wisp_threads_api::AuditStatus::Skipped);
    let a = call(&f.k, "threads.audits", json!({ "id": "t1" })).await.unwrap();
    assert_eq!(a.as_array().unwrap().len(), 2);
    let skipped = a.as_array().unwrap().iter().find(|x| x["turn"] == "u5").unwrap();
    assert_eq!(skipped["status"], "skipped");
    assert!(skipped["why"].as_str().unwrap().contains("copy"));
    let _ = std::fs::remove_dir_all(&f.home);
}

/// ADR-0097: a harness whose stream doesn't say it compacted (Codex) counts it through its hook (`wisp hook compacted`
/// lines in the run's folder): the turn keeps the count beside its slot, the file goes, and the audit runs on it.
#[tokio::test]
async fn compactions_a_hook_counted_are_kept_on_the_turn_and_audited() {
    let f = kernel("marked").await;
    set(&f.k, "goal", "Import CSV files").await;
    f.k.set_setting("threads.compactAudit", Some(json!(true))).unwrap();
    let mut t = turn(1, "go on", "ok");
    t.ext.insert("wisp.threads".into(), json!({ "fresh": true }));
    *f.rt.turns.lock().unwrap() = vec![turn(0, "a", "b"), t];
    let dir = f.home.join("threads").join("out").join("t1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("compactions"), "compacted\ncompacted\n").unwrap();
    end(&f, "u1").await;
    let marks = f.rt.marks.lock().unwrap().clone();
    let m = marks.iter().find(|m| m.turn_id.as_str() == "u1").expect("the turn is marked");
    assert_eq!(m.value, json!({ "fresh": true, "compactions": 2 }), "the count beside what the slot had");
    assert!(!dir.join("compactions").exists(), "counted once");
    assert_eq!(f.audited().await.turn, "u1", "a counted compaction is audited");
    assert_eq!(f.rt.probes.lock().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&f.home);
}

/// ADR-0097: a formula's task starts with the Notes its spec declares, written as yours; a task that has Notes keeps
/// them; a spec without notes writes nothing.
#[tokio::test]
async fn a_task_made_with_notes_in_its_spec_starts_with_them() {
    let f = kernel("seed").await;
    let created = |f: &Fixture| {
        let e = wisp_runtime_api::events::SessionCreated {
            session: wisp_runtime_api::model::Summary { id: "t1".into(), ..Default::default() },
        };
        let ctx = f.k.ctx("wisp.runtime").unwrap();
        ctx.emit(&ctx.root_call(), &e);
    };
    created(&f);
    f.k.idle().await;
    let n = call(&f.k, "threads.notes", json!({ "id": "t1" })).await.unwrap();
    assert_eq!(n["rev"], 0, "no notes in the spec: nothing");
    f.rt.spec.lock().unwrap().notes = Some(json!({ "goal": "Ship the importer", "constraints": ["No Redis", " ", "Keep the API"] }));
    created(&f);
    f.k.idle().await;
    let n = call(&f.k, "threads.notes", json!({ "id": "t1" })).await.unwrap();
    let text = |id: &str| n["sections"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap().clone();
    assert_eq!((text("goal")["text"].as_str(), text("goal")["by"].as_str()), (Some("Ship the importer"), Some("you")));
    assert_eq!(text("constraints")["text"], "No Redis\nKeep the API");
    assert_eq!(n["rev"], 2);
    // made again (an event delivered twice): the Notes it has stay
    f.rt.spec.lock().unwrap().notes = Some(json!({ "goal": "Something else" }));
    created(&f);
    f.k.idle().await;
    let n = call(&f.k, "threads.notes", json!({ "id": "t1" })).await.unwrap();
    assert_eq!(n["rev"], 2);
    let _ = std::fs::remove_dir_all(&f.home);
}
