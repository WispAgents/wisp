//! ADR-0042's edge cases for wisp.history, on a kernel with stand-ins for wisp.runtime and wisp.share.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::assert_is_empty)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_history_api::{BLAME, BlameSource, HistoryStore, STORE};
use wisp_kernel::{BoxFuture, Caller, Command, Kernel, Manifest, Plugin, Result, Setup};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

macro_rules! fake_cmd {
    ($ty:ident, $id:literal) => {
        struct $ty;
        impl Command for $ty {
            const ID: &'static str = $id;
            type Args = Value;
            type Out = Value;
        }
    };
}
fake_cmd!(Get, "runtime.get");
fake_cmd!(SetExt, "runtime.setExt");
fake_cmd!(Turns, "runtime.turns");
/// `share.preview`, under the real one's permission.
struct Preview;
impl Command for Preview {
    const ID: &'static str = "share.preview";
    const PERMISSION: Option<&'static str> = Some("share:make");
    type Args = Value;
    type Out = Value;
}

/// The fake task's `wisp.history` slot, and a write to land just before the next offer does (another write that
/// lands between the offer's read and its write).
#[derive(Default)]
struct Slot {
    value: Value,
    before_offer: Option<Value>,
    /// the task's `wisp.worktrees` slot (its worktree, branch and remote), when it has one
    worktrees: Value,
}

/// wisp.runtime with one task, `t1`, in `dir`; its title can change; its history slot is kept and `runtime.setExt`
/// honours `expect` as the real one does (refused with `conflict` unless the slot still holds it).
struct Runtime(PathBuf, Arc<Mutex<String>>, Arc<Mutex<Slot>>);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(
            json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1", "activation": ["onStartup"] }),
        )
        .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let (dir, title, slot) = (self.0.display().to_string(), self.1.clone(), self.2.clone());
        let read = slot.clone();
        s.command::<Get, _, _>(move |_c, _call, _a| {
            let (dir, title) = (dir.clone(), title.lock().unwrap().clone());
            let (ext, wt) = {
                let s = read.lock().unwrap();
                (s.value.clone(), s.worktrees.clone())
            };
            // the real runtime.get's shape (a whole Session), so a wrong-shape read fails here
            let mut t = wisp_runtime_api::model::Session {
                id: "01a1bc12ab34cdef0123456789abcdef".into(),
                title,
                cwd: dir,
                agent: "claude".into(),
                state: wisp_runtime_api::model::SessionState::Done,
                ..Default::default()
            };
            if !ext.is_null() {
                t.ext.insert(wisp_history_api::EXT.into(), ext);
            }
            if !wt.is_null() {
                t.ext.insert("wisp.worktrees".into(), wt);
            }
            async move { Ok(serde_json::to_value(t).unwrap()) }
        });
        s.command::<SetExt, _, _>(move |_c, _call, a| {
            let mut s = slot.lock().unwrap();
            if a["value"]["offer"] == true
                && let Some(v) = s.before_offer.take()
            {
                s.value = v;
            }
            let r = match a.get("expect") {
                Some(e) if e["value"] != s.value => Err(wisp_kernel::WireError::conflict("changed meanwhile")),
                _ => {
                    s.value = a["value"].clone();
                    Ok(json!({}))
                }
            };
            async move { r }
        });
        s.command::<Turns, _, _>(|_c, _call, _a| async { Ok(json!([])) });
        Ok(())
    }
}

/// wisp.share: a full bundle for the task.
struct Share(Arc<Mutex<String>>);
impl Plugin for Share {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(
            json!({ "wispPlugin": 1, "id": "wisp.share", "version": "1.0.0-alpha.1", "api": "^1", "activation": ["onStartup"] }),
        )
        .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let title = self.0.clone();
        s.command::<Preview, _, _>(move |_c, _call, _a| {
            let title = title.lock().unwrap().clone();
            async move { Ok(json!({ "bundle": { "wispTask": 1, "id": "share-x", "level": "full", "title": title, "createdAt": 1_794_181_336_230_i64, "version": "2", "needs": [], "spec": { "prompt": "fix it", "agent": "claude" }, "formula": {}, "report": { "answers": [], "files": ["math.js"] }, "history": [{ "turn": { "prompt": "fix it" } }] }, "redactions": [] })) }
        });
        Ok(())
    }
}

#[derive(Default, Clone)]
struct Kept(Arc<Mutex<Vec<String>>>);

/// A user's store, `db`: it keeps the paths it's handed.
struct Db(Kept);
impl HistoryStore for Db {
    fn id(&self) -> &'static str {
        "db"
    }
    fn write(&self, _root: PathBuf, files: Vec<(String, Vec<u8>)>) -> BoxFuture<'static, Result<Vec<String>>> {
        let names: Vec<String> = files.into_iter().map(|(r, _)| r).collect();
        self.0.0.lock().unwrap().extend(names.clone());
        Box::pin(async move { Ok(names) })
    }
    fn remove(&self, _root: PathBuf, _rels: Vec<String>) -> BoxFuture<'static, Result<Vec<String>>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn read(&self, _root: PathBuf, _rel: String) -> BoxFuture<'static, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
}

/// A user's blame source, `jj`: every line of every file is one commit that names t1.
struct Jj;
impl BlameSource for Jj {
    fn id(&self) -> &'static str {
        "jj"
    }
    fn blame(&self, _r: PathBuf, _f: String, _g: Option<(u32, u32)>) -> BoxFuture<'static, Vec<(String, u32)>> {
        Box::pin(async { vec![("c0ffee".to_owned(), 7)] })
    }
    fn message(&self, _r: PathBuf, _s: String) -> BoxFuture<'static, String> {
        Box::pin(async { "x\n\nWisp-Task: 01a1bc12ab34".to_owned() })
    }
    fn commits(&self, _r: PathBuf, _s: String, _p: String) -> BoxFuture<'static, Vec<String>> {
        Box::pin(async { vec!["c0ffee".to_owned()] })
    }
    fn grep(&self, _r: PathBuf, _n: String) -> BoxFuture<'static, Vec<String>> {
        Box::pin(async { Vec::new() })
    }
    fn added(&self, _r: PathBuf, _s: String) -> BoxFuture<'static, Vec<(String, u32)>> {
        Box::pin(async { Vec::new() })
    }
    fn knows(&self, _r: PathBuf) -> BoxFuture<'static, bool> {
        Box::pin(async { true })
    }
}

