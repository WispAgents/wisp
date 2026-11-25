//! wisp.workflows host (ADR-0028, ADR-0119): the `workflow` resource kind (`workflows:` in a formula, kept in
//! `workflows_def`), the runner (run.rs), `/workflow` (slash.rs) and each task's line of its runs (the row summary).
//!
//! A run belongs to a task: the agent's that started it with `workflows_run`, the one the user named, or a held task
//! made for it. The runner asks wisp-workflows-core what starts next, makes each step a child task of that owner with
//! `runtime.create`, wakes on `turn.ended` to read how each ended, writes where the run stands on the owner
//! (`ext["wisp.workflows"].runs`) and, when it ended, sends the owner every step's result.
//!
//! Not yet: `on:` another machine (steps run here, and the run says so), and picking up a run wisp was stopped in the
//! middle of (it says it was cut, and its owner can run it again).

mod edit;
mod run;
mod slash;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use wisp_kernel::{Call, Failure, InterceptOpts, Manifest, Plugin, Result, Setup, Verdict, WireError};
use wisp_resources_api::{CrudKind, KindInfo, RESOURCE_KIND, Resource, ResourceKind, schema_of};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::events::TurnEnded;
use wisp_runtime_api::hooks::{BeforeCreate, BeforeFollowUp};
use wisp_store::{Col, EntityDef, Migration, Query, Row, Store};
use wisp_workflows_api::{EXT, Edit, List, NamedWorkflow, Remove, Run as RunCmd, Set, Starters, Status, Stop, WorkflowSpec};
use wisp_workflows_core::check;

pub use wisp_workflows_api::PLUGIN_ID;

const OWNER: &str = "workflows";
const MIGRATIONS: &[Migration] = &[Migration::sql(1, "CREATE TABLE workflows_def (id TEXT PRIMARY KEY, body TEXT NOT NULL) STRICT;")];

fn topics(_: &Row) -> Vec<String> {
    vec!["plugin:wisp.workflows:workflows".into()]
}

const DEF: EntityDef = EntityDef {
    kind: "workflows_def",
    table: "workflows_def",
    key: "id",
    columns: &[("id", Col::Text), ("body", Col::Text)],
    json_columns: &["body"],
    parent: None,
    seq: None,
    topics,
};

#[allow(clippy::needless_pass_by_value)] // for map_err
fn store_err(e: wisp_store::StoreError) -> WireError {
    WireError::internal(format!("store: {e}"))
}

/// The prebuilt workflows (starters.json): run by name (`/workflow audit`, `wisp run audit`) unless one of yours has
/// the name; read-only, and never in an export (save one to make it yours: `wisp workflows starters`, then apply it).
pub(crate) fn starters() -> Vec<NamedWorkflow> {
    serde_json::from_str(include_str!("../starters.json")).unwrap_or_default()
}

/// Yours, then the prebuilt ones yours don't name.
pub(crate) async fn runnable(store: &Store) -> Result<Vec<NamedWorkflow>> {
    let mut all = list(store).await?;
    for s in starters() {
        if !all.iter().any(|w| w.name == s.name) {
            all.push(s);
        }
    }
    Ok(all)
}

pub(crate) async fn list(store: &Store) -> Result<Vec<NamedWorkflow>> {
    let rows = store.read_async(|r| r.query(DEF.kind, &Query::new())).await.map_err(store_err)?;
    let mut out: Vec<NamedWorkflow> = rows.into_iter().filter_map(|r| serde_json::from_value(r.get("body")?.clone()).ok()).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

async fn put(store: &Store, w: NamedWorkflow) -> Result<NamedWorkflow> {
    if w.name.trim().is_empty() || !w.name.chars().all(|c| c.is_alphanumeric() || "-_.".contains(c)) {
        return Err(WireError::invalid(format!("a workflow's name is letters, digits, - _ . (not {:?})", w.name)));
    }
    let problems = check(&w.spec);
    if !problems.is_empty() {
        return Err(WireError::invalid(format!("{}: {}", w.name, problems.join("; "))));
    }
    let body = serde_json::to_value(&w).map_err(|e| WireError::internal(e.to_string()))?;
    let id = w.name.clone();
    store
        .write(move |wr| {
            let mut r = Row::new();
            r.insert("id".into(), json!(id));
            r.insert("body".into(), body);
            wr.upsert(DEF.kind, r).map(|_| ())
        })
        .await
        .map_err(store_err)?;
    Ok(w)
}

/// `runtime.prompt@1`: the lead's note in front of a turn whose prompt is `/workflow <words>` (slash.rs); wisp's own
/// follow-ups (a run's result) are never read as one.
struct Lead(wisp_kernel::Ctx);

impl wisp_runtime_api::points::PromptPart for Lead {
    fn id(&self) -> &'static str {
        "wisp.workflows.lead"
    }
    fn part(&self, _session: &wisp_runtime_api::model::Session) -> String {
        String::new()
    }
    fn before(&self, note: wisp_runtime_api::points::TurnNote) -> wisp_kernel::BoxFuture<'static, String> {
        let ctx = self.0.clone();
        Box::pin(async move {
            if note.turn.by.as_deref().is_some_and(|b| b != "user") {
                return String::new();
            }
            let Some(words) = slash::typed(&note.turn.prompt) else { return String::new() };
            let names: Vec<slash::Known> = match ctx.service::<Store>() {
                Ok(store) => runnable(&store)
                    .await
                    .map(|l| {
                        l.into_iter().map(|w| slash::Known { inputs: w.spec.inputs.keys().cloned().collect(), name: w.name }).collect()
                    })
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            let tool = note.tools.map(|pre| format!("{pre}workflows_run"));
            slash::lead_note(words, &names, tool.as_deref()).unwrap_or_default()
        })
    }
}

