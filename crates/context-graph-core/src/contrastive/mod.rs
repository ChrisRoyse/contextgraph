//! Phase 3: Contrastive pair miner.
//!
//! A **contrastive pair** is a `(anchor, negative, similarity_profile)` triple
//! mined from the existing memory corpus. The miner surfaces *structurally
//! interesting negatives* — pairs that agree strongly on one embedder axis
//! while disagreeing on another — and packages them for downstream
//! contrastive / metric-learning training.
//!
//! ## Module structure
//!
//! - [`types`] — [`ContrastivePair`], [`AnomalyKind`], [`MiningConfig`],
//!   [`MiningSummary`], [`ContrastiveError`].
//! - [`mining`] — pure functions [`mining::similarity_profile`],
//!   [`mining::classify_anomaly`], [`mining::mine_pair_from_candidate`].
//!
//! ## Versioning
//!
//! [`CONTRASTIVE_PAIR_VERSION`] is the version byte prefix for
//! `CF_CONTRASTIVE_PAIRS` payloads. Deserialization rejects mismatches with
//! `CoreError::SerializationError` — no automatic migration.
//!
//! ## Per-embedder similarity contract
//!
//! All entries of [`ContrastivePair::similarity_profile`] are in the `[0, 1]`
//! SRC-3 convention used throughout the codebase: raw cosine `c ∈ [-1, 1]` is
//! mapped via `(c + 1) / 2`. For sparse embedders (E6, E13) we use sparse
//! Jaccard, which is already in `[0, 1]`. For the token-level embedder (E12),
//! we mean-pool the token vectors per side and run SRC-3-normalized cosine;
//! when either side has no tokens the entry is `0.0`.

pub mod mining;
pub mod types;

#[cfg(test)]
mod tests;

pub use mining::{classify_anomaly, mine_pair_from_candidate, similarity_profile};
pub use types::{
    AnomalyKind, ContrastiveError, ContrastivePair, MiningConfig, MiningSummary,
    DEFAULT_HIGH_THRESHOLD, DEFAULT_LOW_THRESHOLD, DEFAULT_MAX_PAIRS, DEFAULT_MIN_DISAGREEMENT,
    DEFAULT_TOP_K_CANDIDATES_PER_ANCHOR, NUM_ANOMALY_KINDS,
};

/// Current on-disk version byte for [`ContrastivePair`] payloads in
/// `CF_CONTRASTIVE_PAIRS`.
///
/// Bump on breaking layout changes; deserialization rejects mismatches with
/// `CoreError::SerializationError`. No automatic migration is supported.
pub const CONTRASTIVE_PAIR_VERSION: u8 = 1;
