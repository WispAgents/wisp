//! `connectors.call` (v1 connectors.js `mcpCall`): one tool of a remote connector, called by wisp itself over
//! streamable HTTP (`initialize`, `notifications/initialized`, `tools/call`, with the server's session), with
//! the connector's headers and its sign-in. Its token never leaves wisp. A JSON-RPC error or a tool's `isError`
//! comes back as the server's own words. A connector wisp starts itself (stdio) isn't called here yet. `request` is
//! the same for any method (MCP Events' `events/list` and `events/poll`, `events.rs`).
//!
//! Also `issues.connectors@1`: the connectors with their secrets revealed and whether each is signed in, for
//! the issue picker (Linear's API key, Jira's site and token, or a signed-in server's tools).

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_connectors_api::{CallArgs, CallOut, secrets_ns};
use wisp_connectors_core::rpc::{Sse, initialize};
use wisp_kernel::services::{NetRequest, NetResponse, NetStream, net, secrets};
use wisp_kernel::{BoxFuture, Call, Code, Ctx, Result, Setup, WireError};
use wisp_resources_api::StoredKind;

use crate::kinds::ConnectorSpec;
use crate::pane::Pane;

async fn specs(kind: &StoredKind) -> Vec<(String, ConnectorSpec)> {
    kind.specs().await.unwrap_or_default().into_iter().filter_map(|(n, v)| Some((n, serde_json::from_value(v).ok()?))).collect()
}

/// The stored OAuth token, when it signed in with the browser.
async fn token(ctx: &Ctx, id: &str) -> Option<String> {
    secrets(ctx).ok()?.get(&format!("{}:token", secrets_ns(id))).await.ok().flatten().filter(|t| !t.is_empty())
}

/// The JSON-RPC answer in a reply: the body, or the event of an SSE stream that carries an id.
fn answer(r: &NetResponse) -> Option<Value> {
    let text = if r.header("content-type").is_some_and(|t| t.contains("text/event-stream")) {
        Sse::default().push(&r.body).into_iter().map(|e| e.data).find(|d| d.contains("\"id\""))?
    } else {
        String::from_utf8_lossy(&r.body).into_owned()
    };
    serde_json::from_str(&text).ok()
}

/// What a JSON-RPC answer or a tool result says went wrong, in the server's words.
pub fn refusal_of(v: &Value) -> Option<String> {
    if let Some(e) = v.get("error") {
        return Some(e.get("message").and_then(Value::as_str).unwrap_or("It refused").chars().take(300).collect());
    }
    let r = v.get("result")?;
    if r.get("isError") == Some(&Value::Bool(true)) {
        let text: Vec<&str> = r
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|c| c.get("text").and_then(Value::as_str))
            .collect();
        let t = text.join(" ");
        return Some(if t.trim().is_empty() { "The tool failed".to_owned() } else { t.trim().chars().take(300).collect() });
    }
    None
}

async fn post(ctx: &Ctx, url: &str, headers: &[(String, String)], session: Option<&str>, body: &Value) -> Result<NetResponse> {
    let n = net(ctx)?;
    let mut h = headers.to_vec();
    if let Some(s) = session {
        h.push(("mcp-session-id".into(), s.to_owned()));
    }
    let r = n
        .fetch(NetRequest {
            method: "POST".into(),
            url: url.to_owned(),
            headers: h,
            body: body.to_string().into_bytes(),
            timeout_ms: 60_000,
        })
        .await?;
    match r.status {
        401 | 403 => Err(WireError::forbidden(format!("needs sign-in (HTTP {})", r.status))),
        s if s >= 400 => Err(WireError::unavailable(format!("HTTP {s}"))),
        _ => Ok(r),
    }
}

pub async fn call(ctx: &Ctx, pane: &Pane, a: &CallArgs) -> Result<CallOut> {
    let args = if a.args.is_null() { json!({}) } else { a.args.clone() };
    let v = request(ctx, pane, &a.id, "tools/call", json!({ "name": a.tool, "arguments": args }), false).await?;
    if let Some(why) = refusal_of(&v) {
        return Err(WireError::unavailable(why));
    }
    Ok(CallOut { result: v.get("result").cloned().unwrap_or(Value::Null) })
}

/// What a request got: the answer, or (a stream asked for, and the server answered with SSE) the open stream.
pub enum Got {
    Answer(Value),
    Stream(NetStream),
}

