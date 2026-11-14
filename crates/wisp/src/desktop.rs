//! The desktop window for a wisp from npm (v1 bin/desktop-shell.js): the Tauri shell alone, one small
//! archive per platform on each GitHub release, downloaded once into ~/.wisp/desktop/<version>/,
//! checked against that release's SHA256SUMS.txt, unpacked with the system's tar, moved into place in
//! one step. It runs this binary as its server (`WISP_BIN`, `WISP_PORT`). Anything short of a shell that's
//! all there and checked gives a reason, and the caller opens a browser tab instead. An installed app (where the
//! installers put it) comes first. `WISP_DESKTOP_APP` names the app to run instead (`none`: no installed app).
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::client::{CliError, Client};

pub const RELEASES: &str = "https://github.com/WispAgents/wisp/releases/download";
pub const GET_APP: &str = "https://trywisp.dev/#install";

/// desktop/scripts/package.mjs names them the same way.
pub fn shell_asset(os: &str, arch: &str) -> Option<String> {
    let os = match os {
        "macos" => "mac",
        "windows" => "windows",
        "linux" => "linux",
        _ => return None,
    };
    let arch = match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => return None,
    };
    Some(format!("wisp-shell-{os}-{arch}.tar.gz"))
}

/// The app bundle on a Mac (its Info.plist names the Dock tile), the bare binary elsewhere.
pub fn shell_exe(dir: &Path, os: &str) -> PathBuf {
    match os {
        "macos" => dir.join("wisp.app/Contents/MacOS/wisp"),
        "windows" => dir.join("wisp.exe"),
        _ => dir.join("wisp"),
    }
}

/// The rolling nightly is tagged `nightly`, every other release v<version>; `WISP_SHELL_URL` stands in.
pub fn release_url(version: &str, over: Option<&str>) -> String {
    over.map_or_else(
        || format!("{RELEASES}/{}", if version.contains("-nightly.") { "nightly".to_owned() } else { format!("v{version}") }),
        str::to_owned,
    )
    .trim_end_matches('/')
    .to_owned()
}

pub fn sum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hash, file) = l.trim().split_once(char::is_whitespace)?;
        (hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) && file.trim().trim_start_matches('*') == name)
            .then(|| hash.to_ascii_lowercase())
    })
}

async fn get(client: &reqwest::Client, url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    let r = client.get(url).timeout(timeout).send().await.map_err(|e| format!("couldn't reach {url} ({e})"))?;
    if r.status().as_u16() == 404 {
        return Err(format!("{url} isn't there"));
    }
    if !r.status().is_success() {
        return Err(format!("{url} answered {}", r.status()));
    }
    Ok(r.bytes().await.map_err(|e| e.to_string())?.to_vec())
}

/// The shell's executable, fetched if this version has none yet; Err: why there is no window.
pub async fn shell(home: &Path, version: &str) -> Result<PathBuf, String> {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let name = shell_asset(os, arch).ok_or_else(|| format!("no wisp app for {os} {arch}"))?;
    let dir = home.join("desktop").join(version);
    let exe = shell_exe(&dir, os);
    if exe.is_file() {
        return Ok(exe);
    }
    let base = release_url(version, std::env::var("WISP_SHELL_URL").ok().as_deref());
    let client = reqwest::Client::new();
    let sums = String::from_utf8_lossy(&get(&client, &format!("{base}/SHA256SUMS.txt"), Duration::from_secs(10)).await?).into_owned();
    let want = sum_for(&sums, &name).ok_or_else(|| format!("wisp {version} has no {name}"))?;
    eprintln!("  ◌ getting the wisp app for {version} (once)…");
    let body = get(&client, &format!("{base}/{name}"), Duration::from_secs(120)).await?;
    if crate::services::hex(&Sha256::digest(&body)) != want {
        return Err(format!("{name} doesn't match SHA256SUMS.txt"));
    }
    let part = dir.with_extension(format!("part-{}", std::process::id()));
    let out = unpack(&part, &name, &body, os).and_then(|()| {
        std::fs::create_dir_all(dir.parent().unwrap_or(home)).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&dir);
        // another wisp may have got there first: its copy is as good
        std::fs::rename(&part, &dir).or_else(|e| if exe.is_file() { Ok(()) } else { Err(e.to_string()) })
    });
    let _ = std::fs::remove_dir_all(&part);
    out.map(|()| exe)
}

