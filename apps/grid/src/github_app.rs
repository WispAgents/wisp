//! The grid's GitHub App (ADR-0148 §8e): a team installs it on its org, and its hub (and the hub's sandboxes) get
//! short-lived tokens for the repos it was given, minted here with the App's key; GitHub's webhooks for those repos
//! come here, signed, and go on to the team's hub as trigger events.
//!
//! - **Connecting proves the installer.** An installation id is a guessable number, and GitHub's redirect after an
//!   install carries it in the URL: trusted as is, anyone could tie someone else's org to their team. So the App asks
//!   GitHub to authorize the installer during the install (its "Request user authorization (OAuth) during
//!   installation"), the redirect brings a `code`, and the grid trades it for that person's token and checks the
//!   installation is among theirs (`GET /user/installations`). Their token is used for that and dropped.
//! - **The state** ties the redirect to the team and the person who started it, for 15 minutes, once.
//! - **Tokens** last GitHub's hour; one is kept per installation until 5 minutes before its end, so a hub and its
//!   sandboxes asking every few minutes cost one mint an hour.
//! - **Webhooks** are checked against the App's secret (`X-Hub-Signature-256`, HMAC-SHA256) before anything is read.
use std::collections::HashMap;
use std::sync::Mutex;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use hmac::{Hmac, Mac as _};
use rsa::pkcs1::DecodeRsaPrivateKey as _;
use rsa::pkcs8::DecodePrivateKey as _;
use rsa::signature::{SignatureEncoding as _, Signer as _};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::clock::SharedClock;
use crate::config::{Env, GitHubAppConfig, read_secret};
use crate::error::{GridError, Result, bad};

/// How long a connect link's state holds.
const STATE_MS: i64 = 15 * 60_000;
/// A token is GitHub's for an hour: kept until this long before its end.
const TOKEN_MS: i64 = 55 * 60_000;
const SPARE_MS: i64 = 5 * 60_000;

/// A team and the person a connect started for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub team_id: String,
    /// its slug, for the page GitHub's return goes back to
    pub slug: String,
    pub user_id: String,
}

pub struct GitHubApp {
    pub id: String,
    pub slug: String,
    client_id: String,
    client_secret: String,
    key: rsa::pkcs1v15::SigningKey<Sha256>,
    webhook_secret: String,
    pub api: String,
    pub web: String,
    http: reqwest::Client,
    clock: SharedClock,
    states: Mutex<HashMap<String, (Started, i64)>>,
    tokens: Mutex<HashMap<i64, (String, i64)>>,
}

impl std::fmt::Debug for GitHubApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubApp").field("id", &self.id).field("slug", &self.slug).finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn trimmed(s: &str) -> String {
    s.trim_end_matches('/').to_owned()
}

/// The App's private key from its PEM (GitHub gives PKCS#1; PKCS#8 works too).
pub fn signing_key(pem: &str) -> Result<rsa::pkcs1v15::SigningKey<Sha256>> {
    let key = rsa::RsaPrivateKey::from_pkcs1_pem(pem)
        .or_else(|_| rsa::RsaPrivateKey::from_pkcs8_pem(pem))
        .map_err(|_| GridError::Config("githubApp: its private key isn't a PEM RSA key".into()))?;
    Ok(rsa::pkcs1v15::SigningKey::<Sha256>::new(key))
}

/// `X-Hub-Signature-256` (`sha256=<hex>`) checked against `secret`, in constant time.
pub fn signed(secret: &str, body: &[u8], header: &str) -> bool {
    let Some(hex) = header.trim().strip_prefix("sha256=") else { return false };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else { return false };
    mac.update(body);
    let want = crate::secrets::hex(&mac.finalize().into_bytes());
    !secret.is_empty() && subtle::ConstantTimeEq::ct_eq(want.as_bytes(), hex.to_ascii_lowercase().as_bytes()).into()
}

impl GitHubApp {
    /// The App grid.json names, its secrets read; none when it names none.
    pub fn from_config(cfg: Option<&GitHubAppConfig>, env: &Env, clock: SharedClock) -> Result<Option<Self>> {
        let Some(c) = cfg else { return Ok(None) };
        let client_secret =
            read_secret(None, c.client_secret_file.as_deref().map(std::path::Path::new), Some("GRID_GITHUB_APP_CLIENT_SECRET"), env)?;
        let pem = read_secret(None, c.private_key_file.as_deref().map(std::path::Path::new), Some("GRID_GITHUB_APP_PRIVATE_KEY"), env)?;
        let webhook_secret =
            read_secret(None, c.webhook_secret_file.as_deref().map(std::path::Path::new), Some("GRID_GITHUB_APP_WEBHOOK_SECRET"), env)?;
        if client_secret.is_empty() || pem.is_empty() || webhook_secret.is_empty() {
            return Err(GridError::Config(
                "githubApp needs its client secret, private key and webhook secret (GRID_GITHUB_APP_CLIENT_SECRET, _PRIVATE_KEY, _WEBHOOK_SECRET, or their _FILE)".into(),
            ));
        }
        Ok(Some(Self {
            id: c.id.clone(),
            slug: c.slug.clone(),
            client_id: c.client_id.clone(),
            client_secret,
            key: signing_key(&pem)?,
            webhook_secret,
            api: trimmed(c.api.as_deref().unwrap_or("https://api.github.com")),
            web: trimmed(c.web.as_deref().unwrap_or("https://github.com")),
            http: reqwest::Client::builder()
                .user_agent("wisp-grid")
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .map_err(|e| GridError::Config(format!("githubApp: {e}")))?,
            clock,
            states: Mutex::new(HashMap::new()),
            tokens: Mutex::new(HashMap::new()),
        }))
    }

