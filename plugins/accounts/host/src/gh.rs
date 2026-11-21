//! GitHub's CLI, signed in to and out of from Settings → Agents' GitHub row (v1 agent-login.js LOGINS.gh, the
//! owner's Agents audit 2026-11-18: v2's row only said Signed in or not). gh isn't a coding agent: it isn't
//! registered as one; the sign-in commands find it here when they're asked for `gh` (`signer`). Its web login prints
//! a one-time code and a link, as Codex's does.

use std::sync::Arc;

use wisp_kernel::{BoxFuture, Ctx, Result, WireError};
use wisp_runtime_api::Harness;
use wisp_runtime_api::harness::{
    HarnessFound, HarnessInfo, HarnessLogin, HarnessRun, HarnessSys, Launch, ParserInput, ParserOutput, RunDriver,
};

/// The id the GitHub row's sign-in goes by.
pub const ID: &str = "gh";

/// v1 LOGINS.gh: the browser login over HTTPS for github.com (no SSH key), its logout and status.
pub fn login() -> HarnessLogin {
    let v = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    HarnessLogin {
        bin: "gh".into(),
        bin_env: "WISP_GH_BIN".into(),
        args: v(&["auth", "login", "--web", "--hostname", "github.com", "--git-protocol", "https", "--skip-ssh-key"]),
        logout: v(&["auth", "logout", "--hostname", "github.com"]),
        status: v(&["auth", "status", "--hostname", "github.com"]),
        keys: v(&["GH_TOKEN", "GITHUB_TOKEN"]),
        tool: true,
        ..HarnessLogin::default()
    }
}

/// What `gh auth status` said (v1 `said`): "not logged in" is out, "logged in" is in.
pub fn said(out: &str) -> Option<bool> {
    let o = out.to_lowercase();
    if o.contains("not logged in") || o.contains("not signed in") {
        Some(false)
    } else if o.contains("logged in") || o.contains("signed in") {
        Some(true)
    } else {
        None
    }
}

#[derive(Debug)]
pub struct GhTool {
    info: HarnessInfo,
    login: HarnessLogin,
}

impl Default for GhTool {
    fn default() -> Self {
        Self { info: HarnessInfo { id: ID.into(), name: "GitHub CLI".into(), ..HarnessInfo::default() }, login: login() }
    }
}

/// Never runs: gh signs in here, it doesn't run tasks.
struct NoRun;

impl RunDriver for NoRun {
    fn on<'a>(
        &'a mut self,
        _input: ParserInput<'a>,
        _now_ms: i64,
        _sys: &'a dyn HarnessSys,
        _out: &'a mut ParserOutput,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
}

impl Harness for GhTool {
    fn info(&self) -> &HarnessInfo {
        &self.info
    }
    fn detect<'a>(&'a self, _sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        Box::pin(async { HarnessFound::default() })
    }
    fn launch(&self, _run: &HarnessRun) -> Result<Launch> {
        Err(WireError::invalid("The GitHub CLI doesn't run tasks."))
    }
    fn driver(&self, _run: &HarnessRun) -> Box<dyn RunDriver> {
        Box::new(NoRun)
    }
    fn login(&self) -> Option<&HarnessLogin> {
        Some(&self.login)
    }
    fn signed_in(&self, out: &str, _code: Option<i32>) -> Option<bool> {
        said(out)
    }
}

/// After a sign-in to GitHub, git pushes with gh's login (v1 gitFromGh).
pub const SETUP_GIT: [&str; 4] = ["auth", "setup-git", "--hostname", "github.com"];

/// What git takes from the GitHub user (`gh api user`) when it has no name or email of its own: the name (else the
/// login), the email (else GitHub's no-reply address). Nothing it has already is changed.
pub fn identity(user_json: &str, has_name: bool, has_email: bool) -> Vec<(&'static str, String)> {
    let Ok(u) = serde_json::from_str::<serde_json::Value>(user_json) else { return Vec::new() };
    let Some(login) = u["login"].as_str().filter(|l| !l.is_empty()) else { return Vec::new() };
    let mut out = Vec::new();
    if !has_name {
        out.push(("user.name", u["name"].as_str().filter(|n| !n.is_empty()).unwrap_or(login).to_owned()));
    }
    if !has_email {
        let email = u["email"]
            .as_str()
            .filter(|e| !e.is_empty())
            .map_or_else(|| format!("{}+{login}@users.noreply.github.com", u["id"]), str::to_owned);
        out.push(("user.email", email));
    }
    out
}

