//! Snapshotting `doc` blocks when a turn finishes (v1 `Store.keepDocs`): a markdown file the agent wrote
//! goes into the turn as it was then (`text`, `abs`, and `truncated` past [`MAX_KEPT_DOC`]), so it still
//! reads the same once the file changes, moves, its worktree goes, or the task is shared.
//! Sans-IO: the host reads the disk and checks symlinks.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::media::resolve;

/// The largest markdown document kept inline on a turn (v1 `MAX_KEPT_DOC`, 512 KiB).
pub const MAX_KEPT_DOC: u64 = 512 * 1024;

/// Extensions a `doc` block may read outside the task's folders (also used by Save to My Stuff).
pub const DOC_EXTS: [&str; 5] = ["md", "markdown", "mdx", "txt", "rst"];

/// Whether `p`'s extension is one of [`DOC_EXTS`] (case-insensitive).
#[must_use]
pub fn is_doc_ext(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| DOC_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Expands a leading `~/` or `~\` against `home`; returns `None` if `raw` does not start with a tilde prefix.
#[must_use]
pub fn expand_tilde(raw: &str, home: Option<&Path>) -> Option<PathBuf> {
    let rest = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))?;
    let home = home?;
    Some(resolve(home, rest))
}

/// Folds `.` and `..` components on an already-joined path without touching disk.
#[must_use]
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Candidate absolute paths for a `doc` block's `path` field, in lookup order (deduplicated).
#[must_use]
pub fn resolve_candidates(dirs: &[&Path], home: Option<&Path>, raw: &str) -> Vec<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if let Some(expanded) = expand_tilde(trimmed, home) {
        return vec![expanded];
    }
    let p = Path::new(trimmed);
    if p.is_absolute() {
        return vec![normalize(p)];
    }
    let mut out = Vec::new();
    for dir in dirs {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let cand = resolve(dir, trimmed);
        if !out.contains(&cand) {
            out.push(cand);
        }
    }
    out
}

/// One `doc` block whose file should be snapshotted into the turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocKeep {
    pub index: usize,
    pub candidates: Vec<PathBuf>,
}

/// Plans which `doc` blocks in `blocks` need their file read from disk: any `doc` block with a non-empty
/// `path` and no string `text` already present.
#[must_use]
pub fn doc_plan(dirs: &[&Path], home: Option<&Path>, blocks: &[Value]) -> Vec<DocKeep> {
    let mut out = Vec::new();
    for (index, b) in blocks.iter().enumerate() {
        if b.get("type").and_then(Value::as_str) != Some("doc") {
            continue;
        }
        if b.get("text").is_some_and(Value::is_string) {
            continue;
        }
        let Some(raw) = b.get("path").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        let candidates = resolve_candidates(dirs, home, raw);
        if !candidates.is_empty() {
            out.push(DocKeep { index, candidates });
        }
    }
    out
}

/// Converts a bounded byte prefix of a text document to UTF-8, rejecting binary files (NUL bytes) and
/// trimming an incomplete trailing UTF-8 character caused by byte-cap truncation.
#[must_use]
pub fn utf8_prefix(bytes: &[u8]) -> Option<String> {
    if bytes.contains(&0) {
        return None;
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => Some(s.to_owned()),
        Err(err) if err.error_len().is_none() => Some(String::from_utf8_lossy(&bytes[..err.valid_up_to()]).into_owned()),
        Err(_) => Some(String::from_utf8_lossy(bytes).into_owned()),
    }
}

/// Stamps a `doc` block with its snapshotted text, resolved path (`abs`), and `truncated` when cut.
pub fn kept_doc(block: &mut Value, text: String, abs: &Path, truncated: bool) {
    if let Some(o) = block.as_object_mut() {
        o.insert("text".into(), Value::String(text));
        o.insert("abs".into(), Value::String(abs.display().to_string()));
        if truncated {
            o.insert("truncated".into(), Value::Bool(true));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn doc_plan_resolves_relative_multi_dir_absolute_and_tilde_paths_and_skips_existing_text() {
        let dir = Path::new("/w/app");
        let extra = Path::new("/w/other");
        let home = Path::new("/Users/me");
        let blocks = [
            json!({ "type": "doc", "path": "docs/plan.md" }),
            json!({ "type": "doc", "path": "~/notes.md" }),
            json!({ "type": "doc", "path": "/tmp/report.md" }),
            json!({ "type": "doc", "path": "already.md", "text": "# Given" }),
            json!({ "type": "doc", "path": "empty.md", "text": "" }),
            json!({ "type": "doc", "path": "   " }),
            json!({ "type": "markdown", "path": "docs/plan.md" }),
        ];
        let plan = doc_plan(&[dir, extra, dir], Some(home), &blocks);
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].index, 0);
        assert_eq!(plan[0].candidates, vec![PathBuf::from("/w/app/docs/plan.md"), PathBuf::from("/w/other/docs/plan.md")]);
        assert_eq!(plan[1].index, 1);
        assert_eq!(plan[1].candidates, vec![PathBuf::from("/Users/me/notes.md")]);
        assert_eq!(plan[2].index, 2);
        assert_eq!(plan[2].candidates, vec![PathBuf::from("/tmp/report.md")]);
    }

    #[test]
    fn utf8_prefix_trims_cut_multibyte_tail_and_rejects_binary() {
        // "hi—" = [0x68, 0x69, 0xe2, 0x80, 0x94]; cutting after 4 bytes leaves an incomplete em-dash
        let raw = "hi—".as_bytes();
        assert_eq!(utf8_prefix(raw), Some("hi—".into()));
        assert_eq!(utf8_prefix(&raw[..4]), Some("hi".into()));
        assert_eq!(utf8_prefix(b""), Some(String::new()));
        assert_eq!(utf8_prefix(b"abc\0def"), None);
    }

    #[test]
    fn is_doc_ext_and_kept_doc_match_v1_shape() {
        for ok in ["a.md", "a.MD", "a.markdown", "a.mdx", "a.txt", "a.rst"] {
            assert!(is_doc_ext(Path::new(ok)), "{ok}");
        }
        for bad in ["id_rsa", ".env", "a.png", "a.rs", "a.json"] {
            assert!(!is_doc_ext(Path::new(bad)), "{bad}");
        }
        let mut b = json!({ "type": "doc", "path": "docs/plan.md" });
        kept_doc(&mut b, "# Plan".into(), Path::new("/w/app/docs/plan.md"), true);
        assert_eq!(b["text"], "# Plan");
        assert_eq!(b["abs"], "/w/app/docs/plan.md");
        assert_eq!(b["truncated"], true);
    }
}
