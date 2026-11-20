//! A formula as one task's overlay (ADR-0027, ADR-0111), sans-IO. `wisp run --formula`, `wisp "…" --formula`, a task
//! file's `formula:`, `wisp_launch`'s `formula` and New task's Formula… all turn a formula into the same thing here: the
//! sections a task reads (its agents, projects, harnesses, skills, teams, connectors: that task's alone, nothing
//! applied), a declared goal it names inlined from the formula's `goals:`, and every name the task gives checked against
//! what the formula and this wisp declare, said with what there is.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use wisp_resources_api::{Names, Resource, names_in};

use crate::module::Config;

/// The sections a task reads from its overlay (kind, section); the rest only means something applied.
pub const SECTIONS: [(&str, &str); 7] = [
    ("agent", "agents"),
    ("project", "projects"),
    ("harness", "harnesses"),
    ("skill", "skills"),
    ("team", "teams"),
    ("connector", "connectors"),
    // a factory the task runs through (ADR-0114): wisp.factory reads the overlay's before its own
    ("factory", "factories"),
];

/// `runtime.create`'s overlay from resources: `{agents: [{name, …}], …}`, and what it leaves out (`kind/name`; a declared
/// goal a task names is inlined instead, by [`inline_goal`]).
#[must_use]
pub fn overlay_of(resources: &[Resource]) -> (Value, Vec<String>) {
    let mut out = Map::new();
    let mut left = Vec::new();
    for r in resources {
        let Some((_, section)) = SECTIONS.iter().find(|(k, _)| *k == r.kind) else {
            if !matches!(r.kind.as_str(), "goal" | "task") {
                left.push(format!("{}/{}", r.kind, r.name));
            }
            continue;
        };
        let mut spec = r.spec.as_object().cloned().unwrap_or_default();
        spec.insert("name".into(), json!(r.name));
        if let Value::Array(list) = out.entry((*section).to_owned()).or_insert_with(|| json!([])) {
            list.retain(|x| x.get("name") != Some(&json!(r.name)));
            list.push(Value::Object(spec));
        }
    }
    (Value::Object(out), left)
}

/// `b` over `a`, section by section: a named item of `b` replaces `a`'s of that name (a list or a map form).
#[must_use]
pub fn merge_overlays(a: &Value, b: &Value) -> Value {
    let mut out = a.as_object().cloned().unwrap_or_default();
    for (k, v) in b.as_object().into_iter().flatten() {
        let mut list = items(out.get(k));
        for (n, spec) in items(Some(v)) {
            list.retain(|(m, _)| *m != n);
            list.push((n, spec));
        }
        let arr: Vec<Value> = list
            .into_iter()
            .map(|(n, s)| {
                let mut o = s.as_object().cloned().unwrap_or_default();
                o.insert("name".into(), json!(n));
                Value::Object(o)
            })
            .collect();
        out.insert(k.clone(), Value::Array(arr));
    }
    Value::Object(out)
}

/// A section's `(name, spec)`s, as a list of named items or a map.
fn items(v: Option<&Value>) -> Vec<(String, Value)> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| {
                let mut o = x.as_object()?.clone();
                let n = o.remove("name")?.as_str()?.to_owned();
                Some((n, Value::Object(o)))
            })
            .collect(),
        Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => Vec::new(),
    }
}

/// An overlay written inline (a task file's `formula: { agents: [...] }`, as `wisp task export` writes it) as the
/// resources it declares of the sections a task reads; anything else in it (`crewMember`, `team`: what the task ran
/// with, for the record) isn't a resource.
#[must_use]
pub fn inline_resources(v: &Value) -> Vec<Resource> {
    let mut out = Vec::new();
    for (kind, section) in SECTIONS {
        for (n, spec) in items(v.get(section)) {
            if spec.is_object() {
                out.push(Resource::new(kind, n, spec));
            }
        }
    }
    out
}

/// The names an overlay declares of `section` (`agents`).
#[must_use]
pub fn declared_in(overlay: &Value, section: &str) -> Vec<String> {
    items(overlay.get(section)).into_iter().map(|(n, _)| n).collect()
}

/// What a task file's `formula:` names: a formula inline (as `wisp task export` writes it), a file (relative to the task
/// file), or a module of the folder's formula (`module.team`).
#[derive(Debug, Clone, PartialEq)]
pub enum FormulaRef {
    Inline(Value),
    File(String),
    Module(String),
}

