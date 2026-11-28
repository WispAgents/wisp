//! The product is a list (plan §6.3): the ONLY place wisp's features are named. Each group is plugin
//! names in order; a plugin is its host crate `wisp-<name>` (id `wisp.<name>`), compiled in behind the
//! cargo feature of the same name, so a build drops any plugin by leaving its feature out. A name whose
//! crate isn't in this build is skipped (and `wisp plugin` says so).
//!   `DefaultPlugins`   everything wisp ships: `wisp` on a laptop and the desktop app
//!   `BoxPlugins`       Default minus machines and update: a grid box (its proc is the grid's)
//!   `HubPlugins`       Box with machines: a team's hub box, whose sandbox plugin adds the sandboxes it makes (ADR-0148)
//!   `HeadlessPlugins`  runtime, the harnesses, git, mcp: CI, `wisp mcp`, servers
//!   `MinimalPlugins`   runtime, one harness: the smallest working wisp, and the base for tests
use std::path::PathBuf;

use wisp_kernel::PluginGroup;

/// What a plugin may need to be built: wisp's home and the user's.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "worktrees"), allow(dead_code))]
pub struct Where {
    pub home: PathBuf,
    pub user_home: PathBuf,
}

pub const DEFAULT: [&str; 54] = [
    "cli",
    "events",
    "threads",
    "resources",
    "workflows",
    "factory",
    "share",
    "history",
    "settings",
    "runtime",
    "claude",
    "codex",
    "opencode",
    "cursor",
    "antigravity",
    "grok",
    "copilot",
    "accounts",
    "git",
    "worktrees",
    "ship",
    "review",
    "risk",
    "issues",
    "terminal",
    "lsp",
    "machine",
    "machines",
    "grid-client",
    "crew",
    "goal",
    "later",
    "budget",
    "router",
    "automations",
    "learn",
    "channels",
    "telegram",
    "slack",
    "discord",
    "github",
    "email",
    "ntfy",
    "push",
    "mcp",
    "connectors",
    "stuff",
    "plugins",
    "telemetry",
    "posthog",
    "update",
    "termctx",
    "sandbox",
    "computer",
];
/// A grid box serves the web frontend only (ADR-0024).
pub const BOX_OFF: [&str; 3] = ["machines", "update", "cli"];
/// A team's hub box (ADR-0148 §8): a box that keeps machines, so its sandboxes are peers it reaches. People there still
/// can't (every box and hub set denies `machines:*`); only plugins, on their own grants.
pub const HUB_OFF: [&str; 2] = ["update", "cli"];
pub const HEADLESS: [&str; 16] = [
    "cli",
    "resources",
    "workflows",
    "share",
    "settings",
    "runtime",
    "claude",
    "codex",
    "opencode",
    "cursor",
    "antigravity",
    "grok",
    "copilot",
    "git",
    "worktrees",
    "mcp",
];
pub const MINIMAL: [&str; 5] = ["cli", "resources", "settings", "runtime", "claude"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Default,
    Box,
    Hub,
    Headless,
    Minimal,
}

impl Group {
    /// `WISP_PLUGINS=default|box|hub|headless|minimal` (a box defaults to `box`, a team's hub box to `hub`).
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "default" => Some(Self::Default),
            "box" => Some(Self::Box),
            "hub" => Some(Self::Hub),
            "headless" => Some(Self::Headless),
            "minimal" => Some(Self::Minimal),
            _ => None,
        }
    }

    pub fn names(self) -> Vec<&'static str> {
        let mut seen: Vec<&'static str> = Vec::new();
        let all: Vec<&'static str> = match self {
            Self::Default => DEFAULT.to_vec(),
            Self::Box => DEFAULT.iter().copied().filter(|n| !BOX_OFF.contains(n)).collect(),
            Self::Hub => DEFAULT.iter().copied().filter(|n| !HUB_OFF.contains(n)).collect(),
            Self::Headless => HEADLESS.to_vec(),
            Self::Minimal => MINIMAL.to_vec(),
        };
        for n in all {
            if !seen.contains(&n) {
                seen.push(n);
            }
        }
        seen
    }

    /// The group with every plugin of it this build has.
    pub fn build(self, at: &Where) -> PluginGroup {
        self.names().into_iter().fold(PluginGroup::new(), |g, n| add(g, n, at))
    }
}

