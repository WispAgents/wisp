//! A topic's first look in pages (ADR-0081): [`PagedSource`], its read in one read transaction, and the pages
//! handed over as they are read (`Snapshots::snapshot_pages` on the store).
use serde_json::{Map, Value};
use wisp_kernel::BoxFuture;
use wisp_protocol::{Row, Topic};
use wisp_store::{Query, Reader};

use super::{StoreSnapshots, TopicReader, TopicSource, delta_read, full_rows, id_text, seq};
use crate::egress;
use crate::v2::{PageSink, Read};

/// A topic whose full snapshot is one kind's rows read in pages (ADR-0081): wisp.runtime's tasks on `list`, so a first
/// look draws the first screen from `first` rows before the other thousands are read. `queries` run in order, in one
/// read transaction (every page at one seq), each paged with `limit`/`offset` over its own order (they set neither).
/// A resume (`since` the feed still holds) reads the change feed, as any topic. Additive (2026-11-16).
#[derive(Clone, Copy)]
pub struct PagedSource {
    pub kind: &'static str,
    /// the kind's key column (`EntityDef::key`): a row's `id` on the wire
    pub key: &'static str,
    pub matches: fn(&str) -> bool,
    pub queries: fn(&str) -> Vec<Query>,
    /// rows in the first page
    pub first: u64,
    /// rows in each page after it
    pub page: u64,
}

impl std::fmt::Debug for PagedSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PagedSource")
            .field("kind", &self.kind)
            .field("first", &self.first)
            .field("page", &self.page)
            .finish_non_exhaustive()
    }
}

/// What one read of a topic sees: its sources, readers and paged sources.
#[derive(Clone, Copy)]
pub(super) struct TopicReads<'a> {
    pub sources: &'a [TopicSource],
    pub readers: &'a [TopicReader],
    pub pages: &'a [PagedSource],
}

/// The rows changed since a cursor the change feed still holds; `None` for a first look or a cursor it dropped.
pub(super) fn resume(r: &Reader<'_>, s: TopicReads<'_>, topic: &str, since: Option<u64>) -> wisp_store::Result<Option<Read>> {
    let Some(since) = since else { return Ok(None) };
    let Some(mut d) = r.changed_since(topic, i64::try_from(since).unwrap_or(i64::MAX))? else { return Ok(None) };
    if let Some(live) = s.readers.iter().find(|x| (x.matches)(topic)).and_then(|x| x.live) {
        (live.delta)(r, topic, &mut d.rows)?;
    }
    let key_of =
        |e: &str| s.sources.iter().find(|x| x.kind == e).map(|x| x.key).or_else(|| s.pages.iter().find(|x| x.kind == e).map(|x| x.key));
    Ok(Some(delta_read(d, false, key_of)))
}

/// A [`PagedSource`]'s topic in full, each page handed to `emit` as it is read (`more` on all but the last); other
/// sources' kinds on the same topic go in the first page.
fn read_paged(r: &Reader<'_>, s: TopicReads<'_>, p: &PagedSource, topic: &str, emit: &mut dyn FnMut(Read, bool)) -> wisp_store::Result<()> {
    let head = seq(r.head_seq()?);
    let page = |rows: Vec<Row>| Read { seq: head, rows, full: true, deleted: Vec::new() };
    let others: Vec<TopicSource> = s.sources.iter().filter(|x| x.kind != p.kind).copied().collect();
    let mut rows = full_rows(r, &others, topic)?.unwrap_or_default();
    // a plugin's kind that isn't defined yet (it hasn't started, or it's off) has no rows to show
    if r.defines(p.kind) {
        let mut cap = p.first.max(1);
        for q in (p.queries)(topic) {
            let mut offset = 0;
            loop {
                let want = cap.saturating_sub(rows.len() as u64).max(1);
                let text = r.query_json(p.kind, &q.clone().limit(want).offset(offset))?;
                let got: Vec<Map<String, Value>> =
                    serde_json::from_str(&text).map_err(|e| wisp_store::StoreError::Internal(format!("{}: {e}", p.kind)))?;
                drop(text);
                let n = got.len() as u64;
                offset += n;
                rows.extend(got.into_iter().map(|v| Row { e: p.kind.to_owned(), id: id_text(v.get(p.key)), v: Value::Object(v) }));
                if rows.len() as u64 >= cap {
                    emit(page(std::mem::take(&mut rows)), true);
                    cap = p.page.max(1);
                }
                if n < want {
                    break;
                }
            }
        }
    }
    emit(page(rows), false);
    Ok(())
}