fn unpack(part: &Path, name: &str, body: &[u8], os: &str) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(part);
    std::fs::create_dir_all(part).map_err(|e| e.to_string())?;
    std::fs::write(part.join(name), body).map_err(|e| e.to_string())?;
    // from inside the folder: Git Bash's GNU tar takes C:\ for a host name
    let tar = if os == "windows" {
        std::env::var("SystemRoot")
            .map_or_else(|_| PathBuf::from(r"C:\Windows\System32\tar.exe"), |r| Path::new(&r).join(r"System32\tar.exe"))
    } else {
        PathBuf::from("tar")
    };
    let ok = std::process::Command::new(tar)
        .args(["-xzf", name])
        .current_dir(part)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("couldn't unpack {name}: {e}"))?;
    if !ok.success() {
        return Err(format!("couldn't unpack {name}"));
    }
    let _ = std::fs::remove_file(part.join(name));
    if !shell_exe(part, os).is_file() {
        return Err(format!("{name} holds no wisp app"));
    }
    Ok(())
}

/// Where the installers put the app (desktop/scripts/package.mjs, tauri.conf.json `productName: wisp`): the Mac
/// app in /Applications or ~/Applications; on Windows the NSIS installer's current-user folder (%LOCALAPPDATA%\wisp,
/// `installMode: currentUser`), else a per-machine one; on Linux the .deb's binary (its resources in /usr/lib/wisp,
/// which tells it from a CLI named wisp in /usr/bin) or the `AppImage` in ~/Applications. Each with the file that
/// proves it's the app, then the file to run.
pub fn installed_candidates(os: &str, user_home: &Path, env: &dyn Fn(&str) -> Option<String>) -> Vec<(PathBuf, PathBuf)> {
    let both = |p: PathBuf| (p.clone(), p);
    match os {
        "macos" => [PathBuf::from("/Applications"), user_home.join("Applications")]
            .into_iter()
            .map(|d| both(d.join("wisp.app/Contents/MacOS/wisp")))
            .collect(),
        "windows" => ["LOCALAPPDATA", "ProgramFiles"]
            .into_iter()
            .filter_map(|v| env(v).filter(|d| !d.is_empty()))
            .map(|d| both(Path::new(&d).join("wisp").join("wisp.exe")))
            .collect(),
        "linux" => {
            let apps = user_home.join("Applications");
            vec![
                (PathBuf::from("/usr/lib/wisp/resources/app/wisp"), PathBuf::from("/usr/bin/wisp")),
                both(apps.join("wisp.AppImage")),
                both(apps.join(format!("wisp-linux-{}.AppImage", if std::env::consts::ARCH == "aarch64" { "arm64" } else { "x64" }))),
            ]
        }
        _ => Vec::new(),
    }
}

/// The installed app to run, if there is one. `WISP_DESKTOP_APP` names it instead (`none`: there is none), never
/// this binary itself.
pub fn installed(user_home: &Path) -> Option<PathBuf> {
    // canonicalize: both sides alike, only compared with each other, never handed to git or a shell
    let me = std::env::current_exe().ok().and_then(|m| m.canonicalize().ok());
    // canonicalize: as above
    let not_me = |p: &PathBuf| me.as_ref().is_none_or(|m| p.canonicalize().ok().as_ref() != Some(m));
    if let Ok(over) = std::env::var("WISP_DESKTOP_APP") {
        return Some(PathBuf::from(over)).filter(|p| p.as_os_str() != "none" && p.is_file() && not_me(p));
    }
    installed_candidates(std::env::consts::OS, user_home, &|v| std::env::var(v).ok())
        .into_iter()
        .find(|(proof, exe)| proof.is_file() && exe.is_file() && not_me(exe))
        .map(|(_, exe)| exe)
}

/// Starts the window, detached, so it outlives the terminal. `server`: the binary it runs as its server when no wisp
/// answers on `port` (the shell from a release holds none; an installed app runs its own).
pub fn launch(exe: &Path, port: u16, server: Option<&Path>) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(exe);
    cmd.env("WISP_PORT", port.to_string());
    if let Some(bin) = server {
        cmd.env("WISP_BIN", bin);
    }
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    cmd.spawn().map(drop)
}

pub fn no_app_line(why: &str) -> String {
    format!("  ◌ no wisp app here ({why}): opening a browser tab. The app: {GET_APP}")
}

/// `wisp desktop` with no app: how to get one, and the tab that works now.
pub fn get_app_line(why: &str) -> String {
    format!("no wisp app here ({why}). Get it at {GET_APP}, or open wisp in a browser tab: wisp --browser")
}

/// Whether a wisp answers on `port`. Something listening that doesn't answer yet may be a wisp still starting (two
/// `wisp` at once): five seconds for it, then it's someone else's port, and nothing starts a second server on it.
pub async fn reach(port: u16) -> crate::client::Result<bool> {
    let c = Client::new(port);
    if c.ping().await {
        return Ok(true);
    }
    let taken = tokio::time::timeout(Duration::from_millis(500), tokio::net::TcpStream::connect(("127.0.0.1", port)))
        .await
        .is_ok_and(|r| r.is_ok());
    if !taken {
        return Ok(false);
    }
    if up_within(&c, Duration::from_secs(5)).await {
        return Ok(true);
    }
    Err(CliError::said(format!("port {port} is taken by something that isn't wisp (wisp --port N picks another)")))
}

