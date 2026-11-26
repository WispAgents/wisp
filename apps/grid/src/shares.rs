//! Shares (ADR-0038, v1 grid/shares.js): everything shared through wisp Cloud, one store for every kind
//! (a task, a setup or module, crew, a workflow, an app, a plugin, a doc, a report, a My Stuff item).
//!
//! - **Versions:** a share is `(user, key)`, at one link. Sharing the same key again with other content makes
//!   version n+1; the same content changes nothing. `?ref=v2` reads that version.
//! - **Signed:** each version carries the sharer's ed25519 signature over its bytes and the key that made
//!   it. The grid checks it, and pins the key at the first version: a later version must be signed by
//!   the same key, so nobody but the sharer's wisp can update a link. A receiver checks it again.
//! - **Access:** `anyone` with the link (the default), `people` (handles, signed in), `teams` (members of
//!   the teams named), or `password` (kept as v1's scrypt `salt:hash`, compared in constant time). Expiry
//!   and revoke answer 410. A refused reader learns only that it's locked, and how it opens.
//! - **Counts:** a read of the page or the content is a view, an import an import; the sharer's own
//!   reads count for nothing.
//! - **Gallery:** a share its sharer published (and whose access is `anyone`) is listed and searchable.
//! - **Names** (ADR-0130): a published share opens by name, `grid://<handle>/<key>`. One that isn't opens by its id
//!   (`/s/<id>`), or by name with its capability, `grid://<handle>/<key>#<cap>`: keys are guessable (`crew`, a
//!   setup's name), so a name alone must not open an unlisted link. A name refused answers exactly as a missing one.
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::sha256_hex;
use crate::clock::SharedClock;
use crate::db::Db;
use crate::error::{Result, bad};
use crate::rows::UserRow;
use crate::teams::Teams;

#[path = "shares_pass.rs"]
mod pass;
#[path = "shares_read.rs"]
mod read;
pub use pass::{check_password, hash_password};

/// What a share may be.
pub const KINDS: &[&str] = &["task", "setup", "module", "crew", "workflow", "agent", "app", "plugin", "doc", "report", "stuff"];
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const PER_USER: i64 = 200;
pub const PASSWORD_MAX: usize = 128;

/// Who may read a share.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Access {
    /// `anyone` | `people` | `teams` | `password`
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub people: Vec<String>,
    #[serde(default)]
    pub teams: Vec<String>,
    #[serde(default)]
    pub password: Option<String>,
}

/// What a sharer's wisp sends.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Put {
    pub kind: String,
    /// what it is on the sharer's wisp (a task id, a setup's name), so sharing it again updates the link
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// the content, base64: the exact bytes signed
    pub body: String,
    /// the sharer's ed25519 public key and signature, base64
    pub signer: String,
    pub sig: String,
    #[serde(default)]
    pub access: Option<Access>,
    /// ms since the epoch; `0` clears it
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub publish: Option<bool>,
}

/// A change to a share's settings (no new content).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    #[serde(default)]
    pub access: Option<Access>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub publish: Option<bool>,
}

/// One share, as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Rec {
    pub id: String,
    pub user_id: String,
    pub handle: String,
    pub key: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub version: i64,
    pub access: String,
    pub people: Vec<String>,
    pub teams: Vec<String>,
    pub pass: Option<String>,
    pub expires_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub views: i64,
    pub imports: i64,
    pub signer: Option<String>,
    pub published: bool,
    /// 128 random bits, hex: what opens it by name while it isn't published (ADR-0130)
    pub cap: String,
    pub size: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A version's content, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub version: i64,
    pub body: Vec<u8>,
    pub sig: String,
    pub signer: String,
}

/// Why a read was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// 410: `revoked` or `expired`
    Gone(&'static str),
    /// 403: locked, and how it opens (`people`, `teams`, `password`)
    Locked(String),
    /// 404
    NoVersion,
}

const SELECT: &str = "SELECT s.id, s.user_id, u.handle, s.key, s.kind, s.title, s.summary, s.tags, s.version, s.access, s.people, s.teams, s.pass, s.expires_at, s.revoked_at, s.views, s.imports, s.signer, s.published, s.size, s.created_at, s.updated_at, s.cap FROM shares s JOIN users u ON u.id = s.user_id";

