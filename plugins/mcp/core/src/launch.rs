//! `wisp_launch` from a task's run (v1 routes/control.js `/internal/launch`): the tool's arguments as the
//! `runtime.create` the user's launcher would send (audit C1). Every field its schema documents is passed: a project by
//! name or id (its folder), a folder, or `scratch`; `also` (more folders), `effort`, `worktree` (default on in a git
//! repo, except in plan mode), `subagents` (a team) or `preset` (a saved one). `machine` is the host's: it routes the
//! same body to that machine. Pure: the host reads the projects, the asking task, the setting and the folder.

use serde_json::{Map, Value, json};

/// Where the new task runs: none (a scratch folder of its own), or a folder. `project`: the named project's folder,
/// when `a` names one and it was found; `asking`: the asking task's own project folder (its main checkout).
///
/// # Errors
/// A project it names that isn't there, in words the agent can act on.
pub fn folder(a: &Map<String, Value>, project: Option<&str>, asking: Option<&str>) -> Result<Option<String>, String> {
    let s = |k: &str| a.get(k).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty());
    if a.get("scratch") == Some(&Value::Bool(true)) {
        return Ok(None);
    }
    if let Some(name) = s("project") {
        return project.map(|p| Some(p.to_owned())).ok_or_else(|| format!("No project “{name}”. wisp_available lists them."));
    }
    Ok(s("cwd").or(asking).map(str::to_owned))
}

/// The `runtime.create` body: `cwd` from [`folder`]; `tree_on`: a worktree by default there (Worktree per task on,
/// and the folder is a git repo). An explicit `worktree` wins; plan mode makes none by default (it edits nothing).
#[must_use]
pub fn start_body(a: &Map<String, Value>, cwd: Option<&str>, tree_on: bool) -> Value {
    let s = |k: &str| a.get(k).and_then(Value::as_str).filter(|v| !v.is_empty());
    let mut b = json!({ "prompt": a.get("prompt").cloned().unwrap_or(Value::Null), "by": "agent" });
    // `goal`: a declared goal's name is that goal (wisp.goal reads it, ADR-0110), other words what done means
    // `factory`: the stations run it (ADR-0114); wisp.factory refuses a name no factory has
    for k in ["agent", "model", "effort", "mode", "goal", "factory"] {
        if let Some(v) = s(k) {
            b[k] = json!(v);
        }
    }
    if let Some(c) = cwd {
        b["cwd"] = json!(c);
        let plan = s("mode") == Some("plan");
        b["worktree"] = json!(a.get("worktree").and_then(Value::as_bool).unwrap_or(tree_on && !plan));
    }
    if let Some(also) = a.get("also").and_then(Value::as_array) {
        b["dirs"] = json!(also.iter().filter_map(Value::as_str).filter(|d| !d.is_empty()).collect::<Vec<_>>());
    }
    // a team inline wins over a preset (v1: `preset` only when no `subagents`)
    if let Some(team) = a.get("subagents").filter(|t| t.is_array()) {
        b["team"] = team.clone();
    } else if let Some(p) = s("preset") {
        b["team"] = json!(p);
    }
    b
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn args(v: &Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn every_field_wisp_launch_documents_reaches_runtime_create_audit_c1() {
        let a = args(&json!({ "prompt": "fix it", "project": "proj", "agent": "codex", "model": "m", "effort": "high", "mode": "edits",
                             "also": ["/lib"], "subagents": [{ "agent": "claude" }], "preset": "p1", "machine": "box", "session": "s1" }));
        let cwd = folder(&a, Some("/w/proj"), Some("/w/other")).unwrap();
        assert_eq!(cwd.as_deref(), Some("/w/proj"), "a project by name is its folder, never an empty scratch");
        let b = start_body(&a, cwd.as_deref(), true);
        assert_eq!(
            b,
            json!({ "prompt": "fix it", "by": "agent", "agent": "codex", "model": "m", "effort": "high", "mode": "edits", "cwd": "/w/proj",
                    "worktree": true, "dirs": ["/lib"], "team": [{ "agent": "claude" }] })
        );
        // a project that isn't there: said, not a scratch folder
        assert!(folder(&a, None, None).unwrap_err().contains("No project “proj”"));
        // scratch: no folder, no worktree; a folder: as given; nothing: the asking task's project
        assert_eq!(folder(&args(&json!({ "scratch": true, "cwd": "/x" })), None, Some("/w")).unwrap(), None);
        assert_eq!(folder(&args(&json!({ "cwd": "/x" })), None, Some("/w")).unwrap().as_deref(), Some("/x"));
        assert_eq!(folder(&args(&json!({})), None, Some("/w")).unwrap().as_deref(), Some("/w"));
        let b = start_body(&args(&json!({ "prompt": "q", "scratch": true })), None, true);
        assert!(b.get("cwd").is_none() && b.get("worktree").is_none(), "{b}");
        // worktree: the default in a git repo, none in plan mode or outside one, and what the agent said wins
        let tree = |v: Value, on: bool| start_body(&args(&v), Some("/w"), on)["worktree"].clone();
        assert_eq!(tree(json!({ "prompt": "x" }), true), json!(true));
        assert_eq!(tree(json!({ "prompt": "x", "mode": "plan" }), true), json!(false));
        assert_eq!(tree(json!({ "prompt": "x" }), false), json!(false));
        assert_eq!(tree(json!({ "prompt": "x", "worktree": false }), true), json!(false));
        assert_eq!(tree(json!({ "prompt": "x", "worktree": true, "mode": "plan" }), false), json!(true));
        // a goal: a declared one's name or words, as given (wisp.goal tells them apart)
        assert_eq!(start_body(&args(&json!({ "prompt": "x", "goal": "green-build" })), None, false)["goal"], json!("green-build"));
        // a factory by name (ADR-0114)
        assert_eq!(start_body(&args(&json!({ "prompt": "x", "factory": "default" })), None, false)["factory"], json!("default"));
        // a preset when no team is given
        assert_eq!(start_body(&args(&json!({ "prompt": "x", "preset": "p1" })), None, false)["team"], json!("p1"));
    }
}
