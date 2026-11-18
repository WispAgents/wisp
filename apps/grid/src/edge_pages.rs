//! The grid's signed-in pages and their forms (v1 grid/server.js: `/`, `/account`, `/go`, and the POSTs under
//! them). Only on the grid's own origin, with the session cookie and its CSRF token: never at
//! `<handle>/_grid/*`, which a page a box serves could reach.
//!
//! - `GET /`: home (your cloud wisp, your computers, your published apps), or the public page when signed out.
//! - `GET /account`: signed-in devices, the Telegram bot for `on laptop` (when this grid routes), how you
//!   sign in, sign out, delete.
//! - `GET /go[?computer=]`: to your cloud wisp, or one of your computers.
//! - `POST /instance/stop`, `/computers/:id/remove`, `/sites/:key/remove`, `/devices/:id/revoke`,
//!   `/identities/unlink`, `/channels/route`, `/channels/route/remove`, `/signout[?all=1]`, `/account/delete`, and
//!   `/keys` (make an API key: the page says it once), `/keys/:id/revoke`, `/shares/:id/revoke` (docs/ux/2026-11-18-grid.md):
//!   each answers with a redirect back and a flash line (`?m=…&tone=bad`), as v1 did.

use std::sync::Arc;

use http::HeaderMap;
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use super::{COOKIE, Grid, Who, query};
use crate::boxhost::BoxHost;
use crate::error::{Result, bad};
use crate::http::{CookieOptions, Resp, add_cookie, cookie, cookies, encode_component, html, parse_body, redirect, safe_path};
use crate::pages::{
    self, AccountPage, ComputerView, DeviceView, HomePage, IdentityView, InstanceView, KeyView, MachineView, Offers, PageUser, RouteView,
    SiteView, Start, Tone, Wish,
};
use crate::rows::{BoxUser, UserRow};

/// What a form did, said once on the next page: a cookie the page reads and drops, so a reload doesn't say it again
/// (v1 kept it in the URL, `?m=`, and every reload repeated it).
const FLASH: &str = "wisp_flash";
/// Home's Getting-started steps you took (`a`, `t`) or dismissed (`x`), set by the page's own script (`HOME_JS`).
const START: &str = "wisp_start";

fn page_user(u: &UserRow) -> PageUser {
    PageUser { handle: u.handle.clone(), name: u.name.clone(), email: u.email.clone(), avatar: u.avatar.clone() }
}

impl Grid {
    /// The signed-in pages' routes, or None for any other path.
    pub(super) async fn pages_route(
        self: &Arc<Self>,
        method: &str,
        parts: &[&str],
        q: &str,
        h: &HeaderMap,
        bytes: &[u8],
        who: &Who,
    ) -> Option<Result<Resp>> {
        let user = match who {
            Who::Session(u) => Some(u.clone()),
            _ => None,
        };
        let r = match (method, parts) {
            ("GET", []) => match &user {
                Some(u) => self.home(u, h, q),
                // signed out (or just signed out): the landing with its way in, as v1's `/`
                None => Ok(self.sign_in_page(h, safe_path(&query(q, "return").unwrap_or_default(), ""), "", false, 200)),
            },
            ("GET", ["account"]) => match &user {
                Some(u) => self.account(u, h, q),
                None => Ok(redirect("/signin?return=%2Faccount", &[])),
            },
            ("GET", ["go"]) => match &user {
                Some(u) => self.go(u, q),
                // signed out at the grid too: sign in, then on to the same `/go`
                None => Ok(redirect(&format!("/signin?return={}", encode_component(&format!("/go?{q}"))), &[])),
            },
            (
                "POST",
                ["instance", "stop"]
                | ["computers" | "sites", _, "remove"]
                | ["keys" | "signout"]
                | ["keys" | "shares" | "devices", _, "revoke"]
                | ["identities", "unlink"]
                | ["channels", "route", ..]
                | ["machines", "waitlist"]
                | ["account", "delete"],
            ) => match &user {
                Some(u) => self.form(u, parts, q, h, bytes).await,
                None => Ok(redirect("/signin", &[])),
            },
            _ => return None,
        };
        Some(r)
    }

    fn csrf_of(h: &HeaderMap) -> String {
        Self::csrf(&cookies(h).get(COOKIE).cloned().unwrap_or_default())
    }

    /// Back to `to`, with what happened.
    pub(super) fn back(&self, to: &str, msg: &str, bad_tone: bool) -> Resp {
        let mut r = redirect(to, &[]);
        let v = format!("{}{msg}", if bad_tone { "!" } else { "" });
        add_cookie(&mut r, &cookie(FLASH, &v, CookieOptions { secure: self.cfg.secure, max_age: Some(60), path: None, framed: false }));
        r
    }

