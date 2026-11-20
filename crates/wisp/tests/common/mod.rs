//! What the serve e2e tests share: `wisp serve` on a temp home with wisp-testkit's fake harnesses on
//! the env, and a client for `POST /api/cmd`.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The server, killed and its home removed when the test ends (passing or not).
pub struct Serve {
    pub child: Child,
    pub dir: PathBuf,
    pub keep: bool,
}

impl Serve {
    /// Stops the server and keeps its home, for a restart on the same `WISP_HOME`.
    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(self.dir.join("home/server.json"));
        self.keep = true;
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

/// The fake CLIs the server is pointed at, by file name and override variable.
pub const FAKES: [(&str, &str); 7] = [
    ("claude", "WISP_CLAUDE_BIN"),
    ("codex", "WISP_CODEX_BIN"),
    ("opencode", "WISP_OPENCODE_BIN"),
    ("cursor-agent", "WISP_CURSOR_BIN"),
    ("copilot", "WISP_COPILOT_BIN"),
    ("grok", "WISP_GROK_BIN"),
    ("agy", "WISP_ANTIGRAVITY_BIN"),
];

/// wisp-testkit's fakes (`wisp-fake-agent`, `wisp-fake-gh`), built from the artifact dir of this
/// crate's `wisp` bin (`<target>/<profile>/`), in every cargo layout. Not the test's own folder:
/// cargo's build-dir layout puts test executables under `build/<pkg>/<hash>/`, so
/// `current_exe()` points nowhere useful. Cargo doesn't build another crate's bins for these
/// tests, so each process builds both once into the same target dir and profile, then serves its
/// own copies from `<target>/<profile>/wisp-fakes/` (see [`fake_bins_into`]).
pub fn fake_bins() -> (PathBuf, PathBuf) {
    static BUILT: std::sync::OnceLock<(PathBuf, PathBuf)> = std::sync::OnceLock::new();
    BUILT.get_or_init(|| fake_bins_into(&bin_dir().join("wisp-fakes"))).clone()
}

pub fn bin_dir() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_wisp")).parent().unwrap()
}

/// Builds the fakes and puts them in `out`, safe when several test processes do it at once (monoflow
/// runs packages in parallel, agents run copies): the build and the copy run under an exclusive lock
/// on `out/.lock`, and each copy is written to a temp file and renamed into place, so a fake a server
/// is running (or a symlink to it) is never missing or half-written. The copies also keep a later
/// `cargo test -p wisp-testkit`, which re-links `<target>/<profile>/wisp-fake-agent`, from pulling a
/// binary out from under a running test.
///
/// A copy already in place with the build's bytes is left alone: another process may be running it,
/// and Windows refuses to rename over a running executable (os error 5, "Access is denied"). A copy of
/// another build that is running is moved aside first (Windows lets a running executable be renamed,
/// not replaced) and removed once nothing runs it, by this placement or a later one.
pub fn fake_bins_into(out: &Path) -> (PathBuf, PathBuf) {
    place_fakes(out, true)
}

/// The lock every process takes to build the fakes into `<target>/<profile>/` or copy them out of there: a no-op
/// `cargo build` still puts `wisp-fake-agent` back in that folder (remove, then link), so a copy another process
/// made meanwhile found no file (os error 2, 5 of 50 runs of `fake_build.rs`, 8 at once). `out/.lock` alone guards one
/// folder's placement, and test processes place into different folders.
fn built_lock() -> std::fs::File {
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(bin_dir().join(".wisp-fakes.lock")).unwrap();
    lock.lock().unwrap();
    lock
}

/// `cargo build` of the fakes into the target dir, no placement (a race's parent builds once, its children place).
pub fn build_fakes() {
    let _built = built_lock();
    build_unlocked();
}

fn build_unlocked() {
    let dir = bin_dir();
    let target = dir.parent().unwrap();
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.args(["build", "--quiet", "-p", "wisp-testkit", "--bin", "wisp-fake-agent", "--bin", "wisp-fake-gh", "--target-dir"]).arg(target);
    if dir.ends_with("release") {
        cmd.arg("--release");
    }
    assert!(cmd.status().unwrap().success(), "building wisp-testkit's fakes into {}", target.display());
}

/// [`fake_bins_into`] without the build, for a process whose parent built them: the placement alone, the part that
/// races (`fake_build.rs`). A cargo no-op build is seconds on Windows, and the build runs under the lock anyway.
pub fn place_built_fakes(out: &Path) -> (PathBuf, PathBuf) {
    place_fakes(out, false)
}

