//! Durable events (ADR-0044) with a real store under the kernel: appended in the command's transaction,
//! replayed from a cursor, effectively once, deduped, trimmed, parked and retried.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wisp_kernel::durable::{Durable, EventLog, EventLogService};
use wisp_kernel::{App, Caller, Command, Event, Kernel, Manifest, Plugin, PluginGroup, Result, Setup, WireError};
use wisp_store::{Col, EntityDef, Location, ManualClock, Migration, Row, Store};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Made {
    n: u32,
    #[serde(default)]
    key: Option<String>,
}
impl Event for Made {
    const TYPE: &'static str = "src.made";
    const DURABLE: bool = true;
    fn key(&self) -> Option<String> {
        self.key.clone()
    }
    fn subject(&self) -> Option<String> {
        Some(format!("n{}", self.n))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MakeArgs {
    n: u32,
    #[serde(default)]
    count: u32,
    #[serde(default)]
    key: Option<String>,
    /// fail the write (rolled back)
    #[serde(default)]
    fail_write: bool,
    /// write nothing
    #[serde(default)]
    no_write: bool,
}

struct MakeCmd;
impl Command for MakeCmd {
    const ID: &'static str = "src.make";
    type Args = MakeArgs;
    type Out = ();
}

struct RetryCmd;
impl Command for RetryCmd {
    const ID: &'static str = "sink.retry";
    type Args = u64;
    type Out = usize;
}

fn on_notes(_: &Row) -> Vec<String> {
    vec!["plugin:wisp.src:notes".into()]
}

const NOTE: EntityDef = EntityDef {
    kind: "src_note",
    table: "src_note",
    key: "id",
    columns: &[("id", Col::Text), ("body", Col::Text)],
    json_columns: &[],
    parent: None,
    seq: None,
    topics: on_notes,
};

fn note(id: &str) -> Row {
    let mut r = Row::new();
    r.insert("id".into(), json!(id));
    r.insert("body".into(), json!("x"));
    r
}

struct P {
    id: &'static str,
    f: Arc<dyn Fn(&mut Setup) + Send + Sync>,
}

impl Plugin for P {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": self.id, "version": "1.0.0", "api": "^1", "permissions": ["src:make"] }))
            .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        (self.f)(s);
        Ok(())
    }
}

fn store_of(k: &wisp_kernel::Ctx) -> Store {
    k.service::<Store>().unwrap().as_ref().clone()
}

/// The source: `src.make` emits `count` (or 1) durable events, then writes a note (or not, or a failing write).
fn source() -> P {
    P {
        id: "wisp.src",
        f: Arc::new(|s| {
            s.command::<MakeCmd, _, _>(|ctx, call, a| async move {
                for i in 0..a.count.max(1) {
                    ctx.emit(&call, &Made { n: a.n + i, key: a.key.clone() });
                }
                if a.no_write {
                    return Ok(());
                }
                let st = store_of(&ctx);
                let fail = a.fail_write;
                let id = format!("made-{}", a.n);
                st.write(move |w| {
                    w.upsert(NOTE.kind, note(&id))?;
                    if fail {
                        return Err(wisp_store::StoreError::Internal("injected".into()));
                    }
                    Ok(())
                })
                .await
                .map_err(|e| WireError::internal(e.to_string()))
            });
        }),
    }
}

#[derive(Default)]
struct Seen {
    got: Mutex<Vec<u32>>,
    depths: Mutex<Vec<u32>>,
    gaps: Mutex<Vec<(u64, u64)>>,
    tries: AtomicU32,
}

/// The sink: a durable subscription that records what it saw and writes one effect row per event.
fn sink(seen: Arc<Seen>, mode: &'static str) -> P {
    P {
        id: "wisp.sink",
        f: Arc::new(move |s| {
            let seen = seen.clone();
            s.on_durable::<Made, _, _>("count", move |ctx, call, d| {
                let seen = seen.clone();
                async move {
                    match d {
                        Durable::Gap { from, to } => {
                            seen.gaps.lock().unwrap().push((from, to));
                            Ok(())
                        }
                        Durable::Event { event, seq, .. } => {
                            let t = seen.tries.fetch_add(1, Ordering::SeqCst);
                            match mode {
                                "fail-write-once" if t == 0 => {
                                    // the effect and the cursor roll back together
                                    let st = store_of(&ctx);
                                    let r = st
                                        .write(move |w| {
                                            w.upsert(NOTE.kind, note(&format!("effect-{seq}")))?;
                                            Err::<(), _>(wisp_store::StoreError::Internal("crash before commit".into()))
                                        })
                                        .await;
                                    return r.map_err(|e| WireError::internal(e.to_string()));
                                }
                                "panic-after-write" => {
                                    let st = store_of(&ctx);
                                    st.write(move |w| w.upsert(NOTE.kind, note(&format!("effect-{seq}"))).map(|_| ())).await.unwrap();
                                    panic!("after the write");
                                }
                                "always-fail" => return Err(WireError::internal("no")),
                                "chain" => {
                                    seen.depths.lock().unwrap().push(call.cause_depth());
                                    seen.got.lock().unwrap().push(event.n);
                                    if event.n < 5 {
                                        ctx.run_json(&call, "src.make", json!({ "n": event.n + 1, "noWrite": true, "no_write": true }))
                                            .await?;
                                    }
                                    return Ok(());
                                }
                                _ => {}
                            }
                            let st = store_of(&ctx);
                            st.write(move |w| w.upsert(NOTE.kind, note(&format!("effect-{seq}"))).map(|_| ()))
                                .await
                                .map_err(|e| WireError::internal(e.to_string()))?;
                            seen.got.lock().unwrap().push(event.n);
                            Ok(())
                        }
                    }
                }
            });
            s.command::<RetryCmd, _, _>(|ctx, _call, seq| async move { ctx.retry_event(seq).await });
        }),
    }
}

/// An in-process (ephemeral-path) subscriber to the same type.
fn live(got: Arc<Mutex<Vec<u32>>>) -> P {
    P {
        id: "wisp.live",
        f: Arc::new(move |s| {
            let got = got.clone();
            s.on::<Made, _, _>(move |_, e| {
                let got = got.clone();
                async move { got.lock().unwrap().push(e.n) }
            });
        }),
    }
}

fn open(path: &Path) -> Store {
    let now = i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()).unwrap();
    let st = Store::open(Location::Path(path.to_path_buf()), Arc::new(ManualClock::new(now))).unwrap();
    st.migrate("src", &[Migration::sql(1, "CREATE TABLE src_note (id TEXT PRIMARY KEY, body TEXT) STRICT;")]).unwrap();
    st.define_entity(NOTE).unwrap();
    st
}

