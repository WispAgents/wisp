//! ADR-0011's server budgets against a real `wisp serve` (ADR-0080), on a home seeded with 5 000 tasks:
//!   cold start  process start → the first 200 from `GET /api/status`, median of 5 restarts    ≤ 150 ms
//!   idle RSS    the server's resident set once it settles, after a client read the list      ≤ 120 MB
//!   list        `GET /api/events?fresh=list` → the `list` snapshot's first page, median of 12  ≤ 5 ms
//!   patch p99   a running task's output written → each client's patch carrying it, p99           ≤ 150 ms
//!               (10 tasks streaming through the fake claude's `FAKE_STREAM`, 4 clients on `list` and all 10 tasks)
//!   append      durable events a second (spikes/events' `append_rate`, 1 000 a transaction)      ≥ 20 000 /s
//!   write amp   bytes the store wrote (WAL + database growth) ÷ the activity the 10 tasks kept    ≤ 15×
//! The list is the UI's first read (packages/sync's hello: `fresh=list`), answered by the server's own snapshot path in
//! pages (ADR-0081): the first is what the first screen draws; the time to the last page (every task) is reported.
//! The home is seeded through wisp.runtime's schema and `repo::save_session` (the fixture its list bench uses), never
//! a hand-made database. Fake harness CLIs, a cleared environment, no network. The three timed budgets are gated on
//! the median of rounds spread over the job (5, 5 and 3), each bracketed by a calibration of the runner (ADR-0089).
//! Exits 1 on a miss, 2 when it can't run, 3 when a timed budget is over while most of its rounds ran on a slowed
//! runner (too noisy to judge: never a pass).
//!
//!   cargo run --profile budgets --manifest-path spikes/server-budgets/Cargo.toml -- [--wisp=<bin>] [--fake=<bin>] [--summary=<file>]
//!
//! `--wisp` and `--fake` default to `$CARGO_TARGET_DIR/budgets/{wisp,wisp-fake-agent}` (else `target/budgets/…`):
//! `cargo build --profile budgets -p wisp -p wisp-testkit` first. Under GitHub Actions the table also goes to `$GITHUB_STEP_SUMMARY`.
mod mem;
mod wal;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use wisp_runtime::{repo, schema};
use wisp_runtime_api::model::{Mode, Session, SessionState, Step};
use wisp_runtime_core::summary::{RowFacts, summarize};
use wisp_store::{Location, Query, Store, StoreError, SystemClock};

// tools/budgets.ts `SERVER_BUDGETS`, the one list of the numbers (tools/test/budgets.test.ts fails when these differ)
const COLD_START_MS: f64 = 150.0;
const IDLE_RSS_MB: f64 = 120.0;
const LIST_MS: f64 = 5.0;
const PATCH_P99_MS: f64 = 150.0;
const APPEND_PER_S: f64 = 20_000.0;
const WRITE_AMPLIFICATION: f64 = 44.0;

const TASKS: usize = 5_000;
/// The timed budgets (cold start, the list, patch p99) are taken in rounds spread over the job and gated on the median
/// of the rounds (ADR-0089): a slow spell on a shared runner spoils a round, not the verdict. Cold start and the list
/// take turns, `ROUNDS` times: each round `COLD_PER_ROUND` fresh servers, then `LIST_PER_ROUND` reads (after
/// `LIST_WARMUP`) by a fresh client of the idle server; the stream runs `PATCH_ROUNDS` times, fresh tasks and clients.
const ROUNDS: usize = 5;
const COLD_PER_ROUND: usize = 5;
const LIST_WARMUP: usize = 2;
const LIST_PER_ROUND: usize = 12;
const PATCH_ROUNDS: usize = 3;
/// The runner's calibration (`calibrate`): a round whose bracket ran over `NOISY`× the job's fastest is disturbed
const NOISY: f64 = 1.5;
const CALIB_RUNS: usize = 5;
/// RSS is settled once 8 samples 250 ms apart (2 s) stay within 1 MB; given up after 30 s
const RSS_EVERY: Duration = Duration::from_millis(250);
const RSS_WINDOW: usize = 8;
const RSS_SPREAD_KB: u64 = 1024;
const RSS_CAP: Duration = Duration::from_secs(30);
/// One heavy user (plan §8): 10 running tasks, 4 clients each on `list` and every running task's topic (the worst
/// fan-out: G3's clients hold one open task each). Each task streams a real agent's steps (the fake's
/// `FAKE_STREAM_MIX=agent`: thinking, Bash calls, their results, answers, sized as 275 374 real steps were) at 2 a
/// second for 10 s: above the busiest 10 s of 99 % of 3 872 real turns (1.8 a second; README *The stream's shape*).
const RUNNING: usize = 10;
const CLIENTS: usize = 4;
const STREAM: Shape = Shape { per_s: 2, ms: 10_000, mix: Mix::Agent };
/// `--stream=<per_s>:<ms>:text`'s line length (ADR-0082's stream: 400 B text lines, the runtime bench's 2 MB in 5 000)
const TEXT_BYTES: usize = 400;

/// What each running task streams: `FAKE_STREAM` lines a second for `FAKE_STREAM_MS`, of `FAKE_STREAM_MIX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shape {
    per_s: u64,
    ms: u64,
    mix: Mix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mix {
    /// a real agent's steps (`FAKE_STREAM_MIX=agent`)
    Agent,
    /// `TEXT_BYTES` of assistant text a line (ADR-0082's first stream)
    Text,
}

impl Shape {
    /// stamped lines each task writes: the n-th is due at n / rate, n from 0, while that is under `ms`
    fn stamps(self) -> usize {
        usize::try_from((self.per_s * self.ms).div_ceil(1000)).unwrap_or(usize::MAX)
    }

    /// `--stream=<per_s>:<ms>:<agent|text>`, to measure another shape by hand (the gate is `STREAM`)
    fn parse(s: &str) -> Option<Self> {
        let mut p = s.split(':');
        let (per_s, ms) = (p.next()?.parse().ok().filter(|r| *r > 0)?, p.next()?.parse().ok().filter(|m| *m > 0)?);
        let mix = match p.next()? {
            "agent" => Mix::Agent,
            "text" => Mix::Text,
            _ => return None,
        };
        p.next().is_none().then_some(Self { per_s, ms, mix })
    }
}

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
/// Answers a version probe and does nothing else: every CLI but `claude`, which is wisp-testkit's fake agent (the
/// patch latency's running tasks stream through it).
const FAKE_CLI: &str = "#!/bin/sh\ncase \"$1\" in --version|-v|version) echo \"0.0.0 (fake)\" ;; esac\nexit 0\n";

type Res<T> = Result<T, String>;

fn said<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> String + '_ {
    move |e| format!("{what}: {e}")
}

struct Opts {
    wisp: PathBuf,
    fake: PathBuf,
    summary: Option<PathBuf>,
    keep: bool,
    stream: Shape,
}

