//! The open task's thread in the TUI (ADR-0079, ADR-0097), as the GUI's Thread tab has it: the health line (the
//! level's word and what made it, none when Fine) on the task's header and atop the Thread pane (`t`), and the Notes
//! below it. Read from the task's own topic (`threads_notes`, `threads_health` rows); nothing is asked for.

use serde_json::Value;
use wisp_threads_core::notes::{Shown, markdown};

/// What the GUI calls a level.
pub fn level_word(level: &str) -> &'static str {
    match level {
        "worn" => "Worn",
        "heavy" => "Getting long",
        _ => "Fine",
    }
}

fn n(h: &Value, k: &str) -> u64 {
    h.get(k).and_then(Value::as_u64).unwrap_or(0)
}

fn plural(count: u64, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The facts after the level's word, worst first, with the GUI's thresholds and words (`healthFacts`).
pub fn facts(h: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(share) = h.get("share").and_then(Value::as_f64).filter(|s| *s >= 0.6) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a share in 0..=1
        let pct = (share * 100.0).round().clamp(0.0, 100.0) as u64;
        out.push(format!("context {pct}% full"));
    }
    if n(h, "compactions") >= 2 {
        out.push(plural(n(h, "compactions"), "compaction", "compactions"));
    }
    if let Some(r) = h.get("repeat").map(|r| n(r, "n")).filter(|x| *x >= 3) {
        out.push(format!("same error {r}×"));
    }
    if n(h, "turns") >= 30 {
        out.push(format!("{} turns", n(h, "turns")));
    }
    if n(h, "reverted") >= 2 {
        out.push(plural(n(h, "reverted"), "edit undone", "edits undone"));
    }
    if n(h, "notesAge") >= 8 && n(h, "turns") >= 8 {
        out.push(format!("Notes {} turns old", n(h, "notesAge")));
    }
    out
}

/// `Worn · context 85% full · 3 compactions`, or None when the thread is Fine (or never worked out).
pub fn health_line(h: &Value) -> Option<(String, &'static str)> {
    let level = h.get("level").and_then(Value::as_str).filter(|l| *l != "ok")?;
    let mut parts = vec![level_word(level).to_owned()];
    parts.extend(facts(h));
    Some((parts.join(" · "), if level == "worn" { "worn" } else { "heavy" }))
}

/// The Notes as the pane shows them: each section with text, in order, as markdown lines.
pub fn notes_lines(sections: &[(String, String)]) -> Vec<String> {
    let shown: Vec<Shown<'_>> = sections.iter().map(|(id, text)| Shown { id, text }).collect();
    let md = markdown(&shown, &[], usize::MAX, "wisp notes");
    md.lines().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_health_line_says_what_the_gui_says_and_nothing_when_fine() {
        let worn =
            json!({ "level": "worn", "share": 0.85, "compactions": 3, "turns": 12, "reverted": 0, "repeat": { "sig": "x", "n": 4 } });
        assert_eq!(health_line(&worn).unwrap(), ("Worn · context 85% full · 3 compactions · same error 4×".to_owned(), "worn"));
        let heavy = json!({ "level": "heavy", "turns": 31, "compactions": 0, "reverted": 2 });
        assert_eq!(health_line(&heavy).unwrap().0, "Getting long · 31 turns · 2 edits undone");
        assert_eq!(health_line(&json!({ "level": "ok", "turns": 3 })), None);
        assert_eq!(health_line(&Value::Null), None);
        let lines = notes_lines(&[("constraints".into(), "No Redis\nKeep the API".into()), ("goal".into(), "Ship it".into())]);
        assert_eq!(lines, ["## Goal", "Ship it", "", "## Constraints", "- No Redis", "- Keep the API"]);
    }
}
