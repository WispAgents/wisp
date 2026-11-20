//! wisp.goal contract (goals: rounds, verdicts, judges, stalls). A task's goal is its spec's `goal`
//! (`{ text, judges?, vote?, maxRounds? }` or the text alone); what the plugin knows of it is the task's
//! `ext["wisp.goal"]` ([`GoalState`]). The defaults are the `goal` resource.

use serde::{Deserialize, Serialize};
use wisp_kernel::Command;

pub const PLUGIN: &str = "wisp.goal";

/// `goal.set`: set, change or clear a task's goal after it started (the task's Done-when, the GUI's goal band).
/// Empty `text` clears it. A new goal starts its count again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct SetArgs {
    /// the task
    pub id: String,
    /// what done means; empty: no goal
    pub text: String,
    /// judged at once when the task isn't running (it may already hold), as `/goal …` in a reply box does (v1
    /// Goals#set `kick`); a running task's turn is judged when it ends either way. Additive (2026-11-14).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, as = "Option<bool>")]
    pub kick: bool,
    /// who judges it: a harness (`claude`), a harness with its model and effort (`claude/opus/high`), or a profile or
    /// crew member by name; not given: the task's spec's, else Settings' defaults; given empty: no judges, the task's
    /// own quick check (B49: clearing them fell back to the defaults). Additive (2026-11-16, B44: they were sent and
    /// dropped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub judges: Option<Vec<String>>,
    /// how their verdicts count (`all`, `majority`, `any`); none: the spec's, else Settings'. Additive (2026-11-16).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(optional, as = "Option<String>")]
    pub vote: String,
    /// a declared goal by name (`goals:` in a formula, ADR-0110): its text, judges, vote and round limit, under the
    /// judges and vote given here. A `text` that is exactly a declared goal's name is that goal too (`wisp goal set
    /// <task> green-build`). Additive (2026-11-19).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(optional, as = "Option<String>")]
    pub name: String,
}

/// `goal.declared`: the goals declared by name (`goals:` in a formula), without `goals.default`, each as stored (its
/// own fields; the defaults fill the rest when it's judged). Read-only; for the composer's Goal… menu, `wisp goal
/// declared` and agents. Additive (2026-11-19, ADR-0110).
#[derive(Debug)]
pub struct Declared;
impl Command for Declared {
    const ID: &'static str = "goal.declared";
    const READ_ONLY: bool = true;
    type Args = serde_json::Value;
    type Out = serde_json::Value;
    // typed: `wisp goal declared` takes no flag, and says so for one it doesn't take (serve_cli_verbs)
    fn args_ts() -> Option<String> {
        Some(<serde_json::Value as ::ts_rs::TS>::inline(&::ts_rs::Config::default()))
    }
}

/// `goal.pause` / `goal.resume` / `goal.drop` / `goal.check`: a task's goal held, going again, gone, or judged now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskArgs {
    /// the task
    pub id: String,
    /// why it's paused (shown on the band)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub why: Option<String>,
}

macro_rules! goal_command {
    ($ty:ident, $id:literal) => {
        #[derive(Debug)]
        pub struct $ty;
        impl Command for $ty {
            const ID: &'static str = $id;
            type Args = TaskArgs;
            type Out = serde_json::Value;
            fn args_ts() -> Option<String> {
                Some(format!(
                    "{}{}",
                    <TaskArgs as ::ts_rs::TS>::docs().unwrap_or_default(),
                    <TaskArgs as ::ts_rs::TS>::inline(&::ts_rs::Config::default())
                ))
            }
        }
    };
}
goal_command!(Pause, "goal.pause");
goal_command!(Resume, "goal.resume");
goal_command!(Drop, "goal.drop");
goal_command!(Check, "goal.check");

#[derive(Debug)]
pub struct Set;
impl Command for Set {
    const ID: &'static str = "goal.set";
    type Args = SetArgs;
    type Out = serde_json::Value;
    fn args_ts() -> Option<String> {
        Some(format!(
            "{}{}",
            <SetArgs as ::ts_rs::TS>::docs().unwrap_or_default(),
            <SetArgs as ::ts_rs::TS>::inline(&::ts_rs::Config::default())
        ))
    }
}

/// `ext["wisp.goal"]`: where the goal stands.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalState {
    /// judging | again | met | stalled
    pub state: String,
    pub rounds: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verdicts: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub why: String,
    /// the goal set after the task started (`goal.set`), over its spec's
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// the judges and vote `goal.set` chose, over the spec's (B44); `Some([])`: none, the quick check (B49). Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judges: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vote: String,
    /// the round limit of the declared goal `goal.set` named, over the spec's (ADR-0110). Additive (2026-11-19).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rounds: Option<u32>,
    /// whether that declared goal's judges run in a sandbox (ADR-0047, ADR-0110). Additive (2026-11-19).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
    /// the declared goal `goal.set` named (ADR-0110), '' for a goal in words: the task's list row names it, and New
    /// task's Goal… puts the ones this project used most recently first (owner-list #63). Additive (2026-11-20).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// the agent's own wake-up (a `wisp.later` entry) the goal waits for before it's checked (v1 #775). Additive.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub wake: String,
    /// while judging: each judge's run under way (`{run, name, at}`), whose record opens like a task. Additive (2026-11-14).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub running: Vec<serde_json::Value>,
}

/// A task's goal was met: its judges agreed (ADR-0043: the `goal.met` trigger source). Durable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalMet {
    pub session_id: String,
    pub goal: String,
    pub rounds: u32,
}
impl wisp_kernel::Event for GoalMet {
    const TYPE: &'static str = "goal.met";
    const DURABLE: bool = true;
    fn subject(&self) -> Option<String> {
        Some(self.session_id.clone())
    }
    fn key(&self) -> Option<String> {
        Some(self.session_id.clone())
    }
}

/// A task's goal stalled: past its rounds, or its judges can't agree; it asks you now (`goal.failed`). Durable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalFailed {
    pub session_id: String,
    pub goal: String,
    pub rounds: u32,
    pub why: String,
}
impl wisp_kernel::Event for GoalFailed {
    const TYPE: &'static str = "goal.failed";
    const DURABLE: bool = true;
    fn subject(&self) -> Option<String> {
        Some(self.session_id.clone())
    }
    fn key(&self) -> Option<String> {
        Some(format!("{}:{}", self.session_id, self.rounds))
    }
}
