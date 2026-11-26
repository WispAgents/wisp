//! Shares, the gallery and teams on the grid's own address (ADR-0038, ADR-0041): the API, the share pages, `/gallery`.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use http::{HeaderMap, Method};
use serde_json::{Value, json};

use super::{Grid, Who, answer, no, query};
use crate::error::{Result, bad};
use crate::http::{Resp, client_ip, html, json, json_with, parse_body};
use crate::pages::{self, MessagePage, ShareCard, Tone, Tried};
use crate::rows::UserRow;
use crate::shares::{Access, Patch, Put, Refused, check_password, could_be_password, hash_password, parse_ref};
use crate::trust::{Reach, Verdict, may};

/// Password guesses at one share a minute: from one address, and from everywhere (a botnet's spread guesses).
const GUESSES_PER_IP: usize = 10;
const GUESSES_PER_SHARE: usize = 60;
const TOO_MANY: &str = "Too many tries. Wait a minute and try again.";
const BUSY: &str = "The grid is busy checking passwords. Try again in a second.";

impl Grid {
    #[allow(clippy::too_many_arguments)] // the request's parts, and what to read
    pub(super) async fn read_share(
        &self,
        id: &str,
        q: &str,
        h: &HeaderMap,
        peer: Option<std::net::IpAddr>,
        who: &Who,
        password: Option<String>,
        count: bool,
    ) -> Result<std::result::Result<(crate::shares::Rec, crate::shares::Content), Refused>> {
        let s = self.shares.get(id)?.ok_or_else(|| bad("There's no share with that id.", 404))?;
        let pass = password.or_else(|| h.get("x-wisp-share-password").and_then(|v| v.to_str().ok()).map(str::to_owned));
        let pass_ok = match pass {
            Some(guess) if self.shares.asks_password(&s, who.user()) => match self.share_password(&s, guess, peer, h).await {
                Ok(ok) => ok,
                Err(why) => return Ok(Err(why)),
            },
            _ => false,
        };
        if let Err(r) = self.shares.may_read_checked(&s, who.user(), |_| pass_ok)? {
            return Ok(Err(r));
        }
        let version = query(q, "ref").and_then(|r| parse_ref(&r));
        Ok(self.shares.read(&s, who.user(), version, count)?.map(|c| (s, c)))
    }

    /// A guess at `s`'s password: counted against this address and this share first (past either, refused with no
    /// hashing at all), refused as wrong when it couldn't be one (empty, longer than any password kept), and only
    /// then checked, on the blocking pool.
    async fn share_password(
        &self,
        s: &crate::shares::Rec,
        pass: String,
        peer: Option<std::net::IpAddr>,
        h: &HeaderMap,
    ) -> std::result::Result<bool, Refused> {
        let now = self.clock.now_ms();
        let ip = client_ip(peer, h);
        if self.limits.hit(&format!("sharepw|{}|{ip}", s.id), GUESSES_PER_IP, now).is_err()
            || self.limits.hit(&format!("sharepw|{}", s.id), GUESSES_PER_SHARE, now).is_err()
        {
            return Err(Refused::TooMany);
        }
        if !could_be_password(&pass) {
            return Ok(false);
        }
        let kept = s.pass.clone().unwrap_or_default();
        self.off_the_workers(move || check_password(&kept, &pass)).await
    }

