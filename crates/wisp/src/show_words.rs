//! What `wisp show` says besides the answer: a task's goal and where its judging stands (audit D10), and a turn held
//! before it started with how to go on from here (audit D4). Pure: the task and its turn as the server answered them.
use std::fmt::Write as _;

use serde_json::Value;

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// A factory run's stations, its round of the limit and why it ended, on one line (ADR-0114; the TUI's header line).
/// '' for any other task.
pub fn factory_line(task: &Value) -> String {
    let Some(run) = task.pointer("/ext/wisp.factory").filter(|r| r.get("stations").is_some()) else { return String::new() };
    let Some(line) = wisp_factory_tui::line(run) else { return String::new() };
    let why = run.get("why").and_then(Value::as_str).filter(|w| !w.is_empty()).map_or_else(String::new, |w| format!("  · {w}"));
    // its PR by number, never the raw link at the end of the line (audit G15)
    let pr = run.get("pr").and_then(Value::as_str).filter(|p| !p.is_empty()).map_or_else(String::new, |p| format!("  · {}", pr_words(p)));
    format!("\n▦ {line}{pr}{why}")
}

/// A PR link as words: `https://github.com/acme/repo/pull/1` is "PR #1"; anything else stays as it is.
fn pr_words(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit_once("/pull/")
        .filter(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        .map_or_else(|| url.to_owned(), |(_, n)| format!("PR #{n}"))
}

/// The task's goal and where its judging stands, on one line (audit D10: only `runtime.get`'s `ext.wisp.goal` had it):
/// `wisp.goal`'s state over the goal its spec set. '' with no goal.
pub fn goal_line(task: &Value) -> String {
    let st = task.pointer("/ext/wisp.goal").cloned().unwrap_or(Value::Null);
    let state = st.get("state").and_then(Value::as_str).unwrap_or_default();
    if state == "cleared" {
        return String::new();
    }
    let spec = task.pointer("/ext/wisp.runtime/spec/goal").cloned().unwrap_or(Value::Null);
    let text = st
        .get("text")
        .and_then(Value::as_str)
        .filter(|t| !t.trim().is_empty())
        .or_else(|| spec.as_str().or_else(|| spec.get("text").and_then(Value::as_str)))
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let Some(text) = text else { return String::new() };
    let n = st.get("rounds").and_then(Value::as_u64).unwrap_or(0);
    let rounds = if n == 1 { "1 round".to_owned() } else { format!("{n} rounds") };
    let said = st.get("why").and_then(Value::as_str).unwrap_or_default();
    let why = judges_said(said).map_or_else(String::new, |w| format!(": {w}"));
    let at = match state {
        "met" => format!("met after {rounds}"),
        // a judge that couldn't judge stopped it: that, not "not met" (audit G2)
        "stalled" if said.starts_with("The goal can't be judged") => said.lines().next().unwrap_or(said).trim().to_owned(),
        "stalled" => format!("not met after {rounds}{why}"),
        "paused" => "paused".to_owned(),
        "judging" => "being checked".to_owned(),
        _ if n == 0 => "checked when a turn ends".to_owned(),
        _ => format!("not met yet after {rounds}"),
    };
    format!("\n◎ Goal: {text}  · {at}")
}

/// What the judges said, from a stalled goal's question ("The goal isn't met after 1 round.\n<judge>: <why>\nKeep going?"):
/// their lines, without the question's own head and tail, which the line already says (audit G16: "not met after 1
/// round: The goal isn't met after 1 rounds.").
fn judges_said(why: &str) -> Option<String> {
    let lines: Vec<&str> =
        why.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("The goal isn't met after") && *l != "Keep going?").collect();
    (!lines.is_empty()).then(|| lines.join("; "))
}

/// A turn held before it started (project trust, a changed harness, a failed setup, the plan's line): what holds it,
/// what it would run, and how to go on from here, as the page's card says (audit D4: `wisp show` said nothing). ''
/// when the turn isn't held.
pub fn holding(id: &str, turn: &Value) -> String {
    if turn.get("status").and_then(Value::as_str) != Some("held") {
        return String::new();
    }
    let h = turn.get("held").cloned().unwrap_or(Value::Null);
    let d = h.get("data").cloned().unwrap_or(Value::Null);
    let text = |k: &str| d.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty());
    let kind = h.get("kind").and_then(Value::as_str).unwrap_or_default();
    let trust = kind == "trust";
    let why = if trust {
        "This repo asks to run its own programs: these would run on this computer, as you, whatever the mode."
    } else {
        text("why").or_else(|| text("what")).or_else(|| text("line")).unwrap_or_default()
    };
    let mut out = format!("\n\n◷ Held before it started{}", if why.is_empty() { String::new() } else { format!(": {why}") });
    for i in d.get("items").and_then(Value::as_array).into_iter().flatten() {
        let k = match i.get("kind").and_then(Value::as_str).unwrap_or_default() {
            "hook" => "Hook",
            "mcp" => "MCP server",
            "setup" => "Setup",
            "plugin" => "Plugin",
            _ => "Command",
        };
        let at = i.get("file").and_then(Value::as_str).map_or_else(String::new, |f| format!("  ({f})"));
        let _ = write!(out, "\n  {k} {}: {}{at}", s(i, "name"), s(i, "command"));
    }
    let start = if trust || kind == "harness-changed" { "Trust and start" } else { "Start now" };
    let _ = write!(out, "\n  {start}: wisp held {id} start");
    if trust {
        let _ = write!(out, "\n  Run without them: wisp held {id} without");
    }
    let _ = write!(out, "\n  Skip this run: wisp held {id} skip");
    out
}

