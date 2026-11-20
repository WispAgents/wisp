//! wisp.factory host (ADR-0114): the `factory` resource kind (`factories:` in a formula, kept in `factory_def`, a
//! built-in `default` beside them) and the runs.
//!
//! A task made with `factory: <name>` is a run: wisp.factory's `before:create` reads the name and changes the args (it
//! writes nothing): the task is made held, never runs an agent and makes no worktree; a name no factory has is
//! refused, saying which there are. Once the task exists (`session.created`), the driver ([`drive`]) makes each
//! station's task as its child and does what wisp-factory-core's machine says, keeping where it stands on the parent
//! (`ext["wisp.factory"]`) and its list row (`runtime.summary`'s `factory`).
//!
//! size-ok: one plugin's setup: its kind, commands, `before:create`, `before:run`, summary field and listeners

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::sync::broadcast;
use wisp_factory_api::{
    DEFAULT, EXT, FactoryRow, FactorySpec, List, NamedFactory, Names, Remove, RunState, Set, Stop, builtin_default, check,
};
use wisp_kernel::{BoxFuture, Call, Failure, InterceptOpts, Manifest, Plugin, Result, Setup, Verdict, WireError};
use wisp_resources_api::{CrudKind, KindInfo, RESOURCE_KIND, Resource, ResourceKind, schema_of};
use wisp_runtime_api::commands::StartArgs;
use wisp_runtime_api::events::{SessionCreated, SessionDeleted, TurnEnded};
use wisp_runtime_api::hooks::{BeforeCreate, BeforeRun};
use wisp_store::{Col, EntityDef, Migration, Query, Row, Store};

pub use wisp_factory_api::PLUGIN_ID;

mod drive;

const OWNER: &str = "factory";
const MIGRATIONS: &[Migration] = &[Migration::sql(1, "CREATE TABLE factory_def (id TEXT PRIMARY KEY, body TEXT NOT NULL) STRICT;")];

fn topics(_: &Row) -> Vec<String> {
    vec!["plugin:wisp.factory:factories".into()]
}

