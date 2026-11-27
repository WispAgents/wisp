//! The generic plan (ADR-0026): a desired spec is a patch over the current one (objects merge, the rest is
//! replaced), so a formula that leaves a field out keeps its value, and an export applied back is a no-op.
//! Pure: current and desired in, ops out.

use serde_json::{Map, Value};
use wisp_resources_api::{FieldChange, KindInfo, Op, OpKind, Resource};

/// `desired` over `current`: objects merge key by key, anything else is `desired`.
pub fn merge(current: &Value, desired: &Value) -> Value {
    match (current, desired) {
        (Value::Object(c), Value::Object(d)) => {
            let mut out: Map<String, Value> = c.clone();
            for (k, v) in d {
                let next = c.get(k).map_or_else(|| v.clone(), |had| merge(had, v));
                out.insert(k.clone(), next);
            }
            Value::Object(out)
        }
        _ => desired.clone(),
    }
}

/// The fields that differ between `from` and `to`, by path.
#[allow(clippy::many_single_char_names)]
pub fn diff(from: &Value, to: &Value, path: &str, out: &mut Vec<FieldChange>) {
    match (from, to) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let p = if path.is_empty() { k.clone() } else { format!("{path}/{k}") };
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => diff(x, y, &p, out),
                    (x, y) => out.push(FieldChange { path: p, from: x.cloned(), to: y.cloned() }),
                }
            }
        }
        (a, b) if a != b => out.push(FieldChange { path: path.to_owned(), from: Some(a.clone()), to: Some(b.clone()) }),
        _ => {}
    }
}

/// One kind's ops: creates and updates in the formula's order, then (with `prune`) deletes of what it
/// doesn't declare. Returns the ops and how many declared resources are already so.
pub fn plan(current: &[Resource], desired: &[Resource], prune: bool) -> (Vec<Op>, usize) {
    let mut ops = Vec::new();
    let mut same = 0;
    for d in desired {
        match current.iter().find(|c| c.name == d.name) {
            None => ops.push(Op {
                op: OpKind::Create,
                kind: d.kind.clone(),
                name: d.name.clone(),
                spec: d.spec.clone(),
                id: None,
                changes: Vec::new(),
                before: None,
            }),
            Some(c) => {
                let next = merge(&c.spec, &d.spec);
                if next == c.spec {
                    same += 1;
                } else {
                    let mut changes = Vec::new();
                    diff(&c.spec, &next, "", &mut changes);
                    ops.push(Op {
                        op: OpKind::Update,
                        kind: d.kind.clone(),
                        name: d.name.clone(),
                        spec: next,
                        id: c.id.clone(),
                        changes,
                        before: Some(c.spec.clone()),
                    });
                }
            }
        }
    }
    if prune {
        for c in current.iter().filter(|c| !desired.iter().any(|d| d.name == c.name)) {
            ops.push(Op {
                op: OpKind::Delete,
                kind: c.kind.clone(),
                name: c.name.clone(),
                spec: c.spec.clone(),
                id: c.id.clone(),
                changes: Vec::new(),
                before: None,
            });
        }
    }
    (ops, same)
}

/// How `--prune` picks what goes (AAC-1): not at all; only what this formula's source applied before (`--prune`);
/// every undeclared resource of the kinds it names, whoever applied it (`--prune-all`, and an API call naming no source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prune {
    Off,
    Own,
    All,
}

/// What [`own`] leaves of a kind's plan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pruned {
    pub ops: Vec<Op>,
    /// `kind/name` the formula doesn't declare but another source (or none recorded) applied: `--prune` keeps them
    pub kept: Vec<String>,
    /// `kind/name` this source applied before and no longer declares, without `--prune`: what it would remove
    pub stale: Vec<String>,
}

