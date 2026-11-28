//! A computer's webhook triggers from the internet (ADR-0147).
//!
//! - `POST <computer>--<handle>.<domain>/_hook/<token>`: a URL wisp on that computer made for one of its webhook
//!   triggers (`hooks.rs`). The host names the computer that runs it; the token is the only way in: unknown, revoked,
//!   or another computer's, it's the same 404, and a trigger's name alone reaches nothing. The call goes through the
//!   tunnel to `POST /api/triggers/<trigger>` there, whose wisp still checks the trigger's secret or HMAC header and
//!   takes it as `public` (an open route, no one's credential).
//! - `GET /api/hooks` → `{hooks: [{id, trigger, computer, hint, createdAt, lastUsedAt}]}`, this computer's;
//!   `POST /api/hooks {trigger}` → 201 `{…, url, token}`, the only time the URL is said (at most
//!   [`CREATES_PER_MINUTE`] a minute per user, [`crate::hooks::MAX_HOOKS`] live per computer);
//!   `DELETE /api/hooks/<id>` → `{ok: true}` or 404. Only a device reaches them (`grid:hooks`), and only for the
//!   computer it is: a URL always starts work where it was made.
//!
//! What goes through: the caller's own headers (its signature, `x-wisp-secret`, its delivery id), without the grid's
//! cookie, a grid bearer (`wgd_`, `wgk_`), any grid mark or `x-forwarded-*`, marked `x-wisp-route: hook` (which the
//! edge drops from everything else it forwards, and the computer takes on that one path only). 1 MB at most, the
//! trigger's own cap. Offline: held, sealed, and delivered in order once its tunnel is back (202 `{held: true}`,
//! `hook_held.rs`); a call that can't be held is 503 with `Retry-After`. Nothing a call said, nor a token, goes to a log.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use bytes::Bytes;
use http::{HeaderMap, Method};
use hyper::body::Incoming;
use serde_json::{Value, json};

use super::{Grid, Who, no};
use crate::error::{Result, bad};
use crate::hook_held::{Call, Held};
use crate::hooks::{CREATES_PER_MINUTE, Hook, trigger_name};
use crate::http::{Resp, json, json_with, parse_body, read_bytes};
use crate::rows::ComputerRow;
use crate::trust::{Credential, Reach, may};
use crate::tunnels::{AskOptions, HOP};

/// The trigger's own cap (wisp.automations `WEBHOOK_MAX`).
pub const HOOK_MAX: usize = 1024 * 1024;
/// What the grid marks a webhook call with (`x-wisp-route: hook`).
pub const HOOK_MARK: &str = "hook";
/// How long a computer may take to take a call: the trigger answers at once (its task starts behind the answer).
const HOOK_WAIT_MS: u64 = 30_000;

/// The token a webhook call carries, when it is one: `POST /_hook/<token>`, one path segment.
pub fn token_of<'a>(method: &Method, path: &'a str) -> Option<&'a str> {
    let t = path.strip_prefix("/_hook/")?;
    (*method == Method::POST && !t.is_empty() && !t.contains('/')).then_some(t)
}

/// `/api/triggers/<trigger>`, the name percent-encoded (it's one segment whatever it holds), and the call's query.
pub fn trigger_path(trigger: &str, query: Option<&str>) -> String {
    let mut p = String::from("/api/triggers/");
    for b in trigger.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            p.push(char::from(b));
        } else {
            p.push('%');
            p.push(char::from(b"0123456789ABCDEF"[usize::from(b >> 4)]));
            p.push(char::from(b"0123456789ABCDEF"[usize::from(b & 15)]));
        }
    }
    if let Some(q) = query.filter(|q| !q.is_empty()) {
        p.push('?');
        p.push_str(q);
    }
    p
}

/// The headers a webhook call takes to the computer: the caller's own, without the grid's credentials and marks.
pub fn hook_headers(h: &HeaderMap) -> Vec<(String, String)> {
    let grid_bearer = |v: &str| v.strip_prefix("Bearer ").is_some_and(|t| t.starts_with("wgd_") || t.starts_with("wgk_"));
    let mut out: Vec<(String, String)> = h
        .iter()
        .filter_map(|(k, v)| Some((k.as_str().to_ascii_lowercase(), v.to_str().ok()?.to_owned())))
        .filter(|(k, v)| {
            let mark = k.starts_with("x-wisp-") && !matches!(k.as_str(), "x-wisp-secret" | "x-wisp-delivery");
            !(HOP.contains(&k.as_str())
                || mark
                || k.starts_with("x-forwarded-")
                || matches!(k.as_str(), "cookie" | "origin" | "content-length")
                || (k == "authorization" && grid_bearer(v)))
        })
        .collect();
    out.push((crate::route::ROUTE_MARK.to_owned(), HOOK_MARK.to_owned()));
    out
}

