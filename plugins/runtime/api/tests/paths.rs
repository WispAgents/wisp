//! A project's path is stored canonical (`wisp add .` stored "/repo/."; Windows' `\\?\` breaks git).
#![allow(clippy::unwrap_used)]

use wisp_runtime_api::paths::canonical_project_path as canon;

#[test]
fn wisp_add_dot_stores_the_folder_itself_absolute_without_a_trailing_dot_or_slash() {
    // the bug: `wisp add .` in /repo stored "/repo/."
    assert_eq!(canon("/repo/.", None), "/repo");
    assert_eq!(canon("/repo/", None), "/repo");
    assert_eq!(canon("/repo/./", None), "/repo");
    assert_eq!(canon("/repo/./web/../api", None), "/repo/api");
    // relative: resolved against the folder it was typed in
    assert_eq!(canon(".", Some("/home/me/repo")), "/home/me/repo");
    assert_eq!(canon("../other/", Some("/home/me/repo")), "/home/me/other");
    // the root stays the root; a URL stays as it is
    assert_eq!(canon("/", None), "/");
    assert_eq!(canon("/..", None), "/");
    assert_eq!(canon("https://github.com/acme/api.git", None), "https://github.com/acme/api.git");
    assert_eq!(canon("git@github.com:acme/api.git", None), "git@github.com:acme/api.git");
}

