//! Adapters bridging external implementations to core traits.
//!
//! This module provides adapter types that bridge real implementations
//! from specialized crates to the core trait interfaces.
//!
//! # Architecture Note
//!
//! The adapters live in the MCP crate (not core) to avoid cyclic dependencies:
//! - `context-graph-utl` depends on `context-graph-core` for types
//! - `context-graph-embeddings` depends on `context-graph-core` for types
//! - The adapters bridge both, so they live in a crate that depends on both
//!
//! # Available Adapters
//!
//! - [`UtlProcessorAdapter`]: Bridges `context_graph_utl::UtlProcessor` to core trait
//! - [`LazyMultiArrayProvider`]: Wraps provider for lazy loading on MCP startup

pub mod lazy_provider;
pub mod utl_adapter;

// UtlProcessorAdapter is prepared for future use when UTL integration is enabled
#[allow(unused_imports)]
pub use utl_adapter::UtlProcessorAdapter;

// LazyMultiArrayProvider allows immediate MCP startup while models load in background
pub use lazy_provider::LazyMultiArrayProvider;
