//! A `/sandbox` launch as a row of its own from the first frame (`sandbox.start`): the task list shows it at once,
//! glowing while the sandbox comes up, step by step (made, started, the project in, the task starting), then the task
//! itself; or why it couldn't, kept until it's tried again or dismissed. A launch that needs Claude's sign-in for
//! sandboxes waits for it and goes on by itself once the sign-in is done. Rows are `sandbox.launch` on
//! [`api::LAUNCHES_TOPIC`]; they live as long as the launch needs them (a started one goes after a minute, when its
//! task's own row has long arrived).

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use wisp_kernel::{Ctx, Result, WireError};
use wisp_sandbox_api as api;
use wisp_sandbox_core as core;
use wisp_store::{Col, EntityDef, Migration, Query, Row, Store};

use crate::make::{self, Step};

const MIGRATIONS: &[Migration] = &[
    Migration::sql(
        1,
        "CREATE TABLE sandbox_v_launch (id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL, step TEXT NOT NULL, error TEXT,
       at INTEGER NOT NULL, agent TEXT NOT NULL, peer TEXT, task TEXT, args TEXT NOT NULL) STRICT;
     CREATE TABLE sandbox_v_signin (id TEXT PRIMARY KEY, state TEXT NOT NULL, link TEXT, error TEXT) STRICT;",
    ),
    // SBX-7: how the project's `.wisp/sandbox.sh` went (`{ok, tail}`), so a failed one is shown
    Migration::sql(2, "ALTER TABLE sandbox_v_launch ADD COLUMN setup TEXT;"),
];

fn topics(_: &Row) -> Vec<String> {
    vec![api::LAUNCHES_TOPIC.into()]
}

pub const LAUNCH: EntityDef = EntityDef {
    kind: "sandbox.launch",
    table: "sandbox_v_launch",
    key: "id",
    columns: &[
        ("id", Col::Text),
        ("title", Col::Text),
        // preparing (`step` says where), signin (Claude's sign-in for sandboxes first), failed (`error`), started
        ("state", Col::Text),
        ("step", Col::Text),
        ("error", Col::Text),
        ("at", Col::Int),
        ("agent", Col::Text),
        ("peer", Col::Text),
        ("task", Col::Text),
        ("args", Col::Text),
        // the setup's run once started (`api::SetupRun`), when it ran
        ("setup", Col::Text),
    ],
    json_columns: &["args", "setup"],
    parent: None,
    seq: None,
    topics,
};

/// Claude's sign-in for sandboxes as it goes (`id: claude`): idle, waiting, link, saving, done, failed.
pub const SIGNIN: EntityDef = EntityDef {
    kind: "sandbox.signin",
    table: "sandbox_v_signin",
    key: "id",
    columns: &[("id", Col::Text), ("state", Col::Text), ("link", Col::Text), ("error", Col::Text)],
    json_columns: &[],
    parent: None,
    seq: None,
    topics,
};

/// How long a started launch's row stays (the page that shows it opens the task as soon as it reads `started`).
const STARTED_KEEP_MS: u64 = 60_000;

pub struct Launches {
    store: Store,
    /// the ids whose launch is running now: one run per id
    running: Mutex<HashSet<String>>,
}

pub type Shared = Arc<Launches>;

fn running(l: &Launches) -> std::sync::MutexGuard<'_, HashSet<String>> {
    l.running.lock().unwrap_or_else(PoisonError::into_inner)
}

fn store_err(e: wisp_store::StoreError) -> WireError {
    WireError::from(e)
}

/// The table and its kinds (startup: no write yet, the store's writer may still be busy starting).
pub fn install(store: &Store) -> Result<Shared> {
    store.migrate("sandbox_view", MIGRATIONS).map_err(store_err)?;
    store.define_entity(LAUNCH).map_err(store_err)?;
    store.define_entity(SIGNIN).map_err(store_err)?;
    Ok(Arc::new(Launches { store: store.clone(), running: Mutex::default() }))
}

