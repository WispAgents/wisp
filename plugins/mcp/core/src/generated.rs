//! MCP tools generated from the catalogue (ADR-0040): one per command the caller may run, named after it
//! (`runtime.create` → `runtime_create`), its input schema made from its argument type. A command marked
//! `uiOnly` is left out, and a generated tool never takes one of wisp's own tools' names.

use serde_json::{Map, Value, json};
use wisp_kernel::CommandInfo;
use wisp_kernel::decl::{ArgSpec, Kind, parse};

/// The tool name of a command id.
pub fn tool_name(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}

/// A JSON Schema for a command's arguments.
pub fn schema_of(spec: &ArgSpec) -> Value {
    if spec.none {
        return json!({ "type": "object", "properties": {} });
    }
    if !spec.object {
        return json!({ "type": "object", "properties": { "args": { "description": ARGS_DOC } } });
    }
    let mut props = Map::new();
    let mut required = Vec::new();
    for f in &spec.fields {
        let mut p = match &f.kind {
            Kind::Str => json!({ "type": "string" }),
            Kind::Num => json!({ "type": "number" }),
            Kind::Bool => json!({ "type": "boolean" }),
            Kind::List => json!({ "type": "array", "items": { "type": "string" } }),
            Kind::Choice(c) => json!({ "type": "string", "enum": c }),
            Kind::Json => json!({}),
        };
        if !f.doc.is_empty() {
            p["description"] = json!(f.doc);
        }
        if !f.optional {
            required.push(json!(f.name));
        }
        props.insert(f.name.clone(), p);
    }
    let mut s = json!({ "type": "object", "properties": props });
    if !required.is_empty() {
        s["required"] = Value::Array(required);
    }
    s
}

/// (tool name, command id, read only, the tool as `tools/list` shows it) for each command of `catalogue`
/// that's for MCP, skipping names in `taken`.
pub fn tools(catalogue: &[CommandInfo], taken: &[&str]) -> Vec<(String, String, bool, Value)> {
    catalogue
        .iter()
        .filter(|c| c.ui_only.is_none())
        .filter_map(|c| {
            let name = tool_name(&c.id);
            if taken.contains(&name.as_str()) {
                return None;
            }
            let spec = parse(c.args.as_deref());
            let mut desc = c.title.clone().unwrap_or_else(|| c.id.clone());
            if !spec.doc.is_empty() {
                desc = format!("{desc}. {}", spec.doc);
            }
            let mut schema = schema_of(&spec);
            // every tool runs on another machine too (ADR-0041): `peer`, unless the command has a field of that name
            if !spec.fields.iter().any(|f| f.name == PEER)
                && let Some(props) = schema.get_mut("properties").and_then(Value::as_object_mut)
            {
                props.insert(PEER.into(), json!({ "type": "string", "description": "run it on that machine (its name or id in machines); this wisp when left out" }));
            }
            let tool = json!({ "name": name, "description": format!("{desc} (wisp command {})", c.id), "inputSchema": schema, "annotations": { "readOnlyHint": c.read_only } });
            Some((name, c.id.clone(), c.read_only, tool))
        })
        .collect()
}

/// The argument every generated tool takes to run on another machine.
pub const PEER: &str = "peer";

/// What a tool whose command declares no argument type says of `args`.
pub const ARGS_DOC: &str = "the command's arguments, as JSON: an object of its fields, or left out when it takes none (its fields may also be given beside `args`, at the top level)";

/// A tool call's arguments as the command's: the object as it is (without `peer`, unless the command has
/// its own), or `args` for a non-object type. Without `args`, the call's own fields are the arguments (`{}` when there
/// are none): a tool called as its schema allows, with nothing, got `null` and the command's type error back
/// (`crew.list: invalid type: null, expected struct NoArgs`, audit D6).
pub fn args_of(spec_object: bool, input: &Value) -> Value {
    if spec_object {
        return input.clone();
    }
    match input.get("args") {
        Some(a) => a.clone(),
        None => match input {
            Value::Object(_) => input.clone(),
            _ => json!({}),
        },
    }
}

/// A generated tool call's arguments as the command takes them (audit E10, D6): `args`, the documented form (`ARGS_DOC`),
/// works for every tool, not only those with no declared fields (`crew_agent_list {"args":{"session":"x"}}` answered
/// "missing field `session`"); the fields given beside it win. A run's own task (`session`): the field a task's agent
/// command names its caller's task by is filled from the call when left out (the agent's token speaks for that task only,
/// so asking for it was a field nobody could fill but with that one value).
pub fn call_args(spec: &wisp_kernel::decl::ArgSpec, input: &Value, session: Option<&str>) -> Value {
    if !spec.object || spec.none {
        return args_of(spec.object, input);
    }
    let has = |n: &str| spec.fields.iter().any(|f| f.name == n);
    let mut v = match input {
        Value::Object(o) => o.clone(),
        _ => Map::new(),
    };
    if !has("args")
        && let Some(Value::Object(inner)) = v.remove("args")
    {
        for (k, x) in inner {
            v.entry(k).or_insert(x);
        }
    }
    if let Some(sid) = session.filter(|s| !s.is_empty()) {
        for name in ["session", "sessionId"] {
            if has(name) && !v.contains_key(name) {
                v.insert(name.into(), json!(sid));
            }
        }
    }
    Value::Object(v)
}

