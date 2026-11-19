//! Rows ↔ the api's types. A session's indexed fields are columns; the rest of it is `body` (JSON), and
//! its list row is kept in `summary`, written with the row so a list read decodes one small column. The
//! queue lives in its own table and is filled in by the repo.

use serde_json::{Map, Value, json};
use wisp_runtime_api::model::{Activity, QueueItem, Session, Step, Summary, Turn};
use wisp_runtime_core::upcast;
use wisp_store::Row;

use crate::error::Result;

const SESSION_COLS: &[(&str, &str)] = &[
    ("id", "id"),
    ("parent_id", "parentId"),
    ("title", "title"),
    ("state", "state"),
    ("agent", "agent"),
    ("archived", "archived"),
    ("unread", "unread"),
    ("created_at", "createdAt"),
    ("updated_at", "updatedAt"),
    ("rev", "rev"),
];

fn object(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// `hide`: where the task's text quotes a .env value it read (`env_session::hide_of`), kept in `body` so a row sent
/// before those values are in memory again still masks (ADR-0050); never read back into the `Session`.
pub fn session_row(s: &Session, summary: &Summary, hide: Option<&Value>) -> Result<Row> {
    let mut body = object(serde_json::to_value(s)?);
    body.remove("queue");
    if let Some(h) = hide {
        body.insert(wisp_runtime_core::env_session::HIDE.into(), h.clone());
    }
    let mut row = Map::new();
    for (col, key) in SESSION_COLS {
        row.insert((*col).to_owned(), body.remove(*key).unwrap_or(Value::Null));
    }
    row.insert("body".into(), Value::Object(body));
    row.insert("summary".into(), serde_json::to_value(summary)?);
    Ok(row)
}

pub fn session_of(row: Row, queue: Vec<QueueItem>) -> Result<Session> {
    let mut row = row;
    let mut body = object(row.remove("body").unwrap_or(Value::Null));
    for (col, key) in SESSION_COLS {
        if let Some(v) = row.remove(*col).filter(|v| !v.is_null()) {
            body.insert((*key).to_owned(), v);
        }
    }
    body.insert("queue".into(), serde_json::to_value(queue)?);
    Ok(serde_json::from_value(Value::Object(body))?)
}

pub fn summary_of(row: &Row) -> Option<Summary> {
    row.get("summary").and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// The columns [`sessions_of`] reads beside `body` (no `summary`: a whole task never needs its list row).
pub const SESSION_READ: &[&str] =
    &["id", "parent_id", "title", "state", "agent", "archived", "unread", "created_at", "updated_at", "rev", "body"];

/// Every task in `query_json`'s text of [`SESSION_READ`], each parsed once straight from the text into a `Session`
/// (perf audit 2026-11-19 #4: each body was decoded into a `Value` tree, then into a `Session`, with its list row too:
/// 75–85 % of `runtime.history`'s 66 ms at 5,000 tasks). The columns go in front of the body's own fields, as
/// [`session_of`] puts them in; a row that doesn't read that way takes [`session_of`]'s path.
pub fn sessions_of(text: &str) -> Result<Vec<Session>> {
    use serde_json::value::RawValue;
    let rows: Vec<std::collections::HashMap<&str, &RawValue>> = serde_json::from_str(text)?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let body = r.get("body").map_or("{}", |b| b.get()).trim();
        let mut json = String::with_capacity(body.len() + 256);
        json.push_str("{\"queue\":[]");
        for (col, key) in SESSION_COLS {
            if let Some(v) = r.get(col).map(|v| v.get()).filter(|v| *v != "null") {
                json.push_str(",\"");
                json.push_str(key);
                json.push_str("\":");
                json.push_str(v);
            }
        }
        match body.strip_prefix('{').map(str::trim_start) {
            Some(rest) if rest.starts_with('}') => json.push('}'),
            Some(rest) => {
                json.push(',');
                json.push_str(rest);
            }
            None => json.push('}'),
        }
        if let Ok(s) = serde_json::from_str::<Session>(&json) {
            out.push(s);
        } else {
            // an older row (a column's field still in its body): read it field by field
            let row: Row = r.iter().map(|(k, v)| ((*k).to_owned(), serde_json::from_str(v.get()).unwrap_or(Value::Null))).collect();
            if let Ok(s) = session_of(row, Vec::new()) {
                out.push(s);
            }
        }
    }
    Ok(out)
}

/// The list rows in `query_json`'s text of `id, summary`, each summary parsed once straight from the text (ADR-0084:
/// no row maps and no copy of each, the startup's list fill ~3× faster on 5k tasks). A row whose summary is missing or
/// doesn't parse is skipped, as [`summary_of`] skips it.
pub fn summaries_of(text: &str) -> serde_json::Result<Vec<Summary>> {
    #[derive(serde::Deserialize)]
    struct Listed<'a> {
        #[serde(borrow)]
        summary: Option<&'a serde_json::value::RawValue>,
    }
    let rows: Vec<Listed<'_>> = serde_json::from_str(text)?;
    Ok(rows.iter().filter_map(|r| serde_json::from_str(r.summary?.get()).ok()).collect())
}

/// A turn's row; `known`: what its task read (`None`: that couldn't be read), so its body keeps where its prose quotes
/// one (`envHide`, ADR-0050). Only the row carries them: a `Turn` read back drops them.
pub fn turn_row(t: &Turn, known: Option<&[String]>) -> Result<Row> {
    let mut row = Map::new();
    row.insert("id".into(), json!(t.id));
    row.insert("session_id".into(), json!(t.session_id));
    row.insert("idx".into(), json!(t.idx));
    row.insert("status".into(), serde_json::to_value(t.status)?);
    row.insert("started_at".into(), json!(t.started_at));
    row.insert("ended_at".into(), json!(t.ended_at));
    row.insert("prompt".into(), json!(t.prompt));
    row.insert("answer".into(), json!(t.text));
    let mut body = serde_json::to_value(t)?;
    let hide = wisp_runtime_core::env_turn::hide_of(&body, known);
    if let Some(b) = body.as_object_mut() {
        b.insert(wisp_runtime_core::env_session::HIDE.into(), hide);
    }
    row.insert("body".into(), body);
    Ok(row)
}

