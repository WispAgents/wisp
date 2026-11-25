//! wisp.cursor: the Cursor CLI as a module of wisp.runtime (`runtime.harness@1`).
//!
//! What it contributes (v1 adapters/cursor.js): the CLI's description (no ask tool: cursor-agent
//! takes no MCP server by flag), `detect` (`WISP_CURSOR_BIN` or `cursor-agent` on PATH, its version,
//! `--list-models`), the launch, a driver over `wisp-cursor-core`'s parser with the file tracker on,
//! and the resume command.

mod driver;
pub mod sessions;

use std::sync::{Arc, Mutex};

use wisp_cursor_core::{CursorParser, args, list_models};
use wisp_harness_kit::RunPaths;
use wisp_harness_kit::detect::version_in;
use wisp_harness_kit::protocol::{FileNote, attachment_note};
use wisp_kernel::{BoxFuture, Manifest, Plugin, Result, Setup};
use wisp_runtime_api::harness::{
    ExecReq, HarnessFound, HarnessInfo, HarnessLogin, HarnessModel, HarnessRun, HarnessSys, Launch, ResumeCommand, ResumeOptions,
    RunDriver, SetupTip, Stdin,
};
use wisp_runtime_api::model::Mode;
use wisp_runtime_api::{HARNESS, Harness};

pub const DEFAULT_BIN: &str = "cursor-agent";

pub fn info() -> HarnessInfo {
    HarnessInfo {
        id: "cursor".into(),
        name: "Cursor".into(),
        models: vec![HarnessModel {
            id: String::new(),
            label: "Default".into(),
            hint: Some("Whatever Cursor is set to".into()),
            efforts: None,
            resolves: None,
        }],
        ..HarnessInfo::default()
    }
}

async fn output(sys: &dyn HarnessSys, bin: &str, args: &[&str]) -> Option<String> {
    let req =
        ExecReq { program: bin.to_owned(), args: args.iter().map(|a| (*a).to_owned()).collect(), timeout_ms: 15_000, ..ExecReq::default() };
    match sys.exec(req).await {
        Ok(o) if o.code == Some(0) && !o.timed_out => Some(o.stdout),
        _ => None,
    }
}

fn mode(m: Mode) -> &'static str {
    match m {
        Mode::Plan => "plan",
        Mode::Edits => "edits",
        Mode::Auto => "auto",
    }
}

/// The launch (the runtime already put wisp's protocol ahead of a new chat's prompt).
pub fn launch(run: &HarnessRun, bin: &str) -> Launch {
    let notes: Vec<FileNote> =
        run.attachments.iter().filter_map(|a| a.path.clone().map(|path| FileNote { path, mime: a.mime.clone(), size: a.size })).collect();
    let input = format!("{}{}", run.prompt, attachment_note(&notes, None));
    Launch {
        program: bin.to_owned(),
        args: args(mode(run.mode), &run.model, run.resume_id.as_deref()),
        cwd: run.cwd.clone(),
        stdin: Stdin::Bytes(input.into_bytes()),
        ..Launch::default()
    }
}

/// Signing in from wisp (v1 agent-login.js LOGINS.cursor): its own login, with no browser opened by the CLI (the page
/// shows the link), its logout and status.
pub fn login() -> HarnessLogin {
    HarnessLogin {
        bin: DEFAULT_BIN.into(),
        bin_env: "WISP_CURSOR_BIN".into(),
        args: vec!["login".into()],
        logout: vec!["logout".into()],
        status: vec!["status".into()],
        keys: vec!["CURSOR_API_KEY".into()],
        extra_env: [("NO_OPEN_BROWSER".to_owned(), "1".to_owned())].into_iter().collect(),
        ..HarnessLogin::default()
    }
}

/// What a CLI's status said (v1 agent-login.js `said`): "not logged / signed in" is out, "logged / signed in" is in, the
/// rest isn't an answer.
fn said(out: &str) -> Option<bool> {
    let o = out.to_lowercase();
    if o.contains("not logged in") || o.contains("not signed in") {
        Some(false)
    } else if o.contains("logged in") || o.contains("signed in") {
        Some(true)
    } else {
        None
    }
}

#[derive(Debug)]
pub struct CursorHarness {
    info: HarnessInfo,
    bin: Mutex<Option<String>>,
    login: HarnessLogin,
}

impl Default for CursorHarness {
    fn default() -> Self {
        Self { info: info(), bin: Mutex::new(None), login: login() }
    }
}

impl CursorHarness {
    fn bin(&self) -> String {
        self.bin
            .lock()
            .ok()
            .and_then(|b| b.clone())
            .or_else(|| std::env::var("WISP_CURSOR_BIN").ok().filter(|b| !b.is_empty()))
            .unwrap_or_else(|| DEFAULT_BIN.to_owned())
    }
}

