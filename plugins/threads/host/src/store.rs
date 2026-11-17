//! wisp.threads' tables (ADR-0004, ADR-0079), forward-only. Every row is published on its task's `session:<id>`.
//! - `threads_notes`: one row per section of a task's Notes (`<task>|<section>`): its text, rev, who wrote it, when.
//! - `threads_revisions`: every earlier version of a section (`<task>|<section>|<rev>`), so a rewrite loses nothing.
//! - `threads_meta`: per task, the Notes' revision, what the agent last read (`seen`), the task's turn count when they
//!   were last written (`turn`) and as last seen (`count`).
//! - `threads_health`: per task, the thread's health as last worked out (a turn's end).

use serde_json::{Value, json};
use wisp_store::{Col, EntityDef, Migration, Query, Row, Store, Writer};

pub const OWNER: &str = "threads";

pub const MIGRATIONS: &[Migration] = &[Migration::sql(
    1,
    "CREATE TABLE threads_notes (id TEXT PRIMARY KEY, task_id TEXT NOT NULL, section TEXT NOT NULL, text TEXT NOT NULL, rev INTEGER NOT NULL, by TEXT NOT NULL, at INTEGER NOT NULL) STRICT;
     CREATE INDEX threads_notes_task ON threads_notes(task_id);
     CREATE TABLE threads_revisions (id TEXT PRIMARY KEY, task_id TEXT NOT NULL, section TEXT NOT NULL, rev INTEGER NOT NULL, text TEXT NOT NULL, by TEXT NOT NULL, at INTEGER NOT NULL) STRICT;
     CREATE INDEX threads_revisions_task ON threads_revisions(task_id, section, rev);
     CREATE TABLE threads_meta (id TEXT PRIMARY KEY, rev INTEGER NOT NULL, seen INTEGER NOT NULL, turn INTEGER NOT NULL, count INTEGER NOT NULL, at INTEGER NOT NULL) STRICT;
     CREATE TABLE threads_health (id TEXT PRIMARY KEY, body TEXT NOT NULL) STRICT;",
)];

fn task_of(r: &Row) -> String {
    let id = r.get("task_id").or_else(|| r.get("id")).and_then(Value::as_str).unwrap_or_default();
    id.split('|').next().unwrap_or_default().to_owned()
}

fn topics(r: &Row) -> Vec<wisp_store::Topic> {
    vec![wisp_threads_api::topic(&task_of(r))]
}

pub const NOTES: EntityDef = EntityDef {
    kind: "threads_notes",
    table: "threads_notes",
    key: "id",
    columns: &[
        ("id", Col::Text),
        ("task_id", Col::Text),
        ("section", Col::Text),
        ("text", Col::Text),
        ("rev", Col::Int),
        ("by", Col::Text),
        ("at", Col::Int),
    ],
    json_columns: &[],
    parent: None,
    seq: None,
    topics,
};
pub const REVISIONS: EntityDef = EntityDef {
    kind: "threads_revisions",
    table: "threads_revisions",
    key: "id",
    columns: &[
        ("id", Col::Text),
        ("task_id", Col::Text),
        ("section", Col::Text),
        ("rev", Col::Int),
        ("text", Col::Text),
        ("by", Col::Text),
        ("at", Col::Int),
    ],
    json_columns: &[],
    parent: None,
    seq: None,
    topics,
};
pub const META: EntityDef = EntityDef {
    kind: "threads_meta",
    table: "threads_meta",
    key: "id",
    columns: &[("id", Col::Text), ("rev", Col::Int), ("seen", Col::Int), ("turn", Col::Int), ("count", Col::Int), ("at", Col::Int)],
    json_columns: &[],
    parent: None,
    seq: None,
    topics,
};
pub const HEALTH: EntityDef = EntityDef {
    kind: "threads_health",
    table: "threads_health",
    key: "id",
    columns: &[("id", Col::Text), ("body", Col::Text)],
    json_columns: &["body"],
    parent: None,
    seq: None,
    topics,
};

pub fn install(store: &Store) -> wisp_store::Result<()> {
    store.migrate(OWNER, MIGRATIONS)?;
    for d in [NOTES, REVISIONS, META, HEALTH] {
        store.define_entity(d)?;
    }
    Ok(())
}

pub fn row(v: Value) -> Row {
    match v {
        Value::Object(m) => m,
        _ => Row::new(),
    }
}

/// Everything a task has here goes (the task was deleted).
pub fn forget(w: &mut Writer<'_>, task: &str) -> wisp_store::Result<()> {
    for kind in [NOTES.kind, REVISIONS.kind] {
        for r in w.query(kind, &Query::new().eq("task_id", task))? {
            if let Some(id) = r.get("id").and_then(Value::as_str) {
                w.del(kind, id.to_owned())?;
            }
        }
    }
    w.del(META.kind, task.to_owned())?;
    w.del(HEALTH.kind, task.to_owned())?;
    Ok(())
}

/// A task's meta row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Meta {
    pub rev: u32,
    pub seen: u32,
    pub turn: u32,
    pub count: u32,
    pub at: i64,
}

fn num(r: &Row, k: &str) -> u32 {
    r.get(k).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).unwrap_or(0)
}

impl Meta {
    pub fn of(r: Option<&Row>) -> Self {
        r.map_or_else(Self::default, |r| Self {
            rev: num(r, "rev"),
            seen: num(r, "seen"),
            turn: num(r, "turn"),
            count: num(r, "count"),
            at: r.get("at").and_then(Value::as_i64).unwrap_or(0),
        })
    }
    pub fn row(&self, task: &str) -> Row {
        row(json!({ "id": task, "rev": self.rev, "seen": self.seen, "turn": self.turn, "count": self.count, "at": self.at }))
    }
}
