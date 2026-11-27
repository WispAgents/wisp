//! What each resource was last applied as (`resources_applied`), so a plan tells drift (wisp's copy
//! changed since, in the app or by a command) from a change in the files. Written after each apply, with the source the
//! apply came from (a folder and workspace, or `-f` files), so `--prune` removes only what that source applied (AAC-1).

use serde_json::{Value, json};
use wisp_kernel::{Ctx, Result, WireError};
use wisp_store::{Col, EntityDef, Migration, Query, Row, Store};

const KIND: &str = "resources_applied";

fn topics(_: &Row) -> Vec<String> {
    vec!["plugin:wisp.resources:applied".into()]
}

const DEF: EntityDef = EntityDef {
    kind: KIND,
    table: KIND,
    key: "id",
    columns: &[("id", Col::Text), ("body", Col::Text), ("source", Col::Text)],
    json_columns: &["body"],
    parent: None,
    seq: None,
    topics,
};

/// The stores this kind is set up on: per store, not per process, so a second store in one process (a test's, or a
/// daemon's after another's) gets its table and entity too.
static READY: std::sync::Mutex<Vec<std::sync::Weak<Store>>> = std::sync::Mutex::new(Vec::new());

fn ready(s: &std::sync::Arc<Store>) -> bool {
    let mut seen = READY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    seen.retain(|w| w.strong_count() > 0);
    seen.iter().any(|w| std::ptr::eq(w.as_ptr(), std::sync::Arc::as_ptr(s)))
}

fn store(ctx: &Ctx) -> Result<std::sync::Arc<Store>> {
    let s = ctx.service::<Store>()?;
    if !ready(&s) {
        s.migrate(
            "resources.applied",
            &[
                Migration::sql(1, "CREATE TABLE resources_applied (id TEXT PRIMARY KEY, body TEXT NOT NULL) STRICT;"),
                // rows from before have none: an unknown source, which `--prune` never removes
                Migration::sql(2, "ALTER TABLE resources_applied ADD COLUMN source TEXT;"),
            ],
        )
        .map_err(|e| WireError::internal(e.to_string()))?;
        s.define_entity(DEF).map_err(|e| WireError::internal(e.to_string()))?;
        READY.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(std::sync::Arc::downgrade(&s));
    }
    Ok(s)
}

pub fn key(kind: &str, name: &str) -> String {
    format!("{kind}/{name}")
}

/// A resource's last apply: its spec, and where the formula came from (None: unknown, a row from before sources).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Baseline {
    pub spec: Value,
    pub source: Option<String>,
}

/// Every resource's last apply, by `kind/name`.
pub async fn all(ctx: &Ctx) -> Result<std::collections::HashMap<String, Baseline>> {
    let s = store(ctx)?;
    let rows = s.read_async(|r| r.query(KIND, &Query::new())).await.map_err(|e| WireError::internal(e.to_string()))?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let source = r.get("source").and_then(Value::as_str).map(str::to_owned);
            Some((r.get("id")?.as_str()?.to_owned(), Baseline { spec: r.get("body")?.clone(), source }))
        })
        .collect())
}

/// Sets (`Some`) or forgets (`None`) these baselines, in one write.
pub async fn set(ctx: &Ctx, items: Vec<(String, Option<Baseline>)>) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let s = store(ctx)?;
    s.write(move |w| {
        for (id, spec) in items {
            match spec {
                Some(b) => {
                    let mut r = Row::new();
                    r.insert("id".into(), json!(id));
                    r.insert("body".into(), b.spec);
                    r.insert("source".into(), b.source.map_or(Value::Null, Value::String));
                    w.upsert(KIND, r)?;
                }
                None => {
                    w.del(KIND, id.as_str())?;
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| WireError::internal(e.to_string()))
}
