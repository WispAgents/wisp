//! A team's hub through the grid's own API (ADR-0147 §1, §3, §4, §6), for wisp on a computer (its device token) and the
//! grid's pages (a session). Never a box's, never a key's (`trust`: `GridTeams`).
//!
//! | route | who | what |
//! |---|---|---|
//! | `GET /api/teams/<t>/hub` | anyone in it | the hub: address, state, your role |
//! | `POST /api/teams/<t>/hub` | owner, admin | a hub for a team from before hubs |
//! | `POST /api/teams/<t>/hub/restart` | owner, admin | stop the box so it starts again with new secrets |
//! | `DELETE /api/teams/<t>` `{confirm}` | owner | the team, its hub, its box and home |
//! | `GET·POST /api/teams/<t>/invites`, `DELETE …/invites/<id>` | owner, admin | invite links |
//! | `POST /api/join {code}` | anyone signed in | take an invite |
//! | `GET·POST /api/teams/<t>/keys`, `DELETE …/keys/<id>` | owner, admin | deploy keys (a new one said once) |
//! | `GET /api/teams/<t>/secrets`, `PUT {key, value}`, `DELETE …/secrets/<key>` | owner, admin | the hub's env |
//! | `PUT·DELETE /api/teams/<t>/secrets/logins/<agent> {token}` | owner, admin | the hub's harness sign-ins |
//! | `GET /api/teams/<t>/deploys` | anyone in it | the hub's deploys |
use http::{HeaderMap, Method};
use serde_json::{Map, Value, json};

use super::{Grid, Who, no};
use crate::boxhost::BoxHost as _;
use crate::error::{Result, bad};
use crate::http::{Resp, json, parse_body};
use crate::hub_invites::{Ask, TTL_MS};
use crate::hub_keys::Rights;
use crate::hubs::Hub;
use crate::rows::{BoxUser, UserRow};
use crate::teams::Role;
use crate::trust::{Reach, may};

fn s<'a>(b: &'a Map<String, Value>, k: &str) -> &'a str {
    b.get(k).and_then(Value::as_str).unwrap_or_default()
}

impl Grid {
    /// The routes above; None for any other `/api/teams…` path (`teams_route` has those).
    pub(super) async fn hub_api(&self, method: &Method, parts: &[&str], h: &HeaderMap, bytes: &[u8], who: &Who) -> Option<Result<Resp>> {
        let hub_path = parts == ["api", "join"]
            || (*method == Method::DELETE && matches!(parts, ["api", "teams", _]))
            || matches!(parts, ["api", "teams", _, "hub" | "invites" | "keys" | "secrets" | "deploys", ..]);
        if !hub_path {
            return None;
        }
        Some(self.hub_api_in(method, parts, h, bytes, who).await)
    }