const SLASH: InterceptOpts = InterceptOpts { priority: 12, failure: Failure::Open, timeout_ms: 300 };

fn veto<T>(message: String) -> Verdict<T> {
    Verdict::Veto { code: "invalid".into(), message, then: Vec::new() }
}

/// The `workflow` resource kind; a definition is checked when it's applied.
struct WorkflowKind(CrudKind);

impl ResourceKind for WorkflowKind {
    fn info(&self) -> KindInfo {
        self.0.info()
    }
    fn read<'a>(&'a self, call: &'a Call) -> wisp_kernel::BoxFuture<'a, Result<Vec<Resource>>> {
        self.0.read(call)
    }
    fn check(&self, r: &Resource) -> Vec<String> {
        match serde_json::from_value::<WorkflowSpec>(r.spec.clone()) {
            Ok(w) => check(&w),
            Err(e) => vec![e.to_string()],
        }
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a wisp_resources_api::Op) -> wisp_kernel::BoxFuture<'a, Result<()>> {
        self.0.apply(call, op)
    }
}

/// `runtime.after@1`: a task whose agent's turn ends while a run of its goes stays in Working on it (the list and the
/// task say it works, not that it waits on you); the run's end lands it (wisp's message with the results starts the
/// agent's next turn, or `runtime.settle`). You stopping the task stops its runs.
struct Hold(wisp_kernel::Ctx, Arc<run::State>);

impl wisp_runtime_api::points::AfterTurn for Hold {
    fn id(&self) -> &'static str {
        "wisp.workflows.hold"
    }
    fn order(&self) -> i32 {
        0
    }
    fn wants(
        &self,
        session: &wisp_runtime_api::model::Session,
        _turn: &wisp_runtime_api::model::Turn,
        end: wisp_runtime_api::points::TurnEnd,
    ) -> bool {
        !end.stopped && self.1.owns_live(session.id.as_str())
    }
    fn run(
        &self,
        session: wisp_runtime_api::model::Session,
        _turn: wisp_runtime_api::model::Turn,
    ) -> wisp_kernel::BoxFuture<'static, Result<()>> {
        let (ctx, st) = (self.0.clone(), self.1.clone());
        Box::pin(async move {
            // it ended between the turn's end and now: nothing will land the task, so it lands here
            if !st.owns_live(session.id.as_str()) {
                run::settle(&ctx, &ctx.root_call(), session.id.as_str()).await;
            }
            Ok(())
        })
    }
    fn cancel(&self, session: &wisp_runtime_api::model::Session, stop: bool, _by: Option<&str>) -> bool {
        // a follow-up moved the task on: its runs go on, and hold it again at that turn's end
        stop && self.1.stop_owned(session.id.as_str())
    }
}

/// `runtime.summary@1`: a task's runs on its list row (`workflowRuns`, the newest last), and a step's task names its
/// run (`workflowStep`). The page's band reads them.
struct Summary;

impl wisp_runtime_api::points::SummaryField for Summary {
    fn plugin(&self) -> &str {
        PLUGIN_ID
    }
    fn fields(
        &self,
        session: &wisp_runtime_api::model::Session,
        _last: Option<&wisp_runtime_api::model::Turn>,
        out: &mut BTreeMap<String, Value>,
    ) {
        let Some(slot) = session.ext.get(EXT) else { return };
        if let (Some(run), Some(node)) = (slot.get("run"), slot.get("node")) {
            out.insert("workflowStep".into(), json!({ "run": run, "node": node }));
        }
        if let Some(runs) = slot.get("runs").filter(|r| r.as_array().is_some_and(|a| !a.is_empty())) {
            out.insert("workflowRuns".into(), runs.clone());
        }
    }
}

