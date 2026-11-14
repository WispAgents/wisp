//! The grid's machines as wisp.machines peers (through its commands, as any plugin would): the cloud box
//! (transport `grid`) and the other computers on the account (transport `grid-tunnel`), each with the
//! device token as its secret. This computer's own entry is never added as a peer of itself. With several
//! accounts (ADR-0061) each peer carries the `grid` and `handle` it came from, and every account's computers are
//! listed.
use std::sync::Arc;

use serde_json::{Value, json};
use wisp_kernel::Result;
use wisp_machines_api::{AddArgs, GRID, GRID_TUNNEL};

use crate::calls::{self, Peer};
use crate::net;
use crate::state::St;

/// The origin's host and port (`http://ada.wisp.localhost:8080` → (`ada.wisp.localhost`, 8080)).
pub fn host_port(url: &str) -> (String, u16) {
    let https = url.starts_with("https:");
    let host = net::host_of(url);
    match host.rsplit_once(':') {
        Some((h, p)) if p.bytes().all(|b| b.is_ascii_digit()) && !p.is_empty() => {
            (h.to_owned(), p.parse().unwrap_or(if https { 443 } else { 80 }))
        }
        _ => (host.to_owned(), if https { 443 } else { 80 }),
    }
}

/// The cloud box's peer among `have`: the one it was, or this account's `grid` peer (this grid, this handle:
/// matched on the grid alone, a second account on it renewed the first's box instead of adding its own).
pub(crate) fn cloud_of<'a>(have: &'a [Peer], prev: &str, grid: &str, handle: &str) -> Option<&'a Peer> {
    have.iter().find(|p| {
        (!prev.is_empty() && p.id == prev) || (p.transport == GRID && p.field("grid") == Some(grid) && p.field("handle") == Some(handle))
    })
}

/// A new cloud box's name: `Cloud`, or `Cloud (<handle>)` when one is called that already (v1 machines.js
/// `addGrid`: a second account's box is told apart in Run on, Machines and every toast).
pub(crate) fn cloud_name(have: &[Peer], handle: &str) -> String {
    if have.iter().any(|p| p.name == "Cloud") { format!("Cloud ({handle})") } else { "Cloud".into() }
}

/// A grid peer's transport fields.
fn data(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, Value> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), json!(v))).collect()
}

/// The cloud box's peer: added, or its token renewed. → its id.
pub async fn cloud_box(st: &Arc<St>, prev: &str, grid: &str, handle: &str, url: &str, token: &str) -> Result<String> {
    let have = calls::peers(&st.ctx).await;
    if let Some(p) = cloud_of(&have, prev, grid, handle) {
        calls::update(&st.ctx, &p.id, AddArgs { token: Some(token.to_owned()), ..AddArgs::default() }).await?;
        return Ok(p.id.clone());
    }
    let (host, port) = host_port(url);
    let add = AddArgs {
        name: Some(cloud_name(&have, handle)),
        transport: Some(GRID.into()),
        address: Some(host),
        port: Some(json!(port)),
        token: Some(token.to_owned()),
        data: data(&[("url", url), ("grid", grid), ("handle", handle)]),
        connect: Some(false),
        ..AddArgs::default()
    };
    let id = calls::add(&st.ctx, add).await?;
    // a cloud computer is watched, not woken, until it's used
    calls::warn("machines.follow (the cloud box)", calls::follow(&st.ctx, &id).await);
    Ok(id)
}

/// Every account's other computers as `grid-tunnel` peers (new ones added, renamed ones renamed). An account
/// the grid refuses is skipped (logged); the others still sync.
pub async fn sync_computers(st: &Arc<St>) -> Result<usize> {
    let mut n = 0;
    let mut first_err = None;
    for (a, token) in st.signed_in_all().await {
        match sync_account(st, &a, &token).await {
            Ok(k) => n += k,
            Err(e) => {
                tracing::warn!(error = %e.message, grid = %a.grid, "grid-client: an account's computers");
                first_err.get_or_insert(e);
            }
        }
    }
    match first_err {
        Some(e) if n == 0 => Err(e),
        _ => Ok(n),
    }
}

