//! A failed job's log for the Steward: timestamps and ANSI codes out, its end kept, and the words its fingerprint
//! is made of (numbers and ids taken out).

/// GitHub's log lines start with an ISO timestamp.
pub(super) fn strip_stamp(l: &str) -> &str {
    let b = l.as_bytes();
    if b.len() > 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && let Some(i) = l.find(' ')
    {
        return &l[i + 1..];
    }
    l
}

/// Digits become 0; a run of 7+ hex characters (a sha, an address) becomes `#`.
pub(super) fn normalize(l: &str) -> String {
    let mut out = String::new();
    for word in l.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        let hexish = word.len() >= 7 && word.chars().all(|c| c.is_ascii_hexdigit()) && word.chars().any(|c| c.is_ascii_digit());
        if hexish {
            out.push('#');
        } else {
            out.extend(word.chars().map(|c| if c.is_ascii_digit() { '0' } else { c }));
        }
    }
    out
}

/// A job's log as the prompt quotes it: ANSI codes and timestamps out, its last `LOG_LINES` lines.
pub fn log_tail(raw: &str) -> String {
    let clean: Vec<String> = raw.lines().map(|l| strip_ansi(strip_stamp(l))).collect();
    let from = clean.len().saturating_sub(super::LOG_LINES);
    clean[from..].join("\n")
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// The last `max` bytes of `s`, cut at a line start.
pub(super) fn tail_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut at = s.len() - max;
    while !s.is_char_boundary(at) {
        at += 1;
    }
    let cut = &s[at..];
    cut.find('\n').map_or(cut, |i| &cut[i + 1..])
}
