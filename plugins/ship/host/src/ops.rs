//! One repo's Ship it, through git.service and gh: its status, commit → push → PR, the squash merge,
//! and whether it really conflicts with its base.

use std::sync::Arc;

use wisp_git_api::{CatchUpRequest, CaughtUp, CommitRequest, FetchRequest, MergeTree, PushRequest, Repo, WorktreeAdd};
use wisp_kernel::{Call, CancellationToken, Clock};
use wisp_ship_api::{PrSummary, RepoStatus, Synced, Worktree};
use wisp_ship_core::Refusal;
use wisp_ship_core::decide::{
    HISTORY_DIR, Tidied, cleans_up, commit_message, created_url, diverged, merge_plan, merged_head, pr_create_args, push_refspec,
    push_target, repo_status, reuse_pr, tidy_note, unpushed_from,
};

use crate::error::{Result, ShipError};
use crate::gh::Gh;

/// A merged task's folder removed through the worktrees plugin (`worktrees.remove`, strict, the branch left to ship):
/// its per-folder lock, the turns' claims and the folders working tasks are in all hold, and Keep worktree keeps it.
/// Given the call, the task's id and its worktree.
pub type Tidier = Arc<dyn Fn(Call, String, Worktree) -> wisp_kernel::BoxFuture<'static, Tidied> + Send + Sync>;

/// What Ship it runs on. Cheap to clone.
#[derive(Clone)]
pub struct Shipper {
    pub git: Arc<dyn wisp_git_api::GitService>,
    pub gh: Gh,
    /// the plugin's scope: repo handles are released with it
    pub owner: CancellationToken,
    pub clock: Arc<dyn Clock>,
    pub tidier: Tidier,
}

impl std::fmt::Debug for Shipper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shipper").field("gh", &self.gh).finish_non_exhaustive()
    }
}

/// The stage a ship is at: commit, push, pr.
pub type OnStep<'a> = &'a (dyn Fn(&str) + Send + Sync);

pub async fn exists(path: &str) -> bool {
    !path.is_empty() && tokio::fs::try_exists(path).await.unwrap_or(false)
}

impl Shipper {
    pub fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    pub async fn repo(&self, call: &Call, path: &str) -> Result<Arc<dyn Repo>> {
        Ok(self.git.repo(call, &self.owner, path).await?)
    }

    /// Where to run gh and git for a worktree: its folder, else (gone after a merge) the main checkout.
    pub async fn dir_of(wt: &Worktree) -> String {
        if exists(&wt.path).await { wt.path.clone() } else { wt.root.clone() }
    }

    /// v1 worktreeStatus: dirty paths, commits past the base, commits not on the PR. After its PR
    /// merged, only commits since the merged head count (edge case Git 6). Err when `git status` fails: git can't
    /// tell, which is never "clean".
    pub async fn status(&self, call: &Call, wt: &Worktree, pr: Option<&PrSummary>) -> Result<RepoStatus> {
        if !exists(&wt.path).await {
            return Ok(RepoStatus::default());
        }
        let repo = self.repo(call, &wt.path).await?;
        let (st, ahead) = tokio::join!(repo.status(call), repo.count(call, &format!("{}..HEAD", wt.base_ref)));
        let dirty = u32::try_from(st?.dirty()).unwrap_or(u32::MAX);
        let mut counts = Vec::new();
        for r in unpushed_from(wt, merged_head(pr)) {
            let n = repo.count(call, &format!("{r}..HEAD")).await.ok().flatten();
            counts.push(n);
            if n.is_some() {
                break;
            }
        }
        Ok(repo_status(true, dirty, ahead.ok().flatten(), &counts))
    }

    /// Archiving removed a clean worktree: bring it back from its branch before shipping.
    pub async fn ensure(&self, call: &Call, wt: &Worktree) -> Result<()> {
        if exists(&wt.path).await {
            return Ok(());
        }
        if wt.foreign {
            // no prune, and no add: that would make a new folder at its path
            return Err(Refusal::Conflict(format!("This worktree's folder is gone: {}", wt.path)).into());
        }
        let root = self.repo(call, &wt.root).await?;
        root.worktree_prune(call).await.ok();
        root.worktree_add(call, WorktreeAdd { path: wt.path.clone(), branch: wt.branch.clone(), start: None }).await?;
        Ok(())
    }

