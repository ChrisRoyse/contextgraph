//! E8 Graph Asymmetric Similarity
//!
//! Implements Constitution-specified asymmetric similarity for E8 Graph embeddings:
//!
//! ```text
//! sim = base_cos × direction_mod × (0.7 + 0.3 × connectivity_overlap)
//! ```
//!
//! # Direction Modifiers (Per Constitution, following E5 Causal pattern)
//!
//! - source→target: 1.2 (forward relationship amplified)
//! - target→source: 0.8 (backward relationship dampened)
//! - same_direction: 1.0 (no modification)
//!
//! # Architecture
//!
//! This module parallels E5 Causal asymmetric similarity but for structural
//! relationships rather than causal relationships:
//!
//! - E5 (Causal): cause→effect, effect→cause
//! - E8 (Graph): source→target, target→source
//!
//! # References
//!
//! - E5 Causal asymmetric: `causal/asymmetric.rs`
//! - E8 upgrade specification: `docs/e8upgrade.md`
//! - Constitution `graph_asymmetric_sim` section

use serde::{Deserialize, Serialize};

/// Direction modifiers per Constitution specification.
///
/// Following the E5 Causal pattern (ARCH-15), E8 Graph uses the same
/// direction modifier values for consistency.
///
/// # Constitution Reference
/// ```yaml
/// graph_asymmetric_sim:
///   direction_modifiers:
///     source_to_target: 1.2
///     target_to_source: 0.8
///     same_direction: 1.0
/// ```
pub mod direction_mod {
    /// source→target amplification factor
    pub const SOURCE_TO_TARGET: f32 = 1.2;
    /// target→source dampening factor
    pub const TARGET_TO_SOURCE: f32 = 0.8;
    /// No modification for same-direction comparisons
    pub const SAME_DIRECTION: f32 = 1.0;
    /// Default for unknown direction (no modification)
    pub const UNKNOWN: f32 = 1.0;
}

/// Graph direction for asymmetric similarity computation.
///
/// Represents the structural role of an entity in graph relationships.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GraphDirection {
    /// Entity is a source (has outgoing edges)
    /// Example: "Module A imports B" - A is source
    Source,
    /// Entity is a target (has incoming edges)
    /// Example: "B is imported by A" - B is target
    Target,
    /// Direction unknown or bidirectional
    #[default]
    Unknown,
}

impl GraphDirection {
    /// Get direction modifier when comparing query_direction to result_direction.
    ///
    /// # Returns
    ///
    /// Direction modifier per Constitution:
    /// - 1.2 if query=Source and result=Target (source→target)
    /// - 0.8 if query=Target and result=Source (target→source)
    /// - 1.0 otherwise (same direction or unknown)
    pub fn direction_modifier(query_direction: Self, result_direction: Self) -> f32 {
        match (query_direction, result_direction) {
            // Query is source looking for target: AMPLIFY
            (Self::Source, Self::Target) => direction_mod::SOURCE_TO_TARGET,
            // Query is target looking for source: DAMPEN
            (Self::Target, Self::Source) => direction_mod::TARGET_TO_SOURCE,
            // Same direction or unknown: NO CHANGE
            (Self::Source, Self::Source) => direction_mod::SAME_DIRECTION,
            (Self::Target, Self::Target) => direction_mod::SAME_DIRECTION,
            (Self::Unknown, _) => direction_mod::UNKNOWN,
            (_, Self::Unknown) => direction_mod::UNKNOWN,
        }
    }

    /// Convert from string representation.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "source" => Self::Source,
            "target" => Self::Target,
            _ => Self::Unknown,
        }
    }
}

impl std::fmt::Display for GraphDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source => write!(f, "source"),
            Self::Target => write!(f, "target"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Connectivity context for computing structural overlap.
///
/// Represents the structural relationships involved in graph analysis.
/// Used to compute the connectivity_overlap term in the asymmetric similarity formula.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectivityContext {
    /// Names/IDs of connected entities (neighbors)
    pub connected_entities: Vec<String>,
    /// Type of relationships (e.g., "import", "call", "extend")
    pub relationship_types: Vec<String>,
    /// Module/namespace context
    pub module_context: Option<String>,
    /// Depth in dependency graph (if known)
    pub depth: Option<usize>,
}

impl ConnectivityContext {
    /// Create a new empty connectivity context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a connected entity.
    pub fn with_entity(mut self, entity: impl Into<String>) -> Self {
        self.connected_entities.push(entity.into());
        self
    }

    /// Add a relationship type.
    pub fn with_relationship(mut self, rel_type: impl Into<String>) -> Self {
        self.relationship_types.push(rel_type.into());
        self
    }

