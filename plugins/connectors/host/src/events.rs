//! MCP Events from a remote connector (SEP-3415; ADR-0145): `connectors.events` lists what it serves (`events/list`),
//! and the `mcp.event` trigger source (`trigger.source@1`, poll) reads `events/poll` with the cursor the engine keeps,
//! so any events-capable MCP server (an incident pager, a mail inbox, a CI) can start a task or wake a crew member with no wisp
//! plugin of its own. Every request carries the connector's sign-in and headers (`call::request`); a server that
//! refuses `initialize` is asked the `2026-07-28` way.
//!
//! Poll only: it works behind NAT, from a laptop, with nothing kept on the server. Push (`events/stream`) and webhook
//! delivery are later (ADR-0145).

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_automations_api::{Polled, SOURCE, SourceOn, TriggerEvent, TriggerSource};
use wisp_connectors_api::{EventsArgs, EventsOut};
use wisp_connectors_core::events::{self as ev, MAX_PAGES, Subscription};
use wisp_kernel::{BoxFuture, Ctx, Result, Setup, WireError};

use crate::call::request;
use crate::pane::Pane;

/// `params` with the extension declared in `_meta`.
fn with_meta(mut params: Value) -> Value {
    if let Some(o) = params.as_object_mut() {
        o.insert("_meta".into(), ev::meta(None));
    }
    params
}

/// The result of an events request, or why not in words.
async fn ask(ctx: &Ctx, pane: &Pane, connector: &str, event: &str, method: &str, params: Value) -> Result<Value> {
    let v = request(ctx, pane, connector, method, with_meta(params), true).await?;
    if let Some(e) = v.get("error") {
        return Err(WireError::unavailable(ev::refusal(connector, event, e)));
    }
    Ok(v.get("result").cloned().unwrap_or(Value::Null))
}

/// `connectors.events`: every page of `events/list` (ten at most).
async fn list(ctx: &Ctx, pane: &Pane, id: &str) -> Result<EventsOut> {
    let mut events = Vec::new();
    let mut cursor = Value::Null;
    for _ in 0..MAX_PAGES {
        let params = if cursor.is_null() { json!({}) } else { json!({ "cursor": cursor }) };
        let v = request(ctx, pane, id, "events/list", with_meta(params), true).await?;
        if let Some(e) = v.get("error") {
            if e.get("code").and_then(Value::as_i64) == Some(-32601) {
                return Ok(EventsOut { served: false, events: Vec::new() });
            }
            return Err(WireError::unavailable(ev::refusal(id, "events/list", e)));
        }
        let r = v.get("result").cloned().unwrap_or(Value::Null);
        events.extend(r.get("events").and_then(Value::as_array).cloned().unwrap_or_default());
        cursor = r.get("nextCursor").cloned().unwrap_or(Value::Null);
        if cursor.is_null() {
            break;
        }
    }
    Ok(EventsOut { served: true, events })
}

/// `mcp.event`, polled.
pub struct McpEvents {
    pane: Arc<Pane>,
    ctx: Ctx,
}

impl McpEvents {
    async fn poll(&self, s: Subscription, kept: Value) -> Result<Polled> {
        let now = self.ctx.clock()?.now_ms();
        let mut kept = kept;
        let mut events = Vec::new();
        for _ in 0..MAX_PAGES {
            let r = ask(&self.ctx, &self.pane, &s.connector, &s.event, "events/poll", ev::poll_params(&s, &kept)).await?;
            let p =
                ev::page(&s, &r).ok_or_else(|| WireError::unavailable(format!("{} answered events/poll with no events", s.connector)))?;
            if p.truncated {
                tracing::warn!(connector = %s.connector, event = %s.event, "events were skipped: the cursor fell out of the server's retention");
            }
            events.extend(p.events.into_iter().map(|o| TriggerEvent {
                on: ev::ON.into(),
                key: o.key,
                at: o.at.unwrap_or(now),
                payload: o.payload,
                summary: o.summary,
                ..TriggerEvent::default()
            }));
            kept = ev::kept(&p.cursor);
            if !p.has_more {
                break;
            }
        }
        Ok(Polled { events, cursor: kept, retry_at: None })
    }
}

impl TriggerSource for McpEvents {
    fn plugin(&self) -> &'static str {
        "wisp.connectors"
    }
    fn ons(&self) -> Vec<SourceOn> {
        vec![SourceOn {
            on: ev::ON.into(),
            title: "An MCP server's event".into(),
            fields: json!({ "type": "object", "required": ["connector", "event"], "properties": {
                "connector": { "type": "string", "description": "a remote connector that serves MCP Events" },
                "event": { "type": "string", "description": "one of its event types (connectors.events lists them)" },
                "arguments": { "type": "object", "description": "the event type's arguments (its inputSchema)" }
            } }),
            mode: "poll".into(),
            every: Some(1),
        }]
    }
    fn poll(&self, _on: &str, spec: &Value, cursor: Value) -> BoxFuture<'static, Result<Polled>> {
        let me = Self { pane: self.pane.clone(), ctx: self.ctx.clone() };
        let sub = ev::subscription(spec);
        Box::pin(async move { me.poll(sub.map_err(WireError::invalid)?, cursor).await })
    }
}

pub(crate) fn register(s: &mut Setup, pane: Arc<Pane>) {
    let p = pane.clone();
    s.command::<wisp_connectors_api::Events, _, _>(move |ctx, _call, a: EventsArgs| {
        let pane = p.clone();
        async move { list(&ctx, &pane, &a.id).await }
    });
    let src: Arc<dyn TriggerSource> = Arc::new(McpEvents { pane, ctx: s.ctx() });
    s.contribute(&SOURCE, src);
}
