//! The `router` resource kind (ADR-0028): Auto's settings as one object (`router:` in a formula), read
//! from `router.view` and written with `router.set`. What the router learned stays data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wisp_kernel::{BoxFuture, Call, Ctx, Result};
use wisp_resources_api::{KindInfo, Op, OpKind, Resource, ResourceKind, schema_of};

/// How Auto picks a harness and model for a task.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouterSpec {
    /// what it optimises for (`balanced`, `cheap`, `best`, …)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// try models it knows less about now and then
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explore: Option<bool>,
    /// the harnesses it may pick (null: every one)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<Vec<String>>,
    /// harnesses or models it never picks
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Vec<String>>,
    /// ones it picks first when they fit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer: Option<Vec<String>>,
    /// learn from what you pick by hand
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub habits: Option<bool>,
    /// a failed run goes again on a stronger model
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalate: Option<bool>,
    /// hard tasks run twice and the better answer is kept
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best2: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scout: Option<Value>,
}

/// Auto's one switch (wisp.runtime's setting).
const AUTO_SETTING: &str = "runtime.auto";

/// What a plan naming `router:` says while Auto is off.
pub const AWAY: &str = "Auto is off (runtime.auto), so the router is put away: turn it on with wisp settings set runtime.auto true";

#[derive(Debug)]
pub struct RouterKind {
    pub ctx: Ctx,
}

impl ResourceKind for RouterKind {
    /// While Auto's one switch is off (`runtime.auto`, off until the owner reworks it: 2026-11-17) the kind is put away
    /// (setup audit B6, D2): no kinds list, `wisp export` (`export router` included), Setup, formula, validate or apply
    /// takes it, and a plan naming it says Auto is off. The code is kept: on, it's a kind as before.
    fn info(&self) -> KindInfo {
        let i = KindInfo::new("router", "router", wisp_router_api::PLUGIN_ID, schema_of::<RouterSpec>()).singleton().order(30);
        if self.ctx.setting(AUTO_SETTING) == Some(Value::Bool(true)) { i } else { i.away(AWAY) }
    }

    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        Box::pin(async move {
            let v = self.ctx.run_json(call, "router.view", json!({})).await?;
            let mut spec = v.get("settings").cloned().unwrap_or_else(|| json!({}));
            if let Some(o) = spec.as_object_mut() {
                o.insert("goal".into(), v.get("goal").cloned().unwrap_or(Value::Null));
                o.retain(|k, x| {
                    !x.is_null()
                        && ["goal", "explore", "allowed", "blocked", "prefer", "habits", "escalate", "best2", "scout"].contains(&k.as_str())
                });
            }
            Ok(vec![Resource::new("router", "default", spec)])
        })
    }

    fn apply<'a>(&'a self, call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            if op.op == OpKind::Delete {
                return Ok(());
            }
            self.ctx.run_json(call, "router.set", op.spec.clone()).await.map(|_| ())
        })
    }
}
