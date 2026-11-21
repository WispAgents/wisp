//! v1 test/update.test.js, the parts that are decisions: channels, install methods, what to run, the
//! relaunch hand-off and the app's release pick. The IO parts are in wisp-update's tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests may fail loudly
use serde_json::{Value, json};
use wisp_update_core::app::sha256_hex;
use wisp_update_core::deferred::Files;
use wisp_update_core::{
    Env, Install, InstallStep, Method, Platform, Refusal, Status, Swap, Updater, asset_name, cargo_clone, detect, installed_version,
};

const CURRENT: &str = "2.0.0";
const KEG_BIN: &str =
    "/opt/homebrew/Cellar/wisp-agents/2.0.0/libexec/lib/node_modules/wisp-agents/node_modules/wisp-agents-darwin-arm64/bin/wisp";

fn mac() -> Platform {
    Platform::new("darwin", "arm64")
}

fn global() -> Install {
    detect("/usr/local/lib/node_modules/wisp-agents/node_modules/wisp-agents-darwin-arm64/bin/wisp", &mac(), None, false, None)
}

fn files() -> Files {
    Files { dir: "C:\\Users\\O'Brien\\.wisp\\plugins\\wisp.update".into(), exe: "C:\\x\\wisp.exe".into() }
}

fn updater(install: Install, env: Env) -> Updater {
    Updater::new(install, mac(), CURRENT, 1, env)
}

fn check(u: &mut Updater, pref: &str, body: &Value) -> wisp_update_core::View {
    let req = u.begin_check(0, pref);
    u.end_check(&req, Some(body));
    u.view(pref)
}

#[test]
fn npm_installs_follow_their_channel_and_the_newest_wins() {
    let dist = json!({ "latest": "2.0.1", "next": "2.1.0-beta.1", "nightly": "2.1.0-nightly.20261024.317" });
    for (channel, latest, tag) in [
        ("stable", "2.0.1", "latest"),
        ("beta", "2.1.0-beta.1", "next"),
        ("nightly", "2.1.0-nightly.20261024.317", "nightly"),
        ("bogus", "2.0.1", "latest"),
    ] {
        let mut u = updater(global(), Env::default());
        let v = check(&mut u, channel, &dist);
        assert_eq!(v.latest.as_deref(), Some(latest), "{channel}");
        assert_eq!(v.channel, if channel == "bogus" { "stable" } else { channel });
        assert_eq!(v.command.as_deref(), Some(format!("npm install -g wisp-agents@{tag}").as_str()));
        assert!(v.available);
    }
    // A stable release newer than the last beta: beta takes it, from latest.
    let mut u = updater(global(), Env::default());
    assert_eq!(check(&mut u, "beta", &json!({ "latest": "2.1.0", "next": "2.1.0-beta.3" })).latest.as_deref(), Some("2.1.0"));
    assert_eq!(u.command().as_deref(), Some("npm install -g wisp-agents@latest"));
    u.install.method = Method::Npx;
    assert!(u.relaunch_command(false).starts_with("npx -y wisp-agents@latest "));
}

#[test]
fn the_registry_is_asked_and_a_failed_fetch_changes_nothing() {
    let mut u = updater(global(), Env::default());
    let req = u.begin_check(0, "stable");
    assert_eq!(req.url, "https://registry.npmjs.org/-/package/wisp-agents/dist-tags");
    assert_eq!(req.accept, "application/json");
    assert!(!u.end_check(&req, None));
    assert_eq!(u.view("stable").latest, None);
    assert!(!u.available);
}

#[test]
fn homebrew_stays_on_stable_whatever_the_channel_says() {
    let mut u = updater(detect(KEG_BIN, &mac(), None, false, None), Env::default());
    let v = check(&mut u, "nightly", &json!({ "latest": "2.0.1", "nightly": "99.0.0-nightly.1" }));
    assert_eq!(v.channel, "stable");
    assert_eq!(v.latest.as_deref(), Some("2.0.1"));
}

