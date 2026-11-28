//! A team's hub at `<team>.<domain>` (ADR-0148 §2, §5): the team's members by role, its deploy keys, and its deploys.
//!
//! Who gets in, and what the hub's wisp hears (`x-wisp-proxy-via`, set here only; wisp-api auth.rs turns it into a set):
//! an owner or admin `owner`, a member `member`, a viewer `viewer`, a deploy key `deploy` or
//! `plan`, a connector's webhook `hook`. Anyone else is refused. A person's API key (`wgk_`) opens their own wisp only.
//!
//! `POST /api/cmd` with `resources.apply` is a deploy: one at a time per hub (`hub_deploys.rs`), kept with its source
//! (`x-wisp-deploy-source`) and what it changed. `GET /_grid/deploys` lists them for the team and its keys.
use http::Method;
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use serde_json::{Value, json};

use super::{Grid, Via, Who, handoff, incoming, no};
use crate::error::{Result, bad};
use crate::http::{Resp, full, header_str, json, json_with, read_bytes};
use crate::hub_deploys::{By, counts};
use crate::hub_keys::{HubKey, Rights};
use crate::hubs::Hub;
use crate::rows::{BoxUser, UserRow};
use crate::teams::Role;
use crate::trust::{Credential, Reach, may};

/// The most a deploy's formula may weigh (a folder of YAML, evaluated): far past any real one.
const DEPLOY_MAX: usize = 8 << 20;

/// Who is at a hub's address, once checked.
enum At {
    Key(HubKey),
    Person(UserRow, Role, Credential),
    Hook,
}

impl At {
    /// What the hub's wisp hears (None: the box's person).
    fn via(&self) -> Option<&'static str> {
        match self {
            Self::Key(k) if k.rights == Rights::Deploy => Some("deploy"),
            Self::Key(_) => Some("plan"),
            // the box's person, with its machines (the hub's own sandboxes)
            Self::Person(_, Role::Owner | Role::Admin, c) if *c != Credential::Box => Some("owner"),
            Self::Person(_, Role::Owner | Role::Admin, _) => None,
            Self::Person(_, Role::Member, _) => Some("member"),
            Self::Person(_, Role::Viewer, _) => Some("viewer"),
            Self::Hook => Some("hook"),
        }
    }
    /// Who a deploy is by, when this caller may deploy.
    fn deployer(&self) -> Option<By> {
        match self {
            Self::Key(k) if k.rights == Rights::Deploy => Some(By { kind: "key", name: k.name.clone() }),
            Self::Person(u, r, _) if r.manages() => Some(By { kind: "person", name: u.handle.clone() }),
            _ => None,
        }
    }
}

/// `/api/cmd`'s own answer shape, so `wisp grid deploy` and the GUI read a refusal like any command's.
fn cmd_refusal(status: u16, code: &str, message: &str, retry: bool) -> Resp {
    json(status, &json!({ "ok": false, "error": { "code": code, "message": message, "retry": retry }, "message": message }))
}

impl Grid {
    pub(super) async fn hub_edge(&self, req: http::Request<Incoming>, acct: &UserRow, hub: &Hub) -> Result<Resp> {
        let go = format!("/go?hub={}", hub.slug);
        let req = match self.address_own(req, &hub.slug, acct, &go, false) {
            handoff::Own::Done(r) => return Ok(r),
            handoff::Own::Next(r) => r,
        };
        let at = match self.hub_who(&req, acct, hub)? {
            Ok(at) => at,
            Err(r) => return Ok(r),
        };
        let cred = match &at {
            At::Key(_) => Credential::HubKey,
            At::Person(_, _, c) => *c,
            At::Hook => Credential::Hook,
        };
        if let Some(r) = no(&may(cred, Reach::Box)).filter(|_| cred != Credential::Session) {
            return Ok(r);
        }
        if cred == Credential::Cookie && self.foreign_origin(req.method(), req.headers(), &hub.slug) {
            return Err(bad("forbidden origin", 403));
        }
        if req.uri().path() == "/_grid/deploys" && req.method() == Method::GET {
            return Ok(json_with(
                200,
                &json!({ "deploys": self.teams.deploys(&hub.team_id, 50)? }),
                &[("cache-control", "no-store".into())],
            ));
        }
        let mut req = req;
        // a bearer can't be sent by a browser on its own: no Origin for the box's wisp to refuse (edge_reach.rs)
        if matches!(cred, Credential::HubKey | Credential::Device) {
            req.headers_mut().remove(http::header::ORIGIN);
        }
        if let Some(v) = at.via() {
            req.extensions_mut().insert(Via(v));
        }
        let bu = BoxUser::user(acct);
        match at.deployer() {
            Some(by) if req.method() == Method::POST && req.uri().path() == "/api/cmd" => self.hub_cmd(req, &bu, hub, by).await,
            _ => self.proxy(req.map(incoming), &bu, &hub.slug).await,
        }
    }

