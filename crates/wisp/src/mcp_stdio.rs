//! `wisp mcp`'s stdio loop, its I/O split out (`repo_cli::mcp` answers the messages). One request no longer holds up
//! the rest: a `wisp_wait` of four minutes and the editor's `ping` behind it were answered in that order, and the editor
//! took wisp for dead. `initialize` is answered before anything else is dispatched; every other request runs in its own
//! task; `notifications/cancelled` ends the one it names with no reply; one writer owns stdout, a whole line per
//! message, so two replies never interleave.
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::mpsc;

/// One message's answer (None: nothing to say, as for a notification).
pub type Reply = Pin<Box<dyn Future<Output = Option<Value>> + Send>>;
pub type Handler = Arc<dyn Fn(Value) -> Reply + Send + Sync>;

/// The JSON-RPC answer to a line that isn't JSON: said, never silence (the client would wait on it forever).
pub fn parse_error(e: &serde_json::Error) -> Value {
    json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("parse error: {e}") } })
}

/// Answers `lines` until they end and every request still running has answered (or been cancelled), each reply sent to
/// `out` whole.
pub async fn serve(mut lines: mpsc::Receiver<String>, out: mpsc::UnboundedSender<Value>, handle: Handler) {
    // request id (as JSON) → (which request it was, its abort handle): an id reused later is a new entry
    let mut running: HashMap<String, (u64, tokio::task::AbortHandle)> = HashMap::new();
    let (done_tx, mut done) = mpsc::unbounded_channel::<(String, u64)>();
    let mut seq = 0u64;
    let mut open = true;
    while open || !running.is_empty() {
        tokio::select! {
            line = lines.recv(), if open => {
                let Some(line) = line else {
                    open = false;
                    continue;
                };
                if line.trim().is_empty() {
                    continue;
                }
                let msg = match serde_json::from_str::<Value>(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = out.send(parse_error(&e));
                        continue;
                    }
                };
                let method = msg.get("method").and_then(Value::as_str).unwrap_or_default();
                if method == "notifications/cancelled" {
                    if let Some((_, h)) = msg.pointer("/params/requestId").and_then(|id| running.remove(&id.to_string())) {
                        h.abort();
                    }
                    continue;
                }
                let Some(id) = msg.get("id").map(Value::to_string) else { continue };
                if method == "initialize" {
                    // before anything else is dispatched: nothing races the handshake
                    if let Some(r) = handle(msg).await {
                        let _ = out.send(r);
                    }
                    continue;
                }
                seq += 1;
                let (h, out, done_tx, key, n) = (handle.clone(), out.clone(), done_tx.clone(), id.clone(), seq);
                let task = tokio::spawn(async move {
                    if let Some(r) = h(msg).await {
                        let _ = out.send(r);
                    }
                    let _ = done_tx.send((key, n));
                });
                running.insert(id, (seq, task.abort_handle()));
            }
            Some((key, n)) = done.recv() => {
                if running.get(&key).is_some_and(|(m, _)| *m == n) {
                    running.remove(&key);
                }
            }
        }
    }
}

/// Stdin's lines, read on a blocking thread (never a std read on a runtime worker), until it closes.
pub fn stdin_lines() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel(64);
    tokio::task::spawn_blocking(move || {
        use std::io::BufRead as _;
        for line in std::io::stdin().lock().lines().map_while(std::result::Result::ok) {
            if tx.blocking_send(line).is_err() {
                return;
            }
        }
    });
    rx
}

/// The one writer of stdout: each message a whole line, flushed. Done once every sender is gone.
pub fn stdout_writer() -> (mpsc::UnboundedSender<Value>, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
    let task = tokio::task::spawn_blocking(move || {
        use std::io::Write as _;
        let mut stdout = std::io::stdout().lock();
        while let Some(v) = rx.blocking_recv() {
            if writeln!(stdout, "{v}").and_then(|()| stdout.flush()).is_err() {
                return;
            }
        }
    });
    (tx, task)
}

