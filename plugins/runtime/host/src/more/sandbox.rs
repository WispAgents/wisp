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
    /// followed up. → `<machine>|<task>`. A member whose sandbox or task there is gone (deleted in Settings, the grid
    /// dropped it) gets a new one, as a machine member does, and the new key replaces the stale one.
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
            match self.route(machine, "runtime.followUp", json!({ "id": id, "text": prompt, "queue": true })).await {
                Ok(f) => return Ok((format!("{machine}|{id}"), str_at(&f, "turnId"))),
                Err(e) if self.box_gone(machine, &e).await => {
                    tracing::info!(machine, error = %e.message, "runtime: a sandbox member's box is gone; a new one");
                }
                // still at work there (Conflict), asleep a moment: said as it is
                Err(e) => return Err(wire(e)),
            }
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

    /// A follow-up's refusal means the member's sandbox (or its task there) is gone: not found there, or a machine no
    /// longer in `machines.list`. One that's only unreachable a while (asleep, offline) is still the member's.
    async fn box_gone(&self, machine: &str, e: &wisp_kernel::WireError) -> bool {
        if e.code == wisp_kernel::Code::NotFound {
            return true;
        }
        if e.code != wisp_kernel::Code::Unavailable {
            return false;
        }
        match crate::calls::run::<wisp_machines_api::List>(&*self.host, wisp_machines_api::NoArgs {}).await {
            Ok(list) => !list.iter().any(|m| m.id == machine),
            Err(_) => false,
        }
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