/// A task file's `formula:`: nothing, one (a path, `module.x`, or a formula inline) or a list of them, in order.
///
/// # Errors
/// Anything else, saying what it takes.
pub fn formula_refs(v: Option<&Value>) -> Result<Vec<FormulaRef>, String> {
    let one = |x: &Value| -> Result<FormulaRef, String> {
        match x {
            Value::Object(_) => Ok(FormulaRef::Inline(x.clone())),
            Value::String(s) if s.trim().is_empty() => Err("formula: an empty name".into()),
            Value::String(s) => Ok(match s.trim().strip_prefix("module.") {
                Some(m) if !m.is_empty() && !m.contains('/') && !m.contains('\\') => FormulaRef::Module(m.to_owned()),
                _ => FormulaRef::File(s.trim().to_owned()),
            }),
            other => {
                Err(format!("formula: {other} isn't a formula: a file (./wisp.yml), a module (module.team), a list of them, or one inline"))
            }
        }
    };
    match v {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(a)) => a.iter().map(one).collect(),
        Some(x) => Ok(vec![one(x)?]),
    }
}

/// The task file's formula files that loop: a file named twice in one list, or the task file itself.
#[must_use]
pub fn looping_files(task_file: &str, files: &[String]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for f in files {
        if f == task_file {
            return Some(format!("formula: {f} is this task file itself"));
        }
        if seen.contains(&f.as_str()) {
            return Some(format!("formula: {f} is named twice"));
        }
        seen.push(f);
    }
    None
}

/// `v` with each string that is one of `names`' keys renamed to its value.
fn rename(v: &Value, names: &BTreeMap<String, String>) -> Value {
    match v {
        Value::String(s) => names.get(s).map_or_else(|| v.clone(), |n| json!(n)),
        Value::Array(a) => Value::Array(a.iter().map(|x| rename(x, names)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), rename(x, names))).collect()),
        other => other.clone(),
    }
}

/// Module `m`'s resources from a folder's configuration, under the names they have inside it (`reviewer`, not
/// `team-reviewer`): a task file that says `formula: module.team` names them as the module does. A reference inside the
/// module to one of them is renamed with it.
#[must_use]
pub fn module_resources(c: &Config, m: &str) -> Option<Vec<Resource>> {
    let at = format!("module.{m}.");
    let prefix = format!("{m}-");
    if !c.modules.iter().any(|(name, _)| name == m) {
        return None;
    }
    let renamed: BTreeMap<String, String> = c
        .addresses
        .iter()
        .filter(|(a, ..)| a.starts_with(&at))
        .map(|(_, _, stored, _)| (stored.clone(), stored.strip_prefix(&prefix).unwrap_or(stored).to_owned()))
        .collect();
    Some(
        c.resources
            .iter()
            .filter(|r| renamed.contains_key(&r.name))
            .map(|r| Resource::new(r.kind.clone(), renamed[&r.name].clone(), rename(&r.spec, &renamed)))
            .collect(),
    )
}

/// The fields of a task (a task file's `task:`, a declared task, a launch) that name something, with the kind each names.
#[must_use]
pub fn task_name_fields() -> Vec<Names> {
    vec![
        Names::new("agent", "agent"),
        Names::new("connectors[]", "connector"),
        Names::new("crew", "crew"),
        Names::new("team", "team"),
        Names::new("goal", "goal"),
        Names::new("goal.name", "goal"),
        Names::new("project", "project"),
        Names::new("cwd", "project"),
    ]
}

/// The declared goal a task names in `formula` (`goal: green-build`, or `{ name: green-build, maxRounds: 3 }`), written
/// in: `goals.default`'s fields under the goal's under the task's own, without its name (nothing was applied, so wisp.goal
/// couldn't look it up). True when it was one of the formula's.
pub fn inline_goal(task: &mut Value, formula: &[Resource]) -> bool {
    let (name, own) = match task.get("goal") {
        Some(Value::String(s)) => (s.trim().to_owned(), Map::new()),
        Some(Value::Object(o)) => match o.get("name").and_then(Value::as_str) {
            Some(n) => (n.trim().to_owned(), o.iter().filter(|(k, _)| *k != "name").map(|(k, v)| (k.clone(), v.clone())).collect()),
            None => return false,
        },
        _ => return false,
    };
    let goal = |n: &str| formula.iter().find(|r| r.kind == "goal" && r.name == n).and_then(|r| r.spec.as_object().cloned());
    let Some(declared) = goal(&name).filter(|_| name != "default") else { return false };
    let mut g = goal("default").unwrap_or_default();
    g.extend(declared);
    g.extend(own);
    task["goal"] = Value::Object(g);
    true
}

