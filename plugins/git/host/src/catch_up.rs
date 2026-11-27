//! `Repo::catch_up`: before Ship's push, the commits wisp's history push put on the task's branch (a save's `.wisp/`
//! files, built on the branch's remote tip) merged into HEAD, so the push is a fast-forward and never needs a force.
//! Only those: each commit has the history push's subject and touches nothing but what a save writes (`within`, and `also` beside the folder).
//! Anything else only the remote has (someone else's push, a `.wisp/worktree.json` whose `setup` would run here) is
//! left for the user: nothing is merged or moved.

use std::path::{Path, PathBuf};

use wisp_git_api::{CatchUpRequest, CaughtUp};

use crate::error::Result;
use crate::repo::Inner;
use crate::run::{Git, Opts, args};

/// Whether `path` (from git's top) is in a folder named `folder`, at the top or in a subfolder (a monorepo's
/// `app/.wisp/`), and there inside one of its subfolders `within` (anywhere in it when `within` is empty).
pub(crate) fn in_folder(path: &str, folder: &str, within: &[String]) -> bool {
    let folder = folder.trim_matches('/');
    let parts: Vec<&str> = path.split('/').collect();
    let inside = |i: usize| match parts.get(i + 1) {
        None => false,
        Some(_) if within.is_empty() => true,
        // a file named like the subfolder (`.wisp/tasks`) is not in it
        Some(sub) => i + 2 < parts.len() && within.iter().any(|w| w.trim_matches('/') == *sub),
    };
    !folder.is_empty() && parts.iter().enumerate().any(|(i, part)| *part == folder && inside(i))
}

/// Whether a commit's subject is the history push's own (`start…end`; both empty match anything).
pub(crate) fn is_history_subject(subject: &str, start: &str, end: &str) -> bool {
    subject.len() >= start.len() + end.len() && subject.starts_with(start) && subject.ends_with(end)
}

/// Whether `path` is one of the files a save writes beside its folder (`req.also`: `.gitattributes`, `app/.gitattributes`),
/// and `paths` (the same commit's) write inside that very folder: a `src/.gitattributes` with no `src/.wisp/` beside it,
/// or one inside `.wisp/`, is not the save's.
fn beside_the_folder(path: &str, req: &CatchUpRequest, paths: &[&str]) -> bool {
    let only = req.only.trim_matches('/');
    req.also.iter().map(|n| n.trim_matches('/')).filter(|n| !n.is_empty() && !only.is_empty()).any(|name| {
        let prefix = match path.strip_suffix(name) {
            Some("") => "",
            Some(p) if p.ends_with('/') => p,
            _ => return false,
        };
        let root = format!("{prefix}{only}/");
        paths.iter().any(|q| q.starts_with(&root) && in_folder(q, only, &req.within))
    })
}

/// The paths `HEAD..theirs` touches, and what makes it more than the history: every path of a commit that isn't a
/// history push (its subject when it has none, a merge), and any path outside what a save writes. `log` is
/// `log --format=%x00%s --name-only`.
pub(crate) fn sort_out(log: &str, req: &CatchUpRequest) -> (Vec<String>, Vec<String>) {
    let (mut touched, mut outside) = (Vec::new(), Vec::new());
    for commit in log.split('\0').skip(1) {
        let mut lines = commit.lines();
        let subject = lines.next().unwrap_or_default();
        let paths: Vec<&str> = lines.filter(|l| !l.is_empty()).collect();
        let saved = is_history_subject(subject, &req.subject_start, &req.subject_end);
        if !saved && paths.is_empty() {
            outside.push(subject.to_owned());
        }
        // a save's `.gitattributes` (the LFS line) goes with it, never on its own: a conflict on it stays the user's
        let theirs = |p: &&&str| !saved || !(in_folder(p, &req.only, &req.within) || beside_the_folder(p, req, &paths));
        outside.extend(paths.iter().filter(theirs).map(|p| (*p).to_owned()));
        touched.extend(paths.into_iter().map(str::to_owned));
    }
    for v in [&mut touched, &mut outside] {
        v.sort();
        v.dedup();
    }
    (touched, outside)
}

