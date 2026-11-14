//! `runtime.agents`: the harnesses as Settings → Agents shows them, each detected now.

use futures::StreamExt;
use wisp_kernel::BoxFuture;
use wisp_runtime_api::commands::{AgentStatus, AgentsArgs};
use wisp_runtime_api::harness::HarnessFound;

use crate::runtime::Runtime;
use crate::sys::ProcSys;

/// How many harnesses detect at once (each runs its few probes at once too): every one wisp ships, with room for a
/// user's, without a machine with dozens of harness plugins spawning them all at once.
const DETECTS_AT_ONCE: usize = 12;

impl Runtime {
    /// `runtime.agents` (v1 adapters/index.js `list`): every harness, detected now. All at once: each detect spawns
    /// its CLI a few times (`--version`, `models`, `--help`), and one after another the answer waited for the sum of
    /// every probe, not the slowest harness (`wisp_available`, `wisp_agents` and Settings → Agents all ask this).
    /// Each probe runs once while its CLI is the same file (`probes.rs`): a second ask answers at once. `fresh`
    /// (Re-check): `all` (or empty) probes every harness again, a harness id that one.
    pub async fn agents(&self, a: &AgentsArgs) -> Vec<AgentStatus> {
        let platform = match std::env::consts::OS {
            "macos" => "darwin",
            "windows" => "win32",
            other => other,
        };
        let harnesses = self.host.harnesses();
        let detects: Vec<BoxFuture<'_, HarnessFound>> = harnesses
            .iter()
            .map(|h| -> BoxFuture<'_, HarnessFound> {
                let fresh = a.fresh.as_deref().is_some_and(|f| f.is_empty() || f == "all" || f == h.info().id);
                let sys = ProcSys::probing(self.proc.clone(), self.probes.clone(), fresh);
                let h = h.clone();
                Box::pin(async move { h.detect(&sys).await })
            })
            .collect();
        let found: Vec<HarnessFound> = futures::stream::iter(detects).buffered(DETECTS_AT_ONCE).collect().await;
        harnesses
            .iter()
            .zip(found)
            .map(|(h, found)| {
                let info = h.info().clone();
                if let Some(v) = &found.version {
                    crate::setup::note_version(&format!("harness:{}", info.id), v);
                }
                AgentStatus {
                    id: info.id,
                    name: info.name,
                    available: found.available,
                    setup: if found.available { None } else { h.setup(platform) },
                    version: found.version,
                    error: found.error,
                    models: found.models.unwrap_or(info.models),
                    efforts: found.efforts.unwrap_or(info.efforts),
                    delegates: info.ask_tool.is_some(),
                    signed_in: found.signed_in,
                }
            })
            .collect()
    }
}
