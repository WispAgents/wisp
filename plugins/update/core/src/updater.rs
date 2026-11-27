//! The update check and install as a sans-IO state machine (v1 server/update.js `Updater`). The host asks
//! it what to fetch or run, does that, and feeds the answer back; everything decided in between (which
//! release counts, what to run, what to say when it fails) is here and tested without a network.
//!
//! size-ok: one sans-IO state machine; splitting its transitions would scatter one decision table

use serde::Serialize;
use serde_json::Value;

use crate::channel::Channel;
use crate::deferred::{self, Files, Pending};
use crate::install::{DOWNLOADS, FORMULA, Install, Method, PKG, Platform, REGISTRY, RELEASES, asset_name};
use crate::version::{find_version, newer, newest};

pub const DAY_MS: i64 = 24 * 3600 * 1000;
/// The daily check's hour, on the user's clock: a release out overnight shows as the Update pill in the morning.
pub const MORNING_HOUR: i64 = 9;
/// Timers stall while the machine sleeps, so the host polls the wall clock this often (a computer that slept
/// through the morning checks within this of waking; a check that failed is tried again this much later).
pub const TICK_MS: u64 = 30 * 60 * 1000;
/// The first check, after start.
pub const FIRST_CHECK_MS: u64 = 5000;
pub const CHECK_TIMEOUT_MS: u64 = 10_000;
pub const DOWNLOAD_TIMEOUT_MS: u64 = 15 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Idle,
    Installing,
    Restarting,
}

/// What GET /api/update answers (v1 `Updater.view()`), field for field.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct View {
    pub current: String,
    pub latest: Option<String>,
    pub available: bool,
    pub method: &'static str,
    pub channel: &'static str,
    pub status: Status,
    pub error: Option<String>,
    pub enabled: bool,
    /// what installs the latest version; a clone has none (v1 left the key out)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// A release download (GitHub's asset object).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    /// `sha256:<hex>`, when GitHub has one
    pub digest: Option<String>,
}

/// What the relaunch has to finish once this process is gone (the app method).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Swap {
    /// macOS: delete the old bundle, open the new one
    Mac { open: String, remove: String },
    /// Windows: the NSIS installer runs once wisp has quit, so it can replace the files
    Windows { installer: String },
    /// Linux: the `AppImage`, replaced in place
    Linux { open: String },
}

/// What the host reads from the environment once, at start.
#[derive(Debug, Clone, Default)]
pub struct Env {
    /// `WISP_UPDATE_CHECK`
    pub update_check: Option<String>,
    /// CI (any non-empty value turns checks off, as in v1)
    pub ci: Option<String>,
    /// `WISP_DESKTOP`, read at start: the desktop shell drops it once the server is up
    pub desktop: Option<String>,
    /// APPIMAGE
    pub appimage: Option<String>,
    /// the node that runs the launcher (`WISP_NODE`, set by bin/wisp.js), else `node`
    pub node: Option<String>,
    /// this process's parent (the desktop shell, which holds the single-instance lock)
    pub ppid: u32,
    /// `WISP_UPDATE_FEED`, through [`feed_override`]: the app's release feed instead of GitHub's, for tests and a
    /// local fake feed only
    pub feed: Option<String>,
}

/// `WISP_UPDATE_FEED`, when it names a feed on this computer (`http://127.0.0.1:<port>/…`, `http://localhost:…`,
/// `http://[::1]:…`): what the app checks instead of GitHub Releases. Anything else is ignored, so the variable can
/// only ever point an update at a feed someone runs here (a test, a packaged build tried end to end).
pub fn feed_override(v: Option<&str>) -> Option<String> {
    let v = v?.trim().trim_end_matches('/');
    let rest = v.strip_prefix("http://")?;
    // the authority, read strictly: no `user@` (`http://localhost:80@evil.com` goes to evil.com), no `\`, no `?`/`#`
    let host = rest.split('/').next().unwrap_or("");
    if host.contains(['@', '\\', '?', '#']) || rest.contains('\\') {
        return None;
    }
    let (name, port) = match host.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && h != "[" && !p.contains(']') => (h, Some(p)),
        _ => (host, None),
    };
    let port_ok = port.is_none_or(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()));
    (port_ok && matches!(name, "127.0.0.1" | "localhost" | "[::1]")).then(|| v.to_owned())
}

