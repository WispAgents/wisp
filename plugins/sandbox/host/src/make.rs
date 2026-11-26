//! Making a sandbox for an owner, waiting for it, carrying the project in, starting work there, and dropping it
//! (ADR-0047). The grid is reached through wisp.grid-client, a sandbox through `machines.route` (the project itself
//! through `machines.send`, raw: ADR-0060), git through proc.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use wisp_kernel::services::Paths;
use wisp_kernel::{Call, Command, Ctx, Result, WireError};
use wisp_machines_api::PeerState;
use wisp_proc::{ExecOpts, Proc};
use wisp_runtime_api::commands as rt;
use wisp_sandbox_api as api;
use wisp_sandbox_core::{self as core, Held, Owner};

use crate::calls;

/// How long a new sandbox has to come up (a cold start is ~25 s on a real grid).
const UP_MS: u64 = 90_000;
const POLL_MS: u64 = 1_500;

pub(crate) fn now(ctx: &Ctx) -> i64 {
    ctx.clock().map_or(0, |c| c.now_ms())
}

/// The sandboxes this computer keeps.
pub(crate) async fn held(ctx: &Ctx, call: &Call) -> Vec<Held> {
    core::held(&calls::machines(ctx, call).await)
}

/// Sets a sandbox machine's owner (`data.owner`; machines keeps its other fields).
pub(crate) async fn set_owner(ctx: &Ctx, call: &Call, machine: &str, owner: &Owner) -> Result<()> {
    calls::set_data(ctx, call, machine, BTreeMap::from([("owner".to_owned(), owner.to_value())])).await
}

/// The sign-ins every sandbox starts with (`claude` when the user signed in for sandboxes). A grid that can't be
/// asked says why, rather than passing for "not signed in" (which sent a signed-in user to Settings).
pub(crate) async fn logins(ctx: &Ctx, call: &Call) -> Result<Vec<String>> {
    let v = calls::grid_info(ctx, call).await?;
    Ok(core::logins_of(v.get("logins")))
}

/// A sandbox for `owner`, up and answering: `(machine, peer, sandbox id)`. A full grid fails at once. Once the grid
/// made it, a failure after (its owner's write, its wait) drops it: left behind, it held one of the user's few
/// sandboxes for good, with no owner to go with.
pub(crate) async fn make(ctx: &Ctx, call: &Call, owner: &Owner, name: &str) -> Result<(String, String, String)> {
    let made = calls::grid_new(ctx, call, name).await?;
    let (machine, sandbox) = (made.machine, made.id);
    if machine.is_empty() {
        return Err(WireError::internal("the sandbox was made but no machine here keeps it"));
    }
    let up = async {
        set_owner(ctx, call, &machine, owner).await?;
        wait_up(ctx, call, &machine).await
    };
    match up.await {
        Ok(peer) => Ok((machine, peer, sandbox)),
        Err(e) => {
            drop_one(ctx, call, &machine, &sandbox).await;
            Err(e)
        }
    }
}

/// [`UP_MS`] from now, on the plugin's clock.
fn deadline(ctx: &Ctx) -> Result<i64> {
    Ok(ctx.clock()?.now_ms() + i64::try_from(UP_MS).unwrap_or(i64::MAX))
}

/// Waits for a sandbox's wisp to answer; its peer id.
///
/// As v1's `waitOnline`: a box that is off or in error (its tunnel failed once, a first connect that failed) gets one
/// fresh connect per wait, so it's usable again the next time it's used, and the machine card shows *connecting*
/// meanwhile. In error after that try, the wait fails at once with the box's own reason: machines put it in error
/// because retrying wouldn't help (a lost tunnel it retries itself, with backoff, as *reconnecting*).
async fn wait_up(ctx: &Ctx, call: &Call, machine: &str) -> Result<String> {
    let clock = ctx.clock()?;
    let until = deadline(ctx)?;
    let mut tried = false;
    loop {
        let m = calls::machine(ctx, call, machine).await?;
        let state = m.state;
        if state == PeerState::Online {
            // the app reaches it as the peer named by this machine's own id (`/api/peer/<id>`)
            return Ok(machine.to_owned());
        }
        if state == PeerState::Error && tried {
            let why = if m.error.is_empty() { "no answer" } else { m.error.as_str() };
            return Err(WireError::unavailable(format!("The sandbox didn't start: {why}")));
        }
        if state == PeerState::Off || state == PeerState::Error {
            calls::connect(ctx, call, machine).await;
            tried = true;
        }
        // a new box sleeps until it's used: one real request wakes it (a passive watch never does). Once it answers,
        // its link connects now: machines' watch of an asleep box only looks every 20 s
        if state == PeerState::Asleep
            && calls::wake(ctx, call, machine).await
            && calls::machine(ctx, call, machine).await.is_ok_and(|m| m.state == PeerState::Asleep)
        {
            calls::connect(ctx, call, machine).await;
        }
        if clock.now_ms() > until {
            let why = if m.error.is_empty() { "it didn't answer in 90 s" } else { m.error.as_str() };
            let state = serde_json::to_value(state).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            return Err(WireError::unavailable(format!("The sandbox didn't come up ({state}): {why}")));
        }
        clock.sleep(POLL_MS).await;
    }
}

