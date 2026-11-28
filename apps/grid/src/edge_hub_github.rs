//! A team hub's GitHub App (ADR-0148 §8e) at the grid's own address.
//!
//! | route | who | what |
//! |---|---|---|
//! | `GET /api/teams/<t>/github` | anyone in it | whether the grid has an App, and the team's installation |
//! | `POST /api/teams/<t>/github` | owner, admin | where to install it: `{url}`, GitHub's page with a state for this team |
//! | `DELETE /api/teams/<t>/github` | owner, admin | untie the installation (the App stays on GitHub until removed there) |
//! | `GET /github/setup?installation_id&code&state` | the signed-in person who started it | GitHub's return: checked theirs, tied |
//! | `POST /github/webhook` | GitHub, signed | to the team's hub (waking it): `pull_request` to wisp.github, `issues` to wisp.issues; an uninstall unties |
//! | `GET /api/github/token` | a hub's own box | a token for its team's repos (an hour) |
use http::Method;
use hyper::body::Incoming;
use serde_json::{Value, json};

use super::{Grid, Via, Who};
use crate::error::{Result, bad};
use crate::github_app::Started;
use crate::http::{Resp, full, json, read_bytes};
use crate::rows::{BoxUser, UserRow};
use crate::teams::Role;

/// GitHub's own cap on a webhook's body.
const WEBHOOK_MAX: usize = 25 * 1024 * 1024;
/// The events a hub turns into triggers' and the hub route each goes to (its plugin makes the poll's own events of
/// it); anything else isn't worth a wake.
pub const FORWARDED: &[(&str, &str)] = &[("pull_request", "/api/github/webhook"), ("issues", "/api/issues/webhook")];

fn q<'a>(query: &'a str, k: &str) -> Option<&'a str> {
    query.split('&').find_map(|p| p.split_once('=').filter(|(a, _)| *a == k).map(|(_, v)| v))
}

impl Grid {
    fn app(&self) -> Result<&std::sync::Arc<crate::github_app::GitHubApp>> {
        self.github.as_ref().ok_or_else(|| bad("This grid has no GitHub App: give the hub a GH_TOKEN secret instead.", 409))
    }

    /// `/api/teams/<t>/github`.
    pub(super) fn hub_github_api(&self, method: &Method, me: &UserRow, slug: &str) -> Result<Resp> {
        match method.as_str() {
            "GET" => {
                let hub = self.member_hub(me, slug)?;
                let connected = self.teams.github_of(&hub.team_id)?.map(|c| c.view());
                let app = self.github.as_ref().map(|a| json!({ "slug": a.slug, "url": format!("{}/apps/{}", a.web, a.slug) }));
                Ok(json(200, &json!({ "app": app, "connected": connected })))
            }
            "POST" => {
                let hub = self.managed_hub(me, slug)?;
                let url = self.app()?.install_url(Started { team_id: hub.team_id, slug: hub.slug, user_id: me.id.clone() });
                Ok(json(200, &json!({ "url": url })))
            }
            "DELETE" => {
                let hub = self.managed_hub(me, slug)?;
                let was = self.teams.github_disconnect(&hub.team_id)?;
                if let (Some(id), Some(app)) = (was, self.github.as_ref()) {
                    app.forget(id);
                }
                tracing::info!("grid: {} untied {slug}'s GitHub", me.handle);
                Ok(json(200, &json!({ "disconnected": was.is_some() })))
            }
            _ => Ok(json(405, &json!({ "error": "GET, POST or DELETE" }))),
        }
    }

    /// `GET /github/setup`: GitHub's return after an install, for the person who began it from their team's page.
    pub(super) async fn github_setup(&self, query: &str, who: &Who) -> Result<Resp> {
        let Who::Session(me) = who else { return Ok(self.back("/teams", "Sign in to the grid, then connect GitHub again.", true)) };
        let app = self.app()?;
        let Some(started) = q(query, "state").and_then(|s| app.take_state(s)) else {
            return Ok(self.back("/teams", "That GitHub link is old or was used: connect again from the team's page.", true));
        };
        let page = format!("/teams/{}#github", started.slug);
        if started.user_id != me.id || !self.teams.role(&me.id, &started.slug)?.is_some_and(Role::manages) {
            return Ok(self.back(&page, "Only the owner or admin who began connecting GitHub can finish it.", true));
        }
        if q(query, "setup_action") == Some("request") {
            return Ok(self.back(&page, "GitHub asked your org's owners to approve the App. Connect again once they have.", false));
        }
        let id = q(query, "installation_id").and_then(|v| v.parse::<i64>().ok());
        let (Some(id), Some(code)) = (id, q(query, "code")) else {
            return Ok(self.back(&page, "GitHub didn't say which installation, or who made it: connect again.", true));
        };
        // the installation must be one the person who installed it can see: an id in a URL proves nothing
        if !app.installer_has(code, id).await? {
            tracing::warn!("grid: {} tried to tie GitHub installation {id} they can't see to {}", me.handle, started.slug);
            return Ok(self.back(&page, "That GitHub installation isn't one of yours.", true));
        }
        let account = app.account_of(id).await?;
        self.teams.github_connect(&started.team_id, id, &account, &me.handle)?;
        tracing::info!("grid: {} tied {}'s GitHub to {account} ({id})", me.handle, started.slug);
        Ok(self.back(&page, &format!("GitHub connected: {account}. The hub's agents get tokens for the repos you gave the App."), false))
    }

