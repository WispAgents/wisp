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

/// How often the latest versions are read again: daily, as wisp's own update check (wisp.update).
const EVERY: Duration = Duration::from_secs(24 * 3600);
/// The first scheduled read, a minute after start: a server that's up for less (a test's, QA's, a quick `wisp serve`)
/// asks nothing unless Re-check is pressed.
const FIRST: Duration = Duration::from_secs(60);
/// The switch in Settings (on unless switched off).
pub const SETTING: &str = "accounts.updateCheck";

/// Setup audit B5: whether harness versions may be asked for (`npm view`, which tells the registry which harnesses
/// are here). The same switches as wisp's own update check (`WISP_UPDATE_CHECK=0`, any `CI`) and its own setting:
/// tests, QA and CI never reach the registry, and a user can say no.
pub fn checks_on(update_check: Option<&str>, ci: Option<&str>, setting: Option<&Value>) -> bool {
    let off = update_check.is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off"));
    !off && ci.is_none_or(|c| c.trim().is_empty()) && setting.and_then(Value::as_bool) != Some(false)
}

fn on(ctx: &Ctx) -> bool {
    checks_on(std::env::var("WISP_UPDATE_CHECK").ok().as_deref(), std::env::var("CI").ok().as_deref(), ctx.setting(SETTING).as_ref())
}