/// Command `C` on a sandbox the work there depends on (its seed, its task), waited for up to [`UP_MS`].
///
/// `machines.route` answers "waiting for <box> to wake" (`retry: true`) after 30 s while the command is still on its
/// way (the link keeps it and sends it after the next welcome), and "offline" (`retry: true`) when the link is down.
/// Either way the sandbox is waited for as at its start ([`wait_up`]: woken, connected, or failed at once with its
/// reason) and the command asked again under the **same** mid, over the link: the sandbox's wisp runs one mid once.
async fn route_up<C: Command>(ctx: &Ctx, call: &Call, machine: &str, args: C::Args) -> Result<C::Out> {
    let clock = ctx.clock()?;
    let until = deadline(ctx)?;
    let (args, mid) = (calls::to_args::<C>(args)?, format!("sbx-{}", wisp_kernel::ids::uuid7()));
    loop {
        match calls::route_as::<C>(ctx, call, machine, args.clone(), &mid).await {
            Err(e) if e.retry && clock.now_ms() < until => {
                tracing::info!(machine, cmd = C::ID, "sandbox: {}; waiting for it, then asking again", e.message);
                clock.sleep(POLL_MS).await;
                wait_up(ctx, call, machine).await?;
            }
            out => return out,
        }
    }
}

/// The project in, there: its folder in the sandbox (an empty `~/code/scratch` when there's none), and how its
/// `.wisp/sandbox.sh` went when it ran. `apart`: a folder of this seed's own (a crew member's sandbox, whose jobs run
/// side by side), else `~/code/<name>`.
async fn seed(ctx: &Ctx, call: &Call, machine: &str, cwd: Option<&str>, apart: bool) -> Result<(String, Option<api::SetupRun>)> {
    let args = match cwd.filter(|c| !c.is_empty()) {
        // the project goes raw (`machines.send`, every ref and what isn't committed), never inside a sync message
        // (2 MB at most): only the seed's own args go through `machines.route`
        Some(c) => {
            let key = format!("seed-{}", wisp_kernel::ids::uuid7());
            let sent = calls::send(ctx, call, machine, c, &key).await?;
            api::SeedArgs {
                name: if apart { core::seed_folder(c, &key) } else { core::name_of(c) },
                key: sent.key,
                branch: sent.branch,
                origin: sent.origin,
                author_name: sent.author_name,
                author_email: sent.author_email,
                ..api::SeedArgs::default()
            }
        }
        None => api::SeedArgs { name: "scratch".into(), ..api::SeedArgs::default() },
    };
    let out = route_up::<api::Seed>(ctx, call, machine, args).await?;
    if out.cwd.is_empty() {
        return Err(WireError::internal("the sandbox took the project but didn't say where"));
    }
    let setup = out.setup.map(|tail| api::SetupRun { ok: !out.setup_failed, tail });
    Ok((out.cwd, setup))
}

