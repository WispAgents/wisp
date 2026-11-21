//! What goes in front of a follow-up (v1 runner.js turn note, web/js/threads.js `ledgerNote`): the Notes as they
//! are now (at the end of the conversation, so the prompt cache before them holds), what you changed in them since
//! the agent last read them, and a nudge when your message takes back or changes something said earlier
//! (supersession: words, not a model call; the agent decides).

/// Characters of Notes in front of a follow-up, at most.
pub const NOTE_MAX: usize = 6000;
/// Characters of a lead's Notes a subagent hears, at most.
pub const LEAD_MAX: usize = 4000;

/// Phrases that take back or change something said earlier, each matched on word boundaries, any case.
const SUPERSEDE: [&str; 24] = [
    "actually",
    "instead",
    "scratch that",
    "scrap that",
    "forget that",
    "forget it",
    "forget about",
    "forget what i",
    "change of plan",
    "change of plans",
    "never mind",
    "nevermind",
    "on second thought",
    "no longer",
    "anymore",
    "rather than",
    "switch to",
    "switch back",
    "let's not",
    "lets not",
    "after all",
    "revert that",
    "undo that",
    "don't do that",
];
/// A message that starts by saying no: "No, keep the old API", "Wait: …", "Stop."
const OPENERS: [&str; 3] = ["no", "wait", "stop"];

fn boundary(c: Option<char>) -> bool {
    c.is_none_or(|c| !c.is_alphanumeric())
}

/// `needle` in `hay` (both lower case) with a non-word character (or nothing) on each side.
fn has_word(hay: &str, needle: &str) -> bool {
    hay.match_indices(needle).any(|(i, _)| boundary(hay[..i].chars().next_back()) && boundary(hay[i + needle.len()..].chars().next()))
}

/// Whether a message may change something said earlier. Only its first 2000 characters are read.
pub fn supersedes(text: &str) -> bool {
    let t: String = text.chars().take(2000).collect::<String>().to_lowercase().replace(['’', '‘'], "'");
    if SUPERSEDE.iter().any(|p| has_word(&t, p)) {
        return true;
    }
    let head = t.trim_start();
    OPENERS.iter().any(|o| head.strip_prefix(o).is_some_and(|r| r.starts_with([',', '.', '!', ':', ';'])))
}

/// What the note is made from.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoteIn<'a> {
    /// the Notes as markdown ('' when there are none)
    pub notes: &'a str,
    pub rev: u32,
    /// you changed a section since the agent last read them
    pub yours_unseen: bool,
    /// the new turn's place (0: the task's first)
    pub turn: u32,
    pub prompt: &'a str,
    /// how the agent names `wisp_notes` (`mcp__wisp__wisp_notes`)
    pub tool: &'a str,
}

/// The note in front of a follow-up, or None: nothing to say.
pub fn turn_note(n: &NoteIn<'_>) -> Option<String> {
    let tool = n.tool;
    let mut parts = Vec::new();
    if !n.notes.trim().is_empty() {
        let yours = if n.yours_unseen {
            "The user edited them since your last turn: where they differ from earlier messages, the Notes win. "
        } else {
            ""
        };
        parts.push(format!(
            "<task-notes rev=\"{}\">\nwisp keeps this task's Notes: its state as it is now, newer than anything you may remember from earlier in the conversation (which may have been compacted). {yours}Keep them current with `{tool}` as you work.\n\n{}\n</task-notes>",
            n.rev, n.notes
        ));
    } else if n.turn >= 2 && n.turn % 4 == 2 {
        parts.push(format!("This task has no Notes yet. If it is more than a quick answer, write them with `{tool}` (goal, constraints, decisions and why, what failed and why, where it stands), so they survive compaction and fresh threads."));
    }
    if n.turn > 0 && supersedes(n.prompt) {
        parts.push(format!("This message may change something said earlier. If it does, rewrite that section of the Notes with `{tool}` (the whole section, as it should be from now on) so the old version is gone, and follow the new one from here on."));
    }
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// What a subagent hears of its lead's Notes, ahead of its task (v1 subagents.js `leadLedgerNote`).
pub fn lead_note(notes: &str) -> Option<String> {
    (!notes.trim().is_empty()).then(|| {
        format!("<lead-notes>\nThe task your lead is working on, as its Notes have it. Stay within these constraints and decisions; if your work needs to break one, don't: say so in your report.\n\n{notes}\n</lead-notes>")
    })
}
