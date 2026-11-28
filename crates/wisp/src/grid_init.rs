//! What `wisp grid init <team>` writes (ADR-0148 §5): a formula folder for a team's hub, ready to `wisp grid plan`.
//! It never overwrites a file: one that's there is kept and said. The GitHub workflow goes at the repo's root (where
//! GitHub reads it), pointed at the folder; with no repo, in the folder.
use std::path::{Path, PathBuf};

use crate::client::{CliError, Result};
use crate::grid_cli::{FILE, hub_url};

/// The files written and the ones kept.
#[derive(Debug, Default)]
pub struct Written {
    pub made: Vec<PathBuf>,
    pub kept: Vec<PathBuf>,
}

const PROJECT: &str = r"wisp: 1
# A repo the hub's agents work in. The hub clones it with its GH_TOKEN secret (wisp team secret <team> GH_TOKEN)
# and fetches it before each task.
projects:
  app:
    path: https://github.com/your-org/your-repo
";

const AGENT: &str = r"wisp: 1
# The team's agents: crew members that triggers and people hand work to. Each runs on the hub, in a sandbox of
# its own per job, with the harness the hub is signed in to (wisp team login <team> claude).
crew:
  triager:
    role: triager
    brief: >-
      You look at new issues first. Read the code the issue is about, say where the problem is and how you'd fix it,
      in a comment on the issue. Don't open a pull request and never merge.
    sandbox: job
";

const TRIGGER: &str = r#"wisp: 1
# What starts work on the hub. issue.opened polls GitHub with the hub's GH_TOKEN; see the triggers reference for
# pr.opened, pr.review_requested, ci.failed, schedule, webhook and the rest.
triggers:
  triage-new-issues:
    on: issue.opened
    project: app
    crew: triager
    prompt: "A new issue: {event.summary}. Find the code it's about and comment on the issue with what you found."
"#;

fn grid_yml(team: &str, grid: &str) -> String {
    format!(
        "# Which hub this folder deploys to (wisp grid plan | deploy). ADR-0148.\ngrid: {grid}\nteam: {team}\nhub: {}\n",
        hub_url(grid, team)
    )
}

fn readme(team: &str) -> String {
    format!(
        "# {team}'s hub\n\nThe agents, projects and triggers that run on {team}'s hub, as code. Each `*.wisp.yml` is a \
formula; `grid.yml` says which hub they go to.\n\n```sh\nwisp grid plan      # what deploying would change on the hub\nwisp grid deploy    # plan, confirm, apply\nwisp grid deploys   # who deployed what, from which commit\n```\n\nThe hub's secrets are set on the grid, never here: `wisp team secret {team} GH_TOKEN`, \
`wisp team login {team} claude`.\n\nCI: `wisp team key {team} ci` makes a deploy key; put it in the repo's secrets as `WISP_HUB_KEY`. \
The workflow plans every pull request and deploys every merge to main.\n"
    )
}

fn workflow(folder: &str) -> String {
    format!(
        r#"# Plans the hub's folder on every pull request (the plan as a comment) and deploys it on every merge to main.
name: wisp hub
on:
  pull_request:
    paths: ["{folder}**"]
  push:
    branches: [main]
    paths: ["{folder}**"]
permissions:
  contents: read
  pull-requests: write
jobs:
  hub:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: WispAgents/wisp/action@main
        with:
          hub: {dir}
          deploy: ${{{{ github.event_name == 'push' && 'apply' || 'plan' }}}}
          hub-key: ${{{{ secrets.WISP_HUB_KEY }}}}
"#,
        dir = if folder.is_empty() { "." } else { folder.trim_end_matches('/') }
    )
}

/// The repo `dir` is in (its top folder), if any.
fn repo_root(dir: &Path) -> Option<PathBuf> {
    let mut d = dir.to_path_buf();
    loop {
        if d.join(".git").exists() {
            return Some(d);
        }
        if !d.pop() {
            return None;
        }
    }
}

/// Writes the folder for `team`'s hub on `grid` into `dir`.
pub fn write(dir: &Path, team: &str, grid: &str) -> Result<Written> {
    let root = repo_root(dir.parent().unwrap_or(dir)).unwrap_or_else(|| dir.to_path_buf());
    let rel = dir.strip_prefix(&root).map(|p| p.display().to_string()).unwrap_or_default();
    let rel = if rel.is_empty() { String::new() } else { format!("{rel}/") };
    let files: Vec<(PathBuf, String)> = vec![
        (dir.join(FILE), grid_yml(team, grid)),
        (dir.join("README.md"), readme(team)),
        (dir.join("projects/app.wisp.yml"), PROJECT.to_owned()),
        (dir.join("agents/triager.wisp.yml"), AGENT.to_owned()),
        (dir.join("triggers/issues.wisp.yml"), TRIGGER.to_owned()),
        (root.join(".github/workflows/wisp-hub.yml"), workflow(&rel)),
    ];
    let mut w = Written::default();
    for (path, text) in files {
        if path.exists() {
            w.kept.push(path);
            continue;
        }
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| CliError::said(format!("{}: {e}", p.display())))?;
        }
        std::fs::write(&path, text).map_err(|e| CliError::said(format!("{}: {e}", path.display())))?;
        w.made.push(path);
    }
    Ok(w)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn init_writes_the_folder_once_and_the_workflow_at_the_repos_root() {
        let base = std::env::temp_dir().join(format!("wisp-grid-init-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join(".git")).unwrap();
        let dir = base.join("ops/hub");
        let w = write(&dir, "acme", "https://grid.trywisp.dev").unwrap();
        assert_eq!(w.made.len(), 6, "{:?}", w.made);
        let grid = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(grid.contains("team: acme") && grid.contains("hub: https://acme.grid.trywisp.dev"), "{grid}");
        let wf = std::fs::read_to_string(base.join(".github/workflows/wisp-hub.yml")).unwrap();
        assert!(wf.contains("hub: ops/hub") && wf.contains(r#"paths: ["ops/hub/**"]"#), "{wf}");
        assert!(wf.contains("${{ secrets.WISP_HUB_KEY }}"), "{wf}");
        // a second time keeps everything, a file someone edited included
        std::fs::write(dir.join("triggers/issues.wisp.yml"), "mine").unwrap();
        let again = write(&dir, "acme", "https://grid.trywisp.dev").unwrap();
        assert!(again.made.is_empty() && again.kept.len() == 6);
        assert_eq!(std::fs::read_to_string(dir.join("triggers/issues.wisp.yml")).unwrap(), "mine");
        let _ = std::fs::remove_dir_all(&base);
    }
}