    /// Runs scrypt (`f`) on tokio's blocking pool, holding one of the grid's few `scrypt` permits: each hash is ~50 ms
    /// of CPU and would stall every request on an async worker. With none free it is `Busy` (503) at once, never a
    /// queue that grows with the guesses.
    pub(super) async fn off_the_workers<T: Send + 'static>(
        &self,
        f: impl FnOnce() -> T + Send + 'static,
    ) -> std::result::Result<T, Refused> {
        let Ok(permit) = self.scrypt.clone().try_acquire_owned() else { return Err(Refused::Busy) };
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            f()
        })
        .await
        .map_err(|_| Refused::Busy)
    }

    /// A share's new password, hashed off the workers before `put` or `patch` keeps it (the length is checked by
    /// `check_access`, so nothing overlong is hashed). None: no permit free, answer `busy()`.
    async fn hash_access(&self, a: Option<&mut Access>) -> Option<()> {
        let Some(a) = a.filter(|a| a.kind == "password") else { return Some(()) };
        let Some(p) = a.password.clone().filter(|p| could_be_password(p)) else { return Some(()) };
        a.hashed = Some(self.off_the_workers(move || hash_password(&p)).await.ok()?);
        Some(())
    }

    fn busy() -> Resp {
        json_with(503, &json!({ "error": BUSY }), &[("retry-after", "1".into())])
    }

    pub(super) fn refused(r: &Refused) -> Resp {
        match r {
            Refused::Gone(why) => json(410, &json!({ "error": format!("This share is {why}."), "gone": why })),
            Refused::Locked(how) => json(403, &json!({ "error": "This share is locked.", "locked": how })),
            Refused::NoVersion => json(404, &json!({ "error": "There's no such version." })),
            Refused::TooMany => json(429, &json!({ "error": TOO_MANY })),
            Refused::Busy => Self::busy(),
        }
    }

    pub(super) fn content_json(&self, s: &crate::shares::Rec, c: &crate::shares::Content) -> Result<Value> {
        Ok(
            json!({ "card": self.shares.card(s), "version": c.version, "ref": format!("v{}", c.version), "latest": s.version, "body": B64.encode(&c.body), "sig": c.sig, "signer": c.signer, "versions": self.shares.versions(s)? }),
        )
    }

    #[allow(clippy::too_many_lines, clippy::too_many_arguments)] // one match over the shares API, each arm a line or two; the request's parts
    pub(super) async fn shares_route(
        &self,
        method: &Method,
        parts: &[&str],
        q: &str,
        h: &HeaderMap,
        peer: Option<std::net::IpAddr>,
        bytes: &[u8],
        who: &Who,
    ) -> Result<Resp> {
        let publisher = || -> Result<&UserRow> {
            let c = who.credential().ok_or_else(|| bad("Sign in first.", 401))?;
            if let Verdict::No(why) = may(c, Reach::GridPublish) {
                return Err(bad(why, 403));
            }
            who.user().ok_or_else(|| bad("A box shares through its owner's wisp.", 403))
        };
        match (method.as_str(), parts) {
            ("POST", ["api", "shares"]) => {
                let mut p: Put = serde_json::from_slice(bytes).map_err(|e| bad(format!("Bad share: {e}"), 400))?;
                let user = publisher()?;
                if self.hash_access(p.access.as_mut()).await.is_none() {
                    return Ok(Self::busy());
                }
                Ok(json(200, &self.shares.put(user, &p)?))
            }
            ("GET", ["api", "shares"]) => Ok(json(200, &json!(self.shares.mine(&publisher()?.id)?))),
            ("PATCH", ["api", "shares", id]) => {
                let mut p: Patch = serde_json::from_slice(bytes).map_err(|e| bad(format!("Bad change: {e}"), 400))?;
                let user = publisher()?;
                if self.hash_access(p.access.as_mut()).await.is_none() {
                    return Ok(Self::busy());
                }
                Ok(json(200, &self.shares.patch(user, id, &p)?))
            }
            ("DELETE", ["api", "shares", id]) => Ok(json(200, &self.shares.revoke(publisher()?, id)?)),
            ("GET", ["api", "shares", "by", handle, key]) => {
                // an unlisted share opens by name only with its cap (ADR-0130), `?cap=` or `x-wisp-cap`; refused, it
                // answers exactly as a name nobody has, so a guess learns nothing
                let cap = query(q, "cap").or_else(|| h.get("x-wisp-cap").and_then(|v| v.to_str().ok()).map(str::to_owned));
                let s = self
                    .shares
                    .named(handle, key, who.user(), cap.as_deref())?
                    .ok_or_else(|| bad(format!("There's no grid://{handle}/{key}."), 404))?;
                match self.read_share(&s.id, q, h, peer, who, None, true).await? {
                    Ok((s, c)) => Ok(json(200, &self.content_json(&s, &c)?)),
                    Err(r) => Ok(Self::refused(&r)),
                }
            }
            ("GET", ["api", "shares", id]) => match self.read_share(id, q, h, peer, who, None, true).await? {
                Ok((s, c)) => Ok(json(200, &self.content_json(&s, &c)?)),
                Err(r) => Ok(Self::refused(&r)),
            },
            ("POST", ["api", "shares", id, "import"]) => match self.read_share(id, q, h, peer, who, None, false).await? {
                Ok((s, _)) => {
                    self.shares.count_import(&s, who.user())?;
                    Ok(json(200, &json!({ "ok": true })))
                }
                Err(r) => Ok(Self::refused(&r)),
            },
            ("GET", ["api", "gallery"]) => {
                Ok(json(200, &json!(self.shares.gallery(&query(q, "q").unwrap_or_default(), query(q, "kind").as_deref())?)))
            }
            ("GET", ["gallery"]) => {
                let qs = query(q, "q").unwrap_or_default();
                Ok(html(200, pages::gallery_page(&self.page, &qs, &self.shares.gallery(&qs, None)?, self.viewer(h).as_ref()), &[]))
            }
            ("GET" | "POST", ["s", id]) => self.share_page(method, id, q, h, peer, bytes, who).await,
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }

    /// `/s/<id>`: a share's page, its password form, or why it doesn't open (gone, locked, a typo'd link).
    #[allow(clippy::too_many_arguments)] // the request's parts, as `shares_route` has them
    async fn share_page(
        &self,
        method: &Method,
        id: &str,
        q: &str,
        h: &HeaderMap,
        peer: Option<std::net::IpAddr>,
        bytes: &[u8],
        who: &Who,
    ) -> Result<Resp> {
        let password =
            if *method == Method::POST { parse_body(h, bytes)?.get("password").and_then(Value::as_str).map(str::to_owned) } else { None };
        let tried = if password.is_some() { Tried::Wrong } else { Tried::Not };
        // the link's version, parsed: the password form posts back to it
        let version = query(q, "ref").and_then(|r| parse_ref(&r));
        // who's signed in to the grid, for its header (QA round 6 #4)
        let viewer = self.viewer(h);
        let read = match self.read_share(id, q, h, peer, who, password, true).await {
            Err(e) if e.status() == 404 => {
                let m = MessagePage {
                    title: "Nothing here".into(),
                    text: "This isn’t shared any more, or its link has a typo.".into(),
                    link: Some((self.cfg.public_url.clone(), "Home".into())),
                    tone: Tone::Plain,
                    user: viewer.clone(),
                };
                return Ok(html(404, pages::message_page(&self.page, &m), &[]));
            }
            other => other?,
        };
        match read {
            Ok((s, c)) => {
                let content: Value = serde_json::from_slice(&c.body).unwrap_or(Value::Null);
                let card = ShareCard {
                    id: s.id.clone(),
                    kind: s.kind.clone(),
                    title: s.title.clone(),
                    summary: s.summary.clone(),
                    from: s.handle.clone(),
                    version: c.version,
                    source: crate::shares::Shares::source(&s),
                    access: s.access.clone(),
                    signed_in: matches!(who, Who::Session(_)),
                };
                Ok(html(200, pages::share_page(&self.page, &card, &content, viewer.as_ref()), &[]))
            }
            Err(Refused::Gone(why)) => Ok(html(410, pages::gone_page(&self.page, why, viewer.as_ref()), &[])),
            Err(Refused::Locked(how)) => Ok(html(403, pages::locked_page(&self.page, id, &how, tried, version, viewer.as_ref()), &[])),
            Err(Refused::TooMany) => {
                Ok(html(429, pages::locked_page(&self.page, id, "password", Tried::TooMany, version, viewer.as_ref()), &[]))
            }
            Err(Refused::Busy) => {
                let m = MessagePage { title: "One moment".into(), text: BUSY.into(), link: None, tone: Tone::Plain, user: viewer.clone() };
                Ok(html(503, pages::message_page(&self.page, &m), &[("retry-after", "1".into())]))
            }
            Err(Refused::NoVersion) => Ok(self.not_found_for(
                answer::Asker { page: true, viewer: viewer.clone() },
                "Nothing here",
                "There’s no such version of this.",
            )),
        }
    }

    pub(super) fn teams_route(&self, method: &Method, parts: &[&str], h: &HeaderMap, bytes: &[u8], who: &Who) -> Result<Resp> {
        let c = who.credential().ok_or_else(|| bad("Sign in first.", 401))?;
        if let Some(r) = no(&may(c, Reach::GridTeams)) {
            return Ok(r);
        }
        let me = who.user().ok_or_else(|| bad("Sign in first.", 401))?;
        let b = parse_body(h, bytes).unwrap_or_default();
        let f = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        match (method.as_str(), parts) {
            ("POST", ["api", "teams"]) => Ok(json(200, &self.teams.create(me, &f("slug"), &f("name"))?)),
            ("GET", ["api", "teams"]) => Ok(json(200, &json!(self.teams.of(&me.id)?))),
            ("GET", ["api", "teams", slug]) => {
                if self.teams.role(&me.id, slug)?.is_none() {
                    return Err(bad(format!("You aren't in {slug}."), 403));
                }
                Ok(json(200, &json!({ "slug": slug, "members": self.teams.members(slug)? })))
            }
            ("POST", ["api", "teams", slug, "members"]) => {
                self.teams.add(me, slug, &f("handle"), &f("role"))?;
                Ok(json(200, &json!({ "ok": true })))
            }
            ("DELETE", ["api", "teams", slug, "members", handle]) => {
                self.teams.remove(me, slug, handle)?;
                Ok(json(200, &json!({ "ok": true })))
            }
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }
}
