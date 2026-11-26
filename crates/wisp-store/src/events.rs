//! The durable event log (ADR-0044): `event`, `event_cursor` and `event_parked`, kernel-owned tables.
//!
//! - `event` is rowid-only (no secondary index, as `change`): `seq` is the rowid. One writer appends, so
//!   committed seqs are contiguous (what the kernel's workers rely on to never skip one). `data` is compact
//!   JSON, zstd above 1 KB (`codec` 1).
//! - **In the command's transaction:** `Store::write` asks the kernel (`durable::take`) for the durable
//!   events the running command emitted so far and the cursor of the handler it runs in, and writes them
//!   in the same transaction as the command's own ops. A rollback takes them with it. Just before COMMIT the
//!   write also stages every event the command emitted so far (ADR-0075: `InTx::stage`, held in the queues,
//!   let go after COMMIT, put back on ROLLBACK), so whoever reads its rows finds them queued.
//! - **Dedupe:** a `(type, key)` appended within 24 h is recorded once. The keys of the last 24 h are kept in
//!   memory (read back at open), so the check needs no index.
//! - **Retention:** 30 days or 1 M rows, whichever is first, trimmed from the oldest every 10 k appends and at
//!   open. A cursor behind the first kept seq gets a gap from the kernel.
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use wisp_kernel::BoxFuture;
use wisp_kernel::durable::{EventLog, NewEvent, Parked, StoredEvent};

use crate::error::{Result, StoreError};
use crate::store::{Store, lock};
use crate::writer::Writer;

pub const DEDUPE_MS: i64 = 24 * 3_600_000;
pub const KEEP_MS: i64 = 30 * 24 * 3_600_000;
pub const KEEP_ROWS: i64 = 1_000_000;
const TRIM_EVERY: u64 = 10_000;
/// Keys added between two sweeps of `LogState::recent` at the least (or half the map, when that's more): the
/// sweep walks the whole map, so it runs at most once per as many inserts as there are keys to walk.
const SWEEP_EVERY: u64 = 4096;
const ZSTD_OVER: usize = 1024;

/// The tables, as the kernel owner's migration 2.
pub const SQL: &str = r"
    CREATE TABLE event (
      seq INTEGER PRIMARY KEY,
      type TEXT NOT NULL,
      plugin TEXT NOT NULL,
      subject TEXT,
      key TEXT,
      cause INTEGER,
      root TEXT NOT NULL,
      at INTEGER NOT NULL,
      codec INTEGER NOT NULL DEFAULT 0,
      data BLOB NOT NULL
    ) STRICT;
    CREATE TABLE event_cursor (plugin TEXT NOT NULL, name TEXT NOT NULL, seq INTEGER NOT NULL, PRIMARY KEY (plugin, name)) STRICT, WITHOUT ROWID;
    CREATE TABLE event_parked (plugin TEXT NOT NULL, name TEXT NOT NULL, seq INTEGER NOT NULL, why TEXT NOT NULL, at INTEGER NOT NULL, PRIMARY KEY (plugin, name, seq)) STRICT, WITHOUT ROWID;
";

/// What the store keeps in memory for the log.
#[derive(Debug, Default)]
pub(crate) struct LogState {
    /// (type, key) → when it was appended, the last 24 h
    pub(crate) recent: Mutex<HashMap<(String, String), i64>>,
    pub(crate) appended: AtomicU64,
    /// keys put in `recent` since its last sweep
    pub(crate) unswept: AtomicU64,
}

fn encode(v: &Value) -> (i64, Vec<u8>) {
    let raw = serde_json::to_vec(v).unwrap_or_default();
    if raw.len() > ZSTD_OVER
        && let Ok(z) = zstd::encode_all(raw.as_slice(), 3)
    {
        return (1, z);
    }
    (0, raw)
}

fn decode(codec: i64, b: &[u8]) -> Value {
    let raw = if codec == 1 { zstd::decode_all(b).unwrap_or_default() } else { b.to_vec() };
    serde_json::from_slice(&raw).unwrap_or(Value::Null)
}