#[test]
fn install_method_tells_a_clone_npx_a_keg_an_app_and_a_global_install_apart() {
    let p = mac();
    assert_eq!(detect("/home/u/.npm/_npx/abc123/node_modules/wisp-agents-linux-x64/bin/wisp", &p, None, false, None).method, Method::Npx);
    let keg = detect(KEG_BIN, &p, None, false, None);
    assert_eq!(keg.method, Method::Brew);
    assert_eq!(keg.brew.as_deref(), Some("/opt/homebrew"));
    assert_eq!(
        detect("/home/linuxbrew/.linuxbrew/Cellar/wisp-agents/2.0.0/bin/wisp", &p, None, false, None).brew.as_deref(),
        Some("/home/linuxbrew/.linuxbrew")
    );
    let g = global();
    assert_eq!(g.method, Method::Global);
    assert_eq!(g.root, "/usr/local/lib/node_modules/wisp-agents", "hoisted or nested, the launcher's folder");
    assert_eq!(
        detect("/usr/local/lib/node_modules/wisp-agents-darwin-arm64/bin/wisp", &p, None, false, None).root,
        "/usr/local/lib/node_modules/wisp-agents"
    );
    assert_eq!(cargo_clone("/Users/me/wisp/target/release/wisp").as_deref(), Some("/Users/me/wisp"));
    assert_eq!(cargo_clone("/usr/local/bin/wisp"), None);
    assert_eq!(detect("/Users/me/wisp/target/release/wisp", &p, None, false, Some("/Users/me/wisp")).method, Method::Git);
    let app = detect("/Applications/wisp.app/Contents/MacOS/wisp", &p, None, true, None);
    assert_eq!((app.method, app.root.as_str()), (Method::App, "/Applications/wisp.app"));
    let image = detect("/tmp/.mount_wispAb12/usr/bin/wisp", &Platform::new("linux", "x64"), Some("/home/u/wisp.AppImage"), true, None);
    assert_eq!((image.method, image.root.as_str()), (Method::App, "/home/u/wisp.AppImage"));
    let win = detect("C:\\Users\\u\\AppData\\Local\\Programs\\wisp\\wisp.exe", &Platform::new("win32", "x64"), None, true, None);
    assert_eq!((win.method, win.root.as_str()), (Method::App, "C:\\Users\\u\\AppData\\Local\\Programs\\wisp"));
}

/// ADR-0090: the platform packages are `wisp-agents-<os>-<arch>`, unscoped; a look-alike package is not one.
#[test]
fn a_hoisted_binary_is_found_only_in_a_platform_package_folder() {
    let p = Platform::new("linux", "x64");
    let hoisted = detect("/usr/local/lib/node_modules/wisp-agents-linux-x64/bin/wisp", &p, None, false, None);
    assert_eq!((hoisted.method, hoisted.root.as_str()), (Method::Global, "/usr/local/lib/node_modules/wisp-agents"));
    let win = Platform::new("win32", "x64");
    let exe = "C:\\npm\\node_modules\\wisp-agents-win32-arm64\\bin\\wisp.exe";
    assert_eq!(detect(exe, &win, None, false, None).root, "C:\\npm\\node_modules\\wisp-agents");
    for other in [
        "/usr/local/lib/node_modules/wisp-agents-foo/bin/wisp",
        "/usr/local/lib/node_modules/wisp-agents-linux-x64-extra/bin/wisp",
        "/usr/local/lib/node_modules/wisp-agents-mcp-server/bin/wisp",
        "/usr/local/lib/node_modules/@wisp/core-linux-x64/bin/wisp",
        "/opt/wisp-agents-linux-x64/bin/wisp",
    ] {
        assert_eq!(detect(other, &p, None, false, None).root, other, "{other} has no launcher folder");
    }
    // hoisted inside a keg: the keg's Cellar/wisp-agents segment isn't the launcher folder
    let keg = detect(
        "/opt/homebrew/Cellar/wisp-agents/2.0.0/libexec/lib/node_modules/wisp-agents-darwin-arm64/bin/wisp",
        &mac(),
        None,
        false,
        None,
    );
    assert_eq!(keg.method, Method::Brew);
    assert_eq!(keg.root, "/opt/homebrew/Cellar/wisp-agents/2.0.0/libexec/lib/node_modules/wisp-agents");
}

#[test]
fn a_git_clone_never_offers_an_update() {
    let mut u = updater(Install { method: Method::Git, root: "/src/wisp".into(), brew: None }, Env::default());
    assert!(!u.enabled);
    let v = check(&mut u, "stable", &json!({ "latest": "99.0.0" }));
    assert!(!v.available);
    assert_eq!(v.command, None);
    assert!(serde_json::to_value(&v).is_ok_and(|j| j.get("command").is_none()), "v1 left the key out");
    assert!(matches!(u.begin_install(), Err(Refusal::Invalid(m)) if m.contains("git clone")));
}

#[test]
fn opt_outs_turn_the_check_off() {
    for env in [
        Env { update_check: Some("0".into()), ..Env::default() },
        Env { update_check: Some("off".into()), ..Env::default() },
        Env { ci: Some("true".into()), ..Env::default() },
        Env { ci: Some("0".into()), ..Env::default() },
    ] {
        assert!(!updater(global(), env).enabled);
    }
    assert!(updater(global(), Env { update_check: Some("1".into()), ci: Some(String::new()), ..Env::default() }).enabled);
}

#[test]
fn due_at_most_once_a_day_on_the_wall_clock() {
    let mut u = updater(global(), Env::default());
    let t = 1_000_000;
    assert!(u.due(t));
    u.begin_check(t, "stable");
    assert!(!u.due(t + 23 * 3_600_000));
    assert!(u.due(t + 24 * 3_600_000));
}

