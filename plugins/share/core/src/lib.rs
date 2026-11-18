//! wisp-share-core: what a share takes out before it leaves (ADR-0027), without IO.
//!
//! - **Secrets**: API keys and tokens by their shapes (OpenAI/Anthropic `sk-…`, GitHub `ghp_…` /
//!   `github_pat_…`, Slack `xox…`, AWS `AKIA…`, Google `AIza…`, bearer tokens, private key blocks, and
//!   `password=…`-style assignments) → `<secret>`.
//! - **Env**: the values of the sharer's secret-looking environment variables (`*KEY*`, `*TOKEN*`,
//!   `*SECRET*`, `*PASSWORD*`) → `<env:NAME>`.
//! - **Paths**: the project folder → `<project>`, the home folder → `~`.
//! - **Others**: where the text refers to another task or a crew member the sharer didn't choose (a mention chip,
//!   `@[Title](wisp:<id>)` / `@[Name](crew:<id>)`, or the bare id) → `<task>` / `<crew>`. Their titles and names as
//!   words stay: a task titled "login" never blanks "fix the login bug" (QA round 12 #3).
//!
//! The same scrub serves `wisp history export`.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::{Map, Value};
use wisp_share_api::{Bundle, Redaction};

/// What to take out besides the secret shapes.
#[derive(Debug, Clone, Default)]
pub struct Scrub {
    /// (name, value) of the sharer's secret-looking environment variables
    pub env: Vec<(String, String)>,
    pub project: String,
    pub home: String,
    /// other tasks' ids: a mention of one, or the id, → `<task>`
    pub tasks: Vec<String>,
    /// the ids of crew members the sharer didn't choose: a mention of one, or the id, → `<crew>`
    pub crew: Vec<String>,
}

fn mention() -> Option<&'static Regex> {
    static M: OnceLock<Option<Regex>> = OnceLock::new();
    M.get_or_init(|| Regex::new(r"@\[([^\]\n]{1,200})\]\((wisp|crew):([A-Za-z0-9_-]{4,64})\)").ok()).as_ref()
}

/// `t` with each mention chip of one of `ids` (of `kind`, `wisp` or `crew`) and each whole occurrence of one of the
/// ids (not inside a longer word) as `to`; what went, by its title or the id, in `went`.
fn others(t: &str, kind: &str, ids: &[String], to: &str, went: &mut Vec<String>) -> String {
    if ids.is_empty() {
        return t.to_owned();
    }
    let chips = mention().map_or_else(
        || t.to_owned(),
        |m| {
            m.replace_all(t, |c: &regex::Captures<'_>| {
                if &c[2] == kind && ids.iter().any(|i| *i == c[3]) {
                    went.push(c[1].to_owned());
                    to.to_owned()
                } else {
                    c[0].to_owned()
                }
            })
            .into_owned()
        },
    );
    let word = |ch: Option<char>| ch.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-');
    let mut out = chips;
    for id in ids.iter().filter(|i| i.len() >= 8) {
        let mut next = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(at) = rest.find(id.as_str()) {
            let (before, after) = (rest[..at].chars().next_back(), rest[at + id.len()..].chars().next());
            next.push_str(&rest[..at]);
            if word(before) || word(after) {
                next.push_str(id);
            } else {
                next.push_str(to);
                went.push(id.clone());
            }
            rest = &rest[at + id.len()..];
        }
        next.push_str(rest);
        out = next;
    }
    out
}

/// Environment variables whose values a share masks.
pub fn secret_env(vars: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vars
        .into_iter()
        .filter(|(k, v)| {
            let k = k.to_ascii_uppercase();
            v.len() >= 8 && ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL"].iter().any(|w| k.contains(w))
        })
        .collect();
    // longest first, so a value inside another is masked by the longer
    out.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    out
}

