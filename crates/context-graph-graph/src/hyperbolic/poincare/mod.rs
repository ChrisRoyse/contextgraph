//! PoincarePoint implementation for 64D hyperbolic space.
//!
//! # Poincare Ball Model
//!
//! The Poincare ball model represents hyperbolic space as the interior of a
//! unit ball. Points must satisfy ||x|| < 1 (strictly inside). Points near
//! the boundary represent specific/leaf concepts; points near origin represent
//! general/root concepts.
//!
//! # Performance
//!
//! - Memory: 256 bytes per point (64 * 4 bytes, 64-byte aligned)
//! - norm_squared(): O(64) with SIMD optimization potential
//! - project(): O(64) when rescaling needed
//!
//! # Constitution Reference
//!
//! - hyperbolic.dim: 64
//! - hyperbolic.max_norm: 0.99999 (1.0 - 1e-5)
//! - perf.latency.entailment_check: <1ms
//!
//! # Module Structure
//!
//! - [`types`]: Core PoincarePoint struct definition
//! - [`ops`]: All operations (construction, norm, projection, validation)

mod ops;
mod types;

#[cfg(test)]
mod tests;

// Re-export PoincarePoint at module root for backwards compatibility
pub use types::PoincarePoint;
