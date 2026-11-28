//! A team hub's webhook trigger URLs at its own address (ADR-0148 with ADR-0147's tokens):
//!
//! - `POST <team>.<domain>/_hook/<token>`: before any sign-in. A token unknown, revoked or another hub's is the same
//!   404; over 1 MB is 413. The call goes to the hub's wisp as `POST /api/triggers/<trigger>` with the caller's own
//!   headers (its signature, `x-wisp-secret`, its delivery id) and none of the grid's (`hook_headers`), marked
//!   `x-wisp-proxy-via: hook`, so the hub's wisp takes it as `public` on that open route and the trigger checks its own
//!   secret. The hub's box wakes for it (nothing is held, unlike an offline computer's).
//! - `/api/hooks` from a hub's own box (its secret): that hub's URLs, so its Automations page makes and revokes them.
//! - `/api/teams/<team>/hooks` (`edge_hub_api.rs`): the same for its owners and admins, from wisp or the grid's pages.
use http::{HeaderMap, Method};
use hyper::body::Incoming;
use serde_json::{Map, Value, json};

use super::hook::{HOOK_MAX, hook_headers, trigger_path};
use super::{Grid, Via, no};
use crate::error::{Result, bad};
use crate::http::{Resp, full, json, json_with, read_bytes};
use crate::hubs::Hub;
use crate::rows::{BoxUser, UserRow};
use crate::trust::{Credential, Reach, may};

impl Grid {
    /// `POST /_hook/<token>` at a hub's address: the hub's trigger, as no one.
    pub(super) async fn hub_hook(&self, req: http::Request<Incoming>, acct: &UserRow, hub: &Hub, token: &str) -> Result<Resp> {
        if let Some(r) = no(&may(Credential::Webhook, Reach::Trigger)) {
            return Ok(r);
        }
        let Some(hook) = self.teams.hook_check(hub, token)? else {
            return Ok(json(404, &json!({ "error": "not found" })));
        };
        let (head, body) = req.into_parts();
        let Ok(bytes) = read_bytes(body, HOOK_MAX).await else {
            return Ok(json(413, &json!({ "error": "the body is over 1 MB" })));
        };
        let mut b = http::Request::builder().method(Method::POST).uri(trigger_path(&hook.trigger, head.uri.query()));
        for (k, v) in hook_headers(&head.headers) {
            b = b.header(k, v);
        }
        let mut call = b.body(full(bytes)).map_err(|e| bad(format!("That call can't be passed on: {e}"), 400))?;
        if let Some(p) = head.extensions.get::<super::common::Peer>() {
            call.extensions_mut().insert(super::common::Peer(p.0));
        }
        // no one: the hub's wisp sees `public` on the trigger's open route, whose own secret or signature decides
        call.extensions_mut().insert(Via("hook"));
        tracing::info!("grid: hub {} trigger URL {} called", hub.slug, hook.id);
        self.proxy(call, &BoxUser::user(acct), &hub.slug).await
    }

    /// A hub's own box at `/api/hooks`: its hub's trigger URLs (list, make, revoke), as a computer's device does its own.
    pub(super) fn hub_box_hooks(&self, method: &Method, id: Option<&str>, h: &HeaderMap, bytes: &[u8], hub: &Hub) -> Result<Resp> {
        let b = crate::http::parse_body(h, bytes).unwrap_or_default();
        self.hub_hooks_api(method, id, &b, hub, &hub.slug)
    }

    /// The hub's URLs: `GET` the list, `POST {trigger}` one more (said once), `DELETE <id>` one.
    pub(super) fn hub_hooks_api(&self, method: &Method, id: Option<&str>, b: &Map<String, Value>, hub: &Hub, by: &str) -> Result<Resp> {
        match (method.as_str(), id) {
            ("GET", None) => Ok(json(200, &json!({ "hooks": self.teams.hooks_of(hub)? }))),
            ("POST", None) => {
                self.limits.hit(&format!("hook|{}", hub.user_id), crate::hooks::CREATES_PER_MINUTE, self.clock.now_ms())?;
                let trigger = crate::hooks::trigger_name(b.get("trigger"))?;
                let (mut view, token) = self.teams.hook_create(hub, by, &trigger)?;
                view["url"] = json!(format!("{}/_hook/{token}", self.cfg.instance_url(&hub.slug)));
                view["token"] = json!(token);
                Ok(json_with(201, &view, &[("cache-control", "no-store".into())]))
            }
            ("DELETE", Some(id)) => {
                if self.teams.hook_revoke(hub, id)? {
                    Ok(json(200, &json!({ "ok": true })))
                } else {
                    Err(bad("No such trigger URL.", 404))
                }
            }
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }
}
