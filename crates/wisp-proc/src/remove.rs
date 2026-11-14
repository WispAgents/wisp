//! Removing a folder wisp owns (a carried copy, a sandbox seed, a worktree, an installed plugin), the one way
//! (2026-11-14). Git marks its pack and object files read-only, and on Windows a read-only file can't be deleted:
//! `remove_dir_all` of a checkout failed there with "Access is denied" and the copy stayed. So a first refusal clears
//! the read-only bit on everything inside (on Unix: makes each folder writable by its owner, which is what deleting
//! its entries needs) and tries once more. Links are never followed: only what is inside the folder is touched.

use std::io;
use std::path::{Path, PathBuf};

/// `dir` and everything in it gone, read-only files and folders included; gone already is fine. Blocking: for
/// startup, tests and code already off the runtime.
pub fn remove_tree_blocking(dir: impl AsRef<Path>) -> io::Result<()> {
    let dir = dir.as_ref();
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => {
            writable(dir);
            match std::fs::remove_dir_all(dir) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                r => r,
            }
        }
    }
}

/// [`remove_tree_blocking`], without blocking a runtime thread.
pub async fn remove_tree(dir: impl AsRef<Path>) -> io::Result<()> {
    let dir: PathBuf = dir.as_ref().to_owned();
    tokio::task::spawn_blocking(move || remove_tree_blocking(dir)).await.map_err(io::Error::other)?
}

/// Every entry under `dir` (and `dir`) made deletable; best effort, links left alone.
fn writable(dir: &Path) {
    let mut todo = vec![dir.to_owned()];
    while let Some(d) = todo.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&d) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        unlock(&d, &meta);
        if !meta.is_dir() {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        todo.extend(rd.flatten().map(|e| e.path()));
    }
}

#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)] // Windows: the read-only attribute, nothing world-writable
fn unlock(p: &Path, meta: &std::fs::Metadata) {
    let mut perm = meta.permissions();
    if perm.readonly() {
        perm.set_readonly(false);
        let _ = std::fs::set_permissions(p, perm);
    }
}

#[cfg(not(windows))]
fn unlock(p: &Path, meta: &std::fs::Metadata) {
    use std::os::unix::fs::PermissionsExt;
    // a folder's entries go when it's writable (and searchable) by its owner; a file's own bits don't matter
    if meta.is_dir() {
        let mode = meta.permissions().mode();
        if mode & 0o700 != 0o700 {
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode | 0o700));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wisp-proc-rm-{name}-{}", std::process::id()));
        let _ = remove_tree_blocking(&d);
        d
    }

    /// `serve_delegate_machine.rs` on Windows: a carried copy's `.git` holds read-only pack files (and, as git leaves
    /// them on Unix, read-only object folders); the copy must still go, the folder beside it untouched.
    #[test]
    fn a_checkout_with_read_only_files_and_folders_is_removed() {
        let root = tmp("ro");
        let dir = root.join("copy");
        let pack = dir.join(".git/objects/pack");
        std::fs::create_dir_all(&pack).unwrap();
        let file = pack.join("pack-1.pack");
        std::fs::write(&file, b"x").unwrap();
        let keep = root.join("other");
        std::fs::create_dir_all(&keep).unwrap();
        let mut ro = std::fs::metadata(&file).unwrap().permissions();
        ro.set_readonly(true);
        std::fs::set_permissions(&file, ro.clone()).unwrap();
        // a read-only folder: what makes the plain removal fail on Unix (as a read-only file does on Windows)
        std::fs::set_permissions(&pack, ro).unwrap();
        remove_tree_blocking(&dir).unwrap();
        assert!(!dir.exists(), "the copy went");
        assert!(keep.exists(), "only that folder");
        remove_tree_blocking(&dir).unwrap();
        remove_tree_blocking(&root).unwrap();
    }

    #[tokio::test]
    async fn gone_already_is_fine_and_a_link_out_is_not_followed() {
        let root = tmp("link");
        let outside = root.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("f"), b"x").unwrap();
        let dir = root.join("copy");
        std::fs::create_dir_all(&dir).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, dir.join("out")).unwrap();
        remove_tree(&dir).await.unwrap();
        assert!(!dir.exists());
        assert!(outside.join("f").exists(), "what a link points at stays");
        remove_tree(&dir).await.unwrap();
        remove_tree(&root).await.unwrap();
    }
}
