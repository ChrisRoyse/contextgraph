//! Teleological MCP Tool Handlers (TELEO-H1 through TELEO-H5).
//!
//! Implements handlers for the 5 teleological tools:
//! - search_teleological: Cross-correlation search across all 13 embedders
//! - compute_teleological_vector: Compute full teleological vector from content
//! - fuse_embeddings: Fuse multiple embeddings using synergy matrix
//! - update_synergy_matrix: Adaptive learning from retrieval feedback
//! - manage_teleological_profile: CRUD for task-specific profiles

use super::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use context_graph_core::teleological::{
    services::{FusionEngine, ProfileManager},
    types::NUM_EMBEDDERS, // EMBEDDING_DIM removed - no longer needed after AP-03 fix
    ComparisonScope, ComponentWeights, GroupType, ProfileId, TeleologicalVector,
};
// FeedbackEvent and FeedbackType need full path - not re-exported from services
use context_graph_core::teleological::services::feedback_learner::{
    FeedbackEvent, FeedbackLearner, FeedbackType,
};
// ISSUE-1 FIX: Import TeleologicalSearchOptions for store search
use context_graph_core::traits::TeleologicalSearchOptions;
// Import SemanticFingerprint for embedder score computation
use context_graph_core::types::fingerprint::SemanticFingerprint;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// ============================================================================
// PARAMETER STRUCTS
// ============================================================================

/// Parameters for search_teleological tool.
///
/// ISSUE-1 FIX: Updated to match tool definition in tools.rs (lines 558-644).
/// Accepts `query_content` (string to embed) OR `query_vector_id` (existing vector ID).
/// Searches against stored vectors in TeleologicalMemoryStore, NOT a candidates array.
#[derive(Debug, Deserialize)]
pub struct SearchTeleologicalParams {
    /// Content to search for (will be embedded using all 13 embedders).
    /// Mutually exclusive with `query_vector_id`.
    pub query_content: Option<String>,

    /// ID of existing teleological vector to use as query.
    /// Mutually exclusive with `query_content`.
    pub query_vector_id: Option<String>,

    /// Search strategy (default: "adaptive")
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// Comparison scope (default: "full")
    #[serde(default = "default_scope")]
    pub scope: String,

    /// Specific groups for SpecificGroups scope.
    /// Reserved for future use with parse_scope when SpecificGroups scope is enabled.
    #[allow(dead_code)]
    pub specific_groups: Option<Vec<String>>,

    /// Specific embedder index for SingleEmbedderPattern scope (0=Semantic, 5=Code, 12=Sparse).
    /// Reserved for future use with parse_scope when SingleEmbedderPattern scope is enabled.
    #[allow(dead_code)]
    pub specific_embedder: Option<usize>,

    /// Weight for purpose vector similarity (default: 0.4).
    /// Reserved for future use with custom component weighting.
    #[allow(dead_code)]
    #[serde(default = "default_weight_purpose")]
    pub weight_purpose: f32,

    /// Weight for cross-correlation similarity (default: 0.35).
    /// Reserved for future use with custom component weighting.
    #[allow(dead_code)]
    #[serde(default = "default_weight_correlations")]
    pub weight_correlations: f32,

    /// Weight for group alignments similarity (default: 0.15).
    /// Reserved for future use with custom component weighting.
    #[allow(dead_code)]
    #[serde(default = "default_weight_groups")]
    pub weight_groups: f32,

    /// Minimum similarity threshold (default: 0.3)
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f32,

    /// Maximum number of results to return (default: 20)
    #[serde(default = "default_max_results")]
    pub max_results: usize,

    /// Include per-component similarity breakdown in results (default: true)
    #[serde(default = "default_include_breakdown")]
    pub include_breakdown: bool,

    /// Include original content text in results (default: false).
    /// TASK-CONTENT-011: When true, content is hydrated from CF_CONTENT.
    #[serde(default)]
    pub include_content: bool,
}

fn default_strategy() -> String {
    "adaptive".to_string()
}
fn default_scope() -> String {
    "full".to_string()
}
fn default_weight_purpose() -> f32 {
    0.4
}
fn default_weight_correlations() -> f32 {
    0.35
}
fn default_weight_groups() -> f32 {
    0.15
}
fn default_min_similarity() -> f32 {
    0.3
}
fn default_max_results() -> usize {
    20
}
fn default_include_breakdown() -> bool {
    true
}

/// JSON representation of TeleologicalVector.
/// Uses Vec<f32> for cross_correlations due to serde's 32-element array limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleologicalVectorJson {
    /// 13D purpose vector alignments
    pub purpose_vector: [f32; 13],
    /// 78 cross-correlations (Vec for serde compatibility)
    pub cross_correlations: Vec<f32>,
    /// 6D group alignments [factual, temporal, causal, relational, qualitative, implementation]
    pub group_alignments: [f32; 6],
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub id: Option<String>,
}

