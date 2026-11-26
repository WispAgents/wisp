//! The `computer` MCP server as a harness sees it: one JSON-RPC message (or a batch) per HTTP request.
//! `initialize`, `ping` and `tools/list` are answered here without the driver (a run that never touches
//! the screen never starts it: on a box that's a screen and a browser it doesn't pay for); `tools/call`
//! is judged here and handed to the host, which passes it to the driver and its answer back as it is (a
//! screenshot stays an MCP image).

use serde_json::{Value, json};

use crate::codes;
use crate::policy::{self, Reach};

/// What the driver lists before it has started: cua-driver's own list for [`wisp_computer_api::DRIVER_VERSION`].
pub const SNAPSHOT: &str = include_str!("../data/tools.json");

/// What a message asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// answered here: the whole JSON-RPC reply
    Reply(Value),
    /// a notification: nothing to answer
    Silent,
    /// the tool list (the host knows the driver's current one)
    List { id: Value },
    /// a tool call: the host checks it with [`judge`], then the driver runs it
    Call { id: Value, name: String, args: Value },
}

/// The messages in a request body, or the parse error to answer with.
pub fn messages(body: &str) -> Result<Vec<Value>, Value> {
    match serde_json::from_str::<Value>(body) {
        Ok(Value::Array(a)) if a.is_empty() => Err(error(&Value::Null, codes::INVALID_REQUEST, "Invalid Request")),
        Ok(Value::Array(a)) => Ok(a),
        Ok(v) => Ok(vec![v]),
        Err(_) => Err(error(&Value::Null, codes::PARSE_ERROR, "Parse error")),
    }
}

/// One message's step. `instructions`: what `initialize` tells the harness about using the tools.
pub fn step(msg: &Value, instructions: &str) -> Step {
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        return Step::Reply(error(msg.get("id").unwrap_or(&Value::Null), codes::INVALID_REQUEST, "Invalid Request"));
    };
    // a message without an id is a notification (`notifications/initialized`, a cancel): never answered
    let Some(id) = msg.get("id").cloned() else { return Step::Silent };
    match method {
        "initialize" => Step::Reply(result(&id, &initialize(msg.pointer("/params/protocolVersion"), instructions))),
        "ping" => Step::Reply(result(&id, &json!({}))),
        "tools/list" => Step::List { id },
        "tools/call" => {
            let name = msg.pointer("/params/name").and_then(Value::as_str).unwrap_or_default().to_owned();
            if name.is_empty() {
                return Step::Reply(error(&id, codes::INVALID_PARAMS, "tools/call needs a tool's name"));
            }
            let args = msg.pointer("/params/arguments").cloned().unwrap_or_else(|| json!({}));
            Step::Call { id, name, args }
        }
        // nothing else is offered (resources, prompts: the driver's skill files stay the driver's)
        other => Step::Reply(error(&id, codes::METHOD_NOT_FOUND, &format!("Method not found: {other}"))),
    }
}

/// `initialize`'s result: the harness's version when it's one the gateway speaks, else the newest.
pub fn initialize(asked: Option<&Value>, instructions: &str) -> Value {
    let asked = asked.and_then(Value::as_str).unwrap_or_default();
    let version = crate::PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(crate::PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": wisp_computer_api::SERVER, "title": "wisp computer use (Cua Driver)", "version": wisp_computer_api::DRIVER_VERSION },
        "instructions": instructions,
    })
}

/// `tools/list`'s reply: what this task may use of `tools`.
pub fn list(id: &Value, tools: &[Value], reach: Reach) -> Value {
    result(id, &json!({ "tools": policy::visible(tools, reach) }))
}

/// A call that may go to the driver (`None`), or the reply that refuses it: a tool the task isn't offered is
/// a tool result with `isError` (the agent reads why and goes on), a name nobody lists is an unknown tool.
pub fn judge(id: &Value, name: &str, tools: &[Value], reach: Reach) -> Option<Value> {
    let tool = tools.iter().find(|t| t.get("name").and_then(Value::as_str) == Some(name));
    if let Some(why) = policy::refusal(name, tool, reach) {
        return Some(result(id, &tool_error(&why)));
    }
    if tool.is_none() {
        return Some(error(id, codes::INVALID_PARAMS, &format!("Unknown tool: {name}")));
    }
    None
}

/// The snapshot's (instructions, tools).
pub fn snapshot() -> (String, Vec<Value>) {
    let v: Value = serde_json::from_str(SNAPSHOT).unwrap_or(Value::Null);
    let instructions = v.get("instructions").and_then(Value::as_str).unwrap_or_default().to_owned();
    let tools = v.get("tools").and_then(Value::as_array).cloned().unwrap_or_default();
    (instructions, tools)
}

/// What wisp adds to the driver's own instructions.
pub fn instructions(driver: &str, place: policy::Place) -> String {
    let here = match place {
        policy::Place::Box => {
            "This computer is a cloud box (a sandbox): a 1280x800 Linux screen with Chromium already open on it, which the user can watch live. Use it to open and test what you build (a dev server on localhost shows in it)."
        }
        policy::Place::Local => {
            "This is the user's own computer: they may be using it while you work. Stay in the background (the default delivery), and don't close or change windows you didn't open."
        }
    };
    format!("wisp computer use, through Cua Driver. {here}\n\n{driver}")
}

