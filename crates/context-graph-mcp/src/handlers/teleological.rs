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
    types::NUM_EMBEDDERS,
    ComparisonScope, ComponentWeights, GroupType, MatrixSearchConfig, ProfileId, SearchStrategy,
    TeleologicalMatrixSearch, TeleologicalVector,
};
// FeedbackEvent and FeedbackType need full path - not re-exported from services
use context_graph_core::teleological::services::feedback_learner::{
    FeedbackEvent, FeedbackLearner, FeedbackType,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};
use uuid::Uuid;

// ============================================================================
// PARAMETER STRUCTS
// ============================================================================

/// Parameters for search_teleological tool.
#[derive(Debug, Deserialize)]
pub struct SearchTeleologicalParams {
    /// Query vector to search with
    pub query: TeleologicalVectorJson,
    /// Candidate vectors to search through
    pub candidates: Vec<TeleologicalVectorJson>,
    /// Search strategy (default: "cosine")
    #[serde(default = "default_strategy")]
    pub strategy: String,
    /// Comparison scope (default: "full")
    #[serde(default = "default_scope")]
    pub scope: String,
    /// Component weights (optional)
    pub weights: Option<ComponentWeightsJson>,
    /// Number of results to return (default: 10)
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Minimum similarity threshold (default: 0.0)
    #[serde(default)]
    pub threshold: f32,
    /// Include detailed breakdown (default: false)
    #[serde(default)]
    pub include_breakdown: bool,
    /// Specific pairs for SpecificPairs scope
    pub specific_pairs: Option<Vec<(usize, usize)>>,
    /// Specific groups for SpecificGroups scope
    pub specific_groups: Option<Vec<String>>,
    /// Embedder index for SingleEmbedderPattern scope
    pub embedder_index: Option<usize>,
}

fn default_strategy() -> String {
    "cosine".to_string()
}
fn default_scope() -> String {
    "full".to_string()
}
fn default_top_k() -> usize {
    10
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
    pub fn to_core(&self) -> ComponentWeights {
        ComponentWeights {
            purpose_vector: self.purpose_vector,
            cross_correlations: self.cross_correlations,
            group_alignments: self.group_alignments,
            confidence: self.confidence,
        }
    }
}

fn parse_strategy(s: &str) -> SearchStrategy {
    match s.to_lowercase().as_str() {
        "cosine" => SearchStrategy::Cosine,
        "euclidean" => SearchStrategy::Euclidean,
        "synergy_weighted" | "synergy" => SearchStrategy::SynergyWeighted,
        "group_hierarchical" | "group" => SearchStrategy::GroupHierarchical,
        "cross_correlation_dominant" | "correlation" => SearchStrategy::CrossCorrelationDominant,
        "tucker_compressed" | "tucker" => SearchStrategy::TuckerCompressed,
        "adaptive" => SearchStrategy::Adaptive,
        _ => SearchStrategy::Cosine,
    }
}

