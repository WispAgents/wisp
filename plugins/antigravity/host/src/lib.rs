//! wisp.antigravity: the Antigravity CLI (`agy`) as a module of wisp.runtime (`runtime.harness@1`).
//!
//! What it contributes (v1 adapters/antigravity.js): the CLI's description (no ask tool: agy reads
//! MCP servers only from its own config), `detect` (`WISP_ANTIGRAVITY_BIN` or `agy` on PATH; `agy
//! models`; its efforts from `--help`), the launch (the prompt as one stream-json `user` event), a
//! driver over `wisp-antigravity-core`'s parser with the file tracker on, and the resume command.

mod driver;
pub mod sessions;

use std::sync::{Arc, Mutex};

use serde_json::json;
use wisp_antigravity_core::{AgyParser, args, effort_in_id, list_models, stdin_event};
use wisp_harness_kit::RunPaths;
use wisp_harness_kit::detect::{help_efforts, version_in};
use wisp_harness_kit::protocol::{FileNote, attachment_note};
use wisp_kernel::{BoxFuture, Manifest, Plugin, Result, Setup};
use wisp_runtime_api::harness::{
    ExecReq, HarnessFound, HarnessInfo, HarnessLogin, HarnessModel, HarnessRun, HarnessSys, Launch, ResumeCommand, ResumeOptions,
    RunDriver, SetupTip, Stdin,
};
use wisp_runtime_api::model::Mode;
use wisp_runtime_api::{HARNESS, Harness};

pub const DEFAULT_BIN: &str = "agy";

pub fn info() -> HarnessInfo {
    HarnessInfo {
        id: "antigravity".into(),
        name: "Antigravity".into(),
        models: vec![HarnessModel {
            id: String::new(),
            label: "Default".into(),
            hint: Some("Whatever Antigravity is set to".into()),
            efforts: None,
            resolves: None,
        }],
        efforts: ["low", "medium", "high"].map(str::to_owned).to_vec(),
        ..HarnessInfo::default()
    }
}

fn mode(m: Mode) -> &'static str {
    match m {
        Mode::Plan => "plan",
        Mode::Edits => "edits",
        Mode::Auto => "auto",
    }
}

fn dir_of(p: &str) -> String {
    match p.rfind(['/', '\\']) {
        Some(0) => "/".to_owned(),
        Some(i) => p[..i].to_owned(),
        None => ".".to_owned(),
    }
}

/// The launch (the runtime already put wisp's protocol ahead of a new conversation's prompt).
pub fn launch(run: &HarnessRun, bin: &str) -> Launch {
    let with_path: Vec<_> = run.attachments.iter().filter(|a| a.path.is_some()).collect();
    let dirs: Vec<String> = with_path.iter().filter_map(|a| a.path.as_deref().map(dir_of)).collect();
    let notes: Vec<FileNote> =
        with_path.iter().map(|a| FileNote { path: a.path.clone().unwrap_or_default(), mime: a.mime.clone(), size: a.size }).collect();
    let content = format!("{}{}", run.prompt, attachment_note(&notes, None));
    Launch {
        program: bin.to_owned(),
        args: args(mode(run.mode), &run.model, &run.effort, run.resume_id.as_deref(), &dirs),
        cwd: run.cwd.clone(),
        // closing stdin ends the session once that turn's result is out
        stdin: Stdin::Bytes(stdin_event(&content).into_bytes()),
        ..Launch::default()
    }
}

async fn output(sys: &dyn HarnessSys, bin: &str, args: &[&str]) -> Option<(String, String)> {
    let req =
        ExecReq { program: bin.to_owned(), args: args.iter().map(|a| (*a).to_owned()).collect(), timeout_ms: 15_000, ..ExecReq::default() };
    match sys.exec(req).await {
        Ok(o) if o.code == Some(0) && !o.timed_out => Some((o.stdout, o.stderr)),
        _ => None,
    }
}

