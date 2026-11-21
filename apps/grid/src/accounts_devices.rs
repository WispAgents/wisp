//! Devices (v1 grid/accounts.js): the RFC 8628 device flow that signs wisp on a computer or a phone in, the device
//! tokens it hands out (kept as their SHA-256), revoking them, and the computers a device adds to the grid.
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use super::{Accounts, DAY, hex_id, sha256_hex, sha256_v1, token, upgrade};
use crate::error::{Result, bad};
use crate::rows::{ComputerRow, DeviceRow, UserRow};

/// How long a device code waits for its approval.
pub const CODE_TTL_MS: i64 = 10 * 60_000;
/// How often a device may poll (RFC 8628 `interval`), in seconds.
pub const INTERVAL_S: i64 = 5;

/// Letters that can't be misread for each other (no vowels: no words), as v1.
const USER_CODE: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ";

/// What revoking a device did: whether it was signed in until now, and the computers that went with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revoked {
    pub gone: bool,
    pub computers: Vec<String>,
}

/// Revokes `user_id`'s device `id` (`by`: `account` from the account page, `device` from the device itself) inside
/// the caller's transaction: the revocation, its audit line (v1's `device.revoke`) and its computer go together.
fn revoke_in(c: &rusqlite::Connection, user_id: &str, id: &str, by: &str, now: i64) -> rusqlite::Result<Revoked> {
    let n =
        c.execute("UPDATE devices SET revoked_at = ?1 WHERE id = ?2 AND user_id = ?3 AND revoked_at IS NULL", params![now, id, user_id])?;
    if n > 0 {
        crate::rows::find::audit(c, Some(user_id), "device.revoke", Some(&json!({ "id": id, "by": by })), now)?;
    }
    let computers = c
        .prepare("DELETE FROM computers WHERE device_id = ?1 AND user_id = ?2 RETURNING id")?
        .query_map(params![id, user_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Revoked { gone: n > 0, computers })
}

/// What polling a device code answers: RFC 8628's error, or the device token once allowed.
#[derive(Debug, Clone, PartialEq)]
pub enum Poll {
    Pending,
    SlowDown,
    Denied,
    Expired,
    /// unknown or already used
    Invalid,
    Token {
        token: String,
        user: UserRow,
    },
}

impl Poll {
    /// The RFC 8628 error code, for everything but a token.
    pub fn error(&self) -> Option<&'static str> {
        match self {
            Self::Pending => Some("authorization_pending"),
            Self::SlowDown => Some("slow_down"),
            Self::Denied => Some("access_denied"),
            Self::Expired => Some("expired_token"),
            Self::Invalid => Some("invalid_grant"),
            Self::Token { .. } => None,
        }
    }
}

/// `abcd efgh`, `ABCDEFGH`, `abcd-efgh` → `ABCD-EFGH`; anything else → "".
pub fn normal_code(c: &str) -> String {
    let s: String = c.chars().filter(char::is_ascii_alphabetic).map(|c| c.to_ascii_uppercase()).collect();
    if s.len() == 8 { format!("{}-{}", &s[..4], &s[4..]) } else { String::new() }
}

/// A computer's label from its name: lowercase words joined by `-`, at most 20 characters, never one that
/// reads as a preview's (`p5173`) or a sandbox's (`sb-<8 hex>`) address.
pub fn computer_slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    let s: String = s.trim_matches('-').chars().take(20).collect();
    let s = s.trim_end_matches('-').to_owned();
    let preview = s.strip_prefix('p').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
    let sandbox =
        s.strip_prefix("sb-").is_some_and(|d| d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    if s.is_empty() {
        "computer".into()
    } else if preview || sandbox {
        format!("{s}-pc")
    } else {
        s
    }
}

impl Accounts {
    // --- device codes (RFC 8628) ------------------------------------------------------------------

    /// A new code for a device called `name`: what the device shows and polls with.
    pub fn device_code(&self, name: &str) -> Result<Value> {
        let n: String = name.trim().chars().take(60).collect();
        let n = if n.is_empty() { "A device".to_owned() } else { n };
        let code = token("");
        let pick = || (0..4).map(|_| char::from(USER_CODE[usize::from(rand::random::<u8>()) % USER_CODE.len()])).collect::<String>();
        let now = self.now();
        let user_code = self.db.with(|c| {
            let mut uc = format!("{}-{}", pick(), pick());
            while c.query_row("SELECT 1 FROM device_codes WHERE user_code = ?1", [&uc], |_| Ok(())).optional()?.is_some() {
                uc = format!("{}-{}", pick(), pick());
            }
            c.execute(
                "INSERT INTO device_codes (code_hash, user_code, name, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![sha256_hex(&code), uc, n, now, now + CODE_TTL_MS],
            )?;
            c.execute("DELETE FROM device_codes WHERE expires_at < ?1", [now - DAY])?;
            Ok(uc)
        })?;
        Ok(json!({ "device_code": code, "user_code": user_code, "expires_in": CODE_TTL_MS / 1000, "interval": INTERVAL_S }))
    }

