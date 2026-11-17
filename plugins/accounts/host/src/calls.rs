//! wisp.accounts' calls to other plugins, through their typed commands (`wisp-runtime-api`, `wisp-telemetry-api`,
//! `wisp-git-api`, `wisp-machines-api`). What accounts only reports on (a usage stat, a held turn let go, a sign-in card woken) is logged
//! when refused; what a caller answers with is returned.

use serde_json::{Map, Value};
use wisp_git_api::Tools;
use wisp_git_api::commands as g;
use wisp_kernel::{Ctx, Result};
use wisp_machines_api::commands as m;
use wisp_runtime_api::commands::{self as r, AgentStatus};
use wisp_runtime_api::{SessionId, TurnId};
use wisp_telemetry_api as tel;

/// `telemetry.track`: a usage event this plugin owns (wisp.telemetry's catalogue: v1's names and keys); props that
/// aren't an object are sent as none.
pub async fn track(ctx: &Ctx, event: &str, props: Value) {
    let props = match props {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    if let Err(err) = ctx.run::<tel::Track>(&ctx.root_call(), tel::TrackArgs { event: event.into(), props }).await {
        tracing::warn!(error = %err.message, event, "accounts: telemetry.track");
    }
}

/// `runtime.release` (start): a turn the harness watch held runs now.
pub async fn release(ctx: &Ctx, id: &str, turn: &str) {
    let a = r::ReleaseArgs { id: SessionId(id.to_owned()), turn_id: TurnId(turn.to_owned()), action: r::ReleaseAction::Start };
    if let Err(err) = ctx.run::<r::Release>(&ctx.root_call(), a).await {
        tracing::warn!(error = %err.message, task = id, turn, "accounts: runtime.release");
    }
}

/// Where a turn the harness watch held stands now (`runtime.get` + `runtime.turns`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldNow {
    /// still held by `by` for `kind`, its task there and not archived: Trust starts it
    Ours,
    /// gone for good: its task deleted or archived, or the turn no longer held (skipped, run, stopped)
    Stale,
    /// not now: not written yet (held a moment ago), its task marked done, held by another hook since, or runtime refused
    Unknown,
}

pub async fn held_now(ctx: &Ctx, id: &str, turn: &str, by: &str, kind: &str) -> HeldNow {
    let sid = SessionId(id.to_owned());
    let a = r::TurnsArgs { id: sid.clone(), before: None, limit: None, masked: None };
    // the turn first: one not written yet says nothing (its task may be archived until its start un-archives it)
    let found = match ctx.run::<r::Turns>(&ctx.root_call(), a).await {
        Ok(ts) => ts.into_iter().find(|t| t.id.0 == turn),
        Err(err) if err.code == wisp_kernel::Code::NotFound => return HeldNow::Stale,
        Err(err) => {
            tracing::debug!(error = %err.message, task = id, turn, "accounts: runtime.turns");
            return HeldNow::Unknown;
        }
    };
    let Some(t) = found else { return HeldNow::Unknown };
    if t.status != wisp_runtime_api::model::TurnStatus::Held {
        return HeldNow::Stale;
    }
    match ctx.run::<r::Get>(&ctx.root_call(), r::IdArgs { id: sid }).await {
        Ok(s) if s.archived => HeldNow::Stale,
        // marked done: not now (Trust leaves it; reopened, its hold is as it was: QA r1 #10)
        Ok(s) if s.state == wisp_runtime_api::model::SessionState::Done => HeldNow::Unknown,
        Ok(_) if t.held.as_ref().is_some_and(|h| h.by == by && h.kind == kind) => HeldNow::Ours,
        Ok(_) => HeldNow::Unknown,
        Err(err) if err.code == wisp_kernel::Code::NotFound => HeldNow::Stale,
        Err(err) => {
            tracing::debug!(error = %err.message, task = id, "accounts: runtime.get");
            HeldNow::Unknown
        }
    }
}

/// `runtime.signInDone`: its models are looked up again, and the turn a sign-in card waits on runs again.
pub async fn sign_in_done(ctx: &Ctx, agent: &str, account: &str) {
    let a = r::SignInDoneArgs { agent: agent.to_owned(), account: account.to_owned() };
    if let Err(err) = ctx.run::<r::SignInDone>(&ctx.root_call(), a).await {
        tracing::warn!(error = %err.message, agent, "accounts: runtime.signInDone");
    }
}

/// `runtime.agents`: every harness as runtime sees it (`fresh: Some("all")` detects them all again).
pub async fn agents(ctx: &Ctx, fresh: Option<&str>) -> Result<Vec<AgentStatus>> {
    ctx.run::<r::Agents>(&ctx.root_call(), r::AgentsArgs { fresh: fresh.map(str::to_owned) }).await
}

/// `git.tools`: git's and gh's install state (None, logged at debug, when refused: asked on every publish).
pub async fn tools(ctx: &Ctx) -> Option<Tools> {
    match ctx.run::<g::GetTools>(&ctx.root_call(), ()).await {
        Ok(t) => Some(t),
        Err(err) => {
            tracing::debug!(error = %err.message, "accounts: git.tools");
            None
        }
    }
}

/// `machines.recheckAgents`: every online computer detects its harnesses again (v1 POST /agents/recheck asked them
/// with this one's). Reported only: a computer that's gone, or wisp.machines off, leaves this one's answer as it is.
pub async fn recheck_machines(ctx: &Ctx) {
    if let Err(err) = ctx.run::<m::RecheckAgents>(&ctx.root_call(), m::RecheckArgs { id: None, agent: None }).await {
        tracing::debug!(error = %err.message, "accounts: machines.recheckAgents");
    }
}
