//! Repo root resolution and `.wisp/wisp.yml` setting lookup.

use std::path::{Path, PathBuf};

use serde_json::Value;
use wisp_history_core::repo;
use wisp_kernel::{Ctx, Result, WireError};
use wisp_proc::Proc;
use wisp_runtime_api::model::Session;
use wisp_runtime_api::paths::Folder;

use crate::git;

/// The server's own wisp home (`~/.wisp` or `WISP_HOME`), when the paths service is registered.
pub(crate) fn wisp_home(ctx: &Ctx) -> Option<PathBuf> {
    wisp_kernel::services::paths(ctx).ok().map(|p| p.home().to_path_buf())
}

/// True when `dot_wisp` is wisp's own app data directory (`~/.wisp`), never a project's `.wisp/` history folder.
pub(crate) async fn is_app_home(dot_wisp: &Path, home: Option<&Path>) -> bool {
    if let Some(h) = home
        && (dot_wisp == h || wisp_proc::canonical(dot_wisp).await.ok().zip(wisp_proc::canonical(h).await.ok()).is_some_and(|(a, b)| a == b))
    {
        return true;
    }
    tokio::fs::metadata(dot_wisp.join("wisp.db")).await.is_ok()
        || tokio::fs::metadata(dot_wisp.join("server.json")).await.is_ok()
        || tokio::fs::metadata(dot_wisp.join("worktrees")).await.is_ok_and(|m| m.is_dir())
}

/// The repo a folder belongs to: the nearest folder up (within git's top, never crossing into `~/.wisp`)
/// with a project `.wisp/`, else git's top, else the folder.
pub(crate) async fn root_of(proc: &Proc, home: Option<&Path>, cwd: &str) -> PathBuf {
    let start = Path::new(cwd);
    let exists = tokio::fs::metadata(start).await.is_ok();
    let top =
        if exists { git::run(proc, "git", start, &["rev-parse", "--show-toplevel"]).await.map(|t| PathBuf::from(t.trim())) } else { None };
    if exists {
        for d in start.ancestors() {
            let dot = d.join(".wisp");
            if tokio::fs::metadata(&dot).await.is_ok_and(|m| m.is_dir()) && !is_app_home(&dot, home).await {
                let is_repo_wisp = tokio::fs::metadata(dot.join("wisp.yml")).await.is_ok()
                    || tokio::fs::metadata(dot.join("tasks")).await.is_ok_and(|m| m.is_dir())
                    || top.as_deref() == Some(d)
                    || top.is_none();
                if is_repo_wisp {
                    return d.to_owned();
                }
            }
            if top.as_deref() == Some(d) {
                break;
            }
        }
    }
    top.unwrap_or_else(|| start.to_owned())
}

/// Whether `task`'s ship slot says all its own PRs are already merged.
fn pr_merged(task: &Session) -> bool {
    let ship = task.ext_of("wisp.ship");
    let state = ship.pointer("/pr/state").and_then(Value::as_str);
    if state.is_some_and(|s| s.eq_ignore_ascii_case("merged")) {
        return true;
    }
    ship.get("repos").and_then(Value::as_array).is_some_and(|rs| {
        !rs.is_empty()
            && rs.iter().all(|r| r.pointer("/pr/state").and_then(Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case("merged")))
    })
}

/// True when `wt` has neither uncommitted edits nor commits ahead of `base_ref`.
async fn worktree_clean(proc: &Proc, wt: &Path, base_ref: &str) -> bool {
    let dirty = git::run(proc, "git", wt, &["status", "--porcelain"]).await.is_some_and(|s| !s.trim().is_empty());
    if dirty || base_ref.is_empty() {
        return false;
    }
    git::run(proc, "git", wt, &["rev-list", "--count", &format!("{base_ref}..HEAD")])
        .await
        .and_then(|s| s.trim().parse::<u32>().ok())
        .is_some_and(|n| n == 0)
}

/// The repo a task saves into when no `repo` arg is named: for a worktree task whose worktree is gone,
/// already merged, or has no unshipped changes, the main checkout (`ext["wisp.worktrees"].root`) so it
/// appears in Repo history right away instead of walking up into `~/.wisp` or landing in a dead worktree.
pub(crate) async fn root_of_task(ctx: &Ctx, proc: &Proc, task: &Session) -> Result<PathBuf> {
    let home = wisp_home(ctx);
    let wt = task.ext_of("wisp.worktrees");
    let wt_root = wt.get("root").and_then(Value::as_str).filter(|s| !s.is_empty());
    let wt_path = wt.get("path").and_then(Value::as_str).filter(|s| !s.is_empty());
    let base_ref = wt.get("baseRef").and_then(Value::as_str).unwrap_or_default();
    if let Some(main_root) = wt_root
        && tokio::fs::metadata(main_root).await.is_ok_and(|m| m.is_dir())
    {
        let wt_exists = wt_path.is_some_and(|p| Path::new(p).is_dir());
        if !wt_exists || pr_merged(task) {
            return Ok(root_of(proc, home.as_deref(), main_root).await);
        }
        if let Some(p) = wt_path
            && worktree_clean(proc, Path::new(p), base_ref).await
        {
            return Ok(root_of(proc, home.as_deref(), main_root).await);
        }
    }
    let dir =
        Folder::of(&task.cwd).map_err(|e| WireError::invalid(format!("task {} has {e}, so no repo: name one (repo)", task.id.as_str())))?;
    Ok(root_of(proc, home.as_deref(), &dir.to_string()).await)
}

/// A `history:` setting of the repo (`.wisp/wisp.yml`).
pub(crate) async fn setting(root: &Path, key: &str, default: &str) -> String {
    let text = tokio::fs::read_to_string(root.join(".wisp/wisp.yml")).await.unwrap_or_default();
    repo::history_setting(&text, key, default)
}
