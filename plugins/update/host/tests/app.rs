//! The desktop app's self-update (`wisp_update::app::fetch_app`, v1 server/update.js `fetchApp`): a release file
//! served by a stand-in release host (a fake `Net`, the plugin's only way out) is checked and staged for each OS, and
//! each failure says what went wrong and where to download it. On macOS the real `ditto` unpacks a real zip.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests may fail loudly

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use wisp_kernel::services::{Net, NetRequest, NetResponse};
use wisp_kernel::{BoxFuture, Result, WireError};
use wisp_update::app::{Unzip, fetch_app};
use wisp_update_core::app::sha256_hex;
use wisp_update_core::updater::DOWNLOAD_TIMEOUT_MS;
use wisp_update_core::{Asset, Env, Platform, Swap, Updater, detect};

const PLIST: &str = "<?xml version=\"1.0\"?>\n<plist version=\"1.0\"><dict>\n<key>CFBundleShortVersionString</key>\n<string>{v}</string>\n</dict></plist>\n";

/// The release host: one answer for every request, and what was asked.
struct Release {
    answer: std::result::Result<(u16, Vec<u8>), String>,
    asked: Mutex<Vec<NetRequest>>,
}
impl Release {
    fn serving(status: u16, body: &[u8]) -> Self {
        Self { answer: Ok((status, body.to_vec())), asked: Mutex::default() }
    }
    fn unreachable() -> Self {
        Self { answer: Err("connection refused".into()), asked: Mutex::default() }
    }
}
impl Net for Release {
    fn fetch(&self, req: NetRequest) -> BoxFuture<'static, Result<NetResponse>> {
        self.asked.lock().unwrap().push(req);
        let a = self.answer.clone();
        Box::pin(async move {
            let (status, body) = a.map_err(WireError::unavailable)?;
            Ok(NetResponse { status, headers: Vec::new(), body })
        })
    }
}

/// A fresh folder whose path has a space, as `/Users/Ann Lee` has.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wisp app update {name} {}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn updater(os: &str, exe: &str, appimage: Option<&str>) -> Updater {
    let p = Platform::new(os, "x64");
    let env = Env { appimage: appimage.map(str::to_owned), desktop: Some("tauri".into()), ..Env::default() };
    Updater::new(detect(exe, &p, appimage, true, None), p, "2.0.0", 4477, env)
}

fn asset(name: &str, body: &[u8]) -> Asset {
    Asset { name: name.into(), url: format!("https://dl.test/{name}"), digest: Some(format!("sha256:{}", sha256_hex(body))) }
}

/// An unzip that never runs (Windows and Linux have no zip).
fn no_unzip() -> Unzip {
    Arc::new(|_, _| Box::pin(async { Err("no zip here".to_owned()) }))
}

/// Unpacks as `ditto` would: `wisp.app` with this version in its Info.plist.
fn unzip_to(version: &'static str) -> Unzip {
    Arc::new(move |_zip: PathBuf, dir: PathBuf| {
        Box::pin(async move {
            let c = dir.join("wisp.app/Contents");
            std::fs::create_dir_all(c.join("Resources/app")).unwrap();
            std::fs::write(c.join("Info.plist"), PLIST.replace("{v}", version)).unwrap();
            std::fs::write(c.join("Resources/app/wisp"), format!("wisp {version}")).unwrap();
            Ok(())
        })
    })
}

/// The installed bundle at `<root>/Applications/wisp.app`, running 2.0.0.
fn installed_bundle(root: &Path) -> PathBuf {
    let b = root.join("Applications/wisp.app");
    std::fs::create_dir_all(b.join("Contents/Resources/app")).unwrap();
    std::fs::write(b.join("Contents/Info.plist"), PLIST.replace("{v}", "2.0.0")).unwrap();
    std::fs::write(b.join("Contents/Resources/app/wisp"), "wisp 2.0.0").unwrap();
    b
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}

#[tokio::test]
async fn the_download_asks_for_the_assets_url_with_the_long_timeout() {
    let d = scratch("asks");
    let body = b"MZ installer";
    let net = Release::serving(200, body);
    let a = asset("wisp-windows-x64-setup.exe", body);
    let u = updater("win32", "C:\\Users\\u\\AppData\\Local\\wisp\\resources\\app\\wisp.exe", None);
    fetch_app(&net, &u, &a, &d.join("dl"), &no_unzip()).await.unwrap();
    let asked = net.asked.lock().unwrap();
    assert_eq!(asked.len(), 1);
    assert_eq!((asked[0].method.as_str(), asked[0].url.as_str()), ("GET", "https://dl.test/wisp-windows-x64-setup.exe"));
    assert_eq!(asked[0].timeout_ms, DOWNLOAD_TIMEOUT_MS, "15 minutes: a release is tens of MB");
    let _ = std::fs::remove_dir_all(&d);
}

