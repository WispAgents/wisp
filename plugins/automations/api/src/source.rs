//! `trigger.source@1` (ADR-0043): where a trigger's events come from. Every durable event is a source by
//! itself (`on: <type>`, read from the log); this point is for the rest: polled services (CI, issues) and
//! pushes from outside (a chat message, files changing). A source lives in the plugin that owns the data:
//! switching that plugin off takes its sources with it, and nothing else.
//!
//! - **push:** the source calls `automations.trigger.push` with a [`TriggerEvent`] when something happens.
//! - **poll:** the engine calls `poll` for each trigger on it, every `every` (one scheduler for all), with
//!   the cursor it returned last time; a rate limit answers `retry_at` and isn't a failure.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use wisp_kernel::{BoxFuture, Event, Point, Result};

/// What a source saw: one event for a trigger.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "automations/")]
pub struct TriggerEvent {
    /// the source name (`ci.failed`, `chat.message`)
    pub on: String,
    /// its dedupe key: the same key fires a trigger once
    pub key: String,
    #[ts(type = "number")]
    pub at: i64,
    #[ts(type = "unknown")]
    #[schemars(with = "Value")]
    pub payload: Value,
    /// one line for the prompt (`{event.summary}`)
    #[serde(default)]
    pub summary: String,
    /// what it's about: a task id (a follow-up goes there)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub subject: Option<String>,
    /// the lineage of the task it came from (`trigger:<name>`): a trigger never fires on its own tasks
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub by: Option<String>,
    /// how many causes deep (a trigger's task's event that started a trigger's task …)
    #[serde(default)]
    pub depth: u32,
}

/// One `on` a source offers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "automations/")]
pub struct SourceOn {
    pub on: String,
    pub title: String,
    /// the JSON Schema of its fields (the editor, `wisp trigger add --help`, formula checks)
    #[ts(type = "unknown")]
    pub fields: Value,
    /// push | poll
    pub mode: String,
    /// a poll's default minutes between polls
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub every: Option<u32>,
}

/// A poll's answer: what's new since `cursor`, and where to start next time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Polled {
    pub events: Vec<TriggerEvent>,
    pub cursor: Value,
    /// rate-limited: don't ask again before this (ms); not a failure
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<i64>,
}

pub trait TriggerSource: Send + Sync + 'static {
    /// the plugin it belongs to (shown when it's off)
    fn plugin(&self) -> &'static str;
    fn ons(&self) -> Vec<SourceOn>;
    /// a polled source's events for a trigger with `spec` (its `match` and flat fields), since `cursor`
    fn poll(&self, _on: &str, _spec: &Value, _cursor: Value) -> BoxFuture<'static, Result<Polled>> {
        Box::pin(async { Ok(Polled::default()) })
    }
}

pub const SOURCE: Point<dyn TriggerSource> = Point::new("trigger.source", 1);

/// A trigger fired: what it started (ADR-0043). Durable: a hand-off or an audit log can follow it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "automations/")]
pub struct TriggerFired {
    pub trigger: String,
    pub on: String,
    /// the event's key
    pub key: String,
    /// started | queued | held | skipped | awaiting
    pub result: String,
    #[serde(default)]
    pub why: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub session: Option<String>,
    /// how many causes deep it is: the event that made the trigger decide, plus one (a trigger on `trigger.fired`
    /// reads it, so two triggers on each other's runs stop at `MAX_DEPTH`). Additive (2026-11-25).
    #[serde(default, skip_serializing_if = "is_zero")]
    #[ts(as = "Option<u32>", optional)]
    pub depth: u32,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if takes a reference
fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Event for TriggerFired {
    const TYPE: &'static str = "trigger.fired";
    const DURABLE: bool = true;
    fn subject(&self) -> Option<String> {
        self.session.clone()
    }
    fn key(&self) -> Option<String> {
        Some(format!("{}:{}:{}", self.trigger, self.key, self.result))
    }
}

/// A trigger a plugin provides, and what the engine can't read from its spec.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Provided {
    pub name: String,
    pub spec: crate::spec::TriggerSpec,
    /// whom it wakes is paused: its jobs are held until they're resumed
    pub paused: bool,
    /// whom it wakes runs in Full auto (approval never asks then)
    pub full_auto: bool,
}