/// Adds plugin `name` if its crate is in this build.
#[allow(unused_variables)]
#[allow(clippy::too_many_lines)] // one arm per plugin this build has: a table, not logic
fn add(g: PluginGroup, name: &str, at: &Where) -> PluginGroup {
    match name {
        #[cfg(feature = "settings")]
        "settings" => g.add(wisp_settings::SettingsPlugin),
        #[cfg(feature = "runtime")]
        "runtime" => g.add(wisp_runtime::RuntimePlugin),
        #[cfg(feature = "claude")]
        "claude" => g.add(wisp_claude::ClaudePlugin),
        #[cfg(feature = "codex")]
        "codex" => g.add(wisp_codex::CodexPlugin),
        #[cfg(feature = "cursor")]
        "cursor" => g.add(wisp_cursor::CursorPlugin),
        #[cfg(feature = "opencode")]
        "opencode" => g.add(wisp_opencode::OpencodePlugin),
        #[cfg(feature = "copilot")]
        "copilot" => g.add(wisp_copilot::CopilotPlugin),
        #[cfg(feature = "grok")]
        "grok" => g.add(wisp_grok::GrokPlugin),
        #[cfg(feature = "antigravity")]
        "antigravity" => g.add(wisp_antigravity::AntigravityPlugin),
        #[cfg(feature = "accounts")]
        "accounts" => g.add(wisp_accounts::AccountsPlugin),
        #[cfg(feature = "git")]
        "git" => g.add(wisp_git::GitPlugin::default()),
        #[cfg(feature = "worktrees")]
        "worktrees" => g.add(wisp_worktrees::WorktreesPlugin::new(at.home.display().to_string(), at.user_home.display().to_string())),
        #[cfg(feature = "ship")]
        "ship" => g.add(wisp_ship::ShipPlugin::default()),
        #[cfg(feature = "review")]
        "review" => g.add(wisp_review::ReviewPlugin),
        #[cfg(feature = "risk")]
        "risk" => g.add(wisp_risk::RiskPlugin),
        #[cfg(feature = "issues")]
        "issues" => g.add(wisp_issues::IssuesPlugin),
        #[cfg(feature = "terminal")]
        "terminal" => g.add(wisp_terminal::TerminalPlugin),
        #[cfg(feature = "lsp")]
        "lsp" => g.add(wisp_lsp::LspPlugin),
        #[cfg(feature = "machine")]
        "machine" => g.add(wisp_machine::MachinePlugin),
        #[cfg(feature = "machines")]
        "machines" => g.add(wisp_machines::MachinesPlugin),
        #[cfg(feature = "events")]
        "events" => g.add(wisp_events::EventsPlugin),
        #[cfg(feature = "threads")]
        "threads" => g.add(wisp_threads::ThreadsPlugin),
        #[cfg(feature = "termctx")]
        "termctx" => g.add(wisp_termctx::TermctxPlugin),
        #[cfg(feature = "grid-client")]
        "grid-client" => g.add(wisp_grid_client::GridClientPlugin),
        #[cfg(feature = "crew")]
        "crew" => g.add(wisp_crew::CrewPlugin),
        #[cfg(feature = "goal")]
        "goal" => g.add(wisp_goal::GoalPlugin),
        #[cfg(feature = "history")]
        "history" => g.add(wisp_history::HistoryPlugin),
        #[cfg(feature = "sandbox")]
        "sandbox" => g.add(wisp_sandbox::SandboxPlugin::default()),
        #[cfg(feature = "computer")]
        "computer" => g.add(wisp_computer::ComputerPlugin),
        #[cfg(feature = "later")]
        "later" => g.add(wisp_later::LaterPlugin),
        #[cfg(feature = "budget")]
        "budget" => g.add(wisp_budget::BudgetPlugin),
        #[cfg(feature = "router")]
        "router" => g.add(wisp_router::RouterPlugin),
        #[cfg(feature = "automations")]
        "automations" => g.add(wisp_automations::AutomationsPlugin),
        #[cfg(feature = "learn")]
        "learn" => g.add(wisp_learn::LearnPlugin),
        #[cfg(feature = "channels")]
        "channels" => g.add(wisp_channels::ChannelsPlugin),
        #[cfg(feature = "telegram")]
        "telegram" => g.add(wisp_telegram::TelegramPlugin),
        #[cfg(feature = "slack")]
        "slack" => g.add(wisp_slack::SlackPlugin),
        #[cfg(feature = "discord")]
        "discord" => g.add(wisp_discord::DiscordPlugin),
        #[cfg(feature = "github")]
        "github" => g.add(wisp_github::GithubPlugin),
        #[cfg(feature = "email")]
        "email" => g.add(wisp_email::EmailPlugin),
        #[cfg(feature = "ntfy")]
        "ntfy" => g.add(wisp_ntfy::NtfyPlugin),
        #[cfg(feature = "push")]
        "push" => g.add(wisp_push::PushPlugin),
        #[cfg(feature = "mcp")]
        "mcp" => g.add(wisp_mcp::McpPlugin),
        #[cfg(feature = "connectors")]
        "connectors" => g.add(wisp_connectors::ConnectorsPlugin),
        #[cfg(feature = "stuff")]
        "stuff" => g.add(wisp_stuff::StuffPlugin),
        #[cfg(feature = "plugins")]
        "plugins" => g.add(wisp_plugins::PluginsPlugin),
        #[cfg(feature = "cli")]
        "cli" => g.add(wisp_cli::CliPlugin),
        #[cfg(feature = "resources")]
        "resources" => g.add(wisp_resources::ResourcesPlugin),
        #[cfg(feature = "workflows")]
        "workflows" => g.add(wisp_workflows::WorkflowsPlugin),
        #[cfg(feature = "factory")]
        "factory" => g.add(wisp_factory::FactoryPlugin),
        #[cfg(feature = "share")]
        "share" => g.add(wisp_share::SharePlugin),
        #[cfg(feature = "telemetry")]
        "telemetry" => g.add(wisp_telemetry::TelemetryPlugin::default()),
        #[cfg(feature = "posthog")]
        "posthog" => g.add(wisp_posthog::PostHogPlugin),
        #[cfg(feature = "update")]
        "update" => g.add(wisp_update::UpdatePlugin),
        _ => g,
    }
}

