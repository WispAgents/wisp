//! The `crew` resource kind (ADR-0026): members as `crew:` in a formula, through the crew's own commands
//! (`crew.list`, `.create`, `.update`, `.remove`). A schedule in words is read as the crew reads it, so
//! "every weekday at 9" plans as unchanged against the cron it was saved as.

use serde_json::Value;
use wisp_crew_api::spec::CrewSpec;
use wisp_kernel::{BoxFuture, Call, Ctx, Result};
use wisp_resources_api::{CrudKind, KindInfo, Op, Resource, ResourceKind, schema_of};

#[derive(Debug)]
pub struct CrewKind(CrudKind);

impl CrewKind {
    pub fn new(ctx: Ctx) -> Self {
        use wisp_resources_api::Names;
        // what a member names (ADR-0111); a delegation that comes back round is refused
        let info = KindInfo::new("crew", "crew", wisp_crew_api::PLUGIN_ID, schema_of::<CrewSpec>())
            .order(60)
            .names(Names::new("agent", "agent"))
            .names(Names::new("machine", "machine"))
            .names(Names::new("team[].crew", "crew").acyclic())
            .names(Names::new("wakes[].task", "task"))
            .names(Names::new("wakes[].workflow", "workflow"))
            .names(Names::new("wakes[].crew", "crew"))
            .names(Names::new("wakes[].agent", "agent"))
            .names(Names::new("wakes[].project", "project"));
        Self(CrudKind::new::<CrewSpec>(info, ctx, "crew.list", "crew.create", "crew.update", "crew.remove"))
    }
}

/// A schedule as words or an alias → the 5-field cron the crew saves; anything else as it is.
pub fn cron_of(said: &str) -> String {
    let s = said.trim();
    if s.is_empty() || s.split_whitespace().count() == 5 {
        return s.to_owned();
    }
    let alias = match s {
        "@hourly" => Some("0 * * * *"),
        "@daily" => Some("0 0 * * *"),
        "@weekly" => Some("0 0 * * 0"),
        "@monthly" => Some("0 0 1 * *"),
        "@weekdays" => Some("0 9 * * 1-5"),
        _ => None,
    };
    alias.map(str::to_owned).or_else(|| wisp_automations_api::to_cron(s)).unwrap_or_else(|| s.to_owned())
}

impl ResourceKind for CrewKind {
    fn info(&self) -> KindInfo {
        self.0.info()
    }
    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        self.0.read(call)
    }
    fn normalize(&self, spec: &mut Value) {
        if let Some(c) = spec.get("cron").and_then(Value::as_str).map(cron_of) {
            spec["cron"] = Value::String(c);
        }
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        self.0.apply(call, op)
    }
    /// A schedule or a wake the crew would refuse is said by `wisp validate`, `diff` and `apply --dry-run`, not only
    /// half-way through an apply.
    fn check(&self, r: &Resource) -> Vec<String> {
        member_problems(r)
    }
    /// A loop of hand-offs (A wakes on B's report, B on A's) is refused at apply, naming it (ADR-0043).
    fn check_all(&self, after: &[Resource]) -> Vec<String> {
        wake_loops(after)
    }
}

/// What `crew.create`/`.update` would refuse in a member's spec: its `cron` (read as the crew reads a schedule) and
/// each wake (as a trigger aimed at the member), by their path.
pub fn member_problems(r: &Resource) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(said) = r.spec.get("cron").and_then(Value::as_str)
        && let Err(e) = wisp_crew_core::cron::read_schedule(&cron_of(said), &wisp_crew_core::cron::CronOnly)
    {
        out.push(format!("cron: {e}"));
    }
    for (i, w) in r.spec.get("wakes").and_then(Value::as_array).into_iter().flatten().enumerate() {
        match serde_json::from_value::<wisp_automations_api::TriggerSpec>(w.clone()) {
            Ok(t) => out.extend(wisp_crew_core::member::wake_problem(&r.name, &t).map(|p| format!("wakes[{i}]: {p}"))),
            Err(e) => out.push(format!("wakes[{i}]: not a trigger: {e}")),
        }
    }
    out
}

/// The crew in a plan, as it would be after it: a loop of hand-offs is a problem naming it.
pub fn wake_loops(after: &[Resource]) -> Vec<String> {
    let wakes: Vec<(String, wisp_automations_api::TriggerSpec)> = after
        .iter()
        .filter(|r| r.kind == "crew")
        .flat_map(|r| {
            let list: Vec<wisp_automations_api::TriggerSpec> =
                r.spec.get("wakes").and_then(|w| serde_json::from_value(w.clone()).ok()).unwrap_or_default();
            list.into_iter().map(move |w| (r.name.clone(), w))
        })
        .collect();
    wisp_automations_api::spec::wake_cycle(&wisp_automations_api::spec::wake_edges(&wakes))
        .map(|c| vec![format!("crew wakes go round in a loop: {} (each wakes on the one before's report)", c.join(" → "))])
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A cron the crew can't read and a wake it would refuse passed `wisp validate` and `apply --dry-run`, and failed
    /// only half-way through the apply: each is one problem, by its path; a member that's fine has none.
    #[test]
    fn a_bad_cron_or_wake_is_a_problem_before_the_apply() {
        let bad_cron = member_problems(&Resource::new("crew", "ada", json!({ "cron": "0 99 * * *" })));
        assert_eq!(bad_cron.len(), 1, "{bad_cron:?}");
        assert!(bad_cron[0].starts_with("cron: "), "{}", bad_cron[0]);
        let bad_wake = member_problems(&Resource::new("crew", "ada", json!({ "wakes": [{ "on": "pr.opened" }, { "on": "schedule" }] })));
        assert_eq!(bad_wake.len(), 1, "{bad_wake:?}");
        assert!(bad_wake[0].starts_with("wakes[1]: a schedule needs cron"), "{}", bad_wake[0]);
        let not_one = member_problems(&Resource::new("crew", "ada", json!({ "wakes": [{ "on": "pr.opened", "nope": 1 }] })));
        assert!(not_one.len() == 1 && not_one[0].starts_with("wakes[0]: not a trigger"), "{not_one:?}");
        let fine = json!({ "cron": "every weekday at 9", "wakes": [{ "on": "schedule", "cron": "0 9 * * *" }, { "on": "pr.opened" }] });
        assert!(member_problems(&Resource::new("crew", "ada", fine)).is_empty());
        assert!(member_problems(&Resource::new("crew", "ada", json!({ "cron": "" }))).is_empty());
    }

    #[test]
    fn crew_wake_cycle_refused_at_apply() {
        let member = |name: &str, after: &str| {
            Resource::new("crew", name, json!({ "wakes": [{ "on": "crew.reported", "match": { "member": after } }] }))
        };
        let fine = vec![member("bo", "ada"), member("cy", "bo"), Resource::new("crew", "ada", json!({}))];
        assert!(wake_loops(&fine).is_empty());
        let looped = vec![member("ada", "bo"), member("bo", "ada")];
        let p = wake_loops(&looped);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("ada → bo → ada") || p[0].contains("bo → ada → bo"), "names the loop: {}", p[0]);
        // the plan's after-set: a loop closed by a member the formula doesn't touch is still found
        let with_other_kind = vec![member("ada", "bo"), Resource::new("trigger", "bo", json!({})), member("bo", "ada")];
        assert_eq!(wake_loops(&with_other_kind).len(), 1);
    }
}