    /// Set the module context.
    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module_context = Some(module.into());
        self
    }

    /// Set the depth in dependency graph.
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = Some(depth);
        self
    }

    /// Compute connectivity overlap with another context.
    ///
    /// Uses a size-normalized approach that blends Jaccard similarity (specificity)
    /// with containment metric (flexibility for asymmetric set sizes).
    ///
    /// # Formula
    ///
    /// ```text
    /// entity_containment = entity_intersection / min(|A|, |B|)
    /// entity_jaccard = entity_intersection / entity_union
    /// entity_blended = 0.5 * jaccard + 0.5 * containment
    ///
    /// rel_jaccard = rel_intersection / rel_union
    ///
    /// overlap = 0.5 * entity_blended + 0.3 * rel_jaccard + module_bonus + depth_bonus
    /// ```
    ///
    /// # Returns
    ///
    /// Value in [0, 1] where:
    /// - 0 = no shared connectivity
    /// - 1 = perfect overlap in entities, relationships, module, and depth
    pub fn overlap_with(&self, other: &Self) -> f32 {
        if self.connected_entities.is_empty() && other.connected_entities.is_empty() {
            // Both empty contexts: treat as neutral (0.5)
            return 0.5;
        }

        if self.connected_entities.is_empty() || other.connected_entities.is_empty() {
            // One empty, one not: minimal overlap
            return 0.1;
        }

        // Entity overlap (primary factor)
        let self_entities: std::collections::HashSet<_> = self.connected_entities.iter().collect();
        let other_entities: std::collections::HashSet<_> = other.connected_entities.iter().collect();

        let entity_intersection = self_entities.intersection(&other_entities).count();
        let entity_union = self_entities.union(&other_entities).count();
        let entity_min_size = self_entities.len().min(other_entities.len());

        let entity_containment = if entity_min_size > 0 {
            entity_intersection as f32 / entity_min_size as f32
        } else {
            0.0
        };

        let entity_jaccard = if entity_union > 0 {
            entity_intersection as f32 / entity_union as f32
        } else {
            0.0
        };

        let entity_blended = entity_jaccard * 0.5 + entity_containment * 0.5;

        // Relationship type overlap (secondary factor)
        let self_rels: std::collections::HashSet<_> = self.relationship_types.iter().collect();
        let other_rels: std::collections::HashSet<_> = other.relationship_types.iter().collect();

        let rel_intersection = self_rels.intersection(&other_rels).count();
        let rel_union = self_rels.union(&other_rels).count();

        let rel_jaccard = if rel_union > 0 {
            rel_intersection as f32 / rel_union as f32
        } else {
            0.5 // Neutral if no relationship types specified
        };

        // Compute base overlap from entities and relationships
        let base_overlap = entity_blended * 0.5 + rel_jaccard * 0.3;

        // Only apply bonuses if there's meaningful base overlap
        let module_bonus = if base_overlap > 0.1 {
            match (&self.module_context, &other.module_context) {
                (Some(m1), Some(m2)) if m1 == m2 => 0.1,
                (Some(m1), Some(m2)) if m1.starts_with(m2) || m2.starts_with(m1) => 0.05,
                _ => 0.0,
            }
        } else {
            0.0
        };

        let depth_bonus = if base_overlap > 0.1 {
            match (self.depth, other.depth) {
                (Some(d1), Some(d2)) if d1 == d2 => 0.1,
                (Some(d1), Some(d2)) if (d1 as i32 - d2 as i32).abs() <= 1 => 0.05,
                _ => 0.0,
            }
        } else {
            0.0
        };

        // Final overlap capped at 1.0
        (base_overlap + module_bonus + depth_bonus).clamp(0.0, 1.0)
    }

    /// Check if this context is empty.
    pub fn is_empty(&self) -> bool {
        self.connected_entities.is_empty()
            && self.relationship_types.is_empty()
            && self.module_context.is_none()
            && self.depth.is_none()
    }
}

/// Compute E8 asymmetric graph similarity.
///
/// # Formula (Constitution)
///
/// ```text
/// sim = base_cos × direction_mod × (0.7 + 0.3 × connectivity_overlap)
/// ```
///
/// # Arguments
///
/// * `base_cosine` - Base cosine similarity between embeddings [0, 1]
/// * `query_direction` - Graph direction of the query
/// * `result_direction` - Graph direction of the result
/// * `query_context` - Connectivity context of the query (optional)
/// * `result_context` - Connectivity context of the result (optional)
///
/// # Returns
///
/// Adjusted similarity value. Note: Can exceed 1.0 due to direction_mod=1.2.
///
/// # Example
///
/// ```
/// use context_graph_core::graph::asymmetric::{
///     compute_graph_asymmetric_similarity, GraphDirection, ConnectivityContext
/// };
///
/// let base_sim = 0.8;
/// let query_dir = GraphDirection::Source;
/// let result_dir = GraphDirection::Target;
/// let query_ctx = ConnectivityContext::new().with_entity("utils");
/// let result_ctx = ConnectivityContext::new().with_entity("utils");
///
/// let adjusted = compute_graph_asymmetric_similarity(
///     base_sim,
///     query_dir,
///     result_dir,
///     Some(&query_ctx),
///     Some(&result_ctx),
/// );
///
/// // source→target with high overlap = amplified similarity
/// assert!(adjusted > base_sim);
/// ```
pub fn compute_graph_asymmetric_similarity(
    base_cosine: f32,
    query_direction: GraphDirection,
    result_direction: GraphDirection,
    query_context: Option<&ConnectivityContext>,
    result_context: Option<&ConnectivityContext>,
) -> f32 {
    // Get direction modifier
    let direction_mod = GraphDirection::direction_modifier(query_direction, result_direction);

    // Compute connectivity overlap
    let connectivity_overlap = match (query_context, result_context) {
        (Some(q), Some(r)) => q.overlap_with(r),
        _ => 0.5, // Default to neutral if no context provided
    };

    // Apply Constitution formula:
    // sim = base_cos × direction_mod × (0.7 + 0.3 × connectivity_overlap)
    let overlap_factor = 0.7 + 0.3 * connectivity_overlap;

    base_cosine * direction_mod * overlap_factor
}