/// `sandbox.place`: the owner's sandbox from before (a member's own, a delegate's, a task's judges'), else a new one,
/// seeded.
pub(crate) async fn place(ctx: &Ctx, call: &Call, a: &api::PlaceArgs) -> Result<api::Placed> {
    let owner: Owner = serde_json::from_value(a.owner.clone())
        .map_err(|_| WireError::invalid("a sandbox's owner is a task, a delegate, a crew job or member, or a goal's judges"))?;
    // a member's own sandbox runs their jobs side by side: each seed in a folder of its own
    let apart = matches!(owner, Owner::Member { .. });
    if let Some(h) = core::reusable(&held(ctx, call).await, &owner) {
        let peer = wait_up(ctx, call, &h.machine).await?;
        let (cwd, setup) = seed(ctx, call, &h.machine, a.cwd.as_deref(), apart).await?;
        note_cwd(ctx, call, &h.machine, &cwd).await;
        return Ok(api::Placed { machine: h.machine.clone(), peer, sandbox: h.sandbox.clone(), cwd, made: false, setup });
    }
    let name = a.cwd.as_deref().map_or_else(|| "work".to_owned(), core::name_of);
    let (machine, peer, sandbox) = make(ctx, call, &owner, &name).await?;
    match seed(ctx, call, &machine, a.cwd.as_deref(), apart).await {
        Ok((cwd, setup)) => {
            note_cwd(ctx, call, &machine, &cwd).await;
            Ok(api::Placed { machine, peer, sandbox, cwd, made: true, setup })
        }
        Err(e) => {
            drop_one(ctx, call, &machine, &sandbox).await;
            Err(e)
        }
    }
}

/// The project's folder there, on the machine (`data.cwd`): its owner going asks there whether work is left.
async fn note_cwd(ctx: &Ctx, call: &Call, machine: &str, cwd: &str) {
    calls::warn(
        "noting a sandbox's folder",
        calls::set_data(ctx, call, machine, BTreeMap::from([("cwd".to_owned(), Value::from(cwd))])).await,
    );
}

/// A delegate's sandbox still holds work its lead never got: a patch kept here (a clash, a hold), or changes there not
/// brought back yet (one that can't be asked counts as holding some). Such a sandbox outlives its lead, for the user to
/// delete in Settings → Computers → Sandboxes.
pub(crate) async fn holds_work(ctx: &Ctx, call: &Call, h: &Held) -> bool {
    if !matches!(h.owner, Some(Owner::Delegate { .. })) {
        return false;
    }
    if h.kept {
        return true;
    }
    // one placed before its folder was noted: nothing to ask, so it goes with its lead as it did
    if h.cwd.is_empty() {
        return false;
    }
    match calls::route::<api::PatchCmd>(ctx, call, &h.machine, api::PatchArgs { cwd: h.cwd.clone() }).await {
        Ok(p) => !p.patch.trim().is_empty(),
        Err(e) => {
            tracing::info!(machine = %h.machine, "sandbox: couldn't ask what's left there ({}), so it stays", e.message);
            true
        }
    }
}

/// `sandbox.launch`: `/sandbox <prompt>` in New task. Refused up front when the harness can't start there.
pub(crate) async fn launch(ctx: &Ctx, call: &Call, a: &api::LaunchArgs) -> Result<api::Launched> {
    launch_with(ctx, call, a, &|_| Box::pin(async {})).await
}

/// Where a launch is, for its row (`sandbox.start`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// the grid makes the sandbox
    Make,
    /// it starts and answers
    Wake,
    /// the project goes in
    Copy,
    /// the task starts there
    Start,
}

/// Says a [`Step`] (writes the launch's row) before the launch goes on.
pub(crate) type StepFn = dyn Fn(Step) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync;