/// The port in this wisp's own address (`http://127.0.0.1:4891`, `http://[::1]:4891/`): what the relaunch reuses.
/// `WISP_PORT` alone missed `wisp --port 4891`, so the update came back on 4477 and the open tab never found it.
pub fn port_of(url: &str) -> Option<u16> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split('/').next().unwrap_or("");
    let (_, port) = authority.rsplit_once(':')?;
    port.parse().ok().filter(|p| *p != 0)
}

/// The fetch a check makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRequest {
    /// when it was asked: what [`Updater::due`] counts from once it's answered
    pub at: i64,
    pub url: String,
    pub accept: &'static str,
    pub channel: Channel,
}

/// What the install does before the hand-off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallStep {
    /// npx: nothing to install, it fetches on relaunch
    Nothing,
    /// npm or brew: run this shell command with these extra env vars
    Run { cmd: String, env: Vec<(String, String)> },
    /// npm on Windows: the hand-off runs `cmd` once wisp has exited (`crate::deferred`)
    AfterExit { cmd: String },
    /// the desktop app: download `asset` and swap it in
    DownloadApp { asset: Asset },
}

/// Why an install is refused before it starts: 400 or 409 on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Invalid(String),
    Conflict(String),
}

#[derive(Debug, Clone)]
pub struct Updater {
    pub install: Install,
    pub platform: Platform,
    pub current: String,
    pub port: u16,
    pub enabled: bool,
    pub env: Env,
    checked_at: Option<i64>,
    pub latest: Option<String>,
    pub available: bool,
    pub status: Status,
    pub error: Option<String>,
    last_channel: Channel,
    /// npm: the dist-tag that has the version on offer
    pub tag: String,
    /// app: the chosen release's download for this OS and CPU
    pub asset: Option<Asset>,
    /// app (beta, nightly): that release's page
    pub release: Option<String>,
    pub swap: Option<Swap>,
    /// Windows npm: what the hand-off installs once this wisp has exited
    pub deferred: Option<String>,
}

/// The latest [`MORNING_HOUR`] on the user's clock at or before `now_ms` (UTC ms; `offset_ms`: the zone's offset).
pub fn last_morning(now_ms: i64, offset_ms: i64) -> i64 {
    let local = now_ms + offset_ms;
    let morning = local.div_euclid(DAY_MS) * DAY_MS + MORNING_HOUR * 3_600_000;
    (if local < morning { morning - DAY_MS } else { morning }) - offset_ms
}

fn falsy(v: Option<&str>) -> bool {
    v.is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off"))
}

/// `s` with its first letter lower case (a sentence carried on after a comma).
fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_lowercase().chain(c).collect()).unwrap_or_default()
}

/// `s` as one `sh` word: in single quotes, each `'` as `'\''`, so nothing in it (`$`, a backtick, `"`, a space) is
/// ever expanded or run. v1 used JSON quotes, inside which sh still expands `$(…)` and backticks.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// `JSON.stringify` of a string: how v1 quoted paths in shell commands (Windows still: cmd has no single quotes).
pub fn json_quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
}

impl Updater {
    pub fn new(install: Install, platform: Platform, current: &str, port: u16, env: Env) -> Self {
        let enabled = install.method != Method::Git && !falsy(env.update_check.as_deref()) && env.ci.as_deref().is_none_or(str::is_empty);
        Self {
            install,
            platform,
            current: current.to_owned(),
            port,
            enabled,
            env,
            checked_at: None,
            latest: None,
            available: false,
            status: Status::Idle,
            error: None,
            last_channel: Channel::Stable,
            tag: "latest".into(),
            asset: None,
            release: None,
            swap: None,
            deferred: None,
        }
    }

    pub fn method(&self) -> Method {
        self.install.method
    }

    /// Homebrew's tap only follows stable releases.
    pub fn channel(&self, pref: &str) -> Channel {
        if self.method() == Method::Brew { Channel::Stable } else { Channel::of(pref) }
    }

