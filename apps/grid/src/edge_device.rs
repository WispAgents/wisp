//! The device API and its page on the grid's own address (v1 grid/server.js): RFC 8628 sign-in (`/api/device/code`,
//! `/api/device/token`, `/activate`), a device signing itself out (`/api/device/revoke`, ADR-0078), and the dev sign-in.

use http::{HeaderMap, Method};
use serde_json::{Value, json};

use super::{COOKIE, Grid, Who, no, query};
use crate::accounts::Poll;
use crate::error::{Result, bad};
use crate::http::{Resp, cookies, header_str, html, json, parse_body, redirect, safe_path};
use crate::pages::{self, ActivatePage, DeviceAsk, MessagePage, PageUser, Tone};
use crate::rows::ComputerRow;
use crate::trust::{Credential, Reach, may};

impl Grid {
    /// Signing in, on the grid's own address: the device API (RFC 8628 and its computers), `/activate`, `/events`, the
    /// sign-in page, the providers' flows and the dev sign-in. None for any other path.
    pub(super) async fn account_routes(
        &self,
        method: &Method,
        parts: &[&str],
        q: &str,
        h: &HeaderMap,
        bytes: &[u8],
        who: &Who,
    ) -> Option<Result<Resp>> {
        let (m, p) = (method.as_str(), parts);
        let ours = matches!((m, p), ("POST", ["api", "device", "code" | "token" | "revoke"] | ["api", "computers"]))
            || matches!((m, p), ("GET" | "POST", ["activate"]))
            || matches!(
                (m, p),
                ("GET", ["auth", "dev"] | ["events" | "signin"] | ["auth", _, "start" | "callback"] | ["api", "me" | "computers"])
            );
        if !ours {
            return None;
        }
        Some(self.account_route(method, parts, q, h, bytes, who).await)
    }

    async fn account_route(&self, method: &Method, parts: &[&str], q: &str, h: &HeaderMap, bytes: &[u8], who: &Who) -> Result<Resp> {
        match (method.as_str(), parts) {
            ("POST", ["api", "device", "code"]) => {
                let b = parse_body(h, bytes)?;
                let name = b.get("name").and_then(Value::as_str).unwrap_or_default();
                let mut v = self.accounts.device_code(name)?;
                let code = v["user_code"].as_str().unwrap_or_default().to_owned();
                v["verification_uri"] = json!(format!("{}/activate", self.cfg.public_url));
                v["verification_uri_complete"] = json!(format!("{}/activate?code={code}", self.cfg.public_url));
                Ok(json(200, &v))
            }
            ("POST", ["api", "device", "token"]) => self.device_token(h, bytes),
            ("POST", ["api", "device", "revoke"]) => self.device_revoke(h),
            ("GET" | "POST", ["activate"]) => self.activate(method, h, bytes, q),
            ("GET", ["auth", "dev"]) => self.dev_sign_in(q),
            ("GET", ["events"]) => Ok(self.events(match who {
                Who::Session(u) => Some(u),
                _ => None,
            })),
            ("GET", ["signin"]) => Ok(self.signin_route(h, q)),
            ("GET", ["auth", id, "start"]) => self.auth_start(h, id, q).await,
            ("GET", ["auth", id, "callback"]) => Ok(self.auth_callback(h, id, q).await),
            ("GET", ["api", "me"]) => {
                let Who::Device(u, _) = who else { return Err(bad("Sign in first.", 401)) };
                let computers: Vec<Value> = self.accounts.computers(&u.id)?.iter().map(|c| self.computer_view(c)).collect();
                Ok(json(
                    200,
                    &json!({ "user": { "handle": u.handle, "name": u.name }, "instance": self.instance(u), "computers": computers, "teams": self.teams.of(&u.id)? }),
                ))
            }
            ("POST", ["api", "computers"]) => {
                let Who::Device(user, device) = who else { return Err(bad("Sign in with this computer first.", 401)) };
                if let Some(r) = no(&may(Credential::Device, Reach::GridDevice)) {
                    return Ok(r);
                }
                let body = parse_body(h, bytes)?;
                let comp = self.accounts.add_computer(user, device, body.get("name").and_then(Value::as_str).unwrap_or_default())?;
                Ok(json(200, &self.computer_view(&comp)))
            }
            ("GET", ["api", "computers"]) => {
                let Some(u) = who.user() else { return Err(bad("Sign in first.", 401)) };
                let list: Vec<Value> = self.accounts.computers(&u.id)?.iter().map(|c| self.computer_view(c)).collect();
                Ok(json(200, &json!(list)))
            }
            // `account_routes` sent only what's matched above
            _ => Err(bad("not found", 404)),
        }
    }

    pub(super) fn computer_view(&self, c: &ComputerRow) -> Value {
        json!({ "id": c.id, "name": c.name, "label": c.label, "url": self.cfg.instance_url(&c.label), "online": self.tunnels.online(&c.id) })
    }

    pub(super) fn device_token(&self, h: &HeaderMap, bytes: &[u8]) -> Result<Resp> {
        let b = parse_body(h, bytes)?;
        let code = b.get("device_code").and_then(Value::as_str).unwrap_or_default();
        Ok(match self.accounts.poll(code)? {
            Poll::Token { token, user } => json(
                200,
                &json!({ "token": token, "user": { "handle": user.handle, "name": user.name }, "instance": self.instance(&user) }),
            ),
            other => json(400, &json!({ "error": other.error().unwrap_or("invalid_grant") })),
        })
    }

