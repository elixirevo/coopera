//! coopera-core — shared engine for the coopera harness.
//!
//! All persistent data lives in git. This crate holds the wiki model,
//! injection-pack builder (ranking + token budget), git shell-out wrapper,
//! hook I/O types, digest schema, and redaction. The CLI crate is a thin
//! command layer over this library.

pub mod archive;
pub mod config;
pub mod digest;
pub mod distill;
pub mod gitio;
pub mod hookio;
pub mod inject;
pub mod metrics;
pub mod presence;
pub mod redact;
pub mod spawn;
pub mod staleness;
pub mod wiki;
