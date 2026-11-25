//! Arguments and answers of asks, approvals, /btw, delegation, sign-in, side runs and import.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::RunPick;
use crate::model::{Activity, Attachment, Ext, SessionState};
use crate::{SessionId, TurnId};

/// `runtime.setup.ask` (ADR-0113): a change to the user's setup an agent asks for. `command` is one of
/// `connectors.agent.add`, `crew.agent.add`, `crew.agent.edit`, `crew.agent.remove` or `runtime.projectAdd`, and
/// `args` its arguments; the card says what it would change in the server's words, and the write runs only when the
/// user answers Allow, never before.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct SetupAskArgs {
    pub id: SessionId,
    pub command: String,
    #[ts(type = "unknown")]
    pub args: Value,
}

/// The card is up: its answer comes through `runtime.pollAnswer` (`askId`); `text` says so in words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct SetupAsked {
    pub ask_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ApprovalArgs {
    pub id: SessionId,
    pub tool: String,
    pub input: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tool_use_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ApproveBlockedArgs {
    pub id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub turn_id: Option<TurnId>,
    pub allow: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct BtwArgs {
    pub id: SessionId,
    pub question: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<Attachment>>", optional)]
    pub attachments: Vec<Attachment>,
}

/// `runtime.probe`: a question asked of a copy of the task's conversation (its harness forks it, as /btw), answering
/// only (no tools, plan mode), awaited, and kept nowhere: not in the conversation, not on the task. A long thread's
/// compaction audit asks what the compacted conversation still remembers (ADR-0096). A harness that can't fork, or
/// a task with no conversation yet: `conflict`. Additive (2026-11-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ProbeArgs {
    pub id: SessionId,
    pub question: String,
    /// at most this long (default 120 s, at most 300 s)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ProbeAnswer {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cost_usd: Option<f64>,
}

/// A /btw side question and its answer (v1 btw.js), kept on the task, never in its conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct BtwEntry {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub answer: String,
    pub at: i64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub pending: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct BtwDropArgs {
    pub id: SessionId,
    pub btw_id: String,
}

/// Show in folder for a picture or video an answer showed (v1 POST /sessions/:id/reveal): `path` as the block named
/// it. Additive (2026-11-12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct RevealArgs {
    pub id: SessionId,
    pub path: String,
}

/// Reveal one masked .env item of a turn as it is (ADR-0050, v1 GET /sessions/:id/unmasked): a step's output by its
/// event `step` (seq), an answer `block` by its index, or the captured change to `file`. Exactly one. Refused to a
/// capped agent and to a share page's caller; answers `not_found` for an item that isn't masked. Additive (2026-11-13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct UnmaskedArgs {
    pub id: SessionId,
    pub turn_id: TurnId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub step: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub block: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct DelegateTask {
    /// A member of the lead's team; empty: a new subagent made from `agent` (and `machine`, `name`) on the fly.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub subagent: String,
    pub prompt: String,
    /// A new subagent on this harness (`auto`: wisp picks harness and model).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub effort: Option<String>,
    /// Run the new subagent on this machine (its id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub machine: Option<String>,
    /// The new subagent's name; one the team has already continues that one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// What the new subagent is for, shown to the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub r#use: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct DelegateArgs {
    pub id: SessionId,
    pub tasks: Vec<DelegateTask>,
}

/// One subagent a lead started (v1 subagents.js).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct Delegation {
    pub id: SessionId,
    pub subagent: String,
    pub agent: String,
    pub model: String,
    pub effort: String,
    pub turn_id: TurnId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct ResultsArgs {
    pub id: SessionId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<SessionId>>", optional)]
    pub ids: Vec<SessionId>,
    /// wait (up to this many seconds) for them to finish
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait: Option<u32>,
}