    /// `POST /github/webhook`: GitHub's event for an installation, checked the App's, to its team's hub.
    pub(super) async fn github_webhook(&self, req: http::Request<Incoming>) -> Result<Resp> {
        let app = self.app()?.clone();
        let (head, body) = req.into_parts();
        let Ok(bytes) = read_bytes(body, WEBHOOK_MAX).await else {
            return Ok(json(413, &json!({ "error": "too big" })));
        };
        let h = |k: &str| head.headers.get(k).and_then(|v| v.to_str().ok()).unwrap_or_default().to_owned();
        if !app.webhook_signed(&bytes, &h("x-hub-signature-256")) {
            return Ok(json(401, &json!({ "error": "not signed by the App" })));
        }
        let event = h("x-github-event");
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let Some(installation) = v.pointer("/installation/id").and_then(Value::as_i64) else {
            return Ok(json(202, &json!({ "ignored": "no installation" })));
        };
        if event == "installation" && matches!(v.get("action").and_then(Value::as_str), Some("deleted" | "suspend")) {
            app.forget(installation);
            if let Some(team) = self.teams.github_gone(installation)? {
                tracing::info!("grid: GitHub removed installation {installation}: untied from team {team}");
            }
            return Ok(json(200, &json!({ "ok": true })));
        }
        let Some(&(_, to)) = FORWARDED.iter().find(|(e, _)| *e == event) else {
            return Ok(json(202, &json!({ "ignored": event })));
        };
        let Some(c) = self.teams.github_by_installation(installation)? else {
            return Ok(json(202, &json!({ "ignored": "no team" })));
        };
        let Some(hub) = self.teams.hub_by_team(&c.team_id)? else { return Ok(json(202, &json!({ "ignored": "no hub" }))) };
        let acct = self.accounts.user_by_id(&hub.user_id)?.ok_or_else(|| bad("That hub has no account.", 409))?;
        let mut b = http::Request::builder()
            .method(Method::POST)
            .uri(to)
            .header("content-type", "application/json")
            .header("x-github-event", event.as_str());
        let delivery = h("x-github-delivery");
        if !delivery.is_empty() {
            b = b.header("x-github-delivery", delivery);
        }
        let mut call = b.body(full(bytes)).map_err(|e| bad(format!("That event can't be passed on: {e}"), 400))?;
        // the grid checked GitHub's signature: the hub's wisp takes it as `grid:github`, which may only hand it to triggers
        call.extensions_mut().insert(Via("github"));
        tracing::info!("grid: GitHub {event} for {} to its hub", hub.slug);
        self.proxy(call, &BoxUser::user(&acct), &hub.slug).await
    }

    /// `GET /api/github/token`: a hub's own box asking for its team's token.
    pub(super) async fn github_token(&self, who: &Who) -> Result<Resp> {
        let Who::Box(u) = who else { return Err(bad("A hub's own box asks for its token.", 401)) };
        let Some(hub) = self.teams.hub_by_user(&u.id)? else { return Err(bad("Only a team's hub has a GitHub token here.", 403)) };
        let Some(app) = self.github.as_ref() else { return Ok(json(404, &json!({ "error": "This grid has no GitHub App." }))) };
        let Some(c) = self.teams.github_of(&hub.team_id)? else {
            return Ok(json(404, &json!({ "error": format!("{} hasn't connected GitHub.", hub.slug) })));
        };
        let (token, until) = app.token(c.installation_id).await?;
        Ok(json(200, &json!({ "token": token, "expiresAt": until, "account": c.account })))
    }

    /// Mints (or keeps) a hub's team's token now, so a box starting next has it in hand; a failure only says so.
    pub(super) async fn github_warm(&self, hub_user: &str) {
        let Some(app) = self.github.as_ref() else { return };
        let Ok(Some(hub)) = self.teams.hub_by_user(hub_user) else { return };
        let Ok(Some(c)) = self.teams.github_of(&hub.team_id) else { return };
        if let Err(e) = app.token(c.installation_id).await {
            tracing::warn!("grid: no GitHub token for {}'s sandbox: {e}", hub.slug);
        }
    }
}
