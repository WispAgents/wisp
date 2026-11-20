//! What a generated verb's result says, in a sentence, where its fields alone read as a dump (audit H8: `factory stop`
//! printed `stopping  true`, `crew checkin` `session  01a1f9… · Zed · Check-in · running`, `review maybe` `why  …`,
//! `crew update` every field of the member). Pure: the command, the args sent and the result in, the lines out; None:
//! the result's own fields say it (`--json` always prints the result).
use serde_json::Value;

fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// The sentence for `id`'s result, if it has one.
pub fn said(id: &str, args: &Value, out: &Value) -> Option<String> {
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
            let changed: Vec<&str> = args.as_object()?.keys().map(String::as_str).filter(|k| *k != "id").collect();
            let what = match (changed.as_slice(), out.get("paused").and_then(Value::as_bool)) {
                (["paused"], Some(true)) => format!("paused (wisp crew resume {name} brings them back)"),
                (["paused"], Some(false)) => "back: not paused".to_owned(),
                ([], _) => "nothing changed".to_owned(),
                (keys, _) => keys.iter().map(|k| wisp_cli_core::human::words(k)).collect::<Vec<_>>().join(", ") + " changed",
            };
            Some(format!("  ✓ {name}: {what}"))
        }
        "later.cancel" => Some(match out.pointer("/entry/text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
            Some(t) => format!("  ✓ cancelled: {t}"),
            None => format!("  ✓ cancelled what was scheduled on {}", text(args, "sessionId")),
        }),
        "later.cancelPlan" => Some(format!("  ✓ plan {} taken back: it won't start", text(args, "id"))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::said;

    /// Audit H8: each of these verbs said its result as a field dump; each says a sentence, and `--json` is untouched
    /// (the caller prints the result whole there).
    #[test]
    fn verbs_say_a_sentence_instead_of_a_field_dump_audit_h8() {
        let stop = said("factory.stop", &json!({ "id": "r1" }), &json!({ "stopping": true })).unwrap();
        assert!(stop.starts_with("  ✓ stopping factory run r1: its stations stop") && !stop.contains("true"), "{stop}");
        let job = json!({ "session": { "id": "01a1", "title": "Zed · Check-in", "state": "running", "agent": "codex" } });
        assert_eq!(
            said("crew.checkin", &json!({ "id": "Zed" }), &job).unwrap(),
            "  ✓ Zed · Check-in, running\n    wisp attach 01a1 follows it"
        );
        assert_eq!(
            said("review.maybe", &json!({ "id": "t1" }), &json!({ "why": "Review by another harness is off." })).unwrap(),
            "  · no review: Review by another harness is off."
        );
        let m = json!({ "name": "Zed", "paused": true, "agent": "codex", "brief": "x", "checkins": 0 });
        assert_eq!(
            said("crew.update", &json!({ "id": "zed", "paused": true }), &m).unwrap(),
            "  ✓ Zed: paused (wisp crew resume Zed brings them back)"
        );
        assert_eq!(said("crew.update", &json!({ "id": "zed", "role": "QA" }), &m).unwrap(), "  ✓ Zed: role changed");
        assert!(said("crew.get", &json!({}), &m).is_none(), "a read prints its fields");
        // `later cancel` printed "ok  true" (audit H7)
        assert_eq!(
            said("later.cancelPlan", &json!({ "id": "p1" }), &json!({ "ok": true })).unwrap(),
            "  ✓ plan p1 taken back: it won't start"
        );
        assert!(said("later.cancel", &json!({ "sessionId": "t1" }), &json!({ "ok": true })).unwrap().starts_with("  ✓ cancelled"));
    }
}
