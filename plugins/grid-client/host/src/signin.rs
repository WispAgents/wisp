//! Signing in to a grid (v1 server/services/cloud.js): the device flow, RFC 8628. wisp asks the grid for a
//! code, you approve it in your browser (signed in to the grid), and the grid hands this computer a
//! device token. The token goes to secrets (keyed by grid and account); the cloud box shows up as a machine
//! (transport `grid`). Signing in again adds an account next to the others and makes it current (ADR-0061).
//!
//! Polling follows the grid's interval, plus 5 s for each `slow_down`; a network blip keeps it going until
//! the code expires; `access_denied` and `expired_token` end it in words.
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use wisp_grid_client_api::{DEFAULT_GRID, SignIn, SignInArgs};
use wisp_grid_client_core::{grid_url, label_host, next_interval};
use wisp_kernel::{Result, WireError};

use crate::net;
use crate::state::{Account, St, key_of, lock, token_key};

/// What this computer calls itself to the grid.
pub fn device_name() -> String {
    let host = std::env::var("HOSTNAME").ok().filter(|h| !h.is_empty()).unwrap_or_else(|| "this computer".into());
    format!("{} (wisp)", host.trim_end_matches(".local"))
}

fn now(st: &St) -> i64 {
    st.ctx.clock().map_or(0, |c| c.now_ms())
}

pub async fn start(st: &Arc<St>, a: &SignInArgs) -> Result<SignIn> {
    let grid = grid_url(a.grid.as_deref().unwrap_or(DEFAULT_GRID)).ok_or_else(|| WireError::invalid("that isn't a grid's address"))?;
    let (status, b) = net::call(&st.http, "POST", &format!("{grid}/api/device/code"), None, Some(&json!({ "name": device_name() })), &[])
        .await
        .map_err(|e| WireError::invalid(format!("Couldn’t reach {}: {}", net::host_of(&grid), e.message)))?;
    let code = b.get("device_code").and_then(Value::as_str).unwrap_or_default().to_owned();
    if status >= 400 || code.is_empty() {
        return Err(WireError::invalid(format!("{} isn’t a wisp Grid (HTTP {status})", net::host_of(&grid))));
    }
    let s = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let interval = b.get("interval").and_then(Value::as_u64).unwrap_or(5).max(1);
    let expires_in = b.get("expires_in").and_then(Value::as_i64).unwrap_or(600);
    let verify = if s("verification_uri_complete").is_empty() { s("verification_uri") } else { s("verification_uri_complete") };
    let flow = SignIn {
        id: St::new_flow_id(),
        grid: grid.clone(),
        state: "waiting".into(),
        user_code: s("user_code"),
        verify_url: verify,
        expires_at: now(st) + expires_in * 1000,
        ..SignIn::default()
    };
    lock(&st.flows).insert(flow.id.clone(), flow.clone());
    let (st2, id, add) = (st.clone(), flow.id.clone(), a.add_computer);
    st.ctx.spawn(async move { poll(st2, id, code, interval, add).await });
    Ok(flow)
}

fn finish(st: &Arc<St>, id: &str, state: &str, error: &str) {
    if let Some(f) = lock(&st.flows).get_mut(id) {
        f.state = state.into();
        f.error = error.into();
    }
    crate::view::publish(st);
}

