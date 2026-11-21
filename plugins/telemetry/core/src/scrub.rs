//! Paths in messages and stacks would leak usernames and project names (v1 server/telemetry.js, unchanged):
//! wisp's own files keep their package-relative path (`<wisp>/…`), the home folder becomes `~`, and any
//! other absolute path is cut down to its file name (`…/a.ts`). This is v1's regex
//! `(?<![\w<>~.:/\\-])(?:~|[A-Za-z]:)?[/\\](?:[^\s'"`():/\\]+[/\\])+([^\s'"`():/\\]*)` → `…/$1`, written
//! out as a scanner (no regex crate), then cut to 500 UTF-16 units.

use serde_json::{Value, json};

use crate::text::slice_utf16;

#[derive(Debug, Clone, Default)]
pub struct ScrubOptions {
    /// the user's home folder
    pub home: String,
    /// wisp's own install or source folder
    pub root: String,
    /// other spellings of the home folder (Windows: the long name behind an 8.3 short one, `USERPROFILE`
    /// when `HOME` is set too): each becomes `~` like `home`
    pub also_home: Vec<String>,
}

/// One character of a folder, as Windows compares it: either slash is a separator and ASCII letters are
/// equal in any case (the drive `C:`/`c:`, `RUNNER~1`/`runner~1`). Elsewhere this scrubs a little more, never less.
fn same(a: char, b: char) -> bool {
    (slash(a) && slash(b)) || a.eq_ignore_ascii_case(&b)
}

/// `s` with every spelling of the folder `dir` (either separator, any ASCII case) replaced by `to`, only where
/// the folder ends (`/Users/ada` is not cut out of `/Users/adam`).
fn replace_dir(s: &str, dir: &str, to: &str) -> String {
    let d: Vec<char> = dir.trim_end_matches(['/', '\\']).chars().collect();
    if d.is_empty() {
        return s.to_owned();
    }
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        let end = i + d.len();
        if end <= cs.len() && cs[i..end].iter().zip(&d).all(|(a, b)| same(*a, *b)) && cs.get(end).is_none_or(|c| !seg(*c)) {
            out.push_str(to);
            i = end;
        } else {
            out.push(cs[i]);
            i += 1;
        }
    }
    out
}

/// `s` with the home folder, in every spelling `o` knows, as `~` (the longest spelling first).
pub fn replace_home(s: &str, o: &ScrubOptions) -> String {
    let mut homes: Vec<&str> =
        std::iter::once(o.home.as_str()).chain(o.also_home.iter().map(String::as_str)).filter(|h| !h.is_empty()).collect();
    homes.sort_by_key(|h| std::cmp::Reverse(h.chars().count()));
    homes.into_iter().fold(s.to_owned(), |s, h| replace_dir(&s, h, "~"))
}

/// The 8.3 short spellings of the home folder (`C:\\Users\\RUNNER~1`) that the folders the system names show
/// (`TEMP`, `TMP`, `LOCALAPPDATA`, `APPDATA`), from each one as given and its long canonical path: where the long
/// path lies in `home_long`, the given one's first as many parts is the home under another name. Pure: the
/// caller resolves the canonical paths.
pub fn short_homes(home_long: &str, seen: &[(String, String)]) -> Vec<String> {
    let parts = |p: &str| -> Vec<String> { p.split(['/', '\\']).filter(|x| !x.is_empty()).map(str::to_owned).collect() };
    let home = parts(home_long);
    if home.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    for (given, long) in seen {
        let (g, l) = (parts(given), parts(long));
        let under = l.len() >= home.len() && l.iter().zip(&home).all(|(a, b)| a.eq_ignore_ascii_case(b));
        if !under || g.len() != l.len() {
            continue; // a short name stands for one part: the counts match, or this one says nothing
        }
        let sep = if given.contains('\\') { "\\" } else { "/" };
        let lead = if given.starts_with(['/', '\\']) { sep } else { "" };
        let alias = format!("{lead}{}", g[..home.len()].join(sep));
        if !alias.eq_ignore_ascii_case(home_long.trim_end_matches(['/', '\\'])) && !out.iter().any(|o| o.eq_ignore_ascii_case(&alias)) {
            out.push(alias);
        }
    }
    out
}