/// [`launch`], saying each [`Step`] as it gets there.
pub(crate) async fn launch_with(ctx: &Ctx, call: &Call, a: &api::LaunchArgs, step: &StepFn) -> Result<api::Launched> {
    let agent = a.agent.clone().unwrap_or_default();
    core::may_start(&agent, &logins(ctx, call).await?).map_err(WireError::invalid)?;
    let mode = match &a.mode {
        Some(m) => Some(
            serde_json::from_value::<wisp_runtime_api::model::Mode>(Value::String(m.clone()))
                .map_err(|_| WireError::invalid(format!("{m} isn't a mode (plan, edits or auto)")))?,
        ),
        None => None,
    };
    let name = a
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| a.prompt.lines().next().unwrap_or("task").chars().take(40).collect());
    let pending = Owner::Pending { pending: now(ctx) };
    step(Step::Make).await;
    let made = calls::grid_new(ctx, call, &name).await?;
    let (machine, sandbox) = (made.machine, made.id);
    if machine.is_empty() {
        return Err(WireError::internal("the sandbox was made but no machine here keeps it"));
    }
    if let Err(e) = set_owner(ctx, call, &machine, &pending).await {
        // no owner, nothing would ever drop it
        drop_one(ctx, call, &machine, &sandbox).await;
        return Err(e);
    }
    step(Step::Wake).await;
    let peer = match wait_up(ctx, call, &machine).await {
        Ok(p) => p,
        Err(e) => {
            drop_one(ctx, call, &machine, &sandbox).await;
            return Err(e);
        }
    };
    step(Step::Copy).await;
    let (cwd, setup) = match seed(ctx, call, &machine, a.cwd.as_deref(), false).await {
        Ok(c) => c,
        Err(e) => {
            drop_one(ctx, call, &machine, &sandbox).await;
            return Err(e);
        }
    };
    let args = rt::StartArgs {
        prompt: a.prompt.clone(),
        cwd: Some(cwd),
        title: a.title.clone(),
        // Auto picks Claude in a sandbox
        agent: Some(if agent == "auto" || agent.is_empty() { "claude".to_owned() } else { agent }),
        model: a.model.clone(),
        effort: a.effort.clone(),
        mode,
        goal: a.goal.clone(),
        ..rt::StartArgs::default()
    };
    step(Step::Start).await;
    let made = match route_up::<rt::Create>(ctx, call, &machine, args).await {
        Ok(m) => m,
        Err(e) => {
            drop_one(ctx, call, &machine, &sandbox).await;
            return Err(e);
        }
    };
    let task = made.id.0;
    set_owner(ctx, call, &machine, &Owner::Task { task: format!("{peer}:{task}") }).await?;
    Ok(api::Launched { machine, peer, task, sandbox, setup })
}

/// Deletes one on the grid, then its machine here. A grid that can't be reached marks it `dropping` for the sweep.
pub(crate) async fn drop_one(ctx: &Ctx, call: &Call, machine: &str, sandbox: &str) -> bool {
    match calls::grid_remove(ctx, call, sandbox).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(sandbox, "a sandbox's drop failed, the sweep tries again: {}", e.message);
            calls::warn(
                "marking a sandbox dropping",
                calls::set_data(ctx, call, machine, BTreeMap::from([("dropping".to_owned(), Value::Bool(true))])).await,
            );
            false
        }
    }
}

pub(crate) async fn drop_all(ctx: &Ctx, call: &Call, list: &[&Held]) -> u32 {
    let mut n = 0;
    for h in list {
        if drop_one(ctx, call, &h.machine, &h.sandbox).await {
            n += 1;
        }
    }
    n
}

/// `sandbox.bring`: a delegate's changes in its sandbox, applied to the lead's folder here (or kept in a file, `keep`).
/// Once they came back, in either place, the sandbox counts its changes from there (`sandbox.rebase`), so the next job
/// brings only its own.
pub(crate) async fn bring(ctx: &Ctx, call: &Call, a: &api::BringArgs) -> Result<api::Brought> {
    let got = calls::route::<api::PatchCmd>(ctx, call, &a.machine, api::PatchArgs { cwd: a.from.clone() }).await?;
    let proc = ctx.service::<Proc>()?;
    let plugin = ctx.service::<Paths>()?.plugin_dir(api::PLUGIN_ID);
    let out = take_back(&proc, &plugin, &a.into, a.keep, got.patch).await?;
    if out.kept.is_some() {
        let kept = BTreeMap::from([("kept".to_owned(), Value::Bool(true))]);
        calls::warn("marking a sandbox's patch kept", calls::set_data(ctx, call, &a.machine, kept).await);
    }
    let back = out.error.is_none() || out.kept.is_some();
    if back && !got.tree.is_empty() {
        let r = calls::route::<api::Rebase>(ctx, call, &a.machine, api::RebaseArgs { cwd: a.from.clone(), tree: got.tree }).await;
        calls::warn("moving a sandbox's base on", r);
    }
    Ok(out)
}