/// Signing in (v1 agent-login.js LOGINS.antigravity): agy has no login command. A first prompt without
/// a login prints Google's link and reads the code back, then answers. It only asks on a terminal
/// (`tty`: wisp.accounts runs it under script(1) on Linux).
pub fn login() -> HarnessLogin {
    HarnessLogin {
        bin: DEFAULT_BIN.into(),
        bin_env: "WISP_ANTIGRAVITY_BIN".into(),
        args: vec!["-p".into(), "Reply with OK.".into()],
        status: vec!["models".into()],
        keys: vec!["GEMINI_API_KEY".into()],
        tty: true,
        ..HarnessLogin::default()
    }
}

#[derive(Debug)]
pub struct AntigravityHarness {
    info: HarnessInfo,
    bin: Mutex<Option<String>>,
    login: HarnessLogin,
}

impl Default for AntigravityHarness {
    fn default() -> Self {
        Self { info: info(), bin: Mutex::new(None), login: login() }
    }
}

impl AntigravityHarness {
    fn bin(&self) -> String {
        self.bin
            .lock()
            .ok()
            .and_then(|b| b.clone())
            .or_else(|| std::env::var("WISP_ANTIGRAVITY_BIN").ok().filter(|b| !b.is_empty()))
            .unwrap_or_else(|| DEFAULT_BIN.to_owned())
    }
}

impl Harness for AntigravityHarness {
    fn info(&self) -> &HarnessInfo {
        &self.info
    }

