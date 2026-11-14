//! `sandbox.seed`, inside a sandbox only (`WISP_SANDBOX=1`, ADR-0047): the project comes in as a bundle and its
//! uncommitted changes, lands in `~/code/<name>` with its origin and author, and its `.wisp/sandbox.sh` runs once.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use wisp_kernel::{Ctx, Result, WireError};
use wisp_proc::{Env, ExecOpts, Proc};
use wisp_sandbox_api as api;
use wisp_sandbox_core as core;

/// Whether this wisp is a sandbox's (the grid starts a sandbox's box with `WISP_SANDBOX=1`).
pub fn in_sandbox(env: &dyn Fn(&str) -> Option<String>) -> bool {
    env("WISP_SANDBOX").is_some_and(|v| v == "1")
}

async fn run(
    proc: &Proc,
    cwd: &Path,
    bin: &str,
    args: &[String],
    input: Option<Vec<u8>>,
    secs: u64,
) -> std::result::Result<String, String> {
    let opts = ExecOpts {
        cwd: Some(cwd.to_owned()),
        env: Env::default().with("GIT_TERMINAL_PROMPT", "0"),
        input,
        timeout: Some(Duration::from_secs(secs)),
        // a patch back is up to 16 MB (machines' cap); more is refused, never cut short
        max_output: 17 << 20,
        ..ExecOpts::default()
    };
    let r = proc.exec(bin, args, opts).await.map_err(|e| e.to_string())?;
    if r.truncated {
        return Err(format!("{bin} wrote more than 17 MB"));
    }
    if r.exit.success() { Ok(r.stdout_text()) } else { Err(format!("{}{}", r.stdout_text(), r.stderr_text()).trim().to_owned()) }
}

/// `~/.sandbox/incoming`: uploads waiting for their `sandbox.seed` (ADR-0060).
fn incoming(home: &Path) -> PathBuf {
    home.join(".sandbox/incoming")
}

/// An upload's bytes (`POST /api/sandbox/seed/<key>/<part>`), kept for `sandbox.seed {key}`.
pub async fn receive(home: &Path, key: &str, part: &str, bytes: &[u8]) -> Result<()> {
    let Some(name) = core::upload_name(key, part) else { return Err(WireError::invalid("a seed's part is a bundle or a patch")) };
    let dir = incoming(home);
    tokio::fs::create_dir_all(&dir).await.map_err(|e| WireError::internal(format!("~/.sandbox: {e}")))?;
    tokio::fs::write(dir.join(name), bytes).await.map_err(|e| WireError::internal(format!("the upload: {e}")))
}

/// The folder's whole working state (tracked, changed and new files, not ignored ones) as a tree, read through a
/// scratch index: the folder's own index stays as it was.
async fn tree_of(proc: &Proc, dir: &Path) -> std::result::Result<String, String> {
    let scratch = std::env::temp_dir().join(format!("wisp-sbx-{}.index", wisp_kernel::ids::uuid7()));
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    let out = async {
        run_in(proc, dir, &s(&["read-tree", "HEAD"]), &scratch).await?;
        run_in(proc, dir, &s(&["add", "-A"]), &scratch).await?;
        run_in(proc, dir, &s(&["write-tree"]), &scratch).await
    }
    .await;
    let _ = tokio::fs::remove_file(&scratch).await;
    out.map(|t| t.trim().to_owned())
}

async fn run_in(proc: &Proc, cwd: &Path, args: &[String], index: &Path) -> std::result::Result<String, String> {
    let opts = ExecOpts {
        cwd: Some(cwd.to_owned()),
        env: Env::default().with("GIT_TERMINAL_PROMPT", "0").with("GIT_INDEX_FILE", index.display().to_string()),
        timeout: Some(Duration::from_secs(120)),
        max_output: 1 << 20,
        ..ExecOpts::default()
    };
    let r = proc.exec("git", args, opts).await.map_err(|e| e.to_string())?;
    if r.exit.success() { Ok(r.stdout_text()) } else { Err(format!("{}{}", r.stdout_text(), r.stderr_text()).trim().to_owned()) }
}

