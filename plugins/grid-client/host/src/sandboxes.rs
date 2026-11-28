//! Sandboxes as peers (ADR-0041): a box of its own on the grid for one task, made here and kept in machines as a
//! `grid` peer that opens with the sandbox's own token (never this computer's device token), so a task routed
//! there (`--on <sandbox>`) runs in it and nothing in it can reach this computer or the user's own box.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_grid_client_api as api;
use wisp_kernel::{Result, WireError};

use crate::calls::{self, Peer};
use crate::net;
use crate::peers::host_port;
use crate::state::St;

fn machine_of(list: &[Peer], id: &str) -> String {
    list.iter().find(|p| p.field("sandbox") == Some(id)).map(|p| p.id.clone()).unwrap_or_default()
}

/// How this wisp asks the grid for sandboxes: its grid, handle, and credential (an `Authorization` value).
pub(crate) struct Asker {
    grid: String,
    handle: String,
    auth: String,
}

/// A team's hub box (ADR-0147 §8) makes its own sandboxes with its box secret (`Box <handle> <secret>`, which the grid
/// takes from a hub's box only); anywhere else, this computer's device token. A sandbox never makes one.
fn box_asker(env: &dyn Fn(&str) -> Option<String>) -> Option<Asker> {
    if env("WISP_BOX").as_deref() != Some("1") || env("WISP_SANDBOX").as_deref() == Some("1") {
        return None;
    }
    let grid = env("WISP_GRID_URL").filter(|g| !g.is_empty())?.trim_end_matches('/').to_owned();
    let secret = env("WISP_TRUSTED_PROXY_SECRET").filter(|s| !s.is_empty())?;
    let public = env("WISP_PUBLIC_URL").filter(|s| !s.is_empty())?;
    let handle = public.split("://").nth(1).unwrap_or(&public).split(['/', ':', '.']).next().unwrap_or_default().to_owned();
    Some(Asker { grid, auth: format!("Box {handle} {secret}"), handle })
}

pub(crate) async fn asker(st: &Arc<St>) -> Result<Asker> {
    match st.signed_in().await {
        Ok((a, token)) => Ok(Asker { grid: a.grid, handle: a.handle, auth: format!("Bearer {token}") }),
        Err(why) => box_asker(&|k| std::env::var(k).ok()).ok_or_else(|| WireError::unavailable(why)),
    }
}

/// One call to the grid as [`asker`] says.
async fn ask(st: &Arc<St>, who: &Asker, method: &str, path: &str, body: Option<&Value>) -> Result<(u16, Value)> {
    net::call(&st.http, method, &format!("{}{path}", who.grid), None, body, &[("authorization", who.auth.as_str())]).await
}

/// Makes one on the grid and adds its peer.
pub async fn create(st: &Arc<St>, name: &str) -> Result<api::Sandbox> {
    let a = asker(st).await?;
    let (status, v) = ask(st, &a, "POST", "/api/sandboxes", Some(&json!({ "name": name }))).await?;
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let (id, url, own) = (s("id"), s("url"), s("token"));
    if id.is_empty() || url.is_empty() || own.is_empty() {
        return Err(WireError::internal("the grid made a sandbox but didn't say where or with what token"));
    }
    let (host, port) = host_port(&url);
    let label = format!("Sandbox {}", if s("name").is_empty() { id.clone() } else { s("name") });
    // its own token: the peer opens that sandbox and nothing else
    let data = [("url", &url), ("grid", &a.grid), ("handle", &a.handle), ("sandbox", &id)]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), json!(v)))
        .collect();
    let add = wisp_machines_api::AddArgs {
        name: Some(label),
        transport: Some(wisp_machines_api::GRID.into()),
        address: Some(host),
        port: Some(json!(port)),
        token: Some(own.trim_start_matches("Bearer ").to_owned()),
        data,
        connect: Some(false),
        ..wisp_machines_api::AddArgs::default()
    };
    let machine = calls::add(&st.ctx, add).await?;
    calls::warn("machines.follow (a sandbox)", calls::follow(&st.ctx, &machine).await);
    Ok(api::Sandbox { id, name: s("name"), url, machine })
}

