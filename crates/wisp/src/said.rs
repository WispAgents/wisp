//! What a generated verb's result says, in a sentence, where its fields alone read as a dump (audit H8: `factory stop`
//! printed `stopping  true`, `crew checkin` `session  01a1f9… · Zed · Check-in · running`, `review maybe` `why  …`,
//! `crew update` every field of the member). Pure: the command, the args sent and the result in, the lines out; None:
//! the result's own fields say it (`--json` always prints the result).
use serde_json::Value;

fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// Whether field `k` of a member reads the same before and after (`on` is the member's wakes' names).
fn same(k: &str, before: &Value, out: &Value) -> bool {
    let of = |v: &Value| match k {
        "on" | "wakes" => {
            let names: Vec<Value> = v
                .get("wakes")
                .and_then(Value::as_array)
                .map(|w| w.iter().filter_map(|x| x.get("on").cloned()).collect())
                .or_else(|| v.get("on").and_then(Value::as_array).cloned())
                .unwrap_or_default();
            Value::Array(names)
        }
        _ => v.get(k).cloned().unwrap_or(Value::Null),
    };
    of(before) == of(out)
}

/// The sentence for `id`'s result, if it has one. `before`: what `crew.update` changed, read first (audit I7: it said
/// "on changed" when nothing had).
pub fn said(id: &str, args: &Value, out: &Value, before: Option<&Value>) -> Option<String> {
    match id {
        "factory.stop" => {
            let run = text(args, "id");
            Some(if out.get("stopping") == Some(&Value::Bool(true)) {
                format!("  ✓ stopping factory run {run}: its stations stop and the run ends Stopped (their tasks and worktrees are kept)")
            } else {
                format!("  · factory run {run} has nothing working to stop")
            })
        }
        "crew.checkin" | "crew.assign" => {
            let s = out.get("session")?;
            let (task, title, state) = (text(s, "id"), text(s, "title"), text(s, "state"));
            let state = if state.is_empty() { String::new() } else { format!(", {state}") };
            Some(format!("  ✓ {title}{state}\n    wisp attach {task} follows it"))
        }
        "review.maybe" => {
            let why = text(out, "why").trim();
            Some(if why.is_empty() {
                format!("  ✓ a review started: wisp show {} shows it once it's in", text(args, "id"))
            } else {
                format!("  · no review: {why}")
            })
        }
        "crew.update" => {
            let name = Some(text(out, "name")).filter(|n| !n.is_empty()).unwrap_or_else(|| text(args, "id"));
            let given: Vec<&str> = args.as_object()?.keys().map(String::as_str).filter(|k| *k != "id").collect();
            let changed: Vec<&str> = given.iter().copied().filter(|k| before.is_none_or(|b| !same(k, b, out))).collect();
            let what = match (changed.as_slice(), out.get("paused").and_then(Value::as_bool)) {
                (["paused"], Some(true)) => format!("paused (wisp crew resume {name} brings them back)"),
                (["paused"], Some(false)) => "back at work: no longer paused".to_owned(),
                ([], _) if !given.is_empty() => format!(
                    "nothing changed ({} already as given)",
                    given.iter().map(|k| wisp_cli_core::human::words(k)).collect::<Vec<_>>().join(", ")
                ),
                ([], _) => "nothing changed".to_owned(),
                (keys, _) => keys.iter().map(|k| wisp_cli_core::human::words(k)).collect::<Vec<_>>().join(", ") + " changed",
            };
            Some(format!("  ✓ {name}: {what}"))
        }
        // audit I12: `goal set` printed `judges / name agent / … / round 0 / set just now / status working / …`
        "goal.set" => {
            let task = text(args, "id");
            let what = Some(text(out, "text")).filter(|t| !t.is_empty()).or_else(|| Some(text(out, "name")).filter(|n| !n.is_empty()));
            let judges: Vec<&str> = out
                .get("judges")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|j| j.get("name").or_else(|| j.get("agent")).and_then(Value::as_str))
                .collect();
            let by = if judges.is_empty() { String::new() } else { format!(" · judged by {}", judges.join(", ")) };
            Some(format!("  ✓ goal set: {}{by}\n    wisp show {task} follows it", what.unwrap_or("the goal")))
        }
        // audit I12: `crew create` printed every field ("created in a moment" for a member just made)
        "crew.create" => {
            let (name, role, agent) = (text(out, "name"), text(out, "role"), text(out, "agent"));
            let role = if role.is_empty() { String::new() } else { format!(", {role}") };
            let agent = if agent.is_empty() { String::new() } else { format!(" · {agent}") };
            Some(format!("  ✓ {name} joined the crew{role}{agent}\n    wisp crew ls lists them; wisp crew assign {name} \"…\" gives a job"))
        }
        // audit I12: `review view` printed every field
        "review.view" => Some(review_words(text(args, "id"), out)),
        "later.cancel" => Some(match out.pointer("/entry/text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
            Some(t) => format!("  ✓ cancelled: {t}"),
            None => format!("  ✓ cancelled what was scheduled on {}", text(args, "sessionId")),
        }),
        "later.cancelPlan" => Some(format!("  ✓ plan {} taken back: it won't start", text(args, "id"))),
        _ => None,
    }
}

