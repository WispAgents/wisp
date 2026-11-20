//! `wisp_launch` from a task's run (audit C1): the asking task, the named project, Worktree per task and the folder's
//! git repo read here; `wisp_mcp_core::launch` turns the tool's arguments into the `runtime.create` the user's launcher
//! sends, and a `machine` routes it there (`machines.route`, as a generated tool's `peer` does).

use serde_json::{Map, Value, json};
use wisp_kernel::{Call, Ctx};
use wisp_mcp_core::launch;
use wisp_runtime_api::commands as rt;

/// The asking task's own folder: its main checkout when it runs in a worktree, nothing when it runs in a scratch
/// folder (the new task gets one of its own).
async fn asking(ctx: &Ctx, call: &Call, session: &str) -> Option<String> {
    let s = ctx.run::<rt::Get>(call, rt::IdArgs { id: session.into() }).await.ok()?;
    let s = serde_json::to_value(s).ok()?;
    let root = s.pointer("/ext/wisp.worktrees/root").and_then(Value::as_str).filter(|r| !r.is_empty());
    let cwd = root.or_else(|| s.get("cwd").and_then(Value::as_str)).filter(|c| !c.is_empty())?;
    let scratch = wisp_kernel::services::paths(ctx).ok().map(|p| p.home().join("scratch"));
    if scratch.is_some_and(|d| std::path::Path::new(cwd).starts_with(d)) {
        return None;
    }
    Some(cwd.to_owned())
}

/// The folder of the project `a` names (by name, any case, or id), when it names one that is there. Read on
/// wisp.mcp's own grant (`resources:read`), as `wisp_available` lists them to the agent; the task is still created
/// as the agent, so its caps hold.
async fn project(ctx: &Ctx, a: &Map<String, Value>) -> Option<String> {
    let want = a.get("project").and_then(Value::as_str).map(str::trim).filter(|p| !p.is_empty())?;
    let args = wisp_resources_api::ExportArgs { kinds: vec!["project".into()], include: Vec::new() };
    let all = ctx.run::<wisp_resources_api::Export>(&ctx.root_call(), args).await.ok()?;
    let hit = all.iter().filter(|r| r.kind == "project").find(|r| r.id.as_deref() == Some(want) || r.name.eq_ignore_ascii_case(want));
    hit.and_then(|r| r.spec.get("path")?.as_str().map(str::to_owned))
}

/// Whether `dir` is in a git repo (a `.git` in it or a folder above).
async fn in_git(dir: &str) -> bool {
    let mut at = Some(std::path::Path::new(dir));
    while let Some(d) = at {
        if tokio::fs::metadata(d.join(".git")).await.is_ok() {
            return true;
        }
        at = d.parent();
    }
    false
}

/// Worktree per task (`worktrees.worktree`): on unless switched off.
fn trees_on(ctx: &Ctx) -> bool {
    !ctx.setting("worktrees.worktree").is_some_and(|v| v == "off" || v == false)
}

/// `formula` (ADR-0111): files in the asking task's own folder (one, a comma list or a list), or a formula inline; the
/// body resolved under it by `resources.overlay`, as `wisp "…" --formula` is, with the agent's own reach (review r194
/// #1: inside its task's folder only, no `file()`, no environment). A name that names nothing is the agent's error,
/// saying what there is.
async fn under_formula(ctx: &Ctx, call: &Call, body: Value, f: &Value, cwd: Option<&str>) -> Result<Value, String> {
    let mut args =
        wisp_resources_api::OverlayArgs { cwd: cwd.unwrap_or_default().to_owned(), task: Some(body.clone()), ..Default::default() };
    match f {
        Value::Object(inline) => {
            let mut doc = inline.clone();
            doc.insert("wisp".into(), json!(1));
            args.text = Value::Object(doc).to_string();
            args.name = "formula.json".into();
        }
        Value::String(list) => args.files = list.split(',').map(str::trim).filter(|x| !x.is_empty()).map(str::to_owned).collect(),
        Value::Array(list) => args.files = list.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        other => return Err(format!("formula: {other} isn't a formula: a file, a list of files, or one inline")),
    }
    let out = ctx.run::<wisp_resources_api::Overlay>(call, args).await.map_err(|e| e.message)?;
    if !out.problems.is_empty() {
        return Err(out.problems.join("; "));
    }
    // only what the formula names for it (its agent, its goal written in): never the overlay's raw task fields (review r194
    // round 2 #3)
    let mut b = body;
    let task = out.task.unwrap_or_default();
    for k in ["agent", "goal"] {
        if let Some(v) = task.get(k).filter(|v| !v.is_null()) {
            b[k] = v.clone();
        }
    }
    if out.overlay.as_object().is_some_and(|m| !m.is_empty()) {
        b["formula"] = out.overlay;
    }
    Ok(b)
}

/// The new task, made: here, or on the machine `a` names. Its `runtime.create` answer.
///
/// # Errors
/// A project or machine that isn't there, or what `runtime.create` refused.
pub(crate) async fn launch(ctx: &Ctx, call: &Call, session: &str, a: &Map<String, Value>) -> Result<Value, String> {
    let named = project(ctx, a).await;
    let mine = if a.get("cwd").and_then(Value::as_str).is_some_and(|c| !c.is_empty()) { None } else { asking(ctx, call, session).await };
    let cwd = launch::folder(a, named.as_deref(), mine.as_deref())?;
    let tree_on = match &cwd {
        Some(c) => trees_on(ctx) && in_git(c).await,
        None => false,
    };
    let mut body = launch::start_body(a, cwd.as_deref(), tree_on);
    if let Some(f) = a.get("formula").filter(|f| !f.is_null()) {
        body = under_formula(ctx, call, body, f, cwd.as_deref()).await?;
    }
    // the parent check is runtime.create's own (one place, review r194 round 2 #2)
    let Some(m) = a.get("machine").and_then(Value::as_str).map(str::trim).filter(|m| !m.is_empty() && *m != "here") else {
        let args = serde_json::from_value::<rt::StartArgs>(body).map_err(|e| format!("runtime.create: {e}"))?;
        let made = ctx.run::<rt::Create>(call, args).await.map_err(|e| e.message)?;
        return serde_json::to_value(made).map_err(|e| e.to_string());
    };
    // another machine: its runtime.create doesn't see this agent, so a task held below Full auto (its own mode, or a
    // channel's cap) starts none there (review r194 round 2 #2)
    let parent = ctx.run::<rt::Get>(&ctx.root_call(), rt::IdArgs { id: session.into() }).await.ok();
    let held = call.caller().set.starts_with("agent:capped") || parent.is_some_and(|p| p.mode != wisp_runtime_api::model::Mode::Auto);
    if held {
        return Err("Refused: a task an agent starts can't run on another machine while its own task is held below Full auto. Say so in your report; the user can start it there in wisp.".into());
    }
    let list = ctx.run::<wisp_machines_api::List>(call, wisp_machines_api::NoArgs {}).await.map_err(|e| e.message)?;
    let list = serde_json::to_value(list).unwrap_or_default();
    let pid = wisp_mcp_core::generated::find_peer(&list, m).ok_or_else(|| format!("No machine “{m}”. wisp_available lists them."))?;
    let mid = format!("launch-{session}-{}", ctx.clock().map_or(0, |c| c.now_ms()));
    let args = wisp_machines_api::RouteArgs { id: Some(pid), cmd: "runtime.create".into(), args: body, mid };
    ctx.run::<wisp_machines_api::Route>(call, args).await.map_err(|e| e.message).map(|v| json!(v))
}
