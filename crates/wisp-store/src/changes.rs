//! The change feed: what a client missed since its cursor, filtered by the topics it watches. A change's
//! first topic is on the row; the others are in `change_topic`, so one write on two topics is still one
//! change, delivered once.

use std::collections::HashSet;

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::entity::{Id, Kind, Row, Topic};
use crate::error::{Result, StoreError, invalid};
use crate::reads::get_row;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeOp {
    Upsert,
    Patch,
    Del,
    Append,
}

impl ChangeOp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Patch => "patch",
            Self::Del => "del",
            Self::Append => "append",
        }
    }
    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "upsert" => Self::Upsert,
            "patch" => Self::Patch,
            "del" => Self::Del,
            "append" => Self::Append,
            other => return Err(StoreError::Internal(format!("unknown change op {other}"))),
        })
    }
}

/// One committed change. `patch` is what sync puts on the wire: the full row for `upsert` (a row that
/// did not exist), RFC 6902 ops for `patch`, null for `del`, the new rows for `append`. The feed stores an append's
/// keys only (ADR-0085): read back, its rows are as they are now in their table (a later patch already in them), a
/// row gone since left out. Live (`on_commit`) an append carries its rows as they were written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeRow {
    pub seq: i64,
    pub topic: Topic,
    /// every topic, `topic` first, the rest sorted
    pub topics: Vec<Topic>,
    pub entity: String,
    /// the row's key; for `append`, the parent the batch is grouped under
    pub id: String,
    pub op: ChangeOp,
    pub patch: Value,
    pub at: i64,
}

/// The rows on a topic that changed after a cursor, as they are now, for a sync delta.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    /// (kind, current row), in the order their last change was made
    pub rows: Vec<(String, Row)>,
    /// (kind, id) of rows that changed and no longer exist
    pub deleted: Vec<(String, String)>,
    /// the feed's head when this was read: the client's next cursor
    pub head: i64,
}

pub const MAX_LIMIT: u32 = 10_000;
const COLS: &str = "seq, topic, more, entity, id, op, patch, at";

/// The newest seq: the rowid's max (the b-tree's last cell, no scan). Retention keeps the newest row, so it never
/// goes down and SQLite's next rowid (max + 1) is never one a cursor has seen (ADR-0085).
pub(crate) fn head_seq(db: &Connection) -> Result<i64> {
    Ok(db.prepare_cached("SELECT coalesce(max(seq), 0) FROM change")?.query_row([], |r| r.get(0))?)
}

pub(crate) fn oldest_seq(db: &Connection) -> Result<i64> {
    let min: Option<i64> = db.prepare_cached("SELECT min(seq) FROM change")?.query_row([], |r| r.get(0))?;
    match min {
        Some(s) => Ok(s),
        None => Ok(head_seq(db)? + 1),
    }
}

/// Whether a client that has seen everything up to `cursor` can resume from the feed.
pub(crate) fn can_resume(db: &Connection, cursor: i64) -> Result<bool> {
    Ok(cursor >= 0 && cursor <= head_seq(db)? && cursor >= oldest_seq(db)? - 1)
}

pub(crate) fn read(db: &Connection, since: i64, topics: Option<&[&str]>, limit: u32) -> Result<Vec<ChangeRow>> {
    if since < 0 {
        return Err(invalid("changes_since: seq must be a whole number ≥ 0"));
    }
    if limit == 0 || limit > MAX_LIMIT {
        return Err(invalid(format!("changes_since: limit must be 1…{MAX_LIMIT}")));
    }
    let seqs = match topics {
        Some([]) => return Ok(Vec::new()),
        None => None,
        Some(ts) => Some(topic_seqs(db, since, ts, limit)?),
    };
    let mut out = Vec::new();
    match seqs {
        None => {
            let mut st = db.prepare_cached(&format!("SELECT {COLS} FROM change WHERE seq > ? ORDER BY seq LIMIT ?"))?;
            let mut rows = st.query([since, i64::from(limit)])?;
            while let Some(r) = rows.next()? {
                out.push(decode(r)?);
            }
        }
        Some(seqs) => {
            let mut st = db.prepare_cached(&format!("SELECT {COLS} FROM change WHERE seq = ?"))?;
            for seq in seqs {
                let mut rows = st.query([seq])?;
                if let Some(r) = rows.next()? {
                    out.push(decode(r)?);
                }
            }
        }
    }
    Ok(out)
}

/// [`read`], each append's rows read back from their table (the feed keeps their keys only, ADR-0085). A row
/// deleted since is left out; a kind that isn't loaded keeps the keys.
pub(crate) fn read_whole(
    db: &Connection,
    kinds: &dyn Fn(&str) -> Option<std::sync::Arc<Kind>>,
    since: i64,
    topics: Option<&[&str]>,
    limit: u32,
) -> Result<Vec<ChangeRow>> {
    let mut out = read(db, since, topics, limit)?;
    for c in out.iter_mut().filter(|c| c.op == ChangeOp::Append) {
        let Some(k) = kinds(&c.entity) else { continue };
        let keys = c.patch.as_array().map(Vec::as_slice).unwrap_or_default();
        let mut rows = Vec::with_capacity(keys.len());
        for key in keys.iter().filter_map(Value::as_object) {
            if let Some(row) = get_row(db, &k, &k.id_value(key)?)? {
                rows.push(Value::Object(row));
            }
        }
        c.patch = Value::Array(rows);
    }
    Ok(out)
}

