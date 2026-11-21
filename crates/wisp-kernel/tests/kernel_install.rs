//! ADR-0046: a user's plugin installed while the kernel runs. It starts off until switched on (the consent to what
//! it asks for), a built-in id or a bad manifest is refused, and uninstall takes it off and out.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::sync::{Arc, Mutex};

use serde_json::json;
use wisp_kernel::{Caller, Ctx, ExecPlugin, PluginGroup};

cmd!(Hello, "user.hello.say", (), String);

/// The plugin manager: it holds `plugins:manage`, and keeps its context for the test.
fn manager(slot: Arc<Mutex<Option<Ctx>>>) -> common::P {
    common::plugin("wisp.plugins", json!({ "permissions": ["plugins:manage"] }), move |s| {
        let slot = slot.clone();
        s.on_start(move |ctx| {
            *slot.lock().unwrap() = Some(ctx);
            async { Ok(()) }
        });
    })
}

fn hello() -> Arc<dyn wisp_kernel::Plugin> {
    Arc::new(common::plugin(
        "user.hello",
        json!({ "contributes": { "commands": [{ "id": "user.hello.say", "title": "Say hello" }] } }),
        |s| {
            s.command::<Hello, _, _>(|_ctx, _call, ()| async { Ok("hello".to_owned()) });
        },
    ))
}

async fn kernel() -> (wisp_kernel::Kernel, Ctx) {
    let slot = Arc::new(Mutex::new(None));
    let k = common::start(PluginGroup::new().add(manager(slot.clone()))).await;
    let ctx = slot.lock().unwrap().clone().expect("the manager started");
    (k, ctx)
}

#[tokio::test]
async fn an_installed_plugin_starts_off_until_switched_on() {
    let (k, ctx) = kernel().await;
    ctx.install_plugin(hello()).await.unwrap();
    let st = k.status().into_iter().find(|s| s.id == "user.hello").unwrap();
    assert_eq!(st.state, "off", "{st:?}");
    assert!(st.reason.as_deref().unwrap_or_default().contains("not switched on yet"));
    assert!(k.call(Caller::user(), "user.hello.say", json!(null)).await.is_err(), "nothing it adds runs before the yes");
    k.set_enabled("user.hello", true).await.unwrap();
    assert_eq!(k.call(Caller::user(), "user.hello.say", json!(null)).await.unwrap(), json!("hello"));
    // the same id again is refused while it's here
    assert!(ctx.install_plugin(hello()).await.is_err());
}

#[tokio::test]
async fn uninstall_takes_a_user_plugin_off_and_removes_it() {
    let (k, ctx) = kernel().await;
    ctx.install_plugin(hello()).await.unwrap();
    k.set_enabled("user.hello", true).await.unwrap();
    ctx.uninstall_plugin("user.hello").await.unwrap();
    assert!(!k.status().iter().any(|s| s.id == "user.hello"));
    assert!(k.call(Caller::user(), "user.hello.say", json!(null)).await.is_err(), "its command went with it");
    // a built-in is switched off, never removed
    assert!(ctx.uninstall_plugin("wisp.plugins").await.is_err());
    // and it can come back
    ctx.install_plugin(hello()).await.unwrap();
}

#[tokio::test]
async fn an_invalid_manifest_is_refused_and_nothing_is_kept() {
    let (k, ctx) = kernel().await;
    let bad = Arc::new(common::bare("invalid.manifest", json!({ "description": "broken" })));
    assert!(ctx.install_plugin(bad).await.is_err());
    assert!(!k.status().iter().any(|s| s.id == "invalid.manifest"));
}

/// r310: uninstall answers once the plugin's process has exited (bounded), not just been told to stop, so whatever
/// replaces or deletes its folder next can: on Windows a running program's folder (here the program itself is in it,
/// as a user's compiled plugin is) can't be moved or emptied.
#[tokio::test]
async fn uninstall_waits_for_its_process_to_exit() {
    let (k, ctx) = kernel().await;
    let root = std::env::temp_dir().join(format!("wisp-kernel-uninstall-{}", wisp_kernel::ids::uuid7()));
    let dir = root.join("echo");
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join(if cfg!(windows) { "echo.exe" } else { "echo" });
    std::fs::copy(env!("CARGO_BIN_EXE_wisp-kernel-exec-echo"), &exe).unwrap();
    let pidfile = dir.join("pid");
    let m =
        common::manifest("user.echo", json!({ "server": { "exec": [exe.display().to_string(), "idle", pidfile.display().to_string()] } }));
    ctx.install_plugin(Arc::new(ExecPlugin::new(m, Some(dir.clone())))).await.unwrap();
    k.set_enabled("user.echo", true).await.unwrap();
    assert_eq!(k.status().into_iter().find(|s| s.id == "user.echo").map(|s| s.state).as_deref(), Some("active"));
    let pid = std::fs::read_to_string(&pidfile).unwrap();
    ctx.uninstall_plugin("user.echo").await.unwrap();
    #[cfg(unix)]
    {
        let alive = std::process::Command::new("kill").args(["-0", pid.trim()]).stderr(std::process::Stdio::null()).status().unwrap();
        assert!(!alive.success(), "its process {pid} has exited (and been reaped) when uninstall answers");
    }
    let _ = &pid;
    std::fs::rename(&dir, root.join("moved")).expect("its folder moves: the program in it isn't running any more");
    let _ = std::fs::remove_dir_all(&root);
}

/// Audit D8: `wisp plugin new` then Settings → Plugins read "user.qa-theme was switched off" (it never was on), and
/// after a restart "is off (disabled in the plugin group)". Switching off a plugin that's off keeps what it said, and a
/// user's plugin kept off at start just reads off; a built-in disabled in the group still says so.
#[tokio::test]
async fn a_plugin_never_switched_on_never_reads_switched_off_audit_d8() {
    let (k, ctx) = kernel().await;
    ctx.install_plugin(hello()).await.unwrap();
    k.set_enabled("user.hello", false).await.unwrap();
    let st = k.status().into_iter().find(|s| s.id == "user.hello").unwrap();
    assert_eq!(st.reason.as_deref(), Some("user.hello is installed, not switched on yet"), "{st:?}");
    k.set_enabled("user.hello", true).await.unwrap();
    k.set_enabled("user.hello", false).await.unwrap();
    let st = k.status().into_iter().find(|s| s.id == "user.hello").unwrap();
    assert_eq!(st.reason.as_deref(), Some("user.hello was switched off"), "once on, off is switched off");

    // a restart: the user's plugins join the group off
    let user = common::plugin("user.hello", json!({}), |_| {});
    let k = common::start(PluginGroup::new().add(user).disable("user.hello")).await;
    assert_eq!(common::state(&k, "user.hello"), ("off".into(), Some("user.hello is off".into())));
}
