//! Where the stream's WAL went, writer by writer (reported, not gated): each frame's page is named by the b-tree it
//! belongs to (SQLite's `dbstat`, read once the server stopped), and each commit by the tables it touched, so a run
//! says which plugin's commits the bytes are, not only how many there were.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

/// Tables every commit touches (the change feed, page 1, freed pages): left out of a commit's name.
const EVERY: [&str; 4] = ["change", "change_topic", "page 1", "(free)"];

/// One kind of commit: the tables it wrote (but those in `EVERY`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Kind {
    pub commits: usize,
    pub frames: usize,
}

#[derive(Debug, Default)]
pub struct Writers {
    pub commits: usize,
    pub frames: usize,
    /// by the tables a commit touched, most frames first
    pub kinds: Vec<(String, Kind)>,
    /// frames by table (its indexes in it), most first
    pub tables: Vec<(String, usize)>,
}

/// The WAL's transactions: the pages of each, in order (a frame whose commit field is non-zero ends one).
pub fn transactions(wal: &[u8]) -> Vec<Vec<u32>> {
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    let (mut out, mut open) = (Vec::new(), Vec::new());
    if wal.len() < 32 {
        return out;
    }
    let size = 24 + be(&wal[8..12]) as usize;
    let mut at = 32;
    while at + size <= wal.len() {
        open.push(be(&wal[at..at + 4]));
        if be(&wal[at + 4..at + 8]) != 0 {
            out.push(std::mem::take(&mut open));
        }
        at += size;
    }
    out
}

/// A commit's name: the tables it wrote, sorted, `+`-joined (`EVERY` left out; none left is "(feed only)").
pub fn kind(tables: &BTreeSet<&str>) -> String {
    let named: Vec<&str> = tables.iter().copied().filter(|t| !EVERY.contains(t)).collect();
    if named.is_empty() { "(feed only)".to_owned() } else { named.join("+") }
}

/// Reads `wal` (the stream's frames) against `db` (the database they went into).
pub fn writers(wal: &Path, db: &Path) -> Result<Writers, String> {
    let bytes = std::fs::read(wal).map_err(|e| format!("read {}: {e}", wal.display()))?;
    let c = wisp_store::rusqlite::Connection::open(db).map_err(|e| format!("open the store: {e}"))?;
    let tbl: HashMap<String, String> = {
        let mut q = c.prepare("SELECT name, tbl_name FROM sqlite_schema").map_err(|e| e.to_string())?;
        q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    let page: HashMap<u32, String> = {
        let mut q = c.prepare("SELECT pageno, name FROM dbstat").map_err(|e| e.to_string())?;
        q.query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?
    };
    let name = |p: u32| -> String {
        if p == 1 {
            return "page 1".to_owned();
        }
        page.get(&p).map_or_else(|| "(free)".to_owned(), |n| tbl.get(n).cloned().unwrap_or_else(|| n.clone()))
    };
    let (mut w, mut kinds, mut tables) = (Writers::default(), BTreeMap::<String, Kind>::new(), BTreeMap::<String, usize>::new());
    for tx in transactions(&bytes) {
        let names: Vec<String> = tx.iter().map(|p| name(*p)).collect();
        for n in &names {
            *tables.entry(n.clone()).or_default() += 1;
        }
        let k = kinds.entry(kind(&names.iter().map(String::as_str).collect())).or_default();
        (k.commits, k.frames) = (k.commits + 1, k.frames + tx.len());
        (w.commits, w.frames) = (w.commits + 1, w.frames + tx.len());
    }
    w.kinds = kinds.into_iter().collect();
    w.kinds.sort_by_key(|k| std::cmp::Reverse(k.1.frames));
    w.tables = tables.into_iter().collect();
    w.tables.sort_by_key(|t| std::cmp::Reverse(t.1));
    Ok(w)
}

impl Writers {
    /// The report's lines: every kind of commit, then the tables by frames.
    pub fn lines(&self) -> Vec<String> {
        let mut out = vec![format!("where the stream's WAL went: {} commits, {} frames", self.commits, self.frames)];
        for (k, n) in &self.kinds {
            out.push(format!("  {:>4} commits {:>5} frames  {k}", n.commits, n.frames));
        }
        let t: Vec<String> = self.tables.iter().map(|(t, n)| format!("{t} {n}")).collect();
        out.push(format!("  frames by table: {}", t.join(" · ")));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(page: u32, commit: u32, size: usize) -> Vec<u8> {
        let mut f = vec![0; 24 + size];
        f[0..4].copy_from_slice(&page.to_be_bytes());
        f[4..8].copy_from_slice(&commit.to_be_bytes());
        f
    }

    #[test]
    fn a_wals_frames_split_into_its_commits_and_a_torn_tail_is_left_out() {
        let mut wal = vec![0; 32];
        wal[8..12].copy_from_slice(&512u32.to_be_bytes());
        for (p, c) in [(5, 0), (1, 9), (7, 9), (5, 0)] {
            wal.extend(frame(p, c, 512));
        }
        wal.extend(&[0; 100]);
        assert_eq!(transactions(&wal), vec![vec![5, 1], vec![7]]);
        assert!(transactions(&[0; 10]).is_empty());
    }

    #[test]
    fn a_commit_is_named_by_its_tables_but_the_feeds() {
        assert_eq!(kind(&["change", "page 1", "runtime_event", "event_cursor"].into()), "event_cursor+runtime_event");
        assert_eq!(kind(&["change", "change_topic"].into()), "(feed only)");
    }
}