fn place_fakes(out: &Path, build: bool) -> (PathBuf, PathBuf) {
    let dir = bin_dir();
    let exe = |d: &Path, n: &str| d.join(format!("{n}{}", std::env::consts::EXE_SUFFIX));
    std::fs::create_dir_all(out).unwrap();
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(out.join(".lock")).unwrap();
    lock.lock().unwrap();
    // under the lock no placement is under way: what's left is a crashed one's temp file or a copy an
    // earlier one moved aside, which goes once nothing runs it (on Windows a running one stays)
    for e in std::fs::read_dir(out).unwrap().flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') && (name.ends_with(".old") || name.ends_with(".tmp")) {
            let _ = std::fs::remove_file(e.path());
        }
    }
    // out's lock, then the target dir's (always in this order)
    let built = built_lock();
    if build {
        build_unlocked();
    }
    let place = |n: &str| {
        let (src, dst) = (exe(dir, n), exe(out, n));
        if wisp_testkit::same_bytes(&src, &dst) {
            return dst;
        }
        let tmp = out.join(format!(".{n}.{}.tmp", std::process::id()));
        std::fs::copy(&src, &tmp).unwrap_or_else(|e| panic!("copying {} to {}: {e}", src.display(), tmp.display()));
        if let Err(e) = std::fs::rename(&tmp, &dst) {
            // another build's copy is running (Windows): move it aside, then the rename has a free name
            let old = out.join(format!(".{n}.{}.old", std::process::id()));
            std::fs::rename(&dst, &old).unwrap_or_else(|e2| panic!("placing {}: {e}; moving the old copy aside: {e2}", dst.display()));
            std::fs::rename(&tmp, &dst).unwrap_or_else(|e| panic!("placing {} after moving the old copy aside: {e}", dst.display()));
            let _ = std::fs::remove_file(&old);
        }
        dst
    };
    let (agent, gh) = (place("wisp-fake-agent"), place("wisp-fake-gh"));
    drop(built);
    lock.unlock().unwrap();
    assert!(agent.is_file() && gh.is_file(), "{} and {} after the build", agent.display(), gh.display());
    (agent, gh)
}

/// Puts the fakes in `dir` under the CLIs' names (`claude`, `gh`, …).
pub fn install_fakes(dir: &Path, names: &[&str]) {
    let (agent, gh) = fake_bins();
    wisp_testkit::install_fakes_from(dir, names, &agent, &gh).unwrap();
}

/// A fake CLI a test writes itself, at `at` without an extension: the `sh` script off Windows (made executable), and
/// on Windows, which runs no sh script, `at.cmd` holding `cmd` (a batch file, as npm installs CLIs there). The path to
/// name it by comes back. Read a `.cmd` fake's first two arguments as `%A1%` and `%A2%` after [`CMD_ARGS`]: wisp
/// caret-escapes each argument twice for an unknown batch file (`wisp_proc::resolve`, as cross-spawn does, so a prompt a
/// batch file forwards with `%*` can't start a command), so `%~1` is `^"install^"` and reads back as `"install"`.
pub fn script(at: &Path, sh: &str, cmd: &str) -> PathBuf {
    #[cfg(unix)]
    {
        let _ = cmd;
        std::fs::write(at, sh).unwrap();
        std::fs::set_permissions(at, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        at.to_owned()
    }
    #[cfg(windows)]
    {
        let _ = sh;
        let file = at.with_extension("cmd");
        std::fs::write(&file, format!("@echo off\r\n{}", cmd.replace("\r\n", "\n").replace('\n', "\r\n"))).unwrap();
        file
    }
}

/// A `.cmd` fake's prelude: its first two arguments, quotes gone, in `A1` and `A2` (see [`script`]).
#[allow(dead_code)]
pub const CMD_ARGS: &str = "set A1=%~1\nset A2=%~2\nif defined A1 set A1=%A1:\"=%\nif defined A2 set A2=%A2:\"=%\n";

/// Where a test server's user lives: its own home under the test's folder, never the real one (owner-list row 66: a
/// test server read this Mac's real Cursor and opencode MCP config). `HOME`, its Windows twins and the XDG folders
/// point there; each harness's own override (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME`, `GROK_HOME`) is
/// cleared, so it falls back under that home too. A test that sets one passes it in `start_with`'s `env`, which wins.
pub fn user_home(dir: &Path) -> PathBuf {
    dir.join("user-home")
}

/// The variables that keep a spawned wisp off the real user's files (see [`user_home`]): `(set, remove)`.
pub fn isolated_env(dir: &Path) -> (Vec<(&'static str, PathBuf)>, [&'static str; 4]) {
    let home = user_home(dir);
    let set = vec![
        ("HOME", home.clone()),
        ("USERPROFILE", home.clone()),
        ("APPDATA", home.join("AppData/Roaming")),
        ("LOCALAPPDATA", home.join("AppData/Local")),
        ("XDG_CONFIG_HOME", home.join(".config")),
        ("XDG_DATA_HOME", home.join(".local/share")),
        ("XDG_STATE_HOME", home.join(".local/state")),
        ("XDG_CACHE_HOME", home.join(".cache")),
    ];
    (set, ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "COPILOT_HOME", "GROK_HOME"])
}