struct Acme(Kept);
impl Plugin for Acme {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "acme.db", "version": "1.0.0", "api": "^1", "activation": ["onStartup"] }))
            .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.contribute(&STORE, Arc::new(Db(self.0.clone())) as Arc<dyn HistoryStore>);
        s.contribute(&BLAME, Arc::new(Jj) as Arc<dyn BlameSource>);
        Ok(())
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn dir(name: &str, yml: &str, repo: bool) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wisp-history-{name}-{}", wisp_kernel::ids::uuid7()));
    std::fs::create_dir_all(d.join(".wisp")).unwrap();
    std::fs::write(d.join(".wisp/wisp.yml"), format!("wisp: 1\n{yml}\n")).unwrap();
    std::fs::write(d.join("math.js"), "a + b\n").unwrap();
    if repo {
        git(&d, &["init", "-q", "-b", "main"]);
        git(&d, &["add", "."]);
        git(&d, &["commit", "-qm", "init"]);
    }
    d
}

async fn kernel(d: &Path, title: &Arc<Mutex<String>>, with_history: bool, kept: &Kept) -> Kernel {
    kernel_with(d, title, with_history, kept, Arc::default()).await
}

async fn kernel_with(d: &Path, title: &Arc<Mutex<String>>, with_history: bool, kept: &Kept, slot: Arc<Mutex<Slot>>) -> Kernel {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let mut app = TestApp::new()
        .with(Runtime(d.to_owned(), title.clone(), slot))
        .with(Share(title.clone()))
        .with(Acme(kept.clone()))
        .service(store)
        .service::<wisp_proc::Proc>(wisp_proc::local());
    if with_history {
        app = app.with(super::HistoryPlugin);
    }
    app.start().await
}

#[test]
fn repo_paths_use_slashes_on_windows_too() {
    // the Windows CI failure: `.wisp\wisp.yml`, and a retitled task kept its old `tasks\…` files
    assert_eq!(super::slashed(r"tasks\2026\11\01a1bc12ab34-fix.task.yml", true), "tasks/2026/11/01a1bc12ab34-fix.task.yml");
    assert_eq!(super::slashed(".wisp/wisp.yml", true), ".wisp/wisp.yml");
    assert_eq!(super::slashed(r"odd\name", false), r"odd\name");
    let listing = [super::slashed(r"tasks\2026\11\01a1bc12ab34-fix-the-login.task.yml", true)];
    assert_eq!(wisp_history_core::files_of(&listing, "01a1bc12ab34cdef0123456789abcdef").len(), 1);
}

fn files_under(d: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![d.join(".wisp")];
    while let Some(x) = stack.pop() {
        for e in std::fs::read_dir(&x).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                out.push(super::rel(e.path().strip_prefix(d).unwrap()));
            }
        }
    }
    out.sort();
    out
}

fn track() -> Value {
    json!({ "id": "t1" })
}

#[tokio::test(flavor = "multi_thread")]
async fn history_off_leaves_share_working() {
    let d = dir("off", "", true);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), false, &Kept::default()).await;
    assert!(k.call(Caller::user(), "share.preview", json!({ "id": "t1" })).await.is_ok());
    let e = k.call(Caller::user(), "history.track", track()).await.unwrap_err();
    assert!(e.message.contains("history.track"), "{e:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_user_store_receives_the_writes() {
    let d = dir("db", "history: { transcripts: db }", true);
    let kept = Kept::default();
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &kept).await;
    let t = k.call(Caller::user(), "history.track", track()).await.unwrap();
    assert_eq!(t["store"], "db");
    assert_eq!(kept.0.lock().unwrap().len(), 3, "task, report, transcript: {:?}", kept.0.lock().unwrap());
    assert_eq!(files_under(&d), [".wisp/wisp.yml"], "nothing in the tree");
}

#[tokio::test(flavor = "multi_thread")]
async fn notes_keep_the_tree_clean() {
    let d = dir("notes", "history: { transcripts: notes }", true);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    let t = k.call(Caller::user(), "history.track", track()).await.unwrap();
    let files = files_under(&d);
    assert!(files.iter().any(|f| f.ends_with(".task.yml")) && files.iter().any(|f| f.ends_with(".report.md")), "{files:?}");
    assert!(!files.iter().any(|f| f.contains("history/")), "the transcript isn't in the tree: {files:?}");
    assert!(t["files"].to_string().contains("refs/notes/wisp:history/01a1bc12ab34cdef0123456789abcdef.jsonl.zst"), "{t}");
    assert!(!git(&d, &["notes", "--ref=wisp", "list"]).trim().is_empty());
    // the note reads back exactly
    let st = super::stores::Notes(Arc::new(wisp_proc::local()));
    let z = st.read(d.clone(), "history/01a1bc12ab34cdef0123456789abcdef.jsonl.zst".into()).await.unwrap();
    assert_eq!(wisp_history_core::repo::read_transcript(&z), vec![json!({ "turn": { "prompt": "fix it" } })]);
    // and untracking takes it out of the notes
    k.call(Caller::user(), "history.untrack", json!({ "id": "01a1bc12ab34", "repo": d.display().to_string() })).await.unwrap();
    assert!(git(&d, &["notes", "--ref=wisp", "list"]).trim().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn notes_work_where_git_has_no_identity() {
    // CI and fresh boxes: no user.name/email and none to guess (`useConfigOnly` makes git refuse to guess)
    let d = dir("notes-noid", "history: { transcripts: notes }", true);
    git(&d, &["config", "user.useConfigOnly", "true"]);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    let t = k.call(Caller::user(), "history.track", track()).await.unwrap();
    assert!(t["files"].to_string().contains("refs/notes/wisp:"), "{t}");
    let who = std::process::Command::new("git")
        .args(["log", "-1", "--format=%an <%ae> / %cn <%ce>", "refs/notes/wisp"])
        .current_dir(&d)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&who.stdout).trim(),
        "wisp <wisp@localhost> / wisp <wisp@localhost>",
        "the notes commit is wisp's, not a guess"
    );
    let cfg = std::fs::read_to_string(d.join(".git/config")).unwrap();
    assert!(!cfg.contains("[user]\n\tname") && !cfg.contains("email"), "wisp never writes the user's git config: {cfg}");
    k.call(Caller::user(), "history.untrack", json!({ "id": "01a1bc12ab34", "repo": d.display().to_string() })).await.unwrap();
    assert!(git(&d, &["notes", "--ref=wisp", "list"]).trim().is_empty(), "removing works without an identity too");
}

