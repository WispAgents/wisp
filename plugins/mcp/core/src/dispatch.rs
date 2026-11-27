//! The dispatcher: one parsed message and who sent it in, what to answer out. `initialize`, `server/discover`,
//! `tools/list`, `ping`, refusals and unknown methods are answered here at once; a tool call comes back as a `Call`
//! for the host to run (it waits on the kernel), and `finish` turns its outcome into v1's answer. A method another
//! plugin serves (`mcp.method@1`, ADR-0146: `events/poll`) comes back as `Step::Method`.
//!
//! Dual-era (ADR-0146): a legacy client opens with `initialize`; a modern (`2026-07-28`) one declares its version in
//! each request's `_meta` and gets `"resultType":"complete"` on every result. Every request is served statelessly
//! either way, as it always was.

use serde_json::value::RawValue;
use serde_json::{Map, Value};
use wisp_mcp_api::{Audience, SessionInfo};

use crate::rpc::{self, Incoming, Message};
use crate::table::{self, OwnFlags, Refusal};
use crate::{McpError, words};

const USER_INSTRUCTIONS: &str = "wisp runs coding agents in the background. Launch with wisp_launch, then wisp_wait / wisp_get for the result; wisp_reply to follow up or answer a question. wisp_crew* manage the user's crew of long-lived agents.";
/// v1's agent server always said 0.1.0; harnesses may have cached it.
const AGENT_SERVER_VERSION: &str = "0.1.0";

/// Who a message is from, as the tools see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Who {
    /// a task's agent: its session (the caller's) and that session's flags
    Agent { session: String, info: SessionInfo },
    /// the user's own editor (`wisp mcp`), and wisp's version for `serverInfo`
    User { version: String },
}

impl Who {
    /// A caller with a session is that session's agent; an `agent:capped` set is capped whatever the
    /// session's flags say. Anyone else is the user's editor.
    pub fn of(set: &str, session: Option<&str>, info: SessionInfo, version: &str) -> Self {
        match session {
            Some(s) => {
                Self::Agent { session: s.to_owned(), info: SessionInfo { capped: info.capped || set.starts_with("agent:capped"), ..info } }
            }
            None => Self::User { version: version.to_owned() },
        }
    }
}

/// A tool another plugin contributed, as the dispatcher sees it.
#[derive(Debug, Clone)]
pub struct Extra {
    pub name: String,
    /// its definition as JSON text
    pub raw: String,
    pub audience: Audience,
    pub flags: OwnFlags,
}

impl Extra {
    fn for_agent(&self) -> bool {
        matches!(self.audience, Audience::Agent | Audience::Both)
    }
    fn for_user(&self) -> bool {
        matches!(self.audience, Audience::User | Audience::Both)
    }
}

/// The protocol versions wisp answers: the modern one first, then the legacy ones (`initialize`).
pub const SUPPORTED_VERSIONS: &[&str] = &["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26"];
/// `UnsupportedProtocolVersionError` (`2026-07-28`).
pub const UNSUPPORTED_VERSION: i64 = -32022;

/// The methods other plugins serve (`mcp.method@1`) and the extensions they declare, by identifier.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Served {
    pub methods: Vec<String>,
    pub extensions: Map<String, Value>,
}

/// The methods a contribution can't take: the core protocol's, answered here.
pub fn core_method(m: &str) -> bool {
    matches!(m, "initialize" | "ping" | "server/discover") || m.starts_with("tools/") || m.starts_with("notifications/")
}

/// A tool call for the host to run.
#[derive(Debug)]
pub struct Call {
    pub id: Box<RawValue>,
    pub name: String,
    pub args: Map<String, Value>,
    /// the index in `extras` when the tool is a contributed one
    pub extra: Option<usize>,
}

#[derive(Debug)]
pub enum Step {
    /// the whole answer, as JSON text
    Reply(String),
    /// a notification: no answer
    Silent,
    Call(Call),
    /// a method a plugin serves, for the host to run as the caller
    Method {
        id: Box<RawValue>,
        method: String,
        params: Value,
    },
}

/// What one line or body asks for: its steps, and whether they answer as a batch.
#[derive(Debug)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub batch: bool,
    /// a modern request (`_meta` names its version): each result gets `"resultType":"complete"` (`rpc::completed`)
    pub modern: bool,
}

pub fn plan(text: &str, who: &Who, extras: &[Extra]) -> Plan {
    plan_served(text, who, extras, &Served::default())
}

