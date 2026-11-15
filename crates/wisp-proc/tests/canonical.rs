//! `wisp_proc::canonical`: the one canonicalize, never Windows' verbatim `\\?\` form (git reads `//?/C:/x` as a
//! host: "hostname contains invalid characters", 13 tests on the first Windows CI run).
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use wisp_proc::{canonical, canonical_blocking, strip_verbatim, without_verbatim};

#[test]
fn a_verbatim_drive_or_unc_path_loses_its_prefix_and_anything_else_is_kept() {
    assert_eq!(strip_verbatim(r"\\?\C:\Users\me\AppData\Local\Temp\x"), r"C:\Users\me\AppData\Local\Temp\x");
    assert_eq!(strip_verbatim(r"\\?\c:\"), r"c:\");
    assert_eq!(strip_verbatim(r"\\?\D:"), "D:");
    assert_eq!(strip_verbatim(r"\\?\UNC\srv\share\repo"), r"\\srv\share\repo");
    // no plain spelling: kept
    assert_eq!(
        strip_verbatim(r"\\?\Volume{1b3b1146-4076-11e1-84aa-806e6f6e6963}\x"),
        r"\\?\Volume{1b3b1146-4076-11e1-84aa-806e6f6e6963}\x"
    );
    assert_eq!(strip_verbatim(r"\\?\GLOBALROOT\Device\x"), r"\\?\GLOBALROOT\Device\x");
    // already plain: as it is
    for p in [r"C:\x", r"\\srv\share", "/tmp/x", "", "relative", r"\\.\pipe\x"] {
        assert_eq!(strip_verbatim(p), p, "{p:?}");
    }
    assert_eq!(without_verbatim(PathBuf::from(r"\\?\C:\x")), PathBuf::from(r"C:\x"));
    assert_eq!(without_verbatim(PathBuf::from("/tmp/x")), PathBuf::from("/tmp/x"));
}

#[tokio::test]
async fn canonical_resolves_links_and_dots_and_never_returns_a_verbatim_path() {
    let base = std::env::temp_dir().join(format!("wisp-proc-canonical-{}", std::process::id()));
    std::fs::create_dir_all(base.join("a/b")).unwrap();
    let dotted = base.join("a/b/../b/.");
    let blocking = canonical_blocking(&dotted).unwrap();
    let not = canonical(&dotted).await.unwrap();
    assert_eq!(blocking, not);
    assert!(!blocking.to_string_lossy().starts_with(r"\\?\"), "{}", blocking.display());
    assert!(blocking.ends_with(Path::new("a").join("b")), "{}", blocking.display());
    assert!(!blocking.to_string_lossy().contains(".."), "{}", blocking.display());
    assert!(canonical_blocking(base.join("not-there")).is_err());
    std::fs::remove_dir_all(&base).unwrap();
}

/// CI 708ba936: `%TEMP%` on the Windows runner is `C:\Users\RUNNER~1\…` (an 8.3 short name), a task's folder kept that
/// spelling while a canonical path said `C:\Users\runneradmin\…`, and one folder was two. `plain_path` spells out the
/// short names and changes nothing else: on Windows a folder with no links reads as its canonical path (the runner's
/// temp folder is the short-named case), and a path not there yet has the part of it that is spelled out; on Unix
/// every path comes back as it is.
#[tokio::test]
async fn a_short_name_is_spelled_out_and_nothing_else_changes() {
    let tmp = std::env::temp_dir().join(format!("wisp-proc-long-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("inner")).unwrap();
    let inner = tmp.join("inner");
    let long = wisp_proc::plain_path(&inner).await;
    assert_eq!(long, wisp_proc::plain_path_blocking(&inner));
    let gone = tmp.join("not-there").join("x");
    if cfg!(windows) {
        assert_eq!(long, canonical_blocking(&inner).unwrap(), "every short name spelled out, as the canonical path has it");
        assert!(!long.to_string_lossy().contains('~'), "{}", long.display());
        let real = canonical_blocking(&tmp).unwrap();
        assert_eq!(wisp_proc::plain_path_blocking(&gone), real.join("not-there").join("x"), "the part that is there spelled out");
    } else {
        assert_eq!(long, inner, "Unix has no short names");
        assert_eq!(wisp_proc::plain_path_blocking(&gone), gone);
    }
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// The runner's two spellings of one folder, by a mapping of its own (so this runs on every OS): the longest part that
/// exists is spelled out, the rest kept; a relative path and a root are left alone; the verbatim prefix goes.
#[test]
fn an_8_3_name_is_spelled_out_by_the_longest_part_that_exists() {
    let long_of = |p: &str| match p {
        r"C:\Users\RUNNER~1" => Some(r"C:\Users\runneradmin".to_owned()),
        r"C:\Users\RUNNER~1\AppData\Local\Temp\wisp-x\proj" => Some(r"C:\Users\runneradmin\AppData\Local\Temp\wisp-x\proj".to_owned()),
        _ => None,
    };
    let s = |p: &str| wisp_proc::spell_out_with(p, long_of);
    assert_eq!(s(r"C:\Users\RUNNER~1\AppData\Local\Temp\wisp-x\proj"), r"C:\Users\runneradmin\AppData\Local\Temp\wisp-x\proj");
    // not there yet: what is there spelled out, the rest as it was
    assert_eq!(
        s(r"C:\Users\RUNNER~1\AppData\Local\Temp\wisp-x\proj\gone.png"),
        r"C:\Users\runneradmin\AppData\Local\Temp\wisp-x\proj\gone.png"
    );
    assert_eq!(s(r"C:\Users\RUNNER~1\elsewhere\y"), r"C:\Users\runneradmin\elsewhere\y");
    assert_eq!(s(r"\\?\C:\Users\RUNNER~1\z"), r"C:\Users\runneradmin\z");
    for p in [r"C:\", "/", "relative\\RUNNER~1", "", r"D:\nothing\known"] {
        assert_eq!(s(p), p, "{p:?}");
    }
    assert_eq!(wisp_proc::spell_out_with("/tmp/a/b", |p| (p == "/tmp").then(|| "/private/tmp".to_owned())), "/private/tmp/a/b");
}

/// The comparison every containment check makes: both sides spelled out first, by parts, either separator, a Windows
/// path in any case. A short spelling of a project's subfolder is inside its long spelling (`runtime.projectRun`
/// refused `RUNNER~1\…\proj\src` "isn't in a declared project" in CI 8bb8ccd0).
#[test]
fn a_short_spelling_is_within_its_long_one() {
    // as Windows reads it: either separator, any case
    let long_of = |p: &str| (p.replace('/', "\\").to_lowercase() == r"c:\users\runner~1").then(|| r"C:\Users\runneradmin".to_owned());
    let w = |p: &str, b: &str| wisp_proc::within_with(p, b, long_of);
    assert!(w(r"C:\Users\RUNNER~1\t\proj\src", r"C:\Users\runneradmin\t\proj"));
    assert!(w(r"C:\Users\runneradmin\t\proj", r"C:\Users\RUNNER~1\t\proj\"));
    assert!(w(r"c:/users/RUNNER~1/t/proj", r"C:\Users\runneradmin\t\proj"), "either separator, any case");
    assert!(!w(r"C:\Users\RUNNER~1\t\proj2", r"C:\Users\runneradmin\t\proj"), "a sibling with a shared prefix");
    assert!(!w(r"C:\Users\runneradmin\t", r"C:\Users\RUNNER~1\t\proj"), "a parent isn't within");
    assert!(!w("", r"C:\x") && !w(r"C:\x", ""));
    assert!(wisp_proc::within_with("/a/b/c", "/a/b", |_| None) && !wisp_proc::within_with("/a/bc", "/a/b", |_| None));
    assert!(!wisp_proc::within_with("/A/b", "/a", |_| None), "a POSIX name keeps its case");
    assert!(!wisp_proc::within_with("/a/b/../../etc", "/a/b", |_| None), "a `..` never reads as inside");
}