/// A [`PagedSource`]'s topic in one read: every page joined (`Snapshots::snapshot`).
pub(super) fn read_joined(r: &Reader<'_>, s: TopicReads<'_>, p: &PagedSource, topic: &str) -> wisp_store::Result<Read> {
    let mut all = Read::default();
    read_paged(r, s, p, topic, &mut |page, _| {
        all.seq = page.seq;
        all.full = true;
        all.rows.extend(page.rows);
    })?;
    Ok(all)
}

/// [`Snapshots::snapshot_pages`](crate::v2::Snapshots::snapshot_pages) on the store: the reader hands each page over as it
/// reads the next, and each leaves through the views here.
pub(super) fn snapshot_pages(snaps: &StoreSnapshots, topic: &Topic, since: Option<u64>, mut page: PageSink) -> BoxFuture<'static, ()> {
    let Some(p) = snaps.pages.iter().find(|x| (x.matches)(topic.as_str())).copied() else {
        let read = crate::v2::Snapshots::snapshot(snaps, topic, since);
        return Box::pin(async move { page(read.await, false) });
    };
    let (store, sources, readers, pages) = (snaps.store.clone(), snaps.sources.clone(), snaps.readers.clone(), snaps.pages.clone());
    let (views, topic) = (snaps.views.clone(), topic.as_str().to_owned());
    let again = (sources.clone(), readers.clone(), pages.clone());
    Box::pin(async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(Read, bool)>();
        let t = topic.clone();
        let reading = store.read_async(move |r| {
            let s = TopicReads { sources: &sources, readers: &readers, pages: &pages };
            if let Some(read) = resume(r, s, &t, since)? {
                let _ = tx.send((read, false));
                return Ok(());
            }
            read_paged(r, s, &p, &t, &mut |read, more| {
                let _ = tx.send((read, more));
            })
        });
        // `(seq, closed)` of what went out: a read that stops partway still closes the snapshot at that seq
        let mut sent: Option<(u64, bool)> = None;
        let sending = async {
            while let Some((mut read, more)) = rx.recv().await {
                egress::rows(&views.views(), &mut read.rows);
                sent = Some((read.seq, !more));
                page(read, more);
            }
        };
        let (read, ()) = tokio::join!(reading, sending);
        if let Err(e) = read {
            tracing::warn!(topic, "snapshot failed: {e}");
        }
        match sent {
            Some((_, true)) => {}
            // pages went out and the read stopped: the whole topic, read once more, closes the snapshot (at a later seq
            // the client starts over with it), never the pages alone, which would drop every task they didn't reach;
            // a second failure closes it with what went (as a failed read answers empty)
            Some((seq, false)) => {
                let (t, (sources, readers, pages)) = (topic.clone(), again);
                let read =
                    store.read_async(move |r| read_joined(r, TopicReads { sources: &sources, readers: &readers, pages: &pages }, &p, &t));
                match read.await {
                    Ok(mut read) => {
                        egress::rows(&views.views(), &mut read.rows);
                        page(read, false);
                    }
                    Err(e) => {
                        tracing::warn!(topic, "snapshot failed again, closed with the pages sent: {e}");
                        page(Read { seq, full: true, ..Read::default() }, false);
                    }
                }
            }
            None => page(Read { full: since.is_none(), ..Read::default() }, false),
        }
    })
}
