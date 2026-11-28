//! wisp.sandbox (ADR-0047): sandboxes on the user's wisp Grid, each for one owner (a task's `/sandbox`, a
//! delegate, a crew job or member, a goal's judges), seeded with the project, deleted with their owner. On a
//! computer it makes, places, drops and sweeps them; inside a sandbox (`WISP_SANDBOX=1`) it only takes the project in.
//!
//! An owner going: a local task deleted (`session.deleted`: its delegates', judges' and a crew job's sandboxes), a
//! member leaving the crew (`crew.left`), and a sandboxed task deleted on its sandbox's own wisp, seen by the
//! minute's look at each task-owned sandbox online now (its task gone there: the sandbox goes; an asleep one is never
//! woken for it). The sweep, at start and every 10 minutes, drops what a failed drop left, launches that never got
//! their task, and crew jobs' sandboxes never named.
//!
//! resources: data-only (its tables are launches under way and a sign-in's progress, nothing a user declares)

mod calls;
mod claude;
mod launches;
mod make;
mod seed;

use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;
use wisp_kernel::{Ctx, Manifest, Plugin, Result, Setup, WireError};
use wisp_sandbox_api as api;
use wisp_sandbox_core::{self as core, Owner};

pub use seed::in_sandbox;

/// The launches' rows, once the plugin started.
pub(crate) type Slot = Arc<OnceLock<launches::Shared>>;

pub(crate) fn launches_of(slot: &Slot) -> Result<launches::Shared> {
    slot.get().cloned().ok_or_else(|| WireError::unavailable("wisp.sandbox is starting"))
}

#[derive(Debug, Default)]
pub struct SandboxPlugin {
    /// a sandbox's wisp or not; unset: `WISP_SANDBOX=1` says (tests set it)
    pub inside: Option<bool>,
    /// where projects land (`<home>/code`); unset: `$HOME`
    pub home: Option<std::path::PathBuf>,
}

/// How often task-owned sandboxes are looked at (is their task still there?).
const OWNERS_MS: u64 = 60_000;
/// How long one sandbox is asked about its task, at most (kept: asked again the next minute).
const CHECK_MS: u64 = 10_000;

/// `$HOME` (else the temp folder), spelled as every path in wisp is (ADR-0071: an 8.3 short name spelled out), so a
/// project under `~/code` compares with the task folders runtime stores. Startup only (blocking).
fn home() -> std::path::PathBuf {
    wisp_proc::plain_path_blocking(std::env::var_os("HOME").map_or_else(std::env::temp_dir, std::path::PathBuf::from))
}

async fn info(ctx: &Ctx, call: &wisp_kernel::Call) -> Result<api::Info> {
    let grid = calls::grid_info(ctx, call).await;
    let (enabled, why, logins, env) = match &grid {
        Ok(v) => {
            let names = |k: &str| {
                v.get(k)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_owned).or_else(|| x.get("agent").and_then(Value::as_str).map(str::to_owned)))
                    .collect::<Vec<_>>()
            };
            let on = v.get("enabled").and_then(Value::as_bool).unwrap_or(false);
            (on, (!on).then(|| "This grid doesn't make sandboxes.".to_owned()), core::logins_of(v.get("logins")), names("env"))
        }
        Err(e) => (false, Some(e.message.clone()), Vec::new(), Vec::new()),
    };
    // every sandbox the grid holds counts against the user's cap: one no machine here keeps is listed too, and the
    // grid not answering leaves this computer's list rather than a blank one
    let theirs = grid.as_ref().ok().and_then(|v| v.get("sandboxes")).and_then(Value::as_array).map(Vec::as_slice);
    let sandboxes = core::listed(&calls::machines(ctx, call).await, theirs)
        .into_iter()
        // the app reaches a machine by its own id (`/api/peer/<id>`): that's its peer there
        .map(|l| api::SandboxView {
            peer: l.machine.clone(),
            machine: l.machine,
            sandbox: l.sandbox,
            name: l.name,
            owner: l.owner,
            state: l.state,
            elsewhere: l.elsewhere,
        })
        .collect();
    let used = theirs.and_then(|t| u32::try_from(t.len()).ok());
    let limit = grid.as_ref().ok().and_then(|v| v.get("limit")).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
    Ok(api::Info { enabled, why, logins, env, sandboxes, used, limit })
}

