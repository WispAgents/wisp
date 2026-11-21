//! wisp.mcp: wisp's MCP server (v1 server/mcp-ask.js for a task's agent, server/mcp-wisp.js for the
//! user's editors). `POST /api/mcp` takes one JSON-RPC message or a batch; `wisp-mcp-core` parses it,
//! answers what needs no kernel (`initialize`, `tools/list`, `ping`, refusals) and hands each tool
//! call back here, where it runs as its owner's command, through the kernel, as the caller.
//!
//! The session a tool acts for is the caller's (its token's), never an argument. A session's flags
//! (side discussion, capped, team) are what the runtime last said with `mcp.session.set`.

mod crew;
mod kinds;
mod launch;
mod run;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::Value;
use wisp_kernel::http::{Request, Response, Route};
use wisp_kernel::{Call, Ctx, Manifest, Plugin, Result, Setup};
use wisp_mcp_api::{Rpc, RpcArgs, SessionInfo, SessionSet, TOOL_POINT, Tools};
use wisp_mcp_core::table::OwnFlags;
use wisp_mcp_core::{Extra, Who};

/// The server's own version, for `serverInfo` on the user's side.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Default)]
pub struct McpPlugin;

/// What the runtime said about each session's run.
#[derive(Debug, Default)]
pub(crate) struct Sessions(Mutex<HashMap<String, SessionInfo>>, pub(crate) crate::kinds::Tools);

impl Sessions {
    fn get(&self, id: &str) -> SessionInfo {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).get(id).cloned().unwrap_or_default()
    }
    fn set(&self, id: String, info: Option<SessionInfo>) {
        let mut m = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match info {
            Some(i) => m.insert(id, i),
            None => m.remove(&id),
        };
    }
}

/// Contributed tools as the dispatcher sees them, in the order plugins added them.
pub(crate) fn extras(ctx: &Ctx) -> (Vec<Extra>, Vec<Arc<wisp_mcp_api::ToolContribution>>) {
    let list = ctx.contributions(&TOOL_POINT);
    let ex = list
        .iter()
        .map(|c| Extra {
            name: c.tool.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            raw: c.tool.to_string(),
            audience: c.audience,
            flags: OwnFlags { read_only: c.read_only, capped_off: c.capped_off },
        })
        .collect();
    (ex, list)
}

/// Whether Auto is off (`runtime.auto`, default off; docs/v2-plan.md §3): the tool lists then offer no `auto` harness.
pub(crate) fn auto_off(ctx: &wisp_kernel::Ctx) -> bool {
    ctx.setting("runtime.auto") != Some(Value::Bool(true))
}