#[tokio::test(flavor = "multi_thread")]
async fn lfs_marks_transcripts() {
    let d = dir("lfs", "history: { transcripts: lfs }", true);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    k.call(Caller::user(), "history.track", track()).await.unwrap();
    k.call(Caller::user(), "history.track", track()).await.unwrap();
    let attrs = std::fs::read_to_string(d.join(".gitattributes")).unwrap();
    assert_eq!(attrs.matches(super::stores::LFS_LINE).count(), 1, "{attrs}");
    assert!(files_under(&d).iter().any(|f| f.ends_with(".jsonl.zst")));
}

#[tokio::test(flavor = "multi_thread")]
async fn tracking_twice_rewrites_in_place() {
    let d = dir("twice", "", true);
    let title = Arc::new(Mutex::new("Fix the login".to_owned()));
    let k = kernel(&d, &title, true, &Kept::default()).await;
    k.call(Caller::user(), "history.track", track()).await.unwrap();
    *title.lock().unwrap() = "Fix the login for good".into();
    k.call(Caller::user(), "history.track", track()).await.unwrap();
    let files = files_under(&d);
    assert_eq!(files.iter().filter(|f| f.ends_with(".task.yml")).count(), 1, "{files:?}");
    assert_eq!(files.iter().filter(|f| f.ends_with(".report.md")).count(), 1, "{files:?}");
    assert!(files.iter().any(|f| f.contains("fix-the-login-for-good")), "the new title's name: {files:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_without_git_still_tracks() {
    let d = dir("nogit", "", false);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    let t = k.call(Caller::user(), "history.track", track()).await.unwrap();
    assert_eq!((t["store"].clone(), t["commits"].clone(), t["pr"].clone()), (json!("files"), json!([]), Value::Null));
    let b = k.call(Caller::user(), "history.blame", json!({ "target": "math.js:1", "repo": d.display().to_string() })).await.unwrap();
    assert_eq!(b["kind"], "unknown");
    assert!(b["note"].as_str().unwrap().contains("isn't under git"), "{b}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_user_blame_source_answers() {
    let d = dir("jj", "history: { blame: jj }", false);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    k.call(Caller::user(), "history.track", track()).await.unwrap();
    let b = k.call(Caller::user(), "history.blame", json!({ "target": "math.js", "repo": d.display().to_string() })).await.unwrap();
    assert_eq!((b["lines"].clone(), b["tasks"][0]["task"].clone()), (json!(7), json!("01a1bc12ab34cdef0123456789abcdef")), "{b}");
}

#[tokio::test(flavor = "multi_thread")]
async fn still_there_counts_the_lines_left() {
    let d = dir("still", "", true);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    std::fs::write(d.join("code.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    git(&d, &["add", "code.txt"]);
    git(&d, &["commit", "-qm", "the task's code\n\nWisp-Task: 01a1bc12ab34"]);
    k.call(Caller::user(), "history.track", track()).await.unwrap();
    std::fs::write(d.join("code.txt"), "one\nTWO\nthree\nfour\n").unwrap();
    git(&d, &["commit", "-qam", "by hand"]);
    let b = k.call(Caller::user(), "history.blame", json!({ "target": "01a1bc12ab34", "repo": d.display().to_string() })).await.unwrap();
    assert_eq!((b["tasks"][0]["what"]["added"].clone(), b["tasks"][0]["what"]["stillThere"].clone()), (json!(4), json!(0.75)), "{b}");
    std::fs::remove_file(d.join("code.txt")).unwrap();
    git(&d, &["commit", "-qam", "all gone"]);
    let b = k.call(Caller::user(), "history.blame", json!({ "target": "01a1bc12ab34", "repo": d.display().to_string() })).await.unwrap();
    assert_eq!(b["tasks"][0]["what"]["stillThere"], json!(0.0), "{b}");
    // a task with no commits shows nothing, not 0%
    let e = dir("still-none", "", true);
    let k = kernel(&e, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    k.call(Caller::user(), "history.track", track()).await.unwrap();
    let b = k.call(Caller::user(), "history.blame", json!({ "target": "01a1bc12ab34", "repo": e.display().to_string() })).await.unwrap();
    assert_eq!(b["tasks"][0]["what"]["stillThere"], Value::Null, "{b}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_can_ask_but_not_answer() {
    let d = dir("agent", "", true);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    let agent = Caller { set: "agent:session".into(), session: Some("01a1bc12ab34cdef0123456789abcdef".into()), plugin: None };
    let r = k.call(agent.clone(), "history.resumeRequest", json!({ "id": "t1", "why": "the login broke again" })).await.unwrap();
    assert_eq!(r["state"], "pending");
    let e = k.call(agent, "history.resumeAnswer", json!({ "request": r["id"], "approve": true })).await.unwrap_err();
    assert_eq!(e.code, wisp_kernel::Code::Forbidden, "{e:?}");
}

/// The TUI's why-gutter: a file's blame with `byLine` lists every line with the task that wrote it (its title and
/// prompt), and a line written by hand with none.
#[tokio::test(flavor = "multi_thread")]
async fn blame_by_line_names_each_lines_task() {
    let d = dir("byline", "", true);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    std::fs::write(d.join("code.txt"), "one\ntwo\nthree\n").unwrap();
    git(&d, &["add", "code.txt"]);
    git(&d, &["commit", "-qm", "the task's code\n\nWisp-Task: 01a1bc12ab34"]);
    k.call(Caller::user(), "history.track", track()).await.unwrap();
    std::fs::write(d.join("code.txt"), "one\nTWO\nthree\n").unwrap();
    git(&d, &["commit", "-qam", "by hand"]);
    let b = k
        .call(Caller::user(), "history.blame", json!({ "target": "code.txt", "repo": d.display().to_string(), "byLine": true }))
        .await
        .unwrap();
    let lines = b["byLine"].as_array().unwrap();
    assert_eq!(lines.len(), 3, "{b}");
    assert_eq!((lines[0]["n"].clone(), lines[0]["text"].clone()), (json!(1), json!("one")));
    assert!(lines[0]["task"].as_str().unwrap().starts_with("01a1bc12ab34"), "{b}");
    assert!(lines[0]["title"].is_string(), "the task's title for the gutter: {b}");
    assert_eq!((lines[1]["text"].clone(), lines[1]["task"].clone()), (json!("TWO"), Value::Null), "by hand: no task");
    let plain = k.call(Caller::user(), "history.blame", json!({ "target": "code.txt", "repo": d.display().to_string() })).await.unwrap();
    assert!(plain.get("byLine").is_none(), "only when asked");
}

/// A task with no folder has no repo: track and untrack without `repo` refuse it, and never fall back to the
/// server's own folder (`Path::new("")` would read and remove the server cwd's `.wisp/`).
#[tokio::test(flavor = "multi_thread")]
async fn a_task_with_no_folder_is_never_tracked_in_the_servers_folder() {
    let k = kernel(Path::new(""), &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    for cmd in ["history.track", "history.untrack"] {
        let e = k.call(Caller::user(), cmd, track()).await.unwrap_err();
        assert_eq!(e.code, wisp_kernel::Code::Invalid, "{cmd}: {e:?}");
        assert!(e.message.contains("no folder"), "{cmd}: {e:?}");
    }
}

/// A `repo` that's empty or relative names the server's own folder (`Path::new("")` reads, writes and removes its
/// `.wisp/`): every command that takes one refuses it.
#[tokio::test(flavor = "multi_thread")]
async fn a_named_repo_that_is_empty_or_relative_is_refused_by_every_command() {
    let d = dir("named", "", true);
    let k = kernel(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    for repo in ["", " ", ".", "relative/repo"] {
        let calls = [
            ("history.track", json!({ "id": "t1", "repo": repo })),
            ("history.save", json!({ "id": "t1", "repo": repo })),
            ("history.untrack", json!({ "id": "t1", "repo": repo })),
            ("history.list", json!({ "repo": repo })),
            ("history.report", json!({ "id": "t1", "repo": repo })),
            ("history.blame", json!({ "target": "code.txt", "repo": repo })),
            ("history.resumeRequest", json!({ "id": "t1", "repo": repo })),
        ];
        for (cmd, args) in calls {
            let e = k.call(Caller::user(), cmd, args).await.unwrap_err();
            assert_eq!(e.code, wisp_kernel::Code::Invalid, "{cmd} {repo:?}: {e:?}");
            assert!(e.message.contains("repo"), "{cmd} {repo:?}: {e:?}");
        }
    }
}

/// The change a save's own write makes runs the done check again: the save counted the turns as seen, so no offer
/// comes back over `tracked` (`serve_history.rs`'s `offers_the_mark_resume_requests_and_injection` failed on CI this way).
#[tokio::test(flavor = "multi_thread")]
async fn a_saved_task_is_not_offered_again() {
    let d = dir("saved", "history: { track: ask }", true);
    let slot = Arc::new(Mutex::new(Slot::default()));
    let k = kernel_with(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default(), slot.clone()).await;
    let ctx = k.ctx("wisp.history").unwrap();
    super::on_done(&ctx, "t1").await;
    assert_eq!(slot.lock().unwrap().value["offer"], true, "offered first: {}", slot.lock().unwrap().value);
    k.call(Caller::user(), "history.save", track()).await.unwrap();
    super::on_done(&ctx, "t1").await;
    let v = slot.lock().unwrap().value.clone();
    assert_eq!((v["offer"].clone(), v["tracked"]["store"].clone()), (json!(false), json!("files")), "{v}");
}

/// A save that lands between the done check's read of the slot and its write: the offer, computed from the copy
/// before the save, never lands over it (it reads again, sees the turns saved, and writes nothing).
#[tokio::test(flavor = "multi_thread")]
async fn a_save_between_the_offers_read_and_write_wins() {
    let d = dir("race", "history: { track: ask }", true);
    let saved = json!({ "tracked": { "repo": d.display().to_string(), "files": [".wisp/x.task.yml"], "store": "files" }, "offer": false, "seenTurns": 0 });
    let slot = Arc::new(Mutex::new(Slot { value: Value::Null, before_offer: Some(saved.clone()), ..Slot::default() }));
    let k = kernel_with(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default(), slot.clone()).await;
    super::on_done(&k.ctx("wisp.history").unwrap(), "t1").await;
    let s = slot.lock().unwrap();
    assert!(s.before_offer.is_none(), "the save landed mid-offer");
    assert_eq!(s.value, saved, "the save wins");
}

/// perf audit r2 #2: blame reads every commit's message from one `git log`; its records split into each one's.
#[test]
fn one_logs_records_are_each_commits_message() {
    let text = "abc\0first\n\nWisp-Task: t1\n\u{1e}\ndef\0second\n\u{1e}\n";
    let got = crate::git::messages_of(text);
    assert_eq!(got, vec![("abc".to_owned(), "first\n\nWisp-Task: t1\n".to_owned()), ("def".to_owned(), "second\n".to_owned())]);
}

/// Regression: a deleted worktree under `$HOME/.wisp/worktrees/...` must NEVER walk up into `$HOME/.wisp`
/// (wisp's own app data dir); it falls back to `ext["wisp.worktrees"].root` (the main repo checkout).
#[tokio::test(flavor = "multi_thread")]
async fn deleted_worktree_saves_to_main_repo_not_app_home() {
    let fake_home = std::env::temp_dir().join(format!("wisp-history-home-{}", wisp_kernel::ids::uuid7()));
    let app_wisp = fake_home.join(".wisp");
    std::fs::create_dir_all(app_wisp.join("worktrees")).unwrap();
    std::fs::write(app_wisp.join("wisp.db"), b"").unwrap();
    let repo = fake_home.join("my-repo");
    std::fs::create_dir_all(repo.join(".wisp")).unwrap();
    std::fs::write(repo.join(".wisp/wisp.yml"), "wisp: 1\nhistory: { track: ask }\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);

    let deleted_wt = app_wisp.join("worktrees/wisp-gone-task");
    let mut task = wisp_runtime_api::model::Session {
        id: "01a1bc12ab34cdef0123456789abcdef".into(),
        cwd: deleted_wt.display().to_string(),
        ..Default::default()
    };
    task.ext.insert(
        "wisp.worktrees".into(),
        json!({ "path": deleted_wt.display().to_string(), "root": repo.display().to_string(), "branch": "wisp/x", "baseRef": "main" }),
    );
    let k = kernel(&repo, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default()).await;
    let ctx = k.ctx("wisp.history").unwrap();
    let p = wisp_proc::local();
    let resolved = super::config::root_of_task(&ctx, &p, &task).await.unwrap();
    assert_eq!(resolved, repo, "must resolve to main repo, never fake_home");
    // even without worktree ext, root_of must never match $HOME/.wisp (the app data dir)
    let plain = fake_home.join("plain-repo");
    std::fs::create_dir_all(&plain).unwrap();
    git(&plain, &["init", "-q", "-b", "main"]);
    let plain_root = super::config::root_of(&p, Some(&app_wisp), &plain.display().to_string()).await;
    assert_ne!(plain_root, fake_home, "must never walk up into $HOME/.wisp");
}

/// A main checkout with a bare `origin` holding its `main`, and a save's two files in `.wisp/` (not committed).
fn saved_checkout(name: &str) -> (PathBuf, PathBuf, Vec<String>) {
    let d = dir(name, "", true);
    // the push's own git (commit-tree) needs a name, as a user's machine has one
    git(&d, &["config", "user.name", "t"]);
    git(&d, &["config", "user.email", "t@t"]);
    let origin = std::env::temp_dir().join(format!("wisp-history-{name}-origin-{}.git", wisp_kernel::ids::uuid7()));
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "-q", "--bare", "-b", "main"]);
    git(&d, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&d, &["push", "-q", "origin", "main"]);
    let files = vec![".wisp/tasks/01a1bc12ab34-fix.task.yml".to_owned(), ".wisp/tasks/01a1bc12ab34-fix.report.md".to_owned()];
    std::fs::create_dir_all(d.join(".wisp/tasks")).unwrap();
    for f in &files {
        std::fs::write(d.join(f), format!("{f}\n")).unwrap();
    }
    (d, origin, files)
}

const PUSHED_ID: &str = "01a1bc12ab34cdef0123456789abcdef";

/// final audit F4: with no PR (gh missing or failing) the save's files were taken out all the same, and nothing
/// would bring them back: they stay, the branch is on the remote, and the answer says no PR opened.
#[tokio::test(flavor = "multi_thread")]
async fn a_push_with_no_pr_keeps_the_saved_files() {
    let (d, origin, files) = saved_checkout("push-nopr");
    let p = wisp_proc::local();
    let gh = d.join("no-such-gh").display().to_string();
    let out = super::push::as_pr(&p, &gh, &d, "origin", (PUSHED_ID, "Fix"), &files, None).await.unwrap();
    assert_eq!((out.branch.as_str(), out.pr.as_deref()), ("wisp/history-01a1bc12ab34", None), "{out:?}");
    assert!(out.removed.is_empty(), "{out:?}");
    assert!(files.iter().all(|f| d.join(f).is_file()), "{:?}", files_under(&d));
    assert_eq!(out.compare, None, "a path remote has no page to open a PR on");
    assert!(out.no_pr, "the line says no PR opened, even with no page to open one on: {out:?}");
    let on_origin = git(&origin, &["ls-tree", "-r", "--name-only", "wisp/history-01a1bc12ab34"]);
    assert!(files.iter().all(|f| on_origin.lines().any(|l| l == f)), "{on_origin}");
}

/// final audit F4 and F3: with a PR the files are taken out (the pull brings them back), and pushing again answers
/// what that push did instead of "Save to repo again".
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_push_with_a_pr_takes_the_files_out_and_a_second_push_answers_it() {
    use std::os::unix::fs::PermissionsExt;
    let (d, _origin, files) = saved_checkout("push-pr");
    let gh = d.join("gh-stub");
    std::fs::write(&gh, "#!/bin/sh\necho https://github.com/acme/app/pull/1\n").unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let gh = gh.display().to_string();
    let p = wisp_proc::local();
    let first = super::push::as_pr(&p, &gh, &d, "origin", (PUSHED_ID, "Fix"), &files, None).await.unwrap();
    assert_eq!(first.pr.as_deref(), Some("https://github.com/acme/app/pull/1"), "{first:?}");
    assert_eq!(first.removed, files);
    assert!(files.iter().all(|f| !d.join(f).exists()), "{:?}", files_under(&d));
    // what push() hands it from the task's slot, as the first push marked it
    let slot = json!({ "pushed": { "branch": first.branch, "commit": first.commit, "pr": first.pr, "compare": null } });
    let earlier = super::push::earlier(&slot, &files);
    let again = super::push::as_pr(&p, &gh, &d, "origin", (PUSHED_ID, "Fix"), &files, earlier).await.unwrap();
    assert_eq!((again.branch, again.commit, again.pr), (first.branch, first.commit, first.pr));
    // never pushed: the files really are gone
    let e = super::push::as_pr(&p, &gh, &d, "origin", (PUSHED_ID, "Fix"), &files, None).await.unwrap_err();
    assert!(e.message.contains("Save to repo again"), "{e:?}");
}

/// final audit F2/F3c: a save after a push drops that push from the slot, so neither a reload's line nor a later push
/// answers the old branch and PR for the newer save: once its files are gone, push says to save again.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_save_after_a_push_forgets_that_push() {
    use std::os::unix::fs::PermissionsExt;
    let (d, _origin, files) = saved_checkout("push-resave");
    let gh = d.join("gh-stub");
    std::fs::write(&gh, "#!/bin/sh\necho https://github.com/acme/app/pull/1\n").unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let gh = gh.display().to_string();
    let p = wisp_proc::local();
    let first = super::push::as_pr(&p, &gh, &d, "origin", (PUSHED_ID, "Fix"), &files, None).await.unwrap();
    let pushed = json!({ "tracked": { "repo": d.display().to_string(), "files": files, "store": "files" }, "offer": false,
        "pushed": { "branch": first.branch, "commit": first.commit, "pr": first.pr } });
    // Save to repo again: the files come back, and the slot is marked as track() marks it
    for f in &files {
        std::fs::write(d.join(f), format!("{f} again\n")).unwrap();
    }
    let mark = json!({ "tracked": { "repo": d.display().to_string(), "files": files, "store": "files" }, "offer": false, "seenTurns": 2 });
    let slot = super::saved_slot(&pushed, &mark);
    assert_eq!(slot.get("pushed"), None, "{slot}");
    assert_eq!(slot.pointer("/tracked/files").and_then(Value::as_array).map(Vec::len), Some(2));
    let earlier = super::push::earlier(&slot, &files);
    assert_eq!(earlier, None);
    // the newer save's files taken by hand (or an untrack): not the old push's answer, but save again
    for f in &files {
        std::fs::remove_file(d.join(f)).unwrap();
    }
    let e = super::push::as_pr(&p, &gh, &d, "origin", (PUSHED_ID, "Fix"), &files, earlier).await.unwrap_err();
    assert!(e.message.contains("Save to repo again"), "{e:?}");
}

/// final5 F3: Ship's catch-up knows a history push's commit by its subject and the folders a save writes: both come
/// from the one place (wisp-history-api) the push builds them from.
#[test]
fn a_history_push_commit_is_what_ship_merges_in() {
    let msg = super::push::message("Fix it", PUSHED_ID);
    let subject = msg.lines().next().unwrap();
    assert_eq!(subject, "wisp: save task \"Fix it\" to the repo's history");
    assert!(subject.starts_with(wisp_history_api::SAVE_SUBJECT_START) && subject.ends_with(wisp_history_api::SAVE_SUBJECT_END));
    let (report, history) = (Some(json!({ "answers": [] })), Some(json!([{ "turn": { "prompt": "x" } }])));
    let b = wisp_share_api::Bundle { id: PUSHED_ID.into(), title: "Fix it".into(), report, history, ..Default::default() };
    for (rel, _) in wisp_history_core::repo::files_for(&b, &[], None, "me") {
        let top = rel.split('/').next().unwrap();
        assert!(wisp_history_api::SAVED_DIRS.contains(&top), "{rel}");
    }
}

#[test]
fn the_new_pr_page_is_in_the_hosts_own_format() {
    let page = super::push::new_pr_page;
    assert_eq!(
        page("https://github.com/acme/app", "main", "wisp/history-x").as_deref(),
        Some("https://github.com/acme/app/compare/main...wisp/history-x?expand=1")
    );
    assert_eq!(
        page("https://gitlab.com/acme/app", "main", "wisp/history-x").as_deref(),
        Some(
            "https://gitlab.com/acme/app/-/merge_requests/new?merge_request%5Bsource_branch%5D=wisp/history-x&merge_request%5Btarget_branch%5D=main"
        )
    );
    assert_eq!(
        page("https://bitbucket.org/acme/app", "main", "wisp/history-x").as_deref(),
        Some("https://bitbucket.org/acme/app/pull-requests/new?source=wisp/history-x&dest=main")
    );
    assert_eq!(page("https://git.example.com/acme/app", "main", "wisp/history-x"), None, "an unknown host gets no wrong link");
}

#[test]
fn a_remotes_web_page_never_carries_a_user() {
    let web = super::push::web_of;
    assert_eq!(web("git@github.com:acme/app.git").as_deref(), Some("https://github.com/acme/app"));
    assert_eq!(web("https://x-access-token:abc@github.com/acme/app.git").as_deref(), Some("https://github.com/acme/app"));
    assert_eq!(web("ssh://git@github.com:22/acme/app").as_deref(), Some("https://github.com/acme/app"));
    assert_eq!(web("/tmp/origin.git"), None);
    assert_eq!(web(r"C:\repos\origin.git"), None);
    assert_eq!(web("C:/repos/origin.git"), None);
    assert_eq!(super::push::earlier(&json!({}), &[]), None);
    let on_base = super::push::earlier(&json!({ "pushed": { "branch": "", "commit": "c", "pr": null } }), &[]).unwrap();
    assert_eq!((on_base.branch.as_str(), on_base.pr), ("", None), "already on the base: the second push answers that too");
}

const TASK_BRANCH: &str = "wisp/task-fix";

/// A main checkout with a bare `origin` holding its `main` (`.wisp/` gitignored when `ignored`), the task's worktree
/// on its own branch off it, and a save's two files in the worktree's `.wisp/`: (checkout, origin, worktree, files).
fn saved_worktree(name: &str, ignored: bool) -> (PathBuf, PathBuf, PathBuf, Vec<String>) {
    let d = dir(name, "", true);
    git(&d, &["config", "user.name", "t"]);
    git(&d, &["config", "user.email", "t@t"]);
    if ignored {
        std::fs::write(d.join(".gitignore"), ".wisp/\n").unwrap();
        git(&d, &["add", ".gitignore"]);
        git(&d, &["commit", "-qm", "ignore .wisp"]);
    }
    let origin = std::env::temp_dir().join(format!("wisp-history-{name}-origin-{}.git", wisp_kernel::ids::uuid7()));
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "-q", "--bare", "-b", "main"]);
    git(&d, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&d, &["push", "-q", "origin", "main"]);
    let wt = std::env::temp_dir().join(format!("wisp-history-{name}-wt-{}", wisp_kernel::ids::uuid7()));
    git(&d, &["worktree", "add", "-q", "-b", TASK_BRANCH, wt.to_str().unwrap(), "main"]);
    let files = vec![".wisp/tasks/01a1bc12ab34-fix.task.yml".to_owned(), ".wisp/tasks/01a1bc12ab34-fix.report.md".to_owned()];
    std::fs::create_dir_all(wt.join(".wisp/tasks")).unwrap();
    for f in &files {
        std::fs::write(wt.join(f), format!("{f}\n")).unwrap();
    }
    (d, origin, wt, files)
}

fn no_gh(d: &Path) -> String {
    d.join("no-such-gh").display().to_string()
}

/// final3 doc5a: Push to repo from a live worktree builds its commit on the branch's remote tip. The agent's commit
/// that was never pushed stays unpushed, and the worktree's HEAD and index are as they were.
#[tokio::test(flavor = "multi_thread")]
async fn a_worktree_push_never_ships_the_agents_unpushed_commits() {
    let (d, origin, wt, files) = saved_worktree("push-wt", false);
    git(&wt, &["push", "-q", "origin", TASK_BRANCH]);
    let shipped = git(&origin, &["rev-parse", TASK_BRANCH]).trim().to_owned();
    std::fs::write(wt.join("math.js"), "a - b\n").unwrap();
    git(&wt, &["commit", "-qam", "not shipped yet"]);
    let local = git(&wt, &["rev-parse", "HEAD"]).trim().to_owned();
    std::fs::write(wt.join("staged.txt"), "staged\n").unwrap();
    git(&wt, &["add", "staged.txt"]);
    let cached = git(&wt, &["diff", "--cached"]);
    let p = wisp_proc::local();
    let out = super::push::onto_branch(&p, &no_gh(&d), &wt, "origin", TASK_BRANCH, (PUSHED_ID, "Fix"), &files, None).await.unwrap();
    let tip = git(&origin, &["rev-parse", TASK_BRANCH]).trim().to_owned();
    assert_eq!((out.branch.as_str(), out.commit.as_str()), (TASK_BRANCH, tip.as_str()), "{out:?}");
    assert_eq!(git(&origin, &["rev-parse", &format!("{TASK_BRANCH}^1")]).trim(), shipped);
    assert!(!git(&origin, &["rev-list", TASK_BRANCH]).lines().any(|l| l == local), "the unpushed commit went out");
    let tree = git(&origin, &["ls-tree", "-r", "--name-only", TASK_BRANCH]);
    assert!(files.iter().all(|f| tree.lines().any(|l| l == f)), "{tree}");
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]).trim(), local);
    // final4 F1: past the tip, the branch isn't moved (Ship merges the history in before its push)
    assert_eq!(git(&wt, &["rev-parse", &format!("refs/heads/{TASK_BRANCH}")]).trim(), local);
    assert_eq!(git(&wt, &["diff", "--cached"]), cached);
    assert!(files.iter().all(|f| wt.join(f).is_file()));
    // pushed again as saved: nothing new, the tip answered
    let again = super::push::onto_branch(&p, &no_gh(&d), &wt, "origin", TASK_BRANCH, (PUSHED_ID, "Fix"), &files, None).await.unwrap();
    assert_eq!(again.commit, tip);
    assert_eq!(git(&origin, &["rev-parse", TASK_BRANCH]).trim(), tip);
}

/// final4 F1: a worktree whose branch sat at the remote tip follows the history commit it pushed (its index too), so
/// `git status` there is clean and the next push from it, never forced, is a fast-forward.
#[tokio::test(flavor = "multi_thread")]
async fn a_worktree_push_moves_its_branch_along_so_the_next_push_is_a_fast_forward() {
    let (d, origin, wt, files) = saved_worktree("push-wt-follow", false);
    git(&wt, &["push", "-q", "origin", TASK_BRANCH]);
    let p = wisp_proc::local();
    let out = super::push::onto_branch(&p, &no_gh(&d), &wt, "origin", TASK_BRANCH, (PUSHED_ID, "Fix"), &files, None).await.unwrap();
    assert_eq!(out.commit, git(&origin, &["rev-parse", TASK_BRANCH]).trim());
    assert_eq!(git(&wt, &["rev-parse", &format!("refs/heads/{TASK_BRANCH}")]).trim(), out.commit);
    assert_eq!(git(&wt, &["status", "--porcelain"]), "", "no staged deletions, nothing new");
    assert!(files.iter().all(|f| wt.join(f).is_file()));
    std::fs::write(wt.join("math.js"), "a * b\n").unwrap();
    git(&wt, &["commit", "-qam", "more work"]);
    git(&wt, &["push", "-q", "origin", &format!("HEAD:refs/heads/{TASK_BRANCH}")]);
    assert_eq!(git(&origin, &["rev-parse", TASK_BRANCH]), git(&wt, &["rev-parse", "HEAD"]));
}

/// A stand-in gh answering `pr list` for the task's branch with a PR in `state`, and `pr create` with PR 8.
#[cfg(unix)]
fn gh_with_pr(d: &Path, state: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let gh = d.join("gh-state");
    let list = format!(
        r#"[{{"state":"{state}","url":"https://github.com/acme/app/pull/7","headRefName":"{TASK_BRANCH}","headRefOid":"x","number":7}}]"#
    );
    let script = format!(
        "#!/bin/sh\ncase \"$*\" in\n  *'pr list'*) echo '{list}' ;;\n  *'pr create'*) echo https://github.com/acme/app/pull/8 ;;\nesac\n"
    );
    std::fs::write(&gh, script).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    gh.display().to_string()
}

/// final4 F2: the task branch's PR merged, so nothing on that branch gets merged now: the history goes out as its own
/// PR on the base, and the task branch on the remote is left as it was.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_worktree_push_after_its_pr_merged_opens_a_history_pr_on_the_base() {
    let (d, origin, wt, files) = saved_worktree("push-wt-merged", false);
    git(&wt, &["push", "-q", "origin", TASK_BRANCH]);
    let shipped = git(&origin, &["rev-parse", TASK_BRANCH]);
    let p = wisp_proc::local();
    let out = super::push::onto_branch(&p, &gh_with_pr(&d, "MERGED"), &wt, "origin", TASK_BRANCH, (PUSHED_ID, "Fix"), &files, None)
        .await
        .unwrap();
    assert_eq!(
        (out.branch.as_str(), out.pr.as_deref()),
        ("wisp/history-01a1bc12ab34", Some("https://github.com/acme/app/pull/8")),
        "{out:?}"
    );
    assert_eq!(git(&origin, &["rev-parse", TASK_BRANCH]), shipped, "the dead branch is left as it was");
    assert_eq!(git(&origin, &["rev-parse", "wisp/history-01a1bc12ab34^1"]), git(&origin, &["rev-parse", "main"]), "built on the base");
    let tree = git(&origin, &["ls-tree", "-r", "--name-only", "wisp/history-01a1bc12ab34"]);
    assert!(files.iter().all(|f| tree.lines().any(|l| l == f)), "{tree}");
}

