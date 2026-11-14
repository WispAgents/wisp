//! A delegate on another machine (ADR-0059): the lead's work carried there, and its changes brought back.
//!
//! Here (`machines.carry`): the lead's folder packed (a bundle of its HEAD and branch, and its uncommitted changes, new
//! files too, read through a scratch index so the lead's own index is never touched), uploaded as raw bytes over the
//! machine's link (`POST /api/machines/carry/<key>/<part>`: the sync stream's 2 MB body cap would refuse a real
//! repo), then taken in there (`machines.takeIn`, over `/api/cmd`).
//!
//! There (`machines.takeIn`, `machines.changes`): the copy lives only under this wisp's own folder
//! (`<wisp>/plugins/wisp.machines/copies/<key>/<name>`), never a checkout of the user's; what it was when it came is
//! a tree kept in its `.git`, so its changes are the difference from that (no commit made, no ref moved).
//!
//! Here again (`machines.bring`): those changes, applied to the lead's folder; a patch that clashes is kept in
//! `<wisp>/plugins/wisp.machines/patches` and nothing in the lead's folder moves.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use wisp_kernel::http::{Response, Route};
use wisp_kernel::services::Paths;
use wisp_kernel::{Command, Ctx, Setup, WireError};
use wisp_machines_api::{self as api, Body, ChannelRequest};
use wisp_machines_core::carry as core;
use wisp_proc::{Env, ExecOpts, Proc};

use crate::error::{MachinesError, Res};
use crate::{Slot, State, state};

/// `<wisp>/plugins/wisp.machines`.
fn plugin_dir(ctx: &Ctx) -> wisp_kernel::Result<PathBuf> {
    Ok(ctx.service::<Paths>()?.plugin_dir(api::PLUGIN))
}

fn temp(what: &str) -> PathBuf {
    std::env::temp_dir().join(format!("wisp-carry-{}.{what}", wisp_kernel::ids::uuid7()))
}

/// `git <args>` in `cwd` (with `index`: through that scratch index); its stdout, or what it said.
async fn git(proc: &Proc, cwd: &Path, args: &[&str], index: Option<&Path>, input: Option<Vec<u8>>) -> Result<String, String> {
    let mut env = Env::default().with("GIT_TERMINAL_PROMPT", "0");
    if let Some(i) = index {
        env = env.with("GIT_INDEX_FILE", i.display().to_string());
    }
    let opts = ExecOpts {
        cwd: Some(cwd.to_owned()),
        env,
        input,
        timeout: Some(Duration::from_secs(300)),
        max_output: core::MAX_PATCH + (1 << 20),
        ..ExecOpts::default()
    };
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let r = proc.exec("git", &args, opts).await.map_err(|e| e.to_string())?;
    if r.exit.success() { Ok(r.stdout_text()) } else { Err(format!("{}{}", r.stdout_text(), r.stderr_text()).trim().to_owned()) }
}

/// The folder's whole working state (tracked, changed and new files, not ignored ones) as a tree, read through a
/// scratch copy of its index: the folder's own index stays as it was.
async fn tree_of(proc: &Proc, dir: &Path) -> Result<String, String> {
    let at = git(proc, dir, &["rev-parse", "--git-path", "index"], None, None).await?;
    let real = dir.join(at.trim());
    let scratch = temp("index");
    if tokio::fs::copy(&real, &scratch).await.is_err() {
        let _ = tokio::fs::remove_file(&scratch).await;
    }
    let out = async {
        git(proc, dir, &["add", "-A"], Some(&scratch), None).await?;
        git(proc, dir, &["write-tree"], Some(&scratch), None).await
    }
    .await;
    let _ = tokio::fs::remove_file(&scratch).await;
    out.map(|t| t.trim().to_owned())
}

/// The lead's folder, ready to go: the bundle's bytes, the uncommitted changes, and what `takeIn` needs.
struct Pack {
    bundle: Vec<u8>,
    patch: String,
    take: api::TakeInArgs,
}

