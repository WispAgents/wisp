//! A delegate's copy on another machine (ADR-0059), the pure part: where a copy lives (always under wisp's own
//! folder there), which folders give changes back, the git commands that pack, unpack and diff, and the words.

use std::path::{Component, Path, PathBuf};

/// The folder (under the plugin's own) that holds every copy: `<wisp>/plugins/wisp.machines/copies`.
pub const COPIES: &str = "copies";
/// Uploads waiting for their `machines.takeIn`: `<wisp>/plugins/wisp.machines/incoming`.
pub const INCOMING: &str = "incoming";
/// Patches that didn't go in, kept here: `<wisp>/plugins/wisp.machines/patches`.
pub const PATCHES: &str = "patches";
/// The biggest bundle sent (v1 relay `MAX_PATCH`); the upload route takes a little more.
pub const MAX_BUNDLE: usize = 256 * 1024 * 1024;
/// The biggest patch that comes back over the link.
pub const MAX_PATCH: usize = 16 * 1024 * 1024;
/// The marker of what a copy was when it came in (its changes are the difference from it).
pub const BASE_FILE: &str = ".wisp-copy-base";

/// A name that can only be one path segment: letters, digits, `-`, `_` and `.` kept, the rest `-`, no leading dot,
/// at most 80 long. Empty or all dots: `work`.
pub fn segment(s: &str) -> String {
    let mut out: String =
        s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }).take(80).collect();
    while out.starts_with('.') {
        out.remove(0);
    }
    if out.trim_matches(['-', '.']).is_empty() { "work".to_owned() } else { out }
}

/// The project's folder name, from the lead's folder: its last segment.
pub fn name_of(cwd: &str) -> String {
    segment(cwd.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or_default())
}

/// The copy for `key` of project `name`: `<plugin>/copies/<key>/<name>`, whatever the two say.
pub fn copy_dir(plugin: &Path, key: &str, name: &str) -> PathBuf {
    plugin.join(COPIES).join(segment(key)).join(segment(name))
}

/// The upload of `part` (`bundle`, `patch`) for `key`; `None`: not a part.
pub fn incoming(plugin: &Path, key: &str, part: &str) -> Option<PathBuf> {
    matches!(part, "bundle" | "patch").then(|| plugin.join(INCOMING).join(format!("{}.{part}", segment(key))))
}

/// Whether `cwd` is a copy carried here: two segments below `<plugin>/copies`, with no `..` on the way.
pub fn is_copy(plugin: &Path, cwd: &str) -> bool {
    let p = Path::new(cwd);
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    p.strip_prefix(plugin.join(COPIES)).is_ok_and(|rest| rest.components().count() == 2)
}

/// `git` args that pack the lead's folder (in `cwd`) into `bundle`: its HEAD and branch, with their history.
pub fn pack_commands(bundle: &str, branch: &str) -> Vec<Vec<String>> {
    let mut b = vec!["bundle".to_owned(), "create".to_owned(), bundle.to_owned(), "HEAD".to_owned()];
    if !branch.is_empty() {
        b.push(branch.to_owned());
    }
    vec![b]
}

/// The `git` args that unpack it, run in the copies folder: a clone of the bundle, the lead's commit checked out on
/// the lead's branch (`wisp/delegate` when it was detached), its remote gone (a delegate never pushes).
pub fn unpack_commands(bundle: &str, dir: &str, head: &str, branch: &str) -> Vec<Vec<String>> {
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    let on = if branch.is_empty() { "wisp/delegate" } else { branch };
    vec![
        s(&["clone", "--quiet", "--no-checkout", bundle, dir]),
        s(&["-C", dir, "checkout", "--quiet", "-B", on, head]),
        s(&["-C", dir, "remote", "remove", "origin"]),
    ]
}

/// The files a patch touches.
pub fn files_in(patch: &str) -> u32 {
    u32::try_from(patch.lines().filter(|l| l.starts_with("diff --git ")).count()).unwrap_or(u32::MAX)
}

/// The words for a lead's folder that can't go.
pub fn not_a_repo(cwd: &str) -> String {
    format!("{cwd} isn't a git repo with a commit: only a repo's work goes to another machine")
}

pub fn too_big(mb: usize) -> String {
    format!("The lead's work is too big to send ({mb} MB; at most {} MB).", MAX_BUNDLE >> 20)
}

pub fn not_a_copy() -> &'static str {
    "Only a copy wisp carried to this machine gives changes back."
}

pub fn clash(kept: &str, why: &str) -> String {
    format!("Its changes clash with yours here, so none went in. They are kept in {kept}. ({why})")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_is_always_under_wisps_own_folder() {
        let plugin = Path::new("/h/.wisp/plugins/wisp.machines");
        assert_eq!(copy_dir(plugin, "lead1-ada", "app"), plugin.join("copies/lead1-ada/app"));
        // whatever the key and name say, two segments below copies
        for (k, n) in [("../../code", "../app"), ("/etc", "."), ("a/b", "..\\x"), ("", "")] {
            let d = copy_dir(plugin, k, n);
            assert!(d.starts_with(plugin.join(COPIES)), "{d:?}");
            assert!(is_copy(plugin, d.to_str().unwrap()), "{d:?}");
        }
        assert_eq!(segment("../../code"), "-..-code");
        assert_eq!(name_of("/Users/me/code/my app/"), "my-app");
    }

    #[test]
    fn only_a_carried_copy_gives_changes_back() {
        let plugin = Path::new("/h/.wisp/plugins/wisp.machines");
        assert!(is_copy(plugin, "/h/.wisp/plugins/wisp.machines/copies/k/app"));
        for no in [
            "/Users/me/code/app",
            "/h/.wisp/plugins/wisp.machines/copies/k",
            "/h/.wisp/plugins/wisp.machines/copies/k/app/sub",
            "/h/.wisp/plugins/wisp.machines/copies/k/../../../../../Users/me/app",
        ] {
            assert!(!is_copy(plugin, no), "{no}");
        }
        assert!(incoming(plugin, "k", "bundle").is_some());
        assert!(incoming(plugin, "k", "../x").is_none());
    }

    #[test]
    fn a_detached_lead_gets_a_branch_of_its_own_there() {
        let c = unpack_commands("/b", "/d", "abc", "");
        assert_eq!(c[1], ["-C", "/d", "checkout", "--quiet", "-B", "wisp/delegate", "abc"]);
        let c = unpack_commands("/b", "/d", "abc", "fix");
        assert_eq!(c[1][5], "fix");
        assert_eq!(pack_commands("/b", "fix")[0], ["bundle", "create", "/b", "HEAD", "fix"]);
        assert_eq!(files_in("diff --git a/x b/x\n+1\ndiff --git a/y b/y\n"), 2);
    }
}
