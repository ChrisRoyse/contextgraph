//! Injection pipeline types for context injection.
//!
//! This module provides types for the injection pipeline:
//! - [`InjectionCandidate`] - Memory candidate for injection with scores
//! - [`InjectionCategory`] - Priority category determining budget
//! - [`TokenBudget`] - Token allocation limits for context injection
//! - [`InjectionResult`] - Output from context injection pipeline
//! - [`InjectionPipeline`] - Full context injection orchestrator
//! - [`InjectionError`] - Error types for pipeline failures
//!
//! # Constitution Compliance
//! - ARCH-09: Topic threshold = weighted_agreement >= 2.5
//! - ARCH-10: Divergence detection uses SEMANTIC embedders only
//! - AP-60: Temporal embedders NEVER count toward topics
//! - AP-62: Divergence alerts MUST only use SEMANTIC embedders

pub mod budget;
pub mod candidate;
pub mod formatter;
pub mod pipeline;
pub mod priority;
pub mod result;
pub mod temporal_enrichment;

pub use budget::{
    TokenBudget, TokenBudgetManager, SelectionStats,
    DEFAULT_TOKEN_BUDGET, BRIEF_BUDGET, estimate_tokens,
};
pub use candidate::{
    InjectionCandidate, InjectionCategory, MAX_DIVERSITY_BONUS, MAX_RECENCY_FACTOR,
    MAX_WEIGHTED_AGREEMENT, MIN_DIVERSITY_BONUS, MIN_RECENCY_FACTOR, TOKEN_MULTIPLIER,
};
pub use pipeline::{InjectionError, InjectionPipeline};
pub use priority::{DiversityBonus, PriorityRanker, RecencyFactor};
pub use result::InjectionResult;
pub use temporal_enrichment::{
    TemporalBadge, TemporalBadgeType, TemporalEnrichmentProvider,
    DEFAULT_SAME_SESSION_THRESHOLD, DEFAULT_SAME_DAY_THRESHOLD,
    DEFAULT_SAME_PERIOD_THRESHOLD, DEFAULT_SAME_SEQUENCE_THRESHOLD,
};
pub use formatter::{ContextFormatter, SUMMARY_MAX_WORDS, BRIEF_MAX_TOKENS};
