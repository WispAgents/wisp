//! The Teams pages and their forms (ADR-0148), signed in on the grid's own origin: `/teams`, `/teams/<team>`, and an
//! invite link's `/join/<code>`. Every form is the session's own POST with its CSRF token, answered with a redirect back
//! and a flash line (`pages_route`'s way); a deploy key or an invite link just made is said once, as Account's keys are
//! (`fresh_keys`, the `wisp_new_key` cookie naming it).
use http::HeaderMap;
use serde_json::{Map, Value};

use super::forms::NEW_KEY;
use super::{COOKIE, Grid, Who};
use crate::boxhost::BoxHost as _;
use crate::error::{Result, bad};
use crate::http::{CookieOptions, Resp, add_cookie, cookie, cookies, encode_component, html, parse_body, redirect};
use crate::hub_invites::Ask;
use crate::hub_keys::Rights;
use crate::pages::{self, DeployRow, HookRow, HubKeyRow, InviteRow, MemberRow, TeamPage, TeamRow, TeamsPage};
use crate::rows::UserRow;
use crate::teams::Role;

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned()
}

impl Grid {
    /// The routes above; None for any other path.
    pub(super) async fn teams_pages(
        &self,
        method: &str,
        parts: &[&str],
        q: &str,
        h: &HeaderMap,
        bytes: &[u8],
        who: &Who,
    ) -> Option<Result<Resp>> {
        let ours = matches!(parts, ["teams", ..]) || matches!((method, parts), ("GET", ["join", _]));
        if !ours {
            return None;
        }
        let Who::Session(u) = who else {
            let back = format!("/{}", parts.join("/"));
            return Some(Ok(redirect(&format!("/signin?return={}", encode_component(&back)), &[])));
        };
        Some(match (method, parts) {
            ("GET", ["teams"]) => self.teams_list(u, h, q),
            ("GET", ["teams", slug]) => self.team_view(u, slug, h, q),
            ("GET", ["join", code]) => Ok(match self.teams.join(u, code) {
                Ok(v) => {
                    self.back(&format!("/teams/{}", s(&v, "slug")), &format!("You’re in {} as {}.", s(&v, "slug"), s(&v, "role")), false)
                }
                Err(e) if e.status() < 500 => self.back("/teams", &e.to_string(), true),
                Err(e) => return Some(Err(e)),
            }),
            ("POST", ["teams", rest @ ..]) => self.team_form(u, rest, h, bytes).await,
            _ => return None,
        })
    }

    fn teams_list(&self, u: &UserRow, h: &HeaderMap, q: &str) -> Result<Resp> {
        let (flash, tone) = Self::flash_of(h, q);
        let hubs = self.teams.hubs_of(&u.id, |s| self.cfg.instance_url(s))?;
        let teams = self
            .teams
            .of(&u.id)?
            .iter()
            .map(|t| {
                let slug = s(t, "slug");
                let hub = hubs.iter().find(|h| h["slug"] == t["slug"]);
                let state =
                    self.teams.hub(&slug).ok().flatten().map(|h| self.boxes.state(&h.user_id).as_str().to_owned()).unwrap_or_default();
                TeamRow { url: hub.map(|h| s(h, "url")).unwrap_or_default(), name: s(t, "name"), role: s(t, "role"), slug, state }
            })
            .collect();
        let p = TeamsPage { user: super::pages_route::page_user(u), teams, csrf: Self::csrf_of(h), flash, tone };
        Ok(self.shown(h, html(200, pages::teams_page(&self.page, &p), &[])))
    }

