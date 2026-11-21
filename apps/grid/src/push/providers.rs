//! FCM (HTTP v1, a service account's OAuth token) and APNs (token auth, ES256), for real. Each credential is read
//! once at start from grid.json's `push` section (files or env vars); a missing or broken one leaves that platform
//! unserved, and the relay answers `unavailable` for it. Tokens are cached until a few minutes before they expire.

use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Delivered, Outgoing, Platform, PushSender, Target, apns_payload, fcm_message};
use crate::config::{ApnsConfig, Env, FcmConfig, PushConfig, read_secret};
use crate::error::{GridError, Result};
use crate::secrets::b64u;

const FCM_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
const JWT_BEARER: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn jwt(header: &Value, claims: &Value, sign: impl FnOnce(&[u8]) -> Vec<u8>) -> String {
    let input = format!("{}.{}", b64u(header.to_string()), b64u(claims.to_string()));
    let sig = sign(input.as_bytes());
    format!("{input}.{}", b64u(sig))
}

struct Fcm {
    project: String,
    email: String,
    token_uri: String,
    key: rsa::RsaPrivateKey,
    api: String,
    /// (access token, expires at ms)
    cached: Mutex<Option<(String, i64)>>,
}

impl Fcm {
    fn from(c: &FcmConfig, env: &Env) -> Result<Self> {
        use rsa::pkcs8::DecodePrivateKey as _;
        let file = c.service_account_file.as_deref().map(std::path::Path::new);
        let text = read_secret(None, file, Some(c.service_account_env.as_deref().unwrap_or("GRID_FCM_SERVICE_ACCOUNT")), env)?;
        if text.is_empty() {
            return Err(GridError::Config("push.fcm: no service account".into()));
        }
        let sa: Value =
            serde_json::from_str(&text).map_err(|e| GridError::Config(format!("push.fcm: the service account isn't JSON: {e}")))?;
        let s = |k: &str| sa.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        if s("project_id").is_empty() || s("client_email").is_empty() {
            return Err(GridError::Config("push.fcm: the service account names no project_id or client_email".into()));
        }
        let key =
            rsa::RsaPrivateKey::from_pkcs8_pem(&s("private_key")).map_err(|e| GridError::Config(format!("push.fcm: private_key: {e}")))?;
        let token_uri = Some(s("token_uri")).filter(|u| !u.is_empty()).unwrap_or_else(|| "https://oauth2.googleapis.com/token".into());
        Ok(Self {
            project: s("project_id"),
            email: s("client_email"),
            token_uri,
            key,
            api: c.api.clone().unwrap_or_else(|| "https://fcm.googleapis.com".into()),
            cached: Mutex::new(None),
        })
    }

    async fn access(&self, http: &reqwest::Client, now_ms: i64) -> std::result::Result<String, String> {
        use rsa::signature::{SignatureEncoding as _, Signer as _};
        if let Some((t, until)) = lock(&self.cached).clone()
            && until > now_ms + 300_000
        {
            return Ok(t);
        }
        let iat = now_ms / 1000;
        let claims = json!({ "iss": self.email, "scope": FCM_SCOPE, "aud": self.token_uri, "iat": iat, "exp": iat + 3600 });
        let signer = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(self.key.clone());
        let assertion = jwt(&json!({ "alg": "RS256", "typ": "JWT" }), &claims, |m| signer.sign(m).to_vec());
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", JWT_BEARER)
            .append_pair("assertion", &assertion)
            .finish();
        let r = http
            .post(&self.token_uri)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(form)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?;
        let status = r.status().as_u16();
        let v: Value = r.json().await.map_err(|e| e.without_url().to_string())?;
        let Some(t) = v.get("access_token").and_then(Value::as_str).filter(|_| status == 200) else {
            return Err(format!("Google's token endpoint answered {status}"));
        };
        let ttl = v.get("expires_in").and_then(Value::as_i64).unwrap_or(3600);
        *lock(&self.cached) = Some((t.to_owned(), now_ms + ttl * 1000));
        Ok(t.to_owned())
    }

    async fn send(&self, http: &reqwest::Client, to: &Target, msg: &Outgoing, now_ms: i64) -> Delivered {
        let tok = match self.access(http, now_ms).await {
            Ok(t) => t,
            Err(e) => return Delivered::Failed(e),
        };
        let url = format!("{}/v1/projects/{}/messages:send", self.api, self.project);
        let r = http.post(url).bearer_auth(tok).json(&fcm_message(to, msg)).timeout(std::time::Duration::from_secs(10)).send().await;
        match r {
            Ok(r) if r.status().is_success() => Delivered::Sent,
            Ok(r) => {
                let status = r.status().as_u16();
                let body = r.text().await.unwrap_or_default();
                // only FCM's own word that the token is gone: a bare 404 (a wrong project, a proxy) never drops a phone
                if body.contains("UNREGISTERED") { Delivered::Gone } else { Delivered::Failed(format!("FCM answered {status}")) }
            }
            Err(e) => Delivered::Failed(e.without_url().to_string()),
        }
    }
}

struct Apns {
    key: p256::ecdsa::SigningKey,
    key_id: String,
    team_id: String,
    topic: String,
    api: String,
    /// (provider token, made at ms): Apple wants it refreshed between 20 and 60 minutes
    cached: Mutex<Option<(String, i64)>>,
}

