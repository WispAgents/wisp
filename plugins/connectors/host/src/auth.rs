//! A signed-in connector's token, kept fresh (v1 connector-auth.js `auth.token`): the stored one while it works,
//! else refreshed with the stored refresh token first. `connectors.call`, a check and `connectors.token` all ask
//! here; a refresh also runs a minute before the token ends, on the plugin's scope (switching it off cancels it).
//! Refreshing runs once per connector however many ask at once, and the run cache is read again after it, so a
//! new run gets the new bearer. A sign-in it can't renew (no refresh token, refused, no network) marks the row
//! "needs sign-in", kept and published as a check is, so the row offers Sign in without a check again.

use std::sync::atomic::Ordering;

use wisp_connectors_api::{OAuthMeta, ProbeResult, TokenOut, secrets_ns};
use wisp_connectors_core::oauth::{self, TokenStep, Tokens};
use wisp_kernel::services::{NetRequest, net, secrets};
use wisp_kernel::{Code, Ctx, Result, WireError};

use crate::pane::{Pane, lock};

/// The proactive refresh waits at least this long: a token that lives under a minute would refresh in a loop.
const MIN_WAIT_MS: i64 = 30_000;

fn now(ctx: &Ctx) -> i64 {
    ctx.clock().map_or(0, |k| k.now_ms())
}

/// `connector:<id>:<what>`
fn key(id: &str, what: &str) -> String {
    format!("{}:{what}", secrets_ns(id))
}

/// A form to the token endpoint (a code or a refresh token) → the tokens, or why not in the server's words.
pub(crate) async fn token_request(ctx: &Ctx, url: &str, form: &[(String, String)]) -> Result<Tokens> {
    let r = net(ctx)?
        .fetch(NetRequest {
            method: "POST".into(),
            url: url.to_owned(),
            headers: vec![
                ("content-type".into(), "application/x-www-form-urlencoded".into()),
                ("accept".into(), "application/json".into()),
            ],
            body: oauth::form_encode(form).into_bytes(),
            timeout_ms: 15_000,
        })
        .await?;
    Ok(oauth::token_reply(r.status, &String::from_utf8_lossy(&r.body))?)
}

impl Pane {
    pub(crate) async fn stored_token(ctx: &Ctx, id: &str) -> Option<String> {
        secrets(ctx).ok()?.get(&key(id, "token")).await.ok().flatten().filter(|t| !t.is_empty())
    }

    /// The bearer for `id`: the stored one while it's fresh, else refreshed first. `refused`: the token the server
    /// just refused (401/403), refreshed even if it hasn't run out, unless another caller already replaced it.
    pub(crate) async fn fresh_token(&self, ctx: &Ctx, id: &str, refused: Option<&str>) -> Result<String> {
        let gate = lock(&self.refreshing).entry(id.to_owned()).or_default().clone();
        let guard = gate.lock().await;
        // read under the gate: a caller that waited on a refresh sees its token and its new end, and skips its own
        let s = secrets(ctx)?;
        let access = Self::stored_token(ctx, id).await;
        let refresh = s.get(&key(id, "refresh")).await.ok().flatten().filter(|t| !t.is_empty());
        let meta = Self::oauth_of(ctx, id).await;
        let force = refused.is_some_and(|r| access.as_deref() == Some(r));
        let at = now(ctx);
        let next = match (oauth::token_step(meta.as_ref(), access, refresh, at, force), meta) {
            (TokenStep::Use(t), _) => return Ok(t),
            (TokenStep::Refresh(r), Some(o)) => self.renew(ctx, id, &o, &r, at).await,
            _ => Err(WireError::forbidden("its sign-in ran out")),
        };
        drop(guard);
        match next {
            Ok((t, o)) => {
                self.runs.rebuild().await;
                self.refresh_later(ctx, id, &o);
                Ok(t)
            }
            Err(e) => Err(self.needs_signin(ctx, id, &e.message).await),
        }
    }