/// `:(literal)path` each, so a file name with `*` or `:` in it is only itself.
fn literal(paths: &[String]) -> Vec<String> {
    paths.iter().map(|p| format!(":(literal){p}")).collect()
}

/// One turn at a worktree's aside folder at a time in this process: a `catch_up` (from its leftovers' restore to its
/// own put back) and the restore a repo's open or a worktree's removal runs (`restore_aside`). Without it, an open's
/// background restore could check a leftover while a `catch_up` had the worktree's copy set aside, put the older
/// leftover back mid-merge, and the merge's own put back then replaced it (final7 review).
async fn turn(aside: &Path) -> tokio::sync::OwnedMutexGuard<()> {
    static TURNS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<PathBuf, std::sync::Arc<tokio::sync::Mutex<()>>>>> =
        std::sync::OnceLock::new();
    let lock = {
        let mut map = TURNS.get_or_init(Default::default).lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        std::sync::Arc::clone(map.entry(aside.to_path_buf()).or_default())
    };
    lock.lock_owned().await
}

impl Inner {
    pub async fn catch_up(&self, req: CatchUpRequest) -> Result<CaughtUp> {
        let top = PathBuf::from(&self.info.top);
        let aside = self.aside_dir().await;
        let _turn = match &aside {
            Some(aside) => Some(turn(aside).await),
            None => None,
        };
        if let Some(aside) = &aside {
            restore_leftovers(&self.git, aside, &top, false).await;
        }
        let theirs = format!("refs/remotes/{}/{}", req.remote, req.branch);
        // the tracking ref named outright: a worktree's remote may have no fetch refspec that would move it
        let spec = format!("+refs/heads/{}:{theirs}", req.branch);
        let fetched = self.git.exec(&self.dir, &args(&["fetch", "--quiet", &req.remote, &spec]), Opts::timeout(30)).await?;
        if !fetched.exit.success() {
            // not on the remote yet (the first ship) is Even; anything else git can't tell
            let listed = self
                .git
                .exec(&self.dir, &args(&["ls-remote", "--heads", &req.remote, &format!("refs/heads/{}", req.branch)]), Opts::timeout(30))
                .await?;
            return Ok(if listed.exit.success() && listed.stdout_text().trim().is_empty() { CaughtUp::Even } else { CaughtUp::Unknown });
        }
        self.notify("fetched");
        if self.is_ancestor(&theirs, "HEAD").await? {
            return Ok(CaughtUp::Even);
        }
        // every commit's subject and own paths (no renames: a move out of the folder counts its old path too)
        let range = format!("HEAD..{theirs}");
        let log = self.git.read(&self.dir, &["log", "--no-renames", "--format=%x00%s", "--name-only", &range]).await?;
        let (touched, outside) = sort_out(&log, &req);
        if !outside.is_empty() {
            return Ok(CaughtUp::Diverged { files: outside });
        }
        // a newer save not committed here (`.wisp/` untracked, or ignored: git overwrites those in a merge without a
        // word) is set aside and put back after, whatever the merge did: it stays the pending change the next
        // history push sends, never replaced by the older copy the remote has
        let kept = self.set_aside(aside.as_deref(), &top, &touched).await;
        // `merge_history`'s own errors come back as `out`, so the saves go back whatever it did (but never over one
        // history wrote meanwhile: its `on_done` takes no turn here, and a fetch and merge can take minutes); a quit or a dropped
        // future in between leaves them in the git dir, put back by the next `catch_up` above
        let out = self.merge_history(&req, &theirs, &top).await;
        if let Some(batch) = &kept {
            put_back(&self.git, batch, &top, Back::AfterMerge).await;
            if let Some(aside) = &aside {
                // only when empty: a batch that couldn't all go back stays for the next `catch_up`
                let _ = tokio::fs::remove_dir(aside).await;
            }
        }
        out
    }

    /// Where saves are set aside during a merge: `<git dir>/wisp-aside/` (this worktree's own, so another worktree's
    /// leftovers never land here); None when git can't say.
    async fn aside_dir(&self) -> Option<PathBuf> {
        let git_dir = self.git.quiet(&self.dir, &["rev-parse", "--absolute-git-dir"]).await;
        let git_dir = git_dir.trim();
        (!git_dir.is_empty()).then(|| Path::new(git_dir).join("wisp-aside"))
    }

