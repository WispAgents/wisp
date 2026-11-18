//! Shares, the gallery and teams on the grid's own address (ADR-0038, ADR-0041): the API, the share pages, `/gallery`.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use http::{HeaderMap, Method};
use serde_json::{Value, json};

use super::{Grid, Who, answer, query};
use crate::error::{Result, bad};
use crate::http::{Resp, html, json, parse_body};
use crate::pages::{self, MessagePage, ShareCard, Tone};
use crate::rows::UserRow;
use crate::shares::{Patch, Put, Refused, parse_ref};
use crate::trust::{Reach, Verdict, may};

impl Grid {
    pub(super) fn read_share(
        &self,
        id: &str,
        q: &str,
        h: &HeaderMap,
        who: &Who,
        password: Option<String>,
        count: bool,
    ) -> Result<std::result::Result<(crate::shares::Rec, crate::shares::Content), Refused>> {
        let s = self.shares.get(id)?.ok_or_else(|| bad("There's no share with that id.", 404))?;
        let pass = password.or_else(|| h.get("x-wisp-share-password").and_then(|v| v.to_str().ok()).map(str::to_owned));
        if let Err(r) = self.shares.may_read(&s, who.user(), pass.as_deref())? {
            return Ok(Err(r));
        }
        let version = query(q, "ref").and_then(|r| parse_ref(&r));
        Ok(self.shares.read(&s, who.user(), version, count)?.map(|c| (s, c)))
    }

    pub(super) fn refused(r: &Refused) -> Resp {
        match r {
            Refused::Gone(why) => json(410, &json!({ "error": format!("This share is {why}."), "gone": why })),
            Refused::Locked(how) => json(403, &json!({ "error": "This share is locked.", "locked": how })),
            Refused::NoVersion => json(404, &json!({ "error": "There's no such version." })),
        }
    }

    pub(super) fn content_json(&self, s: &crate::shares::Rec, c: &crate::shares::Content) -> Result<Value> {
        Ok(
            json!({ "card": self.shares.card(s), "version": c.version, "ref": format!("v{}", c.version), "latest": s.version, "body": B64.encode(&c.body), "sig": c.sig, "signer": c.signer, "versions": self.shares.versions(s)? }),
        )
    }

    #[allow(clippy::too_many_lines)] // one match over the shares API, each arm a line or two
    pub(super) fn shares_route(&self, method: &Method, parts: &[&str], q: &str, h: &HeaderMap, bytes: &[u8], who: &Who) -> Result<Resp> {
        let publisher = || -> Result<&UserRow> {
            let c = who.credential().ok_or_else(|| bad("Sign in first.", 401))?;
            if let Verdict::No(why) = may(c, Reach::GridPublish) {
                return Err(bad(why, 403));
            }
            who.user().ok_or_else(|| bad("A box shares through its owner's wisp.", 403))
        };
        match (method.as_str(), parts) {
            ("POST", ["api", "shares"]) => {
                let p: Put = serde_json::from_slice(bytes).map_err(|e| bad(format!("Bad share: {e}"), 400))?;
                Ok(json(200, &self.shares.put(publisher()?, &p)?))
            }
            ("GET", ["api", "shares"]) => Ok(json(200, &json!(self.shares.mine(&publisher()?.id)?))),
            ("PATCH", ["api", "shares", id]) => {
                let p: Patch = serde_json::from_slice(bytes).map_err(|e| bad(format!("Bad change: {e}"), 400))?;
                Ok(json(200, &self.shares.patch(publisher()?, id, &p)?))
            }
            ("DELETE", ["api", "shares", id]) => Ok(json(200, &self.shares.revoke(publisher()?, id)?)),
            ("GET", ["api", "shares", "by", handle, key]) => {
                let s = self.shares.by_name(handle, key)?.ok_or_else(|| bad(format!("There's no grid://{handle}/{key}."), 404))?;
                match self.read_share(&s.id, q, h, who, None, true)? {
                    Ok((s, c)) => Ok(json(200, &self.content_json(&s, &c)?)),
                    Err(r) => Ok(Self::refused(&r)),
                }
            }
            ("GET", ["api", "shares", id]) => match self.read_share(id, q, h, who, None, true)? {
                Ok((s, c)) => Ok(json(200, &self.content_json(&s, &c)?)),
                Err(r) => Ok(Self::refused(&r)),
            },
            ("POST", ["api", "shares", id, "import"]) => match self.read_share(id, q, h, who, None, false)? {
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
                Ok(html(200, pages::gallery_page(&self.page, &qs, &self.shares.gallery(&qs, None)?), &[]))
            }
            ("GET" | "POST", ["s", id]) => self.share_page(method, id, q, h, bytes, who),
            _ => Ok(json(404, &json!({ "error": "not found" }))),
        }
    }

    /// `/s/<id>`: a share's page, its password form, or why it doesn't open (gone, locked, a typo'd link).
    fn share_page(&self, method: &Method, id: &str, q: &str, h: &HeaderMap, bytes: &[u8], who: &Who) -> Result<Resp> {
        let password =
            if *method == Method::POST { parse_body(h, bytes)?.get("password").and_then(Value::as_str).map(str::to_owned) } else { None };
        let tried = password.is_some();
        let read = match self.read_share(id, q, h, who, password, true) {
            Err(e) if e.status() == 404 => {
                let m = MessagePage {
                    title: "Nothing here".into(),
                    text: "This isn’t shared any more, or its link has a typo.".into(),
                    link: Some((self.cfg.public_url.clone(), "Home".into())),
                    tone: Tone::Plain,
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
                    source: format!("grid://{}/{}", s.handle, s.key),
                    access: s.access.clone(),
                    signed_in: matches!(who, Who::Session(_)),
                };
                Ok(html(200, pages::share_page(&self.page, &card, &content), &[]))
            }
            Err(Refused::Gone(why)) => Ok(html(410, pages::gone_page(&self.page, why), &[])),
            Err(Refused::Locked(how)) => Ok(html(403, pages::locked_page(&self.page, id, &how, tried), &[])),
            Err(Refused::NoVersion) => {
                Ok(self.not_found_for(answer::Asker { page: true }, "Nothing here", "There’s no such version of this."))
            }
        }
    }

    pub(super) fn teams_route(&self, method: &Method, parts: &[&str], h: &HeaderMap, bytes: &[u8], who: &Who) -> Result<Resp> {
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