/// Compute graph asymmetric similarity with default (neutral) contexts.
///
/// Convenience function when connectivity contexts are not available.
///
/// # Formula (Simplified)
///
/// ```text
/// sim = base_cos × direction_mod × 0.85
/// ```
///
/// (0.85 = 0.7 + 0.3 × 0.5 for neutral overlap)
pub fn compute_graph_asymmetric_similarity_simple(
    base_cosine: f32,
    query_direction: GraphDirection,
    result_direction: GraphDirection,
) -> f32 {
    compute_graph_asymmetric_similarity(base_cosine, query_direction, result_direction, None, None)
}

/// Adjust a batch of similarity scores with the same query context.
///
/// Optimized for multi-result scenarios where the query is constant.
///
/// # Arguments
///
/// * `base_similarities` - Slice of (base_cosine, result_direction, result_context) tuples
/// * `query_direction` - Graph direction of the query
/// * `query_context` - Connectivity context of the query (optional)
///
/// # Returns
///
/// Vector of adjusted similarities in the same order as input.
pub fn adjust_batch_graph_similarities(
    base_similarities: &[(f32, GraphDirection, Option<&ConnectivityContext>)],
    query_direction: GraphDirection,
    query_context: Option<&ConnectivityContext>,
) -> Vec<f32> {
    base_similarities
        .iter()
        .map(|(base, result_dir, result_ctx)| {
            compute_graph_asymmetric_similarity(
                *base,
                query_direction,
                *result_dir,
                query_context,
                *result_ctx,
            )
        })
        .collect()
}

// =============================================================================
// E8 Asymmetric Fingerprint-Based Similarity
// =============================================================================

use crate::types::fingerprint::SemanticFingerprint;

/// Compute asymmetric E8 graph similarity between query and document fingerprints.
///
/// This function implements the asymmetric similarity computation for graph retrieval:
///
/// - For "what imports X" queries (query is searching for sources):
///   query_as_source is compared against doc_as_target
///
/// - For "what does X import" queries (query is searching for targets):
///   query_as_target is compared against doc_as_source
///
/// # Arguments
///
/// * `query` - Query fingerprint
/// * `doc` - Document fingerprint to compare against
/// * `query_is_source` - If true, treat query as potential source (for "what imports X" queries);
///                       if false, treat query as potential target (for "what does X import" queries)
///
/// # Returns
///
/// Cosine similarity between the appropriate E8 vectors, clamped to [0, 1].
/// Uses the asymmetric pairing:
/// - query_is_source=true:  cosine(query.e8_graph_as_source, doc.e8_graph_as_target)
/// - query_is_source=false: cosine(query.e8_graph_as_target, doc.e8_graph_as_source)
///
/// # Example
///
/// ```ignore
/// use context_graph_core::graph::asymmetric::compute_e8_asymmetric_fingerprint_similarity;
/// use context_graph_core::types::fingerprint::SemanticFingerprint;
///
/// let query = SemanticFingerprint::zeroed();
/// let doc = SemanticFingerprint::zeroed();
///
/// // For "what imports utils?" queries, query is looking for sources (query_is_source=true)
/// let sim = compute_e8_asymmetric_fingerprint_similarity(&query, &doc, true);
/// assert!(sim >= 0.0 && sim <= 1.0);
/// ```
pub fn compute_e8_asymmetric_fingerprint_similarity(
    query: &SemanticFingerprint,
    doc: &SemanticFingerprint,
    query_is_source: bool,
) -> f32 {
    let (query_vec, doc_vec) = if query_is_source {
        // Query represents a potential source, looking for targets
        // Compare query's source encoding against doc's target encoding
        (&query.e8_graph_as_source, &doc.e8_graph_as_target)
    } else {
        // Query represents a potential target, looking for sources
        // Compare query's target encoding against doc's source encoding
        (&query.e8_graph_as_target, &doc.e8_graph_as_source)
    };

    cosine_similarity_f32(query_vec, doc_vec).max(0.0)
}

