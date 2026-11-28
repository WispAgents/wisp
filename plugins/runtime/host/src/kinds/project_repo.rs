//! A project by its repo (ADR-0147 §7): `repo: https://github.com/acme/api` and no `path`. Applied, wisp clones it into
//! a folder of its own (`<wisp home>/repos/<project>/<repo>`, through `git.clone`, which tries `gh repo clone` first so a
//! hub's `GH_TOKEN` reaches a private repo) and keeps that folder as the project's path. A formula that names the repo
//! only plans as unchanged once it's cloned: its path, left out, is that folder ([`ResourceKind::normalize_named`]).
//! A project with a `path` of its own is a folder, as before; `repo` beside it is only a note.
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use serde_json::{Value, json};
use wisp_kernel::{BoxFuture, Call, Ctx, Result};
use wisp_resources_api::{KindInfo, Op, OpKind, Ref, Resource, ResourceKind, StoredKind};

/// The folder a repo project is cloned into: `<home>/repos/<project>/<the repo's name>`.
pub fn clone_dir(home: &std::path::Path, project: &str, repo: &str) -> PathBuf {
    let name = repo.trim_end_matches('/').trim_end_matches(".git").rsplit(['/', ':']).next().unwrap_or(project);
    home.join("repos").join(project).join(if name.is_empty() { project } else { name })
}

/// A spec's `repo`, when it has one and no folder of its own (`path` left out or empty).
pub fn repo_only(spec: &Value) -> Option<&str> {
    let repo = spec.get("repo").and_then(Value::as_str).map(str::trim).filter(|r| !r.is_empty())?;
    let path = spec.get("path").and_then(Value::as_str).map(str::trim).unwrap_or_default();
    path.is_empty().then_some(repo)
}

/// A repo URL wisp can clone: `https://…`, `git@…:…`, or GitHub's `owner/repo`.
pub fn repo_ok(repo: &str) -> bool {
    let r = repo.trim();
    r.starts_with("https://") || r.starts_with("http://") || r.starts_with("git@") || {
        let mut p = r.split('/');
        matches!((p.next(), p.next(), p.next()), (Some(o), Some(n), None) if !o.is_empty() && !n.is_empty())
    }
}

/// The project kind, with repo projects cloned on apply.
pub struct ProjectKind {
    pub inner: Arc<StoredKind>,
    pub ctx: Ctx,
    home: OnceLock<PathBuf>,
}

impl ProjectKind {
    pub fn new(inner: Arc<StoredKind>, ctx: Ctx) -> Self {
        Self { inner, ctx, home: OnceLock::new() }
    }

    fn home(&self) -> Option<&PathBuf> {
        if let Some(h) = self.home.get() {
            return Some(h);
        }
        let h = wisp_kernel::services::paths(&self.ctx).ok()?.home().to_owned();
        Some(self.home.get_or_init(|| h))
    }

    /// The repo project's folder, cloned now if it isn't there.
    async fn clone_if_needed(&self, call: &Call, name: &str, repo: &str, dir: &std::path::Path) -> Result<()> {
        if tokio::fs::metadata(dir.join(".git")).await.is_ok() {
            return Ok(());
        }
        let parent = dir.parent().map(|p| p.display().to_string()).unwrap_or_default();
        let url = if repo.contains("://") || repo.starts_with("git@") { repo.to_owned() } else { format!("https://github.com/{repo}") };
        let got = self
            .ctx
            .run::<wisp_git_api::commands::Clone>(call, wisp_git_api::CloneRequest { url, parent })
            .await
            .map_err(|e| wisp_kernel::WireError::invalid(format!("project {name}: {repo} couldn't be cloned: {}", e.message)))?;
        let at = got.dir.as_str();
        if std::path::Path::new(at) != dir {
            return Err(wisp_kernel::WireError::invalid(format!(
                "project {name}: {repo} was cloned to {at}, not {}: take that folder out and apply again",
                dir.display()
            )));
        }
        Ok(())
    }
}