async fn up_within(c: &Client, within: Duration) -> bool {
    let until = std::time::Instant::now() + within;
    while std::time::Instant::now() < until {
        if c.ping().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    false
}

/// `wisp` (and `npx wisp-agents`): the running wisp, else one started. Shown in the desktop app if there is one (it
/// attaches to the running wisp, or runs its own), else (or `--browser`) in a tab; `--no-open` shows nothing.
pub async fn open(home: &Path, user_home: &Path, port: u16, open: bool, browser: bool) -> crate::client::Result<()> {
    let running = reach(port).await?;
    if open && !browser && desktop(home, user_home, port, running, false).await? {
        return Ok(());
    }
    if !running {
        return crate::serve_here(home, port, None, open, None).await;
    }
    let url = Client::new(port).url;
    if open {
        crate::serve::open_url(&format!("{url}/"));
    }
    eprintln!("\n  ◌ wisp is open at {url}\n");
    Ok(())
}

/// The desktop app on `port`: the installed one, else this version's shell from its release. Ok(false): there is
/// none (said, and a tab opens instead); `strict` (`wisp desktop`) makes that an error that says how to get one. An
/// app that started but brought no server up in time is an error either way: a server started here now could race
/// the app's own for the port.
pub async fn desktop(home: &Path, user_home: &Path, port: u16, running: bool, strict: bool) -> crate::client::Result<bool> {
    let found = match installed(user_home) {
        Some(exe) => Ok((exe, None)),
        None => shell(home, crate::serve::VERSION).await.map(|exe| (exe, std::env::current_exe().ok())),
    };
    let found = found.and_then(|(exe, server)| launch(&exe, port, server.as_deref()).map_err(|e| format!("it didn't start: {e}")));
    match found {
        Err(why) if strict => Err(CliError::said(get_app_line(&why))),
        Err(why) => {
            eprintln!("{}", no_app_line(&why));
            Ok(false)
        }
        // the app runs its own server when none answers: open once it does
        Ok(()) if !running && !up_within(&Client::new(port), Duration::from_secs(30)).await => {
            Err(CliError::said("the wisp app didn't start its server in time: wisp --browser opens a tab instead"))
        }
        Ok(()) => {
            eprintln!("\n  ◌ wisp is open\n");
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_sums() {
        assert_eq!(shell_asset("macos", "aarch64").as_deref(), Some("wisp-shell-mac-arm64.tar.gz"));
        assert_eq!(shell_asset("linux", "arm"), None);
        assert_eq!(shell_exe(Path::new("/d"), "macos"), PathBuf::from("/d/wisp.app/Contents/MacOS/wisp"));
        assert_eq!(release_url("2.0.0", None), format!("{RELEASES}/v2.0.0"));
        assert_eq!(release_url("2.1.0-nightly.3", None), format!("{RELEASES}/nightly"));
        assert_eq!(release_url("2.0.0", Some("http://x/y/")), "http://x/y");
        let h = "a".repeat(64);
        assert_eq!(
            sum_for(&format!("{h}  wisp-shell-mac-arm64.tar.gz\n{}  other", "b".repeat(64)), "wisp-shell-mac-arm64.tar.gz"),
            Some(h.clone())
        );
        assert_eq!(sum_for(&format!("{h} *x.tar.gz"), "x.tar.gz"), Some(h));
        assert_eq!(sum_for("zz  x.tar.gz", "x.tar.gz"), None);
    }

    #[test]
    fn installed_apps_are_looked_for_where_the_installers_put_them() {
        let home = Path::new("/home/u");
        let env = |v: &str| match v {
            "LOCALAPPDATA" => Some(r"C:\Users\u\AppData\Local".to_owned()),
            "ProgramFiles" => Some(String::new()),
            _ => None,
        };
        let exes = |os| installed_candidates(os, home, &env).into_iter().map(|(_, e)| e).collect::<Vec<_>>();
        assert_eq!(
            exes("macos"),
            [PathBuf::from("/Applications/wisp.app/Contents/MacOS/wisp"), home.join("Applications/wisp.app/Contents/MacOS/wisp")]
        );
        assert_eq!(exes("windows"), [Path::new(r"C:\Users\u\AppData\Local").join("wisp").join("wisp.exe")], "an empty folder is no folder");
        let linux = installed_candidates("linux", home, &env);
        assert_eq!(
            linux[0],
            (PathBuf::from("/usr/lib/wisp/resources/app/wisp"), PathBuf::from("/usr/bin/wisp")),
            "a CLI in /usr/bin isn't the app"
        );
        assert!(linux[1..].iter().all(|(p, e)| p == e && p.starts_with(home.join("Applications"))));
        assert_eq!(exes("freebsd"), Vec::<PathBuf>::new());
    }
}