/// A command's refusal as a tool's words: serde's "missing field `id`" says which field and what it is (audit D6:
/// "crew.get: missing field `id`").
pub fn said(id: &str, spec: &wisp_kernel::decl::ArgSpec, message: &str) -> String {
    let Some(name) = message.split("missing field `").nth(1).and_then(|r| r.split('`').next()) else {
        return message.to_owned();
    };
    let doc = spec.fields.iter().find(|f| f.name == name).map(|f| f.doc.trim()).filter(|d| !d.is_empty());
    let all: Vec<&str> = spec.fields.iter().filter(|f| !f.optional).map(|f| f.name.as_str()).collect();
    let what = doc.map_or_else(String::new, |d| format!(" ({d})"));
    let needs = if all.len() > 1 { format!(" It takes {}.", all.join(", ")) } else { String::new() };
    format!("{} needs `{name}`{what}.{needs}", tool_name(id))
}

/// A tool's answer: a `null` answer says it found nothing (audit D6: `stuff_get {"id":"x"}` answered `null`).
pub fn answer_text(v: &Value) -> String {
    if v.is_null() { "null: nothing found for those arguments".to_owned() } else { v.to_string() }
}

/// A command's answer that is a refusal in its own shape (`{"error": "…"}` with no `text`, as `later.agent.schedule`
/// answers `{}`): its words, so the tool result says isError as the other refusals do (audit F orchestration F7).
pub fn refusal(v: &Value) -> Option<String> {
    let o = v.as_object()?;
    let error = o.get("error")?.as_str().filter(|e| !e.trim().is_empty())?;
    o.get("text").is_none_or(Value::is_null).then(|| error.to_owned())
}

/// The machine a call names (`peer`), and the arguments without it. `own_peer`: the command has a `peer`
/// field of its own, so it's left alone.
pub fn split_peer(input: &Value, own_peer: bool) -> (Option<String>, Value) {
    if own_peer {
        return (None, input.clone());
    }
    let mut v = input.clone();
    let peer = v.as_object_mut().and_then(|o| o.remove(PEER)).and_then(|p| p.as_str().map(str::to_owned)).filter(|p| !p.is_empty());
    (peer, v)
}

