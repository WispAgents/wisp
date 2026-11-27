//! `history.push`: the `.wisp/` files a save wrote, committed and pushed so they don't stay on this computer.
//!
//! Two places a save lands (`config::root_of_task`), two ways out:
//! - the task's own worktree, its work not shipped yet. Once the task's branch is on its remote, a commit of just
//!   these files is built on that branch's remote tip in a scratch index and pushed as a fast-forward (its PR, if
//!   any, gets them). The worktree's HEAD, index and files are never touched, and the agent's own unpushed commits
//!   never go out with it: shipping those is the user's call, not a side effect of saving history. With the
//!   branch not on the remote yet (nothing shipped), it goes out as the main checkout's does, below: no branch is
//!   ever made under the task's name;
//! - the main checkout (the work merged, or the worktree gone): a commit built on the remote's base branch in a
//!   scratch index, so the user's checkout, branch and index are never touched, pushed to `wisp/history-<id>`
//!   with a PR opened by `gh`. The local copies are then taken out (restored to HEAD when tracked): left in place
//!   they'd make the next `git pull` refuse ("untracked working tree files would be overwritten"); they come
//!   back with that pull once the PR merges. Without a PR (no `gh`, or it failed) they stay: nothing would bring
//!   them back, and the answer names where to open the PR by hand (`compare`).
//!
//! Either way it pushes to the task's own remote (`wisp.worktrees`' `remote`, else `origin`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_history_api::{self as api, Pushed};
use wisp_history_core::repo;
use wisp_kernel::{Call, Ctx, Result, WireError};
use wisp_proc::{Env, ExecOpts, Proc};

use crate::{calls, git};

/// The remote the task's worktree came from (`wisp.worktrees`' `remote`), else `origin`: a fork's checkout may
/// have only `upstream`, and pushing to an `origin` that isn't there (or is someone else's) is wrong either way.
pub(crate) fn remote_of(wt: &Value) -> &str {
    wt.get("remote").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).unwrap_or("origin")
}

