//! The brief a fresh thread starts from (v1 web/js/threads.js `freshBrief`): the Notes as they are, or what was asked
//! first when there are none, the files the task changed, and its last two turns. You read and edit it before it goes;
//! it is the new conversation's first message.

/// The brief, at most (v1 `FRESH_MAX`).
pub const BRIEF_MAX: usize = 24_000;
/// What was asked first, at most, when there are no Notes.
pub const FIRST_MAX: usize = 3_000;
/// Each of the last turns' request and answer, at most.
pub const TURN_MAX: usize = 3_000;
/// Files listed, at most (the latest).
pub const FILES_MAX: usize = 40;

/// One of the last turns: what was asked and what it answered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Recent<'a> {
    pub n: u32,
    pub prompt: &'a str,
    pub answer: &'a str,
}

/// What a brief is made from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BriefIn<'a> {
    /// the Notes as markdown (`## Goal` …), '' when there are none
    pub notes: &'a str,
    /// the task's first request (used when there are no Notes)
    pub first: &'a str,
    /// files the task changed, oldest first
    pub files: &'a [String],
    pub branch: Option<&'a str>,
    pub recent: &'a [Recent<'a>],
    /// how the agent's harness names `wisp_notes`
    pub tool: &'a str,
}

/// `text` cut to `max` characters (between characters), with `…` when cut.
pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

/// An answer as the brief quotes it: without its ```` ```wisp ```` block (the presentation's JSON, not words).
pub fn plain(answer: &str) -> String {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in answer.lines() {
        let t = line.trim_start();
        if !fenced && t.starts_with("```wisp") {
            fenced = true;
        } else if fenced && t.starts_with("```") {
            fenced = false;
        } else if !fenced {
            out.push(line);
        }
    }
    out.join("\n").trim().to_owned()
}

/// The brief.
pub fn brief(b: &BriefIn<'_>) -> String {
    let mut out = vec![
        "# Fresh thread".to_owned(),
        format!(
            "This task carries on in a new conversation: the old one got long. Everything that matters from it is below. Trust this over anything you would otherwise assume, check the work as it stands before you change it, and keep the task's Notes current with `{}`.",
            b.tool
        ),
    ];
    if b.notes.trim().is_empty() {
        if !b.first.trim().is_empty() {
            out.push(format!("## What was asked first\n\n{}", clip(b.first.trim(), FIRST_MAX)));
        }
    } else {
        // one level down: the brief's own sections are `##`
        let notes: Vec<String> =
            b.notes.lines().map(|l| l.strip_prefix("## ").map_or_else(|| l.to_owned(), |h| format!("### {h}"))).collect();
        out.push(format!("## Task Notes\n\n{}", notes.join("\n")));
    }
    let mut files: Vec<&str> = Vec::new();
    for f in b.files.iter().rev() {
        if !files.contains(&f.as_str()) {
            files.push(f);
        }
    }
    files.truncate(FILES_MAX);
    files.reverse();
    let mut work = Vec::new();
    if let Some(br) = b.branch.filter(|x| !x.is_empty()) {
        work.push(format!("- Branch `{br}`"));
    }
    if !files.is_empty() {
        work.push(format!("- Files changed in this task: {}", files.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", ")));
    }
    if !work.is_empty() {
        out.push(format!("## The work\n\n{}", work.join("\n")));
    }
    let turns: Vec<String> = b
        .recent
        .iter()
        .filter(|r| !r.prompt.trim().is_empty() || !r.answer.trim().is_empty())
        .map(|r| {
            format!(
                "### Turn {}\n\n**Asked:** {}\n\n**Answer:** {}",
                r.n,
                clip(r.prompt.trim(), TURN_MAX),
                clip(&plain(r.answer), TURN_MAX)
            )
        })
        .collect();
    if !turns.is_empty() {
        let head = if turns.len() > 1 { format!("The last {} turns", turns.len()) } else { "The last turn".to_owned() };
        out.push(format!("## {head}\n\n{}", turns.join("\n\n")));
    }
    clip(&out.join("\n\n"), BRIEF_MAX)
}
