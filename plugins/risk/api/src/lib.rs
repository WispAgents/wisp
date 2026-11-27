//! wisp-risk-api: merge risk (`risk.harness`, off by default, ADR-0140). For every open PR of a task (the ones Ship it
//! opened and the ones its agent attached), a harness other than the one that wrote it (or the one picked) reads the
//! PR's diff and says how risky merging it is: low, medium or high, a score out of 100, one line and the reasons. One
//! verdict per PR, of its head: a push makes it stale and it is judged again. The verdicts sit on the task
//! (`ext["wisp.risk"]`) and show on the PR row, in the PR panel and in Merge's confirmation; Merge stays possible
//! either way. It runs on the judging harness's current account: wisp never switches accounts for it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use wisp_kernel::{Command, Event};

/// The session ext slot merge risk owns.
pub const EXT: &str = "wisp.risk";
/// The slot it reads the task's PRs from (wisp.ship's: `repos[].pr`, `prs[].pr`).
pub const SHIP_EXT: &str = "wisp.ship";
/// The setting that says who judges: off, another harness than the author's, or one harness.
pub const SETTING: &str = "risk.harness";
/// Every harness a judge may be, in the order `other` tries them.
pub const HARNESSES: &[&str] = &["claude", "codex", "antigravity", "copilot", "opencode", "cursor", "grok"];
/// What `risk.harness` may be.
pub const VALUES: &[&str] = &["off", "other", "claude", "codex", "antigravity", "copilot", "opencode", "cursor", "grok"];

/// Who judged: a harness, and the model when one was pinned ("" for its default).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "risk/")]
pub struct Pick {
    pub agent: String,
    pub model: String,
}

/// How risky merging the PR is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "risk/")]
pub enum Level {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "risk/")]
pub enum Status {
    #[default]
    Running,
    Done,
    Failed,
}

/// One PR's merge risk, of one head.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "risk/")]
pub struct Verdict {
    /// the PR's URL: what a verdict is keyed by (a task's PRs may sit in several repos)
    pub url: String,
    pub repo: String,
    pub number: u64,
    /// the head commit it judged
    pub head: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub level: Option<Level>,
    /// 0 (trivial) to 100 (very likely to break something)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub score: Option<u8>,
    /// one line: why this level
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub summary: String,
    /// what makes it risky (or not), most important first
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub by: Option<Pick>,
    /// why it failed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub why: Option<String>,
    /// the judged diff was cut to fit the judge's budget
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub partial: bool,
    /// ms since the epoch: when it started (running) or ended
    #[ts(type = "number")]
    pub at: i64,
}

/// wisp.risk's slot on a task: a verdict per PR.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "risk/")]
pub struct Slot {
    #[serde(default)]
    pub prs: Vec<Verdict>,
}

/// What the page sees of one PR: its verdict, and whether it is of the PR's head now (`current`). A judge that ran past
/// its time (a restart cut it short) reads as failed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "risk/")]
pub struct RiskView {
    #[serde(flatten)]
    pub verdict: Verdict,
    pub current: bool,
}

// ---------- commands and events ----------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "risk/")]
pub struct JudgeArgs {
    /// the task whose PRs get judged (its id)
    pub id: String,
    /// Judge again (or the first time, by hand): judges heads already judged, and while a turn runs
    #[serde(default)]
    pub force: bool,
    /// only this PR (its URL); every open PR of the task when absent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub url: Option<String>,
}

/// How many PRs it started judging, and why none when it's 0 ("" when they're judged already or judging).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "risk/")]
pub struct Judged {
    pub started: u32,
    pub why: String,
}

/// `risk.judge`: judge the merge risk of a task's open PRs whose head isn't judged yet. Ship calls it when a PR's head
/// changes; a PR attached and a turn's end call it too.
#[derive(Debug)]
pub struct Judge;
impl Command for Judge {
    const ID: &'static str = "risk.judge";
    type Args = JudgeArgs;
    fn args_ts() -> Option<String> {
        Some(format!(
            "{}{}",
            <JudgeArgs as ::ts_rs::TS>::docs().unwrap_or_default(),
            <JudgeArgs as ::ts_rs::TS>::inline(&::ts_rs::Config::default())
        ))
    }
    type Out = Judged;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "risk/")]
pub struct ViewArgs {
    pub id: String,
}

/// `risk.view`: the task's merge risk, one entry per PR it judged.
#[derive(Debug)]
pub struct View;
impl Command for View {
    const ID: &'static str = "risk.view";
    const READ_ONLY: bool = true;
    type Args = ViewArgs;
    fn args_ts() -> Option<String> {
        Some(format!(
            "{}{}",
            <ViewArgs as ::ts_rs::TS>::docs().unwrap_or_default(),
            <ViewArgs as ::ts_rs::TS>::inline(&::ts_rs::Config::default())
        ))
    }
    type Out = Vec<RiskView>;
}

/// A PR's merge risk was judged (or the judge failed): telemetry and notify listen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "risk/")]
pub struct RiskJudged {
    pub session_id: String,
    pub url: String,
    pub head: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub level: Option<Level>,
    pub judge: String,
    pub author: String,
}
impl Event for RiskJudged {
    const TYPE: &'static str = "risk.judged";
    const DURABLE: bool = true;
    fn subject(&self) -> Option<String> {
        Some(self.session_id.clone())
    }
    fn key(&self) -> Option<String> {
        Some(format!("{}@{}@{}", self.session_id, self.url, self.head))
    }
}
