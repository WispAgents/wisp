//! ADR-0079 through wisp.threads' manifest (default deny): a task's Notes are rewritten a section at a time with every
//! version kept and who wrote it; a subagent reads its lead's Notes and can't write them; the note before a follow-up
//! carries them and what you changed; health counts compactions and context; a deleted task leaves nothing behind.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::services::Paths;
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup, WireError};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::events::SessionDeleted;
use wisp_runtime_api::model::{Session, Turn, TurnStats};
use wisp_runtime_api::points::{OFFLOAD_DIR_ENV, PROMPT, RUN, TurnNote};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;
use wisp_threads::ThreadsPlugin;

/// wisp.runtime as far as wisp.threads asks it: tasks by id (a subagent names its lead), and a task's turns.
#[derive(Clone, Default)]
struct Runtime {
    tasks: Arc<Mutex<BTreeMap<String, Session>>>,
    turns: Arc<Mutex<Vec<Turn>>>,
}

impl Runtime {
    fn task(&self, id: &str, lead: Option<&str>) {
        let s = Session { id: id.into(), parent_id: lead.map(Into::into), title: format!("task {id}"), ..Default::default() };
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
        Ok(())
    }
}

struct Fixture {
    k: Kernel,
    rt: Runtime,
    home: std::path::PathBuf,
    /// the kernel's store, to count what a turn's end wrote
    store: Store,
}

async fn kernel(name: &str) -> Fixture {
    let home = std::env::temp_dir().join(format!("wisp-threads-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let rt = Runtime::default();
    rt.task("t1", None);
    rt.task("c1", Some("t1"));
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new().service(store.clone()).service(Paths::new(&home)).with(ThreadsPlugin).with(rt.clone()).start().await;
    Fixture { k, rt, home, store }
}

async fn set(k: &Kernel, id: &str, section: &str, text: &str, by: &str) -> std::result::Result<Value, WireError> {
    k.call(Caller::user(), "threads.notesUpdate", json!({ "id": id, "section": section, "text": text, "by": by })).await
}

fn text<'a>(n: &'a Value, section: &str) -> &'a str {
    n["sections"].as_array().unwrap().iter().find(|s| s["id"] == section).and_then(|s| s["text"].as_str()).unwrap_or("")
}

#[tokio::test]
async fn a_section_is_rewritten_whole_with_each_version_kept_and_who_wrote_it() {
    let f = kernel("rewrite").await;
    let n = set(&f.k, "t1", "goal", "Ship the importer", "agent").await.unwrap();
    assert_eq!((text(&n, "goal"), n["rev"].as_u64()), ("Ship the importer", Some(1)));
    let n = set(&f.k, "t1", "constraints", "- No Redis\n- No Redis\n2. Keep the API", "agent").await.unwrap();
    assert_eq!(text(&n, "constraints"), "No Redis\nKeep the API", "one item a line, marks and repeats gone");
    // the user changes their mind: the section is replaced, never appended to
    let n = set(&f.k, "t1", "constraints", "Redis is fine", "you").await.unwrap();
    assert_eq!(text(&n, "constraints"), "Redis is fine");
    assert_eq!(n["rev"].as_u64(), Some(3));
    // the same text again writes nothing
    let n = set(&f.k, "t1", "constraints", "Redis is fine", "you").await.unwrap();
    assert_eq!(n["rev"].as_u64(), Some(3), "unchanged text is not a new version");
    let revs = f.k.call(Caller::user(), "threads.revisions", json!({ "id": "t1", "section": "constraints" })).await.unwrap();
    let revs = revs.as_array().unwrap();
    assert_eq!(revs.len(), 2, "{revs:?}");
    let by: Vec<&str> = revs.iter().map(|r| r["by"].as_str().unwrap()).collect();
    assert!(by.contains(&"agent") && by.contains(&"you"), "{by:?}");
    let e = set(&f.k, "t1", "mood", "fine", "you").await.unwrap_err();
    assert!(e.message.contains("goal"), "an unknown section names the real ones: {}", e.message);
    let _ = std::fs::remove_dir_all(&f.home);
}

