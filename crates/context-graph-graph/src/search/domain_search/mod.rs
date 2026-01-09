//! Domain-aware search with Marblestone neurotransmitter modulation.
//!
//! # CANONICAL FORMULA
//!
//! ```text
//! net_activation = excitatory - inhibitory + (modulatory * 0.5)
//! domain_bonus = 0.1 if node_domain == query_domain else 0.0
//! modulated_score = base_similarity * (1.0 + net_activation + domain_bonus)
//! ```
//!
//! # Constitution Reference
//!
//! - edge_model.nt_weights: Definition and formula
//! - edge_model.nt_weights.domain: Code|Legal|Medical|Creative|Research|General
//! - AP-001: Never unwrap() in prod - all errors properly typed
//! - AP-009: NaN/Infinity clamped to valid range
//!
//! # Module Structure
//!
//! - `types` - Result types and constants
//! - `search` - Core search implementation
//! - `tests` - Unit and integration tests

mod search;
mod types;

#[cfg(test)]
mod tests;

// Re-exports for backwards compatibility
pub use search::{
    compute_net_activation, domain_aware_search, domain_nt_summary, expected_domain_boost,
};
pub use types::{
    DomainSearchResult, DomainSearchResults, DOMAIN_MATCH_BONUS, OVERFETCH_MULTIPLIER,
};
