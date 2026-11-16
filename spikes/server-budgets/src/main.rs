//! ADR-0011's server budgets against a real `wisp serve` (ADR-0080), on a home seeded with 5 000 tasks:
//!   cold start  process start → the first 200 from `GET /api/status`, median of 7 restarts   ≤ 150 ms
//!   idle RSS    the server's resident set once it settles, after a client read the list      ≤ 120 MB
//!   list        `GET /api/events?fresh=list` → the `list` snapshot's last byte, median of 30  ≤ 5 ms
//! The list is the UI's first read (packages/sync's hello: `fresh=list`), answered by the server's own snapshot path.
//! The home is seeded through wisp.runtime's schema and `repo::save_session` (the fixture its list bench uses), never
//! a hand-made database. Fake harness CLIs, a cleared environment, no network. Exits 1 on a miss, 2 when it can't run.
//!
//!   cargo run --profile budgets --manifest-path spikes/server-budgets/Cargo.toml -- [--wisp=<bin>] [--summary=<file>]
//!
//! `--wisp` defaults to `$CARGO_TARGET_DIR/budgets/wisp` (else `target/budgets/wisp`):
//! `cargo build --profile budgets -p wisp` first. Under GitHub Actions the table also goes to `$GITHUB_STEP_SUMMARY`.
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use wisp_runtime::{repo, schema};
use wisp_runtime_api::model::{Mode, Session, SessionState};
use wisp_runtime_core::summary::{RowFacts, summarize};
use wisp_store::{Location, Query, Store, StoreError, SystemClock};

// tools/budgets.ts `SERVER_BUDGETS`, the one list of the numbers (tools/test/budgets.test.ts fails when these differ)
const COLD_START_MS: f64 = 150.0;
const IDLE_RSS_MB: f64 = 120.0;
const LIST_MS: f64 = 5.0;

const TASKS: usize = 5_000;
const COLD_RUNS: usize = 7;
const LIST_WARMUP: usize = 3;
const LIST_RUNS: usize = 30;
/// RSS is settled once 8 samples 250 ms apart (2 s) stay within 1 MB; given up after 30 s
const RSS_EVERY: Duration = Duration::from_millis(250);
const RSS_WINDOW: usize = 8;
const RSS_SPREAD_KB: u64 = 1024;
const RSS_CAP: Duration = Duration::from_secs(30);

/// The harness CLIs wisp looks for, by file name and override variable (crates/wisp/tests/common `FAKES`, plus gh).
const FAKES: [(&str, &str); 8] = [
    ("claude", "WISP_CLAUDE_BIN"),
    ("codex", "WISP_CODEX_BIN"),
    ("opencode", "WISP_OPENCODE_BIN"),
    ("cursor-agent", "WISP_CURSOR_BIN"),
    ("copilot", "WISP_COPILOT_BIN"),
    ("grok", "WISP_GROK_BIN"),
    ("agy", "WISP_ANTIGRAVITY_BIN"),
    ("gh", "WISP_GH_BIN"),
];
/// Answers a version probe and does nothing else: no harness ever runs here.
const FAKE_CLI: &str = "#!/bin/sh\ncase \"$1\" in --version|-v|version) echo \"0.0.0 (fake)\" ;; esac\nexit 0\n";

type Res<T> = Result<T, String>;

fn said<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> String + '_ {
    move |e| format!("{what}: {e}")
}

struct Opts {
    wisp: PathBuf,
    summary: Option<PathBuf>,
    keep: bool,
}

fn opts() -> Opts {
    let arg = |name: &str| std::env::args().find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_owned));
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| repo.join("target"), PathBuf::from);
    Opts {
        wisp: arg("wisp").map_or_else(|| target.join("budgets").join(format!("wisp{}", std::env::consts::EXE_SUFFIX)), PathBuf::from),
        summary: arg("summary").or_else(|| std::env::var("GITHUB_STEP_SUMMARY").ok()).filter(|s| !s.is_empty()).map(PathBuf::from),
        keep: std::env::args().any(|a| a == "--keep"),
    }
}