async fn kernel(st: &Store, plugins: Vec<P>) -> Kernel {
    let mut g = PluginGroup::new();
    for p in plugins {
        g = g.add(p);
    }
    App::new().service(st.clone()).service(EventLogService(Arc::new(st.clone()))).plugins(g).start().await.unwrap()
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "wisp-durable-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d.join("wisp.db")
}

async fn make(k: &Kernel, args: Value) -> std::result::Result<Value, WireError> {
    k.call(Caller::user(), "src.make", args).await
}

fn notes(st: &Store) -> Vec<String> {
    st.read(|r| {
        Ok(r.query(NOTE.kind, &wisp_store::Query::new())?
            .iter()
            .filter_map(|x| x.get("id").and_then(Value::as_str).map(str::to_owned))
            .collect::<Vec<_>>())
    })
    .unwrap()
}

async fn settle(k: &Kernel) {
    tokio::time::timeout(Duration::from_secs(20), k.idle()).await.expect("settles");
}

#[tokio::test(flavor = "multi_thread")]
async fn durable_event_rolled_back_with_its_command_is_never_seen() {
    let path = temp("rollback");
    let st = open(&path);
    let seen = Arc::new(Seen::default());
    let live_got = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&st, vec![source(), sink(seen.clone(), ""), live(live_got.clone())]).await;
    assert!(make(&k, json!({ "n": 1, "fail_write": true })).await.is_err());
    make(&k, json!({ "n": 2 })).await.unwrap();
    settle(&k).await;
    let rows = EventLog::read(&st, 0, 100).await.unwrap();
    assert_eq!(rows.iter().map(|r| r.event.data["n"].as_u64().unwrap()).collect::<Vec<_>>(), [2], "the rolled-back one isn't in the log");
    assert_eq!(*seen.got.lock().unwrap(), [2]);
    assert_eq!(*live_got.lock().unwrap(), [2]);
    // in the command's own transaction: the note and the event together
    assert!(notes(&st).contains(&"made-2".to_owned()) && !notes(&st).contains(&"made-1".to_owned()));
    assert_eq!(rows[0].event.subject.as_deref(), Some("n2"));
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_replays_from_cursor_without_loss_or_duplicate_effect() {
    let path = temp("restart");
    let seen = Arc::new(Seen::default());
    {
        let st = open(&path);
        let k = kernel(&st, vec![source(), sink(seen.clone(), "")]).await;
        make(&k, json!({ "n": 1 })).await.unwrap();
        settle(&k).await;
        assert_eq!(*seen.got.lock().unwrap(), [1]);
    }
    {
        // the sink isn't here (switched off, or not yet started): events go on being made
        let st = open(&path);
        let k = kernel(&st, vec![source()]).await;
        make(&k, json!({ "n": 2 })).await.unwrap();
        make(&k, json!({ "n": 3, "no_write": true })).await.unwrap();
        settle(&k).await;
    }
    let st = open(&path);
    let again = Arc::new(Seen::default());
    let k = kernel(&st, vec![source(), sink(again.clone(), "")]).await;
    settle(&k).await;
    assert_eq!(*again.got.lock().unwrap(), [2, 3], "replayed from its cursor: not 1 again, and nothing lost");
    let effects: Vec<String> = notes(&st).into_iter().filter(|n| n.starts_with("effect-")).collect();
    assert_eq!(effects.len(), 3, "{effects:?}");
    assert_eq!(st.cursor("wisp.sink", "count").await.unwrap(), Some(3));
}