/// The latest version of each harness that has an update route, from `npm view <package> version`; kept in `State`.
/// Nothing while the checks are off ([`checks_on`]).
pub async fn check(ctx: &Ctx, st: &State) {
    if !on(ctx) {
        return;
    }
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

/// A minute after start, then daily (wisp's own update check is daily too), and on Re-check: the latest versions, and the view
/// again when they're in; never while the checks are off.
pub fn follow(ctx: &Ctx, st: &Arc<State>) {
    let (c, s) = (ctx.clone(), st.clone());
    ctx.spawn(async move {
        tokio::time::sleep(FIRST).await;
        loop {
            if on(&c) {
                check(&c, &s).await;
                crate::view::publish(&c, &s).await;
            }
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
    // the version as it is now (the last detection's: nothing runs again for it), to tell an update from none
    let before = crate::calls::agents(ctx, None).await.ok().and_then(|l| l.into_iter().find(|a| a.id == agent)).and_then(|a| a.version);
    lock(&st.installing).insert(agent.to_owned());
    crate::view::publish(ctx, st).await;
    let done = run(st, &how, &bin, &u.package).await;
    lock(&st.installing).remove(agent);
    done.map_err(|e| WireError::invalid(format!("Updating {name} {e}")))?;
    // only this one detected again: the others' rows don't go back to Checking… (nor their Update buttons)
    let agents = crate::calls::agents(ctx, Some(agent)).await?;
    // setup audit B4: an updater that exits 0 and changes nothing (`claude update` on a copy it can't replace) is no
    // update: the version detected now says, never the exit code alone
    let target = lock(&st.latest).get(agent).cloned();
    let now = agents.iter().find(|a| a.id == agent).and_then(|a| a.version.clone());
    if let Some(still) = still_behind(before.as_deref(), now.as_deref(), target.as_deref()) {
        crate::calls::track(ctx, "harness_update", serde_json::json!({ "agent": agent, "how": label(&how), "ok": false })).await;
        return Err(WireError::invalid(format!(
            "{name} is still {still} after the update (the latest is {}). Run `{}` in a terminal, then re-check.",
            target.unwrap_or_default(),
            by_hand(&how, &program, &u.package)
        )));
    }
    crate::calls::track(ctx, "harness_update", serde_json::json!({ "agent": agent, "how": label(&how) })).await;
    serde_json::to_value(agents).map_err(|e| WireError::internal(e.to_string()))
}

/// The version detected after an update when the update changed nothing: the same as before it, and not the latest
/// wanted (`9.9.9` for `9.9.10`). None: it moved (to the latest, or past it: npm's `latest` or the harness's own channel
/// may be newer than the one read earlier), it's the latest, or there's nothing to compare (no version read, the latest
/// unknown). A detected version is the CLI's line (`2.1.3 (Claude Code)`), so the latest's number in it is the latest.
fn still_behind(before: Option<&str>, now: Option<&str>, target: Option<&str>) -> Option<String> {
    let (now, target) = (now?.trim(), target?.trim());
    if now.is_empty() || target.is_empty() || before.map(str::trim) != Some(now) {
        return None;
    }
    let has = now.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-')).any(|w| w.trim_start_matches('v') == target);
    (!has).then(|| now.to_owned())
}

/// What to run by hand when wisp's update didn't take: the harness's own updater, or npm.
fn by_hand(how: &Method, program: &str, package: &str) -> String {
    match how {
        Method::SelfUpdate(a) => format!("{program} {}", a.join(" ")),
        Method::Manual(cmd) => cmd.clone(),
        Method::Tools | Method::NpmGlobal => format!("npm install -g {package}@latest"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Setup audit B4: `claude update` exited 0 and changed nothing, and Agents said "Claude Code updated to 9.9.10"
    /// while the row still read 9.9.9. Only the version detected after says the update took.
    #[test]
    fn an_update_that_left_the_old_version_is_still_behind() {
        let same = |v: &str, target: &str| still_behind(Some(v), Some(v), Some(target));
        assert_eq!(same("9.9.9", "9.9.10").as_deref(), Some("9.9.9"));
        assert_eq!(same("9.9.9 (Claude Code)", "9.9.10").as_deref(), Some("9.9.9 (Claude Code)"));
        assert_eq!(same("9.9.10", "9.9.10"), None, "already the latest");
        assert_eq!(same("codex-cli v0.60.1", "0.60.1"), None);
        // it moved: to the latest, or past the latest read earlier (npm's `latest` moved on, the harness's own channel)
        assert_eq!(still_behind(Some("9.9.9"), Some("9.9.10"), Some("9.9.10")), None);
        assert_eq!(still_behind(Some("2.1.2 (Claude Code)"), Some("2.1.3 (Claude Code)"), Some("2.1.3")), None);
        assert_eq!(still_behind(Some("9.9.9"), Some("9.9.11"), Some("9.9.10")), None, "newer than the latest read: it took");
        assert_eq!(still_behind(None, Some("9.9.9"), Some("9.9.10")), None, "no version before: nothing to compare");
        assert_eq!(still_behind(Some("9.9.9"), None, Some("9.9.10")), None, "nothing read after: nothing to say");
        assert_eq!(still_behind(Some("9.9.9"), Some("9.9.9"), None), None, "the latest unknown");
    }

    /// Setup audit B5: every `wisp serve` asked the npm registry, whatever `WISP_UPDATE_CHECK`, `CI` or Settings said.
    #[test]
    fn harness_version_checks_follow_wisps_update_switches_and_their_own() {
        assert!(checks_on(None, None, None));
        for off in ["0", "false", "off", "NO"] {
            assert!(!checks_on(Some(off), None, None), "WISP_UPDATE_CHECK={off}");
        }
        assert!(checks_on(Some("1"), None, None));
        assert!(!checks_on(None, Some("true"), None), "CI");
        assert!(checks_on(None, Some(""), None), "an empty CI is none");
        assert!(!checks_on(None, None, Some(&Value::Bool(false))), "Settings off");
        assert!(checks_on(None, None, Some(&Value::Bool(true))));
    }

    #[test]
    fn by_hand_names_the_harness_updater_or_npm() {
        assert_eq!(by_hand(&Method::SelfUpdate(vec!["update".into()]), "claude", "@anthropic-ai/claude-code"), "claude update");
        assert_eq!(by_hand(&Method::NpmGlobal, "codex", "@openai/codex"), "npm install -g @openai/codex@latest");
    }
}