/// `cmd` with [`isolated_env`] applied (before the caller's own variables, which win).
pub fn isolate(cmd: &mut Command, dir: &Path) {
    let (set, remove) = isolated_env(dir);
    std::fs::create_dir_all(user_home(dir)).unwrap();
    for k in remove {
        cmd.env_remove(k);
    }
    cmd.envs(set);
    // commits made for the task (checkpoints, worktrees) need a name, which the real ~/.gitconfig gave
    cmd.envs([
        ("GIT_AUTHOR_NAME", "wisp test"),
        ("GIT_AUTHOR_EMAIL", "test@wisp.invalid"),
        ("GIT_COMMITTER_NAME", "wisp test"),
        ("GIT_COMMITTER_EMAIL", "test@wisp.invalid"),
    ]);
}

pub fn start(dir: &Path) -> (Serve, u16) {
    start_with(dir, &[])
}

pub fn start_with(dir: &Path, env: &[(&str, &str)]) -> (Serve, u16) {
    install_fakes(&dir.join("bin"), &FAKES.map(|(n, _)| n));
    spawn(dir, env)
}

/// `wisp serve` again on the same home and the same fake CLIs, as they are now (a test may have changed one): nothing
/// is installed again. Installing replaced the fakes, and changing one back meant deleting a program the new server
/// was already running to detect its harness, which Windows refuses (Access is denied). The last server's address
/// goes with it, so the new one's is the one read. `serve` (kept, `keep: true`) must be dropped first.
pub fn restart(dir: &Path) -> (Serve, u16) {
    let _ = std::fs::remove_file(dir.join("home").join("server.json"));
    spawn(dir, &[])
}