    /// At start, then every morning ([`MORNING_HOUR`] on the user's clock, `offset_ms` east of UTC): due when no
    /// check has been answered since the latest morning. Was "a day after the last check": a wisp started at
    /// 10:00 before the evening's release next looked at 10:00 the day after, so the pill only showed once the
    /// user had updated by hand and restarted. A check that wasn't answered (offline) doesn't count.
    pub fn due(&self, now_ms: i64, offset_ms: i64) -> bool {
        self.enabled && self.checked_at.is_none_or(|t| t < last_morning(now_ms, offset_ms))
    }

    pub fn view(&self, pref: &str) -> View {
        View {
            current: self.current.clone(),
            latest: self.latest.clone(),
            available: self.available,
            method: self.method().name(),
            channel: self.channel(pref).name(),
            status: self.status,
            error: self.error.clone(),
            enabled: self.enabled,
            command: self.command(),
        }
    }

    /// What installs the latest version; shown to copy if it fails. A clone has none.
    pub fn command(&self) -> Option<String> {
        match self.method() {
            Method::Brew => Some(format!("{} upgrade {FORMULA}", self.quote(&self.platform.join(self.brew(), &["bin", "brew"])))),
            Method::App => Some(self.release.clone().unwrap_or_else(|| DOWNLOADS.to_owned())),
            Method::Global => Some(format!("npm install -g {PKG}@{}", self.tag)),
            Method::Npx => Some(format!("npx -y {PKG}@{}", self.tag)),
            Method::Git => None,
        }
    }

    fn brew(&self) -> &str {
        self.install.brew.as_deref().unwrap_or("")
    }

    /// Where the updated copy lives: the same folder, except Homebrew puts each version in its own keg and
    /// keeps opt/wisp-agents pointing at the current one.
    pub fn home(&self) -> String {
        if self.method() == Method::Brew {
            return self.platform.join(self.brew(), &["opt", "wisp-agents", "libexec", "lib", "node_modules", PKG]);
        }
        self.install.root.clone()
    }

    /// The app's release feed: GitHub's, or a local one ([`feed_override`]).
    fn releases(&self) -> &str {
        self.env.feed.as_deref().unwrap_or(RELEASES)
    }

    /// Starts a check: records the time and says what to fetch. The app follows GitHub Releases (a release
    /// is published once all its downloads are up): stable asks for the latest one, the other channels look
    /// through the newest few.
    pub fn begin_check(&self, now_ms: i64, pref: &str) -> CheckRequest {
        let channel = self.channel(pref);
        let app = self.method() == Method::App;
        let url = match (app, channel) {
            (false, _) => REGISTRY.to_owned(),
            (true, Channel::Stable) => format!("{}/latest", self.releases()),
            (true, _) => format!("{}?per_page=30", self.releases()),
        };
        CheckRequest { at: now_ms, url, accept: if app { "application/vnd.github+json" } else { "application/json" }, channel }
    }

    /// The check's answer. `body` is None when the fetch failed or wasn't 2xx (offline: next round).
    /// True when what the UI shows changed.
    pub fn end_check(&mut self, req: &CheckRequest, body: Option<&Value>) -> bool {
        let Some(body) = body else { return false };
        self.checked_at = Some(req.at);
        let channel = req.channel;
        let version = if self.method() == Method::App { self.pick_release(channel, body) } else { self.pick_tag(channel, body) };
        let available = self.method() != Method::Git && version.as_deref().is_some_and(|v| newer(v, &self.current));
        let mut changed = version != self.latest || available != self.available || channel != self.last_channel;
        if changed {
            self.latest = version;
            self.available = available;
            self.last_channel = channel;
        }
        // an install refused before this check knew of a download (no asset then) isn't this copy's state any more
        if self.status != Status::Installing && self.asset.is_some() && self.error.take().is_some() {
            changed = true;
        }
        changed
    }

    fn pick_tag(&mut self, channel: Channel, body: &Value) -> Option<String> {
        let tags = channel.tags();
        let vals: Vec<&str> = tags.iter().map(|t| body.get(*t).and_then(Value::as_str).unwrap_or("")).collect();
        let version = newest(vals.iter().copied()).map(str::to_owned);
        tags.iter().zip(&vals).find(|(_, v)| Some(**v) == version.as_deref()).map_or("latest", |(t, _)| *t).clone_into(&mut self.tag);
        version
    }

