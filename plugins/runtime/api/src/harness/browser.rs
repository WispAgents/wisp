//! The browser a run's agent drives (ADR-0128): the `runtime.browser` setting (and its per-harness override), what it
//! resolves to for one harness on one computer, and the `browser` MCP server wisp's runtime adds for it. Plain data and
//! pure functions: the runtime reads the environment (a sandbox, its `wisp-browser-mcp`, a system Chrome) and passes it
//! in, so a harness's launch tests build the same server the runtime would.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

/// Playwright's MCP server, pinned: an unpinned `npx` would run whatever npm serves that day, on every run. The sandbox
/// image (`grid/deploy/image/Dockerfile`) installs this same version; a runtime test compares the two.
pub const PLAYWRIGHT_MCP: &str = "@playwright/mcp@0.0.86";

/// The name of the MCP server the browser part adds (`mcp__browser__*`); a connector the user named so wins.
pub const BROWSER_SERVER: &str = "browser";

/// The sandbox image's launcher for Playwright's MCP server on the box's own Chromium (`grid/deploy/image/sandbox`).
pub const SANDBOX_BROWSER_BIN: &str = "wisp-browser-mcp";

/// What a run's agent may drive.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "runtime/")]
pub enum Browser {
    /// nothing set: the harness's own default (Claude Code follows its `/chrome` choice; the others have none)
    #[default]
    Default,
    Off,
    /// the user's own Chrome: Claude in Chrome on Claude Code, Playwright on the system Chrome elsewhere
    Chrome,
    /// Playwright's own headless browser, on a fresh profile each run
    Headless,
    /// a sandbox's Chromium, the one the user watches (`wisp-browser-mcp`, over CDP)
    Sandbox,
}

impl Browser {
    /// A setting's value; anything else is unset.
    pub fn parse(v: &str) -> Self {
        match v {
            "off" => Self::Off,
            "chrome" => Self::Chrome,
            "headless" => Self::Headless,
            "sandbox" => Self::Sandbox,
            _ => Self::Default,
        }
    }
}

/// What a harness can do with a browser, as the picker and the runtime both read it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct BrowserSupport {
    /// it takes per-run MCP servers (Cursor and Antigravity don't): the browser part reaches it
    pub mcp: bool,
    /// its CLI drives the user's own Chrome itself (Claude in Chrome, `--chrome`)
    pub own_chrome: bool,
    /// this wisp is a sandbox's (`WISP_SANDBOX=1`): there is no Chrome of the user's, only the box's Chromium
    pub sandbox: bool,
}

/// The browser a run gets: `setting` (the harness's override, else the global one) on a harness that can do `can`.
/// A harness without per-run MCP and its own Chrome gets none; in a sandbox anything not off is the box's browser
/// (Claude in Chrome needs the user's Chrome, which a sandbox doesn't have).
pub fn resolve(setting: Browser, can: BrowserSupport) -> Browser {
    if setting == Browser::Off || (!can.mcp && !can.own_chrome) {
        return Browser::Off;
    }
    if can.sandbox {
        return if can.mcp { Browser::Sandbox } else { Browser::Off };
    }
    match setting {
        // the harness's own default is its own Chrome, when it has one; the others had no browser (v1)
        Browser::Default if can.own_chrome => Browser::Default,
        // its own Chrome, or Playwright on it (it takes MCP: the early return)
        Browser::Chrome => Browser::Chrome,
        // a sandbox's value outside a sandbox: Playwright's headless one
        Browser::Headless | Browser::Sandbox if can.mcp => Browser::Headless,
        Browser::Default | Browser::Off | Browser::Headless | Browser::Sandbox => Browser::Off,
    }
}

