//! wisp.computer (ADR-0127): computer use for every harness, through Cua Driver. Three pieces:
//!
//! - a run part: every task's run gets the `computer` MCP server (HTTP, this wisp, the task's own token) while
//!   computer use is on, and on a grid box `browser` too (Playwright on the box's watched Chromium)
//! - the route behind it, `POST /api/computer/mcp`: wisp-computer-core's gateway answers what it can and judges
//!   each call (the task's mode, the deny list); the rest goes to the driver
//! - the driver ([`driver::Driver`]), started on the first call and kept in the plugin's scope
//!
//! `computer.status` says whether it's on and why not, from Settings, the CLI and a task's agent alike.

mod driver;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_computer_api::{self as api, Status};
use wisp_computer_core::gateway::{self, Step};
use wisp_computer_core::policy::{Place, Reach};
use wisp_kernel::http::{Request, Response, Route};
use wisp_kernel::{Ctx, Manifest, Plugin, Result, Setup};
use wisp_proc::{ExecOpts, Proc};
use wisp_runtime_api::model::{Mode, Session};
use wisp_runtime_api::points::{AgentAccess, McpQuery, RUN, RunPart};

use crate::driver::Driver;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

struct State {
    ctx: Ctx,
    proc: Proc,
    access: Option<Arc<dyn AgentAccess>>,
    place: Place,
    /// the driver, once a call started it
    driver: tokio::sync::Mutex<Option<Arc<Driver>>>,
    /// what `tools/list` answers: the snapshot until the driver ran, then its own list
    known: Mutex<(String, Vec<Value>)>,
    /// each task's mode as its last run started (the runtime said so, never the request)
    modes: Mutex<HashMap<String, Mode>>,
}

type Slot = Arc<OnceLock<Arc<State>>>;

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

/// Switched on: the user's setting, or `WISP_COMPUTER=on` (a grid box's image).
fn enabled(ctx: &Ctx) -> bool {
    env(api::ENV_ON).is_some_and(|v| v == "on" || v == "1") || ctx.setting(api::SETTING_ENABLED).and_then(|v| v.as_bool()) == Some(true)
}

/// The driver's command as given (`WISP_COMPUTER_DRIVER`, else `cua-driver`).
fn driver_cmd() -> String {
    env(api::ENV_DRIVER).unwrap_or_else(|| api::DRIVER_BIN.to_owned())
}

/// Where the driver's command is: a path as given, else the first on the search path, else where Cua's own
/// installer puts it (`~/.local/bin`, often not on a GUI app's PATH).
fn find(cmd: &str, path: Option<&std::ffi::OsStr>, home: Option<PathBuf>, is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if cmd.contains('/') || cmd.contains('\\') {
        return Some(PathBuf::from(cmd)).filter(|p| is_file(p));
    }
    let exe = if cfg!(windows) { format!("{cmd}.exe") } else { cmd.to_owned() };
    path.iter().flat_map(std::env::split_paths).chain(home.map(|h| h.join(".local").join("bin"))).map(|d| d.join(&exe)).find(|p| is_file(p))
}

/// Where Cua's Mac app lives: it holds the Accessibility and Screen Recording grants, and a plain
/// `cua-driver mcp` hands its work to it.
const MAC_APP: &str = "/Applications/CuaDriver.app";

/// The driver's arguments. On a Mac without Cua's app, `--direct`: the driver runs in wisp's process tree and
/// macOS asks for the grants for wisp (the desktop app, or the terminal it runs in) instead of failing on the
/// missing app. A driver command of the user's own (`WISP_COMPUTER_DRIVER`) gets plain `mcp`.
fn driver_args(mac: bool, own: bool, app: bool) -> Vec<String> {
    let mut a = vec!["mcp".to_owned()];
    if mac && !own && !app {
        a.push("--direct".to_owned());
    }
    a
}

fn driver_path() -> Option<PathBuf> {
    let path = wisp_proc::search_path();
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    find(&driver_cmd(), path.as_deref(), home, Path::is_file)
}

impl State {
    fn reach(&self, session: Option<&str>) -> Reach {
        let mode = session.and_then(|s| lock(&self.modes).get(s).copied());
        Reach { read_only: mode == Some(Mode::Plan), place: self.place }
    }

    /// The running driver, or a fresh one.
    async fn driver(&self) -> Result<Arc<Driver>> {
        let mut d = self.driver.lock().await;
        if let Some(x) = d.as_ref().filter(|x| x.running()) {
            return Ok(x.clone());
        }
        let bin = driver_path().ok_or_else(|| wisp_kernel::WireError::unavailable(not_found()))?;
        let args = driver_args(cfg!(target_os = "macos"), env(api::ENV_DRIVER).is_some(), Path::new(MAC_APP).exists());
        let fresh = Driver::start(&self.ctx, &self.proc, &bin.to_string_lossy(), &args).await?;
        *lock(&self.known) = (fresh.instructions.clone(), fresh.tools.clone());
        *d = Some(fresh.clone());
        Ok(fresh)
    }

