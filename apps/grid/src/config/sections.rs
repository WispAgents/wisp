//! The sections of grid.json, as typed settings. Unknown keys are kept (`extra`) so `wisp grid init`
//! writes back what an operator added.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(default)]
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// OIDC: the issuer; Google's is built in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    /// GitHub: where to send the browser and call the API (tests point them at a fake).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Allow {
    #[serde(default)]
    pub emails: Vec<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub github_orgs: Vec<String>,
    #[serde(default)]
    pub accounts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
/// No seat cap: the owner removed `maxUsers` on 2026-10-27, so every grid takes everyone `signup` and `allow` let in. A
/// grid.json that still says `maxUsers` or `seats` loads with them ignored (serde skips keys it doesn't know).
pub struct AuthConfig {
    pub dev: bool,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub signup: String,
    pub allow: Allow,
    pub admins: Vec<String>,
}

/// The container driver's settings (the defaults are boxd's `CONTAINER`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContainerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_swap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pids: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns: Option<Vec<String>>,
    /// boxd's socket: with it, boxd runs the containers and the grid needs no Docker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boxd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `hostd.admit`: false, or the memory one more box needs and what the host keeps for itself.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdmitConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_mb: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserve_mb: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HostdConfig {
    /// "process" or "container".
    pub driver: String,
    pub idle_ms: i64,
    pub check_ms: i64,
    pub start_ms: i64,
    pub max_running: i64,
    /// A user's own box and their sandboxes running at once (default 4): past it a sandbox waits for room.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_running_per_user: Option<i64>,
    pub env: BTreeMap<String, String>,
    pub path: String,
    /// `false` (as a JSON value) turns admission off; an object sets it; absent: the driver's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admit: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previews: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evict_after_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerConfig>,
    /// The box's entry: the Rust `wisp` binary by default; tests run a stand-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wisp_bin: Option<String>,
    /// hostd in its own process (`wisp-grid-hostd`): the unix socket the edge reaches it on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl HostdConfig {
    /// `hostd.admit`, read: None for off, Some for on (with the given numbers, defaults filled later).
    pub fn admit(&self) -> Option<Option<AdmitConfig>> {
        match &self.admit {
            None => None,
            Some(Value::Bool(false)) => Some(None),
            Some(v) => Some(serde_json::from_value(v.clone()).ok()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rows: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_retry_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliver_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telegram_api: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MailInbound {
    pub provider: String,
    #[serde(default)]
    pub topic_arn: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dkim_keys: Option<BTreeMap<String, String>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MailOutbound {
    pub provider: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key_env: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MailConfig {
    pub domain: String,
    pub inbound: MailInbound,
    pub outbound: MailOutbound,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_per_hour: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_days: Option<i64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `sites`, `shares`, `sandboxes`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeatureConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_user: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_bytes: Option<i64>,
}

/// `push`: the phone relay's FCM and APNs credentials (ADR-0065 §3). Each secret is a file or an env var
/// (`GRID_FCM_SERVICE_ACCOUNT(_FILE)`, `GRID_APNS_KEY(_FILE)`), never grid.json itself. Without them the relay answers
/// `unavailable`, and wisp keeps ntfy.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PushConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fcm: Option<FcmConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apns: Option<ApnsConfig>,
    /// sends a minute to one handle (default 30)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_minute: Option<usize>,
}

/// FCM HTTP v1: a Google service account's JSON key (it names its project).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FcmConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account_file: Option<String>,
    /// default `GRID_FCM_SERVICE_ACCOUNT`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account_env: Option<String>,
    /// tests and proxies: default `https://fcm.googleapis.com`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
}

/// APNs with a token key (`.p8`): its key id, the team, and the app's bundle id (the topic).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApnsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_file: Option<String>,
    /// default `GRID_APNS_KEY`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_env: Option<String>,
    pub key_id: String,
    pub team_id: String,
    pub topic: String,
    /// the development gateway (`api.sandbox.push.apple.com`)
    #[serde(default)]
    pub sandbox: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
}

/// `githubApp`: the GitHub App a team installs so its hub gets short-lived tokens for its repos and its webhooks
/// (ADR-0148 §8e). Its secrets are files or env vars, never grid.json: the OAuth client's secret
/// (`GRID_GITHUB_APP_CLIENT_SECRET`), the app's private key (`GRID_GITHUB_APP_PRIVATE_KEY`, PEM) and its webhook
/// secret (`GRID_GITHUB_APP_WEBHOOK_SECRET`), each also `*_FILE`. Without it, hubs use a `GH_TOKEN` secret.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitHubAppConfig {
    /// the App's id (a number, as GitHub shows it)
    pub id: String,
    /// its URL name: `https://github.com/apps/<slug>`
    pub slug: String,
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_secret_file: Option<String>,
    /// tests and GitHub Enterprise: default `https://api.github.com`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    /// default `https://github.com`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<String>,
}
