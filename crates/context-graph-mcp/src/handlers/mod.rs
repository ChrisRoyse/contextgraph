//! Request handlers for MCP methods.
//!
//! This module provides the main `Handlers` struct that dispatches JSON-RPC
//! requests to their appropriate handler methods.
//!
//! # Module Organization
//!
//! - `core`: Core Handlers struct and dispatch logic
//! - `lifecycle`: MCP lifecycle handlers (initialize, shutdown)
//! - `tools`: MCP tool call handlers
//! - `memory`: Legacy memory operation handlers
//! - `search`: Multi-embedding weighted search handlers (TASK-S002)
//! - `purpose`: Purpose and goal alignment handlers (TASK-S003)
//! - `johari`: Johari quadrant handlers (TASK-S004)
//! - `utl`: UTL computation handlers
//! - `system`: System status and health handlers
//! - `gwt_traits`: GWT provider traits for consciousness/Kuramoto integration (TASK-GWT-001)
//! - `gwt_providers`: Real GWT provider implementations wrapping actual components (TASK-GWT-001)
//! - `atc`: Adaptive Threshold Calibration handlers (TASK-ATC-001)
//! - `steering`: Steering subsystem handlers (TASK-STEERING-001)
//! - `causal`: Causal inference handlers (TASK-CAUSAL-001)
//! - `teleological`: Teleological search, fusion, and profile handlers (TELEO-H1 to TELEO-H5)
//! - `autonomous`: NORTH autonomous system handlers (TASK-AUTONOMOUS-MCP)

mod atc;
mod autonomous;
mod causal;
mod core;
mod dream;
mod johari;
mod lifecycle;
mod memory;
mod neuromod;
// NOTE: mod north_star REMOVED - Manual North Star tools created single 1024D embeddings
// that cannot be meaningfully compared to 13-embedder teleological arrays.
// Use purpose/ endpoints which work with the autonomous teleological system.
pub mod gwt_providers;
pub mod gwt_traits;
mod purpose;
mod search;
mod steering;
mod system;
mod teleological;
mod tools;
mod utl;

#[cfg(test)]
mod tests;

// Re-export the main Handlers struct for backward compatibility
pub use self::core::Handlers;

// Re-export MetaUtlTracker for server initialization
pub use self::core::MetaUtlTracker;

// Re-export GWT traits for external use (TASK-GWT-001)
// Note: These are public API re-exports - unused within this crate but available to consumers
#[allow(unused_imports)]
pub use self::gwt_traits::{
    GwtSystemProvider, KuramotoProvider, MetaCognitiveProvider, SelfEgoProvider, WorkspaceProvider,
    NUM_OSCILLATORS,
};

// Re-export GWT provider implementations for wiring (TASK-GWT-001)
// Note: These are public API re-exports - unused within this crate but available to consumers
#[allow(unused_imports)]
pub use self::gwt_providers::{
    GwtSystemProviderImpl, KuramotoProviderImpl, MetaCognitiveProviderImpl, SelfEgoProviderImpl,
    WorkspaceProviderImpl,
};