async fn pack(proc: &Proc, cwd: &str, key: &str) -> Res<Pack> {
    let dir = PathBuf::from(cwd);
    let line = |r: Result<String, String>| r.map(|x| x.trim().to_owned()).unwrap_or_default();
    let head = line(git(proc, &dir, &["rev-parse", "--verify", "HEAD"], None, None).await);
    if head.is_empty() {
        return Err(MachinesError::Wire(WireError::invalid(core::not_a_repo(cwd))));
    }
    let branch = line(git(proc, &dir, &["symbolic-ref", "--quiet", "--short", "HEAD"], None, None).await);
    let tmp = temp("bundle");
    let tmp_s = tmp.display().to_string();
    let made = match core::pack_commands(&tmp_s, &branch).first() {
        Some(args) => git(proc, &dir, &args.iter().map(String::as_str).collect::<Vec<_>>(), None, None).await,
        None => Err(String::new()),
    };
    let bundle = tokio::fs::read(&tmp).await;
    let _ = tokio::fs::remove_file(&tmp).await;
    let bundle = match (made, bundle) {
        (Ok(_), Ok(b)) => b,
        (Err(e), _) => return Err(MachinesError::Wire(WireError::internal(format!("git bundle: {e}")))),
        (_, Err(e)) => return Err(MachinesError::Wire(WireError::internal(format!("the bundle: {e}")))),
    };
    if bundle.len() > core::MAX_BUNDLE {
        return Err(MachinesError::Wire(WireError::invalid(core::too_big(bundle.len() >> 20))));
    }
    // what isn't committed, new files too: the difference from HEAD to the folder's whole state
    let patch = match tree_of(proc, &dir).await {
        Ok(tree) => git(proc, &dir, &["diff", "--binary", "HEAD", &tree], None, None).await.unwrap_or_default(),
        Err(e) => return Err(MachinesError::Wire(WireError::internal(format!("the uncommitted changes: {e}")))),
    };
    let take = api::TakeInArgs {
        key: core::segment(key),
        name: core::name_of(cwd),
        head,
        branch,
        author_name: line(git(proc, &dir, &["config", "user.name"], None, None).await),
        author_email: line(git(proc, &dir, &["config", "user.email"], None, None).await),
    };
    Ok(Pack { bundle, patch, take })
}

/// One raw upload to the machine (`POST /api/machines/carry/<key>/<part>`).
async fn upload(st: &Arc<State>, id: &str, key: &str, part: &str, bytes: Vec<u8>) -> Res<()> {
    let row = st.peer(id)?;
    let _ = crate::manager::secret_of(st, id).await;
    let ch = st.channel(id)?;
    let headers = vec![("content-type".into(), "application/octet-stream".into()), ("content-length".into(), bytes.len().to_string())];
    let req = ChannelRequest {
        method: "POST".into(),
        path: format!("/machines/carry/{key}/{part}"),
        headers,
        body: Body::Bytes(bytes),
        timeout_ms: Some(600_000),
        passive: false,
    };
    let r = ch.request(req).await.map_err(MachinesError::Wire)?;
    let status = r.status;
    if status >= 400 {
        let v = wisp_machines_core::reply::parse_reply(&r.bytes(1 << 20).await);
        let (message, hold) = wisp_machines_core::reply::reply_error(&row.name, status, &v);
        return Err(MachinesError::Peer { message, status, hold });
    }
    Ok(())
}

/// A typed command run by the machine's wisp (`POST /api/cmd`), with a timeout long enough for git.
async fn on_machine<C: Command>(st: &Arc<State>, id: &str, args: C::Args) -> Res<C::Out> {
    let body = json!({ "cmd": C::ID, "args": serde_json::to_value(args).unwrap_or(Value::Null) });
    let v = crate::service::json(st.clone(), id.to_owned(), "POST".into(), "/cmd".into(), Some(body), Some(600_000)).await?;
    let out = if v.get("ok") == Some(&Value::Bool(true)) { v.get("result").cloned().unwrap_or(Value::Null) } else { v };
    serde_json::from_value(out).map_err(|e| MachinesError::Wire(WireError::internal(format!("{}: {e}", C::ID))))
}

/// `machines.carry`: the lead's work copied onto the machine.
async fn carry(st: &Arc<State>, a: api::CarryArgs) -> Res<api::Carried> {
    let row = st.peer(&a.id)?;
    let proc = st.ctx.service::<Proc>().map_err(MachinesError::Wire)?;
    let p = pack(&proc, &a.cwd, &a.key).await?;
    let key = p.take.key.clone();
    upload(st, &a.id, &key, "bundle", p.bundle).await?;
    upload(st, &a.id, &key, "patch", p.patch.into_bytes()).await?;
    let took = on_machine::<api::TakeIn>(st, &a.id, p.take).await?;
    Ok(api::Carried { cwd: took.cwd, machine: row.name })
}

fn wire(e: impl std::fmt::Display) -> WireError {
    WireError::internal(e.to_string())
}