/// Compute E8 asymmetric similarity with direction modifier applied.
///
/// Combines the raw asymmetric similarity with the Constitution-specified
/// direction modifiers (source→target=1.2, target→source=0.8).
///
/// # Formula
///
/// ```text
/// sim = asymmetric_cosine × direction_mod × (0.7 + 0.3 × connectivity_overlap)
/// ```
///
/// # Arguments
///
/// * `query` - Query fingerprint
/// * `doc` - Document fingerprint
/// * `query_direction` - Graph direction of the query
/// * `result_direction` - Graph direction of the document
/// * `query_context` - Optional connectivity context for query
/// * `result_context` - Optional connectivity context for document
///
/// # Returns
///
/// Adjusted similarity score (may exceed 1.0 due to amplification).
pub fn compute_e8_asymmetric_full(
    query: &SemanticFingerprint,
    doc: &SemanticFingerprint,
    query_direction: GraphDirection,
    result_direction: GraphDirection,
    query_context: Option<&ConnectivityContext>,
    result_context: Option<&ConnectivityContext>,
) -> f32 {
    // Determine asymmetric pairing based on query direction
    let query_is_source = matches!(query_direction, GraphDirection::Source);

    // Get base asymmetric similarity
    let base_sim = compute_e8_asymmetric_fingerprint_similarity(query, doc, query_is_source);

    // Apply Constitution formula with direction modifier
    compute_graph_asymmetric_similarity(
        base_sim,
        query_direction,
        result_direction,
        query_context,
        result_context,
    )
}

/// Detect graph query intent from query text.
///
/// Analyzes the query text to determine if the user is asking for:
/// - Sources ("what imports X", "what uses X", "what calls X") → GraphDirection::Source
/// - Targets ("what does X import", "dependencies of X") → GraphDirection::Target
/// - Unknown direction → GraphDirection::Unknown
///
/// Uses score-based detection with disambiguation for queries that match
/// both source and target indicators.
///
/// # Arguments
///
/// * `query` - The query text to analyze
///
/// # Returns
///
/// The detected graph direction of the query.
///
/// # Example
///
/// ```
/// use context_graph_core::graph::asymmetric::{detect_graph_query_intent, GraphDirection};
///
/// assert_eq!(detect_graph_query_intent("what imports utils?"), GraphDirection::Source);
/// assert_eq!(detect_graph_query_intent("what does auth import?"), GraphDirection::Target);
/// assert_eq!(detect_graph_query_intent("show me the code"), GraphDirection::Unknown);
/// ```
pub fn detect_graph_query_intent(query: &str) -> GraphDirection {
    let query_lower = query.to_lowercase();

    // Source-seeking indicators: user wants to find things that POINT TO X
    // "What imports X?" → looking for sources of X
    let source_seeking_indicators = [
        "what imports",
        "what uses",
        "what requires",
        "what needs",
        "what depends on",
        "what calls",
        "what invokes",
        "what extends",
        "what implements",
        "what inherits",
        "what contains",
        "what includes",
        "what references",
        "what accesses",
        "who uses",
        "who imports",
        "who calls",
        "which module imports",
        "which modules import",
        "which module uses",
        "which modules use",
        "which file imports",
        "which files import",
        "dependents of",
        "consumers of",
        "users of",
        "callers of",
        "what relies on",
        "what depends upon",
        "imported by",
        "used by",
        "called by",
        "extended by",
        "implemented by",
    ];

    // Target-seeking indicators: user wants to find things that X POINTS TO
    // "What does X import?" → looking for targets of X
    let target_seeking_indicators = [
        "what does",  // "what does X import/use/call"
        "dependencies of",
        "imports of",
        "what are the imports",
        "what are the dependencies",
        "what are the requirements",
        "show imports",
        "show dependencies",
        "list imports",
        "list dependencies",
        "find dependencies",
        "find imports",
        "get dependencies",
        "get imports",
        "imports",  // standalone "imports" often means "show imports"
        "depends",
        "requires",
    ];

    // Score-based detection
    let source_score: usize = source_seeking_indicators
        .iter()
        .filter(|p| query_lower.contains(*p))
        .count();
    let target_score: usize = target_seeking_indicators
        .iter()
        .filter(|p| query_lower.contains(*p))
        .count();

    // Disambiguation
    match source_score.cmp(&target_score) {
        std::cmp::Ordering::Greater => GraphDirection::Source,
        std::cmp::Ordering::Less => GraphDirection::Target,
        std::cmp::Ordering::Equal if source_score > 0 => {
            // Tie-breaker: prefer source (more common query pattern)
            GraphDirection::Source
        }
        _ => GraphDirection::Unknown,
    }
}

