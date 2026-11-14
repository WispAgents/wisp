//! A path's real form, the one way wisp asks for it (`tools/check` rule `rust-canonicalize`).
//!
//! `std::fs::canonicalize` (and tokio's) on Windows returns a verbatim path, `\\?\C:\x` or `\\?\UNC\srv\share\x`.
//! Git reads `//?/C:/x` as a host name ("hostname contains invalid characters"), shells and most tools refuse it,
//! and it shows up as is in YAML, URLs and the UI. So every canonical path wisp makes goes through here and loses
//! the prefix: `\\?\C:\x` → `C:\x`, `\\?\UNC\srv\share\x` → `\\srv\share\x`. A verbatim path with no plain
//! spelling (`\\?\GLOBALROOT\…`, a `\\?\Volume{…}`) is kept: there's nothing shorter that means the same file.
//! On Unix nothing has the prefix, so the strip is a no-op there.

use std::io;
use std::path::{Path, PathBuf};

/// `p` without Windows' verbatim prefix (by its text alone, on any OS): `\\?\C:\x` is `C:\x`, `\\?\UNC\srv\share`
/// is `\\srv\share`; anything else (a plain path, `\\?\Volume{…}`) is `p` as it is.
#[must_use]
pub fn strip_verbatim(p: &str) -> String {
    if let Some(rest) = p.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    match p.strip_prefix(r"\\?\") {
        // a drive: C:\x (or a bare C:)
        Some(rest) if is_drive(rest) => rest.to_owned(),
        _ => p.to_owned(),
    }
}

fn is_drive(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || matches!(b[2], b'\\' | b'/'))
}

/// [`strip_verbatim`] on a path; one that isn't UTF-8 is returned as it is.
#[must_use]
pub fn without_verbatim(p: PathBuf) -> PathBuf {
    match p.to_str() {
        Some(s) if s.starts_with(r"\\?\") => PathBuf::from(strip_verbatim(s)),
        _ => p,
    }
}

/// `p` with every link and `..` resolved (it must exist), never in Windows' verbatim form. Blocking: for startup,
/// tests and code already off the runtime.
pub fn canonical_blocking(p: impl AsRef<Path>) -> io::Result<PathBuf> {
    // canonicalize: the helper itself
    std::fs::canonicalize(p).map(without_verbatim)
}

/// [`canonical_blocking`], without blocking a runtime thread.
pub async fn canonical(p: impl AsRef<Path>) -> io::Result<PathBuf> {
    // canonicalize: the helper itself
    tokio::fs::canonicalize(p).await.map(without_verbatim)
}
