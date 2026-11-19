//! The desktop app's self-update (v1 `Updater.fetchApp`): what to check in a download and what to say when
//! a step fails. The host downloads, writes and renames; this decides.
//!
//! The release downloads (desktop/scripts/package.mjs): macOS `wisp-mac-<arch>.zip` holding `wisp.app` (unpacked
//! next to the running bundle, on its volume, checked, then renamed into its place once the running one is set aside
//! beside it as `wisp.app.old-<n>`; the running app keeps its open files), Windows
//! `wisp-windows-<arch>-setup.exe` (run silently once wisp has quit), Linux `wisp-linux-<arch>.AppImage` (copied
//! over `$APPIMAGE`: the running one stays mounted from the old file).

use sha2::{Digest, Sha256};

use crate::updater::{Asset, Updater};
use crate::version::newer;

/// The bundle in the macOS zip (`ditto --keepParent`), whatever this copy was renamed to.
pub const FRESH_BUNDLE: &str = "wisp.app";

/// The running bundle's name while it's set aside, next to it: `wisp.app.old-<n>`. Same folder, so a rename (never a
/// copy across volumes), and a name Finder doesn't open as an app.
pub fn aside_name(bundle_file: &str, n: u32) -> String {
    format!("{bundle_file}.old-{n}")
}

/// macOS runs an app opened where it was downloaded from a randomised read-only copy (App Translocation): it can't
/// replace itself there.
pub fn translocated(bundle: &str) -> bool {
    bundle.contains("/AppTranslocation/")
}

/// The version in a bundle's `Contents/Info.plist` (Tauri writes the app's version there as
/// `CFBundleShortVersionString`; v2's bundle has no package.json).
pub fn bundle_version(info_plist: &str) -> Option<String> {
    let key = "<key>CFBundleShortVersionString</key>";
    let rest = &info_plist[info_plist.find(key)? + key.len()..];
    let rest = rest.trim_start().strip_prefix("<string>")?;
    let v = rest[..rest.find("</string>")?].trim();
    (!v.is_empty()).then(|| v.to_owned())
}

/// The sha256 of `data`, lower-case hex.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(b & 15), 16).unwrap_or('0'));
    }
    out
}

impl Updater {
    /// The download answered `status` (not 2xx).
    pub fn download_failed(&self, asset: &Asset, status: u16) -> String {
        format!("Couldn't download {} (HTTP {status}). {}", asset.name, self.later())
    }

    /// The download didn't reach the release at all (offline, refused, timed out).
    pub fn cant_download(&self, asset: &Asset, why: &str) -> String {
        format!("Couldn't download {}: {why}. {}", asset.name, self.later())
    }

    /// The download couldn't be written to disk (full, no permission).
    pub fn cant_save(&self, asset: &Asset, why: &str) -> String {
        format!("Couldn't save {}: {why}. {}", asset.name, self.later())
    }

    /// macOS: the zip couldn't be unpacked.
    pub fn cant_unpack(&self, asset: &Asset, why: &str) -> String {
        format!("Couldn't unpack {}: {why}. {}", asset.name, self.later())
    }

    /// The release's sha256 for `asset`: a download with none is never installed (v1 installed it unchecked).
    pub fn wanted_digest<'a>(&self, asset: &'a Asset) -> Result<&'a str, String> {
        let want = asset.digest.as_deref().unwrap_or("").trim();
        let want = want.strip_prefix("sha256:").unwrap_or(want);
        if want.len() == 64 && want.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(want)
        } else {
            Err(format!("The release has no checksum for {}, so wisp won't install it. {}", asset.name, self.later()))
        }
    }

    /// Checks a download against the release's sha256; one without a sha256 is refused ([`Self::wanted_digest`]).
    pub fn verify_download(&self, asset: &Asset, data: &[u8]) -> Result<(), String> {
        let want = self.wanted_digest(asset)?;
        if !sha256_hex(data).eq_ignore_ascii_case(want) {
            return Err(format!("{} didn't match the release's checksum. {}", asset.name, self.later()));
        }
        Ok(())
    }

    /// macOS: the unpacked bundle's version ([`bundle_version`] of its Contents/Info.plist).
    pub fn check_bundle(&self, got: Option<&str>) -> Result<(), String> {
        if got.is_some_and(|g| newer(g, &self.current)) {
            Ok(())
        } else {
            Err(format!("The download isn't newer than wisp {}. {}", self.current, self.later()))
        }
    }

    /// macOS: the running bundle couldn't be moved aside.
    pub fn cant_replace_bundle(&self, folder: &str) -> String {
        format!("wisp can't replace itself in {folder} (no permission). {}", self.later())
    }

    /// macOS: this copy is translocated ([`translocated`]).
    pub fn cant_update_translocated(&self) -> String {
        format!(
            "macOS is running this wisp from a temporary copy because it was opened where it was downloaded. Move wisp.app to \
             Applications, open it from there, then update. {}",
            self.later()
        )
    }

    /// macOS: this copy is on a read-only disk (the disk image it came in).
    pub fn cant_update_read_only(&self, folder: &str) -> String {
        format!(
            "wisp is running from a read-only disk ({folder}), such as the disk image it came in. Drag wisp.app to Applications, \
             open it from there, then update. {}",
            self.later()
        )
    }

    /// macOS: the new bundle couldn't be put in place (the old one went back).
    pub fn cant_place_bundle(&self, why: &str) -> String {
        format!("Couldn't put the new wisp in place: {why}. {}", self.later())
    }

    /// Linux: the `AppImage` couldn't be replaced.
    pub fn cant_replace_image(&self, image: &str) -> String {
        format!("wisp can't replace {image} (no permission). {}", self.later())
    }
}
