//! An event type's `payloadSchema` for `events/list` (ADR-0146), inferred from the newest events of that type the log
//! keeps: each field's JSON types (an integer that's sometimes a fraction is a number), objects and lists looked into
//! up to four deep, and `required` for the fields every sample had. Inferred, and said so in its `description`: wisp's
//! event structs carry no JSON Schema of their own (ADR-0044 left it to a later change), and the samples are what a
//! client will actually get.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

/// How many of a type's newest events a schema is read from, and how far back the log is read for them.
pub const SAMPLES: usize = 20;
pub const LOOK_BACK: u64 = 5_000;
const DEPTH: usize = 4;

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// The schema of `samples` (all of one field, or the payloads themselves).
pub fn infer(samples: &[&Value]) -> Value {
    infer_at(samples, 0)
}

fn infer_at(samples: &[&Value], depth: usize) -> Value {
    let mut kinds: BTreeSet<&str> = samples.iter().map(|v| kind(v)).collect();
    // an integer here and a fraction there is a number
    if kinds.contains("number") {
        kinds.remove("integer");
    }
    let mut out = Map::new();
    match kinds.len() {
        0 => {}
        1 => {
            out.insert("type".into(), json!(kinds.iter().next()));
        }
        _ => {
            out.insert("type".into(), json!(kinds.iter().collect::<Vec<_>>()));
        }
    }
    if depth >= DEPTH {
        return Value::Object(out);
    }
    let objects: Vec<&Map<String, Value>> = samples.iter().filter_map(|v| v.as_object()).collect();
    if !objects.is_empty() {
        let mut fields: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
        for o in &objects {
            for (k, v) in *o {
                fields.entry(k.as_str()).or_default().push(v);
            }
        }
        let required: Vec<&str> = fields.iter().filter(|(_, vs)| vs.len() == objects.len()).map(|(k, _)| *k).collect();
        let props: Map<String, Value> = fields.iter().map(|(k, vs)| ((*k).to_owned(), infer_at(vs, depth + 1))).collect();
        out.insert("properties".into(), Value::Object(props));
        if !required.is_empty() {
            out.insert("required".into(), json!(required));
        }
    }
    let items: Vec<&Value> = samples.iter().filter_map(|v| v.as_array()).flatten().collect();
    if !items.is_empty() {
        out.insert("items".into(), infer_at(&items, depth + 1));
    }
    Value::Object(out)
}

/// A type's `payloadSchema`: inferred from its samples (newest first), or a bare object when none is kept.
pub fn payload_schema(ty: &str, samples: &[Value]) -> Value {
    if samples.is_empty() {
        return json!({ "type": "object", "description": format!("no {ty} kept yet to read its shape from") });
    }
    let refs: Vec<&Value> = samples.iter().collect();
    let mut s = infer(&refs);
    s["description"] = json!(format!("inferred from the newest {} {ty} events wisp keeps", samples.len()));
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn fields_their_types_and_what_every_sample_has() {
        let a = json!({ "sessionId": "t1", "turnId": "u1", "cost": 1, "tags": ["x"], "by": { "who": "ada" } });
        let b = json!({ "sessionId": "t2", "cost": 0.5, "tags": [], "by": { "who": "bo", "crew": true }, "text": null });
        let s = infer(&[&a, &b]);
        assert_eq!(s["type"], "object");
        assert_eq!(s["required"], json!(["by", "cost", "sessionId", "tags"]));
        assert_eq!(s["properties"]["cost"]["type"], "number", "1 and 0.5: a number");
        assert_eq!(s["properties"]["turnId"]["type"], "string");
        assert_eq!(s["properties"]["text"]["type"], "null");
        assert_eq!(s["properties"]["tags"]["items"]["type"], "string");
        assert_eq!(s["properties"]["by"]["required"], json!(["who"]));
        assert_eq!(s["properties"]["by"]["properties"]["crew"]["type"], "boolean");
    }

    #[test]
    fn mixed_types_are_listed_and_depth_is_bounded() {
        let s = infer(&[&json!({ "x": 1 }), &json!({ "x": "one" })]);
        assert_eq!(s["properties"]["x"]["type"], json!(["integer", "string"]));
        let deep = json!({ "a": { "b": { "c": { "d": { "e": 1 } } } } });
        let s = infer(&[&deep]);
        assert!(s["properties"]["a"]["properties"]["b"]["properties"]["c"]["properties"]["d"].get("properties").is_none());
    }

    #[test]
    fn a_type_with_nothing_kept_is_a_bare_object() {
        let s = payload_schema("goal.met", &[]);
        assert_eq!(s["type"], "object");
        assert!(s.get("properties").is_none());
        let s = payload_schema("goal.met", &[json!({ "goal": "g" })]);
        assert!(s["description"].as_str().unwrap().contains("inferred from the newest 1 goal.met"));
    }
}