#[tokio::test]
async fn a_subagent_reads_its_leads_notes_and_cant_change_them() {
    let f = kernel("lead").await;
    set(&f.k, "t1", "decisions", "Use SQLite: one file, no server", "agent").await.unwrap();
    let n = f.k.call(Caller::user(), "threads.notes", json!({ "id": "c1" })).await.unwrap();
    assert_eq!(n["lead"], "t1");
    assert_eq!(text(&n, "decisions"), "Use SQLite: one file, no server");
    let e = set(&f.k, "c1", "decisions", "Use Postgres", "agent").await.unwrap_err();
    assert!(e.message.contains("lead"), "{}", e.message);
    let n = f.k.call(Caller::user(), "threads.notes", json!({ "id": "t1" })).await.unwrap();
    assert_eq!(text(&n, "decisions"), "Use SQLite: one file, no server");
    // and its run is told the lead's decisions before its task
    let ctx = f.k.ctx("wisp.runtime").unwrap();
    let parts = ctx.contributions(&PROMPT);
    let child = f.rt.tasks.lock().unwrap()["c1"].clone();
    let note = parts[0].before(TurnNote { session: child, turn: Turn::default(), tools: Some("mcp__wisp__".into()) }).await;
    assert!(note.contains("<lead-notes>") && note.contains("SQLite"), "{note}");
    let _ = std::fs::remove_dir_all(&f.home);
}

#[tokio::test]
async fn the_note_before_a_follow_up_carries_the_notes_your_edit_and_the_take_back_nudge() {
    let f = kernel("note").await;
    let ctx = f.k.ctx("wisp.runtime").unwrap();
    let part = ctx.contributions(&PROMPT).into_iter().next().unwrap();
    let task = f.rt.tasks.lock().unwrap()["t1"].clone();
    let q = |idx: u32, prompt: &str, tools: Option<&str>| TurnNote {
        session: task.clone(),
        turn: Turn { idx, prompt: prompt.into(), ..Default::default() },
        tools: tools.map(Into::into),
    };
    let first = part.before(q(0, "build the importer", Some("mcp__wisp__"))).await;
    assert!(first.contains("mcp__wisp__wisp_notes"), "the first turn is told how to keep them: {first}");
    assert_eq!(part.before(q(3, "go on", None)).await, "", "a run without wisp's tools gets no note");
    set(&f.k, "t1", "goal", "Import CSV files", "agent").await.unwrap();
    set(&f.k, "t1", "constraints", "No new dependencies", "you").await.unwrap();
    let note = part.before(q(3, "actually, forget the CSV part, use JSON", Some("mcp__wisp__"))).await;
    assert!(note.contains("<task-notes rev=\"2\">") && note.contains("Import CSV files"), "{note}");
    assert!(note.contains("The user edited them"), "your edit since the agent last read them: {note}");
    assert!(note.contains("change something said earlier"), "the take-back nudge: {note}");
    // the agent has now read your edit: the next note doesn't say it again
    let next = part.before(q(4, "go on", Some("mcp__wisp__"))).await;
    assert!(!next.contains("The user edited them"), "{next}");
    let _ = std::fs::remove_dir_all(&f.home);
}

#[tokio::test]
async fn health_counts_the_threads_context_and_compactions() {
    let f = kernel("health").await;
    let turn = |idx: u32, by: Option<&str>, context: Option<(u64, u64)>| Turn {
        id: format!("u{idx}").as_str().into(),
        idx,
        by: by.map(Into::into),
        stats: context.map(|(c, w)| TurnStats { context_tokens: Some(c), context_window: Some(w), ..Default::default() }),
        ..Default::default()
    };
    *f.rt.turns.lock().unwrap() = vec![turn(0, None, Some((10_000, 200_000)))];
    let h = f.k.call(Caller::user(), "threads.health", json!({ "id": "t1" })).await.unwrap();
    assert_eq!(h["level"], "ok", "{h}");
    *f.rt.turns.lock().unwrap() = vec![
        turn(0, None, None),
        turn(1, Some("compact"), None),
        turn(2, None, None),
        turn(3, Some("compact"), None),
        turn(4, Some("compact"), None),
        turn(5, None, Some((170_000, 200_000))),
    ];
    let ended: wisp_runtime_api::events::TurnEnded = serde_json::from_value(
        json!({ "sessionId": "t1", "turnId": "u5", "status": "done", "interrupted": false, "agent": "claude", "model": "" }),
    )
    .unwrap();
    let ctx = f.k.ctx("wisp.runtime").unwrap();
    ctx.emit(&ctx.root_call(), &ended);
    f.k.idle().await;
    let h = f.k.call(Caller::user(), "threads.health", json!({ "id": "t1" })).await.unwrap();
    assert_eq!(h["level"], "worn", "{h}");
    assert_eq!(h["compactions"], 3);
    let reasons = h["reasons"].to_string();
    assert!(reasons.contains("85%") && reasons.contains("compacted 3"), "{reasons}");
    let _ = std::fs::remove_dir_all(&f.home);
}