impl ResourceKind for ProjectKind {
    fn info(&self) -> KindInfo {
        self.inner.info()
    }
    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        self.inner.read(call)
    }
    fn plan(&self, current: &[Resource], desired: &[Resource], prune: bool) -> Option<Vec<wisp_resources_api::Op>> {
        self.inner.plan(current, desired, prune)
    }
    fn normalize(&self, spec: &mut Value) {
        self.inner.normalize(spec);
    }
    /// A repo project's path, left out, is the folder wisp clones it into.
    fn normalize_named(&self, name: &str, spec: &mut Value) {
        if let (Some(repo), Some(home)) = (repo_only(spec).map(str::to_owned), self.home())
            && let Some(o) = spec.as_object_mut()
        {
            o.insert("path".into(), json!(clone_dir(home, name, &repo).display().to_string()));
        }
        self.inner.normalize_named(name, spec);
    }
    /// A repo project's folder is made on apply, so a missing one isn't a problem; a repo wisp can't clone is.
    fn check(&self, r: &Resource) -> Vec<String> {
        if let Some(repo) = r.spec.get("repo").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
            if !repo_ok(repo) {
                return vec![format!("repo: {repo} isn't a repo URL (https://…, git@…, or owner/repo)")];
            }
            let cloned_here = self.home().is_some_and(|h| {
                r.spec.get("path").and_then(Value::as_str).is_some_and(|p| std::path::Path::new(p) == clone_dir(h, &r.name, repo))
            });
            if cloned_here || repo_only(&r.spec).is_some() {
                return Vec::new();
            }
        }
        self.inner.check(r)
    }
    fn check_all(&self, after: &[Resource]) -> Vec<String> {
        self.inner.check_all(after)
    }
    fn refs(&self, r: &Resource) -> Vec<Ref> {
        self.inner.refs(r)
    }
    fn refers_to(&self) -> Vec<String> {
        self.inner.refers_to()
    }
    fn also_named<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Vec<String>> {
        self.inner.also_named(call)
    }
    fn describe(&self, op: &Op) -> Option<String> {
        let repo = op.spec.get("repo").and_then(Value::as_str).filter(|_| op.op == OpKind::Create)?;
        Some(format!("clones {repo}"))
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            if op.op != OpKind::Delete
                && let Some(repo) = op.spec.get("repo").and_then(Value::as_str).filter(|s| !s.trim().is_empty())
                && let Some(home) = self.home()
            {
                let dir = clone_dir(home, &op.name, repo);
                let path = op.spec.get("path").and_then(Value::as_str).unwrap_or_default();
                if path.is_empty() || std::path::Path::new(path) == dir {
                    self.clone_if_needed(call, &op.name, repo, &dir).await?;
                    let mut op = op.clone();
                    if let Some(o) = op.spec.as_object_mut() {
                        o.insert("path".into(), json!(dir.display().to_string()));
                    }
                    return self.inner.apply(call, &op).await;
                }
            }
            self.inner.apply(call, op).await
        })
    }
    fn rename<'a>(&'a self, call: &'a Call, from: &'a str, to: &'a str) -> BoxFuture<'a, Result<()>> {
        self.inner.rename(call, from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repo_project_has_a_folder_of_its_own_and_a_path_wins() {
        let home = std::path::Path::new("/home/box/.wisp");
        assert_eq!(clone_dir(home, "app", "https://github.com/acme/api.git"), home.join("repos/app/api"));
        assert_eq!(clone_dir(home, "app", "git@github.com:acme/api.git"), home.join("repos/app/api"));
        assert_eq!(clone_dir(home, "app", "acme/api"), home.join("repos/app/api"));
        assert_eq!(repo_only(&json!({ "repo": "acme/api" })), Some("acme/api"));
        assert_eq!(repo_only(&json!({ "repo": "acme/api", "path": "/w/api" })), None, "a folder of its own");
        assert_eq!(repo_only(&json!({ "path": "/w/api" })), None);
        assert!(repo_ok("acme/api") && repo_ok("https://gitlab.com/a/b") && repo_ok("git@github.com:a/b.git"));
        assert!(!repo_ok("api") && !repo_ok("a/b/c") && !repo_ok("/w/api"));
    }
}