/// Rank results by connectivity strength.
///
/// Re-ranks search results based on their structural connectivity
/// in addition to semantic similarity.
///
/// # Arguments
///
/// * `results` - Slice of (fingerprint_id, base_similarity, connectivity_context) tuples
/// * `query_direction` - Graph direction of the query
/// * `query_context` - Connectivity context of the query
/// * `connectivity_weight` - Weight for connectivity score (0.0-1.0)
///
/// # Returns
///
/// Vector of (fingerprint_id, adjusted_score) sorted by descending score.
pub fn rank_by_connectivity(
    results: &[(uuid::Uuid, f32, Option<ConnectivityContext>)],
    query_direction: GraphDirection,
    query_context: Option<&ConnectivityContext>,
    connectivity_weight: f32,
) -> Vec<(uuid::Uuid, f32)> {
    let mut scored: Vec<(uuid::Uuid, f32)> = results
        .iter()
        .map(|(id, base_sim, result_ctx)| {
            // Compute connectivity overlap
            let overlap = match (query_context, result_ctx.as_ref()) {
                (Some(q), Some(r)) => q.overlap_with(r),
                _ => 0.5,
            };

            // Blend semantic similarity with connectivity
            let semantic_weight = 1.0 - connectivity_weight;
            let blended = base_sim * semantic_weight + overlap * connectivity_weight;

            // Apply direction modifier
            let result_direction = if result_ctx
                .as_ref()
                .map(|c| !c.connected_entities.is_empty())
                .unwrap_or(false)
            {
                // If result has outgoing connections, it's a source
                GraphDirection::Source
            } else {
                GraphDirection::Unknown
            };

            let direction_mod = GraphDirection::direction_modifier(query_direction, result_direction);

            (*id, blended * direction_mod)
        })
        .collect();

    // Sort by descending score
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored
}