impl Apns {
    fn from(c: &ApnsConfig, env: &Env) -> Result<Self> {
        use p256::pkcs8::DecodePrivateKey as _;
        let file = c.key_file.as_deref().map(std::path::Path::new);
        let pem = read_secret(None, file, Some(c.key_env.as_deref().unwrap_or("GRID_APNS_KEY")), env)?;
        if pem.is_empty() || c.key_id.is_empty() || c.team_id.is_empty() || c.topic.is_empty() {
            return Err(GridError::Config("push.apns: needs a key, keyId, teamId and topic".into()));
        }
        let key = p256::ecdsa::SigningKey::from_pkcs8_pem(&pem).map_err(|e| GridError::Config(format!("push.apns: the key: {e}")))?;
        let gateway = if c.sandbox { "https://api.sandbox.push.apple.com" } else { "https://api.push.apple.com" };
        Ok(Self {
            key,
            key_id: c.key_id.clone(),
            team_id: c.team_id.clone(),
            topic: c.topic.clone(),
            api: c.api.clone().unwrap_or_else(|| gateway.into()),
            cached: Mutex::new(None),
        })
    }

    fn provider_token(&self, now_ms: i64) -> String {
        use p256::ecdsa::signature::Signer as _;
        if let Some((t, at)) = lock(&self.cached).clone()
            && now_ms - at < 40 * 60_000
        {
            return t;
        }
        let t = jwt(&json!({ "alg": "ES256", "kid": self.key_id }), &json!({ "iss": self.team_id, "iat": now_ms / 1000 }), |m| {
            let s: p256::ecdsa::Signature = self.key.sign(m);
            s.to_bytes().to_vec()
        });
        *lock(&self.cached) = Some((t.clone(), now_ms));
        t
    }

    async fn send(&self, http: &reqwest::Client, to: &Target, msg: &Outgoing, now_ms: i64) -> Delivered {
        let url = format!("{}/3/device/{}", self.api, to.token);
        let r = http
            .post(url)
            .header("authorization", format!("bearer {}", self.provider_token(now_ms)))
            .header("apns-topic", &self.topic)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .header("apns-collapse-id", &msg.collapse)
            .json(&apns_payload(msg))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;
        match r {
            Ok(r) if r.status().is_success() => Delivered::Sent,
            Ok(r) => {
                let status = r.status().as_u16();
                let body = r.text().await.unwrap_or_default();
                // BadDeviceToken is also what a wrong `sandbox` flag gets: never read as gone
                if status == 410 || body.contains("Unregistered") {
                    Delivered::Gone
                } else {
                    Delivered::Failed(format!("APNs answered {status}"))
                }
            }
            Err(e) => Delivered::Failed(e.without_url().to_string()),
        }
    }
}

/// FCM and APNs over HTTPS (APNs needs HTTP/2: reqwest negotiates it).
pub struct HttpPush {
    http: reqwest::Client,
    fcm: Option<Fcm>,
    apns: Option<Apns>,
}

impl std::fmt::Debug for HttpPush {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpPush").field("fcm", &self.fcm.is_some()).field("apns", &self.apns.is_some()).finish_non_exhaustive()
    }
}

impl HttpPush {
    /// What grid.json's `push` section can serve; None when it serves neither platform.
    pub fn from_config(c: Option<&PushConfig>, env: &Env) -> Option<Self> {
        let c = c?;
        let fcm = c.fcm.as_ref().and_then(|f| Fcm::from(f, env).map_err(|e| tracing::warn!("grid: push: no FCM: {e}")).ok());
        let apns = c.apns.as_ref().and_then(|a| Apns::from(a, env).map_err(|e| tracing::warn!("grid: push: no APNs: {e}")).ok());
        (fcm.is_some() || apns.is_some()).then(|| Self { http: reqwest::Client::new(), fcm, apns })
    }
}

#[async_trait]
impl PushSender for HttpPush {
    fn serves(&self, p: Platform) -> bool {
        match p {
            Platform::Android => self.fcm.is_some(),
            Platform::Ios => self.apns.is_some(),
        }
    }
    async fn send(&self, to: &Target, msg: &Outgoing, now_ms: i64) -> Delivered {
        match (to.platform, &self.fcm, &self.apns) {
            (Platform::Android, Some(f), _) => f.send(&self.http, to, msg, now_ms).await,
            (Platform::Ios, _, Some(a)) => a.send(&self.http, to, msg, now_ms).await,
            _ => Delivered::Failed("no credentials for this platform".into()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn nothing_configured_serves_nothing() {
        assert!(HttpPush::from_config(None, &Env::new()).is_none());
        let c = PushConfig { fcm: Some(FcmConfig::default()), apns: None, per_minute: None };
        assert!(HttpPush::from_config(Some(&c), &Env::new()).is_none(), "no service account: unavailable, not a crash");
    }

    #[test]
    fn an_apns_provider_token_is_an_es256_jwt_kept_for_forty_minutes() {
        use p256::pkcs8::EncodePrivateKey as _;
        let key = p256::ecdsa::SigningKey::from_slice(&[7u8; 32]).unwrap();
        let pem = key.to_pkcs8_pem(p256::pkcs8::LineEnding::LF).unwrap().to_string();
        let env: Env = [("GRID_APNS_KEY".to_owned(), pem)].into_iter().collect();
        let c = ApnsConfig { key_id: "K1".into(), team_id: "T1".into(), topic: "dev.trywisp.mobile".into(), ..ApnsConfig::default() };
        let a = Apns::from(&c, &env).unwrap();
        let t = a.provider_token(1_000_000);
        assert_eq!(t.split('.').count(), 3);
        assert_eq!(a.provider_token(1_000_000 + 39 * 60_000), t);
        assert_ne!(a.provider_token(1_000_000 + 41 * 60_000), t);
        assert_eq!(a.api, "https://api.push.apple.com");
    }
}
