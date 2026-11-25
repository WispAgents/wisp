//! wisp.grok: Grok Build as a module of wisp.runtime (`runtime.harness@1`).
//!
//! What it contributes (v1 adapters/grok.js): the CLI's description, `detect` (`WISP_GROK_BIN` or
//! `grok` on PATH; `grok models` with each model's levels from `$GROK_HOME/models_cache.json`), the
//! launch (the prompt as a launch file, the protocol with `--rules`, wisp's MCP server through a
//! throwaway agent definition), a driver over `wisp-grok-core`'s parser with the file tracker on, and
//! the resume command (its TUI takes no first prompt: wisp types it in).

mod driver;
pub mod sessions;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use wisp_grok_core::{GrokArgs, GrokParser, agent_definition, args, efforts_of, model_ids};
use wisp_harness_kit::detect::version_in;
use wisp_harness_kit::protocol::{FileNote, attachment_note};
use wisp_harness_kit::{RunPaths, clip};
use wisp_kernel::{BoxFuture, Manifest, Plugin, Result, Setup};
use wisp_runtime_api::harness::{
    ExecReq, HarnessFound, HarnessInfo, HarnessLogin, HarnessModel, HarnessRun, HarnessSys, Launch, LaunchFile, ResumeCommand,
    ResumeOptions, RunDriver, SetupTip,
};
use wisp_runtime_api::model::Mode;
use wisp_runtime_api::{HARNESS, Harness};

pub const DEFAULT_BIN: &str = "grok";

