//! `wisp grid init|plan|deploy|deploys` (ADR-0147 §5): a team's hub as code, like Terraform. `init` writes a formula
//! folder with a `grid.yml` that says which hub it's for; `plan` and `deploy` evaluate the folder here and run the
//! same `resources.plan` / `resources.apply` as `wisp apply`, on the hub:
//!
//! - with `WISP_HUB_KEY` (a deploy key, `wisp team key`): straight to the hub's address, no wisp needed on this
//!   machine (CI);
//! - without: through this computer's wisp and its grid sign-in (`grid-client.hubCall`), as you.
//!
//! Either way the hub keeps one deploy at a time and its history (`wisp grid deploys`), with this folder's repo and
//! commit as the deploy's source.
use std::path::{Path, PathBuf};

use serde_json::json;
use wisp_cli_core::term::{BOLD, DIM, GREEN, paint};

use crate::client::{CliError, Client, Result};
use crate::code::Opts;
use crate::scripted::say;

pub const FILE: &str = "grid.yml";

/// What `grid.yml` says.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Target {
    pub grid: String,
    pub team: String,
    pub hub: String,
}

/// `grid.yml`'s three keys (`grid:`, `team:`, `hub:`), each `key: value` on its line; `#` starts a comment.
pub fn read_target(text: &str) -> Target {
    let mut t = Target::default();
    for line in text.lines() {
        let line = line.split(" #").next().unwrap_or_default().trim();
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim().trim_matches(['"', '\'']).to_owned();
            match k.trim() {
                "grid" => t.grid = v,
                "team" => t.team = v,
                "hub" => t.hub = v,
                _ => {}
            }
        }
    }
    t
}

/// A hub's address on a grid: `https://grid.trywisp.dev` → `https://acme.grid.trywisp.dev`.
pub fn hub_url(grid: &str, team: &str) -> String {
    match grid.trim_end_matches('/').split_once("://") {
        Some((scheme, host)) => format!("{scheme}://{team}.{host}"),
        None => format!("https://{team}.{grid}"),
    }
}

