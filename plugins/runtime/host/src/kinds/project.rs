//! The `project` kind: a folder wisp works in, and how (its worktrees, setup, trust, tests, instructions), and
//! how a task's folder finds the project that holds it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How a project uses worktrees (v1 `worktree`, `worktreeOnDone`, `worktreeCleanup`, `worktreeMax`,
/// `cloneDir`, `fixConflicts`, `.worktreeinclude`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorktreePolicy {
    /// each task in a worktree of its own
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// when a task is done: ask | remove-clean | keep
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_done: String,
    /// days a finished task's worktree stays before cleanup
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup_days: Option<u32>,
    /// at most this many worktrees at once
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
    /// where clones and worktrees go
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dir: String,
    /// resolve merge conflicts with an agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_conflicts: Option<bool>,
    /// untracked files copied into each worktree (`.env`, `node_modules`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
}

/// What a project trusts without asking: its hooks, MCP servers and setup script.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTrust {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hooks: bool,
    /// the project's own MCP servers, by name
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub setup: bool,
}

/// A project: a folder wisp works in, and how.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSpec {
    /// the folder (absolute), or a repo URL cloned into the worktree dir
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
    /// the agent profile or harness its tasks use unless they say
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreePolicy>,
    /// commands run in a new worktree before the first turn
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub setup: Vec<String>,
    /// commands that start the project (a dev server), for Run
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<ProjectTrust>,
    /// the command that runs its tests (Auto learns from whether a task's tests pass)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub test: String,
    /// notes every task in it starts with (learned preferences, per-project rules)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instructions: String,
}

/// Setup audit B2: a project is a folder that's there, however it arrives (`wisp apply`, `wisp import`, a formula), as
/// `runtime.projectAdd` says for Projects' Add and `wisp add` (audit A F1). The spec is tidied first (its path
/// canonical); a repo URL is left to the clone. Read when a plan creates or changes a project, never per task.
pub fn check_project(spec: &Value) -> Vec<String> {
    let Some(path) = spec.get("path").and_then(Value::as_str).filter(|p| !p.trim().is_empty()) else { return Vec::new() };
    if path.contains("://") || path.starts_with("git@") || wisp_runtime_api::paths::Folder::of(path).is_err() {
        return Vec::new();
    }
    // one stat per project a plan creates or changes (`check` is sync, as `tidy_project`'s `plain_str_blocking` is):
    // never per task, per event or per row read
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => Vec::new(),
        Ok(_) => vec![format!("{path} is a file, not a folder: pick the folder it's in")],
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![format!("No folder at {path}")],
        Err(e) => vec![format!("Can't open {path}: {e}")],
    }
}

/// A folder as two projects' paths are compared (setup audit D12): by its real path when it's there (links followed:
/// macOS's `/tmp` is `/private/tmp`, a symlinked checkout is its target), else by its spelling (`folder_key`). A URL is
/// itself. One `realpath` per project a plan holds, never per task (the apply check is sync, as [`check_project`]).
pub fn folder_identity(path: &str) -> String {
    use wisp_runtime_api::paths::folder_key;
    if path.contains("://") || path.starts_with("git@") || wisp_runtime_api::paths::Folder::of(path).is_err() {
        return path.to_owned();
    }
    let real = wisp_proc::canonical_blocking(std::path::Path::new(path))
        .ok()
        .map(|p| wisp_runtime_api::paths::canonical_project_path(&p.to_string_lossy(), None));
    folder_key(real.as_deref().unwrap_or(path))
}

/// A project spec as it's stored: its path canonical (`wisp_runtime_api::paths`), `~` your home's.
pub fn tidy_project(spec: &mut Value) {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from);
    tidy_project_at(spec, home.as_deref());
}

/// [`tidy_project`] with the home folder given: "~/code/app" typed in Projects is that folder under your home (v1's
/// server expanded it; v2 stored a folder named `~`).
pub fn tidy_project_at(spec: &mut Value, home: Option<&std::path::Path>) {
    if let Some(p) = spec.get("path").and_then(Value::as_str) {
        let c = home_canonical(p, home);
        // one folder, one spelling (ADR-0071): an 8.3 short name spelled out (`RUNNER~1` → `runneradmin`), so the
        // project and a task in it (whose folder is spelled out too, `specs::spell_folders`) are one folder to every
        // comparison; a URL is left as it is. Unix: nothing changes.
        let c = if c.contains("://") || c.starts_with("git@") { c } else { wisp_proc::plain_str_blocking(&c) };
        if let Some(o) = spec.as_object_mut() {
            o.insert("path".into(), Value::String(c));
        }
    }
}

