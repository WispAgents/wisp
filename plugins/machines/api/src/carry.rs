//! A delegate on another machine (ADR-0059): the lead's work carried there into a copy under that wisp's own folder
//! (never the user's checkout there), and the copy's changes brought back into the lead's folder here.
//!
//! - `machines.carry` (here): the lead's folder packed (a bundle of its branch and its uncommitted changes) and sent
//!   over the machine's link; the machine takes it in (`machines.takeIn`) and says where the copy is.
//! - `machines.bring` (here): what changed in that copy since it came in (`machines.changes`, there), applied to the
//!   lead's folder; a patch that clashes is kept in a file and nothing here moves.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// `machines.carry`: the lead's folder here, copied onto machine `id` for `key` (the lead and its member).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CarryArgs {
    /// the machine (its local handle)
    pub id: String,
    /// the lead's folder here: a git repo with a commit
    pub cwd: String,
    /// whose copy: one per lead and member, the same folder there each time (letters, digits, `-`, `_`)
    pub key: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Carried {
    /// the copy's folder on the machine
    pub cwd: String,
    /// the machine's name, as the user named it ("says where")
    pub machine: String,
}

/// `machines.takeIn` (on the machine, called over the link after the upload): the carried work into its copy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TakeInArgs {
    pub key: String,
    /// the project's folder name (the copy is `<wisp>/plugins/wisp.machines/copies/<key>/<name>`)
    pub name: String,
    /// the commit the lead's folder is on (the bundle has it)
    pub head: String,
    /// the branch the lead is on (empty: a detached HEAD, the copy's branch is `wisp/delegate`)
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_email: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TakenIn {
    pub cwd: String,
}

/// `machines.changes` (on the machine): what changed in a carried copy since it came in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ChangesArgs {
    pub cwd: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Changes {
    /// one binary diff, new files too (empty: nothing changed)
    pub patch: String,
}

/// `machines.bring`: a copy's changes on machine `id`, into the lead's folder here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BringArgs {
    pub id: String,
    /// the copy's folder there (`Carried.cwd`)
    pub from: String,
    /// the lead's folder here
    pub into: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Brought {
    /// files the changes touched (0: nothing changed there)
    pub files: u32,
    /// why they didn't go in (they clash with changes made here meanwhile)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
    /// the patch kept here when it didn't go in, so the work isn't lost
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kept: Option<String>,
}