/// A view that still says `running` at start was cut by wisp stopping: it says so (picking it back up is not yet).
pub fn interrupted(slot: &Value) -> Option<Value> {
    let runs = slot.get("runs")?.as_array()?;
    if !runs.iter().any(|r| r.get("state").and_then(Value::as_str) == Some("running")) {
        return None;
    }
    let mut v = slot.clone();
    for r in v.get_mut("runs").and_then(Value::as_array_mut).into_iter().flatten() {
        if r.get("state").and_then(Value::as_str) != Some("running") {
            continue;
        }
        r["state"] = json!("stopped");
        r["notes"] = json!(["wisp stopped while this ran: run it again to start it over"]);
        for s in r.get_mut("steps").and_then(Value::as_array_mut).into_iter().flatten() {
            match s.get("state").and_then(Value::as_str) {
                Some("running") => {
                    s["state"] = json!("failed");
                    s["why"] = json!("wisp stopped");
                }
                Some("waiting") => {
                    s["state"] = json!("skipped");
                    s["why"] = json!("wisp stopped");
                }
                _ => {}
            }
        }
    }
    if v.get("state").and_then(Value::as_str) == Some("running") {
        v["state"] = json!("stopped");
    }
    Some(v)
}

/// At start: every run a task still says is going was cut; each says so.
async fn recover(ctx: &wisp_kernel::Ctx) {
    let call = ctx.root_call();
    let a = r::SlotsArgs { plugins: vec![EXT.into()], all: false, having: vec!["runs".into()] };
    let Ok(found) = ctx.run::<r::Slots>(&call, a).await else { return };
    for t in found {
        let Some(v) = t.ext.get(EXT).and_then(interrupted) else { continue };
        // set-ext: whole one writer: no run is going yet, so nothing else writes these slots now
        let a = r::SetExtArgs { id: t.id.clone(), plugin: EXT.into(), value: v, expect: None };
        if let Err(err) = ctx.run::<r::SetExt>(&call, a).await {
            tracing::warn!(error = %err.message, task = %t.id.as_str(), "workflows: a cut run couldn't say so");
        }
    }
}

#[derive(Debug, Default)]
pub struct WorkflowsPlugin;