    /// The device's name for a code still waiting, for the approval page; None when it isn't one.
    pub fn pending(&self, user_code: &str) -> Result<Option<String>> {
        let now = self.now();
        self.db.with(|c| {
            c.query_row(
                "SELECT name FROM device_codes WHERE user_code = ?1 AND expires_at >= ?2 AND consumed_at IS NULL AND approved_at IS NULL AND denied_at IS NULL",
                params![normal_code(user_code), now],
                |r| r.get(0),
            )
            .optional()
        })
    }

    /// The signed-in person allows (or declines) a code.
    pub fn decide(&self, user_code: &str, user_id: &str, allow: bool) -> Result<()> {
        if self.pending(user_code)?.is_none() {
            return Err(bad("That code expired or was already used. Start again on the device.", 400));
        }
        let now = self.now();
        let col = if allow { "approved_at" } else { "denied_at" };
        self.db.with(|c| {
            c.execute(
                &format!("UPDATE device_codes SET {col} = ?1, user_id = ?2 WHERE user_code = ?3"),
                params![now, user_id, normal_code(user_code)],
            )
        })?;
        Ok(())
    }

    /// The device polls: RFC 8628's answer, or its token (once).
    pub fn poll(&self, device_code: &str) -> Result<Poll> {
        type Code = (i64, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<String>, String);
        let h = sha256_hex(device_code);
        let now = self.now();
        let row: Option<Code> = self.db.with(|c| {
            let row = c.query_row("SELECT expires_at, approved_at, denied_at, consumed_at, last_poll_at, user_id, name, code_hash FROM device_codes WHERE code_hash IN (?1, ?2)", params![h, sha256_v1(device_code)], |r| Ok(((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?), r.get::<_, String>(7)?))).optional()?;
            let Some((row, kept)) = row else { return Ok(None) };
            upgrade(c, "device_codes", "code_hash", &kept, device_code)?;
            Ok(Some(row))
        })?;
        let Some((expires, approved, denied, consumed, last_poll, user_id, name)) = row else { return Ok(Poll::Invalid) };
        if consumed.is_some() {
            return Ok(Poll::Invalid);
        }
        if expires < now {
            return Ok(Poll::Expired);
        }
        if denied.is_some() {
            return Ok(Poll::Denied);
        }
        if approved.is_none() {
            let fast = last_poll.is_some_and(|l| now - l < INTERVAL_S * 1000 - 250);
            self.db.with(|c| c.execute("UPDATE device_codes SET last_poll_at = ?1 WHERE code_hash = ?2", params![now, h]))?;
            return Ok(if fast { Poll::SlowDown } else { Poll::Pending });
        }
        let uid = user_id.unwrap_or_default();
        let t = token("wgd_");
        let id = hex_id(12);
        self.db.tx(|tx| {
            tx.execute("UPDATE device_codes SET consumed_at = ?1 WHERE code_hash = ?2", params![now, h])?;
            tx.execute(
                "INSERT INTO devices (id, user_id, name, token_hash, created_at, last_seen_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![id, uid, name, sha256_hex(&t), now],
            )?;
            Ok(())
        })?;
        let user = self.user_by_id(&uid)?.ok_or_else(|| bad("That account is gone.", 410))?;
        Ok(Poll::Token { token: t, user })
    }

    // --- devices ----------------------------------------------------------------------------------

    /// Who holds this device token (`Bearer wgd_…` or the token alone); None when it's unknown or revoked.
    pub fn device(&self, bearer: &str) -> Result<Option<(UserRow, DeviceRow)>> {
        let t = bearer.strip_prefix("Bearer ").or_else(|| bearer.strip_prefix("bearer ")).unwrap_or(bearer).trim();
        if !t.starts_with("wgd_") {
            return Ok(None);
        }
        let d: Option<DeviceRow> = self.db.with(|c| {
            let d = c
                .query_row(
                    &format!("SELECT {} FROM devices WHERE token_hash IN (?1, ?2)", DeviceRow::COLS),
                    params![sha256_hex(t), sha256_v1(t)],
                    DeviceRow::from_row,
                )
                .optional()?;
            if let Some(d) = &d {
                upgrade(c, "devices", "token_hash", &d.token_hash, t)?;
            }
            Ok(d)
        })?;
        let Some(d) = d.filter(|d| d.revoked_at.is_none()) else { return Ok(None) };
        let Some(u) = self.user_by_id(&d.user_id)?.filter(|u| u.disabled == 0) else { return Ok(None) };
        let now = self.now();
        if d.last_seen_at.is_none_or(|l| now - l > 60_000) {
            self.db.with(|c| c.execute("UPDATE devices SET last_seen_at = ?1 WHERE id = ?2", params![now, d.id]))?;
        }
        Ok(Some((u, d)))
    }

