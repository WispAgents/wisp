//! ADR-0059: a delegate's copy on another machine. What the machine's wisp does with carried work (`machines.takeIn`,
//! `machines.changes`) and what `machines.carry` refuses before anything leaves; the whole trip between two wisps is
//! `crates/wisp/tests/serve_delegate_machine.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde_json::json;
use wisp_kernel::services::Paths;
use wisp_kernel::{Caller, Code, Kernel};
use wisp_machines::MachinesPlugin;
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wisp-carry-{name}-{}", wisp_kernel::ids::uuid7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn git(at: &Path, args: &[&str]) -> String {
    let o = Command::new("git").current_dir(at).args(args).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

async fn kernel(home: &Path) -> Kernel {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    TestApp::new().service(store).service(Paths::new(home)).service(wisp_proc::local()).with(MachinesPlugin).start().await
}

/// A lead's repo on branch `fix`: a commit, an uncommitted edit and a new file. What the upload route keeps for `key`
/// (as `machines.carry` makes it), and its `machines.takeIn` args.
fn carried(home: &Path, key: &str) -> (PathBuf, serde_json::Value) {
    let p = dir("lead").join("my app");
    std::fs::create_dir_all(&p).unwrap();
    git(&p, &["init", "-q", "-b", "main"]);
    // the lead's checkout as the test writes it: no line-ending conversion (the Windows runner's git says autocrlf=true)
    git(&p, &["config", "core.autocrlf", "false"]);
    std::fs::write(p.join("math.js"), "a - b\n").unwrap();
    std::fs::write(p.join("README.md"), "hi\n").unwrap();
    git(&p, &["add", "."]);
    git(&p, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "first"]);
    git(&p, &["checkout", "-qb", "fix"]);
    git(&p, &["remote", "add", "origin", "git@github.com:me/app.git"]);
    std::fs::write(p.join("README.md"), "hi\nnot committed\n").unwrap();
    std::fs::write(p.join("new.txt"), "a new file\n").unwrap();
    let incoming = home.join("plugins/wisp.machines/incoming");
    std::fs::create_dir_all(&incoming).unwrap();
    git(&p, &["bundle", "create", incoming.join(format!("{key}.bundle")).to_str().unwrap(), "HEAD", "fix"]);
    // the uncommitted work, new files too (machines.carry reads it through a scratch index)
    git(&p, &["add", "-N", "new.txt"]);
    std::fs::write(incoming.join(format!("{key}.patch")), git(&p, &["diff", "HEAD", "--binary"])).unwrap();
    git(&p, &["reset", "-q", "new.txt"]);
    let head = git(&p, &["rev-parse", "HEAD"]).trim().to_owned();
    (p, json!({ "key": key, "name": "my-app", "head": head, "branch": "fix", "authorName": "Ada", "authorEmail": "ada@example.com" }))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_copy_lands_under_wisps_own_folder_never_the_users_checkout() {
    let home = dir("home");
    let k = kernel(&home).await;
    // a key that tries to climb out lands under copies/ all the same
    let (_lead, mut args) = carried(&home, "-..-code");
    args["key"] = json!("../../code");
    let out = k.call(Caller::user(), "machines.takeIn", args).await.unwrap();
    let cwd = PathBuf::from(out["cwd"].as_str().unwrap());
    assert_eq!(cwd, home.join("plugins/wisp.machines/copies/-..-code/my-app"), "{out}");
    assert!(!home.join("code").exists(), "nothing outside wisp's own folder");
    // the lead's branch, its commit, its uncommitted edit and its new file; no remote to push to; the lead as author
    assert_eq!(git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "fix");
    assert_eq!(std::fs::read_to_string(cwd.join("README.md")).unwrap(), "hi\nnot committed\n");
    assert_eq!(std::fs::read_to_string(cwd.join("new.txt")).unwrap(), "a new file\n");
    assert_eq!(git(&cwd, &["remote"]).trim(), "", "a delegate never pushes");
    assert_eq!(git(&cwd, &["config", "user.email"]).trim(), "ada@example.com");
    assert!(!home.join("plugins/wisp.machines/incoming/-..-code.bundle").exists(), "the upload is gone once taken in");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_copys_changes_are_the_difference_from_what_came_in() {
    let home = dir("home2");
    let k = kernel(&home).await;
    let (lead, args) = carried(&home, "lead1-far");
    let cwd = k.call(Caller::user(), "machines.takeIn", args).await.unwrap()["cwd"].as_str().unwrap().to_owned();
    let copy = PathBuf::from(&cwd);
    let none = k.call(Caller::user(), "machines.changes", json!({ "cwd": cwd })).await.unwrap();
    assert_eq!(none["patch"], "", "nothing changed yet: the lead's own uncommitted work isn't a change");
    // the delegate edits a file and adds one, and doesn't commit
    std::fs::write(copy.join("math.js"), "a + b\n").unwrap();
    std::fs::write(copy.join("notes.txt"), "from the delegate\n").unwrap();
    let patch = k.call(Caller::user(), "machines.changes", json!({ "cwd": cwd })).await.unwrap()["patch"].as_str().unwrap().to_owned();
    assert!(patch.contains("math.js") && patch.contains("notes.txt"), "{patch}");
    assert!(!patch.contains("README.md") && !patch.contains("new.txt"), "only the delegate's: {patch}");
    // it applies to the lead's folder as it is (with its own uncommitted work), and the copy's index stayed put
    let o = Command::new("git").current_dir(&lead).args(["apply", "-"]).stdin(std::process::Stdio::piped()).spawn().unwrap();
    {
        use std::io::Write as _;
        o.stdin.as_ref().unwrap().write_all(patch.as_bytes()).unwrap();
    }
    assert!(o.wait_with_output().unwrap().status.success());
    assert_eq!(std::fs::read_to_string(lead.join("math.js")).unwrap(), "a + b\n");
    assert_eq!(std::fs::read_to_string(lead.join("README.md")).unwrap(), "hi\nnot committed\n");
    assert_eq!(git(&copy, &["diff", "--cached", "--name-only"]).trim(), "", "nothing staged in the copy");
}

#[tokio::test(flavor = "multi_thread")]
async fn only_a_carried_copy_gives_its_changes_back() {
    let home = dir("home3");
    let k = kernel(&home).await;
    let (lead, _args) = carried(&home, "k");
    for cwd in [lead.display().to_string(), home.join("plugins/wisp.machines/copies/k/../../../../x").display().to_string()] {
        let e = k.call(Caller::user(), "machines.changes", json!({ "cwd": cwd })).await.unwrap_err();
        assert_eq!(e.code, Code::Forbidden, "{cwd}: {e:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lead_folder_that_isnt_a_repo_or_an_unknown_machine_is_refused_before_anything_goes() {
    let home = dir("home4");
    let k = kernel(&home).await;
    let plain = dir("plain");
    let e = k.call(Caller::user(), "machines.carry", json!({ "id": "nope", "cwd": plain, "key": "k" })).await.unwrap_err();
    assert_eq!(e.code, Code::NotFound, "{e:?}");
    let m = k
        .call(Caller::user(), "machines.add", json!({ "transport": "loopback", "address": "127.0.0.1", "port": 9, "connect": false }))
        .await
        .unwrap();
    let e = k.call(Caller::user(), "machines.carry", json!({ "id": m["id"], "cwd": plain, "key": "k" })).await.unwrap_err();
    assert!(e.message.contains("isn't a git repo with a commit"), "{e:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn carrying_and_taking_in_need_their_permissions() {
    let home = dir("home5");
    let k = kernel(&home).await;
    let nobody = Caller::set("pane:wisp-testkit-nobody");
    for (cmd, args) in [
        ("machines.carry", json!({ "id": "x", "cwd": "/", "key": "k" })),
        ("machines.takeIn", json!({ "key": "k", "name": "n", "head": "ab" })),
        ("machines.changes", json!({ "cwd": "/" })),
        ("machines.bring", json!({ "id": "x", "from": "/", "into": "/" })),
        ("machines.send", json!({ "id": "x", "cwd": "/", "key": "k", "to": "/sandbox/seed" })),
        ("machines.uncarry", json!({ "id": "x", "lead": "t1" })),
        ("machines.dropCopies", json!({ "lead": "t1" })),
    ] {
        let e = k.call(nobody.clone(), cmd, args).await.unwrap_err();
        assert_eq!(e.code, Code::Forbidden, "{cmd}: {e:?}");
    }
}

/// ADR-0060: a lead deleted while its delegate's machine can't be reached: the drop is kept in the machine's row for
/// the sweep (once, however often it's asked), and the machine's own copies of that lead go when it's told, only
/// that lead's.
#[tokio::test(flavor = "multi_thread")]
async fn a_machine_that_cant_be_reached_keeps_the_drop_for_the_sweep() {
    let home = dir("home6");
    let k = kernel(&home).await;
    let m = k
        .call(Caller::user(), "machines.add", json!({ "transport": "loopback", "address": "127.0.0.1", "port": 9, "connect": false }))
        .await
        .unwrap();
    for _ in 0..2 {
        let out = k.call(Caller::user(), "machines.uncarry", json!({ "id": m["id"], "lead": "t1", "task": "r1" })).await.unwrap();
        assert_eq!(out["done"], false, "{out}");
    }
    let row = k.call(Caller::user(), "machines.get", json!({ "id": m["id"] })).await.unwrap();
    assert_eq!(row["uncarry"], json!([{ "lead": "t1", "task": "r1" }]), "kept once, for the sweep: {row}");
    assert!(k.call(Caller::user(), "machines.send", json!({ "id": m["id"], "cwd": "/", "key": "k", "to": "/cmd" })).await.is_err());

    // on the machine: that lead's copies and uploads go, another lead's stay
    let (_l, a) = carried(&home, "t1-far");
    k.call(Caller::user(), "machines.takeIn", a).await.unwrap();
    let (_l, b) = carried(&home, "t10-far");
    k.call(Caller::user(), "machines.takeIn", b).await.unwrap();
    std::fs::write(home.join("plugins/wisp.machines/incoming/t1-near.bundle"), b"x").unwrap();
    let out = k.call(Caller::user(), "machines.dropCopies", json!({ "lead": "t1" })).await.unwrap();
    assert_eq!(out["copies"], 1, "{out}");
    let copies = home.join("plugins/wisp.machines/copies");
    assert!(!copies.join("t1-far").exists() && copies.join("t10-far/my-app").is_dir());
    assert!(!home.join("plugins/wisp.machines/incoming/t1-near.bundle").exists());
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

/// Windows CI (round 196): the machine's git checked the copy out with CRLF, so the lead's LF files read
/// "hi\r\nnot committed\r\n" there and every line was a change. A copy is wisp's own: byte for byte what was committed
/// (LF, CRLF and mixed files alike), the lead's uncommitted edit on top, nothing reads as changed until the delegate
/// changes something, and then only its line.
#[tokio::test(flavor = "multi_thread")]
async fn a_copy_is_byte_for_byte_what_came_whatever_the_machines_git_says() {
    let home = dir("home-crlf");
    let config = home.join("windows.gitconfig");
    std::fs::write(&config, "[core]\n\tautocrlf = true\n").unwrap();
    let proc: wisp_proc::Proc = Arc::new(AutocrlfGit { inner: wisp_proc::local(), config });
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new().service(store).service(Paths::new(&home)).service(proc).with(MachinesPlugin).start().await;

    // the lead's own checkout says autocrlf=true too (kept: wisp never changes it); its blobs are written as they are
    let p = dir("lead-crlf").join("app");
    std::fs::create_dir_all(&p).unwrap();
    git(&p, &["init", "-q", "-b", "main"]);
    git(&p, &["config", "core.autocrlf", "true"]);
    let files = [("lf.txt", "one\ntwo\n"), ("crlf.txt", "one\r\ntwo\r\n"), ("mixed.txt", "one\r\ntwo\n")];
    for (f, text) in files {
        std::fs::write(p.join(f), text).unwrap();
    }
    git(&p, &["-c", "core.autocrlf=false", "add", "."]);
    git(&p, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "first"]);
    std::fs::write(p.join("lf.txt"), "one\ntwo\nnot committed\n").unwrap();
    let incoming = home.join("plugins/wisp.machines/incoming");
    std::fs::create_dir_all(&incoming).unwrap();
    git(&p, &["bundle", "create", incoming.join("crlf.bundle").to_str().unwrap(), "HEAD", "main"]);
    std::fs::write(incoming.join("crlf.patch"), git(&p, &["diff", "HEAD", "--binary"])).unwrap();
    let head = git(&p, &["rev-parse", "HEAD"]).trim().to_owned();

    let args = json!({ "key": "crlf", "name": "app", "head": head, "branch": "main" });
    let cwd = k.call(Caller::user(), "machines.takeIn", args).await.unwrap()["cwd"].as_str().unwrap().to_owned();
    let copy = PathBuf::from(&cwd);
    assert_eq!(std::fs::read(copy.join("lf.txt")).unwrap(), b"one\ntwo\nnot committed\n", "the lead's edit, as it was");
    assert_eq!(std::fs::read(copy.join("crlf.txt")).unwrap(), b"one\r\ntwo\r\n");
    assert_eq!(std::fs::read(copy.join("mixed.txt")).unwrap(), b"one\r\ntwo\n");
    assert_eq!(git(&copy, &["config", "--local", "core.autocrlf"]).trim(), "false", "the copy's own setting, for the delegate's git too");
    let none = k.call(Caller::user(), "machines.changes", json!({ "cwd": cwd })).await.unwrap();
    assert_eq!(none["patch"], "", "nothing reads as changed: {none}");

    std::fs::write(copy.join("crlf.txt"), "one\r\ntwo\r\nthree\r\n").unwrap();
    let patch = k.call(Caller::user(), "machines.changes", json!({ "cwd": cwd })).await.unwrap()["patch"].as_str().unwrap().to_owned();
    let changed: Vec<&str> = patch
        .split('\n')
        .filter(|l| (l.starts_with('+') || l.starts_with('-')) && !l.starts_with("+++") && !l.starts_with("---"))
        .collect();
    assert_eq!(changed, ["+three\r"], "only the delegate's line: {patch}");
}