/// A kind's ops (planned with prune on when `mode` isn't `Off`) with the deletes cut to what this `source` applied: a
/// repo's `--prune` deleted the agents another repo, another `-f` file or the app had made, as they weren't in its
/// files. `from(name)` is the source the resource's last apply recorded (None: unknown, never pruned by `Own`).
pub fn own(
    ops: Vec<Op>,
    current: &[Resource],
    desired: &[Resource],
    from: &dyn Fn(&str) -> Option<String>,
    source: Option<&str>,
    mode: Prune,
) -> Pruned {
    let mine = |name: &str| source.is_some() && from(name).as_deref() == source;
    let mut out = Pruned::default();
    match mode {
        Prune::All => out.ops = ops,
        Prune::Own => {
            for o in ops {
                if o.op == OpKind::Delete && !mine(&o.name) {
                    out.kept.push(format!("{}/{}", o.kind, o.name));
                } else {
                    out.ops.push(o);
                }
            }
        }
        Prune::Off => {
            out.ops = ops.into_iter().filter(|o| o.op != OpKind::Delete).collect();
            out.stale = current
                .iter()
                .filter(|c| !desired.iter().any(|d| d.name == c.name) && mine(&c.name))
                .map(|c| format!("{}/{}", c.kind, c.name))
                .collect();
        }
    }
    out
}

/// The op that undoes `done`, given what the kind holds now (`now`: its resource of that name, if any).
/// None: nothing to undo (a create that left nothing, an update with no spec before it).
pub fn undo(done: &Op, now: Option<&Resource>) -> Option<Op> {
    let id = now.and_then(|r| r.id.clone()).or_else(|| done.id.clone());
    match done.op {
        OpKind::Create => now.map(|r| Op {
            op: OpKind::Delete,
            kind: done.kind.clone(),
            name: done.name.clone(),
            spec: r.spec.clone(),
            id,
            changes: Vec::new(),
            before: None,
        }),
        OpKind::Update => {
            let before = done.before.clone()?;
            let mut changes = Vec::new();
            diff(now.map_or(&done.spec, |r| &r.spec), &before, "", &mut changes);
            Some(Op { op: OpKind::Update, kind: done.kind.clone(), name: done.name.clone(), spec: before, id, changes, before: None })
        }
        OpKind::Delete => Some(Op {
            op: OpKind::Create,
            kind: done.kind.clone(),
            name: done.name.clone(),
            spec: done.spec.clone(),
            id: None,
            changes: Vec::new(),
            before: None,
        }),
    }
}

/// The order ops run in: creates and updates by kind order (projects before crew), deletes after, in
/// reverse (crew before projects).
pub fn order(ops: &mut [Op], kinds: &[KindInfo]) {
    let rank = |k: &str| kinds.iter().find(|x| x.kind == k).map_or(100, |x| x.order);
    ops.sort_by_key(|o| if o.op == OpKind::Delete { (1, -rank(&o.kind)) } else { (0, rank(&o.kind)) });
}

/// A value on one line for a diff (`"opus"`, `4`, `[2 items]`).
pub fn short(v: Option<&Value>) -> String {
    match v {
        None => "—".to_owned(),
        Some(Value::String(s)) if s.chars().count() > 40 => format!("{:?}", format!("{}…", s.chars().take(39).collect::<String>())),
        Some(Value::String(s)) => format!("{s:?}"),
        Some(Value::Array(a)) if a.len() > 3 => format!("[{} items]", a.len()),
        Some(Value::Object(o)) if o.len() > 3 => format!("{{{} fields}}", o.len()),
        Some(other) => other.to_string(),
    }
}

