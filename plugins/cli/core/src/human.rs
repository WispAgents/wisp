//! A command's result as text, for a generated verb without `--json`: a list of objects as aligned
//! rows of their short fields under a header of their names, an object as `key  value` lines, a scalar as it is.
//! Names read as words (`lastRunAt` → `last run`) and a time as how long ago (audit C #14: `uitrig off 0 trigger
//! 1795089973373`, no header; `turnId 01a1f3a1…`). Pure: the caller passes the time now.

use serde_json::Value;
use wisp_cli_api::Style;

use crate::term::{self, BOLD, DIM, paint};

/// The fields a row shows first when an object has them.
/// The lead fields that say which row it is.
const NAMING: &[&str] = &["id", "key", "name", "title", "label"];
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
        // now itself is a moment away (a check-in due this minute), never "just now" (audit E12)
        0..=44 => return (if later || d == 0 { "in a moment" } else { "just now" }).to_owned(),
        45..=3599 => format!("{}m", n.div_ceil(60).max(1)),
        3600..=86_399 => format!("{}h", n / 3600),
        _ => format!("{}d", n / 86_400),
    };
    if later { format!("in {span}") } else { format!("{span} ago") }
}

/// A time field left at 0: never set (audit H8: `connectors list` read "added 0"), so it says nothing.
fn unset_time(key: &str, v: &Value) -> bool {
    (matches!(key, "at" | "since" | "until") || key.ends_with("At") || key.ends_with("_at")) && v.as_i64() == Some(0)
}

fn cell(key: &str, v: &Value, now: Option<i64>) -> String {
    if unset_time(key, v) {
        return String::new();
    }
    match (now, v.as_i64()) {
        (Some(now), Some(ms)) if is_time(key, v) => ago(ms, now),
        _ => short(v),
    }
}

/// `n thing`/`n things`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A nested value in one line (a table's cell, a deep field) saying what's in it (audit D4: `4 fields`, `0 items`):
/// nothing is `none`, words joined, rows by what names them, a map as `key value` pairs; only a list of lists counted.
fn short(v: &Value) -> String {
    match v {
        Value::Array(a) if a.is_empty() => "none".to_owned(),
        Value::Array(a) if a.iter().all(|x| scalar(x).is_some()) => a.iter().filter_map(scalar).collect::<Vec<_>>().join(", "),
        Value::Array(a) => {
            let names: Option<Vec<String>> = a.iter().map(|x| x.as_object().and_then(|o| named(o).or_else(|| lead(o)))).collect();
            names.map_or_else(|| count(a.len(), "item", "items"), |n| n.join(", "))
        }
        Value::Object(o) if o.is_empty() => "none".to_owned(),
        Value::Object(o) => named(o).unwrap_or_else(|| {
            o.iter()
                .map(|(k, x)| match x {
                    Value::Object(_) => format!("{} ({})", words(k), short(x)),
                    _ => format!("{} {}", words(k), short(x)),
                })
                .collect::<Vec<_>>()
                .join(", ")
        }),
        other => scalar(other).unwrap_or_default(),
    }
}

/// A row's first lead field that's set (`name`, `key`, …): what names a row with no `id`.
fn lead(o: &serde_json::Map<String, Value>) -> Option<String> {
    LEAD.iter().find_map(|k| o.get(*k).and_then(scalar).filter(|v| !v.is_empty()))
}

