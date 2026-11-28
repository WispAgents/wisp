//! Users and how they get in (v1 grid/accounts.js): browser sessions, devices (wisp on a computer, with a
//! bearer token from the RFC 8628 device flow), and computers (a device's own wisp, reached at
//! `<name>--<handle>.<domain>` through its tunnel). Tokens are kept as their SHA-256 only.
//!
//! Sign-in with a provider (GitHub, Google, any OIDC issuer: `auth.rs`) ends in [`Accounts::sign_in`]: an
//! identity signs in as the user it's linked to, or a new one; never by matching email (a provider that lets
//! anyone claim an address would hand the account over). `auth.allow` and `auth.signup` say who may come in. A
//! grid run with `auth.dev` also signs anyone in by handle at `/auth/dev` (development and tests only).
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use rusqlite::{OptionalExtension, params};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::clock::SharedClock;
use crate::db::Db;
use crate::error::{Result, bad};
use crate::rows::UserRow;

#[path = "accounts_devices.rs"]
mod devices;
pub use devices::{CODE_TTL_MS, INTERVAL_S, LIVE_CODES, LIVE_CODES_PER_IP, Pending, Poll, Revoked, computer_slug, normal_code};

const DAY: i64 = 24 * 3_600_000;
const SESSION_MS: i64 = 30 * DAY;

pub fn sha256_hex(s: &str) -> String {
    let d = Sha256::digest(s.as_bytes());
    let mut out = String::with_capacity(64);
    for b in d {
        out.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
    }
    out
}

/// A token's hash as v1's grid kept it (SHA-256, base64url). A live grid.db opens as it was (ADR-0014), so its
/// sessions, devices and device codes still hold these: a lookup tries both, and a hit rewrites the row to
/// [`sha256_hex`], so nobody signed in under v1 is signed out at cutover.
pub fn sha256_v1(s: &str) -> String {
    B64URL.encode(Sha256::digest(s.as_bytes()))
}

/// The row `col = kept` found by `token`: when it's v1's hash, move it to v2's (the two never collide: 43
/// characters against 64).
fn upgrade(c: &rusqlite::Connection, table: &str, col: &str, kept: &str, token: &str) -> rusqlite::Result<()> {
    let h = sha256_hex(token);
    if kept != h {
        c.execute(&format!("UPDATE {table} SET {col} = ?1 WHERE {col} = ?2"), params![h, kept])?;
    }
    Ok(())
}

/// `prefix` + 32 random bytes, base64url.
pub fn token(prefix: &str) -> String {
    format!("{prefix}{}", B64URL.encode(rand::random::<[u8; 32]>()))
}

fn hex_id(n: usize) -> String {
    let mut out = String::new();
    for _ in 0..n {
        out.push(char::from(b"0123456789abcdef"[usize::from(rand::random::<u8>() & 15)]));
    }
    out
}

/// Whether `auth.allow` lets an identity in: no list lets everyone in; else its account (`github:octocat`, by login or
/// id), its verified email or that email's domain, or one of its GitHub orgs. An unverified email counts for nothing.
pub fn allowed(auth: &crate::config::AuthConfig, idn: &crate::auth::Identity) -> bool {
    let a = &auth.allow;
    let low = |v: &[String]| v.iter().map(|x| x.trim().to_lowercase()).collect::<Vec<_>>();
    let (emails, domains, orgs, accounts) = (
        low(&a.emails),
        low(&a.domains).into_iter().map(|d| d.trim_start_matches('@').to_owned()).collect::<Vec<_>>(),
        low(&a.github_orgs),
        low(&a.accounts),
    );
    if emails.is_empty() && domains.is_empty() && orgs.is_empty() && accounts.is_empty() {
        return true;
    }
    let me = format!("{}:{}", idn.provider, idn.subject).to_lowercase();
    if accounts.contains(&me) || (!idn.login.is_empty() && accounts.contains(&format!("{}:{}", idn.provider, idn.login).to_lowercase())) {
        return true;
    }
    let email = if idn.email_verified { idn.email.to_lowercase() } else { String::new() };
    if !email.is_empty() && (emails.contains(&email) || email.rsplit_once('@').is_some_and(|(_, d)| domains.iter().any(|x| x == d))) {
        return true;
    }
    !orgs.is_empty() && idn.orgs.as_ref().is_some_and(|o| o.iter().any(|x| orgs.contains(&x.to_lowercase())))
}

