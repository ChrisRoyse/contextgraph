//! TeleologicalFingerprint: Complete node representation with purpose-aware metadata.
//!
//! This is the top-level fingerprint type that wraps SemanticFingerprint with:
//! - Purpose Vector (13D alignment to North Star goal)
//! - Johari Fingerprint (per-embedder awareness classification)
//! - Purpose Evolution (time-series of alignment changes)
//!
//! Enables goal-aligned retrieval: "find memories similar to X that serve the same purpose"

mod alignment;
mod core;
mod types;

#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod tests;

// Re-export the main type for backwards compatibility
pub use types::TeleologicalFingerprint;
