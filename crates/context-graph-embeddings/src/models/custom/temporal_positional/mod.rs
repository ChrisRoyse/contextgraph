//! Temporal-Positional embedding model (E4) using transformer-style sinusoidal encoding.
//!
//! This custom model produces 512-dimensional vectors encoding absolute time positions
//! using the standard transformer positional encoding formula.
//!
//! # Design
//!
//! The 512D vector is structured as 256 sin/cos pairs:
//! - PE(pos, 2i) = sin(pos / 10000^(2i/d_model))
//! - PE(pos, 2i+1) = cos(pos / 10000^(2i/d_model))
//!
//! Unlike E3 (Fourier/Periodic) which captures cyclic patterns, E4 provides unique
//! positional encodings for absolute timestamps that:
//! - Are deterministic for the same timestamp
//! - Can represent relative positions through attention
//! - Scale gracefully for far-future timestamps
//!
//! # Key Differences from Other Temporal Models
//!
//! | Model | Purpose | Math |
//! |-------|---------|------|
//! | E2 TemporalRecent | Recency (how recently?) | Exponential decay: e^(-t/tau) |
//! | E3 TemporalPeriodic | Periodicity (what cycle?) | Fourier: sin(2π * t/P) |
//! | E4 TemporalPositional | Position (absolute when?) | Transformer PE: sin(pos/10000^(2i/d)) |
//!
//! # Thread Safety
//! - `AtomicBool` for initialized state (always true for custom models)
//! - Pure computation with no shared mutable state
//!
//! # Performance
//! - Latency budget: <2ms per constitution.yaml
//! - No I/O or pretrained weights - pure CPU computation
//!
//! # Module Structure
//!
//! This module is split into submodules for maintainability:
//! - `constants`: Dimension and configuration constants
//! - `encoding`: Positional encoding computation
//! - `timestamp`: Timestamp parsing utilities
//! - `model`: Core model implementation and trait implementations

mod constants;
mod encoding;
mod model;
mod session_signature;
mod timestamp;

#[cfg(test)]
mod tests;

// Re-export public API for backwards compatibility
pub use constants::{
    DEFAULT_BASE, HYBRID_MODE_DEFAULT, POSITION_ENCODING_DIMENSION, SESSION_SIGNATURE_DIMENSION,
    TEMPORAL_POSITIONAL_DIMENSION,
};
pub use model::TemporalPositionalModel;
pub use session_signature::{compute_session_signature, compute_session_signature_or_default};
pub use timestamp::{HybridPositionInfo, PositionInfo};