pub async fn list(st: &Arc<St>) -> Result<Vec<api::Sandbox>> {
    let a = asker(st).await?;
    let (status, v) = ask(st, &a, "GET", "/api/sandboxes", None).await?;
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    let have = calls::peers(&st.ctx).await;
    Ok(v.get("sandboxes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|x| {
            let s = |k: &str| x.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            api::Sandbox { machine: machine_of(&have, &s("id")), id: s("id"), name: s("name"), url: s("url") }
        })
        .collect())
}

/// Deletes it on the grid (its box and home), then its peer here.
pub async fn remove(st: &Arc<St>, id: &str) -> Result<Value> {
    let a = asker(st).await?;
    let (status, v) = ask(st, &a, "DELETE", &format!("/api/sandboxes/{id}"), None).await?;
    if status >= 400 && status != 404 {
        return Err(net::refused(status, &v));
    }
    let machine = machine_of(&calls::peers(&st.ctx).await, id);
    if !machine.is_empty() {
        calls::warn("machines.remove (a sandbox)", calls::remove(&st.ctx, &machine).await);
    }
    Ok(json!({ "ok": true, "machine": machine }))
}

/// One call to the grid's `/api/sandboxes…` with this computer's device token (ADR-0047).
async fn grid(st: &Arc<St>, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
    let a = asker(st).await?;
    let (status, v) = ask(st, &a, method, &format!("/api/sandboxes{path}"), body).await?;
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    Ok(v)
}

/// Whether the grid makes sandboxes, the sign-ins and env names they start with, and the list.
pub async fn info(st: &Arc<St>) -> Result<Value> {
    grid(st, "GET", "", None).await
}

/// A sign-in every sandbox starts with (`claude`: a `claude setup-token` token), sealed on the grid.
pub async fn login(st: &Arc<St>, a: &api::SandboxSetArgs) -> Result<Value> {
    if a.agent.trim().is_empty() || a.token.trim().is_empty() {
        return Err(WireError::invalid("which sign-in, and its token?"));
    }
    grid(st, "PUT", &format!("/logins/{}", a.agent.trim()), Some(&json!({ "token": a.token.trim() }))).await
}

pub async fn logout(st: &Arc<St>, a: &api::SandboxSetArgs) -> Result<Value> {
    grid(st, "DELETE", &format!("/logins/{}", a.agent.trim()), None).await
}

/// An env var every sandbox gets; an empty value takes it out.
pub async fn env(st: &Arc<St>, a: &api::SandboxSetArgs) -> Result<Value> {
    let key = a.key.trim();
    if key.is_empty() {
        return Err(WireError::invalid("which variable?"));
    }
    if a.value.is_empty() {
        grid(st, "DELETE", &format!("/env/{key}"), None).await
    } else {
        grid(st, "PUT", "/env", Some(&json!({ "key": key, "value": a.value }))).await
    }
}

/// The HTTP call a team op is.
pub fn team_call(a: &api::TeamsArgs) -> Result<(&'static str, String, Option<Value>)> {
    let need = |v: &str, what: &str| {
        if v.trim().is_empty() { Err(WireError::invalid(format!("wisp team {}: which {what}?", a.op))) } else { Ok(()) }
    };
    Ok(match a.op.as_str() {
        "ls" | "list" | "" => ("GET", "/api/teams".into(), None),
        "new" | "create" => {
            need(&a.slug, "team (its slug)")?;
            ("POST", "/api/teams".into(), Some(json!({ "slug": a.slug, "name": if a.name.is_empty() { &a.slug } else { &a.name } })))
        }
        "show" => {
            need(&a.slug, "team")?;
            ("GET", format!("/api/teams/{}", a.slug), None)
        }
        "add" => {
            need(&a.slug, "team")?;
            need(&a.handle, "member")?;
            (
                "POST",
                format!("/api/teams/{}/members", a.slug),
                Some(json!({ "handle": a.handle, "role": if a.role.is_empty() { "member" } else { &a.role } })),
            )
        }
        "rm" | "remove" => {
            need(&a.slug, "team")?;
            need(&a.handle, "member")?;
            ("DELETE", format!("/api/teams/{}/members/{}", a.slug, a.handle), None)
        }
        other => return Err(WireError::invalid(format!("wisp team {other}: ls, new, show, add or rm"))),
    })
}