    /// Who is asking at a hub's address: a deploy key, someone in the team (and their role), or a connector's webhook;
    /// otherwise the answer to give them (a sign-in, or a refusal).
    fn hub_who(&self, req: &http::Request<Incoming>, acct: &UserRow, hub: &Hub) -> Result<std::result::Result<At, Resp>> {
        let h = req.headers();
        let auth = header_str(h, "authorization");
        let who = self.who(h);
        if matches!(who, Who::Nobody) {
            if let Some(k) = self.teams.key_check(auth)? {
                if k.hub.team_id != hub.team_id {
                    return Err(bad("That deploy key is another team's.", 403));
                }
                return Ok(Ok(At::Key(k)));
            }
            if crate::keys::bearer_key(auth).is_some() {
                return Err(bad("An API key opens your own wisp. A hub takes its team's deploy keys (wgt_…).", 403));
            }
        }
        let (person, cred) = match (who.user().cloned(), who.credential()) {
            // a box asking for itself: the hub's own wisp
            (Some(u), Some(Credential::Box)) if u.id == acct.id => return Ok(Ok(At::Person(u, Role::Owner, Credential::Box))),
            (Some(u), Some(c)) => (u, c),
            _ => match self.address_user(h, &hub.slug, acct) {
                Some(u) => (u, Credential::Cookie),
                None if super::reach::events_hook(req.method(), req.uri().path()) => return Ok(Ok(At::Hook)),
                None => {
                    let pq = req.uri().path_and_query().map_or("/", |p| p.as_str()).to_owned();
                    return Ok(Err(self.sign_in_at(req.method(), h, &pq, &format!("/go?hub={}", hub.slug))));
                }
            },
        };
        match self.teams.role(&person.id, &hub.slug)? {
            Some(role) => Ok(Ok(At::Person(person, role, cred))),
            None => Err(bad(format!("That hub is {}'s: ask one of its owners to add you.", hub.slug), 403)),
        }
    }

    /// A command into a hub from someone who may deploy: `resources.apply` takes the hub's deploy lock and is kept in
    /// its history; anything else goes straight through.
    async fn hub_cmd(&self, req: http::Request<Incoming>, bu: &BoxUser, hub: &Hub, by: By) -> Result<Resp> {
        let (head, body) = req.into_parts();
        let bytes = read_bytes(body, DEPLOY_MAX).await?;
        let cmd = serde_json::from_slice::<Value>(&bytes).ok().and_then(|v| v.get("cmd").and_then(Value::as_str).map(str::to_owned));
        let source = header_str(&head.headers, "x-wisp-deploy-source").to_owned();
        let req = http::Request::from_parts(head, full(bytes));
        if cmd.as_deref() != Some("resources.apply") {
            return self.proxy(req, bu, &hub.slug).await;
        }
        let n = match self.teams.deploy_begin(&hub.team_id, &by, &source) {
            Ok(n) => n,
            Err(e) if e.status() == 409 => return Ok(cmd_refusal(409, "conflict", &e.to_string(), true)),
            Err(e) => return Err(e),
        };
        let resp = match self.proxy(req, bu, &hub.slug).await {
            Ok(r) => r,
            Err(e) => {
                self.teams.deploy_end(&hub.team_id, n, Err(e.to_string()))?;
                return Err(e);
            }
        };
        let (parts, body) = resp.into_parts();
        let bytes = match body.collect().await {
            Ok(b) => b.to_bytes(),
            Err(e) => {
                self.teams.deploy_end(&hub.team_id, n, Err(format!("the hub's answer broke off: {e}")))?;
                return Err(bad("The hub's answer broke off.", 502));
            }
        };
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let outcome = if parts.status.is_success() && v.get("ok") == Some(&Value::Bool(true)) {
            Ok(counts(&v))
        } else {
            let why = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("the hub refused the deploy");
            Err(why.to_owned())
        };
        self.teams.deploy_end(&hub.team_id, n, outcome)?;
        Ok(http::Response::from_parts(parts, full(bytes)))
    }
}
