//! `runtime.agents`: the harnesses as Settings → Agents shows them, each detected now; and the `harnesses` topic
//! (ADR-0066) that the runner picker and Settings → Agents watch, filled as each harness's detection settles.
//!
//! A detection round writes every known harness's row at once (`checking`, with what was known: its declared
//! models, or the last answer), then each harness's row again the moment its own detect lands. A CLI that hangs
//! holds back only its own row, for its 15 s; the others are on screen in milliseconds.
//!
//! - **no flash**: a harness this process already settled stays as it was while a plain round runs (its row
//!   changes once, in place, when the answer lands); a Re-check (`fresh`) for it, or its first round since wisp
//!   started (a row from the last run may be stale), shows `checking`;
//! - **newest wins**: two rounds at once (Settings' Re-check while the picker's ask still waits on a hung CLI):
//!   a round's answer never overwrites a row a round that started after it wrote (its `checking` or its answer);
//! - **never stranded**: a round runs on its own task (`Host::spawn`), so a caller that hangs up (an MCP client gone,
//!   a closed page) can't leave rows `checking`: the round still lands every answer (a hung CLI ends at its timeout,
//!   its probe's group killed then: `serve_orphans` `a_probe_whose_caller_hung_up_ends_at_its_time_limit`);
//! - **gone is gone**: a harness no longer registered (its plugin switched off) loses its row.
//!
//! `runtime.agents` still answers the whole list, for the CLI and MCP callers; its rows land on the topic as
//! they settle, so its UI callers don't wait on it.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::StreamExt;
use serde_json::{Value, json};
use wisp_kernel::BoxFuture;
use wisp_runtime_api::Harness;
use wisp_runtime_api::commands::{AgentStatus, AgentsArgs, DefaultModelArgs, HarnessCheck, HarnessRow};
use wisp_runtime_api::harness::HarnessFound;
use wisp_store::{Query, Row};

use crate::runtime::Runtime;
use crate::schema::HARNESS;
use crate::sys::ProcSys;

/// How many harnesses detect at once (each runs its few probes at once too): every one wisp ships, with room for a
/// user's, without a machine with dozens of harness plugins spawning them all at once.
const DETECTS_AT_ONCE: usize = 12;

/// Each harness's row, as this process wrote it: held across a row's write, so two rounds' writes never land out of
/// order.
#[derive(Debug, Default)]
pub struct Rounds {
    next: AtomicU64,
    rows: tokio::sync::Mutex<HashMap<String, Wrote>>,
}

#[derive(Debug, Clone, Default)]
struct Wrote {
    /// the newest round that wrote the row (`checking` or an answer): an older round's answer doesn't land
    round: u64,
    /// an answer was written this process (a plain round leaves it on screen instead of `checking`)
    settled: bool,
    /// the row as this process last stored it, but its `at`: a write that would change nothing else is skipped
    /// (ADR-0084: a start's four rounds wrote every row six times, four of them for its timestamp alone)
    stored: Option<Row>,
}

/// A harness row but its `at`, what tells two writes apart.
fn bare(row: &Row) -> Row {
    let mut r = row.clone();
    r.remove("at");
    r
}

/// What a declared `harnesses:` entry changes in an answer (`kinds::declared`); applied to each harness as it lands,
/// so the topic and the answer agree.
pub type Adjust = Arc<dyn Fn(&mut AgentStatus) + Send + Sync>;

fn platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// A harness's status from its detect.
fn status_of(h: &dyn Harness, found: HarnessFound) -> AgentStatus {
    let info = h.info().clone();
    AgentStatus {
        id: info.id,
        name: info.name,
        available: found.available,
        setup: if found.available { None } else { h.setup(platform()) },
        version: found.version,
        error: found.error,
        models: found.models.unwrap_or(info.models),
        efforts: found.efforts.unwrap_or(info.efforts),
        delegates: info.ask_tool.is_some(),
        signed_in: found.signed_in,
        bin: found.bin,
    }
}

/// What a harness is known as before its detect answers: its declared name, models and thinking levels.
fn declared_status(h: &dyn Harness) -> AgentStatus {
    let info = h.info().clone();
    AgentStatus {
        id: info.id,
        name: info.name,
        available: false,
        setup: None,
        version: None,
        error: None,
        models: info.models,
        efforts: info.efforts,
        delegates: info.ask_tool.is_some(),
        signed_in: None,
        bin: None,
    }
}

