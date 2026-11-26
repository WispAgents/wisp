//! One supervised Cua Driver (`cua-driver mcp`): started on the first tool call, spoken to over its stdio,
//! started again on the next call when it died. It belongs to the plugin's scope, so switching wisp.computer
//! off (or wisp stopping) ends it and everything it started.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::oneshot;
use wisp_computer_core::client::{Client, Event};
use wisp_kernel::{Ctx, Result, WireError};
use wisp_proc::{ChildHandle, Env, Proc, SpawnOpts};

/// How long the driver may take to start (on a box `wisp-computer` brings the screen and the browser up first).
const START: Duration = Duration::from_secs(30);
/// How long one tool call may take (`run_steps` and `run_script` batch many actions).
pub const CALL: Duration = Duration::from_secs(300);

type Pending = HashMap<u64, oneshot::Sender<std::result::Result<Value, Value>>>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct Driver {
    handle: ChildHandle,
    client: Mutex<Client>,
    pending: Arc<Mutex<Pending>>,
    /// what its `initialize` said to do with its tools
    pub instructions: String,
    /// its own `tools/list`
    pub tools: Vec<Value>,
}

/// The env the driver gets on top of wisp's: no product telemetry (wisp doesn't send the user's activity anywhere
/// they didn't choose; `cua-driver telemetry disable` writes the same choice to its config) and no update check
/// (wisp pins the version it was tested with: a newer driver is wisp's update, not the driver's).
pub fn env() -> Env {
    Env::default()
        .with("CUA_DRIVER_RS_TELEMETRY_ENABLED", "0")
        .with("CUA_TELEMETRY_ENABLED", "0")
        .with("CUA_DRIVER_RS_UPDATE_CHECK", "false")
}

impl Driver {
    pub fn running(&self) -> bool {
        self.handle.running()
    }

    /// Starts `bin <args>` (`mcp`), initializes it and reads its tools.
    pub async fn start(ctx: &Ctx, proc: &Proc, bin: &str, args: &[String]) -> Result<Arc<Self>> {
        let opts = SpawnOpts { env: env(), scope: Some(ctx.scope().clone()), ..SpawnOpts::default() };
        let child = proc.spawn(bin, args, opts).map_err(|e| WireError::unavailable(format!("Cua Driver didn't start: {e}")))?;
        let (handle, mut rx) = child.split();
        let pending: Arc<Mutex<Pending>> = Arc::default();
        let mut d =
            Self { handle, client: Mutex::new(Client::new()), pending: pending.clone(), instructions: String::new(), tools: Vec::new() };
        let writer = d.handle.clone();
        ctx.spawn(async move {
            let mut client = Client::new();
            while let Some(o) = rx.recv().await {
                if o.stream != wisp_proc::Stream::Stdout {
                    // its logs (a missing accessibility bus, a refused permission): kept for `wisp logs`, never parsed
                    tracing::debug!(target: "wisp.computer", "{}", String::from_utf8_lossy(&o.data).trim_end());
                    continue;
                }
                for e in client.feed(&o.data) {
                    match e {
                        Event::Reply { id, outcome } => {
                            if let Some(tx) = lock(&pending).remove(&id) {
                                let _ = tx.send(outcome);
                            }
                        }
                        Event::Send(b) => {
                            let _ = writer.write(b);
                        }
                    }
                }
            }
            // it ended: every call still waiting hears so now, not at its timeout
            for (_, tx) in lock(&pending).drain() {
                let _ = tx.send(Err(json!({ "code": -32000, "message": "Cua Driver stopped" })));
            }
        });
        let init = d
            .ask_raw(Client::initialize, START)
            .await
            .map_err(|e| WireError::unavailable(format!("Cua Driver didn't start: {}", words(&e))))?;
        let _ = d.handle.write(Client::initialized());
        d.instructions = init.get("instructions").and_then(Value::as_str).unwrap_or_default().to_owned();
        let list = d.ask_raw(|c| c.request("tools/list", &json!({})), START).await.map_err(|e| WireError::unavailable(words(&e)))?;
        d.tools = list.get("tools").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(Arc::new(d))
    }

    async fn ask_raw(&self, make: impl FnOnce(&mut Client) -> (u64, Vec<u8>), wait: Duration) -> std::result::Result<Value, Value> {
        let (id, bytes) = make(&mut lock(&self.client));
        let (tx, rx) = oneshot::channel();
        lock(&self.pending).insert(id, tx);
        if self.handle.write(bytes).is_err() {
            lock(&self.pending).remove(&id);
            return Err(json!({ "code": -32000, "message": "Cua Driver isn't running" }));
        }
        if let Ok(Ok(outcome)) = tokio::time::timeout(wait, rx).await {
            outcome
        } else {
            lock(&self.pending).remove(&id);
            Err(json!({ "code": -32001, "message": format!("Cua Driver didn't answer in {} s", wait.as_secs()) }))
        }
    }

    /// `tools/call`: the driver's result as it is, or its JSON-RPC error.
    pub async fn call(&self, name: &str, args: &Value) -> std::result::Result<Value, Value> {
        self.ask_raw(|c| c.request("tools/call", &json!({ "name": name, "arguments": args })), CALL).await
    }
}

/// A JSON-RPC error's message.
pub fn words(e: &Value) -> String {
    e.get("message").and_then(Value::as_str).map_or_else(|| e.to_string(), str::to_owned)
}
