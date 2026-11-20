//! Declared tasks (ADR-0027): a task spec kept as a template (`tasks:` in a formula). Applying the
//! formula registers it; each run (its schedule, `wisp run <name>`, a trigger) makes a task from it.
//! A `{param}` in its prompt or steps is filled from the run's values: recipes are these.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wisp_kernel::Command;

/// A task you can run by name, by schedule or from a trigger.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskDecl {
    /// what to do; `{param}` is filled from the run's values (every task but `tasks.default` has one)
    #[serde(default)]
    pub prompt: String,
    /// its name as you wrote it ("Run tests"), shown on its chip, when the name (a key: `wisp recipes run run-tests`)
    /// can't hold it (a v1 recipe, audit D11). Additive (2026-11-19).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// prompts sent after the first, each when the turn before ends
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<String>,
    /// the folder it runs in
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project: String,
    /// an agent profile's name or a harness
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub agent: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub effort: String,
    /// plan | edits | auto
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mode: String,
    /// run in a worktree of its own
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<bool>,
    /// the goal judged after each turn: a declared goal's name (`green-build`, from `goals:`), or inline
    /// (`{ text: tests pass, judges: [reviewer] }`); `{ name: green-build, maxRounds: 3 }` is the declared one with
    /// its own limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<Value>,
    /// the software factory it runs through (ADR-0114): a factory's name (`default` is always there); its stations run
    /// it instead of one agent
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub factory: String,
    /// the Notes it starts with (ADR-0097): `{ goal: …, constraints: [...] }`, kept by wisp.threads
    #[serde(default, skip_serializing_if = "wisp_threads_api::SeedNotes::is_empty")]
    pub notes: wisp_threads_api::SeedNotes,
    /// the crew member it runs for
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub crew: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connectors: Vec<String>,
    /// when it runs by itself: a cron or words ("every day at 2"), in UTC
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub schedule: String,
    /// paused: its schedule doesn't fire (it still runs by hand)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
}

/// The name that holds every task's defaults (ADR-0110): never a task of its own, never run.
pub const DEFAULT: &str = "default";

/// The fields `tasks.default` can't give: they're each task's own (what it does, when it runs by itself).
pub const OWN_FIELDS: [&str; 5] = ["prompt", "title", "steps", "schedule", "paused"];

/// The problems of `tasks.default`'s spec: a field that is each task's own.
pub fn default_problems(spec: &Value) -> Vec<String> {
    OWN_FIELDS
        .iter()
        .filter(|k| {
            spec.get(**k).is_some_and(|v| {
                !v.is_null() && v != &Value::Bool(false) && v.as_str() != Some("") && v.as_array().is_none_or(|a| !a.is_empty())
            })
        })
        .map(|k| format!("tasks.default gives every task its defaults: {k} is each task's own"))
        .collect()
}

/// `t` with `tasks.default`'s fields under its own: a field the task gives wins, one it leaves out is the default's
/// (`d`'s own fields, [`OWN_FIELDS`], never come from it).
pub fn with_defaults(t: &TaskDecl, d: &TaskDecl) -> TaskDecl {
    let (Ok(Value::Object(mut base)), Ok(Value::Object(own))) = (serde_json::to_value(d), serde_json::to_value(t)) else {
        return t.clone();
    };
    for k in OWN_FIELDS {
        base.remove(k);
    }
    base.extend(own);
    serde_json::from_value(Value::Object(base)).unwrap_or_else(|_| t.clone())
}

/// The goal a task names, as `runtime.create` takes it: a string in a task is a declared goal's name (`{ name }`,
/// checked by wisp.goal when the task is made); an object is as written.
pub fn goal_arg(goal: Option<&Value>) -> Option<Value> {
    match goal? {
        Value::String(s) if !s.trim().is_empty() => Some(serde_json::json!({ "name": s.trim() })),
        Value::String(_) | Value::Null => None,
        v => Some(v.clone()),
    }
}