fn list(v: &str) -> Vec<String> {
    serde_json::from_str(v).unwrap_or_default()
}

fn rec(r: &rusqlite::Row<'_>) -> rusqlite::Result<Rec> {
    Ok(Rec {
        id: r.get(0)?,
        user_id: r.get(1)?,
        handle: r.get(2)?,
        key: r.get(3)?,
        kind: r.get(4)?,
        title: r.get(5)?,
        summary: r.get(6)?,
        tags: list(&r.get::<_, String>(7)?),
        version: r.get(8)?,
        access: r.get(9)?,
        people: list(&r.get::<_, String>(10)?),
        teams: list(&r.get::<_, String>(11)?),
        pass: r.get(12)?,
        expires_at: r.get(13)?,
        revoked_at: r.get(14)?,
        views: r.get(15)?,
        imports: r.get(16)?,
        signer: r.get(17)?,
        published: r.get::<_, i64>(18)? != 0,
        size: r.get(19)?,
        created_at: r.get(20)?,
        updated_at: r.get(21)?,
        cap: r.get::<_, Option<String>>(22)?.unwrap_or_default(),
    })
}

/// What a share is on the sharer's wisp: a task id, `kind.name`, or the crew's `crew:<id>,<id>` (sorted member ids,
/// `wisp_crew_core::source::share_key`), so up to 256 bytes and `:` `,` too. Never `/`: it is a path segment in
/// `grid://<handle>/<key>` and `/api/shares/by/<handle>/<key>`.
pub(crate) fn key_ok(k: &str) -> bool {
    !k.is_empty() && k.len() <= 256 && k.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':' | b','))
}

/// A new share's capability: 128 random bits, hex.
fn new_cap() -> String {
    format!("{:032x}", rand::random::<u128>())
}

/// `v2`, `2` → 2.
pub fn parse_ref(r: &str) -> Option<i64> {
    r.strip_prefix('v').unwrap_or(r).parse().ok().filter(|n| *n > 0)
}

/// Checks `sig` (base64) over `body` against `signer` (a base64 ed25519 public key).
pub fn verify(signer: &str, sig: &str, body: &[u8]) -> bool {
    let (Ok(k), Ok(s)) = (B64.decode(signer.trim()), B64.decode(sig.trim())) else { return false };
    let (Ok(k), Ok(s)) = (<[u8; 32]>::try_from(k.as_slice()), <[u8; 64]>::try_from(s.as_slice())) else { return false };
    VerifyingKey::from_bytes(&k).is_ok_and(|vk| vk.verify(body, &Signature::from_bytes(&s)).is_ok())
}

#[derive(Debug, Clone)]
pub struct Shares {
    pub db: Arc<Db>,
    pub clock: SharedClock,
    pub teams: Teams,
    /// `<publicUrl>`, for links
    pub public_url: String,
}

impl Shares {
    pub fn url(&self, id: &str) -> String {
        format!("{}/s/{id}", self.public_url)
    }

    /// What the sharer sees: everything but the password.
    pub fn view(&self, s: &Rec) -> Value {
        json!({
            "id": s.id, "url": self.url(&s.id), "key": s.key, "kind": s.kind, "title": s.title, "summary": s.summary,
            "tags": s.tags, "version": s.version, "ref": format!("v{}", s.version),
            "access": { "kind": s.access, "people": s.people, "teams": s.teams },
            "expiresAt": s.expires_at, "revoked": s.revoked_at.is_some(), "views": s.views, "imports": s.imports,
            "published": s.published, "size": s.size, "createdAt": s.created_at, "updatedAt": s.updated_at,
            "source": Self::source(s),
        })
    }

    pub fn get(&self, id: &str) -> Result<Option<Rec>> {
        if id.is_empty() || id.len() > 64 {
            return Ok(None);
        }
        self.db.with(|c| c.query_row(&format!("{SELECT} WHERE s.id = ?1"), [id], rec).optional())
    }

    /// `grid://<handle>/<key>`'s share, whoever asks (the sharer's own update finds it here). A reader goes through
    /// `named`.
    pub fn by_name(&self, handle: &str, key: &str) -> Result<Option<Rec>> {
        self.db.with(|c| c.query_row(&format!("{SELECT} WHERE u.handle = ?1 AND s.key = ?2"), params![handle, key], rec).optional())
    }

