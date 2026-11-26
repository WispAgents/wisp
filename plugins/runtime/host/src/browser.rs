//! The run's browser (ADR-0128): `runtime.browser` (Settings → Agents, every harness) and `runtime.browsers` (one
//! harness's own, from its detail or the picker), resolved for the run's harness on this computer, and the `browser`
//! MCP server it needs, which the runtime adds beside the connectors (never over a connector the user named
//! `browser`). What it reads of the system (a sandbox, its `wisp-browser-mcp`, a Chrome) comes through `env` and
//! `is_file`, so a test sets them without touching the process's environment.

use std::path::{Path, PathBuf};

use serde_json::Value;
use wisp_runtime_api::harness::browser::{self, Browser, BrowserSupport, SANDBOX_BROWSER_BIN};

/// The setting every harness follows.
pub(crate) const SETTING: &str = "runtime.browser";
/// One harness's own (`{ <harness>: "off" | "chrome" | "headless" }`); a harness it doesn't name follows [`SETTING`].
pub(crate) const PER_HARNESS: &str = "runtime.browsers";

/// What the system looks like to the browser part.
pub(crate) struct Sys<'a> {
    pub env: &'a dyn Fn(&str) -> Option<String>,
    pub is_file: &'a dyn Fn(&Path) -> bool,
}

pub(crate) fn real_env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

pub(crate) fn real_is_file(p: &Path) -> bool {
    p.is_file()
}

const REAL: Sys<'static> = Sys { env: &real_env, is_file: &real_is_file };

/// A harness's browser support here: `own_chrome` and `mcp` from its `HarnessInfo`, the sandbox from the environment.
pub(crate) fn support(own_chrome: bool, mcp: bool) -> BrowserSupport {
    support_in(own_chrome, mcp, &REAL)
}

fn support_in(own_chrome: bool, mcp: bool, sys: &Sys<'_>) -> BrowserSupport {
    BrowserSupport { mcp, own_chrome, sandbox: (sys.env)("WISP_SANDBOX").as_deref() == Some("1") }
}

/// The setting for `harness`: its own when it names one, else every harness's.
pub(crate) fn setting_of(global: Option<&Value>, own: Option<&Value>, harness: &str) -> Browser {
    let mine = own.and_then(|o| o.get(harness)).and_then(Value::as_str).map(Browser::parse).filter(|b| *b != Browser::Default);
    mine.unwrap_or_else(|| global.and_then(Value::as_str).map_or(Browser::Default, Browser::parse))
}

