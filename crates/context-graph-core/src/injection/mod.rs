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

pub use crate::clustering::MAX_WEIGHTED_AGREEMENT;
pub use budget::{
    estimate_tokens, BudgetTooSmall, SelectionStats, TokenBudget, TokenBudgetManager, BRIEF_BUDGET,
    DEFAULT_TOKEN_BUDGET, MIN_BUDGET,
};
pub use candidate::{
    InjectionCandidate, InjectionCategory, MAX_DIVERSITY_BONUS, MAX_RECENCY_FACTOR,
    MIN_DIVERSITY_BONUS, MIN_RECENCY_FACTOR, TOKEN_MULTIPLIER,
};
pub use formatter::{ContextFormatter, BRIEF_MAX_TOKENS, SUMMARY_MAX_WORDS};
pub use pipeline::{InjectionError, InjectionPipeline};
pub use priority::{DiversityBonus, PriorityRanker, RecencyFactor};
pub use result::InjectionResult;
pub use temporal_enrichment::{
    TemporalBadge, TemporalBadgeType, TemporalEnrichmentProvider, DEFAULT_SAME_DAY_THRESHOLD,
    DEFAULT_SAME_PERIOD_THRESHOLD, DEFAULT_SAME_SEQUENCE_THRESHOLD, DEFAULT_SAME_SESSION_THRESHOLD,
};
