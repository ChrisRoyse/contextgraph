//! Hyperbolic geometry module using Poincare ball model.
//!
//! This module implements hyperbolic geometry operations for representing
//! hierarchical relationships in the knowledge graph. Points closer to the
//! boundary represent more specific concepts; points near origin are general.
//!
//! # Mathematics
//!
//! The Poincare ball model uses the unit ball B^n = {x in R^n : ||x|| < 1}
//! with the metric:
//!
//! ```text
//! d(x,y) = arcosh(1 + 2||x-y||² / ((1-||x||²)(1-||y||²)))
//! ```
//!
//! # Components
//!
//! - [`PoincarePoint`]: 64D point in hyperbolic space
//! - [`PoincareBall`]: Mobius operations (add, distance, exp/log maps)
//!
//! # Constitution Reference
//!
//! - perf.latency.entailment_check: <1ms
//! - hyperbolic.curvature: -1.0 (default)
//!
//! # GPU Acceleration
//!
//! CUDA kernels for batch operations: TODO: M04-T23

pub mod mobius;
pub mod poincare;

pub use mobius::PoincareBall;
pub use poincare::PoincarePoint;
