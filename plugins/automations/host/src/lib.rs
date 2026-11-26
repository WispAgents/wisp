//! wisp.automations host (triggers, watchers, recipes, cron schedules, inbound webhooks).
//!
//! Ported so far: **declared tasks** (ADR-0027), the `tasks:` of a formula. A declared task is a task
//! spec kept as a template in `automations_task`; `automations.task.run` makes a task from it (its
//! `{params}` filled, the spec's fields under the run's), and its schedule fires it once per matching
//! minute (`automations_task_run` keeps the minute it last fired, so a restart in that minute doesn't
//! fire it twice). Recipes are declared tasks with params. Triggers are one engine (ADR-0043, `engine.rs`,
//! `trigger.rs`).

pub mod engine;
mod plan;
mod trigger;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use wisp_automations_api::tasks::{
    DEFAULT, NamedTask, RunTask, TaskDecl, TaskRemove, TaskRun, TaskSet, Tasks, default_problems, fill, goal_arg, goal_ref, with_defaults,
};
use wisp_automations_api::{parse_cron, read_schedule};
use wisp_automations_core::schedule::{self, Due};
use wisp_kernel::{BoxFuture, Call, Command, Ctx, Manifest, Plugin, Result, Setup, WireError};
use wisp_resources_api::{CrudKind, KindInfo, Op, RESOURCE_KIND, Resource, ResourceKind, schema_of};
use wisp_runtime_api::Mode;
use wisp_runtime_api::commands::{Create, StartArgs};
use wisp_runtime_api::spec::Lineage;
use wisp_store::{Col, EntityDef, Migration, Query, Row, Store};

pub const OWNER: &str = "automations";

const MIGRATIONS: &[Migration] = &[
    Migration::sql(
        1,
        "CREATE TABLE automations_task (id TEXT PRIMARY KEY, body TEXT NOT NULL) STRICT;
     CREATE TABLE automations_task_run (id TEXT PRIMARY KEY, body TEXT NOT NULL) STRICT;",
    ),
    Migration::sql(2, "CREATE TABLE automations_trigger_state (id TEXT PRIMARY KEY, body TEXT NOT NULL) STRICT;"),
];

fn topics(_: &Row) -> Vec<String> {
    vec!["plugin:wisp.automations:tasks".into()]
}

