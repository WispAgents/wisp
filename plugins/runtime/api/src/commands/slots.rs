//! `runtime.slots`: plugins' `ext` slots of every task in one store read (perf audit 2026-11-18 #3–#5): worktrees,
//! ship and the machine view each read `runtime.get` for every task to find the few with a worktree or a PR, 5,000
//! store reads for one answer. A plugin reads its own slot here, and the slots it reads beside it.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::SessionId;

/// Which plugins' fields on each task (`wisp.worktrees`, `wisp.ship`, …); a task none of them has fields on is left out
/// unless `all` (then every task comes, with what it has).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SlotsArgs {
    pub plugins: Vec<String>,
    #[serde(default)]
    pub all: bool,
    /// only the tasks whose stored body names one of these JSON keys (a plugin's slot, a key in one), as a quoted
    /// string: the store reads no other task's body. A prefilter only: what comes is what `plugins`/`all` say of those, newest first within
    /// each word's tasks. Empty: every task.
    /// Additive (2026-11-19).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub having: Vec<String>,
}

/// One task's slots of the plugins asked for (only those it has).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskSlots {
    pub id: SessionId,
    #[ts(type = "Record<string, unknown>")]
    pub ext: BTreeMap<String, Value>,
}
