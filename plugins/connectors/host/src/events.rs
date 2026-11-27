//! MCP Events from a remote connector (SEP-3415; ADR-0145): `connectors.events` lists what it serves (`events/list`),
//! and the `mcp.event` trigger source (`trigger.source@1`, poll) reads `events/poll` with the cursor the engine keeps,
//! so any events-capable MCP server (an incident pager, a mail inbox, a CI) can start a task or wake a crew member with no wisp
//! plugin of its own. Every request carries the connector's sign-in and headers (`call::request`); a server that
//! refuses `initialize` is asked the `2026-07-28` way.
//!
//! Push first (ADR-0145): when the connector's listing offers `push` for the event (and the trigger doesn't say
//! `match.delivery: poll`), the first poll opens `events/stream` and every event reaches the engine as it happens
//! (`automations.trigger.push`). Polls then only keep the stream alive and carry its cursor to the trigger's row; a
//! stream that's down hands back to polling from its own cursor. Webhook delivery is later.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_automations_api::commands::TriggerPush;
use wisp_automations_api::{Polled, SOURCE, SourceOn, TriggerEvent, TriggerSource};
use wisp_connectors_api::{EventsArgs, EventsOut};
use wisp_connectors_core::events::{self as ev, MAX_PAGES, Subscription};
use wisp_connectors_core::push;
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

/// How long a connector's listing (`events/list`) is trusted before it's asked again.
const LISTING_MS: i64 = 10 * 60_000;
/// A stream the server ended (terminated, refused) is opened again after this; polls carry on meanwhile.
const ENDED_MS: i64 = 60 * 60_000;
/// After this many failed opens in a row, polls carry on for `DOWN_MS` before a stream is tried again.
const MAX_FAILURES: u32 = 5;
const DOWN_MS: i64 = 10 * 60_000;

/// A subscription's stream, shared by every trigger that names it. The engine's poll keeps it alive: a stream no
/// trigger polled for `idle_ms` (a trigger switched off or removed) closes.
struct Live {
    /// the server's cursor, past the last event the stream gave
    cursor: Value,
    touched: i64,
    idle_ms: i64,
    /// polls carry on until then (the stream is down); `None`: it's open
    down_until: Option<i64>,
}

