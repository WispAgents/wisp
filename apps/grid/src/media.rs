//! Signed media links (ADR-0122, v1 grid/server.js `signMedia`/`signedMedia`): a picture, video or attachment the
//! phone app's page loads with a plain `<img>`/`<video>`, which can carry neither the device's bearer token nor a
//! cookie (the page is another origin, `https://localhost`). The phone asks `POST <address>/_grid/media {paths}` with
//! its device token; each answer is the path with `_ge=<expiry>&_gd=<device>&_gs=<HMAC>` added, the HMAC over the
//! address, the account, the device, the expiry and the exact path and query. Only `GET`/`HEAD` of an answer's file
//! (`/api/sessions/<id>/file?path=…`) or a prompt's upload (`/api/uploads/<id>`), on the wisp or a machine through it
//! (`/api/peer/<m>/…`); an hour; the device revoked, its links stop at once (the edge checks that, not this module).
//!
//! Pure: the clock and the device check are the caller's.
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::secrets::{b64u, safe_equal};

/// How long a link opens its file.
pub const TTL_S: i64 = 3600;
/// Paths per `POST /_grid/media` (the phone splits a longer list).
pub const MAX_PATHS: usize = 100;
/// Longest path signed.
pub const MAX_PATH_LEN: usize = 4096;

/// The key media links are signed with (`Secrets::media_key`, or a random one when the grid has no master key).
#[derive(Clone)]
pub struct MediaKey([u8; 32]);

impl std::fmt::Debug for MediaKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MediaKey(..)")
    }
}

/// A link that checked out: the path to pass on (the signature taken off) and the device that asked for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signed {
    pub path: String,
    pub device: String,
}

/// One path segment of an id or a machine's name: what the page puts there (`encodeURIComponent` of an id).
fn segment(s: &str) -> bool {
    !s.is_empty() && s.len() <= 200 && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'%'))
}

/// Whether `p` (path and query) is an answer's file or an upload, on the wisp or a machine through it.
pub fn is_media_path(p: &str) -> bool {
    if p.len() > MAX_PATH_LEN || p.contains('#') {
        return false;
    }
    let rest = match p.strip_prefix("/api/peer/") {
        Some(r) => {
            let Some((m, rest)) = r.split_once('/') else { return false };
            if !segment(m) {
                return false;
            }
            match rest.strip_prefix("api/") {
                Some(x) => x,
                None => return false,
            }
        }
        None => match p.strip_prefix("/api/") {
            Some(x) => x,
            None => return false,
        },
    };
    if let Some(id) = rest.strip_prefix("uploads/") {
        return segment(id);
    }
    let Some(r) = rest.strip_prefix("sessions/") else { return false };
    let Some((id, q)) = r.split_once("/file?path=") else { return false };
    segment(id) && !q.is_empty()
}

impl MediaKey {
    pub fn new(key: [u8; 32]) -> Self {
        Self(key)
    }

    fn mac(&self, label: &str, user: &str, device: &str, exp: i64, path: &str) -> String {
        let Ok(mut m) = <Hmac<Sha256> as Mac>::new_from_slice(&self.0) else { return String::new() };
        m.update(format!("media|{}|{user}|{device}|{exp}|{path}", label.to_ascii_lowercase()).as_bytes());
        b64u(m.finalize().into_bytes())
    }

    /// `path` signed for `device` of `user` at the address `label`, good until `now_s + TTL_S`.
    pub fn sign(&self, label: &str, user: &str, device: &str, now_s: i64, path: &str) -> String {
        let exp = now_s + TTL_S;
        let sep = if path.contains('?') { '&' } else { '?' };
        format!("{path}{sep}_ge={exp}&_gd={device}&_gs={}", self.mac(label, user, device, exp, path))
    }

