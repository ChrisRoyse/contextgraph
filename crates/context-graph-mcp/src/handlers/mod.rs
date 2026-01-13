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

// TASK-METAUTL-P0-005: Meta-learning handlers for self-correction tools
pub mod meta_learning;

// TASK-GWT-P0-002: Background stepper for Kuramoto oscillator network
pub mod kuramoto_stepper;

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

// Re-export Kuramoto stepper for wiring (TASK-GWT-P0-002)
// Note: These are public API re-exports - unused within this crate but available to consumers
#[allow(unused_imports)]
pub use self::kuramoto_stepper::{KuramotoStepper, KuramotoStepperConfig, KuramotoStepperError};

// ============================================================================
// Factory Functions (TASK-IDENTITY-P0-001)
// ============================================================================

/// Create GwtSystemProviderImpl sharing the listener's monitor.
///
/// This ensures MCP tools read from the same monitor that
/// processes workspace events, fixing the dual monitor desync bug.
///
/// # Arguments
/// * `listener` - The IdentityContinuityListener that owns the monitor
///
/// # Returns
/// GwtSystemProviderImpl that shares the listener's monitor
///
/// # TASK-IDENTITY-P0-001
#[allow(dead_code)]
pub fn create_gwt_provider_with_listener(
    listener: &context_graph_core::gwt::listeners::IdentityContinuityListener
) -> gwt_providers::GwtSystemProviderImpl {
    let shared_monitor = listener.monitor();
    gwt_providers::GwtSystemProviderImpl::with_shared_monitor(shared_monitor)
}
