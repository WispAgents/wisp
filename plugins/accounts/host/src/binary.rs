//! `accounts.install` without npm (`Harness::binary_package`): the harness's own platform package from the npm
//! registry, checked against the registry's sha512 and unpacked into wisp's tools folder, where its detect looks.
//! The registry answer and the archive are read by `wisp_accounts_core::binary`; this does the IO (the kernel's net
//! service, a blocking thread for the hash, the gunzip and the write).

use std::path::{Path, PathBuf};

use wisp_accounts_core::binary as b;
use wisp_kernel::Ctx;
use wisp_kernel::services::NetRequest;
use wisp_runtime_api::harness::BinaryPackage;

/// The registry's small JSON answer; then the tarball (~70 MB for opencode), on a slow line too.
const META_TIMEOUT_MS: u64 = 30_000;
const TARBALL_TIMEOUT_MS: u64 = 600_000;

/// This computer's OS and CPU in Node's names (what `binary_package` takes): darwin | win32 | linux, arm64 | x64.
pub fn platform() -> (&'static str, &'static str) {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        o => o,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        a => a,
    };
    (os, arch)
}

/// Downloads `p` and puts its executable at `<tools>/<p.dest>`. Err: what the setup strip shows.
pub async fn install(ctx: &Ctx, tools: &Path, p: &BinaryPackage) -> Result<PathBuf, String> {
    let net = wisp_kernel::services::net(ctx).map_err(|e| e.message)?;
    let get = |url: String, timeout_ms: u64| NetRequest {
        method: "GET".into(),
        url,
        headers: vec![("user-agent".into(), "wisp".into())],
        body: Vec::new(),
        timeout_ms,
    };
    let registry = std::env::var("WISP_NPM_REGISTRY").ok().filter(|r| !r.is_empty()).unwrap_or_else(|| b::REGISTRY.into());
    let meta = net
        .fetch(get(b::latest_url(&registry, &p.package), META_TIMEOUT_MS))
        .await
        .map_err(|e| format!("can’t reach the npm registry: {}", e.message))?;
    if meta.status != 200 {
        return Err(format!("the npm registry answered {} for {}", meta.status, p.package));
    }
    let latest: serde_json::Value = meta.json().map_err(|e| e.message)?;
    let (tarball, integrity) = b::dist(&latest, &registry).map_err(|e| e.to_string())?;
    let tgz = net.fetch(get(tarball, TARBALL_TIMEOUT_MS)).await.map_err(|e| format!("the download stopped: {}", e.message))?;
    if tgz.status != 200 {
        return Err(format!("the download answered {}", tgz.status));
    }
    let (entry, dest) = (p.entry.clone(), tools.join(&p.dest));
    // tens of MB to hash and unpack: off the runtime's threads
    tokio::task::spawn_blocking(move || -> Result<PathBuf, String> {
        if !b::integrity_ok(&integrity, &tgz.body) {
            return Err(b::BinaryError::Integrity.to_string());
        }
        let exe = b::tar_gz_entry(&tgz.body, &entry).map_err(|e| e.to_string())?;
        write_executable(&dest, &exe).map_err(|e| format!("can’t write {}: {e}", dest.display()))?;
        Ok(dest)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Next to it first, then renamed over it: a half-written file is never the one detect finds, and a running copy
/// (Windows locks it) fails here instead of being cut in half.
fn write_executable(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = dest.parent().ok_or_else(|| std::io::Error::other("no folder"))?;
    std::fs::create_dir_all(dir)?;
    let part = dest.with_extension("part");
    std::fs::write(&part, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&part, dest).inspect_err(|_| {
        let _ = std::fs::remove_file(&part);
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{platform, write_executable};

    #[test]
    fn the_executable_lands_whole_and_runnable_where_detect_looks() {
        let d = std::env::temp_dir().join(format!("wisp-binary-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let dest = d.join("opencode/bin/opencode");
        write_executable(&dest, b"#!/bin/sh\necho 1.0.0\n").unwrap();
        write_executable(&dest, b"#!/bin/sh\necho 1.0.1\n").unwrap(); // a reinstall replaces it
        assert_eq!(std::fs::read(&dest).unwrap(), b"#!/bin/sh\necho 1.0.1\n");
        assert!(!dest.with_extension("part").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777, 0o755);
        }
        let _ = std::fs::remove_dir_all(&d);
        let (os, arch) = platform();
        #[cfg(target_os = "macos")]
        assert_eq!(os, "darwin", "Node's name, as binary_package takes it");
        #[cfg(target_arch = "aarch64")]
        assert_eq!(arch, "arm64");
        let _ = (os, arch);
    }
}