#[derive(Default)]
struct Streams {
    live: Mutex<HashMap<String, Live>>,
    listings: Mutex<HashMap<String, (i64, Vec<Value>)>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

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

fn trigger_event(o: ev::Occurrence, now: i64) -> TriggerEvent {
    TriggerEvent {
        on: ev::ON.into(),
        key: o.key,
        at: o.at.unwrap_or(now),
        payload: o.payload,
        summary: o.summary,
        ..TriggerEvent::default()
    }
}

/// How a stream ended.
enum Ended {
    /// it dropped, timed out or the server closed it: open it again
    Again { said_something: bool },
    /// the server ended the subscription: poll for a while
    Stop(String),
    /// no trigger wants it
    Idle,
}

/// One subscription's stream, opened again with its cursor until no trigger wants it, the server ends it, or it
/// fails `MAX_FAILURES` times in a row (then polls carry on for a while).
async fn run(ctx: Ctx, pane: Arc<Pane>, streams: Arc<Streams>, key: String, s: Subscription) {
    let mut failures = 0u32;
    loop {
        let Ok(now) = ctx.clock().map(|c| c.now_ms()) else { return };
        let cursor = {
            let mut live = lock(&streams.live);
            match live.get(&key) {
                Some(l) if now - l.touched > l.idle_ms => {
                    live.remove(&key);
                    return;
                }
                Some(l) => l.cursor.clone(),
                None => return,
            }
        };
        let ended =
            match crate::call::open_stream(&ctx, &pane, &s.connector, "events/stream", with_meta(push::stream_params(&s, &cursor))).await {
                Ok(crate::call::Got::Stream(st)) => read(&ctx, &streams, &key, &s, st).await,
                Ok(crate::call::Got::Answer(v)) => Ended::Stop(v.get("error").map_or_else(
                    || format!("{} answered events/stream without a stream", s.connector),
                    |e| ev::refusal(&s.connector, &s.event, e),
                )),
                Err(e) => Ended::Again { said_something: e.code == wisp_kernel::Code::Timeout },
            };
        match ended {
            Ended::Idle => {
                lock(&streams.live).remove(&key);
                return;
            }
            Ended::Stop(why) => {
                tracing::warn!(connector = %s.connector, event = %s.event, "its stream ended, polling for now: {why}");
                return down(&ctx, &streams, &key, ENDED_MS);
            }
            Ended::Again { said_something: true } => failures = 0,
            Ended::Again { said_something: false } => failures += 1,
        }
        if failures >= MAX_FAILURES {
            tracing::warn!(connector = %s.connector, event = %s.event, "its stream keeps failing, polling for now");
            return down(&ctx, &streams, &key, DOWN_MS);
        }
        let jitter = ctx.clock().map_or(0, |c| c.now_ms().rem_euclid(1000).unsigned_abs());
        tokio::time::sleep(std::time::Duration::from_millis(push::backoff_ms(failures, jitter))).await;
    }
}

fn down(ctx: &Ctx, streams: &Streams, key: &str, for_ms: i64) {
    let now = ctx.clock().map_or(0, |c| c.now_ms());
    if let Some(l) = lock(&streams.live).get_mut(key) {
        l.down_until = Some(now + for_ms);
    }
}

/// A stream's frames until it ends: each event pushed to the trigger engine at once, the cursor kept as it moves.
/// No message for three heartbeats: it's dead (SEP-3415).
async fn read(ctx: &Ctx, streams: &Streams, key: &str, s: &Subscription, st: wisp_kernel::services::NetStream) -> Ended {
    use futures::StreamExt as _;
    let mut body = st.body;
    let mut sse = wisp_connectors_core::rpc::Sse::default();
    let mut heartbeat = push::HEARTBEAT_MS;
    let mut said = false;
    loop {
        let wait = std::time::Duration::from_millis(heartbeat.saturating_mul(3));
        let Ok(Some(Ok(chunk))) = tokio::time::timeout(wait, body.next()).await else {
            return Ended::Again { said_something: said };
        };
        for e in sse.push(&chunk) {
            let Ok(msg) = serde_json::from_str::<Value>(&e.data) else { continue };
            said = true;
            let set = |c: Value| {
                if let Some(l) = lock(&streams.live).get_mut(key) {
                    l.cursor = c;
                }
            };
            match push::frame(s, &msg) {
                push::Frame::Active { cursor, heartbeat_ms, truncated } => {
                    if truncated {
                        tracing::warn!(connector = %s.connector, event = %s.event, "events were skipped: the cursor fell out of the server's retention");
                    }
                    heartbeat = heartbeat_ms.unwrap_or(heartbeat).max(1_000);
                    set(cursor);
                }
                push::Frame::Event { occurrence, cursor } => {
                    let now = ctx.clock().map_or(0, |c| c.now_ms());
                    if let Err(e) = ctx.run::<TriggerPush>(&ctx.root_call(), trigger_event(occurrence, now)).await {
                        tracing::warn!(connector = %s.connector, "an event didn't reach the triggers: {}", e.message);
                    }
                    set(cursor);
                }
                push::Frame::Heartbeat { cursor } => set(cursor),
                push::Frame::Error(why) => tracing::warn!(connector = %s.connector, event = %s.event, "{why}"),
                push::Frame::Terminated(why) => return Ended::Stop(why),
                push::Frame::Closed => return Ended::Again { said_something: said },
                push::Frame::Other => {}
            }
        }
        let now = ctx.clock().map_or(0, |c| c.now_ms());
        if lock(&streams.live).get(key).is_none_or(|l| now - l.touched > l.idle_ms) {
            return Ended::Idle;
        }
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
        let me = Self { pane: self.pane.clone(), ctx: self.ctx.clone(), streams: self.streams.clone() };
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
    let src: Arc<dyn TriggerSource> = Arc::new(McpEvents { pane, ctx: s.ctx(), streams: Arc::default() });
    s.contribute(&SOURCE, src);
}