/// The names `task` gives that name nothing in `known` (kind → names there are: the formula's and this wisp's), each
/// said as the plan says it: `<at>: no agent named critic: agents has reviewer (or codex, auto)`. `also` is what a field
/// takes besides (the harnesses, for an agent); `sections` names each kind's section.
#[must_use]
pub fn unknown_names(
    at: &str,
    task: &Value,
    known: &BTreeMap<String, Vec<String>>,
    also: &BTreeMap<String, Vec<String>>,
    sections: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut out = Vec::new();
    for n in task_name_fields() {
        // a goal in words is what done means, not a name (`goal: "the build is green"`)
        if n.field == "goal" && task.get("goal").and_then(Value::as_str).is_some_and(|g| g.contains(char::is_whitespace)) {
            continue;
        }
        // a folder is a project's name only when it's a bare name (a path is a path)
        for name in names_in(task, &n) {
            let have = known.get(&n.kind).map_or(&[][..], Vec::as_slice);
            let more = also.get(&n.kind).map_or(&[][..], Vec::as_slice);
            if have.iter().chain(more).any(|h| *h == name) || (n.kind == "project" && have.iter().any(|h| h.eq_ignore_ascii_case(&name))) {
                continue;
            }
            let section = sections.get(&n.kind).cloned().unwrap_or_else(|| n.kind.clone());
            let mut has = if have.is_empty() { format!("no {section} are declared") } else { format!("{section} has {}", some_of(have)) };
            if !more.is_empty() {
                has = format!("{has} (or {})", some_of(more));
            }
            let line = format!("{at}: no {} named {name}: {has}", n.kind);
            if !out.contains(&line) {
                out.push(line);
            }
        }
    }
    out
}

/// At most [`NAMES_SHOWN`] names, then how many more: a refusal names what there is without listing 32 goals in a toast
/// 13 lines tall (audit G10).
pub fn some_of<S: AsRef<str>>(names: &[S]) -> String {
    let shown: Vec<&str> = names.iter().take(NAMES_SHOWN).map(AsRef::as_ref).collect();
    match names.len().saturating_sub(NAMES_SHOWN) {
        0 => shown.join(", "),
        more => format!("{} and {more} more", shown.join(", ")),
    }
}

