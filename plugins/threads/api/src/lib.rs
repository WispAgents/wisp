//! wisp-threads-api: the contract of `wisp.threads` (ADR-0079, long threads). A long conversation degrades: the goal
//! is far behind, superseded instructions are still in it, and a compaction's summary drops what was rejected and
//! why. So a task's state lives outside the conversation, in its **Notes** (goal, constraints, decisions, what
//! failed, where it stands, open questions, key files), kept by its agent (`wisp_notes`) and by you (the Thread tab,
//! `wisp notes`). A section is rewritten, never appended to, so an old instruction is gone. Every write is a
//! revision with who wrote it. The thread's **health** (how worn it is) is read from what wisp already has.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use wisp_kernel::Command;

pub const PLUGIN: &str = "wisp.threads";
pub const PERM_READ: &str = "threads:read";
pub const PERM_WRITE: &str = "threads:write";
/// Where a task's Notes and health are published: the task's own topic (`session:<task id>`), so whoever has the task
/// open gets them with it (as wisp.goal's check records).
pub fn topic(task: &str) -> String {
    format!("session:{task}")
}

/// The sections, in order. `goal` and `state` are one text; the rest are lists (one item a line).
pub const SECTIONS: [&str; 7] = ["goal", "constraints", "decisions", "failed", "state", "open", "files"];

/// Who wrote a section: the task's agent, or you (the Thread tab, the CLI).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "threads/")]
pub enum By {
    #[default]
    Agent,
    You,
}

/// One section as it stands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "threads/")]
pub struct Section {
    pub id: String,
    pub label: String,
    /// one text (goal, state) or its items, one a line
    pub text: String,
    /// its own revision (0: never written)
    pub rev: u32,
    pub by: By,
    #[ts(type = "number")]
    pub at: i64,
}

/// A task's Notes: every section (empty ones too, in order), the whole Notes' revision, and what the agent last read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "threads/")]
pub struct Notes {
    pub task: String,
    pub sections: Vec<Section>,
    pub rev: u32,
    /// the revision the agent last had in front of it (the note before a follow-up, or its own write)
    pub seen: u32,
    /// these are the task's lead's (a subagent reads its lead's Notes, and can't write them)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lead: Option<String>,
}

/// One earlier version of a section.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "threads/")]
pub struct Revision {
    pub section: String,
    pub rev: u32,
    pub text: String,
    pub by: By,
    #[ts(type = "number")]
    pub at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "threads/")]
pub struct TaskArgs {
    /// the task (for a subagent: its own id; its lead's Notes are read)
    pub id: String,
}

/// `threads.notesUpdate`: rewrite one section (never append): the text as it should be from now on; '' clears it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "threads/")]
pub struct UpdateArgs {
    pub id: String,
    pub section: String,
    pub text: String,
    /// who writes it; the agent's tool says `agent`, everyone else is `you`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub by: Option<By>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "threads/")]
pub struct RevisionsArgs {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub section: Option<String>,
}

/// How worn a thread is: `ok`, `heavy` or `worn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "threads/")]
pub enum Level {
    #[default]
    Ok,
    Heavy,
    Worn,
}

/// The same failure again and again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "threads/")]
pub struct Repeat {
    /// the error's words without what changes between tries (numbers, paths, hashes)
    pub sig: String,
    pub n: u32,
}

/// The thread's health, from what wisp already has (no model call): context, compactions, turns since it was
/// fresh, the same error repeated, edits the agent undid, how old the Notes are.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "threads/")]
pub struct Health {
    pub task: String,
    pub level: Level,
    /// one short phrase each, worst first ("context 82% full", "the same error 4× in the last 3 turns")
    pub reasons: Vec<String>,
    /// turns in this thread (since it was last fresh), compactions aside
    pub turns: u32,
    pub compactions: u32,
    /// the last turn's context over its window, 0..1, when both are known
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub share: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub repeat: Option<Repeat>,
    /// edits in the recent turns that undid an earlier edit of this thread
    pub reverted: u32,
    /// turns since the Notes were last written (None: no Notes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub notes_age: Option<u32>,
}

macro_rules! command {
    ($ty:ident, $id:literal, $perm:expr, $ro:literal, $args:ty, $out:ty) => {
        #[derive(Debug)]
        pub struct $ty;
        impl Command for $ty {
            const ID: &'static str = $id;
            const PERMISSION: Option<&'static str> = Some($perm);
            const READ_ONLY: bool = $ro;
            type Args = $args;
            type Out = $out;
            fn args_ts() -> Option<String> {
                Some(<$args as ::ts_rs::TS>::inline(&::ts_rs::Config::default()))
            }
        }
    };
}

command!(NotesCmd, "threads.notes", PERM_READ, true, TaskArgs, Notes);
command!(NotesUpdate, "threads.notesUpdate", PERM_WRITE, false, UpdateArgs, Notes);
command!(RevisionsCmd, "threads.revisions", PERM_READ, true, RevisionsArgs, Vec<Revision>);
command!(HealthCmd, "threads.health", PERM_READ, true, TaskArgs, Health);