/// A subagent's report to its lead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct Report {
    pub id: SessionId,
    pub subagent: String,
    pub state: SessionState,
    pub done: bool,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct Reports {
    pub reports: Vec<Report>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct RerunArgs {
    pub id: SessionId,
    pub turn_id: TurnId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub account: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct SignInDoneArgs {
    pub agent: String,
    pub account: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct OverageArgs {
    pub id: SessionId,
    pub turn_id: TurnId,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct NoteArgs {
    pub id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub turn_id: Option<TurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tone: Option<crate::model::Tone>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct SettleArgs {
    pub id: SessionId,
    /// the after-turn step found nothing to say: land without marking the task unread
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub quiet: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct SearchArgs {
    pub q: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct QuickArgs {
    pub prompt: String,
    pub cwd: String,
    /// this harness and model; default: the cheapest one available (v1 quick.js)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pick: Option<RunPick>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub timeout_ms: Option<u64>,
    /// the answer is a JSON object: parsed into `json`, null when it isn't one
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub json: bool,
    /// the caller's id for this run (a goal check's id): `runtime.quickCancel` with it stops the run, its process
    /// tree killed. Additive (2026-11-14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub run_id: Option<String>,
}

/// `runtime.quickCancel`: the quick run started with this `runId` stops (its process tree killed, the run answers
/// `conflict` with `details.cancelled`). One that hasn't started yet is refused when it does; an unknown or finished
/// one is a no-op. Additive (2026-11-14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct QuickCancelArgs {
    pub run_id: String,
}

/// What a cancelled quick run answers (`conflict`; `details.cancelled` here, the message alone once a peer relayed it).
pub const QUICK_CANCELLED: &str = "It was cancelled.";

/// What `runtime.quickCancel` found: `true` when a run under way was stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct QuickCancelled {
    pub stopped: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct QuickResult {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub json: Option<Value>,
    pub agent: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<Activity>>", optional)]
    pub activity: Vec<Activity>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct AgentsArgs {
    /// detect again (Settings → Agents → Re-check); a harness id detects only that one again
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fresh: Option<String>,
}

/// `runtime.defaultModel`: what `agent`'s Default runs in `cwd` (the picked project, a task's folder; none: this
/// computer's), as the harness reads its own config, never a run (owner's delegate 2026-11-18).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct DefaultModelArgs {
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct AgentStatus {
    pub id: String,
    pub name: String,
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
    pub models: Vec<crate::harness::HarnessModel>,
    pub efforts: Vec<String>,
    /// can lead a team of subagents (takes wisp's MCP server)
    pub delegates: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub signed_in: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub setup: Option<crate::harness::SetupTip>,
    /// the program wisp found and runs (Settings → Agents shows it under the harness). Additive (2026-11-25).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bin: Option<String>,
}

/// Where a harness's detection stands on the `harnesses` topic (ADR-0066).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub enum HarnessCheck {
    /// its detect hasn't answered yet: `agent` is what was known before (its declared models, or the last answer)
    Checking,
    /// found (signed in or not: `agent.signedIn`)
    Installed,
    /// not found, or its detect failed (`agent.error`)
    Missing,
}

/// One harness on `plugin:wisp.runtime:harnesses` (ADR-0066): written `checking` the moment a detection starts, then
/// again as that harness's own detect lands, so a picker renders at once and a hung CLI holds back only its own row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct HarnessRow {
    /// the harness id
    pub id: String,
    /// the order the harnesses register in (`runtime.agents`' order)
    pub ord: u32,
    pub state: HarnessCheck,
    /// what `runtime.agents` says of it (while checking: the last known)
    pub agent: AgentStatus,
    /// when this row was written (ms)
    #[ts(type = "number")]
    pub at: i64,
}

/// `runtime.card`: a task that is only a message (v1 `kind: 'reminder'`): review, unread, one done turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct CardArgs {
    pub title: String,
    pub prompt: String,
    pub blocks: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub started_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[ts(as = "Option<Ext>", optional)]
    pub ext: Ext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ForkArgs {
    /// the task to fork: a new task goes on from its conversation
    pub id: SessionId,
    /// the new task's id, made by the client that shows its row and page at once (as a new task's own id is). Additive (2026-11-14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub new_id: Option<SessionId>,
    /// fork after this turn (default: the last)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub turn_id: Option<TurnId>,
    /// the new task's title (default: the task's, as a fork)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
}

/// Bring the tasks of wisp 1 (`~/.wisp/sessions/`) in, each resumable; one brought in before is skipped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct ImportArgs {
    /// the sessions folder (default `~/.wisp/sessions`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dir: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct ImportReport {
    pub imported: u32,
    /// already imported, unchanged since
    pub skipped: u32,
    /// unreadable files, with why
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct PresentationArgs {
    pub id: SessionId,
    /// a turn index; default the last
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub turn: Option<u32>,
}

/// `wisp_sessions` (v1 GET /internal/sessions/:id): the user's other tasks, newest first. `session` is the
/// asking agent's task (wisp.mcp sets it).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct AgentSessionsArgs {
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub query: Option<String>,
    /// comma-separated states
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub archived: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
}

/// `wisp_session` (v1 GET /internal/session/:id?of&turn): one task in words.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct AgentSessionArgs {
    pub session: String,
    pub of: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub turn: Option<u32>,
}

/// `wisp_watch`'s call (the MCP tool's input) on the agent's task: `action` and what it needs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ::ts_rs::TS)]
pub struct AgentWatchArgs {
    pub session: String,
    #[serde(flatten)]
    pub body: serde_json::Map<String, Value>,
}

/// `wisp_tag` (v1 POST /internal/tags): the topics of the agent's own task, the whole list (it replaces them).
/// `tags` as the agent sent it: a list, or a comma-separated string. Additive (2026-11-13).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ::ts_rs::TS)]
pub struct AgentTagArgs {
    pub session: String,
    #[serde(default)]
    pub tags: Value,
}

/// `wisp_blocks` (v1 protocol.js `blockDocs`): the full docs of the answer blocks the prompt only names.
/// Additive (2026-11-13).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ::ts_rs::TS)]
pub struct AgentBlocksArgs {
    pub session: String,
    /// a list of block types, or one
    #[serde(default)]
    pub types: Value,
}

/// `wisp_stats` (v1 GET /internal/stats/:id): tool calls, turns, cost and success over the user's tasks.
/// `days` as the agent sent it (a number, or a numeric string); `project` "current" = the asking task's.
/// Additive (2026-11-13).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ::ts_rs::TS)]
pub struct AgentStatsArgs {
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// `wisp_available` (v1 GET /internal/available/:id): `session` is the asking agent's task. Additive (2026-11-13).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ::ts_rs::TS)]
pub struct AgentAvailableArgs {
    pub session: String,
}

/// `wisp_task` (v1 POST /internal/task/:id): the tool's input as the agent sent it, plus `session`. Additive (2026-11-13).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ::ts_rs::TS)]
pub struct AgentTaskArgs {
    pub session: String,
    #[serde(default)]
    pub task: String,
    /// reply, stop, done, archive, unarchive or rename
    #[serde(default)]
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// reply: wait for its current run (only `true` does)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// What an agent tool reads back: `text`, or `error`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct AgentText {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

/// `runtime.projectRun`: the run scripts of the project a folder is in (its `run:`, a dev server), in a
/// terminal of their own (`run:<project>`). Additive (2026-11-09).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ProjectRunArgs {
    /// a folder in the project
    pub cwd: String,
}

/// `runtime.projectTest`: a declared project's test command (Settings → Auto, Tests per project); empty
/// clears it. Additive (2026-11-10).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ProjectTestArgs {
    /// the project's name
    pub name: String,
    pub test_command: String,
}

/// `runtime.projectAdd`: a folder as a project, named on the server (ADR-0076, ADR-0077): the same folder again is
/// the project it is (nothing written); else `name` (or the folder's last part), `-2`, `-3`… when that's taken. The
/// imperative add every client uses (`wisp add`, Projects' Add, Run on, an agent); `resources.apply` stays the
/// declarative one, where a spec's name is its id. Additive (2026-11-15).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ProjectAddArgs {
    /// a full path to the folder (`~` is your home); clone a repo URL first (`git.clone`)
    pub path: String,
    /// the name it should go by; a taken one gets `-2` (left out: the folder's last part)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
}

/// `runtime.projectAdd`'s answer: the project's name, and whether this call made it (`false`: the folder was one).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ProjectAdded {
    pub name: String,
    /// the folder as it's stored (canonical, ADR-0071)
    pub path: String,
    pub created: bool,
}

/// `runtime.sessions`: this folder's sessions run outside wisp, across harnesses (ADR-0035). Additive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct SessionsArgs {
    pub cwd: String,
}

