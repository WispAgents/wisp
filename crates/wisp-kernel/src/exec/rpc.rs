//! JSON-RPC 2.0 over a pair of byte streams, one message per line (the LSP/MCP model without headers).
//! Errors carry the `WireError` in `data`, so a refusal keeps its code across the process boundary.
//! Cheap requests (`emit`, `log`, registrations) are answered in arrival order, so an `emit` the child
//! sent before a command's result is buffered before that result is seen; long ones run as tasks.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{BoxFuture, Code, Result, WireError};

pub const METHOD_NOT_FOUND: i64 = -32601;
pub const PARSE_ERROR: i64 = -32700;
pub const SERVER_ERROR: i64 = -32000;

/// What a handler answers: now (in arrival order) or later (a task).
pub enum Reply {
    Now(Result<Value>),
    Later(BoxFuture<'static, Result<Value>>),
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Now(_) => "Now",
            Self::Later(_) => "Later",
        })
    }
}

pub type Handler = Arc<dyn Fn(&str, Value) -> Reply + Send + Sync>;

type Waiter = oneshot::Sender<Result<Value>>;

#[derive(Debug)]
pub struct Peer {
    out: mpsc::UnboundedSender<String>,
    pending: Arc<Mutex<HashMap<u64, Waiter>>>,
    next: AtomicU64,
    closed: Mutex<Option<String>>,
    stop: CancellationToken,
}

/// Takes a request's waiter out of `pending` when its future is dropped before the answer came (a timeout, a caller
/// gone), so a child that never answers doesn't grow the map for the life of the process.
struct Waiting {
    pending: Arc<Mutex<HashMap<u64, Waiter>>>,
    id: u64,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        if let Ok(mut p) = self.pending.lock() {
            p.remove(&self.id);
        }
    }
}

fn wire(e: &WireError) -> Value {
    serde_json::to_value(e).unwrap_or(Value::Null)
}

