//! `runtime.slots` (perf audit 2026-11-18 #3–#5): plugins' `ext` slots of every task, from one query of the session
//! table, each body read only as far as its `ext` (borrowed, never a whole `Session`). Was: `runtime.get` per task,
//! 5,000 store reads to answer worktrees' pane.
//!
//! Its own slot (`wisp.runtime`) goes out reduced to what carries no text: the compare `group`, and each side
//! question's `pending` and `at` (a question's words could quote a .env value, and `runtime.get` masks that slot).
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{Map, Value, json};
use wisp_runtime_api::commands::{SlotsArgs, TaskSlots};

const RUNTIME: &str = "wisp.runtime";

#[derive(Deserialize)]
struct Body<'a> {
    #[serde(borrow, default)]
    ext: Option<BTreeMap<&'a str, &'a RawValue>>,
}

#[derive(Deserialize)]
struct Row<'a> {
    id: String,
    #[serde(borrow)]
    body: Option<&'a RawValue>,
}

/// `wisp.runtime`'s slot as `runtime.slots` tells it: the facts other plugins read, no text.
fn runtime_facts(v: &Value) -> Value {
    let mut out = Map::new();
    if let Some(g) = v.get("group").filter(|g| g.is_string()) {
        out.insert("group".into(), g.clone());
    }
    if let Some(btw) = v.get("btw").and_then(Value::as_array) {
        let sides: Vec<Value> = btw.iter().map(|x| json!({ "pending": x.get("pending").cloned().unwrap_or(Value::Bool(false)), "at": x.get("at").cloned().unwrap_or(Value::Null) })).collect();
        out.insert("btw".into(), Value::Array(sides));
    }
    Value::Object(out)
}

/// The rows of `SELECT id, body` (as `Reader::query_json` writes them) as each task's slots of `a.plugins`.
pub fn of(text: &str, a: &SlotsArgs) -> serde_json::Result<Vec<TaskSlots>> {
    let rows: Vec<Row<'_>> = serde_json::from_str(text)?;
    let mut out = Vec::new();
    for r in rows {
        let ext = r.body.and_then(|b| serde_json::from_str::<Body<'_>>(b.get()).ok()).and_then(|b| b.ext).unwrap_or_default();
        let mut slots = BTreeMap::new();
        for p in &a.plugins {
            let Some(raw) = ext.get(p.as_str()) else { continue };
            let v: Value = serde_json::from_str(raw.get())?;
            if v.is_null() {
                continue;
            }
            slots.insert(p.clone(), if p == RUNTIME { runtime_facts(&v) } else { v });
        }
        if a.all || !slots.is_empty() {
            out.push(TaskSlots { id: r.id.into(), ext: slots });
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn rows() -> String {
        json!([
            { "id": "a", "body": { "title": "x", "ext": { "wisp.worktrees": { "path": "/w/a" }, "wisp.ship": { "pr": { "number": 3 } }, "wisp.other": 1 } } },
            { "id": "b", "body": { "title": "y" } },
            { "id": "c", "body": { "ext": { "wisp.runtime": { "group": "g1", "btw": [{ "pending": true, "at": 5, "q": "is sk-live-1 right?" }], "note": "secret" } } } }
        ])
        .to_string()
    }

    #[test]
    fn only_tasks_with_a_slot_asked_for_and_only_those_slots() {
        let a = SlotsArgs { plugins: vec!["wisp.worktrees".into(), "wisp.ship".into()], all: false };
        let got = of(&rows(), &a).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id.as_str(), "a");
        assert_eq!(got[0].ext.keys().collect::<Vec<_>>(), ["wisp.ship", "wisp.worktrees"]);
        let all = of(&rows(), &SlotsArgs { all: true, ..a }).unwrap();
        assert_eq!(all.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    }

    #[test]
    fn runtimes_own_slot_goes_out_without_its_text() {
        let got = of(&rows(), &SlotsArgs { plugins: vec![RUNTIME.into()], all: false }).unwrap();
        assert_eq!(got[0].ext[RUNTIME], json!({ "group": "g1", "btw": [{ "pending": true, "at": 5 }] }));
        assert!(!serde_json::to_string(&got).unwrap().contains("sk-live"));
    }
}