pub fn info() -> HarnessInfo {
    HarnessInfo {
        id: "grok".into(),
        name: "Grok".into(),
        models: vec![HarnessModel {
            id: String::new(),
            label: "Default".into(),
            hint: Some("Whatever Grok is set to".into()),
            efforts: None,
            resolves: None,
        }],
        efforts: ["low", "medium", "high", "xhigh"].map(str::to_owned).to_vec(),
        ask_tool: Some("wisp__ask_user".into()),
        system_prompt_flag: true,
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

/// grok -p doesn't read stdin, and a prompt in argv is capped on Windows: it goes as a file.
pub fn launch(run: &HarnessRun, bin: &str) -> Launch {
    let notes: Vec<FileNote> =
        run.attachments.iter().filter_map(|a| a.path.clone().map(|path| FileNote { path, mime: a.mime.clone(), size: a.size })).collect();
    let mut files =
        vec![LaunchFile { path: "grok-prompt.md".into(), contents: format!("{}{}", run.prompt, attachment_note(&notes, None)) }];
    let agent = (!run.mcp_servers.is_empty()).then_some("{tmp}/grok-wisp.md");
    if agent.is_some() {
        files.push(LaunchFile { path: "grok-wisp.md".into(), contents: agent_definition(&run.mcp_servers) });
    }
    let a = GrokArgs {
        prompt_file: "{tmp}/grok-prompt.md",
        mode: mode(run.mode),
        rules: &run.system_prompt,
        model: &run.model,
        effort: &run.effort,
        resume_id: run.resume_id.as_deref(),
        agent_file: agent,
    };
    Launch {
        program: bin.to_owned(),
        args: args(&a),
        env: BTreeMap::from([("GROK_DISABLE_AUTOUPDATER".to_owned(), "1".to_owned())]),
        cwd: run.cwd.clone(),
        files,
        ..Launch::default()
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

/// `grok models` with each id's name, hint and levels from grok's cached catalog.
fn models_of(out: &str, cache: &Value) -> (Vec<HarnessModel>, Option<Vec<String>>) {
    let (ids, def) = model_ids(out);
    let models: Vec<HarnessModel> = ids
        .into_iter()
        .map(|id| {
            let info = &cache["models"][&id]["info"];
            let efforts =
                if info["supports_reasoning_effort"] == false { Some(Vec::new()) } else { efforts_of(&info["reasoning_efforts"]) };
            HarnessModel {
                label: info["name"].as_str().unwrap_or(&id).to_owned(),
                hint: Some(clip(info["description"].as_str().unwrap_or(&id), 60)),
                efforts,
                id,
                resolves: None,
            }
        })
        .collect();
    // Default runs grok's own default, so it takes that model's levels
    let levels = def.and_then(|d| models.iter().find(|m| m.id == d)).and_then(|m| m.efforts.clone()).filter(|e| !e.is_empty());
    (models, levels)
}

/// Signing in from wisp (v1 agent-login.js LOGINS.grok): its device login (a link and a code), its logout; `models`
/// says whether it's signed in.
pub fn login() -> HarnessLogin {
    HarnessLogin {
        bin: DEFAULT_BIN.into(),
        bin_env: "WISP_GROK_BIN".into(),
        args: vec!["login".into(), "--device-auth".into()],
        logout: vec!["logout".into()],
        status: vec!["models".into()],
        keys: vec!["XAI_API_KEY".into()],
        ..HarnessLogin::default()
    }
}

#[derive(Debug)]
pub struct GrokHarness {
    info: HarnessInfo,
    bin: Mutex<Option<String>>,
    login: HarnessLogin,
}

impl Default for GrokHarness {
    fn default() -> Self {
        Self { info: info(), bin: Mutex::new(None), login: login() }
    }
}

impl GrokHarness {
    fn bin(&self) -> String {
        self.bin
            .lock()
            .ok()
            .and_then(|b| b.clone())
            .or_else(|| std::env::var("WISP_GROK_BIN").ok().filter(|b| !b.is_empty()))
            .unwrap_or_else(|| DEFAULT_BIN.to_owned())
    }
}

/// The days `live_limits` totals (as opencode's `stats --days 7`).
const SPEND_DAYS: i64 = 7;
/// At most this many sessions are read per pass (one `grok usage` each), newest first.
const SPEND_SESSIONS: usize = 40;

impl GrokHarness {
    /// What Grok's own records say was spent since `since` (ms): `grok usage <id>` for each session under
    /// `<GROK_HOME>/sessions/<project>/<id>/` whose `summary.json` says it was updated since then (at most
    /// [`SPEND_SESSIONS`], newest first), summed over the turns that ended in the time. Nothing spent, the CLI asked.
    async fn spent_since(&self, sys: &dyn HarnessSys, env: &BTreeMap<String, String>, since: i64) -> Option<Value> {
        let home = sys.env("GROK_HOME").filter(|h| !h.is_empty()).unwrap_or_else(|| format!("{}/.grok", sys.home()));
        let root = format!("{home}/sessions");
        let mut found: Vec<(i64, String)> = Vec::new();
        for project in sys.list_dir(&root).await {
            for id in sys.list_dir(&format!("{root}/{project}")).await {
                if !wisp_runtime_api::harness::sessions::safe_id(&id) {
                    continue;
                }
                let Some(text) = sys.read_file(&format!("{root}/{project}/{id}/summary.json")).await else { continue };
                let at = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|v| v.get("updated_at").and_then(Value::as_str).map(wisp_runtime_api::harness::sessions::iso_ms))
                    .unwrap_or(0);
                if at >= since {
                    found.push((at, id));
                }
            }
        }
        found.sort_unstable_by(|a, b| b.cmp(a));
        found.truncate(SPEND_SESSIONS);
        let mut sessions = Vec::new();
        for (_, id) in found {
            let req =
                ExecReq { program: self.bin(), args: vec!["usage".into(), id], env: env.clone(), timeout_ms: 10_000, ..ExecReq::default() };
            // no usage recorded: a non-zero exit, skipped
            let Some(out) = sys.exec(req).await.ok().filter(|o| o.code == Some(0) && !o.timed_out) else { continue };
            sessions.extend(wisp_grok_core::usage(&out.stdout));
        }
        let s = wisp_grok_core::spent(&sessions, since, &wisp_runtime_api::harness::sessions::iso_ms);
        let mut v = serde_json::json!({ "windows": [], "tokens": s.tokens, "costDays": SPEND_DAYS });
        if let Some(c) = s.cost_usd {
            v["cost"] = c.into();
        }
        Some(v)
    }
}

impl Harness for GrokHarness {
    fn info(&self) -> &HarnessInfo {
        &self.info
    }
    fn login(&self) -> Option<&HarnessLogin> {
        Some(&self.login)
    }
    /// "not authenticated" is out; "logged in", or `grok models` answering at all, is in (v1 read only "logged in", so a
    /// sign-in that worked ended "failed": the list of models says neither)
    fn signed_in(&self, out: &str, code: Option<i32>) -> Option<bool> {
        let o = out.to_lowercase();
        if o.contains("not authenticated") || o.contains("not signed in") {
            Some(false)
        } else if o.contains("logged in") || (code == Some(0) && !o.trim().is_empty()) {
            Some(true)
        } else {
            None
        }
    }
    fn vendor(&self) -> Option<&str> {
        Some("xai")
    }

    fn detect<'a>(&'a self, sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        Box::pin(async move {
            let bin = match sys.env("WISP_GROK_BIN").filter(|b| !b.is_empty()) {
                Some(b) => Some(b),
                None => sys.which(DEFAULT_BIN).await,
            };
            let Some(bin) = bin else {
                return HarnessFound { available: false, error: Some("Grok isn't installed".into()), ..HarnessFound::default() };
            };
            // both at once: a CLI that hangs costs one time limit, not two
            let found = wisp_harness_kit::probe::gated(output(sys, &bin, &["--version"]), output(sys, &bin, &["models"])).await;
            let Some((line, listed)) = found else {
                return HarnessFound { available: false, error: Some("grok --version failed".into()), ..HarnessFound::default() };
            };
            if let Ok(mut b) = self.bin.lock() {
                *b = Some(bin.clone());
            }
            let v = version_in(&line);
            let mut found = HarnessFound {
                available: true,
                version: Some(if v.is_empty() { line.trim().to_owned() } else { v }),
                bin: Some(bin.clone()),
                ..HarnessFound::default()
            };
            let home = sys.env("GROK_HOME").filter(|h| !h.is_empty()).unwrap_or_else(|| format!("{}/.grok", sys.home()));
            let cache = sys
                .read_file(&format!("{home}/models_cache.json"))
                .await
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            let (models, levels) = models_of(&listed.unwrap_or_default(), &cache);
            if !models.is_empty() {
                found.efforts = levels;
                let mut all = vec![self.info.models[0].clone()];
                all.extend(models);
                found.models = Some(all);
            }
            found
        })
    }

    fn program(&self) -> Option<String> {
        Some(self.bin())
    }

    /// No windows: what Grok's own `grok usage` records total for the last [`SPEND_DAYS`] days (ADR-0123).
    fn live_limits<'a>(&'a self, sys: &'a dyn HarnessSys, env: &'a BTreeMap<String, String>) -> BoxFuture<'a, Option<Value>> {
        Box::pin(async move {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_millis();
            let now = i64::try_from(now).ok()?;
            self.spent_since(sys, env, now - SPEND_DAYS * 86_400_000).await
        })
    }

    fn launch(&self, run: &HarnessRun) -> Result<Launch> {
        Ok(launch(run, &run.bin.clone().unwrap_or_else(|| self.bin())))
    }

    fn driver(&self, run: &HarnessRun) -> Box<dyn RunDriver> {
        let paths = RunPaths::same(run.cwd.clone());
        driver::KitDriver::boxed(GrokParser::new(paths.clone()), paths, true)
    }

    fn setup(&self, _platform: &str) -> Option<SetupTip> {
        Some(SetupTip { tip: "Install Grok Build (grok) and sign in".into(), command: None, url: Some("https://x.ai/cli".into()) })
    }

    fn resume_command(&self, o: &ResumeOptions) -> Option<ResumeCommand> {
        let mut argv = vec![self.bin()];
        if let Some(r) = o.resume_id.as_ref().filter(|r| !r.is_empty()) {
            argv.extend(["--resume".to_owned(), r.clone()]);
        }
        if !o.model.is_empty() {
            argv.extend(["--model".to_owned(), o.model.clone()]);
        }
        if !o.effort.is_empty() {
            argv.extend(["--reasoning-effort".to_owned(), o.effort.clone()]);
        }
        if o.trusted {
            argv.push("--trust".into());
        }
        Some(ResumeCommand { argv, types_prompt: true })
    }
}