/// final4 F2: with its PR still open, the history goes onto the task's branch as before, and its PR is named.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_worktree_push_with_its_pr_open_goes_onto_its_branch() {
    let (d, origin, wt, files) = saved_worktree("push-wt-open", false);
    git(&wt, &["push", "-q", "origin", TASK_BRANCH]);
    let p = wisp_proc::local();
    let out =
        super::push::onto_branch(&p, &gh_with_pr(&d, "OPEN"), &wt, "origin", TASK_BRANCH, (PUSHED_ID, "Fix"), &files, None).await.unwrap();
    assert_eq!((out.branch.as_str(), out.pr.as_deref()), (TASK_BRANCH, Some("https://github.com/acme/app/pull/7")), "{out:?}");
    assert_eq!(out.commit, git(&origin, &["rev-parse", TASK_BRANCH]).trim());
    assert!(!git(&origin, &["for-each-ref", "--format=%(refname)", "refs/heads"]).contains("wisp/history-"));
}

/// final3 doc5a: nothing shipped yet, so no branch goes up under the task's name: the history goes out as its own PR.
#[tokio::test(flavor = "multi_thread")]
async fn a_worktree_push_before_the_branch_is_shipped_goes_out_as_a_pr() {
    let (d, origin, wt, files) = saved_worktree("push-wt-new", false);
    let p = wisp_proc::local();
    let out = super::push::onto_branch(&p, &no_gh(&d), &wt, "origin", TASK_BRANCH, (PUSHED_ID, "Fix"), &files, None).await.unwrap();
    assert_eq!(out.branch, "wisp/history-01a1bc12ab34", "{out:?}");
    let heads = git(&origin, &["for-each-ref", "--format=%(refname)", "refs/heads"]);
    assert!(!heads.lines().any(|l| l == format!("refs/heads/{TASK_BRANCH}")), "{heads}");
    assert!(heads.lines().any(|l| l == "refs/heads/wisp/history-01a1bc12ab34"), "{heads}");
}