#[tokio::test(flavor = "multi_thread")]
async fn crash_between_handler_write_and_cursor_cannot_happen() {
    // the handler's write fails before commit: its effect and its cursor roll back together, and the retry does both
    let st = open(&temp("crash-a"));
    let seen = Arc::new(Seen::default());
    let k = kernel(&st, vec![source(), sink(seen.clone(), "fail-write-once")]).await;
    make(&k, json!({ "n": 7, "no_write": true })).await.unwrap();
    settle(&k).await;
    assert_eq!(*seen.got.lock().unwrap(), [7]);
    assert_eq!(notes(&st).iter().filter(|n| n.starts_with("effect-")).count(), 1);
    assert_eq!(st.cursor("wisp.sink", "count").await.unwrap(), Some(1));
    drop(k);
    // the handler crashes after its write committed: the cursor rode on it, so the effect isn't repeated
    let st = open(&temp("crash-b"));
    let seen = Arc::new(Seen::default());
    let k = kernel(&st, vec![source(), sink(seen.clone(), "panic-after-write")]).await;
    make(&k, json!({ "n": 8, "no_write": true })).await.unwrap();
    settle(&k).await;
    assert_eq!(seen.tries.load(Ordering::SeqCst), 1, "no retry after the write committed");
    assert_eq!(notes(&st).iter().filter(|n| n.starts_with("effect-")).count(), 1);
    assert_eq!(st.cursor("wisp.sink", "count").await.unwrap(), Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn lagging_durable_subscriber_reads_the_table_then_rejoins_live_in_order() {
    let path = temp("lag");
    let n = u32::try_from(wisp_kernel::durable::RING).unwrap() + 500;
    {
        let st = open(&path);
        let k = kernel(&st, vec![source()]).await;
        // a subscriber that knew the log before this burst: its cursor is at 0
        st.set_cursor("wisp.sink", "count", 0).await.unwrap();
        make(&k, json!({ "n": 0, "count": n, "no_write": true })).await.unwrap();
        settle(&k).await;
    }
    let st = open(&path);
    let seen = Arc::new(Seen::default());
    let k = kernel(&st, vec![source(), sink(seen.clone(), "")]).await;
    settle(&k).await;
    make(&k, json!({ "n": 100_000, "no_write": true })).await.unwrap();
    settle(&k).await;
    let got = seen.got.lock().unwrap().clone();
    assert_eq!(got.len(), n as usize + 1);
    assert!(got[..n as usize].iter().enumerate().all(|(i, x)| *x == u32::try_from(i).unwrap()), "in order from the table");
    assert_eq!(*got.last().unwrap(), 100_000, "then live");
}

#[tokio::test(flavor = "multi_thread")]
async fn cursor_past_retention_gets_gap_and_resyncs_state() {
    let path = temp("gap");
    let st = open(&path);
    st.set_cursor("wisp.sink", "count", 0).await.unwrap();
    {
        let k = kernel(&st, vec![source()]).await;
        make(&k, json!({ "n": 1, "count": 5, "no_write": true })).await.unwrap();
        settle(&k).await;
    }
    // only the newest 2 are kept
    st.trim_events(i64::MAX, 2).await.unwrap();
    let seen = Arc::new(Seen::default());
    let k = kernel(&st, vec![source(), sink(seen.clone(), "")]).await;
    settle(&k).await;
    assert_eq!(*seen.gaps.lock().unwrap(), [(1, 3)], "once, for what's gone");
    assert_eq!(*seen.got.lock().unwrap(), [4, 5]);
    assert!(k.failures("wisp.sink").iter().any(|f| f.code == "gap"));
}

#[tokio::test(flavor = "multi_thread")]
async fn same_type_and_key_within_24h_recorded_once() {
    let st = open(&temp("dedupe"));
    let live_got = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&st, vec![source(), live(live_got.clone())]).await;
    make(&k, json!({ "n": 1, "key": "pr-42" })).await.unwrap();
    make(&k, json!({ "n": 2, "key": "pr-42" })).await.unwrap();
    make(&k, json!({ "n": 3, "key": "pr-43", "no_write": true })).await.unwrap();
    settle(&k).await;
    let rows = EventLog::read(&st, 0, 10).await.unwrap();
    assert_eq!(rows.iter().map(|r| r.event.key.clone().unwrap()).collect::<Vec<_>>(), ["pr-42", "pr-43"]);
    assert_eq!(*live_got.lock().unwrap(), [1, 3], "a repeat isn't published either");
    // a store opened again remembers the last day's keys
    let path = st.home().unwrap().join("wisp.db");
    drop(k);
    drop(st);
    let st = open(&path);
    let k = kernel(&st, vec![source()]).await;
    make(&k, json!({ "n": 4, "key": "pr-42", "no_write": true })).await.unwrap();
    settle(&k).await;
    assert_eq!(EventLog::read(&st, 0, 10).await.unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn causation_chain_depth_cuts_trigger_loops_at_3() {
    let st = open(&temp("cause"));
    let seen = Arc::new(Seen::default());
    let k = kernel(&st, vec![source(), sink(seen.clone(), "chain")]).await;
    make(&k, json!({ "n": 1, "no_write": true })).await.unwrap();
    settle(&k).await;
    assert_eq!(*seen.got.lock().unwrap(), [1, 2, 3, 4, 5]);
    // each handler's call knows how deep it is; what a trigger refuses past 3 (ADR-0043)
    assert_eq!(*seen.depths.lock().unwrap(), [1, 2, 3, 4, 5]);
    let rows = EventLog::read(&st, 0, 10).await.unwrap();
    assert_eq!(rows[0].event.cause, None);
    assert_eq!(rows[1].event.cause, Some(rows[0].seq));
    assert_eq!(rows[4].event.cause, Some(rows[3].seq));
}

#[tokio::test(flavor = "multi_thread")]
async fn plugin_switched_off_keeps_its_cursor_and_resumes_on_again() {
    let st = open(&temp("off"));
    let seen = Arc::new(Seen::default());
    let k = kernel(&st, vec![source(), sink(seen.clone(), "")]).await;
    make(&k, json!({ "n": 1 })).await.unwrap();
    settle(&k).await;
    k.set_enabled("wisp.sink", false).await.unwrap();
    make(&k, json!({ "n": 2 })).await.unwrap();
    make(&k, json!({ "n": 3 })).await.unwrap();
    settle(&k).await;
    assert_eq!(*seen.got.lock().unwrap(), [1]);
    k.set_enabled("wisp.sink", true).await.unwrap();
    settle(&k).await;
    assert_eq!(*seen.got.lock().unwrap(), [1, 2, 3]);
}

#[tokio::test(flavor = "multi_thread")]
async fn handler_panic_retried_3_times_then_parked_with_reason() {
    let st = open(&temp("park"));
    let seen = Arc::new(Seen::default());
    let k = kernel(&st, vec![source(), sink(seen.clone(), "always-fail")]).await;
    make(&k, json!({ "n": 1, "no_write": true })).await.unwrap();
    make(&k, json!({ "n": 2, "no_write": true })).await.unwrap();
    settle(&k).await;
    assert_eq!(seen.tries.load(Ordering::SeqCst), 6, "3 tries each");
    let parked = st.parked().await.unwrap();
    assert_eq!(parked.iter().map(|p| (p.seq, p.why.as_str())).collect::<Vec<_>>(), [(1, "no"), (2, "no")]);
    // the cursor moved on: the next one isn't held up
    assert_eq!(st.cursor("wisp.sink", "count").await.unwrap(), Some(2));
    // retry delivers it again (and fails again: parked again)
    let n = k.call(Caller::plugin("wisp.sink"), "sink.retry", json!(1)).await.unwrap();
    settle(&k).await;
    assert_eq!(n, json!(1));
    assert_eq!(seen.tries.load(Ordering::SeqCst), 9);
    assert_eq!(st.parked().await.unwrap().len(), 2);
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EndArgs {
    n: u32,
    /// the old way: write, then emit once the write returned (ADR-0073's gap)
    #[serde(default)]
    after: bool,
    #[serde(default)]
    fail_write: bool,
}
struct EndCmd;
impl Command for EndCmd {
    const ID: &'static str = "src.end";
    type Args = EndArgs;
    type Out = ();
}
struct CountCmd;
impl Command for CountCmd {
    const ID: &'static str = "live.count";
    type Args = ();
    type Out = usize;
}

/// A writer outside a command's own events (a turn's runner): its row and its durable event, in an outbox.
fn ender() -> P {
    P {
        id: "wisp.src",
        f: Arc::new(|s| {
            s.command::<EndCmd, _, _>(|ctx, _call, a| async move {
                let st = store_of(&ctx);
                let (id, fail) = (format!("end-{}", a.n), a.fail_write);
                let job = move |w: &mut wisp_store::Writer<'_>| {
                    w.upsert(NOTE.kind, note(&id))?;
                    if fail {
                        return Err(wisp_store::StoreError::Internal("injected".into()));
                    }
                    Ok(())
                };
                let r = if a.after {
                    let r = st.write(job).await;
                    if r.is_ok() {
                        ctx.emit(&ctx.root_call(), &Made { n: a.n, key: None });
                    }
                    r
                } else {
                    let ob = ctx.outbox();
                    ob.emit(&Made { n: a.n, key: None });
                    st.write_with(ob, job).await
                };
                r.map_err(|e| WireError::internal(e.to_string()))
            });
        }),
    }
}

/// An in-process subscriber whose command answers from what it heard, after its handlers took what came before
/// (ADR-0073), as budget.compare does.
fn counter(got: Arc<Mutex<Vec<u32>>>) -> P {
    P {
        id: "wisp.live",
        f: Arc::new(move |s| {
            let g = got.clone();
            s.on::<Made, _, _>(move |_, e| {
                let got = g.clone();
                async move { got.lock().unwrap().push(e.n) }
            });
            let g = got.clone();
            s.command::<CountCmd, _, _>(move |ctx, _, ()| {
                let got = g.clone();
                async move {
                    ctx.events_delivered(Duration::from_secs(10)).await;
                    Ok(got.lock().unwrap().len())
                }
            });
        }),
    }
}

/// ADR-0074: a reader who can see a write's rows finds its events queued. The store's commit listener is the reader
/// (writer thread, right after COMMIT: the row is visible, the write hasn't returned to its caller yet); it asks a
/// command that answers after its handlers took what came before. Written the old way (write, then emit), the event
/// doesn't exist yet and the answer misses it (deterministically: the emit can't run before the listener returns);
/// through an outbox it's in the answer, its durable row is in the log, and a rolled-back write's event is never
/// heard of.
#[tokio::test(flavor = "multi_thread")]
async fn a_reader_who_sees_a_writes_row_finds_its_event_already_queued() {
    let path = temp("outbox");
    let st = open(&path);
    let got = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&st, vec![ender(), counter(got.clone())]).await;
    let kk = Arc::new(Mutex::new(Some(k.clone())));
    let answers = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let (a2, handle) = (answers.clone(), tokio::runtime::Handle::current());
    let _sub = st.on_commit(move |rows| {
        let Some(row) = rows.iter().find(|r| r.entity == NOTE.kind && r.id.starts_with("end-")) else { return };
        let Some(k) = kk.lock().unwrap().clone() else { return };
        let r = handle.block_on(k.call(Caller::user(), "live.count", json!(null))).unwrap();
        a2.lock().unwrap().push((row.id.clone(), r));
    });
    k.call(Caller::user(), "src.end", json!({ "n": 1, "after": true })).await.unwrap();
    settle(&k).await;
    k.call(Caller::user(), "src.end", json!({ "n": 2 })).await.unwrap();
    settle(&k).await;
    assert!(k.call(Caller::user(), "src.end", json!({ "n": 3, "fail_write": true })).await.is_err());
    settle(&k).await;
    assert_eq!(
        *answers.lock().unwrap(),
        vec![("end-1".to_owned(), json!(0)), ("end-2".to_owned(), json!(2))],
        "the old way the reader misses its own write's event; through the outbox it's there"
    );
    assert_eq!(*got.lock().unwrap(), vec![1, 2], "3 rolled back: never heard of");
    let log: Vec<u32> =
        EventLog::read(&st, 0, 100).await.unwrap().iter().map(|e| u32::try_from(e.event.data["n"].as_u64().unwrap()).unwrap()).collect();
    assert_eq!(log, vec![1, 2], "the durable row went in with the write; the rolled-back one didn't");
    assert_eq!(notes(&st).iter().filter(|n| n.starts_with("end-")).count(), 2);
}

/// ADR-0075: inside a command, the events it emitted before its write go out with that write's commit, not when the
/// command returns. The commit listener is the reader (the row is visible; `src.make` hasn't returned): the command's
/// events are in the answer of a subscriber that waits for its handlers (before, they were published at the return,
/// after the listener: 0). A write that rolls back takes them with it (the command fails: never heard of, never
/// logged); a command that writes nothing publishes at its return, as before; the durable rows are logged once, in
/// the write's transaction.
#[tokio::test(flavor = "multi_thread")]
async fn a_reader_who_sees_a_commands_row_finds_the_commands_events_already_queued() {
    let path = temp("cmd-staged");
    let st = open(&path);
    let got = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&st, vec![source(), counter(got.clone())]).await;
    let kk = Arc::new(Mutex::new(Some(k.clone())));
    let answers = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let (a2, handle) = (answers.clone(), tokio::runtime::Handle::current());
    let _sub = st.on_commit(move |rows| {
        let Some(row) = rows.iter().find(|r| r.entity == NOTE.kind && r.id.starts_with("made-")) else { return };
        let Some(k) = kk.lock().unwrap().clone() else { return };
        let r = handle.block_on(k.call(Caller::user(), "live.count", json!(null))).unwrap();
        a2.lock().unwrap().push((row.id.clone(), r));
    });
    make(&k, json!({ "n": 1, "count": 2 })).await.unwrap();
    settle(&k).await;
    assert!(make(&k, json!({ "n": 10, "fail_write": true })).await.is_err());
    settle(&k).await;
    make(&k, json!({ "n": 20, "no_write": true })).await.unwrap();
    settle(&k).await;
    make(&k, json!({ "n": 30 })).await.unwrap();
    settle(&k).await;
    assert_eq!(
        *answers.lock().unwrap(),
        vec![("made-1".to_owned(), json!(2)), ("made-30".to_owned(), json!(4))],
        "the reader of each committed row heard the command's events, the command still running"
    );
    assert_eq!(*got.lock().unwrap(), vec![1, 2, 20, 30], "10 rolled back with its write and its command: never heard of");
    let log: Vec<u32> =
        EventLog::read(&st, 0, 100).await.unwrap().iter().map(|e| u32::try_from(e.event.data["n"].as_u64().unwrap()).unwrap()).collect();
    assert_eq!(log, vec![1, 2, 20, 30], "each logged once: with its write, or at the return of a command that wrote nothing");
}

/// ADR-0074 for a caller that must not await (`write_detached_with`: channels' task origin, a peer's status): the
/// reader of the row finds the event queued; nobody hears it before the row is there; a failed detached write
/// publishes and logs nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_detached_writes_events_go_out_with_its_commit() {
    let path = temp("detached");
    let st = open(&path);
    let got = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&st, vec![source(), counter(got.clone())]).await;
    let kk = Arc::new(Mutex::new(Some(k.clone())));
    let answers = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let (a2, handle) = (answers.clone(), tokio::runtime::Handle::current());
    let _sub = st.on_commit(move |rows| {
        let Some(row) = rows.iter().find(|r| r.entity == NOTE.kind && r.id.starts_with("det-")) else { return };
        let Some(k) = kk.lock().unwrap().clone() else { return };
        let r = handle.block_on(k.call(Caller::user(), "live.count", json!(null))).unwrap();
        a2.lock().unwrap().push((row.id.clone(), r));
    });
    let ctx = k.ctx("wisp.src").unwrap();
    for (n, fail) in [(1, false), (2, true), (3, false)] {
        let ob = ctx.outbox();
        ob.emit(&Made { n, key: None });
        st.write_detached_with(ob, move |w| {
            w.upsert(NOTE.kind, note(&format!("det-{n}")))?;
            if fail {
                return Err(wisp_store::StoreError::Internal("injected".into()));
            }
            Ok(())
        });
    }
    // a later awaited write finds the detached ones done (submit order)
    st.write(|_| Ok(())).await.unwrap();
    settle(&k).await;
    assert_eq!(*answers.lock().unwrap(), vec![("det-1".to_owned(), json!(1)), ("det-3".to_owned(), json!(2))]);
    assert_eq!(*got.lock().unwrap(), vec![1, 3], "2's write failed: never heard of");
    let log: Vec<u32> =
        EventLog::read(&st, 0, 100).await.unwrap().iter().map(|e| u32::try_from(e.event.data["n"].as_u64().unwrap()).unwrap()).collect();
    assert_eq!(log, vec![1, 3]);
}