/// A sandbox's patch into `into`, or kept in `<plugin>/patches/<id>.patch` (`keep`, or when it doesn't apply), as
/// machines' carry keeps a copy's.
pub(crate) async fn take_back(proc: &Proc, plugin: &Path, into: &str, keep: bool, patch: String) -> Result<api::Brought> {
    if patch.trim().is_empty() {
        return Ok(api::Brought::default());
    }
    let files = u32::try_from(patch.lines().filter(|l| l.starts_with("diff --git ")).count()).unwrap_or(u32::MAX);
    if keep {
        return Ok(match keep_patch(plugin, &patch).await {
            Some(at) => api::Brought { files, error: None, kept: Some(at) },
            None => api::Brought { files, error: Some("they couldn't be kept here".into()), kept: None },
        });
    }
    let opts = ExecOpts {
        cwd: Some(PathBuf::from(into)),
        input: Some(patch.clone().into_bytes()),
        timeout: Some(Duration::from_secs(60)),
        max_output: 1 << 20,
        ..ExecOpts::default()
    };
    let args = ["apply", "--whitespace=nowarn", "-"].map(str::to_owned).to_vec();
    let r = proc.exec("git", &args, opts).await.map_err(|e| WireError::unavailable(format!("git: {e}")))?;
    if r.exit.success() {
        return Ok(api::Brought { files, error: None, kept: None });
    }
    let why = r.stderr_text().trim().to_owned();
    let kept = keep_patch(plugin, &patch).await;
    let at = kept.as_deref().unwrap_or("nowhere (it couldn't be saved)");
    Ok(api::Brought {
        files,
        error: Some(format!("Its changes clash with yours here, so none went in. They are kept in {at}. ({why})")),
        kept,
    })
}

/// The patch saved under `<plugin>/patches`; its path (None: it couldn't be written).
async fn keep_patch(plugin: &Path, patch: &str) -> Option<String> {
    let at = plugin.join("patches").join(format!("{}.patch", wisp_kernel::ids::uuid7()));
    let dir = at.parent()?;
    (tokio::fs::create_dir_all(dir).await.is_ok() && tokio::fs::write(&at, patch).await.is_ok()).then(|| at.display().to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sh(at: &Path, args: &[&str]) {
        let o = std::process::Command::new("git").current_dir(at).args(args).output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    }

    /// A delegate's patch that doesn't apply to the lead's folder is kept in a file, its path said, and the folder left
    /// as it was. (Before, the clash was said and the patch dropped: the delegate's work was lost with its sandbox.)
    #[tokio::test(flavor = "multi_thread")]
    async fn a_patch_that_clashes_is_kept_and_nothing_here_moves() {
        let root = std::env::temp_dir().join(format!("wisp-sbx-bring-{}", wisp_kernel::ids::uuid7()));
        let (lead, plugin) = (root.join("lead"), root.join("plugin"));
        std::fs::create_dir_all(&lead).unwrap();
        sh(&lead, &["init", "-q"]);
        sh(&lead, &["config", "core.autocrlf", "false"]);
        std::fs::write(lead.join("math.js"), "a - b\n").unwrap();
        sh(&lead, &["add", "."]);
        sh(&lead, &["-c", "user.name=T", "-c", "user.email=t@e", "commit", "-qm", "one"]);
        // the lead changed the same line meanwhile
        std::fs::write(lead.join("math.js"), "a * b\n").unwrap();
        let patch = "diff --git a/math.js b/math.js\n--- a/math.js\n+++ b/math.js\n@@ -1 +1 @@\n-a - b\n+a + b\n".to_owned();
        let proc = wisp_proc::local();
        let out = take_back(&proc, &plugin, lead.to_str().unwrap(), false, patch.clone()).await.unwrap();
        assert_eq!(out.files, 1);
        let kept = out.kept.clone().expect("kept");
        assert!(out.error.as_deref().unwrap().contains(&kept), "{out:?}");
        assert!(Path::new(&kept).starts_with(plugin.join("patches")), "{kept}");
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), patch, "the work isn't lost");
        assert_eq!(std::fs::read_to_string(lead.join("math.js")).unwrap(), "a * b\n", "nothing here moved");
        // held (`keep`): kept even though it would go in, and nothing here moves
        std::fs::write(lead.join("math.js"), "a - b\n").unwrap();
        let held = take_back(&proc, &plugin, lead.to_str().unwrap(), true, patch.clone()).await.unwrap();
        assert!(held.error.is_none(), "{held:?}");
        assert_eq!(std::fs::read_to_string(held.kept.unwrap()).unwrap(), patch);
        assert_eq!(std::fs::read_to_string(lead.join("math.js")).unwrap(), "a - b\n");
        // one that fits goes in
        let ok = take_back(&proc, &plugin, lead.to_str().unwrap(), false, patch).await.unwrap();
        assert_eq!((ok.files, ok.error, ok.kept), (1, None, None));
        assert_eq!(std::fs::read_to_string(lead.join("math.js")).unwrap(), "a + b\n");
        let _ = std::fs::remove_dir_all(root);
    }
}
