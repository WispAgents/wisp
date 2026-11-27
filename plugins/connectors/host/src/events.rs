//! MCP Events from a remote connector (SEP-3415; ADR-0146): `connectors.events` lists what it serves (`events/list`),
//! and the `mcp.event` trigger source (`trigger.source@1`, poll) reads `events/poll` with the cursor the engine keeps,
//! so any events-capable MCP server (an incident pager, a mail inbox, a CI) can start a task or wake a crew member with no wisp
//! plugin of its own. Every request carries the connector's sign-in and headers (`call::request`); a server that
//! refuses `initialize` is asked the `2026-07-28` way.
//!
//! Webhook first when this wisp has a public address (`hook.rs`), then push (ADR-0146): when the connector's listing offers `push` for the event (and the trigger doesn't say
//! `match.delivery: poll`), the first poll opens `events/stream` and every event reaches the engine as it happens
//! (`automations.trigger.push`). Polls then only keep the stream alive and carry its cursor to the trigger's row; a
//! stream that's down hands back to polling from its own cursor. Webhook delivery is later.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_automations_api::{Polled, SOURCE, SourceOn, TriggerEvent, TriggerSource};
use wisp_connectors_api::{EventsArgs, EventsOut};
use wisp_connectors_core::events::{self as ev, MAX_PAGES, Subscription};
use wisp_connectors_core::push;
use wisp_kernel::{BoxFuture, Ctx, Result, Setup, WireError};

use crate::call::request;
use crate::pane::Pane;
use crate::stream::{Live, Streams, lock, run};