/// final3 doc5a: every saved file gone: Save to repo again, and nothing on the remote moves (it used to push HEAD).
#[tokio::test(flavor = "multi_thread")]
async fn a_worktree_push_with_its_files_gone_pushes_nothing() {
    let (d, origin, wt, files) = saved_worktree("push-wt-gone", false);
    git(&wt, &["push", "-q", "origin", TASK_BRANCH]);
    std::fs::write(wt.join("math.js"), "a - b\n").unwrap();
    git(&wt, &["commit", "-qam", "not shipped yet"]);
    for f in &files {
        std::fs::remove_file(wt.join(f)).unwrap();
    }
    let refs = git(&origin, &["for-each-ref"]);
    let p = wisp_proc::local();
    let e = super::push::onto_branch(&p, &no_gh(&d), &wt, "origin", TASK_BRANCH, (PUSHED_ID, "Fix"), &files, None).await.unwrap_err();
    assert!(e.message.contains("Save to repo again"), "{e:?}");
    assert_eq!(git(&origin, &["for-each-ref"]), refs);
}

/// final3: a repo that gitignores `.wisp/` still pushes from its worktree (`git add` refused the ignored paths).
#[tokio::test(flavor = "multi_thread")]
async fn a_worktree_push_works_when_wisp_is_gitignored() {
    let (d, origin, wt, files) = saved_worktree("push-wt-ignored", true);
    git(&wt, &["push", "-q", "origin", TASK_BRANCH]);
    let p = wisp_proc::local();
    let out = super::push::onto_branch(&p, &no_gh(&d), &wt, "origin", TASK_BRANCH, (PUSHED_ID, "Fix"), &files, None).await.unwrap();
    assert_eq!(out.branch, TASK_BRANCH);
    let tree = git(&origin, &["ls-tree", "-r", "--name-only", TASK_BRANCH]);
    assert!(files.iter().all(|f| tree.lines().any(|l| l == f)), "{tree}");
}

