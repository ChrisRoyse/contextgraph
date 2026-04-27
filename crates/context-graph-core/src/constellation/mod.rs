//! Phase 2: Constellation compiler.
//!
//! A **constellation** is a compiled summary of a set of memories selected by
//! topic, session, tag, time range, or explicit id list. It captures per-
//! embedder centroids, spread statistics, and aggregate topic / group /
//! cross-correlation centroids so downstream classifiers can answer "does
//! this new memory belong to the same cluster?" without re-running the full
//! retrieval pipeline.
//!
//! Records live in `CF_CONSTELLATIONS` (keyed by a fresh UUID) with a
//! secondary index `CF_CONSTELLATION_BY_SELECTOR` for O(1) selector lookup.
//!
//! ## Module structure
//!
//! - [`types`] — [`Constellation`], [`ConstellationSelector`],
//!   [`EmbedderStats`], [`VectorKind`], [`ConstellationScoringResult`].
//! - [`welford`] — [`welford::WelfordStats`] / [`welford::WelfordVector`]
//!   online mean + variance.
//! - [`reservoir`] — [`reservoir::ReservoirSample`] bounded-memory percentile
//!   estimator.
//! - [`compiler`] — [`ConstellationAccumulator`],
//!   [`compile_constellation`], [`score_memory_against_constellation`].
//!
//! ## Versioning
//!
//! [`CONSTELLATION_VERSION`] is the version byte prefix for
//! `CF_CONSTELLATIONS` payloads. Deserialization rejects mismatches with
//! `CoreError::SerializationError` — no automatic migration.

pub mod compiler;
pub mod reservoir;
pub mod types;
pub mod welford;

#[cfg(test)]
mod tests;

pub use compiler::{
    compile_constellation, score_memory_against_constellation, ConstellationAccumulator,
    ConstellationError, DEFAULT_MAX_MEMBERS,
};
pub use types::{
    Constellation, ConstellationScoringResult, ConstellationSelector, EmbedderStats, VectorKind,
    CROSS_CORRELATION_CENTROID_DIM, GROUP_ALIGNMENT_CENTROID_DIM, NUM_CONSTELLATION_EMBEDDERS,
    TOPIC_PROFILE_CENTROID_DIM,
};

/// Current on-disk version byte for [`Constellation`].
///
/// Bump on breaking layout changes; deserialization rejects mismatches with
/// `CoreError::SerializationError`. No automatic migration is supported.
///
/// v2: E14-aware shape (14 per-embedder stats, 14D topic centroid,
/// 91 cross-correlation centroid entries).
pub const CONSTELLATION_VERSION: u8 = 2;

/// Reservoir capacity for percentile estimation. Capped at 1024 samples per
/// reservoir to bound memory while giving tight p50/p95 convergence.
pub const RESERVOIR_SAMPLE_SIZE: usize = 1024;

/// Minimum number of members required to compile a constellation.
/// Below this threshold statistics are meaningless; `finalize` returns
/// `ConstellationError::TooFewMembers`.
pub const MIN_CONSTELLATION_MEMBERS: usize = 3;