/// `machines.takeIn` (on the machine): the uploaded work, a fresh copy under this wisp's own folder.
pub(crate) async fn take_in(ctx: &Ctx, a: &api::TakeInArgs) -> wisp_kernel::Result<api::TakenIn> {
    let proc = ctx.service::<Proc>()?;
    let plugin = plugin_dir(ctx)?;
    let dir = core::copy_dir(&plugin, &a.key, &a.name);
    let (Some(bundle), Some(patch)) = (core::incoming(&plugin, &a.key, "bundle"), core::incoming(&plugin, &a.key, "patch")) else {
        return Err(WireError::invalid("no such upload"));
    };
    if !tokio::fs::metadata(&bundle).await.is_ok_and(|m| m.is_file()) {
        return Err(WireError::not_found("Nothing was carried here for that copy."));
    }
    if a.head.is_empty() || !a.head.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(WireError::invalid("the lead's commit isn't a commit id"));
    }
    let Some(parent) = dir.parent().map(Path::to_owned) else { return Err(WireError::internal("the copy has no folder")) };
    // the same copy again (the member's next job): a fresh copy of what came now; its last changes went back already
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&parent).await.map_err(wire)?;
    let (b, d) = (bundle.display().to_string(), dir.display().to_string());
    for args in core::unpack_commands(&b, &d, &a.head, &a.branch) {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        git(&proc, &parent, &args, None, None).await.map_err(|e| WireError::internal(format!("git {}: {e}", args[0])))?;
    }
    if !a.author_name.is_empty() {
        let _ = git(&proc, &dir, &["config", "user.name", &a.author_name], None, None).await;
    }
    if !a.author_email.is_empty() {
        let _ = git(&proc, &dir, &["config", "user.email", &a.author_email], None, None).await;
    }
    let uncommitted = tokio::fs::read(&patch).await.unwrap_or_default();
    if !uncommitted.iter().all(u8::is_ascii_whitespace) {
        git(&proc, &dir, &["apply", "--whitespace=nowarn", "-"], None, Some(uncommitted))
            .await
            .map_err(|e| WireError::internal(format!("the lead's uncommitted changes didn't apply: {e}")))?;
    }
    let _ = tokio::fs::remove_file(&bundle).await;
    let _ = tokio::fs::remove_file(&patch).await;
    // what it was when it came: its changes go back as the difference from this
    let base = tree_of(&proc, &dir).await.map_err(|e| WireError::internal(format!("the copy's state: {e}")))?;
    tokio::fs::write(dir.join(".git").join(core::BASE_FILE), base).await.map_err(wire)?;
    Ok(api::TakenIn { cwd: d })
}

/// `machines.changes` (on the machine): a carried copy's changes since it came in, new files too.
pub(crate) async fn changes(ctx: &Ctx, cwd: &str) -> wisp_kernel::Result<api::Changes> {
    let proc = ctx.service::<Proc>()?;
    if !core::is_copy(&plugin_dir(ctx)?, cwd) {
        return Err(WireError::forbidden(core::not_a_copy()));
    }
    let dir = PathBuf::from(cwd);
    let base = tokio::fs::read_to_string(dir.join(".git").join(core::BASE_FILE))
        .await
        .map_err(|_| WireError::not_found("That copy didn't come in through wisp."))?;
    let now = tree_of(&proc, &dir).await.map_err(|e| WireError::internal(format!("the copy's state: {e}")))?;
    let patch = git(&proc, &dir, &["diff", "--binary", base.trim(), &now], None, None)
        .await
        .map_err(|e| WireError::internal(format!("git diff: {e}")))?;
    if patch.len() > core::MAX_PATCH {
        return Err(WireError::invalid(format!("Its changes are too big to send back ({} MB).", patch.len() >> 20)));
    }
    Ok(api::Changes { patch })
}

/// A copy's patch into the lead's folder `into`; kept in `<plugin>/patches` when it doesn't go in.
pub(crate) async fn apply_back(proc: &Proc, plugin: &Path, into: &str, patch: String) -> api::Brought {
    if patch.trim().is_empty() {
        return api::Brought::default();
    }
    let files = core::files_in(&patch);
    match git(proc, Path::new(into), &["apply", "--whitespace=nowarn", "-"], None, Some(patch.clone().into_bytes())).await {
        Ok(_) => api::Brought { files, error: None, kept: None },
        Err(why) => {
            let kept = plugin.join(core::PATCHES).join(format!("{}.patch", wisp_kernel::ids::uuid7()));
            let saved = match kept.parent() {
                Some(p) => tokio::fs::create_dir_all(p).await.is_ok() && tokio::fs::write(&kept, patch).await.is_ok(),
                None => false,
            };
            let at = if saved { kept.display().to_string() } else { "nowhere (it couldn't be saved)".to_owned() };
            api::Brought { files, error: Some(core::clash(&at, &why)), kept: saved.then(|| kept.display().to_string()) }
        }
    }
}