/// `runtime.adoptSession`: one of them as a wisp task; its follow-ups continue it in its CLI. Additive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct AdoptSessionArgs {
    pub harness: String,
    pub id: String,
}

/// `runtime.restoreSession`: a harness's own session log put back into its store on this computer, for
/// `cwd`, so its CLI resumes it (`wisp open` on a task from the repo or a share). Additive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct RestoreSessionArgs {
    pub harness: String,
    pub id: String,
    pub cwd: String,
    /// the log, base64
    pub raw: String,
}

/// `runtime.usage`: Settings → Usage's line over the last `days` (7 when unset). Additive (2026-11-10).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct UsageArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub days: Option<u32>,
    /// added 2026-11-25 (additive): the viewer's offset from UTC in minutes (`-Date#getTimezoneOffset()`), so
    /// the window starts at local midnight and each bar is a local day; UTC when unset
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tz_offset_min: Option<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct UsageSummary {
    /// the window's start (ms)
    #[ts(type = "number")]
    pub from: i64,
    /// dollars, where the harness says
    pub spend: f64,
    pub turns: u32,
    /// 0–100: the ended turns that didn't fail; null when none ended
    pub worked: Option<f64>,
    /// turns that ended by asking a question
    pub questions: u32,
    /// added 2026-11-12 (additive): turns with a price (a harness that reports cost)
    #[serde(default)]
    pub priced: u32,
    /// added 2026-11-12: tasks the turns belong to
    #[serde(default)]
    pub sessions: u32,
    /// added 2026-11-25 (additive, ADR-0120): of `priced`, the turns whose cost is a list-price estimate of their tokens
    /// (the harness reported none); `cost_estimated`: `spend` holds at least one, so it shows as `~$x.xx`
    #[serde(default)]
    pub estimated: u32,
    #[serde(default)]
    pub cost_estimated: bool,
    /// added 2026-11-23 (TRY-32, additive): total tokens (read + output) across all harnesses
    #[serde(default)]
    #[ts(type = "number")]
    pub tokens: u64,
    /// added 2026-11-23 (TRY-32, additive): total input/read tokens (cache reads and writes included)
    #[serde(default)]
    #[ts(type = "number")]
    pub input_tokens: u64,
    /// added 2026-11-23 (TRY-32, additive): total output tokens written
    #[serde(default)]
    #[ts(type = "number")]
    pub output_tokens: u64,
    /// added 2026-11-23 (TRY-32, additive): total input tokens read from prompt cache
    #[serde(default)]
    #[ts(type = "number")]
    pub cache_tokens: u64,
    /// added 2026-11-12: one per day of the window, oldest first (v1 usage.js "Spend per day")
    #[serde(default)]
    pub daily: Vec<UsageDay>,
    /// added 2026-11-25 (additive): each day split by harness, for the stacked chart; only days a harness ran
    #[serde(default)]
    pub daily_by_harness: Vec<UsageHarnessDay>,
    /// added 2026-11-25 (additive): `daily` rolled up by ISO week (Monday first), oldest first
    #[serde(default)]
    pub weekly: Vec<UsageWeek>,
    /// added 2026-11-23 (TRY-32, additive): by harness (all models of each harness rolled up)
    #[serde(default)]
    pub harnesses: Vec<UsageRow>,
    /// added 2026-11-12: by project (the task's folder), most spent first
    #[serde(default)]
    pub projects: Vec<UsageRow>,
    /// added 2026-11-12: by harness · model, most spent first
    #[serde(default)]
    pub runners: Vec<UsageRow>,
}

