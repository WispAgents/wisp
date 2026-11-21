//! A task's Notes in the store: read them whole, rewrite one section (a revision kept for each version, who wrote
//! it), mark what the agent has read. One write path: each change is one store transaction.

use serde_json::{Value, json};
use wisp_store::{Query, Store};
use wisp_threads_api::{By, Notes, Revision, Section};
use wisp_threads_core::notes::{META, Shown, clean, markdown, meta, unknown};

use crate::store::{self, Meta};

fn by_of(v: Option<&Value>) -> By {
    if v.and_then(Value::as_str) == Some("you") { By::You } else { By::Agent }
}

fn by_str(b: By) -> &'static str {
    match b {
        By::You => "you",
        By::Agent => "agent",
    }
}

fn section_of(r: &serde_json::Map<String, Value>) -> Section {
    let s = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let id = s("section");
    Section {
        label: meta(&id).map(|m| m.label.to_owned()).unwrap_or_default(),
        text: s("text"),
        rev: r.get("rev").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).unwrap_or(0),
        by: by_of(r.get("by")),
        at: r.get("at").and_then(Value::as_i64).unwrap_or(0),
        id,
    }
}

/// The Notes and their meta.
pub async fn load(st: &Store, task: &str) -> wisp_store::Result<(Notes, Meta)> {
    let t = task.to_owned();
    st.read_async(move |r| {
        let rows = r.query(store::NOTES.kind, &Query::new().eq("task_id", t.as_str()))?;
        let m = Meta::of(r.get(store::META.kind, t.clone())?.as_ref());
        let sections = META
            .iter()
            .map(|x| {
                rows.iter().map(section_of).find(|s| s.id == x.id).unwrap_or_else(|| Section {
                    id: x.id.into(),
                    label: x.label.into(),
                    ..Section::default()
                })
            })
            .collect();
        Ok((Notes { task: t, sections, rev: m.rev, seen: m.seen, lead: None }, m))
    })
    .await
}

/// The Notes as markdown (empty sections left out), at most `max` characters.
pub fn md(n: &Notes, only: &[&str], max: usize, tool: &str) -> String {
    let shown: Vec<Shown<'_>> = n.sections.iter().map(|s| Shown { id: &s.id, text: &s.text }).collect();
    markdown(&shown, only, max, tool)
}

/// You changed the Notes since the agent last read them (it reads them whole with each note and each write of its own,
/// so a write of yours after that is unseen until the next turn). v1 kept one `by` for the whole ledger, so an agent's
/// write after yours hid that you had changed something (docs/v1-bugs-fixed.md).
pub fn yours_unseen(n: &Notes) -> bool {
    n.rev > n.seen && n.sections.iter().any(|s| s.by == By::You && s.rev > 0)
}

/// What a rewrite did.
#[derive(Debug, Clone, Default)]
pub struct Wrote {
    pub notes: Notes,
    pub changed: bool,
}

/// Rewrites one section (`text` as it should be from now on; '' clears it). Unchanged text is no new revision.
/// `turns`: the task's turns now (how old the Notes are is counted from it: health's "Notes N turns old").
pub async fn update(
    st: &Store,
    task: &str,
    section: &str,
    text: &str,
    by: By,
    now: i64,
    turns: u32,
) -> wisp_store::Result<Result<Wrote, String>> {
    if meta(section).is_none() {
        return Ok(Err(unknown(section)));
    }
    // what clean drops (marks, repeats, past the newest 20) the agent sees when it reads them back
    let (clean_text, _) = clean(section, text);
    let (t, sec) = (task.to_owned(), section.to_owned());
    let changed = st
        .write(move |w| {
            let id = format!("{t}|{sec}");
            let old = w.get(store::NOTES.kind, id.clone())?;
            let old_text = old.as_ref().and_then(|r| r.get("text")).and_then(Value::as_str).unwrap_or_default().to_owned();
            if old_text == clean_text {
                return Ok(false);
            }
            let rev = old.as_ref().and_then(|r| r.get("rev")).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).unwrap_or(0) + 1;
            let by = by_str(by);
            w.upsert(store::NOTES.kind, store::row(json!({ "id": id, "task_id": t, "section": sec, "text": clean_text, "rev": rev, "by": by, "at": now })))?;
            w.upsert(store::REVISIONS.kind, store::row(json!({ "id": format!("{t}|{sec}|{rev}"), "task_id": t, "section": sec, "rev": rev, "text": clean_text, "by": by, "at": now })))?;
            let mut m = Meta::of(w.get(store::META.kind, t.clone())?.as_ref());
            m.rev += 1;
            m.turn = turns;
            m.at = now;
            // the agent knows what it wrote
            if by == "agent" {
                m.seen = m.rev;
            }
            w.upsert(store::META.kind, m.row(&t))?;
            Ok(true)
        })
        .await?;
    let (notes, _) = load(st, task).await?;
    Ok(Ok(Wrote { notes, changed }))
}

/// The agent has read the Notes at `rev` (they went in front of its turn).
pub async fn mark_seen(st: &Store, task: &str, rev: u32) -> wisp_store::Result<()> {
    let t = task.to_owned();
    st.write(move |w| {
        let mut m = Meta::of(w.get(store::META.kind, t.clone())?.as_ref());
        if m.seen >= rev {
            return Ok(());
        }
        m.seen = rev;
        w.upsert(store::META.kind, m.row(&t))?;
        Ok(())
    })
    .await
}

/// Every earlier version, oldest first; `section`: only that one's.
pub async fn revisions(st: &Store, task: &str, section: Option<&str>) -> wisp_store::Result<Vec<Revision>> {
    let (t, s) = (task.to_owned(), section.map(str::to_owned));
    st.read_async(move |r| {
        let mut q = Query::new().eq("task_id", t.as_str());
        if let Some(s) = &s {
            q = q.eq("section", s.as_str());
        }
        let mut out: Vec<Revision> = r
            .query(store::REVISIONS.kind, &q.asc("at"))?
            .iter()
            .map(|x| {
                let sec = section_of(x);
                Revision { section: sec.id, rev: sec.rev, text: sec.text, by: sec.by, at: sec.at }
            })
            .collect();
        out.sort_by_key(|x| (x.at, x.rev));
        Ok(out)
    })
    .await
}