    /// The account page's Remove: the device `id` of `user_id`, revoked. A revoked device's computer goes with it:
    /// it can't hold its tunnel any more (the ids returned, for the edge to close their tunnels).
    pub fn revoke_device(&self, user_id: &str, id: &str) -> Result<Revoked> {
        let now = self.now();
        self.db.tx(|tx| Ok(revoke_in(tx, user_id, id, "account", now)?))
    }

    /// A device signing itself out (`POST /api/device/revoke`): the device that holds `bearer`, and only that one,
    /// revoked with its computer in one transaction, audited as the account page's Remove is. The row is found by
    /// the token's hash, revoked or not, so asking again (an answer lost on a bad network) is `gone: false`, not a
    /// refusal; a token the grid never handed out is `None`.
    pub fn revoke_own(&self, bearer: &str) -> Result<Option<Revoked>> {
        let t = bearer.strip_prefix("Bearer ").or_else(|| bearer.strip_prefix("bearer ")).unwrap_or(bearer).trim();
        if !t.starts_with("wgd_") {
            return Ok(None);
        }
        let now = self.now();
        self.db.tx(|tx| {
            let row: Option<(String, String)> = tx
                .query_row("SELECT id, user_id FROM devices WHERE token_hash IN (?1, ?2)", params![sha256_hex(t), sha256_v1(t)], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?;
            let Some((id, user_id)) = row else { return Ok(None) };
            Ok(Some(revoke_in(tx, &user_id, &id, "device", now)?))
        })
    }

    // --- computers --------------------------------------------------------------------------------

    /// The device's computer (one each), added or renamed.
    pub fn add_computer(&self, user: &UserRow, device: &DeviceRow, name: &str) -> Result<ComputerRow> {
        let n: String = name.trim().chars().take(40).collect();
        let n = if n.is_empty() { "Computer".to_owned() } else { n };
        let have: Option<ComputerRow> = self.db.with(|c| {
            c.query_row(&format!("SELECT {} FROM computers WHERE device_id = ?1", ComputerRow::COLS), [&device.id], ComputerRow::from_row)
                .optional()
        })?;
        if let Some(mut h) = have {
            if h.name != n {
                self.db.with(|c| c.execute("UPDATE computers SET name = ?1 WHERE id = ?2", params![n, h.id]))?;
                h.name = n;
            }
            return Ok(h);
        }
        let slug = computer_slug(&n);
        let now = self.now();
        let id = hex_id(12);
        let label = self.db.with(|c| {
            let taken = |l: &str| -> rusqlite::Result<bool> {
                Ok(c.query_row("SELECT 1 FROM computers WHERE label = ?1", [l], |_| Ok(())).optional()?.is_some()
                    || c.query_row("SELECT 1 FROM users WHERE handle = ?1", [l], |_| Ok(())).optional()?.is_some())
            };
            let mut label = format!("{slug}--{}", user.handle);
            let mut k = 2;
            while taken(&label)? {
                label = format!("{slug}-{k}--{}", user.handle);
                k += 1;
            }
            c.execute(
                "INSERT INTO computers (id, user_id, device_id, name, label, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, user.id, device.id, n, label, now],
            )?;
            Ok(label)
        })?;
        Ok(ComputerRow { id, user_id: user.id.clone(), device_id: device.id.clone(), name: n, label, created_at: now, last_seen_at: None })
    }

    pub fn computers(&self, user_id: &str) -> Result<Vec<ComputerRow>> {
        self.db.with(|c| crate::rows::find::computers_of(c, user_id))
    }

    pub fn computer(&self, id: &str) -> Result<Option<ComputerRow>> {
        self.db.with(|c| crate::rows::find::computer_by_id(c, id))
    }

    pub fn computer_by_label(&self, label: &str) -> Result<Option<ComputerRow>> {
        self.db.with(|c| crate::rows::find::computer_by_label(c, &label.to_ascii_lowercase()))
    }

    pub fn seen_computer(&self, id: &str) -> Result<()> {
        let now = self.now();
        self.db.with(|c| c.execute("UPDATE computers SET last_seen_at = ?1 WHERE id = ?2", params![now, id]))?;
        Ok(())
    }
}
