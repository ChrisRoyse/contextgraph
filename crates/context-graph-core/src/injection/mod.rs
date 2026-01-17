//! Injection pipeline types for context injection.
//!
//! This module provides types for the injection pipeline:
//! - [`InjectionCandidate`] - Memory candidate for injection with scores
//! - [`InjectionCategory`] - Priority category determining budget
//! - [`TokenBudget`] - Token allocation limits for context injection
//! - [`InjectionResult`] - Output from context injection pipeline
//!
//! # Constitution Compliance
//! - ARCH-09: Topic threshold = weighted_agreement >= 2.5
//! - AP-60: Temporal embedders NEVER count toward topics

pub mod budget;
pub mod candidate;
pub mod result;
pub mod temporal_enrichment;

pub use budget::{TokenBudget, DEFAULT_TOKEN_BUDGET, BRIEF_BUDGET};
pub use candidate::{
    InjectionCandidate, InjectionCategory, MAX_DIVERSITY_BONUS, MAX_RECENCY_FACTOR,
    MAX_WEIGHTED_AGREEMENT, MIN_DIVERSITY_BONUS, MIN_RECENCY_FACTOR, TOKEN_MULTIPLIER,
};
pub use result::InjectionResult;
pub use temporal_enrichment::{
    TemporalBadge, TemporalBadgeType, TemporalEnrichmentProvider,
    DEFAULT_SAME_SESSION_THRESHOLD, DEFAULT_SAME_DAY_THRESHOLD,
    DEFAULT_SAME_PERIOD_THRESHOLD, DEFAULT_SAME_SEQUENCE_THRESHOLD,
};