/// Installed (signed in or not) or missing.
pub fn check_of(a: &AgentStatus) -> HarnessCheck {
    if a.available || a.signed_in == Some(false) { HarnessCheck::Installed } else { HarnessCheck::Missing }
}

fn row_of(r: &HarnessRow) -> Row {
    let mut row = Row::new();
    row.insert("id".into(), json!(r.id));
    row.insert("ord".into(), json!(r.ord));
    row.insert("state".into(), serde_json::to_value(r.state).unwrap_or(Value::Null));
    row.insert("at".into(), json!(r.at));
    row.insert("agent".into(), serde_json::to_value(&r.agent).unwrap_or(Value::Null));
    row
}

impl Runtime {
    /// `runtime.agents` (v1 adapters/index.js `list`): every harness, detected now. All at once: each detect spawns
    /// its CLI a few times (`--version`, `models`, `--help`), and one after another the answer waited for the sum of
    /// every probe, not the slowest harness (`wisp_available`, `wisp_agents` and Settings → Agents all ask this).
    /// Each probe runs once while its CLI is the same file (`probes.rs`): a second ask answers at once. `fresh`
    /// (Re-check): `all` (or empty) probes every harness again, a harness id that one.
    pub async fn agents(self: &Arc<Self>, a: &AgentsArgs) -> Vec<AgentStatus> {
        self.agents_as(a, Arc::new(|_| {})).await
    }

    /// `runtime.defaultModel`: what `agent`'s Default runs in `cwd`, as the harness reads its own config there (a
    /// project that pins a model, else this computer's): its files only, never a run. None: it can't tell, or there's
    /// no such harness.
    pub async fn default_model(self: &Arc<Self>, a: &DefaultModelArgs) -> Option<String> {
        let h = self.harness(&a.agent).ok()?;
        let sys = ProcSys::probing(self.proc.clone(), self.probes.clone(), false);
        h.default_model(&sys, a.cwd.as_deref()).await
    }

    /// [`Runtime::agents`], each answer changed by `adjust` (what `harnesses:` declares) before it's written to the
    /// `harnesses` topic and returned. The round runs on its own task: dropped, the caller only stops waiting.
    pub async fn agents_as(self: &Arc<Self>, a: &AgentsArgs, adjust: Adjust) -> Vec<AgentStatus> {
        let (tx, rx) = futures::channel::oneshot::channel();
        let (me, a) = (self.clone(), a.clone());
        self.host.spawn(Box::pin(async move {
            let _ = tx.send(me.round(&a, adjust).await);
        }));
        // the plugin's scope ended (switched off) before the round did: nothing to answer
        rx.await.unwrap_or_default()
    }