    /// A device signs itself out (v1 grid/server.js `/api/device/revoke`): the token that asks is the one revoked,
    /// never another (there is no id to name), with its computer and its tunnel. Asking again with the same token
    /// answers 200 `revoked: false` (the first answer may have been lost); a token the grid never issued is 401.
    pub(super) fn device_revoke(&self, h: &HeaderMap) -> Result<Resp> {
        if let Some(r) = no(&may(Credential::Device, Reach::GridDevice)) {
            return Ok(r);
        }
        let Some(r) = self.accounts.revoke_own(header_str(h, "authorization"))? else {
            return Err(bad("This device isn't signed in.", 401));
        };
        for c in &r.computers {
            self.tunnels.close(c);
        }
        Ok(json(200, &json!({ "ok": true, "revoked": r.gone })))
    }

    /// `/activate`: a signed-in person allows (or declines) a device's code.
    pub(super) fn activate(&self, method: &Method, h: &HeaderMap, bytes: &[u8], q: &str) -> Result<Resp> {
        let cookie = cookies(h).get(COOKIE).cloned().unwrap_or_default();
        let Some(user) = self.accounts.session(&cookie)? else {
            let back = format!("/activate?{q}");
            let link = if self.cfg.auth.dev && self.signin.providers.is_empty() {
                format!("/auth/dev?next={}", crate::http::encode_component(&back))
            } else {
                format!("/signin?return={}", crate::http::encode_component(&back))
            };
            return Ok(html(
                401,
                pages::message_page(
                    &self.page,
                    &MessagePage {
                        title: "Sign in first".into(),
                        text: "Sign in to the grid to let a device in.".into(),
                        link: Some((link, "Sign in".into())),
                        tone: Tone::Plain,
                    },
                ),
                &[],
            ));
        };
        let pu = PageUser { handle: user.handle.clone(), name: user.name.clone(), ..PageUser::default() };
        let csrf = Self::csrf(&cookie);
        if *method == Method::POST {
            return self.activate_answer(h, bytes, &user.id, &csrf);
        }
        let code = query(q, "code").unwrap_or_default();
        let device = if code.is_empty() {
            None
        } else {
            self.accounts.pending(&code)?.map(|name| DeviceAsk { name, user_code: crate::accounts::normal_code(&code) })
        };
        let error = if !code.is_empty() && device.is_none() {
            "That code expired or was already used. Start again on the device.".to_owned()
        } else {
            String::new()
        };
        Ok(html(200, pages::activate_page(&self.page, &pu, &ActivatePage { code, device, app: false, csrf, error }), &[]))
    }

    /// `POST /activate`: the person's Allow or Don't allow for a device's code (with the page's CSRF token).
    fn activate_answer(&self, h: &HeaderMap, bytes: &[u8], user_id: &str, csrf: &str) -> Result<Resp> {
        let b = parse_body(h, bytes)?;
        let field = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        if field("csrf") != *csrf {
            return Err(bad("That form is stale. Go back and try again.", 403));
        }
        let allow = field("answer") == "allow";
        self.accounts.decide(&field("code"), user_id, allow)?;
        let text = if allow { "It's in. You can close this page and go back to it." } else { "It wasn't let in." };
        Ok(html(
            200,
            pages::message_page(
                &self.page,
                &MessagePage {
                    title: if allow { "Device connected".into() } else { "Not allowed".into() },
                    text: text.into(),
                    link: None,
                    tone: Tone::Plain,
                },
            ),
            &[],
        ))
    }

    /// `/auth/dev?handle=…&next=…`: development sign-in (only with `auth.dev`).
    pub(super) fn dev_sign_in(&self, q: &str) -> Result<Resp> {
        if !self.cfg.auth.dev {
            return Err(bad("This grid signs in with a provider, not by handle.", 404));
        }
        let Some(handle) = query(q, "handle").filter(|h| !h.is_empty()) else {
            let next = query(q, "next").unwrap_or_default();
            let body = format!(
                r#"<form method="get" action="/auth/dev"><input class="input" name="handle" placeholder="handle" autofocus required><input type="hidden" name="next" value="{}"><button class="primary tall">Sign in</button></form>"#,
                pages::esc(&next)
            );
            return Ok(html(
                200,
                pages::message_page(
                    &self.page,
                    &MessagePage { title: "Sign in (development)".into(), text: String::new(), link: None, tone: Tone::Plain },
                )
                .replace("</h1>", &format!("</h1>{body}")),
                &[],
            ));
        };
        let user = self.accounts.dev_user(&handle)?;
        let t = self.accounts.new_session(&user.id)?;
        let next = query(q, "next").unwrap_or_default();
        let mut r = redirect(safe_path(&next, "/"), &[]);
        crate::http::add_cookie(
            &mut r,
            &crate::http::cookie(
                COOKIE,
                &t,
                crate::http::CookieOptions { secure: self.cfg.secure, max_age: Some(30 * 86_400), path: None, framed: false },
            ),
        );
        Ok(r)
    }
}