    /// One request body's answer (`None`: only notifications).
    async fn answer(&self, body: &str, session: Option<&str>) -> Option<Value> {
        let msgs = match gateway::messages(body) {
            Ok(m) => m,
            Err(reply) => return Some(reply),
        };
        let batch = body.trim_start().starts_with('[');
        let reach = self.reach(session);
        let on = enabled(&self.ctx);
        let mut replies = Vec::new();
        for m in &msgs {
            let instructions = gateway::instructions(&lock(&self.known).0, self.place);
            match gateway::step(m, &instructions) {
                Step::Reply(r) => replies.push(r),
                Step::Silent => {}
                // switched off mid-run: nothing to offer
                Step::List { id } if !on => replies.push(gateway::result(&id, &json!({ "tools": [] }))),
                Step::List { id } => replies.push(gateway::list(&id, &lock(&self.known).1, reach)),
                Step::Call { id, .. } if !on => replies.push(gateway::result(&id, &gateway::tool_error(OFF))),
                Step::Call { id, name, args } => replies.push(self.call(&id, &name, &args, reach).await),
            }
        }
        gateway::answer(replies, batch)
    }

    async fn call(&self, id: &Value, name: &str, args: &Value, reach: Reach) -> Value {
        let tools = lock(&self.known).1.clone();
        if let Some(refused) = gateway::judge(id, name, &tools, reach) {
            return refused;
        }
        let d = match self.driver().await {
            Ok(d) => d,
            Err(e) => return gateway::result(id, &gateway::tool_error(&format!("wisp: {}", e.message))),
        };
        // the live list may differ from the snapshot (another platform, another version): judged again by it
        if let Some(refused) = gateway::judge(id, name, &d.tools, reach) {
            return refused;
        }
        let started = std::time::Instant::now();
        let out = d.call(name, args).await;
        tracing::info!(target: "wisp.computer", tool = name, ok = out.is_ok(), ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX), "computer call");
        match out {
            Ok(r) => gateway::result(id, &r),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
        }
    }

    async fn status(&self) -> Status {
        let on = enabled(&self.ctx);
        let path = driver_path();
        let mut s = Status {
            enabled: on,
            driver: path.as_ref().map(|p| p.display().to_string()),
            running: self.driver.lock().await.as_ref().is_some_and(|d| d.running()),
            browser: on && env(api::ENV_BROWSER).is_some(),
            ..Status::default()
        };
        if let Some(p) = &path {
            let opts = ExecOpts { env: driver::env(), timeout: Some(Duration::from_secs(10)), ..ExecOpts::default() };
            if let Ok(r) = self.proc.exec(&p.to_string_lossy(), &["--version".to_owned()], opts).await
                && r.exit.success()
            {
                s.version = Some(r.stdout_text().trim().to_owned()).filter(|v| !v.is_empty());
            }
        }
        s.hint = if path.is_none() {
            Some(not_found())
        } else if !on {
            Some("Computer use is off: switch it on in Settings (computer.enabled).".to_owned())
        } else {
            None
        };
        s
    }

    /// The MCP servers a run of `session` gets.
    fn servers(&self, session: &Session, q: &McpQuery) -> std::collections::BTreeMap<String, Value> {
        lock(&self.modes).insert(session.id.0.clone(), q.mode);
        let mut out = std::collections::BTreeMap::new();
        let Some(acc) = self.access.as_ref() else { return out };
        if !enabled(&self.ctx) || driver_path().is_none() {
            return out;
        }
        let (url, token) = (acc.url(), acc.token(&session.id.0));
        out.insert(
            api::SERVER.to_owned(),
            json!({ "type": "http", "url": format!("{url}/api{}", api::MCP_PATH), "headers": { "x-wisp-token": token, "x-wisp-session": session.id.0 } }),
        );
        // the box's browser, for Playwright's tools: it drives the page, so a Plan mode run doesn't get it
        if q.mode != Mode::Plan
            && let Some(b) = env(api::ENV_BROWSER)
        {
            out.insert(api::BROWSER_SERVER.to_owned(), json!({ "type": "stdio", "command": b, "args": [] }));
        }
        out
    }
}

const OFF: &str = "wisp: computer use is off on this wisp (Settings → computer.enabled).";