/// `bin args` in `cwd` (with `env` added): stdout, or what it said on stderr.
async fn exec(p: &Proc, bin: &str, cwd: &Path, env: &[(&str, &str)], args: &[&str]) -> std::result::Result<String, String> {
    let mut e = Env::default();
    for (k, v) in env {
        e = e.with(*k, *v);
    }
    let opts = ExecOpts {
        cwd: Some(cwd.to_owned()),
        env: e,
        timeout: Some(Duration::from_secs(120)),
        max_output: 16 << 20,
        ..ExecOpts::default()
    };
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    match p.exec(bin, &args, opts).await {
        Ok(r) if r.exit.success() => Ok(r.stdout_text().trim().to_owned()),
        Ok(r) => Err(String::from_utf8_lossy(&r.stderr).trim().to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

async fn git_ok(p: &Proc, cwd: &Path, env: &[(&str, &str)], args: &[&str], what: &str) -> Result<String> {
    exec(p, "git", cwd, env, args).await.map_err(|e| WireError::invalid(format!("{what}: {e}")))
}

/// The branch a history PR goes on: one per task, so pushing again updates it.
pub(crate) fn history_branch(id: &str) -> String {
    format!("wisp/history-{}", repo::short(id))
}

/// The commit message: what it is and which task, without the `Wisp-Task:` trailer (that marks the task's own
/// code commits, and this one isn't).
pub(crate) fn message(title: &str, id: &str) -> String {
    let title = if title.trim().is_empty() { repo::short(id) } else { title.trim() };
    format!("wisp: save task \"{title}\" to the repo's history\n\nIts spec, report and transcript in .wisp/ (task {id}).")
}

/// `<remote>/HEAD`'s branch (`main` from `origin/main`), else `main` or `master` when the remote has it.
async fn base_branch(p: &Proc, root: &Path, remote: &str) -> Option<String> {
    if let Ok(r) = exec(p, "git", root, &[], &["symbolic-ref", "--short", &format!("refs/remotes/{remote}/HEAD")]).await
        && let Some(b) = r.strip_prefix(&format!("{remote}/"))
        && !b.is_empty()
    {
        return Some(b.to_owned());
    }
    for b in ["main", "master"] {
        if exec(p, "git", root, &[], &["rev-parse", "--verify", "--quiet", &format!("refs/remotes/{remote}/{b}")]).await.is_ok() {
            return Some(b.to_owned());
        }
    }
    None
}

/// Whether `root` is inside the task's live worktree (its work not shipped: the save went there).
async fn in_worktree(root: &Path, wt: &Value) -> Option<String> {
    let path = wt.get("path").and_then(Value::as_str).filter(|s| !s.is_empty())?;
    let branch = wt.get("branch").and_then(Value::as_str).filter(|s| !s.is_empty())?;
    let (a, b) = (wisp_proc::canonical(root).await.ok()?, wisp_proc::canonical(Path::new(path)).await.ok()?);
    a.starts_with(&b).then(|| branch.to_owned())
}

pub(crate) async fn push(ctx: &Ctx, call: &Call, a: &api::PushArgs) -> Result<Pushed> {
    let task = calls::task(ctx, call, &a.id).await?;
    let id = task.id.as_str().to_owned();
    let tracked = task.ext_of(api::EXT).get("tracked").cloned().unwrap_or(Value::Null);
    let repo_dir = tracked.get("repo").and_then(Value::as_str).filter(|s| !s.is_empty());
    let files: Vec<String> = tracked
        .get("files")
        .and_then(Value::as_array)
        .map(|v| v.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let (Some(repo_dir), false) = (repo_dir, files.is_empty()) else {
        return Err(WireError::invalid(format!("{id} isn't saved to its repo yet: Save to repo first")));
    };
    let root = PathBuf::from(repo_dir);
    // the worktree it saved into, cleaned up since: git would run in no repo at all and answer "no remote"
    if !tokio::fs::metadata(&root).await.is_ok_and(|m| m.is_dir()) {
        return Err(WireError::invalid(format!(
            "the folder it saved into ({}) is gone (its worktree was cleaned up): Save to repo again",
            root.display()
        )));
    }
    let p = crate::proc(ctx)?;
    let wt = task.ext_of("wisp.worktrees");
    let remote = remote_of(wt);
    git_ok(&p, &root, &[], &["remote", "get-url", remote], &format!("no remote {remote} to push to")).await?;
    let earlier = earlier(task.ext_of(api::EXT), &files);
    let gh = git::gh_bin();
    let out = if let Some(branch) = in_worktree(&root, wt).await {
        onto_branch(&p, &gh, &root, remote, &branch, (&id, &task.title), &files, earlier).await?
    } else {
        as_pr(&p, &gh, &root, remote, (&id, &task.title), &files, earlier).await?
    };
    let mark = json!({ "pushed": { "branch": out.branch, "commit": out.commit, "pr": out.pr, "compare": out.compare, "noPr": out.no_pr } });
    calls::warn("marking the task pushed (setExt)", calls::update_ext(ctx, call, &id, |slot| Some(calls::merged(slot, &mark))).await);
    Ok(out)
}

/// What the last push did, from the task's slot (`pushed`): what a second push answers once that one took the files out.
pub(crate) fn earlier(slot: &Value, files: &[String]) -> Option<Pushed> {
    let p = slot.get("pushed")?;
    let text = |k: &str| p.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned);
    Some(Pushed {
        // '' when the base had them already (nothing pushed, the files taken out all the same)
        branch: p.get("branch").and_then(Value::as_str)?.to_owned(),
        commit: text("commit")?,
        files: files.to_vec(),
        pr: text("pr"),
        removed: Vec::new(),
        compare: text("compare"),
        no_pr: p.get("noPr").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// The files that are still there (a later untrack or a hand edit may have taken some).
async fn present(root: &Path, files: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for f in files {
        if tokio::fs::metadata(root.join(f)).await.is_ok_and(|m| m.is_file()) {
            out.push(f.clone());
        }
    }
    out
}

/// The task's worktree: a commit of just these files on the remote tip of its branch, pushed as a fast-forward.
/// Built in a scratch index from blobs (`hash-object`), so the worktree is left as it was, `.wisp/` may be
/// gitignored, and whatever the agent committed but didn't push stays unpushed. No branch on the remote yet: as
/// the main checkout does ([`as_pr`]).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn onto_branch(
    p: &Proc,
    gh: &str,
    root: &Path,
    remote: &str,
    branch: &str,
    (id, title): (&str, &str),
    files: &[String],
    earlier: Option<Pushed>,
) -> Result<Pushed> {
    let have = present(root, files).await;
    if have.is_empty() {
        return earlier.ok_or_else(|| WireError::invalid("the files it saved aren't in the repo any more: Save to repo again"));
    }
    let head = format!("refs/heads/{branch}");
    let listed = git_ok(p, root, &[], &["ls-remote", "--heads", remote, &head], "git ls-remote").await?;
    let Some(tip) = listed.lines().find_map(|l| l.split_once('\t').filter(|(_, r)| *r == head).map(|(sha, _)| sha.to_owned())) else {
        return as_pr(p, gh, root, remote, (id, title), files, earlier).await;
    };
    git_ok(p, root, &[], &["fetch", "--quiet", remote, &head], "git fetch").await?;
    let tree = scratch_tree(p, root, id, &tip, &have).await?;
    let tip_tree = git_ok(p, root, &[], &["rev-parse", &format!("{tip}^{{tree}}")], "git rev-parse").await?;
    let commit = if tree == tip_tree {
        // the branch has them already, as saved
        tip
    } else {
        let commit = git_ok(p, root, &[], &["commit-tree", &tree, "-p", &tip, "-m", &message(title, id)], "git commit-tree").await?;
        // no --force: a push that landed meanwhile is refused, never overwritten
        git_ok(p, root, &[], &["push", remote, &format!("{commit}:{head}")], "git push").await?;
        commit
    };
    let pr = git::pr_of_with(p, gh, root, std::slice::from_ref(&commit), branch).await;
    Ok(Pushed { branch: branch.to_owned(), commit, files: have, pr, removed: Vec::new(), compare: None, no_pr: false })
}

/// The tree of `base` with these files over it, built in a scratch index so the checkout's own index, branch and
/// files are never touched (and ignore rules don't apply: the blobs go in by hash).
async fn scratch_tree(p: &Proc, root: &Path, id: &str, base: &str, files: &[String]) -> Result<String> {
    // paths in the index are from git's top; the repo's `.wisp/` may sit in a subfolder of it
    let prefix = git_ok(p, root, &[], &["rev-parse", "--show-prefix"], "git rev-parse").await?;
    // one per push, not per process: two pushes of one task at once shared it, and one's read-tree emptied the other's
    let index = std::env::temp_dir().join(format!("wisp-history-{}-{}.index", repo::short(id), wisp_kernel::ids::uuid7()));
    let index_s = index.display().to_string();
    let env = [("GIT_INDEX_FILE", index_s.as_str())];
    let built = async {
        git_ok(p, root, &env, &["read-tree", base], "git read-tree").await?;
        for f in files {
            // through the repo's filters (an LFS store's transcripts go in as pointers)
            let blob = git_ok(p, root, &[], &["hash-object", "-w", "--", f], "git hash-object").await?;
            let at = format!("100644,{blob},{prefix}{f}");
            git_ok(p, root, &env, &["update-index", "--add", "--cacheinfo", &at], "git update-index").await?;
        }
        git_ok(p, root, &env, &["write-tree"], "git write-tree").await
    }
    .await;
    let _ = tokio::fs::remove_file(&index).await;
    built
}

/// The main checkout: a commit on the remote's base made in a scratch index, pushed to its own branch, a PR.
/// `earlier` is what the last push did: it took the files out, so pushing again answers that, not "Save again".
pub(crate) async fn as_pr(
    p: &Proc,
    gh: &str,
    root: &Path,
    remote: &str,
    (id, title): (&str, &str),
    files: &[String],
    earlier: Option<Pushed>,
) -> Result<Pushed> {
    let files = present(root, files).await;
    if files.is_empty() {
        return earlier.ok_or_else(|| WireError::invalid("the files it saved aren't in the repo any more: Save to repo again"));
    }
    let msg = &message(title, id);
    let base = base_branch(p, root, remote)
        .await
        .ok_or_else(|| WireError::invalid(format!("no base branch on {remote} (main or {remote}/HEAD)")))?;
    git_ok(p, root, &[], &["fetch", "--quiet", remote, &base], "git fetch").await?;
    let base_sha = git_ok(p, root, &[], &["rev-parse", &format!("refs/remotes/{remote}/{base}^{{commit}}")], "git rev-parse").await?;
    let tree = scratch_tree(p, root, id, &base_sha, &files).await?;
    let branch = history_branch(id);
    let base_tree = git_ok(p, root, &[], &["rev-parse", &format!("{base_sha}^{{tree}}")], "git rev-parse").await?;
    let (commit, pr, pushed_to) = if tree == base_tree {
        // already on the remote's base, as saved: nothing to push, the local copies only wait for a pull
        (base_sha, None, String::new())
    } else {
        let commit = git_ok(p, root, &[], &["commit-tree", &tree, "-p", &base_sha, "-m", msg], "git commit-tree").await?;
        // the task's own branch: pushing again (after a save again) replaces what it pushed before
        git_ok(p, root, &[], &["push", "--force", remote, &format!("{commit}:refs/heads/{branch}")], "git push").await?;
        let pr = open_pr(p, gh, root, &branch, &base, title, id).await;
        (commit, pr, branch)
    };
    // taken out only when a pull brings them back: a PR to merge, or the base has them already. A branch with no PR
    // would leave them nowhere but on a remote branch nobody merges
    if pr.is_none() && !pushed_to.is_empty() {
        let url = exec(p, "git", root, &[], &["remote", "get-url", remote]).await.unwrap_or_default();
        let compare = web_of(&url).and_then(|w| new_pr_page(&w, &base, &pushed_to));
        return Ok(Pushed { branch: pushed_to, commit, files, pr, removed: Vec::new(), compare, no_pr: true });
    }
    let removed = take_out(p, root, &files).await;
    Ok(Pushed { branch: pushed_to, commit, files, pr, removed, compare: None, no_pr: false })
}

/// The remote's web page (`https://github.com/o/r`) from its URL: https, `git@host:o/r.git` or `ssh://git@host/o/r`.
/// None for a path or anything else (no page to open a PR on).
pub(crate) fn web_of(remote: &str) -> Option<String> {
    let r = remote.trim();
    let (host, path) = if let Some(rest) = r.strip_prefix("https://").or_else(|| r.strip_prefix("http://")) {
        rest.split_once('/')?
    } else if let Some(rest) = r.strip_prefix("ssh://") {
        let (host, path) = rest.split_once('/')?;
        (host.split_once(':').map_or(host, |(h, _)| h), path)
    } else if !r.contains("://") && !r.starts_with('/') && !r.starts_with('.') {
        r.split_once(':')?
    } else {
        return None;
    };
    // no user (`git@`, `x-access-token:…@`): never a credential in a link
    let host = host.rsplit('@').next().unwrap_or(host);
    let path = path.trim_matches('/').trim_end_matches(".git");
    // a host has a dot (`C:/repo` is a Windows path, not host `C`)
    (host.contains('.') && !path.is_empty() && !r.contains('\\')).then(|| format!("https://{host}/{path}"))
}

/// Where to open a PR from `head` into `base` by hand on the remote's web page `web`, in its host's own format:
/// GitHub's compare page, GitLab's new merge request, Bitbucket's new pull request. None for any other host (its
/// format is unknown: a wrong link is worse than none).
pub(crate) fn new_pr_page(web: &str, base: &str, head: &str) -> Option<String> {
    let host = web.strip_prefix("https://")?.split('/').next()?.to_ascii_lowercase();
    let q = |s: &str| s.replace('%', "%25").replace('&', "%26").replace('#', "%23").replace('+', "%2B").replace(' ', "%20");
    if host == "github.com" || host.starts_with("github.") {
        Some(format!("{web}/compare/{base}...{head}?expand=1"))
    } else if host == "gitlab.com" || host.starts_with("gitlab.") {
        Some(format!(
            "{web}/-/merge_requests/new?merge_request%5Bsource_branch%5D={}&merge_request%5Btarget_branch%5D={}",
            q(head),
            q(base)
        ))
    } else if host == "bitbucket.org" {
        Some(format!("{web}/pull-requests/new?source={}&dest={}", q(head), q(base)))
    } else {
        None
    }
}

/// `gh pr create`, or the PR already open on that branch.
async fn open_pr(p: &Proc, gh: &str, root: &Path, branch: &str, base: &str, title: &str, id: &str) -> Option<String> {
    let t = if title.trim().is_empty() { repo::short(id) } else { title.trim() };
    let pr_title = format!("wisp: save task \"{t}\" to the repo's history");
    let body = format!(
        "Its spec, report and transcript in `.wisp/`, scrubbed of keys and private paths (task `{id}`).\n\nPushed from wisp's Save to repo."
    );
    let made = exec(p, gh, root, &[], &["pr", "create", "--head", branch, "--base", base, "--title", &pr_title, "--body", &body]).await;
    match made {
        Ok(out) => out.lines().rev().find(|l| l.starts_with("http")).map(str::to_owned),
        Err(e) => {
            tracing::info!(branch, "history: gh pr create: {e}");
            git::pr_of_with(p, gh, root, &[], branch).await
        }
    }
}

/// The local copies out of the checkout: tracked ones back to HEAD, new ones deleted.
async fn take_out(p: &Proc, root: &Path, files: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for f in files {
        let tracked = exec(p, "git", root, &[], &["ls-files", "--error-unmatch", "--", f]).await.is_ok();
        let done = if tracked {
            exec(p, "git", root, &[], &["checkout", "HEAD", "--", f]).await.is_ok()
        } else {
            tokio::fs::remove_file(root.join(f)).await.is_ok()
        };
        if done {
            out.push(f.clone());
        }
    }
    out
}