    fn pick_release(&mut self, channel: Channel, body: &Value) -> Option<String> {
        let list: Vec<&Value> = body.as_array().map_or_else(|| vec![body], |a| a.iter().collect());
        let flag = |r: &Value, k: &str| r.get(k).and_then(Value::as_bool).unwrap_or(false);
        let ok = |r: &&Value| {
            let nightly_tag = r.get("tag_name").and_then(Value::as_str) == Some("nightly");
            !flag(r, "draft")
                && match channel {
                    Channel::Nightly => true,
                    Channel::Beta => !nightly_tag,
                    Channel::Stable => !flag(r, "prerelease"),
                }
        };
        let list: Vec<&Value> = list.into_iter().filter(ok).collect();
        let versions: Vec<String> = list.iter().map(|r| release_version(r).unwrap_or_default()).collect();
        let version = newest(versions.iter().map(String::as_str)).map(str::to_owned);
        let pick = list.iter().zip(&versions).find(|(_, v)| Some(v.as_str()) == version.as_deref()).map(|(r, _)| *r);
        let want = asset_name(&self.platform, self.env.appimage.as_deref());
        self.asset = pick.and_then(|r| {
            r.get("assets")?.as_array()?.iter().find(|a| a.get("name").and_then(Value::as_str) == want.as_deref()).and_then(asset_of)
        });
        self.release = if channel == Channel::Stable { None } else { pick.and_then(|r| r.get("html_url")?.as_str()).map(str::to_owned) };
        version
    }

    /// Starts an install, or refuses it.
    pub fn begin_install(&mut self) -> Result<InstallStep, Refusal> {
        if self.method() == Method::Git {
            return Err(Refusal::Invalid("This copy of wisp is a git clone, so it does not update itself.".into()));
        }
        if self.status == Status::Installing {
            return Err(Refusal::Conflict("Already updating.".into()));
        }
        let step = match self.method() {
            Method::App => {
                let Some(asset) = self.asset.clone() else {
                    // checks on but none answered yet (right after start): nothing is known of the update, so the copy
                    // isn't to blame, and nothing is kept as its error (audit I, smaller notes: it said "no automatic
                    // update for this copy" and kept that after the check found 2.0.1)
                    if self.enabled && self.latest.is_none() {
                        return Err(Refusal::Invalid(
                            "wisp hasn't found out about the update yet (it checks shortly after it starts): run wisp update check, then try again."
                                .into(),
                        ));
                    }
                    // checks off: nothing looked for the download, so the copy isn't to blame (audit H HS10)
                    let m = if self.enabled {
                        format!("There's no automatic update for this copy of wisp. {}", self.later())
                    } else {
                        format!(
                            "Update checks are off here (WISP_UPDATE_CHECK=0 or CI), so wisp didn't look for the update. \
                             Unset WISP_UPDATE_CHECK (outside CI) and try again, or {}",
                            lower_first(&self.later())
                        )
                    };
                    self.error = Some(m.clone());
                    return Err(Refusal::Invalid(m));
                };
                InstallStep::DownloadApp { asset }
            }
            Method::Npx => InstallStep::Nothing,
            // Old kegs stay until brew's own periodic cleanup: this process still runs from one.
            Method::Brew => {
                InstallStep::Run { cmd: self.command().unwrap_or_default(), env: vec![("HOMEBREW_NO_INSTALL_CLEANUP".into(), "1".into())] }
            }
            // wisp.exe runs from the folder npm swaps, and Windows locks a running exe
            Method::Global if self.platform.windows() => InstallStep::AfterExit { cmd: self.command().unwrap_or_default() },
            Method::Global | Method::Git => InstallStep::Run { cmd: self.command().unwrap_or_default(), env: Vec::new() },
        };
        self.deferred = match &step {
            InstallStep::AfterExit { cmd } => Some(cmd.clone()),
            _ => None,
        };
        self.status = Status::Installing;
        self.error = None;
        Ok(step)
    }

    /// After npm or brew ran: relaunching a copy that didn't change would leave the UI waiting for a
    /// version that never comes. `got`: the version in the updated copy's package.json.
    pub fn check_installed(&self, got: Option<&str>) -> Result<(), String> {
        if got.is_some_and(|g| newer(g, &self.current)) {
            return Ok(());
        }
        Err(if self.method() == Method::Brew {
            format!("Homebrew doesn't have wisp {} yet. Try again later.", self.latest.as_deref().unwrap_or("?"))
        } else {
            format!("npm installed wisp somewhere else: this copy is still {}.", got.unwrap_or(&self.current))
        })
    }