/// `params` with the extension declared in `_meta`.
pub(crate) fn with_meta(mut params: Value) -> Value {
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

/// How long a connector's listing (`events/list`) is trusted before it's asked again.
const LISTING_MS: i64 = 10 * 60_000;
/// What `poll` does for a subscription.
enum Plan {
    /// a stream is open: nothing to ask, here is its cursor
    Streaming(Value),
    /// poll, from this cursor
    Poll(Value),
    /// see whether the server offers push
    TryPush(Value),
}

/// `mcp.event`: pushed (`events/stream`) when the connector offers it, polled (`events/poll`) otherwise or while its
/// stream is down. The engine's poll is the stream's keep-alive and carries its cursor to the trigger's row, so a
/// restart opens the stream where it was.
pub struct McpEvents {
    pane: Arc<Pane>,
    ctx: Ctx,
    streams: Arc<Streams>,
    hooks: Arc<crate::hook::Hooks>,
}

impl McpEvents {
    fn plan(&self, key: &str, kept: &Value, idle_ms: i64, now: i64) -> Plan {
        let mut live = lock(&self.streams.live);
        let Some(l) = live.get_mut(key) else { return Plan::TryPush(kept.clone()) };
        l.touched = now;
        l.idle_ms = idle_ms;
        match l.down_until {
            None => Plan::Streaming(ev::kept(&l.cursor)),
            // the stream's cursor is newer than the engine's
            Some(until) if until > now => Plan::Poll(ev::kept(&l.cursor)),
            Some(_) => {
                let c = ev::kept(&l.cursor);
                live.remove(key);
                Plan::TryPush(c)
            }
        }
    }

    /// The connector's listing, at most `LISTING_MS` old; one that can't be read lists nothing (so: poll).
    async fn listing(&self, connector: &str, now: i64) -> Vec<Value> {
        if let Some((at, l)) = lock(&self.streams.listings).get(connector).filter(|(at, _)| now - *at < LISTING_MS) {
            let _ = at;
            return l.clone();
        }
        let l = list(&self.ctx, &self.pane, connector).await.map(|o| o.events).unwrap_or_default();
        lock(&self.streams.listings).insert(connector.to_owned(), (now, l.clone()));
        l
    }

    async fn poll(&self, s: Subscription, spec: &Value, kept: Value) -> Result<Polled> {
        let now = self.ctx.clock()?.now_ms();
        let key = ev::sub_key(&s);
        // three polls missed: no trigger wants it any more
        let every = spec.get("every").and_then(Value::as_i64).unwrap_or(1).max(1);
        let idle_ms = (every * 60_000 * 3).max(5 * 60_000);
        // webhook first, when this wisp has a public address and the server offers it (SEP-3415's SDK guidance)
        if let Some(c) = self.by_webhook(&s, spec, &key, &kept, idle_ms, now).await {
            return Ok(Polled { events: Vec::new(), cursor: c, retry_at: None });
        }
        let kept = match self.plan(&key, &kept, idle_ms, now) {
            Plan::Streaming(c) => return Ok(Polled { events: Vec::new(), cursor: c, retry_at: None }),
            Plan::Poll(c) => c,
            Plan::TryPush(c) => {
                if push::wants_push(spec) && push::offers(&self.listing(&s.connector, now).await, &s.event, "push") {
                    let l = Live { cursor: ev::server_cursor(&c), touched: now, idle_ms, down_until: None };
                    lock(&self.streams.live).insert(key.clone(), l);
                    let (ctx, pane, streams) = (self.ctx.clone(), self.pane.clone(), self.streams.clone());
                    self.ctx.spawn(async move { run(ctx, pane, streams, key, s).await });
                    return Ok(Polled { events: Vec::new(), cursor: c, retry_at: None });
                }
                c
            }
        };
        let polled = self.poll_once(&s, kept, now).await?;
        // a stream that's down keeps the newest cursor for when it opens again
        if let Some(l) = lock(&self.streams.live).get_mut(&key) {
            l.cursor = ev::server_cursor(&polled.cursor);
        }
        Ok(polled)
    }

    /// The webhook's watermark while it's subscribed (subscribing or refreshing first when due); `None`: not by
    /// webhook (no public address, not offered, not wanted, or refused lately).
    async fn by_webhook(&self, s: &Subscription, spec: &Value, key: &str, kept: &Value, idle_ms: i64, now: i64) -> Option<Value> {
        for (sub, url) in self.hooks.idle(now) {
            // best effort: the server's TTL ends it anyway
            let _ = ask(
                &self.ctx,
                &self.pane,
                &sub.connector,
                &sub.event,
                "events/unsubscribe",
                wisp_connectors_core::hook::unsubscribe_params(&sub, &url),
            )
            .await;
        }
        if !push::wants(spec, "webhook") || self.hooks.refused(key, now) {
            return None;
        }
        let base = crate::hook::base(&self.ctx)?;
        match self.hooks.current(key, idle_ms, now) {
            Some((c, false)) => return Some(ev::kept(&c)),
            Some((_, true)) => {
                let (url, secret, cursor) = self.hooks.refresh_of(key)?;
                return self.subscribe(key, s, &url, &secret, &cursor, now).await;
            }
            None => {}
        }
        if !push::offers(&self.listing(&s.connector, now).await, &s.event, "webhook") {
            return None;
        }
        let cursor = ev::server_cursor(kept);
        let (url, secret) = self.hooks.open(key, s, &base, cursor.clone(), idle_ms, now);
        self.subscribe(key, s, &url, &secret, &cursor, now).await
    }

    async fn subscribe(&self, key: &str, s: &Subscription, url: &str, secret: &str, cursor: &Value, now: i64) -> Option<Value> {
        let params = wisp_connectors_core::hook::subscribe_params(s, url, secret, cursor);
        match ask(&self.ctx, &self.pane, &s.connector, &s.event, "events/subscribe", params).await {
            Ok(r) => Some(ev::kept(&self.hooks.granted(key, &r, now))),
            Err(e) => {
                tracing::warn!(connector = %s.connector, event = %s.event, "its webhook was refused, by push or poll for now: {}", e.message);
                self.hooks.refuse(key, now);
                None
            }
        }
    }

    async fn poll_once(&self, s: &Subscription, kept: Value, now: i64) -> Result<Polled> {
        let mut kept = kept;
        let mut events = Vec::new();
        for _ in 0..MAX_PAGES {
            let r = ask(&self.ctx, &self.pane, &s.connector, &s.event, "events/poll", ev::poll_params(s, &kept)).await?;
            let p =
                ev::page(s, &r).ok_or_else(|| WireError::unavailable(format!("{} answered events/poll with no events", s.connector)))?;
            if p.truncated {
                tracing::warn!(connector = %s.connector, event = %s.event, "events were skipped: the cursor fell out of the server's retention");
            }
            events.extend(p.events.into_iter().map(|o| trigger_event(o, now)));
            kept = ev::kept(&p.cursor);
            if !p.has_more {
                break;
            }
        }
        Ok(Polled { events, cursor: kept, retry_at: None })
    }
}

pub(crate) fn trigger_event(o: ev::Occurrence, now: i64) -> TriggerEvent {
    TriggerEvent {
        on: ev::ON.into(),
        key: o.key,
        at: o.at.unwrap_or(now),
        payload: o.payload,
        summary: o.summary,
        ..TriggerEvent::default()
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
        let me = Self { pane: self.pane.clone(), ctx: self.ctx.clone(), streams: self.streams.clone(), hooks: self.hooks.clone() };
        let (sub, spec) = (ev::subscription(spec), spec.clone());
        Box::pin(async move { me.poll(sub.map_err(WireError::invalid)?, &spec, cursor).await })
    }
}

pub(crate) fn register(s: &mut Setup, pane: Arc<Pane>) {
    let p = pane.clone();
    s.command::<wisp_connectors_api::Events, _, _>(move |ctx, _call, a: EventsArgs| {
        let pane = p.clone();
        async move { list(&ctx, &pane, &a.id).await }
    });
    let hooks = Arc::new(crate::hook::Hooks::default());
    let h = hooks.clone();
    s.route(wisp_kernel::http::Route {
        method: "POST",
        pattern: crate::hook::ROUTE,
        permission: wisp_kernel::http::OPEN,
        doc: true,
        summary: "An MCP server's webhook delivery for an mcp.event trigger (Standard Webhooks signature; at most 256 KiB)",
        raw: true,
        max_body: Some(256 * 1024),
        handler: Arc::new(move |ctx, _call, req| {
            let h = h.clone();
            Box::pin(async move { crate::hook::receive(ctx, h, req).await })
        }),
    });
    let src: Arc<dyn TriggerSource> = Arc::new(McpEvents { pane, ctx: s.ctx(), streams: Arc::default(), hooks });
    s.contribute(&SOURCE, src);
}
