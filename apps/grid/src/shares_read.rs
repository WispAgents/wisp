//! Reading a share (ADR-0038, ADR-0130): who may, by which name, which version, what counts as a view or an import, what a
//! reader sees, and the gallery.
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use super::{Content, Rec, Refused, SELECT, Shares, check_password, rec};
use crate::error::Result;
use crate::rows::UserRow;

/// Whether `got` is `cap`, compared in constant time. An empty cap (none kept) opens nothing.
fn cap_is(cap: &str, got: &str) -> bool {
    use subtle::ConstantTimeEq as _;
    !cap.is_empty() && bool::from(cap.as_bytes().ct_eq(got.trim().as_bytes()))
}

impl Shares {
    /// The share's name: `grid://<handle>/<key>` once published, else with its capability after `#` (ADR-0130), the
    /// one way it opens by name.
    pub fn source(s: &Rec) -> String {
        if s.published { format!("grid://{}/{}", s.handle, s.key) } else { format!("grid://{}/{}#{}", s.handle, s.key, s.cap) }
    }

    /// What a reader sees before the content: who shared it, what it is. Only ever shown to a reader who opened it
    /// (by id, or by name with its cap) or, published, in the gallery: its source (with the cap while unlisted) tells
    /// them nothing the link they used didn't.
    pub fn card(&self, s: &Rec) -> Value {
        json!({
            "id": s.id, "url": self.url(&s.id), "kind": s.kind, "title": s.title, "summary": s.summary, "tags": s.tags,
            "version": s.version, "ref": format!("v{}", s.version), "from": { "handle": s.handle },
            "source": Self::source(s), "updatedAt": s.updated_at,
        })
    }

    /// `grid://<handle>/<key>[#cap]` as `who` asks for it (ADR-0130): None, the same as no such name, unless it's
    /// published, `who` shared it, `cap` is its capability, or it's for people or teams and `who` is one of them. An
    /// unlisted `anyone` or `password` share needs the cap: its password is asked only once the name has opened.
    pub fn named(&self, handle: &str, key: &str, who: Option<&UserRow>, cap: Option<&str>) -> Result<Option<Rec>> {
        let Some(s) = self.by_name(handle, key)? else { return Ok(None) };
        let opens = s.published
            || who.is_some_and(|u| u.id == s.user_id)
            || cap.is_some_and(|c| cap_is(&s.cap, c))
            || (matches!(s.access.as_str(), "people" | "teams") && self.may_read(&s, who, None)?.is_ok());
        Ok(opens.then_some(s))
    }

    /// Whether `who` (signed in, or not) with `password` may read `s`; why not otherwise.
    pub fn may_read(&self, s: &Rec, who: Option<&UserRow>, password: Option<&str>) -> Result<std::result::Result<(), Refused>> {
        if who.is_some_and(|u| u.id == s.user_id) {
            return Ok(Ok(()));
        }
        if s.revoked_at.is_some() {
            return Ok(Err(Refused::Gone("revoked")));
        }
        if s.expires_at.is_some_and(|e| e <= self.clock.now_ms()) {
            return Ok(Err(Refused::Gone("expired")));
        }
        let ok = match s.access.as_str() {
            "anyone" => true,
            "people" => who.is_some_and(|u| s.people.iter().any(|p| p == &u.handle)),
            "teams" => match who {
                Some(u) => {
                    let mut any = false;
                    for t in &s.teams {
                        any |= self.teams.role(&u.id, t)?.is_some();
                    }
                    any
                }
                None => false,
            },
            "password" => s.pass.as_deref().zip(password).is_some_and(|(k, p)| check_password(k, p)),
            _ => false,
        };
        Ok(if ok { Ok(()) } else { Err(Refused::Locked(s.access.clone())) })
    }

    /// A version's content (the latest, or `version`). Counts a view unless it's the sharer's own.
    pub fn read(&self, s: &Rec, who: Option<&UserRow>, version: Option<i64>, count: bool) -> Result<std::result::Result<Content, Refused>> {
        let v = version.unwrap_or(s.version);
        let row: Option<(Vec<u8>, String)> = self.db.with(|c| {
            c.query_row("SELECT body, sig FROM share_versions WHERE share_id = ?1 AND version = ?2", params![s.id, v], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()
        })?;
        let Some((body, sig)) = row else { return Ok(Err(Refused::NoVersion)) };
        if count {
            self.count_view(s, who)?;
        }
        Ok(Ok(Content { version: v, body, sig, signer: s.signer.clone().unwrap_or_default() }))
    }

    pub fn count_view(&self, s: &Rec, who: Option<&UserRow>) -> Result<()> {
        if who.is_none_or(|u| u.id != s.user_id) {
            self.db.with(|c| c.execute("UPDATE shares SET views = views + 1 WHERE id = ?1", [&s.id]))?;
        }
        Ok(())
    }

    pub fn count_import(&self, s: &Rec, who: Option<&UserRow>) -> Result<()> {
        if who.is_none_or(|u| u.id != s.user_id) {
            self.db.with(|c| c.execute("UPDATE shares SET imports = imports + 1 WHERE id = ?1", [&s.id]))?;
        }
        Ok(())
    }

    /// The versions there are (newest first): number and when.
    pub fn versions(&self, s: &Rec) -> Result<Vec<Value>> {
        self.db.with(|c| {
            let mut st = c.prepare("SELECT version, created_at, length(body) FROM share_versions WHERE share_id = ?1 ORDER BY version DESC")?;
            st.query_map([&s.id], |r| Ok(json!({ "version": r.get::<_, i64>(0)?, "ref": format!("v{}", r.get::<_, i64>(0)?), "at": r.get::<_, i64>(1)?, "size": r.get::<_, i64>(2)? })))?.collect()
        })
    }

    /// The gallery: published shares anyone may read, matching every word of `q` (title, summary, tags).
    pub fn gallery(&self, q: &str, kind: Option<&str>) -> Result<Vec<Value>> {
        let now = self.clock.now_ms();
        let rows: Vec<Rec> = self.db.with(|c| {
            let mut s = c.prepare(&format!("{SELECT} WHERE s.published = 1 AND s.access = 'anyone' AND s.revoked_at IS NULL AND (s.expires_at IS NULL OR s.expires_at > ?1) ORDER BY s.imports DESC, s.updated_at DESC LIMIT 500"))?;
            s.query_map([now], rec)?.collect()
        })?;
        let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
        Ok(rows
            .iter()
            .filter(|r| kind.is_none_or(|k| r.kind == k))
            .filter(|r| {
                let hay = format!("{} {} {} {}", r.title, r.summary, r.tags.join(" "), r.kind).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .take(100)
            .map(|r| json!({ "card": self.card(r), "imports": r.imports, "views": r.views }))
            .collect())
    }
}