// ---------- the home ----------

fn task(i: usize, projects: &[String], now: i64) -> Session {
    const AGENTS: [(&str, &str); 3] = [("claude", "opus"), ("codex", "gpt-5"), ("opencode", "sonnet")];
    let (agent, model) = AGENTS[i % AGENTS.len()];
    let back = i64::try_from(TASKS - i).unwrap_or(0) * 60_000;
    Session {
        id: format!("s{i:05}").as_str().into(),
        title: format!("Fix the flaky {} test in module {} and add a regression case", ["auth", "sync", "list"][i % 3], i % 97),
        agent: agent.into(),
        model: model.into(),
        mode: Mode::Auto,
        cwd: projects[i % projects.len()].clone(),
        // nothing running: a running task at boot is an orphan the server would go looking for
        state: [SessionState::Done, SessionState::Review, SessionState::Done, SessionState::Error][i % 4],
        unread: i.is_multiple_of(7),
        archived: i % 10 == 9,
        created_at: now - back - 600_000,
        updated_at: now - back,
        tags: if i.is_multiple_of(3) { vec!["backend".into()] } else { Vec::new() },
        ..Session::default()
    }
}

/// 5 000 tasks in `<home>/wisp.db`, 500 a transaction, through wisp.runtime's own schema and writer.
fn seed(home: &Path, projects: &[String]) -> Res<Duration> {
    let store = Store::open(Location::Path(home.join("wisp.db")), Arc::new(SystemClock)).map_err(said("open the store"))?;
    schema::install(&store).map_err(said("install wisp.runtime's schema"))?;
    let now =
        i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()).unwrap_or(0);
    let t = Instant::now();
    for from in (0..TASKS).step_by(500) {
        let projects = projects.to_vec();
        store
            .write_blocking(move |w| {
                for i in from..(from + 500).min(TASKS) {
                    let s = task(i, &projects, now);
                    let sum = summarize(&s, &RowFacts::default(), BTreeMap::new());
                    repo::save_session(w, &s, &sum, None).map_err(|e| StoreError::Internal(e.to_string()))?;
                }
                Ok(())
            })
            .map_err(said("seed the tasks"))?;
    }
    store.close();
    Ok(t.elapsed())
}

// ---------- the server ----------

struct Bench {
    dir: PathBuf,
    wisp: PathBuf,
}

/// A `wisp serve` this bench started: stopped (SIGTERM, then SIGKILL after 10 s) when dropped, by its own pid only.
struct Serve {
    child: Child,
    port: u16,
    home: PathBuf,
}

impl Drop for Serve {
    fn drop(&mut self) {
        let pid = self.child.id().to_string();
        let _ = Command::new("kill").args(["-TERM", pid.as_str()]).status();
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(self.home.join("server.json"));
    }
}

fn free_port() -> Res<u16> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(said("find a free port"))?;
    l.local_addr().map(|a| a.port()).map_err(said("find a free port"))
}

