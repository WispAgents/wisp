//! Find in a task (R4-3, ADR-0133): the steps of the turns the window sends as stubs (no events), searched where they
//! are kept. The steps are masked first, as they would leave the server ([`env_mask::mask_steps_known`]), so a .env
//! value Find was never shown is never found either: what matches is what the transcript would read.
//! Pure: steps in, hits out.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::env_mask::{self, Known};

/// What a step says that Find reads: its label, whole title, command, output, and a message's or thought's text.
const FIELDS: &[&str] = &["label", "title", "detail", "output", "text", "name"];

/// Characters of context each side of a hit in its snippet.
const AROUND: usize = 40;

/// One step that holds the words: its event and a line around them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub seq: u64,
    pub snippet: String,
}

/// The steps of one turn (`Step` JSON, in order) that hold `query` (case-insensitive), at most `cap`, one per step.
/// `mask`: as the wire does (Hide .env values on); `known` what the task read, for prose quoting a value.
pub fn find_steps(mut steps: Vec<Value>, query: &str, mask: bool, known: Known<'_>, cap: usize) -> Vec<Found> {
    let q = query.trim().to_lowercase();
    if q.is_empty() || cap == 0 {
        return Vec::new();
    }
    if mask {
        env_mask::mask_steps_known(steps.iter_mut(), known);
    }
    // A tool and its separate output are one step in the transcript, which keys it by the output's event (chat-steps.ts):
    // searched together, one hit, with the output's seq. A tool whose error names it (audit F3) is that error's row
    // instead: the error is what it shows, keyed by the error's event.
    let call = |st: &Value| st.get("callId").and_then(Value::as_str).map(str::to_owned);
    let is = |st: &Value, k: &str| st.get("kind").and_then(Value::as_str) == Some(k);
    let tools: HashSet<String> = steps.iter().filter(|st| is(st, "tool")).filter_map(call).collect();
    let of_kind =
        |k: &str| -> HashMap<String, &Value> { steps.iter().filter(|st| is(st, k)).filter_map(|st| call(st).map(|c| (c, st))).collect() };
    let outputs = of_kind("tool_output");
    let errors = of_kind("tool_error");
    let mut out = Vec::new();
    for st in &steps {
        let id = call(st);
        if (is(st, "tool_output") || is(st, "tool_error")) && id.as_ref().is_some_and(|c| tools.contains(c)) {
            continue; // read with its tool
        }
        let joined = if is(st, "tool") { id.as_ref().and_then(|c| errors.get(c).or_else(|| outputs.get(c))).copied() } else { None };
        let Some(seq) = joined.unwrap_or(st).get("seq").and_then(Value::as_u64) else { continue };
        let hit = std::iter::once(st)
            .chain(joined)
            .flat_map(|s| FIELDS.iter().filter_map(move |k| s.get(*k).and_then(Value::as_str)))
            .find_map(|text| snippet(text, &q));
        if let Some(snippet) = hit {
            out.push(Found { seq, snippet });
            if out.len() >= cap {
                break;
            }
        }
    }
    out
}

