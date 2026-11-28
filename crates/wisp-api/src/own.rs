//! The edge's own endpoints (health, status, events, cmd, openapi.json) and the conversion of a plugin
//! route's `Response` into an HTTP answer.
use std::sync::Arc;

use bytes::Bytes;
use serde_json::{Value, json};
use wisp_kernel::http::Response;
use wisp_kernel::{Caller, WireError};

use crate::edge::{API_VERSION, Edge, parse_query};
use crate::error::ApiError;
use crate::respond::{self, Answer, Asked};
use crate::router::{Own, Params};
use crate::sync::{command_answer, has_hello, not_mounted};
use crate::trust::header;

pub(crate) async fn run(
    edge: &Arc<Edge>,
    o: Own,
    caller: Caller,
    parts: &http::request::Parts,
    _params: &Params,
    body: Bytes,
    asked: &Asked,
) -> Result<Answer, ApiError> {
    match o {
        Own::Health => Ok(respond::json(200, &health(edge), asked, &[]).await),
        Own::Status => {
            let v = crate::status::answer(&edge.backend.status_parts(), edge.hub.looking(), &edge.activity, &edge.cfg.version);
            Ok(respond::json(200, &v, asked, &[]).await)
        }
        Own::OpenApi => Ok(respond::json(200, &crate::openapi::document(edge.table().list(), &edge.cfg.version), asked, &[]).await),
        Own::Events => events(edge, caller, parts),
        Own::EventsPost => {
            let m = edge.hub.mounted().ok_or_else(not_mounted)?;
            let client = header(&parts.headers, "x-wisp-client").unwrap_or("").to_owned();
            if writes(edge, &body) {
                edge.activity.touch(crate::edge::now_ms());
            }
            m.message(caller, client, body).await?;
            Ok(respond::plain_json(200, &json!({"ok": true})))
        }
        Own::Cmd => Ok(cmd(edge, caller, &parts.headers, &body).await),
    }
}

/// Whether a sync message is use: a mutate of a command that isn't read-only. A hello, sub, unsub or window is a
/// client looking, and something it can't read is counted (as every other write is).
fn writes(edge: &Edge, body: &[u8]) -> bool {
    let Ok(v) = serde_json::from_slice::<Value>(body) else { return true };
    match v.get("t").and_then(Value::as_str) {
        Some("hello" | "sub" | "unsub" | "window") => false,
        Some("mutate") => !v.get("cmd").and_then(Value::as_str).is_some_and(|c| edge.backend.read_only(c)),
        _ => true,
    }
}

fn health(edge: &Edge) -> Value {
    let url = edge.cfg.proxy.origin().map_or_else(|| format!("http://127.0.0.1:{}", edge.port()), str::to_owned);
    let mut v =
        json!({"ok": true, "name": "wisp", "version": edge.cfg.version, "apiVersion": API_VERSION, "pid": std::process::id(), "url": url});
    // which home this wisp serves, as a hash (health is open: never the path): a CLI for another home must not use it
    if let Some(id) = edge.ident.get() {
        v["instance"] = json!(id.instance);
        v["homeHash"] = json!(id.home_hash);
    }
    v
}

fn encoded(mut a: Answer, enc: respond::Encoding) -> Answer {
    let h = a.headers_mut();
    h.insert("content-encoding", http::HeaderValue::from_static(enc.name()));
    h.insert("vary", http::HeaderValue::from_static("accept-encoding"));
    a
}

fn sse(rx: tokio::sync::mpsc::Receiver<Bytes>) -> Answer {
    http::Response::builder()
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        // a proxy in between (Caddy, nginx) must pass frames on as they come
        .header("x-accel-buffering", "no")
        .body(respond::channel(rx))
        .unwrap_or_else(|_| http::Response::new(respond::empty()))
}

fn events(edge: &Edge, caller: Caller, parts: &http::request::Parts) -> Result<Answer, ApiError> {
    let query = parse_query(parts.uri.query());
    if has_hello(&query) {
        let m = edge.hub.mounted().ok_or_else(not_mounted)?;
        let last = header(&parts.headers, "last-event-id").map(str::to_owned);
        // compressed when the client takes it (every browser and webview does): the list's first look is megabytes
        let rx = m.events(caller, query, last)?;
        return Ok(match respond::encoding_for(&parts.headers) {
            Some(enc) => encoded(sse(crate::sse_enc::compressed(enc, rx)), enc),
            None => sse(rx),
        });
    }
    // v2 only (ADR-0031): a stream starts with its hello
    Err(WireError::invalid("GET /api/events takes the sync hello (proto_min, proto_max, client_id, topics…)").into())
}

async fn cmd(edge: &Edge, caller: Caller, h: &http::HeaderMap, body: &[u8]) -> Answer {
    let Some(key) = header(h, "idempotency-key").filter(|k| !k.is_empty() && k.len() <= 128) else {
        let (s, v) = command_answer(Err(WireError::invalid("Idempotency-Key header required")), 0);
        return respond::plain_json(s, &v);
    };
    let parsed: Result<(String, Value), WireError> = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| Some((v.get("cmd")?.as_str()?.to_owned(), v.get("args").cloned().unwrap_or(Value::Null))))
        .ok_or_else(|| WireError::invalid("body must be {cmd, args}"));
    let id = format!("{}|{}|{key}", caller.set, caller.session.as_deref().unwrap_or(""));
    // a read runs again on a retry: keeping its answer protects nothing and only grows the server (perf audit #1)
    let read = parsed.as_ref().is_ok_and(|(cmd, _)| edge.backend.read_only(cmd));
    if !read {
        edge.activity.touch(crate::edge::now_ms());
    }
    let backend = edge.backend.clone();
    let seq = || edge.hub.mounted().map_or(0, |m| m.head_seq());
    let run = async move {
        match parsed {
            Ok((cmd, args)) => {
                let span = tracing::info_span!("cmd", cmd = %cmd, set = %caller.set);
                let out = tracing::Instrument::instrument(backend.call(caller, &cmd, args), span).await;
                if let Err(e) = &out
                    && e.status() >= 500
                {
                    tracing::error!(cmd = %cmd, "{e}");
                }
                command_answer(out, seq())
            }
            Err(e) => command_answer(Err(e), 0),
        }
    };
    let (status, v) = if read { run.await } else { edge.idem.once(id, run).await };
    respond::plain_json(status, &v)
}

pub(crate) async fn convert(edge: &Edge, r: Response, asked: &Asked) -> Answer {
    match r {
        Response::Json { status, body } => respond::json(status, &body, asked, &[]).await,
        Response::Prepared { status, body } => respond::prepared(&edge.prepared, status, &body, asked).await,
        Response::Bytes { status, content_type, body, headers } => respond::text(status, &content_type, body, &headers),
        Response::Text { status, content_type, body } => respond::text(status, &content_type, body, &[]),
        Response::Redirect { status, location } => respond::redirect(status, &location),
        Response::Stream { content_type, mut body } => {
            let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
            tokio::spawn(async move {
                while let Some(chunk) = body.recv().await {
                    if tx.send(Bytes::from(chunk)).await.is_err() {
                        return;
                    }
                }
            });
            let mut a = sse(rx);
            if let Ok(v) = http::HeaderValue::from_str(&content_type) {
                a.headers_mut().insert("content-type", v);
            }
            a
        }
    }
}
