//! A task's team (Delegate's subagents, v1 server/subagents.js normalizeTeam): the rows a launch or a follow-up gives,
//! made a clean roster the lead's `wisp_delegate` reads (`ext["wisp.runtime"].subagents`, more/delegate.rs). Names are
//! slugs, unique (`codex`, `codex-2`, …; a blank one is its harness's id); copies 1 to 5; a note on what it's for, one
//! line of at most 300 characters; a place away from the lead (`here`, `sandbox` or a machine id) kept when it reads
//! as one; a crew member keeps its id and runs as itself. Pure: the caller says which harnesses exist.

use serde_json::{Map, Value, json};

/// v1 `MAX_COPIES`: parallel copies of one row.
pub const MAX_COPIES: u64 = 5;
/// v1 `MAX_USE`: the "use it for" note.
pub const MAX_USE: usize = 300;

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(24).collect::<String>().trim_end_matches('-').to_owned()
}

/// A string field, its whitespace runs made one space, trimmed, cut at `n` characters.
fn line(m: &Value, k: &str, n: usize) -> String {
    let s = m.get(k).and_then(Value::as_str).unwrap_or_default();
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(n).collect()
}

/// Copies from a JSON number: whole, 1 to `MAX_COPIES` (v1 `Math.min(5, Math.max(1, Math.floor(n) || 1))`).
fn copies(v: Option<&Value>) -> u64 {
    let Some(n) = v.and_then(Value::as_f64).filter(|f| f.is_finite()) else { return 1 };
    // the largest whole k in 1..=MAX_COPIES not above n (no float-to-int cast)
    (1..=MAX_COPIES).rev().find(|k| n >= f64::from(u32::try_from(*k).unwrap_or(1))).unwrap_or(1)
}

fn word(s: &str, n: usize, extra: &[char]) -> bool {
    !s.is_empty() && s.chars().count() <= n && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || extra.contains(&c))
}

