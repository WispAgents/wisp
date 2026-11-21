//! The kernel side of the host seam (`CtxHost`) and the task tree the kernel's agent-token scope reads.

use std::sync::Arc;

use wisp_kernel::{BoxFuture, Ctx, Result as WireResult, WireError};
use wisp_runtime_api::hooks::{BeforeRun, BeforeRunCtx};
use wisp_runtime_api::points::{AFTER, BLOCK, OUTCOME, PROMPT, RUN, SUMMARY};
use wisp_runtime_api::{HARNESS, Harness};

use super::{Slot, rt};
use crate::host::{Emitted, Host};

/// The kernel side of the host seam.
pub(super) struct CtxHost {
    pub(super) ctx: Ctx,
}

impl Host for CtxHost {
    fn setting(&self, key: &str) -> Option<serde_json::Value> {
        self.ctx.setting(key)
    }
    fn shots(&self) -> Option<std::path::PathBuf> {
        self.ctx.service::<wisp_kernel::services::Paths>().ok().map(|p| p.join("shots"))
    }
    fn net(&self) -> Option<wisp_kernel::services::NetService> {
        wisp_kernel::services::net(&self.ctx).ok()
    }
    fn command(
        &self,
        cmd: &str,
        args: serde_json::Value,
    ) -> wisp_kernel::BoxFuture<'static, std::result::Result<serde_json::Value, WireError>> {
        self.ctx.run_json(&self.ctx.root_call(), cmd, args)
    }
    fn emit(&self, e: Emitted) {
        let call = self.ctx.root_call();
        send(&self.ctx, e, &mut |x: Out<'_>| x.on_call(&self.ctx, &call));
    }
    fn outbox(&self, events: &[Emitted]) -> Option<wisp_kernel::Outbox> {
        let ob = self.ctx.outbox();
        for e in events {
            send(&self.ctx, e.clone(), &mut |x: Out<'_>| x.into(&ob));
        }
        Some(ob)
    }
    fn before_run(&self, c: BeforeRunCtx) -> BoxFuture<'static, WireResult<BeforeRunCtx>> {
        self.ctx.run_hook::<BeforeRun>(&self.ctx.root_call(), c)
    }
    fn harnesses(&self) -> Vec<Arc<dyn Harness>> {
        self.ctx.contributions(&HARNESS)
    }
    fn spawn(&self, fut: BoxFuture<'static, ()>) {
        self.ctx.spawn(fut);
    }
    fn track(&self, event: &'static str, props: serde_json::Value) {
        let ctx = self.ctx.clone();
        self.ctx.spawn(Box::pin(async move { super::track(&ctx, event, props).await }));
    }
    fn scope(&self) -> Option<wisp_proc::Scope> {
        Some(self.ctx.scope().clone())
    }
    fn agent_access(&self) -> Option<Arc<dyn wisp_runtime_api::points::AgentAccess>> {
        self.ctx.service::<Arc<dyn wisp_runtime_api::points::AgentAccess>>().ok().map(|a| a.as_ref().clone())
    }
    fn summary_fields(&self) -> Vec<Arc<dyn wisp_runtime_api::points::SummaryField>> {
        self.ctx.contributions(&SUMMARY)
    }
    fn after_steps(&self) -> Vec<Arc<dyn wisp_runtime_api::points::AfterTurn>> {
        self.ctx.contributions(&AFTER)
    }
    fn blocks(&self) -> Vec<Arc<dyn wisp_runtime_api::points::BlockType>> {
        self.ctx.contributions(&BLOCK)
    }
    fn prompt_parts(&self) -> Vec<Arc<dyn wisp_runtime_api::points::PromptPart>> {
        self.ctx.contributions(&PROMPT)
    }
    fn run_parts(&self) -> Vec<Arc<dyn wisp_runtime_api::points::RunPart>> {
        self.ctx.contributions(&RUN)
    }
    fn outcomes(&self) -> Vec<Arc<dyn wisp_runtime_api::points::TurnOutcome>> {
        self.ctx.contributions(&OUTCOME)
    }
    fn router(&self) -> Option<(Arc<dyn wisp_router_api::service::RouterApi>, wisp_kernel::Call)> {
        self.ctx.use_service::<wisp_router_api::service::RouterService>().ok().map(|r| (r, self.ctx.root_call()))
    }
}

/// The task tree as the kernel's scope reads it: a delegation's parent chain.
pub(super) struct Tree(pub(super) Slot);