impl Plugin for WorkflowsPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        use wisp_resources_api::Names;
        // what a workflow and its steps name (ADR-0111)
        let info = KindInfo::new("workflow", "workflows", PLUGIN_ID, schema_of::<WorkflowSpec>())
            .order(90)
            .names(Names::new("project", "project"))
            .names(Names::new("on.trigger", "trigger"))
            .names(Names::new("tasks.*.agent", "agent"))
            .names(Names::new("tasks.*.on", "machine"))
            .names(Names::new("tasks.*.project", "project"));
        let kind: Arc<dyn ResourceKind> = Arc::new(WorkflowKind(
            CrudKind::new::<WorkflowSpec>(info, s.ctx(), "workflows.list", "workflows.set", "workflows.set", "workflows.remove")
                .keyed("name", "name", "name"),
        ));
        s.contribute(&RESOURCE_KIND, kind);
        s.contribute(&wisp_runtime_api::points::SUMMARY, Arc::new(Summary) as Arc<dyn wisp_runtime_api::points::SummaryField>);
        let st = Arc::new(run::State::new());
        s.contribute(&wisp_runtime_api::points::AFTER, Arc::new(Hold(s.ctx(), st.clone())) as Arc<dyn wisp_runtime_api::points::AfterTurn>);
        s.command::<List, _, _>(|ctx, _, _| async move { list(&*ctx.service::<Store>()?).await });
        s.command::<Starters, _, _>(|_ctx, _, _| async move { Ok(starters()) });
        s.command::<Set, _, _>(|ctx, _, w| async move { put(&*ctx.service::<Store>()?, w).await });
        s.command::<Remove, _, _>(|ctx, _, a| async move {
            let store = ctx.service::<Store>()?;
            store.write(move |w| w.del(DEF.kind, a.name.as_str()).map(|_| ())).await.map_err(store_err)?;
            Ok(json!({ "ok": true }))
        });
        let st2 = st.clone();
        s.command::<RunCmd, _, _>(move |ctx, call, a| {
            let st = st2.clone();
            async move { run::start(&ctx, &call, &st, a).await }
        });
        let st2 = st.clone();
        s.command::<Status, _, _>(move |ctx, call, a| {
            let st = st2.clone();
            async move { run::status(&ctx, &call, &st, &a.id).await }
        });
        let st2 = st.clone();
        s.command::<Edit, _, _>(move |ctx, call, a| {
            let st = st2.clone();
            async move { edit::edit(&ctx, &call, &st, a).await }
        });
        let st2 = st.clone();
        s.command::<Stop, _, _>(move |_ctx, call, a| {
            let st = st2.clone();
            async move { run::stop(&call, &st, &a.id) }
        });
        // `/workflow` alone: say what to type (the prompt itself stays as typed; Lead adds the note)
        s.intercept::<BeforeCreate, _, _>(SLASH, |_ctx, _call, a| async move {
            if a.parent_id.is_none() && slash::typed(&a.prompt) == Some("") { veto(slash::SAY_WHAT.into()) } else { Verdict::Continue }
        });
        s.intercept::<BeforeFollowUp, _, _>(SLASH, |_ctx, _call, a| async move {
            if a.by.as_deref().is_none_or(|b| b == "user") && slash::typed(&a.text) == Some("") {
                veto(slash::SAY_WHAT.into())
            } else {
                Verdict::Continue
            }
        });
        s.contribute(&wisp_runtime_api::points::PROMPT, Arc::new(Lead(s.ctx())) as Arc<dyn wisp_runtime_api::points::PromptPart>);
        let st3 = st;
        s.on::<TurnEnded, _, _>(move |_ctx, e| {
            // no run listening (none going) is fine: nothing waits on this turn
            let _ = st3.ended.send((e.session_id.as_str().to_owned(), e.cost_usd.unwrap_or(0.0)));
            async move {}
        });
        s.on_start(|ctx| async move {
            let store = ctx.service::<Store>()?;
            store.migrate(OWNER, MIGRATIONS).map_err(store_err)?;
            store.define_entity(DEF).map_err(store_err)?;
            let c = ctx.clone();
            ctx.spawn(async move { recover(&c).await });
            Ok(())
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_runtime_api::{Mode, SessionState, Turn, TurnStatus};

    #[test]
    fn every_prebuilt_workflow_checks() {
        let all = starters();
        assert!(all.len() >= 5, "{all:?}");
        for w in all {
            assert_eq!(wisp_workflows_core::check(&w.spec), Vec::<String>::new(), "{}", w.name);
            assert!(!w.spec.description.is_empty(), "{} says what it's for", w.name);
        }
    }

    #[test]
    fn manifest_is_valid() {
        assert_eq!(WorkflowsPlugin.manifest().id, PLUGIN_ID);
    }

    #[test]
    fn a_wrong_mode_fails_the_step_and_an_empty_one_is_left_out() {
        assert_eq!(run::mode_of(""), Ok(None));
        assert_eq!(run::mode_of("plan"), Ok(Some(Mode::Plan)));
        assert!(run::mode_of("yolo").is_err_and(|e| e.contains("yolo")));
    }

    fn turn(status: TurnStatus, is_error: bool) -> Turn {
        serde_json::from_value(json!({
            "id": "t1", "sessionId": "s1", "idx": 0, "prompt": "p", "agent": "claude", "model": "", "effort": "",
            "mode": "auto", "status": status, "startedAt": 0, "endedAt": null, "stats": null, "presentation": null,
            "isError": is_error,
        }))
        .unwrap_or_else(|e| panic!("turn: {e}"))
    }

    #[test]
    fn a_step_is_ok_only_when_its_last_turn_is_done_without_an_error_() {
        assert!(run::outcome(SessionState::Review, Some(&turn(TurnStatus::Done, false))).0);
        let (ok, _, why) = run::outcome(SessionState::Review, Some(&turn(TurnStatus::Done, true)));
        assert!(!ok);
        assert_eq!(why, "it ended done");
        assert_eq!(run::outcome(SessionState::Error, Some(&turn(TurnStatus::Stopped, false))).2, "it ended stopped");
        // no turn at all: the task's own state says how it ended
        assert_eq!(run::outcome(SessionState::Error, None), (false, Value::Null, "it ended error".to_owned()));
    }

    #[test]
    fn a_run_cut_by_a_restart_says_so() {
        let slot = json!({ "runs": [
            { "id": "r1", "state": "running", "steps": [{ "id": "a", "state": "done" }, { "id": "b", "state": "running" }, { "id": "c", "state": "waiting" }] },
            { "id": "r0", "state": "done", "steps": [] },
        ] });
        let v = interrupted(&slot).unwrap_or_default();
        assert_eq!(v["runs"][0]["state"], "stopped");
        assert_eq!(v["runs"][0]["steps"][0]["state"], "done");
        assert_eq!(v["runs"][0]["steps"][1]["state"], "failed");
        assert_eq!(v["runs"][0]["steps"][2]["state"], "skipped");
        assert_eq!(v["runs"][1]["state"], "done");
        assert!(interrupted(&json!({ "runs": [{ "state": "done" }] })).is_none());
        assert!(interrupted(&json!({ "run": "r1", "node": "a" })).is_none(), "a step's task isn't a run");
    }
}
