//! Secrets in text: v1's web/js/secret-scan.js patterns, which a connector's command or URL and every string of a
//! formula's specs are checked against (a token pasted there would sit in the store, not in the secrets file). The one
//! list: wisp.connectors and wisp.resources (AAC-4) both read it here.

use std::sync::LazyLock;

use regex::Regex;

/// (what it looks like, pattern). The last one is checked further in [`generic_value`]: the regex
/// crate has no look-ahead, and v1's pattern skips placeholders with one.
const PATTERNS: [(&str, &str); 13] = [
    ("a private key", r"-----BEGIN [A-Z ]*PRIVATE KEY( BLOCK)?-----"),
    ("a GitHub token", r"\b(gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})\b"),
    ("an Anthropic API key", r"\bsk-ant-[A-Za-z0-9_-]{20,}"),
    ("an OpenAI API key", r"\bsk-(proj-|svcacct-)?[A-Za-z0-9_-]{32,}"),
    ("an AWS access key", r"\b(AKIA|ASIA)[0-9A-Z]{16}\b"),
    ("a Google API key", r"\bAIza[0-9A-Za-z_-]{35}\b"),
    ("a Slack token", r"\bxox[abposr]-[A-Za-z0-9-]{10,}"),
    ("a Discord bot token", r"\b[MNO][A-Za-z0-9]{23,}\.[\w-]{6}\.[\w-]{27,}"),
    ("a Stripe key", r"\b[sr]k_live_[A-Za-z0-9]{20,}"),
    ("an npm token", r"\bnpm_[A-Za-z0-9]{36}\b"),
    ("a JSON Web Token", r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"),
    ("a password in a URL", r"(?i)\b[a-z][a-z0-9+.-]*://[^\s:/@]+:[^\s/@]{4,}@"),
    (
        "a secret value",
        r#"(?i)\b[\w-]*(api[_-]?key|secret|token|passw(or)?d|private[_-]?key)\b["']?\s*[:=]\s*["']?(?P<v>[A-Za-z0-9/+_.=-]{16,})"#,
    ),
];

/// What a placeholder starts with (v1's negative look-ahead, case-insensitive).
const PLACEHOLDERS: [&str; 6] = ["xxx", "your_", "your-", "example", "placeholder", "changeme"];

static COMPILED: LazyLock<Vec<(&'static str, Regex)>> =
    LazyLock::new(|| PATTERNS.iter().filter_map(|(what, re)| Regex::new(re).ok().map(|r| (*what, r))).collect());

/// How many patterns compiled (all of them; a test holds it to that).
pub fn pattern_count() -> usize {
    COMPILED.len()
}

fn generic_value(re: &Regex, line: &str) -> bool {
    re.captures_iter(line).any(|c| {
        c.name("v").is_some_and(|v| {
            let v = v.as_str().to_ascii_lowercase();
            !PLACEHOLDERS.iter().any(|p| v.starts_with(p))
        })
    })
}

/// One hit for a line: the first kind that matches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 1-based
    pub line: usize,
    pub what: &'static str,
}

/// text → one hit per line at most: what it looks like, never the secret.
pub fn scan_secrets(text: &str) -> Vec<Hit> {
    let mut out = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let hit = COMPILED.iter().find(|(what, re)| if *what == "a secret value" { generic_value(re, l) } else { re.is_match(l) });
        if let Some((what, _)) = hit {
            out.push(Hit { line: i + 1, what });
        }
    }
    out
}

/// The byte spans of each `${secret:NAME}` and `${env:NAME}` reference in `text`, braces included. A reference names
/// where a secret comes from and is never one itself, though `secret:LINEAR_WEBHOOK_SECRET` reads as `secret: <a long
/// value>` to the generic pattern: masked, an export's own references came back as refused "masked values". Only a
/// well-formed one counts: NAME an identifier (`[A-Za-z_][A-Za-z0-9_]*`) closed at once by `}`, and not itself a token
/// (`${secret:ghp_…}` is a GitHub token in braces). Anything up to the next `}` used to count, across spaces and lines,
/// so a token inside a malformed or unclosed reference went out unmasked and unrefused. In order; wisp.resources'
/// plan check skips the same spans.
pub fn reference_spans(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    for open in ["${secret:", "${env:"] {
        let mut from = 0;
        while let Some(at) = text[from..].find(open) {
            let start = from + at;
            let name_at = start + open.len();
            let len = text[name_at..].bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
            let name = &text[name_at..name_at + len];
            let well_formed = name.bytes().next().is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
                && text[name_at + len..].starts_with('}')
                && !COMPILED.iter().any(|(what, re)| *what != "a secret value" && re.is_match(name));
            from = name_at;
            if well_formed {
                out.push(start..name_at + len + 1);
                from = name_at + len + 1;
            }
        }
    }
    out.sort_by_key(|r| r.start);
    out
}

