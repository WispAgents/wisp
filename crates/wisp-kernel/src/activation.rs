//! Activation events (plan §6.4, VS Code's model): a lazy plugin's code loads on its first event.
//! `onStartup`, `onCommand:<id>{k:v}`, `onTopic:<topic>`, `onRoute:[METHOD ]<path>`, `onView:<id>`,
//! `onBlock:<type>`, `onSchedule:<every>|<name>`, and `onTriggerSource:<on>` (implicit for `contributes.triggerSources`).
//! Names may glob with `*`.
use serde_json::Value;

use crate::manifest::Manifest;
use crate::manifest_check::route_activation;
use crate::perm::glob;

/// 'onCommand:runtime.start{harness:claude}' → ("onCommand", "runtime.start", [("harness", "claude")]).
pub fn parse(a: &str) -> (&str, &str, Vec<(&str, &str)>) {
    let (kind, mut name) = a.split_once(':').unwrap_or((a, ""));
    let mut filter = Vec::new();
    if kind == "onCommand"
        && let Some((n, f)) = name.split_once('{')
        && let Some(f) = f.strip_suffix('}')
    {
        name = n;
        filter = f.split(',').filter_map(|p| p.split_once(':')).map(|(k, v)| (k.trim(), v.trim())).collect();
    }
    (kind, name, filter)
}

fn arg_str(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

/// Does this manifest wait on (`kind`, `name`)? Commands it lists in `contributes.commands` count as
/// `onCommand` implicitly, so a lazy plugin can't hide a command it declares.
pub fn wakes(m: &Manifest, kind: &str, name: &str, args: Option<&Value>) -> bool {
    if kind == "onCommand" && m.commands().iter().any(|c| c.id == name) {
        return true;
    }
    // a trigger source it declares (`contributes.triggerSources`) wakes it when a trigger needs it, as a command it
    // declares does: a lazy plugin's polled source then runs where no one opened anything (a team hub, ADR-0148)
    if kind == "onTriggerSource"
        && m.contributes.get("triggerSources").and_then(Value::as_array).is_some_and(|l| l.iter().any(|t| t.as_str() == Some(name)))
    {
        return true;
    }
    m.activation.iter().any(|a| {
        let (k, n, filter) = parse(a);
        if k != kind {
            return false;
        }
        if kind == "onRoute" {
            return route_matches(n, name);
        }
        if !glob(n, name) {
            return false;
        }
        filter.iter().all(|(fk, fv)| args.and_then(|a| a.get(*fk)).is_some_and(|v| glob(fv, &arg_str(v))))
    })
}

/// `name` is "METHOD /path"; the activation is "[METHOD ]/pattern" with `*` globs.
fn route_matches(activation: &str, name: &str) -> bool {
    let Some((am, ap)) = route_activation(activation) else {
        return false;
    };
    let (m, path) = name.split_once(' ').unwrap_or(("*", name));
    (am == "*" || am == m) && glob(ap, path)
}

/// `onSchedule:5m` → 300000. Units: ms, s, m, h. A name (`onSchedule:nightly`) is woken by a scheduler.
pub fn schedule_ms(name: &str) -> Option<u64> {
    let digits = name.bytes().take_while(u8::is_ascii_digit).count();
    let n: u64 = name.get(..digits)?.parse().ok()?;
    let unit = match name.get(digits..)? {
        "ms" => 1,
        "s" => 1000,
        "m" => 60_000,
        "h" => 3_600_000,
        _ => return None,
    };
    n.checked_mul(unit).filter(|ms| *ms > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(activation: &[&str], commands: &[&str]) -> Manifest {
        let cmds: Vec<Value> = commands.iter().map(|c| serde_json::json!({ "id": c, "title": c })).collect();
        let mut contributes = std::collections::BTreeMap::new();
        contributes.insert("commands".to_owned(), Value::Array(cmds));
        Manifest { activation: activation.iter().map(|s| (*s).to_owned()).collect(), contributes, ..Manifest::default() }
    }

    #[test]
    fn events() {
        let x = m(
            &["onCommand:runtime.start{harness:claude}", "onTopic:session:*", "onRoute:GET /api/x/*", "onView:shell.task"],
            &["claude.login"],
        );
        assert!(wakes(&x, "onCommand", "claude.login", None));
        assert!(wakes(&x, "onCommand", "runtime.start", Some(&serde_json::json!({ "harness": "claude" }))));
        assert!(!wakes(&x, "onCommand", "runtime.start", Some(&serde_json::json!({ "harness": "codex" }))));
        assert!(!wakes(&x, "onCommand", "runtime.start", None));
        assert!(wakes(&x, "onTopic", "session:abc", None));
        assert!(wakes(&x, "onRoute", "GET /api/x/1", None));
        assert!(!wakes(&x, "onRoute", "POST /api/x/1", None));
        assert!(wakes(&x, "onView", "shell.task", None));
        assert!(!wakes(&x, "onBlock", "chart", None));
    }

    /// ADR-0148: a trigger source a lazy plugin declares wakes it when a trigger needs it (a team hub's `issue.opened`
    /// polled where nobody opened the issues list), as a command it declares does; anything else it doesn't declare, not.
    #[test]
    fn a_declared_trigger_source_wakes_its_plugin() {
        let mut x = m(&["onCommand:issues.list"], &[]);
        x.contributes.insert("triggerSources".to_owned(), serde_json::json!(["issue.opened", "issue.labeled"]));
        assert!(wakes(&x, "onTriggerSource", "issue.opened", None));
        assert!(!wakes(&x, "onTriggerSource", "pr.opened", None));
        assert!(!wakes(&m(&[], &[]), "onTriggerSource", "issue.opened", None));
    }

    #[test]
    fn schedules() {
        assert_eq!(schedule_ms("5m"), Some(300_000));
        assert_eq!(schedule_ms("250ms"), Some(250));
        assert_eq!(schedule_ms("nightly"), None);
        assert_eq!(schedule_ms("0s"), None);
    }
}