    /// The untracked or ignored files among `paths` (from git's top), moved into a new folder in `aside` at the same
    /// paths, so the merge neither refuses ("would be overwritten") nor silently replaces them, and a crash mid-merge
    /// still leaves them on disk. The folder, when anything moved.
    async fn set_aside(&self, aside: Option<&Path>, top: &Path, paths: &[String]) -> Option<PathBuf> {
        let aside = aside?;
        if paths.is_empty() {
            return None;
        }
        let specs = literal(paths);
        let mut a = vec!["ls-files", "-o", "-z", "--"];
        a.extend(specs.iter().map(String::as_str));
        let listed = self.git.quiet(top, &a).await;
        let listed: Vec<&str> = listed.split('\0').filter(|p| !p.is_empty()).collect();
        if listed.is_empty() {
            return None;
        }
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let batch = aside.join(format!("{}-{nanos}", std::process::id()));
        for path in listed {
            // one that won't move stays where it is: the merge refuses over it, as it would have before
            if let Err(e) = move_file(&top.join(path), &batch.join(path)).await {
                tracing::warn!(path, "catch_up: a local save couldn't be set aside: {e}");
            }
        }
        Some(batch)
    }

    /// The merge itself: done, done with HEAD's side of the history files both changed, or undone.
    async fn merge_history(&self, req: &CatchUpRequest, theirs: &str, top: &Path) -> Result<CaughtUp> {
        let message = format!("wisp: merge the history saved to {}", req.branch);
        let merged = self
            .git
            .exec(&self.dir, &args(&["merge", "--quiet", "--no-edit", "--no-verify", "-m", &message, theirs]), Opts::timeout(120))
            .await?;
        if merged.exit.success() {
            self.notify("head");
            return Ok(CaughtUp::Merged);
        }
        let conflicted = self.git.quiet(&self.dir, &["diff", "--name-only", "--diff-filter=U"]).await;
        let files: Vec<String> = conflicted.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
        let outside: Vec<String> = files.iter().filter(|f| !in_folder(f, &req.only, &req.within)).cloned().collect();
        // the same history file saved both ways: HEAD's is the newer save (the remote's was pushed from an earlier
        // one), so it wins and goes out with the next history push. Anything else: back to where it was, the user decides
        if !files.is_empty() && outside.is_empty() && self.take_ours(top, &files, &message).await {
            self.notify("head");
            return Ok(CaughtUp::Merged);
        }
        let _ = self.git.exec(&self.dir, &args(&["merge", "--abort"]), Opts::default()).await;
        Ok(match (files.is_empty(), outside.is_empty()) {
            (true, _) => CaughtUp::Unknown,
            (false, true) => CaughtUp::Diverged { files },
            (false, false) => CaughtUp::Diverged { files: outside },
        })
    }

    /// The conflicted `files` resolved to HEAD's side (gone, when HEAD took the file out) and the merge committed.
    async fn take_ours(&self, top: &Path, files: &[String], message: &str) -> bool {
        for f in literal(files) {
            let ours = self.git.exec(top, &args(&["checkout", "--ours", "--", &f]), Opts::default()).await;
            // `-f`: `.wisp/` may be gitignored, and these are tracked already
            let step: &[&str] = if ours.is_ok_and(|r| r.exit.success()) { &["add", "-f", "--", &f] } else { &["rm", "-q", "-f", "--", &f] };
            if !self.git.exec(top, &args(step), Opts::default()).await.is_ok_and(|r| r.exit.success()) {
                return false;
            }
        }
        let done = self.git.exec(&self.dir, &args(&["commit", "--quiet", "--no-verify", "-m", message]), Opts::timeout(60)).await;
        done.is_ok_and(|r| r.exit.success())
    }
}

