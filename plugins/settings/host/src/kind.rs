//! The `setting` resource kind (ADR-0026): every setting a user set, by key, in a formula's `settings:`
//! map. Keys a plugin declares as part of its own kind (`claims_settings`, such as `budget:`) are left
//! to that kind; device settings belong to `ui`, secrets never go in a formula.

use serde_json::{Map, Value, json};
use wisp_kernel::{BoxFuture, Call, Ctx, Result, SettingInfo};
use wisp_resources_api::{KindInfo, Op, OpKind, RESOURCE_KIND, Resource, ResourceKind};

#[derive(Debug)]
pub struct SettingKind {
    pub ctx: Ctx,
}

fn schema_of(s: &SettingInfo) -> Value {
    let d = &s.decl;
    let mut p = match d.ty.as_str() {
        "int" => json!({ "type": "integer" }),
        "bool" => json!({ "type": "boolean" }),
        "enum" => json!({ "type": "string", "enum": d.values.clone().unwrap_or_default() }),
        "string" => json!({ "type": "string" }),
        _ => json!({}),
    };
    if let Some(m) = d.min {
        p["minimum"] = json!(m);
    }
    if let Some(m) = d.max {
        p["maximum"] = json!(m);
    }
    p["description"] = json!(d.title.clone().unwrap_or_else(|| format!("{} ({})", s.key, s.owner)));
    p
}

impl SettingKind {
    /// The settings this kind declares: server-scoped, not secret, not part of another kind.
    fn mine(&self) -> Vec<SettingInfo> {
        let claimed: Vec<String> = self.ctx.contributions(&RESOURCE_KIND).iter().filter_map(|k| k.claims_settings()).collect();
        self.ctx
            .settings_list()
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s.scope != "device" && !s.decl.secret && !claimed.contains(&s.owner))
            .collect()
    }
}

