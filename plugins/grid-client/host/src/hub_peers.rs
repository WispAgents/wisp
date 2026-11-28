//! The hubs of your teams as machines (ADR-0147 §9): each a `grid` peer at the hub's address, opened with this
//! computer's device token (the hub's edge lets a member's device in, by their role). So the app's list, `--on acme`
//! and the TUI reach a hub as they reach the cloud box. A hub you're no longer in leaves the list on the next sync.
use std::sync::Arc;

use serde_json::{Value, json};
use wisp_kernel::Result;
use wisp_machines_api::{AddArgs, GRID};

use crate::calls::{self, Peer};
use crate::net;
use crate::peers::host_port;
use crate::state::St;

/// A hub's peer name: its team's name and "hub" (`Acme hub`).
pub fn hub_name(name: &str, slug: &str) -> String {
    format!("{} hub", if name.trim().is_empty() { slug } else { name.trim() })
}

/// What to do with the peers from one grid: (hubs to add `(slug, name, url)`, peer ids to remove).
pub fn plan(have: &[Peer], grid: &str, hubs: &[Value]) -> (Vec<(String, String, String)>, Vec<String>) {
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let ours: Vec<&Peer> =
        have.iter().filter(|p| p.transport == GRID && p.field("grid") == Some(grid) && p.field("hub").is_some()).collect();
    let add = hubs
        .iter()
        .filter(|h| !s(h, "url").is_empty() && !ours.iter().any(|p| p.field("hub") == Some(s(h, "slug").as_str())))
        .map(|h| (s(h, "slug"), s(h, "name"), s(h, "url")))
        .collect();
    let gone = ours
        .iter()
        .filter(|p| !hubs.iter().any(|h| p.field("hub") == h.get("slug").and_then(Value::as_str)))
        .map(|p| p.id.clone())
        .collect();
    (add, gone)
}

/// Every signed-in account's hubs, added or taken off: how many changed.
pub async fn sync_hubs(st: &Arc<St>) -> Result<usize> {
    let mut n = 0;
    for (a, token) in st.signed_in_all().await {
        let (status, me) = net::call(&st.http, "GET", &format!("{}/api/me", a.grid), Some(&token), None, &[]).await?;
        if status >= 400 {
            continue;
        }
        let hubs = me.get("hubs").and_then(Value::as_array).cloned().unwrap_or_default();
        let have = calls::peers(&st.ctx).await;
        let (add, gone) = plan(&have, &a.grid, &hubs);
        for id in gone {
            calls::warn("machines.remove (a hub you left)", calls::remove(&st.ctx, &id).await);
            n += 1;
        }
        for (slug, name, url) in add {
            let (host, port) = host_port(&url);
            let data = [("url", url.as_str()), ("grid", a.grid.as_str()), ("handle", a.handle.as_str()), ("hub", slug.as_str())]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), json!(v)))
                .collect();
            let add = AddArgs {
                name: Some(hub_name(&name, &slug)),
                transport: Some(GRID.into()),
                address: Some(host),
                port: Some(json!(port)),
                token: Some(token.clone()),
                data,
                connect: Some(false),
                ..AddArgs::default()
            };
            let id = calls::add(&st.ctx, add).await?;
            // watched, not woken, until it's used (as the cloud box)
            calls::warn("machines.follow (a hub)", calls::follow(&st.ctx, &id).await);
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_hub_is_added_a_left_one_removed_and_another_grids_kept() {
        let peer = |id: &str, grid: &str, hub: &str| Peer {
            id: id.into(),
            transport: GRID.into(),
            data: [("grid", grid), ("hub", hub)].into_iter().map(|(k, v)| (k.to_owned(), json!(v))).collect(),
            ..Peer::default()
        };
        let have = vec![peer("m1", "https://g.test", "acme"), peer("m2", "https://g.test", "old"), peer("m3", "https://h.test", "old")];
        let hubs = vec![
            json!({ "slug": "acme", "name": "Acme", "url": "https://acme.g.test" }),
            json!({ "slug": "zeta", "name": "", "url": "https://zeta.g.test" }),
        ];
        let (add, gone) = plan(&have, "https://g.test", &hubs);
        assert_eq!(add, vec![("zeta".into(), String::new(), "https://zeta.g.test".into())]);
        assert_eq!(gone, vec!["m2".to_owned()], "left on this grid; another grid's is its own");
        assert_eq!(hub_name("", "zeta"), "zeta hub");
        assert_eq!(hub_name("Acme", "acme"), "Acme hub");
    }
}
