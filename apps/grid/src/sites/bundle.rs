//! An app's bundle as wisp sends it (v1 grid/sites.js, `shared/bundle`): what's packed, what's private and never
//! served (an env file, a source map, the app's database seed), and unpacking it safely into a folder.
use std::path::Path;

use serde_json::{Value, json};

use super::{HEADER_MAX, MAX_FILES};

fn segments(rel: &str) -> Vec<String> {
    rel.to_lowercase().split(['/', '\\']).map(str::to_owned).collect()
}

fn env_name(n: &str) -> bool {
    n == ".env" || n.starts_with(".env.")
}

/// A file that never goes in an app, saved or published: what holds secrets, and source maps.
#[allow(clippy::case_sensitive_file_extension_comparisons)] // `name` is lowercased already
pub fn private_file(rel: &str) -> bool {
    let mut dirs = segments(rel);
    let name = dirs.pop().unwrap_or_default();
    let ends = |exts: &[&str]| exts.iter().any(|e| name.ends_with(e));
    let secret = env_name(&name)
        || name.ends_with(".env")
        || name == ".envrc"
        || name.trim_start_matches('.').starts_with("credentials")
            && (name.trim_start_matches('.') == "credentials"
                || ends(&[".json", ".yml", ".yaml", ".toml", ".ini", ".txt", ".csv", ".xml"]))
        || name.starts_with("client_secret") && name.ends_with(".json")
        || (name.starts_with("service-account") || name.starts_with("service_account") || name.starts_with("serviceaccount"))
            && name.ends_with(".json")
        || ["secret.json", "secrets.json", "secret.yml", "secrets.yml", "secret.yaml", "secrets.yaml", "secret.toml", "secrets.toml"]
            .contains(&name.as_str())
        || [".npmrc", ".netrc", ".pgpass", ".pypirc", ".htpasswd", ".git-credentials", ".dockercfg"].contains(&name.as_str())
        || ends(&[".pem", ".key", ".p12", ".pfx", ".jks", ".keystore", ".ppk", ".map"])
        || ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"].contains(&name.as_str());
    secret || dirs.iter().any(|d| env_name(d))
}

/// May a visitor open this file: nothing private and no dotfile, but `.well-known` at the top.
pub fn public_file(rel: &str) -> bool {
    !private_file(rel) && segments(rel).iter().enumerate().all(|(i, s)| !s.starts_with('.') || (i == 0 && s == ".well-known"))
}

/// A file's path from a bundle, or None when it would land outside the folder.
pub fn bundle_path(p: &str) -> Option<String> {
    if p.is_empty() || p.len() > 400 || p.starts_with('/') || p.chars().any(|c| c.is_control() || c == '\\') {
        return None;
    }
    p.split('/').all(|x| !x.is_empty() && x != "." && x != "..").then(|| p.to_owned())
}

/// Writes a bundle into `dir` (made), leaving out what `skip` names. → (files, bytes) written.
pub fn unpack(buf: &[u8], dir: &Path, skip: &dyn Fn(&str) -> bool) -> std::result::Result<(usize, u64), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let (mut at, mut files, mut size) = (0usize, 0usize, 0u64);
    while at < buf.len() {
        let len4: [u8; 4] = buf.get(at..at + 4).and_then(|b| b.try_into().ok()).ok_or("The bundle ends early.")?;
        let n = u32::from_be_bytes(len4) as usize;
        at += 4;
        if n > HEADER_MAX || at + n > buf.len() {
            return Err("A file header in the bundle is broken.".into());
        }
        let head: Value = serde_json::from_slice(&buf[at..at + n]).map_err(|_| "A file header in the bundle is broken.")?;
        at += n;
        let raw = head.get("path").and_then(Value::as_str).unwrap_or_default();
        let len = head.get("size").and_then(Value::as_u64).and_then(|l| usize::try_from(l).ok());
        let (Some(rel), Some(len)) = (bundle_path(raw), len) else {
            return Err(format!("The bundle has a bad file: {}", raw.chars().take(80).collect::<String>()));
        };
        if at + len > buf.len() {
            return Err(format!("The bundle has a bad file: {}", rel.chars().take(80).collect::<String>()));
        }
        if skip(&rel) {
            at += len;
            continue;
        }
        files += 1;
        if files > MAX_FILES {
            return Err(format!("More than {MAX_FILES} files."));
        }
        let file = rel.split('/').fold(dir.to_path_buf(), |p, s| p.join(s));
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // create_new: a bundle naming one file twice can't overwrite it
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&file).map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut f, &buf[at..at + len]).map_err(|e| e.to_string())?;
        at += len;
        size += len as u64;
    }
    Ok((files, size))
}

/// One file as a bundle (for tests and wisp's own publish).
pub fn pack(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (p, data) in files {
        let head = json!({ "path": p, "size": data.len() }).to_string();
        out.extend_from_slice(&u32::try_from(head.len()).unwrap_or(u32::MAX).to_be_bytes());
        out.extend_from_slice(head.as_bytes());
        out.extend_from_slice(data);
    }
    out
}