/// The contributed tools and, after them, one per catalogue command `call`'s caller may run (ADR-0040),
/// never under a name wisp's own tools or a contribution already have. A tool call runs through wisp.mcp, so only a
/// command its own manifest grants is listed (audit B #4: ~250 tools were listed and then refused when called).
pub(crate) fn extras_for(ctx: &Ctx, call: &Call) -> (Vec<Extra>, Vec<Arc<wisp_mcp_api::ToolContribution>>) {
    let (mut ex, mut list) = extras(ctx);
    let mut taken: Vec<String> = ex.iter().map(|e| e.name.clone()).collect();
    if let Ok(t) = wisp_mcp_core::table::table() {
        taken.extend(t.agent.iter().chain(t.user.iter()).map(|d| d.name.clone()));
    }
    taken.push("delegate".into());
    let refs: Vec<&str> = taken.iter().map(String::as_str).collect();
    let mut catalogue: Vec<_> = ctx.catalogue(call.caller()).into_iter().filter(|c| ctx.may(&c.permission)).collect();
    // a task's agent gets the user's setup commands too, as cards (ADR-0113): the call asks, the user's Allow writes
    let asks = setup_asks(call.caller());
    if asks {
        let have: Vec<String> = catalogue.iter().map(|c| c.id.clone()).collect();
        catalogue.extend(
            ctx.catalogue(&wisp_kernel::Caller::user())
                .into_iter()
                .filter(|c| SETUP_COMMANDS.contains(&c.id.as_str()) && !have.contains(&c.id) && ctx.may(&c.permission)),
        );
    }
    for (name, id, read_only, tool) in wisp_mcp_core::generated::tools(&catalogue, &refs) {
        let spec = wisp_kernel::decl::parse(catalogue.iter().find(|c| c.id == id).and_then(|c| c.args.as_deref()));
        let own_peer = spec.fields.iter().any(|f| f.name == wisp_mcp_core::generated::PEER);
        let spec = Arc::new(spec);
        let run: wisp_mcp_api::ToolFn = Arc::new(move |ctx, call, tc| {
            let (id, spec) = (id.clone(), spec.clone());
            Box::pin(async move {
                let (peer, input) = wisp_mcp_core::generated::split_peer(&tc.args, own_peer);
                // `args` as documented, and a run's own task filled in (audit E10)
                let session = call.caller().session.clone();
                let args = wisp_mcp_core::generated::call_args(&spec, &input, session.as_deref());
                // a field the command doesn't take is refused, never dropped (audit D6)
                if let Some(e) = wisp_mcp_core::generated::unknown_field(&wisp_mcp_core::generated::tool_name(&id), &spec, &args) {
                    return Ok(wisp_mcp_api::ToolText { text: None, error: Some(e) });
                }
                // the user's harness lowering a safety guard: the user's Allow in wisp first (ADR-0115); on a grid box the
                // user's set is `box`. A guarded write isn't sent to another computer: its value there isn't known here
                let harness = session.is_none() && (call.caller().set.starts_with("user:") || call.caller().set == "box");
                if harness && peer.is_some() && wisp_mcp_core::guard::guarded(&id, &args) {
                    return Ok(wisp_mcp_api::ToolText { text: None, error: Some(wisp_mcp_core::guard::NOT_ON_A_PEER.into()) });
                }
                if harness && let Some(l) = guard_lowered(&ctx, &call, &id, &args).await {
                    return Ok(match crate::run::guard_card(&ctx, &call, &id, args, &l).await {
                        wisp_mcp_core::Outcome::Failed(e) | wisp_mcp_core::Outcome::Rpc(_, e) => {
                            wisp_mcp_api::ToolText { text: None, error: Some(e) }
                        }
                        wisp_mcp_core::Outcome::Text(t) => wisp_mcp_api::ToolText { text: Some(t), error: None },
                    });
                }
                if let (true, Some(sid)) = (asks && SETUP_COMMANDS.contains(&id.as_str()), session.as_deref()) {
                    return Ok(match crate::run::setup(&ctx, &call, sid, &id, args).await {
                        wisp_mcp_core::Outcome::Failed(e) | wisp_mcp_core::Outcome::Rpc(_, e) => {
                            wisp_mcp_api::ToolText { text: None, error: Some(e) }
                        }
                        wisp_mcp_core::Outcome::Text(t) => wisp_mcp_api::ToolText { text: Some(t), error: None },
                    });
                }
                // `peer`: the command runs on that machine, through machines.route (ADR-0041)
                let out = match peer {
                    Some(p) => {
                        let list = ctx
                            .run::<wisp_machines_api::List>(&call, wisp_machines_api::NoArgs {})
                            .await
                            .ok()
                            .and_then(|l| serde_json::to_value(l).ok())
                            .unwrap_or_default();
                        match wisp_mcp_core::generated::find_peer(&list, &p) {
                            Some(pid) => {
                                ctx.run::<wisp_machines_api::Route>(
                                    &call,
                                    wisp_machines_api::RouteArgs {
                                        id: Some(pid),
                                        cmd: id.clone(),
                                        args,
                                        mid: format!("mcp-{:016x}", rand_mid()),
                                    },
                                )
                                .await
                            }
                            None => Err(wisp_kernel::WireError::not_found(format!("there's no machine {p} (machines_list shows them)"))),
                        }
                    }
                    None => ctx.run_json(&call, &id, args).await,
                };
                Ok(match out {
                    // a refusal in the command's own answer (`{"error": …}`) is the tool's error (audit F orchestration F7)
                    Ok(v) => match wisp_mcp_core::generated::refusal(&v) {
                        Some(e) => wisp_mcp_api::ToolText { text: None, error: Some(e) },
                        None => wisp_mcp_api::ToolText { text: Some(wisp_mcp_core::generated::answer_text(&v)), error: None },
                    },
                    // in words, not serde's (audit D6)
                    Err(e) => wisp_mcp_api::ToolText { text: None, error: Some(wisp_mcp_core::generated::said(&id, &spec, &e.message)) },
                })
            })
        });
        ex.push(Extra {
            name: name.clone(),
            raw: tool.to_string(),
            audience: wisp_mcp_api::Audience::Both,
            flags: OwnFlags { read_only, capped_off: false },
        });
        list.push(Arc::new(wisp_mcp_api::ToolContribution {
            tool,
            audience: wisp_mcp_api::Audience::Both,
            read_only,
            capped_off: false,
            run,
        }));
    }
    (ex, list)
}