const DEF: EntityDef = EntityDef {
    kind: "factory_def",
    table: "factory_def",
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

/// The declared factories, as `before:create` reads them (no store read per task).
type Declared = Arc<Mutex<Vec<NamedFactory>>>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

async fn list(store: &Store) -> Result<Vec<NamedFactory>> {
    let rows = store.read_async(|r| r.query(DEF.kind, &Query::new())).await.map_err(store_err)?;
    let mut out: Vec<NamedFactory> = rows.into_iter().filter_map(|r| serde_json::from_value(r.get("body")?.clone()).ok()).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn named(name: &str) -> std::result::Result<(), String> {
    let ok = !name.trim().is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_alphanumeric() || "-_.".contains(c));
    if ok { Ok(()) } else { Err(format!("a factory's name is letters, digits, - _ . (not {name:?})")) }
}

async fn put(store: &Store, declared: &Declared, f: NamedFactory) -> Result<NamedFactory> {
    named(&f.name).map_err(WireError::invalid)?;
    let problems = check(&f.spec);
    if !problems.is_empty() {
        return Err(WireError::invalid(format!("factories.{}: {}", f.name, problems.join("; "))));
    }
    let body = serde_json::to_value(&f).map_err(|e| WireError::internal(e.to_string()))?;
    let id = f.name.clone();
    store
        .write(move |w| {
            let mut r = Row::new();
            r.insert("id".into(), json!(id));
            r.insert("body".into(), body);
            w.upsert(DEF.kind, r).map(|_| ())
        })
        .await
        .map_err(store_err)?;
    *lock(declared) = list(store).await?;
    Ok(f)
}

/// Every factory a run can name: the declared ones, and the built-in `default` unless one is declared by that name.
pub fn rows(declared: &[NamedFactory]) -> Vec<FactoryRow> {
    let mut out: Vec<FactoryRow> = declared
        .iter()
        .map(|f| FactoryRow {
            name: f.name.clone(),
            description: f.spec.description.clone(),
            builtin: false,
            stations: f.spec.station_ids().iter().map(|s| (*s).to_owned()).collect(),
        })
        .collect();
    if !declared.iter().any(|f| f.name == DEFAULT) {
        let d = builtin_default();
        out.insert(
            0,
            FactoryRow {
                name: DEFAULT.into(),
                description: d.description.clone(),
                builtin: true,
                stations: d.station_ids().iter().map(|s| (*s).to_owned()).collect(),
            },
        );
    }
    out
}

/// The factory a run names ('' is `default`): a declared one, else the built-in default; else why not, naming them.
pub fn resolve(declared: &[NamedFactory], name: &str) -> std::result::Result<(String, FactorySpec), String> {
    let name = if name.trim().is_empty() { DEFAULT } else { name.trim() };
    if let Some(f) = declared.iter().find(|f| f.name == name) {
        return Ok((f.name.clone(), f.spec.clone()));
    }
    if name == DEFAULT {
        return Ok((DEFAULT.into(), builtin_default()));
    }
    let names: Vec<String> = rows(declared).into_iter().map(|r| r.name).collect();
    Err(format!("no factory named {name}: factories has {}", names.join(", ")))
}

/// A task's goal as the run reads it (ADR-0110: wisp.goal's `before:create` resolved a declared one already).
fn goal_of(g: &Value) -> Option<wisp_factory_core::Goal> {
    let text = match g {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.get("text").and_then(Value::as_str).unwrap_or_default().to_owned(),
        _ => return None,
    };
    let judges = g
        .get("judges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|j| j.as_str().map(str::to_owned).or_else(|| j.get("agent").and_then(Value::as_str).map(str::to_owned)))
        .filter(|j| !j.is_empty())
        .collect();
    let vote = g.get("vote").and_then(Value::as_str).unwrap_or_default().to_owned();
    let max_rounds = g.get("maxRounds").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
    (!text.trim().is_empty() || !Vec::<String>::is_empty(&judges)).then_some(wisp_factory_core::Goal { text, judges, vote, max_rounds })
}

/// `before:create` for a task that names a factory: it becomes the run's parent (held, no worktree, no terminal, its
/// goal and steps the run's), keeping what the run needs in `ext["wisp.factory"]`. Err: the refusal's words.
pub fn prepare(a: &mut StartArgs, declared: &[NamedFactory], by_agent: bool) -> std::result::Result<bool, String> {
    let Some(asked) = a.factory.clone() else { return Ok(false) };
    // a formula's factories (`--formula`, a task file's, ADR-0111) come first: the task runs under it, nothing applied
    let mut known = overlay_factories(a.formula.as_ref())?;
    let more: Vec<NamedFactory> = declared.iter().filter(|d| !known.iter().any(|k| k.name == d.name)).cloned().collect();
    known.extend(more);
    let (name, spec) = resolve(&known, &asked)?;
    if a.terminal {
        return Err("a factory run has no terminal of its own: its stations run in wisp".into());
    }
    let mut prompt = a.prompt.trim().to_owned();
    for s in a.steps.drain(..) {
        prompt.push_str("\n\nThen: ");
        prompt.push_str(s.trim());
    }
    let mode = a.mode.and_then(|m| serde_json::to_value(m).ok()).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
    let input = json!({
        "factory": name, "spec": spec, "prompt": prompt, "agent": a.agent.clone().unwrap_or_default(), "mode": mode,
        "worktree": a.worktree, "goal": a.goal.take(), "byAgent": by_agent,
    });
    a.factory = Some(name.clone());
    a.hold = true;
    a.worktree = Some(false);
    a.ext.insert(EXT.into(), json!({ "factory": name, "state": "starting", "input": input }));
    Ok(true)
}

/// The factories a task's formula overlay declares (`factories: [{ name, stations, … }]`), each checked.
fn overlay_factories(formula: Option<&Value>) -> std::result::Result<Vec<NamedFactory>, String> {
    let list = formula.and_then(|f| f.get("factories")).and_then(Value::as_array).cloned().unwrap_or_default();
    list.into_iter()
        .map(|v| {
            let f: NamedFactory = serde_json::from_value(v).map_err(|e| format!("factories: {e}"))?;
            let problems = check(&f.spec);
            if problems.is_empty() { Ok(f) } else { Err(format!("factories.{}: {}", f.name, problems.join("; "))) }
        })
        .collect()
}

/// `before:create`'s options (review r194 final pass #3): it fails closed. A task that names a factory is never made as
/// a plain agent's task because the factory's guard failed; it answers at once (a lookup in memory), well inside 500 ms.
pub const CREATE_GUARD: InterceptOpts = InterceptOpts { priority: 20, failure: Failure::Closed, timeout_ms: 500 };

/// What a create is told when reading its factory failed.
pub const CREATE_FAILED: &str = "wisp couldn't read the factory this task names, so nothing was started: try again.";

/// What `before:create` said: a refused factory is the create refused; a guard that broke refuses it in plain words.
fn intercept(declared: &[NamedFactory], by_agent: bool, a: StartArgs) -> Verdict<StartArgs> {
    guarded(|a| prepare(a, declared, by_agent), a)
}

/// `run` on `a`, any panic in it a refusal saying so (never a plain agent's task in its place).
pub fn guarded(run: impl FnOnce(&mut StartArgs) -> std::result::Result<bool, String>, mut a: StartArgs) -> Verdict<StartArgs> {
    let refuse = |message: String| Verdict::Veto { code: "invalid".into(), message, then: Vec::new() };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut a))) {
        Ok(Ok(true)) => Verdict::Modify { ctx: a },
        Ok(Ok(false)) => Verdict::Continue,
        Ok(Err(why)) => refuse(why),
        Err(_) => refuse(CREATE_FAILED.into()),
    }
}