/// A typed folder with `~` as your home and in its canonical form (no file system): the first half of [`tidy_project_at`].
fn home_canonical(p: &str, home: Option<&std::path::Path>) -> String {
    let p = match (p.trim().strip_prefix('~'), home) {
        (Some(rest), Some(h)) if rest.is_empty() || rest.starts_with(['/', '\\']) => format!("{}{rest}", h.display()),
        _ => p.to_owned(),
    };
    wisp_runtime_api::paths::canonical_project_path(&p, None)
}

/// A folder as a project stores it, as [`tidy_project`] spells it but without blocking (`runtime.projectAdd`): `~` your
/// home, canonical, 8.3 names spelled out (ADR-0071).
pub async fn project_path(p: &str) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from);
    let c = home_canonical(p, home.as_deref());
    if c.contains("://") || c.starts_with("git@") { c } else { wisp_proc::plain_str(&c).await }
}

/// What `runtime.projectAdd` does with `path` (stored as it's kept) among the stored projects (ADR-0076): the project
/// the folder already is (nothing to write), or a new one under a free name (`name`, else the folder's last part).
/// Pure: the command runs it inside the store's transaction (`StoredKind::put_with`).
pub fn project_add(projects: &[(String, Value)], path: &str, name: Option<&str>) -> (Option<(String, Value)>, (String, bool)) {
    use wisp_runtime_api::paths::{ProjectName, project_name_as};
    let known: Vec<(String, String)> =
        projects.iter().map(|(n, v)| (n.clone(), v.get("path").and_then(Value::as_str).unwrap_or_default().to_owned())).collect();
    match project_name_as(path, name, &known) {
        ProjectName::Existing(n) => (None, (n, false)),
        ProjectName::New(n) => (Some((n.clone(), serde_json::json!({ "path": path }))), (n, true)),
    }
}

/// A project's spec with its test command set (empty clears it): what `runtime.projectTest` writes.
pub fn with_test(spec: &Value, command: &str) -> Value {
    let mut s = spec.clone();
    if let Some(o) = s.as_object_mut() {
        if command.trim().is_empty() {
            o.remove("test");
        } else {
            o.insert("test".into(), Value::String(command.trim().to_owned()));
        }
    }
    s
}

