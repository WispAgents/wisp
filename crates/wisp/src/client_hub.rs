//! A CLI client for a team's hub (ADR-0148, `wisp grid plan|deploy`): its commands go to the hub's address with a deploy
//! key (no wisp needed on this machine), or through this computer's wisp and its grid sign-in.
use std::time::Duration;

use serde_json::Value;

use super::{CliError, Client, Result};

/// Answers `*.localhost` itself (127.0.0.1, as browsers do), so a grid on this machine works at
/// `http://acme.wisp.localhost:8080` (macOS's resolver says `::1`, where the grid doesn't listen); every other name
/// goes to the system's resolver. The same as wisp.grid-client's.
#[derive(Debug, Default)]
struct LocalFirst;

impl reqwest::dns::Resolve for LocalFirst {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        use std::net::{SocketAddr, ToSocketAddrs as _};
        let host = name.as_str().to_ascii_lowercase();
        Box::pin(async move {
            if host == "localhost" || host.ends_with(".localhost") {
                let a: Box<dyn Iterator<Item = SocketAddr> + Send> = Box::new(std::iter::once(SocketAddr::from(([127, 0, 0, 1], 0))));
                return Ok(a);
            }
            let found = tokio::task::spawn_blocking(move || (host.as_str(), 0).to_socket_addrs().map(Iterator::collect::<Vec<_>>))
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            let a: Box<dyn Iterator<Item = SocketAddr> + Send> = Box::new(found.into_iter());
            Ok(a)
        })
    }
}

/// An HTTP client for a grid and its hubs (no proxy, `*.localhost` here).
pub fn grid_http() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .dns_resolver(std::sync::Arc::new(LocalFirst))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default()
}

/// Where a hub client's commands go.
#[derive(Debug, Clone)]
pub(super) enum Hub {
    /// straight to the hub's address with a deploy key (`WISP_HUB_KEY`): CI, with no wisp on the machine
    Key { url: String, key: String, source: String },
    /// through this computer's wisp and its grid sign-in (`grid-client.hubCall`): a person at their terminal
    Via { team: String, source: String },
}

impl Client {
    /// A client whose commands run on a team's hub at `url`, with deploy key `key` (no wisp needed here). `source`:
    /// where the formula came from, for the hub's deploy history.
    pub fn hub_with_key(url: &str, key: &str, source: &str) -> Self {
        let mut c = Self::new(0);
        c.http = grid_http();
        url.trim_end_matches('/').clone_into(&mut c.url);
        c.token = None;
        c.on = None;
        c.hub = Some(Hub::Key { url: c.url.clone(), key: key.to_owned(), source: source.to_owned() });
        c
    }

    /// This wisp, running every command on `team`'s hub as you (`grid-client.hubCall`).
    pub fn via_hub(mut self, team: &str, source: &str) -> Self {
        self.on = None;
        self.hub = Some(Hub::Via { team: team.to_owned(), source: source.to_owned() });
        self
    }

    /// One command at a hub's address with a deploy key; its answer read like this wisp's own.
    pub(super) async fn hub_cmd(&self, url: &str, bearer: &str, source: &str, cmd: &str, args: Value, key: &str) -> Result<Value> {
        let r = self
            .http
            .post(format!("{url}/api/cmd"))
            .bearer_auth(bearer)
            .header("idempotency-key", key)
            .header("x-wisp-deploy-source", source)
            .json(&serde_json::json!({ "cmd": cmd, "args": args }))
            .send()
            .await
            .map_err(|e| CliError::said(format!("the hub at {url}: {e}")))?;
        let status = r.status().as_u16();
        let v: Value = r.json().await.unwrap_or_default();
        if v.get("ok") == Some(&Value::Bool(true)) {
            return Ok(v.get("result").cloned().unwrap_or(Value::Null));
        }
        // the hub's wisp words a refusal as {error: {message}}, the grid's edge as {error: "…"}
        let message = v
            .pointer("/error/message")
            .and_then(Value::as_str)
            .or_else(|| v.get("error").and_then(Value::as_str))
            .map_or_else(|| format!("{cmd}: HTTP {status} from the hub"), str::to_owned);
        Err(CliError::Http { status, message, body: v })
    }
}