    async fn hub_api_in(&self, method: &Method, parts: &[&str], h: &HeaderMap, bytes: &[u8], who: &Who) -> Result<Resp> {
        let c = who.credential().ok_or_else(|| bad("Sign in first.", 401))?;
        if let Some(r) = no(&may(c, Reach::GridTeams)) {
            return Ok(r);
        }
        let me = who.user().ok_or_else(|| bad("Sign in first.", 401))?.clone();
        let b = parse_body(h, bytes).unwrap_or_default();
        let t = &self.teams;
        match (method.as_str(), parts) {
            ("POST", ["api", "join"]) => {
                let joined = t.join(&me, s(&b, "code"))?;
                let slug = joined["slug"].as_str().unwrap_or_default().to_owned();
                Ok(json(200, &json!({ "slug": slug, "role": joined["role"], "url": self.cfg.instance_url(&slug) })))
            }
            ("DELETE", ["api", "teams", slug]) => {
                if let Some(acct) = t.delete(&me, slug, s(&b, "confirm"))? {
                    self.delete_user(&acct).await?;
                }
                Ok(json(200, &json!({ "ok": true })))
            }
            ("GET", ["api", "teams", slug, "hub"]) => self.hub_view(&me, slug),
            ("POST", ["api", "teams", slug, "hub"]) => {
                t.ensure_hub(&me, slug)?;
                self.hub_view(&me, slug)
            }
            ("POST", ["api", "teams", slug, "hub", "restart"]) => {
                let hub = self.managed_hub(&me, slug)?;
                let was = self.boxes.stop(&hub.user_id, "restarted by the team").await?;
                Ok(json(200, &json!({ "stopped": was })))
            }
            (_, ["api", "teams", slug, "invites", rest @ ..]) => self.invites_api(method, &me, slug, rest, &b),
            (_, ["api", "teams", slug, "keys", rest @ ..]) => self.keys_api(method, &me, slug, rest, &b),
            (_, ["api", "teams", slug, "secrets", rest @ ..]) => self.secrets_api(method, &me, slug, rest, &b),
            ("GET", ["api", "teams", slug, "deploys"]) => {
                let hub = self.member_hub(&me, slug)?;
                Ok(json(200, &json!({ "deploys": t.deploys(&hub.team_id, 50)? })))
            }
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }

    fn member_hub(&self, me: &UserRow, slug: &str) -> Result<Hub> {
        if self.teams.role(&me.id, slug)?.is_none() {
            return Err(bad(format!("You aren't in {slug}."), 403));
        }
        self.teams.hub(slug)?.ok_or_else(|| bad(format!("{slug} has no hub yet: wisp team hub {slug}"), 409))
    }

    fn managed_hub(&self, me: &UserRow, slug: &str) -> Result<Hub> {
        if !self.teams.role(&me.id, slug)?.is_some_and(Role::manages) {
            return Err(bad(format!("Only an owner or admin of {slug} does that."), 403));
        }
        self.teams.hub(slug)?.ok_or_else(|| bad(format!("{slug} has no hub yet: wisp team hub {slug}"), 409))
    }

    fn hub_view(&self, me: &UserRow, slug: &str) -> Result<Resp> {
        let hub = self.member_hub(me, slug)?;
        let role = self.teams.role(&me.id, slug)?.map(Role::as_str);
        let state = self.boxes.state(&hub.user_id).as_str();
        let members = self.teams.members(slug)?;
        Ok(json(
            200,
            &json!({ "slug": hub.slug, "name": hub.name, "url": self.cfg.instance_url(&hub.slug), "state": state, "role": role, "members": members }),
        ))
    }

    fn invites_api(&self, method: &Method, me: &UserRow, slug: &str, rest: &[&str], b: &Map<String, Value>) -> Result<Resp> {
        let t = &self.teams;
        match (method.as_str(), rest) {
            ("GET", []) => Ok(json(200, &json!({ "invites": t.invites(me, slug)? }))),
            ("POST", []) => {
                let role = Role::parse(s(b, "role")).unwrap_or(Role::Member);
                let ttl_ms = b.get("days").and_then(Value::as_i64).map_or(TTL_MS, |d| d.saturating_mul(86_400_000));
                let uses = b.get("uses").and_then(Value::as_i64).unwrap_or(1);
                let (v, code) = t.invite(me, slug, Ask { role, ttl_ms, uses })?;
                let url = format!("{}/join/{code}", self.cfg.public_url);
                let mut out = serde_json::to_value(v).unwrap_or_default();
                out["code"] = json!(code);
                out["url"] = json!(url);
                Ok(json(201, &out))
            }
            ("DELETE", [id]) => {
                t.revoke_invite(me, slug, id)?;
                Ok(json(200, &json!({ "ok": true })))
            }
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }

    fn keys_api(&self, method: &Method, me: &UserRow, slug: &str, rest: &[&str], b: &Map<String, Value>) -> Result<Resp> {
        let t = &self.teams;
        match (method.as_str(), rest) {
            ("GET", []) => Ok(json(200, &json!({ "keys": t.keys(me, slug)?, "url": self.cfg.instance_url(slug) }))),
            ("POST", []) => {
                let name = crate::keys::key_name(b.get("name"))?;
                let rights = Rights::parse(s(b, "rights")).ok_or_else(|| bad("A key's rights are plan or deploy.", 400))?;
                let (v, key) = t.key_create(me, slug, &name, rights)?;
                let mut out = serde_json::to_value(v).unwrap_or_default();
                out["key"] = json!(key);
                out["url"] = json!(self.cfg.instance_url(slug));
                Ok(crate::http::json_with(201, &out, &[("cache-control", "no-store".into())]))
            }
            ("DELETE", [id]) => {
                t.key_revoke(me, slug, id)?;
                Ok(json(200, &json!({ "ok": true })))
            }
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }

    fn secrets_api(&self, method: &Method, me: &UserRow, slug: &str, rest: &[&str], b: &Map<String, Value>) -> Result<Resp> {
        let hub = self.managed_hub(me, slug)?;
        let sb = &self.sandboxes;
        let running = || self.boxes.state(&hub.user_id) == crate::boxhost::BoxState::Running;
        let answer = || json(200, &json!({ "env": sb.env_keys(&hub.user_id), "logins": sb.logins(&hub.user_id), "restart": running() }));
        match (method.as_str(), rest) {
            ("GET", []) => Ok(answer()),
            ("PUT" | "POST", []) => {
                sb.set_env(&hub.user_id, s(b, "key"), s(b, "value"))?;
                self.audit_secret(me, slug, "hubsecret.set", s(b, "key"))?;
                Ok(answer())
            }
            ("DELETE", [key]) => {
                sb.drop_env(&hub.user_id, key)?;
                self.audit_secret(me, slug, "hubsecret.drop", key)?;
                Ok(answer())
            }
            ("PUT", ["logins", agent]) => {
                sb.set_login(&hub.user_id, agent, s(b, "token"))?;
                self.audit_secret(me, slug, "hubsecret.set", agent)?;
                Ok(answer())
            }
            ("DELETE", ["logins", agent]) => {
                sb.drop_login(&hub.user_id, agent)?;
                self.audit_secret(me, slug, "hubsecret.drop", agent)?;
                Ok(answer())
            }
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }

    fn audit_secret(&self, me: &UserRow, slug: &str, action: &str, name: &str) -> Result<()> {
        let now = self.teams.clock.now_ms();
        self.teams.db.with(|c| crate::rows::find::audit(c, Some(&me.id), action, Some(&json!({ "team": slug, "name": name })), now))
    }

    /// The hub box, for tests and the pages: its `BoxUser`.
    pub fn hub_box(&self, slug: &str) -> Result<Option<BoxUser>> {
        Ok(self.teams.hub(slug)?.map(|h| BoxUser { id: h.user_id, handle: h.slug, sandbox: None, owner: None }))
    }
}