/// What a wisp that stopped mid-launch left: a launch still preparing failed with it (its sandbox, owned `pending`,
/// goes with the sweep), a started one's row isn't needed any more, nor a sign-in whose `claude setup-token` stopped.
pub async fn tidy(l: &Launches) {
    let r = l
        .store
        .write(|w| {
            w.del(SIGNIN.kind, "claude".to_owned())?;
            for r in w.query(LAUNCH.kind, &Query::default())? {
                let Some(id) = r.get("id").and_then(Value::as_str).map(str::to_owned) else { continue };
                match r.get("state").and_then(Value::as_str) {
                    Some("started") => {
                        w.del(LAUNCH.kind, id)?;
                    }
                    Some("preparing") => {
                        let mut r = r.clone();
                        r.insert("state".into(), json!("failed"));
                        r.insert("error".into(), json!("wisp stopped while this sandbox was being made."));
                        w.upsert(LAUNCH.kind, r)?;
                    }
                    _ => {}
                }
            }
            Ok(())
        })
        .await;
    if let Err(e) = r {
        tracing::warn!(error = %e, "sandbox: tidying the launches' rows");
    }
}

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// What the row calls the task: its title, else its prompt's first line.
pub fn title_of(a: &api::LaunchArgs) -> String {
    let t = a.title.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let line = || a.prompt.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Sandbox task");
    t.unwrap_or_else(line).chars().take(80).collect()
}

fn step_name(s: Step) -> &'static str {
    match s {
        Step::Make => "make",
        Step::Wake => "wake",
        Step::Copy => "copy",
        Step::Start => "start",
    }
}

async fn get(l: &Launches, id: &str) -> Result<Option<Row>> {
    let id = id.to_owned();
    l.store.read_async(move |r| r.get(LAUNCH.kind, id)).await.map_err(store_err)
}

async fn put(l: &Launches, row: Row) {
    if let Err(e) = l.store.write(move |w| w.upsert(LAUNCH.kind, row).map(|_| ())).await {
        tracing::warn!(error = %e, "sandbox: writing a launch's row");
    }
}

/// The Claude sign-in's row.
pub async fn put_signin(l: &Launches, s: &api::ClaudeSignIn) {
    let row = json!({ "id": "claude", "state": s.state, "link": s.link, "error": s.error });
    let Value::Object(row) = row else { return };
    if let Err(e) = l.store.write(move |w| w.upsert(SIGNIN.kind, row).map(|_| ())).await {
        tracing::warn!(error = %e, "sandbox: writing the sign-in's row");
    }
}

/// The row with some fields changed.
async fn update(l: &Launches, id: &str, fields: Value) {
    let Ok(Some(mut row)) = get(l, id).await else { return };
    if let Value::Object(m) = fields {
        row.extend(m);
    }
    put(l, row).await;
}

/// `sandbox.start`: the row now, the launch behind it.
pub async fn start(ctx: &Ctx, l: &Shared, a: api::StartArgs) -> Result<api::Empty> {
    if !valid_id(&a.id) {
        return Err(WireError::invalid("a launch's id is 1–64 letters, digits, - or _"));
    }
    if a.launch.prompt.trim().is_empty() {
        return Err(WireError::invalid("say what the task is"));
    }
    if get(l, &a.id).await?.is_some() {
        // the same launch asked again (a retried request): it's under way already
        return Ok(api::Empty {});
    }
    let args = serde_json::to_value(&a.launch).map_err(|e| WireError::internal(e.to_string()))?;
    let row = json!({
        "id": a.id, "title": title_of(&a.launch), "state": "preparing", "step": "make", "error": null,
        "at": make::now(ctx), "agent": a.launch.agent.clone().unwrap_or_default(), "peer": null, "task": null, "args": args,
    });
    if let Value::Object(r) = row {
        put(l, r).await;
    }
    spawn_run(ctx, l, a.id, a.launch);
    Ok(api::Empty {})
}