#[tokio::test]
async fn a_run_gets_its_offload_folder_and_a_deleted_task_leaves_nothing() {
    let f = kernel("forget").await;
    set(&f.k, "t1", "goal", "Ship", "agent").await.unwrap();
    let ctx = f.k.ctx("wisp.runtime").unwrap();
    let task = f.rt.tasks.lock().unwrap()["t1"].clone();
    let env = ctx.contributions(&RUN).iter().flat_map(|p| p.env(&task)).collect::<BTreeMap<_, _>>();
    let dir = std::path::PathBuf::from(&env[OFFLOAD_DIR_ENV]);
    assert!(dir.starts_with(&f.home), "under wisp's home, not the OS temp folder: {}", dir.display());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("out-1.txt"), "long").unwrap();
    ctx.emit(&ctx.root_call(), &SessionDeleted { id: "t1".into() });
    f.k.idle().await;
    assert!(!dir.exists(), "the task's long output goes with it");
    let n = f.k.call(Caller::user(), "threads.notes", json!({ "id": "t1" })).await.unwrap();
    assert_eq!((text(&n, "goal"), n["rev"].as_u64()), ("", Some(0)), "{n}");
    let _ = std::fs::remove_dir_all(&f.home);
}

/// Default deny: a caller whose set doesn't name threads:write can't change a task's Notes; a task's agent can.
#[tokio::test]
async fn writing_notes_needs_the_permission() {
    let f = kernel("deny").await;
    let args = json!({ "id": "t1", "section": "goal", "text": "x" });
    let e = f.k.call(Caller::set("pane:wisp-testkit-nobody"), "threads.notesUpdate", args.clone()).await.unwrap_err();
    assert_eq!(e.code, wisp_kernel::Code::Forbidden, "{}", e.message);
    f.k.call(Caller::set("agent:session"), "threads.notesUpdate", args).await.unwrap();
    let _ = std::fs::remove_dir_all(&f.home);
}

/// ADR-0100: a turn's end of a healthy thread writes no row of wisp.threads (no per-turn count, no Fine health row); a
/// worn thread's health is written once, and again only when what it shows changes, not when only its counts move.
#[tokio::test]
async fn a_turns_end_writes_nothing_for_a_healthy_thread_and_health_only_when_it_shows_something_new() {
    let f = kernel("quiet").await;
    let rows = |kind: &'static str| {
        let st = f.store.clone();
        async move { st.read_async(move |r| r.query(kind, &wisp_store::Query::new())).await.unwrap() }
    };
    let turn = |idx: u32, context: Option<(u64, u64)>| Turn {
        id: format!("u{idx}").as_str().into(),
        idx,
        stats: context.map(|(c, w)| TurnStats { context_tokens: Some(c), context_window: Some(w), ..Default::default() }),
        ..Default::default()
    };
    let end = |turn: &str| {
        let e: wisp_runtime_api::events::TurnEnded = serde_json::from_value(
            json!({ "sessionId": "t1", "turnId": turn, "status": "done", "interrupted": false, "agent": "claude", "model": "" }),
        )
        .unwrap();
        let ctx = f.k.ctx("wisp.runtime").unwrap();
        ctx.emit(&ctx.root_call(), &e);
    };
    for i in 0..3 {
        f.rt.turns.lock().unwrap().push(turn(i, Some((10_000, 200_000))));
        end(&format!("u{i}"));
        f.k.idle().await;
    }
    assert!(rows("threads_meta").await.is_empty(), "no per-turn count");
    assert!(rows("threads_health").await.is_empty(), "Fine: nothing to say");
    // worn: written once
    f.rt.turns.lock().unwrap().push(turn(3, Some((170_000, 200_000))));
    end("u3");
    f.k.idle().await;
    let kept = rows("threads_health").await;
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0]["body"]["level"], "heavy", "{kept:?}");
    // the same reasons a turn later: the row isn't written again (its turns count moved, what it shows didn't)
    f.rt.turns.lock().unwrap().push(turn(4, Some((170_000, 200_000))));
    end("u4");
    f.k.idle().await;
    assert_eq!(rows("threads_health").await[0]["body"]["turns"], 4, "kept as it was written");
    // the command answers with the counts now
    let h = f.k.call(Caller::user(), "threads.health", json!({ "id": "t1" })).await.unwrap();
    assert_eq!(h["turns"], 5);
    // what it shows changes: written
    f.rt.turns.lock().unwrap().push(turn(5, Some((190_000, 200_000))));
    end("u5");
    f.k.idle().await;
    assert_eq!(rows("threads_health").await[0]["body"]["reasons"], json!(["context 95% full"]));
    // Notes written now are counted from the task's turns at the write (6), so they're 0 turns old
    f.k.call(Caller::user(), "threads.notesUpdate", json!({ "id": "t1", "section": "goal", "text": "Ship", "by": "agent" })).await.unwrap();
    let h = f.k.call(Caller::user(), "threads.health", json!({ "id": "t1" })).await.unwrap();
    assert_eq!(h["notesAge"], 0, "{h}");
    let _ = std::fs::remove_dir_all(&f.home);
}
