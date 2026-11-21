//! The phone relay's routes (`push.rs`, ADR-0065 §3):
//!
//! - `POST /api/push/register` → `{handle}`: a device token only (the phone's own sign-in, `grid:device`), at most
//!   [`REGISTERS_PER_MINUTE`] a minute per device;
//! - `POST /api/push/send` → `{ok}` or `{error}`: no credential, the handle is the capability (`push.rs`).
//!
//! Neither logs a body: the token and the handle reach the phone.

use http::{HeaderMap, Method};
use serde_json::json;

use super::{Grid, Who, no};
use crate::error::{Result, bad};
use crate::http::{Resp, json, parse_body};
use crate::trust::{Credential, Reach, may};

pub const REGISTERS_PER_MINUTE: usize = 10;

impl Grid {
    pub(super) async fn push_route(&self, method: &Method, which: &str, h: &HeaderMap, bytes: &[u8], who: &Who) -> Result<Resp> {
        match (method.as_str(), which) {
            ("POST", "register") => {
                let Who::Device(user, device) = who else { return Err(bad("Sign in with the wisp app first.", 401)) };
                if let Some(r) = no(&may(Credential::Device, Reach::GridDevice)) {
                    return Ok(r);
                }
                self.limits.hit(&format!("pushreg|{}", device.id), REGISTERS_PER_MINUTE, self.clock.now_ms())?;
                let b = parse_body(h, bytes)?;
                Ok(json(200, &self.push.register(&user.id, &device.id, &b)?))
            }
            ("POST", "send") => {
                let b = parse_body(h, bytes)?;
                let (status, body) = self.push.send(&b).await;
                Ok(json(status, &body))
            }
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }
}