/// The plan as lines: a mark (`+`, `~`, `-`) and the text.
pub fn lines(ops: &[Op]) -> Vec<(char, String)> {
    let mut out = Vec::new();
    for o in ops {
        let what = format!("{}/{}", o.kind, o.name);
        match o.op {
            OpKind::Create => {
                out.push(('+', format!("{what}  (new)")));
                // a kind that knows what a new one changes says it (a setting from its default: `true → false`, audit E2)
                for c in &o.changes {
                    let field = if c.path.is_empty() { String::new() } else { format!("{}: ", c.path) };
                    out.push((' ', format!("    {field}{} → {}", short(c.from.as_ref()), short(c.to.as_ref()))));
                }
            }
            OpKind::Delete => out.push(('-', format!("{what}  (goes)"))),
            OpKind::Update => {
                out.push(('~', what));
                for c in &o.changes {
                    // a whole value (a setting's) has no field name: the values alone, no bare `:` (audit D6)
                    let field = if c.path.is_empty() { String::new() } else { format!("{}: ", c.path) };
                    out.push((' ', format!("    {field}{} → {}", short(c.from.as_ref()), short(c.to.as_ref()))));
                }
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn r(name: &str, spec: Value) -> Resource {
        Resource::new("agent", name, spec)
    }

    /// Audit E2: a new setting's plan line said `(new)` and nothing of its value; a create that says what it changes
    /// (the setting kind's, from its default) shows it under its line, as an update does.
    #[test]
    fn a_create_with_changes_shows_them_audit_e2() {
        let op = Op {
            op: OpKind::Create,
            kind: "setting".into(),
            name: "crew.tint".into(),
            spec: json!(false),
            id: None,
            changes: vec![FieldChange { path: String::new(), from: Some(json!(true)), to: Some(json!(false)) }],
            before: None,
        };
        assert_eq!(lines(&[op]), [('+', "setting/crew.tint  (new)".to_owned()), (' ', "    true → false".to_owned())]);
        assert_eq!(lines(&[Op { changes: Vec::new(), ..r_op() }]), [('+', "agent/a  (new)".to_owned())]);
    }

    fn r_op() -> Op {
        Op { op: OpKind::Create, kind: "agent".into(), name: "a".into(), spec: json!({}), id: None, changes: Vec::new(), before: None }
    }

    #[test]
    fn a_desired_spec_patches_the_current_one() {
        let cur =
            [r("reviewer", json!({ "harness": "codex", "model": "gpt-5", "effort": "high" })), r("fixer", json!({ "harness": "claude" }))];
        let want = [
            r("reviewer", json!({ "model": "gpt-5-pro" })),
            r("fixer", json!({ "harness": "claude" })),
            r("writer", json!({ "harness": "claude" })),
        ];
        let (ops, same) = plan(&cur, &want, false);
        assert_eq!(same, 1);
        assert_eq!(
            ops.iter().map(|o| (o.op, o.name.as_str())).collect::<Vec<_>>(),
            [(OpKind::Update, "reviewer"), (OpKind::Create, "writer")]
        );
        assert_eq!(
            ops[0].spec,
            json!({ "harness": "codex", "model": "gpt-5-pro", "effort": "high" }),
            "effort kept: the formula left it out"
        );
        assert_eq!(ops[0].changes, [FieldChange { path: "model".into(), from: Some(json!("gpt-5")), to: Some(json!("gpt-5-pro")) }]);
        let (ops, _) = plan(&cur, &want[..1], true);
        assert!(ops.iter().any(|o| o.op == OpKind::Delete && o.name == "fixer"), "prune deletes what isn't declared");
    }

    /// Audit D6: a setting's value has no field name, so its change printed `    : "zed" → "cursor"`. A change of the
    /// whole value is the values alone; a field's keeps its name.
    #[test]
    fn a_whole_value_change_prints_without_a_bare_colon_audit_d6() {
        let cur = [Resource::new("setting", "settings.editor", json!("zed"))];
        let (ops, _) = plan(&cur, &[Resource::new("setting", "settings.editor", json!("cursor"))], false);
        assert_eq!(lines(&ops), [('~', "setting/settings.editor".to_owned()), (' ', "    \"zed\" → \"cursor\"".to_owned())]);
        let (ops, _) = plan(&[r("a", json!({ "agent": "claude" }))], &[r("a", json!({ "agent": "codex" }))], false);
        assert_eq!(lines(&ops)[1].1, "    agent: \"claude\" → \"codex\"");
    }

    #[test]
    fn the_current_state_applied_back_is_a_no_op() {
        let cur = [r("reviewer", json!({ "harness": "codex", "connectors": ["linear"], "limits": { "usd": 5 } }))];
        assert_eq!(plan(&cur, &cur, true), (Vec::new(), 1));
    }

    #[test]
    fn each_op_has_its_undo() {
        let cur = [r("reviewer", json!({ "harness": "codex", "effort": "high" }))];
        let (ops, _) =
            plan(&cur, &[r("reviewer", json!({ "effort": "low", "model": "m" })), r("new", json!({ "harness": "claude" }))], false);
        let back = undo(&ops[0], Some(&r("reviewer", ops[0].spec.clone()))).unwrap();
        assert_eq!((back.op, back.spec.clone()), (OpKind::Update, json!({ "harness": "codex", "effort": "high" })));
        assert!(back.changes.iter().any(|c| c.path == "model" && c.to.is_none()), "a field it added goes again: {:?}", back.changes);
        let made = Resource { id: Some("c9".into()), ..r("new", json!({ "harness": "claude" })) };
        let del = undo(&ops[1], Some(&made)).unwrap();
        assert_eq!((del.op, del.id.as_deref()), (OpKind::Delete, Some("c9")), "by the id it got");
        assert!(undo(&ops[1], None).is_none(), "a create that left nothing");
    }

    /// AAC-1: helper came from another repo (B), critic from this one (A); A's formula now declares neither.
    fn by_source() -> (Vec<Resource>, impl Fn(&str) -> Option<String>) {
        let cur = vec![r("helper", json!({ "harness": "codex" })), r("critic", json!({ "harness": "claude" })), r("old", json!({}))];
        let from = |n: &str| match n {
            "helper" => Some("B".to_owned()),
            "critic" => Some("A".to_owned()),
            _ => None, // a row from before sources were kept
        };
        (cur, from)
    }

    #[test]
    fn prune_removes_only_what_this_source_applied_aac_1() {
        let (cur, from) = by_source();
        let (ops, _) = plan(&cur, &[], true);
        let p = own(ops, &cur, &[], &from, Some("A"), Prune::Own);
        assert_eq!(p.ops.iter().map(|o| (o.op, o.name.as_str())).collect::<Vec<_>>(), [(OpKind::Delete, "critic")]);
        assert_eq!(p.kept, ["agent/helper", "agent/old"], "another source's and an unknown one's are kept");
        assert_eq!(p.stale, Vec::<String>::new());
        // no source named: nothing is this formula's, nothing goes
        let (ops, _) = plan(&cur, &[], true);
        assert_eq!(own(ops, &cur, &[], &from, None, Prune::Own).ops, Vec::<Op>::new());
    }

    #[test]
    fn prune_all_removes_every_undeclared_one_aac_1() {
        let (cur, from) = by_source();
        let (ops, _) = plan(&cur, &[], true);
        let p = own(ops, &cur, &[], &from, Some("A"), Prune::All);
        assert_eq!(p.ops.iter().filter(|o| o.op == OpKind::Delete).count(), 3);
        assert_eq!(p.kept, Vec::<String>::new());
    }

    #[test]
    fn without_prune_what_this_source_no_longer_declares_is_named_aac_1() {
        let (cur, from) = by_source();
        let (ops, _) = plan(&cur, &[r("new", json!({}))], false);
        let p = own(ops, &cur, &[r("new", json!({}))], &from, Some("A"), Prune::Off);
        assert_eq!(p.stale, ["agent/critic"]);
        assert_eq!(p.ops.iter().map(|o| o.op).collect::<Vec<_>>(), [OpKind::Create]);
        assert_eq!(p.kept, Vec::<String>::new());
    }

    #[test]
    fn deletes_go_last_and_in_reverse() {
        let kinds =
            [KindInfo::new("project", "projects", "x", json!({})).order(10), KindInfo::new("crew", "crew", "x", json!({})).order(50)];
        let op = |o: OpKind, k: &str| Op {
            op: o,
            kind: k.into(),
            name: "n".into(),
            spec: Value::Null,
            id: None,
            changes: Vec::new(),
            before: None,
        };
        let mut ops =
            vec![op(OpKind::Delete, "project"), op(OpKind::Create, "crew"), op(OpKind::Delete, "crew"), op(OpKind::Create, "project")];
        order(&mut ops, &kinds);
        let got: Vec<(OpKind, &str)> = ops.iter().map(|o| (o.op, o.kind.as_str())).collect();
        assert_eq!(got, [(OpKind::Create, "project"), (OpKind::Create, "crew"), (OpKind::Delete, "crew"), (OpKind::Delete, "project")]);
    }
}
