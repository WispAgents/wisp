//! Usage stats for a grid's operator (v1 grid/telemetry.js): sign-ups, sign-ins, devices, and how boxes start and
//! stop. Off unless grid.json says `"telemetry": true` (a self-hosted grid sends nothing by default), and off with
//! `WISP_TELEMETRY=0` or `DO_NOT_TRACK=1`. Goes to wisp's `PostHog` project like wisp's own stats: fixed values only,
//! and each user as a hash of their id, never a handle, email or address.
//!
//! On wisp's own hosted grid only, the sign-in page loads `PostHog` too, and the server's sign-in steps are sent as
//! that page's anonymous visitor (its cookie), so the funnel reads site → sign-in page → provider → signed up.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::boxhost::{BoxChange, BoxState};
use crate::config::Env;

/// The grid whose sign-in page loads `PostHog` and whose sign-in steps are counted.
pub const HOSTED: &str = "grid.trywisp.dev";
/// wisp's `PostHog` project (the same as `wisp.posthog`'s `DEFAULT_KEY` and trywisp.dev's).
pub const KEY: &str = "phc_wowwZkbP8e4zYuwT67JtX6oxdPwkMWL5kKnrUmYYXhcD";
/// Where batches go; `WISP_POSTHOG_HOST` moves it, as it moves wisp's own (`wisp.posthog`).
const DEFAULT_HOST: &str = "https://us.i.posthog.com";
/// A batch goes at 50 events, else every 15 s.
const BATCH_MAX: usize = 50;
const FLUSH_S: u64 = 15;

/// Audit actions (`accounts.rs`) that become events. Each carries its provider at most.
fn audit_event(action: &str) -> Option<&'static str> {
    Some(match action {
        "signup" => "grid_signup",
        "signin" => "grid_signin",
        "identity.link" => "grid_identity_linked",
        "device.allow" => "grid_device_allowed",
        "device.deny" => "grid_device_denied",
        "device.revoke" => "grid_device_revoked",
        "user.delete" => "grid_user_deleted",
        "signup.refused" => "grid_signup_refused",
        _ => return None,
    })
}

/// Why a box stopped, as a few fixed words (never the error's text).
fn stop_reason(event: &str) -> &'static str {
    match event {
        "idle" => "idle",
        "put to sleep" | "slept" => "user",
        "grid stopped" => "grid",
        "deleted" => "deleted",
        "oom" | "out of memory" => "oom",
        "failed" | "wake failed" => "failed",
        e if e.starts_with("exited") => "crashed",
        _ => "other",
    }
}

#[derive(Debug, Default)]
pub struct GridTelemetry {
    /// events go out at all
    pub on: bool,
    /// the hosted grid: its sign-in page loads `PostHog`, and sign-in steps are counted
    pub pages: bool,
    common: Map<String, Value>,
    queue: Mutex<Vec<Value>>,
    /// a box's start asked for, by user: how long it took to come up
    starting: Mutex<HashMap<String, i64>>,
    client: Option<reqwest::Client>,
    batch_url: String,
}

impl GridTelemetry {
    /// From grid.json's `telemetry`, the grid's domain and its environment.
    #[must_use]
    pub fn new(telemetry: Option<bool>, domain: &str, env: &Env) -> Self {
        let get = |k: &str| env.get(k).map(|v| v.trim().to_ascii_lowercase()).unwrap_or_default();
        let opted_out = matches!(get("WISP_TELEMETRY").as_str(), "0" | "false" | "off" | "no")
            || !matches!(get("DO_NOT_TRACK").as_str(), "" | "0" | "false");
        let on = telemetry == Some(true) && !opted_out;
        let mut common = Map::new();
        common.insert("$lib".into(), "wisp-grid".into());
        common.insert("$process_person_profile".into(), false.into());
        common.insert("version".into(), env!("CARGO_PKG_VERSION").into());
        common.insert("hosted".into(), (domain == HOSTED).into());
        let host = env.get("WISP_POSTHOG_HOST").map(String::as_str).filter(|h| !h.is_empty()).unwrap_or(DEFAULT_HOST);
        let batch_url = format!("{}/batch/", host.trim_end_matches('/'));
        Self { on, pages: on && domain == HOSTED, common, client: on.then(reqwest::Client::new), batch_url, ..Self::default() }
    }

    /// The `PostHog` key the sign-in page loads, on the hosted grid only.
    #[must_use]
    pub fn page_key(&self) -> Option<String> {
        self.pages.then(|| KEY.to_owned())
    }

    /// A user's id as `PostHog` sees it: a hash, never the id.
    #[must_use]
    pub fn who(user: Option<&str>) -> String {
        user.map_or_else(
            || "grid-anonymous".to_owned(),
            |u| {
                let h = Sha256::digest(format!("wisp-grid:{u}").as_bytes());
                format!("grid-{}", crate::secrets::hex(&h[..12]))
            },
        )
    }

