//! A command's result as text, for a generated verb without `--json`: a list of objects as aligned
//! rows of their short fields under a header of their names, an object as `key  value` lines, a scalar as it is.
//! Names read as words (`lastRunAt` → `last run`) and a time as how long ago (audit C #14: `uitrig off 0 trigger
//! 1795089973373`, no header; `turnId 01a1f3a1…`). Pure: the caller passes the time now.

use serde_json::Value;
use wisp_cli_api::Style;

use crate::term::{self, BOLD, DIM, paint};

/// The fields a row shows first when an object has them.
const LEAD: &[&str] = &["id", "key", "name", "title", "label", "state", "status", "value", "agent", "path", "url"];

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some(String::new()),
        _ => None,
    }
}

/// `turnId` → `turn`, `lastRunAt` → `last run`, `name` stays: a field's name as words.
pub fn words(key: &str) -> String {
    let w = wisp_kernel::decl::kebab(key).replace('-', " ");
    for tail in [" id", " at", " ms"] {
        if let Some(head) = w.strip_suffix(tail).filter(|h| !h.is_empty()) {
            return head.to_owned();
        }
    }
    w
}

/// A field holding a time: `at`, `…At`, `…_at`, `since`, `until` (ms since the epoch).
fn is_time(key: &str, v: &Value) -> bool {
    (matches!(key, "at" | "since" | "until") || key.ends_with("At") || key.ends_with("_at"))
        && v.as_i64().is_some_and(|n| n > 100_000_000_000)
}

/// How long ago `ms` was (or how soon it is), from `now`.
pub fn ago(ms: i64, now: i64) -> String {
    let d = now - ms;
    let (n, later) = (d.unsigned_abs() / 1000, d < 0);
    let span = match n {
        0..=44 => return (if later { "in a moment" } else { "just now" }).to_owned(),
        45..=3599 => format!("{}m", n.div_ceil(60).max(1)),
        3600..=86_399 => format!("{}h", n / 3600),
        _ => format!("{}d", n / 86_400),
    };
    if later { format!("in {span}") } else { format!("{span} ago") }
}

fn cell(key: &str, v: &Value, now: Option<i64>) -> String {
    match (now, v.as_i64()) {
        (Some(now), Some(ms)) if is_time(key, v) => ago(ms, now),
        _ => short(v),
    }
}

/// `n thing`/`n things`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn short(v: &Value) -> String {
    match v {
        Value::Array(a) => count(a.len(), "item", "items"),
        Value::Object(o) => count(o.len(), "field", "fields"),
        other => scalar(other).unwrap_or_default(),
    }
}