/// The `browser` MCP server for `browser` (already [`resolve`]d), None when the run needs none: off, the harness's
/// default, or Chrome on a harness that drives Chrome itself. `sandbox_bin`: the box's `wisp-browser-mcp`, if found;
/// `system_chrome`: a Chrome is installed here, which Playwright then drives instead of downloading its own.
pub fn mcp_server(browser: Browser, own_chrome: bool, sandbox_bin: Option<&str>, system_chrome: bool) -> Option<Value> {
    match browser {
        Browser::Off | Browser::Default => None,
        Browser::Chrome if own_chrome => None,
        // CDP to the Chromium the user watches; never `--browser chrome` (the box is Linux, with chromium only)
        Browser::Sandbox => Some(sandbox_bin.map_or_else(|| npx(false), |bin| json!({ "type": "stdio", "command": bin, "args": [] }))),
        Browser::Chrome | Browser::Headless => Some(npx(system_chrome)),
    }
}

fn npx(system_chrome: bool) -> Value {
    let mut args = vec!["-y", PLAYWRIGHT_MCP, "--headless", "--isolated"];
    if system_chrome {
        args.extend(["--browser", "chrome"]);
    }
    json!({ "type": "stdio", "command": "npx", "args": args })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const HERE: BrowserSupport = BrowserSupport { mcp: true, own_chrome: false, sandbox: false };

    #[test]
    fn a_setting_resolves_per_harness_and_computer() {
        let claude = BrowserSupport { own_chrome: true, ..HERE };
        let cursor = BrowserSupport { mcp: false, ..HERE };
        assert_eq!(resolve(Browser::Default, claude), Browser::Default, "Claude follows its own /chrome choice");
        assert_eq!(resolve(Browser::Default, HERE), Browser::Off, "the others had none (v1)");
        assert_eq!(resolve(Browser::Chrome, claude), Browser::Chrome);
        assert_eq!(resolve(Browser::Chrome, HERE), Browser::Chrome);
        assert_eq!(resolve(Browser::Headless, HERE), Browser::Headless);
        for b in [Browser::Default, Browser::Chrome, Browser::Headless] {
            assert_eq!(resolve(b, cursor), Browser::Off, "no per-run MCP: never a browser");
            for h in [HERE, claude] {
                assert_eq!(resolve(b, BrowserSupport { sandbox: true, ..h }), Browser::Sandbox, "a sandbox's own browser");
            }
        }
        assert_eq!(resolve(Browser::Off, BrowserSupport { sandbox: true, ..claude }), Browser::Off);
    }

    #[test]
    fn the_server_is_pinned_playwright_or_the_sandboxs_launcher() {
        let s = mcp_server(Browser::Headless, false, None, false).unwrap();
        assert_eq!(s["command"], "npx");
        assert_eq!(s["args"], json!(["-y", PLAYWRIGHT_MCP, "--headless", "--isolated"]));
        let c = mcp_server(Browser::Chrome, false, None, true).unwrap();
        assert_eq!(c["args"], json!(["-y", PLAYWRIGHT_MCP, "--headless", "--isolated", "--browser", "chrome"]));
        assert_eq!(mcp_server(Browser::Chrome, true, None, true), None, "Claude in Chrome needs no server");
        assert_eq!(mcp_server(Browser::Off, false, None, true), None);
        assert_eq!(mcp_server(Browser::Default, true, None, true), None);
        let sb = mcp_server(Browser::Sandbox, true, Some("/usr/local/bin/wisp-browser-mcp"), true).unwrap();
        assert_eq!(sb["command"], "/usr/local/bin/wisp-browser-mcp");
        let bare = mcp_server(Browser::Sandbox, false, None, true).unwrap();
        assert!(!bare["args"].as_array().unwrap().iter().any(|a| a == "chrome"), "a sandbox's Linux has chromium only");
    }

    #[test]
    fn an_old_run_without_a_browser_decodes_as_the_default() {
        assert_eq!(Browser::parse("headless"), Browser::Headless);
        assert_eq!(Browser::parse(""), Browser::Default);
        assert_eq!(serde_json::to_value(Browser::Headless).unwrap(), "headless");
    }
}