    fn detect<'a>(&'a self, sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        Box::pin(async move {
            let bin = match sys.env("WISP_ANTIGRAVITY_BIN").filter(|b| !b.is_empty()) {
                Some(b) => Some(b),
                None => sys.which(DEFAULT_BIN).await,
            };
            let Some(bin) = bin else {
                return HarnessFound { available: false, error: Some("Antigravity isn't installed".into()), ..HarnessFound::default() };
            };
            // the three at once: a CLI that hangs costs one time limit, not three
            let found = wisp_harness_kit::probe::gated(
                output(sys, &bin, &["--version"]),
                wisp_harness_kit::probe::both(output(sys, &bin, &["--help"]), output(sys, &bin, &["models"])),
            )
            .await;
            let Some(((line, _), (help, listed))) = found else {
                return HarnessFound { available: false, error: Some("agy --version failed".into()), ..HarnessFound::default() };
            };
            if let Ok(mut b) = self.bin.lock() {
                *b = Some(bin.clone());
            }
            // agy prints its help to stderr
            let help = help.map(|(o, e)| format!("{o}\n{e}")).unwrap_or_default();
            let efforts = help_efforts(&help);
            let mut models = vec![self.info.models[0].clone()];
            models.extend(list_models(&listed.map(|(o, _)| o).unwrap_or_default()).into_iter().map(|m| HarnessModel {
                hint: Some(m.id.clone()),
                id: m.id,
                label: m.label,
                // its own levels (agy refuses one it lacks); none for a model agy lists without
                efforts: Some(m.efforts),
                resolves: None,
            }));
            let v = version_in(&line);
            HarnessFound {
                available: true,
                version: Some(if v.is_empty() { line.trim().to_owned() } else { v }),
                bin: Some(bin),
                models: Some(models),
                efforts: (!efforts.is_empty()).then_some(efforts),
                ..HarnessFound::default()
            }
        })
    }

    fn program(&self) -> Option<String> {
        Some(self.bin())
    }

    fn launch(&self, run: &HarnessRun) -> Result<Launch> {
        Ok(launch(run, &run.bin.clone().unwrap_or_else(|| self.bin())))
    }

    fn driver(&self, run: &HarnessRun) -> Box<dyn RunDriver> {
        let paths = RunPaths::same(run.cwd.clone());
        driver::KitDriver::boxed(AgyParser::new(paths.clone()), paths, true)
    }

    /// Reads Antigravity's quota windows without spending tokens (`agy -p "/usage" --output-format json`),
    /// returning wisp's normalised `LimitsReport` JSON.
    fn live_limits<'a>(
        &'a self,
        sys: &'a dyn HarnessSys,
        env: &'a std::collections::BTreeMap<String, String>,
    ) -> BoxFuture<'a, Option<serde_json::Value>> {
        Box::pin(async move {
            let req = ExecReq {
                program: self.bin(),
                args: ["-p", "/usage", "--output-format", "json"].map(str::to_owned).to_vec(),
                env: env.clone(),
                timeout_ms: 15_000,
                ..ExecReq::default()
            };
            let out = sys.exec(req).await.ok()?;
            if out.code != Some(0) || out.timed_out {
                return None;
            }
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .and_then(|d| i64::try_from(d.as_millis()).ok())
                .unwrap_or(0);
            wisp_antigravity_core::usage_limits(&out.stdout, now_ms)
        })
    }

    fn login(&self) -> Option<&HarnessLogin> {
        Some(&self.login)
    }

    fn signed_in(&self, out: &str, code: Option<i32>) -> Option<bool> {
        let o = out.to_lowercase();
        if o.contains("sign in") || o.contains("authenticat") {
            Some(false)
        } else if code == Some(0) {
            Some(true)
        } else {
            None
        }
    }

    fn auth_failed(&self, text: &str) -> bool {
        let t = text.to_lowercase();
        text.len() <= 1200
            && ["authentication required", "authentication failed or timed out", "authentication timed out", "not logged into antigravity"]
                .iter()
                .any(|w| t.contains(w))
    }

    /// Signing out is only `/logout` in its own TUI: wisp removes the login where agy keeps it (the
    /// keychain on macOS, a file in ~/.gemini elsewhere).
    fn sign_out<'a>(&'a self, sys: &'a dyn HarnessSys) -> Option<BoxFuture<'a, Result<()>>> {
        Some(Box::pin(async move {
            let file = format!("{}/.gemini/antigravity-cli/antigravity-oauth-token", sys.home());
            let rm = ExecReq { program: "rm".into(), args: vec!["-f".into(), file], timeout_ms: 5_000, ..ExecReq::default() };
            let _ = sys.exec(rm).await;
            if sys.platform() == "darwin" {
                let k = ExecReq {
                    program: "security".into(),
                    args: ["delete-generic-password", "-s", "gemini", "-a", "antigravity"].map(str::to_owned).to_vec(),
                    timeout_ms: 5_000,
                    ..ExecReq::default()
                };
                let _ = sys.exec(k).await;
            }
            Ok(())
        }))
    }

    fn setup(&self, _platform: &str) -> Option<SetupTip> {
        Some(SetupTip {
            tip: "Install the Antigravity CLI (agy) and sign in. The Antigravity app doesn’t include it".into(),
            command: None,
            url: Some("https://antigravity.google/docs/cli/headless/".into()),
        })
    }

    /// agy has no --trust: `--settings` trusts the folder for this session only.
    fn resume_command(&self, o: &ResumeOptions) -> Option<ResumeCommand> {
        let mut argv = vec![self.bin()];
        if let Some(r) = o.resume_id.as_ref().filter(|r| !r.is_empty()) {
            argv.extend(["--conversation".to_owned(), r.clone()]);
        }
        if !o.model.is_empty() {
            argv.extend(["--model".to_owned(), o.model.clone()]);
        }
        if !o.effort.is_empty() && !effort_in_id(&o.model) {
            argv.extend(["--effort".to_owned(), o.effort.clone()]);
        }
        if o.trusted && !o.cwd.is_empty() {
            argv.extend(["--settings".to_owned(), json!({ "trustedWorkspaces": [o.cwd] }).to_string()]);
        }
        Some(ResumeCommand { argv, types_prompt: true })
    }
}

#[derive(Debug, Default)]
pub struct AntigravityPlugin;

