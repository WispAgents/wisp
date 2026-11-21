//! The desktop app's self-update (v1 server/update.js `fetchApp`): download the release's file for this OS, check
//! it against the release's sha256, and get it ready to replace this copy. What's checked and every message are
//! `wisp_update_core::app`'s; this does the IO. The download goes through the net service; hashing, writing and
//! renaming run on a blocking thread, never on the runtime's workers; the zip is unpacked by `ditto` through the
//! proc service. The swap it returns is finished by the relaunch (`Updater::relaunch_command`) once wisp has quit:
//!
//! - macOS: `ditto -xk` unpacks `wisp.app` into a hidden folder beside the running bundle (its volume: every move
//!   after is a rename), its Info.plist must say a newer version, then the running bundle is renamed aside next to
//!   itself (`wisp.app.old-<n>`; it keeps its open files), the new one renamed into its place and the old one removed;
//!   on any failure the old one goes back. A translocated copy or one on a read-only disk (the DMG) says so. The
//!   relaunch opens the new one.
//! - Windows: the installer waits next to the download; the relaunch runs it silently (`/S /R`) once wisp is gone.
//! - Linux: the `AppImage` is copied next to `$APPIMAGE`, made executable and renamed over it (the running one stays
//!   mounted from the old file); the relaunch starts it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use wisp_kernel::services::{Net, NetRequest};
use wisp_kernel::{BoxFuture, Ctx};
use wisp_update_core::Updater;
use wisp_update_core::app::{FRESH_BUNDLE, aside_name, bundle_version, translocated};
use wisp_update_core::updater::{Asset, DOWNLOAD_TIMEOUT_MS, Swap};

use crate::{St, lock};

/// Unpacks the zip (first) into the folder (second).
pub type Unzip = Arc<dyn Fn(PathBuf, PathBuf) -> BoxFuture<'static, Result<(), String>> + Send + Sync>;

/// `/usr/bin/ditto -xk`, as v1 did and as the release zip was made (`ditto -c -k --keepParent`): it keeps the
/// bundle's executable bits, symlinks and signature.
pub fn ditto(proc: wisp_proc::Proc) -> Unzip {
    Arc::new(move |zip: PathBuf, dir: PathBuf| {
        let proc = proc.clone();
        Box::pin(async move {
            let args = ["-xk".to_owned(), zip.to_string_lossy().into_owned(), dir.to_string_lossy().into_owned()];
            let r = proc.exec("/usr/bin/ditto", &args, wisp_proc::ExecOpts::default()).await.map_err(|e| e.to_string())?;
            if r.exit.success() { Ok(()) } else { Err(r.stderr_text().trim().to_owned()) }
        })
    })
}

/// Runs `f` on a blocking thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// Downloads `asset` into `dir` and stages it for `up`'s OS. Err: the message the row shows. `dir` must not exist yet
/// (its parent must): one made beforehand in a shared temp folder could have its file swapped after the checksum.
pub async fn fetch_app(net: &dyn Net, up: &Updater, asset: &Asset, dir: &Path, unzip: &Unzip) -> Result<Swap, String> {
    // what can be refused before tens of MB come down: no checksum to check it with, a copy that can't replace itself
    up.wanted_digest(asset)?;
    if up.platform.os == "darwin" && translocated(&up.install.root) {
        return Err(up.cant_update_translocated());
    }
    let req = NetRequest {
        method: "GET".into(),
        url: asset.url.clone(),
        headers: vec![("accept".into(), "application/octet-stream".into()), ("user-agent".into(), "wisp".into())],
        body: Vec::new(),
        timeout_ms: DOWNLOAD_TIMEOUT_MS,
    };
    let r = net.fetch(req).await.map_err(|e| up.cant_download(asset, &e.message))?;
    if !(200..300).contains(&r.status) {
        return Err(up.download_failed(asset, r.status));
    }
    let file = dir.join(&asset.name);
    let (u, a, folder, to, data) = (up.clone(), asset.clone(), dir.to_owned(), file.clone(), r.body);
    // a release is tens of MB: hashing it would hold a worker for a while
    blocking(move || {
        u.verify_download(&a, &data)?;
        std::fs::create_dir(&folder).and_then(|()| std::fs::write(&to, &data)).map_err(|e| u.cant_save(&a, &e.to_string()))
    })
    .await?;
    match up.platform.os.as_str() {
        "darwin" => mac(up, asset, dir, file, unzip).await,
        "win32" => Ok(Swap::Windows { installer: file.to_string_lossy().into_owned() }),
        _ => linux(up, file).await,
    }
}