fn slash(c: char) -> bool {
    c == '/' || c == '\\'
}

/// A character of a path segment: not whitespace, a quote, a backtick, a parenthesis, a colon or a slash.
fn seg(c: char) -> bool {
    !(c.is_whitespace() || c == '\u{feff}' || matches!(c, '\'' | '"' | '`' | '(' | ')' | ':' | '/' | '\\'))
}

/// The lookbehind `[\w<>~.:/\\-]` (JS `\w` is ASCII).
fn glued(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '<' | '>' | '~' | '.' | ':' | '/' | '\\' | '-')
}

/// From a slash at `j`: at least one `segment/`, then the base name. Returns (end, where the base starts).
fn path_from(cs: &[char], j: usize) -> Option<(usize, usize)> {
    if !cs.get(j).copied().is_some_and(slash) {
        return None;
    }
    let (mut k, mut base, mut dirs) = (j + 1, j + 1, 0);
    loop {
        let s = k;
        while k < cs.len() && seg(cs[k]) {
            k += 1;
        }
        if k > s && k < cs.len() && slash(cs[k]) {
            k += 1;
            dirs += 1;
            base = k;
        } else {
            break;
        }
    }
    (dirs > 0).then_some((k, base))
}

fn match_at(cs: &[char], i: usize) -> Option<(usize, usize)> {
    if i > 0 && glued(cs[i - 1]) {
        return None;
    }
    let prefixed = match cs[i] {
        '~' => Some(i + 1),
        c if c.is_ascii_alphabetic() && cs.get(i + 1) == Some(&':') => Some(i + 2),
        _ => None,
    };
    prefixed.and_then(|j| path_from(cs, j)).or_else(|| path_from(cs, i))
}

/// Cuts every absolute path outside wisp down to its file name.
fn cut_paths(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        if let Some((end, base)) = match_at(&cs, i) {
            out.push_str("…/");
            out.extend(&cs[base..end]);
            i = end;
        } else {
            out.push(cs[i]);
            i += 1;
        }
    }
    out
}

pub fn scrub(text: &str, o: &ScrubOptions) -> String {
    let mut s = text.to_owned();
    if !o.root.is_empty() {
        s = s.replace(&o.root, "<wisp>");
    }
    s = replace_home(&s, o);
    slice_utf16(&cut_paths(&s), 500).to_owned()
}

/// `at fn (file:line:col)` or `at file:line:col` (v1's frame regex): (function, file, line, col).
fn js_frame(line: &str) -> Option<(Option<&str>, &str, u32, u32)> {
    let rest = line.strip_prefix("at ")?;
    let mut from = 0;
    while let Some(k) = rest[from..].find(" (").map(|k| k + from) {
        if k > 0
            && let Some((file, line, col)) = location(&rest[k + 2..])
        {
            return Some((Some(&rest[..k]), file, line, col));
        }
        from = k + 1;
    }
    location(rest).map(|(file, l, c)| (None, file, l, c))
}

/// `file:line:col` with v1's optional closing parenthesis.
fn location(loc: &str) -> Option<(&str, u32, u32)> {
    loc.strip_suffix(')').and_then(file_line_col).or_else(|| file_line_col(loc))
}

/// `file:line:col` with a non-empty file and all-digit line and col.
fn file_line_col(loc: &str) -> Option<(&str, u32, u32)> {
    let (rest, col) = loc.rsplit_once(':')?;
    let (file, line) = rest.rsplit_once(':')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if file.is_empty() || !digits(line) || !digits(col) {
        return None;
    }
    Some((file, line.parse().unwrap_or(u32::MAX), col.parse().unwrap_or(u32::MAX)))
}