/// The rows of a list under a field shown at most (audit D4: `history` had 27): the rest are counted.
const NESTED_ROWS: usize = 10;

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
    let has = |k: &str| list.iter().any(|x| x.get(k).is_some_and(|v| !unset_time(k, v) && scalar(v).is_some_and(|s| !s.is_empty())));
    // rows with nothing naming them (`channels status`' `transport`) lead with the first text field that differs on
    // every row, or no row says which it is (audit E5)
    if !NAMING.iter().any(|k| has(k))
        && let Some(Value::Object(first)) = list.first()
        && let Some(k) = first.keys().find(|k| {
            let vals: Vec<&str> = list.iter().filter_map(|x| x.get(k.as_str()).and_then(Value::as_str)).collect();
            !LEAD.contains(&k.as_str())
                && vals.len() == list.len()
                && vals.iter().all(|v| !v.is_empty())
                && vals.iter().collect::<std::collections::BTreeSet<_>>().len() == vals.len()
        })
    {
        cols.push(k.clone());
    }
    // a lead field holding a list or a map (`settings list`'s `value`) is a column too: its cells say what's in it (D4)
    let held = |k: &str| list.iter().any(|x| x.get(k).is_some_and(|v| v.is_array() || v.is_object()));
    for k in LEAD {
        if has(k) || held(k) {
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
            // the last column takes what is left; an id is never cut: it's what the next command takes, as printed
            // (audit H7: `later plans` cut a plan's id to 21 characters, and `later cancel` of that did nothing)
            let cap = |i: usize, w: usize| if cols[i] == "id" { w } else { w.min(room / n + 8) };
            let used: usize = widths.iter().take(n.saturating_sub(1)).enumerate().map(|(i, w)| cap(i, *w)).sum();
            for (i, w) in widths.iter_mut().enumerate() {
                *w = if i + 1 == n { room.saturating_sub(used).max(4) } else { cap(i, *w) };
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
    // a field with nothing in it says nothing (audit D8: `wisp crew get` listed `avatar`, `brief`, `machine`, `away
    // none`, `offers none`…); a lead field (`value`, `name`…) stays, so an unset setting still reads as one
    let empty = |x: &Value| match x {
        Value::Null => true,
        Value::String(t) => t.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(m) => m.is_empty(),
        _ => false,
    };
    let shown = |k: &str, x: &Value| LEAD.contains(&k) || (!empty(x) && !unset_time(k, x));
    let kw = o.iter().filter(|(k, x)| shown(k, x)).map(|(k, _)| term::width(&words(k))).max().unwrap_or(kw);
    o.iter()
        .filter(|(k, x)| shown(k, x))
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
                // a list of rows (`wisp connectors list`'s `connectors`): its rows under its name (audit D4)
                Value::Array(a) if a.iter().all(Value::is_object) && depth < 2 => {
                    let inner = Style { width: width.saturating_sub(2), ..*s };
                    let rows = render(&Value::Array(a.iter().take(NESTED_ROWS).cloned().collect()), &inner, now);
                    let pad = format!("{indent}  ");
                    let mut out = vec![format!("{indent}{}", paint(s, BOLD, &words(k)))];
                    out.extend(rows.into_iter().map(|l| format!("{pad}{l}")));
                    if a.len() > NESTED_ROWS {
                        out.push(format!("{pad}… {} more", a.len() - NESTED_ROWS));
                    }
                    return out;
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

    /// Audit D8: `wisp crew create|get|update` printed every field, the empty ones too; they're left out, a lead field
    /// that's empty stays, and a member's team shows its rows.
    #[test]
    fn an_objects_empty_fields_are_left_out_audit_d8() {
        let s = Style::plain(100);
        let m = json!({ "id": "ada", "name": "Ada", "avatar": "", "brief": "", "machine": null, "away": {}, "offers": [], "paused": false,
            "team": [{ "crew": "bob", "why": "reviews" }] });
        let got = render(&m, &s, None).join("\n");
        for gone in ["avatar", "brief", "machine", "away", "offers"] {
            assert!(!got.contains(gone), "{gone}: {got}");
        }
        assert!(got.contains("paused") && got.contains("bob") && !got.contains("1 item"), "{got}");
        let unset = render(&json!({ "key": "theme", "value": "" }), &s, None);
        assert_eq!(unset.iter().map(|l| l.trim_end()).collect::<Vec<_>>(), ["key    theme", "value"], "an unset value still says so");
    }

    /// Audit H7: `wisp later plans` cut a plan's id to 21 characters ("01a1f95a22ac7768acff4…"), and the id as
    /// printed named nothing. An id column is printed whole, however narrow the screen.
    #[test]
    fn an_id_is_printed_whole_audit_h7() {
        let s = Style::plain(60);
        let id = "01a1f95a22ac7768acff4f2c9e8d7b6a";
        let rows = json!([{ "id": id, "kind": "task", "title": "a long title that takes the rest of the line and more", "state": "set", "at": "18:00" }]);
        let got = render(&rows, &s, None);
        assert!(got[1].starts_with(id), "{got:?}");
    }

    /// Audit H8: `wisp connectors list` read "added 0" for a connector added that minute: a time never set says nothing.
    #[test]
    fn a_time_never_set_says_nothing_audit_h8() {
        let s = Style::plain(80);
        let rows = render(&json!([{ "id": "docs", "name": "docs", "addedAt": 0 }]), &s, Some(1_795_090_000_000));
        assert!(rows.iter().all(|l| !l.contains("added") && !l.ends_with(" 0")), "{rows:?}");
        let one = render(&json!({ "name": "docs", "addedAt": 0 }), &s, Some(1_795_090_000_000));
        assert_eq!(one, ["name  docs"]);
    }

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
        // audit E12: a check-in due now is "in a moment", never "just now" or "1m ago"
        assert_eq!((ago(now, now), ago(now - 20_000, now)), ("in a moment".into(), "just now".into()));
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
        assert_eq!(got, ["efforts", "  claude  high", "models", "  codex  gpt-5.5", "rows", "  x", "  1", "tags     a, b"]);
        // a list's time columns, whatever their name says (`since`, `until`, `addedAt`)
        let peers = json!([{ "id": "a030bd", "name": "qabox", "state": "error", "since": now - 2 * 3_600_000, "address": "qa@192.0.2.1" }]);
        let got = render(&peers, &s, Some(now));
        assert!(got[1].contains("2h ago") && !got[1].contains("17950"), "{got:?}");
        assert_eq!((short(&json!({ "a": 1 })), short(&json!([1]))), ("a 1".to_owned(), "1".to_owned()));
    }

    /// Audit E5 (D4's last part): `wisp channels status` printed `state  enabled  installed` and seven rows of `off
    /// false  true`, none saying which channel. Rows with no naming field (id, key, name, title, label) lead with
    /// the first text field that differs on every row (`transport`), at any width.
    #[test]
    fn rows_with_no_name_lead_with_their_first_field_audit_e5() {
        let rows = json!({ "cloud": false, "channels": [
            { "transport": "ntfy", "installed": true, "enabled": false, "state": "off", "saved": [], "linked": 0 },
            { "transport": "telegram", "installed": true, "enabled": true, "state": "failing", "saved": ["token"], "linked": 0 },
        ] });
        for width in [60, 100] {
            let got = render(&rows, &Style::plain(width), None);
            let ch = got.iter().position(|l| l == "channels").unwrap_or(usize::MAX);
            assert!(got.get(ch + 1).is_some_and(|l| l.trim_start().starts_with("transport")), "{width}: {got:?}");
            assert!(got.get(ch + 2).is_some_and(|l| l.trim_start().starts_with("ntfy")), "{width}: {got:?}");
            assert!(got.get(ch + 3).is_some_and(|l| l.trim_start().starts_with("telegram") && l.contains("failing")), "{width}: {got:?}");
        }
        // a row that names itself keeps its columns as they were
        let named = render(&json!([{ "kind": "x", "name": "a", "state": "on" }]), &Style::plain(100), None);
        assert!(named[0].starts_with("name") && named[0].contains("state"), "{named:?}");
    }

    /// Audit D4 (C4 partly fixed): `wisp connectors list` printed `connectors  3 items`, `channels status` `channels  7
    /// items`, `worktrees view` `items  6 items`, `machine read` `history  27 items`, and `settings list` `runtime.launch
    /// 4 fields`, `events.sinks  0 items`. A list of rows prints as rows under its name (the first 10, then how many
    /// more); a cell holding a list or a map says what's in it.
    #[test]
    fn a_list_of_rows_prints_its_rows_and_a_cell_says_whats_in_it_audit_d4() {
        let s = Style::plain(100);
        let conns = json!({ "connectors": [{ "name": "qa_echo", "state": "ready", "tools": 1 }, { "name": "pasted", "state": "off", "tools": 0 }] });
        let got = render(&conns, &s, None);
        assert!(!got.iter().any(|l| l.contains("items")), "{got:?}");
        assert_eq!(got[0], "connectors");
        assert!(got[1].starts_with("  name") && got[1].contains("state"), "a header: {got:?}");
        assert!(got[2].starts_with("  qa_echo") && got[3].starts_with("  pasted"), "{got:?}");
        let history: Vec<Value> = (0..27).map(|i| json!({ "at": i, "cpu": i })).collect();
        let got = render(&json!({ "history": history }), &s, None);
        assert_eq!(got.len(), 1 + 1 + 10 + 1, "name, header, 10 rows, the rest counted: {got:?}");
        assert_eq!(got.last().map(String::as_str), Some("  … 17 more"));
        let settings = json!([
            { "key": "runtime.launch", "value": { "harness": "claude", "models": { "claude": "opus" } } },
            { "key": "events.sinks", "value": [] },
            { "key": "learn.harnesses", "value": ["claude", "codex"] },
        ]);
        let got = render(&settings, &s, None);
        assert!(!got.iter().any(|l| l.contains("fields") || l.contains("items")), "{got:?}");
        assert!(got[1].contains("harness claude, models (claude opus)"), "{got:?}");
        assert!(got[2].contains("none") && got[3].contains("claude, codex"), "{got:?}");
        assert_eq!(short(&json!([{ "name": "a" }, { "name": "b" }])), "a, b", "rows named by their lead field");
        assert_eq!(short(&json!([[1], [2]])), "2 items", "anything else counted");
    }
}
