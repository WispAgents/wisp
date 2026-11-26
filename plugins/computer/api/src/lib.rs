//! wisp-computer-api: the contract of `wisp.computer`, computer use for every harness (ADR-0127).
//!
//! wisp doesn't drive screens itself: Cua Driver (`cua-driver mcp`, MIT, trycua/cua) does, on macOS, Linux
//! and Windows. This plugin is the layer around it: one MCP server, `computer`, that every task's run gets
//! (Claude Code, Codex, opencode, Copilot, Grok: whatever takes MCP servers), answered by wisp at
//! [`MCP_PATH`] with the task's own token, and passed on to one supervised driver. wisp decides what a task
//! may call (off unless switched on, read-only tools in Plan mode, a short deny list), so the user's harness
//! never talks to the driver directly. On a grid box or a sandbox the image ships the driver, a screen and
//! a browser (whose Playwright server is ADR-0128's).

use serde::{Deserialize, Serialize};
use wisp_kernel::Command;

/// The plugin id (manifest `id`).
pub const PLUGIN_ID: &str = "wisp.computer";

/// `POST /api/computer/mcp`: the `computer` MCP server, Streamable HTTP without the stream.
pub const MCP_PATH: &str = "/computer/mcp";

/// The MCP server's name in a run's config (the harness shows `mcp__computer__click`).
pub const SERVER: &str = "computer";

/// The setting that switches computer use on (off by default, D-0023).
pub const SETTING_ENABLED: &str = "computer.enabled";

/// Env: `on` switches it on whatever the setting says (a grid box's image sets it: the box is the sandbox).
pub const ENV_ON: &str = "WISP_COMPUTER";
/// Env: the driver's command (a grid box's image points it at `wisp-computer`, which starts the screen first).
pub const ENV_DRIVER: &str = "WISP_COMPUTER_DRIVER";

/// The driver's command when [`ENV_DRIVER`] isn't set.
pub const DRIVER_BIN: &str = "cua-driver";
/// The Cua Driver release wisp is built and tested against (the grid image pins the same one).
pub const DRIVER_VERSION: &str = "0.62.1";
/// Where a person installs it (macOS and Linux; Windows: `install.ps1`).
pub const INSTALL_HINT: &str = "Install Cua Driver: /bin/bash -c \"$(curl -fsSL https://cua.ai/driver/install.sh)\"";

/// Permissions, each a command's id (`computer.status` → `computer:status`). Default deny: a capability set
/// names them.
pub mod permissions {
    /// call the `computer` MCP server (the route)
    pub const USE: &str = "computer:use";
    /// read whether computer use is on and its driver found
    pub const STATUS: &str = "computer:status";
}

/// `computer.status`'s answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// switched on (Settings, or `WISP_COMPUTER=on`)
    pub enabled: bool,
    /// the driver's command as found, or none
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub driver: Option<String>,
    /// what `cua-driver --version` printed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub version: Option<String>,
    /// the driver is running now
    pub running: bool,
    /// what to do next when it can't work yet, in words
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct Empty {}

/// `computer.status`: whether this wisp's tasks get computer use, and why not.
#[derive(Debug)]
pub struct StatusCmd;
impl Command for StatusCmd {
    const ID: &'static str = "computer.status";
    const READ_ONLY: bool = true;
    type Args = Empty;
    // it takes nothing: `wisp computer status --x` is a usage error
    fn args_ts() -> Option<String> {
        Some(format!(
            "{}{}",
            <Empty as ::ts_rs::TS>::docs().unwrap_or_default(),
            <Empty as ::ts_rs::TS>::inline(&::ts_rs::Config::default())
        ))
    }
    type Out = Status;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_camel_case_and_drops_what_it_doesnt_know() {
        let s = Status { enabled: true, running: false, ..Status::default() };
        let v = serde_json::to_value(&s).ok();
        assert_eq!(v, Some(serde_json::json!({ "enabled": true, "running": false })));
    }
}