    pub fn mine(&self, user_id: &str) -> Result<Vec<Value>> {
        let rows: Vec<Rec> = self.db.with(|c| {
            let mut s = c.prepare(&format!("{SELECT} WHERE s.user_id = ?1 ORDER BY s.updated_at DESC"))?;
            s.query_map([user_id], rec)?.collect()
        })?;
        Ok(rows.iter().map(|r| self.view(r)).collect())
    }

    fn check_access(&self, user: &UserRow, a: &Access) -> Result<(String, String, String, Option<String>)> {
        let kind = if a.kind.is_empty() { "anyone" } else { a.kind.as_str() };
        match kind {
            "anyone" => Ok(("anyone".into(), "[]".into(), "[]".into(), None)),
            "people" => {
                if a.people.is_empty() {
                    return Err(bad("Name the people it's for.", 400));
                }
                Ok(("people".into(), json!(a.people).to_string(), "[]".into(), None))
            }
            "teams" => {
                if a.teams.is_empty() {
                    return Err(bad("Name the teams it's for.", 400));
                }
                for t in &a.teams {
                    if !self.teams.role(&user.id, t)?.is_some_and(crate::teams::Role::shares) {
                        return Err(bad(format!("You can't share to {t}: you'd need to be a member there (not a viewer)."), 403));
                    }
                }
                Ok(("teams".into(), "[]".into(), json!(a.teams).to_string(), None))
            }
            "password" => {
                let p = a.password.as_deref().unwrap_or_default();
                if p.is_empty() || p.len() > PASSWORD_MAX {
                    return Err(bad(format!("A password is 1 to {PASSWORD_MAX} characters."), 400));
                }
                Ok(("password".into(), "[]".into(), "[]".into(), Some(hash_password(p))))
            }
            _ => Err(bad("Access is anyone, people, teams or password.", 400)),
        }
    }

    /// Share (or update) `put.key`: a new version when the content changed.
    pub fn put(&self, user: &UserRow, p: &Put) -> Result<Value> {
        let (body, have) = self.checked(user, p)?;
        let access = p.access.as_ref().map(|a| self.check_access(user, a)).transpose()?;
        let hash = sha256_hex(&B64.encode(&body));
        let now = self.clock.now_ms();
        let title: String = p.title.trim().chars().take(200).collect();
        let title = if title.is_empty() { p.key.clone() } else { title };
        let summary: String = p.summary.chars().take(500).collect();
        let tags: Vec<String> =
            p.tags.iter().map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty() && t.len() <= 30).take(12).collect();
        let id = have.as_ref().map_or_else(|| crate::accounts::token("").chars().take(16).collect(), |h| h.id.clone());
        let expires = p.expires_at.map(|e| (e > 0).then_some(e));
        self.db.tx(|tx| {
            if have.is_none() {
                tx.execute(
                    "INSERT INTO shares (id, user_id, key, title, pass, size, meta, created_at, updated_at, kind, version, signer, summary, tags, cap) VALUES (?1, ?2, ?3, ?4, NULL, ?5, '{}', ?6, ?6, ?7, 0, ?8, ?9, ?10, ?11)",
                    params![id, user.id, p.key, title, i64::try_from(body.len()).unwrap_or(i64::MAX), now, p.kind, p.signer.trim(), summary, json!(tags).to_string(), new_cap()],
                )?;
            }
            let last: Option<String> = tx.query_row("SELECT hash FROM share_versions WHERE share_id = ?1 ORDER BY version DESC LIMIT 1", [&id], |r| r.get(0)).optional()?;
            if last.as_deref() != Some(hash.as_str()) {
                let v: i64 = tx.query_row("SELECT version FROM shares WHERE id = ?1", [&id], |r| r.get(0))?;
                tx.execute("INSERT INTO share_versions (share_id, version, body, hash, sig, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![id, v + 1, body, hash, p.sig.trim(), now])?;
                tx.execute("UPDATE shares SET version = ?1, size = ?2, updated_at = ?3 WHERE id = ?4", params![v + 1, i64::try_from(body.len()).unwrap_or(i64::MAX), now, id])?;
            }
            // sharing again brings back a revoked link
            tx.execute("UPDATE shares SET title = ?1, summary = ?2, tags = ?3, kind = ?4, revoked_at = NULL WHERE id = ?5", params![title, summary, json!(tags).to_string(), p.kind, id])?;
            if let Some((kind, people, teams, pass)) = &access {
                tx.execute("UPDATE shares SET access = ?1, people = ?2, teams = ?3, pass = ?4 WHERE id = ?5", params![kind, people, teams, pass, id])?;
            }
            if let Some(e) = expires {
                tx.execute("UPDATE shares SET expires_at = ?1 WHERE id = ?2", params![e, id])?;
            }
            if let Some(pb) = p.publish {
                tx.execute("UPDATE shares SET published = ?1 WHERE id = ?2", params![i64::from(pb), id])?;
            }
            Ok(())
        })?;
        let r = self.get(&id)?.ok_or_else(|| bad("The share wasn't kept.", 500))?;
        Ok(self.view(&r))
    }

