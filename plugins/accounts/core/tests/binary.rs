//! Installing a harness without npm: the registry's answer, the checksum and the tarball's one executable.
#![allow(clippy::unwrap_used)]

use std::io::Write;

use serde_json::json;
use wisp_accounts_core::binary::{BinaryError, REGISTRY, dist, integrity_ok, latest_url, tar_entry, tar_gz_entry};

/// One tar header + body, as npm packs them (ustar, `package/` prefix in the name).
fn entry(name: &str, kind: u8, body: &[u8]) -> Vec<u8> {
    let mut h = [0u8; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    h[100..107].copy_from_slice(b"0000755");
    let size = format!("{:011o}", body.len());
    h[124..135].copy_from_slice(size.as_bytes());
    h[156] = kind;
    h[257..263].copy_from_slice(b"ustar\0");
    let mut out = h.to_vec();
    out.extend_from_slice(body);
    out.resize(out.len().div_ceil(512) * 512, 0);
    out
}

fn tar(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut t: Vec<u8> = entries.concat();
    t.extend_from_slice(&[0u8; 1024]);
    t
}

fn gz(b: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

#[test]
fn the_registry_answer_gives_the_tarball_and_its_sha512() {
    assert_eq!(latest_url(REGISTRY, "opencode-darwin-arm64"), "https://registry.npmjs.org/opencode-darwin-arm64/latest");
    let ok = json!({ "dist": { "tarball": "https://registry.npmjs.org/opencode-darwin-arm64/-/opencode-darwin-arm64-1.19.6.tgz", "integrity": "sha512-abc" } });
    assert_eq!(dist(&ok, REGISTRY).unwrap().1, "sha512-abc");
    // a test's registry, with or without its trailing slash
    let local = json!({ "dist": { "tarball": "http://127.0.0.1:9/p/-/p-1.tgz", "integrity": "sha512-x" } });
    assert!(dist(&local, "http://127.0.0.1:9/").is_ok() && dist(&local, REGISTRY).is_err());
}

/// A tarball anywhere but the registry, or no sha512 to check it with, is never downloaded.
#[test]
fn a_tarball_off_the_registry_or_without_a_sha512_is_refused() {
    let off = json!({ "dist": { "tarball": "https://evil.example/opencode.tgz", "integrity": "sha512-abc" } });
    assert!(matches!(dist(&off, REGISTRY), Err(BinaryError::Registry(_))));
    let lookalike = json!({ "dist": { "tarball": "https://registry.npmjs.org.evil.example/x.tgz", "integrity": "sha512-abc" } });
    assert!(matches!(dist(&lookalike, REGISTRY), Err(BinaryError::Registry(_))));
    let sha1 = json!({ "dist": { "tarball": "https://registry.npmjs.org/x/-/x-1.tgz", "integrity": "sha1-abc" } });
    assert!(matches!(dist(&sha1, REGISTRY), Err(BinaryError::Registry(_))));
    assert!(matches!(dist(&json!({}), REGISTRY), Err(BinaryError::Registry(_))));
}

#[test]
fn the_download_must_match_the_registrys_sha512() {
    let abc = "sha512-3a81oZNherrMQXNJriBBMRLm+k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4qmslPpUyknw==";
    assert!(integrity_ok(abc, b"abc"));
    assert!(!integrity_ok(abc, b"abd"));
    assert!(!integrity_ok("sha1-qZk+NkcGgWq6PiVxeFDCbJzQ2J0=", b"abc"));
}

/// opencode's platform tarball: package.json, then the binary; pax headers and other files are stepped over.
#[test]
fn the_executable_comes_out_of_the_gzipped_tarball() {
    let bin = vec![7u8; 1300]; // not a multiple of 512: the next header is padded
    let t = tar(&[
        entry("package/package.json", b'0', br#"{"name":"opencode-darwin-arm64"}"#),
        entry("PaxHeader/opencode", b'x', b"30 mtime=1761436800.000000000\n"),
        entry("package/bin/opencode", b'0', &bin),
    ]);
    assert_eq!(tar_gz_entry(&gz(&t), "package/bin/opencode").unwrap(), bin);
    assert_eq!(tar_entry(&t, "package/package.json").unwrap(), br#"{"name":"opencode-darwin-arm64"}"#);
}

#[test]
fn a_tarball_without_it_or_cut_short_or_not_gzip_says_so() {
    let t = tar(&[entry("package/package.json", b'0', b"{}")]);
    assert!(matches!(tar_entry(&t, "package/bin/opencode.exe"), Err(BinaryError::Archive(w)) if w.contains("isn't in it")));
    let mut cut = entry("package/bin/opencode", b'0', &[1u8; 2000]);
    cut.truncate(900);
    assert!(matches!(tar_entry(&cut, "package/bin/opencode"), Err(BinaryError::Archive(w)) if w == "cut short"));
    assert!(matches!(tar_gz_entry(b"not gzip at all", "x"), Err(BinaryError::Archive(_))));
    // a directory of that name isn't the file
    let dir = tar(&[entry("package/bin/opencode", b'5', b"")]);
    assert!(tar_entry(&dir, "package/bin/opencode").is_err());
}