/// One JSON-RPC request to a remote connector, with its headers and sign-in: the whole answer (`result` or `error`),
/// for the caller to read. `modern`: a server that refuses `initialize` is asked again the `2026-07-28` way, the
/// version in `_meta` and the `MCP-Protocol-Version` header (MCP Events, ADR-0145).
pub async fn request(ctx: &Ctx, pane: &Pane, id: &str, method: &str, params: Value, modern: bool) -> Result<Value> {
    match send(ctx, pane, id, method, params, modern, false).await? {
        Got::Answer(v) => Ok(v),
        Got::Stream(_) => Err(WireError::unavailable(format!("{id} answered {method} with a stream"))),
    }
}

/// A long-lived request (`events/stream`): the open SSE stream, or the answer when the server refused it at once.
pub async fn open_stream(ctx: &Ctx, pane: &Pane, id: &str, method: &str, params: Value) -> Result<Got> {
    send(ctx, pane, id, method, params, true, true).await
}

async fn send(ctx: &Ctx, pane: &Pane, id: &str, method: &str, params: Value, modern: bool, stream: bool) -> Result<Got> {
    let Some((_, mut c)) = pane.specs().await.into_iter().find(|(n, _)| *n == id) else {
        return Err(WireError::not_found(format!("No connector called {id}")));
    };
    if c.url.is_empty() {
        return Err(WireError::invalid(format!("{id} runs on this computer: wisp calls only remote connectors' tools")));
    }
    for v in c.headers.values_mut() {
        *v = crate::kinds::reveal(ctx, id, v).await;
    }
    let mut headers: Vec<(String, String)> = c.headers.into_iter().collect();
    headers.push(("content-type".into(), "application/json".into()));
    headers.push(("accept".into(), "application/json, text/event-stream".into()));
    let ask = Ask { id, method, params: &params, modern, stream };
    // a sign-in's token goes only over https or to this computer (`bearer_ok`): elsewhere the call goes without it
    if c.oauth != Some(true) || !crate::kinds::bearer_ok(&c.url) {
        return exchange(ctx, &c.url, &headers, &ask).await;
    }
    // signed in: a token that ran out is refreshed first; a refused one is refreshed and tried once more, and a
    // second refusal (or a refresh that fails) leaves the row asking for a sign-in
    let t = pane.fresh_token(ctx, id, None).await?;
    match exchange(ctx, &c.url, &bearer(&headers, &t), &ask).await {
        Err(e) if e.code == Code::Forbidden => {}
        done => return done,
    }
    let t = pane.fresh_token(ctx, id, Some(&t)).await?;
    match exchange(ctx, &c.url, &bearer(&headers, &t), &ask).await {
        Err(e) if e.code == Code::Forbidden => Err(pane.needs_signin(ctx, id, &e.message).await),
        done => done,
    }
}

/// The headers with the sign-in's bearer in place of any `Authorization` the connector had.
fn bearer(headers: &[(String, String)], token: &str) -> Vec<(String, String)> {
    let mut h: Vec<(String, String)> = headers.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("authorization")).cloned().collect();
    h.push(("Authorization".into(), format!("Bearer {token}")));
    h
}

struct Ask<'a> {
    id: &'a str,
    method: &'a str,
    params: &'a Value,
    modern: bool,
    /// the last request is held open (SSE)
    stream: bool,
}

/// How long one stream may stay open before it's opened again with its cursor (a proxy's or a server's own limit
/// comes first; the heartbeat says when it died).
pub const STREAM_MS: u64 = 30 * 60_000;

/// The request itself, once the handshake is done: answered, or held open.
async fn last(ctx: &Ctx, url: &str, headers: &[(String, String)], session: Option<&str>, a: &Ask<'_>, params: &Value) -> Result<Got> {
    let body = json!({ "jsonrpc": "2.0", "id": 2, "method": a.method, "params": params });
    if !a.stream {
        let r = post(ctx, url, headers, session, &body).await?;
        return answer(&r).map(Got::Answer).ok_or_else(|| WireError::unavailable(format!("{} didn't answer {}", a.id, a.method)));
    }
    let mut h = headers.to_vec();
    if let Some(s) = session {
        h.push(("mcp-session-id".into(), s.to_owned()));
    }
    let req =
        NetRequest { method: "POST".into(), url: url.to_owned(), headers: h, body: body.to_string().into_bytes(), timeout_ms: STREAM_MS };
    let r = net(ctx)?.stream(req).await?;
    match r.status {
        401 | 403 => return Err(WireError::forbidden(format!("needs sign-in (HTTP {})", r.status))),
        st if st >= 400 => return Err(WireError::unavailable(format!("HTTP {st}"))),
        _ => {}
    }
    if r.header("content-type").is_some_and(|t| t.contains("text/event-stream")) {
        return Ok(Got::Stream(r));
    }
    // refused at once (a JSON-RPC error as a plain answer): read it
    let bytes: Vec<u8> = futures::StreamExt::collect::<Vec<_>>(r.body).await.into_iter().filter_map(Result::ok).flatten().collect();
    serde_json::from_slice(&bytes).map(Got::Answer).map_err(|_| WireError::unavailable(format!("{} didn't answer {}", a.id, a.method)))
}

