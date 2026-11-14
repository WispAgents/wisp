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