impl Plugin for AntigravityPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let h: Arc<dyn Harness> = Arc::new(AntigravityHarness::default());
        s.contribute(&HARNESS, h);
        let sessions: Arc<dyn wisp_runtime_api::harness::sessions::HarnessSessions> =
            Arc::new(crate::sessions::AntigravitySessions::from_env());
        s.contribute(&wisp_runtime_api::harness::sessions::SESSIONS, sessions);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        assert_eq!(AntigravityPlugin.manifest().id, "wisp.antigravity");
    }

    #[test]
    fn the_prompt_is_one_user_event_on_stdin() {
        // v1 adapters.test.js "agy takes the prompt as a stream-json user event"
        let run = HarnessRun { prompt: "fix the add bug".into(), cwd: "/w".into(), mode: Mode::Plan, ..HarnessRun::default() };
        let l = launch(&run, "agy");
        assert_eq!(l.stdin, Stdin::Bytes(b"{\"event\":\"user\",\"message\":{\"content\":\"fix the add bug\"}}\n".to_vec()));
        assert!(l.args.windows(2).any(|w| w == ["--mode", "plan"]));
        let h = AntigravityHarness::default();
        let o = ResumeOptions { resume_id: Some("c".into()), trusted: true, cwd: "/w".into(), ..ResumeOptions::default() };
        assert_eq!(
            h.resume_command(&o).map(|c| c.argv.last().cloned()).unwrap_or_default().as_deref(),
            Some("{\"trustedWorkspaces\":[\"/w\"]}")
        );
    }

    struct FakeSys {
        stdout: String,
        seen: Mutex<Option<ExecReq>>,
    }

    impl HarnessSys for FakeSys {
        fn exec(&self, req: ExecReq) -> BoxFuture<'_, Result<wisp_runtime_api::harness::ExecOut>> {
            if let Ok(mut s) = self.seen.lock() {
                *s = Some(req);
            }
            let stdout = self.stdout.clone();
            Box::pin(async move { Ok(wisp_runtime_api::harness::ExecOut { code: Some(0), stdout, ..Default::default() }) })
        }
        fn which(&self, _: &str) -> BoxFuture<'_, Option<String>> {
            Box::pin(async { None })
        }
        fn read_file(&self, _: &str) -> BoxFuture<'_, Option<String>> {
            Box::pin(async { None })
        }
        fn write_file(&self, _: &str, _: String) -> BoxFuture<'_, Result<()>> {
            Box::pin(async { Ok(()) })
        }
        fn list_dir(&self, _: &str) -> BoxFuture<'_, Vec<String>> {
            Box::pin(async { Vec::new() })
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

    #[tokio::test]
    async fn live_limits_runs_agy_usage_and_parses_buckets() {
        let sys = FakeSys {
            stdout: r#"{"status":"SUCCESS","command":{"name":"usage","data":{"groups":[{"name":"Gemini Models","buckets":[{"id":"gemini-5h","window":"5h","remaining_fraction":0.75,"reset_time":"2099-01-01T05:00:00Z"},{"id":"gemini-weekly","window":"weekly","remaining_fraction":0.4,"reset_time":"2099-01-07T00:00:00Z"}]}]}}}"#
                .into(),
            seen: Mutex::new(None),
        };
        let h = AntigravityHarness::default();
        let got = h.live_limits(&sys, &std::collections::BTreeMap::new()).await;
        let req = sys.seen.lock().ok().and_then(|g| g.clone());
        assert_eq!(req.map(|r| r.args), Some(["-p", "/usage", "--output-format", "json"].map(str::to_owned).to_vec()));
        let ws = got.as_ref().and_then(|v| v["windows"].as_array()).map_or(&[][..], Vec::as_slice);
        assert_eq!(ws.len(), 2);
        assert_eq!(ws[0]["id"], "five_hour");
        assert_eq!(ws[1]["id"], "seven_day");
    }
}