    /// `pq` (path and query as asked) when it's a media link this key signed for `user` at `label`, still good at
    /// `now_s`. Whether its device is still allowed is the caller's to check.
    pub fn check(&self, label: &str, user: &str, now_s: i64, pq: &str) -> Option<Signed> {
        let at = pq.rfind("_ge=")?;
        let (path, sig) = (pq.get(..at.checked_sub(1)?)?, &pq[at..]);
        let sep = pq.as_bytes().get(at - 1)?;
        if !matches!(sep, b'?' | b'&') || (*sep == b'&') != path.contains('?') {
            return None;
        }
        let mut it = sig.split('&');
        let exp = it.next()?.strip_prefix("_ge=")?;
        let device = it.next()?.strip_prefix("_gd=")?;
        let mac = it.next()?.strip_prefix("_gs=")?;
        if it.next().is_some() || exp.is_empty() || exp.len() > 12 || !exp.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if device.is_empty() || device.len() > 32 || !device.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return None;
        }
        let exp: i64 = exp.parse().ok()?;
        // expired, or further out than this grid ever signs (a minute's clock skew allowed)
        if exp < now_s || exp > now_s + TTL_S + 60 || !is_media_path(path) {
            return None;
        }
        safe_equal(&self.mac(label, user, device, exp, path), mac).then(|| Signed { path: path.to_owned(), device: device.to_owned() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/api/sessions/s1/file?path=%2Ftmp%2Fa.png";
    const UP: &str = "/api/uploads/u1";

    fn key() -> MediaKey {
        MediaKey::new([7; 32])
    }

    #[test]
    fn media_paths_are_files_and_uploads_only() {
        for ok in [FILE, UP, "/api/peer/my-mac/api/uploads/u1", "/api/peer/a%20b/api/sessions/x.y/file?path=a"] {
            assert!(is_media_path(ok), "{ok}");
        }
        for no in [
            "/api/sessions/s1",
            "/api/sessions/s1/file",
            "/api/sessions/s1/file?path=",
            "/api/sessions/s1/file?path=a#x",
            "/api/uploads/",
            "/api/uploads/u1/x",
            "/api/uploads/../x",
            "/api/peer/m/uploads/u1",
            "/api/peer//api/uploads/u1",
            "/api/stuff/1/file",
            "/_grid/media",
            "/",
        ] {
            assert!(!is_media_path(no), "{no}");
        }
    }

    #[test]
    fn a_signed_link_opens_its_file_until_it_expires() {
        let k = key();
        let f = k.sign("mia", "u1", "d1", 1000, FILE);
        assert!(f.starts_with(&format!("{FILE}&_ge=4600&_gd=d1&_gs=")), "{f}");
        assert_eq!(k.check("mia", "u1", 1000, &f), Some(Signed { path: FILE.into(), device: "d1".into() }));
        assert_eq!(k.check("MIA", "u1", 4600, &f).map(|s| s.path), Some(FILE.into()), "the address is case-blind");
        assert_eq!(k.check("mia", "u1", 4601, &f), None, "expired");
        let u = k.sign("mia", "u1", "d1", 1000, UP);
        assert!(u.starts_with(&format!("{UP}?_ge=")), "{u}");
        assert_eq!(k.check("mia", "u1", 1000, &u).map(|s| s.path), Some(UP.into()));
    }

    #[test]
    fn a_link_opens_nothing_else() {
        let k = key();
        let f = k.sign("mia", "u1", "d1", 1000, FILE);
        assert_eq!(k.check("ned", "u1", 1000, &f), None, "another address");
        assert_eq!(k.check("mia", "u2", 1000, &f), None, "another account");
        assert_eq!(k.check("mia", "u1", 1000, &f.replace("a.png", "b.png")), None, "another file");
        assert_eq!(k.check("mia", "u1", 1000, &f.replace("_gd=d1", "_gd=d2")), None, "another device");
        assert_eq!(k.check("mia", "u1", 1000, &f.replace("_ge=4600", "_ge=4601")), None, "a later expiry");
        let forged = format!("{}x", &f[..f.len() - 1]);
        assert_eq!(k.check("mia", "u1", 1000, &forged), None, "a forged signature");
        assert_eq!(MediaKey::new([8; 32]).check("mia", "u1", 1000, &f), None, "another key");
        assert_eq!(k.check("mia", "u1", 1000, FILE), None, "no signature");
        assert_eq!(k.check("mia", "u1", 1000, &format!("{f}&x=1")), None, "anything after it");
        // a path that isn't media never checks out, even signed
        let other = k.sign("mia", "u1", "d1", 1000, "/api/sessions/s1");
        assert_eq!(k.check("mia", "u1", 1000, &other), None);
    }
}
