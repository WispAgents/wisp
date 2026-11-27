//! The phone app's media links at a box's or a computer's own address (ADR-0122, `media.rs`):
//!
//! - `POST <address>/_grid/media {paths}`: the phone, with its device token, asks for links its `<img>`/`<video>`
//!   can load. A cookie, a key or a box's own secret gets none (a browser at the address needs no link).
//! - `GET`/`HEAD` of a signed link: checked last, after every other way in; let through as the address's owner,
//!   the signature taken off, so the wisp behind it never sees one.
use http::Method;
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use serde_json::{Value, json};

use super::{Grid, Who};
use crate::error::{Result, bad};
use crate::http::{Resp, json_with, read_bytes};
use crate::media::{MAX_PATH_LEN, MAX_PATHS, is_media_path};
use crate::rows::UserRow;

/// The path the phone posts to.
pub(super) const MEDIA: &str = "/_grid/media";

impl Grid {
    /// `POST /_grid/media {paths}` at `label`, owned by the asker (the caller checked).
    pub(super) async fn sign_media(&self, req: http::Request<Incoming>, who: &Who, label: &str) -> Result<Resp> {
        let Who::Device(u, d) = who else { return Err(bad("Signed links are for the phone app.", 403)) };
        let bytes = read_bytes(req.into_body().map_err(std::io::Error::other), (MAX_PATH_LEN + 8) * MAX_PATHS + 64).await?;
        let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let paths: Option<Vec<&str>> = body.get("paths").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect());
        let n = body.get("paths").and_then(Value::as_array).map_or(0, Vec::len);
        let Some(paths) = paths.filter(|p| !p.is_empty() && p.len() == n && p.len() <= MAX_PATHS && p.iter().all(|x| is_media_path(x)))
        else {
            return Err(bad(format!("Up to {MAX_PATHS} paths of a picture, video or upload."), 400));
        };
        let now_s = self.clock.now_ms() / 1000;
        let urls: Vec<String> = paths.iter().map(|p| self.media.sign(label, &u.id, &d.id, now_s, p)).collect();
        Ok(json_with(200, &json!({ "exp": now_s + crate::media::TTL_S, "urls": urls }), &[("cache-control", "no-store".into())]))
    }

    /// Whether `req` is a `GET`/`HEAD` of a media link signed for `owner` at `label` by a device still allowed; if
    /// so, its signature is taken off the URL.
    pub(super) fn signed_media<B>(&self, req: &mut http::Request<B>, label: &str, owner: &UserRow) -> bool {
        if req.method() != Method::GET && req.method() != Method::HEAD {
            return false;
        }
        let pq = req.uri().path_and_query().map_or("/", |p| p.as_str());
        let Some(s) = self.media.check(label, &owner.id, self.clock.now_ms() / 1000, pq) else { return false };
        if !self.accounts.live_device(&owner.id, &s.device).unwrap_or(false) {
            return false;
        }
        let Ok(uri) = s.path.parse() else { return false };
        *req.uri_mut() = uri;
        true
    }
}