/// v1 gitFromGh, after a sign-in to GitHub that worked: `gh auth setup-git`, then a missing git name or email from the
/// GitHub user. Best effort: a step that fails is logged; the sign-in stands.
pub async fn after_sign_in(proc: &wisp_proc::Proc, gh: &str) {
    let opts = || wisp_proc::ExecOpts { timeout: Some(std::time::Duration::from_secs(15)), ..wisp_proc::ExecOpts::default() };
    let args = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    match proc.exec(gh, &args(&SETUP_GIT), opts()).await {
        Ok(r) if r.exit.code != Some(0) => tracing::warn!(code = ?r.exit.code, "gh auth setup-git: {}", r.stdout_text().trim()),
        Err(e) => tracing::warn!("gh auth setup-git: {e}"),
        Ok(_) => {}
    }
    let has =
        async |k: &str| proc.exec("git", &args(&["config", "--global", k]), opts()).await.is_ok_and(|r| !r.stdout_text().trim().is_empty());
    let (name, email) = (has("user.name").await, has("user.email").await);
    if name && email {
        return;
    }
    let Ok(r) = proc.exec(gh, &args(&["api", "user"]), opts()).await else { return };
    for (k, v) in identity(&r.stdout_text(), name, email) {
        match proc.exec("git", &args(&["config", "--global", k, &v]), opts()).await {
            Ok(r) if r.exit.code != Some(0) => tracing::warn!(code = ?r.exit.code, "git config --global {k}"),
            Err(e) => tracing::warn!("git config --global {k}: {e}"),
            Ok(_) => {}
        }
    }
}

/// What a sign-in command signs in to: a harness, or GitHub's CLI for `gh`.
pub fn signer(ctx: &Ctx, agent: &str) -> Option<Arc<dyn Harness>> {
    crate::harness(ctx, agent).or_else(|| (agent == ID).then(|| Arc::new(GhTool::default()) as Arc<dyn Harness>))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// v1 signed in to and out of GitHub from its row: the same login, logout and status, never an SSH key.
    #[test]
    fn github_signs_in_and_out_from_its_row_as_v1() {
        let h = GhTool::default();
        let l = h.login().unwrap();
        assert_eq!((l.bin.as_str(), l.bin_env.as_str(), l.tool), ("gh", "WISP_GH_BIN", true));
        assert!(l.args.contains(&"--skip-ssh-key".to_owned()) && l.args.starts_with(&["auth".to_owned(), "login".to_owned()]));
        assert_eq!(l.logout, vec!["auth", "logout", "--hostname", "github.com"]);
        assert_eq!(l.status, vec!["auth", "status", "--hostname", "github.com"]);
        assert_eq!(h.signed_in("github.com\n  ✓ Logged in to github.com account me (keyring)", Some(0)), Some(true));
        assert_eq!(h.signed_in("You are not logged into any GitHub hosts. To log in, run: gh auth login", Some(1)), Some(false));
        assert!(h.launch(&HarnessRun::default()).is_err(), "gh never runs a task");
    }

    /// v1 gitFromGh: after signing in to GitHub, git pushes with gh's login (`gh auth setup-git`), and a git with no
    /// name or email of its own takes the GitHub user's (the no-reply address when the email is private)
    #[test]
    fn after_a_github_sign_in_git_takes_ghs_login_and_a_missing_name_or_email() {
        assert_eq!(SETUP_GIT, ["auth", "setup-git", "--hostname", "github.com"]);
        let u = r#"{"login":"ada","id":42,"name":"Ada L","email":null}"#;
        assert_eq!(
            identity(u, false, false),
            vec![("user.name", "Ada L".to_owned()), ("user.email", "42+ada@users.noreply.github.com".to_owned())]
        );
        assert_eq!(identity(u, true, false), vec![("user.email", "42+ada@users.noreply.github.com".to_owned())]);
        assert_eq!(identity(r#"{"login":"ada","id":42,"name":"","email":"a@x.dev"}"#, false, true), vec![("user.name", "ada".to_owned())]);
        assert_eq!(identity(u, true, true), Vec::new(), "git has both: nothing is changed");
        assert_eq!(identity("not json", false, false), Vec::new());
    }
}