/// `{"jsonrpc":"2.0","id":…,"result":…}`.
pub fn result(id: &Value, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// `{"jsonrpc":"2.0","id":…,"error":{code,message}}`.
pub fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A tool result that failed, in words the agent acts on.
pub fn tool_error(text: &str) -> Value {
    json!({ "isError": true, "content": [{ "type": "text", "text": text }] })
}

/// A request's answer: one reply, an array for a batch, nothing when every message was a notification.
pub fn answer(replies: Vec<Value>, batch: bool) -> Option<Value> {
    if replies.is_empty() {
        None
    } else if batch {
        Some(Value::Array(replies))
    } else {
        replies.into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::Place;

    const PLAN: Reach = Reach { read_only: true, place: Place::Box };
    const AUTO: Reach = Reach { read_only: false, place: Place::Local };

    #[test]
    fn initialize_and_ping_never_need_the_driver() {
        let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } });
        let Step::Reply(r) = step(&init, "use it well") else { panic!("answered here") };
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(r["result"]["serverInfo"]["name"], "computer");
        assert_eq!(r["result"]["instructions"], "use it well");
        let odd = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "1999-01-01" } });
        let Step::Reply(r) = step(&odd, "") else { panic!() };
        assert_eq!(r["result"]["protocolVersion"], crate::PROTOCOL_VERSIONS[0]);
        assert_eq!(step(&json!({ "jsonrpc": "2.0", "id": 7, "method": "ping" }), ""), Step::Reply(result(&json!(7), &json!({}))));
        assert_eq!(step(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), ""), Step::Silent);
    }

    #[test]
    fn a_call_is_judged_by_the_tasks_mode_and_the_deny_list() {
        let (_, tools) = snapshot();
        let id = json!(3);
        assert_eq!(judge(&id, "get_desktop_state", &tools, PLAN), None, "looking is fine in Plan mode");
        let r = judge(&id, "click", &tools, PLAN).expect("refused");
        assert_eq!(r["result"]["isError"], true);
        assert_eq!(judge(&id, "click", &tools, AUTO), None);
        let r = judge(&id, "set_config", &tools, AUTO).expect("refused");
        assert!(r["result"]["content"][0]["text"].as_str().is_some_and(|t| t.contains("isn't offered")));
        let r = judge(&id, "rm_rf", &tools, AUTO).expect("refused");
        assert_eq!(r["error"]["code"], codes::INVALID_PARAMS);
    }

    #[test]
    fn the_snapshot_is_the_pinned_drivers_list_less_the_deny_list() {
        let v: Value = serde_json::from_str(SNAPSHOT).expect("valid JSON");
        assert_eq!(v["driver"], wisp_computer_api::DRIVER_VERSION, "regenerate data/tools.json with the driver");
        let (instructions, tools) = snapshot();
        assert!(instructions.contains("get_window_state"));
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        for core in ["get_desktop_state", "click", "type_text", "press_key", "scroll", "list_windows", "get_window_state", "launch_app"] {
            assert!(names.contains(&core), "{core}");
        }
        assert!(names.iter().all(|n| !policy::DENIED.contains(n)));
        assert_eq!(policy::visible(&tools, AUTO).len(), tools.len() - policy::DENIED_LOCAL.len());
        let ro = policy::visible(&tools, PLAN);
        assert!(!ro.is_empty() && ro.iter().all(policy::looks_only));
    }

    #[test]
    fn bodies_batches_and_bad_messages() {
        assert!(messages("{nope").is_err_and(|e| e["error"]["code"] == codes::PARSE_ERROR));
        assert!(messages("[]").is_err_and(|e| e["error"]["code"] == codes::INVALID_REQUEST));
        assert_eq!(messages(r#"[{"id":1},{"id":2}]"#).map(|m| m.len()).ok(), Some(2));
        let Step::Reply(r) = step(&json!({ "id": 1 }), "") else { panic!() };
        assert_eq!(r["error"]["code"], codes::INVALID_REQUEST);
        let Step::Reply(r) = step(&json!({ "id": 1, "method": "resources/list" }), "") else { panic!() };
        assert_eq!(r["error"]["code"], codes::METHOD_NOT_FOUND);
        let Step::Reply(r) = step(&json!({ "id": 1, "method": "tools/call", "params": {} }), "") else { panic!() };
        assert_eq!(r["error"]["code"], codes::INVALID_PARAMS);
        let call = step(&json!({ "id": "a", "method": "tools/call", "params": { "name": "click" } }), "");
        assert_eq!(call, Step::Call { id: json!("a"), name: "click".into(), args: json!({}) }, "no arguments: an empty object");
        assert_eq!(answer(vec![], true), None);
        assert_eq!(answer(vec![json!(1)], false), Some(json!(1)));
        assert_eq!(answer(vec![json!(1)], true), Some(json!([1])));
    }
}
