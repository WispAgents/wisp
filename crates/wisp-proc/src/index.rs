//! A scratch copy of a git index, the one way wisp makes one (`tools/check` rule `rust-index-copy`).
//!
//! Git trusts an index entry whose file still has the size and mtime it recorded, unless the entry is *racily
//! clean*: its mtime isn't older than the index file's own, so the file may have changed again in that second.
//! A copy stamped with a fresh mtime (Linux's `fs::copy`) makes every such entry look safely older, and a same-size
//! edit made in the index's own second reads as unchanged: the edit is silently dropped from what `add -A` /
//! `add --update` / `write-tree` see. So the copy keeps the real index's mtime, read from the same open file as
//! the bytes (git swaps its index by rename: a second `open` or `metadata` could see another index).
//!
//! A copy that can't keep the mtime is removed, never left behind: the caller then starts from an empty index or
//! HEAD's tree, which re-hashes every file (slower, never wrong).

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// Copies the index `from` to `to` with `from`'s mtime, blocking. On any error after `to` was created, `to` is
/// removed.
pub fn copy_index_blocking(from: &Path, to: &Path) -> io::Result<()> {
    let mut src = File::open(from)?;
    let mtime = src.metadata()?.modified()?;
    let mut dst = File::create(to)?;
    let done = io::copy(&mut src, &mut dst).and_then(|_| dst.set_modified(mtime));
    if done.is_err() {
        drop(dst);
        let _ = std::fs::remove_file(to);
    }
    done
}

/// [`copy_index_blocking`] on tokio's blocking pool: true when `to` is a copy with `from`'s mtime (else `to` is
/// gone, or never was).
pub async fn copy_index(from: PathBuf, to: PathBuf) -> bool {
    tokio::task::spawn_blocking(move || copy_index_blocking(&from, &to).is_ok()).await.unwrap_or(false)
}
