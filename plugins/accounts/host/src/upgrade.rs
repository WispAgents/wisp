//! Harness updates (TRY-17): each harness's latest version, read from npm at start and twice a day, and
//! `accounts.upgrade`, which updates an install the way it was installed (wisp's tools folder, a global npm install,
//! the harness's own updater). One it can't update safely (Homebrew, an unknown copy) answers with the command to run.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use wisp_accounts_api::commands as c;
use wisp_accounts_core::update::{Method, method};
use wisp_kernel::{Ctx, Setup, WireError};

use crate::{Slot, State, harness, lock, state};

/// How often the latest versions are read again.
const EVERY: Duration = Duration::from_secs(12 * 3600);

/// The latest version of each harness that has an update route, from `npm view <package> version`; kept in `State`.
pub async fn check(ctx: &Ctx, st: &State) {
    for h in ctx.contributions(&wisp_runtime_api::HARNESS) {
        let Some(u) = h.update() else { continue };
        let args = ["view".to_owned(), u.package.clone(), "version".to_owned()];
        let o = wisp_proc::ExecOpts { timeout: Some(Duration::from_secs(20)), ..wisp_proc::ExecOpts::default() };
        let Ok(r) = st.proc.exec(&npm(), &args, o).await else { continue };
        let v = r.stdout_text().trim().to_owned();
        if r.exit.code == Some(0) && !v.is_empty() {
            lock(&st.latest).insert(h.info().id.clone(), v);
        }
    }
}

/// At start, then every 12 hours: the latest versions, and the view again when they're in.
pub fn follow(ctx: &Ctx, st: &Arc<State>) {
    let (c, s) = (ctx.clone(), st.clone());
    ctx.spawn(async move {
        loop {
            check(&c, &s).await;
            crate::view::publish(&c, &s).await;
            tokio::time::sleep(EVERY).await;
        }
    });
}

fn npm() -> String {
    std::env::var("WISP_NPM").ok().filter(|n| !n.is_empty()).unwrap_or_else(|| "npm".into())
}

/// `accounts.upgrade`: the harness to its latest version, the way it was installed; the agents as detected after.
pub(crate) fn register(s: &mut Setup, slot: &Slot) {
    let slot = slot.clone();
    s.command::<c::Upgrade, _, _>(move |ctx, _call, a| {
        let slot = slot.clone();
        async move {
            let st = state(&slot)?;
            let out = upgrade(&ctx, &st, &a.agent).await;
            crate::view::soon(&ctx, &st);
            out
        }
    });
}

async fn upgrade(ctx: &Ctx, st: &Arc<State>, agent: &str) -> Result<Value, WireError> {
    let h = harness(ctx, agent).ok_or_else(|| WireError::not_found("No such harness."))?;
    let name = h.info().name.clone();
    let u = h.update().ok_or_else(|| WireError::invalid(format!("wisp can’t update {name}: update it the way you installed it.")))?;
    // the program as it is now (the watch's pin is only made on a run, and may name an older version's file): where
    // it really lives says how it was installed; the PATH entry is what runs its own updater
    let program = h.program().unwrap_or_else(|| agent.to_owned());
    let (path, real) = crate::run::resolve(&program)
        .await
        .ok_or_else(|| WireError::invalid(format!("{name} isn’t on this computer’s PATH: install it first.")))?;
    let bin = path.to_string_lossy().into_owned();
    let tools = st.home.join("tools").join("node_modules").to_string_lossy().into_owned();
    let how = method(&real.to_string_lossy(), &tools, &u.package, u.self_update.as_deref());
    if let Method::Manual(cmd) = &how {
        return Err(WireError::invalid(format!("wisp can’t update this copy of {name} itself. Run `{cmd}`, then re-check.")));
    }
    lock(&st.installing).insert(agent.to_owned());
    crate::view::publish(ctx, st).await;
    let done = run(st, &how, &bin, &u.package).await;
    lock(&st.installing).remove(agent);
    done.map_err(|e| WireError::invalid(format!("Updating {name} {e}")))?;
    // only this one detected again: the others' rows don't go back to Checking… (nor their Update buttons)
    let agents = crate::calls::agents(ctx, Some(agent)).await?;
    crate::calls::track(ctx, "harness_update", serde_json::json!({ "agent": agent, "how": label(&how) })).await;
    serde_json::to_value(agents).map_err(|e| WireError::internal(e.to_string()))
}

const fn label(how: &Method) -> &'static str {
    match how {
        Method::Tools => "tools",
        Method::NpmGlobal => "npm",
        Method::SelfUpdate(_) => "self",
        Method::Manual(_) => "manual",
    }
}

/// Runs the update; the error says how it went ("failed: …", "timed out").
async fn run(st: &State, how: &Method, bin: &str, package: &str) -> Result<(), String> {
    let pkg = format!("{package}@latest");
    let quiet = ["--no-audit", "--no-fund", "--loglevel=error"];
    let (prog, args): (String, Vec<String>) = match how {
        Method::Manual(_) => return Ok(()),
        Method::Tools => {
            let dir = st.home.join("tools").to_string_lossy().into_owned();
            (npm(), ["install", "--prefix", &dir].iter().chain(&quiet).chain([&pkg.as_str()]).map(|s| (*s).to_owned()).collect())
        }
        Method::NpmGlobal => (npm(), ["install", "-g"].iter().chain(&quiet).chain([&pkg.as_str()]).map(|s| (*s).to_owned()).collect()),
        Method::SelfUpdate(a) => (bin.to_owned(), a.clone()),
    };
    let o = wisp_proc::ExecOpts {
        env: wisp_proc::Env { remove: vec!["ELECTRON_RUN_AS_NODE".into()], ..wisp_proc::Env::default() },
        timeout: Some(Duration::from_secs(300)),
        ..wisp_proc::ExecOpts::default()
    };
    match st.proc.exec(&prog, &args, o).await {
        Ok(r) if r.exit.code == Some(0) => Ok(()),
        Ok(r) => {
            let out = format!("{}{}", r.stdout_text(), r.stderr_text());
            let tail = wisp_accounts_core::login::last_line(&out).map(|l| format!(": {l}")).unwrap_or_default();
            Err(format!("{}{tail}", if r.timed_out { "timed out" } else { "failed" }))
        }
        Err(e) => Err(format!("didn’t start: {e}")),
    }
}