/// Appends `rows` on `db` (inside the caller's transaction), deduping by (type, key) within 24 h. Keys it
/// added are returned so a rollback can take them out again.
/// Each row's seq (`None`: a repeat), and the dedupe keys added.
type Appended = (Vec<Option<u64>>, Vec<(String, String)>);

pub(crate) fn append_on(db: &Connection, st: &LogState, now: i64, rows: &[NewEvent]) -> Result<Appended> {
    let mut out = Vec::with_capacity(rows.len());
    let mut added = Vec::new();
    let mut recent = lock(&st.recent);
    for r in rows {
        if let Some(k) = &r.key {
            let id = (r.ty.clone(), k.clone());
            if recent.get(&id).is_some_and(|at| now - at < DEDUPE_MS) {
                out.push(None);
                continue;
            }
            recent.insert(id.clone(), now);
            added.push(id);
        }
        let (codec, data) = encode(&r.data);
        db.prepare_cached(
            "INSERT INTO event (type, plugin, subject, key, cause, root, at, codec, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?
        .execute(params![
            r.ty,
            r.plugin,
            r.subject,
            r.key,
            r.cause.and_then(|c| i64::try_from(c).ok()),
            r.root,
            if r.at > 0 { r.at } else { now },
            codec,
            data
        ])?;
        out.push(u64::try_from(db.last_insert_rowid()).ok());
    }
    // A key older than DEDUPE_MS dedupes nothing, but only a rollback took one out: the map grew by every keyed
    // event for as long as wisp ran. Swept here, amortised, so no append pays for walking it.
    let fresh = added.len() as u64;
    if fresh > 0 && st.unswept.fetch_add(fresh, Ordering::Relaxed) + fresh >= SWEEP_EVERY.max(recent.len() as u64 / 2) {
        st.unswept.store(0, Ordering::Relaxed);
        recent.retain(|_, at| now - *at < DEDUPE_MS);
    }
    Ok((out, added))
}

pub(crate) fn forget(st: &LogState, keys: &[(String, String)]) {
    let mut recent = lock(&st.recent);
    for k in keys {
        recent.remove(k);
    }
}

pub(crate) fn set_cursor_on(db: &Connection, plugin: &str, name: &str, seq: u64) -> Result<()> {
    db.prepare_cached("INSERT INTO event_cursor (plugin, name, seq) VALUES (?1, ?2, ?3) ON CONFLICT (plugin, name) DO UPDATE SET seq = max(seq, excluded.seq)")?
        .execute(params![plugin, name, i64::try_from(seq).unwrap_or(i64::MAX)])?;
    Ok(())
}

/// Trims what's past retention: older than 30 days, or beyond the newest 1 M.
/// Always preserves the newest row (`seq = max`) so SQLite's rowid never resets back to 1 (as `change` does).
pub(crate) fn trim_on(db: &Connection, now: i64, keep_ms: i64, keep_rows: i64) -> Result<usize> {
    let max: i64 = db.query_row("SELECT coalesce(max(seq), 0) FROM event", [], |r| r.get(0))?;
    if max == 0 {
        return Ok(0);
    }
    let a = db.execute("DELETE FROM event WHERE seq <= ?1 AND seq < ?2", [max - keep_rows, max])?;
    let cut = first_seq_at_or_after(db, now - keep_ms)?;
    let b = match cut {
        Some(s) => db.execute("DELETE FROM event WHERE seq < ?1 AND seq < ?2", [s, max])?,
        None => db.execute("DELETE FROM event WHERE seq < ?1", [max])?,
    };
    Ok(a + b)
}

/// The first seq whose row was appended at `at` or later, none when every row is older. Rowid order is time order, so
/// it's a binary search on seq: about 20 rowid lookups for 1 M rows. `at` has no index, and `WHERE at >= ? ORDER BY
/// seq LIMIT 1` walked the table from its oldest row (perf-r1 #4: at every open, before the first response).
pub(crate) fn first_seq_at_or_after(db: &Connection, at: i64) -> Result<Option<i64>> {
    // one aggregate per query: SQLite reads min or max off the rowid's ends only when it's alone; together they scan
    let min: Option<i64> = db.query_row("SELECT min(seq) FROM event", [], |r| r.get(0))?;
    let max: Option<i64> = db.query_row("SELECT max(seq) FROM event", [], |r| r.get(0))?;
    let (Some(mut lo), Some(mut hi)) = (min, max) else { return Ok(None) };
    // the first row at or after `seq` (seqs have gaps once trimmed or rolled back), and when it was appended
    let mut probe = db.prepare_cached("SELECT seq, at FROM event WHERE seq >= ?1 ORDER BY seq LIMIT 1")?;
    let mut first_from =
        |seq: i64| -> Result<Option<(i64, i64)>> { Ok(probe.query_row([seq], |r| Ok((r.get(0)?, r.get(1)?))).optional()?) };
    if first_from(hi)?.is_none_or(|(_, t)| t < at) {
        return Ok(None);
    }
    // the smallest seq from which the next row is young enough
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if first_from(mid)?.is_some_and(|(_, t)| t >= at) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    Ok(first_from(lo)?.map(|(s, _)| s))
}

/// The dedupe keys appended in the 24 h before `now`, with when each was.
fn recent_keys(db: &Connection, now: i64) -> Result<Vec<(String, String, i64)>> {
    let Some(from) = first_seq_at_or_after(db, now - DEDUPE_MS)? else { return Ok(Vec::new()) };
    let mut s = db.prepare("SELECT type, key, at FROM event WHERE key IS NOT NULL AND seq >= ?1")?;
    let v = s.query_map([from], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<StoredEvent> {
    let codec: i64 = r.get(8)?;
    let data: Vec<u8> = r.get(9)?;
    Ok(StoredEvent {
        seq: u64::try_from(r.get::<_, i64>(0)?).unwrap_or(0),
        event: NewEvent {
            ty: r.get(1)?,
            plugin: r.get(2)?,
            subject: r.get(3)?,
            key: r.get(4)?,
            cause: r.get::<_, Option<i64>>(5)?.and_then(|c| u64::try_from(c).ok()),
            root: r.get(6)?,
            at: r.get(7)?,
            data: decode(codec, &data),
        },
    })
}

const COLS: &str = "seq, type, plugin, subject, key, cause, root, at, codec, data";

impl Store {
    /// At open: the dedupe keys of the last 24 h, and a first trim.
    pub(crate) fn init_events(&self) -> Result<()> {
        let now = self.shared.clock.now_ms();
        let keys = self.read(|r| recent_keys(r.db, now))?;
        {
            let mut recent = lock(&self.shared.log.recent);
            for (t, k, at) in keys {
                recent.insert((t, k), at);
            }
        }
        self.thread.run_raw(move |db| trim_on(db, now, KEEP_MS, KEEP_ROWS).map(|_| ()))
    }

    /// Trims now with these limits (tests, and `wisp events` maintenance).
    pub async fn trim_events(&self, keep_ms: i64, keep_rows: i64) -> Result<usize> {
        self.write(move |w| {
            let now = w.now();
            trim_on(w.db, now, keep_ms, keep_rows)
        })
        .await
    }

    fn after_append(&self, n: usize) {
        let before = self.shared.log.appended.fetch_add(n as u64, Ordering::Relaxed);
        if before / TRIM_EVERY != (before + n as u64) / TRIM_EVERY {
            let now = self.shared.clock.now_ms();
            let s = self.clone();
            s.write_detached(move |w| trim_on(w.db, now, KEEP_MS, KEEP_ROWS).map(|_| ()));
        }
    }

    /// `write`, carrying the running command's durable events and its handler's cursor (ADR-0044), and an outbox's
    /// events (ADR-0074: its durable ones appended here too, all of them staged just before COMMIT).
    pub(crate) async fn write_durable<T, F>(
        &self,
        intx: Option<wisp_kernel::durable::InTx>,
        outbox: Option<wisp_kernel::Outbox>,
        f: F,
    ) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Writer<'_>) -> Result<T> + Send + 'static,
    {
        let rows = intx.as_ref().map(|i| i.rows().to_vec()).unwrap_or_default();
        let cursor = intx.as_ref().and_then(|i| i.cursor().map(|(p, n, s)| (p.to_owned(), n.to_owned(), s)));
        let mine = outbox.as_ref().map(wisp_kernel::Outbox::rows).unwrap_or_default();
        let n = rows.len() + mine.len();
        let shared = self.shared.clone();
        let intx = intx.map(std::sync::Arc::new);
        let intx2 = intx.clone();
        let r = self
            .thread
            .queued(move |s| {
                let added = Mutex::new(Vec::new());
                let res = crate::thread::transact(s, |w| {
                    let v = f(w)?;
                    let now = w.now();
                    let (seqs, keys) = append_on(w.db, &shared.log, now, &rows)?;
                    lock(&added).extend(keys);
                    if let Some((p, name, seq)) = &cursor {
                        set_cursor_on(w.db, p, name, *seq)?;
                    }
                    let mut head = None;
                    // last thing before COMMIT: the command's events so far (ADR-0075), then the outbox's (ADR-0074)
                    if let Some(i) = &intx2 {
                        head = u64::try_from(w.head_seq()?).ok();
                        if let Some(st) = i.stage(head, &seqs) {
                            w.staged.push(st);
                        }
                    }
                    if let Some(ob) = outbox {
                        let (theirs, keys) = append_on(w.db, &shared.log, now, &mine)?;
                        lock(&added).extend(keys);
                        let head = match head {
                            Some(h) => Some(h),
                            None => u64::try_from(w.head_seq()?).ok(),
                        };
                        w.staged.push(ob.stage(head, &theirs));
                    }
                    Ok::<T, StoreError>(v)
                });
                if res.is_err() {
                    forget(&shared.log, &lock(&added));
                }
                res
            })
            .await;
        match r {
            Ok(v) => {
                if let Some(i) = &intx {
                    i.committed();
                }
                if n > 0 {
                    self.after_append(n);
                }
                Ok(v)
            }
            Err(e) => {
                if let Some(i) = &intx {
                    i.rolled_back();
                }
                Err(e)
            }
        }
    }
}

impl Store {
    /// `write_detached`, and `outbox`'s events go out with its commit (ADR-0074): for a caller that must not await
    /// (a channel transport's host, a peer's status) and says what changed. Whoever reads the rows finds the events
    /// queued, and nobody hears them before the rows are there (emitted right after `write_detached`, they could be
    /// delivered before it ran). A failed write publishes none. Additive (2026-11-15).
    pub fn write_detached_with<F>(&self, outbox: wisp_kernel::Outbox, f: F)
    where
        F: FnOnce(&mut Writer<'_>) -> Result<()> + Send + 'static,
    {
        let mine = outbox.rows();
        let logged = mine.len();
        let store = self.clone();
        let _ = self.thread.submit(Box::new(move |shared| {
            let added = Mutex::new(Vec::new());
            let r = crate::thread::transact(shared, |w| {
                f(w)?;
                let (seqs, keys) = append_on(w.db, &shared.log, w.now(), &mine)?;
                lock(&added).extend(keys);
                let head = u64::try_from(w.head_seq()?).ok();
                w.staged.push(outbox.stage(head, &seqs));
                Ok(())
            });
            match r {
                Ok(()) if logged > 0 => store.after_append(logged),
                Ok(()) => {}
                Err(e) => {
                    forget(&shared.log, &lock(&added));
                    tracing::warn!(error = %e, "a detached write failed; its events never went out");
                }
            }
        }));
    }
}

fn i(s: u64) -> i64 {
    i64::try_from(s).unwrap_or(i64::MAX)
}

fn wire(e: &StoreError) -> wisp_kernel::WireError {
    wisp_kernel::WireError::internal(e.to_string())
}

impl EventLog for Store {
    fn append(&self, rows: Vec<NewEvent>) -> BoxFuture<'static, wisp_kernel::Result<Vec<Option<u64>>>> {
        let s = self.clone();
        Box::pin(async move {
            let n = rows.len();
            let shared = s.shared.clone();
            let r = s
                .thread
                .queued(move |st| {
                    let added = Mutex::new(Vec::new());
                    let res = crate::thread::transact(st, |w| {
                        let now = w.now();
                        let (seqs, keys) = append_on(w.db, &shared.log, now, &rows)?;
                        *lock(&added) = keys;
                        Ok(seqs)
                    });
                    if res.is_err() {
                        forget(&shared.log, &lock(&added));
                    }
                    res
                })
                .await;
            match r {
                Ok(v) => {
                    s.after_append(n);
                    Ok(v)
                }
                Err(e) => Err(wire(&e)),
            }
        })
    }
    fn read(&self, after: u64, limit: usize) -> BoxFuture<'static, wisp_kernel::Result<Vec<StoredEvent>>> {
        let s = self.clone();
        Box::pin(async move {
            s.read_async(move |r| {
                let mut st = r.db.prepare_cached(&format!("SELECT {COLS} FROM event WHERE seq > ?1 ORDER BY seq LIMIT ?2"))?;
                Ok(st.query_map(params![i(after), i64::try_from(limit).unwrap_or(256)], row)?.collect::<rusqlite::Result<Vec<_>>>()?)
            })
            .await
            .map_err(|e| wire(&e))
        })
    }
    fn get(&self, seq: u64) -> BoxFuture<'static, wisp_kernel::Result<Option<StoredEvent>>> {
        let s = self.clone();
        Box::pin(async move {
            s.read_async(move |r| {
                Ok(r.db.prepare_cached(&format!("SELECT {COLS} FROM event WHERE seq = ?1"))?.query_row([i(seq)], row).optional()?)
            })
            .await
            .map_err(|e| wire(&e))
        })
    }
    fn bounds(&self) -> BoxFuture<'static, wisp_kernel::Result<(u64, u64)>> {
        let s = self.clone();
        Box::pin(async move {
            s.read_async(|r| {
                let (a, b): (i64, i64) =
                    r.db.query_row("SELECT coalesce(min(seq), 0), coalesce(max(seq), 0) FROM event", [], |x| Ok((x.get(0)?, x.get(1)?)))?;
                Ok((u64::try_from(a).unwrap_or(0), u64::try_from(b).unwrap_or(0)))
            })
            .await
            .map_err(|e| wire(&e))
        })
    }
    fn cursor(&self, plugin: &str, name: &str) -> BoxFuture<'static, wisp_kernel::Result<Option<u64>>> {
        let (s, p, n) = (self.clone(), plugin.to_owned(), name.to_owned());
        Box::pin(async move {
            s.read_async(move |r| {
                Ok(r.db
                    .query_row("SELECT seq FROM event_cursor WHERE plugin = ?1 AND name = ?2", params![p, n], |x| x.get::<_, i64>(0))
                    .optional()?
                    .and_then(|v| u64::try_from(v).ok()))
            })
            .await
            .map_err(|e| wire(&e))
        })
    }
    fn set_cursor(&self, plugin: &str, name: &str, seq: u64) -> BoxFuture<'static, wisp_kernel::Result<()>> {
        let (s, p, n) = (self.clone(), plugin.to_owned(), name.to_owned());
        Box::pin(async move { s.write(move |w| set_cursor_on(w.db, &p, &n, seq)).await.map_err(|e| wire(&e)) })
    }
    /// Every idle subscription's cursor in one transaction (ADR-0088): one commit, not one each.
    fn set_cursors(&self, rows: Vec<(String, String, u64)>) -> BoxFuture<'static, wisp_kernel::Result<()>> {
        let s = self.clone();
        Box::pin(async move {
            s.write(move |w| rows.iter().try_for_each(|(p, n, seq)| set_cursor_on(w.db, p, n, *seq))).await.map_err(|e| wire(&e))
        })
    }
    fn park(&self, x: Parked) -> BoxFuture<'static, wisp_kernel::Result<()>> {
        let s = self.clone();
        Box::pin(async move {
            s.write(move |w| {
                w.db.execute(
                    "INSERT OR REPLACE INTO event_parked (plugin, name, seq, why, at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![x.plugin, x.name, i(x.seq), x.why, x.at],
                )?;
                Ok(())
            })
            .await
            .map_err(|e| wire(&e))
        })
    }
    fn parked(&self) -> BoxFuture<'static, wisp_kernel::Result<Vec<Parked>>> {
        let s = self.clone();
        Box::pin(async move {
            s.read_async(|r| {
                let mut st = r.db.prepare("SELECT plugin, name, seq, why, at FROM event_parked ORDER BY seq")?;
                Ok(st
                    .query_map([], |x| {
                        Ok(Parked {
                            plugin: x.get(0)?,
                            name: x.get(1)?,
                            seq: u64::try_from(x.get::<_, i64>(2)?).unwrap_or(0),
                            why: x.get(3)?,
                            at: x.get(4)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?)
            })
            .await
            .map_err(|e| wire(&e))
        })
    }
    fn unpark(&self, plugin: &str, name: &str, seq: u64) -> BoxFuture<'static, wisp_kernel::Result<bool>> {
        let (s, p, n) = (self.clone(), plugin.to_owned(), name.to_owned());
        Box::pin(async move {
            s.write(move |w| {
                Ok(w.db.execute("DELETE FROM event_parked WHERE plugin = ?1 AND name = ?2 AND seq = ?3", params![p, n, i(seq)])? > 0)
            })
            .await
            .map_err(|e| wire(&e))
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn log() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SQL).unwrap();
        db
    }

    /// `(seq, at, key)` rows, as `append_on` would leave them.
    fn put(db: &Connection, rows: &[(i64, i64, Option<&str>)]) {
        let mut st =
            db.prepare("INSERT INTO event (seq, type, plugin, key, root, at, data) VALUES (?1, 't', 'p', ?3, 'r', ?2, x'7b7d')").unwrap();
        for (seq, at, key) in rows {
            st.execute(params![seq, at, key]).unwrap();
        }
    }

    /// What the lookup was: a walk from the oldest row.
    fn walk(db: &Connection, at: i64) -> Option<i64> {
        db.query_row("SELECT seq FROM event WHERE at >= ?1 ORDER BY seq LIMIT 1", [at], |r| r.get(0)).optional().unwrap()
    }

    #[test]
    fn events_boundary_is_found_on_empty_old_new_exact_and_gapped_logs() {
        let db = log();
        assert_eq!(first_seq_at_or_after(&db, 0).unwrap(), None, "empty");
        put(&db, &[(1, 100, None), (2, 200, None), (3, 300, None), (4, 300, None), (5, 400, None)]);
        assert_eq!(first_seq_at_or_after(&db, 500).unwrap(), None, "every row older");
        assert_eq!(first_seq_at_or_after(&db, 50).unwrap(), Some(1), "every row newer");
        assert_eq!(first_seq_at_or_after(&db, 300).unwrap(), Some(3), "an exact match: the first with it");
        assert_eq!(first_seq_at_or_after(&db, 301).unwrap(), Some(5));
        // trimmed and rolled back: gaps in seq
        db.execute("DELETE FROM event WHERE seq IN (1, 3)", []).unwrap();
        put(&db, &[(9, 500, None), (20, 600, None)]);
        for at in [0, 100, 150, 200, 250, 300, 301, 400, 450, 500, 600, 601] {
            assert_eq!(first_seq_at_or_after(&db, at).unwrap(), walk(&db, at), "at {at}");
        }
        let single = log();
        put(&single, &[(7, 10, None)]);
        assert_eq!(first_seq_at_or_after(&single, 10).unwrap(), Some(7));
        assert_eq!(first_seq_at_or_after(&single, 11).unwrap(), None);
    }

    #[test]
    fn events_dedupe_keys_read_at_open_are_the_old_querys() {
        let db = log();
        let now = 10 * DEDUPE_MS;
        let rows: Vec<(i64, i64, Option<String>)> =
            (1..=400).map(|i| (i, now - 2 * DEDUPE_MS + i * (DEDUPE_MS / 100), (i % 3 != 0).then(|| format!("k{}", i % 50)))).collect();
        let rows: Vec<(i64, i64, Option<&str>)> = rows.iter().map(|(s, a, k)| (*s, *a, k.as_deref())).collect();
        put(&db, &rows);
        let old: Vec<(String, String, i64)> = db
            .prepare("SELECT type, key, at FROM event WHERE key IS NOT NULL AND seq >= coalesce((SELECT seq FROM event WHERE at >= ?1 ORDER BY seq LIMIT 1), 9223372036854775807)")
            .unwrap()
            .query_map([now - DEDUPE_MS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let new = recent_keys(&db, now).unwrap();
        assert!(!old.is_empty() && old.len() < rows.len(), "a mixed log: some in the last 24 h, some older");
        assert_eq!(new, old);
        assert_eq!(recent_keys(&db, 100 * DEDUPE_MS).unwrap(), Vec::new(), "nothing in the last 24 h");
    }

    fn keyed(key: &str) -> NewEvent {
        NewEvent {
            ty: "t".into(),
            plugin: "p".into(),
            subject: None,
            key: Some(key.into()),
            cause: None,
            root: "r".into(),
            at: 0,
            data: Value::Null,
        }
    }

    /// The dedupe map lost a key only on a rollback: every keyed event stayed in memory for as long as wisp ran.
    /// Keys past `DEDUPE_MS` go at the next sweep; within it a repeat is still dropped, and after it taken again.
    #[test]
    fn events_dedupe_map_drops_keys_older_than_a_day() {
        let db = log();
        let st = LogState::default();
        let t0 = DEDUPE_MS;
        let old: Vec<NewEvent> = (0..1000).map(|i| keyed(&format!("old{i}"))).collect();
        append_on(&db, &st, t0, &old).unwrap();
        let (seqs, _) = append_on(&db, &st, t0 + DEDUPE_MS - 1, &[keyed("old0")]).unwrap();
        assert_eq!(seqs, vec![None], "a repeat within 24 h is dropped");
        let later = t0 + DEDUPE_MS + 1;
        let new: Vec<NewEvent> = (0..SWEEP_EVERY).map(|i| keyed(&format!("new{i}"))).collect();
        append_on(&db, &st, later, &new).unwrap();
        let len = lock(&st.recent).len() as u64;
        assert_eq!(len, SWEEP_EVERY, "the day-old keys are gone");
        let (seqs, _) = append_on(&db, &st, later, &[keyed("new0"), keyed("old1")]).unwrap();
        assert_eq!(seqs[0], None, "a key kept is still deduped");
        assert!(seqs[1].is_some(), "after 24 h a key is taken again");
    }

    /// The old query walked every row older than the boundary; the search looks at ~20.
    #[test]
    fn events_boundary_lookup_is_a_few_rowid_probes_not_a_table_walk() {
        let db = log();
        let n: i64 = 1_000_000;
        db.execute_batch("BEGIN").unwrap();
        put(&db, &(1..=n).map(|i| (i, i * 10, None)).collect::<Vec<_>>());
        db.execute_batch("COMMIT").unwrap();
        let at = (n - 10) * 10; // the last day's: almost every row is older
        let t = std::time::Instant::now();
        let found = first_seq_at_or_after(&db, at).unwrap();
        let fast = t.elapsed();
        let t = std::time::Instant::now();
        let walked = walk(&db, at);
        let slow = t.elapsed();
        assert_eq!(found, Some(n - 10));
        assert_eq!(found, walked);
        assert!(fast < std::time::Duration::from_millis(5), "{fast:?} (the walk: {slow:?})");
        assert!(fast < slow, "{fast:?} vs the walk's {slow:?}");
    }
}