/// The project whose path holds `cwd` (the longest), if one is declared.
pub fn project_for(projects: &[(String, Value)], cwd: &str) -> Option<(String, ProjectSpec)> {
    projects
        .iter()
        .filter_map(|(n, v)| Some((n.clone(), serde_json::from_value::<ProjectSpec>(v.clone()).ok()?)))
        .filter(|(_, p)| {
            // any separators (Windows), and macOS's `/tmp` seen as `/private/tmp`
            let within = wisp_runtime_api::paths::is_within;
            within(cwd, &p.path) || cwd.strip_prefix("/private").is_some_and(|c| within(c, &p.path))
        })
        .max_by_key(|(_, p)| p.path.len())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::kinds::brief::brief_text;

    /// `wisp add .` (or a resource applied by hand) stored "/repo/."; on apply the project kind stores it canonical.
    #[test]
    fn a_project_applied_with_a_trailing_dot_or_verbatim_prefix_is_stored_canonical() {
        let mut spec = json!({ "path": "/work/api/.", "color": "teal" });
        tidy_project(&mut spec);
        assert_eq!(spec, json!({ "path": "/work/api", "color": "teal" }));
        let mut win = json!({ "path": r"\\?\C:\work\api\" });
        tidy_project(&mut win);
        assert_eq!(win["path"], r"C:\work\api");
        let mut url = json!({ "path": "https://github.com/acme/api.git" });
        tidy_project(&mut url);
        assert_eq!(url["path"], "https://github.com/acme/api.git");
    }

    /// Setup audit D12: a formula's `/private/tmp/x` (or a link to the folder) is the project at `/tmp/x`, not a
    /// second one: the apply check compares real paths.
    #[test]
    fn two_spellings_or_a_link_of_one_folder_are_one_folder_identity() {
        let dir = std::env::temp_dir().join(format!("wisp-identity-{}", std::process::id()));
        let real = dir.join("repo");
        std::fs::create_dir_all(&real).unwrap();
        let real_s = real.to_string_lossy().into_owned();
        assert_eq!(folder_identity(&format!("{real_s}/")), folder_identity(&real_s));
        #[cfg(unix)]
        {
            let link = dir.join("link");
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(&real, &link).unwrap();
            assert_eq!(folder_identity(&link.to_string_lossy()), folder_identity(&real_s), "a link is its target");
        }
        if cfg!(target_os = "macos") && real_s.starts_with("/var/") {
            assert_eq!(folder_identity(&format!("/private{real_s}")), folder_identity(&real_s));
        }
        assert_eq!(
            folder_identity("/private/tmp/wisp-no-such-folder"),
            folder_identity("/tmp/wisp-no-such-folder"),
            "not there: by spelling"
        );
        assert_ne!(folder_identity(&real_s), folder_identity(&dir.join("other").to_string_lossy()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Projects' box takes "~/code/app" (its folder suggestions answer in that form): stored under your home, as v1's
    /// server did. `~bob` stays as typed.
    #[test]
    fn a_project_typed_from_home_is_stored_under_home() {
        let h = std::path::Path::new("/home/ada");
        let mut spec = json!({ "path": "~/code/app/" });
        tidy_project_at(&mut spec, Some(h));
        assert_eq!(spec["path"], "/home/ada/code/app");
        let mut bob = json!({ "path": "~bob/app" });
        tidy_project_at(&mut bob, Some(h));
        assert_eq!(bob["path"], "~bob/app");
    }

    #[test]
    fn a_projects_test_command_is_set_cleared_and_read_back() {
        let spec = serde_json::json!({ "path": "/w/app" });
        let set = with_test(&spec, "  cargo test ");
        assert_eq!(set["test"], "cargo test");
        let p: ProjectSpec = serde_json::from_value(set.clone()).unwrap();
        assert_eq!(p.test, "cargo test", "the spec reads it back (deny_unknown_fields knows it)");
        assert!(with_test(&set, "").get("test").is_none());
    }

    /// Round 288: two folders named `wisp` are two projects (`wisp`, `wisp-2`, ADR-0076), and each folder's task finds
    /// its own one: neither replaced the other.
    #[test]
    fn two_same_named_folders_are_two_projects_and_each_holds_its_own_tasks() {
        use wisp_runtime_api::paths::{ProjectName, project_name_for};
        let mut a = json!({ "path": "/tmp/a/wisp/" });
        tidy_project_at(&mut a, None);
        let mut stored = vec![("wisp".to_owned(), a)];
        let paths = |s: &[(String, Value)]| -> Vec<(String, String)> {
            s.iter().map(|(n, v)| (n.clone(), v["path"].as_str().unwrap_or_default().to_owned())).collect()
        };
        assert_eq!(project_name_for("/tmp/b/wisp", &paths(&stored)), ProjectName::New("wisp-2".into()));
        stored.push(("wisp-2".to_owned(), json!({ "path": "/tmp/b/wisp" })));
        assert_eq!(project_for(&stored, "/tmp/a/wisp/src").unwrap().0, "wisp");
        assert_eq!(project_for(&stored, "/tmp/b/wisp").unwrap().0, "wisp-2");
        assert_eq!(project_name_for("/tmp/a/wisp", &paths(&stored)), ProjectName::Existing("wisp".into()));
    }

    #[test]
    fn the_deepest_project_holds_the_folder_and_the_brief_reads() {
        let ps = vec![
            ("mono".to_owned(), json!({ "path": "/w" })),
            ("api".to_owned(), json!({ "path": "/w/api", "instructions": "small PRs" })),
        ];
        assert_eq!(project_for(&ps, "/w/api/src").unwrap().0, "api");
        assert_eq!(project_for(&ps, "/w/web").unwrap().0, "mono");
        assert!(project_for(&ps, "/x").is_none());
        let b = brief_text(
            &json!({ "project": "small PRs", "agent": "plan first", "skills": [{ "name": "notes", "description": "release notes", "body": "Keep it short." }] }),
        );
        assert!(
            b.contains("## This project's instructions\n\nsmall PRs") && b.contains("## Your instructions") && b.contains("### notes"),
            "{b}"
        );
    }
}
