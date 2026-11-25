//! Installing a harness without npm (`Harness::binary_package`): a computer with no Node still gets opencode's
//! free models. The host fetches `https://registry.npmjs.org/<package>/latest`, then its tarball; this reads the
//! registry's answer, checks the tarball against the registry's `sha512` integrity, and takes the one executable
//! out of the gzipped tar. Pure: the bytes come in, the file's bytes go out.

use std::io::Read;

use base64::Engine;
use sha2::{Digest, Sha512};

/// The registry, and the only place a tarball is fetched from (`WISP_NPM_REGISTRY` points tests at a fake one).
pub const REGISTRY: &str = "https://registry.npmjs.org";
/// A platform binary is ~200 MB unpacked (opencode's is 180–210 MB): a tar entry past this is refused.
pub const MAX_ENTRY: u64 = 512 * 1024 * 1024;

/// What can go wrong, in the words the setup strip shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryError {
    /// the registry's answer has no tarball, or one that isn't on the registry
    Registry(String),
    /// the download doesn't match the registry's sha512
    Integrity,
    /// not a gzipped tar, or the executable isn't in it
    Archive(String),
}

impl std::fmt::Display for BinaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registry(why) => write!(f, "the npm registry's answer is unusable: {why}"),
            Self::Integrity => f.write_str("the download doesn't match the registry's checksum"),
            Self::Archive(why) => write!(f, "the download isn't the package it should be: {why}"),
        }
    }
}

/// `<registry>/<package>/latest`: where its newest version's tarball and checksum are.
pub fn latest_url(registry: &str, package: &str) -> String {
    format!("{}/{package}/latest", registry.trim_end_matches('/'))
}

/// The tarball's URL and its `sha512-<base64>` integrity, from the registry's answer for one version. The tarball
/// must be on `registry` itself (an answer pointing elsewhere is refused) and the integrity a sha512.
pub fn dist(latest: &serde_json::Value, registry: &str) -> Result<(String, String), BinaryError> {
    let d = &latest["dist"];
    let tarball = d["tarball"].as_str().unwrap_or_default();
    if !tarball.starts_with(&format!("{}/", registry.trim_end_matches('/'))) {
        return Err(BinaryError::Registry(if tarball.is_empty() {
            "no tarball".into()
        } else {
            format!("tarball not on the registry ({tarball})")
        }));
    }
    let integrity = d["integrity"].as_str().unwrap_or_default();
    if !integrity.starts_with("sha512-") {
        return Err(BinaryError::Registry("no sha512 integrity".into()));
    }
    Ok((tarball.to_owned(), integrity.to_owned()))
}

/// Whether `bytes` are what `integrity` (`sha512-<base64>`, npm's Subresource Integrity form) says.
pub fn integrity_ok(integrity: &str, bytes: &[u8]) -> bool {
    let Some(want) = integrity.strip_prefix("sha512-") else {
        return false;
    };
    base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes)) == want
}

/// The file at `entry` (`package/bin/opencode`) in a gzipped tarball (ustar, with pax and GNU long-name headers
/// skipped over).
pub fn tar_gz_entry(tgz: &[u8], entry: &str) -> Result<Vec<u8>, BinaryError> {
    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(tgz).read_to_end(&mut tar).map_err(|e| BinaryError::Archive(format!("not gzip ({e})")))?;
    tar_entry(&tar, entry)
}

/// The file at `entry` in an uncompressed tar.
pub fn tar_entry(tar: &[u8], entry: &str) -> Result<Vec<u8>, BinaryError> {
    let mut at = 0usize;
    while let Some(h) = tar.get(at..at + 512) {
        if h.iter().all(|b| *b == 0) {
            break; // the end: two zero blocks
        }
        let size = octal(&h[124..136]).ok_or_else(|| BinaryError::Archive("a bad tar header".into()))?;
        if size > MAX_ENTRY {
            return Err(BinaryError::Archive("an entry too large".into()));
        }
        let size = usize::try_from(size).map_err(|_| BinaryError::Archive("an entry too large".into()))?;
        let body = at + 512;
        let data = tar.get(body..body + size).ok_or_else(|| BinaryError::Archive("cut short".into()))?;
        // a regular file ('0', or NUL in old tars), named in full by its prefix (ustar) and its name
        if matches!(h[156], b'0' | 0) && name_of(h) == entry {
            return Ok(data.to_vec());
        }
        at = body + size.div_ceil(512) * 512;
    }
    Err(BinaryError::Archive(format!("{entry} isn't in it")))
}

fn field(b: &[u8]) -> &str {
    let end = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    std::str::from_utf8(&b[..end]).unwrap_or_default()
}

fn name_of(h: &[u8]) -> String {
    let (name, prefix) = (field(&h[0..100]), if &h[257..262] == b"ustar" { field(&h[345..500]) } else { "" });
    if prefix.is_empty() { name.to_owned() } else { format!("{prefix}/{name}") }
}

fn octal(b: &[u8]) -> Option<u64> {
    let s = field(b).trim_matches(|c: char| c == ' ' || c == '\0');
    if s.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(s, 8).ok()
}
