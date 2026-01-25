#![deny(deprecated)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::type_complexity)]
#![allow(clippy::result_large_err)]

//! Context Graph MCP Server Library
//!
//! JSON-RPC 2.0 server implementing the Model Context Protocol (MCP)
//! for the Ultimate Context Graph system.
//!
//! This library exposes the handlers and protocol types for integration testing.
//!
//! # Adapters
//!
//! The `adapters` module provides bridge types between specialized implementations
//! and core traits. Notable adapters:
//!
//! - [`UtlProcessorAdapter`]: Bridges the real UTL processor to the core trait interface
//!
//! # Blocked Adapters
//!
//! - `EmbeddingProviderAdapter`: Blocked until TASK-F007 implements multi-array provider.

pub mod adapters;
pub mod gpu_clustering;
pub mod handlers;
pub mod middleware;
pub mod protocol;
pub mod server;
pub mod tools;
pub mod transport;
pub mod weights;

// Re-export adapters for convenient access
pub use adapters::UtlProcessorAdapter;