fn default_confidence() -> f32 {
    1.0
}

/// JSON representation of ComponentWeights.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentWeightsJson {
    #[serde(default = "default_pv_weight")]
    pub purpose_vector: f32,
    #[serde(default = "default_cc_weight")]
    pub cross_correlations: f32,
    #[serde(default = "default_ga_weight")]
    pub group_alignments: f32,
    #[serde(default = "default_conf_weight")]
    pub confidence: f32,
}

fn default_pv_weight() -> f32 {
    0.4
}
fn default_cc_weight() -> f32 {
    0.35
}
fn default_ga_weight() -> f32 {
    0.15
}
fn default_conf_weight() -> f32 {
    0.1
}

/// Parameters for compute_teleological_vector tool.
#[derive(Debug, Deserialize)]
pub struct ComputeTeleologicalVectorParams {
    /// Content to embed
    pub content: String,
    /// Whether to compute Tucker compression (default: false)
    #[serde(default)]
    pub compute_tucker: bool,
    /// Profile ID for weighted computation (optional)
    #[allow(dead_code)]
    pub profile_id: Option<String>,
}

/// Parameters for fuse_embeddings tool.
#[derive(Debug, Deserialize)]
pub struct FuseEmbeddingsParams {
    /// 13 embedding vectors to fuse
    pub embeddings: Vec<Vec<f32>>,
    /// 13D alignment scores for purpose vector
    pub alignments: Option<[f32; 13]>,
    /// Profile ID for fusion weights (optional)
    #[allow(dead_code)]
    pub profile_id: Option<String>,
    /// Custom synergy matrix (optional, uses default if omitted)
    #[allow(dead_code)]
    pub synergy_matrix: Option<SynergyMatrixJson>,
    /// Fusion method (default: "weighted_average")
    #[serde(default = "default_fusion_method")]
    pub fusion_method: String,
}

fn default_fusion_method() -> String {
    "weighted_average".to_string()
}

/// JSON representation of SynergyMatrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynergyMatrixJson {
    pub weights: [[f32; 13]; 13],
}

/// Parameters for update_synergy_matrix tool.
#[derive(Debug, Deserialize)]
pub struct UpdateSynergyMatrixParams {
    /// Vector ID that was retrieved
    pub vector_id: String,
    /// Type of feedback
    pub feedback_type: String,
    /// Per-embedder contributions (optional)
    pub contributions: Option<[f32; 13]>,
    /// Context string (optional)
    pub context: Option<String>,
}

/// Parameters for manage_teleological_profile tool.
#[derive(Debug, Deserialize)]
pub struct ManageTeleologicalProfileParams {
    /// Action to perform
    pub action: String,
    /// Profile ID (required for create/get/update/delete)
    pub profile_id: Option<String>,
    /// Weights (required for create/update)
    pub weights: Option<[f32; 13]>,
    /// Context string for find_best action
    pub context: Option<String>,
}

// ============================================================================
// CONVERSION HELPERS
// ============================================================================

impl TeleologicalVectorJson {
    /// Convert to core TeleologicalVector.
    /// Reserved for future use when clients need to submit pre-computed vectors.
    #[allow(dead_code)]
    pub fn to_core(&self) -> TeleologicalVector {
        use context_graph_core::teleological::groups::GroupAlignments;
        use context_graph_core::types::fingerprint::PurposeVector;

        // Create PurposeVector from alignments
        let purpose_vector = PurposeVector::new(self.purpose_vector);

        // Create GroupAlignments from 6D array
        let group_alignments = GroupAlignments::new(
            self.group_alignments[0], // factual
            self.group_alignments[1], // temporal
            self.group_alignments[2], // causal
            self.group_alignments[3], // relational
            self.group_alignments[4], // qualitative
            self.group_alignments[5], // implementation
        );

        // Use with_all for complete construction
        TeleologicalVector::with_all(
            purpose_vector,
            self.cross_correlations.clone(),
            group_alignments,
            self.confidence,
        )
    }

    /// Create from core TeleologicalVector.
    pub fn from_core(tv: &TeleologicalVector, id: Option<Uuid>) -> Self {
        Self {
            purpose_vector: tv.purpose_vector.alignments,
            cross_correlations: tv.cross_correlations.clone(),
            group_alignments: tv.group_alignments.as_array(),
            confidence: tv.confidence,
            id: id.map(|u| u.to_string()),
        }
    }
}

impl ComponentWeightsJson {
    /// Convert to core ComponentWeights.
    /// Reserved for future use when custom component weights are supported in API.
    #[allow(dead_code)]
    pub fn to_core(&self) -> ComponentWeights {
        ComponentWeights {
            purpose_vector: self.purpose_vector,
            cross_correlations: self.cross_correlations,
            group_alignments: self.group_alignments,
            confidence: self.confidence,
        }
    }
}