// --- ADR-0088: idle cursors saved together, batches written once ---------------------------------------------------

/// Cursors as `set_cursors` takes them: (plugin, subscription, seq).
type Cursors = Vec<(String, String, u64)>;

/// The store's log, counting how cursors reach it: one at a time (`set_cursor`) or together (`set_cursors`).
struct Counted {
    st: Store,
    singles: AtomicU32,
    /// each `set_cursors` once its write landed
    together: Arc<Mutex<Vec<Cursors>>>,
}
impl Counted {
    fn new(st: &Store) -> Arc<Self> {
        Arc::new(Self { st: st.clone(), singles: AtomicU32::new(0), together: Arc::default() })
    }
    /// Every cursor saved together past `0` (a new subscription's first, saved once at its start).
    fn saved(&self) -> Vec<Cursors> {
        self.together
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.iter().filter(|r| r.2 > 0).cloned().collect::<Vec<_>>())
            .filter(|c| !c.is_empty())
            .collect()
    }
}
type Fut<T> = wisp_kernel::BoxFuture<'static, Result<T>>;
impl EventLog for Counted {
    fn append(&self, rows: Vec<wisp_kernel::durable::NewEvent>) -> Fut<Vec<Option<u64>>> {
        self.st.append(rows)
    }
    fn read(&self, after: u64, limit: usize) -> Fut<Vec<wisp_kernel::durable::StoredEvent>> {
        EventLog::read(&self.st, after, limit)
    }
    fn get(&self, seq: u64) -> Fut<Option<wisp_kernel::durable::StoredEvent>> {
        EventLog::get(&self.st, seq)
    }
    fn bounds(&self) -> Fut<(u64, u64)> {
        self.st.bounds()
    }
    fn cursor(&self, plugin: &str, name: &str) -> Fut<Option<u64>> {
        self.st.cursor(plugin, name)
    }
    fn set_cursor(&self, plugin: &str, name: &str, seq: u64) -> Fut<()> {
        self.singles.fetch_add(1, Ordering::SeqCst);
        self.st.set_cursor(plugin, name, seq)
    }
    fn set_cursors(&self, rows: Vec<(String, String, u64)>) -> Fut<()> {
        let (write, seen) = (self.st.set_cursors(rows.clone()), self.together.clone());
        Box::pin(async move {
            write.await?;
            seen.lock().unwrap().push(rows);
            Ok(())
        })
    }
    fn park(&self, p: wisp_kernel::durable::Parked) -> Fut<()> {
        self.st.park(p)
    }
    fn parked(&self) -> Fut<Vec<wisp_kernel::durable::Parked>> {
        self.st.parked()
    }
    fn unpark(&self, plugin: &str, name: &str, seq: u64) -> Fut<bool> {
        self.st.unpark(plugin, name, seq)
    }
}