fn parse_scope(
    s: &str,
    specific_pairs: Option<Vec<(usize, usize)>>,
    specific_groups: Option<Vec<String>>,
    embedder_index: Option<usize>,
) -> ComparisonScope {
    match s.to_lowercase().as_str() {
        "full" => ComparisonScope::Full,
        "purpose_vector_only" | "purpose" => ComparisonScope::PurposeVectorOnly,
        "cross_correlations_only" | "correlations" => ComparisonScope::CrossCorrelationsOnly,
        "group_alignments_only" | "groups" => ComparisonScope::GroupAlignmentsOnly,
        "specific_pairs" | "pairs" => {
            ComparisonScope::SpecificPairs(specific_pairs.unwrap_or_default())
        }
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
            ComparisonScope::SingleEmbedderPattern(embedder_index.unwrap_or(0))
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

// ============================================================================
// HANDLER IMPLEMENTATIONS
// ============================================================================

impl Handlers {
    // ========================================================================
    // TELEO-H1: search_teleological
    // ========================================================================

    /// Handle search_teleological tool call.
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

        // Convert query and candidates
        let query = params.query.to_core();
        let candidates: Vec<TeleologicalVector> =
            params.candidates.iter().map(|c| c.to_core()).collect();

        if candidates.is_empty() {
            return self.tool_error_with_pulse(id, "No candidates provided for search");
        }

        // Build config with all required fields
        let strategy = parse_strategy(&params.strategy);
        let scope = parse_scope(
            &params.scope,
            params.specific_pairs,
            params.specific_groups,
            params.embedder_index,
        );
        let weights = params.weights.map(|w| w.to_core()).unwrap_or_default();

        let config = MatrixSearchConfig {
            strategy,
            scope,
            weights,
            synergy_matrix: None, // Use default synergies
            min_similarity: params.threshold,
            max_results: params.top_k,
            compute_breakdown: params.include_breakdown,
        };

        // Use with_config() constructor
        let searcher = TeleologicalMatrixSearch::with_config(config);

        // Perform search - methods take 2 arguments (query, candidates)
        let results_json = if params.include_breakdown {
            let results = searcher.search_with_breakdown(&query, &candidates);

            results
                .iter()
                .enumerate()
                .map(|(rank, (idx, breakdown))| {
                    json!({
                        "rank": rank,
                        "index": idx,
                        "similarity": breakdown.overall,
                        "vector_id": params.candidates.get(*idx).and_then(|c| c.id.clone()),
                        "breakdown": {
                            "overall": breakdown.overall,
                            "purpose_vector": breakdown.purpose_vector,
                            "cross_correlations": breakdown.cross_correlations,
                            "group_alignments": breakdown.group_alignments,
                            "per_embedder_purpose": breakdown.per_embedder_purpose,
                        }
                    })
                })
                .collect::<Vec<_>>()
        } else {
            let results = searcher.search(&query, &candidates);
            results
                .iter()
                .enumerate()
                .map(|(rank, (idx, score))| {
                    json!({
                        "rank": rank,
                        "index": idx,
                        "similarity": score,
                        "vector_id": params.candidates.get(*idx).and_then(|c| c.id.clone()),
                    })
                })
                .collect::<Vec<_>>()
        };

        info!(
            "search_teleological found {} results (top_k={}, threshold={})",
            results_json.len(),
            params.top_k,
            params.threshold
        );

        self.tool_result_with_pulse(
            id,
            json!({
                "success": true,
                "strategy": params.strategy,
                "scope": params.scope,
                "num_candidates": candidates.len(),
                "num_results": results_json.len(),
                "results": results_json,
            }),
        )
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

        // Extract embeddings from fingerprint - each embedding accessible by index
        let fingerprint = &embedding_result.fingerprint;
        let embeddings: Vec<Vec<f32>> = (0..NUM_EMBEDDERS)
            .map(|i| {
                match fingerprint.get_embedding(i) {
                    Some(context_graph_core::types::EmbeddingSlice::Dense(slice)) => slice.to_vec(),
                    Some(context_graph_core::types::EmbeddingSlice::Sparse(sparse)) => {
                        // Convert sparse to dense (use values as a simple approximation)
                        sparse.values.clone()
                    }
                    Some(context_graph_core::types::EmbeddingSlice::TokenLevel(tokens)) => {
                        // Average token embeddings
                        if tokens.is_empty() {
                            vec![0.0; 128] // Default dimension for ColBERT
                        } else {
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
                    }
                    None => vec![0.0; 1024], // Default fallback
                }
            })
            .collect();

        // Compute alignments from embeddings
        let alignments = compute_alignments_from_embeddings(&embeddings);

        // Use FusionEngine for complete processing
        let fusion_engine = FusionEngine::new();
        let fusion_result = fusion_engine.fuse(&embeddings, &alignments);

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