fn not_found() -> Resp {
    json(404, &json!({ "error": "not found" }))
}

fn offline() -> Resp {
    json_with(503, &json!({ "error": "That computer is offline: try again later." }), &[("retry-after", "60".to_owned())])
}

impl Grid {
    /// `POST <computer>--<handle>/_hook/<token>`: through the tunnel to the trigger the token opens, its answer back.
    pub(super) async fn computer_hook(&self, req: http::Request<Incoming>, c: &ComputerRow, token: &str) -> Result<Resp> {
        if let Some(r) = no(&may(Credential::Webhook, Reach::Trigger)) {
            return Ok(r);
        }
        let Some(hook) = self.hooks.check(token)?.filter(|h| h.computer_id == c.id && h.user_id == c.user_id) else {
            return Ok(not_found());
        };
        let (head, body) = req.into_parts();
        let Ok(bytes) = read_bytes(body, HOOK_MAX).await else {
            return Ok(json(413, &json!({ "error": "the body is over 1 MB" })));
        };
        let path = trigger_path(&hook.trigger, head.uri.query());
        let headers = hook_headers(&head.headers);
        // offline, or calls already waiting for it (nothing passes them): held, in order, until its tunnel is back
        if !self.tunnels.online(&c.id) || self.held.any(&c.id)? {
            return self.hold(&hook, c, &head.headers, path, headers, &bytes);
        }
        let opts = AskOptions {
            method: "POST".into(),
            path: path.clone(),
            headers: headers.clone(),
            body: Some::<Bytes>(bytes.clone()),
            timeout_ms: Some(HOOK_WAIT_MS),
        };
        match self.tunnels.ask(&c.id, opts).await {
            Ok(a) => {
                let mut r = http::Response::new(crate::http::full(a.body));
                *r.status_mut() = http::StatusCode::from_u16(a.status).unwrap_or(http::StatusCode::BAD_GATEWAY);
                if let Some(ct) = a.headers.get("content-type") {
                    r.headers_mut().insert("content-type", ct.clone());
                }
                Ok(r)
            }
            // gone while it was asked: held like any call to an offline computer
            Err(e) if e.status() == 502 && !self.tunnels.online(&c.id) => self.hold(&hook, c, &head.headers, path, headers, &bytes),
            Err(e) => Ok(json(e.status(), &json!({ "error": e.to_string() }))),
        }
    }

    /// Holds a call for the computer (`hook_held.rs`): 202 `{held: true}`, or 503 when it can't be held (a cap, no
    /// master key), so the sender keeps it.
    fn hold(&self, hook: &Hook, c: &ComputerRow, h: &HeaderMap, path: String, headers: Vec<(String, String)>, body: &[u8]) -> Result<Resp> {
        let delivery =
            ["x-wisp-delivery", "x-github-delivery"].iter().find_map(|k| h.get(*k).and_then(|v| v.to_str().ok())).map(str::to_owned);
        let call = Call { path, headers, body: B64.encode(body) };
        match self.held.hold(&hook.user_id, &c.id, &hook.id, delivery.as_deref(), &call)? {
            Held::Yes => {
                tracing::info!("grid: held a call to trigger URL {} for {} ({} bytes)", hook.id, c.label, body.len());
                // online with calls waiting: they (and this one) go now
                if self.tunnels.online(&c.id) {
                    self.held.kick(&c.id);
                }
                Ok(json(202, &json!({ "held": true })))
            }
            Held::Full => Ok(offline()),
        }
    }

