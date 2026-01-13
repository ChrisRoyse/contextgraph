//! L5 Coherence Layer - Kuramoto synchronization and Global Workspace broadcast.
//!
//! The Coherence layer implements Global Workspace Theory (GWT) with Kuramoto
//! oscillator synchronization for conscious memory integration.
//!
//! # Constitution Compliance
//!
//! - Latency budget: <10ms
//! - Throughput: 100/s
//! - Components: Kuramoto sync, GW broadcast, workspace update
//! - UTL: R(t) measurement (resonance/order parameter)
//!
//! # Critical Rules
//!
//! - NO BACKWARDS COMPATIBILITY: System works or fails fast
//! - NO MOCK DATA: Returns real Kuramoto sync or proper errors
//! - NO FALLBACKS: If sync computation fails, ERROR OUT
//!
//! # GWT Consciousness Equation
//!
//! C(t) = I(t) × R(t) × D(t)
//!
//! Where:
//! - I(t) = Integration (information available for global broadcast)
//! - R(t) = Resonance (Kuramoto order parameter r)
//! - D(t) = Differentiation (normalized Shannon entropy of purpose vector)
//!
//! # Kuramoto Oscillator Model
//!
//! dθ_i/dt = ω_i + (K/N) Σ_j sin(θ_j - θ_i)
//!
//! Where:
//! - θ_i = phase of oscillator i ∈ [0, 2π]
//! - ω_i = natural frequency of oscillator i (from constitution gwt.kuramoto.frequencies)
//! - K = global coupling strength (2.0 from constitution)
//! - N = number of oscillators (13 for embedder model, one per E1-E13)

mod constants;
mod layer;
mod network;
mod oscillator;
mod thresholds;
mod workspace;

#[cfg(test)]
mod tests;

// Re-export new thresholds module
pub use thresholds::GwtThresholds;

// Re-export constants (deprecated re-exports with warnings)
#[allow(deprecated)]
pub use constants::{
    FRAGMENTATION_THRESHOLD, GW_THRESHOLD, HYPERSYNC_THRESHOLD, INTEGRATION_STEPS, KURAMOTO_DT,
    KURAMOTO_K, KURAMOTO_N,
};

// Re-export new Kuramoto constants (constitution-aligned)
pub use constants::{KURAMOTO_BASE_FREQUENCIES, KURAMOTO_DEFAULT_COUPLING, KURAMOTO_STEP_INTERVAL_MS};

// Re-export layer components
pub use layer::CoherenceLayer;
pub use network::KuramotoNetwork;
pub use oscillator::KuramotoOscillator;
pub use workspace::{ConsciousnessState, GlobalWorkspace};