fn slug_ok(s: &str) -> bool {
    (2..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !s.starts_with('-')
}

/// The folder a hub command works on: the words' first, else this one; it must hold `grid.yml`.
fn folder(o: &Opts, cwd: &Path) -> Result<(PathBuf, Target)> {
    let dir = o.words.get(1).map_or_else(|| cwd.to_owned(), |d| cwd.join(d));
    let text = std::fs::read_to_string(dir.join(FILE))
        .map_err(|_| CliError::said(format!("no {FILE} in {}: wisp grid init <team> makes one", dir.display())))?;
    let t = read_target(&text);
    if t.hub.is_empty() || t.team.is_empty() {
        return Err(CliError::said(format!("{} needs team: and hub:", dir.join(FILE).display())));
    }
    Ok((dir, t))
}

/// Where this folder's formula comes from: `owner/repo@abc1234` (GitHub Actions' env first, then git), or the folder.
pub fn source(dir: &Path) -> String {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if let (Some(repo), Some(sha)) = (env("GITHUB_REPOSITORY"), env("GITHUB_SHA")) {
        return format!("{repo}@{}", &sha[..sha.len().min(7)]);
    }
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    let repo = git(&["remote", "get-url", "origin"])
        .map(|u| u.trim_end_matches(".git").rsplit([':', '/']).take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/"));
    match (repo, git(&["rev-parse", "--short", "HEAD"])) {
        (Some(r), Some(c)) => format!("{r}@{c}"),
        (None, Some(c)) => format!("{}@{c}", dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
        _ => dir.display().to_string(),
    }
}

/// `wisp grid <verb> …`.
pub async fn run(port: u16, o: &Opts, cwd: &Path) -> Result<i32> {
    match o.words.first().map(String::as_str) {
        Some("init") => init(port, o, cwd).await,
        Some(verb @ ("plan" | "deploy" | "diff" | "validate")) => apply(port, verb, o, cwd).await,
        Some("deploys") => deploys(port, o, cwd).await,
        _ => Err(CliError::usage(
            "wisp grid init <team> [dir]    a folder for a team's hub: projects, agents, triggers (grid.yml says which hub)
wisp grid plan [dir]           what deploying the folder would change on the hub
wisp grid deploy [dir]         plan, confirm, apply   (--yes in CI, --prune: delete what the folder no longer declares)
wisp grid deploys [dir]        the hub's deploys
in CI: WISP_HUB_KEY=<a key from wisp team key> wisp grid deploy --yes",
        )),
    }
}

/// The client a hub's commands go through: the deploy key's, straight there, or this wisp's, as you.
async fn hub_client(port: u16, t: &Target, src: &str) -> Result<Client> {
    if let Some(key) = std::env::var("WISP_HUB_KEY").ok().filter(|k| !k.trim().is_empty()) {
        return Ok(Client::hub_with_key(&t.hub, key.trim(), src));
    }
    let c = Client::connect(port, true).await?;
    Ok(c.via_hub(&t.team, src))
}

async fn apply(port: u16, verb: &str, o: &Opts, cwd: &Path) -> Result<i32> {
    let (dir, t) = folder(o, cwd)?;
    let src = source(&dir);
    let c = hub_client(port, &t, &src).await?;
    say(&paint(&crate::cli::style(), DIM, &format!("  ◌ {} hub at {} ({src})", t.team, t.hub)));
    let verb = if verb == "deploy" { "apply" } else { verb };
    let o = Opts { words: Vec::new(), files: vec![dir.display().to_string()], ..o.clone() };
    crate::code::apply(&c, verb, &o, cwd).await
}

async fn deploys(port: u16, o: &Opts, cwd: &Path) -> Result<i32> {
    let (_, t) = folder(o, cwd)?;
    let st = crate::cli::style();
    let v = if let Some(key) = std::env::var("WISP_HUB_KEY").ok().filter(|k| !k.trim().is_empty()) {
        let r = crate::client::grid_http()
            .get(format!("{}/_grid/deploys", t.hub))
            .bearer_auth(key.trim())
            .send()
            .await
            .map_err(|e| CliError::said(format!("the hub at {}: {e}", t.hub)))?;
        r.json().await.map_err(|e| CliError::said(format!("the hub at {}: {e}", t.hub)))?
    } else {
        Client::connect(port, true).await?.cmd("grid-client.hub", json!({ "op": "deploys", "team": t.team })).await?
    };
    if o.json {
        say(&v.to_string());
    } else {
        crate::hub_cli::print_deploys(&v, &st);
    }
    Ok(0)
}

async fn init(port: u16, o: &Opts, cwd: &Path) -> Result<i32> {
    let team = o.words.get(1).cloned().unwrap_or_default();
    if !slug_ok(&team) {
        return Err(CliError::usage("wisp grid init <team> [dir]: the team's slug (a-z, 0-9 and dashes); wisp team new <team> makes one"));
    }
    let dir = cwd.join(o.words.get(2).map_or("wisp-hub", String::as_str));
    // the grid: --grid, else the one this wisp is signed in to, else wisp Cloud
    let grid = match &o.grid {
        Some(g) => g.clone(),
        None => match Client::connect(port, false).await {
            Ok(c) => c.cmd("grid-client.status", json!({})).await.ok().and_then(|v| v["grid"].as_str().map(str::to_owned)),
            Err(_) => None,
        }
        .filter(|g| !g.is_empty())
        .unwrap_or_else(|| "https://grid.trywisp.dev".to_owned()),
    };
    let written = crate::grid_init::write(&dir, &team, &grid)?;
    let st = crate::cli::style();
    for f in &written.made {
        say(&format!("  {} {}", paint(&st, GREEN, "+"), f.display()));
    }
    for f in &written.kept {
        say(&format!("  {} {} {}", paint(&st, DIM, "="), f.display(), paint(&st, DIM, "(there already: left as it is)")));
    }
    say(&format!(
        "\n  {} for {}'s hub at {}\n  next: edit projects/ and triggers/, then {}",
        paint(&st, BOLD, &dir.display().to_string()),
        team,
        hub_url(&grid, &team),
        paint(&st, BOLD, &format!("wisp grid plan {}", dir.strip_prefix(cwd).unwrap_or(&dir).display()))
    ));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_yml_says_the_team_and_its_hub() {
        let t = read_target("# a hub\ngrid: https://grid.trywisp.dev\nteam: acme   # the team\nhub: \"https://acme.grid.trywisp.dev\"\n");
        assert_eq!(t, Target { grid: "https://grid.trywisp.dev".into(), team: "acme".into(), hub: "https://acme.grid.trywisp.dev".into() });
        assert_eq!(hub_url("https://grid.trywisp.dev/", "acme"), "https://acme.grid.trywisp.dev");
        assert_eq!(hub_url("http://wisp.localhost:8080", "acme"), "http://acme.wisp.localhost:8080");
    }
}
