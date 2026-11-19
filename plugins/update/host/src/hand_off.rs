//! The hand-off (`Updater::hand_off`) and, on Windows, the npm install it runs once wisp has exited
//! (`wisp_update_core::deferred`): the files the waiter and the next start share, in this plugin's folder.

use wisp_kernel::Ctx;
use wisp_update_core::deferred::{self, Files, Outcome, Pending};

use crate::{St, lock};

/// Hands off to the detached waiter that stops this wisp and starts the new one (`Updater::hand_off`: `sh` on Unix,
/// `PowerShell` on Windows), through the proc service. False: it couldn't start, and the user restarts it.
pub(crate) fn relaunch(ctx: &Ctx, st: &St) -> bool {
    // only a Windows npm install uses the folder (and it wrote its pending file there first)
    let files = files(ctx, st).unwrap_or_else(|| Files { dir: String::new(), exe: st.exe.clone() });
    let Some(h) = lock(&st.up).hand_off(std::process::id(), &files) else { return false };
    let Ok(proc) = ctx.service::<wisp_proc::Proc>() else { return false };
    // it stops wisp and the desktop app quits with it, so it leaves the app's job too (the one start that does:
    // everything else wisp starts ends when the window closes, DESK2-05). Err: the user restarts it by hand.
    let opts = wisp_proc::SpawnOpts { no_stdin: true, leave_every_job: true, ..wisp_proc::SpawnOpts::default() };
    proc.spawn_detached(h.bin, &h.args, opts).is_ok()
}

/// Where the Windows hand-off and the next start meet: `<home>/plugins/wisp.update`.
pub(crate) fn files(ctx: &Ctx, st: &St) -> Option<Files> {
    let dir = wisp_kernel::services::paths(ctx).ok()?.plugin_dir(ctx.plugin());
    Some(Files { dir: dir.to_string_lossy().into_owned(), exe: st.exe.clone() })
}

pub(crate) fn pending_of(st: &St) -> Option<Pending> {
    lock(&st.up).pending()
}

/// Before a hand-off that installs after exit: what the next start compares with.
pub(crate) async fn write_pending(ctx: &Ctx, st: &St, p: &Pending) -> std::result::Result<(), String> {
    let files = files(ctx, st).ok_or("wisp has no folder of its own")?;
    let platform = lock(&st.up).platform.clone();
    tokio::fs::create_dir_all(&files.dir).await.map_err(|e| e.to_string())?;
    let body = serde_json::to_vec(p).map_err(|e| e.to_string())?;
    // last run's answers would read as this one's
    for f in [deferred::EXIT, deferred::LOG, deferred::TRACE] {
        let _ = tokio::fs::remove_file(files.path(&platform, f)).await;
    }
    tokio::fs::write(files.path(&platform, deferred::PENDING), body).await.map_err(|e| e.to_string())
}

/// At start: what the last run's Windows hand-off did, read once (the files go), or None when there was none.
pub async fn read_hand_off(dir: &std::path::Path, current: &str) -> Option<Outcome> {
    let pending = dir.join(deferred::PENDING);
    let p: Pending = serde_json::from_slice(&tokio::fs::read(&pending).await.ok()?).ok()?;
    let exit = tokio::fs::read_to_string(dir.join(deferred::EXIT)).await.ok();
    // npm writes its log in the console's code page: read it lossily
    let log = tokio::fs::read(dir.join(deferred::LOG)).await.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    for f in [deferred::PENDING, deferred::EXIT, deferred::LOG] {
        let _ = tokio::fs::remove_file(dir.join(f)).await;
    }
    Some(deferred::outcome(&p, exit.as_deref(), &log, current))
}