/// What a write from the user's harness would lower (ADR-0115), as its card says it. None: it lowers nothing, or it
/// changes nothing (switching on a plugin that's already on; audit I, smaller notes). `runtime.exec`'s card names the
/// folder it runs in: its task's (a worktree's), read here.
async fn guard_lowered(ctx: &Ctx, call: &Call, id: &str, args: &Value) -> Option<wisp_mcp_core::guard::Lowered> {
    let already_on = id == "plugins.setEnabled"
        && args.get("enabled") == Some(&Value::Bool(true))
        && args.get("id").and_then(Value::as_str).is_some_and(|p| ctx.plugin_status().iter().any(|s| s.id == p && s.state == "active"));
    if already_on {
        return None;
    }
    let mut shown = args.clone();
    if id == "runtime.exec"
        && let Some(task) = args.get("id").and_then(Value::as_str)
    {
        let got = ctx.run::<wisp_runtime_api::commands::Get>(call, wisp_runtime_api::commands::IdArgs { id: task.into() }).await.ok();
        if let (Some(t), Some(o)) = (got, shown.as_object_mut()) {
            o.insert("cwd".into(), Value::String(t.cwd));
        }
    }
    wisp_mcp_core::guard::lowers(id, &shown, &|k| ctx.setting(k))
}

/// The tasks that ask the user to allow a guard their harness wants lowered (ADR-0115), while the harness's call waits:
/// the harness may read them, never answer or act on them.
pub(crate) static GUARD_TASKS: std::sync::LazyLock<std::sync::Mutex<std::collections::BTreeSet<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::BTreeSet::new()));

/// Whether `id` is a task asking the user about their harness's guard change.
pub(crate) fn guard_task(id: &str) -> bool {
    GUARD_TASKS.lock().is_ok_and(|g| g.contains(id))
}

/// The catalogue commands that change the user's setup and that an agent reaches as a card (ADR-0113); the crew and
/// connector ones are wisp's own tools (`wisp_crew_add`, `wisp_connector_add`).
const SETUP_COMMANDS: [&str; 1] = ["runtime.projectAdd"];

/// Whether this caller asks for setup changes on a card: a task's agent, not a capped one (v1 offered a capped task
/// none of these).
fn setup_asks(c: &wisp_kernel::Caller) -> bool {
    c.session.is_some() && c.set.starts_with("agent:session")
}

/// A key for a routed call (one per tool call).
fn rand_mid() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos() & u128::from(u64::MAX)).unwrap_or(0))
        ^ u64::from(std::process::id())
}

/// Who `call` speaks for, with its session's flags (and what `mcp.session@1` contributors add).
pub(crate) fn who(ctx: &Ctx, call: &Call, sessions: &Sessions) -> Who {
    let caller = call.caller();
    let info = caller.session.as_deref().map(|s| sessions.get(s)).unwrap_or_default();
    let _ = ctx;
    Who::of(&caller.set, caller.session.as_deref(), info, VERSION)
}

fn route(sessions: Arc<Sessions>) -> Route {
    Route {
        method: "POST",
        pattern: wisp_mcp_api::MCP_PATH,
        permission: "mcp:call",
        doc: true,
        summary: "MCP over HTTP: one JSON-RPC message or a batch (initialize, tools/list, tools/call)",
        raw: false,
        max_body: None,
        handler: Arc::new(move |ctx, call, req: Request| {
            let sessions = sessions.clone();
            Box::pin(async move {
                let text = String::from_utf8_lossy(&req.body).into_owned();
                let cwd = req.header("x-wisp-cwd").map(str::to_owned);
                match run::rpc(&ctx, &call, &sessions, &text, cwd).await {
                    Some(body) => Ok(Response::Text { status: 200, content_type: "application/json".into(), body }),
                    // only notifications: nothing to answer
                    None => Ok(Response::Text { status: 202, content_type: "application/json".into(), body: String::new() }),
                }
            })
        }),
    }
}

impl Plugin for McpPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let tools = crate::kinds::register(s);
        let sessions = Arc::new(Sessions(Mutex::default(), tools));
        s.route(route(sessions.clone()));
        let st = sessions.clone();
        s.command::<Rpc, _, _>(move |ctx, call, a: RpcArgs| {
            let st = st.clone();
            async move {
                let text = a.msg.to_string();
                Ok(run::rpc(&ctx, &call, &st, &text, a.cwd).await.and_then(|s| serde_json::from_str(&s).ok()))
            }
        });
        let st = sessions.clone();
        s.command::<Tools, _, _>(move |ctx, call, _a| {
            let st = st.clone();
            async move {
                let w = who(&ctx, &call, &st);
                let (ex, _) = extras_for(&ctx, &call);
                let all = wisp_mcp_core::tools_list(&w, &ex);
                let all = if auto_off(&ctx) { wisp_mcp_core::table::without_auto(&all).into_owned() } else { all };
                let list: Value = serde_json::from_str(&all).unwrap_or(Value::Null);
                Ok(list.get("tools").and_then(Value::as_array).cloned().unwrap_or_default())
            }
        });
        let st = sessions;
        s.command::<SessionSet, _, _>(move |_ctx, _call, a| {
            let st = st.clone();
            async move {
                st.set(a.session, a.info);
                Ok(())
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        assert_eq!(McpPlugin.manifest().id, "wisp.mcp");
    }
}
