//! ADR-0046: a user's plugin installed from a folder while wisp runs: off until switched on, a built-in id or a bad
//! manifest refused with nothing kept, the same plugin again replacing it, uninstall removing it and its folder, and
//! the folders under `~/.wisp/plugins` read at start.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use wisp_kernel::Caller;
use wisp_kernel::services::Paths;
use wisp_testkit::TestApp;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wisp-plugins-{name}-{}", wisp_kernel::ids::uuid7()));
    std::fs::create_dir_all(&d).unwrap();
    // spelled as wisp spells a home (ADR-0071): the Windows runner's `%TEMP%` is `C:\Users\RUNNER~1\…`
    wisp_proc::plain_path_blocking(d)
}

fn source(at: &Path, id: &str, version: &str) -> PathBuf {
    let d = at.join(format!("src-{id}-{version}"));
    std::fs::create_dir_all(&d).unwrap();
    let m = json!({ "wispPlugin": 1, "id": id, "name": "Theme", "version": version, "api": "^1", "permissions": ["settings:read"], "activation": [], "contributes": {} });
    std::fs::write(d.join("wisp-plugin.json"), m.to_string()).unwrap();
    std::fs::write(d.join("README.md"), "hello").unwrap();
    d
}

fn state(list: &Value, id: &str) -> Option<String> {
    list.as_array().unwrap().iter().find(|p| p["id"] == id).map(|p| p["state"].as_str().unwrap().to_owned())
}

#[tokio::test]
async fn an_installed_plugin_starts_off_until_switched_on_and_uninstall_removes_it() {
    let home = dir("home");
    let k = TestApp::new().with(wisp_plugins::PluginsPlugin).service(Paths::new(&home)).start().await;
    let src = source(&home, "user.theme", "1.0.0");
    let out = k.call(Caller::user(), "plugins.install", json!({ "source": src.display().to_string() })).await.unwrap();
    assert_eq!((out["id"].clone(), out["permissions"].clone()), (json!("user.theme"), json!(["settings:read"])));
    assert!(home.join("plugins/theme/wisp-plugin.json").is_file() && home.join("plugins/theme/README.md").is_file());
    let list = k.call(Caller::user(), "plugins.list", json!({})).await.unwrap();
    assert_eq!(state(&list, "user.theme").as_deref(), Some("off"), "off until switched on");
    k.call(Caller::user(), "plugins.setEnabled", json!({ "id": "user.theme", "enabled": true })).await.unwrap();
    let list = k.call(Caller::user(), "plugins.list", json!({})).await.unwrap();
    assert_ne!(state(&list, "user.theme").as_deref(), Some("off"));
    // installing_the_same_id_again_replaces_it
    let v2 = source(&home, "user.theme", "2.0.0");
    let out = k.call(Caller::user(), "plugins.install", json!({ "source": v2.display().to_string() })).await.unwrap();
    assert_eq!(out["version"], json!("2.0.0"));
    // uninstall_takes_a_user_plugin_off_and_removes_it
    k.call(Caller::user(), "plugins.uninstall", json!({ "id": "user.theme" })).await.unwrap();
    let list = k.call(Caller::user(), "plugins.list", json!({})).await.unwrap();
    assert!(state(&list, "user.theme").is_none());
    assert!(!home.join("plugins/theme").exists());
    assert!(k.call(Caller::user(), "plugins.uninstall", json!({ "id": "wisp.plugins" })).await.is_err(), "a built-in stays");
    let _ = std::fs::remove_dir_all(&home);
}