fn spawn(dir: &Path, env: &[(&str, &str)]) -> (Serve, u16) {
    let bin = dir.join("bin");
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let log = std::fs::File::create(dir.join("serve.log")).unwrap();
    // WISP_TEST_BIN: another build of wisp to run the same test against (a fix's test, shown failing on the build before it)
    let mut cmd = Command::new(std::env::var_os("WISP_TEST_BIN").map_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_wisp")), PathBuf::from));
    isolate(&mut cmd, dir);
    let child = cmd
        .args(["serve", "--port", "0"])
        .env("WISP_HOME", &home)
        .envs(FAKES.map(|(n, var)| (var, bin.join(format!("{n}{}", std::env::consts::EXE_SUFFIX)))))
        // no real CLI config leaks in: each CLI's home is under the test's folder
        .env("CODEX_HOME", dir.join("codex-home"))
        .env("WISP_WEB_DIR", dir.join("no-web"))
        .env("RUST_LOG", "warn")
        // no update check reaches a registry (wisp's own, the harnesses' `npm view`: setup audit B5)
        .env("WISP_UPDATE_CHECK", "0")
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .unwrap();
    let serve = Serve { child, dir: dir.to_owned(), keep: false };
    let at = Instant::now();
    loop {
        if let Some(port) = std::fs::read(home.join("server.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v["port"].as_u64())
        {
            return (serve, u16::try_from(port).unwrap());
        }
        assert!(
            at.elapsed() < Duration::from_secs(30),
            "wisp serve never advertised its port: {}",
            std::fs::read_to_string(dir.join("serve.log")).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub async fn cmd(http: &reqwest::Client, port: u16, key: &str, name: &str, args: Value) -> Value {
    let r = http
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .header("content-type", "application/json")
        .header("idempotency-key", key)
        .body(json!({ "cmd": name, "args": args }).to_string())
        .send()
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&r.bytes().await.unwrap()).unwrap();
    assert_eq!(v["ok"], true, "{name}: {v}");
    v["result"].clone()
}

/// A fresh folder for one test, with `proj/math.js` in it, spelled long (no 8.3 short names) as wisp stores folders.
pub fn temp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("wisp-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    // spelled as wisp spells every folder it is given (ADR-0071): `%TEMP%` on the Windows runner is
    // `C:\Users\RUNNER~1\…`, and a task's folder, its project and its copies are stored `C:\Users\runneradmin\…`
    let dir = wisp_proc::plain_path_blocking(&dir);
    std::fs::write(dir.join("proj/math.js"), "export const add = (a, b) => a - b;\n").unwrap();
    dir
}

pub struct Api {
    pub http: reqwest::Client,
    pub port: u16,
    pub n: std::sync::atomic::AtomicU32,
}

impl Api {
    pub async fn cmd(&self, name: &str, args: Value) -> Value {
        let n = self.n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cmd(&self.http, self.port, &format!("{name}-{n}"), name, args).await
    }

    /// The raw answer, for a command that may be refused.
    pub async fn try_cmd(&self, name: &str, args: Value) -> Value {
        let n = self.n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let r = self
            .http
            .post(format!("http://127.0.0.1:{}/api/cmd", self.port))
            .header("content-type", "application/json")
            .header("idempotency-key", format!("{name}-{n}"))
            .body(json!({ "cmd": name, "args": args }).to_string())
            .send()
            .await
            .unwrap();
        serde_json::from_slice(&r.bytes().await.unwrap()).unwrap()
    }

    pub async fn turns(&self, id: &str) -> Vec<Value> {
        self.cmd("runtime.turns", json!({ "id": id })).await.as_array().cloned().unwrap_or_default()
    }

    /// Waits until `f` holds for the task's turns.
    pub async fn until(&self, id: &str, what: &str, f: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        let at = Instant::now();
        loop {
            let t = self.turns(id).await;
            if f(&t) {
                return t;
            }
            assert!(at.elapsed() < Duration::from_secs(60), "{what}: {t:?}");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

pub fn ended(t: &Value) -> bool {
    t["status"] != "running" && t["status"] != "held"
}

/// Runs one task on `agent` to its end and returns (the turn, the task). The project has `math.js`.
pub async fn run_task(api: &Api, dir: &Path, agent: &str) -> (Value, Value) {
    let made =
        api.cmd("runtime.create", json!({ "prompt": "fix the add bug TOPIC[Fix add]", "cwd": dir.join("proj"), "agent": agent })).await;
    let id = made["id"].as_str().unwrap().to_owned();
    let turns = api.until(&id, "the turn ends", |t| t.last().is_some_and(ended)).await;
    let task = api.cmd("runtime.get", json!({ "id": id })).await;
    (turns.last().cloned().unwrap_or_default(), task)
}

/// The complete lines of a fake's `FAKE_RECORD` file (`<name>.jsonl`): only those ending in a
/// newline. A fake may still be writing its last line; a reader that took it would parse half a record.
pub fn record_text(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.rfind('\n').map_or_else(String::new, |end| text[..=end].to_owned())
}

/// The complete records in a fake's `FAKE_RECORD` file, parsed, in order.
pub fn records(path: &Path) -> Vec<Value> {
    record_text(path)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("a complete record line that isn't JSON ({e}): {l}")))
        .collect()
}

/// The argv of one record.
pub fn argv(record: &Value) -> Vec<String> {
    record["argv"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// Waits until the complete records in `path` satisfy `f`, and returns them.
pub async fn until_records(path: &Path, what: &str, f: impl Fn(&[Value]) -> bool) -> Vec<Value> {
    let at = Instant::now();
    loop {
        let r = records(path);
        if f(&r) {
            return r;
        }
        assert!(at.elapsed() < Duration::from_secs(30), "{what}: {}", record_text(path));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The folder a fake run's `FAKE_GATE` gates live in (`gate <name> …` prompts wait for `<it>/<name>`).
pub fn gates(dir: &Path) -> PathBuf {
    let g = dir.join("gates");
    std::fs::create_dir_all(&g).unwrap();
    g
}

/// Lets the fake run gated on `name` go on (now, or as soon as it gets there).
pub fn open_gate(gates: &Path, name: &str) {
    std::fs::write(gates.join(name), "open").unwrap();
}