async fn mac(up: &Updater, asset: &Asset, dir: &Path, zip: PathBuf, unzip: &Unzip) -> Result<Swap, String> {
    let bundle = PathBuf::from(&up.install.root);
    let (Some(parent), Some(name)) = (bundle.parent().map(Path::to_owned), bundle.file_name().map(|n| n.to_string_lossy().into_owned()))
    else {
        return Err(up.cant_replace_bundle(&up.install.root));
    };
    // beside the bundle, on its volume: the moves below are renames, never copies across volumes
    let stage = parent.join(format!(".{}", dir.file_name().map_or_else(|| "wisp-update".into(), |n| n.to_string_lossy())));
    let (u, st, folder) = (up.clone(), stage.clone(), parent.clone());
    blocking(move || std::fs::create_dir(&st).map_err(|e| moved_error(&u, &e, &folder))).await?;
    let staged = stage_bundle(up, asset, &stage, zip, unzip).await;
    let fresh = match staged {
        Ok(fresh) => fresh,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&stage).await;
            return Err(e);
        }
    };
    let (u, b) = (up.clone(), bundle.clone());
    let old = blocking(move || {
        let free = (1..=1000).map(|n| folder_of(&b).join(aside_name(&name, n))).find(|p| !p.exists());
        let r = match &free {
            Some(old) => swap_in(&u, &b, &fresh, old),
            None => Err(u.cant_replace_bundle(&folder_of(&b).to_string_lossy())),
        };
        let _ = std::fs::remove_dir_all(&stage);
        r?;
        let old = free.unwrap_or_default();
        // the running app keeps its open files; nothing else needs the old bundle now
        let _ = std::fs::remove_dir_all(&old);
        Ok(old)
    })
    .await?;
    Ok(Swap::Mac { open: bundle.to_string_lossy().into_owned(), remove: old.to_string_lossy().into_owned() })
}

fn folder_of(p: &Path) -> PathBuf {
    p.parent().map(Path::to_owned).unwrap_or_default()
}

/// Unpacks the zip into `stage` and checks the bundle in it is newer: its path.
async fn stage_bundle(up: &Updater, asset: &Asset, stage: &Path, zip: PathBuf, unzip: &Unzip) -> Result<PathBuf, String> {
    let r = unzip(zip.clone(), stage.to_owned()).await;
    let _ = tokio::fs::remove_file(&zip).await; // unpacked or not, the zip is only taking room
    r.map_err(|e| up.cant_unpack(asset, &e))?;
    let fresh = stage.join(FRESH_BUNDLE);
    let plist = tokio::fs::read_to_string(fresh.join("Contents").join("Info.plist")).await.ok();
    up.check_bundle(plist.as_deref().and_then(bundle_version).as_deref())?;
    Ok(fresh)
}

/// The running bundle aside to `old`, the new one into its place; on failure the old one goes back.
fn swap_in(u: &Updater, bundle: &Path, fresh: &Path, old: &Path) -> Result<(), String> {
    std::fs::rename(bundle, old).map_err(|e| moved_error(u, &e, &folder_of(bundle)))?;
    if let Err(e) = std::fs::rename(fresh, bundle) {
        // the old one goes back, so this copy still opens; if even that fails, say where it is
        let why = match std::fs::rename(old, bundle) {
            Ok(()) => e.to_string(),
            Err(_) => format!("{e} (this copy of wisp is at {})", old.display()),
        };
        return Err(u.cant_place_bundle(&why));
    }
    Ok(())
}

/// Why the bundle's folder refused a change, in words: a read-only disk (the DMG it came on) or no permission.
fn moved_error(u: &Updater, e: &std::io::Error, folder: &Path) -> String {
    let folder = folder.to_string_lossy();
    if e.kind() == std::io::ErrorKind::ReadOnlyFilesystem { u.cant_update_read_only(&folder) } else { u.cant_replace_bundle(&folder) }
}

async fn linux(up: &Updater, file: PathBuf) -> Result<Swap, String> {
    let image = up.env.appimage.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| up.install.root.clone());
    let (u, img) = (up.clone(), image.clone());
    blocking(move || {
        let next = PathBuf::from(format!("{img}.new"));
        let r = std::fs::copy(&file, &next).and_then(|_| executable(&next)).and_then(|()| std::fs::rename(&next, &img));
        let _ = std::fs::remove_file(&file);
        if r.is_err() {
            let _ = std::fs::remove_file(&next);
            return Err(u.cant_replace_image(&img));
        }
        Ok(())
    })
    .await?;
    Ok(Swap::Linux { open: image })
}