/// `plan`, with the methods and extensions other plugins serve.
pub fn plan_served(text: &str, who: &Who, extras: &[Extra], served: &Served) -> Plan {
    match rpc::parse(text) {
        Incoming::Nothing => Plan { steps: Vec::new(), batch: false, modern: false },
        Incoming::One(m) => {
            let modern = m.modern_version().is_some();
            Plan { steps: vec![step_served(m, who, extras, served)], batch: false, modern }
        }
        Incoming::Batch(items) if items.is_empty() => {
            Plan { steps: vec![Step::Reply(rpc::invalid_request())], batch: false, modern: false }
        }
        Incoming::Batch(items) => Plan {
            steps: items
                .into_iter()
                .map(|m| m.map_or_else(|| Step::Reply(rpc::invalid_request()), |m| step_served(m, who, extras, served)))
                .collect(),
            batch: true,
            modern: false,
        },
    }
}

/// The answers of a plan's steps, in order (`None` for silent ones), as one line or body.
pub fn assemble(replies: Vec<Option<String>>, batch: bool) -> Option<String> {
    let mut replies: Vec<String> = replies.into_iter().flatten().collect();
    if batch { rpc::batch(&replies) } else { replies.pop() }
}

pub fn step(m: Message, who: &Who, extras: &[Extra]) -> Step {
    step_served(m, who, extras, &Served::default())
}

pub fn step_served(mut m: Message, who: &Who, extras: &[Extra], served: &Served) -> Step {
    let Some(id) = m.id.take() else { return Step::Silent };
    if let Some(v) = m.modern_version().filter(|v| !SUPPORTED_VERSIONS.contains(v)) {
        let data = serde_json::json!({ "supported": SUPPORTED_VERSIONS, "requested": v });
        return Step::Reply(rpc::error_data(&id, UNSUPPORTED_VERSION, "Unsupported protocol version", Some(&data)));
    }
    match m.method() {
        Some("initialize") => Step::Reply(rpc::result(&id, &initialize(&m, who, &served.extensions))),
        Some("server/discover") => Step::Reply(rpc::result(&id, &discover(who, &served.extensions))),
        Some("tools/list") => Step::Reply(rpc::result(&id, &tools_list(who, extras))),
        Some("ping") => Step::Reply(rpc::result(&id, "{}")),
        Some("tools/call") => call(id, m, who, extras),
        Some(x) if !core_method(x) && served.methods.iter().any(|s| s == x) => {
            let method = x.to_owned();
            Step::Method { id, method, params: m.take_params() }
        }
        _ => Step::Reply(not_found(&id, &m.method_text())),
    }
}

fn not_found(id: &RawValue, method: &str) -> String {
    rpc::error(id, rpc::METHOD_NOT_FOUND, &format!("Method not found: {method}"))
}

/// `{"tools":{}}`, with `"extensions":{…}` when plugins declare any (ADR-0146).
fn capabilities(extensions: &Map<String, Value>) -> String {
    if extensions.is_empty() {
        return "{\"tools\":{}}".into();
    }
    format!("{{\"tools\":{{}},\"extensions\":{}}}", Value::Object(extensions.clone()))
}

fn initialize(m: &Message, who: &Who, extensions: &Map<String, Value>) -> String {
    let pv = m.protocol_version();
    let caps = capabilities(extensions);
    match who {
        Who::Agent { .. } => format!(
            "{{\"protocolVersion\":{pv},\"capabilities\":{caps},\"serverInfo\":{{\"name\":\"wisp\",\"version\":\"{AGENT_SERVER_VERSION}\"}}}}"
        ),
        Who::User { version } => format!(
            "{{\"protocolVersion\":{pv},\"capabilities\":{caps},\"serverInfo\":{{\"name\":\"wisp\",\"version\":{}}},\"instructions\":{}}}",
            rpc::quote(version),
            rpc::quote(USER_INSTRUCTIONS)
        ),
    }
}