/// A stack as `PostHog`'s raw frames, innermost last. `JavaScript` stacks (the page, v1) as v1 sent them;
/// Rust backtraces (`N: function` / `at file:line:col`) the same way, `lang: "rust"`.
pub fn frames(stack: &str, o: &ScrubOptions) -> Vec<Value> {
    let lines: Vec<&str> = stack.split('\n').collect();
    let mut out: Vec<Value> = Vec::new();
    for line in lines.iter().skip(1).take(29) {
        if let Some((func, file, l, c)) = js_frame(line.trim()) {
            let in_app = !o.root.is_empty() && file.contains(&o.root) && !file.contains("node_modules");
            let file = file.strip_prefix("file://").unwrap_or(file);
            out.push(json!({ "platform": "custom", "lang": "javascript", "function": func.unwrap_or("?"), "filename": scrub(file, o), "lineno": l, "colno": c, "in_app": in_app }));
        }
    }
    if out.is_empty() {
        out = rust_frames(&lines, o);
    }
    out.reverse();
    out
}

fn rust_frames(lines: &[&str], o: &ScrubOptions) -> Vec<Value> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() && out.len() < 29 {
        let t = lines[i].trim();
        let func = t.split_once(": ").filter(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).map(|(_, f)| f);
        i += 1;
        let Some(func) = func else { continue };
        let loc = lines.get(i).and_then(|l| l.trim().strip_prefix("at ")).and_then(file_line_col);
        if loc.is_some() {
            i += 1;
        }
        let (file, lineno, colno) = loc.unwrap_or(("?", 0, 0));
        out.push(json!({ "platform": "custom", "lang": "rust", "function": func, "filename": scrub(file, o), "lineno": lineno, "colno": colno, "in_app": func.starts_with("wisp") }));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn win() -> ScrubOptions {
        let seen = [(r"C:\Users\RUNNER~1\AppData\Local\Temp".to_owned(), r"C:\Users\runneradmin\AppData\Local\Temp".to_owned())];
        ScrubOptions { home: r"C:\Users\runneradmin".into(), root: String::new(), also_home: short_homes(r"C:\Users\runneradmin", &seen) }
    }

    #[test]
    fn a_windows_home_is_scrubbed_in_every_spelling() {
        let o = win();
        assert_eq!(o.also_home, vec![r"C:\Users\RUNNER~1".to_owned()]);
        for p in [
            r"C:\Users\runneradmin\notes\secret.md",
            "C:/Users/runneradmin/notes/secret.md",
            r"c:\users\RunnerAdmin\notes\secret.md",
            r"C:\Users\RUNNER~1\notes\secret.md",
            r"c:/users/runner~1/notes/secret.md",
        ] {
            let out = scrub(&format!("can't open {p}"), &o);
            assert_eq!(out, "can't open …/secret.md", "{p}");
            assert!(!replace_home(p, &o).to_ascii_lowercase().contains("runner"), "{p}: {}", replace_home(p, &o));
        }
    }

    #[test]
    fn a_home_is_cut_only_where_its_folder_ends() {
        let o = ScrubOptions { home: "/Users/ada".into(), ..ScrubOptions::default() };
        assert_eq!(replace_home("/Users/adam/x and /Users/ada/y and /Users/ada", &o), "/Users/adam/x and ~/y and ~");
    }

    #[test]
    fn a_short_name_is_learned_only_from_a_folder_under_the_home() {
        let h = r"C:\Users\runneradmin";
        let seen = |g: &str, l: &str| vec![(g.to_owned(), l.to_owned())];
        assert!(short_homes(h, &seen(r"D:\a\_temp", r"D:\a\_temp")).is_empty(), "elsewhere: nothing");
        assert!(
            short_homes(h, &seen(r"C:\Users\runneradmin\AppData", r"C:\Users\runneradmin\AppData")).is_empty(),
            "the same spelling: nothing new"
        );
        assert!(
            short_homes(h, &seen(r"C:\Users\RUNNER~1\Temp", r"C:\Users\runneradmin\AppData\Local\Temp")).is_empty(),
            "parts don't line up: nothing"
        );
        assert_eq!(short_homes(h, &seen(r"C:\PROGRA~1\x", r"C:\Program Files\x")), Vec::<String>::new());
    }
}