impl Harness for CursorHarness {
    fn info(&self) -> &HarnessInfo {
        &self.info
    }
    fn login(&self) -> Option<&HarnessLogin> {
        Some(&self.login)
    }
    fn signed_in(&self, out: &str, _code: Option<i32>) -> Option<bool> {
        said(out)
    }

    fn detect<'a>(&'a self, sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        Box::pin(async move {
            let bin = match sys.env("WISP_CURSOR_BIN").filter(|b| !b.is_empty()) {
                Some(b) => Some(b),
                None => sys.which(DEFAULT_BIN).await,
            };
            let Some(bin) = bin else {
                return HarnessFound { available: false, error: Some("Cursor isn't installed".into()), ..HarnessFound::default() };
            };
            // both at once: a CLI that hangs costs one time limit, not two
            let found = wisp_harness_kit::probe::gated(output(sys, &bin, &["--version"]), output(sys, &bin, &["--list-models"])).await;
            let Some((line, listed)) = found else {
                return HarnessFound { available: false, error: Some("cursor-agent --version failed".into()), ..HarnessFound::default() };
            };
            if let Ok(mut b) = self.bin.lock() {
                *b = Some(bin.clone());
            }
            let mut models = vec![self.info.models[0].clone()];
            models.extend(list_models(&listed.unwrap_or_default()).into_iter().map(|(id, label)| HarnessModel {
                hint: Some(id.clone()),
                id,
                label,
                efforts: None,
                resolves: None,
            }));
            let v = version_in(&line);
            HarnessFound {
                available: true,
                version: Some(if v.is_empty() { line.trim().to_owned() } else { v }),
                bin: Some(bin),
                models: Some(models),
                ..HarnessFound::default()
            }
        })
    }

    fn program(&self) -> Option<String> {
        Some(self.bin())
    }

    /// A read that spends nothing: `cursor-agent about --format json` names the plan tier. Cursor reports no usage
    /// window, so `windows` is empty and Usage says its limits aren't reported (ADR-0120).
    fn live_limits<'a>(
        &'a self,
        sys: &'a dyn HarnessSys,
        env: &'a std::collections::BTreeMap<String, String>,
    ) -> BoxFuture<'a, Option<serde_json::Value>> {
        Box::pin(async move {
            let req = ExecReq {
                program: self.bin(),
                args: ["about", "--format", "json"].map(str::to_owned).to_vec(),
                env: env.clone(),
                timeout_ms: 15_000,
                ..ExecReq::default()
            };
            let out = sys.exec(req).await.ok().filter(|o| o.code == Some(0) && !o.timed_out)?;
            let a = wisp_cursor_core::about(&out.stdout)?;
            Some(serde_json::json!({ "windows": [], "plan": a.tier }))
        })
    }

    fn launch(&self, run: &HarnessRun) -> Result<Launch> {
        Ok(launch(run, &run.bin.clone().unwrap_or_else(|| self.bin())))
    }

    fn driver(&self, run: &HarnessRun) -> Box<dyn RunDriver> {
        let paths = RunPaths::same(run.cwd.clone());
        driver::KitDriver::boxed(CursorParser::new(paths.clone()), paths, true)
    }

    fn setup(&self, platform: &str) -> Option<SetupTip> {
        Some(SetupTip {
            tip: "Install the Cursor CLI, then run cursor-agent login. The Cursor editor doesn’t include it".into(),
            command: (platform != "win32").then(|| "curl https://cursor.com/install -fsS | bash".into()),
            url: Some("https://cursor.com/cli".into()),
        })
    }

    fn resume_command(&self, o: &ResumeOptions) -> Option<ResumeCommand> {
        let mut argv = vec![self.bin()];
        if let Some(r) = o.resume_id.as_ref().filter(|r| !r.is_empty()) {
            argv.extend(["--resume".to_owned(), r.clone()]);
        }
        if !o.model.is_empty() {
            argv.extend(["--model".to_owned(), o.model.clone()]);
        }
        // a folder wisp made, which it would otherwise stop to ask about
        if o.trusted {
            argv.push("--trust".into());
        }
        if let Some(p) = o.prompt.as_ref().filter(|p| !p.is_empty()) {
            argv.push(p.clone());
        }
        Some(ResumeCommand { argv, types_prompt: false })
    }
}

#[derive(Debug, Default)]
pub struct CursorPlugin;