    /// The refresh itself: the new token, the rotated refresh token (a reply without one keeps the old) and when it
    /// ends, stored. The sign-in's time stays: a refresh isn't a sign-in.
    async fn renew(&self, ctx: &Ctx, id: &str, o: &OAuthMeta, refresh: &str, at: i64) -> Result<(String, OAuthMeta)> {
        let s = secrets(ctx)?;
        let secret = s.get(&key(id, "client_secret")).await.ok().flatten();
        let t = token_request(ctx, &o.token, &oauth::refresh_form(o, refresh, secret.as_deref())).await?;
        s.set(&key(id, "token"), &t.access).await?;
        if let Some(rf) = &t.refresh {
            s.set(&key(id, "refresh"), rf).await?;
        }
        let next = oauth::after_refresh(o, &t, at);
        s.set(&key(id, "oauth"), &serde_json::to_string(&next).unwrap_or_default()).await?;
        Ok((t.access, next))
    }

    /// The row says it needs a sign-in, kept as a check's outcome is (its row outlives a restart) and published; the
    /// error the caller gets.
    pub(crate) async fn needs_signin(&self, ctx: &Ctx, id: &str, why: &str) -> WireError {
        let error = format!("needs sign-in: {why}");
        let r = ProbeResult { at: now(ctx), ok: false, error: Some(error.clone()), auth: true, ..ProbeResult::default() };
        lock(&self.tests).insert(id.to_owned(), r);
        self.publish(ctx).await;
        WireError::forbidden(error)
    }

    /// A refresh a minute before `o`'s token ends; the one armed before for `id` stands down.
    pub(crate) fn refresh_later(&self, ctx: &Ctx, id: &str, o: &OAuthMeta) {
        let mine = self.timer_seq.fetch_add(1, Ordering::Relaxed) + 1;
        lock(&self.timers).insert(id.to_owned(), mine);
        let (Some(at), Ok(clock), Some(me)) = (oauth::refresh_at(o), ctx.clock(), self.me.upgrade()) else { return };
        let wait = u64::try_from((at - clock.now_ms()).max(MIN_WAIT_MS)).unwrap_or(0);
        let (cx, id) = (ctx.clone(), id.to_owned());
        ctx.spawn(async move {
            clock.sleep(wait).await;
            if lock(&me.timers).get(&id) != Some(&mine) {
                return;
            }
            if let Err(e) = me.fresh_token(&cx, &id, None).await {
                tracing::debug!(connector = %id, error = %e.message, "connectors: the refresh ahead of the token's end");
            }
        });
    }

    /// Signed out or removed: no refresh is waiting for it.
    pub(crate) fn forget_token(&self, id: &str) {
        lock(&self.timers).remove(id);
    }

    /// After a start: a token that ran out while wisp was off is refreshed now, the others ahead of their end.
    pub(crate) async fn keep_fresh(&self, ctx: &Ctx) {
        for (id, c) in self.specs().await {
            let Some(o) = Self::oauth_of(ctx, &id).await.filter(|_| c.oauth == Some(true)) else { continue };
            if oauth::refresh_at(&o).is_some_and(|t| t <= now(ctx)) {
                if let Err(e) = self.fresh_token(ctx, &id, None).await {
                    tracing::debug!(connector = %id, error = %e.message, "connectors: a token that ran out while wisp was off");
                }
            } else {
                self.refresh_later(ctx, &id, &o);
            }
        }
    }

    /// `connectors.token`: the connector proxy's bearer, fresh (`force`: the server refused the one it had). None:
    /// only the user can sign in again (the row says so).
    pub(crate) async fn token_for(&self, ctx: &Ctx, id: &str, force: bool) -> Result<TokenOut> {
        let refused = if force { Self::stored_token(ctx, id).await } else { None };
        match self.fresh_token(ctx, id, refused.as_deref()).await {
            Ok(t) => Ok(TokenOut { token: Some(t) }),
            Err(e) if e.code == Code::Forbidden => Ok(TokenOut { token: None }),
            Err(e) => Err(e),
        }
    }
}