/// `server/discover` (`2026-07-28`, which every server must answer): the versions, the capabilities (extensions
/// included) and who it is. Never cached across callers: what a task's agent is offered differs from the user's.
fn discover(who: &Who, extensions: &Map<String, Value>) -> String {
    let version = match who {
        Who::Agent { .. } => AGENT_SERVER_VERSION,
        Who::User { version } => version.as_str(),
    };
    let caps: Value = serde_json::from_str(&capabilities(extensions)).unwrap_or_default();
    let mut v = serde_json::json!({
        "resultType": "complete",
        "supportedVersions": SUPPORTED_VERSIONS,
        "capabilities": caps,
        "_meta": { "io.modelcontextprotocol/serverInfo": { "name": "wisp", "version": version } },
        "cacheScope": "private",
    });
    if matches!(who, Who::User { .. }) {
        v["instructions"] = Value::String(USER_INSTRUCTIONS.into());
    }
    v.to_string()
}

/// `{"tools":[…]}`: the built-ins first, in v1's order, then what plugins contributed.
pub fn tools_list(who: &Who, extras: &[Extra]) -> String {
    let Ok(t) = table::table() else { return "{\"tools\":[]}".into() };
    let (base, own): (std::borrow::Cow<'_, str>, Vec<&str>) = match who {
        Who::User { .. } => (t.user_list().into(), extras.iter().filter(|e| e.for_user()).map(|e| e.raw.as_str()).collect()),
        Who::Agent { info, .. } => (
            table::agent_list(t, info),
            extras
                .iter()
                .filter(|e| e.for_agent() && table::not_offered(&e.name, info, Some(e.flags)).is_none())
                .map(|e| e.raw.as_str())
                .collect(),
        ),
    };
    let mut s = String::with_capacity(base.len() + own.iter().map(|r| r.len() + 1).sum::<usize>() + 12);
    s.push_str("{\"tools\":[");
    s.push_str(&base);
    for r in own {
        if s.len() > "{\"tools\":[".len() {
            s.push(',');
        }
        s.push_str(r);
    }
    s.push_str("]}");
    s
}

fn call(id: Box<RawValue>, mut m: Message, who: &Who, extras: &[Extra]) -> Step {
    let name = m.tool().unwrap_or_default().to_owned();
    match who {
        Who::Agent { info, .. } => {
            let extra = extras.iter().position(|e| e.name == name && e.for_agent());
            let own = extra.and_then(|i| extras.get(i)).map(|e| e.flags);
            match table::not_offered(&name, info, own) {
                Some(Refusal::Capped) => return Step::Reply(rpc::result(&id, &rpc::tool_text(words::CAPPED, true))),
                Some(Refusal::ReadOnly) => return Step::Reply(rpc::result(&id, &rpc::tool_text(words::READ_ONLY, true))),
                Some(Refusal::NotCast) => return Step::Reply(rpc::result(&id, &rpc::tool_text(words::NOT_CAST, true))),
                None => {}
            }
            let builtin =
                table::table().is_ok_and(|t| name == "delegate" || t.agent_def(&name).is_some()) && table::exists_for(&name, info);
            if !builtin && extra.is_none() {
                // the tool, named, as the user's server says it (audit G18: v1 fell through to "Method not found:
                // tools/call", which names the method)
                return Step::Reply(rpc::error(&id, rpc::INVALID_PARAMS, &McpError::UnknownTool(name).to_string()));
            }
            Step::Call(Call { id, name, args: m.take_arguments(), extra: if builtin { None } else { extra } })
        }
        Who::User { .. } => {
            let builtin = table::table().is_ok_and(|t| t.user.iter().any(|d| d.name == name)) || table::USER_CREW.contains(&name.as_str());
            let extra = extras.iter().position(|e| e.name == name && e.for_user());
            if !builtin && extra.is_none() {
                return Step::Reply(rpc::error(&id, rpc::INVALID_PARAMS, &McpError::UnknownTool(name).to_string()));
            }
            Step::Call(Call { id, name, args: m.take_arguments(), extra: if builtin { None } else { extra } })
        }
    }
}

/// How a tool call ended, as the host saw it.
#[derive(Debug)]
pub enum Outcome {
    /// the text the tool answers
    Text(String),
    /// it failed: the owner's refusal or the kernel's error, in words
    Failed(String),
    /// a JSON-RPC error rather than a tool result (`wisp mcp`'s unknown tool)
    Rpc(i64, String),
}