/// `sandbox.retry`: a launch that failed or waited for a sign-in, again from the start.
pub async fn retry(ctx: &Ctx, l: &Shared, id: &str) -> Result<api::Empty> {
    let row = get(l, id).await?.ok_or_else(|| WireError::not_found("That launch is gone."))?;
    if running(l).contains(id) {
        return Ok(api::Empty {});
    }
    let a: api::LaunchArgs = serde_json::from_value(row.get("args").cloned().unwrap_or(Value::Null))
        .map_err(|_| WireError::internal("that launch's task can't be read back"))?;
    update(l, id, json!({ "state": "preparing", "step": "make", "error": null, "setup": null, "at": make::now(ctx) })).await;
    spawn_run(ctx, l, id.to_owned(), a);
    Ok(api::Empty {})
}

/// `sandbox.dismiss`: a failed launch (or one waiting for a sign-in) forgotten. One under way is left alone.
pub async fn dismiss(l: &Shared, id: &str) -> Result<api::Empty> {
    if running(l).contains(id) {
        return Err(WireError::conflict("That sandbox is still being made."));
    }
    let id = id.to_owned();
    l.store.write(move |w| w.del(LAUNCH.kind, id).map(|_| ())).await.map_err(store_err)?;
    Ok(api::Empty {})
}

/// Claude's sign-in for sandboxes is done: every launch that waited for it goes on.
pub async fn signed_in(ctx: &Ctx, l: &Shared) {
    let rows = l.store.read_async(|r| r.query(LAUNCH.kind, &Query::default())).await.unwrap_or_default();
    for r in rows {
        if r.get("state").and_then(Value::as_str) == Some("signin")
            && let Some(id) = r.get("id").and_then(Value::as_str)
            && let Err(e) = retry(ctx, l, id).await
        {
            tracing::warn!(error = %e.message, "sandbox: a launch after the sign-in");
        }
    }
}

fn spawn_run(ctx: &Ctx, l: &Shared, id: String, a: api::LaunchArgs) {
    if !running(l).insert(id.clone()) {
        return;
    }
    let (c, l) = (ctx.clone(), l.clone());
    ctx.spawn(async move {
        run(&c, &l, &id, &a).await;
        running(&l).remove(&id);
    });
}

async fn run(ctx: &Ctx, l: &Shared, id: &str, a: &api::LaunchArgs) {
    let call = ctx.root_call();
    // Claude's sign-in for sandboxes first: the row says so and offers it, then goes on by itself
    match make::logins(ctx, &call).await {
        Ok(logins) => {
            if let Err(why) = core::may_start(a.agent.as_deref().unwrap_or_default(), &logins) {
                update(l, id, json!({ "state": "signin", "step": "", "error": why })).await;
                return;
            }
        }
        Err(e) => {
            update(l, id, json!({ "state": "failed", "error": e.message })).await;
            return;
        }
    }
    let ls = l.clone();
    let lid = id.to_owned();
    let step = move |s: Step| -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let (l, id) = (ls.clone(), lid.clone());
        Box::pin(async move { update(&l, &id, json!({ "step": step_name(s) })).await })
    };
    match make::launch_with(ctx, &call, a, &step).await {
        Ok(done) => {
            update(l, id, json!({ "state": "started", "peer": done.peer, "task": done.task, "error": null, "setup": done.setup })).await;
            let (c, l, id) = (ctx.clone(), l.clone(), id.to_owned());
            ctx.spawn(async move {
                if let Ok(clock) = c.clock() {
                    clock.sleep(STARTED_KEEP_MS).await;
                }
                if let Err(e) = l.store.write(move |w| w.del(LAUNCH.kind, id).map(|_| ())).await {
                    tracing::warn!(error = %e, "sandbox: a started launch's row");
                }
            });
        }
        Err(e) => update(l, id, json!({ "state": "failed", "error": e.message })).await,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_launch_is_called_by_its_title_else_its_first_line() {
        let a = api::LaunchArgs { prompt: "\n  fix the add bug  \nand more".into(), ..api::LaunchArgs::default() };
        assert_eq!(title_of(&a), "fix the add bug");
        let b = api::LaunchArgs { title: Some(" Named ".into()), ..a.clone() };
        assert_eq!(title_of(&b), "Named");
        assert!(valid_id("01a2-b_c") && !valid_id("") && !valid_id("a/b") && !valid_id(&"x".repeat(65)));
    }
}