    /// One detection round: every row `checking` (as due), then each harness's answer as it lands.
    async fn round(&self, a: &AgentsArgs, adjust: Adjust) -> Vec<AgentStatus> {
        let harnesses = self.host.harnesses();
        let round = self.harness_rounds.next.fetch_add(1, Ordering::SeqCst) + 1;
        let fresh_for = |id: &str| a.fresh.as_deref().is_some_and(|f| f.is_empty() || f == "all" || f == id);
        self.harnesses_checking(&harnesses, round, &fresh_for).await;
        let detects: Vec<BoxFuture<'_, AgentStatus>> = harnesses
            .iter()
            .enumerate()
            .map(|(ord, h)| -> BoxFuture<'_, AgentStatus> {
                let sys = ProcSys::probing(self.proc.clone(), self.probes.clone(), fresh_for(&h.info().id));
                let (h, adjust) = (h.clone(), adjust.clone());
                Box::pin(async move {
                    let found = h.detect(&sys).await;
                    let mut status = status_of(h.as_ref(), found);
                    adjust(&mut status);
                    if let Some(v) = &status.version {
                        crate::setup::note_version(&format!("harness:{}", status.id), v);
                    }
                    self.harness_settled(round, u32::try_from(ord).unwrap_or(u32::MAX), &status).await;
                    status
                })
            })
            .collect();
        futures::stream::iter(detects).buffered(DETECTS_AT_ONCE).collect().await
    }

    /// The round's first write: every harness's row, `checking` unless this process settled it and it isn't asked
    /// fresh; a row of a harness that's gone is deleted.
    async fn harnesses_checking(&self, harnesses: &[Arc<dyn Harness>], round: u64, fresh_for: &(dyn Fn(&str) -> bool + Sync)) {
        let mut wrote = self.harness_rounds.rows.lock().await;
        let kept = self.harness_rows().await;
        let now = self.clock.now_ms();
        let mut rows = Vec::new();
        for (ord, h) in harnesses.iter().enumerate() {
            let id = h.info().id.clone();
            let w = wrote.get(&id).cloned().unwrap_or_default();
            if w.round > round || (w.settled && !fresh_for(&id)) {
                continue;
            }
            let agent = kept.get(&id).cloned().flatten().unwrap_or_else(|| declared_status(h.as_ref()));
            let row = row_of(&HarnessRow {
                id: id.clone(),
                ord: u32::try_from(ord).unwrap_or(u32::MAX),
                state: HarnessCheck::Checking,
                agent,
                at: now,
            });
            let same = w.stored.as_ref().is_some_and(|had| *had == bare(&row));
            wrote.insert(id.clone(), Wrote { round, settled: w.settled, stored: w.stored });
            if !same {
                rows.push((id, row));
            }
        }
        let gone: Vec<String> = kept.into_keys().filter(|id| !harnesses.iter().any(|h| h.info().id == *id)).collect();
        for id in &gone {
            wrote.remove(id);
        }
        if rows.is_empty() && gone.is_empty() {
            return;
        }
        let stored: Vec<(String, Row)> = rows.iter().map(|(id, row)| (id.clone(), bare(row))).collect();
        let written = self
            .repo
            .store
            .write(move |w| {
                for (_, row) in rows {
                    w.upsert(HARNESS.kind, row)?;
                }
                for id in gone {
                    w.del(HARNESS.kind, id.as_str())?;
                }
                Ok(())
            })
            .await;
        if let Err(err) = &written {
            tracing::warn!(error = %err, "runtime: the harnesses topic's checking rows");
        }
        // what the store holds now; after a failed write, unknown (the next round writes again)
        for (id, row) in stored {
            if let Some(w) = wrote.get_mut(&id) {
                w.stored = written.is_ok().then_some(row);
            }
        }
    }

    /// The topic's rows by id, each with what its `agent` says. By the row's id: one whose `agent` no longer reads (an
    /// older shape) is still deleted when its harness goes.
    async fn harness_rows(&self) -> HashMap<String, Option<AgentStatus>> {
        let read = self.repo.store.read_async(|r| {
            Ok(r.query(HARNESS.kind, &Query::new())?
                .into_iter()
                .filter_map(|row| {
                    let id = row.get("id").and_then(Value::as_str)?.to_owned();
                    Some((id, row.get("agent").cloned().and_then(|v| serde_json::from_value::<AgentStatus>(v).ok())))
                })
                .collect())
        });
        read.await.unwrap_or_else(|err| {
            tracing::warn!(error = %err, "runtime: the harnesses topic's rows");
            HashMap::new()
        })
    }

    /// One harness's answer, on the topic: unless a later round already wrote its row.
    async fn harness_settled(&self, round: u64, ord: u32, status: &AgentStatus) {
        let mut wrote = self.harness_rounds.rows.lock().await;
        let w = wrote.get(&status.id).cloned().unwrap_or_default();
        if w.round > round {
            return;
        }
        let row =
            row_of(&HarnessRow { id: status.id.clone(), ord, state: check_of(status), agent: status.clone(), at: self.clock.now_ms() });
        let stored = bare(&row);
        if w.stored.as_ref() == Some(&stored) {
            // the same answer the row already holds: only its `at` would change
            wrote.insert(status.id.clone(), Wrote { round, settled: true, stored: w.stored });
            return;
        }
        let written = self.repo.store.write(move |w| w.upsert(HARNESS.kind, row).map(|_| ())).await;
        if let Err(err) = &written {
            tracing::warn!(error = %err, harness = %status.id, "runtime: a harness's row");
        }
        wrote.insert(status.id.clone(), Wrote { round, settled: true, stored: written.is_ok().then_some(stored) });
    }
}