/// `sandbox.drop`: a task's sandboxes, a crew member's, one by its machine, or one the grid holds that no machine here
/// keeps (by its sandbox id: made elsewhere, or its machine gone). An agent drops only its own task's, and never one
/// still holding work its owner never got (ADR-0136): the user deletes that in Settings.
async fn drop_cmd(ctx: &Ctx, call: &wisp_kernel::Call, a: &api::DropArgs) -> Result<api::Dropped> {
    let all = make::held(ctx, call).await;
    let list: Vec<&core::Held> = if let Some(m) = &a.machine {
        all.iter().filter(|h| &h.machine == m).collect()
    } else if let Some(t) = &a.task {
        core::gone_with_task(&all, t)
    } else if let Some(c) = &a.crew {
        core::gone_with_member(&all, c)
    } else if let Some(sb) = &a.sandbox {
        let here: Vec<&core::Held> = all.iter().filter(|h| &h.sandbox == sb).collect();
        if here.is_empty() {
            // no machine keeps it, so no owner names a task: only the user deletes it
            if wisp_kernel::scope::agent_task(call.caller()).is_some() {
                return Err(WireError::forbidden(format!("{sb}: an agent drops only its own task's sandboxes")));
            }
            calls::grid_remove(ctx, call, sb).await?;
            return Ok(api::Dropped { dropped: 1 });
        }
        here
    } else {
        return Err(WireError::invalid("say whose sandboxes: a task, a crew member, a machine or a sandbox"));
    };
    if let Some(me) = wisp_kernel::scope::agent_task(call.caller()) {
        if a.task.as_deref().is_some_and(|t| core::bare(t) != core::bare(me)) {
            return Err(WireError::forbidden("an agent drops only its own task's sandboxes"));
        }
        for h in &list {
            let work = make::holds_work(ctx, call, h).await;
            if let Some(why) = core::agent_drop_refusal(h, me, work) {
                return Err(WireError::forbidden(why));
            }
        }
    }
    Ok(api::Dropped { dropped: make::drop_all(ctx, call, &list).await })
}

/// Task-owned sandboxes whose task is gone on their own wisp go. Only online ones are asked ([`core::to_check`]): an
/// asleep box is never woken (a routed command is a real request, and every box was woken once a minute, so none ever
/// slept), and one asleep, off or unreachable is kept. They're asked side by side, each for [`CHECK_MS`] at most, so
/// one slow box holds back neither the others nor the sweep.
async fn owners_gone(ctx: &Ctx) {
    let call = ctx.root_call();
    let Ok(clock) = ctx.clock() else { return };
    let all = make::held(ctx, &call).await;
    let asks = core::to_check(&all).into_iter().map(|(h, id)| {
        let (call, clock) = (&call, clock.clone());
        async move {
            let get = calls::route::<wisp_runtime_api::commands::Get>(
                ctx,
                call,
                &h.machine,
                wisp_runtime_api::commands::IdArgs { id: id.into() },
            );
            tokio::select! {
                r = get => {
                    if let Err(e) = r
                        && e.code == wisp_kernel::Code::NotFound
                    {
                        make::drop_one(ctx, call, &h.machine, &h.sandbox).await;
                    }
                }
                () = clock.sleep(CHECK_MS) => {}
            }
        }
    });
    futures::future::join_all(asks).await;
}

async fn sweep(ctx: &Ctx) {
    let call = ctx.root_call();
    let all = make::held(ctx, &call).await;
    let n = make::drop_all(ctx, &call, &core::sweep(&all, make::now(ctx))).await;
    if n > 0 {
        tracing::info!("sandbox sweep: {n} dropped");
    }
}

