//! wisp.connectors: the MCP servers agents get. `kinds`: the `connector` resource kind and what runs get
//! (`runtime.run@1`); `pane`: Settings → Connectors' commands, rows and the sign-in callback; `auth`: a
//! signed-in connector's token kept fresh.

mod agent;
mod auth;
mod call;
mod events;
mod hook;
mod kinds;
mod pane;
mod stream;

use wisp_kernel::{Manifest, Plugin, Result, Setup};

pub use crate::pane::ConfigHome;

#[derive(Debug, Default)]
pub struct ConnectorsPlugin;

impl Plugin for ConnectorsPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let (stored, kind, runs) = crate::kinds::register(s);
        let pane = crate::pane::register(s, stored.clone(), kind, runs);
        crate::call::register(s, stored, pane);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        assert_eq!(ConnectorsPlugin.manifest().id, "wisp.connectors");
    }
}