impl Plugin for CursorPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let h: Arc<dyn Harness> = Arc::new(CursorHarness::default());
        s.contribute(&HARNESS, h);
        let sessions: Arc<dyn wisp_runtime_api::harness::sessions::HarnessSessions> = Arc::new(crate::sessions::CursorSessions::from_env());
        s.contribute(&wisp_runtime_api::harness::sessions::SESSIONS, sessions);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The owner's Agents audit (2026-11-18): v1 signed in to and out of Cursor from Settings → Agents (agent-login.js
    /// LOGINS.cursor); v2 had no login for it, so its row said Ready and nothing else.
    #[test]
    fn cursor_signs_in_and_out_from_wisp_as_v1() {
        let h = CursorHarness::default();
        let l = h.login().unwrap();
        assert_eq!((l.bin.as_str(), l.bin_env.as_str()), ("cursor-agent", "WISP_CURSOR_BIN"));
        assert_eq!(
            (l.args.clone(), l.logout.clone(), l.status.clone()),
            (vec!["login".to_owned()], vec!["logout".to_owned()], vec!["status".to_owned()])
        );
        assert_eq!(l.keys, vec!["CURSOR_API_KEY".to_owned()]);
        assert_eq!(l.extra_env.get("NO_OPEN_BROWSER").map(String::as_str), Some("1"));
        assert_eq!(h.signed_in("✓ Logged in as me@example.com", Some(0)), Some(true));
        assert_eq!(h.signed_in("Not logged in", Some(1)), Some(false));
        assert_eq!(h.signed_in("something else", Some(0)), None);
    }

    #[test]
    fn manifest_is_valid() {
        assert_eq!(CursorPlugin.manifest().id, "wisp.cursor");
        assert!(info().ask_tool.is_none(), "no MCP by flag: a question block instead");
    }

    #[test]
    fn launch_and_resume() {
        let run = HarnessRun { prompt: "p".into(), cwd: "/w".into(), mode: Mode::Auto, model: "m-1".into(), ..HarnessRun::default() };
        assert_eq!(launch(&run, "cursor-agent").args, ["-p", "--output-format", "stream-json", "--trust", "--force", "--model", "m-1"]);
        let h = CursorHarness::default();
        let o = ResumeOptions { resume_id: Some("c1".into()), trusted: true, prompt: Some("go".into()), ..ResumeOptions::default() };
        assert_eq!(h.resume_command(&o).map(|c| c.argv[1..].to_vec()).unwrap_or_default(), ["--resume", "c1", "--trust", "go"]);
    }

    /// A computer with only the CLI's answers (`exec` by its arguments), its folders and its files: no real process.
    #[derive(Default)]
    struct FakeSys {
        out: std::collections::BTreeMap<String, (i32, String)>,
        dirs: std::collections::BTreeMap<String, Vec<String>>,
        files: std::collections::BTreeMap<String, String>,
        seen: Mutex<Vec<Vec<String>>>,
    }

    impl HarnessSys for FakeSys {
        fn exec(&self, req: ExecReq) -> BoxFuture<'_, Result<wisp_runtime_api::harness::ExecOut>> {
            let got = self.out.get(&req.args.join(" ")).cloned();
            if let Ok(mut s) = self.seen.lock() {
                s.push(req.args);
            }
            Box::pin(async move {
                let (code, stdout) = got.unwrap_or((1, String::new()));
                Ok(wisp_runtime_api::harness::ExecOut { code: Some(code), stdout, ..Default::default() })
            })
        }
        fn which(&self, _: &str) -> BoxFuture<'_, Option<String>> {
            Box::pin(async { None })
        }
        fn read_file(&self, p: &str) -> BoxFuture<'_, Option<String>> {
            let f = self.files.get(p).cloned();
            Box::pin(async move { f })
        }
        fn write_file(&self, _: &str, _: String) -> BoxFuture<'_, Result<()>> {
            Box::pin(async { Ok(()) })
        }
        fn list_dir(&self, p: &str) -> BoxFuture<'_, Vec<String>> {
            let d = self.dirs.get(p).cloned().unwrap_or_default();
            Box::pin(async move { d })
        }
        fn env(&self, _: &str) -> Option<String> {
            None
        }
        fn home(&self) -> String {
            "/home/test".into()
        }
        fn platform(&self) -> String {
            "darwin".into()
        }
    }

    /// ADR-0120: `cursor-agent about --format json`, as captured (`core/tests/cli/about.json`), gives the tier and no
    /// windows; budget carries it to the Usage page. A failed `about` is no reading.
    #[tokio::test]
    async fn live_limits_reads_the_tier_from_cursor_agent_about() {
        let mut sys = FakeSys::default();
        sys.out.insert("about --format json".into(), (0, include_str!("../../core/tests/cli/about.json").into()));
        let h = CursorHarness::default();
        let got = h.live_limits(&sys, &std::collections::BTreeMap::new()).await;
        assert_eq!(got, Some(serde_json::json!({ "windows": [], "plan": "Free" })));
        assert_eq!(sys.seen.lock().map(|s| s.clone()).unwrap_or_default(), [["about", "--format", "json"]]);
        let none = FakeSys::default();
        assert_eq!(h.live_limits(&none, &std::collections::BTreeMap::new()).await, None, "about failed: nothing made up");
    }
}
