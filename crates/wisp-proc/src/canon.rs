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

/// One folder, one spelling (ADR-0071): `p` with Windows' 8.3 short names spelled out and no verbatim prefix, links
/// not followed. `long_of` gives the long form of a path that exists (`None`: it isn't there), so the longest part of
/// `p` that exists is spelled out and the rest (not there yet: a file to write, a folder to make) is kept as it is
/// (`C:\Users\RUNNER~1\x\gone.png` → `C:\Users\runneradmin\x\gone.png`). A relative path, or an empty one, is only
/// stripped. By its text, with either separator: pure, so it is tested on every OS with a mapping of its own.
#[must_use]
pub fn spell_out_with(p: &str, mut long_of: impl FnMut(&str) -> Option<String>) -> String {
    let p = strip_verbatim(p);
    let root = root_len(&p);
    if root == 0 {
        return p;
    }
    // every prefix ending before a separator, longest first; the root alone is never asked (nothing to spell)
    let mut end = p.len();
    loop {
        let head = &p[..end];
        if head.len() > root
            && let Some(long) = long_of(head)
        {
            let tail = &p[end..];
            let long = if tail.is_empty() { long.as_str() } else { long.trim_end_matches(['/', '\\']) };
            return format!("{long}{tail}");
        }
        match p[..end].trim_end_matches(['/', '\\']).rfind(['/', '\\']) {
            Some(i) if i + 1 > root => end = i,
            _ => return p,
        }
    }
}

/// How long the root of an absolute path is (`/`, `C:\`, `\\srv\share`); 0 for a relative one.
fn root_len(p: &str) -> usize {
    let b = p.as_bytes();
    if p.starts_with(r"\\") || p.starts_with("//") {
        // the server and the share
        let mut seps = p.char_indices().skip(2).filter(|(_, c)| matches!(c, '/' | '\\')).map(|(i, _)| i);
        return match (seps.next(), seps.next()) {
            (Some(_), Some(end)) => end,
            _ => p.len(),
        };
    }
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\') {
        return 3;
    }
    usize::from(p.starts_with('/'))
}

/// [`spell_out_with`] by the file system: Windows asks it (`GetLongPathNameW`); Unix has no short names, so it only
/// loses a verbatim prefix there (no file-system call). A path that isn't UTF-8 is kept as it is. Blocking (one call
/// per missing part): for startup, tests and code already off the runtime.
#[must_use]
pub fn plain_path_blocking(p: impl AsRef<Path>) -> PathBuf {
    let p = p.as_ref();
    let Some(s) = p.to_str() else { return p.to_owned() };
    if cfg!(not(windows)) {
        return PathBuf::from(strip_verbatim(s));
    }
    PathBuf::from(spell_out_with(s, |head| crate::sys::long_path(Path::new(head)).and_then(|l| l.to_str().map(str::to_owned))))
}

/// [`plain_path_blocking`], without blocking a runtime thread.
pub async fn plain_path(p: impl AsRef<Path>) -> PathBuf {
    let p = p.as_ref().to_owned();
    if cfg!(not(windows)) {
        return plain_path_blocking(p);
    }
    let back = p.clone();
    tokio::task::spawn_blocking(move || plain_path_blocking(&p)).await.unwrap_or(back)
}

/// [`plain_path_blocking`] of a path given as text (a command's `cwd`, a spec's `path`), as text.
#[must_use]
pub fn plain_str_blocking(p: &str) -> String {
    plain_path_blocking(p).to_string_lossy().into_owned()
}

/// [`plain_str_blocking`], without blocking a runtime thread.
pub async fn plain_str(p: &str) -> String {
    plain_path(p).await.to_string_lossy().into_owned()
}

/// Whether `p` is `base` or inside it once both are spelled out by `long_of` ([`spell_out_with`]): by parts, either
/// separator matching the other, a Windows path (a drive, a UNC share) without regard to case as NTFS reads names.
/// `C:\Users\RUNNER~1\t\proj\src` is inside `C:\Users\runneradmin\t\proj`; `/a/bc` isn't inside `/a/b`, and a path with
/// `..` in it is inside nothing (resolve it first). Pure.
#[must_use]
pub fn within_with(p: &str, base: &str, mut long_of: impl FnMut(&str) -> Option<String>) -> bool {
    if p.trim().is_empty() || base.trim().is_empty() {
        return false;
    }
    let key = |x: String| {
        let windows = x.starts_with(r"\\") || x.as_bytes().get(1) == Some(&b':');
        let x = x.replace('\\', "/");
        if windows { x.to_lowercase() } else { x }
    };
    let p = key(spell_out_with(p, &mut long_of));
    let base = key(spell_out_with(base, &mut long_of));
    let parts = |x: &str| x.split('/').filter(|s| !s.is_empty() && *s != ".").map(str::to_owned).collect::<Vec<_>>();
    let (pp, bp) = (parts(&p), parts(&base));
    // a `..` could climb out of `base` with every part before it matching: never within (resolve it first)
    if pp.iter().chain(&bp).any(|x| x == "..") {
        return false;
    }
    p.starts_with('/') == base.starts_with('/') && pp.len() >= bp.len() && pp[..bp.len()] == bp[..]
}

/// [`within_with`] by the file system ([`plain_path_blocking`]'s spelling). Blocking.
#[must_use]
pub fn within_blocking(p: &str, base: &str) -> bool {
    if cfg!(not(windows)) {
        return within_with(p, base, |_| None);
    }
    within_with(p, base, |head| crate::sys::long_path(Path::new(head)).and_then(|l| l.to_str().map(str::to_owned)))
}
