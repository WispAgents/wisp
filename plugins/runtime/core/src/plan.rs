//! Whether a Plan run may run commands (v1 #794, runner.js `planRuns`/`outsideTask`): a task you start in Plan runs
//! any command and only can't edit files; a side discussion's Plan only reads, and so does a task a channel or a
//! trigger started (someone outside may have written it) or one carrying a mode cap, and their subagents. How a
//! harness runs it (its flags, what its agent is told) is the harness plugin's. Pure: the host reads the tasks.

use wisp_runtime_api::model::Mode;

/// What a channel's task is started `by` (`channel:<name>`), kept as `ext["wisp.runtime"].startedBy`.
pub const CHANNEL: &str = "channel:";

/// How far up a subagent's leads `outside` looks (v1: 5).
pub const MAX_LEADS: usize = 5;

/// What decides where a task came from: its own and each lead's, nearest first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Origin<'a> {
    /// it carries a mode cap (a channel's, a capped lead's): `ext["wisp.runtime"].capped`
    pub capped: bool,
    /// `ext["wisp.runtime"].spec.lineage.triggeredBy`: `trigger:<name>` for a trigger, `schedule:`, `hand`,
    /// `pipeline:` for your own declared tasks and schedules
    pub triggered_by: Option<&'a str>,
    /// `ext["wisp.runtime"].startedBy`: `channel:<name>` for a channel's task, kept after its cap is lifted
    pub started_by: Option<&'a str>,
}

/// The task (first) or one of its leads was started from outside: capped, by a trigger or by a channel (v1
/// `outsideTask`).
pub fn outside(chain: &[Origin<'_>]) -> bool {
    chain.iter().take(MAX_LEADS + 1).any(|o| {
        o.capped || o.triggered_by.is_some_and(|b| b.starts_with("trigger:")) || o.started_by.is_some_and(|b| b.starts_with(CHANNEL))
    })
}

/// The run's `plan_commands` (v1 `planRuns`): Plan, not a side discussion, not from outside.
pub fn runs_commands(mode: Mode, side: bool, outside: bool) -> bool {
    mode == Mode::Plan && !side && !outside
}
