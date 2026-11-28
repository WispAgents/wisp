//! A team hub's GitHub App webhook for issues (`issues`: `issue.opened`, `issue.labeled`) (ADR-0148 §8e): the grid checked GitHub's signature and passes it here
//! as `grid:github`, which reaches this route only. Its events are the poll's own (same payload, same key: the engine
//! fires a trigger once whichever comes first), each placed in the projects that name its repo, and pushed to the
//! triggers. A repo no project names starts nothing.
use std::sync::Arc;

use serde_json::{Value, json};
use wisp_automations_api::commands::TriggerPush;
use wisp_kernel::http::{Response, Route};
use wisp_kernel::{Ctx, Result, WireError};

/// `POST /api/issues/webhook`.
pub fn route() -> Route {
    Route {
        method: "POST",
        pattern: "/issues/webhook",
        permission: "issues:webhook",
        doc: false,
        summary: "A GitHub App webhook the grid checked, to this hub's triggers",
        raw: false,
        max_body: Some(25 << 20),
        handler: Arc::new(|ctx, _call, req| {
            Box::pin(async move {
                let body: Value = serde_json::from_slice(&req.body).map_err(|e| WireError::invalid(format!("not JSON: {e}")))?;
                let at = ctx.clock()?.now_ms();
                let n = push(&ctx, wisp_issues_core::trigger::webhook_events(&body, at)).await?;
                Ok(Response::Json { status: 200, body: json!({ "pushed": n }) })
            })
        }),
    }
}

/// The events, in each project naming their repo, to the triggers: how many triggers took one.
async fn push(ctx: &Ctx, events: Vec<wisp_automations_api::TriggerEvent>) -> Result<u32> {
    if events.is_empty() {
        return Ok(0);
    }
    let call = ctx.root_call();
    let kinds = vec!["project".to_owned()];
    let all = ctx.run::<wisp_resources_api::Export>(&call, wisp_resources_api::ExportArgs { kinds, include: Vec::new() }).await?;
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let projects: Vec<(String, String, String)> =
        all.iter().filter(|r| r.kind == "project").map(|r| (r.name.clone(), s(&r.spec, "path"), s(&r.spec, "repo"))).collect();
    let mut n = 0;
    for e in wisp_automations_api::placed(events, &projects) {
        n += ctx.run::<TriggerPush>(&call, e).await?.n;
    }
    Ok(n)
}