/// A run wisp was stopped in the middle of: its slot says so and it ends (picking it back up is not yet).
pub fn interrupted(slot: &Value) -> Option<Value> {
    let state = slot.get("state").and_then(Value::as_str)?;
    if !matches!(state, "starting" | "running" | "waiting") {
        return None;
    }
    let mut v = slot.clone();
    v["state"] = json!(RunState::Stopped);
    v["station"] = json!("");
    v["waiting"] = Value::Null;
    v["why"] = json!("wisp stopped while this ran: run it again to start it over");
    if let Some(list) = v.get_mut("stations").and_then(Value::as_array_mut) {
        for s in list {
            if matches!(s.get("state").and_then(Value::as_str), Some("running" | "asking")) {
                s["state"] = json!("failed");
                s["why"] = json!("wisp stopped");
            }
        }
    }
    Some(v)
}

/// `runtime.summary@1`: a run's line on its list row.
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
        // a station's own task carries `{ run, station }`: its row says which run it's a station of
        if let Some(run) = slot.get("run").and_then(Value::as_str) {
            out.insert("factoryStation".into(), json!({ "run": run, "station": slot.get("station").cloned().unwrap_or(Value::Null) }));
            return;
        }
        let view: wisp_factory_api::RunView = match serde_json::from_value(slot.clone()) {
            Ok(v) => v,
            // just made: no view yet
            Err(_) => wisp_factory_api::RunView {
                factory: slot.get("factory").and_then(Value::as_str).unwrap_or(DEFAULT).to_owned(),
                ..wisp_factory_api::RunView::default()
            },
        };
        out.insert("factory".into(), wisp_factory_core::summary_of(&view));
    }
}

/// The `factory` resource kind: checked when it's applied; `default` is always a name a reference may use.
struct FactoryKind(CrudKind);