fn not_found() -> String {
    format!("Cua Driver isn't installed here. {} (wisp is tested with {}).", api::INSTALL_HINT, api::DRIVER_VERSION)
}

fn state(slot: &Slot) -> Result<Arc<State>> {
    slot.get().cloned().ok_or_else(|| wisp_kernel::WireError::unavailable("wisp.computer is starting"))
}

/// The run part: it reads the state the plugin made when it started (a run before that gets nothing).
struct Runs(Slot);

impl RunPart for Runs {
    fn id(&self) -> &'static str {
        api::PLUGIN_ID
    }
    fn mcp_servers(&self, session: &Session, q: &McpQuery) -> std::collections::BTreeMap<String, Value> {
        self.0.get().map(|s| s.servers(session, q)).unwrap_or_default()
    }
}

fn route(slot: Slot) -> Route {
    Route {
        method: "POST",
        pattern: api::MCP_PATH,
        permission: api::permissions::USE,
        doc: true,
        summary: "The computer MCP server (Cua Driver behind wisp's policy): one JSON-RPC message or a batch",
        raw: false,
        max_body: None,
        handler: Arc::new(move |_ctx, call, req: Request| {
            let slot = slot.clone();
            Box::pin(async move {
                let st = state(&slot)?;
                let text = String::from_utf8_lossy(&req.body).into_owned();
                Ok(match st.answer(&text, call.caller().session.as_deref()).await {
                    Some(body) => Response::Json { status: 200, body },
                    None => Response::Text { status: 202, content_type: "application/json".into(), body: String::new() },
                })
            })
        }),
    }
}

#[derive(Debug, Default)]
pub struct ComputerPlugin;

impl Plugin for ComputerPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let slot: Slot = Arc::new(OnceLock::new());
        s.route(route(slot.clone()));
        s.contribute(&RUN, Arc::new(Runs(slot.clone())) as Arc<dyn RunPart>);
        let sl = slot.clone();
        s.command::<api::StatusCmd, _, _>(move |_ctx, _call, _a| {
            let sl = sl.clone();
            async move { Ok(state(&sl)?.status().await) }
        });
        s.on_start(move |ctx| async move {
            let proc = ctx.service::<Proc>()?.as_ref().clone();
            let access = ctx.service::<Arc<dyn AgentAccess>>().ok().map(|a| a.as_ref().clone());
            let place = if env("WISP_BOX").is_some_and(|v| v == "1") { Place::Box } else { Place::Local };
            let st = State {
                ctx: ctx.clone(),
                proc,
                access,
                place,
                driver: tokio::sync::Mutex::new(None),
                known: Mutex::new(gateway::snapshot()),
                modes: Mutex::new(HashMap::new()),
            };
            let _ = slot.set(Arc::new(st));
            Ok(())
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        let m = ComputerPlugin.manifest();
        assert_eq!(m.id, api::PLUGIN_ID);
        let off = m.settings().into_iter().find(|s| s.key == api::SETTING_ENABLED).expect("declared");
        assert_eq!(off.default, Some(json!(false)), "off by default (D-0023)");
    }

    #[test]
    fn a_mac_without_cuas_app_runs_the_driver_direct() {
        assert_eq!(driver_args(true, false, false), ["mcp", "--direct"]);
        assert_eq!(driver_args(true, false, true), ["mcp"], "the app holds the grants");
        assert_eq!(driver_args(true, true, false), ["mcp"], "a command of the user's own");
        assert_eq!(driver_args(false, false, false), ["mcp"]);
    }

    #[test]
    fn the_driver_is_found_on_the_path_then_where_cuas_installer_puts_it() {
        let dirs = std::env::join_paths(["/opt/a", "/opt/b"]).ok();
        let dirs = dirs.as_deref();
        let at = |want: &'static str| move |p: &Path| p == Path::new(want);
        let sep = std::path::MAIN_SEPARATOR;
        if sep == '/' {
            assert_eq!(find("cua-driver", dirs, None, at("/opt/b/cua-driver")), Some("/opt/b/cua-driver".into()));
            assert_eq!(
                find("cua-driver", dirs, Some("/home/u".into()), at("/home/u/.local/bin/cua-driver")),
                Some("/home/u/.local/bin/cua-driver".into())
            );
            assert_eq!(find("cua-driver", dirs, None, |_: &Path| false), None);
            assert_eq!(
                find("/usr/local/bin/wisp-computer", None, None, at("/usr/local/bin/wisp-computer")),
                Some("/usr/local/bin/wisp-computer".into())
            );
            assert_eq!(find("/nope/x", dirs, None, |_: &Path| false), None, "a path as given, never searched");
        }
    }
}
