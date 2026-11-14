//! A runtime over an in-memory store, a simulated proc service and a fake harness whose CLI prints v1
//! event JSON lines (what the recorded transcripts hold), so turns run end to end without any CLI.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use wisp_kernel::BoxFuture;
use wisp_proc::Exit;
use wisp_proc::sim::{SimIo, SimProc};
use wisp_runtime::{Recorder, Runtime, repo::Repo, schema};
use wisp_runtime_api::harness::{
    HarnessFound, HarnessInfo, HarnessModel, HarnessParser, HarnessRun, HarnessSys, Launch, ParserInput, ParserOutput, RunDriver, Stdin,
    pure,
};
use wisp_runtime_api::{Harness, HarnessEvent};
use wisp_store::{Location, Store, SystemClock};

/// A CLI whose output is v1 event JSON, one per line; the prompt is its first argument.
pub struct Fake {
    info: HarnessInfo,
    /// the program it launches (`fake`; another is one the sim doesn't have, so its spawn fails)
    program: &'static str,
    /// its prompt also written to its stdin (`Stdin::Bytes`, a prompt too long for argv)
    piped: bool,
}

impl Fake {
    /// Another fake harness (a hand-off's), with its own thinking levels.
    pub fn named(id: &str, name: &str, efforts: &[&str]) -> Arc<dyn Harness> {
        Arc::new(Self {
            info: HarnessInfo {
                id: id.into(),
                name: name.into(),
                models: vec![HarnessModel { id: format!("{id}-m"), label: "M".into(), hint: None, efforts: vec![] }],
                efforts: efforts.iter().map(|e| (*e).to_owned()).collect(),
                system_prompt_flag: true,
                ..HarnessInfo::default()
            },
            program: "fake",
            piped: false,
        })
    }

    /// A fake harness that runs Try it two ways' variants (`HarnessInfo::variants`), as Claude Code does.
    pub fn varies(id: &str, name: &str) -> Arc<dyn Harness> {
        Arc::new(Self {
            info: HarnessInfo {
                id: id.into(),
                name: name.into(),
                models: vec![HarnessModel { id: format!("{id}-m"), label: "M".into(), hint: None, efforts: vec![] }],
                system_prompt_flag: true,
                variants: vec!["subagentModel".into(), "skills".into()],
                ..HarnessInfo::default()
            },
            program: "fake",
            piped: false,
        })
    }

    /// `varies`, but its CLI isn't installed: every spawn fails (`ProcError::NotFound`).
    pub fn gone(id: &str, name: &str) -> Arc<dyn Harness> {
        let mut info = Self::varies(id, name).info().clone();
        info.id = id.into();
        Arc::new(Self { info, program: "gone-cli", piped: false })
    }

    /// Its prompt goes in on stdin, and its CLI (`deaf`) exits before reading it: the write fails.
    pub fn deaf() -> Arc<dyn Harness> {
        let mut info = Self::harness().info().clone();
        info.id = "deaf".into();
        Arc::new(Self { info, program: "deaf", piped: true })
    }

    pub fn harness() -> Arc<dyn Harness> {
        Arc::new(Self {
            info: HarnessInfo {
                id: "fake".into(),
                name: "Fake CLI".into(),
                models: vec![HarnessModel { id: "m1".into(), label: "M1".into(), hint: None, efforts: vec![] }],
                system_prompt_flag: true,
                ..HarnessInfo::default()
            },
            program: "fake",
            piped: false,
        })
    }
}

struct Lines {
    done: bool,
}

impl HarnessParser for Lines {
    fn on(&mut self, input: ParserInput<'_>, _now: i64, out: &mut ParserOutput) {
        match input {
            ParserInput::Stdout(line) => {
                if let Ok(e) = serde_json::from_str::<Value>(line).map_err(|_| ()).and_then(|v| HarnessEvent::from_v1(v).map_err(|_| ())) {
                    self.done |= e.is_result();
                    out.emit(e);
                }
            }
            ParserInput::Exit(x) if !self.done && x.stop_requested => {
                self.done = true;
                out.emit(HarnessEvent::Result {
                    text: String::new(),
                    is_error: false,
                    stopped: true,
                    stats: None,
                    auth_failed: false,
                    stop_reason: None,
                    background: None,
                    background_agents: None,
                });
            }
            _ => {}
        }
    }
}