#[tokio::test]
async fn a_plugin_claiming_a_built_in_id_is_refused_and_nothing_is_kept() {
    let home = dir("refused");
    let k = TestApp::new().with(wisp_plugins::PluginsPlugin).service(Paths::new(&home)).start().await;
    let src = source(&home, "wisp.theme", "1.0.0");
    let e = k.call(Caller::user(), "plugins.install", json!({ "source": src.display().to_string() })).await.unwrap_err();
    assert!(e.message.contains("isn't a user plugin's id"), "{}", e.message);
    // an_invalid_manifest_is_refused_and_nothing_is_kept
    let bad = home.join("bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("wisp-plugin.json"), "{ not json").unwrap();
    assert!(k.call(Caller::user(), "plugins.install", json!({ "source": bad.display().to_string() })).await.is_err());
    let left: Vec<_> = std::fs::read_dir(home.join("plugins")).map(|r| r.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    assert!(left.is_empty(), "nothing kept: {left:?}");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn user_plugins_load_at_start() {
    let home = dir("start");
    let plugins = home.join("plugins");
    std::fs::create_dir_all(plugins.join("theme")).unwrap();
    std::fs::write(
        plugins.join("theme/wisp-plugin.json"),
        json!({ "wispPlugin": 1, "id": "user.theme", "version": "1.0.0", "api": "^1" }).to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(plugins.join("broken")).unwrap();
    std::fs::write(plugins.join("broken/wisp-plugin.json"), "nope").unwrap();
    let got = wisp_plugins::user_plugins(&plugins);
    assert_eq!(got.iter().map(|p| wisp_kernel::Plugin::manifest(p).id).collect::<Vec<_>>(), ["user.theme"]);
    let _ = std::fs::remove_dir_all(&home);
}

/// ADR-0048: whatever its manifest claims, an installed plugin is untrusted until you trust it; trusting it reloads
/// it as trusted and keeps it on, and taking the trust back makes it untrusted again.
#[tokio::test]
async fn an_installed_plugin_is_untrusted_whatever_its_manifest_says() {
    let home = dir("trust");
    let k = TestApp::new().with(wisp_plugins::PluginsPlugin).service(Paths::new(&home)).start().await;
    let src = source(&home, "user.claims", "1.0.0");
    let m = src.join("wisp-plugin.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&m).unwrap()).unwrap();
    v["trust"] = json!("trusted");
    std::fs::write(&m, v.to_string()).unwrap();
    k.call(Caller::user(), "plugins.install", json!({ "source": src.display().to_string() })).await.unwrap();
    let builtin = |list: &Value| list.as_array().unwrap().iter().find(|p| p["id"] == "user.claims").map(|p| p["builtin"].clone());
    let trust_of = |k: &wisp_kernel::Kernel| k.status().into_iter().find(|s| s.id == "user.claims").map(|s| s.builtin);
    assert_eq!(trust_of(&k), Some(false));
    k.call(Caller::user(), "plugins.setEnabled", json!({ "id": "user.claims", "enabled": true })).await.unwrap();
    k.call(Caller::user(), "plugins.trust", json!({ "id": "user.claims", "trusted": true })).await.unwrap();
    assert!(home.join("plugins/claims/.wisp-trusted").is_file());
    let list = k.call(Caller::user(), "plugins.list", json!({})).await.unwrap();
    assert_eq!(state(&list, "user.claims").as_deref(), Some("active"), "still on after the reload");
    assert_eq!(builtin(&list), Some(json!(false)), "trusted, still yours: never a built-in");
    k.call(Caller::user(), "plugins.trust", json!({ "id": "user.claims", "trusted": false })).await.unwrap();
    assert!(!home.join("plugins/claims/.wisp-trusted").exists());
    // a built-in's trust isn't yours to change
    assert!(k.call(Caller::user(), "plugins.trust", json!({ "id": "wisp.runtime", "trusted": false })).await.is_err());
    // the manifest as read: untrusted whatever it said
    let text = std::fs::read_to_string(home.join("plugins/claims/wisp-plugin.json")).unwrap();
    let read = wisp_plugins::read_plugin(&home.join("plugins/claims"), &text, false).unwrap();
    assert_eq!(wisp_kernel::Plugin::manifest(&read).trust.as_deref(), Some("untrusted"));
}

/// ADR-0048: an untrusted plugin's UI calls wisp as the plugin (`plugin:<id>`), checked against the permissions its
/// manifest asked for and nothing more; and `plugins.ui` hands the app its module, only once it's on.
#[tokio::test]
async fn an_untrusted_plugins_calls_are_checked_against_its_own_permissions() {
    let home = dir("calls");
    let k = TestApp::new().with(wisp_plugins::PluginsPlugin).service(Paths::new(&home)).start().await;
    let d = home.join("src-pane");
    std::fs::create_dir_all(&d).unwrap();
    let m = json!({ "wispPlugin": 1, "id": "user.pane", "name": "Pane", "version": "1.0.0", "api": "^1", "permissions": ["plugins:read"], "activation": [],
        "contributes": { "ui": [{ "id": "user.pane.settings", "attachTo": { "id": "shell.settings", "input": "pane" }, "component": "./ui.js#default", "title": "Pane" }] } });
    std::fs::write(d.join("wisp-plugin.json"), m.to_string()).unwrap();
    std::fs::write(d.join("ui.js"), "wisp.render({ c: 'Text', text: 'hi' });").unwrap();
    k.call(Caller::user(), "plugins.install", json!({ "source": d.display().to_string() })).await.unwrap();
    assert_eq!(k.call(Caller::user(), "plugins.ui", json!({})).await.unwrap(), json!([]), "off: nothing shows");
    k.call(Caller::user(), "plugins.setEnabled", json!({ "id": "user.pane", "enabled": true })).await.unwrap();
    let ui = k.call(Caller::user(), "plugins.ui", json!({})).await.unwrap();
    assert_eq!(
        (ui[0]["plugin"].as_str(), ui[0]["place"].as_str(), ui[0]["module"].as_str()),
        (Some("user.pane"), Some("shell.settings"), Some("wisp.render({ c: 'Text', text: 'hi' });"))
    );
    // as itself: what it asked for, and nothing it didn't
    let me = Caller::set("plugin:user.pane");
    assert!(k.call(me.clone(), "plugins.list", json!({})).await.is_ok(), "plugins:read it asked for");
    let no = k.call(me.clone(), "plugins.install", json!({ "source": d.display().to_string() })).await.unwrap_err();
    assert_eq!(no.code, wisp_kernel::Code::Forbidden, "plugins:write it never asked for: {no:?}");
    assert!(k.call(me, "plugins.setEnabled", json!({ "id": "user.pane", "enabled": false })).await.is_err());
    let _ = std::fs::remove_dir_all(&home);
}

/// A plugin's folder held as a running plugin holds it on Windows: a file in it open with no sharing (Windows), or the
/// folder read-only (Unix: a folder you can't write can't move to another folder). Let go when dropped.
struct Held {
    dir: PathBuf,
    #[cfg(windows)]
    _file: std::fs::File,
}

fn hold(dir: &Path) -> Held {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let f = std::fs::OpenOptions::new().read(true).share_mode(0).open(dir.join("README.md")).unwrap();
        Held { dir: dir.to_owned(), _file: f }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        Held { dir: dir.to_owned() }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o755));
        }
        let _ = &self.dir;
    }
}

fn version_and_state(k: &wisp_kernel::Kernel, id: &str) -> Option<(String, String)> {
    k.status().into_iter().find(|s| s.id == id).map(|s| (s.version, s.state))
}

/// r310: a reinstall (or uninstall) whose old folder can't be moved, as on Windows while the old version runs or a
/// file in it is open, changes nothing: the old version stays installed, on and in its folder, nothing half-done is
/// left in `~/.wisp/plugins`, and the error says what happened. Once the folder is free, the same install works.
#[tokio::test]
async fn a_reinstall_whose_old_folder_is_in_use_keeps_the_old_one_working() {
    let home = dir("held");
    let k = TestApp::new().with(wisp_plugins::PluginsPlugin).service(Paths::new(&home)).start().await;
    let v1 = source(&home, "user.theme", "1.0.0");
    k.call(Caller::user(), "plugins.install", json!({ "source": v1.display().to_string() })).await.unwrap();
    k.call(Caller::user(), "plugins.setEnabled", json!({ "id": "user.theme", "enabled": true })).await.unwrap();
    let on = version_and_state(&k, "user.theme").unwrap();
    assert_eq!(on.0, "1.0.0");
    assert_ne!(on.1, "off");
    let folder = home.join("plugins/theme");
    #[cfg(unix)]
    if std::process::Command::new("id").arg("-u").output().is_ok_and(|o| o.stdout.trim_ascii() == b"0") {
        eprintln!("skipped: as root a read-only folder still moves, so it can't stand in for one in use");
        return;
    }
    let held = hold(&folder);
    let v2 = source(&home, "user.theme", "2.0.0");
    let e = k.call(Caller::user(), "plugins.install", json!({ "source": v2.display().to_string() })).await.unwrap_err();
    assert!(e.message.contains("user.theme 1.0.0 is still installed and on"), "{}", e.message);
    assert_eq!(version_and_state(&k, "user.theme"), Some(on.clone()), "the old one, as it was");
    let text = std::fs::read_to_string(folder.join("wisp-plugin.json")).unwrap();
    assert!(text.contains("\"1.0.0\""), "its folder untouched: {text}");
    let left: Vec<String> = std::fs::read_dir(home.join("plugins"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != "theme" && n != ".old")
        .collect();
    assert!(left.is_empty(), "no staging folder left: {left:?}");
    let e = k.call(Caller::user(), "plugins.uninstall", json!({ "id": "user.theme" })).await.unwrap_err();
    assert!(e.message.contains("Couldn't remove user.theme") && e.message.contains("still installed"), "{}", e.message);
    assert_eq!(version_and_state(&k, "user.theme"), Some(on), "an uninstall that couldn't remove it put it back");
    drop(held);
    let out = k.call(Caller::user(), "plugins.install", json!({ "source": v2.display().to_string() })).await.unwrap();
    assert_eq!(out["version"], json!("2.0.0"));
    assert_eq!(version_and_state(&k, "user.theme").map(|v| v.0).as_deref(), Some("2.0.0"));
    let old: Vec<_> = std::fs::read_dir(home.join("plugins/.old")).map(|r| r.flatten().collect()).unwrap_or_default();
    assert!(old.is_empty(), "the old folder deleted once swapped: {old:?}");
    let _ = std::fs::remove_dir_all(&home);
}

/// r310: at start, `~/.wisp/plugins/.old/` is tidied: a folder a replace or remove set aside is deleted (an
/// uninstalled plugin never comes back), and one a failed replace couldn't put back (its `.keep` mark) returns to its
/// place and loads, unless something took that place.
#[test]
fn folders_set_aside_are_deleted_at_start_or_put_back_when_marked() {
    let home = dir("tidy");
    let plugins = home.join("plugins");
    let manifest = |id: &str| json!({ "wispPlugin": 1, "id": id, "version": "1.0.0", "api": "^1" }).to_string();
    for (name, id) in [("gone.1.a", "user.gone"), ("kept.1.b", "user.kept")] {
        std::fs::create_dir_all(plugins.join(".old").join(name)).unwrap();
        std::fs::write(plugins.join(".old").join(name).join("wisp-plugin.json"), manifest(id)).unwrap();
    }
    std::fs::write(plugins.join(".old/kept.1.b.keep"), "").unwrap();
    let got = wisp_plugins::user_plugins(&plugins);
    assert_eq!(got.iter().map(|p| wisp_kernel::Plugin::manifest(p).id).collect::<Vec<_>>(), ["user.kept"]);
    assert!(plugins.join("kept/wisp-plugin.json").is_file() && !plugins.join("gone").exists());
    assert!(!plugins.join(".old").exists(), "nothing left aside");
    let _ = std::fs::remove_dir_all(&home);
}
