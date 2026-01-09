//! Core domain-aware search implementation.
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

use crate::error::GraphResult;
use crate::index::FaissGpuIndex;
use crate::search::{semantic_search, Domain, NodeMetadataProvider, SearchFilters};

use super::types::{
    DomainSearchResult, DomainSearchResults, DOMAIN_MATCH_BONUS, OVERFETCH_MULTIPLIER,
};

// Re-export from core - DO NOT REDEFINE
use context_graph_core::marblestone::NeurotransmitterWeights;

use tracing::{debug, warn};

/// Compute net activation from NT weights using CANONICAL formula.
///
/// # CANONICAL FORMULA
///
/// ```text
/// net_activation = excitatory - inhibitory + (modulatory * 0.5)
/// ```
///
/// IMPORTANT: This is inline computation because NeurotransmitterWeights
/// does NOT have a net_activation() method.
#[inline]
pub fn compute_net_activation(nt: &NeurotransmitterWeights) -> f32 {
    nt.excitatory - nt.inhibitory + (nt.modulatory * 0.5)
}

/// Perform domain-aware search with neurotransmitter modulation.
///
/// Uses Marblestone-inspired NT modulation to adjust search relevance
/// based on the query domain. Over-fetches 3x candidates, applies
/// NT modulation, then re-ranks by modulated score.
///
/// # CANONICAL FORMULA
///
/// ```text
/// net_activation = excitatory - inhibitory + (modulatory * 0.5)
/// domain_bonus = 0.1 if node_domain == query_domain else 0.0
/// modulated_score = base_similarity * (1.0 + net_activation + domain_bonus)
/// ```
///
/// # Arguments
///
/// * `index` - FAISS GPU index (must be trained)
/// * `query` - Query embedding as f32 slice (1536 dimensions)
/// * `query_domain` - Domain for NT profile selection
/// * `k` - Number of results to return
/// * `filters` - Optional additional filters
/// * `metadata` - Metadata provider for node UUID/domain resolution
///
/// # Returns
///
/// * Top-k results ranked by modulated score
///
/// # Errors
///
/// * `GraphError::IndexNotTrained` - If FAISS index not trained
/// * `GraphError::DimensionMismatch` - If query dimension wrong
/// * `GraphError::FaissSearchFailed` - If FAISS search fails
/// * `GraphError::InvalidConfig` - If filters invalid
///
/// # Performance
///
/// Target: <10ms for k=10 on 10M vectors
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_graph::search::domain_search::{domain_aware_search, DomainSearchResults};
/// use context_graph_graph::search::Domain;
///
/// let results = domain_aware_search(
///     &index,
///     &query_embedding,
///     Domain::Code,
///     10,
///     None,
///     Some(&storage),
/// )?;
///
/// for result in results.iter() {
///     println!("Node {:?} base: {:.3} modulated: {:.3} (delta: {:+.3})",
///         result.node_id,
///         result.base_similarity,
///         result.modulated_score,
///         result.modulation_delta()
///     );
/// }
/// ```
#[tracing::instrument(skip(index, query, metadata), fields(domain = ?query_domain, k = k))]
pub fn domain_aware_search<M: NodeMetadataProvider>(
    index: &FaissGpuIndex,
    query: &[f32],
    query_domain: Domain,
    k: usize,
    filters: Option<SearchFilters>,
    metadata: Option<&M>,
) -> GraphResult<DomainSearchResults> {
    let start = std::time::Instant::now();

    // Validate k
    if k == 0 {
        warn!("domain_aware_search called with k=0, returning empty results");
        return Ok(DomainSearchResults::empty(query_domain));
    }

    // Over-fetch 3x candidates for re-ranking
    let fetch_k = k.saturating_mul(OVERFETCH_MULTIPLIER);
    debug!(fetch_k, "Over-fetching candidates for re-ranking");

    // Get base semantic results
    let semantic_results = semantic_search(index, query, fetch_k, filters, metadata)?;

    if semantic_results.items.is_empty() {
        debug!("No semantic search results found");
        return Ok(DomainSearchResults::empty(query_domain));
    }

    // Get domain-specific NT profile
    let domain_nt = NeurotransmitterWeights::for_domain(query_domain);
    // CANONICAL FORMULA: net_activation computed inline (no method on NeurotransmitterWeights)
    let base_net_activation = compute_net_activation(&domain_nt);
    debug!(
        net_activation = base_net_activation,
        "Using NT profile for domain"
    );

    // Apply modulation to each result
    let mut modulated_results: Vec<DomainSearchResult> =
        Vec::with_capacity(semantic_results.items.len());

    for item in &semantic_results.items {
        // Calculate domain bonus
        let domain_bonus = match item.domain {
            Some(node_domain) if node_domain == query_domain => DOMAIN_MATCH_BONUS,
            _ => 0.0,
        };

        // CANONICAL FORMULA: modulated_score = base * (1.0 + net_activation + domain_bonus)
        let modulated_score = item.similarity * (1.0 + base_net_activation + domain_bonus);

        // Clamp to [0.0, 1.0] per AP-009
        let modulated_score = modulated_score.clamp(0.0, 1.0);

        modulated_results.push(DomainSearchResult::from_semantic_item(
            item,
            modulated_score,
            base_net_activation,
            query_domain,
        ));
    }

    // Re-rank by modulated score (descending)
    modulated_results.sort_by(|a, b| {
        b.modulated_score
            .partial_cmp(&a.modulated_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Update ranks and truncate to k
    for (i, result) in modulated_results.iter_mut().enumerate() {
        result.rank = i;
    }

    let candidates_fetched = modulated_results.len();
    modulated_results.truncate(k);
    let results_returned = modulated_results.len();

    let latency = start.elapsed();
    debug!(
        latency_us = latency.as_micros(),
        candidates = candidates_fetched,
        returned = results_returned,
        "Domain search complete"
    );

    Ok(DomainSearchResults {
        items: modulated_results,
        candidates_fetched,
        results_returned,
        query_domain,
        latency_us: latency.as_micros() as u64,
    })
}

/// Get expected boost ratio for a domain (matching nodes).
///
/// Returns the expected modulation multiplier for nodes matching the query domain.
/// Useful for testing and validation.
#[inline]
pub fn expected_domain_boost(domain: Domain) -> f32 {
    let nt = NeurotransmitterWeights::for_domain(domain);
    let net_activation = compute_net_activation(&nt);
    1.0 + net_activation + DOMAIN_MATCH_BONUS
}

/// Get NT profile summary for a domain.
///
/// Returns a human-readable summary of the NT profile for debugging.
pub fn domain_nt_summary(domain: Domain) -> String {
    let nt = NeurotransmitterWeights::for_domain(domain);
    let net_activation = compute_net_activation(&nt);
    format!(
        "{:?}: exc={:.2} inh={:.2} mod={:.2} net={:+.3} boost={:.2}x",
        domain,
        nt.excitatory,
        nt.inhibitory,
        nt.modulatory,
        net_activation,
        expected_domain_boost(domain)
    )
}