    pub fn install_failed(&mut self, message: &str) {
        self.status = Status::Idle;
        self.error = Some(message.to_owned());
        self.deferred = None;
    }

    /// What the host writes to [`deferred::PENDING`] before a hand-off that installs after exit.
    pub fn pending(&self) -> Option<Pending> {
        let cmd = self.deferred.clone()?;
        Some(Pending { from: self.current.clone(), to: self.latest.clone().unwrap_or_default(), cmd })
    }

    /// At start, what the last run's hand-off did ([`deferred::outcome`]): a failure is this row's error, so the
    /// pane and the page that waited on the restart say why.
    pub fn resume(&mut self, outcome: &deferred::Outcome) {
        if let deferred::Outcome::Failed(m) = outcome {
            self.install_failed(m);
        }
    }

    pub fn install_done(&mut self) {
        self.status = Status::Restarting;
    }

    /// The detached waiter that relaunches wisp (v1 update.js `handOff`): it stops this wisp (`pid`), waits until
    /// it's gone (≤ 30 s), then runs [`Self::relaunch_command`]. The desktop app is reopened, a browser tab kept.
    /// Unix: `sh` sends SIGTERM, wisp's clean stop. Windows (v1 relaunched there too; v2 left it to the user until
    /// 2026-11-14): `PowerShell` ends the process (Windows has no SIGTERM for a console-less server; its children go
    /// with their Job Objects) and starts the command through `cmd /S /C`; an npm install deferred to here runs in
    /// between (`crate::deferred`, with `files`). `None` when there's nothing to relaunch.
    pub fn hand_off(&self, pid: u32, files: &Files) -> Option<HandOff> {
        let desktop = self.env.desktop.is_some();
        let cmd = self.relaunch_command(desktop);
        if cmd.is_empty() || self.status != Status::Restarting {
            return None;
        }
        if let Some(npm) = self.deferred.as_deref().filter(|_| self.platform.windows()) {
            let script = deferred::script(pid, npm, &cmd, &self.wisp_args(desktop), files, &self.platform);
            return Some(powershell(script));
        }
        if self.platform.windows() {
            // -EncodedCommand: no quoting of the script through cmd.exe or PowerShell's own command line
            let quoted = crate::deferred::ps_escape(&format!("/S /C \"{cmd}\""));
            let script = format!(
                "Start-Sleep -Seconds 1; Stop-Process -Id {pid} -Force -ErrorAction SilentlyContinue; \
                 Wait-Process -Id {pid} -Timeout 30 -ErrorAction SilentlyContinue; \
                 Start-Process -FilePath cmd.exe -ArgumentList '{quoted}' -WindowStyle Hidden"
            );
            return Some(powershell(script));
        }
        // let the answer reach the page first, then stop and wait for the port to be free
        let script = format!(
            "sleep 1; kill -TERM {pid} 2>/dev/null; for i in $(seq 300); do kill -0 {pid} 2>/dev/null || break; sleep 0.1; done; {cmd}"
        );
        Some(HandOff { bin: "sh", args: vec!["-c".to_owned(), script.clone()], script })
    }

    /// "Download it from …": the tail of every app update failure.
    pub fn later(&self) -> String {
        format!("Download it from {}", self.release.as_deref().unwrap_or(DOWNLOADS))
    }