pub fn turn_of(row: &Row) -> Result<Turn> {
    Ok(serde_json::from_value(row.get("body").cloned().unwrap_or(Value::Null))?)
}

pub fn queue_row(session: &str, pos: usize, q: &QueueItem) -> Result<Row> {
    let mut row = Map::new();
    row.insert("id".into(), json!(q.id));
    row.insert("session_id".into(), json!(session));
    row.insert("pos".into(), json!(pos));
    row.insert("body".into(), serde_json::to_value(q)?);
    Ok(row)
}

pub fn queue_of(row: &Row) -> Option<QueueItem> {
    row.get("body").and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// One step as an event-log row (shape `upcast::EVENT_V`).
pub fn event_row(session: &str, turn: &str, seq: u64, t: i64, a: &Activity) -> Result<Row> {
    let mut row = Map::new();
    row.insert("session_id".into(), json!(session));
    row.insert("seq".into(), json!(seq));
    row.insert("turn_id".into(), json!(turn));
    row.insert("t".into(), json!(t));
    row.insert("kind".into(), json!(a.kind()));
    row.insert("body".into(), serde_json::to_value(a)?);
    row.insert("v".into(), json!(upcast::EVENT_V));
    Ok(row)
}

/// A v1 activity item as it was (`v = 0`), for the importer: history is never rewritten.
pub fn v0_event_row(session: &str, turn: &str, seq: u64, t: i64, item: &Value) -> Row {
    let mut row = Map::new();
    row.insert("session_id".into(), json!(session));
    row.insert("seq".into(), json!(seq));
    row.insert("turn_id".into(), json!(turn));
    row.insert("t".into(), json!(t));
    row.insert("kind".into(), item.get("kind").cloned().unwrap_or_else(|| json!("text")));
    row.insert("body".into(), item.clone());
    row.insert("v".into(), json!(0));
    row
}

/// An event row as a step, through the upcasters.
pub fn step_of(row: &Row) -> Option<Step> {
    let v = row.get("v").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).unwrap_or(0);
    let activity = upcast::read(v, row.get("body")?)?;
    Some(Step { seq: row.get("seq").and_then(Value::as_u64)?, t: row.get("t").and_then(Value::as_i64).unwrap_or(0), activity })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod summaries_tests {
    use super::*;

    #[test]
    fn summaries_read_from_text_are_the_rows_summaries_and_skip_the_same_broken_ones() {
        let good = serde_json::to_value(Summary { title: "a \"quoted\" title".into(), turn_count: 3, ..Summary::default() }).unwrap();
        let text = format!(
            r#"[{{"id":"a","summary":{good}}},{{"id":"b","summary":null}},{{"id":"c","summary":{{"title":7}}}},{{"id":"d"}},{{"id":"e","summary":{good}}}]"#
        );
        let rows: Vec<Row> = serde_json::from_str(&text).unwrap();
        let by_row: Vec<Summary> = rows.iter().filter_map(summary_of).collect();
        assert_eq!(summaries_of(&text).unwrap(), by_row);
        assert_eq!(by_row.len(), 2, "the null, broken and missing summaries are skipped");
        assert_eq!(summaries_of("[]").unwrap(), Vec::<Summary>::new());
        assert!(summaries_of("not json").is_err());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod sessions_tests {
    use super::*;
    use wisp_runtime_api::model::SessionState;

    /// perf audit 2026-11-19 #4: a task read straight from the store's text is the task [`session_of`] reads, whatever
    /// its columns hold (an empty body, a null parent, an older row whose body still has a column's field).
    #[test]
    fn a_task_read_from_text_is_the_task_session_of_reads() {
        let mut s = Session { id: "t1".into(), title: "Fix login".into(), agent: "claude".into(), cwd: "/w".into(), ..Session::default() };
        s.state = SessionState::Review;
        s.archived = true;
        s.created_at = 5;
        s.updated_at = 9;
        s.rev = 3;
        s.resume_id = Some("conv-1".into());
        s.ext.insert("wisp.runtime".into(), json!({ "spec": { "prompt": "fix it" } }));
        let row = session_row(&s, &Summary::default(), None).unwrap();
        let pick =
            |r: &Row| Value::Object(SESSION_READ.iter().map(|c| ((*c).to_owned(), r.get(*c).cloned().unwrap_or(Value::Null))).collect());
        let mut older = row.clone();
        older.get_mut("body").unwrap().as_object_mut().unwrap().insert("title".into(), json!("Fix login"));
        let mut bare = Row::new();
        bare.insert("id".into(), json!("t2"));
        bare.insert("title".into(), json!("Bare"));
        bare.insert("body".into(), json!({ "agent": "codex", "model": "", "effort": "", "mode": "auto", "cwd": "", "state": "done", "createdAt": 1, "updatedAt": 1 }));
        let text = Value::Array(vec![pick(&row), pick(&older), pick(&bare)]).to_string();
        let got = sessions_of(&text).unwrap();
        assert_eq!(got.len(), 3, "{got:?}");
        assert_eq!(got[0], session_of(row, Vec::new()).unwrap());
        assert_eq!(got[1], got[0], "an older row reads the same");
        assert_eq!(got[2], session_of(bare, Vec::new()).unwrap());
    }
}