/// One request over streamable HTTP: `initialize`, `notifications/initialized`, then the method (in the server's
/// session).
async fn exchange(ctx: &Ctx, url: &str, headers: &[(String, String)], a: &Ask<'_>) -> Result<Got> {
    let init: Value = serde_json::from_str(&initialize(env!("CARGO_PKG_VERSION"))).unwrap_or_default();
    let r = match post(ctx, url, headers, None, &init).await {
        Ok(r) => r,
        // a modern-only server answers a legacy opening with a 4xx: ask it the modern way
        Err(e) if a.modern && e.code == Code::Unavailable => return modern(ctx, url, headers, a).await,
        Err(e) => return Err(e),
    };
    let session = r.header("mcp-session-id").map(str::to_owned);
    if let Some(why) = answer(&r).as_ref().and_then(refusal_of) {
        if a.modern {
            return modern(ctx, url, headers, a).await;
        }
        return Err(WireError::unavailable(why));
    }
    let _ = post(ctx, url, headers, session.as_deref(), &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).await;
    last(ctx, url, headers, session.as_deref(), a, a.params).await
}

/// The `2026-07-28` way: no handshake, the version in the request's `_meta` and in `MCP-Protocol-Version`.
async fn modern(ctx: &Ctx, url: &str, headers: &[(String, String)], a: &Ask<'_>) -> Result<Got> {
    let mut h = headers.to_vec();
    h.push(("mcp-protocol-version".into(), wisp_connectors_core::events::MODERN.into()));
    let mut params = a.params.clone();
    if let Some(o) = params.as_object_mut() {
        let meta = o.entry("_meta").or_insert_with(|| json!({}));
        if let (Some(m), Value::Object(extra)) = (meta.as_object_mut(), wisp_connectors_core::events::meta(Some(env!("CARGO_PKG_VERSION"))))
        {
            m.extend(extra);
        }
    }
    last(ctx, url, &h, None, a, &params).await
}

/// The connectors as the issue picker reads them.
struct ForIssues {
    kind: Arc<StoredKind>,
    ctx: Ctx,
}

impl wisp_issues_api::Connectors for ForIssues {
    fn connectors(&self, _call: &Call) -> BoxFuture<'static, Result<Vec<wisp_issues_api::Connector>>> {
        let (kind, ctx) = (self.kind.clone(), self.ctx.clone());
        Box::pin(async move {
            let mut out = Vec::new();
            for (id, c) in specs(&kind).await {
                let mut env = Vec::new();
                for (k, v) in &c.env {
                    env.push((k.clone(), crate::kinds::reveal(&ctx, &id, v).await));
                }
                let mut headers = Vec::new();
                for (k, v) in &c.headers {
                    headers.push((k.clone(), crate::kinds::reveal(&ctx, &id, v).await));
                }
                let signed_in = c.oauth == Some(true) && token(&ctx, &id).await.is_some();
                out.push(wisp_issues_api::Connector {
                    name: id.clone(),
                    id,
                    url: c.url,
                    command: c.command,
                    args: c.args,
                    enabled: c.enabled != Some(false),
                    env,
                    headers,
                    signed_in,
                });
            }
            Ok(out)
        })
    }
}

pub(crate) fn register(s: &mut Setup, kind: Arc<StoredKind>, pane: Arc<Pane>) {
    let p = pane.clone();
    s.command::<wisp_connectors_api::Call, _, _>(move |ctx, _call, a| {
        let pane = p.clone();
        async move { call(&ctx, &pane, &a).await }
    });
    crate::events::register(s, pane);
    let issues: Arc<dyn wisp_issues_api::Connectors> = Arc::new(ForIssues { kind, ctx: s.ctx() });
    s.contribute(&wisp_issues_api::CONNECTORS, issues);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_servers_refusal_is_said_in_its_own_words() {
        assert_eq!(refusal_of(&json!({ "error": { "message": "Unknown tool" } })).as_deref(), Some("Unknown tool"));
        let failed = json!({ "result": { "isError": true, "content": [{ "type": "text", "text": "Not authorized" }] } });
        assert_eq!(refusal_of(&failed).as_deref(), Some("Not authorized"));
        assert_eq!(refusal_of(&json!({ "result": { "content": [] } })), None);
    }
}