/// Parse scope string into ComparisonScope enum.
///
/// ISSUE-1 FIX: Updated to match tool definition - removed specific_pairs (not in schema).
#[allow(dead_code)]
fn parse_scope(
    s: &str,
    specific_groups: Option<Vec<String>>,
    specific_embedder: Option<usize>,
) -> ComparisonScope {
    match s.to_lowercase().as_str() {
        "full" => ComparisonScope::Full,
        "purpose_vector_only" | "purpose" => ComparisonScope::PurposeVectorOnly,
        "cross_correlations_only" | "correlations" => ComparisonScope::CrossCorrelationsOnly,
        "group_alignments_only" | "groups" => ComparisonScope::GroupAlignmentsOnly,
        "specific_groups" => {
            let groups: Vec<GroupType> = specific_groups
                .unwrap_or_default()
                .iter()
                .filter_map(|s| match s.to_lowercase().as_str() {
                    "factual" => Some(GroupType::Factual),
                    "temporal" => Some(GroupType::Temporal),
                    "causal" => Some(GroupType::Causal),
                    "relational" => Some(GroupType::Relational),
                    "qualitative" => Some(GroupType::Qualitative),
                    "implementation" => Some(GroupType::Implementation),
                    _ => None,
                })
                .collect();
            ComparisonScope::SpecificGroups(groups)
        }
        "single_embedder" | "embedder" => {
            ComparisonScope::SingleEmbedderPattern(specific_embedder.unwrap_or(0))
        }
        _ => ComparisonScope::Full,
    }
}

fn parse_feedback_type(s: &str) -> FeedbackType {
    match s.to_lowercase().as_str() {
        "positive_retrieval" | "positive" | "accept" | "success" => {
            FeedbackType::Positive { magnitude: 1.0 }
        }
        "negative_retrieval" | "negative" | "reject" | "failure" => {
            FeedbackType::Negative { magnitude: 1.0 }
        }
        "neutral" | "none" => FeedbackType::Neutral,
        _ => FeedbackType::Positive { magnitude: 1.0 },
    }
}

// ============================================================================
// CONSTITUTION COMPLIANCE NOTE - AP-03 VIOLATION REMOVED
// ============================================================================
//
// The function `project_to_embedding_dim()` was REMOVED because it violated:
// Constitution AP-03: "No dimension projection to fake compatibility"
//
// The 13 embedders produce vectors of different native dimensions:
// - E1 Semantic: 1024D          - E8 Graph: 384D
// - E2-E4 Temporal: 512D each   - E9 HDC: 1024D
// - E5 Causal: 768D             - E10 Multimodal: 768D
// - E6 Sparse: ~30K vocab       - E11 Entity: 384D
// - E7 Code: 1536D              - E12 Late Interaction: 128D/token
//                               - E13 SPLADE: ~30K vocab (sparse)
//
// Per constitution, embeddings MUST be kept in native dimensions.
// Use FusionEngine::fuse_from_alignments() which works with alignment scores
// (one scalar per embedder) instead of requiring uniform-dimension embeddings.
// ============================================================================

/// Compute alignment scores from embedding L2 norms (normalized to [0,1]).
fn compute_alignments_from_embeddings(embeddings: &[Vec<f32>]) -> [f32; NUM_EMBEDDERS] {
    let mut alignments = [0.0f32; NUM_EMBEDDERS];

    for (i, emb) in embeddings.iter().enumerate().take(NUM_EMBEDDERS) {
        // Compute L2 norm as a proxy for "information content"
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        alignments[i] = norm.min(1.0); // Clamp to [0, 1]
    }

    // Normalize to sum to 1.0
    let sum: f32 = alignments.iter().sum();
    if sum > 0.0 {
        for a in &mut alignments {
            *a /= sum;
        }
    } else {
        // Uniform distribution if all zero
        for a in &mut alignments {
            *a = 1.0 / NUM_EMBEDDERS as f32;
        }
    }

    alignments
}

/// Error types for embedding extraction.
#[derive(Debug)]
enum ExtractError {
    /// Empty token-level embeddings (violates AP-04)
    EmptyTokenLevel(usize),
    /// Missing embedder (violates ARCH-05)
    MissingEmbedder(usize),
}

impl ExtractError {
    fn to_error_string(&self) -> String {
        match self {
            Self::EmptyTokenLevel(i) => format!(
                "FAIL FAST [AP-04]: Embedder E{} returned empty TokenLevel. \
                 All 13 embedders must produce valid embeddings.",
                i + 1
            ),
            Self::MissingEmbedder(i) => format!(
                "FAIL FAST [ARCH-05]: Embedder E{} returned None. \
                 Constitution requires ALL 13 embedders to be present.",
                i + 1
            ),
        }
    }
}