impl ResourceKind for FactoryKind {
    fn info(&self) -> KindInfo {
        self.0.info()
    }
    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        self.0.read(call)
    }
    fn check(&self, r: &Resource) -> Vec<String> {
        if let Err(why) = named(&r.name) {
            return vec![why];
        }
        match serde_json::from_value::<FactorySpec>(r.spec.clone()) {
            Ok(f) => check(&f),
            Err(e) => vec![e.to_string()],
        }
    }
    fn also_named<'a>(&'a self, _call: &'a Call) -> BoxFuture<'a, Vec<String>> {
        Box::pin(async { vec![DEFAULT.to_owned()] })
    }
    fn describe(&self, op: &wisp_resources_api::Op) -> Option<String> {
        let verb = match op.op {
            wisp_resources_api::OpKind::Create => "adds",
            wisp_resources_api::OpKind::Update => "changes",
            wisp_resources_api::OpKind::Delete => "removes",
        };
        let stations = serde_json::from_value::<FactorySpec>(op.spec.clone()).map(|f| f.station_ids().join(" → ")).unwrap_or_default();
        Some(format!("{verb} the factory {} ({stations}); its checks run their commands in a task's worktree", op.name))
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a wisp_resources_api::Op) -> BoxFuture<'a, Result<()>> {
        self.0.apply(call, op)
    }
}

/// The runs going on: (task, cost, turn) of every turn that ended, and the Stop asked of each run.
pub(crate) struct State {
    pub ended: broadcast::Sender<(String, f64, String)>,
    pub stops: Mutex<HashMap<String, String>>,
    pub declared: Declared,
    /// every run's own task this wisp knows (made here, or found at start): none of them ever runs an agent
    pub runs: Mutex<HashSet<String>>,
    /// each run's running check, killed by a Stop or a delete (review r194 factories #4)
    pub cancels: Mutex<HashMap<String, wisp_proc::Scope>>,
    /// the runs are known (`recover` read them at start): until then a turn's task is read to tell
    pub ready: std::sync::atomic::AtomicBool,
}

/// Whether a turn on a task is refused (review r194 factories #5, fail closed): a run's own task this wisp knows, or,
/// before the runs are known, a task read as a run's, or one that couldn't be read (`read`: None when it isn't needed).
pub fn refuse_turn(known_run: bool, ready: bool, read: impl FnOnce() -> Option<std::result::Result<bool, String>>) -> bool {
    if known_run {
        return true;
    }
    if ready {
        return false;
    }
    !matches!(read(), Some(Ok(false)))
}

/// What `before:run` says to a turn on a run's own task: refused, saying where the work goes.
pub const NO_TURN: &str = "This task is a factory run: its stations do the work. Answer its question here, or open a station's task (the band's ⋯) to follow it up.";

/// `before:run` on a run's own task (review r194 factories #5): refused, failing closed (a follow-up typed there, Start now
/// on its held prompt): its stations do the work.
fn guard_turns(s: &mut Setup, st: &Arc<State>) {
    let guard = st.clone();
    s.intercept::<BeforeRun, _, _>(InterceptOpts { priority: 5, failure: Failure::Closed, timeout_ms: 1_000 }, move |ctx, call, c| {
        let known = lock(&guard.runs).contains(c.session_id.as_str());
        let ready = guard.ready.load(std::sync::atomic::Ordering::SeqCst);
        let id = c.session_id.clone();
        async move {
            // before the runs are known (wisp just started), the task itself says: a read error refuses it
            let read = if known || ready {
                None
            } else {
                Some(
                    ctx.run::<wisp_runtime_api::commands::Get>(&call, wisp_runtime_api::commands::IdArgs { id })
                        .await
                        .map(|t| t.ext.get(EXT).is_some_and(|v| v.get("factory").is_some()))
                        .map_err(|e| e.message),
                )
            };
            if refuse_turn(known, ready, || read) {
                Verdict::Veto { code: "invalid".into(), message: NO_TURN.into(), then: Vec::new() }
            } else {
                Verdict::Continue
            }
        }
    });
}

#[derive(Debug, Default)]
pub struct FactoryPlugin;