/// `trigger.provider@1`: triggers a plugin declares inside its own specs, run by the one engine. The crew gives its
/// members' `wakes:` (each a trigger targeting the member), named `crew.<member>.<n>`. Read on every event, so
/// it answers from memory.
pub trait TriggerProvider: Send + Sync + 'static {
    fn triggers(&self) -> Vec<Provided>;
}

pub const PROVIDERS: Point<dyn TriggerProvider> = Point::new("trigger.provider", 1);

/// A repo as `owner/name`, lowercase, from any way it's written (`https://github.com/acme/app.git`,
/// `git@github.com:acme/app.git`, `acme/app`): how a webhook's repository finds the project that names it (ADR-0148
/// §8e). Empty for anything without both parts.
pub fn repo_slug(repo: &str) -> String {
    let r = repo.trim().trim_end_matches('/');
    let r = r.strip_suffix(".git").unwrap_or(r);
    let path = r
        .split_once("://")
        .map_or_else(|| r.split_once(':').map_or(r, |(_, p)| p), |(_, rest)| rest.split_once('/').map_or("", |(_, p)| p));
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        [.., owner, name] => format!("{owner}/{name}").to_lowercase(),
        _ => String::new(),
    }
}

/// A webhook's events (each saying its `repo`) placed in the projects that name that repo: one copy per such project,
/// saying it as `project` (its name) and `cwd` (its folder), which a trigger's `project:` matches either way. A repo no
/// project names places nothing: its events reach no trigger. `projects`: `(name, folder, repo)`.
pub fn placed(events: Vec<TriggerEvent>, projects: &[(String, String, String)]) -> Vec<TriggerEvent> {
    let mut out = Vec::new();
    for e in events {
        let repo = repo_slug(e.payload.get("repo").and_then(Value::as_str).unwrap_or_default());
        if repo.is_empty() {
            continue;
        }
        for (name, folder, _) in projects.iter().filter(|(_, _, r)| repo_slug(r) == repo) {
            let mut one = e.clone();
            one.payload["project"] = Value::String(name.clone());
            one.payload["cwd"] = Value::String(folder.clone());
            out.push(one);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod repo_tests {
    use super::{TriggerEvent, placed, repo_slug};
    use serde_json::json;

    #[test]
    fn a_webhooks_event_goes_to_each_project_naming_its_repo_and_nowhere_else() {
        let ev = TriggerEvent {
            on: "issue.opened".into(),
            key: "issue:3:opened".into(),
            payload: json!({ "repo": "acme/app" }),
            ..TriggerEvent::default()
        };
        let projects = [
            ("app".to_owned(), "/hub/repos/app/app".to_owned(), "https://github.com/acme/app".to_owned()),
            ("web".to_owned(), "/hub/repos/web/web".to_owned(), "acme/web".to_owned()),
            ("app-2".to_owned(), "/hub/repos/app-2/app".to_owned(), "git@github.com:Acme/app.git".to_owned()),
        ];
        let out = placed(vec![ev.clone()], &projects);
        let places: Vec<_> = out.iter().map(|e| (e.payload["project"].clone(), e.payload["cwd"].clone())).collect();
        assert_eq!(places, vec![(json!("app"), json!("/hub/repos/app/app")), (json!("app-2"), json!("/hub/repos/app-2/app"))]);
        assert!(placed(vec![TriggerEvent { payload: json!({ "repo": "acme/other" }), ..ev.clone() }], &projects).is_empty());
        assert!(placed(vec![TriggerEvent { payload: json!({}), ..ev }], &projects).is_empty(), "no repo: nowhere");
    }

    #[test]
    fn a_repo_is_its_owner_and_name_however_written() {
        for r in ["https://github.com/Acme/App", "https://github.com/acme/app.git", "git@github.com:acme/app.git", "acme/app", "acme/app/"]
        {
            assert_eq!(repo_slug(r), "acme/app", "{r}");
        }
        assert_eq!(repo_slug("app"), "");
        assert_eq!(repo_slug(""), "");
    }
}