/// final3 doc5b: the push goes to the task's own remote, not a hardcoded `origin`.
#[tokio::test(flavor = "multi_thread")]
async fn a_push_goes_to_the_tasks_own_remote() {
    let d = dir("push-upstream", "", true);
    git(&d, &["config", "user.name", "t"]);
    git(&d, &["config", "user.email", "t@t"]);
    let upstream = std::env::temp_dir().join(format!("wisp-history-push-upstream-bare-{}.git", wisp_kernel::ids::uuid7()));
    std::fs::create_dir_all(&upstream).unwrap();
    git(&upstream, &["init", "-q", "--bare", "-b", "main"]);
    git(&d, &["remote", "add", "upstream", upstream.to_str().unwrap()]);
    git(&d, &["push", "-q", "upstream", "main"]);
    let files = vec![".wisp/tasks/01a1bc12ab34-fix.task.yml".to_owned()];
    std::fs::create_dir_all(d.join(".wisp/tasks")).unwrap();
    std::fs::write(d.join(&files[0]), "saved\n").unwrap();
    let slot = Arc::new(Mutex::new(Slot {
        value: json!({ "tracked": { "repo": d.display().to_string(), "files": files, "store": "files" } }),
        worktrees: json!({ "remote": "upstream" }),
        ..Slot::default()
    }));
    let k = kernel_with(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default(), slot).await;
    let out = k.call(Caller::user(), "history.push", json!({ "id": "t1" })).await.unwrap();
    assert_eq!(out["branch"], "wisp/history-01a1bc12ab34", "{out}");
    let tree = git(&upstream, &["ls-tree", "-r", "--name-only", "wisp/history-01a1bc12ab34"]);
    assert!(tree.lines().any(|l| l == files[0]), "{tree}");
}