    fn team_view(&self, u: &UserRow, slug: &str, h: &HeaderMap, q: &str) -> Result<Resp> {
        let Some(role) = self.teams.role(&u.id, slug)? else {
            return Ok(self.back("/teams", &format!("You aren’t in {slug}."), true));
        };
        let Some(hub) = self.teams.hub(slug)? else {
            // a team from before hubs: its owner or admin turns it on here
            if role.manages() {
                self.teams.ensure_hub(u, slug)?;
                return self.team_view(u, slug, h, q);
            }
            return Ok(self.back("/teams", &format!("{slug} has no hub yet: ask one of its owners."), true));
        };
        let manage = role.manages();
        let (flash, tone) = Self::flash_of(h, q);
        let fresh = cookies(h).get(NEW_KEY).and_then(|t| self.fresh_keys.take(t, &u.id, self.clock.now_ms()));
        let members = self.teams.members(slug)?.iter().map(|m| MemberRow { handle: s(m, "handle"), role: s(m, "role") }).collect();
        let invites = if manage { self.teams.invites(u, slug)? } else { Vec::new() };
        let keys = if manage { self.teams.keys(u, slug)? } else { Vec::new() };
        let sb = &self.sandboxes;
        let logins =
            sb.logins(&hub.user_id).as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v == true)).collect()).unwrap_or_default();
        let page = TeamPage {
            user: super::pages_route::page_user(u),
            url: self.cfg.instance_url(&hub.slug),
            state: self.boxes.state(&hub.user_id).as_str().to_owned(),
            role: role.as_str().to_owned(),
            members,
            invites: invites
                .into_iter()
                .map(|i| InviteRow { id: i.id, role: i.role, uses_left: i.uses_left, expires_at: i.expires_at, created_by: i.created_by })
                .collect(),
            keys: keys
                .into_iter()
                .map(|k| HubKeyRow {
                    id: k.id,
                    name: k.name,
                    hint: k.hint,
                    rights: k.rights.as_str().to_owned(),
                    created_by: k.created_by,
                    last_used_at: k.last_used_at,
                })
                .collect(),
            env: if manage { sb.env_keys(&hub.user_id) } else { Vec::new() },
            logins: if manage { logins } else { Vec::new() },
            sealed: sb.secrets.is_some(),
            deploys: self
                .teams
                .deploys(&hub.team_id, 20)?
                .into_iter()
                .map(|d| {
                    let n = |k: &str| d.summary[k].as_i64().unwrap_or_default();
                    DeployRow {
                        n: d.n,
                        by: d.by_name,
                        source: d.source,
                        state: d.state,
                        counts: (n("created"), n("updated"), n("deleted")),
                        error: d.error,
                        started_at: d.started_at,
                    }
                })
                .collect(),
            hooks: if manage {
                self.teams
                    .hooks_of(&hub)?
                    .iter()
                    .map(|h| HookRow {
                        id: s(h, "id"),
                        trigger: s(h, "trigger"),
                        hint: s(h, "hint"),
                        last_used_at: h["lastUsedAt"].as_i64(),
                    })
                    .collect()
            } else {
                Vec::new()
            },
            fresh,
            slug: hub.slug,
            name: hub.name,
            csrf: Self::csrf_of(h),
            flash,
            tone,
            now_ms: self.clock.now_ms(),
        };
        let mut r = self.shown(h, html(200, pages::team_page(&self.page, &page), &[]));
        if cookies(h).contains_key(NEW_KEY) {
            add_cookie(
                &mut r,
                &cookie(NEW_KEY, "", CookieOptions { secure: self.cfg.secure, max_age: Some(0), path: None, framed: false }),
            );
        }
        Ok(r)
    }

    /// A key or an invite link just made: back to the team's page, which says it once.
    fn said_once(&self, u: &UserRow, to: &str, what: &str, value: &str) -> Resp {
        let token = self.fresh_keys.put(&u.id, what, value, self.clock.now_ms());
        let mut r = redirect(to, &[]);
        *r.status_mut() = http::StatusCode::SEE_OTHER;
        let held = CookieOptions { secure: self.cfg.secure, max_age: Some(crate::keys_fresh::FRESH_MS / 1000), path: None, framed: false };
        add_cookie(&mut r, &cookie(NEW_KEY, &token, held));
        r
    }

    async fn team_form(&self, u: &UserRow, rest: &[&str], headers: &HeaderMap, bytes: &[u8]) -> Result<Resp> {
        let body = parse_body(headers, bytes)?;
        let field = |k: &str| body.get(k).and_then(Value::as_str).unwrap_or_default().trim().to_owned();
        if field("csrf") != Self::csrf(&cookies(headers).get(COOKIE).cloned().unwrap_or_default()) {
            return Err(bad("That form is stale. Go back and try again.", 403));
        }
        let to = |slug: &str, at: &str| format!("/teams/{slug}#{at}");
        let out: Result<Resp> = match rest {
            [] => self
                .teams
                .create(u, &field("slug"), &field("name"))
                .map(|v| self.back(&format!("/teams/{}", s(&v, "slug")), "Made. Its hub wakes the first time someone opens it.", false)),
            [slug, "members"] => self
                .teams
                .add(u, slug, &field("handle"), &field("role"))
                .map(|()| self.back(&to(slug, "people"), &format!("{} is {} now.", field("handle"), field("role")), false)),
            [slug, "members", handle, "remove"] => self.teams.remove(u, slug, handle).map(|()| {
                if *handle == u.handle {
                    self.back("/teams", &format!("You left {slug}."), false)
                } else {
                    self.back(&to(slug, "people"), &format!("{handle} is out of {slug}."), false)
                }
            }),
            [slug, "invites"] => self.form_invite(u, slug, &body),
            [slug, "invites", id, "revoke"] => self
                .teams
                .revoke_invite(u, slug, id)
                .map(|()| self.back(&to(slug, "invites"), "Revoked: that link adds nobody now.", false)),
            [slug, "keys"] => self.form_key(u, slug, &body),
            [slug, "keys", id, "revoke"] => self
                .teams
                .key_revoke(u, slug, id)
                .map(|()| self.back(&to(slug, "keys"), "Revoked: CI using it is refused from now on.", false)),
            [slug, "hooks"] => self.form_hook(u, slug, &body),
            [slug, "hooks", id, "revoke"] => self.form_hook_revoke(u, slug, id),
            [slug, "secrets", ..] => self.form_secret(u, slug, &rest[2..], &body),
            [slug, "delete"] => match self.teams.delete(u, slug, &field("confirm")) {
                Ok(acct) => {
                    if let Some(a) = acct {
                        self.delete_user(&a).await?;
                    }
                    Ok(self.back("/teams", &format!("{slug} is deleted, with its hub."), false))
                }
                Err(e) => Err(e),
            },
            _ => return Err(bad("Nothing here.", 404)),
        };
        match out {
            Ok(r) => Ok(r),
            Err(e) if e.status() < 500 => {
                let back = rest.first().map_or_else(|| "/teams".to_owned(), |s| format!("/teams/{s}"));
                Ok(self.back(if rest.is_empty() { "/teams#new" } else { &back }, &e.to_string(), true))
            }
            Err(e) => Err(e),
        }
    }

    fn form_hook(&self, u: &UserRow, slug: &str, b: &Map<String, Value>) -> Result<Resp> {
        if !self.teams.role(&u.id, slug)?.is_some_and(Role::manages) {
            return Err(bad(format!("Only an owner or admin of {slug} makes trigger URLs."), 403));
        }
        let hub = self.teams.hub(slug)?.ok_or_else(|| bad("No such hub.", 404))?;
        self.limits.hit(&format!("hook|{}", hub.user_id), crate::hooks::CREATES_PER_MINUTE, self.clock.now_ms())?;
        let trigger = crate::hooks::trigger_name(b.get("trigger"))?;
        let (_, token) = self.teams.hook_create(&hub, &u.handle, &trigger)?;
        let url = format!("{}/_hook/{token}", self.cfg.instance_url(&hub.slug));
        Ok(self.said_once(u, &format!("/teams/{slug}#hooks"), "url", &url))
    }

    fn form_hook_revoke(&self, u: &UserRow, slug: &str, id: &str) -> Result<Resp> {
        if !self.teams.role(&u.id, slug)?.is_some_and(Role::manages) {
            return Err(bad(format!("Only an owner or admin of {slug} revokes trigger URLs."), 403));
        }
        let hub = self.teams.hub(slug)?.ok_or_else(|| bad("No such hub.", 404))?;
        let gone = self.teams.hook_revoke(&hub, id)?;
        Ok(self.back(
            &format!("/teams/{slug}#hooks"),
            if gone { "Revoked: that URL opens nothing now." } else { "That URL was already revoked." },
            !gone,
        ))
    }

    fn form_invite(&self, u: &UserRow, slug: &str, b: &Map<String, Value>) -> Result<Resp> {
        let num = |k: &str, d: i64| b.get(k).and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_i64())).unwrap_or(d);
        let role = Role::parse(b.get("role").and_then(Value::as_str).unwrap_or("member")).unwrap_or(Role::Member);
        let ask = Ask { role, ttl_ms: num("days", 7).saturating_mul(86_400_000), uses: num("uses", 1) };
        let (_, code) = self.teams.invite(u, slug, ask)?;
        let url = format!("{}/join/{code}", self.cfg.public_url);
        Ok(self.said_once(u, &format!("/teams/{slug}#invites"), "invite", &url))
    }

    fn form_key(&self, u: &UserRow, slug: &str, b: &Map<String, Value>) -> Result<Resp> {
        self.limits.hit(&format!("hubkey|{}", u.id), crate::keys::CREATES_PER_MINUTE, self.clock.now_ms())?;
        let name = crate::keys::key_name(b.get("name"))?;
        let rights = Rights::parse(b.get("rights").and_then(Value::as_str).unwrap_or_default()).unwrap_or(Rights::Deploy);
        let (v, key) = self.teams.key_create(u, slug, &name, rights)?;
        Ok(self.said_once(u, &format!("/teams/{slug}#keys"), &v.name, &key))
    }

    fn form_secret(&self, u: &UserRow, slug: &str, rest: &[&str], b: &Map<String, Value>) -> Result<Resp> {
        if !self.teams.role(&u.id, slug)?.is_some_and(Role::manages) {
            return Err(bad(format!("Only an owner or admin of {slug} sets its secrets."), 403));
        }
        let hub = self.teams.hub(slug)?.ok_or_else(|| bad("No such hub.", 404))?;
        let f = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        let sb = &self.sandboxes;
        let msg = match rest {
            [] => {
                sb.set_env(&hub.user_id, &f("key"), &f("value"))?;
                format!("{} is set. A running hub gets it when it next starts.", f("key").trim())
            }
            [key, "remove"] => {
                sb.drop_env(&hub.user_id, key)?;
                format!("{key} is out.")
            }
            ["logins", agent] => {
                sb.set_login(&hub.user_id, agent, &f("token"))?;
                "Signed in. The hub and its sandboxes run with it from their next start.".to_owned()
            }
            ["logins", agent, "remove"] => {
                sb.drop_login(&hub.user_id, agent)?;
                "Signed out.".to_owned()
            }
            _ => return Err(bad("Nothing here.", 404)),
        };
        let now = self.clock.now_ms();
        self.teams
            .db
            .with(|c| crate::rows::find::audit(c, Some(&u.id), "hubsecret.change", Some(&serde_json::json!({ "team": slug })), now))?;
        Ok(self.back(&format!("/teams/{slug}#secrets"), &msg, false))
    }
}
