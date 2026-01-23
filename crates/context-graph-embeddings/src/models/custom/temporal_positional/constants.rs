//! Constants and configuration for the Temporal-Positional embedding model (E4).
//!
//! # Hybrid Mode Architecture
//!
//! E4 supports a hybrid encoding mode that combines:
//! - **First 256D**: Session signature (deterministic hash-based clustering)
//! - **Last 256D**: Position encoding (sinusoidal, original approach)
//!
//! This enables same-session memories to cluster in E4 space while preserving
//! fine-grained position ordering within sessions.

/// Native dimension for TemporalPositional model (E4).
pub const TEMPORAL_POSITIONAL_DIMENSION: usize = 512;

/// Default base frequency for sinusoidal encoding (transformer standard).
/// This value is from the original "Attention Is All You Need" paper.
pub const DEFAULT_BASE: f32 = 10000.0;

/// Minimum valid base frequency (must be > 1.0 for proper frequency scaling).
pub(crate) const MIN_BASE: f32 = 1.0;

/// Maximum valid base frequency (prevent numerical issues).
pub(crate) const MAX_BASE: f32 = 1e10;

// =============================================================================
// HYBRID MODE CONSTANTS
// =============================================================================

/// Dimension for session signature in hybrid mode.
/// The first 256 dimensions encode the session identity.
pub const SESSION_SIGNATURE_DIMENSION: usize = 256;

/// Dimension for position encoding in hybrid mode.
/// The last 256 dimensions encode the sequence position.
pub const POSITION_ENCODING_DIMENSION: usize = 256;

/// Default hybrid mode setting.
/// When true, E4 uses session_signature || position_encoding.
/// When false, E4 uses pure positional encoding (legacy mode).
pub const HYBRID_MODE_DEFAULT: bool = true;

// Compile-time validation: session + position dimensions must equal total
const _: () = {
    assert!(
        SESSION_SIGNATURE_DIMENSION + POSITION_ENCODING_DIMENSION == TEMPORAL_POSITIONAL_DIMENSION,
        "Session and position dimensions must sum to total E4 dimension"
    );
};