#[cfg(test)]
mod tests {
    /// Audit D10: `wisp show` said nothing of the goal; it says the goal and where its judging stands.
    #[test]
    fn show_says_the_goal_and_where_it_stands_audit_d10() {
        let t = |goal: serde_json::Value, spec: serde_json::Value| serde_json::json!({ "ext": { "wisp.goal": goal, "wisp.runtime": { "spec": spec } } });
        let met = t(serde_json::json!({ "state": "met", "rounds": 2 }), serde_json::json!({ "goal": "tests pass" }));
        assert_eq!(super::goal_line(&met), "\n◎ Goal: tests pass  · met after 2 rounds");
        let set = t(serde_json::json!({ "state": "", "rounds": 0, "text": "ship it" }), serde_json::Value::Null);
        assert_eq!(super::goal_line(&set), "\n◎ Goal: ship it  · checked when a turn ends");
        let out = t(
            serde_json::json!({ "state": "stalled", "rounds": 3, "why": "still failing" }),
            serde_json::json!({ "goal": { "text": "green" } }),
        );
        assert_eq!(super::goal_line(&out), "\n◎ Goal: green  · not met after 3 rounds: still failing");
        // audit G16: the stall's question isn't said again inside the line
        let asked = t(
            serde_json::json!({ "state": "stalled", "rounds": 1, "why": "The goal isn't met after 1 round.\ncritic: tests fail\nKeep going?" }),
            serde_json::json!({ "goal": "green" }),
        );
        assert_eq!(super::goal_line(&asked), "\n◎ Goal: green  · not met after 1 round: critic: tests fail");
        let broke = t(
            serde_json::json!({ "state": "stalled", "rounds": 1, "why": "The goal can't be judged: critic couldn't judge: no harness critic\nThe work isn't sent back for that." }),
            serde_json::json!({ "goal": "green" }),
        );
        assert_eq!(super::goal_line(&broke), "\n◎ Goal: green  · The goal can't be judged: critic couldn't judge: no harness critic");
        assert_eq!(super::goal_line(&t(serde_json::json!({ "state": "cleared" }), serde_json::json!({ "goal": "x" }))), "");
        assert_eq!(super::goal_line(&serde_json::json!({ "ext": {} })), "", "no goal: nothing");
        let run = serde_json::json!({ "ext": { "wisp.factory": { "factory": "default", "state": "failed", "station": "", "round": 3, "max": 3,
            "why": "Checks still fail after 3 round(s)", "stations": [{ "id": "build", "state": "done" }, { "id": "checks", "state": "failed" }] } } });
        assert_eq!(super::factory_line(&run), "\n▦ default: ✓ build · ✗ checks  · Checks still fail after 3 round(s)");
        assert_eq!(super::factory_line(&serde_json::json!({ "ext": {} })), "", "no factory: nothing");
        // audit G15: the band line ended with the raw link
        let shipped = serde_json::json!({ "ext": { "wisp.factory": { "factory": "shipit", "state": "done", "station": "", "round": 1, "max": 3,
            "pr": "https://github.com/acme/repo/pull/1", "stations": [{ "id": "build", "state": "done" }] } } });
        let line = super::factory_line(&shipped);
        assert!(line.contains("· PR #1") && !line.contains("https://"), "{line}");
    }

    /// Audit D4: a held turn says why, what it would run and the three ways on; a running one says nothing.
    #[test]
    fn a_held_turn_says_why_what_it_would_run_and_how_to_go_on_audit_d4() {
        let t = serde_json::json!({ "status": "held", "held": { "by": "wisp.worktrees", "kind": "trust", "data": { "key": "/r",
            "items": [{ "kind": "mcp", "file": ".mcp.json", "name": "local", "command": "node server.js", "id": "x" }] } } });
        let said = super::holding("t1", &t);
        assert!(said.contains("Held before it started: This repo asks to run its own programs"), "{said}");
        assert!(said.contains("MCP server local: node server.js  (.mcp.json)"), "{said}");
        assert!(
            said.contains("Trust and start: wisp held t1 start")
                && said.contains("wisp held t1 without")
                && said.contains("wisp held t1 skip")
        );
        let limit = serde_json::json!({ "status": "held", "held": { "kind": "plan", "data": { "why": "past 75%" } } });
        let said = super::holding("t2", &limit);
        assert!(said.contains(": past 75%") && said.contains("Start now: wisp held t2 start") && !said.contains("without"), "{said}");
        assert_eq!(super::holding("t3", &serde_json::json!({ "status": "running" })), "");
    }
}