#[cfg(unix)]
fn executable(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))
}
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // the Unix one can fail; no mode bits to set here
fn executable(_: &Path) -> std::io::Result<()> {
    Ok(())
}

/// This process's parent: the desktop shell, which the macOS and Linux relaunch waits out (it holds the single-instance lock).
#[cfg(unix)]
pub(crate) fn parent_pid() -> u32 {
    std::os::unix::process::parent_id()
}
/// Windows relaunches through the installer, which never waits on the shell.
#[cfg(not(unix))]
pub(crate) fn parent_pid() -> u32 {
    0
}

/// The desktop app: download and stage `asset` ([`fetch_app`]), in a fresh folder of its own under the temp
/// folder; the swap is what the relaunch finishes.
pub(crate) async fn download(ctx: &Ctx, st: &St, asset: &Asset) -> std::result::Result<(), String> {
    let u = lock(&st.up).clone();
    let net = wisp_kernel::services::net(ctx).map_err(|e| u.cant_download(asset, &e.message))?;
    let proc = ctx.service::<wisp_proc::Proc>().map_err(|e| u.cant_download(asset, &e.message))?;
    let now = ctx.clock().map_or(0, |c| c.now_ms());
    let dir = std::env::temp_dir().join(format!("wisp-update-{}-{now}", std::process::id()));
    let r = fetch_app(&*net, &u, asset, &dir, &ditto((*proc).clone())).await;
    // only the Windows installer is still needed from it (the macOS bundle was set aside beside itself)
    if !matches!(r, Ok(Swap::Windows { .. })) {
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
    lock(&st.up).swap = Some(r?);
    Ok(())
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use wisp_update_core::{Env, Platform, Updater, detect};

    use super::{moved_error, swap_in};
    use crate::{St, lock};

    fn mac(bundle: &Path) -> Updater {
        let p = Platform::new("darwin", "arm64");
        let exe = bundle.join("Contents/Resources/app/wisp");
        Updater::new(detect(&exe.to_string_lossy(), &p, None, true, None), p, "2.0.0", 1, Env::default())
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wisp swap {name} {}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The new bundle can't go in (here: it isn't there): the running one goes back where it was, under its name.
    #[test]
    fn a_new_bundle_that_cant_go_in_puts_the_old_one_back() {
        let d = scratch("back");
        let bundle = d.join("wisp.app");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents/x"), "2.0.0").unwrap();
        let old = d.join("wisp.app.old-1");
        let e = swap_in(&mac(&bundle), &bundle, &d.join("missing/wisp.app"), &old).unwrap_err();
        assert!(e.starts_with("Couldn't put the new wisp in place: "), "{e}");
        assert!(!e.contains("this copy of wisp is at"), "it went back: {e}");
        assert_eq!(std::fs::read_to_string(bundle.join("Contents/x")).unwrap(), "2.0.0");
        assert!(!old.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A bundle on a read-only disk (opened from the DMG) says so, not "no permission".
    #[test]
    fn a_read_only_disk_says_so() {
        let u = mac(Path::new("/Volumes/wisp/wisp.app"));
        let ro = std::io::Error::from(std::io::ErrorKind::ReadOnlyFilesystem);
        assert_eq!(moved_error(&u, &ro, Path::new("/Volumes/wisp")), u.cant_update_read_only("/Volumes/wisp"));
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(moved_error(&u, &denied, Path::new("/Applications")), u.cant_replace_bundle("/Applications"));
        // EROFS as the OS reports it
        let erofs = std::io::Error::from_raw_os_error(30);
        assert_eq!(moved_error(&u, &erofs, Path::new("/Volumes/wisp")), u.cant_update_read_only("/Volumes/wisp"));
    }

    /// The macOS relaunch waits for the desktop shell (this process's parent) to go; v2 passed 0, and `kill -0 0`
    /// (this process group) always succeeded, so the new app opened only after the 10 s wait ran out.
    #[test]
    fn the_relaunch_waits_for_the_real_parent() {
        let st = St::new();
        assert_eq!(lock(&st.up).env.ppid, std::os::unix::process::parent_id());
        assert_ne!(lock(&st.up).env.ppid, 0);
    }
}