/// The roster from `list` (a launch's `team`, a follow-up's). `known`: whether wisp has that harness; `auto` always
/// passes (Auto picks when the member first gets work). An unknown harness refuses the whole team, as v1 did.
///
/// # Errors
/// The first row whose harness isn't known (or is `wisp`), in words.
pub fn normalize_team(list: &[Value], known: impl Fn(&str) -> bool) -> Result<Vec<Value>, String> {
    let mut taken: Vec<String> = Vec::new();
    let mut unique = |base: &str| {
        let mut name = base.to_owned();
        let mut i = 2;
        while taken.contains(&name) {
            name = format!("{base}-{i}");
            i += 1;
        }
        taken.push(name.clone());
        name
    };
    let mut out = Vec::new();
    for m in list {
        let crew = m.get("crew").and_then(Value::as_str).unwrap_or_default();
        if word(crew, 64, &[]) {
            let base = slug(m.get("name").and_then(Value::as_str).unwrap_or_default());
            let name = unique(if base.is_empty() { "crew" } else { &base });
            let agent = m.get("agent").and_then(Value::as_str).filter(|a| *a != "wisp" && known(a)).unwrap_or("auto");
            let label = line(m, "label", 24);
            out.push(json!({
                "name": name, "crew": crew, "label": if label.is_empty() { name.clone() } else { label }, "role": line(m, "role", 60),
                "agent": agent, "model": "", "effort": "", "use": line(m, "use", MAX_USE), "count": 1,
            }));
            continue;
        }
        let agent = m.get("agent").and_then(Value::as_str).unwrap_or_default();
        if agent == "wisp" || (agent != "auto" && !known(agent)) {
            return Err(format!("unknown subagent harness: {agent}"));
        }
        let base = slug(m.get("name").and_then(Value::as_str).unwrap_or_default());
        let name = unique(if base.is_empty() { agent } else { &base });
        let count = copies(m.get("count"));
        let model: String = m.get("model").and_then(Value::as_str).unwrap_or_default().chars().take(120).collect();
        let effort = m.get("effort").and_then(Value::as_str).filter(|e| word(e, 20, &[])).unwrap_or_default();
        let mut row = Map::new();
        row.insert("name".into(), name.into());
        row.insert("agent".into(), agent.into());
        row.insert("model".into(), model.into());
        row.insert("effort".into(), effort.into());
        row.insert("use".into(), line(m, "use", MAX_USE).into());
        row.insert("count".into(), count.into());
        // away from the lead: `here` (this computer, when the lead is elsewhere), `sandbox` or a machine's id
        let machine = m.get("machine").and_then(Value::as_str).unwrap_or_default();
        if word(machine, 64, &['.', ':']) {
            row.insert("machine".into(), machine.into());
            let place = line(m, "place", 40);
            if !place.is_empty() {
                row.insert("place".into(), place.into());
            }
        }
        out.push(Value::Object(row));
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const KNOWN: &[&str] = &["claude", "codex", "opencode"];
    fn known(a: &str) -> bool {
        KNOWN.contains(&a)
    }
    fn names(v: &[Value]) -> Vec<&str> {
        v.iter().map(|m| m["name"].as_str().unwrap()).collect()
    }

    #[test]
    fn names_are_unique_slugs_and_a_blank_one_is_its_harness_v1_normalize_team() {
        let t = normalize_team(
            &[
                json!({"agent": "codex"}),
                json!({"agent": "codex", "name": ""}),
                json!({"agent": "claude", "name": "My Tester!"}),
                json!({"agent": "opencode", "name": "my tester"}),
            ],
            known,
        )
        .unwrap();
        assert_eq!(names(&t), ["codex", "codex-2", "my-tester", "my-tester-2"]);
    }

    #[test]
    fn copies_are_one_to_five_and_the_note_one_trimmed_line() {
        let t = normalize_team(
            &[
                json!({"agent": "codex", "count": 99, "use": "  run\n the   tests "}),
                json!({"agent": "codex", "count": 0}),
                json!({"agent": "codex", "count": 2.7}),
            ],
            known,
        )
        .unwrap();
        assert_eq!(t[0]["count"], 5);
        assert_eq!(t[0]["use"], "run the tests");
        assert_eq!(t[1]["count"], 1);
        assert_eq!(t[2]["count"], 2);
        let long = "x".repeat(400);
        assert_eq!(normalize_team(&[json!({"agent": "codex", "use": long})], known).unwrap()[0]["use"].as_str().unwrap().len(), MAX_USE);
    }

    #[test]
    fn a_place_away_from_the_lead_is_kept_only_when_it_reads_as_one() {
        let t = normalize_team(
            &[
                json!({"agent": "opencode", "machine": "peer-1"}),
                json!({"agent": "codex", "machine": "sandbox"}),
                json!({"agent": "codex", "machine": "../etc"}),
                json!({"agent": "codex", "machine": ""}),
            ],
            known,
        )
        .unwrap();
        assert_eq!(t[0]["machine"], "peer-1");
        assert_eq!(t[1]["machine"], "sandbox");
        assert!(t[2].get("machine").is_none());
        assert!(t[3].get("machine").is_none());
    }

    #[test]
    fn an_unknown_harness_or_wisp_refuses_the_team_and_auto_passes() {
        assert_eq!(normalize_team(&[json!({"agent": "nope"})], known).unwrap_err(), "unknown subagent harness: nope");
        assert!(normalize_team(&[json!({"agent": "wisp"})], |_| true).is_err());
        assert_eq!(normalize_team(&[json!({"agent": "auto"})], known).unwrap()[0]["name"], "auto");
        assert_eq!(normalize_team(&[], known).unwrap(), Vec::<Value>::new());
    }

    #[test]
    fn a_crew_member_keeps_its_id_and_runs_as_itself() {
        let t = normalize_team(&[json!({"crew": "ada01abc", "name": "Ada", "role": "Reviews PRs", "agent": "nope"})], known).unwrap();
        assert_eq!(t[0]["crew"], "ada01abc");
        assert_eq!(t[0]["name"], "ada");
        assert_eq!(t[0]["agent"], "auto");
        assert_eq!(t[0]["count"], 1);
    }

    #[test]
    fn a_bad_effort_is_dropped_and_a_long_model_cut() {
        let t = normalize_team(&[json!({"agent": "codex", "effort": "high; rm -rf", "model": "m".repeat(200)})], known).unwrap();
        assert_eq!(t[0]["effort"], "");
        assert_eq!(t[0]["model"].as_str().unwrap().len(), 120);
    }
}