/// The settings' plan (audit E2): a setting is one value, so a declared one is the value it will have. One not set yet is
/// at its default: the same value again is unchanged (no op), and another one says what it changes from, the default
/// (`crew.tint: true → false`), never a bare "(new)". A set one changes as any spec does; `prune` unsets the rest.
fn plan_settings(current: &[Resource], desired: &[Resource], default_of: impl Fn(&str) -> Option<Value>, prune: bool) -> Vec<Op> {
    use wisp_resources_api::FieldChange;
    let mut ops = Vec::new();
    for d in desired {
        let default = default_of(&d.name);
        let had = current.iter().find(|c| c.name == d.name);
        // `null` is "back to its default"
        let want = if d.spec.is_null() { default.clone().unwrap_or(Value::Null) } else { d.spec.clone() };
        let now = had.map(|c| c.spec.clone()).or_else(|| default.clone()).unwrap_or(Value::Null);
        if want == now {
            continue;
        }
        let change = vec![FieldChange { path: String::new(), from: (!now.is_null()).then(|| now.clone()), to: Some(want.clone()) }];
        ops.push(match had {
            Some(c) => Op {
                op: OpKind::Update,
                kind: d.kind.clone(),
                name: d.name.clone(),
                spec: d.spec.clone(),
                id: c.id.clone(),
                changes: change,
                before: Some(c.spec.clone()),
            },
            None => Op {
                op: OpKind::Create,
                kind: d.kind.clone(),
                name: d.name.clone(),
                spec: d.spec.clone(),
                id: None,
                changes: change,
                before: None,
            },
        });
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
    ops
}

impl ResourceKind for SettingKind {
    fn info(&self) -> KindInfo {
        let props: Map<String, Value> = self.mine().iter().map(|s| (s.key.clone(), schema_of(s))).collect();
        let schema = json!({ "type": "object", "description": "Settings by key (wisp settings list)", "properties": props, "additionalProperties": false });
        KindInfo::new("setting", "settings", wisp_settings_api::PLUGIN_ID, schema).map_form().order(5)
    }

    fn read<'a>(&'a self, _call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        Box::pin(async move {
            Ok(self
                .mine()
                .into_iter()
                .filter(|s| s.locked_by.is_none())
                .filter_map(|s| {
                    let v = s.value.clone()?;
                    (s.decl.default.as_ref() != Some(&v)).then(|| Resource::new("setting", s.key, v))
                })
                .collect())
        })
    }

    fn plan(&self, current: &[Resource], desired: &[Resource], prune: bool) -> Option<Vec<Op>> {
        let list = self.ctx.settings_list().unwrap_or_default();
        let default_of = |name: &str| {
            let key = wisp_settings_api::key_of(name);
            list.iter().find(|s| s.key == key).and_then(|s| s.decl.default.clone())
        };
        Some(plan_settings(current, desired, default_of, prune))
    }

    /// The value as the setting takes it (`off` read by YAML as false is the enum's `"off"`, D3), so the plan shows it
    /// and an unchanged one plans as unchanged.
    fn normalize_named(&self, name: &str, spec: &mut Value) {
        let key = wisp_settings_api::key_of(name);
        if let Some(s) = self.ctx.settings_list().unwrap_or_default().into_iter().find(|s| s.key == key) {
            *spec = s.decl.coerce(std::mem::take(spec));
        }
    }

    /// The settings whose value names another kind's (ADR-0111): a channel's default project, the team new tasks start
    /// with. One naming nothing is refused, saying what there is.
    fn refers_to(&self) -> Vec<String> {
        vec!["project".to_owned(), "agent".to_owned()]
    }
    fn refs(&self, r: &Resource) -> Vec<wisp_resources_api::Ref> {
        let n = match wisp_settings_api::key_of(&r.name) {
            "channels.project" => wisp_resources_api::Names::new("", "project"),
            "runtime.subagents" => wisp_resources_api::Names::new("[].agent", "agent"),
            _ => return Vec::new(),
        };
        wisp_resources_api::names_in(&r.spec, &n)
            .into_iter()
            .map(|name| wisp_resources_api::Ref { kind: n.kind.clone(), name, field: r.name.clone() })
            .collect()
    }

    fn check(&self, r: &Resource) -> Vec<String> {
        let key = wisp_settings_api::key_of(&r.name);
        let Some(s) = self.ctx.settings_list().unwrap_or_default().into_iter().find(|s| s.key == key) else {
            return vec![format!("no setting named {key} (wisp settings list)")];
        };
        let mut out = Vec::new();
        if s.scope == "device" {
            out.push(format!("{key} is this device's: it goes under ui:"));
        }
        if let Some(by) = &s.locked_by {
            out.push(format!("{key} is set by {by}"));
        }
        // `worktrees.max: 20` in YAML is a number: the enum's "20" (setup audit F22)
        let spec = s.decl.coerce(r.spec.clone());
        let ok = match s.decl.ty.as_str() {
            "int" => spec.as_i64().is_some(),
            "bool" => spec.is_boolean(),
            "string" => spec.is_string(),
            "enum" => spec.as_str().is_some_and(|v| s.decl.values.as_ref().is_none_or(|vs| vs.iter().any(|x| x == v))),
            _ => true,
        };
        if !ok && !r.spec.is_null() {
            out.push(format!(
                "{key} is {}{}, not {}",
                s.decl.ty,
                s.decl.values.as_ref().map(|v| format!(" ({})", v.join(", "))).unwrap_or_default(),
                r.spec
            ));
        }
        out
    }

    fn apply<'a>(&'a self, _call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let key = wisp_settings_api::key_of(&op.name).to_owned();
            let v = (op.op != OpKind::Delete && !op.spec.is_null()).then(|| op.spec.clone());
            self.ctx.set_setting(&key, v)?;
            crate::durable(&self.ctx).await;
            Ok(())
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Audit E2: `crew.tint: false` (on by default) planned as `+ setting/crew.tint  (new)`, and Setup said "Adds …
    /// setting crew.tint", neither saying it turns a switch off; `crew.tint: true` was "new" too, changing nothing.
    #[test]
    fn a_setting_not_set_yet_plans_from_its_default_audit_e2() {
        let r = |k: &str, v: Value| Resource::new("setting", k, v);
        let defaults = |k: &str| match k {
            "crew.tint" => Some(json!(true)),
            "worktrees.max" => Some(json!("40")),
            _ => None,
        };
        let current = [r("worktrees.max", json!("80"))];
        let desired = [r("crew.tint", json!(false)), r("budget.drainWatch", json!(true)), r("worktrees.max", json!("off"))];
        let ops = plan_settings(&current, &desired, defaults, false);
        let said: Vec<(OpKind, &str, Option<Value>, Option<Value>)> =
            ops.iter().map(|o| (o.op, o.name.as_str(), o.changes[0].from.clone(), o.changes[0].to.clone())).collect();
        assert_eq!(
            said,
            [
                (OpKind::Create, "crew.tint", Some(json!(true)), Some(json!(false))),
                (OpKind::Create, "budget.drainWatch", None, Some(json!(true))),
                (OpKind::Update, "worktrees.max", Some(json!("80")), Some(json!("off"))),
            ]
        );
        assert_eq!(ops[2].before, Some(json!("80")));
        // its default again, or null while unset: nothing changes, so no op
        assert_eq!(plan_settings(&[], &[r("crew.tint", json!(true)), r("crew.tint", Value::Null)], defaults, false), Vec::<Op>::new());
        // pruned: one set and not declared goes back to its default
        let gone = plan_settings(&current, &[], defaults, true);
        assert_eq!((gone.len(), gone[0].op), (1, OpKind::Delete));
    }
}