/// What names a nested row in one line (one with an `id`: a task, a member): its lead fields that are set
/// (`0190… · Ada · Job · running`). Anything else shows its fields (audit C4).
fn named(o: &serde_json::Map<String, Value>) -> Option<String> {
    o.get("id")?;
    let parts: Vec<String> =
        ["id", "name", "title", "state", "status"].iter().filter_map(|k| o.get(*k).and_then(scalar).filter(|v| !v.is_empty())).collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The columns of a list of objects: the lead fields they have, then other scalars, up to `max`.
fn columns(list: &[Value], max: usize) -> Vec<String> {
    let mut cols: Vec<String> = Vec::new();
    let has = |k: &str| list.iter().any(|x| x.get(k).is_some_and(|v| scalar(v).is_some_and(|s| !s.is_empty())));
    for k in LEAD {
        if has(k) {
            cols.push((*k).to_owned());
        }
    }
    if let Some(Value::Object(first)) = list.first() {
        for (k, v) in first {
            if !cols.contains(k) && scalar(v).is_some() && has(k) {
                cols.push(k.clone());
            }
        }
    }
    cols.truncate(max);
    cols
}

/// `v` as lines; `now` (ms) turns times into how long ago (`None`: as they are).
pub fn render(v: &Value, s: &Style, now: Option<i64>) -> Vec<String> {
    match v {
        Value::Array(list) if list.is_empty() => vec![paint(s, DIM, "(none)")],
        Value::Array(list) if list.iter().all(Value::is_object) => {
            let cols = columns(list, if s.width >= 80 { 5 } else { 3 });
            let head: Vec<String> = cols.iter().map(|c| words(c)).collect();
            let cells: Vec<Vec<String>> = std::iter::once(head)
                .chain(list.iter().map(|x| cols.iter().map(|c| x.get(c).map(|v| cell(c, v, now)).unwrap_or_default()).collect()))
                .collect();
            let n = cols.len().max(1);
            let room = s.width.saturating_sub(2 * (n - 1));
            let mut widths: Vec<usize> = (0..cols.len()).map(|i| cells.iter().map(|r| term::width(&r[i])).max().unwrap_or(0)).collect();
            // the last column takes what is left
            let used: usize = widths.iter().take(n.saturating_sub(1)).map(|w| (*w).min(room / n + 8)).sum();
            for (i, w) in widths.iter_mut().enumerate() {
                *w = if i + 1 == n { room.saturating_sub(used).max(4) } else { (*w).min(room / n + 8) };
            }
            cells
                .iter()
                .enumerate()
                .map(|(row, r)| {
                    let parts: Vec<String> =
                        r.iter()
                            .enumerate()
                            .map(|(i, c)| {
                                if i + 1 == n { term::truncate(c, widths[i]) } else { term::pad(&term::truncate(c, widths[i]), widths[i]) }
                            })
                            .collect();
                    let line = parts.join("  ").trim_end().to_owned();
                    if row == 0 { paint(s, DIM, &line) } else { line }
                })
                .collect()
        }
        Value::Array(list) => list.iter().map(|x| term::truncate(&short(x), s.width)).collect(),
        Value::Object(o) if o.is_empty() => vec![paint(s, DIM, "✓ done")],
        Value::Object(o) => fields(o, s, now, 0),
        other => term::wrap(&scalar(other).unwrap_or_default(), s.width),
    }
}

/// An object's fields as `key  value` lines at `depth` (two spaces each), names aligned: a nested object (not a named
/// row) as its name then its own fields under it, a list of words joined, any other list counted (audit C4: `ssh
/// {3 fields}`).
fn fields(o: &serde_json::Map<String, Value>, s: &Style, now: Option<i64>, depth: usize) -> Vec<String> {
    let indent = "  ".repeat(depth);
    let kw = o.keys().map(|k| term::width(&words(k))).max().unwrap_or(0).min(24);
    let width = s.width.saturating_sub(indent.len());
    o.iter()
        .flat_map(|(k, x)| {
            let val = match x {
                Value::Number(_) if now.is_some() && is_time(k, x) => cell(k, x, now),
                Value::String(t) => t.clone(),
                // a nested row (`wisp crew assign`'s `session`): what names it, never its raw JSON (audit B #17)
                Value::Object(inner) if inner.is_empty() => "none".to_owned(),
                Value::Object(inner) => match named(inner) {
                    Some(n) => n,
                    None if depth < 2 => {
                        let mut out = vec![format!("{indent}{}", paint(s, BOLD, &words(k)))];
                        out.extend(fields(inner, s, now, depth + 1));
                        return out;
                    }
                    None => short(x),
                },
                Value::Array(a) if a.is_empty() => "none".to_owned(),
                Value::Array(a) if a.iter().all(|v| !v.is_object() && !v.is_array()) => {
                    a.iter().filter_map(scalar).collect::<Vec<_>>().join(", ")
                }
                other => scalar(other).unwrap_or_else(|| short(other)),
            };
            term::hang(&format!("{indent}{}  ", paint(s, BOLD, &term::pad(&words(k), kw))), &val, width + indent.len())
                .into_iter()
                .take(6)
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn lists_objects_and_scalars() {
        let s = Style::plain(60);
        let got = render(&json!([{ "id": "a1", "name": "Ada", "deep": { "x": 1 } }, { "id": "b22", "name": "Grace" }]), &s, None);
        assert_eq!(got, ["id   name", "a1   Ada", "b22  Grace"]);
        assert_eq!(render(&json!({ "key": "theme", "value": "dark" }), &s, None), ["key    theme", "value  dark"]);
        assert_eq!(render(&json!("ok"), &s, None), ["ok"]);
        assert_eq!(render(&json!([]), &s, None), ["(none)"]);
    }

    /// Audit B #17: `wisp crew assign` printed `session  {"agent":"claude",…}`; a nested row is named, not dumped.
    #[test]
    fn a_nested_object_is_named_by_its_lead_fields() {
        let s = Style::plain(80);
        let started =
            json!({ "session": { "id": "0190aa", "agent": "claude", "title": "Ada · Job", "state": "running", "ext": { "x": 1 } } });
        assert_eq!(render(&started, &s, None), ["session  0190aa · Ada · Job · running"]);
        assert_eq!(render(&json!({ "deep": { "x": 1, "y": 2 } }), &s, None), ["deep", "  x  1", "  y  2"]);
    }

    /// Audit C #14: `wisp automations trigger ls` printed `uitrig off 0 trigger 1795089973373` (epoch ms, no header)
    /// and `queue send-now` `turnId 01a1f3a1…`. Rows come under their names in words, times as how long ago.
    #[test]
    fn rows_have_a_header_names_read_as_words_and_times_as_how_long_ago_audit_c_14() {
        let s = Style::plain(100);
        let now = 1_795_090_000_000;
        let rows = json!([{ "name": "uitrig", "state": "off", "runs": 0, "from": "trigger", "lastRunAt": 1_795_089_973_373_i64 }]);
        let got = render(&rows, &s, Some(now));
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(got[0].starts_with("name") && got[0].contains("state") && got[0].contains("last run"), "a header: {got:?}");
        assert!(got[1].contains("just now") && !got[1].contains("1795089973373"), "{got:?}");
        assert_eq!(render(&json!({ "turnId": "01a1f3a1" }), &s, Some(now)), ["turn  01a1f3a1"]);
        assert_eq!(render(&json!({ "at": now - 3 * 3_600_000 }), &s, Some(now)), ["at  3h ago"]);
        assert_eq!(
            (words("lastRunAt"), words("turnId"), words("name"), words("id")),
            ("last run".into(), "turn".into(), "name".into(), "id".into())
        );
        assert_eq!(
            (ago(now - 90_000, now), ago(now + 120_000, now), ago(now - 2 * 86_400_000, now)),
            ("2m ago".into(), "in 2m".into(), "2d ago".into())
        );
    }

    /// Audit C4: `wisp machines reachable` printed `ssh  {3 fields}` (its banner, port and the tailnet name hidden),
    /// `wisp settings get runtime.launch` `efforts  {1 fields}`, and `wisp machines list` a raw epoch-ms `since`.
    /// A nested object's fields print as indented lines, a list of words joined, counts in English, times in words.
    #[test]
    fn nested_values_print_as_indented_lines_and_every_time_as_words_audit_c4() {
        let s = Style::plain(100);
        let now = 1_795_090_000_000_i64;
        let reach = json!({
            "hostname": "qabox",
            "ssh": { "running": true, "banner": "OpenSSH_9.8", "port": 22 },
            "tailscale": { "installed": true, "running": true, "name": "qabox.tail1.ts.net", "ip": "100.64.0.7" },
            "target": "qa@qabox.tail1.ts.net",
        });
        let got = render(&reach, &s, Some(now));
        assert_eq!(
            got,
            [
                "hostname   qabox",
                "ssh",
                "  banner   OpenSSH_9.8",
                "  port     22",
                "  running  true",
                "tailscale",
                "  installed  true",
                "  ip         100.64.0.7",
                "  name       qabox.tail1.ts.net",
                "  running    true",
                "target     qa@qabox.tail1.ts.net",
            ]
        );
        let launch = json!({ "efforts": { "claude": "high" }, "models": { "codex": "gpt-5.5" }, "tags": ["a", "b"], "rows": [{ "x": 1 }] });
        let got = render(&launch, &s, Some(now));
        assert!(!got.iter().any(|l| l.contains("fields")), "{got:?}");
        assert_eq!(got, ["efforts", "  claude  high", "models", "  codex  gpt-5.5", "rows     1 item", "tags     a, b"]);
        // a list's time columns, whatever their name says (`since`, `until`, `addedAt`)
        let peers = json!([{ "id": "a030bd", "name": "qabox", "state": "error", "since": now - 2 * 3_600_000, "address": "qa@192.0.2.1" }]);
        let got = render(&peers, &s, Some(now));
        assert!(got[1].contains("2h ago") && !got[1].contains("17950"), "{got:?}");
        assert_eq!((short(&json!({ "a": 1 })), short(&json!([1]))), ("1 field".to_owned(), "1 item".to_owned()));
    }
}
