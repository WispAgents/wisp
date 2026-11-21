//! wisp-resources-core: wisp as code without IO (ADR-0026).
//!
//! - [`formula`]: reading `wisp.yml` / `.json` / `.toml`: `wisp: 1`, several documents, `include:`,
//!   `profiles:`, `${env:NAME}` filled in, `${secret:NAME}` kept as references.
//! - [`plan`]: the generic plan (a desired spec patches the current one), the order ops run in, and the
//!   plan as lines.
//! - [`export`]: resources back to a formula (YAML written here, JSON, TOML).
//! - [`schema`]: a spec checked against its kind's schema; the whole formula's schema for editors.
//! - [`overlay`]: a formula as one task's overlay, nothing applied: the sections it reads, a goal it names inlined,
//!   every name it gives checked (ADR-0111).

pub mod export;
pub mod expr;
pub mod formula;
pub mod module;
pub mod overlay;
pub mod plan;
pub mod schema;