/// The declared goal a task names (`goal: green-build` or `{ name: green-build, … }`), for `wisp validate` to check.
pub fn goal_ref(goal: Option<&Value>) -> Option<String> {
    match goal? {
        Value::String(s) => Some(s.trim().to_owned()).filter(|s| !s.is_empty()),
        Value::Object(o) => o.get("name").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned),
        _ => None,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NamedTask {
    pub name: String,
    #[serde(flatten)]
    pub spec: TaskDecl,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskName {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunTask {
    pub name: String,
    /// `{param}` values by name
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    /// values by position, for the params not named
    #[serde(default)]
    pub words: Vec<String>,
    /// any field of the spec, over it (`agent`, `project`, …)
    #[serde(default)]
    pub over: serde_json::Map<String, Value>,
}

/// The `{name}` params of a text, in order, once each.
pub fn params_of(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(a) = rest.find('{') {
        let after = &rest[a + 1..];
        let Some(b) = after.find('}') else { break };
        let name = &after[..b];
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') && !out.iter().any(|x| x == name) {
            out.push(name.to_owned());
        }
        rest = &after[b + 1..];
    }
    out
}

/// `text` with its `{params}` filled; the first missing one is the error.
pub fn fill(text: &str, values: &BTreeMap<String, String>) -> Result<String, String> {
    let mut out = text.to_owned();
    for p in params_of(text) {
        let v = values.get(&p).ok_or_else(|| format!("needs {{{p}}}: wisp run <task> {p}=…"))?;
        out = out.replace(&format!("{{{p}}}"), v);
    }
    Ok(out)
}

macro_rules! command {
    ($name:ident = $id:literal [$perm:literal] $(read $ro:ident)? : $args:ty => $out:ty $(; $typed:ident)?) => {
        #[derive(Debug)]
        pub struct $name;
        impl Command for $name {
            const ID: &'static str = $id;
            const PERMISSION: Option<&'static str> = Some($perm);
            $(const READ_ONLY: bool = { let _ = stringify!($ro); true };)?
            type Args = $args;
            $(fn args_ts() -> Option<String> {
                // `; typed`: the generated CLI's flags and --help come from the argument type (ADR-0022)
                let _ = stringify!($typed);
                Some(format!("{}{}", <$args as ::ts_rs::TS>::docs().unwrap_or_default(), <$args as ::ts_rs::TS>::inline(&::ts_rs::Config::default())))
            })?
            type Out = $out;
        }
    };
}

command!(Tasks = "automations.tasks" ["automations:read"] read yes : Value => Vec<NamedTask>; typed);
/// A command whose arguments are written out for the generated CLI and MCP tools (audit I, D6: these three took only
/// `{args}`, their types having no ts-rs shape: `TaskDecl` flattens into `NamedTask`).
macro_rules! described_command {
    ($name:ident = $id:literal [$perm:literal] : $args:ty => $out:ty, $ts:expr) => {
        #[derive(Debug)]
        pub struct $name;
        impl Command for $name {
            const ID: &'static str = $id;
            const PERMISSION: Option<&'static str> = Some($perm);
            type Args = $args;
            type Out = $out;
            fn args_ts() -> Option<String> {
                Some($ts.to_owned())
            }
        }
    };
}

/// [`TaskSet`]'s argument, as `TaskDecl`'s fields say it.
pub const TASK_SET_TS: &str = "/** Add or change a declared task (a recipe when its prompt has {params}). */ { /** its name: `wisp run <name>` runs it */ name: string, /** what to do; {param} is filled from the run's values */ prompt: string, /** its name as you wrote it, when the name can't hold it */ title?: string, /** prompts sent after the first, each when the turn before ends */ steps?: Array<string>, /** the folder it runs in, or a project's name */ project?: string, /** an agent profile's name or a harness */ agent?: string, /** the model its harness runs */ model?: string, /** how hard it thinks: low, medium, high … */ effort?: string, /** plan, edits or auto */ mode?: string, /** run in a worktree of its own */ worktree?: boolean, /** the goal judged after each turn: a declared goal's name, or { text, judges } */ goal?: unknown, /** the software factory it runs through */ factory?: string, /** the Notes it starts with: { goal, constraints } */ notes?: Record<string, unknown>, /** the crew member it runs for */ crew?: string, /** the connectors it may use, by name */ connectors?: Array<string>, /** when it runs by itself: a cron or words (every day at 2), in UTC */ schedule?: string, /** its schedule doesn't fire (it still runs by hand) */ paused?: boolean }";
/// [`TaskRemove`]'s argument.
pub const TASK_REMOVE_TS: &str = "/** Remove a declared task. */ { /** the declared task's name */ name: string }";
/// [`TaskRun`]'s argument, as `RunTask`'s fields say it.
pub const TASK_RUN_TS: &str = "/** Run a declared task (a recipe with its values). */ { /** the declared task's name */ name: string, /** {param} values by name */ params?: Record<string, string>, /** values by position, for the params not named */ words?: Array<string>, /** any field of the task, over it (agent, project, …) */ over?: Record<string, unknown> }";

described_command!(TaskSet = "automations.task.set" ["automations:write"] : NamedTask => NamedTask, TASK_SET_TS);
described_command!(TaskRemove = "automations.task.remove" ["automations:write"] : TaskName => Value, TASK_REMOVE_TS);
described_command!(TaskRun = "automations.task.run" ["automations:run"] : RunTask => Value, TASK_RUN_TS);

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// ADR-0110: `tasks.default`'s fields under every task's own; what's each task's own never comes from it.
    #[test]
    fn a_tasks_own_fields_win_over_tasks_default() {
        let d: TaskDecl = serde_json::from_value(
            serde_json::json!({ "agent": "builder", "mode": "auto", "worktree": true, "goal": "green-build", "connectors": ["linear"] }),
        )
        .unwrap();
        let t: TaskDecl = serde_json::from_value(serde_json::json!({ "prompt": "fix it", "mode": "plan", "worktree": false })).unwrap();
        let m = with_defaults(&t, &d);
        assert_eq!((m.prompt.as_str(), m.agent.as_str(), m.mode.as_str(), m.worktree), ("fix it", "builder", "plan", Some(false)));
        assert_eq!((m.goal.clone(), m.connectors.clone()), (Some(serde_json::json!("green-build")), vec!["linear".to_owned()]));
        let d2 = TaskDecl { schedule: "0 2 * * *".into(), steps: vec!["more".into()], ..d.clone() };
        let m = with_defaults(&t, &d2);
        assert!(m.schedule.is_empty() && m.steps.is_empty(), "a default never schedules every task or adds steps");
        assert_eq!(
            default_problems(&serde_json::json!({ "agent": "a", "schedule": "0 2 * * *", "prompt": "" })),
            ["tasks.default gives every task its defaults: schedule is each task's own"]
        );
        assert_eq!(default_problems(&serde_json::to_value(&d).unwrap()), Vec::<String>::new());
    }

    #[test]
    fn a_tasks_goal_by_name_or_inline() {
        assert_eq!(goal_arg(Some(&serde_json::json!("green-build"))), Some(serde_json::json!({ "name": "green-build" })));
        assert_eq!(goal_arg(Some(&serde_json::json!({ "text": "tests pass" }))), Some(serde_json::json!({ "text": "tests pass" })));
        assert_eq!(goal_arg(Some(&serde_json::json!(" "))), None);
        assert_eq!(goal_ref(Some(&serde_json::json!({ "name": "g", "maxRounds": 2 }))).as_deref(), Some("g"));
        assert_eq!(goal_ref(Some(&serde_json::json!({ "text": "inline" }))), None);
    }

    #[test]
    fn params_fill_in_order() {
        assert_eq!(params_of("fix {issue} in {repo}, then {issue}"), ["issue", "repo"]);
        let v: BTreeMap<String, String> = [("issue".to_owned(), "42".to_owned()), ("repo".to_owned(), "api".to_owned())].into();
        assert_eq!(fill("fix {issue} in {repo}", &v).unwrap(), "fix 42 in api");
        assert!(fill("fix {issue}", &BTreeMap::new()).unwrap_err().contains("{issue}"));
    }
}