/// An owner going takes its sandboxes with it; the sweep and the look at task owners run on the plugin's clock.
fn watch(s: &mut Setup) {
    // an owner going takes its sandboxes with it
    s.on::<wisp_runtime_api::events::SessionDeleted, _, _>(|ctx, e| async move {
        let call = ctx.root_call();
        let all = make::held(&ctx, &call).await;
        let mut gone = Vec::new();
        for h in core::gone_with_task(&all, e.id.as_str()) {
            // a delegate's sandbox with work its lead never got stays, for the user to delete (Settings → Sandboxes)
            if make::holds_work(&ctx, &call, h).await {
                tracing::warn!(machine = %h.machine, "sandbox: a deleted lead's delegate still holds work there; kept until it's deleted in Settings");
                continue;
            }
            gone.push(h);
        }
        make::drop_all(&ctx, &call, &gone).await;
    });
    s.on::<wisp_crew_api::events::Left, _, _>(|ctx, e| async move {
        let call = ctx.root_call();
        let all = make::held(&ctx, &call).await;
        make::drop_all(&ctx, &call, &core::gone_with_member(&all, &e.crew_id)).await;
    });
    s.on_start(|ctx| async move {
        let Ok(clock) = ctx.clock() else { return Ok(()) };
        // tests look sooner (`WISP_SANDBOX_TICK_MS`); everywhere else, once a minute
        let every = std::env::var("WISP_SANDBOX_TICK_MS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(OWNERS_MS).max(200);
        let scope = ctx.scope().clone();
        let c = ctx.clone();
        ctx.spawn(async move {
            let mut since_sweep = core::SWEEP_MS;
            loop {
                if since_sweep >= core::SWEEP_MS {
                    sweep(&c).await;
                    since_sweep = 0;
                }
                // the look at task owners waits a tick: at start each sandbox's state is the one saved when this wisp
                // stopped ("online"), so asking then woke every sandbox each time a hub woke for its schedule
                tokio::select! {
                    () = scope.cancelled() => return,
                    () = clock.sleep(every) => since_sweep += every,
                }
                owners_gone(&c).await;
            }
        });
        Ok(())
    });
}

/// `POST /api/sandbox/seed/:key/:part` (ADR-0060): the project's bundle or patch as raw bytes, kept for
/// `sandbox.seed {key}`. Only a sandbox takes one; its cap is the carry limit (`wisp_machines_api::MAX_CARRY`), not a
/// sync message's 2 MB.
fn seed_route(inside: bool, at: std::path::PathBuf) -> wisp_kernel::http::Route {
    wisp_kernel::http::Route {
        method: "POST",
        pattern: "/sandbox/seed/:key/:part",
        permission: "sandbox:seed",
        doc: false,
        summary: "A sandbox's project (a bundle, a patch), kept for sandbox.seed",
        raw: false,
        max_body: Some(wisp_machines_api::MAX_CARRY + (1 << 20)),
        handler: Arc::new(move |_ctx, _call, req| {
            let at = at.clone();
            Box::pin(async move {
                if !inside {
                    return Err(WireError::forbidden("Only a sandbox takes a project in (sandbox.seed)."));
                }
                seed::receive(&at, req.param("key").unwrap_or_default(), req.param("part").unwrap_or_default(), &req.body).await?;
                Ok(wisp_kernel::http::Response::Json { status: 200, body: serde_json::json!({ "ok": true }) })
            })
        }),
    }
}

/// What only a sandbox's wisp does (refused anywhere else): take a project in, give its changes back, count them on.
fn inside_commands(s: &mut Setup, inside: bool, home_at: Option<&std::path::PathBuf>) {
    let at_home = || home_at.cloned().map_or_else(home, wisp_proc::plain_path_blocking);
    let at = at_home();
    s.command::<api::Seed, _, _>(move |ctx, _call, a| {
        let at = at.clone();
        async move {
            // only a sandbox takes a project in: anywhere else this would overwrite ~/code
            if !inside {
                return Err(WireError::forbidden("Only a sandbox takes a project in (sandbox.seed)."));
            }
            seed::seed(&ctx, &at, &a).await
        }
    });
    s.route(seed_route(inside, at_home()));
    let at = at_home();
    s.command::<api::PatchCmd, _, _>(move |ctx, _call, a| {
        let at = at.clone();
        async move {
            if !inside {
                return Err(WireError::forbidden("Only a sandbox gives its changes back (sandbox.patch)."));
            }
            seed::patch(&ctx, &at, &a.cwd).await
        }
    });
    let at = at_home();
    s.command::<api::Rebase, _, _>(move |ctx, _call, a| {
        let at = at.clone();
        async move {
            if !inside {
                return Err(WireError::forbidden("Only a sandbox counts its changes (sandbox.rebase)."));
            }
            seed::rebase(&ctx, &at, &a).await
        }
    });
}

impl Plugin for SandboxPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let inside = self.inside.unwrap_or_else(|| in_sandbox(&|k| std::env::var(k).ok()));
        inside_commands(s, inside, self.home.as_ref());
        if inside {
            // a sandbox makes no sandboxes and has no owners to watch
            return Ok(());
        }
        s.command::<api::Bring, _, _>(|ctx, call, a| async move { make::bring(&ctx, &call, &a).await });
        s.command::<api::Launch, _, _>(|ctx, call, a| async move { make::launch(&ctx, &call, &a).await });
        s.command::<api::Place, _, _>(|ctx, call, a| async move { make::place(&ctx, &call, &a).await });
        s.command::<api::Drop, _, _>(|ctx, call, a| async move { drop_cmd(&ctx, &call, &a).await });
        s.command::<api::Own, _, _>(|ctx, call, a| async move {
            let owner: Owner = serde_json::from_value(a.owner).map_err(|_| WireError::invalid("not a sandbox owner"))?;
            let all = make::held(&ctx, &call).await;
            let Some(h) = all.iter().find(|h| h.machine == a.machine) else {
                return Err(WireError::not_found("That machine isn't a sandbox."));
            };
            // an agent names its own task's only (ADR-0136): never another task's box, never a launch's
            if let Some(me) = wisp_kernel::scope::agent_task(call.caller())
                && let Some(why) = core::agent_own_refusal(h.owner.as_ref(), &owner, me)
            {
                return Err(WireError::forbidden(why));
            }
            make::set_owner(&ctx, &call, &a.machine, &owner).await?;
            Ok(api::Empty {})
        });
        s.command::<api::InfoCmd, _, _>(|ctx, call, _a| async move { info(&ctx, &call).await });
        // the launches' rows (`sandbox.start`), once the store is there
        let slot: Slot = Arc::new(OnceLock::new());
        let l = slot.clone();
        s.on_start(move |ctx| async move {
            let store = ctx.service::<wisp_store::Store>()?.as_ref().clone();
            let shared = launches::install(&store)?;
            let _ = l.set(shared.clone());
            ctx.spawn(async move { launches::tidy(&shared).await });
            Ok(())
        });
        let l = slot.clone();
        s.command::<api::Start, _, _>(move |ctx, _call, a| {
            let l = l.clone();
            async move { launches::start(&ctx, &launches_of(&l)?, a).await }
        });
        let l = slot.clone();
        s.command::<api::Retry, _, _>(move |ctx, _call, a| {
            let l = l.clone();
            async move { launches::retry(&ctx, &launches_of(&l)?, &a.id).await }
        });
        let l = slot.clone();
        s.command::<api::Dismiss, _, _>(move |_ctx, _call, a| {
            let l = l.clone();
            async move { launches::dismiss(&launches_of(&l)?, &a.id).await }
        });
        let flow: claude::Shared = Arc::new(Mutex::new(claude::Flow::default()));
        let (f, l) = (flow.clone(), slot.clone());
        s.command::<api::ClaudeBegin, _, _>(move |ctx, _call, _a| {
            let (f, l) = (f.clone(), l.clone());
            async move { claude::begin(&ctx, &f, &l).await }
        });
        let (f, l) = (flow.clone(), slot);
        s.command::<api::ClaudeCode, _, _>(move |ctx, call, a| {
            let (f, l) = (f.clone(), l.clone());
            async move { claude::code(&ctx, &call, &f, &a.code, &l).await }
        });
        let f = flow;
        s.command::<api::ClaudeState, _, _>(move |_ctx, _call, _a| {
            let f = f.clone();
            async move { Ok(claude::current(&f)) }
        });
        s.command::<api::Logout, _, _>(|ctx, call, a| async move {
            calls::grid_logout(&ctx, &call, &a.agent).await?;
            info(&ctx, &call).await
        });
        s.command::<api::Env, _, _>(|ctx, call, a| async move {
            calls::grid_env(&ctx, &call, &a.key, &a.value).await?;
            info(&ctx, &call).await
        });
        watch(s);
        Ok(())
    }
}
