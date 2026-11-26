//! wisp-computer-core: wisp.computer's pure logic, no I/O (ADR-0013's `core` rule). The host owns the
//! driver's pipe, the HTTP route and the clock; everything that decides lives here and is tested here.
//!
//! - [`policy`]: which of the driver's tools a task may see and call
//! - [`gateway`]: the `computer` MCP server's answers to a harness (initialize, tools/list, tools/call)
//! - [`client`]: wisp as the driver's MCP client over stdio (newline-delimited JSON-RPC)

pub mod client;
pub mod gateway;
pub mod policy;

/// The MCP versions the gateway speaks; the first is what it answers when the harness asks for another.
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// JSON-RPC's error codes the gateway answers with.
pub mod codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
}