fn patterns() -> &'static [Regex] {
    static P: OnceLock<Vec<Regex>> = OnceLock::new();
    P.get_or_init(|| {
        [
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
            r"\bsk-(?:ant-|proj-)?[A-Za-z0-9_-]{20,}",
            r"\bgh[pousr]_[A-Za-z0-9]{30,}",
            r"\bgithub_pat_[A-Za-z0-9_]{30,}",
            r"\bxox[abprs]-[A-Za-z0-9-]{10,}",
            r"\bAKIA[0-9A-Z]{16}\b",
            r"\bAIza[0-9A-Za-z_-]{35}",
            r"(?i)\bbearer\s+[A-Za-z0-9._~+/-]{20,}=*",
            r#"(?i)\b(?:api[_-]?key|(?:access[_-]?)?token|secret|password|passwd)\b\s*[:=]\s*["']?[^\s"',;]{8,}"#,
        ]
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
    })
}

/// A secret shortened for the preview: its start and end only.
fn hint(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    if c.len() <= 8 {
        return "…".into();
    }
    format!("{}…{}", c[..4].iter().collect::<String>(), c[c.len() - 4..].iter().collect::<String>())
}

/// `text` scrubbed, with what was taken out (`at` is where it is).
pub fn scrub_text(text: &str, s: &Scrub, at: &str, out: &mut Vec<Redaction>) -> String {
    let mut t = text.to_owned();
    for re in patterns() {
        if re.is_match(&t) {
            let mut found = Vec::new();
            t = re
                .replace_all(&t, |c: &regex::Captures<'_>| {
                    found.push(c[0].to_owned());
                    "<secret>".to_owned()
                })
                .into_owned();
            out.extend(found.into_iter().map(|f| Redaction {
                kind: "secret".into(),
                at: at.into(),
                was: hint(&f),
                now: "<secret>".into(),
            }));
        }
    }
    for (name, v) in &s.env {
        if t.contains(v.as_str()) {
            let now = format!("<env:{name}>");
            t = t.replace(v.as_str(), &now);
            out.push(Redaction { kind: "env".into(), at: at.into(), was: hint(v), now });
        }
    }
    for (from, to) in [(s.project.as_str(), "<project>"), (s.home.as_str(), "~")] {
        let from = from.trim_end_matches('/');
        if from.len() > 1 && t.contains(from) {
            t = t.replace(&format!("/private{from}"), to).replace(from, to);
            out.push(Redaction { kind: "path".into(), at: at.into(), was: from.into(), now: to.into() });
        }
    }
    for (kind, ids, to) in [("wisp", &s.tasks, "<task>"), ("crew", &s.crew, "<crew>")] {
        let mut went = Vec::new();
        t = others(&t, kind, ids, to, &mut went);
        out.extend(went.into_iter().map(|was| Redaction { kind: "name".into(), at: at.into(), was, now: to.into() }));
    }
    t
}

/// Every string in `v` scrubbed (keys too, for paths).
pub fn scrub(v: &Value, s: &Scrub, at: &str, out: &mut Vec<Redaction>) -> Value {
    match v {
        Value::String(t) => Value::String(scrub_text(t, s, at, out)),
        Value::Array(a) => Value::Array(a.iter().enumerate().map(|(i, x)| scrub(x, s, &format!("{at}/{i}"), out)).collect()),
        Value::Object(o) => Value::Object(
            o.iter().map(|(k, x)| (scrub_text(k, s, at, &mut Vec::new()), scrub(x, s, &format!("{at}/{k}"), out))).collect::<Map<_, _>>(),
        ),
        other => other.clone(),
    }
}

/// The bytes a bundle's signature is over: its JSON without `sig`, keys sorted.
pub fn canonical(b: &Bundle) -> Vec<u8> {
    let mut v = serde_json::to_value(b).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        o.remove("sig");
    }
    // serde_json's map is ordered by key: the text is canonical
    v.to_string().into_bytes()
}

/// How many redactions of each kind.
pub fn counts(r: &[Redaction]) -> Value {
    let mut m = Map::new();
    for x in r {
        let n = m.get(&x.kind).and_then(Value::as_u64).unwrap_or(0) + 1;
        m.insert(x.kind.clone(), Value::from(n));
    }
    Value::Object(m)
}