impl wisp_kernel::scope::TaskTree for Tree {
    fn within(&self, task: &str, root: &str) -> BoxFuture<'static, bool> {
        let (slot, task, root) = (self.0.clone(), task.to_owned(), root.to_owned());
        Box::pin(async move {
            match rt(&slot) {
                Ok(r) => r.under(&task, &root).await,
                Err(_) => false,
            }
        })
    }
    fn agent_set(&self, task: &str) -> Option<String> {
        rt(&self.0).ok()?.agent_set(task)
    }
}

/// `machine.runs@1`: the runs going now, each by the pid of the CLI wisp spawned. This computer places a run's
/// process tree from it on every OS (Windows has no `ps`, and a CLI's environment isn't read there), from its spawn,
/// before the CLI says anything (audit I IS12). A lock and a copy, as the point asks.
pub(super) struct Runs(pub(super) Slot);

impl wisp_machine_api::this_mac::LiveRuns for Runs {
    fn live(&self) -> Vec<wisp_machine_api::this_mac::LiveRun> {
        rt(&self.0).map_or_else(
            |_| Vec::new(),
            |r| r.live_pids().into_iter().map(|(task, pid)| wisp_machine_api::this_mac::LiveRun { task, pid }).collect(),
        )
    }
}

/// One runtime event, typed, on its way to a call (`emit`) or a write's outbox (ADR-0074).
enum Out<'a> {
    Created(&'a wisp_runtime_api::events::SessionCreated),
    Changed(&'a wisp_runtime_api::events::SessionChanged),
    Deleted(&'a wisp_runtime_api::events::SessionDeleted),
    Started(&'a wisp_runtime_api::events::TurnStarted),
    Activity(&'a wisp_runtime_api::events::TurnActivity),
    Finished(&'a wisp_runtime_api::events::TurnFinished),
    Ended(&'a wisp_runtime_api::events::TurnEnded),
    Held(&'a wisp_runtime_api::events::TurnHeld),
    Limits(&'a wisp_runtime_api::events::TurnLimits),
    Fact(&'static str, &'a wisp_runtime_api::events::TaskFact),
}

macro_rules! each {
    ($x:expr, |$e:ident| $body:expr) => {{
        use wisp_runtime_api::events::{TaskAsked, TaskDone, TaskFailed, TaskFinished, TaskHeld};
        match $x {
            Out::Created($e) => $body,
            Out::Changed($e) => $body,
            Out::Deleted($e) => $body,
            Out::Started($e) => $body,
            Out::Activity($e) => $body,
            Out::Finished($e) => $body,
            Out::Ended($e) => $body,
            Out::Held($e) => $body,
            Out::Limits($e) => $body,
            Out::Fact(ty, f) => match ty {
                "task.finished" => {
                    let $e = &TaskFinished(f.clone());
                    $body
                }
                "task.failed" => {
                    let $e = &TaskFailed(f.clone());
                    $body
                }
                "task.held" => {
                    let $e = &TaskHeld(f.clone());
                    $body
                }
                "task.asked" => {
                    let $e = &TaskAsked(f.clone());
                    $body
                }
                _ => {
                    let $e = &TaskDone(f.clone());
                    $body
                }
            },
        }
    }};
}

impl Out<'_> {
    fn on_call(self, ctx: &Ctx, call: &wisp_kernel::Call) {
        each!(self, |e| ctx.emit(call, e));
    }
    fn into(self, ob: &wisp_kernel::Outbox) {
        each!(self, |e| ob.emit(e));
    }
}

/// `e` as its typed event, to `put`; a step's .env output never leaves on the event either (ADR-0050).
fn send(ctx: &Ctx, e: Emitted, put: &mut dyn FnMut(Out<'_>)) {
    match e {
        Emitted::Created(x) => put(Out::Created(&x)),
        Emitted::Changed(x) => put(Out::Changed(&x)),
        Emitted::Deleted(x) => put(Out::Deleted(&x)),
        Emitted::Started(x) => put(Out::Started(&x)),
        Emitted::Activity(mut x) => {
            if crate::env_view::on(ctx) {
                match crate::env_view::mask_items(std::mem::take(&mut x.items)) {
                    Ok(items) => x.items = items,
                    Err(_) => return,
                }
            }
            put(Out::Activity(&x));
        }
        Emitted::Finished(x) => put(Out::Finished(&x)),
        Emitted::Ended(x) => put(Out::Ended(&x)),
        Emitted::Held(x) => put(Out::Held(&x)),
        Emitted::Limits(x) => put(Out::Limits(&x)),
        Emitted::Task(ty, f) => put(Out::Fact(ty, &f)),
    }
}
