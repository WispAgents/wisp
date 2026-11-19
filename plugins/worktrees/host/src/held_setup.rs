//! A new worktree's setup the repo isn't trusted for (v1 project-trust.js setupHold, routes/sessions.js held): it waited
//! in the worktree's `prepared.held`, and nothing held the turn, so each run went straight to the harness without its
//! setup (audit D5). The trust card lists it with the repo's hooks and servers; Trust and start runs it in the worktree
//! (the card says so meanwhile), then the run starts; Run without them runs without it.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_kernel::{Code, Ctx};
use wisp_runtime_api::commands::{Get, IdArgs, Release, ReleaseAction, ReleaseArgs};
use wisp_runtime_api::{SessionId, TurnId};
use wisp_worktrees_api::{EXT, TrustItem, Worktree};

use crate::engine::{Engine, prepared_or_none};

/// The task's worktree, when its setup still waits for trust.
pub async fn waiting(ctx: &Ctx, session: &str) -> Option<Worktree> {
    let s = ctx.run::<Get>(&ctx.root_call(), IdArgs { id: SessionId(session.to_owned()) }).await.ok()?;
    let wt: Worktree = serde_json::from_value(s.ext_of(EXT).clone()).ok()?;
    wt.prepared.as_ref().and_then(|p| p.held.as_ref()).is_some_and(|h| !h.is_empty()).then_some(wt)
}

/// What waits in the worktree (the card's setup items).
pub fn items(wt: &Worktree) -> Vec<TrustItem> {
    wt.prepared.as_ref().and_then(|p| p.held.clone()).unwrap_or_default()
}

/// The hold the card shows while the trusted setup runs (v1 `setup: 'running'`): its buttons wait.
pub fn running(mut ctx: Value, key: &str, items: &[TrustItem]) -> Value {
    if let Some(o) = ctx.as_object_mut() {
        let data = json!({ "key": key, "items": items, "setup": "running" });
        o.insert("hold".into(), json!({ "by": "wisp.worktrees", "kind": "trust", "data": data }));
    }
    ctx
}

/// Trust and start with a waiting setup: it runs in the worktree, what it did is kept on the task, then the held turn
/// starts (a setup that failed is said there and the run goes, as v1). Off the run's way: the interceptor has 1 s.
pub fn run_then_start(ctx: &Ctx, engine: Arc<Engine>, wt: Worktree, session: String, turn: String) {
    let ctx2 = ctx.clone();
    ctx.spawn(async move {
        if let Some(p) = crate::prepare::run_held(&engine, &wt).await {
            let prepared = prepared_or_none(p);
            let call = ctx2.root_call();
            let put = |slot: &Value, _: &_| {
                let mut v = slot.clone();
                if let Some(o) = v.as_object_mut() {
                    o.insert("prepared".into(), serde_json::to_value(&prepared).unwrap_or(Value::Null));
                }
                Ok(Some(v))
            };
            if let Err(e) = wisp_runtime_api::ext::update_ext(&ctx2, &call, &session, EXT, put).await {
                tracing::warn!(error = %e.message, task = %session, "worktrees: keeping what the setup did");
            }
        }
        start(&ctx2, &session, &turn).await;
    });
}

/// The held turn starts. The re-hold that says "running" is written by the runtime as `before:run` returns, so a setup
/// that ends first finds the turn not held yet: it asks again until it is (at most 10 s).
async fn start(ctx: &Ctx, session: &str, turn: &str) {
    for _ in 0..100 {
        let a = ReleaseArgs { id: SessionId(session.to_owned()), turn_id: TurnId(turn.to_owned()), action: ReleaseAction::Start };
        match ctx.run::<Release>(&ctx.root_call(), a).await {
            Ok(_) => return,
            Err(e) if e.code == Code::Conflict => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
            Err(e) => {
                tracing::warn!(error = %e.message, task = session, "worktrees: starting the run after its setup");
                return;
            }
        }
    }
    tracing::warn!(task = session, "worktrees: the run after its setup was never held to start");
}
