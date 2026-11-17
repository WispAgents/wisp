//! A Make a pane task's first run gets the pane guide before what the user asked (v1 server/pane-guide.js, a
//! system-prompt section there): its `ext` says `wisp.stuff: { kind: "pane" }`. Without it an agent asked for "a
//! hello world pane" built its own harness's kind of pane (owner, 2026-11-17). Reruns carry the kind (`TaskSpec.ext`).

use wisp_kernel::{Ctx, Failure, InterceptOpts, Setup, Verdict};
use wisp_runtime_api::commands::{Get, IdArgs};
use wisp_runtime_api::hooks::{BeforeRun, BeforeRunCtx, RunKind};
use wisp_stuff_core::guide;

pub fn register(s: &mut Setup) {
    // late (after the account, the budget and trust have had their say): it only adds words to the prompt
    s.intercept::<BeforeRun, _, _>(InterceptOpts { priority: 80, failure: Failure::Open, timeout_ms: 2000 }, |ctx, _call, c| async move {
        with_guide(&ctx, c).await
    });
}

async fn with_guide(ctx: &Ctx, mut c: BeforeRunCtx) -> Verdict<BeforeRunCtx> {
    if c.kind != RunKind::First {
        return Verdict::Continue;
    }
    let Ok(task) = ctx.run::<Get>(&ctx.root_call(), IdArgs { id: c.session_id.clone() }).await else { return Verdict::Continue };
    let ext = serde_json::to_value(&task.ext).unwrap_or_default();
    if !guide::is_pane_task(&ext) {
        return Verdict::Continue;
    }
    c.prompt = guide::with_guide("the ask_user tool", &c.prompt);
    Verdict::Modify { ctx: c }
}