    /// The command that starts the new version once this process has let go of the port. Same port as now
    /// (wisp may have been started with --port), so the open tab finds it again. `desktop`: reopen the app;
    /// otherwise keep the browser tab.
    pub fn relaunch_command(&self, desktop: bool) -> String {
        let tauri = self.env.desktop.as_deref() == Some("tauri");
        if self.method() == Method::App {
            // macOS and Linux: Tauri's shell holds the single-instance lock until it's gone, so the new app waits it out
            let gone = if tauri { self.shell_gone() } else { String::new() };
            // NSIS: /S installs silently over this copy, then /R starts the new one. cmd.exe takes no JSON escapes,
            // and a Windows path has no `"`; sh gets every path in single quotes.
            return match &self.swap {
                Some(Swap::Windows { installer }) => format!("\"{installer}\" /S /R"),
                Some(Swap::Mac { open, remove }) => format!("rm -rf {}; {gone}open {}", sh_quote(remove), sh_quote(open)),
                Some(Swap::Linux { open }) => format!("{gone}{} >/dev/null 2>&1", sh_quote(open)),
                None => String::new(),
            };
        }
        let args = self.wisp_args(desktop);
        // A shell started by bin/wisp.js holds the same lock: the new `wisp` would hand over to it and go.
        let wait = if tauri && !self.platform.windows() { self.shell_gone() } else { String::new() };
        if self.method() == Method::Npx {
            return format!("{wait}npx -y {PKG}@{}{args}", self.tag);
        }
        let node = self.quote(self.env.node.as_deref().unwrap_or("node"));
        let launcher = self.quote(&self.platform.join(&self.home(), &["bin", "wisp.js"]));
        format!("{wait}{node} {launcher}{args}")
    }

    /// A path for this platform's shell: `cmd` (Windows, as v1 quoted it) or `sh` ([`sh_quote`]).
    fn quote(&self, p: &str) -> String {
        if self.platform.windows() { json_quote(p) } else { sh_quote(p) }
    }

    /// The new wisp's arguments (leading space): the same port; desktop: reopen the app, otherwise keep the tab.
    pub fn wisp_args(&self, desktop: bool) -> String {
        format!("{} --port {}", if desktop { "" } else { " --browser --no-open" }, self.port)
    }

    /// Tauri's shell is this process's parent and outlives it by a moment, holding the single-instance
    /// lock: a new app opened meanwhile hands over to the old one and quits, and nothing comes back.
    fn shell_gone(&self) -> String {
        format!("for i in $(seq 100); do kill -0 {} 2>/dev/null || break; sleep 0.1; done; ", self.env.ppid)
    }
}

/// How [`Updater::hand_off`] starts its waiter: `bin` with `args`, detached. `script` is what it runs, readable
/// (`args` carries it encoded on Windows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandOff {
    pub bin: &'static str,
    pub args: Vec<String>,
    pub script: String,
}

/// `PowerShell` running `script`, passed encoded so no quote in it reaches cmd.exe's or `PowerShell`'s command line.
fn powershell(script: String) -> HandOff {
    let args = ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand"].map(str::to_owned);
    HandOff { bin: "powershell", args: args.into_iter().chain([encode_ps(&script)]).collect(), script }
}

/// `PowerShell`'s `-EncodedCommand`: the script as UTF-16LE, in base64.
pub fn encode_ps(script: &str) -> String {
    use base64::Engine as _;
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The rolling nightly release is tagged `nightly`; its title carries the version ("wisp 0.38.0-nightly.…").
pub fn release_version(r: &Value) -> Option<String> {
    let tag = r.get("tag_name").and_then(Value::as_str).unwrap_or("");
    if tag == "nightly" {
        return find_version(r.get("name").and_then(Value::as_str).unwrap_or("")).map(str::to_owned);
    }
    Some(tag.strip_prefix('v').unwrap_or(tag).to_owned())
}

fn asset_of(a: &Value) -> Option<Asset> {
    Some(Asset {
        name: a.get("name")?.as_str()?.to_owned(),
        url: a.get("browser_download_url").and_then(Value::as_str).unwrap_or("").to_owned(),
        digest: a.get("digest").and_then(Value::as_str).map(str::to_owned),
    })
}

/// A shell command's failure, as v1 put it: npm's or brew's last three lines, and a hint on EACCES.
pub fn run_failed(cmd: &str, stderr: &str) -> String {
    let lower = stderr.to_ascii_lowercase();
    let hint = if stderr.contains("EACCES") || lower.contains("permission denied") {
        " (permission denied — run it yourself, maybe with sudo)"
    } else {
        ""
    };
    // each line trimmed: cmd's `echo … 1>&2` and npm on Windows leave spaces and `\r` at line ends
    let lines: Vec<&str> = stderr.trim().lines().map(str::trim).collect();
    let tail = lines[lines.len().saturating_sub(3)..].join(" ");
    format!("`{cmd}` failed{hint}: {tail}")
}
