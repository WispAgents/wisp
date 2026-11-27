//! MCP Events (SEP-3415, `io.modelcontextprotocol/events`; ADR-0146): the durable log served to any MCP client, so
//! an SDK agent, a chat app or another harness can react to `task.finished`, `pr.opened` or `goal.failed` without
//! `wisp_events` in a loop of its own.
//!
//! - `events/list`: the durable types of the catalogue (`events.ls`), each `delivery: ["webhook", "push", "poll"]`, with one argument
//!   (`subject`, a task id).
//! - `events/poll`: `events.poll`, run as the caller, so a task's agent sees its own task's events only. The cursor
//!   is the log's seq; a cursor past retention answers `truncated: true` and goes on from the oldest kept.
//! - `events/stream` (push, ADR-0146): opened with `events.poll`'s checks, then `active`, the backlog and each new event
//!   as the feed writes it, a heartbeat with the cursor in quiet stretches, over Streamable HTTP's SSE.
//! - `events/subscribe` / `events/unsubscribe` (webhook, `webhook.rs`): signed POSTs to a verified callback.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_events_api as api;
use wisp_kernel::durable::StoredEvent;
use wisp_kernel::{Call, Code, Ctx, Result, WireError};
use wisp_mcp_api::{METHOD_POINT, MethodContribution, MethodError};

pub const EXTENSION: &str = "io.modelcontextprotocol/events";
/// SEP-3415's codes (provisional numbers in the SEP, recorded in the core schema).
pub const NOT_FOUND: i64 = -32023;
pub const FORBIDDEN: i64 = -32024;
const INVALID_PARAMS: i64 = -32602;
pub(crate) const INTERNAL: i64 = -32603;
/// What a poll suggests between polls: the log is local, a poll that finds nothing costs one indexed read.
pub const NEXT_POLL_MS: u64 = 5_000;
const DEFAULT_MAX: u32 = 50;
pub(crate) const MAX: u32 = 500;
/// How many rows one poll reads before it answers `hasMore` (a narrow type over a long log).
const SCAN: usize = 20_000;

/// `new Date(ms).toISOString()`.
pub fn iso(ms: i64) -> String {
    let (days, rem) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    // Howard Hinnant's civil_from_days
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (h, m, s, milli) = (rem / 3_600_000, rem / 60_000 % 60, rem / 1000 % 60, rem % 1000);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}.{milli:03}Z")
}

/// The catalogue's durable types: only those are in the log a poll reads.
pub fn durable_types(catalogue: &Value) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = catalogue
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e.get("durable") == Some(&Value::Bool(true)))
        .filter_map(|e| Some((e.get("type")?.as_str()?.to_owned(), e.get("plugin").and_then(Value::as_str).unwrap_or_default().to_owned())))
        .collect();
    out.sort();
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// `events/list`'s result.
pub fn list(catalogue: &Value, samples: &std::collections::HashMap<String, Vec<Value>>) -> Value {
    let events: Vec<Value> = durable_types(catalogue)
        .into_iter()
        .map(|(ty, plugin)| {
            json!({
                "name": ty,
                "description": format!("wisp's {ty} (from {plugin}), as its durable event log keeps it"),
                "delivery": ["webhook", "push", "poll"],
                "inputSchema": { "type": "object", "properties": {
                    "subject": { "type": "string", "description": "only what's about this task (its id)" }
                } },
                "payloadSchema": crate::schema::payload_schema(&ty, samples.get(&ty).map_or(&[][..], Vec::as_slice)),
            })
        })
        .collect();
    json!({ "resultType": "complete", "events": events })
}

/// The newest kept events' data of each type (`schema::SAMPLES` a type), read from the last `schema::LOOK_BACK`
/// events: what `events/list` infers each `payloadSchema` from. Only shapes leave here, never the values.
pub async fn samples(ctx: &Ctx) -> std::collections::HashMap<String, Vec<Value>> {
    let mut out: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();
    let Ok(l) = crate::log(ctx) else { return out };
    let Ok((_, last)) = l.bounds().await else { return out };
    let mut after = last.saturating_sub(crate::schema::LOOK_BACK);
    while after < last {
        let Ok(rows) = l.read(after, 1000).await else { break };
        let Some(end) = rows.last().map(|r| r.seq) else { break };
        for r in rows.iter().rev() {
            let v = out.entry(r.event.ty.clone()).or_default();
            if v.len() < crate::schema::SAMPLES {
                v.push(occurrence(r)["data"].clone());
            }
        }
        after = end;
    }
    out
}