/// The peer `on` names in `machines.list`: by id, else by name (any case).
pub fn find_peer(list: &Value, on: &str) -> Option<String> {
    let all = list.as_array()?;
    let s = |p: &Value, k: &str| p.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    all.iter().find(|p| s(p, "id") == on).or_else(|| all.iter().find(|p| s(p, "name").eq_ignore_ascii_case(on))).map(|p| s(p, "id"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Audit E10: the documented `args` form for a tool with declared fields, and a run's own task filled in.
    #[test]
    fn args_given_as_args_and_a_runs_own_task_are_taken_audit_e10() {
        let spec = parse(Some("{ session: string, limit?: number, }"));
        assert_eq!(call_args(&spec, &json!({ "args": { "session": "x" } }), None), json!({ "session": "x" }));
        assert_eq!(
            call_args(&spec, &json!({ "args": { "session": "x", "limit": 2 }, "limit": 5 }), None),
            json!({ "session": "x", "limit": 5 })
        );
        assert_eq!(call_args(&spec, &json!({}), Some("run1")), json!({ "session": "run1" }));
        assert_eq!(
            call_args(&spec, &json!({ "session": "other" }), Some("run1")),
            json!({ "session": "other" }),
            "the kernel's scope refuses it"
        );
        // a command with a field named args keeps it
        let own = parse(Some("{ args: string, }"));
        assert_eq!(call_args(&own, &json!({ "args": { "a": 1 } }), None), json!({ "args": { "a": 1 } }));
        // no declared fields: as before
        assert_eq!(call_args(&parse(None), &json!({ "args": { "id": "y" } }), Some("run1")), json!({ "id": "y" }));
    }

    /// Audit D6: serde's words become the tool's: which field, what it is, what the tool takes.
    #[test]
    fn a_missing_field_is_said_in_words_audit_d6() {
        let spec = parse(Some("{ \n/** the member's id or name */ id: string, }"));
        assert_eq!(said("crew.get", &spec, "crew.get: missing field `id`"), "crew_get needs `id` (the member's id or name).");
        let two = parse(Some("{ id: string, prompt: string, cron?: string, }"));
        assert_eq!(said("crew.assign", &two, "missing field `prompt` at line 1"), "crew_assign needs `prompt`. It takes id, prompt.");
        assert_eq!(said("x.y", &two, "That task is gone."), "That task is gone.");
        assert_eq!(answer_text(&Value::Null), "null: nothing found for those arguments");
        assert_eq!(answer_text(&json!({ "a": 1 })), "{\"a\":1}");
    }

    #[test]
    fn mcp_tools_take_a_peer() {
        let t = tools(&[cmd("runtime.get", Some("{ id: string }"), None)], &[]);
        assert_eq!(t[0].3["inputSchema"]["properties"]["peer"]["type"], "string");
        assert_eq!(t[0].3["inputSchema"]["required"], json!(["id"]));
        let (p, a) = split_peer(&json!({ "id": "x", "peer": "studio" }), false);
        assert_eq!((p.as_deref(), a), (Some("studio"), json!({ "id": "x" })));
        assert_eq!(split_peer(&json!({ "peer": "studio" }), true).0, None);
        let list = json!([{ "id": "aa11bb", "name": "Studio" }, { "id": "cc22dd", "name": "Cloud" }]);
        assert_eq!(
            (find_peer(&list, "studio").as_deref(), find_peer(&list, "cc22dd").as_deref(), find_peer(&list, "x")),
            (Some("aa11bb"), Some("cc22dd"), None)
        );
    }

    fn cmd(id: &str, args: Option<&str>, ui: Option<&str>) -> CommandInfo {
        CommandInfo {
            id: id.into(),
            plugin: "p".into(),
            title: Some(format!("Do {id}")),
            read_only: false,
            permission: "x:y".into(),
            args: args.map(str::to_owned),
            ui_only: ui.map(str::to_owned),
        }
    }

    #[test]
    fn the_input_schema_follows_the_args_type() {
        let c = cmd(
            "history.track",
            Some(
                "/** Track a task */ { \n/** the task */ id: string, level?: \"full\" | \"report\" | \"spec\", n?: number, quiet?: boolean, tags?: Array<string>, extra?: Foo, }",
            ),
            None,
        );
        let t = &tools(&[c], &[])[0];
        assert_eq!(t.0, "history_track");
        let s = &t.3["inputSchema"];
        assert_eq!(s["required"], json!(["id"]));
        assert_eq!(s["properties"]["id"], json!({ "type": "string", "description": "the task" }));
        assert_eq!(s["properties"]["level"]["enum"], json!(["full", "report", "spec"]));
        assert_eq!(
            (s["properties"]["n"]["type"].clone(), s["properties"]["quiet"]["type"].clone(), s["properties"]["tags"]["type"].clone()),
            (json!("number"), json!("boolean"), json!("array"))
        );
        assert!(t.3["description"].as_str().unwrap().contains("wisp command history.track"));
    }

    #[test]
    fn ui_only_commands_are_left_out_and_hand_made_names_kept() {
        let all = [cmd("a.b", None, None), cmd("c.d", None, Some("a drag on the canvas")), cmd("ask.user", None, None)];
        let names: Vec<String> = tools(&all, &["ask_user"]).into_iter().map(|t| t.0).collect();
        assert_eq!(names, ["a_b"], "ui-only out; ask_user stays the hand-made one");
        assert_eq!(args_of(false, &json!({ "args": [1] })), json!([1]));
        // audit D6: called with nothing (as the schema allows), a command that takes no fields gets `{}`, not `null`; its
        // fields given at the top level are its arguments
        assert_eq!(args_of(false, &json!({})), json!({}));
        assert_eq!(args_of(false, &Value::Null), json!({}));
        assert_eq!(args_of(false, &json!({ "id": "x" })), json!({ "id": "x" }));
        assert_eq!(args_of(false, &json!({ "args": { "id": "y" } })), json!({ "id": "y" }));
    }

    /// Audit F orchestration F7: as a run, `later_agent_schedule {}` answered `{"error":"Say when: …"}` as ordinary
    /// content; a command's own refusal is the tool's error.
    #[test]
    fn a_commands_own_refusal_is_the_tools_error_audit_f_orchestration_f7() {
        use serde_json::json;
        assert_eq!(
            super::refusal(&json!({ "error": "Say when: at (a time) or in (minutes)." })).as_deref(),
            Some("Say when: at (a time) or in (minutes).")
        );
        assert_eq!(super::refusal(&json!({ "text": "Scheduled.", "error": null })), None);
        assert_eq!(super::refusal(&json!({ "text": "x", "error": "also" })), None, "an answer with its text isn't refused");
        assert_eq!(super::refusal(&json!({ "error": "" })), None);
        assert_eq!(super::refusal(&json!([{ "error": "a row's field" }])), None);
        assert_eq!(super::refusal(&json!(null)), None);
    }
}
