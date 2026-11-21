//! The compaction audit (v1 server/services/threads.js `audit`): after a compaction, a copy of the compacted
//! conversation is asked what it remembers of the goal, constraints, decisions and what failed ([`RECALL`]); each item
//! of the Notes is then kept or lost by a word match against that answer ([`score`]: no model call, unlike v1's
//! second run). What was lost goes in front of the next follow-up ([`lost_note`]).
//!
//! A match is on content words: an item is kept when at least [`KEEP_SHARE`] of its words (three letters or more,
//! common words left out, a plural's `s` too) are in the answer, in any order and case. It can't tell a fact the
//! answer contradicts from one it repeats; the audit says what may be lost, and the note asks the agent to check.

use crate::notes::{META, Shown};

/// The sections asked about.
pub const AUDITED: [&str; 4] = ["goal", "constraints", "decisions", "failed"];
/// Items asked about, at most (v1 `AUDIT_ITEMS`).
pub const ITEMS_MAX: usize = 24;
/// The share of an item's words the answer must have.
pub const KEEP_SHARE: f64 = 0.6;
/// The answer read, at most.
pub const RECALL_MAX: usize = 6_000;

/// What the copy is asked (v1 `RECALL`).
pub const RECALL: &str = "Quick check by wisp, not a task. Do not use tools, read files or continue the work. From what you remember of this conversation only, write down:
- the goal
- the constraints (must / must not)
- the decisions made, and why
- what was tried and failed, and why
Short bullets, under 300 words. Say \"don't know\" for a part you don't remember.";

const COMMON: [&str; 40] = [
    "the", "and", "for", "but", "not", "with", "that", "this", "from", "into", "onto", "are", "was", "were", "been", "has", "have", "had",
    "its", "it's", "our", "you", "your", "use", "used", "using", "all", "any", "can", "will", "should", "must", "when", "then", "than",
    "only", "also", "just", "out", "per",
];

/// The items asked about, as `Label: item`, in the Notes' order, at most [`ITEMS_MAX`].
pub fn items(sections: &[Shown<'_>]) -> Vec<String> {
    let mut out = Vec::new();
    for m in META.iter().filter(|m| AUDITED.contains(&m.id)) {
        let Some(s) = sections.iter().find(|s| s.id == m.id) else { continue };
        let lines: Vec<&str> = if m.one { vec![s.text.trim()] } else { s.text.lines().map(str::trim).collect() };
        out.extend(lines.into_iter().filter(|l| !l.is_empty()).map(|l| format!("{}: {l}", m.label)));
    }
    out.truncate(ITEMS_MAX);
    out
}

/// An item's or an answer's content words: lower case, letters and digits, three or more, common words out, a
/// plural's `s` off.
pub fn words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in text.to_lowercase().split(|c: char| !(c.is_alphanumeric() || c == '\'')) {
        // a possessive is the word (`API's` is `api`)
        let w = w.trim_matches('\'');
        let w = w.strip_suffix("'s").unwrap_or(w);
        if w.chars().count() < 3 || COMMON.contains(&w) {
            continue;
        }
        let w = if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") { &w[..w.len() - 1] } else { w };
        if !out.iter().any(|x| x == w) {
            out.push(w.to_owned());
        }
    }
    out
}

/// Whether `recall` still has `item` (its label aside).
pub fn kept(item: &str, recall: &str) -> bool {
    let body = item.split_once(": ").map_or(item, |(_, b)| b);
    let want = words(body);
    if want.is_empty() {
        return true;
    }
    let have = words(&recall.chars().take(RECALL_MAX).collect::<String>());
    let found = want.iter().filter(|w| have.contains(w)).count();
    #[allow(clippy::cast_precision_loss)] // a few dozen words
    let share = found as f64 / want.len() as f64;
    share >= KEEP_SHARE
}

/// How many of `items` the answer kept, and the ones it lost, in order.
pub fn score(items: &[String], recall: &str) -> (u32, Vec<String>) {
    let lost: Vec<String> = items.iter().filter(|i| !kept(i, recall)).cloned().collect();
    (u32::try_from(items.len() - lost.len()).unwrap_or(u32::MAX), lost)
}

/// What goes in front of the next follow-up when the compaction lost something.
pub fn lost_note(lost: &[String], tool: &str) -> Option<String> {
    (!lost.is_empty()).then(|| {
        format!(
            "<compaction-lost>\nThe last compaction may have dropped these from your memory of this task. They still hold (they are in the task's Notes, `{tool}`):\n{}\n</compaction-lost>",
            lost.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
        )
    })
}