impl Harness for Fake {
    fn info(&self) -> &HarnessInfo {
        &self.info
    }
    fn detect<'a>(&'a self, _sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        Box::pin(async { HarnessFound { available: true, ..HarnessFound::default() } })
    }
    fn launch(&self, run: &HarnessRun) -> wisp_kernel::Result<Launch> {
        let mut args = vec![run.prompt.clone()];
        if let Some(r) = &run.resume_id {
            args.push(format!("--resume={r}"));
        }
        if run.quiet {
            args.push("--quiet".into());
        }
        // what the run was told, as flags a test can read
        if run.system_prompt.contains("\"type\":\"app\"") {
            args.push("--app".into());
        }
        if run.system_prompt.contains("no `wisp` block") {
            args.push("--plain".into());
        }
        if run.env.contains_key("WISP_WATCH") {
            args.push("--watch".into());
        }
        if !run.subagent_model.is_empty() {
            args.push(format!("--sub={}", run.subagent_model));
        }
        if !run.skills.is_empty() {
            args.push(format!("--skills={}", run.skills));
        }
        let stdin = if self.piped { Stdin::Bytes(run.prompt.clone().into_bytes()) } else { Stdin::Null };
        Ok(Launch { program: self.program.into(), args, stdin, ..Launch::default() })
    }
    fn driver(&self, _run: &HarnessRun) -> Box<dyn RunDriver> {
        pure(Lines { done: false })
    }
}

/// Whether the fake CLI is signed in: a `signedout…` prompt fails signed out until a test sets it.
pub static SIGNED_IN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The answer a turn prints, by its prompt: `slow…` waits to be killed, `fail…` errors, `refuse…` turns it down,
/// else a hero.
pub fn script(prompt: &str) -> Vec<Value> {
    // `shot <path>`: the answer shows that picture too
    // built as JSON, so a Windows path stays a valid string (spliced in raw, `C:\Users` was no wisp block)
    let mut blocks = vec![
        json!({"type":"hero","title":"Did it","topic":format!("Topic of {prompt}"),"summary":format!("done: {prompt}"),"status":"done"}),
    ];
    if let Some(p) = prompt.strip_prefix("shot ") {
        blocks.push(json!({"type":"image","path":p}));
    }
    let hero = format!("```wisp\n{}\n```", json!({ "blocks": blocks }));
    if prompt.starts_with("fail") {
        return vec![json!({"type":"session","id":"conv-1"}), json!({"type":"result","text":"Error: boom\nmore","isError":true})];
    }
    // signed out on its first run, then fine once it runs again (the sign-in landed)
    if prompt.starts_with("signedout") && !SIGNED_IN.load(std::sync::atomic::Ordering::SeqCst) {
        return vec![
            json!({"type":"session","id":"conv-1"}),
            json!({"type":"result","text":"Not logged in · Please run /login","isError":true,"authFailed":true}),
        ];
    }
    if prompt.starts_with("limit") {
        return vec![
            json!({"type":"session","id":"conv-1"}),
            json!({"type":"result","text":"Claude AI usage limit reached|1767225600","isError":true}),
        ];
    }
    if prompt.starts_with("refuse") {
        return vec![
            json!({"type":"session","id":"conv-1"}),
            json!({"type":"result","text":"I can't help with that.","isError":false,"stopReason":"refusal"}),
        ];
    }
    // `envread…`: it prints a .env file, then quotes its value in its answer (ADR-0050)
    if prompt.starts_with("envread") {
        let text = format!("The key is sk-live-handoff.\n{hero}");
        return vec![
            json!({"type":"session","id":"conv-1"}),
            json!({"type":"tool","kind":"run","label":"cat .env","detail":"cat .env","id":"e1"}),
            json!({"type":"tool_output","id":"e1","text":"API_KEY=sk-live-handoff\nPORT=3000"}),
            json!({"type":"text","text":text}),
            json!({"type":"result","text":text,"isError":false}),
        ];
    }
    let cli = prompt.strip_prefix("cli ").map(|v| json!({"type":"cli","version":v}));
    let mut lines = vec![json!({"type":"session","id":"conv-1"})];
    lines.extend(cli);
    lines.extend([
        json!({"type":"tool","kind":"run","label":"Run tests","id":"c1"}),
        json!({"type":"tool_output","id":"c1","text":"ok"}),
        json!({"type":"change","file":"a.ts","op":"edit","edits":[{"before":"a","after":"b"}]}),
        json!({"type":"text","text":hero}),
        json!({"type":"result","text":hero,"isError":false,"stats":{"costUsd":0.01,"durationMs":5}}),
    ]);
    lines
}