/// `machines.bring`: the copy's changes there, into the lead's folder here.
async fn bring(st: &Arc<State>, a: api::BringArgs) -> Res<api::Brought> {
    st.peer(&a.id)?;
    let patch = on_machine::<api::ChangesOf>(st, &a.id, api::ChangesArgs { cwd: a.from.clone() }).await?.patch;
    let proc = st.ctx.service::<Proc>().map_err(MachinesError::Wire)?;
    let plugin = plugin_dir(&st.ctx).map_err(MachinesError::Wire)?;
    Ok(apply_back(&proc, &plugin, &a.into, patch).await)
}

/// The upload route (`POST /api/machines/carry/:key/:part`, on the machine): the bytes, kept for `machines.takeIn`.
fn upload_route() -> Route {
    Route {
        method: "POST",
        pattern: "/machines/carry/:key/:part",
        permission: "machines:takeIn",
        doc: false,
        summary: "A delegate's carried work (a bundle, a patch), kept for machines.takeIn",
        raw: false,
        max_body: Some(core::MAX_BUNDLE + (1 << 20)),
        handler: Arc::new(|ctx, _call, req| {
            Box::pin(async move {
                let plugin = plugin_dir(&ctx)?;
                let Some(to) = core::incoming(&plugin, req.param("key").unwrap_or_default(), req.param("part").unwrap_or_default()) else {
                    return Err(WireError::invalid("a carried part is a bundle or a patch"));
                };
                if let Some(p) = to.parent() {
                    tokio::fs::create_dir_all(p).await.map_err(wire)?;
                }
                tokio::fs::write(&to, &req.body).await.map_err(wire)?;
                Ok(Response::Json { status: 200, body: json!({ "ok": true }) })
            })
        }),
    }
}

pub(crate) fn register(s: &mut Setup, slot: &Slot) {
    s.route(upload_route());
    s.command::<api::TakeIn, _, _>(|ctx, _call, a| async move { take_in(&ctx, &a).await });
    s.command::<api::ChangesOf, _, _>(|ctx, _call, a| async move { changes(&ctx, &a.cwd).await });
    let sl = slot.clone();
    s.command::<api::Carry, _, _>(move |_ctx, _call, a| {
        let sl = sl.clone();
        async move { carry(&state(&sl)?, a).await.map_err(WireError::from) }
    });
    let sl = slot.clone();
    s.command::<api::Bring, _, _>(move |_ctx, _call, a| {
        let sl = sl.clone();
        async move { bring(&state(&sl)?, a).await.map_err(WireError::from) }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sh(at: &Path, args: &[&str]) {
        let o = std::process::Command::new("git").current_dir(at).args(args).output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_bring_that_clashes_keeps_the_patch_and_moves_nothing() {
        let root = temp("clash");
        let (lead, plugin) = (root.join("lead"), root.join("plugin"));
        std::fs::create_dir_all(&lead).unwrap();
        sh(&lead, &["init", "-q"]);
        std::fs::write(lead.join("math.js"), "a - b\n").unwrap();
        sh(&lead, &["add", "."]);
        sh(&lead, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "one"]);
        // the lead changed the same line meanwhile
        std::fs::write(lead.join("math.js"), "a * b\n").unwrap();
        let patch = "diff --git a/math.js b/math.js\n--- a/math.js\n+++ b/math.js\n@@ -1 +1 @@\n-a - b\n+a + b\n".to_owned();
        let out = apply_back(&wisp_proc::local(), &plugin, lead.to_str().unwrap(), patch.clone()).await;
        assert_eq!(out.files, 1);
        let kept = out.kept.clone().unwrap();
        assert!(out.error.as_deref().unwrap().contains(&kept), "{out:?}");
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), patch, "the work isn't lost");
        assert_eq!(std::fs::read_to_string(lead.join("math.js")).unwrap(), "a * b\n", "nothing here moved");
        // a patch that fits goes in
        std::fs::write(lead.join("math.js"), "a - b\n").unwrap();
        let ok = apply_back(&wisp_proc::local(), &plugin, lead.to_str().unwrap(), patch).await;
        assert_eq!((ok.files, ok.error.is_none()), (1, true), "{ok:?}");
        assert_eq!(std::fs::read_to_string(lead.join("math.js")).unwrap(), "a + b\n");
        let _ = std::fs::remove_dir_all(root);
    }
}