async fn counted_kernel(st: &Store, log: Arc<Counted>, plugins: Vec<P>) -> Kernel {
    let mut g = PluginGroup::new();
    for p in plugins {
        g = g.add(p);
    }
    App::new().service(st.clone()).service(EventLogService(log)).plugins(g).start().await.unwrap()
}

/// Two subscriptions whose handlers write nothing (a sink, telemetry).
fn quiet(heard: Arc<AtomicU32>) -> P {
    P {
        id: "wisp.quiet",
        f: Arc::new(move |s| {
            for name in ["one", "two"] {
                let heard = heard.clone();
                s.on_durable_any(name, "src.*", move |_, _, _| {
                    let heard = heard.clone();
                    async move {
                        heard.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                });
            }
        }),
    }
}

async fn until(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..400 {
        if ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{what}");
}

#[tokio::test(flavor = "multi_thread")]
async fn idle_cursors_of_every_subscription_are_saved_together_within_a_second() {
    let st = open(&temp("together"));
    let log = Counted::new(&st);
    let heard = Arc::new(AtomicU32::new(0));
    let k = counted_kernel(&st, log.clone(), vec![source(), quiet(heard.clone())]).await;
    // the new subscriptions' first cursors (0) are saved once, together: past that, nothing is due
    until("the first cursors are saved", || !log.together.lock().unwrap().is_empty()).await;
    make(&k, json!({ "n": 1, "no_write": true })).await.unwrap();
    make(&k, json!({ "n": 2, "no_write": true })).await.unwrap();
    settle(&k).await;
    assert_eq!(heard.load(Ordering::SeqCst), 4, "both subscriptions had both");
    // not one write each as they caught up: one write for both, within SAVE_WITHIN_MS of the first
    until("the cursors are saved", || log.saved().iter().flatten().filter(|r| r.2 == 2).count() == 2).await;
    let mut saved = log.saved();
    assert_eq!(log.singles.load(Ordering::SeqCst), 0);
    assert_eq!(saved.len(), 1, "{saved:?}");
    saved[0].sort();
    assert_eq!(saved[0], [("wisp.quiet".to_owned(), "one".to_owned(), 2), ("wisp.quiet".to_owned(), "two".to_owned(), 2)]);
    assert_eq!(st.cursor("wisp.quiet", "two").await.unwrap(), Some(2));
    // a new start replays nothing: they were saved
    drop(k);
    let again = Arc::new(AtomicU32::new(0));
    let k = counted_kernel(&st, log.clone(), vec![source(), quiet(again.clone())]).await;
    settle(&k).await;
    assert_eq!(again.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_subscription_switched_off_saves_its_cursor_at_once() {
    let st = open(&temp("off-saves"));
    let log = Counted::new(&st);
    let heard = Arc::new(AtomicU32::new(0));
    let k = counted_kernel(&st, log.clone(), vec![source(), quiet(heard.clone())]).await;
    make(&k, json!({ "n": 1, "no_write": true })).await.unwrap();
    settle(&k).await;
    // switched off (as wisp's stop switches every plugin off before it closes the store): saved when it returns
    k.set_enabled("wisp.quiet", false).await.unwrap();
    assert_eq!(st.cursor("wisp.quiet", "one").await.unwrap(), Some(1));
    assert_eq!(st.cursor("wisp.quiet", "two").await.unwrap(), Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn switched_off_and_straight_back_on_replays_nothing() {
    let st = open(&temp("off-on"));
    let heard = Arc::new(AtomicU32::new(0));
    let k = kernel(&st, vec![source(), quiet(heard.clone())]).await;
    for n in 1..=3 {
        make(&k, json!({ "n": n, "no_write": true })).await.unwrap();
    }
    settle(&k).await;
    assert_eq!(heard.load(Ordering::SeqCst), 6);
    k.set_enabled("wisp.quiet", false).await.unwrap();
    k.set_enabled("wisp.quiet", true).await.unwrap();
    make(&k, json!({ "n": 4, "no_write": true })).await.unwrap();
    settle(&k).await;
    assert_eq!(heard.load(Ordering::SeqCst), 8, "only the new one, to each");
}

/// A batch subscription: what each call got, and one note a call (failing a batch of more than one when asked).
fn batcher(calls: Arc<Mutex<Vec<Vec<u64>>>>, fail_many: bool) -> P {
    P {
        id: "wisp.batch",
        f: Arc::new(move |s| {
            let calls = calls.clone();
            s.on_durable_batch("rows", "src.*", move |ctx, _, envs| {
                let calls = calls.clone();
                async move {
                    let seqs: Vec<u64> = envs.iter().filter_map(|e| e.durable_seq).collect();
                    calls.lock().unwrap().push(seqs.clone());
                    if fail_many && seqs.len() > 1 {
                        return Err(WireError::internal("too many"));
                    }
                    let id = format!("rows-{}", seqs.iter().map(u64::to_string).collect::<Vec<_>>().join("-"));
                    store_of(&ctx)
                        .write(move |w| w.upsert(NOTE.kind, note(&id)).map(|_| ()))
                        .await
                        .map_err(|e| WireError::internal(e.to_string()))
                }
            });
        }),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_subscription_gets_a_commits_events_in_one_call_its_cursor_riding_its_one_write() {
    let st = open(&temp("batch"));
    let log = Counted::new(&st);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let k = counted_kernel(&st, log.clone(), vec![source(), batcher(calls.clone(), false)]).await;
    // three events logged in one commit (the command's write), then one more alone
    make(&k, json!({ "n": 1, "count": 3 })).await.unwrap();
    settle(&k).await;
    make(&k, json!({ "n": 9, "no_write": true })).await.unwrap();
    settle(&k).await;
    assert_eq!(*calls.lock().unwrap(), [vec![1, 2, 3], vec![4]]);
    let rows: Vec<String> = notes(&st).into_iter().filter(|n| n.starts_with("rows-")).collect();
    assert_eq!(rows, ["rows-1-2-3", "rows-4"]);
    // the cursor went with each write: nothing waits to be saved
    assert_eq!(st.cursor("wisp.batch", "rows").await.unwrap(), Some(4));
    tokio::time::sleep(Duration::from_millis(wisp_kernel::durable::SAVE_WITHIN_MS + 200)).await;
    // (a new subscription's first cursor, 0, is saved once as before: where a restart resumes it)
    let again = log.saved();
    assert!(again.is_empty() && log.singles.load(Ordering::SeqCst) == 0, "no second write of the cursor: {again:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_that_fails_is_delivered_again_one_by_one() {
    let st = open(&temp("batch-fails"));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let k = kernel(&st, vec![source(), batcher(calls.clone(), true)]).await;
    make(&k, json!({ "n": 1, "count": 3 })).await.unwrap();
    settle(&k).await;
    assert_eq!(*calls.lock().unwrap(), [vec![1, 2, 3], vec![1], vec![2], vec![3]]);
    let rows: Vec<String> = notes(&st).into_iter().filter(|n| n.starts_with("rows-")).collect();
    assert_eq!(rows, ["rows-1", "rows-2", "rows-3"]);
    assert_eq!(st.cursor("wisp.batch", "rows").await.unwrap(), Some(3));
}
