//! A task's folder on another computer (v1 services/places.js `originOf`, `checkout`, `discoverRepos`): the folder's
//! repo here (`git.origin`), and on the other computer its checkout of that repo, found or cloned (`git.checkout`).

use std::path::{Path, PathBuf};

use wisp_git_api::{CheckoutArgs, CloneRequest, Cloned, Origin};
use wisp_git_core::repo_url::{REPO_ROOTS, native_folder, same_repo};

use crate::error::Result;
use crate::service::GitServiceImpl;

/// v1 `discoverRepos`: at most this many repos looked at in the usual folders.
const MAX_FOUND: usize = 60;

impl GitServiceImpl {
    /// The folder's origin and its checkout's top folder; '' for both when it isn't a repo or has no origin.
    pub async fn origin_at(&self, path: &str) -> Origin {
        let dir = Path::new(path);
        let origin = self.git.quiet(dir, &["remote", "get-url", "origin"]).await.trim().to_owned();
        if origin.is_empty() {
            return Origin::default();
        }
        // git prints `C:/x` on Windows: the folder is given back as the OS spells it (`C:\x`)
        let root = native_folder(self.git.quiet(dir, &["rev-parse", "--show-toplevel"]).await.trim());
        Origin { origin, root }
    }

    /// Projects' "Your GitHub repos" (v1 GET /projects/github): `gh repo list` as the signed-in user, minus the repos of
    /// the projects at `paths`, last pushed first. Empty when gh isn't installed or signed in (as v1).
    pub async fn github_repos(&self, home: &str, paths: &[String]) -> Vec<wisp_git_api::GithubRepo> {
        let args =
            crate::run::args(&["repo", "list", "--limit", "40", "--json", "nameWithOwner,description,isPrivate,isFork,pushedAt,url"]);
        let o = crate::run::Opts::timeout(20);
        let Ok(r) = self.git.exec_bin("gh", Path::new(home), &args, o).await else { return Vec::new() };
        if !r.exit.success() {
            return Vec::new();
        }
        let listed: Vec<wisp_git_api::GhListed> = serde_json::from_slice(&r.stdout).unwrap_or_default();
        let mut have = Vec::new();
        for p in paths {
            let o = self.origin_at(p).await.origin;
            if !o.is_empty() {
                have.push(o);
            }
        }
        wisp_git_core::repo_url::github_repos(listed, &have, 24)
    }

    /// This computer's checkout of `url`: a known folder or one in the usual repo folders with that origin, else a clone
    /// into `parent` (where clones go).
    ///
    /// # Errors
    /// The clone failed (no such repo, no access).
    pub async fn checkout(&self, a: CheckoutArgs, home: &str, parent: String) -> Result<Cloned> {
        for dir in a.known.iter().map(PathBuf::from).chain(found(home).await) {
            let origin = self.git.quiet(&dir, &["remote", "get-url", "origin"]).await;
            if same_repo(origin.trim(), &a.url) {
                return Ok(Cloned { dir: dir.display().to_string(), existed: true });
            }
        }
        self.clone_repo_into(CloneRequest { url: a.url, parent }).await
    }
}

/// Repos up to two levels under the usual folders (v1 `discoverRepos`), at most `MAX_FOUND`.
async fn found(home: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for r in REPO_ROOTS {
        walk(&Path::new(home).join(r), 0, &mut out).await;
        if out.len() >= MAX_FOUND {
            break;
        }
    }
    out
}

async fn walk(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
    if out.len() >= MAX_FOUND {
        return;
    }
    if tokio::fs::metadata(dir.join(".git")).await.is_ok() {
        out.push(dir.to_owned());
        return;
    }
    if depth >= 2 {
        return;
    }
    let Ok(mut rd) = tokio::fs::read_dir(dir).await else { return };
    let mut subs = Vec::new();
    while let Ok(Some(e)) = rd.next_entry().await {
        let name = e.file_name();
        let skip = name.to_string_lossy().starts_with('.') || name == "node_modules";
        if !skip && e.file_type().await.is_ok_and(|t| t.is_dir()) {
            subs.push(e.path());
        }
    }
    subs.sort();
    for s in subs {
        Box::pin(walk(&s, depth + 1, out)).await;
    }
}