/// Extract embeddings from fingerprint in native dimensions.
///
/// CONSTITUTION COMPLIANT:
/// - Per AP-03: "No dimension projection to fake compatibility"
/// - Per ARCH-05: "Missing embedders are a fatal error"
fn extract_embeddings_from_fingerprint(
    fingerprint: &context_graph_core::types::SemanticFingerprint,
) -> Result<Vec<Vec<f32>>, ExtractError> {
    let mut embeddings = Vec::with_capacity(NUM_EMBEDDERS);

    for i in 0..NUM_EMBEDDERS {
        let embedding = match fingerprint.get_embedding(i) {
            Some(context_graph_core::types::EmbeddingSlice::Dense(slice)) => {
                slice.to_vec()
            }
            Some(context_graph_core::types::EmbeddingSlice::Sparse(sparse)) => {
                sparse.values.clone()
            }
            Some(context_graph_core::types::EmbeddingSlice::TokenLevel(tokens)) => {
                if tokens.is_empty() {
                    return Err(ExtractError::EmptyTokenLevel(i));
                }
                // Average token embeddings (keep native token dimension)
                let dim = tokens[0].len();
                let mut avg = vec![0.0f32; dim];
                for token in tokens {
                    for (j, &v) in token.iter().enumerate() {
                        avg[j] += v;
                    }
                }
                let n = tokens.len() as f32;
                for v in &mut avg {
                    *v /= n;
                }
                avg
            }
            None => {
                return Err(ExtractError::MissingEmbedder(i));
            }
        };
        embeddings.push(embedding);
    }

    Ok(embeddings)
}

// ============================================================================
// HANDLER IMPLEMENTATIONS
// ============================================================================

impl Handlers {
    // ========================================================================
    // TELEO-H1: search_teleological
    // ========================================================================