async fn poll(st: Arc<St>, id: String, code: String, mut interval: u64, add_computer: bool) {
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let Some(f) = lock(&st.flows).get(&id).cloned() else { return };
        if f.state != "waiting" {
            return;
        }
        if now(&st) > f.expires_at {
            return finish(&st, &id, "expired", "The code expired. Start again.");
        }
        let Ok((status, b)) =
            net::call(&st.http, "POST", &format!("{}/api/device/token", f.grid), None, Some(&json!({ "device_code": code })), &[]).await
        else {
            continue;
        };
        let e = b.get("error").and_then(Value::as_str).unwrap_or_default();
        match e {
            "authorization_pending" | "slow_down" => interval = next_interval(interval, e),
            "access_denied" => return finish(&st, &id, "denied", "Sign-in was declined in the browser."),
            "expired_token" => return finish(&st, &id, "expired", "The code expired. Start again."),
            _ if !e.is_empty() || status >= 400 => {
                return finish(
                    &st,
                    &id,
                    "failed",
                    &format!("The grid said {}.", if e.is_empty() { status.to_string() } else { e.to_owned() }),
                );
            }
            _ => {
                match done(&st, &f, &b, add_computer).await {
                    Ok((handle, machine)) => {
                        if let Some(f) = lock(&st.flows).get_mut(&id) {
                            (f.state, f.handle, f.machine) = ("done".into(), handle, machine);
                        }
                        crate::view::publish(&st);
                    }
                    Err(err) => finish(&st, &id, "failed", &err.message),
                }
                return;
            }
        }
    }
}

/// Signed in: the token kept under its account, the account added (or renewed) and made current, its cloud
/// box added as a machine (or its token renewed).
async fn done(st: &Arc<St>, f: &SignIn, b: &Value, add_computer: bool) -> Result<(String, String)> {
    let token = b.get("token").and_then(Value::as_str).unwrap_or_default();
    let handle = b.pointer("/user/handle").and_then(Value::as_str).unwrap_or_default().to_owned();
    if token.is_empty() || handle.is_empty() || handle.contains('#') {
        return Err(WireError::invalid("The grid's answer had no token or handle."));
    }
    let url = b.pointer("/instance/url").and_then(Value::as_str).map_or_else(
        || label_host(&f.grid, &handle).map(|h| format!("{}://{h}", f.grid.split("://").next().unwrap_or("https"))).unwrap_or_default(),
        str::to_owned,
    );
    let key = key_of(&f.grid, &handle);
    let prev = st.accounts().get(&key).map(|a| a.machine.clone()).unwrap_or_default();
    let machine = crate::peers::cloud_box(st, &prev, &f.grid, &handle, &url, token).await?;
    let was = st.accounts().current;
    // the account as it is now (a sign-out or a computer change may have landed while the box was added), then its
    // token: written last, so a sign-out racing this one can't leave the account without it, nor a failed box a
    // token no account owns
    let (grid, who, added, m) = (f.grid.clone(), handle.clone(), now(st), machine.clone());
    st.change(move |x| {
        let mut a = x.get(&key_of(&grid, &who)).cloned().unwrap_or_else(|| Account { grid, handle: who, added, ..Account::default() });
        a.machine = m;
        x.add(a);
    })
    .await?;
    wisp_kernel::services::secrets(&st.ctx)?.set(&token_key(&key), token).await?;
    if was != key {
        crate::computer::follow_current(st).await;
    }
    if add_computer {
        crate::calls::warn("this computer on the grid", crate::computer::set(st, true, None).await);
    }
    crate::calls::warn("the account's computers", crate::peers::sync_computers(st).await);
    Ok((handle, machine))
}

pub fn get(st: &St, id: &str) -> Result<SignIn> {
    lock(&st.flows).get(id).cloned().ok_or_else(|| WireError::not_found("No such sign-in."))
}

pub fn cancel(st: &St, id: &str) -> Result<SignIn> {
    let mut flows = lock(&st.flows);
    let f = flows.get_mut(id).ok_or_else(|| WireError::not_found("No such sign-in."))?;
    if f.state == "waiting" {
        f.state = "cancelled".into();
    }
    Ok(f.clone())
}

/// Signed out of the current account (`wisp cloud signout`): `accounts::remove` on it.
pub async fn sign_out(st: &Arc<St>) -> Result<()> {
    let key = st.accounts().current;
    if key.is_empty() {
        return Ok(());
    }
    crate::accounts::remove(st, &key).await
}