/// The first `limit` seqs after `since` on any of `topics`: one index range per topic (each already in
/// seq order and cut at `limit`), merged. A change on two watched topics counts once.
fn topic_seqs(db: &Connection, since: i64, topics: &[&str], limit: u32) -> Result<Vec<i64>> {
    let mut st = db.prepare_cached("SELECT seq FROM change_topic WHERE topic = ? AND seq > ? ORDER BY seq LIMIT ?")?;
    let mut all = Vec::new();
    for t in topics {
        let mut rows = st.query(params![t, since, i64::from(limit)])?;
        while let Some(r) = rows.next()? {
            all.push(r.get::<_, i64>(0)?);
        }
    }
    all.sort_unstable();
    all.dedup();
    all.truncate(limit as usize);
    Ok(all)
}

fn decode(r: &rusqlite::Row<'_>) -> Result<ChangeRow> {
    let topic: String = r.get(1)?;
    let more: Option<String> = r.get(2)?;
    let patch: Option<String> = r.get(6)?;
    let op: String = r.get(5)?;
    let mut topics = vec![topic.clone()];
    topics.extend(more.iter().flat_map(|m| m.split(' ')).map(str::to_owned));
    Ok(ChangeRow {
        seq: r.get(0)?,
        topic,
        topics,
        entity: r.get(3)?,
        id: r.get(4)?,
        op: ChangeOp::parse(&op)?,
        patch: patch.map_or(Ok(Value::Null), |p| serde_json::from_str(&p))?,
        at: r.get(7)?,
    })
}

/// The rows on `topic` changed after `since`, as they are now; `None` when `since` is older than what
/// retention kept (the client must take a snapshot instead). One read transaction: callers pass a reader.
pub(crate) fn changed_since(
    db: &Connection,
    kinds: &dyn Fn(&str) -> Option<std::sync::Arc<Kind>>,
    topic: &str,
    since: i64,
) -> Result<Option<Delta>> {
    if !can_resume(db, since)? {
        return Ok(None);
    }
    let head = head_seq(db)?;
    // (kind, id) in the order of their latest change; appends name their rows in the patch.
    // Collect pages oldest→newest, then walk newest→oldest across ALL pages so a multi-page
    // range (> MAX_LIMIT changes) deduplicates by each row's truly newest change and delivers oldest first.
    let mut pages: Vec<Vec<ChangeRow>> = Vec::new();
    let mut cursor = since;
    loop {
        let page = read_keys(db, cursor, topic, MAX_LIMIT)?;
        let Some(last) = page.last() else { break };
        cursor = last.seq;
        let more = page.len() == MAX_LIMIT as usize;
        pages.push(page);
        if !more {
            break;
        }
    }
    let mut order: Vec<(String, Id)> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for page in pages.iter().rev() {
        for c in page.iter().rev() {
            for id in touched(c, kinds)? {
                if seen.insert((c.entity.clone(), id.to_string())) {
                    order.push((c.entity.clone(), id));
                }
            }
        }
    }
    order.reverse(); // collected newest first; deliver oldest first, each at its latest change
    let mut delta = Delta { head, ..Delta::default() };
    for (entity, id) in order {
        let Some(k) = kinds(&entity) else { continue }; // an owner that is not loaded: skip its rows
        match get_row(db, &k, &id)? {
            Some(row) => delta.rows.push((entity, row)),
            None => delta.deleted.push((entity, id.to_string())),
        }
    }
    Ok(Some(delta))
}

/// [`read`] of one topic for [`changed_since`], which needs only which rows changed: an append's patch (its rows' keys)
/// is read, every other change's is not. A first look at a topic with no snapshot source replays its whole feed here,
/// and a plugin that upserts a 30 KB row every 10 s (This Mac's snapshot) left 21k such patches on its topic: parsing
/// them all held the page on "Reading…" for 10–12 s.
fn read_keys(db: &Connection, since: i64, topic: &str, limit: u32) -> Result<Vec<ChangeRow>> {
    let mut st = db.prepare_cached(
        "SELECT c.seq, c.entity, c.id, c.op, CASE WHEN c.op = 'append' THEN c.patch END, c.at
         FROM change_topic t JOIN change c ON c.seq = t.seq WHERE t.topic = ? AND t.seq > ? ORDER BY t.seq LIMIT ?",
    )?;
    let mut rows = st.query(params![topic, since, i64::from(limit)])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let op: String = r.get(3)?;
        let patch: Option<String> = r.get(4)?;
        out.push(ChangeRow {
            seq: r.get(0)?,
            topic: topic.to_owned(),
            topics: vec![topic.to_owned()],
            entity: r.get(1)?,
            id: r.get(2)?,
            op: ChangeOp::parse(&op)?,
            patch: patch.map_or(Ok(Value::Null), |p| serde_json::from_str(&p))?,
            at: r.get(5)?,
        });
    }
    Ok(out)
}

fn touched(c: &ChangeRow, kinds: &dyn Fn(&str) -> Option<std::sync::Arc<Kind>>) -> Result<Vec<Id>> {
    if c.op != ChangeOp::Append {
        return Ok(vec![Id::Text(c.id.clone())]);
    }
    let Some(k) = kinds(&c.entity) else {
        return Ok(Vec::new());
    };
    let rows = c.patch.as_array().map(Vec::as_slice).unwrap_or_default();
    let mut ids = rows.iter().rev().filter_map(|r| r.as_object()).map(|r| k.id_value(r)).collect::<Result<Vec<_>>>()?;
    ids.dedup();
    Ok(ids)
}
