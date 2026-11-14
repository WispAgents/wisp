//! Several grid accounts, one current (ADR-0061; v1 web/js/features/cloud.js Add an account): switching which one
//! shares, sandboxes, keys and this computer's tunnel go through, and signing out of one. Signing in adds one
//! (`signin.rs`).
use std::sync::Arc;

use wisp_kernel::{Result, WireError};

use crate::calls::Peer;
use crate::state::{Account, St, token_key};

/// Whether a peer came from this account: its cloud box, or a computer or sandbox on its grid as it.
pub(crate) fn from_account(p: &Peer, a: &Account) -> bool {
    p.id == a.machine || (p.field("grid") == Some(a.grid.as_str()) && p.field("handle") == Some(a.handle.as_str()))
}

/// `grid-client.accountSwitch`: another signed-in account becomes current; the tunnel follows it.
pub async fn switch(st: &Arc<St>, key: &str) -> Result<()> {
    let was = st.accounts().current;
    if !st.change(|x| x.switch(key)).await? {
        return Err(WireError::not_found("Not signed in to that account here."));
    }
    if was != key {
        crate::computer::follow_current(st).await;
    }
    Ok(())
}

/// `grid-client.accountRemove`: signed out of one account: its token gone, its cloud box, computers and
/// sandboxes taken out of Machines (another account's on the same grid stay), and if it was current, the
/// first one left is current.
pub async fn remove(st: &Arc<St>, key: &str) -> Result<()> {
    let accounts = st.accounts();
    let Some(a) = accounts.get(key).cloned() else { return Err(WireError::not_found("Not signed in to that account here.")) };
    let current = accounts.current == key;
    if current {
        crate::computer::stop(st);
    }
    for p in crate::calls::peers(&st.ctx).await {
        if from_account(&p, &a) {
            crate::calls::warn("machines.remove (signed out)", crate::calls::remove(&st.ctx, &p.id).await);
        }
    }
    wisp_kernel::services::secrets(&st.ctx)?.delete(&token_key(key)).await?;
    st.change(|x| x.remove(key)).await?;
    if current {
        crate::computer::follow_current(st).await;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::state::Accounts;

    fn acct(grid: &str, handle: &str, added: i64) -> Account {
        Account { grid: grid.into(), handle: handle.into(), machine: format!("m-{handle}"), added, ..Account::default() }
    }

    #[test]
    fn accounts_add_switch_and_remove_keep_one_current() {
        let mut x = Accounts::default();
        assert!(x.current().is_none());
        x.add(acct("https://g1", "ada", 1));
        x.add(acct("https://g2", "ada", 2));
        x.add(acct("https://g1", "bea", 3));
        assert_eq!(x.list.len(), 3, "the same handle on two grids, two handles on one grid: three accounts");
        assert_eq!(x.current, "https://g1#bea", "the one signed in to last is current");
        // signing in again as one of them renews it in its place
        let mut again = acct("https://g2", "ada", 9);
        again.machine = "m-new".into();
        x.add(again);
        assert_eq!(x.list.iter().map(Account::key).collect::<Vec<_>>(), ["https://g1#ada", "https://g2#ada", "https://g1#bea"]);
        assert_eq!((x.current.as_str(), x.get("https://g2#ada").unwrap().machine.as_str()), ("https://g2#ada", "m-new"));
        assert!(x.switch("https://g1#ada"));
        assert!(!x.switch("https://g3#ada"), "an account not signed in to");
        assert_eq!(x.current, "https://g1#ada");
        // a removed non-current one leaves current alone; the current one hands over to the first left
        assert!(x.remove("https://g1#bea").is_some());
        assert_eq!(x.current, "https://g1#ada");
        x.remove("https://g1#ada");
        assert_eq!(x.current, "https://g2#ada");
        x.remove("https://g2#ada");
        assert_eq!((x.current.as_str(), x.list.len()), ("", 0));
        assert!(x.remove("https://g2#ada").is_none());
    }

    #[test]
    fn a_stale_current_falls_back_to_the_first_and_the_list_keeps_its_order() {
        let x = Accounts::new(vec![acct("https://g2", "bo", 5), acct("https://g1", "ada", 1)], "https://gone#x");
        assert_eq!((x.list[0].handle.as_str(), x.current.as_str()), ("ada", "https://g1#ada"));
        let infos = x.infos();
        assert_eq!(infos.iter().filter(|i| i.current).count(), 1);
        assert_eq!(x.on_grid("https://g2").map(|a| a.handle.as_str()), Some("bo"));
    }

    /// v1 signed one account out by removing its cloud box only, so its computers stayed, holding its token; v2's
    /// first port removed every peer of the grid, so signing out of one account dropped the other's computers too.
    #[test]
    fn signing_out_of_one_account_takes_only_its_own_peers() {
        let ada = acct("https://g1", "ada", 1);
        let peer = |id: &str, grid: &str, handle: &str| Peer {
            id: id.into(),
            data: [("grid", grid), ("handle", handle)].iter().map(|(k, v)| ((*k).to_owned(), serde_json::json!(v))).collect(),
            ..Peer::default()
        };
        assert!(from_account(&peer("m-ada", "", ""), &ada), "its cloud box");
        assert!(from_account(&peer("c1", "https://g1", "ada"), &ada), "its computer");
        assert!(!from_account(&peer("c2", "https://g1", "bea"), &ada), "another account's computer on the same grid");
        assert!(!from_account(&peer("c3", "https://g2", "ada"), &ada), "the same handle on another grid");
    }
}
