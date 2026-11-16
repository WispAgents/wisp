//! A plugin's folder replaced or removed all or nothing (r310): the old folder moved aside into `<plugins>/.old/`,
//! the new one moved in, the old one deleted (or at the next start). On Windows a folder in use can't be moved: then
//! nothing changes and the old plugin goes back into the kernel as it was.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use wisp_kernel::{Ctx, Manifest, PluginStatus, Result, WireError};

use crate::install::{MANIFEST, TRUSTED, read};

/// Where a replaced or removed plugin's folder waits to be deleted: `<plugins>/.old/` (hidden, so never loaded),
/// tidied at the next start when something still held it (`tidy_blocking`).
pub(crate) const OLD: &str = ".old";

/// `dir` moved out of the way into `<plugins>/.old/` (`None`: nothing there). Another folder on purpose: a folder
/// that can't be moved fails here on Unix too (one you can't write), as one in use does on Windows.
pub(crate) async fn set_aside(root: &Path, dir: &Path) -> std::io::Result<Option<PathBuf>> {
    if tokio::fs::symlink_metadata(dir).await.is_err() {
        return Ok(None);
    }
    let old = root.join(OLD);
    tokio::fs::create_dir_all(&old).await?;
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let aside = old.join(format!("{name}.{}.{}", std::process::id(), wisp_kernel::ids::uuid7()));
    // Windows lets go of a program that just exited a moment later (its image, an antivirus scan): a few tries
    let mut wait = 50;
    loop {
        match tokio::fs::rename(dir, &aside).await {
            Ok(()) => return Ok(Some(aside)),
            Err(e) if cfg!(windows) && wait <= 800 && in_use(&e) => {
                tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
                wait *= 2;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Access denied (5) or a sharing violation (32): something still holds a file in the folder.
fn in_use(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32)
}

/// A plugin that counts as on: running, or on and waiting for its first event.
pub(crate) fn is_on(state: &str) -> bool {
    matches!(state, "active" | "resolved")
}

/// At start (blocking is fine there): each folder left in `.old/` deleted, or, when a replace couldn't put it back
/// then (its `<name>.keep` mark beside it), put back where it was if nothing took its place.
pub(crate) fn tidy_blocking(plugins: &Path) {
    let old = plugins.join(OLD);
    let Ok(rd) = std::fs::read_dir(&old) else { return };
    let (dirs, marks): (Vec<PathBuf>, Vec<PathBuf>) = rd.flatten().map(|e| e.path()).partition(|p| p.is_dir());
    for p in dirs {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // `<slug>.<pid>.<uuid>`; a slug has no dots
        let home = name.split('.').next().filter(|s| !s.is_empty()).map(|s| plugins.join(s));
        let keep = marks.iter().any(|m| m.file_name().is_some_and(|n| n.to_string_lossy() == format!("{name}.keep")));
        if keep && let Some(h) = home.filter(|h| !h.exists()) {
            match std::fs::rename(&p, &h) {
                Ok(()) => tracing::warn!(folder = %h.display(), "plugins: a plugin a replace couldn't put back is in place again"),
                Err(err) => tracing::warn!(folder = %p.display(), "plugins: a plugin set aside couldn't go back: {err}"),
            }
            continue;
        }
        if let Err(err) = wisp_proc::remove_tree_blocking(&p) {
            tracing::warn!(folder = %p.display(), "plugins: an old plugin folder stays: {err}");
        }
    }
    for m in marks {
        let _ = std::fs::remove_file(m);
    }
    let _ = std::fs::remove_dir(&old);
}

/// `staging` in place of `dir`, all or nothing: the old folder moved aside, the new one moved in, the old one
/// deleted. On an error `dir` is as it was (the old folder back in place).
pub(crate) async fn swap(root: &Path, staging: &Path, dir: &Path) -> std::result::Result<(), String> {
    let aside = set_aside(root, dir).await.map_err(|e| format!("its folder {} couldn't be moved aside ({e})", dir.display()))?;
    if let Err(e) = tokio::fs::rename(staging, dir).await {
        if let Some(a) = &aside
            && let Err(back) = tokio::fs::rename(a, dir).await
        {
            // the next start puts it back instead of deleting it
            let _ = tokio::fs::write(format!("{}.keep", a.display()), b"").await;
            return Err(format!(
                "the new one couldn't be moved in ({e}) and the old one couldn't be put back ({back}): it's in {} until wisp next starts",
                a.display()
            ));
        }
        return Err(format!("the new one couldn't be moved into {} ({e})", dir.display()));
    }
    discard(aside).await;
    Ok(())
}

/// A folder set aside deleted, best effort: what is left is deleted at the next start.
pub(crate) async fn discard(aside: Option<PathBuf>) {
    let Some(a) = aside else { return };
    if let Err(e) = wisp_proc::remove_tree(&a).await {
        tracing::warn!(folder = %a.display(), "plugins: an old plugin folder stays until the next start: {e}");
    }
}

/// The plugin in `dir` back in the kernel as it was: its trust mark, and on if it was.
pub(crate) async fn restore(ctx: &Ctx, dir: &Path, id: &str, on: bool) -> Result<()> {
    let text = tokio::fs::read_to_string(dir.join(MANIFEST)).await.map_err(|e| WireError::internal(format!("its {MANIFEST}: {e}")))?;
    let trusted = tokio::fs::metadata(dir.join(TRUSTED)).await.is_ok_and(|m| m.is_file());
    ctx.install_plugin(Arc::new(read(dir, &text, trusted)?)).await?;
    if on {
        ctx.set_plugin_enabled(id, true).await?;
    }
    Ok(())
}

/// The error for a replace that changed nothing, with the old plugin (if there was one) put back as it was.
pub(crate) async fn not_replaced(ctx: &Ctx, dir: &Path, was: Option<PluginStatus>, m: &Manifest, why: &str) -> WireError {
    let Some(old) = was else { return WireError::unavailable(format!("Couldn't install {}: {why}.", m.id)) };
    let on = is_on(&old.state);
    if !tokio::fs::metadata(dir.join(MANIFEST)).await.is_ok_and(|m| m.is_file()) {
        return WireError::unavailable(format!(
            "Couldn't replace {} {} with {}: {why}; it goes back in place when wisp next starts.",
            old.id, old.version, m.version
        ));
    }
    let kept = match restore(ctx, dir, &old.id, on).await {
        Ok(()) => format!("{} {} is still installed{}", old.id, old.version, if on { " and on" } else { "" }),
        Err(e) => format!("{} {} is still in {} but didn't load again: {}", old.id, old.version, dir.display(), e.message),
    };
    WireError::unavailable(format!(
        "Couldn't replace {} {} with {}: {why}. {kept}. Close what is using its folder (a program it started, an editor) and install again.",
        old.id, old.version, m.version
    ))
}
