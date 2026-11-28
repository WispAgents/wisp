//! A team's hub (ADR-0148) through the grid's `/api/teams/<team>/…`, with this computer's device token: what `wisp team`
//! and the Settings pane ask. `hubCall` runs a deploy's `resources.*` command on the hub itself, as you, so
//! `wisp grid deploy` works without a key; the grid keeps that deploy in the hub's history under your handle.
use std::sync::Arc;

use serde_json::{Value, json};
use wisp_grid_client_api as api;
use wisp_kernel::{Result, WireError};

use crate::net;
use crate::state::St;

/// A team's slug as the grid makes them: anything else never reaches a path.
fn slug(t: &str) -> Result<&str> {
    let t = t.trim();
    let ok = (2..=40).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if ok { Ok(t) } else { Err(WireError::invalid("team: a team's slug (a-z, 0-9 and dashes)")) }
}

/// An invite's or a key's id (hex), or a secret's name (A-Z, 0-9, _): one path segment, nothing else.
fn segment<'a>(s: &'a str, what: &str) -> Result<&'a str> {
    let s = s.trim();
    let ok = !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if ok { Ok(s) } else { Err(WireError::not_found(format!("No such {what}."))) }
}

/// An invite link (`…/join/wgi_…`) or its code: the code.
pub fn invite_code(link: &str) -> &str {
    let l = link.trim();
    l.rsplit('/').next().unwrap_or(l).split(['?', '#']).next().unwrap_or_default()
}

/// The HTTP call a hub op is.
pub fn hub_call(a: &api::HubArgs) -> Result<(&'static str, String, Option<Value>)> {
    if a.op == "join" {
        let code = invite_code(&a.code);
        if !code.starts_with("wgi_") {
            return Err(WireError::invalid("code: an invite link (…/join/wgi_…) or its code"));
        }
        return Ok(("POST", "/api/join".into(), Some(json!({ "code": code }))));
    }
    let t = slug(&a.team)?;
    Ok(match a.op.as_str() {
        "show" | "" => ("GET", format!("/api/teams/{t}/hub"), None),
        "create" => ("POST", format!("/api/teams/{t}/hub"), None),
        "restart" => ("POST", format!("/api/teams/{t}/hub/restart"), None),
        "delete" => ("DELETE", format!("/api/teams/{t}"), Some(json!({ "confirm": a.confirm }))),
        "invites" => ("GET", format!("/api/teams/{t}/invites"), None),
        "uninvite" => ("DELETE", format!("/api/teams/{t}/invites/{}", segment(&a.id, "invite")?), None),
        "keys" => ("GET", format!("/api/teams/{t}/keys"), None),
        "unkey" => ("DELETE", format!("/api/teams/{t}/keys/{}", segment(&a.id, "key")?), None),
        "secrets" => ("GET", format!("/api/teams/{t}/secrets"), None),
        "unset" => ("DELETE", format!("/api/teams/{t}/secrets/{}", segment(&a.name, "secret")?), None),
        "logout" => ("DELETE", format!("/api/teams/{t}/secrets/logins/{}", segment(&a.name, "sign-in")?), None),
        "deploys" => ("GET", format!("/api/teams/{t}/deploys"), None),
        other => {
            return Err(WireError::invalid(format!(
                "hub {other}: show, create, restart, delete, invites, uninvite, join, keys, unkey, secrets, unset, logout or deploys"
            )));
        }
    })
}

/// One call to the grid with this computer's sign-in.
async fn grid(st: &Arc<St>, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
    let (acct, token) = st.signed_in().await.map_err(WireError::unavailable)?;
    let (status, v) = net::call(&st.http, method, &format!("{}{path}", acct.grid), Some(&token), body, &[]).await?;
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    Ok(v)
}

pub async fn hub(st: &Arc<St>, a: &api::HubArgs) -> Result<Value> {
    let (method, path, body) = hub_call(a)?;
    let v = grid(st, method, &path, body.as_ref()).await?;
    // a hub joined, turned on or deleted: the machines list follows now, not at the next start
    if matches!(a.op.as_str(), "join" | "create" | "delete") {
        crate::calls::warn("the hubs of your teams", crate::hub_peers::sync_hubs(st).await);
    }
    Ok(v)
}

/// The hubs of your teams, from `/api/me`: `[{slug, name, role, url}]`.
pub async fn list(st: &Arc<St>) -> Result<Value> {
    let me = grid(st, "GET", "/api/me", None).await?;
    Ok(me.get("hubs").cloned().unwrap_or_else(|| json!([])))
}

pub async fn invite(st: &Arc<St>, a: &api::HubInviteArgs) -> Result<Value> {
    let t = slug(&a.team)?;
    let body = json!({ "role": if a.role.is_empty() { "member" } else { &a.role }, "days": a.days, "uses": a.uses.unwrap_or(1) });
    grid(st, "POST", &format!("/api/teams/{t}/invites"), Some(&body)).await
}

pub async fn key_create(st: &Arc<St>, a: &api::HubKeyArgs) -> Result<Value> {
    let t = slug(&a.team)?;
    let rights = if a.rights.is_empty() { "deploy" } else { &a.rights };
    grid(st, "POST", &format!("/api/teams/{t}/keys"), Some(&json!({ "name": a.name, "rights": rights }))).await
}

