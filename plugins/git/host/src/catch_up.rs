//! `Repo::catch_up`: before Ship's push, the commits wisp's history push put on the task's branch (a save's `.wisp/`
//! files, built on the branch's remote tip) merged into HEAD, so the push is a fast-forward and never needs a force.
//! Only those: each commit has the history push's subject and touches nothing but what a save writes (`within`).
//! Anything else only the remote has (someone else's push, a `.wisp/worktree.json` whose `setup` would run here) is
//! left for the user: nothing is merged or moved.

use std::path::{Path, PathBuf};

use wisp_git_api::{CatchUpRequest, CaughtUp};

use crate::error::Result;
use crate::repo::Inner;
use crate::run::{Opts, args};

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
        outside.extend(paths.iter().filter(|p| !saved || !in_folder(p, &req.only, &req.within)).map(|p| (*p).to_owned()));
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

impl Inner {
    pub async fn catch_up(&self, req: CatchUpRequest) -> Result<CaughtUp> {
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
        let top = PathBuf::from(&self.info.top);
        // a newer save not committed here (`.wisp/` untracked, or ignored: git overwrites those in a merge without a
        // word) is set aside and put back after, whatever the merge did: it stays the pending change the next
        // history push sends, never replaced by the older copy the remote has
        let kept = self.set_aside(&top, &touched).await;
        let out = self.merge_history(&req, &theirs, &top).await;
        for (path, bytes) in &kept {
            put_back(&top.join(path), bytes).await;
        }
        out
    }

    /// The untracked or ignored files among `paths` (from git's top), read and taken off disk, so the merge neither
    /// refuses ("would be overwritten") nor silently replaces them.
    async fn set_aside(&self, top: &Path, paths: &[String]) -> Vec<(String, Vec<u8>)> {
        if paths.is_empty() {
            return Vec::new();
        }
        let specs = literal(paths);
        let mut a = vec!["ls-files", "-o", "-z", "--"];
        a.extend(specs.iter().map(String::as_str));
        let listed = self.git.quiet(top, &a).await;
        let mut kept = Vec::new();
        for path in listed.split('\0').filter(|p| !p.is_empty()) {
            let file = top.join(path);
            if let Ok(bytes) = tokio::fs::read(&file).await
                && tokio::fs::remove_file(&file).await.is_ok()
            {
                kept.push((path.to_owned(), bytes));
            }
        }
        kept
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

/// A file set aside before the merge, written back (its folder made again if the merge took it).
async fn put_back(file: &Path, bytes: &[u8]) {
    if let Some(dir) = file.parent() {
        let _ = tokio::fs::create_dir_all(dir).await;
    }
    if let Err(e) = tokio::fs::write(file, bytes).await {
        tracing::warn!(file = %file.display(), "catch_up: a local save couldn't be put back: {e}");
    }
}

#[cfg(test)]
mod tests {
    use wisp_git_api::CatchUpRequest;

    use super::{in_folder, is_history_subject, sort_out};

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
}