    /// The App's own JWT (RS256, 9 minutes, backdated a minute for clock drift): what asks GitHub as the App.
    pub fn jwt(&self) -> String {
        let now = self.clock.now_ms() / 1000;
        let head = B64URL.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
        let claims = B64URL.encode(json!({ "iat": now - 60, "exp": now + 540, "iss": self.id }).to_string());
        let signing = format!("{head}.{claims}");
        let sig = self.key.sign(signing.as_bytes());
        format!("{signing}.{}", B64URL.encode(sig.to_bytes()))
    }

    /// Where a connect begins for `started`: the App's install page, with a state only this grid can take back.
    pub fn install_url(&self, started: Started) -> String {
        let state = crate::accounts::token("ghs_");
        let now = self.clock.now_ms();
        let mut states = lock(&self.states);
        states.retain(|_, (_, at)| now - *at < STATE_MS);
        states.insert(state.clone(), (started, now));
        format!("{}/apps/{}/installations/new?state={state}", self.web, self.slug)
    }

    /// The team and person a state was made for, once and within its 15 minutes.
    pub fn take_state(&self, state: &str) -> Option<Started> {
        let now = self.clock.now_ms();
        lock(&self.states).remove(state).filter(|(_, at)| now - *at < STATE_MS).map(|(s, _)| s)
    }

    async fn call(&self, method: reqwest::Method, url: &str, bearer: &str, body: Option<&Value>) -> Result<Value> {
        let mut r = self.http.request(method, url).bearer_auth(bearer).header("accept", "application/vnd.github+json");
        if let Some(b) = body {
            r = r.json(b);
        }
        let resp = r.send().await.map_err(|e| bad(format!("GitHub didn't answer: {e}"), 502))?;
        let status = resp.status().as_u16();
        let v: Value = resp.json().await.unwrap_or(Value::Null);
        if status >= 400 {
            let why = v.get("message").and_then(Value::as_str).unwrap_or("refused");
            return Err(bad(format!("GitHub said {status}: {why}"), if status == 404 { 404 } else { 502 }));
        }
        Ok(v)
    }

    /// Whether the person behind `code` (the install's OAuth) can see installation `id`: what makes a connect theirs.
    pub async fn installer_has(&self, code: &str, id: i64) -> Result<bool> {
        let r = self
            .http
            .post(format!("{}/login/oauth/access_token", self.web))
            .header("accept", "application/json")
            .json(&json!({ "client_id": self.client_id, "client_secret": self.client_secret, "code": code }))
            .send()
            .await
            .map_err(|e| bad(format!("GitHub didn't answer: {e}"), 502))?;
        let v: Value = r.json().await.unwrap_or(Value::Null);
        let Some(token) = v.get("access_token").and_then(Value::as_str).filter(|t| !t.is_empty()) else {
            return Err(bad("GitHub didn't confirm who installed it. Try connecting again.", 400));
        };
        let mine = self.call(reqwest::Method::GET, &format!("{}/user/installations?per_page=100", self.api), token, None).await?;
        let ids = mine.get("installations").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(ids.iter().any(|i| i.get("id").and_then(Value::as_i64) == Some(id)))
    }

    /// The account (org or person) installation `id` is on.
    pub async fn account_of(&self, id: i64) -> Result<String> {
        let v = self.call(reqwest::Method::GET, &format!("{}/app/installations/{id}", self.api), &self.jwt(), None).await?;
        Ok(v.pointer("/account/login").and_then(Value::as_str).unwrap_or_default().to_owned())
    }

    /// A token for installation `id`'s repos, and when it stops working (ms): one kept until near its end.
    pub async fn token(&self, id: i64) -> Result<(String, i64)> {
        let now = self.clock.now_ms();
        if let Some((t, until)) = lock(&self.tokens).get(&id).cloned()
            && until - now > SPARE_MS
        {
            return Ok((t, until));
        }
        let v = self
            .call(reqwest::Method::POST, &format!("{}/app/installations/{id}/access_tokens", self.api), &self.jwt(), Some(&json!({})))
            .await?;
        let t = v.get("token").and_then(Value::as_str).filter(|t| !t.is_empty()).ok_or_else(|| bad("GitHub gave no token.", 502))?;
        let until = now + TOKEN_MS;
        lock(&self.tokens).insert(id, (t.to_owned(), until));
        Ok((t.to_owned(), until))
    }