/// Where each secret in `text` is (byte ranges, in order, not overlapping) and what it looks like: the token itself, or
/// for "a secret value" its value alone (`api_key: <this>`), so a mask keeps what names it. Nothing inside a
/// `${secret:NAME}` or `${env:NAME}` reference counts.
pub fn find_secrets(text: &str) -> Vec<(&'static str, std::ops::Range<usize>)> {
    let refs = reference_spans(text);
    let mut out: Vec<(&'static str, std::ops::Range<usize>)> = Vec::new();
    for (what, re) in COMPILED.iter() {
        let found: Vec<std::ops::Range<usize>> = if *what == "a secret value" {
            re.captures_iter(text)
                .filter_map(|c| c.name("v"))
                .filter(|v| !PLACEHOLDERS.iter().any(|p| v.as_str().to_ascii_lowercase().starts_with(p)))
                .map(|v| v.range())
                .collect()
        } else {
            re.find_iter(text).map(|m| m.range()).collect()
        };
        for r in found {
            let overlaps = |o: &std::ops::Range<usize>| o.start < r.end && r.start < o.end;
            if !out.iter().any(|(_, o)| overlaps(o)) && !refs.iter().any(overlaps) {
                out.push((what, r));
            }
        }
    }
    out.sort_by_key(|(_, r)| r.start);
    out
}

/// `text` with each secret [`find_secrets`] finds as its first four characters and `…(masked)` (`ghp_…(masked)`): what
/// kind of token it was stays readable, the token doesn't.
pub fn mask_secrets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (_, r) in find_secrets(text) {
        out.push_str(&text[at..r.start]);
        out.extend(text[r.clone()].chars().take(4));
        out.push_str("…(masked)");
        at = r.end;
    }
    out.push_str(&text[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_compiles_and_a_mask_keeps_only_the_tokens_head() {
        assert_eq!(pattern_count(), PATTERNS.len());
        let token = format!("ghp_{}", "a".repeat(36));
        assert_eq!(mask_secrets(&format!("Bearer {token}")), "Bearer ghp_…(masked)");
        assert_eq!(scan_secrets(&format!("Bearer {token}")).first().map(|h| h.what), Some("a GitHub token"));
        assert_eq!(mask_secrets("api_key: abcdefghijklmnopqrst"), "api_key: abcd…(masked)");
        assert_eq!(mask_secrets("Bearer ${secret:GH_TOKEN}"), "Bearer ${secret:GH_TOKEN}");
        assert_eq!(mask_secrets("api_key: your_key_goes_here_please"), "api_key: your_key_goes_here_please", "a placeholder");
    }

    /// `secret:ANTHROPIC_API_KEY` is `secret: <16+ chars>` to the generic pattern: an export masked its own references,
    /// and re-applying it was refused as a masked value.
    #[test]
    fn a_reference_is_never_a_secret() {
        assert_eq!(find_secrets("${secret:ANTHROPIC_API_KEY}"), vec![]);
        assert_eq!(find_secrets("${env:LINEAR_WEBHOOK_SECRET}"), vec![]);
        assert_eq!(find_secrets("x ${secret:LINEAR_WEBHOOK_SECRET} y ${env:GITHUB_TOKEN_FOR_CI_RUNS}"), vec![]);
        let token = format!("ghp_{}", "a".repeat(36));
        let text = format!("${{secret:ANTHROPIC_API_KEY}} token={token}");
        let found = find_secrets(&text);
        assert_eq!(found.len(), 1);
        assert_eq!(&text[found[0].1.clone()], token);
    }

    /// final4 F4: anything from `${secret:`/`${env:` to the next `}` was a reference, across spaces and lines, so a
    /// token inside a malformed or unclosed one, or one whose NAME is the token, went out unmasked and unrefused.
    #[test]
    fn only_a_well_formed_reference_is_skipped() {
        let token = format!("ghp_{}", "a".repeat(36));
        assert_eq!(mask_secrets(&format!("Authorization: Bearer ${{secret:{token}}}")), "Authorization: Bearer ${secret:ghp_…(masked)}");
        let key = format!("sk-ant-{}", "b".repeat(40));
        assert_eq!(mask_secrets(&format!("${{env: x token={key}}} ")), "${env: x token=sk-a…(masked)} ");
        assert_eq!(mask_secrets(&format!("${{secret:A\n{token}\n}}")), "${secret:A\nghp_…(masked)\n}");
        assert_eq!(mask_secrets("${secret:LINEAR_WEBHOOK_SECRET}"), "${secret:LINEAR_WEBHOOK_SECRET}");
        assert_eq!(mask_secrets("${env:HOME_DIR}"), "${env:HOME_DIR}");
        assert_eq!(reference_spans("a ${secret:B} ${env:_C1} ${secret:1x} ${env:}"), vec![2..13, 14..24]);
    }
}