/// Helper: compute cosine similarity between two f32 slices.
///
/// Returns a value in [-1, 1]. For empty or zero-norm vectors, returns 0.0.
fn cosine_similarity_f32(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let norm_product = (norm_a * norm_b).sqrt();

    if norm_product < f32::EPSILON {
        0.0
    } else {
        dot / norm_product
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Direction Modifier Tests
    // ============================================================================

    #[test]
    fn test_direction_mod_source_to_target() {
        let modifier =
            GraphDirection::direction_modifier(GraphDirection::Source, GraphDirection::Target);
        assert_eq!(modifier, 1.2);
        println!("[VERIFIED] source→target direction_mod = 1.2");
    }

    #[test]
    fn test_direction_mod_target_to_source() {
        let modifier =
            GraphDirection::direction_modifier(GraphDirection::Target, GraphDirection::Source);
        assert_eq!(modifier, 0.8);
        println!("[VERIFIED] target→source direction_mod = 0.8");
    }

    #[test]
    fn test_direction_mod_same_direction() {
        assert_eq!(
            GraphDirection::direction_modifier(GraphDirection::Source, GraphDirection::Source),
            1.0
        );
        assert_eq!(
            GraphDirection::direction_modifier(GraphDirection::Target, GraphDirection::Target),
            1.0
        );
        println!("[VERIFIED] same_direction direction_mod = 1.0");
    }

    #[test]
    fn test_direction_mod_unknown() {
        assert_eq!(
            GraphDirection::direction_modifier(GraphDirection::Unknown, GraphDirection::Source),
            1.0
        );
        assert_eq!(
            GraphDirection::direction_modifier(GraphDirection::Target, GraphDirection::Unknown),
            1.0
        );
        println!("[VERIFIED] unknown direction_mod = 1.0");
    }

    // ============================================================================
    // Connectivity Context Tests
    // ============================================================================

    #[test]
    fn test_empty_contexts_neutral_overlap() {
        let ctx1 = ConnectivityContext::new();
        let ctx2 = ConnectivityContext::new();

        let overlap = ctx1.overlap_with(&ctx2);
        assert_eq!(overlap, 0.5);
        println!("[VERIFIED] Empty contexts → neutral overlap 0.5");
    }

    #[test]
    fn test_one_empty_context_minimal_overlap() {
        let ctx1 = ConnectivityContext::new().with_entity("utils");
        let ctx2 = ConnectivityContext::new();

        let overlap = ctx1.overlap_with(&ctx2);
        assert_eq!(overlap, 0.1);
        println!("[VERIFIED] One empty context → minimal overlap 0.1");
    }

    #[test]
    fn test_identical_entities_high_overlap() {
        let ctx1 = ConnectivityContext::new()
            .with_entity("utils")
            .with_entity("config");
        let ctx2 = ConnectivityContext::new()
            .with_entity("utils")
            .with_entity("config");

        let overlap = ctx1.overlap_with(&ctx2);
        // entity_jaccard = 1.0, entity_containment = 1.0, entity_blended = 1.0
        // rel_jaccard = 0.5 (neutral, no rels)
        // base = 1.0 * 0.5 + 0.5 * 0.3 = 0.65
        assert!(overlap > 0.6 && overlap < 0.7);
        println!("[VERIFIED] Identical entities → overlap ~0.65: {}", overlap);
    }

    #[test]
    fn test_partial_overlap() {
        let ctx1 = ConnectivityContext::new()
            .with_entity("utils")
            .with_entity("config");
        let ctx2 = ConnectivityContext::new()
            .with_entity("utils")
            .with_entity("database");

        let overlap = ctx1.overlap_with(&ctx2);
        // entity_jaccard = 1/3 = 0.333, entity_containment = 1/2 = 0.5
        // entity_blended = 0.5 * 0.333 + 0.5 * 0.5 = 0.4165
        // base = 0.4165 * 0.5 + 0.5 * 0.3 = 0.358
        assert!(overlap > 0.3 && overlap < 0.45);
        println!("[VERIFIED] Partial overlap computed correctly: {}", overlap);
    }

    #[test]
    fn test_relationship_type_bonus() {
        let ctx1 = ConnectivityContext::new()
            .with_entity("utils")
            .with_relationship("import");
        let ctx2 = ConnectivityContext::new()
            .with_entity("utils")
            .with_relationship("import");

        let ctx3 = ConnectivityContext::new()
            .with_entity("utils")
            .with_relationship("call");

        let overlap_same_rel = ctx1.overlap_with(&ctx2);
        let overlap_diff_rel = ctx1.overlap_with(&ctx3);

        assert!(overlap_same_rel > overlap_diff_rel);
        println!(
            "[VERIFIED] Same relationship type gives higher overlap: {} > {}",
            overlap_same_rel, overlap_diff_rel
        );
    }

    #[test]
    fn test_module_bonus() {
        let ctx1 = ConnectivityContext::new()
            .with_entity("utils")
            .with_module("auth::middleware");
        let ctx2 = ConnectivityContext::new()
            .with_entity("utils")
            .with_module("auth::middleware");

        let ctx3 = ConnectivityContext::new()
            .with_entity("utils")
            .with_module("database::pool");

        let overlap_same_module = ctx1.overlap_with(&ctx2);
        let overlap_diff_module = ctx1.overlap_with(&ctx3);

        assert!(overlap_same_module > overlap_diff_module);
        println!(
            "[VERIFIED] Same module gives higher overlap: {} > {}",
            overlap_same_module, overlap_diff_module
        );
    }

    #[test]
    fn test_depth_bonus() {
        let ctx1 = ConnectivityContext::new()
            .with_entity("utils")
            .with_depth(2);
        let ctx2 = ConnectivityContext::new()
            .with_entity("utils")
            .with_depth(2);

        let ctx3 = ConnectivityContext::new()
            .with_entity("utils")
            .with_depth(5);

        let overlap_same_depth = ctx1.overlap_with(&ctx2);
        let overlap_diff_depth = ctx1.overlap_with(&ctx3);

        assert!(overlap_same_depth > overlap_diff_depth);
        println!(
            "[VERIFIED] Same depth gives higher overlap: {} > {}",
            overlap_same_depth, overlap_diff_depth
        );
    }

    // ============================================================================
    // Asymmetric Similarity Formula Tests
    // ============================================================================

    #[test]
    fn test_formula_source_to_target_high_overlap() {
        let base = 0.8;
        let query_ctx = ConnectivityContext::new().with_entity("utils");
        let result_ctx = ConnectivityContext::new().with_entity("utils");

        let sim = compute_graph_asymmetric_similarity(
            base,
            GraphDirection::Source,
            GraphDirection::Target,
            Some(&query_ctx),
            Some(&result_ctx),
        );

        // direction_mod = 1.2
        // overlap is high due to identical entities
        // sim > base due to amplification
        assert!(sim > base);
        println!(
            "[VERIFIED] source→target with high overlap: {} > {} (base)",
            sim, base
        );
    }

    #[test]
    fn test_formula_target_to_source_high_overlap() {
        let base = 0.8;
        let query_ctx = ConnectivityContext::new().with_entity("utils");
        let result_ctx = ConnectivityContext::new().with_entity("utils");

        let sim = compute_graph_asymmetric_similarity(
            base,
            GraphDirection::Target,
            GraphDirection::Source,
            Some(&query_ctx),
            Some(&result_ctx),
        );

        // direction_mod = 0.8, so result should be dampened
        assert!(sim < base);
        println!(
            "[VERIFIED] target→source with high overlap: {} < {} (base)",
            sim, base
        );
    }

    #[test]
    fn test_formula_no_context() {
        let base = 0.8;

        let sim = compute_graph_asymmetric_similarity(
            base,
            GraphDirection::Source,
            GraphDirection::Target,
            None,
            None,
        );

        // direction_mod = 1.2, overlap = 0.5 (default)
        // factor = 0.7 + 0.3 * 0.5 = 0.85
        // sim = 0.8 * 1.2 * 0.85 = 0.816
        let expected = base * 1.2 * 0.85;
        assert!((sim - expected).abs() < 0.01);
        println!(
            "[VERIFIED] source→target no context: {} (expected {})",
            sim, expected
        );
    }

    #[test]
    fn test_simple_function_matches() {
        let base = 0.8;
        let query_dir = GraphDirection::Source;
        let result_dir = GraphDirection::Target;

        let full = compute_graph_asymmetric_similarity(base, query_dir, result_dir, None, None);
        let simple = compute_graph_asymmetric_similarity_simple(base, query_dir, result_dir);

        assert_eq!(full, simple);
        println!("[VERIFIED] Simple function matches full with None contexts");
    }

    #[test]
    fn test_batch_adjustment() {
        let query_dir = GraphDirection::Source;
        let query_ctx = ConnectivityContext::new().with_entity("utils");

        let result_ctx1 = ConnectivityContext::new().with_entity("utils");
        let result_ctx2 = ConnectivityContext::new().with_entity("database");

        let batch = vec![
            (0.8, GraphDirection::Target, Some(&result_ctx1)),
            (0.7, GraphDirection::Target, Some(&result_ctx2)),
            (0.9, GraphDirection::Source, None),
        ];

        let adjusted = adjust_batch_graph_similarities(&batch, query_dir, Some(&query_ctx));

        assert_eq!(adjusted.len(), 3);
        // First: source→target with high overlap → highest adjustment
        // Second: source→target with low overlap → lower adjustment
        assert!(adjusted[0] > adjusted[1]);
        println!("[VERIFIED] Batch adjustment produces {:?}", adjusted);
    }

    // ============================================================================
    // Constitution Compliance Tests
    // ============================================================================

    #[test]
    fn test_constitution_direction_mod_values() {
        // Constitution: source_to_target: 1.2
        assert_eq!(direction_mod::SOURCE_TO_TARGET, 1.2);
        // Constitution: target_to_source: 0.8
        assert_eq!(direction_mod::TARGET_TO_SOURCE, 0.8);
        // Constitution: same_direction: 1.0
        assert_eq!(direction_mod::SAME_DIRECTION, 1.0);

        println!("[VERIFIED] All direction_mod values match Constitution spec");
    }

    #[test]
    fn test_constitution_formula_components() {
        // Constitution formula: sim = base_cos × direction_mod × (0.7 + 0.3×connectivity_overlap)

        let base = 0.6;
        let direction_mod = 1.2;
        let connectivity_overlap = 0.5;

        // Manual calculation
        let expected = base * direction_mod * (0.7 + 0.3 * connectivity_overlap);

        // Via function (neutral overlap = 0.5)
        let actual = compute_graph_asymmetric_similarity(
            base,
            GraphDirection::Source,
            GraphDirection::Target,
            None,
            None,
        );

        assert!((actual - expected).abs() < 0.01);
        println!("[VERIFIED] Constitution formula implemented correctly");
        println!("  base_cos = {}", base);
        println!("  direction_mod = {} (source→target)", direction_mod);
        println!(
            "  connectivity_overlap = {} (neutral default)",
            connectivity_overlap
        );
        println!("  result = {} (expected {})", actual, expected);
    }

    #[test]
    fn test_asymmetry_effect() {
        // Same base similarity, but different directions should produce different results
        let base = 0.8;

        let source_to_target = compute_graph_asymmetric_similarity_simple(
            base,
            GraphDirection::Source,
            GraphDirection::Target,
        );

        let target_to_source = compute_graph_asymmetric_similarity_simple(
            base,
            GraphDirection::Target,
            GraphDirection::Source,
        );

        // source→target should be HIGHER than target→source
        assert!(source_to_target > target_to_source);

        // Ratio should be 1.2/0.8 = 1.5
        let ratio = source_to_target / target_to_source;
        assert!((ratio - 1.5).abs() < 0.01);

        println!(
            "[VERIFIED] Asymmetry: source→target ({}) > target→source ({})",
            source_to_target, target_to_source
        );
        println!("  Ratio: {} (expected 1.5)", ratio);
    }

    // ============================================================================
    // Graph Query Intent Detection Tests
    // ============================================================================

    #[test]
    fn test_detect_graph_query_what_imports() {
        assert_eq!(
            detect_graph_query_intent("what imports utils?"),
            GraphDirection::Source
        );
        assert_eq!(
            detect_graph_query_intent("what uses the database module?"),
            GraphDirection::Source
        );
        assert_eq!(
            detect_graph_query_intent("what calls this function?"),
            GraphDirection::Source
        );
        println!("[VERIFIED] 'what imports/uses/calls X' detected as Source-seeking");
    }

    #[test]
    fn test_detect_graph_query_what_does_import() {
        assert_eq!(
            detect_graph_query_intent("what does auth import?"),
            GraphDirection::Target
        );
        assert_eq!(
            detect_graph_query_intent("dependencies of this module"),
            GraphDirection::Target
        );
        assert_eq!(
            detect_graph_query_intent("show imports of auth"),
            GraphDirection::Target
        );
        println!("[VERIFIED] 'what does X import' detected as Target-seeking");
    }

    #[test]
    fn test_detect_graph_query_dependents() {
        assert_eq!(
            detect_graph_query_intent("dependents of utils"),
            GraphDirection::Source
        );
        assert_eq!(
            detect_graph_query_intent("consumers of this API"),
            GraphDirection::Source
        );
        assert_eq!(
            detect_graph_query_intent("callers of this function"),
            GraphDirection::Source
        );
        println!("[VERIFIED] 'dependents/consumers/callers of' detected as Source-seeking");
    }

    #[test]
    fn test_detect_graph_query_unknown() {
        assert_eq!(
            detect_graph_query_intent("show me the code"),
            GraphDirection::Unknown
        );
        assert_eq!(
            detect_graph_query_intent("list all files"),
            GraphDirection::Unknown
        );
        assert_eq!(
            detect_graph_query_intent("format this function"),
            GraphDirection::Unknown
        );
        println!("[VERIFIED] Non-graph queries detected as Unknown");
    }

    // ============================================================================
    // Cosine Similarity Tests
    // ============================================================================

    #[test]
    fn test_cosine_similarity_f32_basic() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity_f32(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6, "Identical vectors should have sim=1.0");

        let c = vec![0.0, 1.0, 0.0];
        let sim_ortho = cosine_similarity_f32(&a, &c);
        assert!(sim_ortho.abs() < 1e-6, "Orthogonal vectors should have sim=0.0");

        let d = vec![-1.0, 0.0, 0.0];
        let sim_opp = cosine_similarity_f32(&a, &d);
        assert!((sim_opp - (-1.0)).abs() < 1e-6, "Opposite vectors should have sim=-1.0");

        println!("[VERIFIED] cosine_similarity_f32 works correctly");
    }

    #[test]
    fn test_cosine_similarity_f32_edge_cases() {
        // Empty vectors
        let empty: Vec<f32> = vec![];
        assert_eq!(cosine_similarity_f32(&empty, &empty), 0.0);

        // Mismatched lengths
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity_f32(&a, &b), 0.0);

        // Zero norm vectors
        let zeros = vec![0.0, 0.0, 0.0];
        let ones = vec![1.0, 1.0, 1.0];
        assert_eq!(cosine_similarity_f32(&zeros, &ones), 0.0);

        println!("[VERIFIED] cosine_similarity_f32 handles edge cases");
    }

    // ============================================================================
    // GraphDirection Tests
    // ============================================================================

    #[test]
    fn test_graph_direction_display() {
        assert_eq!(format!("{}", GraphDirection::Source), "source");
        assert_eq!(format!("{}", GraphDirection::Target), "target");
        assert_eq!(format!("{}", GraphDirection::Unknown), "unknown");
        println!("[VERIFIED] GraphDirection display works");
    }

    #[test]
    fn test_graph_direction_from_str() {
        assert_eq!(GraphDirection::from_str("source"), GraphDirection::Source);
        assert_eq!(GraphDirection::from_str("SOURCE"), GraphDirection::Source);
        assert_eq!(GraphDirection::from_str("target"), GraphDirection::Target);
        assert_eq!(GraphDirection::from_str("TARGET"), GraphDirection::Target);
        assert_eq!(GraphDirection::from_str("other"), GraphDirection::Unknown);
        println!("[VERIFIED] GraphDirection from_str works");
    }

    #[test]
    fn test_graph_direction_default() {
        assert_eq!(GraphDirection::default(), GraphDirection::Unknown);
        println!("[VERIFIED] GraphDirection default is Unknown");
    }

    // ============================================================================
    // Rank by Connectivity Tests
    // ============================================================================

    #[test]
    fn test_rank_by_connectivity_basic() {
        let id1 = uuid::Uuid::new_v4();
        let id2 = uuid::Uuid::new_v4();
        let id3 = uuid::Uuid::new_v4();

        let ctx1 = ConnectivityContext::new().with_entity("utils").with_entity("config");
        let ctx2 = ConnectivityContext::new().with_entity("database");
        let ctx3 = ConnectivityContext::new().with_entity("utils");

        let query_ctx = ConnectivityContext::new().with_entity("utils");

        let results = vec![
            (id1, 0.7, Some(ctx1)),
            (id2, 0.8, Some(ctx2)),
            (id3, 0.6, Some(ctx3)),
        ];

        let ranked = rank_by_connectivity(
            &results,
            GraphDirection::Source,
            Some(&query_ctx),
            0.3, // 30% connectivity weight
        );

        assert_eq!(ranked.len(), 3);
        // Results should be re-ordered based on connectivity + similarity blend
        println!("[VERIFIED] rank_by_connectivity produces: {:?}", ranked);
    }
}