/// One kept event as an `EventOccurrence`: its seq is its id (stable, and the log never reuses one).
pub fn occurrence(e: &StoredEvent) -> Value {
    let data = match &e.event.data {
        Value::Object(_) => e.event.data.clone(),
        Value::Null => json!({}),
        other => json!({ "value": other }),
    };
    let mut meta = json!({ "dev.trywisp/seq": e.seq });
    if let Some(s) = &e.event.subject {
        meta["dev.trywisp/subject"] = json!(s);
    }
    json!({ "eventId": e.seq.to_string(), "name": e.event.ty, "timestamp": iso(e.event.at), "data": data, "_meta": meta })
}

/// A command's refusal in SEP-3415's codes.
pub fn refusal(e: &WireError) -> MethodError {
    let code = match e.code {
        Code::NotFound => NOT_FOUND,
        Code::Forbidden => FORBIDDEN,
        Code::Invalid => INVALID_PARAMS,
        _ => INTERNAL,
    };
    MethodError::new(code, e.message.clone())
}

/// `events.poll`: what matches after `cursor`, up to `maxEvents`, and where the next poll starts.
pub async fn poll(ctx: &Ctx, call: &Call, a: api::PollArgs) -> Result<Value> {
    if !durable_types(&crate::catalogue(ctx)).iter().any(|(t, _)| *t == a.name) {
        return Err(WireError::not_found(format!("No durable event called {} (events.ls lists them)", a.name)));
    }
    let asked = a.arguments.get("subject").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned);
    let subject = crate::scoped_subject(call, asked)?;
    let l = crate::log(ctx)?;
    let (first, last) = l.bounds().await?;
    let Some(cursor) = a.cursor.as_deref().filter(|c| !c.is_empty()) else {
        // no cursor: from now on
        return Ok(json!({ "events": [], "cursor": last.to_string(), "truncated": false, "hasMore": false, "nextPollMs": NEXT_POLL_MS }));
    };
    let c: u64 = cursor.parse().map_err(|_| WireError::invalid(format!("cursor {cursor} isn't one this server gave")))?;
    // the events right after the cursor are gone (retention): say so, and go on from the oldest kept
    let truncated = first > 0 && c.saturating_add(1) < first;
    let mut after = if truncated { first - 1 } else { c.min(last) };
    let limit = usize::try_from(a.max_events.unwrap_or(DEFAULT_MAX).clamp(1, MAX)).unwrap_or(50);
    let oldest = match a.max_age_ms {
        Some(age) => ctx.clock()?.now_ms().saturating_sub(i64::try_from(age).unwrap_or(i64::MAX)),
        None => i64::MIN,
    };
    let (mut out, mut scanned) = (Vec::new(), 0usize);
    'read: while after < last && scanned < SCAN {
        let rows = l.read(after, 1000).await?;
        if rows.is_empty() {
            break;
        }
        scanned += rows.len();
        for r in &rows {
            after = r.seq;
            let ok =
                r.event.ty == a.name && r.event.at >= oldest && subject.as_deref().is_none_or(|s| r.event.subject.as_deref() == Some(s));
            if ok {
                out.push(occurrence(r));
                if out.len() >= limit {
                    break 'read;
                }
            }
        }
    }
    Ok(json!({ "events": out, "cursor": after.to_string(), "truncated": truncated, "hasMore": after < last, "nextPollMs": NEXT_POLL_MS }))
}

fn method(name: &str, run: wisp_mcp_api::MethodFn) -> Arc<MethodContribution> {
    Arc::new(MethodContribution { method: name.into(), extension: Some((EXTENSION.into(), json!({}))), run })
}

/// A method's params as the command takes them (`_meta` is the protocol's).
pub(crate) fn poll_args(params: Value, method: &str) -> std::result::Result<api::PollArgs, MethodError> {
    let mut p = params;
    if let Some(o) = p.as_object_mut() {
        for k in ["_meta", "heartbeatMs", "delivery", "ttlMs"] {
            o.remove(k);
        }
    }
    serde_json::from_value(p).map_err(|e| MethodError::new(INVALID_PARAMS, format!("{method}: {e}")))
}