fn opts() -> Opts {
    let arg = |name: &str| std::env::args().find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_owned));
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| repo.join("target"), PathBuf::from);
    let built = |name: &str| target.join("budgets").join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    Opts {
        wisp: arg("wisp").map_or_else(|| built("wisp"), PathBuf::from),
        fake: arg("fake").map_or_else(|| built("wisp-fake-agent"), PathBuf::from),
        summary: arg("summary").or_else(|| std::env::var("GITHUB_STEP_SUMMARY").ok()).filter(|s| !s.is_empty()).map(PathBuf::from),
        keep: std::env::args().any(|a| a == "--keep"),
        stream: arg("stream").map_or(Some(STREAM), |s| Shape::parse(&s)).unwrap_or_else(|| {
            eprintln!("server budgets: --stream=<lines a second>:<ms>:<agent|text>");
            std::process::exit(2);
        }),
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
    stream: Shape,
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

    /// A copy of the home once migrated, where the cold starts boot while the idle server keeps `home` (ADR-0089)
    fn cold_home(&self) -> PathBuf {
        self.dir.join("cold-home")
    }

    /// `wisp serve` on a seeded home, in a cleared environment: fakes first on PATH, every CLI's home under the
    /// bench's folder, no telemetry, no update check, no web build.
    fn spawn(&self, home: &Path, run: &str, env: &[(&str, String)]) -> Res<(Serve, Instant)> {
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
            .env("WISP_HOME", home)
            .env("WISP_WEB_DIR", self.dir.join("no-web"))
            .env("RUST_LOG", "warn")
            .env("CI", "1")
            .env("DO_NOT_TRACK", "1")
            .env("WISP_TELEMETRY", "0")
            .env("WISP_UPDATE_CHECK", "0")
            // only a run streams or waits on the gate: the probes and the other servers here start none
            .env("FAKE_STREAM", self.stream.per_s.to_string())
            .env("FAKE_STREAM_MS", self.stream.ms.to_string())
            .env("FAKE_STREAM_BYTES", TEXT_BYTES.to_string())
            .env("FAKE_STREAM_MIX", if self.stream.mix == Mix::Agent { "agent" } else { "text" })
            .env("FAKE_GATE", self.dir.join("gates"))
            .envs(FAKES.map(|(n, var)| (var, bin.join(n))))
            .envs(env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        let t0 = Instant::now();
        let child = cmd.spawn().map_err(said(&format!("start {}", self.wisp.display())))?;
        Ok((Serve { child, port, home: home.to_owned() }, t0))
    }

    fn log(&self, run: &str) -> String {
        std::fs::read_to_string(self.dir.join(format!("serve-{run}.log"))).unwrap_or_default()
    }

    /// Starts a server and waits for its first successful response: the time from process start to it.
    fn boot(&self, run: &str) -> Res<(Serve, Duration)> {
        self.boot_in(&self.home(), run, &[])
    }

    /// `boot` with more environment.
    fn boot_with(&self, run: &str, env: &[(&str, String)]) -> Res<(Serve, Duration)> {
        self.boot_in(&self.home(), run, env)
    }

    /// `boot` on `home`.
    fn boot_in(&self, home: &Path, run: &str, env: &[(&str, String)]) -> Res<(Serve, Duration)> {
        let (mut s, t0) = self.spawn(home, run, env)?;
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
    /// to the first page's last byte: what the first screen draws (ADR-0081), gated
    took: Duration,
    /// to the last page's last byte: every task there (search, the counts), reported
    all: Duration,
    /// tasks in the first page
    first: usize,
    /// distinct tasks over every page
    tasks: usize,
    pages: usize,
    bytes: usize,
}

/// The `list` snapshot as its pages arrive (ADR-0081: the first small, then the rest, all at one seq; `more` on all
/// but the last). A task in two pages, or pages at two seqs, is an error: the client would start over.
#[derive(Default)]
struct Pages {
    first: Option<(Duration, usize)>,
    seq: Option<u64>,
    ids: std::collections::HashSet<String>,
    pages: usize,
    bytes: usize,
}

impl Pages {
    /// One whole SSE frame read `at` after the hello; true once the snapshot's last page is in.
    fn frame(&mut self, frame: &[u8], at: Duration) -> Res<bool> {
        if find(frame, b"\"t\":\"snapshot\"").is_none() || find(frame, b"\"topic\":\"list\"").is_none() {
            return Ok(false);
        }
        let data = frame.split(|b| *b == b'\n').find_map(|l| l.strip_prefix(b"data: ")).ok_or("a snapshot frame with no data")?;
        let msgs: Vec<Value> = serde_json::from_slice(data).map_err(said("the snapshot frame"))?;
        self.bytes += frame.len();
        for snap in msgs.iter().filter(|m| m["t"] == "snapshot" && m["topic"] == "list") {
            let seq = snap["seq"].as_u64().ok_or("a list page with no seq")?;
            if self.seq.is_some_and(|s| s != seq) {
                return Err(format!("list pages at seq {} and {seq}: a client would start over", self.seq.unwrap_or(0)));
            }
            self.seq = Some(seq);
            self.pages += 1;
            let rows = snap["rows"].as_array().map(Vec::as_slice).unwrap_or_default();
            let mut tasks = 0;
            for r in rows.iter().filter(|r| r["e"] == "session") {
                let id = r["id"].as_str().unwrap_or_default().to_owned();
                if !self.ids.insert(id.clone()) {
                    return Err(format!("task {id} came in two pages of the list"));
                }
                tasks += 1;
            }
            self.first.get_or_insert((at, tasks));
            if snap["more"] != true {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn read(self, all: Duration) -> Res<ListRead> {
        let (took, first) = self.first.ok_or("no list page")?;
        Ok(ListRead { took, all, first, tasks: self.ids.len(), pages: self.pages, bytes: self.bytes })
    }
}

/// A sync stream (`GET /api/events`) read frame by frame: the head, the chunked body and whole SSE frames.
struct Sse {
    s: TcpStream,
    raw: Vec<u8>,
    body: Vec<u8>,
    chunked: Option<Chunked>,
    head_done: bool,
    /// where the next frame starts, and where the search for its end goes on from
    /// (searched from where the last look stopped: rescanning a 4.5 MB frame on every read would time the bench)
    scan: usize,
    searched: usize,
    buf: Vec<u8>,
}

impl Sse {
    /// Connects and sends a v2 hello with `query` (`client_id`, `fresh`, …); `wait` is each read's limit.
    fn open(port: u16, query: &str, wait: Duration) -> Res<Self> {
        let mut s = TcpStream::connect(addr(port)).map_err(said("connect"))?;
        s.set_nodelay(true).map_err(said("nodelay"))?;
        s.set_read_timeout(Some(wait)).map_err(said("timeout"))?;
        write!(
            s,
            "GET /api/events?proto_min=2&proto_max=2&build=budgets&{query} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\n\r\n"
        )
        .map_err(said("send the hello"))?;
        Ok(Self { s, raw: Vec::new(), body: Vec::new(), chunked: None, head_done: false, scan: 0, searched: 0, buf: vec![0u8; 256 * 1024] })
    }

    /// One read: each whole frame it completes goes to `f` (`id`, `event`, `data: [messages]`, without the blank line);
    /// `f` answering true stops there (Ok(true)). Ok(false) also when the read timed out with nothing new.
    fn read(&mut self, f: &mut dyn FnMut(&[u8]) -> Res<bool>) -> Res<bool> {
        let got = match self.s.read(&mut self.buf) {
            Ok(0) => return Err("the event stream closed".into()),
            Ok(n) => n,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return Ok(false),
            Err(e) => return Err(format!("read the stream: {e}")),
        };
        if self.head_done {
            match self.chunked.as_mut() {
                Some(c) => c.feed(&self.buf[..got], &mut self.body)?,
                None => self.body.extend_from_slice(&self.buf[..got]),
            }
        } else {
            self.raw.extend_from_slice(&self.buf[..got]);
            let Some(end) = find(&self.raw, b"\r\n\r\n") else { return Ok(false) };
            let head = String::from_utf8_lossy(&self.raw[..end]).to_ascii_lowercase();
            if !head.starts_with("http/1.1 200") {
                return Err(format!("GET /api/events answered {}", head.lines().next().unwrap_or("")));
            }
            self.head_done = true;
            let rest = self.raw.split_off(end + 4);
            if head.contains("transfer-encoding: chunked") {
                let mut c = Chunked::default();
                c.feed(&rest, &mut self.body)?;
                self.chunked = Some(c);
            } else {
                self.body = rest;
            }
        }
        while let Some(end) = find(&self.body[self.searched..], b"\n\n").map(|e| self.searched + e) {
            let start = self.scan;
            (self.scan, self.searched) = (end + 2, end + 2);
            if f(&self.body[start..end])? {
                return Ok(true);
            }
        }
        self.searched = self.body.len().saturating_sub(1).max(self.scan);
        // a long-lived stream drops what it has read (the list's reads never get here: it ends at the snapshot)
        if self.scan > 1 << 20 {
            self.body.drain(..self.scan);
            (self.searched, self.scan) = (self.searched - self.scan, 0);
        }
        Ok(false)
    }
}

/// The UI's first read: a v2 hello from `client` asking for `list` fresh, timed from connect to each page's last byte.
fn list_once(port: u16, client: &str) -> Res<ListRead> {
    let t0 = Instant::now();
    let mut sse = Sse::open(port, &format!("client_id={client}&fresh=list"), Duration::from_secs(10))?;
    let mut pages = Pages::default();
    let mut done = None;
    loop {
        let mut on = |frame: &[u8]| -> Res<bool> {
            let at = t0.elapsed();
            let last = pages.frame(frame, at)?;
            if last {
                done = Some(at);
            }
            Ok(last)
        };
        sse.read(&mut on)?;
        if let Some(at) = done {
            return pages.read(at);
        }
        if t0.elapsed() > Duration::from_secs(30) {
            return Err("no list snapshot in 30 s (heartbeats only)".into());
        }
    }
}

// ---------- commands ----------

/// An HTTP/1.1 chunked body read to its end.
fn dechunk(mut raw: &[u8]) -> Res<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let end = find(raw, b"\r\n").ok_or("a chunk with no size line")?;
        let line = String::from_utf8_lossy(&raw[..end]);
        let size = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16).map_err(said("a chunk size"))?;
        if size == 0 {
            return Ok(out);
        }
        let data = raw.get(end + 2..end + 2 + size).ok_or("a chunk cut short")?;
        out.extend_from_slice(data);
        raw = raw.get(end + 4 + size..).unwrap_or_default();
    }
}

/// `POST /api/cmd`: the command's result, or its error as text.
fn cmd(port: u16, name: &str, args: &Value) -> Res<Value> {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let key = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let body = serde_json::json!({ "cmd": name, "args": args }).to_string();
    let mut s = TcpStream::connect(addr(port)).map_err(said("connect"))?;
    s.set_read_timeout(Some(Duration::from_secs(30))).map_err(said("timeout"))?;
    write!(
        s,
        "POST /api/cmd HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nIdempotency-Key: budgets-{}-{key}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        std::process::id(),
        body.len()
    )
    .map_err(said(name))?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(said(name))?;
    let end = find(&raw, b"\r\n\r\n").ok_or_else(|| format!("{name}: no answer"))?;
    let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
    let body = if head.contains("transfer-encoding: chunked") { dechunk(&raw[end + 4..])? } else { raw[end + 4..].to_vec() };
    let v: Value = serde_json::from_slice(&body).map_err(said(name))?;
    if v["ok"] == true { Ok(v["result"].clone()) } else { Err(format!("{name}: {v}")) }
}

/// A task's last turn: its id and status.
fn last_turn(port: u16, id: &str) -> Res<(String, String)> {
    let turns = cmd(port, "runtime.turns", &serde_json::json!({ "id": id }))?;
    let t = turns.as_array().and_then(|a| a.last()).ok_or_else(|| format!("task {id} has no turn"))?;
    Ok((t["id"].as_str().unwrap_or_default().to_owned(), t["status"].as_str().unwrap_or_default().to_owned()))
}

/// The JSON bytes of the steps a turn kept (the runtime bench's payload: `write_amplification_2mb_session_in_5000_events`).
fn kept(port: u16, id: &str, turn: &str) -> Res<(u64, usize)> {
    let (mut bytes, mut steps, mut since) = (0, 0, None::<u64>);
    loop {
        let mut args = serde_json::json!({ "id": id, "turnId": turn });
        if let Some(s) = since {
            args["sinceSeq"] = s.into();
        }
        let page = cmd(port, "runtime.activity", &args)?;
        let items = page["items"].as_array().map(Vec::as_slice).unwrap_or_default();
        if items.is_empty() {
            return Ok((bytes, steps));
        }
        for i in items {
            // a step flattens its activity beside `seq` and `t`: only the activity is payload, as in the bench
            let step: Step = serde_json::from_value(i.clone()).map_err(said("a kept step"))?;
            bytes += serde_json::to_string(&step.activity).map_err(said("a kept step"))?.len() as u64;
            steps += 1;
        }
        let seq = page["seq"].as_u64();
        if seq.is_none() || seq == since {
            return Ok((bytes, steps));
        }
        since = seq;
    }
}

// ---------- patch latency and write amplification ----------

fn now_us() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX))
}

/// Each `~stamp:<pid>:<n>:<µs>~` the fake agent wrote that `frame` carries: ((pid, n), when it was written).
fn stamps(frame: &[u8]) -> Vec<((u32, u64), u64)> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = find(&frame[at..], b"~stamp:") {
        let from = at + i + b"~stamp:".len();
        let Some(end) = frame[from..].iter().position(|b| *b == b'~').map(|e| from + e) else { break };
        let mut parts = std::str::from_utf8(&frame[from..end]).unwrap_or_default().split(':');
        let (pid, n, us) = (parts.next(), parts.next(), parts.next());
        if let (Some(Ok(pid)), Some(Ok(n)), Some(Ok(us)), None) = (pid.map(str::parse), n.map(str::parse), us.map(str::parse), parts.next())
        {
            out.push(((pid, n), us));
        }
        at = end + 1;
    }
    out
}