/// The names of a group this build compiled in.
pub fn compiled(group: Group) -> Vec<&'static str> {
    group.names().into_iter().filter(|n| in_build(n)).collect()
}

#[allow(clippy::match_like_matches_macro)] // one arm per feature: which are true depends on the build
pub fn in_build(name: &str) -> bool {
    match name {
        "cli" => cfg!(feature = "cli"),
        "resources" => cfg!(feature = "resources"),
        "workflows" => cfg!(feature = "workflows"),
        "factory" => cfg!(feature = "factory"),
        "share" => cfg!(feature = "share"),
        "settings" => cfg!(feature = "settings"),
        "runtime" => cfg!(feature = "runtime"),
        "claude" => cfg!(feature = "claude"),
        "codex" => cfg!(feature = "codex"),
        "cursor" => cfg!(feature = "cursor"),
        "opencode" => cfg!(feature = "opencode"),
        "copilot" => cfg!(feature = "copilot"),
        "grok" => cfg!(feature = "grok"),
        "antigravity" => cfg!(feature = "antigravity"),
        "accounts" => cfg!(feature = "accounts"),
        "git" => cfg!(feature = "git"),
        "worktrees" => cfg!(feature = "worktrees"),
        "ship" => cfg!(feature = "ship"),
        "review" => cfg!(feature = "review"),
        "risk" => cfg!(feature = "risk"),
        "issues" => cfg!(feature = "issues"),
        "terminal" => cfg!(feature = "terminal"),
        "lsp" => cfg!(feature = "lsp"),
        "machine" => cfg!(feature = "machine"),
        "machines" => cfg!(feature = "machines"),
        "events" => cfg!(feature = "events"),
        "threads" => cfg!(feature = "threads"),
        "termctx" => cfg!(feature = "termctx"),
        "grid-client" => cfg!(feature = "grid-client"),
        "crew" => cfg!(feature = "crew"),
        "goal" => cfg!(feature = "goal"),
        "history" => cfg!(feature = "history"),
        "sandbox" => cfg!(feature = "sandbox"),
        "computer" => cfg!(feature = "computer"),
        "later" => cfg!(feature = "later"),
        "budget" => cfg!(feature = "budget"),
        "router" => cfg!(feature = "router"),
        "automations" => cfg!(feature = "automations"),
        "learn" => cfg!(feature = "learn"),
        "channels" => cfg!(feature = "channels"),
        "telegram" => cfg!(feature = "telegram"),
        "slack" => cfg!(feature = "slack"),
        "discord" => cfg!(feature = "discord"),
        "github" => cfg!(feature = "github"),
        "email" => cfg!(feature = "email"),
        "ntfy" => cfg!(feature = "ntfy"),
        "push" => cfg!(feature = "push"),
        "mcp" => cfg!(feature = "mcp"),
        "connectors" => cfg!(feature = "connectors"),
        "stuff" => cfg!(feature = "stuff"),
        "plugins" => cfg!(feature = "plugins"),
        "telemetry" => cfg!(feature = "telemetry"),
        "posthog" => cfg!(feature = "posthog"),
        "update" => cfg!(feature = "update"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v1's order (mcp-wisp.js `AGENTS`), as each harness's UI says it (`order` 10…70): the runtime lists harnesses in
    /// the order their plugins start, so the launcher's picker and the crew editor read the same (audit E, as in D)
    const HARNESSES: [&str; 7] = ["claude", "codex", "opencode", "cursor", "antigravity", "grok", "copilot"];

    #[test]
    fn harnesses_start_in_v1s_order_so_every_picker_lists_them_alike() {
        for g in [Group::Default, Group::Headless] {
            let got: Vec<&str> = g.names().into_iter().filter(|n| HARNESSES.contains(n)).collect();
            assert_eq!(got, HARNESSES, "{g:?}");
        }
    }

    #[test]
    fn groups_are_what_the_plan_says() {
        let d = Group::Default.names();
        assert!(d.iter().all(|n| d.iter().filter(|m| *m == n).count() == 1), "no plugin twice");
        let b = Group::Box.names();
        assert!(!b.contains(&"machines") && !b.contains(&"update") && !b.contains(&"cli") && b.contains(&"runtime"));
        let h = Group::Hub.names();
        assert!(h.contains(&"machines") && h.contains(&"sandbox") && !h.contains(&"update") && !h.contains(&"cli"));
        assert!(HARNESSES.iter().all(|h| Group::Headless.names().contains(h)));
        assert_eq!(Group::Minimal.names(), ["cli", "resources", "settings", "runtime", "claude"]);
        for g in [Group::Box, Group::Hub, Group::Headless, Group::Minimal] {
            assert!(g.names().iter().all(|n| d.contains(n)), "every group is part of Default");
        }
        assert!(compiled(Group::Default).iter().all(|n| in_build(n)));
    }

    /// A manifest the kernel refuses at start (a command listed twice) drops the whole plugin: the ship
    /// plugin was gone that way. Every built-in plugin's manifest validates.
    #[test]
    fn every_built_in_plugins_manifest_validates() {
        let at = Where { home: std::env::temp_dir().join("wisp-groups-test"), user_home: std::env::temp_dir() };
        let bad: Vec<String> = Group::Default
            .build(&at)
            .manifests()
            .iter()
            .filter_map(|m| m.validate().err().map(|e| format!("{}: {}", m.id, e.message)))
            .collect();
        assert_eq!(bad, Vec::<String>::new());
    }

    #[test]
    #[cfg(feature = "all-plugins")]
    fn the_default_build_has_every_plugin_host() {
        let have = compiled(Group::Default);
        let missing: Vec<&str> = DEFAULT.iter().copied().filter(|n| !have.contains(n)).collect();
        assert_eq!(missing, Vec::<&str>::new());
        assert!(["settings", "runtime", "claude"].iter().all(|n| have.contains(n)), "a task can run");
    }
}
