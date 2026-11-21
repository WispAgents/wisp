//! Reaching a computer or a box through the grid: the tunnel a computer holds open, a computer's address, a user's
//! box at `<handle>` and the proxy into it (woken unless passive, counted as use until answered).
use std::sync::Arc;

use http::{HeaderMap, Method};
use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use serde_json::json;

use super::{Grid, PASSIVE, Via, Who, answer, handoff, incoming, no};
use crate::boxhost::{BoxHost, BoxState};
use crate::error::{Result, bad};
use crate::http::{Resp, empty, header_str, json, json_with};
use crate::rows::{BoxUser, ComputerRow};
use crate::trust::{Credential, Reach, may};
use crate::tunnels::TUNNEL_PROTOCOL;

impl Grid {
    /// `GET /api/computers/<id>/tunnel` with `Upgrade: wisp-tunnel`: the computer holds its tunnel open.
    pub(super) fn tunnel(self: &Arc<Self>, req: http::Request<Incoming>, who: &Who, id: &str) -> Result<Resp> {
        let c = who.credential().ok_or_else(|| bad("This device isn't signed in.", 401))?;
        if let Some(r) = no(&may(c, Reach::GridTunnel)) {
            return Ok(r);
        }
        let Who::Device(u, d) = who else { return Err(bad("This device isn't signed in.", 401)) };
        let comp = self
            .accounts
            .computer(id)?
            .filter(|c| c.user_id == u.id && c.device_id == d.id)
            .ok_or_else(|| bad("The grid doesn't know this computer any more.", 404))?;
        if !header_str(req.headers(), "upgrade").eq_ignore_ascii_case(TUNNEL_PROTOCOL) {
            return Err(bad(format!("Upgrade: {TUNNEL_PROTOCOL}"), 400));
        }
        let me = self.clone();
        let cid = comp.id.clone();
        let now = self.clock.now_ms();
        let owner = u.id.clone();
        tokio::spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(up) => {
                    let _ = me.accounts.seen_computer(&cid);
                    // its owner's open pages say Online once the tunnel is up, and Offline once it's gone
                    let (watch, id, who) = (me.clone(), cid.clone(), owner.clone());
                    tokio::spawn(async move {
                        for _ in 0..50 {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            if watch.tunnels.online(&id) {
                                return watch.notify(&who);
                            }
                        }
                    });
                    let _ = me.tunnels.accept(&cid, up, now).await;
                    me.notify(&owner);
                }
                Err(e) => tracing::warn!("grid: tunnel {cid}: {e}"),
            }
        });
        let mut r = crate::http::respond(
            101,
            "",
            empty(),
            &[("upgrade", TUNNEL_PROTOCOL.to_owned()), ("connection", "Upgrade".to_owned()), ("cache-control", String::new())],
        );
        *r.status_mut() = http::StatusCode::SWITCHING_PROTOCOLS;
        Ok(r)
    }

    /// A computer (or a page on it, `preview`) through its tunnel: only its own user's device reaches it.
    pub(super) async fn computer_reach(&self, req: http::Request<Incoming>, c: &ComputerRow, preview: Option<u16>) -> Result<Resp> {
        let Some(owner) = self.accounts.user_by_id(&c.user_id)?.filter(|u| u.disabled == 0) else {
            return Ok(self.not_found(&req, "No such computer", "Nothing answers at this address."));
        };
        let go = format!("/go?computer={}", c.id);
        let req = if preview.is_some() {
            req
        } else {
            match self.address_own(req, &c.label, &owner, &go, !self.tunnels.online(&c.id)) {
                handoff::Own::Done(r) => return Ok(r),
                handoff::Own::Next(r) => r,
            }
        };
        let who = self.who(req.headers());
        let here = match (&who, preview) {
            (Who::Nobody, None) => self.address_user(req.headers(), &c.label, &owner),
            _ => None,
        };
        let cred = match (who.credential(), &here) {
            (Some(c), _) => c,
            (None, Some(_)) => Credential::Cookie,
            (None, None) if preview.is_none() => {
                let pq = req.uri().path_and_query().map_or("/", |p| p.as_str()).to_owned();
                return Ok(self.sign_in_at(req.method(), req.headers(), &pq, &go));
            }
            (None, None) => return Err(bad("Sign in first.", 401)),
        };
        if let Some(r) = no(&may(cred, if preview.is_some() { Reach::Preview } else { Reach::Computer }))
            .filter(|_| !(preview.is_some() && cred == Credential::Device))
        {
            return Ok(r);
        }
        if here.is_none() && who.user().is_none_or(|u| u.id != c.user_id) {
            return Err(bad("That computer isn't yours.", 403));
        }
        if cred == Credential::Cookie && self.foreign_origin(req.method(), req.headers(), &c.label) {
            return Err(bad("forbidden origin", 403));
        }
        // its waiting page (QA round 10 Grid #2): while it's offline, the page; once it's back, what was asked for
        // (a preview's too, on its own address: its page waits the same way)
        if req.uri().path() == answer::WAITING {
            let online = self.tunnels.online(&c.id);
            let store = [("cache-control", "no-store".to_owned())];
            // its poll: 200 {online}, never a 503 in the console every few seconds
            if !answer::Asker::of(req.method(), req.headers()).page {
                return Ok(crate::http::json_with(200, &serde_json::json!({ "online": online }), &store));
            }
            let next = super::query(req.uri().query().unwrap_or_default(), "r").unwrap_or_default();
            if online {
                return Ok(crate::http::redirect(crate::http::safe_path(&next, "/"), &store));
            }
            let here = req.uri().path_and_query().map_or(answer::WAITING, |p| p.as_str()).to_owned();
            return Ok(self.offline(&answer::Asker { page: true, viewer: None }, c, &here));
        }
        let (mut head, body) = req.into_parts();
        let path = head.uri.path_and_query().map_or("/", |p| p.as_str()).to_owned();
        head.uri = format!("http://{}{path}", c.label).parse().map_err(|_| bad("Bad path.", 400))?;
        head.headers.remove("authorization");
        head.headers.remove("cookie");
        head.headers.remove("x-wisp-tunnel");
        head.headers.remove("x-wisp-preview");
        // only the grid's own route marks a request to a computer as a routed message (`route.rs`)
        head.headers.remove(crate::route::ROUTE_MARK);
        if let Some(p) = preview
            && let Ok(v) = p.to_string().parse()
        {
            head.headers.insert("x-wisp-preview", v);
        }
        let asker = answer::Asker::of(&head.method, &head.headers);
        let req = http::Request::from_parts(head, incoming(body));
        Ok(self.tunnels.forward(&c.id, req, preview.is_some()).await.unwrap_or_else(|| self.offline(&asker, c, &path)))
    }

    pub(super) async fn edge(&self, req: http::Request<Incoming>, handle: &str) -> Result<Resp> {
        let Some(user) = self.accounts.user_by_handle(handle)?.filter(|u| u.disabled == 0) else {
            return Ok(self.not_found(&req, "Nobody here", "There's nobody with that handle here."));
        };
        let req = match self.address_own(req, handle, &user, "/go", false) {
            handoff::Own::Done(r) => return Ok(r),
            handoff::Own::Next(r) => r,
        };
        let who = self.who(req.headers());
        // an API key opens this address only (never a computer's, a sandbox's or the grid's own API: `trust`)
        let key = match who {
            Who::Nobody => self.keys.check(header_str(req.headers(), "authorization"))?,
            _ => None,
        };
        // a browser signed in here from the grid's pages (`/go`, `handoff.rs`)
        let here = match (&key, &who) {
            (None, Who::Nobody) => self.address_user(req.headers(), handle, &user),
            _ => None,
        };
        let (cred, owner) = match (&key, who.credential(), here) {
            (Some((u, _)), _, _) => (Credential::Key, Some(u.id.clone())),
            (None, Some(c), _) => (c, who.user().map(|u| u.id.clone())),
            (None, None, Some(u)) => (Credential::Cookie, Some(u.id)),
            (None, None, None) => {
                let pq = req.uri().path_and_query().map_or("/", |p| p.as_str()).to_owned();
                return Ok(self.sign_in_at(req.method(), req.headers(), &pq, "/go"));
            }
        };
        if let Some(r) = no(&may(cred, Reach::Box)).filter(|_| cred != Credential::Session) {
            return Ok(r);
        }
        if cred == Credential::Cookie && self.foreign_origin(req.method(), req.headers(), handle) {
            return Err(bad("forbidden origin", 403));
        }
        // another user's key is refused outright, like their device token
        if owner.is_none_or(|id| id != user.id) {
            return Err(bad("That wisp isn't yours.", 403));
        }
        // a page in the box at its own address: a one-time link there (edge_preview.rs)
        if req.uri().path() == "/_grid/preview" && req.method() == Method::GET {
            let q = req.uri().query().unwrap_or_default().to_owned();
            return self.preview_link(&q, &user, cred);
        }
        let mut req = req;
        if key.is_some() {
            // the box's wisp speaks for a key with the `apikey` set (wisp-api auth.rs): never key management
            req.extensions_mut().insert(Via("key"));
        }
        self.proxy(req, &BoxUser::user(&user), handle).await
    }

    /// A request into a box (the user's own, or a sandbox): woken unless passive, counted as use until answered.
    pub(super) async fn proxy(&self, req: http::Request<Incoming>, bu: &BoxUser, handle: &str) -> Result<Resp> {
        let path = req.uri().path().to_owned();
        if path == "/_grid/state" {
            return Ok(json(200, &json!({ "state": self.boxes.state(&bu.id).as_str() })));
        }
        if path == "/_grid/stop" && req.method() == Method::POST {
            let was = self.boxes.stop(&bu.id, "stopped").await?;
            return Ok(json(200, &json!({ "stopped": was })));
        }
        let h = req.headers();
        let passive = header_str(h, PASSIVE) == "1" || header_str(h, "x-wisp-relay") == "1";
        if passive && self.boxes.state(&bu.id) != BoxState::Running {
            return Ok(json_with(503, &json!({ "error": "This box is asleep.", "asleep": true }), &[("retry-after", "20".into())]));
        }
        let running = self.boxes.ensure(bu, false).await?;
        let busy = std::sync::Mutex::new(self.boxes.begin(&bu.id, !passive));
        let secret = self.boxes.secret(&bu.id).unwrap_or_default();
        let (mut head, body) = req.into_parts();
        let via = head.extensions.remove::<Via>();
        let pq = head.uri.path_and_query().map_or("/", |p| p.as_str()).to_owned();
        head.uri = format!("http://127.0.0.1:{}{pq}", running.port).parse().map_err(|_| bad("Bad path.", 400))?;
        let public = self.cfg.instance_url(handle);
        let fwd_host = public.split("://").nth(1).unwrap_or_default().to_owned();
        for k in [
            "authorization",
            "cookie",
            "x-wisp-proxy-secret",
            "x-wisp-proxy-via",
            "x-forwarded-host",
            "x-forwarded-proto",
            PASSIVE,
            "x-wisp-relay",
        ] {
            head.headers.remove(k);
        }
        let set = |h: &mut HeaderMap, k: &'static str, v: &str| {
            if let Ok(v) = v.parse() {
                h.insert(k, v);
            }
        };
        set(&mut head.headers, "x-wisp-proxy-secret", &secret);
        set(&mut head.headers, "x-forwarded-host", &fwd_host);
        set(&mut head.headers, "x-forwarded-proto", &self.cfg.scheme);
        if let Some(Via(v)) = via {
            set(&mut head.headers, "x-wisp-proxy-via", v);
        }
        set(&mut head.headers, "host", &format!("127.0.0.1:{}", running.port));
        let up = self
            .client
            .request(http::Request::from_parts(head, incoming(body)))
            .await
            .map_err(|e| bad(format!("Your wisp didn't answer: {e}"), 502))?;
        let (parts, b) = up.into_parts();
        // the request counts as open (and, unless passive, as use) until its answer has been read
        let b = b.map_err(std::io::Error::other).map_frame(move |f| {
            let _ = &busy;
            f
        });
        Ok(http::Response::from_parts(parts, b.boxed()))
    }
}
