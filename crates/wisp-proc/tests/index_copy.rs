//! `copy_index`: a scratch copy of a git index keeps the real one's mtime (git's racy-clean check), and a copy
//! that can't is never left behind (round 188).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wisp-proc-index-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn git(at: &Path, index: Option<&Path>, args: &[&str]) -> String {
    let mut c = Command::new("git");
    c.current_dir(at).args(args).env("GIT_CONFIG_NOSYSTEM", "1");
    if let Some(i) = index {
        c.env("GIT_INDEX_FILE", i);
    }
    let o = c.output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

fn stamp(p: &Path, when: SystemTime) {
    std::fs::File::options().write(true).open(p).unwrap().set_modified(when).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_copy_has_the_index_s_bytes_and_mtime() {
    let d = dir("bytes");
    let then = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    std::fs::write(d.join("index"), b"DIRC\0\0\0\x02 any bytes").unwrap();
    stamp(&d.join("index"), then);
    assert!(wisp_proc::copy_index(d.join("index"), d.join("scratch")).await);
    assert_eq!(std::fs::read(d.join("scratch")).unwrap(), b"DIRC\0\0\0\x02 any bytes");
    assert_eq!(std::fs::metadata(d.join("scratch")).unwrap().modified().unwrap(), then);
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test(flavor = "multi_thread")]
async fn no_index_no_copy() {
    let d = dir("none");
    assert!(!wisp_proc::copy_index(d.join("index"), d.join("scratch")).await);
    assert!(!d.join("scratch").exists(), "nothing left behind");
    // a scratch path that can't be written: false, and the real index is untouched
    std::fs::write(d.join("index"), b"x").unwrap();
    assert!(!wisp_proc::copy_index(d.join("index"), d.join("no/such/dir/scratch")).await);
    assert_eq!(std::fs::read(d.join("index")).unwrap(), b"x");
    let _ = std::fs::remove_dir_all(d);
}

/// The bug class itself: a same-size edit made in the index's own second (stamps pinned, ctime not trusted, as
/// it can't be pinned) is seen through the copy. A copy with a fresh mtime (Linux's `fs::copy`) misses it.
#[test]
fn a_same_size_edit_in_the_index_s_own_second_is_seen_through_the_copy() {
    let d = dir("racy");
    git(&d, None, &["init", "-q"]);
    git(&d, None, &["config", "core.trustctime", "false"]);
    let then = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    std::fs::write(d.join("a.txt"), "x\n").unwrap();
    stamp(&d.join("a.txt"), then);
    git(&d, None, &["add", "a.txt"]);
    stamp(&d.join(".git/index"), then);
    std::fs::write(d.join("a.txt"), "y\n").unwrap();
    stamp(&d.join("a.txt"), then);
    let scratch = d.join(".git/scratch.index");
    wisp_proc::copy_index_blocking(&d.join(".git/index"), &scratch).unwrap();
    git(&d, Some(&scratch), &["add", "--update"]);
    let tree = git(&d, Some(&scratch), &["write-tree"]);
    assert_eq!(git(&d, None, &["show", &format!("{tree}:a.txt")]), "y");
    let _ = std::fs::remove_dir_all(d);
}
