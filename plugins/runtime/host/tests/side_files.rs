//! A side run (/btw, a goal's judge, a review, a quick answer, a retro: v1 quick.js) gets the files its launch asked
//! for, as a turn does: grok's prompt file, codex's base instructions, copilot's usage file. Audit B #1: `side_run`
//! spawned the launch's argv as it was, so grok got `--prompt-file {tmp}/grok-prompt.md` (no such file) and never
//! the question.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_kernel::BoxFuture;
use wisp_proc::Exit;
use wisp_proc::sim::{SimIo, SimProc};
use wisp_runtime::{Recorder, Runtime, repo::Repo, schema};
use wisp_runtime_api::commands::QuickArgs;
use wisp_runtime_api::harness::{
    HarnessFound, HarnessInfo, HarnessParser, HarnessRun, HarnessSys, Launch, LaunchFile, ParserInput, ParserOutput, RunDriver, Stdin, pure,
};
use wisp_runtime_api::{Harness, HarnessEvent};
use wisp_store::{Location, Store, SystemClock};

/// A CLI that reads its prompt from a file (`--prompt-file <path>`), as grok does, and a usage file from its env.
struct FileFed {
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

impl Harness for FileFed {
    fn info(&self) -> &HarnessInfo {
        &self.info
    }
    fn detect<'a>(&'a self, _sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        Box::pin(async { HarnessFound { available: true, ..HarnessFound::default() } })
    }
    fn launch(&self, run: &HarnessRun) -> wisp_kernel::Result<Launch> {
        Ok(Launch {
            program: "filecli".into(),
            args: vec!["--prompt-file".into(), "{tmp}/q.md".into()],
            env: [("USAGE_FILE".to_owned(), "{tmp}/usage.json".to_owned())].into_iter().collect(),
            stdin: Stdin::Null,
            files: vec![LaunchFile { path: "q.md".into(), contents: run.prompt.clone() }],
            ..Launch::default()
        })
    }
    fn driver(&self, _run: &HarnessRun) -> Box<dyn RunDriver> {
        pure(Lines)
    }
}

/// Answers with what its prompt file holds (or that it found none) and where its usage file would go.
async fn file_cli(io: SimIo) -> Exit {
    let path = io.args.get(1).cloned().unwrap_or_default();
    let said = std::fs::read_to_string(&path).unwrap_or_else(|_| format!("no prompt file at {path}"));
    let usage = io.env.iter().find(|(k, _)| k == "USAGE_FILE").map(|(_, v)| v.clone()).unwrap_or_default();
    let text = format!("{said} | usage at {usage}");
    io.print(format!("{}\n", json!({"type":"result","text":text,"isError":false})).as_bytes()).await;
    Exit { code: Some(0), signal: None }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_side_run_reads_the_files_its_launch_asked_for() {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    schema::install(&store).unwrap();
    let sim = SimProc::new().program("filecli", file_cli);
    let info = HarnessInfo { id: "filefed".into(), name: "File-fed CLI".into(), ..HarnessInfo::default() };
    let host = Arc::new(Recorder { harnesses: vec![Arc::new(FileFed { info })], ..Recorder::default() });
    let rt = Runtime::new(Repo::new(store.clone()), host, Arc::new(sim.clone()), Arc::new(SystemClock));
    rt.start().await.unwrap();
    let dir = std::env::temp_dir().join(format!("wisp-side-files-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let q = QuickArgs {
        prompt: "is the sign fixed?".into(),
        cwd: dir.display().to_string(),
        pick: None,
        timeout_ms: None,
        json: false,
        run_id: None,
    };
    let out = rt.quick(q).await.unwrap();
    assert!(out.text.starts_with("is the sign fixed? | usage at "), "the CLI read its question from its prompt file: {}", out.text);
    assert!(!out.text.contains("{tmp}"), "no literal {{tmp}} in its argv or env: {}", out.text);
    let args = &sim.spawned()[0].args;
    let file = std::path::PathBuf::from(&args[1]);
    assert!(file.ends_with("q.md") && file.is_absolute(), "{args:?}");
    assert!(!file.exists(), "the side run's files are removed once it answered");
    std::fs::remove_dir_all(&dir).ok();
}