/// A message to the running wisp that may be answered with a stream (MCP Events' `events/stream`, ADR-0146): each SSE
/// frame's message to `out` as it comes, until the stream ends (`None`: everything was said there), or the plain
/// answer. Dropping the future (`notifications/cancelled` aborts the request's task) closes the connection, which
/// ends the stream on the server.
pub async fn forward(c: &crate::client::Client, msg: &Value, cwd: &std::path::Path, out: &mpsc::UnboundedSender<Value>) -> Option<Value> {
    let failed = |e: String| json!({ "jsonrpc": "2.0", "id": msg.get("id").cloned().unwrap_or(Value::Null), "error": { "code": -32603, "message": e } });
    let mut r = match c.mcp_post(msg, cwd).await {
        Ok(r) => r,
        Err(e) => return Some(failed(e.to_string())),
    };
    let sse = r.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|t| t.contains("text/event-stream"));
    if !sse {
        return Some(r.json().await.unwrap_or_else(|e| failed(e.to_string())));
    }
    let mut buf = String::new();
    while let Ok(Some(chunk)) = r.chunk().await {
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = buf.find("\n\n") {
            let frame: String = buf.drain(..end + 2).collect();
            for data in frame.lines().filter_map(|l| l.strip_prefix("data:")) {
                if let Ok(v) = serde_json::from_str::<Value>(data.trim()) {
                    let _ = out.send(v);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::time::Duration;

    use super::*;

    /// `initialize` answers after a moment, `tools/call` (a `wisp_wait` that never settles) never, anything else at once.
    fn fake() -> Handler {
        Arc::new(|msg: Value| {
            Box::pin(async move {
                match msg["method"].as_str() {
                    Some("initialize") => tokio::time::sleep(Duration::from_millis(50)).await,
                    Some("tools/call") => std::future::pending::<()>().await,
                    _ => {}
                }
                Some(json!({ "jsonrpc": "2.0", "id": msg["id"], "result": {} }))
            }) as Reply
        })
    }

    fn line(v: &Value) -> String {
        v.to_string()
    }

    async fn next(out: &mut mpsc::UnboundedReceiver<Value>) -> Value {
        tokio::time::timeout(Duration::from_secs(5), out.recv()).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn a_ping_behind_a_wait_that_never_settles_is_answered_first() {
        let (lines_tx, lines) = mpsc::channel(8);
        let (out_tx, mut out) = mpsc::unbounded_channel();
        let loop_ = tokio::spawn(serve(lines, out_tx, fake()));
        let wait =
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "wisp_wait", "arguments": { "id": "t" } } });
        for m in
            [json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }), wait, json!({ "jsonrpc": "2.0", "id": 3, "method": "ping" })]
        {
            lines_tx.send(line(&m)).await.unwrap();
        }
        assert_eq!(next(&mut out).await["id"], 1, "initialize first, though it took longer");
        assert_eq!(next(&mut out).await["id"], 3, "the ping, not held up by the wait");
        lines_tx.send(line(&json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": 2 } }))).await.unwrap();
        drop(lines_tx);
        tokio::time::timeout(Duration::from_secs(5), loop_).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn a_cancelled_wait_ends_with_no_reply() {
        let (lines_tx, lines) = mpsc::channel(8);
        let (out_tx, mut out) = mpsc::unbounded_channel();
        let loop_ = tokio::spawn(serve(lines, out_tx, fake()));
        lines_tx
            .send(line(&json!({ "jsonrpc": "2.0", "id": "w", "method": "tools/call", "params": { "name": "wisp_wait" } })))
            .await
            .unwrap();
        lines_tx
            .send(line(&json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": "w" } })))
            .await
            .unwrap();
        drop(lines_tx);
        // the loop ends (nothing left running) and the wait said nothing
        tokio::time::timeout(Duration::from_secs(5), loop_).await.unwrap().unwrap();
        assert!(out.recv().await.is_none(), "no reply for a cancelled request");
    }

    #[tokio::test]
    async fn a_line_that_is_not_json_is_answered_with_a_parse_error() {
        let (lines_tx, lines) = mpsc::channel(8);
        let (out_tx, mut out) = mpsc::unbounded_channel();
        let loop_ = tokio::spawn(serve(lines, out_tx, fake()));
        lines_tx.send("{not json".to_owned()).await.unwrap();
        lines_tx.send(String::new()).await.unwrap();
        drop(lines_tx);
        let r = next(&mut out).await;
        assert_eq!((r["jsonrpc"].clone(), r["id"].clone(), r["error"]["code"].clone()), (json!("2.0"), Value::Null, json!(-32700)), "{r}");
        assert!(r["error"]["message"].as_str().is_some_and(|m| !m.is_empty()), "{r}");
        loop_.await.unwrap();
        assert!(out.recv().await.is_none(), "a blank line is no message");
    }
}