impl Peer {
    /// Starts the reader and writer tasks; both end with `stop` or when the stream closes.
    pub fn start<R, W>(input: R, output: W, handler: Handler, stop: CancellationToken) -> Arc<Self>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        let peer =
            Arc::new(Self { out: tx, pending: Arc::default(), next: AtomicU64::new(1), closed: Mutex::default(), stop: stop.clone() });
        tokio::spawn(write_loop(rx, output, stop.clone()));
        tokio::spawn(read_loop(peer.clone(), input, handler, stop));
        peer
    }

    pub fn closed(&self) -> Option<String> {
        self.closed.lock().ok().and_then(|c| c.clone())
    }

    fn send(&self, v: &Value) {
        let _ = self.out.send(v.to_string());
    }

    #[allow(clippy::needless_pass_by_value)] // params are moved into the message by `json!`
    pub fn request(&self, method: &str, params: Value) -> BoxFuture<'static, Result<Value>> {
        if let Some(why) = self.closed() {
            let m = format!("{method}: {why}");
            return Box::pin(async move { Err(WireError::unavailable(m)) });
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        if let Ok(mut p) = self.pending.lock() {
            p.insert(id, tx);
        }
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let (m, waiting) = (method.to_owned(), Waiting { pending: self.pending.clone(), id });
        Box::pin(async move {
            let _waiting = waiting;
            rx.await.unwrap_or_else(|_| Err(WireError::unavailable(format!("{m}: the connection closed"))))
        })
    }

    #[cfg(test)]
    pub(crate) fn pending_len(&self) -> usize {
        self.pending.lock().map(|p| p.len()).unwrap_or_default()
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn notify(&self, method: &str, params: Value) {
        if self.closed().is_none() {
            self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
        }
    }

    /// Fails every pending request and stops: the process is gone or going.
    pub fn close(&self, why: &str) {
        if let Ok(mut c) = self.closed.lock() {
            if c.is_some() {
                return;
            }
            *c = Some(why.to_owned());
        }
        let pending: Vec<Waiter> = self.pending.lock().map(|mut p| p.drain().map(|(_, w)| w).collect()).unwrap_or_default();
        for w in pending {
            let _ = w.send(Err(WireError::unavailable(why.to_owned())));
        }
        self.stop.cancel();
    }

    fn answer(&self, id: &Value, r: Result<Value>) {
        match r {
            Ok(v) => self.send(&json!({ "jsonrpc": "2.0", "id": id, "result": v })),
            Err(e) => {
                let code = if e.code == Code::NotFound && e.message.starts_with("no method") { METHOD_NOT_FOUND } else { SERVER_ERROR };
                self.send(&json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": e.message, "data": wire(&e) } }));
            }
        }
    }

    fn reply(&self, m: &Value) {
        let Some(id) = m.get("id").and_then(Value::as_u64) else {
            return;
        };
        let Some(w) = self.pending.lock().ok().and_then(|mut p| p.remove(&id)) else {
            return;
        };
        let r = match m.get("error") {
            None => Ok(m.get("result").cloned().unwrap_or(Value::Null)),
            Some(err) => Err(err
                .get("data")
                .and_then(|d| serde_json::from_value::<WireError>(d.clone()).ok())
                .unwrap_or_else(|| WireError::internal(err.get("message").and_then(Value::as_str).unwrap_or("error").to_owned()))),
        };
        let _ = w.send(r);
    }
}

async fn write_loop<W: AsyncWrite + Unpin>(mut rx: mpsc::UnboundedReceiver<String>, mut out: W, stop: CancellationToken) {
    loop {
        let line = tokio::select! {
            () = stop.cancelled() => return,
            l = rx.recv() => match l { Some(l) => l, None => return },
        };
        if out.write_all(line.as_bytes()).await.is_err() || out.write_all(b"\n").await.is_err() || out.flush().await.is_err() {
            return;
        }
    }
}

async fn read_loop<R: AsyncRead + Unpin>(peer: Arc<Peer>, input: R, handler: Handler, stop: CancellationToken) {
    let mut lines = BufReader::new(input).lines();
    loop {
        let line = tokio::select! {
            () = stop.cancelled() => break,
            l = lines.next_line() => match l { Ok(Some(l)) => l, _ => break },
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(m) = serde_json::from_str::<Value>(line) else {
            let head: String = line.chars().take(80).collect();
            peer.send(&json!({ "jsonrpc": "2.0", "id": null, "error": { "code": PARSE_ERROR, "message": format!("not JSON: {head}") } }));
            continue;
        };
        match m.get("method").and_then(Value::as_str) {
            Some(method) => {
                let id = m.get("id").filter(|v| !v.is_null()).cloned();
                let params = m.get("params").cloned().unwrap_or(Value::Null);
                match (handler(method, params), id) {
                    (Reply::Now(r), Some(id)) => peer.answer(&id, r),
                    (Reply::Later(f), Some(id)) => {
                        let p = peer.clone();
                        tokio::spawn(async move { p.answer(&id, f.await) });
                    }
                    (Reply::Later(f), None) => drop(tokio::spawn(f)),
                    (Reply::Now(_), None) => {}
                }
            }
            None => peer.reply(&m),
        }
    }
    peer.close("the other side closed the stream");
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A peer whose other side is the test: it reads what the peer sent and writes answers back.
    fn pair() -> (Arc<Peer>, BufReader<tokio::io::DuplexStream>, tokio::io::DuplexStream) {
        let (ours, theirs) = tokio::io::duplex(4096);
        let (to_peer, from_test) = tokio::io::duplex(4096);
        let handler: Handler = Arc::new(|_, _| Reply::Now(Ok(Value::Null)));
        let peer = Peer::start(from_test, ours, handler, CancellationToken::new());
        (peer, BufReader::new(theirs), to_peer)
    }

    #[tokio::test]
    async fn a_dropped_request_frees_its_waiter() {
        let (peer, _sent, _answers) = pair();
        let r = peer.request("intercept.run", json!({}));
        assert_eq!(peer.pending_len(), 1);
        // the hook's timeout: the future goes before any answer came
        assert!(tokio::time::timeout(std::time::Duration::from_millis(20), r).await.is_err());
        assert_eq!(peer.pending_len(), 0);
    }

    #[tokio::test]
    async fn an_answered_request_is_removed_once() {
        let (peer, mut sent, mut answers) = pair();
        let r = peer.request("command.run", json!({}));
        let mut line = String::new();
        sent.read_line(&mut line).await.unwrap();
        let id = serde_json::from_str::<Value>(&line).unwrap()["id"].clone();
        let reply = json!({ "jsonrpc": "2.0", "id": id, "result": 7 }).to_string();
        answers.write_all(format!("{reply}\n{reply}\n").as_bytes()).await.unwrap();
        assert_eq!(r.await.unwrap(), json!(7));
        assert_eq!(peer.pending_len(), 0);
        // the duplicate answer finds nothing, and a later request is still tracked until it settles
        let r2 = peer.request("command.run", json!({}));
        assert_eq!(peer.pending_len(), 1);
        drop(r2);
        assert_eq!(peer.pending_len(), 0);
    }
}