/// The saves an interrupted `catch_up` left set aside in `dir`'s worktree put back (`restore_leftovers`), when it has
/// any. Some(the aside folder) when something is still there (a file that wouldn't move or a folder that couldn't be
/// read: only deleting it or another try clears it), else None.
/// One `rev-parse` and one `stat` when nothing is aside, so opening a repo can afford it.
pub(crate) async fn restore_aside(git: &Git, dir: &Path, skip_own: bool) -> Option<PathBuf> {
    let found = git.quiet(dir, &["rev-parse", "--absolute-git-dir", "--show-toplevel"]).await;
    let mut lines = found.lines().map(str::trim);
    let (git_dir, top) = (lines.next()?, lines.next()?);
    if git_dir.is_empty() || top.is_empty() {
        return None;
    }
    let aside = Path::new(git_dir).join("wisp-aside");
    if !tokio::fs::try_exists(&aside).await.unwrap_or(false) {
        return None;
    }
    let _turn = turn(&aside).await;
    restore_leftovers(git, &aside, Path::new(top), skip_own).await.then_some(aside)
}

/// Saves an earlier `catch_up` set aside and never put back (wisp quit or crashed mid-merge), each back in the worktree
/// only where that can't write over a newer save made since (`newer_here`): there the leftover is an older save of the
/// same file, dropped with a warning (`put_back`). `skip_own`: this process's batches are left alone (a `catch_up` here
/// may be mid-merge with them). True when anything is still aside (only what wouldn't move).
async fn restore_leftovers(git: &Git, aside: &Path, top: &Path, skip_own: bool) -> bool {
    let own = format!("{}-", std::process::id());
    if let Ok(mut entries) = tokio::fs::read_dir(aside).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !entry.file_type().await.is_ok_and(|t| t.is_dir()) || (skip_own && name.starts_with(&own)) {
                continue;
            }
            put_back(git, &entry.path(), top, Back::Leftover).await;
        }
    }
    // only when empty: a batch with a file that wouldn't move stays
    let _ = tokio::fs::remove_dir(aside).await;
    tokio::fs::try_exists(aside).await.unwrap_or(true)
}

/// When `batch` was set aside: its name is `<pid>-<nanos>`. A name that doesn't say is the epoch: only a missing file
/// goes back.
fn set_aside_at(batch: &Path) -> u128 {
    let name = batch.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    name.split_once('-').and_then(|(_, n)| n.parse::<u128>().ok()).unwrap_or(0)
}

/// Which batch goes back: one an interrupted run left, or a `catch_up`'s own right after its merge.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Back {
    Leftover,
    AfterMerge,
}

/// Whether the worktree's `rel` (from git's top) is newer than a copy set aside at `since` (ns since the epoch), so the
/// copy must not replace it. Missing, or not a file (the move fails and the batch stays, the user's to clear): no. The
/// same as HEAD's: the merge's copy, so no; for a leftover, unless the last commit that wrote it is from after the
/// batch (a save committed since: yes). Anything else is a save only when written at or after `since` (history's
/// `track:auto` re-saves `.wisp/` after every turn, mid-merge too); a leftover's tracked and edited copy always is.
async fn newer_here(git: &Git, top: &Path, rel: &str, since: u128, back: Back) -> bool {
    let Ok(meta) = tokio::fs::metadata(top.join(rel)).await else { return false };
    if !meta.is_file() {
        return false;
    }
    let spec = format!(":(literal){rel}");
    let at_head = git.quiet(top, &["rev-parse", "-q", "--verify", &format!("HEAD:{rel}")]).await;
    let at_head = at_head.trim();
    if !at_head.is_empty() && git.quiet(top, &["hash-object", "--", rel]).await.trim() == at_head {
        if back == Back::AfterMerge {
            return false;
        }
        // seconds against the batch's own second: the merge commits after the set-aside, but what it brought is older
        let when = git.quiet(top, &["log", "-1", "--format=%ct", "HEAD", "--", &spec]).await;
        return when.trim().parse::<u128>().map_or(true, |s| s > since / 1_000_000_000);
    }
    if back == Back::Leftover && !git.quiet(top, &["ls-files", "-z", "--", &spec]).await.is_empty() {
        return true;
    }
    let written = meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map_or(u128::MAX, |d| d.as_nanos());
    written >= since
}

/// `from` moved to `to` (its folder made, `to` replaced): a rename, or a copy and a remove when the two are on
/// different devices. `from` stays until its copy is whole.
async fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(dir) = to.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    if tokio::fs::rename(from, to).await.is_ok() {
        return Ok(());
    }
    tokio::fs::copy(from, to).await?;
    tokio::fs::remove_file(from).await
}

