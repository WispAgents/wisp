//! `issue.opened` and `issue.labeled` (ADR-0043): a polled trigger source over the picker's own lister (gh
//! through proc, cached a minute), so the trigger and the picker read GitHub the same way.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_automations_api::{Polled, SourceOn, TriggerSource};
use wisp_issues_api::{ListArgs, Source};
use wisp_kernel::{BoxFuture, Ctx, Result, WireError};

use crate::list::Lister;

pub struct Issues(pub Arc<Lister>, pub Ctx);

impl TriggerSource for Issues {
    fn plugin(&self) -> &'static str {
        "wisp.issues"
    }
    fn ons(&self) -> Vec<SourceOn> {
        // `me` in assignee/assignees/author is whoever this computer's gh is signed in as: one shared trigger, each
        // teammate's wisp takes only their own issues
        let who = |d: &str| json!({ "type": ["string", "array"], "description": d });
        let fields = json!({ "type": "object", "properties": {
            "project": { "type": "string" },
            "label": { "type": "string" },
            "author": who("who opened it (a GitHub login, or me)"),
            "assignee": who("its first assignee (a GitHub login, or me)"),
            "assignees": who("any of its assignees (a GitHub login, or me)"),
            "bot": { "type": "boolean", "description": "false: not opened by a bot (Dependabot, an app)" },
            "trusted": { "type": "boolean", "description": "true: only authors who may push to the repo" },
            "milestone": { "type": ["string", "array"], "description": "its milestone's title (a glob, or !v1*)" },
            "labels": { "type": ["string", "array"], "description": "any of its labels, or !wip for none of them" },
        } });
        vec![
            SourceOn {
                on: "issue.opened".into(),
                title: "An issue opened".into(),
                fields: fields.clone(),
                mode: "poll".into(),
                every: Some(5),
            },
            SourceOn { on: "issue.labeled".into(), title: "An issue labelled".into(), fields, mode: "poll".into(), every: Some(5) },
        ]
    }
    fn poll(&self, on: &str, spec: &Value, cursor: Value) -> BoxFuture<'static, Result<Polled>> {
        let (lister, ctx, on) = (self.0.clone(), self.1.clone(), on.to_owned());
        let project = spec.get("project").and_then(Value::as_str).unwrap_or_default().to_owned();
        let trusted = spec.get("match").and_then(|m| m.get("trusted")).is_some();
        Box::pin(async move {
            if project.is_empty() {
                return Err(WireError::invalid(format!("{on} needs a project (the repo it watches)")));
            }
            let call = ctx.root_call();
            let list = lister.list(&call, ListArgs { source: Source::Github, project: Some(project), q: String::new() }).await?;
            if let Some(e) = list.error {
                return Err(WireError::unavailable(e));
            }
            let now = ctx.clock()?.now_ms();
            let me = lister.me(&call).await;
            let (mut events, cursor) = wisp_issues_core::trigger::issue_events(&on, &list.issues, &cursor, now, &me);
            // `trusted` costs a gh call per new author (kept an hour): only for a trigger that filters on it
            if trusted {
                let repo = list.repo.clone().unwrap_or_default();
                for e in &mut events {
                    let login = e.payload.get("author").and_then(Value::as_str).unwrap_or_default().to_owned();
                    let bot = e.payload.get("bot").and_then(Value::as_bool).unwrap_or(false);
                    e.payload["trusted"] = json!(!bot && lister.may_push(&call, &repo, &login).await);
                }
            }
            Ok(Polled { events, cursor, retry_at: None })
        })
    }
}