#[test]
fn relaunch_keeps_the_port_wisp_is_serving_on() {
    let mut u = Updater::new(global(), mac(), CURRENT, 5123, Env::default());
    for m in [Method::Global, Method::Npx] {
        u.install.method = m;
        assert!(u.relaunch_command(false).ends_with(" --browser --no-open --port 5123"));
        let d = u.relaunch_command(true);
        assert!(d.ends_with("wisp-agents@latest --port 5123") || d.ends_with("wisp.js' --port 5123"), "{d}");
    }
    u.install.method = Method::Global;
    u.env.node = Some("/usr/local/bin/node".into());
    assert_eq!(u.relaunch_command(true), "'/usr/local/bin/node' '/usr/local/lib/node_modules/wisp-agents/bin/wisp.js' --port 5123");
    u.env.desktop = Some("tauri".into());
    u.env.ppid = 77;
    assert!(u.relaunch_command(true).starts_with("for i in $(seq 100); do kill -0 77 2>/dev/null || break; sleep 0.1; done; "));
}

#[test]
fn homebrew_upgrades_through_brew_and_relaunches_from_opt() {
    let u = Updater::new(detect(KEG_BIN, &mac(), None, false, None), mac(), CURRENT, 5123, Env::default());
    assert_eq!(u.command().as_deref(), Some("'/opt/homebrew/bin/brew' upgrade wisp-agents"));
    assert!(
        u.relaunch_command(false)
            .ends_with("'/opt/homebrew/opt/wisp-agents/libexec/lib/node_modules/wisp-agents/bin/wisp.js' --browser --no-open --port 5123")
    );
}

#[test]
fn install_runs_the_command_and_refuses_to_relaunch_the_same_version() {
    let mut brew = Updater::new(detect(KEG_BIN, &mac(), None, false, None), mac(), CURRENT, 1, Env::default());
    brew.latest = Some("99.0.0".into());
    let step = brew.begin_install();
    assert_eq!(
        step,
        Ok(InstallStep::Run {
            cmd: "'/opt/homebrew/bin/brew' upgrade wisp-agents".into(),
            env: vec![("HOMEBREW_NO_INSTALL_CLEANUP".into(), "1".into())]
        })
    );
    assert_eq!(brew.status, Status::Installing);
    assert!(matches!(brew.begin_install(), Err(Refusal::Conflict(m)) if m == "Already updating."));
    let why = brew.check_installed(Some(CURRENT)).unwrap_err();
    assert_eq!(why, "Homebrew doesn't have wisp 99.0.0 yet. Try again later.");
    brew.install_failed(&why);
    assert_eq!((brew.status, brew.error.as_deref()), (Status::Idle, Some(why.as_str())));
    let g = updater(global(), Env::default());
    assert_eq!(g.check_installed(Some(CURRENT)).unwrap_err(), "npm installed wisp somewhere else: this copy is still 2.0.0.");
    assert_eq!(g.check_installed(None).unwrap_err(), "npm installed wisp somewhere else: this copy is still 2.0.0.");
    assert!(g.check_installed(Some("2.0.1")).is_ok());
    let mut npx = updater(global(), Env::default());
    npx.install.method = Method::Npx;
    assert_eq!(npx.begin_install(), Ok(InstallStep::Nothing));
    npx.install_done();
    assert_eq!(npx.view("stable").status, Status::Restarting);
}

#[test]
fn run_failures_say_what_failed_and_hint_at_permissions() {
    let m = wisp_update_core::run_failed("npm install -g wisp-agents@latest", "a\nb\nnpm error EACCES: permission denied\nlast\n");
    assert_eq!(
        m,
        "`npm install -g wisp-agents@latest` failed (permission denied — run it yourself, maybe with sudo): b npm error EACCES: permission denied last"
    );
    assert_eq!(wisp_update_core::run_failed("x", "boom"), "`x` failed: boom");
}

fn app(os: &str, arch: &str, exe: &str, image: Option<&str>) -> Updater {
    let p = Platform::new(os, arch);
    let env = Env { appimage: image.map(str::to_owned), ..Env::default() };
    Updater::new(detect(exe, &p, image, true, None), p, CURRENT, 1, env)
}