    /// Commit whatever the task left, push its branch and open (or update) its PR.
    pub async fn ship(&self, call: &Call, wt: &Worktree, req: &ShipReq<'_>, on_step: OnStep<'_>) -> Result<PrSummary> {
        self.ensure(call, wt).await?;
        let repo = self.repo(call, &wt.path).await?;
        if repo.status(call).await.map_or(0, |s| s.dirty()) > 0 {
            let (message, body) = commit_message(req.title, req.body)?;
            let body = wisp_ship_core::decide::with_task(&body, req.task);
            on_step("commit");
            repo.commit(call, CommitRequest { message, body, all: true, no_edit: false, no_verify: true }).await?;
        }
        let ahead = repo.count(call, &format!("{}..HEAD", wt.base_ref)).await?.unwrap_or(0);
        let remote = push_target(ahead, wt)?;
        on_step("push");
        // a history push from the worktree (Push to repo) put a commit of .wisp/ files on the branch's remote tip:
        // merged in first, so this push stays a fast-forward. Anything else there is refused, never forced over.
        // git failing to tell leaves it to the push, as before. Only the push's own commits (its subject, the files a
        // save writes): someone else's .wisp/worktree.json or sandbox.sh would run here, merged with --no-verify
        let caught = CatchUpRequest {
            remote: remote.clone(),
            branch: wt.branch.clone(),
            only: HISTORY_DIR.into(),
            within: wisp_history_api::SAVED_DIRS.iter().map(|d| (*d).to_owned()).collect(),
            also: wisp_history_api::SAVED_FILES.iter().map(|f| (*f).to_owned()).collect(),
            subject_start: wisp_history_api::SAVE_SUBJECT_START.into(),
            subject_end: wisp_history_api::SAVE_SUBJECT_END.into(),
        };
        if let Ok(CaughtUp::Diverged { files }) = repo.catch_up(call, caught).await {
            return Err(diverged(&remote, &wt.branch, &files).into());
        }
        repo.push(call, PushRequest { remote, refspec: push_refspec(wt), set_upstream: true, delete: false }).await?;
        on_step("pr");
        let now = self.now();
        if let Some(n) = reuse_pr(req.pr) {
            return self.gh.pr_view(&wt.path, &n.to_string(), now).await;
        }
        let title = req.title.trim();
        let out = self.gh.run(&wt.path, &pr_create_args(wt, title, req.body, req.draft), None).await?;
        self.gh.pr_view(&wt.path, &created_url(&out), now).await
    }

    /// Commits on the task's branch the merged PR doesn't have, counted from the branch itself (`refs/heads/<branch>`):
    /// HEAD may have left it, and what's there is still only in this repo. None when git can't tell.
    async fn branch_unpushed(&self, call: &Call, wt: &Worktree, pr: &PrSummary) -> Option<u32> {
        if wt.branch.is_empty() {
            return Some(0);
        }
        let repo = self.repo(call, &wt.root).await.ok()?;
        for r in unpushed_from(wt, merged_head(Some(pr))) {
            if let Some(n) = repo.count(call, &format!("{r}..refs/heads/{}", wt.branch)).await.ok().flatten() {
                return Some(n);
            }
        }
        None
    }

    /// Squash-merge the exact head that was reviewed (a draft readied first), fast-forward the user's
    /// base, then tidy wisp's own worktree and branch (edge cases Git 7, 8) for task `session`.
    pub async fn merge(
        &self,
        call: &Call,
        session: &str,
        wt: &Worktree,
        number: u64,
        admin: bool,
    ) -> Result<(PrSummary, Synced, Option<String>)> {
        let dir = Self::dir_of(wt).await;
        let n = number.to_string();
        let pr = self.gh.pr_view(&dir, &n, self.now()).await?;
        let plan = merge_plan(&pr, &n, admin)?;
        if let Some(ready) = plan.ready {
            self.gh.run(&dir, &ready, None).await?;
        }
        self.gh.run(&dir, &plan.merge, None).await.map_err(|e| held(e, pr.number))?;
        let pr = self.gh.pr_view(&dir, &n, self.now()).await?;
        let root = self.repo(call, &wt.root).await?;
        let mut sync = root.sync_base(call, &wt.base).await.map_or_else(
            |e| Synced { ok: false, text: wisp_git_api::not_updated(&wt.base, &e.message) },
            |s| Synced { ok: s.ok, text: s.text },
        );
        if cleans_up(wt) {
            // work the merge didn't take (uncommitted, or commits after the PR's head, in the folder or on its branch)
            // keeps the folder and its branch (audit A orch #3: a merge removed a worktree holding uncommitted work,
            // without a word); git failing to say counts as work, never as clean
            let note = match self.status(call, wt, Some(&pr)).await {
                Err(e) => Some(format!("The worktree {} was kept: git status failed ({e}).", wt.path)),
                Ok(mut left) => {
                    if left.exists {
                        left.unpushed = left.unpushed.max(self.branch_unpushed(call, wt, &pr).await.unwrap_or(0));
                    }
                    match kept_because(&left) {
                        Some(why) => Some(format!("The worktree {} was kept: {why}.", wt.path)),
                        None => self.tidy(call, session, &root, wt).await,
                    }
                }
            };
            if let Some(note) = note {
                if !sync.text.is_empty() {
                    sync.text.push(' ');
                }
                sync.text.push_str(&note);
                return Ok((pr, sync, Some(note)));
            }
        }
        Ok((pr, sync, None))
    }