/// The answer to a call, in v1's words for a failure: an `approve` that fails denies (the gate fails
/// closed), `ask_user` says the user couldn't be reached, anything else is `wisp: …`.
pub fn finish(id: &RawValue, name: &str, who: &Who, outcome: Outcome) -> String {
    match outcome {
        Outcome::Text(t) => rpc::result(id, &rpc::tool_text(&t, false)),
        Outcome::Rpc(code, message) => rpc::error(id, code, &message),
        Outcome::Failed(why) => {
            let agent = matches!(who, Who::Agent { .. });
            let body = match name {
                "approve" if agent => rpc::tool_text(&words::deny_json(&format!("{}: {why}", words::NO_USER)), false),
                "ask_user" if agent => rpc::tool_text(&format!("{}: {why}", words::NO_USER), true),
                _ => rpc::tool_text(&format!("wisp: {why}"), true),
            };
            rpc::result(id, &body)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn user() -> Who {
        Who::User { version: "2.0.0".into() }
    }

    fn served() -> Served {
        let mut extensions = Map::new();
        extensions.insert("io.modelcontextprotocol/events".into(), serde_json::json!({}));
        Served { methods: vec!["events/list".into(), "events/poll".into(), "tools/sneaky".into()], extensions }
    }

    fn reply(text: &str, s: &Served) -> Value {
        match plan_served(text, &user(), &[], s).steps.pop() {
            Some(Step::Reply(r)) => serde_json::from_str(&r).unwrap(),
            other => panic!("not a reply: {other:?}"),
        }
    }

    #[test]
    fn an_extension_a_plugin_serves_is_declared_in_initialize_and_discover() {
        let init = reply(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#, &served());
        assert_eq!(init["result"]["capabilities"]["extensions"]["io.modelcontextprotocol/events"], serde_json::json!({}));
        let d = reply(r#"{"jsonrpc":"2.0","id":2,"method":"server/discover","params":{}}"#, &served());
        assert_eq!(d["result"]["resultType"], "complete");
        assert_eq!(d["result"]["supportedVersions"][0], "2026-07-28");
        assert_eq!(d["result"]["capabilities"]["tools"], serde_json::json!({}));
        assert!(d["result"]["capabilities"]["extensions"]["io.modelcontextprotocol/events"].is_object());
        assert_eq!(d["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"], "wisp");
    }

    #[test]
    fn without_contributions_initialize_is_v1s_answer() {
        let init = reply(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#, &Served::default());
        assert_eq!(init["result"]["capabilities"], serde_json::json!({ "tools": {} }));
    }

    #[test]
    fn a_served_method_goes_to_the_host_with_its_params_and_a_core_one_never_does() {
        let p = plan_served(
            r#"{"jsonrpc":"2.0","id":7,"method":"events/poll","params":{"name":"task.finished","cursor":null}}"#,
            &user(),
            &[],
            &served(),
        );
        match p.steps.as_slice() {
            [Step::Method { id, method, params }] => {
                assert_eq!(id.get(), "7");
                assert_eq!(method, "events/poll");
                assert_eq!(params["name"], "task.finished");
            }
            other => panic!("{other:?}"),
        }
        // a contribution naming a core method's prefix is never handed over
        let r = reply(r#"{"jsonrpc":"2.0","id":8,"method":"tools/sneaky"}"#, &served());
        assert_eq!(r["error"]["code"], rpc::METHOD_NOT_FOUND);
        // nor is a method nobody serves
        let r = reply(r#"{"jsonrpc":"2.0","id":9,"method":"events/stream"}"#, &served());
        assert_eq!(r["error"]["code"], rpc::METHOD_NOT_FOUND);
    }

    #[test]
    fn a_modern_request_is_marked_and_an_unknown_version_is_refused_with_the_supported_ones() {
        let m =
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}"#;
        let p = plan_served(m, &user(), &[], &served());
        assert!(p.modern);
        let Some(Step::Reply(r)) = p.steps.into_iter().next() else { panic!() };
        let done: Value = serde_json::from_str(&rpc::completed(&r)).unwrap();
        assert_eq!(done["result"]["resultType"], "complete");
        assert!(done["result"]["tools"].is_array());
        let old =
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"1900-01-01"}}}"#;
        let e = reply(old, &served());
        assert_eq!(e["error"]["code"], UNSUPPORTED_VERSION);
        assert_eq!(e["error"]["data"]["requested"], "1900-01-01");
        assert_eq!(e["error"]["data"]["supported"][0], "2026-07-28");
        // an error is left as it was
        assert_eq!(
            rpc::completed(r#"{"jsonrpc":"2.0","id":1,"error":{"code":1,"message":"x"}}"#),
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":1,"message":"x"}}"#
        );
    }
}