/// `name` as a program: a path as it is, a bare name on `PATH`.
fn on_path(name: &str, sys: &Sys<'_>) -> Option<String> {
    if name.contains('/') || name.contains('\\') {
        return (sys.is_file)(Path::new(name)).then(|| name.to_owned());
    }
    let path = (sys.env)("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| (sys.is_file)(p)).map(|p| p.display().to_string())
}

/// The sandbox image's launcher (`WISP_BROWSER_MCP_BIN`, else `wisp-browser-mcp`), when it's there.
fn sandbox_bin(sys: &Sys<'_>) -> Option<String> {
    let name = (sys.env)("WISP_BROWSER_MCP_BIN").filter(|b| !b.is_empty()).unwrap_or_else(|| SANDBOX_BROWSER_BIN.to_owned());
    on_path(&name, sys)
}

/// A Chrome where Playwright's `--browser chrome` looks for one: then it drives that instead of downloading its own.
fn system_chrome(sys: &Sys<'_>) -> bool {
    let env = |k: &str| (sys.env)(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let fixed: Vec<PathBuf> = match std::env::consts::OS {
        "macos" => {
            let app = "Google Chrome.app/Contents/MacOS/Google Chrome";
            [Some(PathBuf::from("/Applications")), env("HOME").map(|h| h.join("Applications"))]
                .into_iter()
                .flatten()
                .map(|d| d.join(app))
                .collect()
        }
        "windows" => ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
            .iter()
            .filter_map(|k| env(k))
            .map(|d| d.join("Google").join("Chrome").join("Application").join("chrome.exe"))
            .collect(),
        _ => vec![PathBuf::from("/opt/google/chrome/chrome")],
    };
    fixed.iter().any(|p| (sys.is_file)(p)) || ["google-chrome", "google-chrome-stable"].iter().any(|n| on_path(n, sys).is_some())
}

/// The run's browser and the `browser` server it needs (None: none, or the harness drives it itself).
pub(crate) fn for_run(setting: Browser, own_chrome: bool, mcp: bool) -> (Browser, Option<Value>) {
    for_run_in(setting, own_chrome, mcp, &REAL)
}

fn for_run_in(setting: Browser, own_chrome: bool, mcp: bool, sys: &Sys<'_>) -> (Browser, Option<Value>) {
    let b = browser::resolve(setting, support_in(own_chrome, mcp, sys));
    let server = match b {
        Browser::Off | Browser::Default => None,
        Browser::Chrome if own_chrome => None,
        Browser::Sandbox => browser::mcp_server(b, own_chrome, sandbox_bin(sys).as_deref(), false),
        _ => browser::mcp_server(b, own_chrome, None, system_chrome(sys)),
    };
    (b, server)
}

impl crate::runtime::Runtime {
    /// The browser for a run on `agent` (its own setting, else every harness's) and its `browser` server, if any.
    pub(crate) fn run_browser(&self, agent: &str, info: &wisp_runtime_api::harness::HarnessInfo) -> (Browser, Option<Value>) {
        let setting = setting_of(self.host.setting(SETTING).as_ref(), self.host.setting(PER_HARNESS).as_ref(), agent);
        for_run(setting, info.own_chrome, info.ask_tool.is_some())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;
    use wisp_runtime_api::harness::browser::PLAYWRIGHT_MCP;

    use super::*;

    fn with<'a>(
        env: &'a BTreeMap<&'static str, String>,
        files: &'a [PathBuf],
    ) -> (impl Fn(&str) -> Option<String> + 'a, impl Fn(&Path) -> bool + 'a) {
        (move |k: &str| env.get(k).cloned(), move |p: &Path| files.iter().any(|f| f == p))
    }

    #[test]
    fn the_browser_part_follows_each_setting_value_with_the_pinned_version() {
        let env = BTreeMap::from([("PATH", "/nowhere".to_owned())]);
        let (e, f) = with(&env, &[]);
        let sys = Sys { env: &e, is_file: &f };
        // a harness that takes MCP but has no Chrome of its own (Codex, opencode, Copilot, Grok)
        for (setting, present) in [(Browser::Default, false), (Browser::Off, false), (Browser::Chrome, true), (Browser::Headless, true)] {
            let (_, s) = for_run_in(setting, false, true, &sys);
            assert_eq!(s.is_some(), present, "{setting:?}");
            if let Some(s) = s {
                assert_eq!(s["command"], "npx");
                assert_eq!(s["args"][1], PLAYWRIGHT_MCP, "pinned");
                assert_eq!(s["args"][1], "@playwright/mcp@0.0.86");
            }
        }
        // Claude Code: its own Chrome by flag, no server; headless is the server
        assert_eq!(for_run_in(Browser::Chrome, true, true, &sys), (Browser::Chrome, None));
        assert_eq!(for_run_in(Browser::Default, true, true, &sys), (Browser::Default, None));
        assert!(for_run_in(Browser::Headless, true, true, &sys).1.is_some());
        // Cursor, Antigravity: no per-run MCP, never a browser
        for b in [Browser::Chrome, Browser::Headless] {
            assert_eq!(for_run_in(b, false, false, &sys), (Browser::Off, None));
        }
    }

    #[test]
    fn a_system_chrome_is_what_playwright_drives() {
        let env = BTreeMap::from([("PATH", "/usr/bin".to_owned())]);
        let files =
            [PathBuf::from("/usr/bin/google-chrome"), PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")];
        let (e, f) = with(&env, &files);
        let (_, s) = for_run_in(Browser::Headless, false, true, &Sys { env: &e, is_file: &f });
        let args = s.unwrap()["args"].clone();
        assert_eq!(args, json!(["-y", PLAYWRIGHT_MCP, "--headless", "--isolated", "--browser", "chrome"]));
    }

    /// In a sandbox (`WISP_SANDBOX=1`) with its `wisp-browser-mcp` on PATH, every setting but off is the box's browser,
    /// on Claude Code too (Claude in Chrome needs the user's own Chrome); never `--browser chrome` there.
    #[test]
    fn in_a_sandbox_the_browser_is_the_boxs_wisp_browser_mcp() {
        let dir = std::env::temp_dir().join(format!("wisp-browser-bin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join(SANDBOX_BROWSER_BIN);
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        let env = BTreeMap::from([("WISP_SANDBOX", "1".to_owned()), ("PATH", format!("/nowhere:{}", dir.display()))]);
        let e = |k: &str| env.get(k).cloned();
        let sys = Sys { env: &e, is_file: &real_is_file };
        for (own, setting) in [(true, Browser::Chrome), (true, Browser::Default), (false, Browser::Headless), (false, Browser::Chrome)] {
            let (b, s) = for_run_in(setting, own, true, &sys);
            assert_eq!(b, Browser::Sandbox, "{setting:?}");
            assert_eq!(s.unwrap()["command"], bin.display().to_string());
        }
        assert_eq!(for_run_in(Browser::Off, true, true, &sys), (Browser::Off, None));
        // WISP_BROWSER_MCP_BIN names another
        let named = BTreeMap::from([("WISP_SANDBOX", "1".to_owned()), ("WISP_BROWSER_MCP_BIN", bin.display().to_string())]);
        let e = |k: &str| named.get(k).cloned();
        let (_, s) = for_run_in(Browser::Headless, false, true, &Sys { env: &e, is_file: &real_is_file });
        assert_eq!(s.unwrap()["command"], bin.display().to_string());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_harness_s_own_setting_wins_over_every_harness_s() {
        let own = json!({ "codex": "headless", "claude": "default" });
        assert_eq!(setting_of(Some(&json!("off")), Some(&own), "codex"), Browser::Headless);
        assert_eq!(setting_of(Some(&json!("off")), Some(&own), "claude"), Browser::Off, "default follows every harness's");
        assert_eq!(setting_of(None, None, "grok"), Browser::Default);
    }

    /// The sandbox image installs the version the runtime pins (one constant; ADR-0128).
    #[test]
    fn the_sandbox_image_pins_the_same_playwright_mcp() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../grid/deploy/image/Dockerfile");
        let docker = std::fs::read_to_string(file).unwrap();
        let line = docker.lines().find(|l| l.contains("npm install -g @playwright/mcp")).unwrap();
        assert!(line.contains(&format!("npm install -g {PLAYWRIGHT_MCP} ")), "{line}");
    }
}