#[tokio::test]
async fn windows_keeps_the_installer_for_the_relaunch_to_run() {
    let d = scratch("windows");
    let body = b"MZ installer 2.1.0";
    let a = asset("wisp-windows-x64-setup.exe", body);
    let mut u = updater("win32", "C:\\Users\\u\\AppData\\Local\\wisp\\resources\\app\\wisp.exe", None);
    let dir = d.join("wisp-update-1");
    let swap = fetch_app(&Release::serving(200, body), &u, &a, &dir, &no_unzip()).await.unwrap();
    let installer = dir.join("wisp-windows-x64-setup.exe");
    assert_eq!(swap, Swap::Windows { installer: installer.to_string_lossy().into_owned() });
    assert_eq!(std::fs::read(&installer).unwrap(), body);
    u.swap = Some(swap);
    assert_eq!(u.relaunch_command(true), format!("\"{}\" /S /R", installer.display()));
    let _ = std::fs::remove_dir_all(&d);
}

#[tokio::test]
async fn a_release_host_that_says_no_or_isnt_there_leaves_nothing_behind() {
    let d = scratch("refused");
    let a = asset("wisp-windows-x64-setup.exe", b"x");
    let u = updater("win32", "C:\\w\\resources\\app\\wisp.exe", None);
    let dir = d.join("dl");
    let e = fetch_app(&Release::serving(404, b"Not Found"), &u, &a, &dir, &no_unzip()).await.unwrap_err();
    assert_eq!(e, u.download_failed(&a, 404));
    assert!(e.starts_with("Couldn't download wisp-windows-x64-setup.exe (HTTP 404). Download it from https://"), "{e}");
    let e = fetch_app(&Release::unreachable(), &u, &a, &dir, &no_unzip()).await.unwrap_err();
    assert!(e.starts_with("Couldn't download wisp-windows-x64-setup.exe: connection refused. Download it from"), "{e}");
    assert!(!dir.exists(), "nothing written");
    let _ = std::fs::remove_dir_all(&d);
}

#[tokio::test]
async fn a_download_that_doesnt_match_the_releases_checksum_is_never_saved() {
    let d = scratch("checksum");
    let a = asset("wisp-windows-x64-setup.exe", b"the real installer");
    let u = updater("win32", "C:\\w\\resources\\app\\wisp.exe", None);
    let dir = d.join("dl");
    let e = fetch_app(&Release::serving(200, b"something else"), &u, &a, &dir, &no_unzip()).await.unwrap_err();
    assert!(e.starts_with("wisp-windows-x64-setup.exe didn't match the release's checksum. Download it from"), "{e}");
    assert!(!dir.join("wisp-windows-x64-setup.exe").exists());
    let _ = std::fs::remove_dir_all(&d);
}