/// A task's review in sentences: who reviewed, what they found, one finding a line, and the tests' verdict.
fn review_words(task: &str, out: &Value) -> String {
    if out.is_null() {
        return format!("  · no review of this task yet (wisp review maybe {task} starts one)");
    }
    let by = out.pointer("/by/agent").and_then(Value::as_str).map(|a| format!(" by {a}")).unwrap_or_default();
    let summary = text(out, "summary").trim();
    let head = match text(out, "status") {
        "running" => format!("  ◌ reviewing{by}…"),
        "held" => format!("  ◷ review{by} held: {summary}"),
        "failed" => format!("  ✗ review{by} failed: {summary}"),
        _ => format!("  ✓ reviewed{by}: {}", if summary.is_empty() { "no summary" } else { summary }),
    };
    let mut lines = vec![head];
    for f in out.get("findings").and_then(Value::as_array).into_iter().flatten() {
        let at = match (f.get("file").and_then(Value::as_str), f.get("line").and_then(Value::as_u64)) {
            (Some(file), Some(line)) => format!(" {file}:{line}"),
            (Some(file), None) => format!(" {file}"),
            _ => String::new(),
        };
        lines.push(format!("    - {}{at}: {}", text(f, "severity"), text(f, "text")));
    }
    if let Some(t) = out.get("tests").filter(|t| t.is_object()) {
        lines.push(format!("    tests: {} ({} passed, {} failed)", text(t, "verdict"), t["passed"], t["failed"]));
    }
    if out.get("current") == Some(&Value::Bool(false)) {
        lines.push("    (of an earlier change: the task has changed since)".to_owned());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::said;

    /// Audit H8: each of these verbs said its result as a field dump; each says a sentence, and `--json` is untouched
    /// (the caller prints the result whole there).
    #[test]
    fn verbs_say_a_sentence_instead_of_a_field_dump_audit_h8() {
        let stop = said("factory.stop", &json!({ "id": "r1" }), &json!({ "stopping": true }), None).unwrap();
        assert!(stop.starts_with("  ✓ stopping factory run r1: its stations stop") && !stop.contains("true"), "{stop}");
        let job = json!({ "session": { "id": "01a1", "title": "Zed · Check-in", "state": "running", "agent": "codex" } });
        assert_eq!(
            said("crew.checkin", &json!({ "id": "Zed" }), &job, None).unwrap(),
            "  ✓ Zed · Check-in, running\n    wisp attach 01a1 follows it"
        );
        assert_eq!(
            said("review.maybe", &json!({ "id": "t1" }), &json!({ "why": "Review by another harness is off." }), None).unwrap(),
            "  · no review: Review by another harness is off."
        );
        let m = json!({ "name": "Zed", "paused": true, "agent": "codex", "brief": "x", "checkins": 0 });
        assert_eq!(
            said("crew.update", &json!({ "id": "zed", "paused": true }), &m, None).unwrap(),
            "  ✓ Zed: paused (wisp crew resume Zed brings them back)"
        );
        assert_eq!(said("crew.update", &json!({ "id": "zed", "role": "QA" }), &m, None).unwrap(), "  ✓ Zed: role changed");
        assert!(said("crew.get", &json!({}), &m, None).is_none(), "a read prints its fields");
        // `later cancel` printed "ok  true" (audit H7)
        assert_eq!(
            said("later.cancelPlan", &json!({ "id": "p1" }), &json!({ "ok": true }), None).unwrap(),
            "  ✓ plan p1 taken back: it won't start"
        );
        assert!(said("later.cancel", &json!({ "sessionId": "t1" }), &json!({ "ok": true }), None).unwrap().starts_with("  ✓ cancelled"));
    }

    /// Audit I7: `wisp crew update bob --on task.done` said "on changed" while `on` stayed `[]`; I12: "✓ Zed: back: not
    /// paused". It says what changed, against the member as it was.
    #[test]
    fn crew_update_says_only_what_changed_audit_i7_i12() {
        let before = json!({ "name": "Bob", "role": "QA", "wakes": [] });
        let same = json!({ "name": "Bob", "role": "QA", "wakes": [] });
        assert_eq!(
            said("crew.update", &json!({ "id": "bob", "on": ["task.done"] }), &same, Some(&before)).unwrap(),
            "  ✓ Bob: nothing changed (on already as given)"
        );
        let woke = json!({ "name": "Bob", "role": "QA", "wakes": [{ "on": "task.done" }] });
        assert_eq!(said("crew.update", &json!({ "id": "bob", "on": ["task.done"] }), &woke, Some(&before)).unwrap(), "  ✓ Bob: on changed");
        assert_eq!(
            said("crew.update", &json!({ "id": "bob", "role": "QA" }), &same, Some(&before)).unwrap(),
            "  ✓ Bob: nothing changed (role already as given)"
        );
        let back = json!({ "name": "Zed", "paused": false });
        assert_eq!(
            said("crew.update", &json!({ "id": "zed", "paused": false }), &back, Some(&json!({ "name": "Zed", "paused": true }))).unwrap(),
            "  ✓ Zed: back at work: no longer paused"
        );
    }

    /// Audit I12: `goal set`, `crew create` and `review view` printed field dumps; each says a sentence.
    #[test]
    fn goal_set_crew_create_and_review_view_say_sentences_audit_i12() {
        let goal = json!({ "text": "the build is green", "judges": [{ "name": "Claude Code", "agent": "claude" }], "round": 0, "status": "working" });
        assert_eq!(
            said("goal.set", &json!({ "id": "t1" }), &goal, None).unwrap(),
            "  ✓ goal set: the build is green · judged by Claude Code\n    wisp show t1 follows it"
        );
        let member = json!({ "name": "Bob", "role": "QA", "agent": "codex", "createdAt": 1 });
        let made = said("crew.create", &json!({ "name": "Bob" }), &member, None).unwrap();
        assert!(made.starts_with("  ✓ Bob joined the crew, QA · codex") && !made.contains("in a moment"), "{made}");
        let review = json!({ "status": "done", "by": { "agent": "codex" }, "summary": "Two issues", "current": true,
            "findings": [{ "severity": "high", "file": "a.rs", "line": 3, "text": "unwrap" }], "tests": { "verdict": "pass", "passed": 4, "failed": 0 } });
        let view = said("review.view", &json!({ "id": "t1" }), &review, None).unwrap();
        assert!(
            view.starts_with("  ✓ reviewed by codex: Two issues")
                && view.contains("- high a.rs:3: unwrap")
                && view.contains("tests: pass (4 passed, 0 failed)"),
            "{view}"
        );
        assert!(
            said("review.view", &json!({ "id": "t1" }), &serde_json::Value::Null, None).unwrap().contains("no review of this task yet")
        );
    }
}