#[test]
fn the_app_checks_github_releases_and_picks_its_download() {
    let assets: Vec<Value> = ["wisp-mac-arm64.zip", "wisp-mac-x64.zip", "wisp-windows-x64-setup.exe", "wisp-linux-arm64.AppImage"]
        .iter()
        .map(|n| json!({ "name": n, "browser_download_url": format!("https://dl/{n}") }))
        .collect();
    let rel = json!({ "tag_name": "v99.0.0", "assets": assets });
    for (os, arch, exe, image, want) in [
        ("darwin", "arm64", "/Applications/wisp.app/Contents/MacOS/wisp", None, Some("wisp-mac-arm64.zip")),
        ("win32", "x64", "C:\\Programs\\wisp\\wisp.exe", None, Some("wisp-windows-x64-setup.exe")),
        ("linux", "arm64", "/tmp/.mount_x/usr/bin/wisp", Some("/home/u/wisp.AppImage"), Some("wisp-linux-arm64.AppImage")),
        ("linux", "x64", "/usr/lib/wisp/wisp", None, None), // a .deb: no automatic update
    ] {
        let mut u = app(os, arch, exe, image);
        let req = u.begin_check(0, "stable");
        assert_eq!(req.url, "https://api.github.com/repos/WispAgents/wisp/releases/latest");
        assert_eq!(req.accept, "application/vnd.github+json");
        u.end_check(&req, Some(&rel));
        let v = u.view("stable");
        assert_eq!((v.method, v.latest.as_deref(), v.available), ("app", Some("99.0.0"), true));
        assert_eq!(v.command.as_deref(), Some("https://github.com/WispAgents/wisp/releases/latest"));
        assert_eq!(u.asset.as_ref().map(|a| a.name.as_str()), want);
        assert_eq!(asset_name(&u.platform, image).as_deref(), want);
    }
}

#[test]
fn the_app_on_beta_or_nightly_picks_the_newest_release_its_channel_allows() {
    let asset = |v: &str| json!([{ "name": "wisp-mac-arm64.zip", "browser_download_url": format!("https://dl/{v}/wisp-mac-arm64.zip") }]);
    let list = json!([
        { "tag_name": "nightly", "name": "wisp 99.1.0-nightly.20261024.317", "prerelease": true, "html_url": "https://github.com/WispAgents/wisp/releases/tag/nightly", "assets": asset("nightly") },
        { "tag_name": "v99.1.0-beta.1", "name": "v99.1.0-beta.1", "prerelease": true, "html_url": "https://github.com/WispAgents/wisp/releases/tag/v99.1.0-beta.1", "assets": asset("beta") },
        { "tag_name": "v99.2.0-beta.1", "draft": true, "prerelease": true, "assets": asset("draft") },
        { "tag_name": "v99.0.0", "prerelease": false, "html_url": "https://github.com/WispAgents/wisp/releases/tag/v99.0.0", "assets": asset("stable") },
    ]);
    for (channel, latest, from) in [("beta", "99.1.0-beta.1", "beta"), ("nightly", "99.1.0-nightly.20261024.317", "nightly")] {
        let mut u = app("darwin", "arm64", "/Applications/wisp.app/Contents/MacOS/wisp", None);
        let req = u.begin_check(0, channel);
        assert_eq!(req.url, "https://api.github.com/repos/WispAgents/wisp/releases?per_page=30");
        u.end_check(&req, Some(&list));
        let v = u.view(channel);
        assert_eq!(v.latest.as_deref(), Some(latest), "{channel}");
        assert_eq!(u.asset.as_ref().map(|a| a.url.clone()), Some(format!("https://dl/{from}/wisp-mac-arm64.zip")));
        assert_eq!(v.command, u.release.clone());
        assert!(v.command.as_deref().is_some_and(|c| c.contains(from)));
    }
}

