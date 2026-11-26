//! A side run (/btw, a quick answer, a goal's judge, a probe) and its CLI: a last result line with no newline after
//! it is still its answer, and the CLI never outlives the run. The run dropped the unterminated line (an empty answer,
//! or the Exit's error), left the CLI running when its prompt couldn't be written, and when its future was dropped.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_kernel::BoxFuture;
use wisp_proc::Exit;
use wisp_proc::sim::{SimIo, SimProc};
use wisp_runtime::{Recorder, Runtime, repo::Repo, schema};
use wisp_runtime_api::commands::QuickArgs;
use wisp_runtime_api::harness::{
    HarnessFound, HarnessInfo, HarnessParser, HarnessRun, HarnessSys, Launch, ParserInput, ParserOutput, RunDriver, Stdin, pure,
};
use wisp_runtime_api::{Harness, HarnessEvent};
use wisp_store::{Location, Store, SystemClock};

/// Runs the program its prompt names (`noeol`, `deafside`, `hangside`), the prompt also on its stdin.
struct Side {
    info: HarnessInfo,
}

struct Lines;

impl HarnessParser for Lines {
    fn on(&mut self, input: ParserInput<'_>, _now: i64, out: &mut ParserOutput) {
        if let ParserInput::Stdout(line) = input
            && let Ok(e) = serde_json::from_str::<Value>(line).map_err(|_| ()).and_then(|v| HarnessEvent::from_v1(v).map_err(|_| ()))
        {
            out.emit(e);
        }
    }
}

impl Harness for Side {
    fn info(&self) -> &HarnessInfo {
        &self.info
    }
    fn detect<'a>(&'a self, _sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        Box::pin(async { HarnessFound { available: true, ..HarnessFound::default() } })
    }
    fn launch(&self, run: &HarnessRun) -> wisp_kernel::Result<Launch> {
        Ok(Launch { program: run.prompt.clone(), stdin: Stdin::Bytes(run.prompt.clone().into_bytes()), ..Launch::default() })
    }
    fn driver(&self, _run: &HarnessRun) -> Box<dyn RunDriver> {
        pure(Lines)
    }
}

static DEAF_KILLED: AtomicBool = AtomicBool::new(false);
static HANG_KILLED: AtomicBool = AtomicBool::new(false);

/// Its result as its last bytes, no newline after them.
async fn noeol(io: SimIo) -> Exit {
    io.print(json!({"type":"result","text":"the sign is fixed","isError":false}).to_string().as_bytes()).await;
    Exit { code: Some(0), signal: None }
}

/// Waits to be told to go (a signal, or cut off), and says it was. Its handles all dropped is no kill: it runs on,
/// orphaned.
async fn waits(mut io: SimIo, flag: &'static AtomicBool) -> Exit {
    tokio::select! {
        Some(_) = io.signals.recv() => {}
        () = io.killed.cancelled() => {}
    }
    flag.store(true, Ordering::SeqCst);
    Exit { code: None, signal: Some("SIGTERM".into()) }
}

async fn rt() -> (Arc<Runtime>, SimProc) {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    schema::install(&store).unwrap();
    let sim = SimProc::new()
        .program("noeol", noeol)
        .program("deafside", |io| waits(io, &DEAF_KILLED))
        .program("hangside", |io| waits(io, &HANG_KILLED))
        .closed_stdin("deafside");
    let info = HarnessInfo { id: "side".into(), name: "Side CLI".into(), ..HarnessInfo::default() };
    let host = Arc::new(Recorder { harnesses: vec![Arc::new(Side { info })], ..Recorder::default() });
    let rt = Runtime::new(Repo::new(store), host, Arc::new(sim.clone()), Arc::new(SystemClock));
    rt.start().await.unwrap();
    (rt, sim)
}

fn ask(program: &str) -> QuickArgs {
    QuickArgs {
        prompt: program.into(),
        cwd: std::env::temp_dir().display().to_string(),
        pick: None,
        timeout_ms: None,
        json: false,
        run_id: None,
    }
}

async fn until(what: &str, f: impl Fn() -> bool) {
    for _ in 0..1000 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {what}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_last_result_line_with_no_newline_is_still_the_answer() {
    let (rt, _) = rt().await;
    let out = rt.quick(ask("noeol")).await.unwrap();
    assert_eq!(out.text, "the sign is fixed");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cli_that_cant_take_its_prompt_is_killed() {
    let (rt, _) = rt().await;
    assert!(rt.quick(ask("deafside")).await.is_err(), "its prompt never reached it");
    until("its CLI is killed", || DEAF_KILLED.load(Ordering::SeqCst)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dropped_side_run_kills_its_cli() {
    let (rt, sim) = rt().await;
    let run = tokio::spawn(async move { rt.quick(ask("hangside")).await });
    until("its CLI runs", || !sim.spawned().is_empty()).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    run.abort();
    until("its CLI is killed", || HANG_KILLED.load(Ordering::SeqCst)).await;
}
