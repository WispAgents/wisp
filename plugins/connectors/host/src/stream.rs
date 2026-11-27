//! A connector's MCP Events by push (SEP-3415 `events/stream`; ADR-0146): one stream a subscription, shared by the
//! triggers that name it, kept alive by the engine's polls. Each event goes to the engine as it comes; no message for
//! three heartbeats, it's opened again with backoff; failing or ended, polling takes over from its cursor.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use wisp_automations_api::commands::TriggerPush;
use wisp_connectors_core::events::{self as ev, Subscription};
use wisp_connectors_core::push;
use wisp_kernel::Ctx;

use crate::events::{trigger_event, with_meta};
use crate::pane::Pane;

/// A stream the server ended (terminated, refused) is opened again after this; polls carry on meanwhile.
const ENDED_MS: i64 = 60 * 60_000;
/// After this many failed opens in a row, polls carry on for `DOWN_MS` before a stream is tried again.
const MAX_FAILURES: u32 = 5;
const DOWN_MS: i64 = 10 * 60_000;

/// A subscription's stream, shared by every trigger that names it. The engine's poll keeps it alive: a stream no
/// trigger polled for `idle_ms` (a trigger switched off or removed) closes.
pub(crate) struct Live {
    /// the server's cursor, past the last event the stream gave
    pub(crate) cursor: Value,
    pub(crate) touched: i64,
    pub(crate) idle_ms: i64,
    /// polls carry on until then (the stream is down); `None`: it's open
    pub(crate) down_until: Option<i64>,
}

#[derive(Default)]
pub(crate) struct Streams {
    pub(crate) live: Mutex<HashMap<String, Live>>,
    pub(crate) listings: Mutex<HashMap<String, (i64, Vec<Value>)>>,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
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
pub(crate) async fn run(ctx: Ctx, pane: Arc<Pane>, streams: Arc<Streams>, key: String, s: Subscription) {
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