fn addr(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

impl Bench {
    fn home(&self) -> PathBuf {
        self.dir.join("home")
    }

    /// `wisp serve` on the seeded home, in a cleared environment: fakes first on PATH, every CLI's home under the
    /// bench's folder, no telemetry, no update check, no web build.
    fn spawn(&self, run: &str) -> Res<(Serve, Instant)> {
        let port = free_port()?;
        let (bin, user) = (self.dir.join("bin"), self.dir.join("user"));
        let log = std::fs::File::create(self.dir.join(format!("serve-{run}.log"))).map_err(said("create the log"))?;
        let mut cmd = Command::new(&self.wisp);
        cmd.args(["serve", "--port", &port.to_string()])
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("HOME", &user)
            .env("XDG_CONFIG_HOME", user.join(".config"))
            .env("CODEX_HOME", user.join(".codex"))
            .env("TMPDIR", std::env::temp_dir())
            .env("WISP_HOME", self.home())
            .env("WISP_WEB_DIR", self.dir.join("no-web"))
            .env("RUST_LOG", "warn")
            .env("CI", "1")
            .env("DO_NOT_TRACK", "1")
            .env("WISP_TELEMETRY", "0")
            .env("WISP_UPDATE_CHECK", "0")
            .envs(FAKES.map(|(n, var)| (var, bin.join(n))))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        let t0 = Instant::now();
        let child = cmd.spawn().map_err(said(&format!("start {}", self.wisp.display())))?;
        Ok((Serve { child, port, home: self.home() }, t0))
    }

    fn log(&self, run: &str) -> String {
        std::fs::read_to_string(self.dir.join(format!("serve-{run}.log"))).unwrap_or_default()
    }

    /// Starts a server and waits for its first successful response: the time from process start to it.
    fn boot(&self, run: &str) -> Res<(Serve, Duration)> {
        let (mut s, t0) = self.spawn(run)?;
        loop {
            if status(s.port) == Some(200) {
                return Ok((s, t0.elapsed()));
            }
            if let Ok(Some(exit)) = s.child.try_wait() {
                return Err(format!("wisp serve exited ({exit}) before answering:\n{}", self.log(run)));
            }
            if t0.elapsed() > Duration::from_secs(60) {
                return Err(format!("wisp serve never answered GET /api/status in 60 s:\n{}", self.log(run)));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// `GET /api/status`'s status code, or None while nothing answers.
fn status(port: u16) -> Option<u16> {
    let mut s = TcpStream::connect_timeout(&addr(port), Duration::from_millis(200)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    write!(s, "GET /api/status HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").ok()?;
    let mut head = Vec::new();
    let mut buf = [0u8; 256];
    while head.len() < 12 {
        let n = s.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        head.extend_from_slice(&buf[..n]);
    }
    std::str::from_utf8(&head[9..12]).ok()?.parse().ok()
}

// ---------- the list ----------

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// An HTTP/1.1 chunked body, decoded as it arrives.
#[derive(Default)]
struct Chunked {
    raw: Vec<u8>,
    left: usize,
    crlf: bool,
}

impl Chunked {
    fn feed(&mut self, bytes: &[u8], out: &mut Vec<u8>) -> Res<()> {
        self.raw.extend_from_slice(bytes);
        loop {
            if self.left > 0 {
                let n = self.left.min(self.raw.len());
                if n == 0 {
                    return Ok(());
                }
                out.extend(self.raw.drain(..n));
                self.left -= n;
            } else if self.crlf {
                if self.raw.len() < 2 {
                    return Ok(());
                }
                self.raw.drain(..2);
                self.crlf = false;
            } else {
                let Some(end) = find(&self.raw, b"\r\n") else { return Ok(()) };
                let line = String::from_utf8_lossy(&self.raw[..end]).into_owned();
                self.raw.drain(..end + 2);
                let size = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16).map_err(said("a chunk size"))?;
                if size == 0 {
                    return Err("the event stream ended before the list snapshot".into());
                }
                (self.left, self.crlf) = (size, true);
            }
        }
    }
}

struct ListRead {
    took: Duration,
    tasks: usize,
    bytes: usize,
}

/// The UI's first read: a v2 hello asking for `list` fresh, timed from connect to the snapshot frame's last byte.
fn list_once(port: u16) -> Res<ListRead> {
    let t0 = Instant::now();
    let mut s = TcpStream::connect(addr(port)).map_err(said("connect"))?;
    s.set_nodelay(true).map_err(said("nodelay"))?;
    s.set_read_timeout(Some(Duration::from_secs(10))).map_err(said("timeout"))?;
    write!(
        s,
        "GET /api/events?proto_min=2&proto_max=2&client_id=budgets&build=budgets&fresh=list HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\n\r\n"
    )
    .map_err(said("send the hello"))?;
    let (mut raw, mut body, mut chunked) = (Vec::new(), Vec::new(), None::<Chunked>);
    let mut head_done = false;
    let (mut scan, mut searched) = (0, 0);
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let got = s.read(&mut buf).map_err(said("read the stream"))?;
        if got == 0 {
            return Err("the event stream closed before the list snapshot".into());
        }
        if head_done {
            match chunked.as_mut() {
                Some(c) => c.feed(&buf[..got], &mut body)?,
                None => body.extend_from_slice(&buf[..got]),
            }
        } else {
            raw.extend_from_slice(&buf[..got]);
            let Some(end) = find(&raw, b"\r\n\r\n") else { continue };
            let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
            if !head.starts_with("http/1.1 200") {
                return Err(format!("GET /api/events answered {}", head.lines().next().unwrap_or("")));
            }
            head_done = true;
            let rest = raw.split_off(end + 4);
            if head.contains("transfer-encoding: chunked") {
                let mut c = Chunked::default();
                c.feed(&rest, &mut body)?;
                chunked = Some(c);
            } else {
                body = rest;
            }
        }
        // whole SSE frames: `id`, `event`, `data: [messages]`, a blank line
        // (searched from where the last look stopped: rescanning a 4.5 MB frame on every read would time the bench)
        while let Some(end) = find(&body[searched..], b"\n\n").map(|e| searched + e) {
            let took = t0.elapsed();
            let frame = &body[scan..end];
            (scan, searched) = (end + 2, end + 2);
            if find(frame, b"\"t\":\"snapshot\"").is_none() || find(frame, b"\"topic\":\"list\"").is_none() {
                continue;
            }
            let data = frame.split(|b| *b == b'\n').find_map(|l| l.strip_prefix(b"data: ")).ok_or("a snapshot frame with no data")?;
            let msgs: Vec<Value> = serde_json::from_slice(data).map_err(said("the snapshot frame"))?;
            let snap = msgs.iter().find(|m| m["t"] == "snapshot" && m["topic"] == "list").ok_or("no list snapshot in its frame")?;
            if snap["more"] == true {
                return Err("the list snapshot came in pages: time every page".into());
            }
            let tasks = snap["rows"].as_array().map_or(0, |r| r.iter().filter(|r| r["e"] == "session").count());
            return Ok(ListRead { took, tasks, bytes: frame.len() });
        }
        searched = body.len().saturating_sub(1).max(scan);
        if t0.elapsed() > Duration::from_secs(30) {
            return Err("no list snapshot in 30 s (heartbeats only)".into());
        }
    }
}

// ---------- RSS ----------

/// The resident set of `pid` in KB.
fn rss_kb(pid: u32) -> Option<u64> {
    if cfg!(target_os = "linux") {
        let s = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        s.lines().find_map(|l| l.strip_prefix("VmRSS:"))?.trim().trim_end_matches("kB").trim().parse().ok()
    } else {
        let o = Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().ok()?;
        String::from_utf8_lossy(&o.stdout).trim().parse().ok()
    }
}

struct Rss {
    kb: u64,
    after: Duration,
    settled: bool,
}

/// Samples until the last 2 s stay within 1 MB (settled) or 30 s pass (the last sample, not settled).
fn settle(pid: u32) -> Res<Rss> {
    let t0 = Instant::now();
    let mut seen: Vec<u64> = Vec::new();
    loop {
        seen.push(rss_kb(pid).ok_or("read the server's RSS")?);
        let last = &seen[seen.len().saturating_sub(RSS_WINDOW)..];
        let spread = last.iter().max().unwrap_or(&0) - last.iter().min().unwrap_or(&0);
        let kb = *last.last().unwrap_or(&0);
        if last.len() == RSS_WINDOW && spread <= RSS_SPREAD_KB {
            return Ok(Rss { kb, after: t0.elapsed(), settled: true });
        }
        if t0.elapsed() >= RSS_CAP {
            return Ok(Rss { kb, after: t0.elapsed(), settled: false });
        }
        std::thread::sleep(RSS_EVERY);
    }
}

// ---------- where the list's time goes (reported, not gated) ----------

struct Breakdown {
    /// `changed_since("list", 0)`: the store read the server's snapshot of `list` makes today (no full-read source)
    feed: Duration,
    /// the same tasks in one query of the session table, as JSON (what a `list` source would read)
    query: Duration,
    /// the change feed's rows encoded as JSON
    encode: Duration,
    rows: usize,
}

/// The list's store read in this process, on the server's home once it stopped: medians of 10.
fn breakdown(home: &Path) -> Res<Breakdown> {
    let store = Store::open(Location::Path(home.join("wisp.db")), Arc::new(SystemClock)).map_err(said("open the store"))?;
    schema::install(&store).map_err(said("define wisp.runtime's kinds"))?;
    let median = |f: &mut dyn FnMut() -> Res<()>| -> Res<Duration> {
        let mut v = Vec::new();
        for _ in 0..10 {
            let t = Instant::now();
            f()?;
            v.push(t.elapsed());
        }
        v.sort();
        Ok(v[v.len() / 2])
    };
    let mut rows = 0;
    let feed = median(&mut || {
        let d = store.changed_since("list", 0).map_err(said("read the change feed"))?.ok_or("the change feed was pruned")?;
        rows = d.rows.len();
        Ok(())
    })?;
    let d = store.changed_since("list", 0).map_err(said("read the change feed"))?.ok_or("the change feed was pruned")?;
    let encode = median(&mut || serde_json::to_vec(&d.rows).map(drop).map_err(said("encode")))?;
    let query = median(&mut || store.query_json("session", &Query::new()).map(drop).map_err(said("query the sessions")))?;
    store.close();
    Ok(Breakdown { feed, query, encode, rows })
}

// ---------- the report ----------

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn mb(kb: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let m = kb as f64 / 1024.0;
    m
}

/// min, median, p95 and max of `v` in ms
fn spread(v: &[Duration]) -> (f64, f64, f64, f64) {
    let mut v = v.to_vec();
    v.sort();
    let at = |i: usize| v.get(i.min(v.len().saturating_sub(1))).map_or(0.0, |d| ms(*d));
    (at(0), at(v.len() / 2), at(v.len() * 95 / 100), at(v.len().saturating_sub(1)))
}

struct Report {
    seeded: Duration,
    first_boot: Duration,
    cold: Vec<Duration>,
    rss_before: Rss,
    rss: Rss,
    list: Vec<Duration>,
    list_bytes: usize,
    why: Option<Breakdown>,
}

impl Report {
    fn cold_ms(&self) -> f64 {
        spread(&self.cold).1
    }
    fn list_ms(&self) -> f64 {
        spread(&self.list).1
    }
    fn over(&self) -> Vec<String> {
        let mut over = Vec::new();
        if self.cold_ms() > COLD_START_MS {
            over.push(format!("cold start {:.1} ms (median of {}) > {COLD_START_MS} ms", self.cold_ms(), self.cold.len()));
        }
        if !self.rss.settled {
            over.push(format!("idle RSS never settled in 30 s (last {:.1} MB, budget {IDLE_RSS_MB} MB)", mb(self.rss.kb)));
        } else if mb(self.rss.kb) > IDLE_RSS_MB {
            over.push(format!("idle RSS {:.1} MB (settled) > {IDLE_RSS_MB} MB", mb(self.rss.kb)));
        }
        if self.list_ms() > LIST_MS {
            over.push(format!("task list {:.2} ms (median of {}) > {LIST_MS} ms", self.list_ms(), self.list.len()));
        }
        over
    }

    fn summary(&self) -> String {
        let mark = |ok: bool| if ok { "✅" } else { "❌" };
        let (c0, c50, c95, c100) = spread(&self.cold);
        let (l0, l50, l95, l100) = spread(&self.list);
        let rss_ok = mb(self.rss.kb) <= IDLE_RSS_MB && self.rss.settled;
        [
            format!("### Server budgets (ADR-0011), `wisp serve` on {TASKS} tasks"),
            String::new(),
            "| | measured | budget | runs (min · p95 · max) | |".into(),
            "|---|---:|---:|---|---|".into(),
            format!(
                "| cold start → first response | {c50:.1} ms median | {COLD_START_MS} ms | {} ({c0:.1} · {c95:.1} · {c100:.1} ms) | {} |",
                self.cold.len(),
                mark(c50 <= COLD_START_MS)
            ),
            format!(
                "| idle RSS | {:.1} MB settled | {IDLE_RSS_MB} MB | after {:.1} s; {:.1} MB before the list was read | {} |",
                mb(self.rss.kb),
                self.rss.after.as_secs_f64(),
                mb(self.rss_before.kb),
                mark(rss_ok)
            ),
            format!(
                "| task list (`fresh=list` snapshot) | {l50:.2} ms median | {LIST_MS} ms | {} ({l0:.2} · {l95:.2} · {l100:.2} ms), {:.0} KB a read | {} |",
                self.list.len(),
                self.list_bytes as f64 / 1024.0,
                mark(l50 <= LIST_MS)
            ),
            String::new(),
            format!(
                "Seeded in {:.1} s through wisp.runtime's writer; the first boot (every plugin's migrations) took {:.0} ms and isn't gated.",
                self.seeded.as_secs_f64(),
                ms(self.first_boot)
            ),
            String::new(),
            self.why.as_ref().map_or_else(String::new, |b| {
                format!(
                    "Where the list's time goes (in process, medians of 10): the change feed read `changed_since(\"list\", 0)` {:.1} ms for {} rows, \
                     their JSON {:.1} ms; one query of the session table as JSON {:.1} ms.\n",
                    ms(b.feed),
                    b.rows,
                    ms(b.encode),
                    ms(b.query)
                )
            }),
        ]
        .join("\n")
    }
}

fn run(o: &Opts, dir: &Path) -> Res<Report> {
    if !o.wisp.is_file() {
        return Err(format!("no wisp binary at {} (cargo build --profile budgets -p wisp, or --wisp=<bin>)", o.wisp.display()));
    }
    let b = Bench { dir: dir.to_owned(), wisp: o.wisp.clone() };
    for d in ["home", "bin", "user", "no-web", "code"] {
        std::fs::create_dir_all(dir.join(d)).map_err(said("create the bench's folders"))?;
    }
    for (name, _) in FAKES {
        let p = dir.join("bin").join(name);
        std::fs::write(&p, FAKE_CLI).map_err(said("write a fake CLI"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).map_err(said("chmod a fake CLI"))?;
        }
    }
    let projects: Vec<String> = (0..12)
        .map(|i| {
            let p = dir.join("code").join(format!("project-{i}"));
            std::fs::create_dir_all(&p).map(|()| p.to_string_lossy().into_owned())
        })
        .collect::<Result<_, _>>()
        .map_err(said("create the projects"))?;
    let seeded = seed(&b.home(), &projects)?;
    println!("seeded {TASKS} tasks in {:.1} s", seeded.as_secs_f64());

    // the first boot migrates every other plugin's tables: a restart's cold start is what's gated
    let (s, first_boot) = b.boot("first")?;
    drop(s);
    println!("first boot (migrations): {:.0} ms", ms(first_boot));
    let mut cold = Vec::new();
    for i in 0..COLD_RUNS {
        let (s, took) = b.boot(&format!("cold-{i}"))?;
        drop(s);
        println!("  cold start {}: {:.1} ms", i + 1, ms(took));
        cold.push(took);
    }

    let (s, _) = b.boot("idle")?;
    let pid = s.child.id();
    let rss_before = settle(pid)?;
    println!("RSS before any client: {:.1} MB (after {:.1} s)", mb(rss_before.kb), rss_before.after.as_secs_f64());
    let first = list_once(s.port)?;
    if first.tasks != TASKS {
        return Err(format!("the list snapshot had {} tasks, not {TASKS}", first.tasks));
    }
    for _ in 0..LIST_WARMUP {
        list_once(s.port)?;
    }
    let mut list = Vec::new();
    for _ in 0..LIST_RUNS {
        list.push(list_once(s.port)?.took);
    }
    println!("first list read (cold): {:.2} ms", ms(first.took));
    let rss = settle(pid)?;
    drop(s);
    let why = breakdown(&b.home()).inspect_err(|e| eprintln!("server budgets: no breakdown: {e}")).ok();
    Ok(Report { seeded, first_boot, cold, rss_before, rss, list, list_bytes: first.bytes, why })
}

fn main() {
    if !cfg!(unix) {
        println!("server budgets: Linux and macOS only (the CI step runs on Linux)");
        return;
    }
    let o = opts();
    let dir = std::env::temp_dir().join(format!("wisp-server-budgets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let r = run(&o, &dir);
    if o.keep {
        println!("kept {}", dir.display());
    } else {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let r = match r {
        Ok(r) => r,
        Err(e) => {
            eprintln!("server budgets: {e}");
            std::process::exit(2);
        }
    };
    let table = r.summary();
    println!("\n{table}");
    if let Some(to) = &o.summary {
        let written = std::fs::OpenOptions::new().create(true).append(true).open(to).and_then(|mut f| f.write_all(table.as_bytes()));
        if let Err(e) = written {
            eprintln!("server budgets: could not write the summary to {}: {e}", to.display());
        }
    }
    let over = r.over();
    let gh = std::env::var_os("GITHUB_ACTIONS").is_some();
    for o in &over {
        eprintln!("{}OVER BUDGET: {o}", if gh { "::error title=Server budget::" } else { "" });
    }
    std::process::exit(i32::from(!over.is_empty()));
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn report(cold: u64, rss_mb: u64, settled: bool, list_us: u64) -> Report {
        let rss = |kb| Rss { kb, after: Duration::from_secs(2), settled };
        Report {
            seeded: Duration::ZERO,
            first_boot: Duration::ZERO,
            cold: vec![Duration::from_millis(cold); 3],
            rss_before: rss(rss_mb * 1024),
            rss: rss(rss_mb * 1024),
            list: vec![Duration::from_micros(list_us); 3],
            list_bytes: 0,
            why: None,
        }
    }

    #[test]
    fn a_chunked_body_decodes_the_same_however_it_is_split() {
        let body = b"id: 1\nevent: m\ndata: [{\"t\":\"welcome\"}]\n\nid: 2\nevent: m\ndata: [{\"t\":\"snapshot\",\"topic\":\"list\"}]\n\n";
        let mut wire = Vec::new();
        for part in body.chunks(17) {
            wire.extend_from_slice(format!("{:x};ext=1\r\n", part.len()).as_bytes());
            wire.extend_from_slice(part);
            wire.extend_from_slice(b"\r\n");
        }
        for step in [1, 2, 3, 7, 64, wire.len()] {
            let (mut c, mut out) = (Chunked::default(), Vec::new());
            for piece in wire.chunks(step) {
                c.feed(piece, &mut out).unwrap();
            }
            assert_eq!(out, body, "fed {step} bytes at a time");
        }
    }

    #[test]
    fn the_last_chunk_before_the_snapshot_is_an_error_not_a_hang() {
        let mut out = Vec::new();
        assert!(Chunked::default().feed(b"0\r\n\r\n", &mut out).is_err());
    }

    #[test]
    fn spread_is_min_median_p95_max_and_never_panics_on_none() {
        let v: Vec<Duration> = [5, 1, 3, 2, 4].iter().map(|m| Duration::from_millis(*m)).collect();
        assert_eq!(spread(&v), (1.0, 3.0, 5.0, 5.0));
        assert_eq!(spread(&[]), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn each_budget_misses_on_its_own_and_the_budget_itself_passes() {
        assert!(report(150, 120, true, 5_000).over().is_empty());
        let over = report(151, 121, true, 5_001).over();
        assert_eq!(over.len(), 3, "{over:?}");
        assert!(over[0].starts_with("cold start") && over[1].starts_with("idle RSS") && over[2].starts_with("task list"));
    }

    #[test]
    fn an_rss_that_never_settles_is_a_miss_even_under_the_budget() {
        let over = report(10, 50, false, 1_000).over();
        assert_eq!(over, vec!["idle RSS never settled in 30 s (last 50.0 MB, budget 120 MB)".to_owned()]);
        assert!(report(10, 50, false, 1_000).summary().contains("| ❌ |"));
    }
}
