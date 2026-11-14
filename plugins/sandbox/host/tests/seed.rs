//! ADR-0047: a sandbox's wisp takes a project in (a bundle of every ref, the uncommitted changes, the branch, origin
//! and author) into `~/code/<name>`, and runs its `.wisp/sandbox.sh` once; anywhere else the seed is refused.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use serde_json::json;
use wisp_kernel::Caller;
use wisp_sandbox::SandboxPlugin;
use wisp_testkit::TestApp;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wisp-sbx-{name}-{}", wisp_kernel::ids::uuid7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn git(at: &Path, args: &[&str]) -> String {
    let o = Command::new("git").current_dir(at).args(args).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// A repo on branch `fix` with a commit, a setup script, and an uncommitted change; its seed args.
fn project() -> (PathBuf, serde_json::Value) {
    let p = dir("proj");
    git(&p, &["init", "-q", "-b", "main"]);
    // the lead's checkout as the test writes it: no line-ending conversion (the Windows runner's git says autocrlf=true)
    git(&p, &["config", "core.autocrlf", "false"]);
    std::fs::write(p.join("app.js"), "one\n").unwrap();
    std::fs::create_dir_all(p.join(".wisp")).unwrap();
    std::fs::write(p.join(".wisp/sandbox.sh"), "echo set up\n").unwrap();
    git(&p, &["add", "."]);
    git(&p, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "first"]);
    git(&p, &["checkout", "-qb", "fix"]);
    std::fs::write(p.join("app.js"), "one\ntwo, not committed\n").unwrap();
    let bundle = dir("bundle").join("b.bundle");
    git(&p, &["bundle", "create", bundle.to_str().unwrap(), "--all"]);
    let b = base64::engine::general_purpose::STANDARD.encode(std::fs::read(&bundle).unwrap());
    let patch = git(&p, &["diff", "HEAD", "--binary"]);
    (
        p,
        json!({ "name": "my app", "bundle": b, "patch": patch, "branch": "fix", "origin": "git@github.com:me/app.git", "authorName": "Ada", "authorEmail": "ada@example.com" }),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn only_a_sandbox_takes_a_seed_and_its_setup_runs_once() {
    let home = dir("home");
    let (_p, args) = project();
    // a computer refuses: a seed would write into its ~/code
    let here =
        TestApp::new().with(SandboxPlugin { inside: Some(false), home: Some(home.clone()) }).service(wisp_proc::local()).start().await;
    let no = here.call(Caller::user(), "sandbox.seed", args.clone()).await.unwrap_err();
    assert!(no.message.contains("Only a sandbox"), "{no:?}");
    let sb = TestApp::new().with(SandboxPlugin { inside: Some(true), home: Some(home.clone()) }).service(wisp_proc::local()).start().await;
    let out = sb.call(Caller::user(), "sandbox.seed", args.clone()).await.unwrap();
    let cwd = PathBuf::from(out["cwd"].as_str().unwrap());
    assert_eq!(cwd, home.join("code/my-app"));
    assert_eq!(std::fs::read_to_string(cwd.join("app.js")).unwrap(), "one\ntwo, not committed\n", "the uncommitted change came along");
    assert_eq!(git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "fix");
    assert_eq!(git(&cwd, &["remote", "get-url", "origin"]).trim(), "git@github.com:me/app.git");
    assert_eq!(git(&cwd, &["config", "user.email"]).trim(), "ada@example.com");
    assert_eq!(out["setup"].as_str().map(str::trim), Some("set up"), "{out}");
    // the same project again: a fresh copy, and the setup doesn't run twice
    let again = sb.call(Caller::user(), "sandbox.seed", args).await.unwrap();
    assert!(again.get("setup").is_none_or(serde_json::Value::is_null), "{again}");
    assert!(home.join(".sandbox/setup-my-app").is_file());
}

/// A delegate's work comes back as the difference from what came in: an edit and a new file, and nothing of the lead's
/// own uncommitted change (it came in with the seed), so the patch applies to the lead's folder as it is.
#[tokio::test(flavor = "multi_thread")]
async fn a_delegates_changes_come_back_as_the_difference_from_what_came_in() {
    let home = dir("home2");
    let (lead, args) = project();
    let sb = TestApp::new().with(SandboxPlugin { inside: Some(true), home: Some(home.clone()) }).service(wisp_proc::local()).start().await;
    let out = sb.call(Caller::user(), "sandbox.seed", args).await.unwrap();
    let cwd = PathBuf::from(out["cwd"].as_str().unwrap());
    std::fs::write(cwd.join("app.js"), "one\ntwo, not committed\nthree, by the delegate\n").unwrap();
    std::fs::write(cwd.join("new.txt"), "made there\n").unwrap();
    let p = sb.call(Caller::user(), "sandbox.patch", serde_json::json!({ "cwd": cwd })).await.unwrap();
    let patch = p["patch"].as_str().unwrap().to_owned();
    assert_eq!(patch.matches("diff --git ").count(), 2, "{patch}");
    assert!(!patch.contains("+two, not committed"), "the lead's own change isn't sent back: {patch}");
    let o = Command::new("git").current_dir(&lead).args(["apply", "-"]).stdin(std::process::Stdio::piped()).spawn().unwrap();
    o.stdin.as_ref().unwrap().write_all(patch.as_bytes()).unwrap();
    assert!(o.wait_with_output().unwrap().status.success());
    assert_eq!(std::fs::read_to_string(lead.join("app.js")).unwrap(), "one\ntwo, not committed\nthree, by the delegate\n");
    assert_eq!(std::fs::read_to_string(lead.join("new.txt")).unwrap(), "made there\n");
    // outside ~/code: nothing to give back
    assert!(sb.call(Caller::user(), "sandbox.patch", serde_json::json!({ "cwd": "/etc" })).await.is_err());
}

fn upload(key: &str, part: &str, body: Vec<u8>) -> wisp_kernel::http::Request {
    wisp_kernel::http::Request {
        method: "POST".into(),
        path: format!("/api/sandbox/seed/{key}/{part}"),
        params: std::collections::BTreeMap::new(),
        query: Vec::new(),
        headers: vec![("content-type".into(), "application/octet-stream".into())],
        body,
    }
}

/// ADR-0060: the project comes raw to `POST /api/sandbox/seed/<key>/<part>` (as `machines.send` posts it: a bundle of
/// every ref, and the uncommitted changes with a new file), and `sandbox.seed {key}` takes it from there; a computer
/// refuses the upload as it refuses the seed, and a seed for nothing sent is refused.
#[tokio::test(flavor = "multi_thread")]
async fn only_a_sandbox_takes_an_upload_and_a_seed_reads_it_by_key() {
    let (p, _) = project();
    std::fs::write(p.join("brand-new.txt"), "not committed, not even added\n").unwrap();
    let bundle = dir("bundle-k").join("b.bundle");
    git(&p, &["bundle", "create", bundle.to_str().unwrap(), "--all"]);
    git(&p, &["add", "-N", "brand-new.txt"]);
    let patch = git(&p, &["diff", "HEAD", "--binary"]);
    git(&p, &["reset", "-q", "brand-new.txt"]);
    let bytes = std::fs::read(&bundle).unwrap();

    let home = dir("home3");
    let here =
        TestApp::new().with(SandboxPlugin { inside: Some(false), home: Some(home.clone()) }).service(wisp_proc::local()).start().await;
    let no = here.serve(Caller::user(), upload("k1", "bundle", bytes.clone())).await.unwrap_err();
    assert!(no.message.contains("Only a sandbox"), "{no:?}");
    assert!(!home.join(".sandbox").exists(), "nothing written on a computer");

    let sb = TestApp::new().with(SandboxPlugin { inside: Some(true), home: Some(home.clone()) }).service(wisp_proc::local()).start().await;
    let none = sb.call(Caller::user(), "sandbox.seed", json!({ "name": "app", "key": "k1" })).await.unwrap_err();
    assert!(none.message.contains("Nothing was sent"), "{none:?}");
    assert!(sb.serve(Caller::user(), upload("k1", "../x", Vec::new())).await.is_err(), "a part is a bundle or a patch");
    sb.serve(Caller::user(), upload("k1", "bundle", bytes)).await.unwrap();
    sb.serve(Caller::user(), upload("k1", "patch", patch.into_bytes())).await.unwrap();
    let out = sb.call(Caller::user(), "sandbox.seed", json!({ "name": "my app", "key": "k1", "branch": "fix" })).await.unwrap();
    let cwd = PathBuf::from(out["cwd"].as_str().unwrap());
    assert_eq!(cwd, home.join("code/my-app"));
    assert_eq!(std::fs::read_to_string(cwd.join("app.js")).unwrap(), "one\ntwo, not committed\n");
    assert_eq!(std::fs::read_to_string(cwd.join("brand-new.txt")).unwrap(), "not committed, not even added\n");
    assert_eq!(git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "fix");
    assert_eq!(std::fs::read_dir(home.join(".sandbox/incoming")).unwrap().count(), 0, "the upload is used once");
    // the lead's new file came in with it: it never goes back as the delegate's
    let back = sb.call(Caller::user(), "sandbox.patch", json!({ "cwd": cwd })).await.unwrap();
    assert_eq!(back["patch"], "", "nothing changed there yet: {back}");
}

/// Every git this test's wisp runs reads `core.autocrlf=true` from a config wisp didn't write, as on the Windows
/// runner (Git for Windows' system default), on any OS.
struct AutocrlfGit {
    inner: wisp_proc::Proc,
    config: PathBuf,
}

impl AutocrlfGit {
    fn env(&self, env: wisp_proc::Env) -> wisp_proc::Env {
        env.with("GIT_CONFIG_GLOBAL", self.config.display().to_string()).with("GIT_CONFIG_NOSYSTEM", "1")
    }
}

impl wisp_proc::ProcService for AutocrlfGit {
    fn which(&self, bin: &str, env: Option<Vec<(String, String)>>) -> wisp_proc::BoxFuture<'static, Option<PathBuf>> {
        self.inner.which(bin, env)
    }
    fn spawn(&self, bin: &str, args: &[String], mut opts: wisp_proc::SpawnOpts) -> wisp_proc::Result<wisp_proc::Child> {
        opts.env = self.env(opts.env);
        self.inner.spawn(bin, args, opts)
    }
    fn exec(
        &self,
        bin: &str,
        args: &[String],
        mut opts: wisp_proc::ExecOpts,
    ) -> wisp_proc::BoxFuture<'static, wisp_proc::Result<wisp_proc::ExecResult>> {
        opts.env = self.env(opts.env);
        self.inner.exec(bin, args, opts)
    }
    fn spawn_detached(&self, bin: &str, args: &[String], mut opts: wisp_proc::SpawnOpts) -> wisp_proc::Result<u32> {
        opts.env = self.env(opts.env);
        self.inner.spawn_detached(bin, args, opts)
    }
    fn live(&self) -> usize {
        self.inner.live()
    }
    fn kill_all(&self, grace: std::time::Duration) -> wisp_proc::BoxFuture<'static, ()> {
        self.inner.kill_all(grace)
    }
}