/// Where the bundle is and what isn't committed: the upload `key` names (ADR-0060), or the inline ones.
async fn bundle_and_patch(home: &Path, name: &str, a: &api::SeedArgs) -> Result<Option<(PathBuf, Vec<u8>, Vec<PathBuf>)>> {
    if !a.key.is_empty() {
        let (Some(b), Some(p)) = (core::upload_name(&a.key, "bundle"), core::upload_name(&a.key, "patch")) else {
            return Err(WireError::invalid("no such upload"));
        };
        let (b, p) = (incoming(home).join(b), incoming(home).join(p));
        if !tokio::fs::metadata(&b).await.is_ok_and(|m| m.is_file()) {
            return Err(WireError::not_found("Nothing was sent here for that seed."));
        }
        let patch = tokio::fs::read(&p).await.unwrap_or_default();
        return Ok(Some((b.clone(), patch, vec![b, p])));
    }
    if a.bundle.is_empty() {
        return Ok(None);
    }
    let bytes =
        base64::engine::general_purpose::STANDARD.decode(a.bundle.as_bytes()).map_err(|_| WireError::invalid("the bundle isn't base64"))?;
    let bundle = home.join(format!(".sandbox/{name}.bundle"));
    if let Some(p) = bundle.parent() {
        let _ = tokio::fs::create_dir_all(p).await;
    }
    tokio::fs::write(&bundle, bytes).await.map_err(|e| WireError::internal(format!("the bundle: {e}")))?;
    Ok(Some((bundle.clone(), a.patch.clone().into_bytes(), vec![bundle])))
}

/// Takes the project into `home/code/<name>`; its folder, and what its setup said when it ran.
pub async fn seed(ctx: &Ctx, home: &Path, a: &api::SeedArgs) -> Result<api::Seeded> {
    let proc = ctx.service::<Proc>()?;
    let name = core::name_of(&a.name);
    let code = home.join("code");
    let dir = code.join(&name);
    tokio::fs::create_dir_all(&code).await.map_err(|e| WireError::internal(format!("~/code: {e}")))?;
    let Some((bundle, patch, used)) = bundle_and_patch(home, &name, a).await? else {
        tokio::fs::create_dir_all(&dir).await.map_err(|e| WireError::internal(format!("{name}: {e}")))?;
        return Ok(api::Seeded { cwd: dir.display().to_string(), setup: None });
    };
    // the same project again (a member's own sandbox, the next job): a fresh copy of what came now
    let _ = tokio::fs::remove_dir_all(&dir).await;
    let cloned = async {
        for args in core::seed_commands(&bundle.display().to_string(), &dir.display().to_string(), &a.branch, &a.origin) {
            run(&proc, &code, "git", &args, None, 300)
                .await
                .map_err(|e| WireError::internal(format!("git {}: {e}", args.first().map_or("", String::as_str))))?;
        }
        Ok::<(), WireError>(())
    }
    .await;
    for f in used {
        let _ = tokio::fs::remove_file(f).await;
    }
    cloned?;
    let set = |k: &str, v: &str| ["config", k, v].map(str::to_owned).to_vec();
    if !a.author_name.is_empty() {
        let _ = run(&proc, &dir, "git", &set("user.name", &a.author_name), None, 10).await;
    }
    if !a.author_email.is_empty() {
        let _ = run(&proc, &dir, "git", &set("user.email", &a.author_email), None, 10).await;
    }
    if !patch.iter().all(u8::is_ascii_whitespace) {
        let apply = ["apply", "--whitespace=nowarn", "-"].map(str::to_owned).to_vec();
        run(&proc, &dir, "git", &apply, Some(patch), 120)
            .await
            .map_err(|e| WireError::internal(format!("the uncommitted changes didn't apply: {e}")))?;
    }
    // what it was when it came, new files too: a delegate's changes go back as the difference from this (no commit
    // made, no ref moved, the folder's index untouched)
    let base = tree_of(&proc, &dir).await.map_err(|e| WireError::internal(format!("the project's state: {e}")))?;
    let _ = tokio::fs::write(home.join(format!(".sandbox/base-{name}")), base).await;
    let setup = setup_once(&proc, home, &dir, &name).await;
    Ok(api::Seeded { cwd: dir.display().to_string(), setup })
}