pub async fn teams(st: &Arc<St>, a: &api::TeamsArgs) -> Result<Value> {
    let (acct, token) = st.signed_in().await.map_err(WireError::unavailable)?;
    let (method, path, body) = team_call(a)?;
    let (status, v) = net::call(&st.http, method, &format!("{}{path}", acct.grid), Some(&token), body.as_ref(), &[]).await?;
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sandbox_peer_is_found_by_its_sandbox_id() {
        let peer = |id: &str, k: &str, v: &str| Peer { id: id.into(), data: [(k.to_owned(), json!(v))].into(), ..Peer::default() };
        let list = vec![peer("m1", "computer", "c1"), peer("m2", "sandbox", "0a1b2c3d")];
        assert_eq!(machine_of(&list, "0a1b2c3d"), "m2");
        assert_eq!(machine_of(&list, "ffffffff"), "");
    }

    #[test]
    fn a_hub_box_asks_for_sandboxes_with_its_own_secret_and_a_sandbox_never() {
        fn env(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
            let pairs = pairs.to_vec();
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| (*v).to_owned())
        }
        let hub = [
            ("WISP_BOX", "1"),
            ("WISP_GRID_URL", "https://grid.example/"),
            ("WISP_TRUSTED_PROXY_SECRET", "s3cret"),
            ("WISP_PUBLIC_URL", "https://acme.grid.example"),
        ];
        let a = box_asker(&env(&hub)).unwrap_or_else(|| panic!("a box asks"));
        assert_eq!((a.grid.as_str(), a.handle.as_str(), a.auth.as_str()), ("https://grid.example", "acme", "Box acme s3cret"));
        let sandbox = [
            ("WISP_BOX", "1"),
            ("WISP_SANDBOX", "1"),
            ("WISP_GRID_URL", "g"),
            ("WISP_TRUSTED_PROXY_SECRET", "s"),
            ("WISP_PUBLIC_URL", "u"),
        ];
        assert!(box_asker(&env(&sandbox)).is_none(), "a sandbox makes none");
        assert!(box_asker(&env(&[("WISP_GRID_URL", "g")])).is_none(), "not a box");
    }

    #[test]
    fn team_ops_are_the_grids_team_api() {
        let t = |op: &str, slug: &str, handle: &str| api::TeamsArgs {
            op: op.into(),
            slug: slug.into(),
            handle: handle.into(),
            ..api::TeamsArgs::default()
        };
        assert_eq!(team_call(&t("ls", "", "")).map(|c| (c.0, c.1)).ok(), Some(("GET", "/api/teams".to_owned())));
        let (m, p, b) = team_call(&t("add", "acme", "bo")).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(
            (m, p.as_str(), b.map(|b| b["role"].clone())),
            ("POST", "/api/teams/acme/members", Some(json!("member"))),
            "a member unless said"
        );
        assert_eq!(team_call(&t("rm", "acme", "bo")).map(|c| (c.0, c.1)).ok(), Some(("DELETE", "/api/teams/acme/members/bo".to_owned())));
        assert!(team_call(&t("add", "acme", "")).is_err() && team_call(&t("dance", "", "")).is_err());
    }
}
