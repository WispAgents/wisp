//! Keeping a harness up to date (TRY-17): is a newer version out, and how this install updates. Pure: the host reads
//! `npm view <package> version`, finds the binary's real path and runs what this decides.

/// `latest` is newer than what's installed (both as a CLI prints them: `2.1.30 (Claude Code)`, `codex-cli 0.58.0`,
/// read by the harness watch's `version_parts`). Unreadable on either side: not newer (never nag on a guess).
#[must_use]
pub fn newer(installed: &str, latest: &str) -> bool {
    let pad = |v: &str| {
        let mut p = crate::watch::version_parts(v);
        (!p.is_empty()).then(|| {
            p.resize(3.max(p.len()), 0);
            p
        })
    };
    matches!((pad(installed), pad(latest)), (Some(a), Some(b)) if b > a)
}

/// How this install updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// it lives in wisp's own tools folder: `npm install --prefix <tools> <package>@latest` (accounts.install)
    Tools,
    /// a global npm install: `npm install -g <package>@latest`
    NpmGlobal,
    /// its own updater, on the binary wisp found (`claude update`)
    SelfUpdate(Vec<String>),
    /// wisp can't update it safely (Homebrew, a package manager, a copy it can't place): the command to run
    Manual(String),
}

/// How to update the binary at `bin` (its real path, links followed), given wisp's tools folder, the npm package and
/// the harness's own updater.
#[must_use]
pub fn method(bin: &str, tools: &str, package: &str, self_update: Option<&[String]>) -> Method {
    let bin = bin.replace('\\', "/");
    let tools = tools.replace('\\', "/");
    if !tools.is_empty() && bin.starts_with(&format!("{}/", tools.trim_end_matches('/'))) {
        return Method::Tools;
    }
    if ["/Cellar/", "/Caskroom/", "/homebrew/", "/linuxbrew/"].iter().any(|b| bin.contains(b)) {
        return Method::Manual("brew upgrade".into());
    }
    // its own updater first: it knows its native install as well as an npm one
    if let Some(args) = self_update.filter(|a| !a.is_empty()) {
        return Method::SelfUpdate(args.to_vec());
    }
    if bin.contains(&format!("/node_modules/{package}/")) {
        return Method::NpmGlobal;
    }
    Method::Manual(format!("npm install -g {package}@latest"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_from_what_a_cli_prints_and_compared_by_number() {
        assert!(newer("2.1.29 (Claude Code)", "2.1.30"));
        assert!(newer("codex-cli 0.57.0", "0.58.0"));
        assert!(!newer("v1.2", "1.2.0"), "1.2 is 1.2.0");
        assert!(newer("2.1.9", "2.1.10"));
        assert!(!newer("2.1.10", "2.1.10"));
        assert!(!newer("2.2.0", "2.1.99"));
        assert!(!newer("unknown", "2.0.0"), "a guess never nags");
    }

    #[test]
    fn each_install_updates_its_own_way() {
        let pkg = "@openai/codex";
        assert_eq!(
            method("/Users/a/.wisp/tools/node_modules/@openai/codex/bin/codex.js", "/Users/a/.wisp/tools", pkg, None),
            Method::Tools
        );
        assert_eq!(method("/usr/local/lib/node_modules/@openai/codex/bin/codex.js", "/Users/a/.wisp/tools", pkg, None), Method::NpmGlobal);
        assert_eq!(method("/opt/homebrew/Cellar/codex/0.58.0/bin/codex", "/t", pkg, None), Method::Manual("brew upgrade".into()));
        let own = vec!["update".to_owned()];
        assert_eq!(
            method("/Users/a/.local/share/claude/versions/2.1.30", "/t", "@anthropic-ai/claude-code", Some(&own)),
            Method::SelfUpdate(own.clone())
        );
        assert_eq!(method("/somewhere/else/codex", "/t", pkg, None), Method::Manual("npm install -g @openai/codex@latest".into()));
        assert_eq!(method("C:\\Users\\a\\.wisp\\tools\\node_modules\\x\\x.cmd", "C:\\Users\\a\\.wisp\\tools", "x", None), Method::Tools);
    }
}
