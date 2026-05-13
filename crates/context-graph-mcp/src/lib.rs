#![deny(deprecated)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::type_complexity)]
#![allow(clippy::result_large_err)]

//! Context Graph MCP Server Library
//!
//! JSON-RPC 2.0 server implementing the Model Context Protocol (MCP)
//! for the 14-embedder semantic memory retrieval system.
//!
//! This library exposes the handlers and protocol types for integration testing.

#[cfg(feature = "llm")]
compile_error!(
    "The MCP local LLM feature was retired on 2026-05-09. Use ME-JEPA evidence capture instead."
);

pub mod adapters;
pub mod daemon;
pub mod daemon_validate;
pub mod deprecation;
pub mod handlers;
pub mod health_probe;
pub mod monitoring;
pub mod protocol;
pub mod server;
pub mod telemetry;
pub mod tools;
pub mod weights;
