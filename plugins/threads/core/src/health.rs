//! How worn a thread is (v1 web/js/threads.js `health`), from what wisp already has: the context's size, the
//! compactions, the turns since the thread was fresh, the same error again and again, edits the agent undid, how old
//! the Notes are. No model call. `ok`, then `heavy`, then `worn`; the Thread tab offers a fresh thread when worn.

use wisp_threads_api::{Health, Level, Repeat};

/// The thresholds (v1 HEALTH), plus `reverted`: edits undone before it counts.
pub const HEAVY: f64 = 0.6;
pub const WORN: f64 = 0.8;
pub const TURNS: u32 = 30;
pub const COMPACTIONS: u32 = 2;
pub const REPEATS: u32 = 3;
pub const STALE: u32 = 8;
pub const REVERTED: u32 = 2;

/// One edit of a file: what was there and what replaced it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditIn {
    pub file: String,
    pub before: String,
    pub after: String,
}

/// One turn, as health reads it.
#[derive(Debug, Clone, Default, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // four independent facts about a turn, not a state
pub struct TurnIn {
    /// a compaction (`/compact`), not a turn of the work
    pub compact: bool,
    /// held, never ran
    pub held: bool,
    /// it ended in an error, or its answer says it didn't get there (blocked, partial)
    pub failed: bool,
    /// a fresh thread starts here (the count starts again)
    pub fresh: bool,
    /// (context tokens, its window), when the harness said
    pub context: Option<(u64, u64)>,
    /// its failed steps' words
    pub errors: Vec<String>,
    pub edits: Vec<EditIn>,
}

/// A failed step's words without what changes between tries (paths, hashes, numbers): the same error twice reads the
/// same (v1 `errorSig`).
pub fn error_sig(label: &str) -> String {
    let low = label.to_lowercase();
    let mut out = String::new();
    for word in low.split_whitespace() {
        let w = if word.contains('/') {
            // a path: what's before it stays (`at`), the path itself is `/…`
            match word.find('/') {
                Some(i) => format!("{}/…", &word[..i]),
                None => word.to_owned(),
            }
        } else if word.len() >= 7 && word.bytes().all(|b| b.is_ascii_hexdigit()) && word.bytes().any(|b| b.is_ascii_digit()) {
            "#".to_owned()
        } else {
            // a run of digits is one 0
            let mut w = String::new();
            let mut digits = false;
            for c in word.chars() {
                if c.is_ascii_digit() {
                    if !digits {
                        w.push('0');
                    }
                    digits = true;
                } else {
                    w.push(c);
                    digits = false;
                }
            }
            w
        };
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&w);
    }
    out.chars().take(120).collect()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // finite and clamped to 0..=100
fn pct(x: f64) -> u32 {
    let p = (x * 100.0).round();
    if p.is_finite() && p > 0.0 { p.min(100.0) as u32 } else { 0 }
}

/// The thread's health. `notes_age`: turns since the Notes were last written (None: no Notes).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
pub fn health(task: &str, turns: &[TurnIn], notes_age: Option<u32>) -> Health {
    let from = turns.iter().rposition(|t| t.fresh).unwrap_or(0);
    let thread = &turns[from..];
    let work: Vec<&TurnIn> = thread.iter().filter(|t| !t.compact && !t.held).collect();
    let compactions = u32::try_from(thread.iter().filter(|t| t.compact).count()).unwrap_or(u32::MAX);
    let n = u32::try_from(work.len()).unwrap_or(u32::MAX);
    let share = work.iter().rev().find_map(|t| t.context).filter(|(_, w)| *w > 0).map(|(c, w)| (c as f64 / w as f64).min(1.0));
    let (mut score, mut reasons) = (0u32, Vec::new());
    if let Some(s) = share.filter(|s| *s >= HEAVY) {
        score += if s >= WORN { 2 } else { 1 };
        reasons.push(format!("context {}% full", pct(s)));
    }
    if compactions >= COMPACTIONS {
        score += if compactions > COMPACTIONS { 2 } else { 1 };
        reasons.push(format!("compacted {compactions}×"));
    }
    if n >= TURNS {
        score += if n >= TURNS * 2 { 2 } else { 1 };
        reasons.push(format!("{n} turns in this thread"));
    }
    // the same failure over the last three turns: going round in circles
    let recent = &work[work.len().saturating_sub(3)..];
    let mut counts: Vec<(String, u32)> = Vec::new();
    for t in recent {
        for e in &t.errors {
            let sig = error_sig(e);
            match counts.iter_mut().find(|(s, _)| *s == sig) {
                Some((_, c)) => *c += 1,
                None => counts.push((sig, 1)),
            }
        }
    }
    let repeat = counts.into_iter().max_by_key(|(_, c)| *c).filter(|(_, c)| *c >= REPEATS).map(|(sig, n)| Repeat { sig, n });
    if let Some(r) = &repeat {
        score += 2;
        let span = if recent.len() > 1 { format!("{} turns", recent.len()) } else { "turn".into() };
        reasons.push(format!("the same error {}× in the last {span}", r.n));
    }
    if recent.len() == 3 && recent.iter().all(|t| t.failed) {
        score += 1;
        reasons.push("the last 3 turns didn’t get there".into());
    }
    let reverted = reverted(&work);
    if reverted >= REVERTED {
        score += 1;
        reasons.push(format!("{reverted} edits undone"));
    }
    if notes_age.is_some_and(|a| a >= STALE) && n >= STALE {
        reasons.push(format!("Notes not updated in {} turns", notes_age.unwrap_or(0)));
    }
    Health {
        task: task.to_owned(),
        level: if score >= 3 {
            Level::Worn
        } else if score >= 1 {
            Level::Heavy
        } else {
            Level::Ok
        },
        reasons,
        turns: n,
        compactions,
        share,
        repeat,
        reverted,
        notes_age,
    }
}

/// Edits that undo an earlier edit of this thread (the same file, its after back to its before).
fn reverted(work: &[&TurnIn]) -> u32 {
    let mut seen: Vec<&EditIn> = Vec::new();
    let mut n = 0u32;
    for t in work {
        for e in &t.edits {
            if e.before != e.after && seen.iter().any(|x| x.file == e.file && x.after == e.before && x.before == e.after) {
                n += 1;
            }
        }
        seen.extend(t.edits.iter());
    }
    n
}
