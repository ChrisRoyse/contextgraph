//! Adapters bridging external implementations to core traits.
//!
//! This module provides adapter types that bridge real implementations
//! from specialized crates to the core trait interfaces.
//!
//! # Available Adapters
//!
//! - [`LazyMultiArrayProvider`]: Wraps provider for lazy loading on MCP startup
pub mod lazy_provider;

// LazyMultiArrayProvider allows immediate MCP startup while models load in background
pub use lazy_provider::LazyMultiArrayProvider;
