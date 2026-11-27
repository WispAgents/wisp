//! `Repo::catch_up`: before Ship's push, the commits wisp's history push put on the task's branch (a save's `.wisp/`
//! files, built on the branch's remote tip) merged into HEAD, so the push is a fast-forward and never needs a force.
//! Anything else only the remote has (someone else's push) is left for the user: nothing is merged or moved.

use wisp_git_api::{CatchUpRequest, CaughtUp};

use crate::error::Result;
use crate::repo::Inner;
use crate::run::{Opts, args};

/// Whether `path` (from git's top) is in a folder named `folder`, at the top or in a subfolder (a monorepo's
/// `app/.wisp/`).
pub(crate) fn in_folder(path: &str, folder: &str) -> bool {
    let folder = folder.trim_matches('/');
    !folder.is_empty() && path.split('/').rev().skip(1).any(|part| part == folder)
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
        // every commit's own paths (no renames: a move out of the folder counts its old path too)
        let range = format!("HEAD..{theirs}");
        let touched = self.git.read(&self.dir, &["log", "--no-renames", "--format=", "--name-only", &range]).await?;
        let mut outside: Vec<String> = touched.lines().filter(|l| !l.is_empty() && !in_folder(l, &req.only)).map(str::to_owned).collect();
        outside.sort();
        outside.dedup();
        if !outside.is_empty() {
            return Ok(CaughtUp::Diverged { files: outside });
        }
        let message = format!("wisp: merge the history saved to {}", req.branch);
        let merged = self
            .git
            .exec(&self.dir, &args(&["merge", "--quiet", "--no-edit", "--no-verify", "-m", &message, &theirs]), Opts::timeout(120))
            .await?;
        if merged.exit.success() {
            self.notify("head");
            return Ok(CaughtUp::Merged);
        }
        // the same history file changed both ways: back to where it was, and the user decides
        let conflicted = self.git.quiet(&self.dir, &["diff", "--name-only", "--diff-filter=U"]).await;
        let _ = self.git.exec(&self.dir, &args(&["merge", "--abort"]), Opts::default()).await;
        let files: Vec<String> = conflicted.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
        Ok(if files.is_empty() { CaughtUp::Unknown } else { CaughtUp::Diverged { files } })
    }
}

#[cfg(test)]
mod tests {
    use super::in_folder;

    #[test]
    fn only_paths_inside_the_folder_count() {
        assert!(in_folder(".wisp/tasks/a.task.yml", ".wisp"));
        assert!(in_folder("app/.wisp/tasks/a.task.yml", ".wisp/"));
        assert!(!in_folder("src/main.rs", ".wisp"));
        assert!(!in_folder(".wisp", ".wisp"), "a file named like the folder is not in it");
        assert!(!in_folder(".wispy/a", ".wisp"));
        assert!(!in_folder(".wisp/a", ""));
    }
}
