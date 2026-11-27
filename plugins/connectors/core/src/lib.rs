//! wisp-connectors-core: the pure, sans-IO part of wisp.connectors (no tokio, fs, net or clock: the
//! host injects them). The connector model and its checks, the harnesses' own
//! configs, OAuth/PKCE steps, the proxy's wire rules and the probe conversation, the agent tools'
//! texts and the sign-in callback page. Every message keeps v1's words: the page shows them as they are.

pub mod agent;
pub mod codex_toml;
pub mod error;
pub mod events;
pub mod harness;
pub mod model;
pub mod oauth;
pub mod page;
pub mod probe;
pub mod push;
pub mod rpc;
pub mod text;

pub use error::{ConnectorError, Result};
/// The one list of what a secret looks like, shared with wisp.resources (a formula's plaintext token, AAC-4).
pub use wisp_envmask::secret_scan;