/// A line of `text` around the first place it holds `q` (lowercase), or `None`.
fn snippet(text: &str, q: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let at = lower.find(q)?;
    // lowercasing may change a char's length: count chars on the lowered text, then take as many from the text
    let start = lower[..at].chars().count();
    let len = q.chars().count();
    let chars: Vec<char> = text.chars().collect();
    let from = start.saturating_sub(AROUND);
    let to = (start + len + AROUND).min(chars.len());
    let body: String = chars[from.min(chars.len())..to].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    Some(format!("{}{body}{}", if from > 0 { "…" } else { "" }, if to < chars.len() { "…" } else { "" }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_label_or_an_output_finds_its_step_case_insensitive_with_a_line_around_it() {
        let steps = vec![
            json!({ "seq": 1, "t": 0, "kind": "tool", "tool": "run", "label": "Running cargo test", "callId": "c1" }),
            json!({ "seq": 2, "t": 0, "kind": "tool_output", "callId": "c1", "text": "test result: FAILED. 1 passed; 2 failed", "env": false }),
            json!({ "seq": 3, "t": 0, "kind": "text", "text": "Nothing here", "hide": [] }),
        ];
        let hits = find_steps(steps, "failed", true, Some(&[]), 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].seq, 2);
        assert!(hits[0].snippet.contains("FAILED"), "{:?}", hits[0]);
        let steps = vec![json!({ "seq": 1, "t": 0, "kind": "tool", "tool": "run", "label": "Running CARGO test" })];
        assert_eq!(find_steps(steps, "cargo", true, Some(&[]), 10)[0].seq, 1);
    }

    #[test]
    fn a_tool_and_its_separate_output_are_one_step_keyed_by_the_outputs_event() {
        // the transcript draws one row for the call, keyed by the output's seq: a hit on the label is that row
        let steps = vec![
            json!({ "seq": 4, "t": 0, "kind": "tool", "tool": "run", "label": "Running cargo test", "callId": "c1" }),
            json!({ "seq": 5, "t": 0, "kind": "tool_output", "callId": "c1", "text": "cargo says ok", "env": false }),
        ];
        let hits = find_steps(steps, "cargo", true, Some(&[]), 10);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].seq, 5);
        assert!(hits[0].snippet.contains("Running"), "{:?}", hits[0]);
    }

    #[test]
    fn a_failed_call_and_the_error_that_names_it_are_one_step_keyed_by_the_errors_event() {
        // chat-steps.ts draws a call its error names as one bad row carrying the error, keyed by the error's seq
        let steps = vec![
            json!({ "seq": 6, "t": 0, "kind": "tool", "tool": "run", "label": "Running make", "callId": "c1" }),
            json!({ "seq": 7, "t": 0, "kind": "tool_output", "callId": "c1", "text": "partial", "env": false }),
            json!({ "seq": 8, "t": 0, "kind": "tool_error", "callId": "c1", "text": "make: no rule" }),
        ];
        let hits = find_steps(steps.clone(), "make", true, Some(&[]), 10);
        assert_eq!(hits.iter().map(|h| h.seq).collect::<Vec<_>>(), vec![8], "{hits:?}");
        assert_eq!(find_steps(steps, "no rule", true, Some(&[]), 10)[0].seq, 8);
        // one that names no call of the turn stays a step of its own
        let steps = vec![json!({ "seq": 9, "t": 0, "kind": "tool_error", "callId": "gone", "text": "boom" })];
        assert_eq!(find_steps(steps, "boom", true, Some(&[]), 10)[0].seq, 9);
    }

    #[test]
    fn a_env_files_value_is_masked_before_it_is_matched() {
        let steps = vec![
            json!({ "seq": 1, "t": 0, "kind": "tool", "tool": "read", "label": "Reading .env", "file": ".env", "callId": "c1" }),
            json!({ "seq": 2, "t": 0, "kind": "tool_output", "callId": "c1", "text": "API_KEY=sk-live-secret", "env": true }),
        ];
        assert_eq!(find_steps(steps.clone(), "sk-live", true, Some(&[]), 10), Vec::new());
        // its key's name is what the masked text shows, so it is found
        assert_eq!(find_steps(steps.clone(), "api_key", true, Some(&[]), 10)[0].seq, 2);
        // Hide .env values off: the value is shown, so it is found
        assert_eq!(find_steps(steps, "sk-live", false, None, 10)[0].seq, 2);
    }

    #[test]
    fn at_most_cap_hits_and_none_for_blank_words() {
        let steps: Vec<Value> = (1..=5).map(|i| json!({ "seq": i, "t": 0, "kind": "tool", "tool": "run", "label": "go" })).collect();
        assert_eq!(find_steps(steps.clone(), "go", true, Some(&[]), 3).len(), 3);
        assert_eq!(find_steps(steps, "  ", true, Some(&[]), 3), Vec::new());
    }
}