#[test]
fn the_remote_falls_back_to_origin() {
    let of = super::push::remote_of;
    assert_eq!(of(&Value::Null), "origin");
    assert_eq!(of(&json!({ "path": "/wt" })), "origin");
    assert_eq!(of(&json!({ "remote": "" })), "origin");
    assert_eq!(of(&json!({ "remote": " " })), "origin");
    assert_eq!(of(&json!({ "remote": "upstream" })), "upstream");
}

/// final3: the worktree it saved into, cleaned up since: say so and to save again, never "no remote to push to".
#[tokio::test(flavor = "multi_thread")]
async fn a_push_after_its_worktree_is_gone_says_to_save_again() {
    let d = dir("push-stale", "", true);
    let gone = std::env::temp_dir().join(format!("wisp-history-push-stale-wt-{}", wisp_kernel::ids::uuid7()));
    let saved = json!({ "tracked": { "repo": gone.display().to_string(), "files": [".wisp/tasks/x.task.yml"], "store": "files" } });
    let slot = Arc::new(Mutex::new(Slot { value: saved.clone(), ..Slot::default() }));
    let k = kernel_with(&d, &Arc::new(Mutex::new("Fix".into())), true, &Kept::default(), slot.clone()).await;
    let e = k.call(Caller::user(), "history.push", json!({ "id": "t1" })).await.unwrap_err();
    assert_eq!(e.code, wisp_kernel::Code::Invalid, "{e:?}");
    assert!(e.message.contains("is gone") && e.message.contains("Save to repo again"), "{e:?}");
    assert!(!e.message.contains("no remote"), "{e:?}");
    assert_eq!(slot.lock().unwrap().value, saved, "no pushed mark");
}