/// Set once a `slowexit…` CLI has exited: told to stop, it takes 300 ms to go (a real CLI's exit, its drive's last git work).
pub static SLOW_EXIT_GONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Set once a `hang…` CLI was killed (a /btw a delete must stop).
pub static HANG_GONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

async fn fake_cli(mut io: SimIo) -> Exit {
    let prompt = io.args.first().cloned().unwrap_or_default();
    if prompt.starts_with("hang") {
        // set as it goes, however: the sim's kill may end it by dropping it (its own `killed` branch wins the
        // select) before a store after this await ran, and a delete that waited for it read it as still running
        struct Gone;
        impl Drop for Gone {
            fn drop(&mut self) {
                HANG_GONE.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let _gone = Gone;
        io.killed.cancelled().await;
        return Exit { code: None, signal: Some("SIGKILL".into()) };
    }

    if prompt.starts_with("slowexit") {
        for l in &script(&prompt)[..2] {
            io.print(format!("{l}\n").as_bytes()).await;
        }
        // its SIGTERM, then 300 ms to go (inside the kill's grace)
        let _ = io.signals.recv().await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        SLOW_EXIT_GONE.store(true, std::sync::atomic::Ordering::SeqCst);
        return Exit { code: None, signal: Some("SIGTERM".into()) };
    }
    let slow = prompt.starts_with("slow");
    let lines = script(&prompt);
    let (head, tail) = if slow { lines.split_at(2) } else { lines.split_at(lines.len()) };
    for l in head {
        io.print(format!("{l}\n").as_bytes()).await;
    }
    if slow {
        io.killed.cancelled().await;
        return Exit { code: None, signal: Some("SIGTERM".into()) };
    }
    for l in tail {
        io.print(format!("{l}\n").as_bytes()).await;
    }
    Exit { code: Some(0), signal: None }
}

/// A CLI that's gone before it reads its input: a line on stderr, exit 1.
async fn deaf_cli(io: SimIo) -> Exit {
    io.eprint(b"error: it went before reading its prompt\n").await;
    Exit { code: Some(1), signal: None }
}

pub struct Rig {
    pub rt: Arc<Runtime>,
    pub host: Arc<Recorder>,
    pub store: Store,
    pub sim: SimProc,
}

pub async fn rig_with(host: Recorder) -> Rig {
    rig_on(host, vec![Fake::harness()]).await
}

/// A rig with these harnesses (each runs the fake CLI).
pub async fn rig_on(host: Recorder, harnesses: Vec<Arc<dyn Harness>>) -> Rig {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    schema::install(&store).unwrap();
    let sim = SimProc::new().program("fake", fake_cli).program("deaf", deaf_cli).closed_stdin("deaf");
    let host = Arc::new(Recorder { harnesses, ..host });
    let rt = Runtime::new(Repo::new(store.clone()), host.clone(), Arc::new(sim.clone()), Arc::new(SystemClock));
    rt.start().await.unwrap();
    Rig { rt, host, store, sim }
}

pub async fn rig() -> Rig {
    rig_with(Recorder::default()).await
}

/// A folder a task may run in on this OS (it must exist: a task whose folder is gone never starts): `/tmp`, and
/// Windows' temp folder there (`/tmp` there is `C:\tmp`, which is no folder).
pub fn tmp() -> String {
    if cfg!(windows) { std::env::temp_dir().display().to_string().trim_end_matches('\\').to_owned() } else { "/tmp".to_owned() }
}

/// Waits (up to 5 s) until `f` holds.
pub async fn until<F, Fut>(what: &str, mut f: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for _ in 0..1000 {
        if f().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {what}");
}