#[tokio::test]
async fn a_download_that_cant_be_saved_says_so() {
    let d = scratch("save");
    std::fs::write(d.join("file"), "").unwrap();
    let body = b"MZ";
    let a = asset("wisp-windows-x64-setup.exe", body);
    let u = updater("win32", "C:\\w\\resources\\app\\wisp.exe", None);
    // its folder would be inside a file
    let e = fetch_app(&Release::serving(200, body), &u, &a, &d.join("file/dl"), &no_unzip()).await.unwrap_err();
    assert!(e.starts_with("Couldn't save wisp-windows-x64-setup.exe: "), "{e}");
    // a folder someone made beforehand (in a shared /tmp) is never used
    std::fs::create_dir_all(d.join("made")).unwrap();
    let e = fetch_app(&Release::serving(200, body), &u, &a, &d.join("made"), &no_unzip()).await.unwrap_err();
    assert!(e.starts_with("Couldn't save wisp-windows-x64-setup.exe: "), "{e}");
    assert!(!d.join("made/wisp-windows-x64-setup.exe").exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// What's in the folder the bundle is in, by name, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

/// The old bundle is set aside next to itself (same folder, same volume: a rename, never into `$TMPDIR` on another
/// volume), the new one renamed into its place, and the old one removed; nothing else is left beside it.
#[tokio::test]
async fn macos_swaps_the_new_bundle_in_beside_the_old_one_and_removes_it() {
    let d = scratch("mac");
    let bundle = installed_bundle(&d);
    let apps = bundle.parent().unwrap().to_owned();
    let body = b"PK zip";
    let a = asset("wisp-mac-x64.zip", body);
    let mut u = updater("darwin", &bundle.join("Contents/Resources/app/wisp").to_string_lossy(), None);
    assert_eq!(u.install.root, bundle.to_string_lossy());
    let dir = d.join("wisp-update-1");
    let swap = fetch_app(&Release::serving(200, body), &u, &a, &dir, &unzip_to("2.1.0")).await.unwrap();
    let old = apps.join("wisp.app.old-1");
    assert_eq!(swap, Swap::Mac { open: bundle.to_string_lossy().into_owned(), remove: old.to_string_lossy().into_owned() });
    assert_eq!(read(&bundle.join("Contents/Resources/app/wisp")), "wisp 2.1.0", "the new one is in place");
    assert_eq!(names(&apps), ["wisp.app"], "the old one and the unpacked folder are gone");
    assert!(!dir.join("wisp-mac-x64.zip").exists(), "the zip is gone once unpacked");
    u.swap = Some(swap);
    u.env.ppid = 77;
    assert_eq!(
        u.relaunch_command(true),
        format!(
            "rm -rf '{}'; for i in $(seq 100); do kill -0 77 2>/dev/null || break; sleep 0.1; done; open '{}'",
            old.display(),
            bundle.display()
        ),
        "the shell (77) holds the single-instance lock until it's gone"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// A `wisp.app.old-1` already there (an update that never got to remove it, or the user's own) is never touched.
#[tokio::test]
async fn macos_never_reuses_a_name_beside_the_bundle() {
    let d = scratch("mac-taken");
    let bundle = installed_bundle(&d);
    let apps = bundle.parent().unwrap().to_owned();
    std::fs::create_dir_all(apps.join("wisp.app.old-1")).unwrap();
    std::fs::write(apps.join("wisp.app.old-1/keep"), "mine").unwrap();
    let body = b"PK zip";
    let a = asset("wisp-mac-x64.zip", body);
    let u = updater("darwin", &bundle.join("Contents/Resources/app/wisp").to_string_lossy(), None);
    let swap = fetch_app(&Release::serving(200, body), &u, &a, &d.join("dl"), &unzip_to("2.1.0")).await.unwrap();
    assert!(matches!(&swap, Swap::Mac { remove, .. } if remove.ends_with("wisp.app.old-2")), "{swap:?}");
    assert_eq!(read(&apps.join("wisp.app.old-1/keep")), "mine");
    assert_eq!(names(&apps), ["wisp.app", "wisp.app.old-1"]);
    let _ = std::fs::remove_dir_all(&d);
}

/// A copy macOS translocated (opened where it was downloaded) can't replace itself: it says what to do, before any
/// download.
#[tokio::test]
async fn macos_a_translocated_copy_says_move_it_to_applications() {
    let body = b"PK zip";
    let a = asset("wisp-mac-x64.zip", body);
    let u = updater("darwin", "/private/var/folders/x1/abc/T/AppTranslocation/0F1E-22/d/wisp.app/Contents/Resources/app/wisp", None);
    let net = Release::serving(200, body);
    let e = fetch_app(&net, &u, &a, &scratch("transloc").join("dl"), &unzip_to("2.1.0")).await.unwrap_err();
    assert_eq!(e, u.cant_update_translocated());
    assert!(net.asked.lock().unwrap().is_empty(), "nothing downloaded");
}

/// A release asset without a sha256 is never downloaded or installed.
#[tokio::test]
async fn a_download_with_no_checksum_is_refused_before_it_starts() {
    let body = b"MZ";
    let a = Asset { digest: None, ..asset("wisp-windows-x64-setup.exe", body) };
    let u = updater("win32", "C:\\w\\resources\\app\\wisp.exe", None);
    let net = Release::serving(200, body);
    let dir = scratch("nodigest").join("dl");
    let e = fetch_app(&net, &u, &a, &dir, &no_unzip()).await.unwrap_err();
    assert!(e.starts_with("The release has no checksum for wisp-windows-x64-setup.exe, so wisp won't install it."), "{e}");
    assert!(net.asked.lock().unwrap().is_empty(), "nothing downloaded");
    assert!(!dir.exists());
}

#[tokio::test]
async fn macos_never_swaps_in_a_bundle_that_isnt_newer() {
    let d = scratch("mac-older");
    let bundle = installed_bundle(&d);
    let body = b"PK zip";
    let a = asset("wisp-mac-x64.zip", body);
    let u = updater("darwin", &bundle.join("Contents/Resources/app/wisp").to_string_lossy(), None);
    for v in ["2.0.0", "1.9.0", ""] {
        let dir = d.join(format!("dl-{v}"));
        let e = fetch_app(&Release::serving(200, body), &u, &a, &dir, &unzip_to(if v.is_empty() { "?" } else { v })).await.unwrap_err();
        assert!(e.starts_with("The download isn't newer than wisp 2.0.0. Download it from"), "{v}: {e}");
        assert_eq!(read(&bundle.join("Contents/Resources/app/wisp")), "wisp 2.0.0", "{v}: untouched");
        assert_eq!(names(bundle.parent().unwrap()), ["wisp.app"], "{v}: the unpacked folder is gone");
    }
    let e = fetch_app(&Release::serving(200, body), &u, &a, &d.join("dl-bad"), &no_unzip()).await.unwrap_err();
    assert!(e.starts_with("Couldn't unpack wisp-mac-x64.zip: no zip here. Download it from"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[tokio::test]
async fn macos_says_so_when_it_cant_move_itself() {
    let d = scratch("mac-stuck");
    // a bundle path that isn't there to move (a read-only or translocated copy fails the same rename)
    let bundle = d.join("Gone/wisp.app");
    let body = b"PK zip";
    let a = asset("wisp-mac-x64.zip", body);
    let u = updater("darwin", &bundle.join("Contents/Resources/app/wisp").to_string_lossy(), None);
    let dir = d.join("dl");
    let e = fetch_app(&Release::serving(200, body), &u, &a, &dir, &unzip_to("2.1.0")).await.unwrap_err();
    assert_eq!(e, u.cant_replace_bundle(&d.join("Gone").to_string_lossy()));
    assert!(!bundle.exists(), "nothing was put where it wasn't");
    let _ = std::fs::remove_dir_all(&d);
}

#[cfg(unix)]
#[tokio::test]
async fn linux_replaces_the_appimage_in_place_and_executable() {
    use std::os::unix::fs::PermissionsExt as _;
    let d = scratch("linux");
    let image = d.join("Apps/wisp.AppImage");
    std::fs::create_dir_all(image.parent().unwrap()).unwrap();
    std::fs::write(&image, "image 2.0.0").unwrap();
    let body = b"image 2.1.0";
    let a = asset("wisp-linux-x64.AppImage", body);
    let img = image.to_string_lossy().into_owned();
    let u = updater("linux", "/tmp/.mount_wispAb/usr/lib/wisp/resources/app/wisp", Some(&img));
    let dir = d.join("dl");
    let swap = fetch_app(&Release::serving(200, body), &u, &a, &dir, &no_unzip()).await.unwrap();
    assert_eq!(swap, Swap::Linux { open: img.clone() });
    assert_eq!(read(&image), "image 2.1.0");
    assert_eq!(std::fs::metadata(&image).unwrap().permissions().mode() & 0o777, 0o755);
    assert!(!PathBuf::from(format!("{img}.new")).exists());
    assert!(!dir.join("wisp-linux-x64.AppImage").exists(), "the download is gone once copied");
    let _ = std::fs::remove_dir_all(&d);
}

#[cfg(unix)]
#[tokio::test]
async fn linux_says_so_when_it_cant_replace_the_appimage() {
    let d = scratch("linux-stuck");
    let img = d.join("Gone/wisp.AppImage").to_string_lossy().into_owned();
    let body = b"image 2.1.0";
    let a = asset("wisp-linux-x64.AppImage", body);
    let u = updater("linux", "/tmp/.mount_wispAb/usr/lib/wisp/resources/app/wisp", Some(&img));
    let e = fetch_app(&Release::serving(200, body), &u, &a, &d.join("dl"), &no_unzip()).await.unwrap_err();
    assert_eq!(e, u.cant_replace_image(&img));
    assert!(!PathBuf::from(format!("{img}.new")).exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// The real `ditto` on a zip made the way the release makes it (desktop/scripts/package.mjs: `ditto -c -k
/// --sequesterRsrc --keepParent`), through the real proc service.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn macos_unpacks_the_release_zip_with_ditto() {
    let d = scratch("ditto");
    let built = d.join("build/wisp.app/Contents");
    std::fs::create_dir_all(built.join("Resources/app")).unwrap();
    std::fs::write(built.join("Info.plist"), PLIST.replace("{v}", "2.1.0")).unwrap();
    std::fs::write(built.join("Resources/app/wisp"), "wisp 2.1.0").unwrap();
    let zip = d.join("wisp-mac-x64.zip");
    let ok = std::process::Command::new("ditto")
        .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
        .arg(d.join("build/wisp.app"))
        .arg(&zip)
        .status()
        .unwrap();
    assert!(ok.success());
    let body = std::fs::read(&zip).unwrap();
    let bundle = installed_bundle(&d);
    let a = asset("wisp-mac-x64.zip", &body);
    let u = updater("darwin", &bundle.join("Contents/Resources/app/wisp").to_string_lossy(), None);
    let unzip = wisp_update::app::ditto(wisp_proc::local());
    let dir = d.join("wisp update 1");
    let swap = fetch_app(&Release::serving(200, &body), &u, &a, &dir, &unzip).await.unwrap();
    assert!(matches!(swap, Swap::Mac { .. }));
    assert_eq!(read(&bundle.join("Contents/Resources/app/wisp")), "wisp 2.1.0");
    let _ = std::fs::remove_dir_all(&d);
}