/// Windows CI (round 196): the box's git checked the seed out with CRLF, so "one\ntwo, not committed\n" read
/// "one\r\ntwo, not committed\r\n" there and every line was a change. A seeded project is byte for byte what was
/// committed (LF, CRLF and mixed files alike), the uncommitted edit on top, and its patch back holds only the
/// delegate's line.
#[tokio::test(flavor = "multi_thread")]
async fn a_seed_is_byte_for_byte_what_came_whatever_the_boxs_git_says() {
    let home = dir("home-crlf");
    let config = home.join("windows.gitconfig");
    std::fs::write(&config, "[core]\n\tautocrlf = true\n").unwrap();
    let proc: wisp_proc::Proc = std::sync::Arc::new(AutocrlfGit { inner: wisp_proc::local(), config });
    let sb = TestApp::new().with(SandboxPlugin { inside: Some(true), home: Some(home.clone()) }).service(proc).start().await;

    // the lead's own checkout says autocrlf=true too (kept: wisp never changes it); its blobs are written as they are
    let p = dir("proj-crlf");
    git(&p, &["init", "-q", "-b", "main"]);
    git(&p, &["config", "core.autocrlf", "true"]);
    for (f, text) in [("lf.txt", "one\ntwo\n"), ("crlf.txt", "one\r\ntwo\r\n"), ("mixed.txt", "one\r\ntwo\n")] {
        std::fs::write(p.join(f), text).unwrap();
    }
    git(&p, &["-c", "core.autocrlf=false", "add", "."]);
    git(&p, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "first"]);
    std::fs::write(p.join("lf.txt"), "one\ntwo, not committed\n").unwrap();
    let bundle = dir("bundle-crlf").join("b.bundle");
    git(&p, &["bundle", "create", bundle.to_str().unwrap(), "--all"]);
    let b = base64::engine::general_purpose::STANDARD.encode(std::fs::read(&bundle).unwrap());
    let patch = git(&p, &["diff", "HEAD", "--binary"]);

    let out =
        sb.call(Caller::user(), "sandbox.seed", json!({ "name": "crlf", "bundle": b, "patch": patch, "branch": "main" })).await.unwrap();
    let cwd = PathBuf::from(out["cwd"].as_str().unwrap());
    assert_eq!(std::fs::read(cwd.join("lf.txt")).unwrap(), b"one\ntwo, not committed\n", "the uncommitted change, as it was");
    assert_eq!(std::fs::read(cwd.join("crlf.txt")).unwrap(), b"one\r\ntwo\r\n");
    assert_eq!(std::fs::read(cwd.join("mixed.txt")).unwrap(), b"one\r\ntwo\n");
    assert_eq!(git(&cwd, &["config", "--local", "core.autocrlf"]).trim(), "false", "the copy's own setting, for the delegate's git too");
    let none = sb.call(Caller::user(), "sandbox.patch", json!({ "cwd": cwd })).await.unwrap();
    assert_eq!(none["patch"], "", "nothing reads as changed: {none}");

    std::fs::write(cwd.join("crlf.txt"), "one\r\ntwo\r\nthree\r\n").unwrap();
    let back = sb.call(Caller::user(), "sandbox.patch", json!({ "cwd": cwd })).await.unwrap()["patch"].as_str().unwrap().to_owned();
    let changed: Vec<&str> =
        back.split('\n').filter(|l| (l.starts_with('+') || l.starts_with('-')) && !l.starts_with("+++") && !l.starts_with("---")).collect();
    assert_eq!(changed, ["+three\r"], "only the delegate's line: {back}");
}