/// A free handle from a login: a-z, 0-9 and single dashes, at most 24; a short or reserved one gets a suffix.
pub fn unique_handle(c: &rusqlite::Connection, base: &str) -> rusqlite::Result<String> {
    let mut h = String::new();
    for ch in base.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            h.push(ch);
        } else if !h.ends_with('-') {
            h.push('-');
        }
    }
    let mut h: String = h.trim_matches('-').chars().take(24).collect::<String>().trim_end_matches('-').to_owned();
    let reserved = |x: &str| crate::config::RESERVED.contains(&x);
    if h.len() < 3 || reserved(&h) {
        h = format!("{}-{}", if h.is_empty() { "user" } else { &h }, hex_id(4));
    }
    let taken = |x: &str| -> rusqlite::Result<bool> {
        Ok(c.query_row("SELECT 1 FROM users WHERE handle = ?1", [x], |_| Ok(())).optional()?.is_some() || reserved(x))
    };
    let mut out = h.clone();
    let mut n = 2;
    while taken(&out)? {
        out = format!("{h}-{n}");
        n += 1;
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct Accounts {
    pub db: Arc<Db>,
    pub clock: SharedClock,
    /// sign-ups, sign-ins and refusals as usage stats (`telemetry.rs`), when the grid sends them
    pub telemetry: Option<Arc<crate::telemetry::GridTelemetry>>,
}

impl Accounts {
    fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    pub fn user_by_id(&self, id: &str) -> Result<Option<UserRow>> {
        self.db.with(|c| c.query_row(&format!("SELECT {} FROM users WHERE id = ?1", UserRow::COLS), [id], UserRow::from_row).optional())
    }

    pub fn user_by_handle(&self, handle: &str) -> Result<Option<UserRow>> {
        self.db.with(|c| {
            c.query_row(&format!("SELECT {} FROM users WHERE handle = ?1", UserRow::COLS), [handle], UserRow::from_row).optional()
        })
    }

    /// A development sign-in: the user with this handle, made if there is none.
    pub fn dev_user(&self, handle: &str) -> Result<UserRow> {
        let h = handle.trim().to_ascii_lowercase();
        if h.is_empty() || h.len() > 30 || h.contains("--") || !crate::hosts::is_handle_label(&h) || h.starts_with('-') || h.ends_with('-')
        {
            return Err(bad("A handle is a-z, 0-9 and single dashes.", 400));
        }
        if let Some(u) = self.user_by_handle(&h)? {
            // a team's hub is an account nobody signs in as, not even here (ADR-0148)
            let hub = self.db.with(|c| c.query_row("SELECT 1 FROM teams WHERE hub_user_id = ?1", [&u.id], |_| Ok(())).optional())?;
            if hub.is_some() {
                return Err(bad(format!("{h} is a team's hub: nobody signs in as it."), 403));
            }
            return Ok(u);
        }
        let id = hex_id(12);
        let now = self.now();
        self.db.with(|c| c.execute("INSERT INTO users (id, handle, name, created_at) VALUES (?1, ?2, ?2, ?3)", params![id, h, now]))?;
        self.user_by_id(&id)?.ok_or_else(|| bad("The user wasn't made.", 500))
    }

    // --- sign-in with a provider -------------------------------------------------------------------

    /// The user an identity signs in as: its link, the signed-in user linking it (`link_to`), or a new user when
    /// sign-ups are open and the allow list lets it in.
    pub fn sign_in(&self, auth: &crate::config::AuthConfig, idn: &crate::auth::Identity, link_to: Option<&str>) -> Result<UserRow> {
        if idn.subject.is_empty() {
            return Err(bad("The provider didn't say who you are.", 400));
        }
        let now = self.now();
        let found: Option<String> = self.db.with(|c| {
            c.query_row("SELECT user_id FROM identities WHERE provider = ?1 AND subject = ?2", params![idn.provider, idn.subject], |r| {
                r.get(0)
            })
            .optional()
        })?;
        let opt = |s: &str| (!s.is_empty()).then(|| s.to_owned());
        if let Some(uid) = link_to {
            if found.as_deref().is_some_and(|f| f != uid) {
                return Err(bad("That account already signs in to another user here.", 409));
            }
            if found.is_none() {
                self.db.with(|c| {
                    c.execute(
                        "INSERT INTO identities (provider, subject, user_id, email, login, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![idn.provider, idn.subject, uid, opt(&idn.email), opt(&idn.login), now],
                    )
                })?;
            }
            self.audit(Some(uid), "identity.link", &idn.provider)?;
            return self.user_by_id(uid)?.ok_or_else(|| bad("That user is gone.", 404));
        }
        if let Some(uid) = found {
            let u = self
                .user_by_id(&uid)?
                .filter(|u| u.disabled == 0)
                .ok_or_else(|| bad("This account is turned off. Ask the grid's operator.", 403))?;
            if !allowed(auth, idn) {
                return Err(bad("This grid no longer lets this account in.", 403));
            }
            self.db.with(|c| {
                c.execute(
                    "UPDATE identities SET email = ?1, login = ?2 WHERE provider = ?3 AND subject = ?4",
                    params![opt(&idn.email), opt(&idn.login), idn.provider, idn.subject],
                )?;
                if u.avatar.as_deref().unwrap_or_default().is_empty() && !idn.avatar.is_empty() {
                    c.execute("UPDATE users SET avatar = ?1 WHERE id = ?2", params![idn.avatar, u.id])?;
                }
                Ok(())
            })?;
            self.audit(Some(&u.id), "signin", &idn.provider)?;
            return self.user_by_id(&u.id)?.ok_or_else(|| bad("That user is gone.", 404));
        }
        if auth.signup == "closed" {
            self.audit(None, "signup.refused", &idn.provider)?;
            return Err(bad("Sign-ups are closed on this grid.", 403));
        }
        if !allowed(auth, idn) {
            self.audit(None, "signup.refused", &idn.provider)?;
            return Err(bad("This grid only lets in invited accounts, and this isn't one of them.", 403));
        }
        self.sign_up(idn, now)
    }

    /// A new user for this identity (the checks above passed), with its instance row, and the seat waitlist told when
    /// its verified address was on it.
    fn sign_up(&self, idn: &crate::auth::Identity, now: i64) -> Result<UserRow> {
        let opt = |s: &str| (!s.is_empty()).then(|| s.to_owned());
        let id = hex_id(16);
        let base = if idn.login.is_empty() { idn.email.split('@').next().unwrap_or("user").to_owned() } else { idn.login.clone() };
        self.db.tx(|c| {
            let handle = unique_handle(c, &base)?;
            c.execute(
                "INSERT INTO users (id, handle, email, name, avatar, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, handle, opt(&idn.email), opt(&idn.name), opt(&idn.avatar), now],
            )?;
            c.execute(
                "INSERT INTO identities (provider, subject, user_id, email, login, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![idn.provider, idn.subject, id, opt(&idn.email), opt(&idn.login), now],
            )?;
            c.execute("INSERT INTO instances (user_id, state, created_at) VALUES (?1, 'stopped', ?2)", params![id, now])?;
            // on the seat waitlist with this (verified) address: joined now (v1 accounts.js)
            if idn.email_verified && !idn.email.is_empty() {
                c.execute(
                    "UPDATE waitlist SET joined_at = ?1, user_id = ?2 WHERE email = ?3 AND joined_at IS NULL",
                    params![now, id, idn.email.to_lowercase()],
                )?;
            }
            Ok(())
        })?;
        self.audit(Some(&id), "signup", &idn.provider)?;
        self.user_by_id(&id)?.ok_or_else(|| bad("The user wasn't made.", 500))
    }

    fn audit(&self, user: Option<&str>, action: &str, provider: &str) -> Result<()> {
        let now = self.now();
        let meta = json!({ "provider": provider });
        self.db.with(|c| crate::rows::find::audit(c, user, action, Some(&meta), now))?;
        if let Some(t) = &self.telemetry {
            t.audit(user, action, provider);
        }
        Ok(())
    }

    // --- browser sessions -------------------------------------------------------------------------

    /// A session for this user: the cookie's value (kept hashed).
    pub fn new_session(&self, user_id: &str) -> Result<String> {
        let t = token("");
        let now = self.now();
        self.db.with(|c| {
            c.execute(
                "INSERT INTO sessions (id_hash, user_id, scope, created_at, expires_at, last_seen_at) VALUES (?1, ?2, 'grid', ?3, ?4, ?3)",
                params![sha256_hex(&t), user_id, now, now + SESSION_MS],
            )
        })?;
        Ok(t)
    }

    pub fn session(&self, cookie: &str) -> Result<Option<UserRow>> {
        if cookie.is_empty() {
            return Ok(None);
        }
        let now = self.now();
        let uid: Option<String> = self.db.with(|c| {
            let row: Option<(String, String)> = c
                .query_row(
                    // the grid's own sign-in only: a preview's (`p:<label>`, previews.rs) or an address's (`i:<label>`,
                    // handoff.rs) is good at its own address alone, never here
                    "SELECT id_hash, user_id FROM sessions WHERE id_hash IN (?1, ?2) AND expires_at > ?3 AND scope = 'grid'",
                    params![sha256_hex(cookie), sha256_v1(cookie), now],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((kept, uid)) = row else { return Ok(None) };
            upgrade(c, "sessions", "id_hash", &kept, cookie)?;
            Ok(Some(uid))
        })?;
        match uid {
            Some(u) => Ok(self.user_by_id(&u)?.filter(|u| u.disabled == 0)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