/// A file name from a title: `fix-the-login.wisp-task`.
pub fn slug(title: &str) -> String {
    let mut s = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() {
            s.extend(c.to_lowercase());
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
        if s.len() >= 48 {
            break;
        }
    }
    let s = s.trim_end_matches('-');
    if s.is_empty() { "task".into() } else { s.to_owned() }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn secrets_env_paths_and_names_go() {
        let s = Scrub {
            env: secret_env([("MY_API_KEY".into(), "hunter2hunter2".into()), ("PATH".into(), "/usr/bin:/bin".into())]),
            project: "/Users/me/api".into(),
            home: "/Users/me".into(),
            tasks: vec!["01aa0000payroll0000000000000001".into()],
            ..Scrub::default()
        };
        let mut r = Vec::new();
        let v = json!({ "prompt": "use sk-ant-abcdefghijklmnopqrstuvwx and token=abcd1234efgh in /Users/me/api/src/a.ts, see ~/notes and /Users/me/x; key hunter2hunter2; like @[Payroll migration](wisp:01aa0000payroll0000000000000001)", "n": 3 });
        let got = scrub(&v, &s, "spec", &mut r);
        let p = got["prompt"].as_str().unwrap();
        assert_eq!(p, "use <secret> and <secret> in <project>/src/a.ts, see ~/notes and ~/x; key <env:MY_API_KEY>; like <task>");
        let kinds: Vec<&str> = r.iter().map(|x| x.kind.as_str()).collect();
        assert_eq!(kinds, ["secret", "secret", "env", "path", "path", "name"]);
        assert_eq!(r[0].was, "sk-a…uvwx", "the preview shows only its ends");
        assert_eq!(counts(&r), json!({ "secret": 2, "env": 1, "path": 2, "name": 1 }));
        let key = "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----";
        assert_eq!(scrub_text(key, &Scrub::default(), "x", &mut Vec::new()), "<secret>");
        assert_eq!(slug("Fix the login: 401s!"), "fix-the-login-401s");
    }

    /// QA round 12 #3: a task titled "login" made the shared prompt "fix the login bug" read "fix the <task> bug". Other
    /// tasks and crew members are taken out where they are referred to (a mention chip, an id), never as words.
    #[test]
    fn other_tasks_and_crew_go_where_they_are_named_never_as_words() {
        let login = "01ab0000000070008000000000000a11";
        let me = "01ab0000000070008000000000000b22";
        let ada = "crew0ada01";
        let s = Scrub { tasks: vec![login.into()], crew: vec![ada.into()], ..Scrub::default() };
        let mut r = Vec::new();
        // the QA example: the words stay, whatever other task is titled "login"
        assert_eq!(scrub_text("fix the login bug", &s, "report/answers/0/prompt", &mut r), "fix the login bug");
        assert!(r.is_empty(), "{r:?}");
        // a mention of that task, and its id, go; a mention of the shared task itself stays
        let t = format!("like @[login](wisp:{login}), see {login}, and @[this one](wisp:{me}); ask @[Ada](crew:{ada}) or Ada herself");
        let got = scrub_text(&t, &s, "spec/prompt", &mut r);
        assert_eq!(got, format!("like <task>, see <task>, and @[this one](wisp:{me}); ask <crew> or Ada herself"));
        let was: Vec<(&str, &str)> = r.iter().map(|x| (x.was.as_str(), x.now.as_str())).collect();
        assert_eq!(was, [("login", "<task>"), (login, "<task>"), ("Ada", "<crew>")], "the preview names what went");
        assert!(r.iter().all(|x| x.kind == "name"));
        // an id inside a longer word is someone else's text
        assert_eq!(scrub_text(&format!("x{login}y"), &s, "x", &mut Vec::new()), format!("x{login}y"));
    }

    #[test]
    fn the_signature_is_over_everything_but_itself() {
        let mut b = Bundle { id: "b1".into(), title: "t".into(), ..Bundle::default() };
        let one = canonical(&b);
        b.sig = "x".into();
        assert_eq!(canonical(&b), one);
        b.title = "u".into();
        assert_ne!(canonical(&b), one);
    }
}