    /// The merged task's worktree goes through the worktrees plugin (`tidier`), then its branch, here and then on the
    /// remote, only once the folder went: a folder that stays keeps its branch (a follow-up brings it back from there).
    /// Best effort: the merge landed. The note when it stayed.
    async fn tidy(&self, call: &Call, session: &str, root: &Arc<dyn Repo>, wt: &Worktree) -> Option<String> {
        let gone = if exists(&wt.path).await { (self.tidier)(call.clone(), session.to_owned(), wt.clone()).await } else { Tidied::Removed };
        if let Some(note) = tidy_note(&wt.path, &gone) {
            tracing::info!(path = %wt.path, "ship: worktree kept after merge: {note}");
            return Some(note);
        }
        if root.branch_delete(call, &wt.branch).await.is_ok() {
            let remote = wt.remote.clone().unwrap_or_else(|| "origin".into());
            root.push(call, PushRequest { remote, refspec: wt.branch.clone(), set_upstream: false, delete: true }).await.ok();
        }
        None
    }

    /// Squash-merge a PR attached by URL: no worktree of wisp's to sync or tidy.
    pub async fn merge_url(&self, dir: &str, url: &str, admin: bool) -> Result<PrSummary> {
        let pr = self.gh.pr_view(dir, url, self.now()).await?;
        let plan = merge_plan(&pr, url, admin)?;
        if let Some(ready) = plan.ready {
            self.gh.run(dir, &ready, None).await?;
        }
        self.gh.run(dir, &plan.merge, None).await.map_err(|e| held(e, pr.number))?;
        self.gh.pr_view(dir, url, self.now()).await
    }

    /// Does the branch really conflict with its freshly fetched base? Asked before spending an agent run
    /// on GitHub's word, which can lag a push. None when git can't tell (< 2.38, edge case Git 9).
    pub async fn conflicts_with_base(&self, call: &Call, wt: &Worktree) -> Result<Option<bool>> {
        let Some(remote) = wt.remote.clone().filter(|r| !r.is_empty()) else {
            return Ok(None);
        };
        if !exists(&wt.path).await {
            return Ok(None);
        }
        let repo = self.repo(call, &wt.path).await?;
        repo.fetch(call, FetchRequest { remote: Some(remote.clone()), refspec: Some(wt.base.clone()), max_age_ms: 0 }).await.ok();
        Ok(match repo.merge_tree(call, &format!("refs/remotes/{remote}/{}", wt.base), "HEAD").await? {
            MergeTree::Clean => Some(false),
            MergeTree::Conflict { .. } => Some(true),
            MergeTree::Unknown => None,
        })
    }

    /// After the fix-up turn: the base is merged in with nothing half done (git.service `base_merged`).
    pub async fn base_merged(&self, call: &Call, wt: &Worktree) -> Result<std::result::Result<(), String>> {
        let repo = self.repo(call, &wt.path).await?;
        let base = format!("refs/remotes/{}/{}", wt.remote.as_deref().unwrap_or("origin"), wt.base);
        let m = repo.base_merged(call, &base).await?;
        Ok(if m.ok { Ok(()) } else { Err(m.why.unwrap_or_else(|| "The base is not merged in.".into())) })
    }
}

/// gh's branch-rules refusal of a merge, typed so the UI asks to bypass or wait; any other failure as it was.
fn held(e: ShipError, number: u64) -> ShipError {
    match e {
        ShipError::Gh(m) if wisp_ship_core::decide::merge_blocked(&m) => ShipError::Blocked(wisp_ship_core::decide::blocked_text(number)),
        e => e,
    }
}

/// Why a merged task's worktree stays: what's in it that the merge didn't take. None: nothing, it can go.
pub fn kept_because(st: &RepoStatus) -> Option<String> {
    let mut why = Vec::new();
    if st.dirty > 0 {
        why.push(format!("it has {} uncommitted change{}", st.dirty, if st.dirty == 1 { "" } else { "s" }));
    }
    if st.unpushed > 0 {
        why.push(format!("{} commit{} the pull request doesn't have", st.unpushed, if st.unpushed == 1 { "" } else { "s" }));
    }
    (!why.is_empty()).then(|| why.join(" and "))
}

/// What a Ship it asks for.
#[derive(Debug, Clone, Copy)]
pub struct ShipReq<'a> {
    /// the task the commit is for (its `Wisp-Task` trailer)
    pub task: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub draft: bool,
    /// the repo's PR so far: an open one is updated, not opened again
    pub pr: Option<&'a PrSummary>,
}
