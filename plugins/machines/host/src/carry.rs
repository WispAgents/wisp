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
pub(crate) fn plugin_dir(ctx: &Ctx) -> wisp_kernel::Result<PathBuf> {
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
    // a patch cut short would be another patch: refused
    if r.truncated {
        return Err(format!("git wrote more than {} MB", (core::MAX_PATCH + (1 << 20)) >> 20));
    }
    if r.exit.success() { Ok(r.stdout_text()) } else { Err(format!("{}{}", r.stdout_text(), r.stderr_text()).trim().to_owned()) }
}

/// The folder's whole working state (tracked, changed and new files, not ignored ones) as a tree, read through a
/// scratch copy of its index: the folder's own index stays as it was.
async fn tree_of(proc: &Proc, dir: &Path) -> Result<String, String> {
    let at = git(proc, dir, &["rev-parse", "--git-path", "index"], None, None).await?;
    let real = dir.join(at.trim());
    let scratch = temp("index");
    // with the real one's mtime (git's racy-clean check), or none: an empty index re-hashes every file
    wisp_proc::copy_index(real, scratch.clone()).await;
    let out = async {
        git(proc, dir, &["add", "-A"], Some(&scratch), None).await?;
        git(proc, dir, &["write-tree"], Some(&scratch), None).await
    }
    .await;
    let _ = tokio::fs::remove_file(&scratch).await;
    out.map(|t| t.trim().to_owned())
}

/// A folder here, ready to go: the bundle's bytes, the uncommitted changes, and what the far side needs.
struct Pack {
    bundle: Vec<u8>,
    patch: String,
    sent: api::Sent,
}

async fn pack(proc: &Proc, cwd: &str, key: &str, all: bool) -> Res<Pack> {
    let dir = PathBuf::from(cwd);
    let line = |r: Result<String, String>| r.map(|x| x.trim().to_owned()).unwrap_or_default();
    let head = line(git(proc, &dir, &["rev-parse", "--verify", "HEAD"], None, None).await);
    if head.is_empty() {
        return Err(MachinesError::Wire(WireError::invalid(core::not_a_repo(cwd))));
    }
    let branch = line(git(proc, &dir, &["symbolic-ref", "--quiet", "--short", "HEAD"], None, None).await);
    let tmp = temp("bundle");
    let tmp_s = tmp.display().to_string();
    let made = match core::pack_commands(&tmp_s, &branch, all).first() {
        Some(args) => git(proc, &dir, &args.iter().map(String::as_str).collect::<Vec<_>>(), None, None).await,
        None => Err(String::new()),
    };
    // too big: refused before it's read, and before anything goes
    if let Ok(m) = tokio::fs::metadata(&tmp).await
        && usize::try_from(m.len()).unwrap_or(usize::MAX) > core::MAX_BUNDLE
    {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(MachinesError::Wire(WireError::new(
            wisp_kernel::Code::TooLarge,
            core::too_big(usize::try_from(m.len() >> 20).unwrap_or(usize::MAX)),
        )));
    }
    let bundle = tokio::fs::read(&tmp).await;
    let _ = tokio::fs::remove_file(&tmp).await;
    let bundle = match (made, bundle) {
        (Ok(_), Ok(b)) => b,
        (Err(e), _) => return Err(MachinesError::Wire(WireError::internal(format!("git bundle: {e}")))),
        (_, Err(e)) => return Err(MachinesError::Wire(WireError::internal(format!("the bundle: {e}")))),
    };
    // what isn't committed, new files too: the difference from HEAD to the folder's whole state
    let patch = match tree_of(proc, &dir).await {
        Ok(tree) => git(proc, &dir, &["diff", "--binary", "HEAD", &tree], None, None)
            .await
            .map_err(|e| MachinesError::Wire(WireError::internal(format!("the uncommitted changes: {e}"))))?,
        Err(e) => return Err(MachinesError::Wire(WireError::internal(format!("the uncommitted changes: {e}")))),
    };
    let sent = api::Sent {
        key: core::segment(key),
        head,
        branch,
        origin: line(git(proc, &dir, &["remote", "get-url", "origin"], None, None).await),
        author_name: line(git(proc, &dir, &["config", "user.name"], None, None).await),
        author_email: line(git(proc, &dir, &["config", "user.email"], None, None).await),
    };
    Ok(Pack { bundle, patch, sent })
}