/// Every file in `batch` (set aside before a merge, at its path from git's top) moved back under `top`, over what the
/// merge wrote (its folder made again if the merge took it), and `batch` removed. Where the worktree has a newer copy
/// (`newer_here`: a save written since), the batch's is an older save of the same file: dropped, with one warning, so
/// the batch still empties and nothing is left to keep the worktree from being removed. Only a file that won't move
/// (or a folder that can't be read) keeps the batch, so the next `catch_up` tries again and nothing is lost.
async fn put_back(git: &Git, batch: &Path, top: &Path, back: Back) {
    let since = set_aside_at(batch);
    let mut whole = true;
    let mut dirs = vec![batch.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
            whole = false;
            continue;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if entry.file_type().await.is_ok_and(|t| t.is_dir()) {
                dirs.push(path);
                continue;
            }
            let Ok(rel) = path.strip_prefix(batch) else { continue };
            let rel = rel.to_string_lossy().replace('\\', "/");
            if newer_here(git, top, &rel, since, back).await {
                tracing::warn!(file = %rel, batch = %batch.display(), "catch_up: a set-aside save is older than the worktree's copy: the worktree's kept");
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    whole = false;
                    tracing::warn!(file = %rel, "catch_up: an older set-aside save couldn't be removed: {e}");
                }
                continue;
            }
            if let Err(e) = move_file(&path, &top.join(&rel)).await {
                whole = false;
                tracing::warn!(file = %rel, "catch_up: a local save couldn't be put back: {e}");
            }
        }
    }
    if whole {
        let _ = tokio::fs::remove_dir_all(batch).await;
    }
}

#[cfg(test)]
mod tests {
    use wisp_git_api::CatchUpRequest;

    use std::path::{Path, PathBuf};

    use super::{Back, in_folder, is_history_subject, put_back, restore_aside, sort_out, turn};