/// What one client saw of the stream.
#[derive(Default)]
struct Seen {
    /// each stamp's first sight: its latency in µs (a later copy, a chunk row growing, never counts again)
    first: std::collections::HashMap<(u32, u64), u64>,
    frames: usize,
    bytes: usize,
}

/// The topics a client has its first look at (each snapshot's last page), from one frame.
fn snapshotted(frame: &[u8], done: &mut std::collections::HashSet<String>) -> Res<()> {
    if find(frame, b"\"t\":\"snapshot\"").is_none() {
        return Ok(());
    }
    let data = frame.split(|b| *b == b'\n').find_map(|l| l.strip_prefix(b"data: ")).ok_or("a snapshot frame with no data")?;
    let msgs: Vec<Value> = serde_json::from_slice(data).map_err(said("a snapshot frame"))?;
    for m in msgs.iter().filter(|m| m["t"] == "snapshot" && m["more"] != true) {
        done.insert(m["topic"].as_str().unwrap_or_default().to_owned());
    }
    Ok(())
}

struct Shared {
    stop: std::sync::atomic::AtomicBool,
    ready: std::sync::atomic::AtomicUsize,
    /// stamps seen, over every client
    seen: std::sync::atomic::AtomicUsize,
    /// this round began (µs): a stamp written before it is an earlier round's (a row that carries old text), never a sample
    since: u64,
}

/// One UI (`id`): `list` and every running task's topic, fresh; counts itself ready once each first look is in, then
/// times every stamp it is sent until told to stop.
fn client(port: u16, id: &str, topics: &[String], sh: &Shared) -> Res<Seen> {
    use std::sync::atomic::Ordering::Relaxed;
    let mut sse = Sse::open(port, &format!("client_id={id}&fresh={}", topics.join(",")), Duration::from_millis(50))?;
    let (mut seen, mut done, mut ready) = (Seen::default(), std::collections::HashSet::new(), false);
    while !sh.stop.load(Relaxed) {
        let mut on = |frame: &[u8]| -> Res<bool> {
            let now = now_us();
            seen.frames += 1;
            seen.bytes += frame.len() + 2;
            for (key, at) in stamps(frame).into_iter().filter(|(_, at)| *at >= sh.since) {
                if let std::collections::hash_map::Entry::Vacant(e) = seen.first.entry(key) {
                    e.insert(now.saturating_sub(at));
                    sh.seen.fetch_add(1, Relaxed);
                }
            }
            if !ready {
                snapshotted(frame, &mut done)?;
                if topics.iter().all(|t| done.contains(t)) {
                    ready = true;
                    sh.ready.fetch_add(1, Relaxed);
                }
            }
            Ok(false)
        };
        sse.read(&mut on)?;
    }
    Ok(seen)
}

fn file_len(p: &Path) -> u64 {
    std::fs::metadata(p).map_or(0, |m| m.len())
}

struct Stream {
    /// every client's latency to every stamp it got, µs
    lat: Vec<u64>,
    /// stamps × clients
    expected: usize,
    frames: usize,
    bytes: usize,
    /// the gate opening → the last turn's end
    took: Duration,
    /// WAL bytes written while the stream ran (a read held open from before it, so nothing was checkpointed away);
    /// 0 on a round that doesn't pin it (only the last round measures write amplification, ADR-0089)
    wal: u64,
    /// the database file's size before (its growth is read once the server stopped)
    db_before: u64,
    /// the steps the 10 turns kept, as JSON, and how many
    payload: u64,
    steps: usize,
}