    /// `/api/hooks` and `/api/hooks/<id>`: the calling device's computer's trigger URLs.
    pub(super) fn hooks_route(&self, method: &Method, id: Option<&str>, h: &HeaderMap, bytes: &[u8], who: &Who) -> Result<Resp> {
        let cred = who.credential().ok_or_else(|| bad("Sign in first.", 401))?;
        if let Some(r) = no(&may(cred, Reach::GridHooks)) {
            return Ok(r);
        }
        let Who::Device(user, dev) = who else { return Err(bad("Sign in with wisp on a computer first.", 401)) };
        let Some(c) = self.accounts.computers(&user.id)?.into_iter().find(|c| c.device_id == dev.id) else {
            return Err(bad("Put this computer on the grid first (wisp computer on).", 409));
        };
        match (method.as_str(), id) {
            ("GET", None) => Ok(json(200, &json!({ "hooks": self.hooks.list(&c.id, &c.name)? }))),
            ("POST", None) => {
                self.limits.hit(&format!("hook|{}", user.id), CREATES_PER_MINUTE, self.clock.now_ms())?;
                let trigger = trigger_name(parse_body(h, bytes)?.get("trigger"))?;
                let (view, token) = self.hooks.create(&user.id, &c.id, &c.name, &trigger)?;
                tracing::info!("grid: {} made trigger URL {} on {}", user.handle, view.id, c.label);
                let mut v = serde_json::to_value(&view)?;
                v["url"] = Value::String(format!("{}/_hook/{token}", self.cfg.instance_url(&c.label)));
                v["token"] = Value::String(token);
                Ok(json(201, &v))
            }
            ("DELETE", Some(id)) => {
                if self.hooks.revoke(&user.id, &c.id, id)? {
                    self.held.drop_hook(id)?;
                    tracing::info!("grid: {} revoked trigger URL {id}", user.handle);
                    Ok(json(200, &json!({ "ok": true })))
                } else {
                    Ok(json(404, &json!({ "error": "No such URL." })))
                }
            }
            _ => Ok(not_found()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hm(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(http::HeaderName::from_bytes(k.as_bytes()).unwrap(), v.parse().unwrap());
        }
        h
    }

    #[test]
    fn only_a_post_to_a_token_is_a_webhook_call_and_a_triggers_name_is_none() {
        assert_eq!(token_of(&Method::POST, "/_hook/wgh_abc"), Some("wgh_abc"));
        assert_eq!(token_of(&Method::GET, "/_hook/wgh_abc"), None);
        assert_eq!(token_of(&Method::POST, "/_hook/"), None);
        assert_eq!(token_of(&Method::POST, "/_hook/a/b"), None);
        assert_eq!(token_of(&Method::POST, "/api/triggers/deploy"), None, "no public URL by a trigger's name");
    }

    #[test]
    fn the_trigger_is_one_encoded_segment_and_the_query_goes_along() {
        assert_eq!(trigger_path("deploy", None), "/api/triggers/deploy");
        assert_eq!(trigger_path("a?b#c%", Some("secret=x")), "/api/triggers/a%3Fb%23c%25?secret=x");
        assert_eq!(trigger_path("ci.main", Some("")), "/api/triggers/ci.main");
    }

    #[test]
    fn a_webhook_call_carries_no_grid_credential_or_mark_and_keeps_its_signature() {
        let h = hm(&[
            ("content-type", "application/json"),
            ("x-hub-signature-256", "sha256=ab"),
            ("x-github-delivery", "d1"),
            ("x-wisp-secret", "s3cret"),
            ("x-wisp-delivery", "k1"),
            ("cookie", "wisp_grid=abc"),
            ("origin", "https://evil.example"),
            ("x-wisp-tunnel", "forged"),
            ("x-wisp-route", "grid"),
            ("x-wisp-proxy-via", "key"),
            ("x-forwarded-for", "1.2.3.4"),
            ("connection", "keep-alive"),
        ]);
        let out = hook_headers(&h);
        let has = |k: &str| out.iter().any(|(n, _)| n == k);
        for k in ["content-type", "x-hub-signature-256", "x-github-delivery", "x-wisp-secret", "x-wisp-delivery"] {
            assert!(has(k), "{k} reaches the trigger");
        }
        for k in ["cookie", "origin", "x-wisp-tunnel", "x-wisp-proxy-via", "x-forwarded-for", "connection"] {
            assert!(!has(k), "{k} never reaches the computer");
        }
        let routes: Vec<_> = out.iter().filter(|(n, _)| n == "x-wisp-route").map(|(_, v)| v.as_str()).collect();
        assert_eq!(routes, ["hook"], "the caller's own x-wisp-route is replaced by the grid's mark");
    }

    #[test]
    fn a_grid_bearer_is_dropped_and_a_callers_own_bearer_stays() {
        for grid in ["Bearer wgd_device", "Bearer wgk_key"] {
            assert!(!hook_headers(&hm(&[("authorization", grid)])).iter().any(|(n, _)| n == "authorization"), "{grid}");
        }
        let out = hook_headers(&hm(&[("authorization", "Bearer s3cret")]));
        assert!(out.iter().any(|(n, v)| n == "authorization" && v == "Bearer s3cret"), "the trigger's secret as a bearer");
    }
}