#[test]
fn app_downloads_are_checked_against_the_release_sha256() {
    let u = app("linux", "x64", "/tmp/.mount_x/usr/bin/wisp", Some("/home/u/wisp.AppImage"));
    let body = b"new build";
    let good = wisp_update_core::Asset {
        name: "wisp-linux-x64.AppImage".into(),
        url: "https://dl/x".into(),
        digest: Some(format!("sha256:{}", sha256_hex(body))),
    };
    assert!(u.verify_download(&good, body).is_ok());
    let bad = wisp_update_core::Asset { digest: Some(format!("sha256:{}", "0".repeat(64))), ..good.clone() };
    assert!(u.verify_download(&bad, body).unwrap_err().contains("didn't match the release's checksum"));
    for digest in [None, Some(String::new()), Some("sha256:".into()), Some("sha256:abc".into())] {
        let e = u.verify_download(&wisp_update_core::Asset { digest: digest.clone(), ..good.clone() }, body).unwrap_err();
        assert!(
            e.starts_with("The release has no checksum for wisp-linux-x64.AppImage, so wisp won't install it. Download it from"),
            "{digest:?}: {e}"
        );
    }
    let upper = wisp_update_core::Asset { digest: Some(sha256_hex(body).to_uppercase()), ..good };
    assert!(u.verify_download(&upper, body).is_ok(), "a bare hex digest, any case");
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn the_app_relaunches_through_the_installer_on_windows_and_open_on_macos() {
    let mut win = app("win32", "x64", "C:\\Programs\\wisp\\wisp.exe", None);
    win.swap = Some(Swap::Windows { installer: "C:\\Temp\\wisp-update-1\\wisp-windows-x64-setup.exe".into() });
    assert_eq!(win.relaunch_command(true), "\"C:\\Temp\\wisp-update-1\\wisp-windows-x64-setup.exe\" /S /R");
    let mut mac = app("darwin", "arm64", "/Applications/wisp.app/Contents/MacOS/wisp", None);
    mac.swap = Some(Swap::Mac { open: "/Applications/wisp.app".into(), remove: "/tmp/wisp-update-1/old.app".into() });
    assert_eq!(mac.relaunch_command(true), "rm -rf '/tmp/wisp-update-1/old.app'; open '/Applications/wisp.app'");
    let mut linux = app("linux", "x64", "/tmp/.mount_x/usr/bin/wisp", Some("/home/u/wisp.AppImage"));
    linux.swap = Some(Swap::Linux { open: "/home/u/wisp.AppImage".into() });
    assert_eq!(linux.relaunch_command(true), "'/home/u/wisp.AppImage' >/dev/null 2>&1");
}

/// Audit H HS10: with checks off, `wisp update apply` blamed the copy ("no automatic update for this copy"), while the
/// same copy updates with checks on. It says checks are off and how to turn them on, as Settings → Updates does.
#[test]
fn an_app_with_checks_off_says_checks_are_off_not_that_it_cant_update() {
    let p = Platform::new("darwin", "arm64");
    let env = Env { update_check: Some("0".into()), ..Env::default() };
    let mut u = Updater::new(detect("/Applications/wisp.app/Contents/Resources/app/wisp", &p, None, true, None), p, CURRENT, 1, env);
    let Err(Refusal::Invalid(m)) = u.begin_install() else { panic!("must refuse") };
    assert!(m.starts_with("Update checks are off here (WISP_UPDATE_CHECK=0 or CI)"), "{m}");
    assert!(
        m.contains("Unset WISP_UPDATE_CHECK") && m.ends_with("download it from https://github.com/WispAgents/wisp/releases/latest"),
        "{m}"
    );
    assert!(!m.contains("no automatic update"), "{m}");
}

#[test]
fn an_app_with_no_download_for_it_says_where_to_get_the_new_version() {
    let mut u = app("linux", "x64", "/usr/lib/wisp/wisp", None);
    let req = u.begin_check(0, "stable");
    u.end_check(&req, Some(&json!({ "tag_name": "v99.0.0", "assets": [] })));
    let Err(Refusal::Invalid(m)) = u.begin_install() else { panic!("must refuse") };
    assert!(m.contains("no automatic update") && m.ends_with("releases/latest"), "{m}");
    assert_eq!(u.status, Status::Idle);
    assert_eq!(u.error.as_deref(), Some(m.as_str()));
}

#[test]
fn installed_version_reads_a_package_json() {
    assert_eq!(installed_version(r#"{"name":"wisp-agents","version":"2.0.1"}"#).as_deref(), Some("2.0.1"));
    assert_eq!(installed_version("not json"), None);
}

/// v1 update.js `handOff`: after an install, a detached waiter stops this wisp (SIGTERM, its clean stop),
/// waits until it's gone and starts the new version on the same port. Only once the install is done.
#[test]
fn an_installed_update_relaunches_wisp_through_a_detached_waiter() {
    let mut u = updater(global(), Env::default());
    let _ = check(&mut u, "stable", &json!({ "latest": "9.0.0" }));
    assert_eq!(u.hand_off(4242, &files()), None, "nothing installed yet: no relaunch");
    u.install_done();
    let h = u.hand_off(4242, &files()).expect("a relaunch");
    assert_eq!((h.bin, h.args[0].as_str()), ("sh", "-c"));
    let script = &h.args[1];
    assert!(script.contains("kill -TERM 4242"), "{script}");
    assert!(script.contains("kill -0 4242"), "it waits for this wisp to be gone: {script}");
    assert!(script.ends_with(&u.relaunch_command(false)), "then starts the new one, keeping the tab: {script}");
}

/// v1 relaunched on Windows too (its node waiter); v2 said "restart wisp yourself" there. Now a detached PowerShell
/// waiter ends this wisp, waits until it's gone and starts the new one on the same port through `cmd /S /C`, the
/// script passed encoded so no quote in a path (`C:\Users\O'Brien`) breaks it.
#[test]
fn an_installed_update_relaunches_wisp_on_windows_through_powershell() {
    let win = Platform::new("win32", "x64");
    let root = "C:\\Users\\O'Brien\\AppData\\Roaming\\npm\\node_modules\\wisp-agents";
    let exe = format!("{root}\\node_modules\\wisp-agents-win32-x64\\bin\\wisp.exe");
    let mut u = Updater::new(detect(&exe, &win, None, false, None), win, CURRENT, 4477, Env::default());
    assert_eq!(u.method(), Method::Global);
    let _ = check(&mut u, "stable", &json!({ "latest": "9.0.0" }));
    assert_eq!(u.hand_off(7, &files()), None, "nothing installed yet: no relaunch");
    u.install_done();
    let h = u.hand_off(7, &files()).expect("a relaunch on Windows");
    assert_eq!(h.bin, "powershell");
    assert_eq!(h.args[..5], ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand"]);
    assert_eq!(h.args[5], wisp_update_core::encode_ps(&h.script), "the script travels encoded");
    assert!(h.script.contains("Stop-Process -Id 7"), "{}", h.script);
    assert!(h.script.contains("Wait-Process -Id 7 -Timeout 30"), "it waits for this wisp to be gone: {}", h.script);
    let cmd = u.relaunch_command(false);
    assert!(cmd.ends_with("--browser --no-open --port 4477"), "same port, the tab kept: {cmd}");
    let quoted = format!("/S /C \"{cmd}\"").replace('\'', "''");
    assert!(h.script.ends_with(&format!("-ArgumentList '{quoted}' -WindowStyle Hidden")), "{}", h.script);
    assert!(h.script.contains("O''Brien"), "a quote in the path is doubled in PowerShell's string: {}", h.script);
}

#[test]
fn powershell_encoded_command_is_utf16le_base64() {
    // what `[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes('dir'))` prints
    assert_eq!(wisp_update_core::encode_ps("dir"), "ZABpAHIA");
}

/// The packaged app's own server (desktop/scripts/package.mjs `overlay`: `Contents/Resources/app` on macOS,
/// `resources/app` elsewhere) is the app however it was started; v2 only knew it with `WISP_DESKTOP` set on
/// Windows and Linux, so the same binary run from a terminal offered `npm install -g`.
#[test]
fn the_packaged_apps_server_is_the_app_in_tauris_layout() {
    let mac = detect("/Applications/wisp.app/Contents/Resources/app/wisp", &mac(), None, false, None);
    assert_eq!((mac.method, mac.root.as_str()), (Method::App, "/Applications/wisp.app"));
    let win = Platform::new("win32", "x64");
    for desktop in [true, false] {
        let w = detect("C:\\Users\\u\\AppData\\Local\\wisp\\resources\\app\\wisp.exe", &win, None, desktop, None);
        assert_eq!((w.method, w.root.as_str()), (Method::App, "C:\\Users\\u\\AppData\\Local\\wisp\\resources\\app"), "desktop {desktop}");
    }
    let linux = Platform::new("linux", "x64");
    let deb = detect("/usr/lib/wisp/resources/app/wisp", &linux, None, false, None);
    assert_eq!((deb.method, deb.root.as_str()), (Method::App, "/usr/lib/wisp/resources/app"));
    let image = detect("/tmp/.mount_wispAb12/usr/lib/wisp/resources/app/wisp", &linux, Some("/home/u/Apps/wisp.AppImage"), false, None);
    assert_eq!((image.method, image.root.as_str()), (Method::App, "/home/u/Apps/wisp.AppImage"), "the AppImage is what updates");
    // not the layout: a folder merely named app, or the repo's own stage
    assert_eq!(
        detect("/usr/local/lib/node_modules/wisp-agents/node_modules/wisp-agents-linux-x64/bin/wisp", &linux, None, false, None).method,
        Method::Global
    );
    assert_eq!(detect("/home/u/app/wisp", &linux, None, false, None).method, Method::Global);
    assert_eq!(detect("/home/u/wisp/desktop/src-tauri/stage/app/wisp", &linux, None, false, Some("/home/u/wisp")).method, Method::Git);
}

#[test]
fn a_bundles_version_is_its_info_plists_short_version() {
    use wisp_update_core::app::bundle_version;
    let plist = "<?xml version=\"1.0\"?>\n<plist version=\"1.0\">\n<dict>\n  <key>CFBundleName</key>\n  <string>wisp</string>\n  \
                 <key>CFBundleShortVersionString</key>\n  <string>2.3.0</string>\n  <key>CFBundleVersion</key>\n  <string>2.3.0.7</string>\n</dict>\n</plist>\n";
    assert_eq!(bundle_version(plist).as_deref(), Some("2.3.0"));
    assert_eq!(bundle_version("<key>CFBundleShortVersionString</key><string>2.4.0-beta.1</string>").as_deref(), Some("2.4.0-beta.1"));
    assert_eq!(bundle_version("<key>CFBundleShortVersionString</key>\n<string></string>"), None, "empty");
    assert_eq!(bundle_version("<key>CFBundleShortVersionString</key>\n<integer>2</integer>"), None, "not a string");
    assert_eq!(bundle_version("<key>CFBundleVersion</key><string>2.3.0</string>"), None, "only the short version counts");
    let u = app("darwin", "arm64", "/Applications/wisp.app/Contents/Resources/app/wisp", None);
    assert!(u.check_bundle(bundle_version(plist).as_deref()).is_ok());
    assert!(u.check_bundle(Some(CURRENT)).unwrap_err().starts_with("The download isn't newer than wisp 2.0.0. Download it from"));
}

#[test]
fn every_app_update_failure_says_where_to_download_it() {
    let u = app("darwin", "arm64", "/Applications/wisp.app/Contents/Resources/app/wisp", None);
    let a = wisp_update_core::Asset { name: "wisp-mac-arm64.zip".into(), url: "https://dl/x".into(), digest: None };
    let tail = "Download it from https://github.com/WispAgents/wisp/releases/latest";
    for (m, head) in [
        (u.download_failed(&a, 404), "Couldn't download wisp-mac-arm64.zip (HTTP 404). "),
        (u.cant_download(&a, "offline"), "Couldn't download wisp-mac-arm64.zip: offline. "),
        (u.cant_save(&a, "disk full"), "Couldn't save wisp-mac-arm64.zip: disk full. "),
        (u.cant_unpack(&a, "bad zip"), "Couldn't unpack wisp-mac-arm64.zip: bad zip. "),
        (u.cant_replace_bundle("/Applications"), "wisp can't replace itself in /Applications (no permission). "),
        (u.cant_place_bundle("busy"), "Couldn't put the new wisp in place: busy. "),
        (u.cant_replace_image("/home/u/wisp.AppImage"), "wisp can't replace /home/u/wisp.AppImage (no permission). "),
        (
            u.cant_update_read_only("/Volumes/wisp"),
            "wisp is running from a read-only disk (/Volumes/wisp), such as the disk image it came in. Drag wisp.app to \
             Applications, open it from there, then update. ",
        ),
        (
            u.cant_update_translocated(),
            "macOS is running this wisp from a temporary copy because it was opened where it was downloaded. Move wisp.app \
             to Applications, open it from there, then update. ",
        ),
    ] {
        assert_eq!(m, format!("{head}{tail}"));
    }
}

/// The Linux relaunch waits for the old desktop shell to go, as macOS does: it holds the single-instance lock, and a
/// new `AppImage` started meanwhile hands over to it and quits, so nothing came back.
#[test]
fn the_appimage_relaunch_waits_for_the_old_shell() {
    let mut linux = app("linux", "x64", "/tmp/.mount_x/usr/lib/wisp/resources/app/wisp", Some("/home/u/wisp.AppImage"));
    linux.swap = Some(Swap::Linux { open: "/home/u/wisp.AppImage".into() });
    linux.env.desktop = Some("tauri".into());
    linux.env.ppid = 4242;
    assert_eq!(
        linux.relaunch_command(true),
        "for i in $(seq 100); do kill -0 4242 2>/dev/null || break; sleep 0.1; done; '/home/u/wisp.AppImage' >/dev/null 2>&1"
    );
    let mut mac = app("darwin", "arm64", "/Applications/wisp.app/Contents/Resources/app/wisp", None);
    mac.swap = Some(Swap::Mac { open: "/Applications/wisp.app".into(), remove: "/Applications/wisp.app.old-1".into() });
    mac.env.desktop = Some("tauri".into());
    mac.env.ppid = 4242;
    assert_eq!(
        mac.relaunch_command(true),
        "rm -rf '/Applications/wisp.app.old-1'; for i in $(seq 100); do kill -0 4242 2>/dev/null || break; sleep 0.1; done; \
         open '/Applications/wisp.app'"
    );
}

/// Every path the relaunch hands to `sh` is one single-quoted word: a `$(…)`, backtick, `"` or `'` in a folder name is
/// never run or split (v1's JSON quotes let sh expand `$(…)` and backticks inside them).
#[test]
fn relaunch_paths_are_never_run_by_sh() {
    use wisp_update_core::updater::sh_quote;
    assert_eq!(sh_quote("/a b/c"), "'/a b/c'");
    assert_eq!(sh_quote("/Ann's $(rm -rf ~) `id` \"x\""), "'/Ann'\\''s $(rm -rf ~) `id` \"x\"'");
    let evil = "/Users/a$(touch /tmp/pwned)`id`\"'b/Apps";
    let mut linux = app("linux", "x64", "/tmp/.mount_x/usr/bin/wisp", Some("/x/wisp.AppImage"));
    linux.swap = Some(Swap::Linux { open: format!("{evil}/wisp.AppImage") });
    let cmd = linux.relaunch_command(true);
    assert_eq!(cmd, format!("{} >/dev/null 2>&1", sh_quote(&format!("{evil}/wisp.AppImage"))));
    // run it for real with an echo in place of the AppImage: sh sees exactly one word, the path as written
    let out = std::process::Command::new("sh").arg("-c").arg(format!("printf %s {}", sh_quote(evil))).output();
    if let Ok(out) = out {
        assert_eq!(String::from_utf8_lossy(&out.stdout), evil);
    }
    let mut bundle = app("darwin", "arm64", "/Applications/wisp.app/Contents/Resources/app/wisp", None);
    bundle.swap = Some(Swap::Mac { open: format!("{evil}/wisp.app"), remove: format!("{evil}/wisp.app.old-1") });
    assert_eq!(
        bundle.relaunch_command(true),
        format!("rm -rf {}; open {}", sh_quote(&format!("{evil}/wisp.app.old-1")), sh_quote(&format!("{evil}/wisp.app")))
    );
    let mut g = Updater::new(global(), mac(), CURRENT, 1, Env { node: Some(format!("{evil}/node")), ..Env::default() });
    g.install.method = Method::Global;
    assert!(g.relaunch_command(true).starts_with(&sh_quote(&format!("{evil}/node"))));
}

/// `WISP_UPDATE_FEED` points the app's check at a feed on this computer (a test, a packaged build tried end to end
/// against a fake release), and at nothing else.
#[test]
fn the_release_feed_can_be_a_local_one_only() {
    use wisp_update_core::updater::feed_override;
    for ok in ["http://127.0.0.1:8123/releases", "http://localhost:9/r/", "http://[::1]:8123/releases", "http://127.0.0.1/r"] {
        assert_eq!(feed_override(Some(ok)).as_deref(), Some(ok.trim_end_matches('/')), "{ok}");
    }
    for no in [
        "https://example.com/releases",
        "http://example.com/releases",
        "http://127.0.0.1.evil.com/r",
        "http://localhost.evil.com:80/r",
        "https://127.0.0.1:8123/r",
        "file:///tmp/r",
        "",
        "http://localhost:80@evil.com/r",
        "http://127.0.0.1@evil.com/r",
        "http://user@127.0.0.1:8123/r",
        "http://localhost:80\\@evil.com/r",
        "http://127.0.0.1:80x/r",
        "http://127.0.0.1:/r",
        "http://localhost?.evil.com/r",
        "http://[::1/r",
    ] {
        assert_eq!(feed_override(Some(no)), None, "{no}");
    }
    assert_eq!(feed_override(None), None);
    let p = Platform::new("darwin", "arm64");
    let env = Env { feed: feed_override(Some("http://127.0.0.1:8123/releases")), ..Env::default() };
    let mut u = Updater::new(detect("/Applications/wisp.app/Contents/Resources/app/wisp", &p, None, true, None), p, CURRENT, 1, env);
    assert_eq!(u.begin_check(0, "stable").url, "http://127.0.0.1:8123/releases/latest");
    assert_eq!(u.begin_check(0, "beta").url, "http://127.0.0.1:8123/releases?per_page=30");
    let mut npm = updater(global(), Env { feed: Some("http://127.0.0.1:1/r".into()), ..Env::default() });
    assert_eq!(npm.begin_check(0, "stable").url, "https://registry.npmjs.org/-/package/wisp-agents/dist-tags", "the app's feed only");
}

/// Audit I (smaller notes): right after start, before the first check, `wisp update apply` (checks on) said "There's no
/// automatic update for this copy of wisp" and `update view` kept that as its error after the check found 2.0.1. Before
/// any check it says wisp hasn't found out yet (and keeps nothing); a refusal kept from before is gone once a check finds
/// the download.
#[test]
fn an_install_before_the_first_check_says_so_and_a_check_clears_a_stale_refusal_audit_i() {
    let rel = json!({ "tag_name": "v99.0.0", "assets": [{ "name": "wisp-mac-arm64.zip", "browser_download_url": "https://dl/wisp-mac-arm64.zip" }] });
    let mut u = app("darwin", "arm64", "/Applications/wisp.app/Contents/MacOS/wisp", None);
    assert!(u.enabled, "checks on");
    let e = u.begin_install().unwrap_err();
    let said = format!("{e:?}");
    assert!(said.contains("hasn't found out about the update yet") && !said.contains("no automatic update"), "{said}");
    assert_eq!(u.view("stable").error, None, "nothing kept");
    // a refusal kept from an answer that had no download for this computer, then a check that has one
    let req = u.begin_check(0, "stable");
    u.end_check(&req, Some(&json!({ "tag_name": "v99.0.0", "assets": [] })));
    assert!(u.begin_install().is_err());
    assert!(u.view("stable").error.is_some_and(|e| e.contains("no automatic update")));
    let req = u.begin_check(1, "stable");
    assert!(u.end_check(&req, Some(&rel)), "what the UI shows changed");
    assert_eq!(u.view("stable").error, None, "the stale refusal is gone");
    assert!(u.begin_install().is_ok());
}