/// One day of `runtime.usage`'s window (the viewer's local day; UTC without `tz_offset_min`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct UsageDay {
    /// `YYYY-MM-DD`
    pub date: String,
    pub cost: f64,
    pub turns: u32,
    /// added 2026-11-23 (TRY-32, additive): total tokens (read + output) on this day
    #[serde(default)]
    #[ts(type = "number")]
    pub tokens: u64,
    /// added 2026-11-25 (additive, ADR-0120): `cost` holds a list-price estimate
    #[serde(default)]
    pub cost_estimated: bool,
}

/// One harness's share of one day of `runtime.usage`'s window (the viewer's local day).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct UsageHarnessDay {
    /// `YYYY-MM-DD`
    pub date: String,
    pub agent: String,
    pub cost: f64,
    #[ts(type = "number")]
    pub tokens: u64,
    /// input tokens read from prompt cache
    #[ts(type = "number")]
    pub cache_read: u64,
    pub turns: u32,
    /// `cost` holds a list-price estimate (ADR-0120)
    #[serde(default)]
    pub cost_estimated: bool,
}

/// One ISO week of `runtime.usage`'s window: its days summed (a week the window cuts holds only its days in it).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct UsageWeek {
    /// `YYYY-Www`, the ISO week
    pub week: String,
    /// its Monday, `YYYY-MM-DD`
    pub start: String,
    pub cost: f64,
    pub turns: u32,
    #[ts(type = "number")]
    pub tokens: u64,
    /// added 2026-11-25 (additive, ADR-0120): `cost` holds a list-price estimate (one of its days does)
    #[serde(default)]
    pub cost_estimated: bool,
}