/// How many names a refusal lists.
pub const NAMES_SHOWN: usize = 8;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Audit G10: "no goal named tiny: goals has goal-01, goal-02, … goal-30, green-build, never" in a toast 13 lines tall.
    #[test]
    fn a_refusal_lists_eight_names_then_how_many_more_audit_g10() {
        let goals: Vec<String> = (1..=32).map(|i| format!("goal-{i:02}")).collect();
        assert_eq!(some_of(&goals), "goal-01, goal-02, goal-03, goal-04, goal-05, goal-06, goal-07, goal-08 and 24 more");
        assert_eq!(some_of(&["a", "b"]), "a, b");
    }

    fn r(kind: &str, name: &str, spec: Value) -> Resource {
        Resource::new(kind, name, spec)
    }

    /// The task-readable sections are the overlay; a goal and a task are used, not "left out"; the rest is said.
    #[test]
    fn a_formula_is_a_tasks_overlay_and_says_what_it_leaves_out() {
        let (o, left) = overlay_of(&[
            r("agent", "critic", json!({ "harness": "codex" })),
            r("connector", "linear", json!({ "url": "https://x" })),
            r("goal", "green", json!({ "text": "green" })),
            r("task", "fix", json!({ "prompt": "fix" })),
            r("crew", "ada", json!({})),
        ]);
        assert_eq!(
            o,
            json!({ "agents": [{ "name": "critic", "harness": "codex" }], "connectors": [{ "name": "linear", "url": "https://x" }] })
        );
        assert_eq!(left, ["crew/ada"]);
        let merged = merge_overlays(&o, &json!({ "agents": { "critic": { "harness": "claude" } }, "teams": [{ "name": "t" }] }));
        assert_eq!(merged["agents"], json!([{ "name": "critic", "harness": "claude" }]), "the later one wins");
        assert_eq!(declared_in(&merged, "teams"), ["t"]);
        let inline =
            inline_resources(&json!({ "agents": [{ "name": "a", "harness": "codex" }], "connectors": ["linear"], "crewMember": "ada" }));
        assert_eq!(inline, [r("agent", "a", json!({ "harness": "codex" }))], "a snapshot's names-only lists aren't resources");
    }

    /// A task file's `formula:` is a file, a module, a list, or inline; a loop is refused.
    #[test]
    fn a_task_files_formula_is_files_modules_or_inline_and_never_loops() {
        assert_eq!(formula_refs(None).unwrap(), Vec::<FormulaRef>::new());
        assert_eq!(formula_refs(Some(&json!("./wisp.yml"))).unwrap(), [FormulaRef::File("./wisp.yml".into())]);
        assert_eq!(
            formula_refs(Some(&json!(["module.team", "../base.wisp.yml", { "agents": [] }]))).unwrap(),
            [FormulaRef::Module("team".into()), FormulaRef::File("../base.wisp.yml".into()), FormulaRef::Inline(json!({ "agents": [] }))]
        );
        assert!(formula_refs(Some(&json!(3))).unwrap_err().contains("isn't a formula"));
        assert_eq!(
            looping_files("/w/x.task.yml", &["/w/a.yml".into(), "/w/a.yml".into()]).as_deref(),
            Some("formula: /w/a.yml is named twice")
        );
        assert_eq!(
            looping_files("/w/x.task.yml", &["/w/x.task.yml".into()]).as_deref(),
            Some("formula: /w/x.task.yml is this task file itself")
        );
        assert_eq!(looping_files("/w/x.task.yml", &["/w/a.yml".into()]), None);
    }

    /// `formula: module.team`: the module's resources under its own names, its references renamed with them.
    #[test]
    fn a_modules_resources_keep_the_names_they_have_inside_it() {
        let c = Config {
            resources: vec![
                r("agent", "team-reviewer", json!({ "skills": ["team-testing"] })),
                r("skill", "team-testing", json!({ "body": "x" })),
                r("agent", "mine", json!({})),
            ],
            addresses: vec![
                ("module.team.agent.reviewer".into(), "agent".into(), "team-reviewer".into(), String::new()),
                ("module.team.skill.testing".into(), "skill".into(), "team-testing".into(), String::new()),
                ("agent.mine".into(), "agent".into(), "mine".into(), String::new()),
            ],
            modules: vec![("team".into(), "./team".into())],
            ..Config::default()
        };
        let got = module_resources(&c, "team").unwrap();
        assert_eq!(got, [r("agent", "reviewer", json!({ "skills": ["testing"] })), r("skill", "testing", json!({ "body": "x" }))]);
        assert_eq!(module_resources(&c, "nosuch"), None);
    }

    /// A declared goal the task names is inlined from the formula: the defaults, the goal, the task's own limit.
    #[test]
    fn a_goal_by_name_is_the_formulas_goal_inlined() {
        let f = [
            r("goal", "default", json!({ "vote": "all", "maxRounds": 5 })),
            r("goal", "green", json!({ "text": "the build is green", "judges": ["critic"] })),
        ];
        let mut t = json!({ "goal": "green" });
        assert!(inline_goal(&mut t, &f));
        assert_eq!(t["goal"], json!({ "vote": "all", "maxRounds": 5, "text": "the build is green", "judges": ["critic"] }));
        let mut t = json!({ "goal": { "name": "green", "maxRounds": 2 } });
        assert!(inline_goal(&mut t, &f));
        assert_eq!(t["goal"]["maxRounds"], 2, "the task's own limit wins");
        let mut t = json!({ "goal": "stored-one" });
        assert!(!inline_goal(&mut t, &f), "not the formula's: wisp.goal looks it up");
        assert!(!inline_goal(&mut json!({ "goal": "default" }), &f), "goals.default is no goal to name");
    }

    /// Every name a task gives is checked, said as the plan says it; a goal in words and a path aren't names.
    #[test]
    fn a_tasks_names_are_checked_and_unknown_ones_say_what_there_is() {
        let known: BTreeMap<String, Vec<String>> = [
            ("agent".to_owned(), vec!["reviewer".to_owned()]),
            ("connector".to_owned(), vec!["linear".to_owned()]),
            ("project".to_owned(), vec!["Web".to_owned()]),
        ]
        .into();
        let also: BTreeMap<String, Vec<String>> = [("agent".to_owned(), vec!["codex".to_owned(), "auto".to_owned()])].into();
        let sections: BTreeMap<String, String> = [
            ("agent", "agents"),
            ("connector", "connectors"),
            ("crew", "crew"),
            ("goal", "goals"),
            ("project", "projects"),
            ("team", "teams"),
        ]
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect();
        let ok = json!({ "agent": "codex", "connectors": ["linear"], "goal": "the build is green", "cwd": "/w/app", "project": "web" });
        assert_eq!(unknown_names("x.task.yml", &ok, &known, &also, &sections), Vec::<String>::new());
        let bad = json!({ "agent": "critic", "connectors": ["jira"], "crew": "ada", "goal": "green", "team": "core" });
        assert_eq!(
            unknown_names("x.task.yml", &bad, &known, &also, &sections),
            [
                "x.task.yml: no agent named critic: agents has reviewer (or codex, auto)",
                "x.task.yml: no connector named jira: connectors has linear",
                "x.task.yml: no crew named ada: no crew are declared",
                "x.task.yml: no team named core: no teams are declared",
                "x.task.yml: no goal named green: no goals are declared",
            ]
        );
    }
}