    /// A fresh repo with one commit, removed when dropped: its top (canonical) and its git dir.
    struct Repo(PathBuf);

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn git(dir: &Path, a: &[&str]) -> String {
        let out = std::process::Command::new("git").args(a).current_dir(dir).env("GIT_CONFIG_NOSYSTEM", "1").output().unwrap();
        assert!(out.status.success(), "git {a:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    fn write(dir: &Path, f: &str, text: &str) {
        let p = dir.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn repo(name: &str) -> (Repo, PathBuf) {
        let d = std::env::temp_dir().join(format!("wisp-catch-up-{name}-{}-{}", std::process::id(), now()));
        std::fs::create_dir_all(&d).unwrap();
        let d = wisp_proc::canonical_blocking(&d).unwrap();
        git(&d, &["init", "-q", "-b", "main"]);
        for (k, v) in [("user.email", "qa@wisp.test"), ("user.name", "QA"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")] {
            git(&d, &["config", k, v]);
        }
        write(&d, "a.txt", "a\n");
        git(&d, &["add", "a.txt"]);
        git(&d, &["commit", "-q", "-m", "init"]);
        let git_dir = PathBuf::from(git(&d, &["rev-parse", "--absolute-git-dir"]));
        (Repo(d), git_dir)
    }

    fn now() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    fn runner() -> std::sync::Arc<crate::run::Git> {
        crate::run::Git::new(wisp_proc::local(), None)
    }

    /// final8 F1: a leftover kept back because the worktree's copy is newer is an older save of that file: it's
    /// dropped, the batch empties and goes, and nothing is left aside to refuse the worktree's removal over.
    #[tokio::test]
    async fn a_leftover_older_than_the_worktree_s_copy_is_dropped_and_its_batch_goes() {
        let (r, git_dir) = repo("leftover-older");
        let aside = git_dir.join("wisp-aside");
        write(&aside.join(format!("{}-2", std::process::id())), ".wisp/x.json", "OLD\n");
        write(&r.0, ".wisp/x.json", "NEWER\n");
        write(&aside.join("1-2"), ".wisp/gone.json", "BACK\n");
        assert_eq!(restore_aside(&runner(), &r.0, false).await, None);
        assert_eq!(std::fs::read_to_string(r.0.join(".wisp/x.json")).unwrap(), "NEWER\n");
        assert_eq!(std::fs::read_to_string(r.0.join(".wisp/gone.json")).unwrap(), "BACK\n", "a missing file still goes back");
        assert!(!aside.exists(), "nothing left aside");
        assert_eq!(restore_aside(&runner(), &r.0, false).await, None, "and nothing to warn about again");
    }

    /// final8 F2: history saves `.wisp/` with no turn shared with `catch_up`, so a save can land while the merge runs:
    /// the same run's put back keeps that newer save, drops its older set-aside copy, and leaves no batch.
    #[tokio::test]
    async fn the_same_run_s_put_back_keeps_a_save_written_during_the_merge() {
        let (r, git_dir) = repo("after-merge-newer");
        let batch = git_dir.join(format!("wisp-aside/{}-{}", std::process::id(), now()));
        write(&batch, ".wisp/x.json", "SET-ASIDE\n");
        write(&batch, ".wisp/missing.json", "BACK\n");
        // past the clock's coarse tick, so the save's mtime is after the set-aside on every file system
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        write(&r.0, ".wisp/x.json", "SAVED-MID-MERGE\n");
        put_back(&runner(), &batch, &r.0, Back::AfterMerge).await;
        assert_eq!(std::fs::read_to_string(r.0.join(".wisp/x.json")).unwrap(), "SAVED-MID-MERGE\n");
        assert_eq!(std::fs::read_to_string(r.0.join(".wisp/missing.json")).unwrap(), "BACK\n");
        assert!(!batch.exists(), "the batch is gone");
    }

    /// final8 F2: what the merge itself wrote (HEAD's blob) is still replaced by the set-aside save, the newer local one.
    #[tokio::test]
    async fn the_same_run_s_put_back_still_goes_over_what_the_merge_wrote() {
        let (r, git_dir) = repo("after-merge-merged");
        let batch = git_dir.join(format!("wisp-aside/{}-{}", std::process::id(), now()));
        write(&batch, ".wisp/x.json", "LOCAL\n");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        write(&r.0, ".wisp/x.json", "MERGED\n");
        git(&r.0, &["add", "-f", ".wisp/x.json"]);
        git(&r.0, &["commit", "-q", "-m", "the merge"]);
        put_back(&runner(), &batch, &r.0, Back::AfterMerge).await;
        assert_eq!(std::fs::read_to_string(r.0.join(".wisp/x.json")).unwrap(), "LOCAL\n");
        assert!(!batch.exists());
    }

    #[tokio::test]
    async fn one_turn_at_an_aside_folder_at_a_time() {
        let (a, b) = (std::path::Path::new("/tmp/wisp-aside-a"), std::path::Path::new("/tmp/wisp-aside-b"));
        let held = turn(a).await;
        // another folder's turn goes at once; the same folder's waits for the first to end
        assert!(tokio::time::timeout(std::time::Duration::from_millis(50), turn(b)).await.is_ok());
        assert!(tokio::time::timeout(std::time::Duration::from_millis(50), turn(a)).await.is_err());
        drop(held);
        assert!(tokio::time::timeout(std::time::Duration::from_millis(500), turn(a)).await.is_ok());
    }

    const START: &str = "wisp: save task \"";
    const END: &str = "\" to the repo's history";

    fn within() -> Vec<String> {
        vec!["tasks".into(), "history".into()]
    }

    #[test]
    fn only_paths_inside_the_folder_count() {
        assert!(in_folder(".wisp/tasks/a.task.yml", ".wisp", &[]));
        assert!(in_folder("app/.wisp/tasks/a.task.yml", ".wisp/", &[]));
        assert!(!in_folder("src/main.rs", ".wisp", &[]));
        assert!(!in_folder(".wisp", ".wisp", &[]), "a file named like the folder is not in it");
        assert!(!in_folder(".wispy/a", ".wisp", &[]));
        assert!(!in_folder(".wisp/a", "", &[]));
    }

    /// final5 F3: only what a history save writes; the files that run code or steer wisp never merge in on their own.
    #[test]
    fn only_what_a_save_writes_counts_within() {
        let w = within();
        assert!(in_folder(".wisp/tasks/a.task.yml", ".wisp", &w));
        assert!(in_folder(".wisp/tasks/2026/11/x.report.md", ".wisp", &w));
        assert!(in_folder("app/.wisp/tasks/2026/11/x.task.yml", ".wisp", &w));
        assert!(in_folder(".wisp/history/x.jsonl.zst", ".wisp", &w));
        for p in [".wisp/worktree.json", ".wisp/sandbox.sh", ".wisp/wisp.yml", ".wisp/tasks", "app/.wisp/hooks/x", "tasks/a.task.yml"] {
            assert!(!in_folder(p, ".wisp", &w), "{p}");
        }
    }

    #[test]
    fn a_history_subject_is_the_push_s_own() {
        assert!(is_history_subject("wisp: save task \"Fix it\" to the repo's history", START, END));
        assert!(!is_history_subject("history", START, END));
        assert!(!is_history_subject("wisp: save task \"x\" to the repo's history, and its setup", START, END));
        assert!(!is_history_subject("wisp: save task \"", START, "\"\""), "the start and the end don't overlap");
        assert!(is_history_subject("anything", "", ""));
    }

    #[test]
    fn a_commit_that_isnt_a_history_push_is_refused_with_its_files() {
        let req = CatchUpRequest {
            only: ".wisp".into(),
            within: within(),
            subject_start: START.into(),
            subject_end: END.into(),
            ..CatchUpRequest::default()
        };
        let save = format!("{START}t{END}");
        let log = format!("\0{save}\n\n.wisp/tasks/x.task.yml\n\0setup\n\n.wisp/tasks/y.task.yml\n\0Merge branch x\n");
        let (touched, outside) = sort_out(&log, &req);
        assert_eq!(touched, [".wisp/tasks/x.task.yml", ".wisp/tasks/y.task.yml"]);
        assert_eq!(outside, [".wisp/tasks/y.task.yml", "Merge branch x"]);
        assert_eq!(sort_out(&format!("\0{save}\n\n.wisp/worktree.json\n"), &req).1, [".wisp/worktree.json"]);
        assert_eq!(sort_out(&format!("\0{save}\n\n.wisp/history/x.jsonl.zst\n"), &req).1, Vec::<String>::new());
    }

    /// final6 F1: the LFS store adds its line to the `.gitattributes` beside `.wisp/` in the same save commit: that
    /// file goes with the save, only where the commit writes inside the folder next to it, and only in a save.
    #[test]
    fn a_save_s_gitattributes_beside_its_folder_is_the_save_s() {
        let req = CatchUpRequest {
            only: ".wisp".into(),
            within: within(),
            also: vec![".gitattributes".into()],
            subject_start: START.into(),
            subject_end: END.into(),
            ..CatchUpRequest::default()
        };
        let save = format!("{START}t{END}");
        let none = Vec::<String>::new();
        assert_eq!(sort_out(&format!("\0{save}\n\n.gitattributes\n.wisp/history/x.zst\n"), &req).1, none);
        assert_eq!(sort_out(&format!("\0{save}\n\napp/.gitattributes\napp/.wisp/history/x.zst\n"), &req).1, none);
        assert_eq!(sort_out(&format!("\0{save}\n\nsrc/.gitattributes\n.wisp/history/x.zst\n"), &req).1, ["src/.gitattributes"]);
        assert_eq!(sort_out(&format!("\0{save}\n\n.wisp/.gitattributes\n.wisp/history/x.zst\n"), &req).1, [".wisp/.gitattributes"]);
        assert_eq!(
            sort_out(&format!("\0{save}\n\n.gitattributes\n.wisp/worktree.json\n"), &req).1,
            [".gitattributes", ".wisp/worktree.json"]
        );
        assert_eq!(sort_out(&format!("\0{save}\n\n.gitattributes\n"), &req).1, [".gitattributes"], "never on its own");
        assert_eq!(sort_out("\0lfs\n\n.gitattributes\n.wisp/history/x.zst\n", &req).1, [".gitattributes", ".wisp/history/x.zst"]);
        let without = CatchUpRequest { also: Vec::new(), ..req };
        assert_eq!(sort_out(&format!("\0{save}\n\n.gitattributes\n.wisp/history/x.zst\n"), &without).1, [".gitattributes"]);
    }
}