#[test]
fn a_windows_path_loses_its_verbatim_prefix_and_trailing_separator() {
    // std::fs::canonicalize on Windows returns \\?\C:\..., which git refuses
    assert_eq!(canon(r"\\?\C:\work\repo", None), r"C:\work\repo");
    assert_eq!(canon(r"C:\work\repo\.", None), r"C:\work\repo");
    assert_eq!(canon(r"C:\work\repo\", None), r"C:\work\repo");
    assert_eq!(canon(r"C:\", None), r"C:\");
    assert_eq!(canon(r"\\?\UNC\server\share\repo\", None), r"\\server\share\repo");
    assert_eq!(canon(r".\api", Some(r"C:\work")), r"C:\work\api");
}

/// A folder that's empty or relative would resolve against the server's own folder: refused, every spelling.
#[test]
fn a_folder_is_a_full_path_and_never_empty_or_relative() {
    use wisp_runtime_api::paths::Folder;
    for bad in ["", "   ", ".", "./src", "src", "..", "~/code/app", "~"] {
        let e = Folder::of(bad).unwrap_err();
        assert_eq!(e.path, bad.trim(), "{bad:?}");
    }
    assert_eq!(Folder::of("").unwrap_err().to_string(), "no folder");
    assert_eq!(Folder::of("src").unwrap_err().to_string(), "src isn't a full path to a folder");
    for good in ["/", "/work/app", " /work/app ", r"C:\work\app", "C:/work/app", r"\\host\share\app"] {
        assert_eq!(Folder::of(good).unwrap().to_string(), good.trim(), "{good:?}");
    }
    // a worktree that's gone falls back to the task's folder; none at all is None
    assert_eq!(Folder::first(["", "/work/app"]).unwrap().path(), std::path::Path::new("/work/app"));
    assert!(Folder::first(["", "relative"]).is_none());
    let w: wisp_kernel::WireError = Folder::of("").unwrap_err().into();
    assert_eq!(w.message, "no folder");
}

/// Windows round 3: a folder compares as one folder whatever its spelling (`C:\Repo\`, `c:/repo`, `\\?\C:\Repo`), and
/// "inside" holds across `\` and `/` (a task in `C:\repo\src` is the `C:/repo` project's).
#[test]
fn a_folder_is_the_same_folder_in_every_spelling_and_holds_what_is_inside_it() {
    use wisp_runtime_api::paths::{folder_key, is_within};
    assert!(wisp_runtime_api::paths::same_folder(r"C:\Repo\", "c:/repo"));
    assert!(wisp_runtime_api::paths::same_folder(r"\\?\C:\Repo", r"C:\repo\."));
    assert!(wisp_runtime_api::paths::same_folder(r"\\host\share\x", r"\\HOST\share\x\"));
    assert_eq!(folder_key(r"\\?\UNC\host\share\X"), "//host/share/x");
    assert!(wisp_runtime_api::paths::same_folder("/repo/.", "/repo/"));
    assert!(!wisp_runtime_api::paths::same_folder("/Repo", "/repo"), "POSIX names keep their case");
    assert!(!wisp_runtime_api::paths::same_folder("", ""), "no folder is no folder");
    assert!(is_within(r"C:\repo\src\a", "C:/Repo"));
    assert!(is_within("C:/repo/src", r"C:\repo\"));
    assert!(is_within(r"C:\repo", r"c:\repo"));
    assert!(!is_within(r"C:\repository", r"C:\repo"));
    assert!(!is_within(r"D:\repo\src", r"C:\repo"));
    assert!(is_within(r"C:\x", r"C:\"), "the drive holds all of it");
    assert!(is_within("/repo/src", "/repo/") && is_within("/repo", "/repo/.") && is_within("/x", "/"));
    assert!(!is_within("/repository", "/repo") && !is_within("/repo/src", "") && !is_within("src", "."));
    // a POSIX name may hold a `\`: it's no separator there, and the case still counts
    assert!(is_within(r"/Work/a\b", "/Work"));
    assert!(!wisp_runtime_api::paths::same_folder(r"/Repo\x", r"/repo\X"));
    assert!(!is_within("/a/b/c", r"/a\b"));
    assert_eq!(folder_key("/repo/./web/../api/"), "/repo/api");
}

/// Round 288: `wisp add /b/wisp` after `/a/wisp` replaced the first project (both were named `wisp`, and a project is
/// keyed by its name). The second gets `wisp-2`; the same folder again, any spelling, is the project it already is.
#[test]
fn a_second_folder_with_a_taken_name_gets_its_own_name_and_the_same_folder_stays_one_project() {
    use wisp_runtime_api::paths::{ProjectName, project_name_for};
    let none: Vec<(String, String)> = Vec::new();
    assert_eq!(project_name_for("/tmp/a/wisp", &none), ProjectName::New("wisp".into()));
    let one = vec![("wisp".to_owned(), "/tmp/a/wisp".to_owned())];
    assert_eq!(project_name_for("/tmp/b/wisp", &one), ProjectName::New("wisp-2".into()));
    let two = vec![one[0].clone(), ("wisp-2".to_owned(), "/tmp/b/wisp".to_owned())];
    assert_eq!(project_name_for("/tmp/c/wisp", &two), ProjectName::New("wisp-3".into()));
    // the same folder, spelled with a trailing slash or `/.`: not a second project
    assert_eq!(project_name_for("/tmp/b/wisp/.", &two), ProjectName::Existing("wisp-2".into()));
    // macOS: `wisp add` in /tmp/b/wisp reads its folder as /private/tmp/b/wisp, and the other way round
    assert_eq!(project_name_for("/private/tmp/b/wisp", &two), ProjectName::Existing("wisp-2".into()));
    let private = vec![("wisp".to_owned(), "/private/tmp/a/wisp".to_owned())];
    assert_eq!(project_name_for("/tmp/a/wisp", &private), ProjectName::Existing("wisp".into()));
    assert_eq!(project_name_for("/privatex/wisp", &private), ProjectName::New("wisp-2".into()));
    // a name taken by hand (a spec's `wisp-2` for another folder) is skipped too
    let by_hand = vec![("wisp".to_owned(), "/x/wisp".to_owned()), ("wisp-2".to_owned(), "/y/other".to_owned())];
    assert_eq!(project_name_for("/z/wisp", &by_hand), ProjectName::New("wisp-3".into()));
    // the base: last part, `.git` off, lower case, spaces as `-`; Windows separators; a root is `project`
    assert_eq!(project_name_for("/w/My App", &none), ProjectName::New("my-app".into()));
    assert_eq!(project_name_for(r"C:\work\Api\", &none), ProjectName::New("api".into()));
    assert_eq!(project_name_for("/", &none), ProjectName::New("project".into()));
}

/// Audit A orch #6: git lists a macOS worktree by its real path (`/private/var/…`), wisp keeps the one it was given
/// (`/var/…`): one folder, so Bring in never offers wisp's own worktree back.
#[test]
fn a_macos_private_path_is_the_folder_its_link_names() {
    assert!(wisp_runtime_api::paths::same_folder("/private/var/folders/x/wt", "/var/folders/x/wt"));
    assert!(wisp_runtime_api::paths::same_folder("/private/tmp/repo", "/tmp/repo/"));
    assert!(wisp_runtime_api::paths::same_folder("/private/etc/hosts", "/etc/hosts"));
    assert!(!wisp_runtime_api::paths::same_folder("/private/home/a", "/home/a"), "only the folders macOS links");
    assert!(!wisp_runtime_api::paths::same_folder("/srv/private/tmp/a", "/srv/tmp/a"), "only at the root");
}
