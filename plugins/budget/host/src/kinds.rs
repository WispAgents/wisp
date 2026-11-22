//! wisp.budget's resource kind (ADR-0028): Automations stop at, per harness, and spend caps (`budget:`
//! in a formula, one object). The stop lines are applied through `budget.setStop`; the spec is kept as
//! declared (`StoredKind`) so export gives back what was declared.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wisp_kernel::{BoxFuture, Call, Result, Setup};
use wisp_resources_api::{KindInfo, Op, OpKind, RESOURCE_KIND, Resource, ResourceKind, StoredKind, schema_of};

/// Where unattended work stops on a usage window, and what a task or workflow may spend.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BudgetSpec {
    /// off | 50 | 75 | 90: percent of a window where automations stop
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stop_at: String,
    /// Claude Code's own line (same | off | 50 | 75 | 90)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub claude: String,
    /// Codex's own line (same | off | 50 | 75 | 90)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub codex: String,
    /// Antigravity's own line (same | off | 50 | 75 | 90)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub antigravity: String,
    /// at most this many dollars per task
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_usd: Option<f64>,
    /// warn when a window is about to drain
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drain_watch: Option<bool>,
}

struct BudgetKind(Arc<StoredKind>);

/// Each stop line: its field in the spec, its key in the plugin's settings, and its default.
const LINES: [(&str, &str, &str); 4] = [
    ("stopAt", "autoStop", "off"),
    ("claude", "autoStopClaude", "same"),
    ("codex", "autoStopCodex", "same"),
    ("antigravity", "autoStopAntigravity", "same"),
];

impl ResourceKind for BudgetKind {
    fn info(&self) -> KindInfo {
        self.0.info()
    }
    /// What was declared, with the stop lines as they are now: one set in Usage (`budget.setStop`) is exported too
    /// (setup audit F13: `wisp export budget` and Setup → Make the formula left it out).
    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        Box::pin(async move {
            let mut out = self.0.read(call).await?;
            let Ok(store) = self.0.ctx.service::<wisp_store::Store>() else { return Ok(out) };
            let live = crate::store::settings(&store).await.unwrap_or_default();
            let at = out.iter().position(|r| r.name == "default");
            let mut spec = at.map_or_else(|| json!({}), |i| out[i].spec.clone());
            let Some(o) = spec.as_object_mut() else { return Ok(out) };
            for (field, key, default) in LINES {
                match live.get(key).and_then(Value::as_str) {
                    Some(v) if v != default => o.insert(field.to_owned(), json!(v)),
                    _ => o.remove(field),
                };
            }
            match (at, spec.as_object().is_some_and(serde_json::Map::is_empty)) {
                (Some(i), true) => {
                    out.remove(i);
                }
                (Some(i), false) => out[i].spec = spec,
                (None, false) => out.push(Resource::new(self.0.info.kind.clone(), "default", spec)),
                (None, true) => {}
            }
            Ok(out)
        })
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            for (field, key, reset) in LINES {
                let v = op.spec.get(field).and_then(Value::as_str).filter(|_| op.op != OpKind::Delete);
                let was = op.before.as_ref().and_then(|b| b.get(field)).is_some() || op.op == OpKind::Delete;
                if let Some(v) = v {
                    self.0.ctx.run_json(call, "budget.setStop", json!({ "key": key, "value": v })).await?;
                } else if was {
                    self.0.ctx.run_json(call, "budget.setStop", json!({ "key": key, "value": reset })).await?;
                }
            }
            self.0.apply(call, op).await
        })
    }
}

pub fn register(s: &mut Setup) {
    let info = KindInfo::new("budget", "budget", "wisp.budget", schema_of::<BudgetSpec>()).singleton().order(34);
    let k: Arc<dyn ResourceKind> = Arc::new(BudgetKind(Arc::new(StoredKind::new(info, s.ctx()))));
    s.contribute(&RESOURCE_KIND, k);
}