/// One raw upload to the machine (`POST /api<to>/<key>/<part>`).
async fn upload(st: &Arc<State>, id: &str, to: &str, key: &str, part: &str, bytes: Vec<u8>) -> Res<()> {
    let row = st.peer(id)?;
    let _ = crate::manager::secret_of(st, id).await;
    let ch = st.channel(id)?;
    let headers = vec![("content-type".into(), "application/octet-stream".into()), ("content-length".into(), bytes.len().to_string())];
    let req = ChannelRequest {
        method: "POST".into(),
        path: format!("{to}/{key}/{part}"),
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
pub(crate) async fn on_machine<C: Command>(st: &Arc<State>, id: &str, args: C::Args) -> Res<C::Out> {
    let body = json!({ "cmd": C::ID, "args": serde_json::to_value(args).unwrap_or(Value::Null) });
    let v = crate::service::json(st.clone(), id.to_owned(), "POST".into(), "/cmd".into(), Some(body), Some(600_000)).await?;
    let out = if v.get("ok") == Some(&Value::Bool(true)) { v.get("result").cloned().unwrap_or(Value::Null) } else { v };
    serde_json::from_value(out).map_err(|e| MachinesError::Wire(WireError::internal(format!("{}: {e}", C::ID))))
}

/// `machines.send`: the folder packed and its two parts uploaded raw to the machine's route `to` (ADR-0060).
async fn send(st: &Arc<State>, a: &api::SendArgs) -> Res<api::Sent> {
    st.peer(&a.id)?;
    if !core::send_route_ok(&a.to) {
        return Err(MachinesError::Wire(WireError::invalid(format!("{} isn't an upload route", a.to))));
    }
    let proc = st.ctx.service::<Proc>().map_err(MachinesError::Wire)?;
    let p = pack(&proc, &a.cwd, &a.key, a.all).await?;
    let key = p.sent.key.clone();
    upload(st, &a.id, &a.to, &key, "bundle", p.bundle).await?;
    upload(st, &a.id, &a.to, &key, "patch", p.patch.into_bytes()).await?;
    Ok(p.sent)
}

/// `machines.carry`: the lead's work copied onto the machine.
async fn carry(st: &Arc<State>, a: api::CarryArgs) -> Res<api::Carried> {
    let row = st.peer(&a.id)?;
    let sent =
        send(st, &api::SendArgs { id: a.id.clone(), cwd: a.cwd.clone(), key: a.key, to: core::CARRY_ROUTE.into(), all: false }).await?;
    let take = api::TakeInArgs {
        key: sent.key,
        name: core::name_of(&a.cwd),
        head: sent.head,
        branch: sent.branch,
        author_name: sent.author_name,
        author_email: sent.author_email,
    };
    let took = on_machine::<api::TakeIn>(st, &a.id, take).await?;
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
    s.command::<api::DropCopies, _, _>(|ctx, _call, a| async move { crate::uncarry::drop_copies(&plugin_dir(&ctx)?, &a.lead).await });
    let sl = slot.clone();
    s.command::<api::Send, _, _>(move |_ctx, _call, a| {
        let sl = sl.clone();
        async move { send(&state(&sl)?, &a).await.map_err(WireError::from) }
    });
    let sl = slot.clone();
    s.command::<api::Uncarry, _, _>(move |_ctx, _call, a| {
        let sl = sl.clone();
        async move { crate::uncarry::uncarry(&state(&sl)?, a).await.map_err(WireError::from) }
    });
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

    fn stamp(p: &Path, when: std::time::SystemTime) {
        std::fs::OpenOptions::new().write(true).open(p).unwrap().set_modified(when).unwrap();
    }

    /// `serve_delegate_machine`'s flake (ubuntu CI): the copy checked out, then the delegate's same-size edit of
    /// math.js in that same second; its changes read through a scratch index that Linux's copy stamped later, so
    /// git took math.js as unchanged and only notes.txt came back. The stamps are pinned here, as on that run.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_same_size_edit_in_the_index_s_own_second_is_a_change() {
        let dir = temp("racy");
        std::fs::create_dir_all(&dir).unwrap();
        sh(&dir, &["init", "-q"]);
        // ctime can't be pinned: the same second's ctime is what CI had
        sh(&dir, &["config", "core.trustctime", "false"]);
        let then = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        std::fs::write(dir.join("math.js"), "a - b\n").unwrap();
        stamp(&dir.join("math.js"), then);
        sh(&dir, &["add", "."]);
        sh(&dir, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "one"]);
        stamp(&dir.join(".git/index"), then);
        let proc = wisp_proc::local();
        let base = tree_of(&proc, &dir).await.unwrap();
        std::fs::write(dir.join("math.js"), "a + b\n").unwrap();
        stamp(&dir.join(".git/index"), then);
        stamp(&dir.join("math.js"), then);
        let now = tree_of(&proc, &dir).await.unwrap();
        let diff = git(&proc, &dir, &["diff", "--name-only", &base, &now], None, None).await.unwrap();
        assert_eq!(diff.trim(), "math.js", "the edit is seen");
        let _ = std::fs::remove_dir_all(dir);
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
