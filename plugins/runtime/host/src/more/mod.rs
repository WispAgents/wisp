//! The rest of v1 runner.js's surface, over the same session, turn and queue primitives: an agent's
//! questions (`ask`, `answer`, `pollAnswer`) and live approvals, blocked actions allowed for one
//! follow-up, /btw side questions, compacting, subagents (`delegate`, `results`), forking, `quick`
//! side runs, a signed-out turn's sign-in and rerun, a turn billed past its plan, and the harnesses
//! as Settings → Agents shows them. One module per surface; this one holds the helpers they share.

mod agents;
mod ask;
mod blocked;
mod btw;
mod crew_member;
mod delegate;
mod far_watch;
mod fork;
mod machine;
mod quick;
mod report_back;
mod sandbox;
mod setup;
mod sign_in;
pub(crate) use ask::{GUARD_ASKS_YOU, no_follow_up_on_guard, open_guard, open_setup};
pub use setup::{KEYS_SETTING, RELAY, may_answer};

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use wisp_runtime_api::TurnId;
use wisp_runtime_api::model::{Activity, Session, ToolKind, Turn, TurnStatus};

use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;

pub(crate) use agents::Rounds;
pub(crate) use ask::{ask_id, reconcile_ask};
pub(crate) use quick::QuickRuns;
pub(crate) use report_back::{lead_of, still_working};
pub(crate) use sign_in::SignIn;

/// How often a wait on an answer or on subagents looks again.
const POLL_MS: u64 = 250;
const SLOT: &str = "wisp.runtime";

fn slot(s: &Session) -> Option<&serde_json::Map<String, Value>> {
    s.ext.get(SLOT)?.as_object()
}

fn str_at(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned()
}

/// Appends to a list in the slot (made if missing).
fn push(m: &mut serde_json::Map<String, Value>, k: &str, v: Value) {
    match m.get_mut(k) {
        Some(Value::Array(a)) => a.push(v),
        _ => {
            m.insert(k.into(), Value::Array(vec![v]));
        }
    }
}

fn tool_row(tool: ToolKind, label: String) -> Activity {
    Activity::Tool {
        parent: None,
        tool,
        label,
        file: None,
        detail: None,
        call_id: None,
        output: None,
        title: None,
        hide: std::collections::BTreeMap::new(),
    }
}

/// A setup card a restart left open, taken off its task (audit G21: its agent is gone, so an Allow would change your
/// setup with nobody told and `wisp show` didn't list it): what it asked, for the turn's words; None when none is open.
pub(crate) fn take_setup_card(s: &mut wisp_runtime_api::model::Session) -> Option<String> {
    ask::open_setup(s)?;
    let ask = with_slot(&mut s.ext, |m| m.remove("ask"))?;
    let action = ask.pointer("/setup/action").and_then(Value::as_str).unwrap_or("change your setup").to_owned();
    Some(action)
}

/// The open setup card's action ("add the connector x"), if the open question is one.
pub(crate) fn open_card_action(s: &wisp_runtime_api::model::Session) -> Option<String> {
    ask::open_setup(s).map(|c| c.get("action").and_then(Value::as_str).unwrap_or("change your setup").to_owned())
}

/// A "You answered"-kind row on a turn (what became of its question).
pub(crate) fn answer_row(label: String) -> Activity {
    tool_row(ToolKind::Answer, label)
}

/// Runs `f` on runtime's own ext slot (made an object if it wasn't one), then puts it back.
fn with_slot<R>(ext: &mut BTreeMap<String, Value>, f: impl FnOnce(&mut serde_json::Map<String, Value>) -> R) -> R {
    let mut m = match ext.remove(SLOT) {
        Some(Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    let out = f(&mut m);
    ext.insert(SLOT.into(), Value::Object(m));
    out
}

impl Runtime {
    /// `runtime.held`: the task's held turn (its last) started or skipped; `yes`: you pressed Start now.
    pub async fn held(self: &Arc<Self>, id: &str, start: bool, yes: bool) -> Result<()> {
        let action = if start { wisp_runtime_api::commands::ReleaseAction::Start } else { wisp_runtime_api::commands::ReleaseAction::Skip };
        self.held_as(id, action, yes).await
    }

    /// `runtime.held` with any of its actions (Run without them too).
    pub async fn held_as(self: &Arc<Self>, id: &str, action: wisp_runtime_api::commands::ReleaseAction, yes: bool) -> Result<()> {
        let turn = self
            .last_turn(id)
            .await?
            .filter(|t| t.status == TurnStatus::Held)
            .ok_or_else(|| RuntimeError::Conflict("nothing is held on this task".into()))?;
        self.release_as(id, turn.id.as_str(), action, yes).await
    }

    async fn last_turn(&self, id: &str) -> Result<Option<Turn>> {
        Ok(self.repo.turns(id, None, Some(1)).await?.pop())
    }

    async fn turn_of(&self, id: &str, turn: Option<&TurnId>) -> Result<Turn> {
        match turn {
            Some(t) => self.repo.turn(t.as_str()).await,
            None => self.last_turn(id).await?.ok_or_else(|| RuntimeError::NoTurn(id.to_owned())),
        }
    }
}
