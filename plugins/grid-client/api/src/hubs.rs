//! A team's hub (ADR-0148): its people, invite links, deploy keys, secrets and deploys, through this computer's sign-in.
//! Making an invite, a key or a secret is a person's (`uiOnly`: a link, a key or a value through an agent would sit
//! in a transcript); reading, revoking and deploying aren't.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use wisp_kernel::Command;

use crate::NoArgs;

/// `grid-client.hub`: one thing about a hub.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "grid-client/")]
pub struct HubArgs {
    /// show | create | restart | delete | invites | uninvite | join | keys | unkey | secrets | unset | logout | deploys
    pub op: String,
    /// the team (its slug)
    #[serde(default)]
    pub team: String,
    /// an invite's or a key's id (uninvite, unkey)
    #[serde(default)]
    pub id: String,
    /// join: the invite link or its code
    #[serde(default)]
    pub code: String,
    /// unset: the secret's name; logout: the harness (claude)
    #[serde(default)]
    pub name: String,
    /// delete: the team's slug, typed again
    #[serde(default)]
    pub confirm: String,
}

/// `grid-client.hubInvite`: an invite link to a team, said once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "grid-client/")]
pub struct HubInviteArgs {
    pub team: String,
    /// member (the default), viewer, or (an owner's) admin
    #[serde(default)]
    pub role: String,
    /// how long it lasts, in days (7 unless said; at most 30)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub days: Option<i64>,
    /// how many people it adds (1 unless said; at most 100)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub uses: Option<i64>,
}

/// `grid-client.hubKeyCreate`: a deploy key for a team's hub, said once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "grid-client/")]
pub struct HubKeyArgs {
    pub team: String,
    pub name: String,
    /// deploy (the default: plan and apply) or plan (pull requests)
    #[serde(default)]
    pub rights: String,
}

/// `grid-client.hubSecretSet`: one of the hub's secrets (an env var its box and sandboxes start with), or with `login`
/// a harness sign-in (`name: claude`, the token from `claude setup-token`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "grid-client/")]
pub struct HubSecretArgs {
    pub team: String,
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub login: bool,
}

/// `grid-client.hubCall`: a `resources.*` command run on a team's hub as you (`wisp grid plan|deploy` without a key).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "grid-client/")]
pub struct HubCallArgs {
    pub team: String,
    pub cmd: String,
    #[serde(default)]
    #[ts(type = "unknown")]
    pub args: Value,
    /// where the formula came from (a repo and commit), for the hub's deploy history
    #[serde(default)]
    pub source: String,
}

/// The commands `hubCall` runs: what deploying needs, nothing else.
pub const HUB_CALLS: [&str; 6] =
    ["resources.kinds", "resources.plan", "resources.apply", "resources.export", "resources.read", "resources.refresh"];

command!(Hubs = "grid-client.hubs" read yes : NoArgs => Value);
command!(Hub = "grid-client.hub" : HubArgs => Value);
command!(HubInvite = "grid-client.hubInvite" : HubInviteArgs => Value);
command!(HubKeyCreate = "grid-client.hubKeyCreate" : HubKeyArgs => Value);
command!(HubSecretSet = "grid-client.hubSecretSet" : HubSecretArgs => Value);
command!(HubCall = "grid-client.hubCall" : HubCallArgs => Value);
