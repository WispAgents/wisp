//! PR links in what someone wrote: a prompt, a follow-up, a channel message. Pasting one is how a person says
//! "this task is about that PR" (ADR-0145), so wisp attaches it as `wisp_pr` would, without being asked twice.

/// At most this many links from one message: a pasted changelog of PRs is not a request to follow them all.
pub const MAX_PER_MESSAGE: usize = 5;

/// The GitHub PR links in `text`, as `https://github.com/<owner>/<repo>/pull/<n>`, each once, in the order
/// they first appear, at most [`MAX_PER_MESSAGE`]. A link with a tail (`/files`, `#discussion…`, `?w=1`) is the
/// same PR. Anything that isn't github.com (an enterprise host, a look-alike) is left alone.
#[must_use]
pub fn pr_links(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("github.com/") {
        let before = &rest[..at];
        let tail = &rest[at + "github.com/".len()..];
        rest = tail;
        // the host must be github.com itself: `notgithub.com/` and `github.com.evil/` don't count
        if !host_starts(before) {
            continue;
        }
        let Some(url) = parse(tail) else { continue };
        if !out.contains(&url) {
            out.push(url);
            if out.len() == MAX_PER_MESSAGE {
                break;
            }
        }
    }
    out
}

/// What comes before `github.com/` is a start of text, a space or bracket, `//`, or `www.`.
fn host_starts(before: &str) -> bool {
    let b = before.strip_suffix("www.").unwrap_or(before);
    match b.chars().last() {
        None => true,
        Some(c) => c == '/' || c.is_whitespace() || "<([\"'`*_".contains(c),
    }
}

/// `owner/repo/pull/123…` → the canonical URL.
fn parse(tail: &str) -> Option<String> {
    let mut parts = tail.splitn(4, '/');
    let owner = parts.next().filter(|s| name(s))?;
    let repo = parts.next().filter(|s| name(s))?;
    if parts.next() != Some("pull") {
        return None;
    }
    let n: String = parts.next()?.chars().take_while(char::is_ascii_digit).collect();
    if n.is_empty() || n.starts_with('0') || n.len() > 10 {
        return None;
    }
    Some(format!("https://github.com/{owner}/{repo}/pull/{n}"))
}

/// A GitHub owner or repo name: letters, digits, `-`, `_`, `.`; never `.` or `..` alone.
fn name(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && s.len() <= 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pasted_pr_link_is_found_once_in_canonical_form() {
        let t = "look at https://github.com/WispAgents/wisp/pull/1056/files and https://github.com/WispAgents/wisp/pull/1056#discussion_r1";
        assert_eq!(pr_links(t), vec!["https://github.com/WispAgents/wisp/pull/1056"]);
    }

    #[test]
    fn links_in_markdown_brackets_and_without_a_scheme_count() {
        let t = "[fix](https://github.com/a/b/pull/2), <http://www.github.com/a/c/pull/3>, github.com/a/d/pull/4.";
        assert_eq!(pr_links(t), vec!["https://github.com/a/b/pull/2", "https://github.com/a/c/pull/3", "https://github.com/a/d/pull/4"]);
    }

    #[test]
    fn issues_lookalike_hosts_and_bad_numbers_are_left_alone() {
        for t in [
            "https://github.com/a/b/issues/2",
            "https://notgithub.com/a/b/pull/2",
            "https://github.com/a/b/pull/",
            "https://github.com/a/b/pull/0",
            "https://github.com/a/b/pull/x1",
            "https://github.com/../b/pull/1",
            "https://github.com/a/b/pulls",
            "no links here",
        ] {
            assert!(pr_links(t).is_empty(), "{t}");
        }
    }

    #[test]
    fn a_message_full_of_prs_attaches_only_the_first_five() {
        let t = (1..=9).map(|n| format!("https://github.com/a/b/pull/{n}")).collect::<Vec<_>>().join(" ");
        let got = pr_links(&t);
        assert_eq!(got.len(), MAX_PER_MESSAGE);
        assert_eq!(got[0], "https://github.com/a/b/pull/1");
        assert_eq!(got[4], "https://github.com/a/b/pull/5");
    }

    #[test]
    fn any_text_is_safe_and_every_find_is_canonical() {
        // a cheap fuzz over awkward slices (multi-byte chars around the needle): no panic, every hit canonical
        let seeds = ["é github.com/", "github.com/ä/b/pull/1", "😀github.com/a/b/pull/12😀", "github.com/a/b/pull/9999999999999"];
        for s in seeds {
            for cut in 0..=s.len() {
                if !s.is_char_boundary(cut) {
                    continue;
                }
                for u in pr_links(&s[..cut]) {
                    assert!(u.starts_with("https://github.com/") && u.contains("/pull/"), "{u}");
                }
            }
        }
    }
}