async fn sync_account(st: &Arc<St>, a: &crate::state::Account, token: &str) -> Result<usize> {
    let (status, list) = net::call(&st.http, "GET", &format!("{}/api/computers", a.grid), Some(token), None, &[]).await?;
    if status >= 400 {
        return Err(net::refused(status, &list));
    }
    let have = calls::peers(&st.ctx).await;
    let mut n = 0;
    for c in list.as_array().into_iter().flatten() {
        let (Some(cid), Some(url)) = (c.get("id").and_then(Value::as_str), c.get("url").and_then(Value::as_str)) else { continue };
        if cid == a.computer_id {
            continue;
        }
        let name = c.get("name").and_then(Value::as_str).unwrap_or("Computer");
        if let Some(p) = have.iter().find(|p| p.field("computer") == Some(cid) && p.field("grid").is_none_or(|g| g == a.grid)) {
            if p.name != name {
                calls::warn(
                    "machines.update (a computer's name)",
                    calls::update(&st.ctx, &p.id, AddArgs { name: Some(name.to_owned()), ..AddArgs::default() }).await,
                );
            }
            continue;
        }
        let (host, port) = host_port(url);
        let label = c.get("label").and_then(Value::as_str).unwrap_or_default();
        let add = AddArgs {
            name: Some(name.to_owned()),
            transport: Some(GRID_TUNNEL.into()),
            address: Some(host),
            port: Some(json!(port)),
            token: Some(token.to_owned()),
            data: data(&[("url", url), ("grid", &a.grid), ("handle", &a.handle), ("computer", cid), ("label", label)]),
            ..AddArgs::default()
        };
        calls::add(&st.ctx, add).await?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_split_into_host_and_port() {
        assert_eq!(host_port("http://ada.wisp.localhost:8080"), ("ada.wisp.localhost".into(), 8080));
        assert_eq!(host_port("https://ada.grid.trywisp.dev"), ("ada.grid.trywisp.dev".into(), 443));
    }

    /// machines.list flattens a peer's transport fields (`grid`, not `data.grid`): read by pointer at
    /// `/data/grid`, a renewed sign-in never found its cloud box and computers were added again each sync.
    #[test]
    fn the_cloud_box_and_computers_are_found_by_their_flattened_fields() {
        let peer =
            |id: &str, t: &str, k: &str, v: &str| Peer { id: id.into(), transport: t.into(), data: data(&[(k, v)]), ..Peer::default() };
        let boxed = |id: &str, grid: &str, handle: &str| Peer {
            id: id.into(),
            name: "Cloud".into(),
            transport: GRID.into(),
            data: data(&[("grid", grid), ("handle", handle)]),
        };
        let have = vec![peer("m1", GRID_TUNNEL, "computer", "c1"), boxed("m2", "https://g.test", "ada")];
        assert_eq!(cloud_of(&have, "gone", "https://g.test", "ada").map(|p| p.id.as_str()), Some("m2"));
        assert_eq!(cloud_of(&have, "m1", "https://other.test", "ada").map(|p| p.id.as_str()), Some("m1"), "the one it was");
        assert!(cloud_of(&have, "gone", "https://other.test", "ada").is_none());
    }

    /// A second account on the same grid gets a box of its own, told apart by its handle (v2's first port matched
    /// on the grid alone and renewed the first account's box with the second's token).
    #[test]
    fn a_second_account_on_one_grid_gets_its_own_cloud_box_named_with_its_handle() {
        let have = vec![Peer {
            id: "m2".into(),
            name: "Cloud".into(),
            transport: GRID.into(),
            data: data(&[("grid", "https://g.test"), ("handle", "ada")]),
        }];
        assert!(cloud_of(&have, "", "https://g.test", "bea").is_none());
        assert_eq!(cloud_name(&have, "bea"), "Cloud (bea)");
        assert_eq!(cloud_name(&[], "ada"), "Cloud");
    }
}
