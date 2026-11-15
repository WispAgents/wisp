//! Subagents whose place is a sandbox (ADR-0047): their task there, routed through `machines.route`; their report is
//! read back as any job elsewhere's (`machine.rs` `far_report`), with their changes brought into the lead's folder once.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_runtime_api::model::Session;

use super::str_at;
use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;

impl Runtime {
    /// A member whose place is a sandbox: its sandbox (made once, the lead's work in it), and its task there, new or
    /// followed up. → `<machine>|<task>`.
    pub(super) async fn sandbox_member(
        self: &Arc<Self>,
        lead: &Session,
        m: &Value,
        slot_name: &str,
        existing: Option<String>,
        prompt: &str,
    ) -> Result<(String, String)> {
        let wire = |e: wisp_kernel::WireError| RuntimeError::Invalid(e.message);
        // a capped lead's work stays on this computer, under its cap (crew refuses a member elsewhere the same way)
        if crate::capped::cap_of(lead).is_some() {
            return Err(RuntimeError::Wire(wisp_kernel::WireError::forbidden(wisp_kernel::caps::capped_refusal("start a sandbox"))));
        }
        if let Some((machine, id)) = existing.as_deref().and_then(|k| k.split_once('|')) {
            let f = self.route(machine, "runtime.followUp", json!({ "id": id, "text": prompt, "queue": true })).await.map_err(wire)?;
            return Ok((format!("{machine}|{id}"), str_at(&f, "turnId")));
        }
        let place = wisp_sandbox_api::PlaceArgs { owner: json!({ "lead": lead.id.0, "member": slot_name }), cwd: Some(lead.cwd.clone()) };
        let placed = crate::calls::run::<wisp_sandbox_api::Place>(&*self.host, place).await.map_err(wire)?;
        let machine = placed.machine;
        let mut args = json!({ "prompt": prompt, "cwd": placed.cwd, "mode": lead.mode, "title": format!("{slot_name} · {}", lead.title) });
        for k in ["agent", "model", "effort"] {
            let v = str_at(m, k);
            if !v.is_empty() {
                args[k] = json!(v);
            }
        }
        let made = self.route(&machine, "runtime.create", args).await.map_err(wire)?;
        Ok((format!("{machine}|{}", str_at(&made, "id")), str_at(&made, "turnId")))
    }

    /// Runtime command `cmd` on sandbox `machine` (`machines.route`, typed; the routed command stays by name: it is
    /// this plugin's own, run by the wisp over there).
    pub(super) async fn route(&self, machine: &str, cmd: &str, args: Value) -> wisp_kernel::Result<Value> {
        let a = wisp_machines_api::RouteArgs {
            id: Some(machine.to_owned()),
            cmd: cmd.to_owned(),
            args,
            mid: format!("dlg-{}", wisp_kernel::ids::uuid7()),
        };
        crate::calls::run::<wisp_machines_api::Route>(&*self.host, a).await
    }
}