const TASK: EntityDef = EntityDef {
    kind: "automations_task",
    table: "automations_task",
    key: "id",
    columns: &[("id", Col::Text), ("body", Col::Text)],
    json_columns: &["body"],
    parent: None,
    seq: None,
    topics,
};
const RUN: EntityDef = EntityDef {
    kind: "automations_task_run",
    table: "automations_task_run",
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

/// The declared tasks; `tasks.default` only `with_default` (the resource kind's read: it's a resource, never a task to
/// list or run, ADR-0110).
async fn list_all(store: &Store, with_default: bool) -> Result<Vec<NamedTask>> {
    let rows = store.read_async(|r| r.query(TASK.kind, &Query::new())).await.map_err(store_err)?;
    let mut out: Vec<NamedTask> = rows
        .into_iter()
        .filter_map(|r| serde_json::from_value::<NamedTask>(r.get("body")?.clone()).ok())
        .filter(|t| with_default || t.name != DEFAULT)
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

async fn list(store: &Store) -> Result<Vec<NamedTask>> {
    list_all(store, false).await
}

/// `tasks.default`'s spec, or an empty one.
async fn defaults_of(store: &Store) -> Result<TaskDecl> {
    Ok(list_all(store, true).await?.into_iter().find(|t| t.name == DEFAULT).map(|t| t.spec).unwrap_or_default())
}

fn checked(mut t: NamedTask) -> Result<NamedTask> {
    if t.name.trim().is_empty() || !t.name.chars().all(|c| c.is_alphanumeric() || "-_.".contains(c)) {
        return Err(WireError::invalid(format!("a task's name is letters, digits, - _ . (not {:?})", t.name)));
    }
    if t.name == DEFAULT {
        let problems = default_problems(&serde_json::to_value(&t.spec).unwrap_or_default());
        if let Some(p) = problems.first() {
            return Err(WireError::invalid(p.clone()));
        }
        return Ok(t);
    }
    if t.spec.prompt.trim().is_empty() {
        return Err(WireError::invalid(format!("{} needs a prompt", t.name)));
    }
    t.spec.schedule = read_schedule(&t.spec.schedule).map_err(|e| WireError::invalid(e.0))?;
    Ok(t)
}

async fn put(store: &Store, t: NamedTask) -> Result<NamedTask> {
    let t = checked(t)?;
    let body = serde_json::to_value(&t).map_err(|e| WireError::internal(e.to_string()))?;
    let id = t.name.clone();
    store
        .write(move |w| {
            let mut r = Row::new();
            r.insert("id".into(), json!(id));
            r.insert("body".into(), body);
            w.upsert(TASK.kind, r).map(|_| ())
        })
        .await
        .map_err(store_err)?;
    Ok(t)
}

/// The task a run makes: `runtime.create`'s args from the spec, the run's values and fields over it.
pub fn start_args(t: &NamedTask, run: &RunTask, by: &str) -> Result<StartArgs> {
    let mut values: BTreeMap<String, String> = run.params.clone();
    let names = wisp_automations_api::tasks::params_of(&format!("{} {}", t.spec.prompt, t.spec.steps.join(" ")));
    let mut words = run.words.iter();
    let open: Vec<&String> = names.iter().filter(|p| !values.contains_key(*p)).collect();
    for p in open {
        if let Some(w) = words.next() {
            values.insert(p.clone(), w.clone());
        }
    }
    let prompt = fill(&t.spec.prompt, &values).map_err(WireError::invalid)?;
    let steps: Vec<String> =
        t.spec.steps.iter().map(|s| fill(s, &values)).collect::<std::result::Result<_, _>>().map_err(WireError::invalid)?;
    let d = &t.spec;
    let opt = |s: &str| (!s.is_empty()).then(|| s.to_owned());
    let args = StartArgs {
        prompt,
        title: Some(t.name.clone()),
        agent: opt(&d.agent),
        model: opt(&d.model),
        effort: opt(&d.effort),
        mode: mode_of(&d.mode)?,
        cwd: opt(&d.project),
        crew: opt(&d.crew),
        schedule: opt(&d.schedule),
        steps,
        connectors: d.connectors.clone(),
        worktree: d.worktree,
        goal: goal_arg(d.goal.as_ref()),
        factory: opt(&d.factory),
        notes: (!d.notes.is_empty()).then(|| serde_json::to_value(&d.notes).ok()).flatten(),
        lineage: Lineage { triggered_by: Some(by.to_owned()), ..Lineage::default() },
        ext: [(wisp_automations_api::PLUGIN_ID.to_owned(), json!({ "task": t.name }))].into_iter().collect(),
        ..StartArgs::default()
    };
    if run.over.is_empty() {
        return Ok(args);
    }
    // the run's own fields (`wisp run <task> --agent …`) are free-form: laid over the args, then checked whole
    let Value::Object(mut over) = serde_json::to_value(&args).map_err(|e| WireError::internal(e.to_string()))? else {
        return Err(WireError::internal("runtime.create's args are not an object"));
    };
    for (k, v) in &run.over {
        let k = if k == "project" { "cwd" } else { k.as_str() };
        over.insert(k.to_owned(), v.clone());
    }
    serde_json::from_value(Value::Object(over)).map_err(|e| WireError::invalid(format!("{}: {e}", Create::ID)))
}

/// A spec's `mode` (`plan`, `edits`, `auto`; empty: the task's default).
pub(crate) fn mode_of(s: &str) -> Result<Option<Mode>> {
    if s.is_empty() {
        return Ok(None);
    }
    serde_json::from_value(json!(s)).map(Some).map_err(|e| WireError::invalid(format!("{}: mode {s}: {e}", Create::ID)))
}

async fn run(ctx: &Ctx, call: &Call, store: &Store, r: RunTask, by: &str) -> Result<Value> {
    if r.name == DEFAULT {
        return Err(WireError::invalid(
            "tasks.default gives every task its defaults: it isn't a task to run (wisp recipes lists the declared tasks, wisp workflows list the workflows)",
        ));
    }
    let tasks = list(store).await?;
    let Some(t) = tasks.iter().find(|t| t.name == r.name) else {
        return Err(WireError::not_found(format!("no task named {} (wisp export tasks lists them)", r.name)));
    };
    // every run of a declared task, by hand, schedule or trigger, takes `tasks.default` under its own fields (ADR-0110)
    let t = &NamedTask { name: t.name.clone(), spec: with_defaults(&t.spec, &defaults_of(store).await?) };
    let mut args = start_args(t, &r, by)?;
    // a recipe that doesn't say: a worktree of its own, as v1 started every task (audit B #7)
    if args.worktree.is_none() && args.cwd.as_deref().is_some_and(|c| !c.is_empty()) {
        let set = ctx.setting(wisp_runtime_api::commands::WORKTREE_SETTING);
        args.worktree = Some(wisp_runtime_api::commands::worktree_by_default(set.as_ref(), args.mode == Some(Mode::Plan)));
    }
    recurring_tree(&mut args, by);
    let made = ctx.run::<Create>(call, args).await?;
    serde_json::to_value(made).map_err(|e| WireError::internal(e.to_string()))
}

/// A trigger's or a schedule's runs (`by`: `trigger:<name>`, `schedule:<name>`) never pile up worktrees (ADR-0108, audit
/// C3): they reuse one per trigger and project (`wisp/trigger-<name>`), and a run that changed nothing leaves none. A run
/// you started (`wisp run`) keeps a worktree of its own.
pub(crate) fn recurring_tree(args: &mut StartArgs, by: &str) {
    let Some((kind, name)) = by.split_once(':').filter(|(k, n)| matches!(*k, "trigger" | "schedule") && !n.is_empty()) else { return };
    args.worktree_key = Some(format!("{kind}-{name}"));
    args.worktree_tidy = true;
}

/// Runs declared task `name` in `cwd` for `by` (a trigger).
pub async fn run_by(ctx: &Ctx, call: &Call, store: &Store, name: &str, cwd: &str, by: &str) -> Result<Value> {
    let mut over = serde_json::Map::new();
    if !cwd.is_empty() {
        over.insert("project".into(), json!(cwd));
    }
    run(ctx, call, store, RunTask { name: name.to_owned(), over, ..RunTask::default() }, by).await
}

/// Once a minute: every declared task whose schedule came due since its last run, once. On local wall-clock minutes
/// in this computer's zone, as a `schedule` trigger is (`schedule::due`): a minute the tick missed (asleep, a slow
/// tick) runs once, late, never twice (audit doc23c: it matched this UTC minute only, so `0 9 * * *` ran at 09:00 UTC
/// and a missed 09:00 never ran).
async fn tick(ctx: &Ctx, store: &Store, now: i64) {
    let Ok(tasks) = list(store).await else { return };
    let zone = engine::zone("");
    for t in tasks.into_iter().filter(|t| !t.spec.paused && !t.spec.schedule.is_empty()) {
        let Ok(c) = parse_cron(&t.spec.schedule) else { continue };
        let due = claim(store, &t.name, c, zone.clone(), now).await.unwrap_or_else(|e| {
            // not marked: the minute isn't run, so a store that can't record it can't run it twice either
            tracing::warn!(task = %t.name, "couldn't record the scheduled minute, so it doesn't run: {e}");
            None
        });
        let Some(due) = due else { continue };
        let call = ctx.root_call();
        let name = t.name.clone();
        let mut r = RunTask { name: name.clone(), ..RunTask::default() };
        if due.late {
            tracing::info!(task = %name, "a scheduled run missed while this computer was off runs now, once");
            r.over.insert("ext".into(), json!({ wisp_automations_api::PLUGIN_ID: { "task": name, "late": true } }));
        }
        if let Err(e) = run(ctx, &call, store, r, &format!("schedule:{name}")).await {
            tracing::warn!(task = %name, "scheduled run failed: {}", e.message);
        }
    }
}

/// The run declared task `name`'s schedule owes at `now`, recorded (in one write, so it's claimed once) as the local
/// minute it is for: None when nothing came due since the last. A row from before local minutes (`minute`, UTC) is
/// read in this zone.
async fn claim(store: &Store, name: &str, c: wisp_automations_api::Cron, zone: engine::Tz, now: i64) -> wisp_store::Result<Option<Due>> {
    let id = name.to_owned();
    store
        .write(move |w| {
            let body = w.get(RUN.kind, id.as_str())?.and_then(|r| r.get("body").cloned()).unwrap_or(Value::Null);
            let last = body
                .get("local")
                .and_then(Value::as_i64)
                .or_else(|| body.get("minute").and_then(Value::as_i64).map(|m| schedule::local_minute(m.saturating_mul(60_000), &zone)));
            let Some(d) = schedule::due(&c, &zone, last, now) else { return Ok(None) };
            let mut r = Row::new();
            r.insert("id".into(), json!(id));
            r.insert("body".into(), json!({ "local": d.minute, "late": d.late }));
            w.upsert(RUN.kind, r)?;
            Ok(Some(d))
        })
        .await
}

/// The `task` resource kind: schedules in words are read as the store keeps them (`0 2 * * *`).
struct TaskKind(CrudKind);

impl ResourceKind for TaskKind {
    fn info(&self) -> KindInfo {
        self.0.info()
    }
    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        self.0.read(call)
    }
    fn normalize(&self, spec: &mut Value) {
        if let Some(c) = spec.get("schedule").and_then(Value::as_str).and_then(|s| read_schedule(s).ok()) {
            spec["schedule"] = Value::String(c);
        }
    }
    fn check(&self, r: &Resource) -> Vec<String> {
        if r.name == DEFAULT {
            return default_problems(&r.spec);
        }
        let mut out: Vec<String> =
            r.spec.get("schedule").and_then(Value::as_str).and_then(|s| read_schedule(s).err()).map(|e| vec![e.0]).unwrap_or_default();
        if r.spec.get("prompt").and_then(Value::as_str).is_none_or(|p| p.trim().is_empty()) {
            out.push("a task says what to do: prompt: …".to_owned());
        }
        out
    }
    /// The declared goal a task names (`goal: green-build`): `wisp validate` and `apply` refuse one no goal has.
    fn refers_to(&self) -> Vec<String> {
        vec!["goal".to_owned()]
    }
    fn refs(&self, r: &Resource) -> Vec<wisp_resources_api::Ref> {
        goal_ref(r.spec.get("goal"))
            .map(|name| vec![wisp_resources_api::Ref { kind: "goal".into(), name, field: "goal".into() }])
            .unwrap_or_default()
    }
    fn describe(&self, op: &Op) -> Option<String> {
        if op.name == DEFAULT {
            let verb = if op.op == wisp_resources_api::OpKind::Delete { "removes" } else { "sets" };
            return Some(format!("{verb} every task's defaults (tasks.default: its agent, mode, …)"));
        }
        let when = op
            .spec
            .get("schedule")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map_or_else(|| "when it's run".to_owned(), |s| format!("by itself on schedule {s}"));
        let verb = match op.op {
            wisp_resources_api::OpKind::Create => "adds",
            wisp_resources_api::OpKind::Update => "changes",
            wisp_resources_api::OpKind::Delete => "removes",
        };
        Some(format!("{verb} the task {} that runs an agent {when}", op.name))
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        self.0.apply(call, op)
    }
}

#[derive(Debug, Default)]
pub struct AutomationsPlugin;

impl Plugin for AutomationsPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let engine = Arc::new(engine::Engine::new());
        crate::trigger::register(s, &engine);
        crate::plan::on_started(s, &engine);
        let started = engine.clone();
        // a scheduled task acts without you: a foreign formula's needs --trust (ADR-0028)
        // what a task names (ADR-0111; its goal is `refs`, which reads the goal's name or object)
        let info = KindInfo::new("task", "tasks", wisp_automations_api::PLUGIN_ID, schema_of::<TaskDecl>())
            .order(80)
            .sensitive()
            .alias("recipes")
            .names(wisp_resources_api::Names::new("agent", "agent"))
            .names(wisp_resources_api::Names::new("connectors[]", "connector"))
            .names(wisp_resources_api::Names::new("crew", "crew"))
            .names(wisp_resources_api::Names::new("project", "project"))
            .names(wisp_resources_api::Names::new("factory", "factory"));
        let kind: Arc<dyn ResourceKind> = Arc::new(TaskKind(
            CrudKind::new::<TaskDecl>(
                info,
                s.ctx(),
                "automations.tasks",
                "automations.task.set",
                "automations.task.set",
                "automations.task.remove",
            )
            .keyed("name", "name", "name")
            // the kind reads `tasks.default` too: it's a resource like any other, kept apart from the tasks
            .list_args(json!({ "withDefault": true })),
        ));
        s.contribute(&RESOURCE_KIND, kind);
        s.command::<Tasks, _, _>(|ctx, _, a| async move {
            let with_default = a.get("withDefault").and_then(Value::as_bool).unwrap_or(false);
            list_all(&*ctx.service::<Store>()?, with_default).await
        });
        s.command::<TaskSet, _, _>(|ctx, _, t| async move { put(&*ctx.service::<Store>()?, t).await });
        s.command::<TaskRemove, _, _>(|ctx, _, a| async move {
            let store = ctx.service::<Store>()?;
            store.write(move |w| w.del(TASK.kind, a.name.as_str()).map(|_| ())).await.map_err(store_err)?;
            Ok(json!({ "ok": true }))
        });
        s.command::<TaskRun, _, _>(|ctx, call, r| async move {
            let store = ctx.service::<Store>()?;
            run(&ctx, &call, &store, r, "hand").await
        });
        s.on_start(move |ctx| async move {
            let store = ctx.service::<Store>()?;
            store.migrate(OWNER, MIGRATIONS).map_err(store_err)?;
            for d in [TASK, RUN, engine::STATE] {
                store.define_entity(d).map_err(store_err)?;
            }
            started.open((*store).clone());
            // rows held for the plan before a restart: their run's start still clears them
            started.seed_plan_runs().await;
            let clock = ctx.clock()?;
            let c2 = ctx.clone();
            ctx.spawn(async move {
                loop {
                    let now = clock.now_ms();
                    // the start of the next minute
                    clock.sleep(u64::try_from(60_000 - now.rem_euclid(60_000)).unwrap_or(60_000)).await;
                    tick(&c2, &store, clock.now_ms()).await;
                }
            });
            Ok(())
        });
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Audit C3 (ADR-0108): a trigger's and a schedule's runs reuse one worktree per trigger (and project) and leave none
    /// when they changed nothing; `wisp run` keeps a worktree of its own.
    #[test]
    fn a_triggers_and_a_schedules_runs_reuse_one_worktree_and_yours_has_its_own() {
        for (by, key) in
            [("trigger:uitrig", Some("trigger-uitrig")), ("schedule:nightly", Some("schedule-nightly")), ("user", None), ("trigger:", None)]
        {
            let mut a = StartArgs::default();
            recurring_tree(&mut a, by);
            assert_eq!(a.worktree_key.as_deref(), key, "{by}");
            assert_eq!(a.worktree_tidy, key.is_some(), "{by}");
        }
    }

    /// Audit doc23c: a declared task's schedule matched the tick's UTC minute only. Now local minutes, in the zone,
    /// and a minute the tick missed runs once, late; the next tick doesn't run it again.
    #[tokio::test]
    async fn a_declared_schedule_missed_at_9_runs_once_late_at_9_03_and_not_again_at_9_04() {
        let store = Store::open(wisp_store::Location::Memory, Arc::new(wisp_store::SystemClock)).unwrap();
        store.migrate(OWNER, MIGRATIONS).unwrap();
        store.define_entity(RUN).unwrap();
        let ny = engine::zone("America/New_York");
        let c = parse_cron("0 9 * * *").unwrap();
        // Monday 2026-11-23: 08:00 in New York is 13:00 UTC
        let eight = 1_795_438_800_000;
        let at = |min: i64| eight + min * 60_000;
        for (i, kept) in
            [json!({ "local": schedule::local_minute(eight, &ny) }), json!({ "minute": eight / 60_000 })].into_iter().enumerate()
        {
            let id = format!("nightly{i}");
            let row = json!({ "id": id, "body": kept });
            store.write(move |w| w.upsert(RUN.kind, serde_json::from_value(row).unwrap()).map(|_| ())).await.unwrap();
            // 09:03 local: 09:00 came and went unrun (asleep, a slow tick)
            let missed = claim(&store, &id, c, ny.clone(), at(63)).await.unwrap().expect("the missed 09:00 runs");
            assert_eq!(missed, Due { minute: schedule::local_minute(at(60), &ny), late: true });
            assert_eq!(claim(&store, &id, c, ny.clone(), at(64)).await.unwrap(), None, "not again at 09:04");
            // the next day's 09:00, on time
            assert_eq!(claim(&store, &id, c, ny.clone(), at(24 * 60 + 60)).await.unwrap().map(|d| d.late), Some(false));
        }
        // 09:00 UTC is 04:00 here: nothing
        assert_eq!(claim(&store, "other", c, ny.clone(), 1_795_424_400_000).await.unwrap(), None);
    }

    #[test]
    fn manifest_is_valid() {
        assert_eq!(AutomationsPlugin.manifest().id, "wisp.automations");
    }

    #[test]
    fn a_run_fills_params_and_takes_the_runs_fields_over_the_spec() {
        let t = NamedTask {
            name: "fix-issue".into(),
            spec: TaskDecl {
                prompt: "fix issue {n}".into(),
                steps: vec!["test {n}".into()],
                project: "/w".into(),
                agent: "reviewer".into(),
                ..TaskDecl::default()
            },
        };
        let r = RunTask {
            name: "fix-issue".into(),
            words: vec!["42".into()],
            over: [("agent".to_owned(), json!("codex"))].into_iter().collect(),
            ..RunTask::default()
        };
        let a = start_args(&t, &r, "hand").unwrap();
        assert_eq!(
            (a.prompt.as_str(), a.steps[0].as_str(), a.cwd.as_deref(), a.agent.as_deref()),
            ("fix issue 42", "test 42", Some("/w"), Some("codex"))
        );
        assert_eq!(a.lineage.triggered_by.as_deref(), Some("hand"));
        assert!(start_args(&t, &RunTask::default(), "hand").is_err(), "a missing param says which");
    }

    /// ADR-0114: a declared task's `factory` (or `tasks.default`'s) reaches its run; `wisp run <task> --factory` lays one over.
    #[test]
    fn a_declared_tasks_factory_reaches_its_run() {
        let d = TaskDecl { factory: "careful".into(), ..TaskDecl::default() };
        let t = NamedTask { name: "fix".into(), spec: with_defaults(&TaskDecl { prompt: "fix it".into(), ..TaskDecl::default() }, &d) };
        let a = start_args(&t, &RunTask { name: "fix".into(), ..RunTask::default() }, "hand").unwrap();
        assert_eq!(a.factory.as_deref(), Some("careful"), "tasks.default's factory");
        let r =
            RunTask { name: "fix".into(), over: [("factory".to_owned(), json!("default"))].into_iter().collect(), ..RunTask::default() };
        assert_eq!(start_args(&t, &r, "hand").unwrap().factory.as_deref(), Some("default"));
        let plain = NamedTask { name: "x".into(), spec: TaskDecl { prompt: "x".into(), ..TaskDecl::default() } };
        assert_eq!(start_args(&plain, &RunTask::default(), "hand").unwrap().factory, None, "none named: one agent");
    }
}