/// Round `round` of the stream: 10 new tasks gated before their first line, 4 new clients on their topics, then the
/// gate opens: each line the fake writes is stamped, and each client's first sight of a stamp is its latency. With
/// `pin`, the store's WAL is pinned meanwhile and the turns' kept steps are read (write amplification).
fn stream(b: &Bench, port: u16, projects: &[String], round: usize, pin: bool) -> Res<Stream> {
    use std::sync::atomic::Ordering::Relaxed;
    // each round its own gate file: the last round's is already open
    let gate = b.dir.join("gates").join(format!("go{round}"));
    let since = now_us();
    let mut ids = Vec::new();
    for i in 0..RUNNING {
        let prompt = format!("gate go{round} stream");
        let args = serde_json::json!({ "prompt": prompt, "cwd": projects[i % projects.len()], "agent": "claude", "worktree": false });
        let made = cmd(port, "runtime.create", &args)?;
        ids.push(made["id"].as_str().ok_or("runtime.create gave no id")?.to_owned());
    }
    let t0 = Instant::now();
    for id in &ids {
        while last_turn(port, id)?.1 != "running" {
            if t0.elapsed() > Duration::from_secs(30) {
                return Err(format!("task {id} never started running: {:?}", last_turn(port, id)?));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let mut topics = vec!["list".to_owned()];
    topics.extend(ids.iter().map(|id| format!("session:{id}")));
    let sh = Shared { stop: false.into(), ready: 0.into(), seen: 0.into(), since };
    let stamps = b.stream.stamps();
    let expected = RUNNING * stamps * CLIENTS;
    let (db, wal) = (b.home().join("wisp.db"), b.home().join("wisp.db-wal"));
    std::thread::scope(|scope| -> Res<Stream> {
        let clients: Vec<_> = (0..CLIENTS)
            .map(|i| {
                scope.spawn({
                    let (topics, sh, id) = (&topics, &sh, format!("budgets-s{round}-{i}"));
                    move || client(port, &id, topics, sh)
                })
            })
            .collect();
        let run = || -> Res<(Duration, u64, u64)> {
            let t0 = Instant::now();
            while sh.ready.load(Relaxed) < CLIENTS {
                if clients.iter().any(|c| c.is_finished()) || t0.elapsed() > Duration::from_secs(60) {
                    return Err(format!("{} of {CLIENTS} clients had their first look", sh.ready.load(Relaxed)));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            if !pin {
                std::fs::write(&gate, "go").map_err(said("open the gate"))?;
                let opened = Instant::now();
                wait_turns(port, &ids, opened, b.stream.ms)?;
                let took = opened.elapsed();
                last_patches(&sh, expected);
                return Ok((took, 0, 0));
            }
            // the WAL from here on: emptied into the database, then held by a read so no checkpoint takes it back
            // (as the runtime bench pins it; across processes, SQLite's shared memory makes it the same)
            let pin = wisp_store::rusqlite::Connection::open(&db).map_err(said("open the store to pin its WAL"))?;
            // each try answers busy at once (rusqlite waits 5 s by default): 100 tries are 2 s, not 500
            pin.busy_timeout(Duration::ZERO).map_err(said("pin the WAL"))?;
            let mut emptied = false;
            for _ in 0..100 {
                let busy: i64 = pin.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0)).map_err(said("checkpoint"))?;
                if busy == 0 {
                    emptied = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            if !emptied {
                return Err("the WAL never emptied: the server kept it busy for 2 s while idle".into());
            }
            pin.execute_batch("BEGIN").map_err(said("pin the WAL"))?;
            let _: i64 = pin.query_row("SELECT count(*) FROM runtime_session", [], |r| r.get(0)).map_err(said("pin the WAL"))?;
            let (wal0, db0) = (file_len(&wal), file_len(&db));
            std::fs::write(&gate, "go").map_err(said("open the gate"))?;
            let opened = Instant::now();
            wait_turns(port, &ids, opened, b.stream.ms)?;
            let took = opened.elapsed();
            last_patches(&sh, expected);
            let wal1 = file_len(&wal);
            // what the stream wrote, frame by frame, for `--keep` (where the bytes went: which pages, how many commits)
            let _ = std::fs::copy(&wal, b.dir.join("stream.wal"));
            pin.execute_batch("COMMIT").map_err(said("let the WAL go"))?;
            Ok((took, wal1.saturating_sub(wal0), db0))
        };
        let ran = run();
        // the gate opens however the run went: a fake still waiting on it would poll on after the bench is gone
        let _ = std::fs::write(&gate, "go");
        sh.stop.store(true, Relaxed);
        let mut seen = Vec::new();
        for c in clients {
            seen.push(c.join().map_err(|_| "a client panicked".to_owned())??);
        }
        let (took, wal, db_before) = ran?;
        let (mut payload, mut steps) = (0, 0);
        for id in ids.iter().filter(|_| pin) {
            let (turn, _) = last_turn(port, id)?;
            let (b, n) = kept(port, id, &turn)?;
            (payload, steps) = (payload + b, steps + n);
        }
        Ok(Stream {
            // only stamps a task writes (n under `stamps`): a stray one can never stand in for a patch that went missing
            lat: seen
                .iter()
                .flat_map(|s| s.first.iter().filter(|((_, n), _)| usize::try_from(*n).is_ok_and(|n| n < stamps)).map(|(_, l)| *l))
                .collect(),
            expected,
            frames: seen.iter().map(|s| s.frames).sum(),
            bytes: seen.iter().map(|s| s.bytes).sum(),
            took,
            wal,
            db_before,
            payload,
            steps,
        })
    })
}

/// Waits until the WAL (its length and its modified time: a WAL reset after a checkpoint is rewritten from its start,
/// so the length alone can stand still while it's written) is unchanged for 1.5 s, at most 15 s; how long that took.
fn wal_quiet(wal: &Path) -> Duration {
    let t0 = Instant::now();
    let look = || std::fs::metadata(wal).ok().map(|m| (m.len(), m.modified().ok()));
    let (mut last, mut since) = (look(), Instant::now());
    while since.elapsed() < Duration::from_millis(1_500) && t0.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(100));
        let now = look();
        if now != last {
            (last, since) = (now, Instant::now());
        }
    }
    t0.elapsed()
}

/// Every task's turn ended (none `running`), or an error a minute past the stream's length.
fn wait_turns(port: u16, ids: &[String], opened: Instant, ms: u64) -> Res<()> {
    for id in ids {
        while last_turn(port, id)?.1 == "running" {
            if opened.elapsed() > Duration::from_millis(ms) + Duration::from_secs(60) {
                return Err(format!("task {id}'s turn never ended"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(())
}

/// The last patches: every stamp to every client, or 5 s.
fn last_patches(sh: &Shared, expected: usize) {
    let ended = Instant::now();
    while sh.seen.load(std::sync::atomic::Ordering::Relaxed) < expected && ended.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The database's growth once the server stopped: its WAL emptied into it first.
fn db_after(home: &Path) -> Res<u64> {
    let c = wisp_store::rusqlite::Connection::open(home.join("wisp.db")).map_err(said("open the store"))?;
    let _: i64 = c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0)).map_err(said("checkpoint"))?;
    drop(c);
    Ok(file_len(&home.join("wisp.db")))
}

/// spikes/events' append rate on a store of its own in the bench's folder.
fn append(dir: &Path) -> Res<f64> {
    std::fs::create_dir_all(dir).map_err(said("create the append bench's folder"))?;
    let st = Store::open(Location::Path(dir.join("wisp.db")), Arc::new(SystemClock)).map_err(said("open the store"))?;
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(said("a tokio runtime"))?;
    let rate = rt.block_on(events_bench::append_rate(&st));
    st.close();
    Ok(rate)
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

/// An idle server's RSS before and after the list's reads, with its parts.
struct Defaults {
    arenas: usize,
    before: Rss,
    after: Rss,
    parts: Option<mem::Parts>,
}

/// The idle measurement again on glibc's defaults (`MALLOC_ARENA_MAX` at glibc's own 8 × cores, so wisp's
/// `tune_heap` leaves malloc alone): what ADR-0087's settings save, in the same CI run. Reported, not gated.
fn glibc_defaults(b: &Bench) -> Res<Defaults> {
    let arenas = 8 * std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let (s, _) = b.boot_with("glibc-defaults", &[("MALLOC_ARENA_MAX", arenas.to_string())])?;
    let before = settle(s.child.id())?;
    // as many reads as the gated idle server takes
    for _ in 0..=(ROUNDS * (LIST_WARMUP + LIST_PER_ROUND)) {
        let r = list_once(s.port, "budgets")?;
        if r.tasks != TASKS {
            return Err(format!("its list snapshot had {} tasks, not {TASKS}", r.tasks));
        }
    }
    let after = settle(s.child.id())?;
    let parts = mem::parts(s.child.id());
    println!(
        "glibc's defaults (MALLOC_ARENA_MAX={arenas}, tune_heap off): {:.1} MB before the list, {:.1} MB settled after it",
        mb(before.kb),
        mb(after.kb)
    );
    if let Some(p) = &parts {
        println!("  where: {}", p.line());
    }
    Ok(Defaults { arenas, before, after, parts })
}

// ---------- where the list's time goes (reported, not gated) ----------

struct Breakdown {
    /// `changed_since("list", 0)`: what a first look replayed before `list` had a paged source (ADR-0081)
    feed: Duration,
    /// every task in one query of the session table, as JSON
    query: Duration,
    /// the first page's query (the 100 latest unarchived, on the `(archived, updated_at)` index), as JSON
    page: Duration,
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
    let first = Query::new().eq("archived", false).desc("updated_at").desc("id").limit(100);
    let page = median(&mut || store.query_json("session", &first).map(drop).map_err(said("query the first page")))?;
    store.close();
    Ok(Breakdown { feed, query, page, encode, rows })
}

// ---------- the runner's calibration and the rounds' verdict (ADR-0089) ----------

/// A fixed piece of work in the bench's own process, nothing of wisp's in it: 200 one-byte round trips to an echo
/// thread over loopback TCP (each a wake-up of another thread, as every request wisp answers is: a CPU-only loop fits in
/// one time slice and never sees a busy runner, measured: 6.7 ms with every core taken, as idle), then 4 passes of
/// xorshift over a 64 KB buffer, each written to `scratch` and read back (CPU and file syscalls). It tracks the runner only.
fn workload(scratch: &Path) -> Res<Duration> {
    const TRIPS: usize = 200;
    let l = TcpListener::bind("127.0.0.1:0").map_err(said("calibrate"))?;
    let at = l.local_addr().map_err(said("calibrate"))?;
    // connected before the echo thread exists (the listen backlog takes it): a failed connect leaves no thread waiting
    let mut c = TcpStream::connect(at).map_err(said("calibrate"))?;
    let echo = std::thread::spawn(move || -> std::io::Result<()> {
        let (mut c, _) = l.accept()?;
        c.set_nodelay(true)?;
        let mut b = [0u8; 1];
        for _ in 0..TRIPS {
            c.read_exact(&mut b)?;
            c.write_all(&b)?;
        }
        Ok(())
    });
    c.set_nodelay(true).map_err(said("calibrate"))?;
    c.set_read_timeout(Some(Duration::from_secs(10))).map_err(said("calibrate"))?;
    let t = Instant::now();
    let mut b = [7u8; 1];
    for _ in 0..TRIPS {
        c.write_all(&b).map_err(said("calibrate"))?;
        c.read_exact(&mut b).map_err(said("calibrate"))?;
    }
    let mut x = 0x9E37_79B9_7F4A_7C15_u64;
    let mut buf = vec![0u8; 64 * 1024];
    for i in 0..4 {
        for b in &mut buf {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = x.to_le_bytes()[0];
        }
        std::fs::write(scratch, &buf).map_err(said("calibrate"))?;
        let back = std::fs::read(scratch).map_err(said("calibrate"))?;
        x ^= u64::from(back.get(i).copied().unwrap_or(0));
    }
    std::hint::black_box(x);
    let took = t.elapsed();
    echo.join().map_err(|_| "the calibration's echo thread panicked".to_owned())?.map_err(said("calibrate"))?;
    Ok(took)
}

/// The runner's speed now: the median of `CALIB_RUNS` workloads, in ms.
fn calibrate(scratch: &Path) -> Res<f64> {
    let mut v = (0..CALIB_RUNS).map(|_| workload(scratch).map(ms)).collect::<Res<Vec<f64>>>()?;
    Ok(upper_median(&mut v))
}

/// The middle value (the upper one of an even count: the slower, never the kinder); no values is infinite, a miss.
fn upper_median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(f64::INFINITY)
}

/// A timed budget taken in rounds: each round's value (ms) and the runner's calibration around it (the slower of the
/// two brackets, ms), index for index.
#[derive(Debug, Clone, Default)]
struct Rounds {
    values: Vec<f64>,
    calib: Vec<f64>,
    /// the share of CPU time the hypervisor took during each round, % (Linux `/proc/stat`; reported, not judged)
    steal: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Pass,
    /// over budget with most rounds run on an undisturbed runner: wisp is slower
    Miss,
    /// over budget, but most rounds ran while the runner itself was slow: this run can't tell (a failure, not a pass)
    Noisy,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Judged {
    verdict: Verdict,
    /// the gated value: the median of the clean rounds while they are most, else of every round
    value: f64,
    clean: usize,
    rounds: usize,
    /// the slowest bracket over `usual`
    worst: f64,
}

impl Rounds {
    fn push(&mut self, value: f64, calib: f64) {
        self.values.push(value);
        self.calib.push(calib);
    }

    /// Round `i` ran while the runner was over `NOISY`× its usual speed (or the calibration is unknown).
    fn disturbed(&self, i: usize, usual: f64) -> bool {
        let known = usual.is_finite() && usual > 0.0;
        self.calib.get(i).is_none_or(|c| !known || !c.is_finite() || *c > NOISY * usual)
    }

    /// The verdict on `budget` (lower is better), with `usual` the runner's undisturbed calibration (the job's fastest).
    /// Noise only ever slows a measure, so a pass is a pass however noisy the runner was; a disturbed round is left out
    /// of the median while the clean rounds are most (the calibration is wisp-free, so this never hides a regression:
    /// it slows the clean rounds too); over budget with most rounds disturbed is `Noisy`, never `Pass`.
    fn judge(&self, usual: f64, budget: f64) -> Judged {
        let n = self.values.len();
        let clean: Vec<f64> = (0..n).filter(|i| !self.disturbed(*i, usual)).map(|i| self.values[i]).collect();
        let most = clean.len() > n / 2;
        let value = if most { upper_median(&mut clean.clone()) } else { upper_median(&mut self.values.clone()) };
        let worst = self.calib.iter().copied().fold(0.0, f64::max) / usual;
        let verdict = if n > 0 && value <= budget {
            Verdict::Pass
        } else if n > 0 && !most {
            Verdict::Noisy
        } else {
            Verdict::Miss
        };
        Judged { verdict, value, clean: clean.len(), rounds: n, worst }
    }

    /// `a · b† · c` in ms, † on a disturbed round
    fn cell(&self, usual: f64, decimals: usize) -> String {
        (0..self.values.len())
            .map(|i| format!("{:.decimals$}{}", self.values[i], if self.disturbed(i, usual) { "†" } else { "" }))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// The machine's CPU time so far, `/proc/stat`'s first line: (steal, every kind but guests) in ticks; None off Linux.
fn cpu_ticks() -> Option<(u64, u64)> {
    let s = std::fs::read_to_string("/proc/stat").ok()?;
    let v: Vec<u64> = s.lines().next()?.split_whitespace().skip(1).filter_map(|t| t.parse().ok()).collect();
    Some((*v.get(7)?, v.iter().take(8).sum()))
}

/// The steal share between two readings, %; None without both, or when no tick passed.
fn steal_pct(a: Option<(u64, u64)>, b: Option<(u64, u64)>) -> Option<f64> {
    let ((s0, t0), (s1, t1)) = (a?, b?);
    #[allow(clippy::cast_precision_loss)]
    let pct = (t1 > t0).then(|| s1.saturating_sub(s0) as f64 * 100.0 / (t1 - t0) as f64);
    pct
}

/// `from` copied into `to`, folders and all (the migrated home the cold starts boot on).
fn copy_dir(from: &Path, to: &Path) -> Res<()> {
    std::fs::create_dir_all(to).map_err(said("copy the home"))?;
    for e in std::fs::read_dir(from).map_err(said("copy the home"))? {
        let e = e.map_err(said("copy the home"))?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        let kind = e.file_type().map_err(said("copy the home"))?;
        if kind.is_dir() {
            copy_dir(&src, &dst)?;
        } else if kind.is_file() {
            std::fs::copy(&src, &dst).map_err(said("copy the home"))?;
        }
    }
    Ok(())
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
    /// every restart, over every round (reported)
    cold: Vec<Duration>,
    /// each round's median of `COLD_PER_ROUND` restarts (gated: the median of the rounds)
    cold_rounds: Rounds,
    rss_before: Rss,
    rss: Rss,
    /// where the RSS is, before the list and settled (`mem`; Linux has every part, macOS the threads)
    parts_before: Option<mem::Parts>,
    parts: Option<mem::Parts>,
    /// the same idle server on glibc's own malloc settings (Linux): wisp's `tune_heap` off (ADR-0087)
    defaults: Option<Defaults>,
    /// to the first page, every timed read of every round (reported)
    list: Vec<Duration>,
    /// each round's median to the first page (gated: the median of the rounds)
    list_rounds: Rounds,
    /// to the last page (reported)
    list_all: Vec<Duration>,
    /// tasks in the first page, pages in all
    list_first: usize,
    list_pages: usize,
    list_bytes: usize,
    why: Option<Breakdown>,
    /// every round's samples together (reported; a patch a client never got, in any round, is a miss)
    patch: Patch,
    /// each round's p99 (gated: the median of the rounds)
    patch_rounds: Rounds,
    /// every calibration the job took (ms): the fastest is the runner's usual speed
    calibrations: Vec<f64>,
    append_per_s: f64,
    amp: Amp,
    /// what each running task streamed
    stream: Shape,
}

/// The patch latency, as the report needs it.
struct Patch {
    /// each client's latency to each stamp it got, µs
    lat: Vec<u64>,
    expected: usize,
    frames: usize,
    bytes: usize,
    took: Duration,
}

impl Patch {
    fn missing(&self) -> usize {
        self.expected.saturating_sub(self.lat.len())
    }
    /// min, p50, p99 and max in ms (index n × q, as spikes/events takes its p99)
    fn spread(&self) -> (f64, f64, f64, f64) {
        let mut v = self.lat.clone();
        v.sort_unstable();
        #[allow(clippy::cast_precision_loss)]
        let at = |q: usize| v.get((v.len() * q / 100).min(v.len().saturating_sub(1))).map_or(0.0, |us| *us as f64 / 1000.0);
        (at(0), at(50), at(99), at(100))
    }
}

/// Write amplification: what the store wrote over the activity the turns kept.
struct Amp {
    wal: u64,
    db: u64,
    payload: u64,
    steps: usize,
}

impl Amp {
    /// ×; no payload at all is infinite (a miss), never a pass
    fn x(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let x = if self.payload == 0 { f64::INFINITY } else { (self.wal + self.db) as f64 / self.payload as f64 };
        x
    }
}

/// The three timed budgets, by name, unit decimals and budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Timed {
    Cold,
    List,
    Patch,
}

impl Timed {
    const ALL: [Self; 3] = [Self::Cold, Self::List, Self::Patch];
    fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold start",
            Self::List => "task list",
            Self::Patch => "patch latency p99",
        }
    }
    fn budget(self) -> f64 {
        match self {
            Self::Cold => COLD_START_MS,
            Self::List => LIST_MS,
            Self::Patch => PATCH_P99_MS,
        }
    }
}

impl Report {
    /// The runner's undisturbed speed: the job's fastest calibration (ms)
    fn usual(&self) -> f64 {
        self.calibrations.iter().copied().fold(f64::INFINITY, f64::min)
    }
    fn rounds(&self, t: Timed) -> &Rounds {
        match t {
            Timed::Cold => &self.cold_rounds,
            Timed::List => &self.list_rounds,
            Timed::Patch => &self.patch_rounds,
        }
    }
    fn judge(&self, t: Timed) -> Judged {
        self.rounds(t).judge(self.usual(), t.budget())
    }

    /// The timed budgets this run couldn't judge: over budget while most of their rounds ran on a slowed runner.
    fn noisy(&self) -> Vec<String> {
        let usual = self.usual();
        Timed::ALL
            .into_iter()
            .filter(|t| *t != Timed::Patch || self.patch.missing() == 0)
            .map(|t| (t, self.judge(t)))
            .filter(|(_, j)| j.verdict == Verdict::Noisy)
            .map(|(t, j)| {
                format!(
                    "too noisy to judge {}: {} of {} rounds ran while the runner's calibration was over {NOISY}× its usual \
                     {usual:.2} ms (up to {:.1}×), and their median {:.2} ms is over the {} ms budget: not a pass, not a \
                     verdict on wisp",
                    t.name(),
                    j.rounds - j.clean,
                    j.rounds,
                    j.worst,
                    j.value,
                    t.budget()
                )
            })
            .collect()
    }

    /// 1 on a miss, else 3 when a timed budget couldn't be judged, else 0 (2, it can't run, is `main`'s)
    fn exit_code(&self) -> i32 {
        if !self.over().is_empty() {
            1
        } else if !self.noisy().is_empty() {
            3
        } else {
            0
        }
    }

    fn over(&self) -> Vec<String> {
        let mut over = Vec::new();
        let cold = self.judge(Timed::Cold);
        if cold.verdict == Verdict::Miss {
            over.push(format!("cold start {:.1} ms (median of {} rounds) > {COLD_START_MS} ms", cold.value, cold.clean));
        }
        if !self.rss.settled {
            over.push(format!("idle RSS never settled in 30 s (last {:.1} MB, budget {IDLE_RSS_MB} MB)", mb(self.rss.kb)));
        } else if mb(self.rss.kb) > IDLE_RSS_MB {
            over.push(format!("idle RSS {:.1} MB (settled) > {IDLE_RSS_MB} MB", mb(self.rss.kb)));
        }
        let list = self.judge(Timed::List);
        if list.verdict == Verdict::Miss {
            over.push(format!("task list {:.2} ms (median of {} rounds) > {LIST_MS} ms", list.value, list.clean));
        }
        let patch = self.judge(Timed::Patch);
        if self.patch.missing() > 0 {
            over.push(format!("{} of {} patches never reached their client", self.patch.missing(), self.patch.expected));
        } else if patch.verdict == Verdict::Miss {
            over.push(format!("patch latency p99 {:.1} ms (median of {} rounds) > {PATCH_P99_MS} ms", patch.value, patch.clean));
        }
        if self.append_per_s < APPEND_PER_S {
            over.push(format!("durable append {:.0} events/s < {APPEND_PER_S} events/s", self.append_per_s));
        }
        if self.amp.x() > WRITE_AMPLIFICATION {
            over.push(format!("write amplification {:.1}× > {WRITE_AMPLIFICATION}×", self.amp.x()));
        }
        over
    }

    /// "Where the idle RSS is (smaps, MB): before the list …; settled …", then a blank line; nothing without parts.
    fn where_rss(&self) -> String {
        let line = |p: &Option<mem::Parts>| p.as_ref().map(mem::Parts::line).filter(|l| !l.is_empty());
        match (line(&self.parts_before), line(&self.parts)) {
            (None, None) => String::new(),
            (b, a) => format!(
                "Where the idle RSS is (`/proc/<pid>/smaps` on Linux): before the list {}; settled after it {}.\n{}",
                b.as_deref().unwrap_or("unread"),
                a.as_deref().unwrap_or("unread"),
                self.defaults.as_ref().map_or_else(String::new, |d| format!(
                    "\nOn glibc's own malloc settings (`MALLOC_ARENA_MAX={}`, ADR-0087's `tune_heap` off), the same idle server: \
                     {:.1} MB before the list, {:.1} MB settled after it{}; where: {}.\n",
                    d.arenas,
                    mb(d.before.kb),
                    mb(d.after.kb),
                    if d.after.settled { "" } else { " (never settled)" },
                    d.parts.as_ref().map_or_else(|| "unread".to_owned(), mem::Parts::line)
                ))
            ),
        }
    }

    fn summary(&self) -> String {
        let mark = |ok: bool| if ok { "✅" } else { "❌" };
        let verdict = |j: Judged| match j.verdict {
            Verdict::Pass => "✅",
            Verdict::Miss => "❌",
            Verdict::Noisy => "⚠️ too noisy to judge",
        };
        let usual = self.usual();
        let (cold, list, patch) = (self.judge(Timed::Cold), self.judge(Timed::List), self.judge(Timed::Patch));
        let (c0, _, _, c100) = spread(&self.cold);
        let (l0, l50, _, l100) = spread(&self.list);
        let (a0, a50, a95, a100) = spread(&self.list_all);
        let rss_ok = mb(self.rss.kb) <= IDLE_RSS_MB && self.rss.settled;
        let (p0, p50, p99, pmax) = self.patch.spread();
        let patch_mark = if self.patch.missing() > 0 { "❌" } else { verdict(patch) };
        let of =
            |j: Judged| if j.clean == j.rounds { format!("{} rounds", j.rounds) } else { format!("{} of {} rounds", j.clean, j.rounds) };
        let steal =
            |r: &Rounds| r.steal.iter().map(|s| s.map_or_else(|| "?".to_owned(), |s| format!("{s:.1}"))).collect::<Vec<_>>().join(" · ");
        let stole = [&self.cold_rounds, &self.list_rounds, &self.patch_rounds].iter().any(|r| r.steal.iter().any(Option::is_some));
        let (cmin, cmax) =
            (self.calibrations.iter().copied().fold(f64::INFINITY, f64::min), self.calibrations.iter().copied().fold(0.0, f64::max));
        [
            format!("### Server budgets (ADR-0011), `wisp serve` on {TASKS} tasks"),
            String::new(),
            "| | measured | budget | rounds († the runner was slowed) and runs | |".into(),
            "|---|---:|---:|---|---|".into(),
            format!(
                "| cold start → first response | {:.1} ms, median of {} | {COLD_START_MS} ms | each the median of {COLD_PER_ROUND} restarts: {} ms (every restart {c0:.1} – {c100:.1}) | {} |",
                cold.value,
                of(cold),
                self.cold_rounds.cell(usual, 1),
                verdict(cold)
            ),
            format!(
                "| idle RSS | {:.1} MB settled | {IDLE_RSS_MB} MB | after {:.1} s; {:.1} MB before the list was read | {} |",
                mb(self.rss.kb),
                self.rss.after.as_secs_f64(),
                mb(self.rss_before.kb),
                mark(rss_ok)
            ),
            format!(
                "| task list (`fresh=list`, its first page: {} tasks) | {:.2} ms, median of {} | {LIST_MS} ms | each the median of {LIST_PER_ROUND} reads by a fresh client: {} ms (every read {l0:.2} · median {l50:.2} · max {l100:.2}) | {} |",
                self.list_first,
                list.value,
                of(list),
                self.list_rounds.cell(usual, 2),
                verdict(list)
            ),
            format!(
                "| patch latency ({RUNNING} running, {CLIENTS} clients) | {:.1} ms p99, median of {} | {PATCH_P99_MS} ms | each round's p99: {} ms; every round's {} of {} together: min {p0:.1} · p50 {p50:.1} · p99 {p99:.1} · max {pmax:.1} ms | {} |",
                patch.value,
                of(patch),
                self.patch_rounds.cell(usual, 1),
                self.patch.lat.len(),
                self.patch.expected,
                patch_mark
            ),
            format!(
                "| durable append | {:.0} events/s | {APPEND_PER_S} events/s | {} events, 1 000 a transaction (spikes/events) | {} |",
                self.append_per_s,
                events_bench::APPENDED,
                mark(self.append_per_s >= APPEND_PER_S)
            ),
            format!(
                "| write amplification | {:.1}× | {WRITE_AMPLIFICATION}× | WAL {:.2} MB + database {:.2} MB over {:.2} MB kept in {} steps | {} |",
                self.amp.x(),
                mb(self.amp.wal / 1024),
                mb(self.amp.db / 1024),
                mb(self.amp.payload / 1024),
                self.amp.steps,
                mark(self.amp.x() <= WRITE_AMPLIFICATION)
            ),
            String::new(),
            format!(
                "The timed budgets are the median of rounds spread over the job (ADR-0089). The runner's calibration (a fixed \
                 workload in the bench's own process: 200 loopback round trips, CPU and file work; median of {CALIB_RUNS}, \
                 before and after each round): \
                 usual (the job's fastest) {cmin:.2} ms, slowest {cmax:.2} ms ({:.1}×). A round run while it was over \
                 {NOISY}× the usual is marked † and left out of the median while most rounds are clean; over budget with \
                 most rounds marked is *too noisy to judge* (exit 3), never a pass.",
                cmax / cmin
            ),
            if stole {
                format!(
                    "\nThe hypervisor's steal (`/proc/stat`, reported, not judged), % of CPU time per round: cold start {}; list {}; stream {}.",
                    steal(&self.cold_rounds),
                    steal(&self.list_rounds),
                    steal(&self.patch_rounds)
                )
            } else {
                String::new()
            },
            String::new(),
            format!(
                "The stream, {PATCH_ROUNDS} rounds: {RUNNING} new tasks × {} lines ({} a second for {:.0} s, {}) through the fake claude, \
                 {CLIENTS} new clients each on `list` and all {RUNNING} task topics; the last round's turns ended {:.1} s after its gate opened; \
                 the clients read {} frames, {:.0} KB; write amplification is the last round's.",
                self.stream.stamps(),
                self.stream.per_s,
                self.stream.ms as f64 / 1000.0,
                if self.stream.mix == Mix::Agent { "a real agent's steps".to_owned() } else { format!("{TEXT_BYTES} B of text") },
                self.patch.took.as_secs_f64(),
                self.patch.frames,
                self.patch.bytes as f64 / 1024.0
            ),
            String::new(),
            format!(
                "Every task (the snapshot's last page, not gated): {a50:.2} ms median ({a0:.2} · {a95:.2} · {a100:.2} ms), {} pages, {:.0} KB a read.",
                self.list_pages,
                self.list_bytes as f64 / 1024.0,
            ),
            String::new(),
            self.where_rss(),
            format!(
                "Seeded in {:.1} s through wisp.runtime's writer; the first boot (every plugin's migrations) took {:.0} ms and isn't gated.",
                self.seeded.as_secs_f64(),
                ms(self.first_boot)
            ),
            String::new(),
            self.why.as_ref().map_or_else(String::new, |b| {
                format!(
                    "Where the list's time goes (in process, medians of 10): the first page's query {:.2} ms; every task in one query \
                     of the session table as JSON {:.1} ms; the change-feed replay a first look made before ADR-0081 \
                     (`changed_since(\"list\", 0)`) {:.1} ms for {} rows, their JSON {:.1} ms.\n",
                    ms(b.page),
                    ms(b.query),
                    ms(b.feed),
                    b.rows,
                    ms(b.encode)
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
    let b = Bench { dir: dir.to_owned(), wisp: o.wisp.clone(), stream: o.stream };
    if !o.fake.is_file() {
        return Err(format!("no fake agent at {} (cargo build --profile budgets -p wisp-testkit, or --fake=<bin>)", o.fake.display()));
    }
    for d in ["home", "bin", "user", "no-web", "code", "gates"] {
        std::fs::create_dir_all(dir.join(d)).map_err(said("create the bench's folders"))?;
    }
    for (name, _) in FAKES {
        let p = dir.join("bin").join(name);
        if name == "claude" {
            std::fs::copy(&o.fake, &p).map_err(said("copy the fake agent"))?;
            continue;
        }
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
    // the cold starts boot on a copy, so they take turns with the list's rounds while the idle server keeps the home
    copy_dir(&b.home(), &b.cold_home())?;

    // on the home as the gated idle server finds it (before the stream adds to it), and not gated
    let defaults = if cfg!(all(target_os = "linux", target_env = "gnu")) {
        glibc_defaults(&b).inspect_err(|e| eprintln!("server budgets: no glibc-defaults run: {e}")).ok()
    } else {
        None
    };
    let (s, _) = b.boot("idle")?;
    let pid = s.child.id();
    let rss_before = settle(pid)?;
    println!("RSS before any client: {:.1} MB (after {:.1} s)", mb(rss_before.kb), rss_before.after.as_secs_f64());
    let parts_before = mem::parts(pid);
    if let Some(p) = &parts_before {
        println!("  where: {}", p.line());
    }
    let first = list_once(s.port, "budgets")?;
    if first.tasks != TASKS {
        return Err(format!("the list snapshot had {} tasks, not {TASKS}", first.tasks));
    }
    println!("first list read (cold): first page {:.2} ms, every task {:.2} ms", ms(first.took), ms(first.all));

    // ADR-0089: cold start and the list take turns, each round bracketed by the runner's calibration
    let scratch = dir.join("calibration");
    let mut calibrations = Vec::new();
    // each bracket: the calibration (ms) and the machine's CPU ticks (Linux: a round's steal share, reported)
    let mut calib = || -> Res<(f64, Option<(u64, u64)>)> {
        let c = calibrate(&scratch)?;
        calibrations.push(c);
        Ok((c, cpu_ticks()))
    };
    let (mut cold, mut cold_rounds, mut list, mut list_all, mut list_rounds) =
        (Vec::new(), Rounds::default(), Vec::new(), Vec::new(), Rounds::default());
    let mut before = calib()?;
    for r in 0..ROUNDS {
        let mut round = Vec::new();
        for i in 0..COLD_PER_ROUND {
            let (s, took) = b.boot_in(&b.cold_home(), &format!("cold-{r}-{i}"), &[])?;
            drop(s);
            round.push(took);
        }
        let between = calib()?;
        let value = spread(&round).1;
        cold_rounds.push(value, before.0.max(between.0));
        cold_rounds.steal.push(steal_pct(before.1, between.1));
        cold.extend(round);
        let client = format!("budgets-r{r}");
        for _ in 0..LIST_WARMUP {
            list_once(s.port, &client)?;
        }
        let mut round = Vec::new();
        for _ in 0..LIST_PER_ROUND {
            let read = list_once(s.port, &client)?;
            round.push(read.took);
            list_all.push(read.all);
        }
        let after = calib()?;
        let reads = spread(&round).1;
        list_rounds.push(reads, between.0.max(after.0));
        list_rounds.steal.push(steal_pct(between.1, after.1));
        list.extend(round);
        println!(
            "round {}: cold start {value:.1} ms, list {reads:.2} ms (calibration {:.2} · {:.2} · {:.2} ms)",
            r + 1,
            before.0,
            between.0,
            after.0
        );
        before = after;
    }
    let rss = settle(pid)?;
    let parts = mem::parts(pid);
    println!("RSS settled after the list: {:.1} MB", mb(rss.kb));
    if let Some(p) = &parts {
        println!("  where: {}", p.line());
    }
    // on the same server, once its idle numbers are in: 10 new tasks stream to 4 new clients, each round; the last
    // round pins the WAL (write amplification)
    let (mut lat, mut expected, mut frames, mut bytes, mut patch_rounds) = (Vec::new(), 0, 0, 0, Rounds::default());
    let mut last = None;
    let mut before = calib()?;
    for r in 0..PATCH_ROUNDS {
        let pin = r + 1 == PATCH_ROUNDS;
        if pin && r > 0 {
            // what the earlier rounds' turn ends still write (cursors saved within 1 s, ADR-0088) isn't the last round's
            let quiet = wal_quiet(&b.home().join("wisp.db-wal"));
            println!("the WAL was quiet {:.1} s after the last round ended", quiet.as_secs_f64());
        }
        let st = stream(&b, s.port, &projects, r, pin)?;
        let after = calib()?;
        let round = Patch { lat: st.lat.clone(), expected: st.expected, frames: 0, bytes: 0, took: st.took };
        let p99 = round.spread().2;
        println!(
            "stream round {}: patch p99 {p99:.1} ms over {} of {} patches (calibration {:.2} · {:.2} ms)",
            r + 1,
            round.lat.len(),
            round.expected,
            before.0,
            after.0
        );
        patch_rounds.push(p99, before.0.max(after.0));
        patch_rounds.steal.push(steal_pct(before.1, after.1));
        before = after;
        (expected, frames, bytes) = (expected + st.expected, frames + st.frames, bytes + st.bytes);
        lat.extend(st.lat.iter().copied());
        last = Some(st);
    }
    let st = last.ok_or("no stream round")?;
    let patch = Patch { lat, expected, frames, bytes, took: st.took };
    println!(
        "patch latency: p99 {:.1} ms over {} of {} patches; {} steps kept",
        patch.spread().2,
        patch.lat.len(),
        patch.expected,
        st.steps
    );
    drop(s);
    let db = db_after(&b.home())?.saturating_sub(st.db_before);
    // which writers the WAL's frames are (reported, not gated)
    match wal::writers(&b.dir.join("stream.wal"), &b.home().join("wisp.db")) {
        Ok(w) => w.lines().iter().for_each(|l| println!("{l}")),
        Err(e) => eprintln!("server budgets: no WAL breakdown: {e}"),
    }
    let append_per_s = append(&dir.join("append"))?;
    println!("durable append: {append_per_s:.0} events/s");
    let why = breakdown(&b.home()).inspect_err(|e| eprintln!("server budgets: no breakdown: {e}")).ok();
    Ok(Report {
        seeded,
        first_boot,
        cold,
        cold_rounds,
        rss_before,
        rss,
        parts_before,
        parts,
        defaults,
        list,
        list_rounds,
        list_all,
        list_first: first.first,
        list_pages: first.pages,
        list_bytes: first.bytes,
        why,
        patch,
        patch_rounds,
        calibrations,
        append_per_s,
        amp: Amp { wal: st.wal, db, payload: st.payload, steps: st.steps },
        stream: o.stream,
    })
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
    let gh = std::env::var_os("GITHUB_ACTIONS").is_some();
    for o in &r.over() {
        eprintln!("{}OVER BUDGET: {o}", if gh { "::error title=Server budget::" } else { "" });
    }
    for n in &r.noisy() {
        eprintln!("{}NOT JUDGED: {n}", if gh { "::error title=Server budget: runner too noisy::" } else { "" });
    }
    std::process::exit(r.exit_code());
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn report(cold: u64, rss_mb: u64, settled: bool, list_us: u64) -> Report {
        report_all(cold, rss_mb, settled, list_us, 150_000, 20_000.0, 44)
    }

    /// Rounds of `values` (ms), all at the usual calibration (1 ms), but those in `slow` at `slow_x`× it.
    fn rounds(values: &[f64], slow: &[usize], slow_x: f64) -> Rounds {
        let mut r = Rounds::default();
        for (i, v) in values.iter().enumerate() {
            r.push(*v, if slow.contains(&i) { slow_x } else { 1.0 });
        }
        r
    }

    #[allow(clippy::cast_precision_loss)]
    fn report_all(cold: u64, rss_mb: u64, settled: bool, list_us: u64, patch_us: u64, append: f64, amp_x: u64) -> Report {
        let rss = |kb| Rss { kb, after: Duration::from_secs(2), settled };
        Report {
            seeded: Duration::ZERO,
            first_boot: Duration::ZERO,
            cold: vec![Duration::from_millis(cold); 3],
            cold_rounds: rounds(&[cold as f64; 5], &[], 1.0),
            list_rounds: rounds(&[list_us as f64 / 1000.0; 5], &[], 1.0),
            patch_rounds: rounds(&[patch_us as f64 / 1000.0; 3], &[], 1.0),
            calibrations: vec![1.0, 1.2, 1.1],
            rss_before: rss(rss_mb * 1024),
            rss: rss(rss_mb * 1024),
            parts_before: None,
            parts: None,
            defaults: None,
            list: vec![Duration::from_micros(list_us); 3],
            list_all: vec![Duration::from_micros(list_us * 10); 3],
            list_first: 100,
            list_pages: 6,
            list_bytes: 0,
            why: None,
            patch: Patch { lat: vec![patch_us; 100], expected: 100, frames: 0, bytes: 0, took: Duration::ZERO },
            append_per_s: append,
            amp: Amp { wal: amp_x * 1000, db: 0, payload: 1000, steps: 1 },
            stream: STREAM,
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

    fn page(seq: u64, ids: &[&str], more: bool) -> Vec<u8> {
        let rows: Vec<Value> = ids.iter().map(|id| serde_json::json!({ "e": "session", "id": id, "v": {} })).collect();
        let mut m = serde_json::json!({ "t": "snapshot", "topic": "list", "seq": seq, "rows": rows });
        if more {
            m["more"] = Value::Bool(true);
        }
        format!("id: {seq}\nevent: sync\ndata: {}", Value::Array(vec![m])).into_bytes()
    }

    #[test]
    fn the_list_is_timed_at_its_first_page_and_whole_at_its_last() {
        let mut p = Pages::default();
        let ms = Duration::from_millis;
        assert!(!p.frame(b"id: 1\nevent: sync\ndata: [{\"t\":\"welcome\"}]", ms(1)).unwrap());
        assert!(!p.frame(&page(9, &["a", "b"], true), ms(2)).unwrap());
        assert!(!p.frame(&page(9, &["c"], true), ms(5)).unwrap());
        assert!(p.frame(&page(9, &[], false), ms(7)).unwrap());
        let r = p.read(ms(7)).unwrap();
        assert_eq!((r.took, r.all, r.first, r.tasks, r.pages), (ms(2), ms(7), 2, 3, 3));
    }

    #[test]
    fn a_task_in_two_pages_or_pages_at_two_seqs_is_an_error() {
        let mut p = Pages::default();
        p.frame(&page(9, &["a"], true), Duration::ZERO).unwrap();
        assert!(p.frame(&page(9, &["a"], false), Duration::ZERO).unwrap_err().contains("two pages"));
        let mut p = Pages::default();
        p.frame(&page(9, &["a"], true), Duration::ZERO).unwrap();
        assert!(p.frame(&page(10, &["b"], false), Duration::ZERO).unwrap_err().contains("seq 9 and 10"));
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
        let over = report_all(151, 121, true, 5_001, 150_001, 19_999.0, 45).over();
        assert_eq!(over.len(), 6, "{over:?}");
        let starts = ["cold start", "idle RSS", "task list", "patch latency", "durable append", "write amplification"];
        for (o, s) in over.iter().zip(starts) {
            assert!(o.starts_with(s), "{o} is the {s} line");
        }
        for (i, r) in [
            report_all(10, 50, true, 1_000, 150_001, 30_000.0, 2),
            report_all(10, 50, true, 1_000, 1_000, 19_999.0, 2),
            report_all(10, 50, true, 1_000, 1_000, 30_000.0, 45),
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(r.over().len(), 1, "budget {i} alone: {:?}", r.over());
        }
    }

    #[test]
    fn a_patch_a_client_never_got_is_a_miss_even_when_the_rest_are_fast() {
        let mut r = report(10, 50, true, 1_000);
        r.patch = Patch { lat: vec![1_000; 99], expected: 100, frames: 0, bytes: 0, took: Duration::ZERO };
        assert_eq!(r.over(), vec!["1 of 100 patches never reached their client".to_owned()]);
        assert!(
            r.summary().contains("every round's 99 of 100 together: min 1.0 · p50 1.0 · p99 1.0 · max 1.0 ms | ❌ |"),
            "{}",
            r.summary()
        );
    }

    #[test]
    fn the_p99_is_the_99th_of_100_and_none_never_panics() {
        let p = |lat: Vec<u64>| Patch { lat, expected: 0, frames: 0, bytes: 0, took: Duration::ZERO }.spread();
        let lat: Vec<u64> = (1..=100).rev().map(|ms| ms * 1000).collect();
        assert_eq!(p(lat), (1.0, 51.0, 100.0, 100.0));
        assert_eq!(p(vec![]), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn a_stream_shape_parses_or_is_refused() {
        assert_eq!(Shape::parse("20:6000:text"), Some(Shape { per_s: 20, ms: 6_000, mix: Mix::Text }));
        assert_eq!(Shape::parse("2:10000:agent"), Some(STREAM));
        for bad in ["", "2", "2:10000", "0:10000:agent", "2:0:agent", "2:10000:noise", "2:10000:agent:x", "x:1:text"] {
            assert_eq!(Shape::parse(bad), None, "{bad}");
        }
        assert_eq!(STREAM.stamps(), 20);
        assert_eq!(Shape { per_s: 3, ms: 1_001, mix: Mix::Text }.stamps(), 4, "n / rate under ms: 0, ⅓, ⅔, 1");
    }

    #[test]
    fn no_payload_is_an_infinite_amplification_never_a_pass() {
        assert!(Amp { wal: 0, db: 0, payload: 0, steps: 0 }.x().is_infinite());
        let mut r = report(10, 50, true, 1_000);
        r.amp = Amp { wal: 0, db: 0, payload: 0, steps: 0 };
        assert_eq!(r.over(), vec!["write amplification inf× > 44×".to_owned()]);
        assert!((Amp { wal: 9_000, db: 1_000, payload: 1_000, steps: 3 }.x() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn stamps_are_read_wherever_a_frame_carries_them_and_a_broken_one_is_skipped() {
        let frame = br#"data: [{"rows":[{"v":"{\"text\":\"~stamp:12:0:1700~ xx\"}"},{"v":"~stamp:12:1:1750~ ~stamp:bad~ ~stamp:13:0:1800~"}],"t":"~stamp:9:9"}]"#;
        assert_eq!(stamps(frame), vec![((12, 0), 1700), ((12, 1), 1750), ((13, 0), 1800)]);
        assert!(stamps(b"data: []").is_empty());
    }

    #[test]
    fn a_topic_counts_as_looked_at_once_its_last_page_is_in() {
        let mut done = std::collections::HashSet::new();
        snapshotted(&page(3, &["a"], true), &mut done).unwrap();
        assert!(done.is_empty(), "a page with more to come");
        snapshotted(&page(3, &[], false), &mut done).unwrap();
        assert!(done.contains("list"));
        snapshotted(b"id: 4\nevent: sync\ndata: [{\"t\":\"patch\",\"topic\":\"session:x\"}]", &mut done).unwrap();
        assert_eq!(done.len(), 1);
    }

    #[test]
    fn a_whole_chunked_body_decodes_and_a_cut_one_is_an_error() {
        assert_eq!(dechunk(b"5\r\nhello\r\n1;x=y\r\n!\r\n0\r\n\r\n").unwrap(), b"hello!");
        assert!(dechunk(b"5\r\nhel").is_err());
        assert!(dechunk(b"").is_err());
    }

    #[test]
    fn an_rss_that_never_settles_is_a_miss_even_under_the_budget() {
        let over = report(10, 50, false, 1_000).over();
        assert_eq!(over, vec!["idle RSS never settled in 30 s (last 50.0 MB, budget 120 MB)".to_owned()]);
        assert!(report(10, 50, false, 1_000).summary().contains("| ❌ |"));
    }

    // ADR-0089: the timed budgets' verdict, on the numbers PR CI recorded (the Linux jobs' gated values; /tmp/ci-*.log of
    // runs db637559, e978…, df56a408, 9dcd…, 4612442b: every one the same list and cold-start code since ADR-0084).

    /// The task list's job medians: 4612442b's quick job read 7.87 ms (p90 67, max 102: the whole runner slow then);
    /// the other jobs on the same code 4.69 (4612442b full), 4.39 and 2.64 (9dcd), 3.86 and 4.47 (df56a408).
    const LIST_CI: [f64; 5] = [7.87, 4.69, 4.39, 3.86, 4.47];

    #[test]
    fn a_round_the_runner_slowed_is_outvoted_and_the_old_single_measure_missed() {
        // the old gate: one measure, the slow spell decides
        assert_eq!(rounds(&[7.87], &[], 1.0).judge(1.0, LIST_MS).verdict, Verdict::Miss);
        // in rounds, the slow one is outvoted even when the calibration didn't catch it
        let j = rounds(&LIST_CI, &[], 1.0).judge(1.0, LIST_MS);
        assert_eq!((j.verdict, j.value, j.clean), (Verdict::Pass, 4.47, 5));
        // and left out when it did: the median of the 4 clean rounds (the upper of the middle two)
        let j = rounds(&LIST_CI, &[0], 3.0).judge(1.0, LIST_MS);
        assert_eq!((j.verdict, j.value, j.clean), (Verdict::Pass, 4.47, 4));
        assert!(rounds(&LIST_CI, &[0], 3.0).cell(1.0, 2).starts_with("7.87† · 4.69 · "));
    }

    #[test]
    fn a_list_twice_as_slow_misses_on_a_quiet_runner_and_is_never_passed_on_a_noisy_one() {
        let slower: Vec<f64> = LIST_CI.iter().map(|v| v * 2.0).collect();
        let j = rounds(&slower, &[], 1.0).judge(1.0, LIST_MS);
        assert_eq!((j.verdict, j.value), (Verdict::Miss, 8.94));
        // two rounds disturbed: still most clean, still a miss (the calibration has nothing of wisp's to hide)
        assert_eq!(rounds(&slower, &[0, 1], 2.0).judge(1.0, LIST_MS).verdict, Verdict::Miss);
        // three of five disturbed: this run can't tell, and says so: a failure, not a pass
        let j = rounds(&slower, &[0, 1, 2], 2.0).judge(1.0, LIST_MS);
        assert_eq!((j.verdict, j.clean, j.rounds), (Verdict::Noisy, 2, 5));
        assert!((j.worst - 2.0).abs() < 1e-9);
    }

    /// Cold start restarts as CI printed them, 5 to a round: 4612442b's full job (its 424.5 ms first restart) and quick
    /// job, then 9dcd's two jobs (its quick job's 226.2 ms first).
    const COLD_CI: [f64; 25] = [
        424.5, 93.8, 107.1, 85.5, 114.9, 99.0, 95.9, 146.9, 123.7, 105.7, 122.8, 110.9, 121.1, 104.8, 127.0, 112.3, 121.5, 101.2, 104.4,
        99.7, 111.2, 226.2, 103.6, 108.3, 91.7,
    ];

    fn cold_rounds(restarts: &[f64], plus: f64) -> Rounds {
        let mut r = Rounds::default();
        for round in restarts.chunks(COLD_PER_ROUND) {
            let mut v: Vec<f64> = round.iter().map(|m| m + plus).collect();
            r.push(upper_median(&mut v), 1.0);
        }
        r
    }

    #[test]
    fn cold_start_passes_on_ci_numbers_and_misses_50_ms_slower_or_on_the_build_before_adr_0084() {
        let j = cold_rounds(&COLD_CI, 0.0).judge(1.0, COLD_START_MS);
        assert_eq!((j.verdict, j.value), (Verdict::Pass, 107.1), "{:?}", cold_rounds(&COLD_CI, 0.0));
        assert_eq!(cold_rounds(&COLD_CI, 50.0).judge(1.0, COLD_START_MS).verdict, Verdict::Miss);
        // db637559's quick job, before ADR-0084 made the start do less: wisp was slower, and the gate says so
        let before = [161.2, 205.5, 167.2, 174.3, 181.9, 165.3, 220.8, 161.2, 205.5, 167.2];
        assert_eq!(cold_rounds(&before, 0.0).judge(1.0, COLD_START_MS).verdict, Verdict::Miss);
    }

    /// Patch p99 per job: 4612442b's full job 170.4 ms (p50 90.6, max 172.8: one stalled commit window), then 109.9,
    /// 103.6, 104.2, 104.7, 103.7.
    #[test]
    fn patch_p99_rounds_outvote_one_stall_and_catch_50_ms_more_on_every_patch() {
        let j = rounds(&[170.4, 109.9, 103.6], &[], 1.0).judge(1.0, PATCH_P99_MS);
        assert_eq!((j.verdict, j.value), (Verdict::Pass, 109.9));
        assert_eq!(rounds(&[220.4, 159.9, 153.6], &[], 1.0).judge(1.0, PATCH_P99_MS).verdict, Verdict::Miss);
        assert_eq!(rounds(&[104.7, 153.7, 154.2], &[], 1.0).judge(1.0, PATCH_P99_MS).verdict, Verdict::Miss, "two slow rounds of three");
    }

    #[test]
    fn one_stalled_commit_window_decides_a_p99_of_800_but_not_of_2400() {
        // 10 tasks × 2 stamps a second: a 100 ms commit window carries ~2 stamps to 4 clients, 8 samples
        let p99 = |n: usize| {
            let mut lat: Vec<u64> = (0..n - 8).map(|i| 1_000 + (i as u64 * 104_000) / n as u64).collect();
            lat.extend([171_000; 8]);
            Patch { lat, expected: n, frames: 0, bytes: 0, took: Duration::ZERO }.spread().2
        };
        assert!((p99(800) - 171.0).abs() < 1e-9, "{}", p99(800));
        assert!(p99(2_400) < 105.0, "{}", p99(2_400));
    }

    #[test]
    fn noise_only_slows_so_a_pass_on_a_noisy_runner_is_a_pass_and_no_rounds_never_pass() {
        assert_eq!(rounds(&[3.0, 3.1, 3.2], &[0, 1, 2], 5.0).judge(1.0, LIST_MS).verdict, Verdict::Pass);
        let none = Rounds::default().judge(1.0, LIST_MS);
        assert_eq!(none.verdict, Verdict::Miss);
        assert!(none.value.is_infinite());
        // a calibration that never ran (or a usual of 0) counts every round disturbed: over budget is Noisy, not judged clean
        assert_eq!(rounds(&[6.0, 6.0, 6.0], &[], 1.0).judge(0.0, LIST_MS).verdict, Verdict::Noisy);
        assert_eq!(rounds(&[6.0, 6.0, 6.0], &[], 1.0).judge(f64::INFINITY, LIST_MS).verdict, Verdict::Noisy);
        let short = Rounds { values: vec![6.0, 6.0, 6.0], ..Rounds::default() };
        assert_eq!(short.judge(1.0, LIST_MS).verdict, Verdict::Noisy, "a round with no calibration is never clean");
    }

    #[test]
    fn at_the_budget_passes_and_the_noisy_threshold_is_exclusive() {
        assert_eq!(rounds(&[5.0, 5.0, 5.0], &[], 1.0).judge(1.0, LIST_MS).verdict, Verdict::Pass);
        assert_eq!(rounds(&[5.01, 5.01, 5.01], &[], 1.0).judge(1.0, LIST_MS).verdict, Verdict::Miss);
        assert_eq!(rounds(&[6.0; 3], &[0, 1], NOISY).judge(1.0, LIST_MS).verdict, Verdict::Miss, "at 1.5× is still clean");
        assert_eq!(rounds(&[6.0; 3], &[0, 1], NOISY + 0.01).judge(1.0, LIST_MS).verdict, Verdict::Noisy);
    }

    #[test]
    fn a_miss_exits_1_a_run_too_noisy_to_judge_exits_3_and_says_why() {
        assert_eq!(report(10, 50, true, 1_000).exit_code(), 0);
        let mut r = report(10, 50, true, 1_000);
        r.list_rounds = rounds(&[9.0, 9.0, 9.0, 4.0, 4.0], &[0, 1, 2], 2.5);
        assert!(r.over().is_empty(), "{:?}", r.over());
        let noisy = r.noisy();
        assert_eq!(noisy.len(), 1);
        assert!(noisy[0].starts_with("too noisy to judge task list: 3 of 5 rounds"), "{}", noisy[0]);
        assert!(noisy[0].contains("up to 2.5×") && noisy[0].contains("not a pass"), "{}", noisy[0]);
        assert_eq!(r.exit_code(), 3);
        assert!(r.summary().contains("⚠️ too noisy to judge"), "{}", r.summary());
        r.cold_rounds = rounds(&[200.0; 5], &[], 1.0);
        assert_eq!(r.exit_code(), 1, "a miss wins over a run that couldn't judge another budget");
    }

    #[test]
    fn the_calibration_is_a_fixed_workload_that_takes_time() {
        let dir = std::env::temp_dir().join(format!("server-budgets-calib-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let c = calibrate(&dir.join("scratch")).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(c.is_finite() && c > 0.0, "{c}");
    }

    #[test]
    fn steal_is_the_share_of_ticks_between_two_readings_and_unknown_without_them() {
        assert_eq!(steal_pct(Some((10, 1_000)), Some((30, 1_400))), Some(5.0));
        assert_eq!(steal_pct(Some((10, 1_000)), Some((10, 1_000))), None, "no tick passed");
        assert_eq!(steal_pct(None, Some((10, 1_000))), None);
    }

    #[test]
    fn copy_dir_copies_folders_and_files() {
        let dir = std::env::temp_dir().join(format!("server-budgets-copy-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(dir.join("a/b/f"), "x").unwrap();
        std::fs::write(dir.join("a/g"), "y").unwrap();
        copy_dir(&dir.join("a"), &dir.join("c")).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("c/b/f")).unwrap(), "x");
        assert_eq!(std::fs::read_to_string(dir.join("c/g")).unwrap(), "y");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