/// One row of a breakdown: a harness, a project, or a harness · model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct UsageRow {
    /// the harness id, the project's folder, or `agent model`
    pub key: String,
    /// the harness id, the project's name, or the model
    pub name: String,
    /// a harness or runner row's harness
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub agent: String,
    pub cost: f64,
    pub priced: u32,
    pub turns: u32,
    /// added 2026-11-23 (TRY-32, additive): tasks the turns in this row belong to
    #[serde(default)]
    pub sessions: u32,
    /// added 2026-11-25 (additive, ADR-0120): of `priced`, the turns whose cost is a list-price estimate of their tokens
    /// (the harness reported none); `cost_estimated`: `cost` holds at least one, so it shows as `~$x.xx`
    #[serde(default)]
    pub estimated: u32,
    #[serde(default)]
    pub cost_estimated: bool,
    /// added 2026-11-25 (additive): Copilot's premium requests, summed (its result's `usage.premiumRequests`)
    #[serde(default)]
    pub premium_requests: u32,
    /// added 2026-11-23 (TRY-32, additive): total tokens (read + output)
    #[serde(default)]
    #[ts(type = "number")]
    pub tokens: u64,
    /// added 2026-11-23 (TRY-32, additive): input/read tokens (cache included)
    #[serde(default)]
    #[ts(type = "number")]
    pub input_tokens: u64,
    /// added 2026-11-23 (TRY-32, additive): output tokens
    #[serde(default)]
    #[ts(type = "number")]
    pub output_tokens: u64,
    /// added 2026-11-23 (TRY-32, additive): cache-read tokens
    #[serde(default)]
    #[ts(type = "number")]
    pub cache_tokens: u64,
    /// 0–100 of the ended ones; null when none ended
    pub worked: Option<f64>,
    pub questions: u32,
    /// the ended turns' average length (ms)
    pub avg_ms: Option<f64>,
}

/// `runtime.cold` (v1 web/js/cold-cache.js): a large idle task's prompt cache, and Compact now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct ColdArgs {
    pub id: SessionId,
}

/// The provider's prompt cache of a large idle task: the next reply re-reads `tokens` once it's `cold`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct ColdCache {
    #[ts(type = "number")]
    pub tokens: u64,
    #[ts(type = "number")]
    pub ttl_ms: u64,
    /// when the cache is taken as gone: its last use + TTL × 1.25 (caches outlive their TTL a little)
    #[ts(type = "number")]
    pub cold_at: i64,
    pub cold: bool,
}

/// Compact now: compacting now against the same reply once cold, USD at API list prices (`None`: a model
/// with no price, tokens only). Offered until `until` (the TTL itself: a dead cache costs what it saves).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "runtime/")]
pub struct CompactOffer {
    #[ts(type = "number")]
    pub tokens: u64,
    pub compact: Option<f64>,
    pub cold: Option<f64>,
    #[ts(type = "number")]
    pub until: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct ColdOut {
    pub cache: Option<ColdCache>,
    pub compact: Option<CompactOffer>,
}

/// `runtime.folders`: the folders a typed path could go on to (v1 `GET /fs?q=`, the project picker's suggestions):
/// under the path's folder, starting with its last part (any case), no hidden ones, 12 at most, sorted. `~` is your
/// home, and a path typed from `~` is answered from `~`. A path that is neither absolute nor `~` gets none (v1 listed the server's own working folder). Additive
/// (2026-11-12).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct FoldersArgs {
    pub q: String,
}

/// `runtime.discover`: git repos on this computer that aren't projects yet (v1 `/projects/discover`, places.js
/// `discoverRepos`): the usual folders under your home, two levels down, the 24 touched last. Additive (2026-11-11).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct DiscoverArgs {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "runtime/")]
pub struct FoundRepo {
    pub path: String,
    pub name: String,
    /// when it was last changed (ms)
    #[ts(type = "number")]
    pub mtime: i64,
}