#[derive(Debug, Default)]
pub struct GrokPlugin;

impl Plugin for GrokPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let h: Arc<dyn Harness> = Arc::new(GrokHarness::default());
        s.contribute(&HARNESS, h);
        let sessions: Arc<dyn wisp_runtime_api::harness::sessions::HarnessSessions> = Arc::new(crate::sessions::GrokSessions::from_env());
        s.contribute(&wisp_runtime_api::harness::sessions::SESSIONS, sessions);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The owner's Agents audit (2026-11-18): v1 signed in to and out of Grok from Settings → Agents (agent-login.js
    /// LOGINS.grok); v2 had no login for it.
    #[test]
    fn grok_signs_in_and_out_from_wisp_as_v1() {
        let h = GrokHarness::default();
        let l = h.login().unwrap();
        assert_eq!((l.bin.as_str(), l.bin_env.as_str()), ("grok", "WISP_GROK_BIN"));
        assert_eq!(l.args, vec!["login".to_owned(), "--device-auth".to_owned()]);
        assert_eq!((l.logout.clone(), l.status.clone()), (vec!["logout".to_owned()], vec!["models".to_owned()]));
        assert_eq!(l.keys, vec!["XAI_API_KEY".to_owned()]);
        assert_eq!(h.signed_in("Logged in as me", Some(0)), Some(true));
        assert_eq!(h.signed_in("Error: not authenticated", Some(1)), Some(false));
        // its models listed: signed in (v1 said nothing, and a sign-in that worked ended failed)
        assert_eq!(h.signed_in("grok-4.7\ngrok-4.6", Some(0)), Some(true));
        assert_eq!(h.signed_in("", Some(1)), None);
    }

    #[test]
    fn manifest_is_valid() {
        assert_eq!(GrokPlugin.manifest().id, "wisp.grok");
    }

    #[test]
    fn detect_reads_each_models_levels_from_the_cache() {
        // v1 adapters.test.js: GROK_HOME's models_cache.json gives grok-9 its levels
        let cache = json!({ "models": { "grok-9": { "info": { "name": "Grok 9", "description": "Big", "supports_reasoning_effort": true, "reasoning_efforts": [{ "id": "xhigh" }, { "id": "high" }, { "id": "low" }] } } } });
        let (m, levels) = models_of("  * grok-9 (default)\n  - grok-8\n", &cache);
        assert_eq!((m[0].label.as_str(), m[0].efforts.as_ref().map_or(0, Vec::len), m[1].label.as_str()), ("Grok 9", 3, "grok-8"));
        assert_eq!(levels, Some(vec!["low".to_owned(), "high".to_owned(), "xhigh".to_owned()]));
    }

    #[test]
    fn the_prompt_and_agent_go_as_files() {
        let mut run =
            HarnessRun { prompt: "fix".into(), cwd: "/w".into(), system_prompt: "SYS".into(), mode: Mode::Plan, ..HarnessRun::default() };
        run.mcp_servers.insert("wisp".into(), json!({ "type": "http", "url": "http://x/api/mcp" }));
        let l = launch(&run, "grok");
        assert_eq!(&l.args[..2], ["--prompt-file", "{tmp}/grok-prompt.md"]);
        assert!(l.args.windows(2).any(|w| w == ["--rules", "SYS"]) && l.args.windows(2).any(|w| w == ["--agent", "{tmp}/grok-wisp.md"]));
        assert_eq!((l.files.len(), l.files[0].contents.as_str()), (2, "fix"));
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

    /// ADR-0123: Grok reports no windows; `live_limits` runs `grok usage <id>` (captured: `core/tests/cli/usage*.json`)
    /// for each session updated in the time, skips one older than it and one with no usage recorded (exit 1), and sums
    /// the turns that ended in it: the cost in ticks as dollars, and the tokens.
    #[tokio::test]
    async fn live_limits_sums_grok_usage_over_the_recent_sessions() {
        let root = "/home/test/.grok/sessions";
        let mut sys = FakeSys::default();
        sys.dirs.insert(root.into(), vec!["%2Fw".into(), "session_search.sqlite".into()]);
        let ids =
            ["01a154df-836c-7cd3-9f61-192d1864227a", "01a155d3-5b2d-7e93-b242-e60d2d6fe266", "01a154e6-9e97-7570-b897-f36e4040a60d", "old"];
        sys.dirs.insert(format!("{root}/%2Fw"), ids.iter().map(|s| (*s).to_owned()).collect());
        let updated = ["2026-10-19T15:56:04.888584Z", "2026-10-19T20:21:21.200000Z", "2026-10-19T16:00:00.000000Z", "2026-10-01T00:00:00Z"];
        for (id, at) in ids.iter().zip(updated) {
            sys.files.insert(format!("{root}/%2Fw/{id}/summary.json"), json!({ "info": { "id": id }, "updated_at": at }).to_string());
        }
        sys.out.insert(format!("usage {}", ids[0]), (0, include_str!("../../core/tests/cli/usage.json").into()));
        sys.out.insert(format!("usage {}", ids[1]), (0, include_str!("../../core/tests/cli/usage-empty.json").into()));
        // ids[2]: "Error: No usage recorded", exit 1 (the fake's default)
        let since = wisp_runtime_api::harness::sessions::iso_ms("2026-10-19T00:00:00Z");
        let got = GrokHarness::default().spent_since(&sys, &BTreeMap::new(), since).await.unwrap_or_default();
        assert_eq!((got["windows"].clone(), got["tokens"].as_u64(), got["costDays"].as_u64()), (json!([]), Some(136_102), Some(7)));
        assert!((got["cost"].as_f64().unwrap_or_default() - 0.118_592).abs() < 1e-12, "{got}");
        let seen = sys.seen.lock().map(|s| s.clone()).unwrap_or_default();
        assert_eq!(seen.len(), 3, "the October 1 session isn't read: {seen:?}");
        assert!(seen.iter().all(|a| a[0] == "usage"));
        // nothing recorded at all: tokens 0 and no cost, never "$0.00"
        let none = GrokHarness::default().spent_since(&FakeSys::default(), &BTreeMap::new(), since).await.unwrap_or_default();
        assert_eq!((none["tokens"].as_u64(), none.get("cost")), (Some(0), None));
    }
}