pub async fn secret_set(st: &Arc<St>, a: &api::HubSecretArgs) -> Result<Value> {
    let t = slug(&a.team)?;
    if a.login {
        let agent = segment(&a.name, "sign-in")?;
        return grid(st, "PUT", &format!("/api/teams/{t}/secrets/logins/{agent}"), Some(&json!({ "token": a.value }))).await;
    }
    grid(st, "PUT", &format!("/api/teams/{t}/secrets"), Some(&json!({ "key": a.name, "value": a.value }))).await
}

/// A `resources.*` command on the hub's own wisp, as you: its result, or its refusal in its own words.
pub async fn call(st: &Arc<St>, a: &api::HubCallArgs) -> Result<Value> {
    if !api::HUB_CALLS.contains(&a.cmd.as_str()) {
        return Err(WireError::forbidden(format!("{}: a hub call runs resources.* only (what deploying needs)", a.cmd)));
    }
    let t = slug(&a.team)?;
    let hub = grid(st, "GET", &format!("/api/teams/{t}/hub"), None).await?;
    let url = hub.get("url").and_then(Value::as_str).ok_or_else(|| WireError::unavailable(format!("{t} has no hub address")))?;
    let (_, token) = st.signed_in().await.map_err(WireError::unavailable)?;
    let key = format!("hub-{}", hex(&rand::random::<[u8; 12]>()));
    let source: String = a.source.chars().filter(|c| !c.is_control()).take(200).collect();
    let headers = [("idempotency-key", key.as_str()), ("x-wisp-deploy-source", source.as_str())];
    let body = json!({ "cmd": a.cmd, "args": a.args });
    let (status, v) = net::call(&st.http, "POST", &format!("{url}/api/cmd"), Some(&token), Some(&body), &headers).await?;
    answer(status, &v)
}

/// `/api/cmd`'s answer: its result, or its error as the hub said it (a grid refusal has `error` as text).
pub fn answer(status: u16, v: &Value) -> Result<Value> {
    if v.get("ok") == Some(&Value::Bool(true)) {
        return Ok(v.get("result").cloned().unwrap_or(Value::Null));
    }
    if let Some(m) = v.pointer("/error/message").and_then(Value::as_str) {
        let mut e = net::refused(status, &json!({ "error": m }));
        e.retry = v.pointer("/error/retry") == Some(&Value::Bool(true));
        return Err(e);
    }
    Err(net::refused(status, v))
}

fn hex(b: &[u8]) -> String {
    use std::fmt::Write as _;
    b.iter().fold(String::new(), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(op: &str, team: &str) -> api::HubArgs {
        api::HubArgs { op: op.into(), team: team.into(), ..api::HubArgs::default() }
    }

    #[test]
    fn hub_ops_are_the_grids_hub_api_and_nothing_else_reaches_a_path() {
        let path = |a: &api::HubArgs| hub_call(a).map(|c| (c.0, c.1)).unwrap_or_else(|e| (e.message.leak() as &str, String::new()));
        assert_eq!(path(&op("show", "acme")), ("GET", "/api/teams/acme/hub".to_owned()));
        assert_eq!(path(&op("deploys", "acme")), ("GET", "/api/teams/acme/deploys".to_owned()));
        assert_eq!(path(&api::HubArgs { id: "0a1b".into(), ..op("unkey", "acme") }), ("DELETE", "/api/teams/acme/keys/0a1b".to_owned()));
        assert_eq!(
            path(&api::HubArgs { name: "GH_TOKEN".into(), ..op("unset", "acme") }),
            ("DELETE", "/api/teams/acme/secrets/GH_TOKEN".to_owned())
        );
        // nothing that isn't a slug or a segment reaches the grid's path
        assert!(hub_call(&op("show", "../keys")).is_err());
        assert!(hub_call(&api::HubArgs { id: "../../x".into(), ..op("unkey", "acme") }).is_err());
        assert!(hub_call(&op("dance", "acme")).is_err());
        let j = hub_call(&api::HubArgs { code: "https://grid.test/join/wgi_abc?x=1".into(), ..op("join", "") }).unwrap();
        assert_eq!((j.0, j.1.as_str(), j.2.unwrap()["code"].clone()), ("POST", "/api/join", json!("wgi_abc")));
        assert!(hub_call(&api::HubArgs { code: "wgk_abc".into(), ..op("join", "") }).is_err());
    }

    #[test]
    fn a_hub_calls_answer_is_its_result_or_its_words() {
        assert_eq!(answer(200, &json!({ "ok": true, "result": { "applied": 2 } })).unwrap(), json!({ "applied": 2 }));
        let e =
            answer(409, &json!({ "ok": false, "error": { "code": "conflict", "message": "Deploy #3 by ci is running", "retry": true } }))
                .unwrap_err();
        assert!(e.message.contains("Deploy #3") && e.retry, "{e:?}");
        let e = answer(403, &json!({ "error": "That hub is acme's: ask one of its owners to add you." })).unwrap_err();
        assert!(e.message.contains("acme's"), "{e:?}");
    }
}