    /// The sign-in page's anonymous `PostHog` id, from its cookie (`ph_<key>_posthog`, JSON), so the server's steps
    /// join the page's in one funnel. Only on the hosted grid; a value that isn't a plain id is ignored.
    #[must_use]
    pub fn visitor(&self, cookies: &HashMap<String, String>) -> Option<String> {
        if !self.pages {
            return None;
        }
        let v: Value = serde_json::from_str(cookies.get(&format!("ph_{KEY}_posthog"))?).ok()?;
        let id = v.get("distinct_id")?.as_str()?;
        let ok = (8..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        ok.then(|| id.to_owned())
    }

    fn send(&self, event: &str, distinct_id: String, props: Value) {
        if !self.on {
            return;
        }
        let mut p = self.common.clone();
        if let Value::Object(extra) = props {
            p.extend(extra);
        }
        p.insert("distinct_id".into(), distinct_id.into());
        let full = {
            let mut q = self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            // no timestamp: PostHog stamps it on arrival, at most 15 s later
            q.push(json!({ "event": event, "properties": p }));
            q.len() >= BATCH_MAX
        };
        if full {
            self.flush();
        }
    }

    /// One event for a user (hashed), or `grid-anonymous`.
    pub fn track(&self, event: &str, user: Option<&str>, props: Value) {
        self.send(event, Self::who(user), props);
    }

    /// A sign-in step (`grid_auth_started` / `_finished` / `_failed`), as the page's visitor. Hosted grid only.
    pub fn funnel(&self, event: &str, visitor: Option<String>, props: Value) {
        if self.pages {
            self.send(event, visitor.unwrap_or_else(|| "grid-anonymous".into()), props);
        }
    }

    /// An audit row (`accounts.rs`) that is one of the events above.
    pub fn audit(&self, user: Option<&str>, action: &str, provider: &str) {
        if let Some(e) = audit_event(action) {
            self.track(e, user, json!({ "provider": provider }));
        }
    }

    /// A box changed state (hostd's change events): started (and how long it took), stopped (why, after how
    /// long) or failed to start. `now_ms`: the grid's clock.
    pub fn box_change(&self, c: &BoxChange, now_ms: i64) {
        let id = c.user.id.as_str();
        let sandbox = json!(c.user.sandbox.is_some());
        let mut starting = self.starting.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match c.state {
            BoxState::Starting => {
                starting.insert(id.to_owned(), now_ms);
            }
            BoxState::Running if c.event == "woke" => {
                let ms = starting.remove(id).map(|t| now_ms - t);
                drop(starting);
                self.track("grid_box_started", Some(id), json!({ "ms": ms, "sandbox": sandbox }));
            }
            BoxState::Stopped if matches!(c.event.as_str(), "failed" | "wake failed") => {
                starting.remove(id);
                drop(starting);
                self.track("grid_box_failed", Some(id), json!({ "sandbox": sandbox }));
            }
            BoxState::Stopped => {
                starting.remove(id);
                drop(starting);
                if let Some(t) = c.started_at {
                    let minutes = (c.stopped_at.unwrap_or(now_ms) - t) / 60_000;
                    self.track(
                        "grid_box_stopped",
                        Some(id),
                        json!({ "reason": stop_reason(&c.event), "minutes": minutes, "sandbox": sandbox }),
                    );
                }
            }
            _ => {}
        }
    }

    /// The events waiting to go, taken (what a flush sends; tests read it).
    pub fn take(&self) -> Vec<Value> {
        std::mem::take(&mut *self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    /// Sends what's waiting, in the background (nothing outside a tokio runtime, nothing when off).
    pub fn flush(&self) {
        let (Some(client), Ok(rt)) = (self.client.clone(), tokio::runtime::Handle::try_current()) else { return };
        let (batch, url) = (self.take(), self.batch_url.clone());
        if batch.is_empty() {
            return;
        }
        rt.spawn(async move {
            let body = json!({ "api_key": KEY, "batch": batch });
            if let Err(e) = client.post(&url).timeout(std::time::Duration::from_secs(10)).json(&body).send().await {
                tracing::debug!("grid: usage stats not sent: {e}");
            }
        });
    }

    /// Every 15 s, what's waiting goes; box changes are followed as they come. Started once, with the grid.
    pub fn run(self: &Arc<Self>, mut changes: tokio::sync::broadcast::Receiver<BoxChange>, clock: crate::clock::SharedClock) {
        let Ok(rt) = tokio::runtime::Handle::try_current() else { return };
        if !self.on {
            return;
        }
        let me = self.clone();
        rt.spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(FLUSH_S));
            loop {
                tokio::select! {
                    _ = tick.tick() => me.flush(),
                    c = changes.recv() => match c {
                        Ok(c) => me.box_change(&c, clock.now_ms()),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    },
                }
            }
            me.flush();
        });
    }
}