/// The project's `.wisp/sandbox.sh`, the first time this sandbox gets the project (a mark in `~/.sandbox`).
async fn setup_once(proc: &Proc, home: &Path, dir: &Path, name: &str) -> Option<String> {
    let script = dir.join(".wisp/sandbox.sh");
    if !tokio::fs::metadata(&script).await.is_ok_and(|m| m.is_file()) {
        return None;
    }
    let mark: PathBuf = home.join(format!(".sandbox/setup-{name}"));
    if tokio::fs::metadata(&mark).await.is_ok() {
        return None;
    }
    let said = match run(proc, dir, "sh", &[".wisp/sandbox.sh".to_owned()], None, 900).await {
        Ok(o) => o,
        Err(e) => format!("setup failed: {e}"),
    };
    let _ = tokio::fs::write(&mark, b"").await;
    Some(said.chars().rev().take(4000).collect::<Vec<_>>().into_iter().rev().collect())
}

/// `sandbox.patch` (inside a sandbox): what changed in a project since it came in, new files too, as one binary diff.
pub async fn patch(ctx: &Ctx, home: &Path, cwd: &str) -> Result<api::Patch> {
    let proc = ctx.service::<Proc>()?;
    let dir = PathBuf::from(cwd);
    let name = core::name_of(cwd);
    if !dir.starts_with(home.join("code")) {
        return Err(WireError::invalid("only a project under ~/code has changes to give back"));
    }
    let base = tokio::fs::read_to_string(home.join(format!(".sandbox/base-{name}")))
        .await
        .map_err(|_| WireError::not_found("That project didn't come in through a seed."))?;
    // new files count: the folder's whole state now, through a scratch index
    let now = tree_of(&proc, &dir).await.map_err(|e| WireError::internal(format!("the project's state: {e}")))?;
    let diff = run(&proc, &dir, "git", &["diff".to_owned(), "--binary".to_owned(), base.trim().to_owned(), now], None, 120)
        .await
        .map_err(|e| WireError::internal(format!("git diff: {e}")))?;
    Ok(api::Patch { patch: diff })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sh(at: &Path, args: &[&str]) {
        let o = std::process::Command::new("git").current_dir(at).args(args).output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    }

    /// Round 188's sweep: `tree_of` starts from an empty index (`read-tree HEAD` records no stat data), so git
    /// re-hashes every file and a same-size edit in the real index's own second is never mistaken for clean.
    /// Stamps pinned as in carry.rs's test; a seeded copy with a fresh mtime would fail here.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_same_size_edit_in_the_index_s_own_second_is_a_change() {
        let dir = std::env::temp_dir().join(format!("wisp-sbx-racy-{}", wisp_kernel::ids::uuid7()));
        std::fs::create_dir_all(&dir).unwrap();
        sh(&dir, &["init", "-q"]);
        sh(&dir, &["config", "core.autocrlf", "false"]);
        sh(&dir, &["config", "core.trustctime", "false"]);
        let then = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let stamp = |p: &Path| std::fs::OpenOptions::new().write(true).open(p).unwrap().set_modified(then).unwrap();
        std::fs::write(dir.join("math.js"), "a - b\n").unwrap();
        stamp(&dir.join("math.js"));
        sh(&dir, &["add", "."]);
        sh(&dir, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "one"]);
        stamp(&dir.join(".git/index"));
        std::fs::write(dir.join("math.js"), "a + b\n").unwrap();
        stamp(&dir.join("math.js"));
        let proc = wisp_proc::local();
        let tree = tree_of(&proc, &dir).await.unwrap();
        let o = std::process::Command::new("git").current_dir(&dir).args(["show", &format!("{tree}:math.js")]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout), "a + b\n", "the edit is seen");
        let _ = std::fs::remove_dir_all(dir);
    }
}
