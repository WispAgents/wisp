//! A session's worktree: made (v1 createWorktree), brought back (ensureWorktree), read (worktreeStatus,
//! folderOnly) and removed (removeWorktree). Every git call goes through git.service@1 with the caller's
//! `Call`, so its permissions are checked there; the decisions are wisp-worktrees-core's.
//!
//! size-ok: every operation on a folder shares its lock, the turns' claims and the removal's guard and in-use check

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use wisp_git_api::{GitService, Repo, WorktreeAdd};
use wisp_kernel::{Call, CancellationToken, Clock};
use wisp_proc::Proc;
use wisp_worktrees_api::{CreateArgs, HELD_FILES_MAX, HeldFile, Prepared, RemoveOpts, Removed, SetupSource, Worktree, WorktreeStatus};
use wisp_worktrees_core::{naming, removal};

use crate::error::{Result, WtError};
use crate::fsx;
use crate::ports::Port;

/// The longest a turn's claim on its folder holds when runtime never says the turn started (a turn refused after
/// its `ready`): its `ready` (at most `READY_MS`) and the save that marks it running, with room to spare.
pub const CLAIM_MS: i64 = 150_000;

/// Who claims a folder: a task's turn (`turn`: its id, let go by that turn's `turn.started` or `turn.held`), or a
/// side run of the task (`turn` None: a /btw or a quick run, which no event confirms; it lapses after `CLAIM_MS`).
#[derive(Debug, Clone, Copy)]
pub struct Claimant<'a> {
    pub task: &'a str,
    pub turn: Option<&'a str>,
}

impl<'a> Claimant<'a> {
    pub fn turn(task: &'a str, turn: &'a str) -> Self {
        Self { task, turn: Some(turn) }
    }
    pub fn side(task: &'a str) -> Self {
        Self { task, turn: None }
    }
}

/// One turn's (or side run's) claim on one folder: its number on the claims' counter, when, and whether it still
/// holds (false once runtime saved that turn running or held: from then the task's state says it's in use, its own
/// folder by its slot, an extra folder by its `dirs`, `Port::extra_in_use`).
#[derive(Debug, Clone)]
struct Claim {
    n: u64,
    at: i64,
    live: bool,
}

/// The claims, by (folder, task, turn; "" for a side run). Per turn: a late `turn.started` of a turn that failed
/// at once lets go of its own claims only, never the next queued turn's (ADR-0053).
type Claims = HashMap<(String, String, String), Claim>;

/// Everything a worktree operation needs. Cheap to share (`Arc<Engine>`).
pub struct Engine {
    pub git: Arc<dyn GitService>,
    pub proc: Proc,
    pub clock: Arc<dyn Clock>,
    /// wisp's home (`~/.wisp`): worktrees go under `<home>/worktrees`
    pub home: String,
    /// the user's home, for Conductor's folders
    pub user_home: String,
    /// "macos" | "linux" | "windows"
    pub platform: &'static str,
    pub sources: Vec<Arc<dyn SetupSource>>,
    /// the plugin's scope: repo handles are released with it
    pub owner: CancellationToken,
    /// one operation at a time per folder: a bring-back waits for a removal under way
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    /// each turn's claim on each folder (`claim`), by (folder, task, turn), on one counter
    claims: std::sync::Mutex<(u64, Claims)>,
    /// the tasks and what they work in (`watch`): `remove` asks it under the folder's lock, so no caller can skip it
    port: std::sync::OnceLock<Arc<dyn Port>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").field("home", &self.home).finish_non_exhaustive()
    }
}