impl Plugin for FactoryPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        use wisp_resources_api::Names as Ref;
        let info = KindInfo::new("factory", "factories", PLUGIN_ID, schema_of::<FactorySpec>())
            .order(88)
            .names(Ref::new("stations.spec.agent", "agent"))
            .names(Ref::new("stations.build.agent", "agent"))
            .names(Ref::new("stations.build.agents[]", "agent"))
            .names(Ref::new("stations.review.agent", "agent"))
            .names(Ref::new("stations.review.agents[]", "agent"));
        let kind: Arc<dyn ResourceKind> = Arc::new(FactoryKind(
            CrudKind::new::<FactorySpec>(info, s.ctx(), "factory.list", "factory.set", "factory.set", "factory.remove")
                .keyed("name", "name", "name"),
        ));
        s.contribute(&RESOURCE_KIND, kind);
        s.contribute(&wisp_runtime_api::points::SUMMARY, Arc::new(Summary) as Arc<dyn wisp_runtime_api::points::SummaryField>);
        let st = Arc::new(State {
            ended: broadcast::channel(256).0,
            stops: Mutex::new(HashMap::new()),
            declared: Arc::default(),
            runs: Mutex::default(),
            cancels: Mutex::default(),
            ready: std::sync::atomic::AtomicBool::new(false),
        });

        s.command::<List, _, _>(|ctx, _, _| async move { list(&*ctx.service::<Store>()?).await });
        let d = st.declared.clone();
        s.command::<Names, _, _>(move |_ctx, _, _| {
            let d = d.clone();
            async move { Ok(rows(&lock(&d))) }
        });
        let d = st.declared.clone();
        s.command::<Set, _, _>(move |ctx, _, f| {
            let d = d.clone();
            async move { put(&*ctx.service::<Store>()?, &d, f).await }
        });
        let d = st.declared.clone();
        s.command::<Remove, _, _>(move |ctx, _, a| {
            let d = d.clone();
            async move {
                let store = ctx.service::<Store>()?;
                store.write(move |w| w.del(DEF.kind, a.name.as_str()).map(|_| ())).await.map_err(store_err)?;
                *lock(&d) = list(&store).await?;
                Ok(json!({ "ok": true }))
            }
        });
        let stop = st.clone();
        s.command::<Stop, _, _>(move |ctx, call, a| {
            let st = stop.clone();
            async move { drive::stop(&ctx, &call, &st, &a.id, "you stopped it").await }
        });

        let d = st.declared.clone();
        s.intercept::<BeforeCreate, _, _>(CREATE_GUARD, move |_ctx, call, a| {
            let declared = lock(&d).clone();
            let by_agent = call.caller().set.starts_with("agent:");
            async move { intercept(&declared, by_agent, a) }
        });

        // a follow-up typed on a run's own task (or Start now on its held prompt) would run an agent there: refused
        guard_turns(s, &st);
        let made = st.clone();
        s.on::<SessionCreated, _, _>(move |ctx, e| {
            let st = made.clone();
            // only a run's own task (its summary says `factory`) is driven; a station's task names its run instead
            let run = e.session.fields.contains_key("factory").then(|| e.session.id.as_str().to_owned());
            if let Some(id) = run {
                lock(&st.runs).insert(id.clone());
                let c = ctx.clone();
                ctx.spawn(async move { drive::run(c, st, id).await });
            }
            async {}
        });
        let ended = st.clone();
        s.on::<TurnEnded, _, _>(move |_ctx, e| {
            // no run listening is fine: nothing waits on this turn
            let _ = ended.ended.send((e.session_id.as_str().to_owned(), e.cost_usd.unwrap_or(0.0), e.turn_id.as_str().to_owned()));
            async {}
        });
        let gone = st.clone();
        s.on::<SessionDeleted, _, _>(move |ctx, e| {
            let id = e.id.as_str().to_owned();
            // a deleted run stops at once (its check killed) and its stations go with it, as a task's worktree does
            if lock(&gone.runs).remove(&id) {
                drive::halt(&gone, &id, "its task was deleted");
                let c = ctx.clone();
                ctx.spawn(async move { drive::delete_stations(&c, &id).await });
            }
            async {}
        });
        let start = st;
        s.on_start(move |ctx| async move {
            let store = ctx.service::<Store>()?;
            store.migrate(OWNER, MIGRATIONS).map_err(store_err)?;
            store.define_entity(DEF).map_err(store_err)?;
            *lock(&start.declared) = list(&store).await?;
            let (c, st) = (ctx.clone(), start.clone());
            ctx.spawn(async move { drive::recover(&c, &st).await });
            Ok(())
        });
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