    /// A share as sent, checked: its kind and key, its content (base64, at most `MAX_BYTES`), its signature, the key
    /// pinned at its first version, and room for one more. The content, and the share it updates if any.
    fn checked(&self, user: &UserRow, p: &Put) -> Result<(Vec<u8>, Option<Rec>)> {
        if !KINDS.contains(&p.kind.as_str()) {
            return Err(bad(format!("A share is one of: {}.", KINDS.join(", ")), 400));
        }
        if !key_ok(&p.key) {
            return Err(bad("Bad share key.", 400));
        }
        let body = B64.decode(p.body.trim()).map_err(|_| bad("The content isn't base64.", 400))?;
        if body.len() > MAX_BYTES {
            return Err(bad(format!("A share is at most {} MB.", MAX_BYTES / (1024 * 1024)), 413));
        }
        if !verify(&p.signer, &p.sig, &body) {
            return Err(bad("The signature doesn't match the content.", 400));
        }
        let have = self.by_name(&user.handle, &p.key)?;
        if let Some(h) = &have
            && h.signer.as_deref().is_some_and(|k| k != p.signer.trim())
        {
            return Err(bad("This share was signed by another key: only the wisp that shared it can update it.", 403));
        }
        if have.is_none() {
            let n: i64 = self.db.with(|c| c.query_row("SELECT count(*) FROM shares WHERE user_id = ?1", [&user.id], |r| r.get(0)))?;
            if n >= PER_USER {
                return Err(bad(format!("You have {PER_USER} shares, as many as this grid keeps. Revoke one first."), 409));
            }
        }
        Ok((body, have))
    }

    fn own(&self, user: &UserRow, id: &str) -> Result<Rec> {
        self.get(id)?.filter(|s| s.user_id == user.id).ok_or_else(|| bad("You have no share with that id.", 404))
    }

    /// Change who may read it, its expiry, whether it's in the gallery.
    pub fn patch(&self, user: &UserRow, id: &str, p: &Patch) -> Result<Value> {
        let s = self.own(user, id)?;
        let access = p.access.as_ref().map(|a| self.check_access(user, a)).transpose()?;
        self.db.tx(|tx| {
            if let Some((kind, people, teams, pass)) = &access {
                tx.execute(
                    "UPDATE shares SET access = ?1, people = ?2, teams = ?3, pass = ?4 WHERE id = ?5",
                    params![kind, people, teams, pass, s.id],
                )?;
            }
            if let Some(e) = p.expires_at {
                tx.execute("UPDATE shares SET expires_at = ?1 WHERE id = ?2", params![(e > 0).then_some(e), s.id])?;
            }
            if let Some(pb) = p.publish {
                tx.execute("UPDATE shares SET published = ?1 WHERE id = ?2", params![i64::from(pb), s.id])?;
            }
            Ok(())
        })?;
        Ok(self.view(&self.own(user, id)?))
    }

    /// The link stops working (sharing it again brings it back, at the next version).
    pub fn revoke(&self, user: &UserRow, id: &str) -> Result<Value> {
        let s = self.own(user, id)?;
        let now = self.clock.now_ms();
        self.db.with(|c| c.execute("UPDATE shares SET revoked_at = ?1, published = 0 WHERE id = ?2", params![now, s.id]))?;
        Ok(self.view(&self.own(user, id)?))
    }
}

#[cfg(test)]
#[path = "shares_tests.rs"]
mod tests;