    /// The flash a form left for this page (or an old `?m=` link's).
    fn flash_of(h: &HeaderMap, q: &str) -> (String, Tone) {
        if let Some(v) = cookies(h).get(FLASH).filter(|v| !v.is_empty()) {
            return match v.strip_prefix('!') {
                Some(m) => (m.to_owned(), Tone::Bad),
                None => (v.clone(), Tone::Plain),
            };
        }
        (query(q, "m").unwrap_or_default(), if query(q, "tone").as_deref() == Some("bad") { Tone::Bad } else { Tone::Plain })
    }

    /// A page that showed the flash drops it.
    fn shown(&self, h: &HeaderMap, mut r: Resp) -> Resp {
        if cookies(h).contains_key(FLASH) {
            add_cookie(&mut r, &cookie(FLASH, "", CookieOptions { secure: self.cfg.secure, max_age: Some(0), path: None, framed: false }));
        }
        r
    }

    fn devices_of(&self, uid: &str) -> Result<Vec<DeviceView>> {
        self.accounts.db.with(|c| {
            let mut st = c.prepare(
                "SELECT id, name, created_at, last_seen_at FROM devices WHERE user_id = ?1 AND revoked_at IS NULL ORDER BY created_at",
            )?;
            let rows =
                st.query_map([uid], |r| Ok(DeviceView { id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)?, last_seen_at: r.get(3)? }))?;
            rows.collect()
        })
    }

    fn identities_of(&self, uid: &str) -> Result<Vec<IdentityView>> {
        self.accounts.db.with(|c| {
            let mut st = c.prepare("SELECT provider, login, email FROM identities WHERE user_id = ?1 ORDER BY created_at")?;
            let rows = st.query_map([uid], |r| Ok(IdentityView { provider: r.get(0)?, login: r.get(1)?, email: r.get(2)? }))?;
            rows.collect()
        })
    }

    fn computers_of(&self, uid: &str) -> Result<Vec<ComputerView>> {
        Ok(self
            .accounts
            .computers(uid)?
            .iter()
            .map(|c| ComputerView {
                id: c.id.clone(),
                device_id: c.device_id.clone(),
                name: c.name.clone(),
                url: self.cfg.instance_url(&c.label),
                online: self.tunnels.online(&c.id),
                last_seen_at: c.last_seen_at,
            })
            .collect())
    }

    /// Your cloud wisp, as the instances table and the box host have it now.
    fn instance_of(&self, uid: &str) -> Result<InstanceView> {
        let row: Option<(Option<i64>, Option<i64>, Option<i64>)> = self.accounts.db.with(|c| {
            c.query_row("SELECT started_at, stopped_at, next_wake_at FROM instances WHERE user_id = ?1", [uid], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()
        })?;
        let (started_at, stopped_at, next_wake_at) = row.unwrap_or_default();
        Ok(InstanceView { state: self.boxes.state(uid).as_str().to_owned(), started_at, stopped_at, next_wake_at })
    }

    fn home(&self, u: &UserRow, h: &HeaderMap, q: &str) -> Result<Resp> {
        let (flash, tone) = Self::flash_of(h, q);
        let sites = self
            .sites
            .list(&u.id)?
            .iter()
            .map(|s| SiteView {
                key: s.key.clone(),
                title: s.title.clone(),
                url: self.sites.view(s).get("url").and_then(Value::as_str).unwrap_or_default().to_owned(),
                files: u32::try_from(s.files).unwrap_or(0),
                updated_at: Some(s.updated_at),
            })
            .collect();
        let shares = self.shares_of(&u.id)?;
        let p = HomePage {
            user: page_user(u),
            url: self.cfg.instance_url(&u.handle),
            instance: self.instance_of(&u.id)?,
            machine: self.machine_of(&u.id)?,
            devices: self.devices_of(&u.id)?,
            computers: self.computers_of(&u.id)?,
            sites,
            shares,
            start: Start::from_cookie(&cookies(h).get(START).cloned().unwrap_or_default()),
            offers: Offers { sites: self.sites.enabled, sandboxes: self.sandboxes.enabled && self.sandboxes.secrets.is_some() },
            csrf: Self::csrf_of(h),
            flash,
            tone,
            now_ms: self.clock.now_ms(),
        };
        Ok(self.shown(h, html(200, pages::home_page(&self.page, &p), &[])))
    }

    fn account(&self, u: &UserRow, h: &HeaderMap, q: &str) -> Result<Resp> {
        self.account_with(u, h, q, None)
    }

    /// Account, with a key just made said once (`new_key`: its name and the key), never kept for a reload.
    pub(super) fn account_with(&self, u: &UserRow, h: &HeaderMap, q: &str, new_key: Option<(String, String)>) -> Result<Resp> {
        let (flash, tone) = Self::flash_of(h, q);
        let route = if self.routes.on {
            let v = self.routes.view(&u.id).unwrap_or(Value::Null);
            Some(RouteView {
                set: v["set"].as_bool().unwrap_or(false),
                bot: v["bot"].as_str().filter(|b| !b.is_empty()).map(str::to_owned),
                senders: v["senders"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned).or_else(|| s.as_i64().map(|n| n.to_string()))).collect())
                    .unwrap_or_default(),
                moved: v["moved"].as_bool().unwrap_or(false),
                shared: v["shared"].as_bool().unwrap_or(false),
            })
        } else {
            None
        };
        let p = AccountPage {
            user: page_user(u),
            devices: self.devices_of(&u.id)?,
            computers: self.computers_of(&u.id)?,
            identities: self.identities_of(&u.id)?,
            providers: self.page_providers(),
            route,
            keys: self
                .keys
                .list(&u.id)?
                .into_iter()
                .map(|k| KeyView { id: k.id, name: k.name, hint: k.hint, created_at: Some(k.created_at), last_used_at: k.last_used_at })
                .collect(),
            new_key,
            csrf: Self::csrf_of(h),
            flash,
            tone,
            now_ms: self.clock.now_ms(),
        };
        Ok(self.shown(h, html(200, pages::account_page(&self.page, &p), &[])))
    }

    #[allow(clippy::many_single_char_names)] // u the user, q the query, h the headers, b the body
    async fn form(self: &Arc<Self>, u: &UserRow, parts: &[&str], q: &str, h: &HeaderMap, bytes: &[u8]) -> Result<Resp> {
        let b = parse_body(h, bytes)?;
        let field = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        let session = cookies(h).get(COOKIE).cloned().unwrap_or_default();
        if field("csrf") != Self::csrf(&session) {
            return Err(bad("That form is stale. Go back and try again.", 403));
        }
        if let Some(r) = self.link_form(u, parts, q, h, &b) {
            return r;
        }
        match parts {
            ["instance", "stop"] => {
                let was = self.boxes.stop(&u.id, "user").await?;
                Ok(self.back(
                    "/",
                    if was { "Your cloud wisp went to sleep. Opening it wakes it." } else { "Your cloud wisp was already asleep." },
                    false,
                ))
            }
            ["computers", id, "remove"] => self.remove_computer(u, id),
            ["machines", "waitlist"] => self.machines_waitlist(u, &b, h),
            ["devices", id, "revoke"] => {
                let r = self.accounts.revoke_device(&u.id, id)?;
                for c in &r.computers {
                    self.tunnels.close(c);
                }
                let gone = r.gone;
                Ok(self.back(
                    "/account",
                    if gone { "Removed. That device is signed out." } else { "That device was already removed." },
                    !gone,
                ))
            }
            ["identities", "unlink"] => {
                let provider = field("provider");
                let ways = self.identities_of(&u.id)?.len();
                if ways < 2 {
                    return Ok(self.back("/account", "That's your only way in: link another before you disconnect it.", true));
                }
                self.accounts
                    .db
                    .with(|c| c.execute("DELETE FROM identities WHERE user_id = ?1 AND provider = ?2", params![u.id, provider]))?;
                Ok(redirect("/account", &[]))
            }
            ["channels", "route"] => self.route_form(u, &field("token"), &field("senders")).await,
            ["channels", "route", "remove"] => {
                self.routes.remove(u).await?;
                Ok(self.back("/account", "Removed. That bot no longer reaches your computers.", false))
            }
            ["signout"] => self.sign_out(u, &session, query(q, "all").as_deref() == Some("1")),
            ["account", "delete"] => self.delete_account(u, &field("confirm")).await,
            _ => Err(bad("not found", 404)),
        }
    }

    /// `POST /channels/route`: giving the grid a Telegram bot for `on laptop`; what went wrong said on Account.
    async fn route_form(&self, u: &UserRow, token: &str, senders: &str) -> Result<Resp> {
        match self.routes.set(u, token, senders).await {
            Ok(name) => Ok(self.back(
                "/account",
                &if name.is_empty() { "Your bot is connected.".to_owned() } else { format!("@{name} is connected.") },
                false,
            )),
            Err(e) if e.status() == 400 || e.status() == 404 => Ok(self.back("/account", &e.to_string(), true)),
            Err(e) => {
                tracing::warn!("grid: route for {}: {e}", u.handle);
                Ok(self.back("/account", "That didn’t work: Telegram couldn’t be reached. Try again.", true))
            }
        }
    }

    /// `POST /signout[?all=1]`: this browser's session, or every one of this account's.
    fn sign_out(&self, u: &UserRow, session: &str, all: bool) -> Result<Resp> {
        self.accounts.db.with(|c| {
            if all {
                c.execute("DELETE FROM sessions WHERE user_id = ?1", [&u.id])
            } else {
                c.execute(
                    "DELETE FROM sessions WHERE id_hash IN (?1, ?2)",
                    [crate::accounts::sha256_hex(session), crate::accounts::sha256_v1(session)],
                )
            }
        })?;
        Ok(self.cleared(redirect("/", &[])))
    }

    /// `POST /account/delete` with the handle typed: the box and its home, the published apps, then the account.
    async fn delete_account(&self, u: &UserRow, confirm: &str) -> Result<Resp> {
        if confirm.trim().to_lowercase() != u.handle {
            return Ok(self.back("/account", &format!("Type {} to delete your account.", u.handle), true));
        }
        let bu = BoxUser { id: u.id.clone(), handle: u.handle.clone(), sandbox: None, owner: None };
        if let Err(e) = self.boxes.remove(&bu).await {
            tracing::warn!("grid: deleting {}'s box: {e}", u.handle);
        }
        self.sites.remove_all(&u.id)?;
        self.accounts.db.with(|c| c.execute("DELETE FROM users WHERE id = ?1", [&u.id]))?;
        Ok(self.cleared(redirect(&format!("/signin?m={}", encode_component("Your account is deleted.")), &[])))
    }

    /// What the cloud wisp is (the container's limits: v1 showed boxd's defaults when grid.json set none; a process
    /// box has no limits of its own, so it says none), and what more this account waits for.
    fn machine_of(&self, uid: &str) -> Result<MachineView> {
        let c = self.cfg.hostd.container.clone().unwrap_or_default();
        let (cpus, memory) = if self.cfg.containers() {
            (Some(c.cpus.unwrap_or(1.5)), Some(c.memory.unwrap_or_else(|| "1536m".into())))
        } else {
            (None, None)
        };
        let wish = self.accounts.machine_wish(uid)?.map(|w| Wish { wants: w.wants, note: (!w.note.is_empty()).then_some(w.note) });
        Ok(MachineView { cpus, memory, wish })
    }

    /// `POST /machines/waitlist` (the Add a cloud machine dialog, v1): on the list for what's picked, off it with
    /// nothing picked. The dialog asks for JSON; a form without script gets Home back with a note.
    fn machines_waitlist(&self, u: &UserRow, b: &serde_json::Map<String, Value>, h: &HeaderMap) -> Result<Resp> {
        let wants = match b.get("wants") {
            Some(v) => crate::wishes::wants_of(v),
            None => crate::wishes::MACHINE_WANTS.iter().filter(|w| b.contains_key(&format!("want_{w}"))).map(|w| (*w).to_owned()).collect(),
        };
        let note = b.get("note").and_then(Value::as_str).unwrap_or_default();
        let wish = self.accounts.wish_machines(&u.id, &wants, note)?;
        self.notify(&u.id);
        if crate::http::header_str(h, "accept").contains("application/json") {
            return Ok(crate::http::json(200, &serde_json::json!({ "ok": true, "wish": wish })));
        }
        let msg = if wish.is_some() { "You’re on the list. We’ll write when it’s ready." } else { "You’re off the list." };
        Ok(self.back("/", msg, false))
    }

    /// Home's Remove: the computer off the grid, its tunnel closed now (not when it next drops).
    fn remove_computer(&self, u: &UserRow, id: &str) -> Result<Resp> {
        let n = self.accounts.db.with(|c| c.execute("DELETE FROM computers WHERE id = ?1 AND user_id = ?2", params![id, u.id]))?;
        if n > 0 {
            self.tunnels.close(id);
        }
        let msg = if n > 0 { "Taken off the grid. wisp on it keeps running." } else { "That computer was already off the grid." };
        Ok(self.back("/", msg, n == 0))
    }

    /// The response with this browser's session cookie dropped.
    fn cleared(&self, mut r: Resp) -> Resp {
        add_cookie(&mut r, &cookie(COOKIE, "", CookieOptions { secure: self.cfg.secure, max_age: Some(0), path: None, framed: false }));
        r
    }
}