    /// Installation `id`'s token if one in hand still has its five minutes to spare: what a box starting now gets.
    pub fn cached(&self, id: i64) -> Option<String> {
        let now = self.clock.now_ms();
        lock(&self.tokens).get(&id).filter(|(_, until)| until - now > SPARE_MS).map(|(t, _)| t.clone())
    }

    /// Forgets installation `id`'s token (disconnected, or GitHub took it back).
    pub fn forget(&self, id: i64) {
        lock(&self.tokens).remove(&id);
    }

    /// Whether a webhook's body is the App's.
    pub fn webhook_signed(&self, body: &[u8], header: &str) -> bool {
        signed(&self.webhook_secret, body, header)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use rsa::pkcs1::EncodeRsaPrivateKey as _;
    use rsa::signature::Verifier as _;

    fn app(clock: SharedClock) -> (GitHubApp, rsa::RsaPrivateKey) {
        let key = crate::auth::tests::rsa_key();
        let pem = key.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let env: Env = [
            ("GRID_GITHUB_APP_CLIENT_SECRET", "cs"),
            ("GRID_GITHUB_APP_PRIVATE_KEY", pem.as_str()),
            ("GRID_GITHUB_APP_WEBHOOK_SECRET", "whsec"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        let cfg = GitHubAppConfig { id: "42".into(), slug: "wisp-test".into(), client_id: "Iv1".into(), ..Default::default() };
        (GitHubApp::from_config(Some(&cfg), &env, clock).unwrap().unwrap(), key)
    }

    #[test]
    fn its_jwt_is_rs256_by_its_key_for_nine_minutes() {
        let clock = ManualClock::new(1_700_000_000_000);
        let (a, key) = app(clock);
        let jwt = a.jwt();
        let [h, c, s] = jwt.split('.').collect::<Vec<_>>()[..] else { panic!("{jwt}") };
        let claims: Value = serde_json::from_slice(&B64URL.decode(c).unwrap()).unwrap();
        assert_eq!(claims, json!({ "iat": 1_700_000_000 - 60, "exp": 1_700_000_000 + 540, "iss": "42" }));
        let sig = rsa::pkcs1v15::Signature::try_from(B64URL.decode(s).unwrap().as_slice()).unwrap();
        let verify = rsa::pkcs1v15::VerifyingKey::<Sha256>::new(key.to_public_key());
        assert!(verify.verify(format!("{h}.{c}").as_bytes(), &sig).is_ok());
    }

    #[test]
    fn a_state_is_taken_back_once_within_fifteen_minutes() {
        let clock = ManualClock::new(1_000);
        let (a, _) = app(clock.clone());
        let started = Started { team_id: "t1".into(), slug: "acme".into(), user_id: "u1".into() };
        let url = a.install_url(started.clone());
        assert!(url.starts_with("https://github.com/apps/wisp-test/installations/new?state=ghs_"), "{url}");
        let state = url.rsplit('=').next().unwrap();
        assert_eq!(a.take_state(state), Some(started.clone()));
        assert_eq!(a.take_state(state), None, "once");
        let late = a.install_url(started).rsplit('=').next().unwrap().to_owned();
        clock.advance(STATE_MS + 1);
        assert_eq!(a.take_state(&late), None, "not after 15 minutes");
        assert_eq!(a.take_state("ghs_made-up"), None);
    }

    #[test]
    fn a_webhook_is_the_apps_only_with_its_signature() {
        let mut mac = Hmac::<Sha256>::new_from_slice(b"whsec").unwrap();
        mac.update(b"{}");
        let good = format!("sha256={}", crate::secrets::hex(&mac.finalize().into_bytes()));
        assert!(signed("whsec", b"{}", &good));
        assert!(signed("whsec", b"{}", &good.to_uppercase().replace("SHA256=", "sha256=")), "hex in either case");
        assert!(!signed("whsec", b"{ }", &good), "another body");
        assert!(!signed("other", b"{}", &good), "another secret");
        assert!(!signed("", b"{}", "sha256="), "no secret, nothing passes");
        assert!(!signed("whsec", b"{}", "sha1=00"));
    }

    #[test]
    fn without_its_secrets_the_app_is_a_config_error() {
        let cfg = GitHubAppConfig { id: "42".into(), slug: "s".into(), client_id: "c".into(), ..Default::default() };
        assert!(GitHubApp::from_config(Some(&cfg), &Env::new(), ManualClock::new(1)).is_err());
        assert!(GitHubApp::from_config(None, &Env::new(), ManualClock::new(1)).unwrap().is_none());
    }
}
