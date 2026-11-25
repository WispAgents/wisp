//! What a new worktree gets copied from the checkout (`.worktreeinclude`, audit git round 1): never a path outside the
//! repo (a `..` line, a symlink in the checkout pointing out of it), and, while the repo isn't trusted, only the root's
//! `.env` files. Its own list is copied once it is trusted, with the held steps.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use wisp_kernel::CancellationToken;
use wisp_worktrees::{Engine, prepare};
use wisp_worktrees_api::{TrustItem, TrustKind, Worktree};

static N: AtomicU32 = AtomicU32::new(0);

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmp(name: &str) -> Tmp {
    let d = std::env::temp_dir().join(format!("wisp-wti-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    Tmp(wisp_proc::canonical_blocking(&d).unwrap())
}

fn engine(t: &Tmp) -> Engine {
    let git = wisp_git::service(wisp_proc::local(), None, Arc::new(|| 1_000_000), None);
    Engine::new(
        Arc::new(wisp_git::Service(git)),
        wisp_proc::local(),
        wisp_testkit::FakeClock::at(1_000_000).service(),
        t.0.join("h").to_string_lossy().into_owned(),
        t.0.join("u").to_string_lossy().into_owned(),
        // a plain copy (no `cp -c`): the test reads what landed, not how
        "windows",
        Vec::new(),
        CancellationToken::new(),
    )
}

/// A checkout at `<t>/app` with `.env`, a local config its `.worktreeinclude` names, and an empty new folder `<t>/wts/wt`.
fn checkout(t: &Tmp, include: &str) -> (String, String) {
    let root = t.0.join("app");
    std::fs::create_dir_all(root.join("config")).unwrap();
    std::fs::write(root.join(".env"), "KEY=1\n").unwrap();
    std::fs::write(root.join("config/local.json"), "{}\n").unwrap();
    std::fs::write(root.join(".worktreeinclude"), include).unwrap();
    let dir = t.0.join("wts/wt");
    std::fs::create_dir_all(&dir).unwrap();
    (root.to_string_lossy().into_owned(), dir.to_string_lossy().into_owned())
}

fn held() -> Vec<TrustItem> {
    vec![TrustItem { kind: TrustKind::Setup, file: None, name: "setup".into(), command: "true".into(), extra: None, id: "x".into() }]
}

#[tokio::test]
async fn an_untrusted_repo_gets_only_its_env_files_and_the_rest_once_trusted() {
    let t = tmp("trust");
    let e = engine(&t);
    let (root, dir) = checkout(&t, ".env\nconfig/local.json\n");
    let p = prepare::prepare(&e, &root, &dir, Some(held()), "main").await;
    assert_eq!(p.copied, [".env"], "{p:?}");
    assert!(!Path::new(&dir).join("config/local.json").exists(), "its own list waits for trust");
    // trusted: the held steps run, and the rest of the list is copied
    let wt = Worktree { path: dir.clone(), cwd: dir.clone(), root: root.clone(), prepared: Some(p), ..Worktree::default() };
    let after = prepare::run_held(&e, &wt).await.unwrap();
    assert!(Path::new(&dir).join("config/local.json").is_file(), "{after:?}");
    assert!(after.copied.contains(&"config/local.json".to_owned()));

    // a trusted repo copies its whole list at once
    let t2 = tmp("trusted");
    let (root, dir) = checkout(&t2, ".env\nconfig/local.json\n");
    let p = prepare::prepare(&e, &root, &dir, None, "main").await;
    assert!(Path::new(&dir).join(".env").is_file() && Path::new(&dir).join("config/local.json").is_file(), "{p:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn nothing_outside_the_repo_is_copied() {
    let t = tmp("out");
    let e = engine(&t);
    let outside = t.0.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), "s\n").unwrap();
    std::fs::write(outside.join(".env"), "OTHER=1\n").unwrap();
    let (root, dir) = checkout(&t, ".env\nshared\n../outside/.env\n");
    std::os::unix::fs::symlink(&outside, Path::new(&root).join("shared")).unwrap();
    let p = prepare::prepare(&e, &root, &dir, None, "main").await;
    assert_eq!(p.copied, [".env"], "{p:?}");
    assert!(!Path::new(&dir).join("shared").exists(), "a symlink out of the checkout is never followed");
    assert!(!t.0.join("wts/outside").exists(), "a `..` line writes nothing beside the worktree");
    assert!(p.problems.iter().any(|m| m.contains("../outside/.env")), "{p:?}");
    assert!(p.problems.iter().any(|m| m.contains("shared")), "{p:?}");
}
