//! The Notes' sections (v1 web/js/threads.js SECTIONS) and the rewrite of one. A section is rewritten, never appended
//! to, so an instruction that changed is gone, not left beside its replacement.

use wisp_threads_api::SECTIONS;

/// One item of a list section, at most.
pub const ITEM_MAX: usize = 400;
/// A one-text section (goal, state), at most.
pub const ONE_MAX: usize = 1200;
/// Items a list section keeps (the oldest go first).
pub const LIST_MAX: usize = 20;

/// A section's facts: its label, whether it's one text, and the hint the agent and the Thread tab show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meta {
    pub id: &'static str,
    pub label: &'static str,
    pub one: bool,
    pub hint: &'static str,
}

pub const META: [Meta; 7] = [
    Meta { id: "goal", label: "Goal", one: true, hint: "What done looks like, in a sentence or two" },
    Meta { id: "constraints", label: "Constraints", one: false, hint: "Must and must-not: stack, scope, style, what not to touch" },
    Meta { id: "decisions", label: "Decisions", one: false, hint: "What was chosen, and why" },
    Meta { id: "failed", label: "Tried and failed", one: false, hint: "What didn’t work, and why: so nobody tries it again" },
    Meta { id: "state", label: "Where it stands", one: true, hint: "What’s done, what’s next" },
    Meta { id: "open", label: "Open questions", one: false, hint: "What’s still undecided or unknown" },
    Meta { id: "files", label: "Key files", one: false, hint: "Paths that matter, with a word on each" },
];

pub fn meta(id: &str) -> Option<&'static Meta> {
    META.iter().find(|m| m.id == id)
}

/// Why a section id isn't one, in words an agent acts on.
pub fn unknown(id: &str) -> String {
    format!("no section \"{id}\" (one of {})", SECTIONS.join(", "))
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// Lowercase, words only: "No Redis." and "no redis" are the same item.
fn key(s: &str) -> String {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// A list item: one line; a leading `- `, `* `, `• ` or `1. ` the writer typed is the list's, not the item's.
fn item(line: &str) -> String {
    let t = line.trim();
    let t = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("• ")).unwrap_or(t);
    let t = match t.split_once(['.', ')']) {
        Some((n, rest)) if !n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()) && rest.starts_with(' ') => rest,
        _ => t,
    };
    cut(t.trim(), ITEM_MAX)
}

/// The text a section keeps for `text` as written: a one-text section trimmed and cut; a list section's items one
/// a line, marks gone, repeats gone, the newest `LIST_MAX`. → (text, problems for the writer).
pub fn clean(id: &str, text: &str) -> (String, Vec<String>) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let Some(m) = meta(id) else { return (String::new(), vec![unknown(id)]) };
    if m.one {
        let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
        return (cut(lines.join("\n").trim(), ONE_MAX), Vec::new());
    }
    let mut items: Vec<String> = Vec::new();
    for x in text.lines().map(item).filter(|x| !x.is_empty()) {
        if !items.iter().any(|y| key(y) == key(&x)) {
            items.push(x);
        }
    }
    let mut problems = Vec::new();
    if items.len() > LIST_MAX {
        problems.push(format!("{id}: only {LIST_MAX} items are kept; merge or drop old ones (the oldest went)"));
        items.drain(..items.len() - LIST_MAX);
    }
    (items.join("\n"), problems)
}

/// One section as the Notes show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown<'a> {
    pub id: &'a str,
    pub text: &'a str,
}

/// The Notes as markdown, sections in order, empty ones left out; `only`: these sections; past `max` characters
/// it's cut with a pointer to the rest.
pub fn markdown(sections: &[Shown<'_>], only: &[&str], max: usize, tool: &str) -> String {
    let mut parts = Vec::new();
    for m in &META {
        if !only.is_empty() && !only.contains(&m.id) {
            continue;
        }
        let Some(s) = sections.iter().find(|s| s.id == m.id && !s.text.trim().is_empty()) else { continue };
        let body = if m.one { s.text.to_owned() } else { s.text.lines().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n") };
        parts.push(format!("## {}\n{body}", m.label));
    }
    let md = parts.join("\n\n");
    if md.chars().count() <= max {
        return md;
    }
    format!("{}\n… (cut: the rest is in {tool})", md.chars().take(max).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_section_keeps_items_one_a_line_without_marks_or_repeats() {
        let (t, p) = clean("constraints", "- No Redis\n* no redis.\n2. Keep the API as it is\n\n  ");
        assert_eq!((t.as_str(), p.len()), ("No Redis\nKeep the API as it is", 0));
        let (one, _) = clean("goal", "  Ship rate limiting \n on /login  ");
        assert_eq!(one, "Ship rate limiting\n on /login");
        let many = (0..25).map(|i| format!("d{i}")).collect::<Vec<_>>().join("\n");
        let (t, p) = clean("decisions", &many);
        assert_eq!((t.lines().count(), t.lines().next()), (LIST_MAX, Some("d5")), "the newest 20");
        assert_eq!(p.len(), 1);
        assert_eq!(clean("plan", "x").1, vec![unknown("plan")]);
        assert_eq!(clean("goal", &"g".repeat(5000)).0.chars().count(), ONE_MAX);
        let md = markdown(
            &[Shown { id: "failed", text: "Polling: too slow" }, Shown { id: "goal", text: "G" }, Shown { id: "open", text: "" }],
            &[],
            usize::MAX,
            "wisp_notes",
        );
        assert_eq!(md, "## Goal\nG\n\n## Tried and failed\n- Polling: too slow", "in order, empty ones out");
    }
}