/// The methods, on `mcp.method@1`. Each runs this plugin's own command from its own context (wisp.mcp's would need
/// this plugin's permissions), as the caller, so the caller's capability set and scoping apply.
pub fn register(s: &mut wisp_kernel::Setup, newest_seq: &Arc<tokio::sync::watch::Sender<u64>>) {
    let own = s.ctx();
    let list_fn: wisp_mcp_api::MethodFn = Arc::new(move |_mcp: Ctx, call: Call, _req| {
        let ctx = own.clone();
        Box::pin(async move {
            let c = ctx.run::<api::Ls>(&call, api::NoArgs {}).await.map_err(|e| refusal(&e))?;
            Ok(wisp_mcp_api::MethodAnswer::Result(list(&c, &samples(&ctx).await)))
        })
    });
    let own = s.ctx();
    let poll_fn: wisp_mcp_api::MethodFn = Arc::new(move |_mcp: Ctx, call: Call, req: wisp_mcp_api::MethodRequest| {
        let ctx = own.clone();
        Box::pin(async move {
            let a = poll_args(req.params, "events/poll")?;
            ctx.run::<api::Poll>(&call, a).await.map(wisp_mcp_api::MethodAnswer::Result).map_err(|e| refusal(&e))
        })
    });
    let (own, woken, open) = (s.ctx(), newest_seq.clone(), Arc::new(crate::stream::Open::default()));
    let stream_fn: wisp_mcp_api::MethodFn = Arc::new(move |_mcp: Ctx, call: Call, req: wisp_mcp_api::MethodRequest| {
        let (ctx, seq, open) = (own.clone(), woken.clone(), open.clone());
        Box::pin(async move { crate::stream::open(ctx, call, req, &seq, &open).await })
    });
    let (own, hooks) = (s.ctx(), Arc::new(crate::webhook::Hooks::default()));
    let h = hooks.clone();
    s.command::<api::Webhooks, _, _>(move |ctx, call, _a| {
        let h = h.clone();
        async move { Ok(crate::webhook::list(&call, &h, ctx.clock()?.now_ms())) }
    });
    let (h, seq) = (hooks.clone(), newest_seq.clone());
    let subscribe_fn: wisp_mcp_api::MethodFn = Arc::new(move |_mcp: Ctx, call: Call, req: wisp_mcp_api::MethodRequest| {
        let (ctx, h, seq) = (own.clone(), h.clone(), seq.clone());
        Box::pin(async move { crate::webhook::subscribe(ctx, call, req, h, seq).await })
    });
    let unsubscribe_fn: wisp_mcp_api::MethodFn = Arc::new(move |_mcp: Ctx, call: Call, req: wisp_mcp_api::MethodRequest| {
        let h = hooks.clone();
        Box::pin(async move { crate::webhook::unsubscribe(&call, &req, &h) })
    });
    for m in [
        method("events/list", list_fn),
        method("events/poll", poll_fn),
        method("events/stream", stream_fn),
        method("events/subscribe", subscribe_fn),
        method("events/unsubscribe", unsubscribe_fn),
    ] {
        s.contribute(&METHOD_POINT, m);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn iso_is_javascripts_to_iso_string() {
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso(1_795_807_011_123), "2026-11-27T19:16:51.123Z");
    }

    #[test]
    fn only_durable_types_are_listed_each_once_by_poll() {
        let cat = json!([
            { "type": "task.finished", "durable": true, "plugin": "wisp.runtime" },
            { "type": "automations.changed", "durable": false, "plugin": "wisp.automations" },
            { "type": "task.finished", "durable": true, "plugin": "wisp.other" },
            { "type": "goal.met", "durable": true, "plugin": "wisp.goal" },
        ]);
        let l = list(&cat, &std::collections::HashMap::new());
        let names: Vec<&str> = l["events"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["goal.met", "task.finished"]);
        assert_eq!(l["events"][0]["delivery"], json!(["webhook", "push", "poll"]));
        assert_eq!(l["resultType"], "complete");
    }

    #[test]
    fn refusals_carry_the_seps_codes() {
        assert_eq!(refusal(&WireError::not_found("x")).code, NOT_FOUND);
        assert_eq!(refusal(&WireError::forbidden("x")).code, FORBIDDEN);
        assert_eq!(refusal(&WireError::invalid("x")).code, INVALID_PARAMS);
        assert_eq!(refusal(&WireError::unavailable("x")).code, INTERNAL);
    }
}