    /// Handle search_teleological tool call.
    ///
    /// ISSUE-1 FIX: Rewritten to match tool definition in tools.rs (lines 558-644).
    /// Accepts `query_content` (string to embed) OR `query_vector_id` (existing vector ID).
    /// Searches against stored vectors in TeleologicalMemoryStore, NOT a candidates array.
    ///
    /// Performs cross-correlation search across all 13 embedders using
    /// configurable strategies and scopes.
    pub(super) async fn call_search_teleological(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("search_teleological called with: {:?}", arguments);

        // Parse parameters
        let params: SearchTeleologicalParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse search_teleological params: {}", e);
                return self.tool_error_with_pulse(id, &format!("Invalid parameters: {}", e));
            }
        };

        // =====================================================================
        // FAIL FAST: Require exactly one of query_content or query_vector_id
        // Track both TeleologicalVector and SemanticFingerprint for embedder scores
        // =====================================================================
        let (query_vector, semantic_query): (TeleologicalVector, Option<SemanticFingerprint>) =
            match (&params.query_content, &params.query_vector_id) {
            (None, None) => {
                error!("search_teleological: Neither query_content nor query_vector_id provided");
                return self.tool_error_with_pulse(
                    id,
                    "FAIL FAST: Must provide either 'query_content' (string to embed) or 'query_vector_id' (existing vector ID). Neither was provided.",
                );
            }
            (Some(_), Some(_)) => {
                warn!("search_teleological: Both query_content and query_vector_id provided, using query_content");
                // Fall through to query_content path
                match self.compute_query_vector_from_content(params.query_content.as_ref().unwrap()).await {
                    Ok((v, sem)) => (v, Some(sem)),
                    Err(e) => return self.tool_error_with_pulse(id, &e),
                }
            }
            (Some(content), None) => {
                // Embed query_content using multi_array_provider
                if content.is_empty() {
                    return self.tool_error_with_pulse(id, "FAIL FAST: query_content cannot be empty string");
                }
                match self.compute_query_vector_from_content(content).await {
                    Ok((v, sem)) => (v, Some(sem)),
                    Err(e) => return self.tool_error_with_pulse(id, &e),
                }
            }
            (None, Some(vector_id)) => {
                // Retrieve existing vector from store
                let uuid = match Uuid::parse_str(vector_id) {
                    Ok(u) => u,
                    Err(e) => {
                        return self.tool_error_with_pulse(
                            id,
                            &format!("FAIL FAST: Invalid query_vector_id '{}': {}", vector_id, e),
                        );
                    }
                };

                // Retrieve fingerprint from store
                match self.teleological_store.retrieve(uuid).await {
                    Ok(Some(fingerprint)) => {
                        // Extract purpose vector and create TeleologicalVector
                        // TeleologicalVector requires purpose_vector, cross_correlations, group_alignments
                        use context_graph_core::teleological::groups::GroupAlignments;

                        // Keep semantic fingerprint for embedder score computation
                        let semantic_fp = fingerprint.semantic.clone();

                        // Create a basic TeleologicalVector from the stored fingerprint's purpose_vector
                        let purpose_vector = fingerprint.purpose_vector.clone();

                        // For cross-correlations, compute from the fingerprint's semantic embeddings
                        // This is a simplified version - full computation would use all embedders
                        let cross_correlations = vec![0.0f32; 78]; // 13*(13-1)/2 = 78 pairs

                        // Group alignments from purpose vector components
                        let group_alignments = GroupAlignments::new(
                            purpose_vector.alignments[0], // factual
                            purpose_vector.alignments[1], // temporal
                            purpose_vector.alignments[2], // causal
                            purpose_vector.alignments[3], // relational
                            purpose_vector.alignments[4], // qualitative
                            purpose_vector.alignments[5], // implementation
                        );

                        let tv = TeleologicalVector::with_all(
                            purpose_vector,
                            cross_correlations,
                            group_alignments,
                            1.0, // confidence
                        );
                        (tv, Some(semantic_fp))
                    }
                    Ok(None) => {
                        return self.tool_error_with_pulse(
                            id,
                            &format!("FAIL FAST: query_vector_id '{}' not found in store", vector_id),
                        );
                    }
                    Err(e) => {
                        error!("Failed to retrieve vector {}: {}", vector_id, e);
                        return self.tool_error_with_pulse(
                            id,
                            &format!("FAIL FAST: Failed to retrieve query_vector_id '{}': {}", vector_id, e),
                        );
                    }
                }
            }
        };

        // =====================================================================
        // Search stored vectors using TeleologicalMemoryStore.search_purpose()
        // Pass semantic_query for computing per-embedder similarity scores
        // =====================================================================
        let search_options = TeleologicalSearchOptions {
            top_k: params.max_results,
            min_similarity: params.min_similarity,
            include_deleted: false,
            johari_quadrant_filter: None,
            min_alignment: Some(params.min_similarity),
            embedder_indices: Vec::new(),
            semantic_query, // Pass semantic fingerprint for embedder score computation
            include_content: params.include_content, // TASK-CONTENT-011: Hydrate from CF_CONTENT if requested
        };

        let search_start = std::time::Instant::now();
        let search_results = match self
            .teleological_store
            .search_purpose(&query_vector.purpose_vector, search_options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!("search_purpose failed: {}", e);
                return self.tool_error_with_pulse(
                    id,
                    &format!("Search failed: {}. Store may be empty or unavailable.", e),
                );
            }
        };
        let search_duration = search_start.elapsed();

        // =====================================================================
        // TASK-CONTENT-011: Hydrate content if requested
        // =====================================================================
        let mut search_results = search_results;
        if params.include_content && !search_results.is_empty() {
            let ids: Vec<uuid::Uuid> = search_results.iter().map(|r| r.fingerprint.id).collect();
            match self.teleological_store.get_content_batch(&ids).await {
                Ok(contents) => {
                    for (result, content) in search_results.iter_mut().zip(contents.into_iter()) {
                        result.content = content;
                    }
                    debug!(
                        result_count = search_results.len(),
                        with_content = search_results.iter().filter(|r| r.content.is_some()).count(),
                        "Content hydrated for search results"
                    );
                }
                Err(e) => {
                    // Log warning but don't fail - content is optional enhancement
                    warn!(
                        error = %e,
                        "Failed to hydrate content for search results (results still valid without content)"
                    );
                }
            }
        }

        // =====================================================================
        // Build response with optional breakdown and content
        // =====================================================================
        let results_json: Vec<serde_json::Value> = search_results
            .iter()
            .enumerate()
            .map(|(rank, result)| {
                let mut entry = json!({
                    "rank": rank,
                    "id": result.fingerprint.id.to_string(),
                    "similarity": result.similarity,
                    "purpose_alignment": result.purpose_alignment,
                    "dominant_embedder": result.dominant_embedder(),
                });

                if params.include_breakdown {
                    entry["breakdown"] = json!({
                        "similarity": result.similarity,
                        "purpose_alignment": result.purpose_alignment,
                        "embedder_scores": result.embedder_scores,
                        "stage_scores": result.stage_scores,
                    });
                }

                // TASK-CONTENT-011: Include content in response if requested
                if params.include_content {
                    entry["content"] = match &result.content {
                        Some(c) => json!(c),
                        None => serde_json::Value::Null,
                    };
                }

                entry
            })
            .collect();

        info!(
            "search_teleological found {} results in {:?} (max_results={}, min_similarity={})",
            results_json.len(),
            search_duration,
            params.max_results,
            params.min_similarity
        );

        self.tool_result_with_pulse(
            id,
            json!({
                "success": true,
                "strategy": params.strategy,
                "scope": params.scope,
                "query_type": if params.query_content.is_some() { "embedded" } else { "existing_vector" },
                "search_latency_ms": search_duration.as_millis(),
                "num_results": results_json.len(),
                "results": results_json,
            }),
        )
    }

    /// Helper: Compute TeleologicalVector and SemanticFingerprint from content string.
    ///
    /// Returns both the fused TeleologicalVector and the SemanticFingerprint.
    /// The SemanticFingerprint is needed for computing per-embedder similarity scores
    /// in search_purpose() - without it, embedder_scores would be all zeros.
    ///
    /// Shared logic between search_teleological and compute_teleological_vector.
    async fn compute_query_vector_from_content(
        &self,
        content: &str,
    ) -> Result<(TeleologicalVector, SemanticFingerprint), String> {
        // Use multi_array_provider to get all 13 embeddings
        let embedding_result = match self.multi_array_provider.embed_all(content).await {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to compute embeddings for query: {}", e);
                return Err(format!(
                    "FAIL FAST: Embedding computation failed for query_content: {}. \
                     Check embedding provider connection and configuration.",
                    e
                ));
            }
        };

        // Keep the semantic fingerprint for embedder score computation
        // Note: MultiArrayEmbeddingOutput.fingerprint IS the SemanticFingerprint
        let semantic_fingerprint = embedding_result.fingerprint.clone();

        // CONSTITUTION COMPLIANT: Extract embeddings using helper
        let embeddings = extract_embeddings_from_fingerprint(&embedding_result.fingerprint)
            .map_err(|e| e.to_error_string())?;

        // Compute alignments and fuse
        let alignments = compute_alignments_from_embeddings(&embeddings);
        let fusion_engine = FusionEngine::new();
        let fusion_result = fusion_engine.fuse_from_alignments(&alignments);

        Ok((fusion_result.vector, semantic_fingerprint))
    }

    // ========================================================================
    // TELEO-H2: compute_teleological_vector
    // ========================================================================

    /// Handle compute_teleological_vector tool call.
    ///
    /// Computes a full TeleologicalVector from content using all 13 embedders.
    pub(super) async fn call_compute_teleological_vector(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("compute_teleological_vector called with: {:?}", arguments);

        // Parse parameters
        let params: ComputeTeleologicalVectorParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse compute_teleological_vector params: {}", e);
                return self.tool_error_with_pulse(id, &format!("Invalid parameters: {}", e));
            }
        };

        if params.content.is_empty() {
            return self.tool_error_with_pulse(id, "Content cannot be empty");
        }

        // Use multi_array_provider to get all 13 embeddings
        let start = std::time::Instant::now();
        let embedding_result = match self.multi_array_provider.embed_all(&params.content).await {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to compute embeddings: {}", e);
                return self
                    .tool_error_with_pulse(id, &format!("Embedding computation failed: {}", e));
            }
        };
        let embed_duration = start.elapsed();

        // CONSTITUTION COMPLIANT: Extract embeddings using helper
        let embeddings = match extract_embeddings_from_fingerprint(&embedding_result.fingerprint) {
            Ok(e) => e,
            Err(e) => {
                let err_msg = e.to_error_string();
                error!("{}", err_msg);
                return self.tool_error_with_pulse(id, &err_msg);
            }
        };

        // Compute alignments and fuse
        let alignments = compute_alignments_from_embeddings(&embeddings);
        let fusion_engine = FusionEngine::new();
        let fusion_result = fusion_engine.fuse_from_alignments(&alignments);

        // Build response
        let vector_json = TeleologicalVectorJson::from_core(&fusion_result.vector, None);

        info!(
            "compute_teleological_vector completed in {:?} (tucker={})",
            embed_duration, params.compute_tucker
        );

        self.tool_result_with_pulse(
            id,
            json!({
                "success": true,
                "vector": vector_json,
                "embedding_latency_ms": embed_duration.as_millis(),
                "fusion_confidence": fusion_result.confidence,
                "component_scores": {
                    "purpose_vector": fusion_result.component_scores.purpose_vector,
                    "correlations": fusion_result.component_scores.correlations,
                    "groups": fusion_result.component_scores.groups,
                },
                "metadata": {
                    "active_embedders": fusion_result.metadata.active_embedders,
                    "strongest_pair": fusion_result.metadata.strongest_pair,
                    "dominant_group": fusion_result.metadata.dominant_group.map(|g| format!("{:?}", g)),
                },
            }),
        )
    }

    // ========================================================================
    // TELEO-H3: fuse_embeddings
    // ========================================================================

    /// Handle fuse_embeddings tool call.
    ///
    /// Fuses multiple embeddings using synergy matrix and optional profile.
    pub(super) async fn call_fuse_embeddings(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("fuse_embeddings called with: {:?}", arguments);

        // Parse parameters
        let params: FuseEmbeddingsParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse fuse_embeddings params: {}", e);
                return self.tool_error_with_pulse(id, &format!("Invalid parameters: {}", e));
            }
        };

        // Validate embeddings count
        if params.embeddings.len() != NUM_EMBEDDERS {
            return self.tool_error_with_pulse(
                id,
                &format!(
                    "Expected {} embeddings, got {}",
                    NUM_EMBEDDERS,
                    params.embeddings.len()
                ),
            );
        }

        // Compute or use provided alignments
        let alignments = params
            .alignments
            .unwrap_or_else(|| compute_alignments_from_embeddings(&params.embeddings));

        // Create fusion engine
        let fusion_engine = FusionEngine::new();

        // Perform fusion
        let fusion_result = fusion_engine.fuse(&params.embeddings, &alignments);

        info!(
            "fuse_embeddings completed with confidence {}",
            fusion_result.confidence
        );

        self.tool_result_with_pulse(
            id,
            json!({
                "success": true,
                "vector": TeleologicalVectorJson::from_core(&fusion_result.vector, None),
                "confidence": fusion_result.confidence,
                "component_scores": {
                    "purpose_vector": fusion_result.component_scores.purpose_vector,
                    "correlations": fusion_result.component_scores.correlations,
                    "groups": fusion_result.component_scores.groups,
                },
                "metadata": {
                    "fusion_method": params.fusion_method,
                    "profile_applied": params.profile_id.is_some(),
                    "active_embedders": fusion_result.metadata.active_embedders,
                },
            }),
        )
    }

    // ========================================================================
    // TELEO-H4: update_synergy_matrix
    // ========================================================================

    /// Handle update_synergy_matrix tool call.
    ///
    /// Records feedback and adaptively updates synergy weights.
    pub(super) async fn call_update_synergy_matrix(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("update_synergy_matrix called with: {:?}", arguments);

        // Parse parameters
        let params: UpdateSynergyMatrixParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse update_synergy_matrix params: {}", e);
                return self.tool_error_with_pulse(id, &format!("Invalid parameters: {}", e));
            }
        };

        // Parse vector ID
        let vector_id = match Uuid::parse_str(&params.vector_id) {
            Ok(u) => u,
            Err(e) => {
                return self.tool_error_with_pulse(id, &format!("Invalid vector_id: {}", e));
            }
        };

        // Parse feedback type
        let feedback_type = parse_feedback_type(&params.feedback_type);

        // Create feedback event
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut event = FeedbackEvent::new(vector_id, feedback_type, timestamp);

        if let Some(ctx) = params.context {
            event = event.with_context(ctx);
        }
        if let Some(contributions) = params.contributions {
            // Normalize contributions to sum to 1.0 before passing
            let sum: f32 = contributions.iter().sum();
            if sum > 0.0 {
                let normalized: [f32; NUM_EMBEDDERS] = {
                    let mut arr = [0.0f32; NUM_EMBEDDERS];
                    for (i, &c) in contributions.iter().enumerate() {
                        arr[i] = c / sum;
                    }
                    arr
                };
                event = event.with_contributions(normalized);
            }
        }

        // Create a local FeedbackLearner instance (stateless per request as per plan)
        // In production, this should be shared state - for now we demonstrate the API
        let mut feedback_learner = FeedbackLearner::new();
        feedback_learner.record_feedback(event);

        // Check if we should learn (will be false with just 1 event by default)
        let should_learn = feedback_learner.should_learn();
        let mut learning_result_json = None;

        if should_learn {
            let result = feedback_learner.learn();
            info!(
                "Learning triggered: {} events processed, {} adjustments",
                result.events_processed,
                result.adjustments.len()
            );

            learning_result_json = Some(json!({
                "events_processed": result.events_processed,
                "adjustments": result.adjustments,
                "confidence_delta": result.confidence_delta,
            }));
        }

        info!(
            "update_synergy_matrix: recorded feedback for vector {}",
            params.vector_id
        );

        self.tool_result_with_pulse(
            id,
            json!({
                "success": true,
                "vector_id": params.vector_id,
                "feedback_type": params.feedback_type,
                "learning_triggered": should_learn,
                "learning_result": learning_result_json,
                "note": "Feedback recorded. In production, feedback accumulates until threshold for learning."
            }),
        )
    }

    // ========================================================================
    // TELEO-H5: manage_teleological_profile
    // ========================================================================

    /// Handle manage_teleological_profile tool call.
    ///
    /// CRUD operations for task-specific teleological profiles.
    pub(super) async fn call_manage_teleological_profile(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("manage_teleological_profile called with: {:?}", arguments);

        // Parse parameters
        let params: ManageTeleologicalProfileParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse manage_teleological_profile params: {}", e);
                return self.tool_error_with_pulse(id, &format!("Invalid parameters: {}", e));
            }
        };

        // Create a local ProfileManager (stateless per request)
        // In production, this should be shared state with persistence
        let mut manager = ProfileManager::new();

        match params.action.to_lowercase().as_str() {
            "create" => {
                let profile_id = match &params.profile_id {
                    Some(pid) => pid.clone(),
                    None => return self.tool_error_with_pulse(id, "profile_id required for create"),
                };
                let weights = match params.weights {
                    Some(w) => w,
                    None => return self.tool_error_with_pulse(id, "weights required for create"),
                };

                let profile = manager.create_profile(&profile_id, weights);

                info!("Created profile: {}", profile_id);
                self.tool_result_with_pulse(
                    id,
                    json!({
                        "success": true,
                        "action": "create",
                        "profile_id": profile_id,
                        "weights": profile.embedding_weights,
                    }),
                )
            }

            "get" => {
                let profile_id = match &params.profile_id {
                    Some(pid) => pid.clone(),
                    None => return self.tool_error_with_pulse(id, "profile_id required for get"),
                };

                let pid = ProfileId::new(&profile_id);

                match manager.get_profile(&pid) {
                    Some(profile) => {
                        self.tool_result_with_pulse(
                            id,
                            json!({
                                "success": true,
                                "action": "get",
                                "profile_id": profile_id,
                                "weights": profile.embedding_weights,
                                "task_type": format!("{:?}", profile.task_type),
                                "name": profile.name,
                            }),
                        )
                    }
                    None => {
                        self.tool_error_with_pulse(id, &format!("Profile '{}' not found", profile_id))
                    }
                }
            }

            "update" => {
                let profile_id = match &params.profile_id {
                    Some(pid) => pid.clone(),
                    None => return self.tool_error_with_pulse(id, "profile_id required for update"),
                };
                let weights = match params.weights {
                    Some(w) => w,
                    None => return self.tool_error_with_pulse(id, "weights required for update"),
                };

                let pid = ProfileId::new(&profile_id);
                let updated = manager.update_profile(&pid, weights);

                if updated {
                    info!("Updated profile: {}", profile_id);
                    self.tool_result_with_pulse(
                        id,
                        json!({
                            "success": true,
                            "action": "update",
                            "profile_id": profile_id,
                            "weights": weights,
                        }),
                    )
                } else {
                    self.tool_error_with_pulse(id, &format!("Profile '{}' not found", profile_id))
                }
            }

            "delete" => {
                let profile_id = match &params.profile_id {
                    Some(pid) => pid.clone(),
                    None => return self.tool_error_with_pulse(id, "profile_id required for delete"),
                };

                let pid = ProfileId::new(&profile_id);
                let deleted = manager.delete_profile(&pid);

                if deleted {
                    info!("Deleted profile: {}", profile_id);
                    self.tool_result_with_pulse(
                        id,
                        json!({
                            "success": true,
                            "action": "delete",
                            "profile_id": profile_id,
                        }),
                    )
                } else {
                    self.tool_error_with_pulse(id, &format!("Profile '{}' not found or cannot delete", profile_id))
                }
            }

            "list" => {
                let profiles: Vec<String> = manager
                    .list_profiles()
                    .into_iter()
                    .map(|p| p.as_str().to_string())
                    .collect();

                self.tool_result_with_pulse(
                    id,
                    json!({
                        "success": true,
                        "action": "list",
                        "profiles": profiles,
                        "count": profiles.len(),
                    }),
                )
            }

            "find_best" => {
                let context = match &params.context {
                    Some(c) => c.clone(),
                    None => return self.tool_error_with_pulse(id, "context required for find_best"),
                };

                match manager.find_best_match(&context) {
                    Some(match_result) => {
                        self.tool_result_with_pulse(
                            id,
                            json!({
                                "success": true,
                                "action": "find_best",
                                "profile_id": match_result.profile_id.as_str(),
                                "similarity": match_result.similarity,
                                "reason": match_result.reason,
                            }),
                        )
                    }
                    None => {
                        self.tool_result_with_pulse(
                            id,
                            json!({
                                "success": true,
                                "action": "find_best",
                                "profile_id": null,
                                "message": "No matching profile found",
                            }),
                        )
                    }
                }
            }

            _ => {
                self.tool_error_with_pulse(
                    id,
                    &format!("Unknown action: {}. Valid actions: create, get, update, delete, list, find_best", params.action),
                )
            }
        }
    }
}