impl Engine {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        git: Arc<dyn GitService>,
        proc: Proc,
        clock: Arc<dyn Clock>,
        home: String,
        user_home: String,
        platform: &'static str,
        sources: Vec<Arc<dyn SetupSource>>,
        owner: CancellationToken,
    ) -> Self {
        Self {
            git,
            proc,
            clock,
            home,
            user_home,
            platform,
            sources,
            owner,
            locks: Mutex::new(HashMap::new()),
            claims: std::sync::Mutex::new((0, HashMap::new())),
            port: std::sync::OnceLock::new(),
        }
    }

    /// Who works where, read by every removal (`path_in_use`). Set once, when the plugin's parts are built.
    pub fn watch(&self, port: Arc<dyn Port>) {
        let _ = self.port.set(port);
    }

    /// Whether `path` must stay: a working task other than `except` is in it (a recurring run's kept folder is shared
    /// by its runs, ADR-0108), a working task has it as an extra folder, or a turn's claim on it still holds.
    /// By path as the disk sees it.
    pub async fn path_in_use(&self, path: &str, except: Option<&str>) -> bool {
        let real = fsx::norm(path).await;
        if self.claimed_since(&real, u64::MAX) {
            return true;
        }
        let Some(port) = self.port.get() else {
            return false;
        };
        for t in port.tasks().await.into_iter().filter(|t| t.busy && except != Some(t.id.as_str())) {
            for x in t.trees.iter().filter(|x| !x.worktree.path.is_empty()) {
                if fsx::norm(&x.worktree.path).await == real {
                    return true;
                }
            }
        }
        port.extra_in_use(path.to_owned()).await
    }

    /// Why `wt.path` is never a folder for a removal to delete, or None. The repo itself (its main checkout or a full
    /// clone: `.git` a folder, not a linked worktree's file; a folder holding the repo; its `.git`), and a folder outside
    /// `<home>/worktrees` that no task of wisp's made (no `created_at`): a bad record must not cost the user a repo.
    async fn guard(&self, wt: &Worktree) -> Option<String> {
        let path = fsx::norm(&wt.path).await;
        if tokio::fs::metadata(fsx::join(&path, ".git")).await.is_ok_and(|m| m.is_dir()) {
            return Some(format!("{} is a repository's own checkout, not a worktree: it is never removed.", wt.path));
        }
        let ours = format!("{}/worktrees", self.home.trim_end_matches(['/', '\\']));
        if !wt.root.is_empty() {
            let root = fsx::norm(&wt.root).await;
            // the record names the repo as its own folder: refused, but for a leftover no repo knows, which is recorded
            // with itself as its root and only ever lives in wisp's worktrees folder
            if root == path && !fsx::within_real(&wt.path, &ours).await {
                return Some(format!("{} is the repository itself, not a worktree: it is never removed.", wt.path));
            }
            if (root != path && fsx::within(&root, &path)) || fsx::within(&path, &fsx::join(&root, ".git")) {
                return Some(format!("{} holds the repository itself: it is never removed.", wt.path));
            }
        }
        if wt.created_at == 0 && !fsx::within_real(&wt.path, &ours).await {
            return Some(format!("{} isn't in wisp's worktrees folder: it is never removed.", wt.path));
        }
        None
    }

    pub async fn repo(&self, call: &Call, path: &str) -> Result<Arc<dyn Repo>> {
        Ok(self.git.repo(call, &self.owner, path).await?)
    }

    async fn lock(&self, path: &str) -> Arc<Mutex<()>> {
        self.locks.lock().await.entry(path.to_owned()).or_default().clone()
    }

    /// Where the claims stand now: a removal decided after this mark asks `claimed_since` last thing.
    pub fn mark(&self) -> u64 {
        self.claims.lock().unwrap_or_else(std::sync::PoisonError::into_inner).0
    }

    /// `who`'s turn (or side run) is about to run in `path`: a removal of it decided before now stands down, and so
    /// does one decided after, until runtime says that turn started (`release`; till then the task still reads
    /// archived and idle) or `CLAIM_MS` passes.
    /// The path is kept as the disk sees it (`fsx::norm`), as `claimed_since` is asked.
    pub async fn claim(&self, path: &str, who: Claimant<'_>) {
        let path = fsx::norm(path).await;
        let now = self.clock.now_ms();
        let mut c = self.claims.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        c.0 += 1;
        let n = c.0;
        // a look older than this is long over; a released claim still answers looks made before it
        c.1.retain(|_, k| now - k.at < 4 * CLAIM_MS);
        let key = (path, who.task.to_owned(), who.turn.unwrap_or_default().to_owned());
        c.1.insert(key, Claim { n, at: now, live: true });
    }

    /// Runtime saved `task`'s turn `turn` (running, or held): its own state guards the folders from here. Only that
    /// turn's claims let go; a side run's, and another turn's of the same task, hold on.
    pub fn release(&self, task: &str, turn: &str) {
        let mut c = self.claims.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        for (_, k) in c.1.iter_mut().filter(|((_, t, u), _)| t == task && !u.is_empty() && u == turn) {
            k.live = false;
        }
    }

    /// Whether a turn claimed `path` after `mark`, or is still starting in it. `path` as the disk sees it
    /// (`fsx::norm`, as the claims are kept): a claim on it through a symlink or with `..` counts.
    pub fn claimed_since(&self, path: &str, mark: u64) -> bool {
        let now = self.clock.now_ms();
        self.claims
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .1
            .iter()
            // a claim on the folder, or on a folder inside it (an extra folder that's a worktree's subfolder)
            .filter(|((p, _, _), _)| fsx::within(p, path))
            .any(|(_, k)| k.n > mark || (k.live && now - k.at < CLAIM_MS))
    }

    pub fn foreign(&self, wt: &Worktree) -> bool {
        removal::is_foreign(wt, &self.user_home, self.sources.iter().any(|s| s.owns(&wt.path)))
    }

    /// A fresh worktree and branch under `<home>/worktrees`; None outside a repo with commits.
    /// `hold`: the setup steps to keep back until the repo is trusted (decided by the caller).
    /// `in_use`: the folders working tasks are in (a recurring run's kept folder among them gets a fresh one instead).
    pub async fn create(
        &self,
        call: &Call,
        args: &CreateArgs,
        hold: Option<Vec<wisp_worktrees_api::TrustItem>>,
        in_use: &[String],
    ) -> Result<Option<Worktree>> {
        let Ok(here) = self.repo(call, &args.cwd).await else {
            return Ok(None);
        };
        let Some(base) = here.base(call, true).await? else {
            return Ok(None);
        };
        let info = here.info().clone();
        let root = self.repo(call, &info.root).await?;
        // a recurring run (ADR-0108): its one folder in this project, reused while no working task is in it
        if let Some(key) = args.reuse.as_deref().and_then(naming::reuse_key)
            && let Some(wt) = self.kept_for(call, args, &key, (&info, &base), hold.clone(), in_use).await?
        {
            return Ok(Some(wt));
        }
        let short = naming::short_branch(&args.title, &args.session_id);
        let taken = root.rev_parse(call, &format!("refs/heads/{short}")).await?.is_some();
        let branch = naming::branch_for(&args.title, &args.session_id, taken);
        let dir = naming::worktree_dir(&self.home, &info.root, &args.session_id);
        fsx::mkdirp(fsx::parent(&dir)).await?;
        // a fork: its parent's commit and a copy of its folder as it is now (snapshotted before the add)
        // a fork from an earlier turn: its folder as it was then (git's checkpoint before the turn after it)
        let copy = match (&args.at, &args.from) {
            (Some(at), _) if !at.head.is_empty() => Some((at.head.clone(), Some(at.to.clone()).filter(|t| !t.is_empty() && *t != at.head))),
            (_, Some(from)) => {
                let src = self.repo(call, from).await?;
                let head = src.rev_parse(call, "HEAD").await?;
                let snap = src.snapshot(call).await?;
                head.map(|h| (h, snap))
            }
            _ => None,
        };
        let start = copy.as_ref().map_or_else(|| base.reference.clone(), |(h, _)| h.clone());
        root.worktree_add(call, WorktreeAdd { path: dir.clone(), branch: branch.clone(), start: Some(start) }).await?;
        if let Some((head, Some(snap))) = &copy {
            self.repo(call, &dir).await?.restore(call, snap, head).await?;
        }
        let prepared = crate::prepare::prepare(self, &info.root, &dir, hold, &base.branch).await;
        if !prepared.problems.is_empty() {
            tracing::warn!(dir = %dir, problems = ?prepared.problems, "worktree prepared with problems");
        }
        let cwd = if info.sub.is_empty() { dir.clone() } else { format!("{dir}/{}", info.sub) };
        Ok(Some(Worktree {
            path: dir,
            cwd,
            branch,
            base: base.branch,
            base_ref: base.reference,
            head: Some(base.sha).filter(|s| !s.is_empty()),
            remote: base.remote,
            root: info.root,
            created_at: self.clock.now_ms(),
            prepared: prepared_or_none(prepared),
            tidy: args.tidy,
            ..Worktree::default()
        }))
    }

    /// A recurring run's kept folder (`<home>/worktrees/<project>-<key>`, branch `wisp/<key>`, ADR-0108): the one there,
    /// when it is this repo's and no working task is in it (`in_use`); made again on its branch (or a new one from the
    /// base) when it isn't there. None: a fresh one of the task's own instead.
    async fn kept_for(
        &self,
        call: &Call,
        args: &CreateArgs,
        key: &str,
        (info, base): (&wisp_git_api::RepoInfo, &wisp_git_api::BaseRef),
        hold: Option<Vec<wisp_worktrees_api::TrustItem>>,
        in_use: &[String],
    ) -> Result<Option<Worktree>> {
        let root = self.repo(call, &info.root).await?;
        let cwd_in = |dir: &str| naming::worktree_cwd(dir, &info.root, &args.cwd);
        let dir = naming::worktree_dir(&self.home, &info.root, key);
        let real = fsx::norm(&dir).await;
        let mut busy = false;
        for p in in_use {
            busy |= fsx::norm(p).await == real;
        }
        if !busy && fsx::exists(&dir).await {
            // ours only when it is this repo's worktree (another project named the same has its own root)
            if let Ok(there) = self.repo(call, &dir).await
                && fsx::norm(&there.info().root).await == fsx::norm(&info.root).await
            {
                let branch =
                    there.status(call).await.ok().map(|s| s.branch).filter(|b| !b.is_empty()).unwrap_or_else(|| naming::reuse_branch(key));
                return Ok(Some(Worktree {
                    cwd: cwd_in(&dir),
                    path: dir,
                    branch,
                    base: base.branch.clone(),
                    base_ref: base.reference.clone(),
                    head: Some(base.sha.clone()).filter(|s| !s.is_empty()),
                    remote: base.remote.clone(),
                    root: info.root.clone(),
                    created_at: self.clock.now_ms(),
                    tidy: args.tidy,
                    ..Worktree::default()
                }));
            }
        } else if !busy {
            let branch = naming::reuse_branch(key);
            let kept = root.rev_parse(call, &format!("refs/heads/{branch}")).await?.is_some();
            fsx::mkdirp(fsx::parent(&dir)).await?;
            let _ = root.worktree_prune(call).await;
            // its branch kept from an earlier run: checked out again; else made from the base
            let start = (!kept).then(|| base.reference.clone());
            root.worktree_add(call, WorktreeAdd { path: dir.clone(), branch: branch.clone(), start }).await?;
            let prepared = crate::prepare::prepare(self, &info.root, &dir, hold, &base.branch).await;
            return Ok(Some(Worktree {
                cwd: cwd_in(&dir),
                path: dir,
                branch,
                base: base.branch.clone(),
                base_ref: base.reference.clone(),
                head: Some(base.sha.clone()).filter(|s| !s.is_empty()),
                remote: base.remote.clone(),
                root: info.root.clone(),
                created_at: self.clock.now_ms(),
                prepared: prepared_or_none(prepared),
                tidy: args.tidy,
                ..Worktree::default()
            }));
        }
        Ok(None)
    }

    /// Brings a cleaned-up worktree back from its branch (its ignored files again, not its setup).
    pub async fn ensure(&self, call: &Call, wt: &Worktree) -> Result<()> {
        self.restore(call, wt, false).await.map(drop)
    }

    /// `ensure`, and when its branch is gone too (deleted after its PR merged, or by a tidy, ADR-0131) and `fresh`: the
    /// branch made again from the base, so the task goes on (its work is on the base by then). `Ok(true)`: it was.
    pub async fn restore(&self, call: &Call, wt: &Worktree, fresh: bool) -> Result<bool> {
        let lock = self.lock(&wt.path).await;
        let _held = lock.lock().await;
        match removal::ensure_plan(fsx::exists(&wt.path).await, self.foreign(wt), &wt.path) {
            removal::EnsurePlan::Nothing => Ok(false),
            removal::EnsurePlan::Refuse(m) => Err(WtError::Conflict(m)),
            removal::EnsurePlan::Restore => {
                let root = self.repo(call, &wt.root).await?;
                let _ = root.worktree_prune(call).await;
                // its branch gone too: made again from the base, or said plainly, never git's "invalid reference"
                let gone = !wt.branch.is_empty() && matches!(root.rev_parse(call, &format!("refs/heads/{}", wt.branch)).await, Ok(None));
                let start = match (gone, fresh) {
                    (false, _) => None,
                    (true, false) => return Err(WtError::NotFound(crate::archived::branch_gone(wt))),
                    (true, true) => Some(self.fresh_start(call, &root, wt).await?),
                };
                let made = start.is_some();
                root.worktree_add(call, WorktreeAdd { path: wt.path.clone(), branch: wt.branch.clone(), start }).await?;
                // its setup still held (not trusted yet): only the root's `.env` files, as when it was made
                let all = wt.prepared.as_ref().is_none_or(|p| p.held.is_none());
                let _ = crate::prepare::copy_some(self, &wt.root, &wt.path, all).await;
                Ok(made)
            }
        }
    }

    /// Where a gone branch starts again: its base as it is now (`origin/main`, else `main`), else the sha it branched
    /// from. `NotFound` when none of them is there any more.
    async fn fresh_start(&self, call: &Call, root: &Arc<dyn Repo>, wt: &Worktree) -> Result<String> {
        // `base_ref` is a sha when the repo has no remote: the base's name comes first then, so it starts where the base is now
        let sha = wt.base_ref.len() >= 40 && wt.base_ref.bytes().all(|b| b.is_ascii_hexdigit());
        let (first, second) = if sha { (wt.base.as_str(), wt.base_ref.as_str()) } else { (wt.base_ref.as_str(), wt.base.as_str()) };
        let mut tries: Vec<&str> = [first, second].into_iter().filter(|b| !b.is_empty()).collect();
        tries.extend(wt.head.as_deref().filter(|h| !h.is_empty()));
        for b in tries {
            if let Ok(Some(sha)) = root.rev_parse(call, b).await {
                return Ok(sha);
            }
        }
        Err(WtError::NotFound(crate::archived::branch_gone(wt)))
    }

    /// Changed paths, commits past the base, commits not on the remote (from the merged PR's head first).
    pub async fn status(&self, call: &Call, wt: &Worktree, merged: Option<&str>) -> Result<WorktreeStatus> {
        if !fsx::exists(&wt.path).await {
            return Ok(WorktreeStatus::default());
        }
        let repo = self.repo(call, &wt.path).await?;
        let base_range = format!("{}..HEAD", wt.base_ref);
        let (st, ahead) = tokio::join!(repo.status(call), repo.count(call, &base_range));
        let ahead = ahead.ok().flatten().unwrap_or(0);
        let mut counts = Vec::new();
        for r in removal::pushed_refs(merged, wt.remote.as_deref(), &wt.branch) {
            let n = repo.count(call, &format!("{r}..HEAD")).await.ok().flatten();
            counts.push(n);
            if n.is_some() {
                break;
            }
        }
        let dirty = st.map_or(0, |s| u32::try_from(s.dirty()).unwrap_or(u32::MAX));
        Ok(WorktreeStatus { exists: true, dirty, ahead, unpushed: removal::unpushed(ahead, &counts) })
    }

    /// What only this folder holds; Err when git can't tell (a repo that moved).
    pub async fn folder_only(&self, call: &Call, wt: &Worktree) -> Result<u32> {
        self.folder_held(call, wt).await.map(|(only, _)| only)
    }

    /// [`Self::folder_only`] and the uncommitted paths behind it (the first [`HELD_FILES_MAX`]), from the
    /// same `git status`: the Done dialog lists what Remove would lose without asking git twice.
    pub async fn folder_held(&self, call: &Call, wt: &Worktree) -> Result<(u32, Vec<HeldFile>)> {
        let repo = self.repo(call, &wt.path).await?;
        let root = self.repo(call, &wt.root).await?;
        let tip_ref = format!("refs/heads/{}", wt.branch);
        let (st, head, tip) = tokio::join!(repo.status(call), repo.rev_parse(call, "HEAD"), async {
            if wt.branch.is_empty() { Ok(None) } else { root.rev_parse(call, &tip_ref).await }
        });
        let st = st?;
        let head = head?.ok_or_else(|| WtError::Conflict("no HEAD".into()))?;
        let tip = tip?;
        if !wt.branch.is_empty() && tip.is_none() {
            return Err(WtError::Conflict(format!("branch {} is gone", wt.branch)));
        }
        let only = removal::folder_only(u32::try_from(st.dirty()).unwrap_or(u32::MAX), &head, tip.as_deref());
        let files = st.entries.into_iter().take(HELD_FILES_MAX).map(|e| HeldFile { path: e.path, xy: e.xy, from: e.from }).collect();
        Ok((only, files))
    }

    /// Removes the folder only when nothing would be lost (or forced); the branch per `opts.branch`.
    /// `still`: asked last thing before the folder goes. Under the folder's lock, whatever the caller: a folder a working
    /// task is in (`path_in_use`) stays and is said `busy`, and one that isn't a worktree of wisp's (`guard`) is refused.
    pub async fn remove(
        &self,
        call: &Call,
        wt: &Worktree,
        opts: RemoveOpts,
        still: Option<&(dyn Fn() -> bool + Send + Sync)>,
    ) -> Result<Removed> {
        let lock = self.lock(&wt.path).await;
        let _held = lock.lock().await;
        let foreign = self.foreign(wt);
        let exists = !foreign && fsx::exists(&wt.path).await;
        if exists {
            if let Some(why) = self.guard(wt).await {
                return Err(WtError::Conflict(why));
            }
            if self.path_in_use(&wt.path, None).await {
                return Ok(Removed { busy: true, ..Removed::default() });
            }
        }
        let dirt = if exists {
            if opts.strict {
                removal::removal_dirt(true, self.folder_only(call, wt).await.ok(), 0)
            } else {
                // git can't tell (a status that failed): dirty, as strict counts it
                match self.repo(call, &wt.path).await {
                    Ok(r) => r.status(call).await.map_or(1, |s| u32::try_from(s.dirty()).unwrap_or(u32::MAX)),
                    Err(_) => 1,
                }
            }
        } else {
            0
        };
        match removal::decide_removal(foreign, exists, dirt, opts.force, still.map(|f| f())) {
            removal::RemoveStep::Foreign => {
                return Ok(Removed { foreign: true, ..Removed::default() });
            }
            removal::RemoveStep::KeepDirty(n) => {
                return Ok(Removed { dirty: Some(n), ..Removed::default() });
            }
            // in use again by the time the lock was held: kept as busy, never told as a failure
            removal::RemoveStep::Overtaken => return Ok(Removed { busy: true, ..Removed::default() }),
            // git's refusal stands (a locked worktree, a main checkout): the folder stays and the caller hears why. A
            // folder git lost track of, `worktree_remove` removes itself.
            removal::RemoveStep::Remove => match self.repo(call, &wt.root).await {
                Ok(root) => root.worktree_remove(call, &wt.path).await?,
                // a leftover no repo knows: only the folder goes (`guard` said it is one of wisp's)
                Err(_) => fsx::rm_rf(&wt.path).await,
            },
            removal::RemoveStep::Gone => {}
        }
        let branch_deleted = self.drop_branch(call, wt, opts).await;
        Ok(Removed { removed: true, branch_deleted, ..Removed::default() })
    }

    async fn drop_branch(&self, call: &Call, wt: &Worktree, opts: RemoveOpts) -> bool {
        if wt.branch.is_empty() || opts.branch == wisp_worktrees_api::BranchMode::Keep {
            return false;
        }
        let Ok(root) = self.repo(call, &wt.root).await else {
            return false;
        };
        let unique = root.count(call, &format!("{}..{}", wt.base_ref, wt.branch)).await.ok().flatten();
        removal::delete_branch(opts.branch, unique) && root.branch_delete(call, &wt.branch).await.unwrap_or(false)
    }
}

/// `prepared` stays off the record unless it says something.
pub fn prepared_or_none(p: Prepared) -> Option<Prepared> {
    (!p.is_empty()).then_some(p)
}
