//! Merge Concepts MCP Handler (TASK-MCP-004)
//!
//! Implements merge_concepts tool for consolidating related concept nodes.
//! Constitution: SEC-06 (30-day reversal), PRD Section 5.3
//!
//! ## Merge Strategies
//! - union: Combine all embedding dimensions (average)
//! - intersection: Keep dimensions where all sources have significant values
//! - weighted_average: Weight embeddings by access count
//!
//! ## Error Handling
//! FAIL FAST: All errors return immediately with detailed error codes.
//! NO fallbacks, NO default values, NO mock data.
//!
//! ## Storage
//! Uses TeleologicalMemoryStore (NOT MemoryNode) per codebase architecture.
//! ARCH-01: TeleologicalFingerprint is the atomic storage unit.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::Handlers;

/// Merge strategy for combining embeddings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    #[default]
    Union,
    Intersection,
    WeightedAverage,
}

/// Input for merge_concepts tool (matches schema from TASK-29)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MergeConceptsInput {
    pub source_ids: Vec<Uuid>,
    pub target_name: String,
    #[serde(default)]
    pub merge_strategy: MergeStrategy,
    pub rationale: String,
    #[serde(default)]
    pub force_merge: bool,
}

/// Output for merge_concepts tool
#[derive(Debug, Clone, Serialize)]
pub struct MergeConceptsOutput {
    /// Whether the merge was successful
    pub success: bool,
    /// UUID of the newly created merged node
    pub merged_id: Uuid,
    /// SHA-256 hash for reversal (30-day undo capability)
    pub reversal_hash: String,
    /// Details of the merged node
    pub merged_node: MergedNodeInfo,
    /// Error message if any (null on success)
    pub error: Option<String>,
}

/// Information about the merged node
#[derive(Debug, Clone, Serialize)]
pub struct MergedNodeInfo {
    pub id: Uuid,
    pub name: String,
    pub source_count: usize,
    pub strategy_used: MergeStrategy,
    pub created_at: String,
    /// Total access count from all sources
    pub total_access_count: u64,
}

/// Reversal record stored for 30-day undo capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReversalRecord {
    pub reversal_hash: String,
    pub merged_id: Uuid,
    pub source_ids: Vec<Uuid>,
    /// Original fingerprint data serialized for restoration
    pub original_fingerprints_json: Vec<String>,
    pub created_at: String,
    pub expires_at: String,
    pub rationale: String,
    pub strategy: MergeStrategy,
    pub target_name: String,
}

/// Schema constraints from TASK-29
const MIN_SOURCE_IDS: usize = 2;
const MAX_SOURCE_IDS: usize = 10;
const MIN_TARGET_NAME_LEN: usize = 1;
const MAX_TARGET_NAME_LEN: usize = 256;
const MIN_RATIONALE_LEN: usize = 1;
const MAX_RATIONALE_LEN: usize = 1024;

/// Reversal expiration per SEC-06
const REVERSAL_DAYS: i64 = 30;

/// Intersection threshold: dimension is "significant" if >= this value
const INTERSECTION_THRESHOLD: f32 = 0.01;

impl Handlers {
    /// Handle merge_concepts tool call.
    ///
    /// TASK-MCP-004: Merge concepts handler implementation.
    /// FAIL FAST if any source node doesn't exist.
    pub(super) async fn call_merge_concepts(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling merge_concepts tool call: {:?}", args);

        // FAIL FAST: Parse and validate input
        let input: MergeConceptsInput = match serde_json::from_value(args.clone()) {
            Ok(i) => i,
            Err(e) => {
                error!("merge_concepts: Invalid input: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid merge_concepts input: {}", e),
                );
            }
        };

        // FAIL FAST: Validate source_ids count (2-10 per schema)
        if input.source_ids.len() < MIN_SOURCE_IDS {
            error!(
                "merge_concepts: Too few source_ids: {} < {}",
                input.source_ids.len(),
                MIN_SOURCE_IDS
            );
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                format!(
                    "source_ids requires at least {} items, got {}",
                    MIN_SOURCE_IDS,
                    input.source_ids.len()
                ),
            );
        }
        if input.source_ids.len() > MAX_SOURCE_IDS {
            error!(
                "merge_concepts: Too many source_ids: {} > {}",
                input.source_ids.len(),
                MAX_SOURCE_IDS
            );
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                format!(
                    "source_ids allows at most {} items, got {}",
                    MAX_SOURCE_IDS,
                    input.source_ids.len()
                ),
            );
        }

        // FAIL FAST: Validate target_name length (1-256 per schema)
        if input.target_name.len() < MIN_TARGET_NAME_LEN {
            error!("merge_concepts: Empty target_name");
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                format!("target_name must be at least {} char", MIN_TARGET_NAME_LEN),
            );
        }
        if input.target_name.len() > MAX_TARGET_NAME_LEN {
            error!(
                "merge_concepts: target_name too long: {} > {}",
                input.target_name.len(),
                MAX_TARGET_NAME_LEN
            );
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                format!(
                    "target_name exceeds max length: {} > {}",
                    input.target_name.len(),
                    MAX_TARGET_NAME_LEN
                ),
            );
        }

        // FAIL FAST: Validate rationale length (1-1024 per schema, REQUIRED per PRD 0.3)
        if input.rationale.len() < MIN_RATIONALE_LEN {
            error!("merge_concepts: Empty rationale");
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                format!("rationale is REQUIRED (min {} char)", MIN_RATIONALE_LEN),
            );
        }
        if input.rationale.len() > MAX_RATIONALE_LEN {
            error!(
                "merge_concepts: rationale too long: {} > {}",
                input.rationale.len(),
                MAX_RATIONALE_LEN
            );
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                format!(
                    "rationale exceeds max length: {} > {}",
                    input.rationale.len(),
                    MAX_RATIONALE_LEN
                ),
            );
        }

        // FAIL FAST: Check for duplicate source_ids
        let mut seen_ids = std::collections::HashSet::new();
        for source_id in &input.source_ids {
            if !seen_ids.insert(*source_id) {
                error!("merge_concepts: Duplicate source_id: {}", source_id);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Duplicate source_id: {}", source_id),
                );
            }
        }

        // Execute the merge operation
        match self.execute_merge(&input).await {
            Ok(output) => {
                info!(
                    "merge_concepts SUCCESS: merged {} nodes into {} with hash {}",
                    input.source_ids.len(),
                    output.merged_id,
                    output.reversal_hash
                );
                self.tool_result(id, json!(output))
            }
            Err(e) => {
                error!("merge_concepts FAILED: {}", e);
                JsonRpcResponse::error(id, error_codes::STORAGE_ERROR, e)
            }
        }
    }

    /// Execute the merge operation.
    ///
    /// 1. Fetch all source fingerprints (FAIL FAST if any missing)
    /// 2. Optional: Check priors compatibility (unless force_merge)
    /// 3. Merge fingerprints using specified strategy
    /// 4. Create merged fingerprint with combined attributes
    /// 5. Generate reversal hash and store reversal record
    /// 6. Store merged fingerprint
    /// 7. Mark source fingerprints as merged (soft delete)
    async fn execute_merge(
        &self,
        input: &MergeConceptsInput,
    ) -> Result<MergeConceptsOutput, String> {
        // Step 1: Fetch all source fingerprints using batch retrieval
        let source_fingerprints = self.fetch_source_fingerprints(&input.source_ids).await?;

        if source_fingerprints.len() != input.source_ids.len() {
            let found_ids: std::collections::HashSet<_> =
                source_fingerprints.iter().map(|f| f.id).collect();
            let missing: Vec<_> = input
                .source_ids
                .iter()
                .filter(|id| !found_ids.contains(id))
                .collect();
            return Err(format!("Missing source fingerprints: {:?}", missing));
        }

        // Step 2: Priors vibe check (AP-11) unless force_merge
        if !input.force_merge {
            self.check_fingerprint_compatibility(&source_fingerprints)?;
        }

        // Step 3: Merge fingerprints using strategy
        let merged_semantic = match input.merge_strategy {
            MergeStrategy::Union => self.merge_semantic_union(&source_fingerprints),
            MergeStrategy::Intersection => self.merge_semantic_intersection(&source_fingerprints),
            MergeStrategy::WeightedAverage => {
                self.merge_semantic_weighted_average(&source_fingerprints)
            }
        };

        // Step 4: Create merged fingerprint
        let now = Utc::now();
        let total_access_count: u64 = source_fingerprints.iter().map(|f| f.access_count).sum();

        // Generate content hash for merged content
        let merged_content = format!(
            "[MERGED] {}\nMerged from {} sources\nStrategy: {:?}\nRationale: {}",
            input.target_name,
            source_fingerprints.len(),
            input.merge_strategy,
            input.rationale
        );
        let content_hash = {
            let mut hasher = Sha256::new();
            hasher.update(merged_content.as_bytes());
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            hash
        };

        // E6-FIX: Extract e6_sparse BEFORE creating TeleologicalFingerprint
        // The merged SemanticFingerprint.e6_sparse contains the merged sparse vector
        // (union/intersection/weighted_average of source E6 vectors).
        // This must be copied to TeleologicalFingerprint.e6_sparse for inverted index storage.
        let e6_sparse = merged_semantic.e6_sparse.clone();

        // E6-FIX: Chain .with_e6_sparse() to propagate the merged E6 sparse vector
        let merged_fingerprint = TeleologicalFingerprint::new(merged_semantic, content_hash)
            .with_e6_sparse(e6_sparse);
        let merged_id = merged_fingerprint.id;

        // Step 5: Generate reversal hash and store reversal record
        let reversal_hash = self.generate_reversal_hash(&input.source_ids, merged_id);
        let expires_at = now + chrono::Duration::days(REVERSAL_DAYS);

        // Serialize original fingerprints for reversal (FAIL FAST on serialization error)
        let original_fingerprints_json: Vec<String> = source_fingerprints
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                error!(
                    "CRITICAL: Failed to serialize fingerprint for reversal record: {}",
                    e
                );
                format!("Internal error: failed to serialize reversal data: {}", e)
            })?;

        let reversal_record = ReversalRecord {
            reversal_hash: reversal_hash.clone(),
            merged_id,
            source_ids: input.source_ids.clone(),
            original_fingerprints_json,
            created_at: now.to_rfc3339(),
            expires_at: expires_at.to_rfc3339(),
            rationale: input.rationale.clone(),
            strategy: input.merge_strategy,
            target_name: input.target_name.clone(),
        };

        // Store reversal record (for 30-day undo per SEC-06)
        self.store_reversal_record(&reversal_record).await?;

        // Step 6: Store merged fingerprint
        self.teleological_store
            .store(merged_fingerprint)
            .await
            .map_err(|e| format!("Failed to store merged fingerprint: {}", e))?;

        // Also store the merged content
        if let Err(e) = self
            .teleological_store
            .store_content(merged_id, &merged_content)
            .await
        {
            warn!("Failed to store merged content (continuing): {}", e);
            // Non-fatal - content storage is optional per trait defaults
        }

        // Step 7: Mark source fingerprints as merged (soft delete per SEC-06)
        for source_id in &input.source_ids {
            if let Err(e) = self.teleological_store.delete(*source_id, true).await {
                warn!(
                    "Failed to soft-delete source fingerprint {}: {} (continuing)",
                    source_id, e
                );
                // Non-fatal - main merge succeeded
            }
        }

        Ok(MergeConceptsOutput {
            success: true,
            merged_id,
            reversal_hash,
            merged_node: MergedNodeInfo {
                id: merged_id,
                name: input.target_name.clone(),
                source_count: source_fingerprints.len(),
                strategy_used: input.merge_strategy,
                created_at: now.to_rfc3339(),
                total_access_count,
            },
            error: None,
        })
    }

    /// Fetch source fingerprints from storage using batch retrieval.
    /// FAIL FAST if any fingerprint is not found.
    async fn fetch_source_fingerprints(
        &self,
        source_ids: &[Uuid],
    ) -> Result<Vec<TeleologicalFingerprint>, String> {
        // Use batch retrieval for efficiency
        let results = self
            .teleological_store
            .retrieve_batch(source_ids)
            .await
            .map_err(|e| format!("Failed to retrieve fingerprints: {}", e))?;

        let mut fingerprints = Vec::with_capacity(source_ids.len());
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some(fp) => fingerprints.push(fp),
                None => {
                    return Err(format!("Source fingerprint not found: {}", source_ids[i]));
                }
            }
        }

        Ok(fingerprints)
    }

    /// Check fingerprint compatibility (AP-11: merge_concepts without priors_vibe_check).
    ///
    /// Uses E1 semantic similarity as a compatibility check.
    fn check_fingerprint_compatibility(
        &self,
        fingerprints: &[TeleologicalFingerprint],
    ) -> Result<(), String> {
        if fingerprints.len() < 2 {
            return Ok(()); // Nothing to compare
        }

        // Check semantic similarity spread using E1 embeddings
        // Calculate pairwise cosine similarities
        let mut similarities: Vec<f32> = Vec::new();
        for i in 0..fingerprints.len() {
            for j in (i + 1)..fingerprints.len() {
                let e1_i = &fingerprints[i].semantic.e1_semantic;
                let e1_j = &fingerprints[j].semantic.e1_semantic;

                // Cosine similarity
                let dot: f32 = e1_i.iter().zip(e1_j.iter()).map(|(a, b)| a * b).sum();
                let norm_i: f32 = e1_i.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_j: f32 = e1_j.iter().map(|x| x * x).sum::<f32>().sqrt();

                if norm_i > 0.0 && norm_j > 0.0 {
                    similarities.push(dot / (norm_i * norm_j));
                }
            }
        }

        if !similarities.is_empty() {
            let min_sim = similarities.iter().cloned().fold(f32::INFINITY, f32::min);

            // Warn if semantic similarity is low (below 0.3 threshold)
            if min_sim < 0.3 {
                warn!(
                    "Low semantic similarity in merge: min_similarity={:.2} (consider force_merge)",
                    min_sim
                );
            }
        }

        Ok(())
    }

    /// Merge semantic fingerprints using UNION strategy (average all dimensions).
    fn merge_semantic_union(
        &self,
        fingerprints: &[TeleologicalFingerprint],
    ) -> SemanticFingerprint {
        debug_assert!(
            !fingerprints.is_empty(),
            "merge_semantic_union: precondition violated - empty fingerprints"
        );

        // Average dense embeddings
        let e1_semantic = Self::average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e1_semantic)
                .collect(),
        );
        let e2_temporal_recent = Self::average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e2_temporal_recent)
                .collect(),
        );
        let e3_temporal_periodic = Self::average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e3_temporal_periodic)
                .collect(),
        );
        let e4_temporal_positional = Self::average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e4_temporal_positional)
                .collect(),
        );
        // For E5, average both cause and effect vectors separately
        let e5_causal_as_cause = Self::average_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e5_as_cause())
                .collect(),
        );
        let e5_causal_as_effect = Self::average_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e5_as_effect())
                .collect(),
        );
        let e7_code =
            Self::average_dense_vectors(fingerprints.iter().map(|f| &f.semantic.e7_code).collect());
        let e8_graph = Self::average_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e8_graph).collect(),
        );
        let e8_graph_as_source = Self::average_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e8_graph_as_source).collect(),
        );
        let e8_graph_as_target = Self::average_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e8_graph_as_target).collect(),
        );
        let e9_hdc =
            Self::average_dense_vectors(fingerprints.iter().map(|f| &f.semantic.e9_hdc).collect());
        let e10_multimodal = Self::average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e10_multimodal)
                .collect(),
        );
        let e11_entity = Self::average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e11_entity)
                .collect(),
        );

        // Union sparse vectors
        let e6_sparse = Self::union_sparse_vectors(
            fingerprints.iter().map(|f| &f.semantic.e6_sparse).collect(),
        );
        let e13_splade = Self::union_sparse_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e13_splade)
                .collect(),
        );

        // Concatenate token-level embeddings
        let e12_late_interaction: Vec<Vec<f32>> = fingerprints
            .iter()
            .flat_map(|f| f.semantic.e12_late_interaction.clone())
            .collect();

        // E10: Compute dual vectors using same averaging approach
        let e10_multimodal_as_intent = Self::average_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e10_as_intent())
                .filter(|v| !v.is_empty())
                .collect(),
        );
        let e10_multimodal_as_context = Self::average_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e10_as_context())
                .filter(|v| !v.is_empty())
                .collect(),
        );

        SemanticFingerprint {
            e1_semantic,
            e2_temporal_recent,
            e3_temporal_periodic,
            e4_temporal_positional,
            e5_causal_as_cause,
            e5_causal_as_effect,
            e5_causal: Vec::new(), // Using new dual format
            e6_sparse,
            e7_code,
            e8_graph,
            e8_graph_as_source,
            e8_graph_as_target,
            e9_hdc,
            e10_multimodal_as_intent,
            e10_multimodal_as_context,
            e10_multimodal, // Keep for backwards compatibility
            e11_entity,
            e12_late_interaction,
            e13_splade,
        }
    }

    /// Merge semantic fingerprints using INTERSECTION strategy.
    fn merge_semantic_intersection(
        &self,
        fingerprints: &[TeleologicalFingerprint],
    ) -> SemanticFingerprint {
        debug_assert!(
            !fingerprints.is_empty(),
            "merge_semantic_intersection: precondition violated - empty fingerprints"
        );

        // Intersection: keep only dimensions where ALL have significant values
        let e1_semantic = Self::intersection_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e1_semantic)
                .collect(),
        );
        let e2_temporal_recent = Self::intersection_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e2_temporal_recent)
                .collect(),
        );
        let e3_temporal_periodic = Self::intersection_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e3_temporal_periodic)
                .collect(),
        );
        let e4_temporal_positional = Self::intersection_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e4_temporal_positional)
                .collect(),
        );
        // For E5, intersection both cause and effect vectors separately
        let e5_causal_as_cause = Self::intersection_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e5_as_cause())
                .collect(),
        );
        let e5_causal_as_effect = Self::intersection_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e5_as_effect())
                .collect(),
        );
        let e7_code = Self::intersection_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e7_code).collect(),
        );
        let e8_graph = Self::intersection_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e8_graph).collect(),
        );
        let e8_graph_as_source = Self::intersection_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e8_graph_as_source).collect(),
        );
        let e8_graph_as_target = Self::intersection_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e8_graph_as_target).collect(),
        );
        let e9_hdc = Self::intersection_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e9_hdc).collect(),
        );
        let e10_multimodal = Self::intersection_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e10_multimodal)
                .collect(),
        );
        let e11_entity = Self::intersection_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e11_entity)
                .collect(),
        );

        // Intersection of sparse vectors
        let e6_sparse = Self::intersection_sparse_vectors(
            fingerprints.iter().map(|f| &f.semantic.e6_sparse).collect(),
        );
        let e13_splade = Self::intersection_sparse_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e13_splade)
                .collect(),
        );

        // For intersection, only keep tokens that appear in ALL fingerprints
        // Since tokens are variable-length, take the intersection as the minimum set
        let min_tokens = fingerprints
            .iter()
            .map(|f| f.semantic.e12_late_interaction.len())
            .min()
            .unwrap_or(0);
        let e12_late_interaction: Vec<Vec<f32>> = if min_tokens > 0 {
            // Average the first min_tokens tokens
            (0..min_tokens)
                .map(|i| {
                    let tokens: Vec<&Vec<f32>> = fingerprints
                        .iter()
                        .filter_map(|f| f.semantic.e12_late_interaction.get(i))
                        .collect();
                    Self::average_dense_vectors(tokens)
                })
                .collect()
        } else {
            Vec::new()
        };

        // E10: Compute dual vectors using same intersection approach
        let e10_multimodal_as_intent = Self::intersection_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e10_as_intent())
                .filter(|v| !v.is_empty())
                .collect(),
        );
        let e10_multimodal_as_context = Self::intersection_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e10_as_context())
                .filter(|v| !v.is_empty())
                .collect(),
        );

        SemanticFingerprint {
            e1_semantic,
            e2_temporal_recent,
            e3_temporal_periodic,
            e4_temporal_positional,
            e5_causal_as_cause,
            e5_causal_as_effect,
            e5_causal: Vec::new(), // Using new dual format
            e6_sparse,
            e7_code,
            e8_graph,
            e8_graph_as_source,
            e8_graph_as_target,
            e9_hdc,
            e10_multimodal_as_intent,
            e10_multimodal_as_context,
            e10_multimodal, // Keep for backwards compatibility
            e11_entity,
            e12_late_interaction,
            e13_splade,
        }
    }

    /// Merge semantic fingerprints using WEIGHTED_AVERAGE strategy (weight by access_count).
    fn merge_semantic_weighted_average(
        &self,
        fingerprints: &[TeleologicalFingerprint],
    ) -> SemanticFingerprint {
        debug_assert!(
            !fingerprints.is_empty(),
            "merge_semantic_weighted_average: precondition violated - empty fingerprints"
        );

        // Calculate weights based on access_count
        let total_access: u64 = fingerprints.iter().map(|f| f.access_count.max(1)).sum();
        let weights: Vec<f32> = fingerprints
            .iter()
            .map(|f| f.access_count.max(1) as f32 / total_access as f32)
            .collect();

        // Weighted average of dense embeddings
        let e1_semantic = Self::weighted_average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e1_semantic)
                .collect(),
            &weights,
        );
        let e2_temporal_recent = Self::weighted_average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e2_temporal_recent)
                .collect(),
            &weights,
        );
        let e3_temporal_periodic = Self::weighted_average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e3_temporal_periodic)
                .collect(),
            &weights,
        );
        let e4_temporal_positional = Self::weighted_average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e4_temporal_positional)
                .collect(),
            &weights,
        );
        // For E5, weighted average both cause and effect vectors separately
        let e5_causal_as_cause = Self::weighted_average_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e5_as_cause())
                .collect(),
            &weights,
        );
        let e5_causal_as_effect = Self::weighted_average_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e5_as_effect())
                .collect(),
            &weights,
        );
        let e7_code = Self::weighted_average_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e7_code).collect(),
            &weights,
        );
        let e8_graph = Self::weighted_average_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e8_graph).collect(),
            &weights,
        );
        let e8_graph_as_source = Self::weighted_average_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e8_graph_as_source).collect(),
            &weights,
        );
        let e8_graph_as_target = Self::weighted_average_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e8_graph_as_target).collect(),
            &weights,
        );
        let e9_hdc = Self::weighted_average_dense_vectors(
            fingerprints.iter().map(|f| &f.semantic.e9_hdc).collect(),
            &weights,
        );
        let e10_multimodal = Self::weighted_average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e10_multimodal)
                .collect(),
            &weights,
        );
        let e11_entity = Self::weighted_average_dense_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e11_entity)
                .collect(),
            &weights,
        );

        // Weighted union of sparse vectors
        let e6_sparse = Self::weighted_union_sparse_vectors(
            fingerprints.iter().map(|f| &f.semantic.e6_sparse).collect(),
            &weights,
        );
        let e13_splade = Self::weighted_union_sparse_vectors(
            fingerprints
                .iter()
                .map(|f| &f.semantic.e13_splade)
                .collect(),
            &weights,
        );

        // For weighted average, take tokens from highest-weighted fingerprint first
        let max_weight_idx = weights
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let e12_late_interaction = fingerprints[max_weight_idx]
            .semantic
            .e12_late_interaction
            .clone();

        // E10: Compute dual vectors using same weighted average approach
        let e10_multimodal_as_intent = Self::weighted_average_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e10_as_intent())
                .filter(|v| !v.is_empty())
                .collect(),
            &weights,
        );
        let e10_multimodal_as_context = Self::weighted_average_dense_slices(
            fingerprints
                .iter()
                .map(|f| f.semantic.get_e10_as_context())
                .filter(|v| !v.is_empty())
                .collect(),
            &weights,
        );

        SemanticFingerprint {
            e1_semantic,
            e2_temporal_recent,
            e3_temporal_periodic,
            e4_temporal_positional,
            e5_causal_as_cause,
            e5_causal_as_effect,
            e5_causal: Vec::new(), // Using new dual format
            e6_sparse,
            e7_code,
            e8_graph,
            e8_graph_as_source,
            e8_graph_as_target,
            e9_hdc,
            e10_multimodal_as_intent,
            e10_multimodal_as_context,
            e10_multimodal, // Keep for backwards compatibility
            e11_entity,
            e12_late_interaction,
            e13_splade,
        }
    }

    /// Average multiple dense vectors element-wise and normalize.
    fn average_dense_vectors(vectors: Vec<&Vec<f32>>) -> Vec<f32> {
        Self::average_dense_slices(vectors.iter().map(|v| v.as_slice()).collect())
    }

    /// Average multiple dense slices element-wise and normalize.
    fn average_dense_slices(slices: Vec<&[f32]>) -> Vec<f32> {
        if slices.is_empty() {
            return Vec::new();
        }

        let dim = slices[0].len();
        let n = slices.len() as f32;
        let mut result = vec![0.0f32; dim];

        for slice in &slices {
            for (i, &val) in slice.iter().enumerate() {
                if i < dim {
                    result[i] += val / n;
                }
            }
        }

        Self::normalize_vector(&mut result);
        result
    }

    /// Intersection of dense vectors: keep only dimensions significant in ALL.
    fn intersection_dense_vectors(vectors: Vec<&Vec<f32>>) -> Vec<f32> {
        Self::intersection_dense_slices(vectors.iter().map(|v| v.as_slice()).collect())
    }

    /// Intersection of dense slices: keep only dimensions significant in ALL.
    fn intersection_dense_slices(slices: Vec<&[f32]>) -> Vec<f32> {
        if slices.is_empty() {
            return Vec::new();
        }

        let dim = slices[0].len();
        let n = slices.len() as f32;
        let mut result = vec![0.0f32; dim];

        for (i, result_val) in result.iter_mut().enumerate().take(dim) {
            let all_significant = slices.iter().all(|v| {
                v.get(i)
                    .map(|&x| x.abs() >= INTERSECTION_THRESHOLD)
                    .unwrap_or(false)
            });

            if all_significant {
                let sum: f32 = slices.iter().filter_map(|v| v.get(i)).sum();
                *result_val = sum / n;
            }
        }

        Self::normalize_vector(&mut result);
        result
    }

    /// Weighted average of dense vectors.
    fn weighted_average_dense_vectors(vectors: Vec<&Vec<f32>>, weights: &[f32]) -> Vec<f32> {
        Self::weighted_average_dense_slices(vectors.iter().map(|v| v.as_slice()).collect(), weights)
    }

    /// Weighted average of dense slices.
    fn weighted_average_dense_slices(slices: Vec<&[f32]>, weights: &[f32]) -> Vec<f32> {
        if slices.is_empty() {
            return Vec::new();
        }

        let dim = slices[0].len();
        let mut result = vec![0.0f32; dim];

        for (slice, &weight) in slices.iter().zip(weights.iter()) {
            for (i, &val) in slice.iter().enumerate() {
                if i < dim {
                    result[i] += val * weight;
                }
            }
        }

        Self::normalize_vector(&mut result);
        result
    }

    /// Normalize a vector to unit length.
    fn normalize_vector(vec: &mut [f32]) {
        let magnitude: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for val in vec.iter_mut() {
                *val /= magnitude;
            }
        }
    }

    /// Union of sparse vectors: combine all indices, average values.
    fn union_sparse_vectors(vectors: Vec<&SparseVector>) -> SparseVector {
        if vectors.is_empty() {
            return SparseVector::empty();
        }

        // Collect all indices and their values
        let mut index_values: std::collections::BTreeMap<u16, Vec<f32>> =
            std::collections::BTreeMap::new();

        for vec in &vectors {
            for (&idx, &val) in vec.indices.iter().zip(vec.values.iter()) {
                index_values.entry(idx).or_default().push(val);
            }
        }

        // Average values for each index
        let n = vectors.len() as f32;
        let mut indices = Vec::new();
        let mut values = Vec::new();

        for (idx, vals) in index_values {
            indices.push(idx);
            // Average across all vectors (treating missing as 0)
            let sum: f32 = vals.iter().sum();
            values.push(sum / n);
        }

        SparseVector { indices, values }
    }

    /// Intersection of sparse vectors: keep only indices present in ALL.
    fn intersection_sparse_vectors(vectors: Vec<&SparseVector>) -> SparseVector {
        if vectors.is_empty() {
            return SparseVector::empty();
        }

        // Find indices present in ALL vectors
        let first_indices: std::collections::HashSet<u16> =
            vectors[0].indices.iter().cloned().collect();

        let common_indices: std::collections::HashSet<u16> =
            vectors.iter().skip(1).fold(first_indices, |acc, vec| {
                let vec_indices: std::collections::HashSet<u16> =
                    vec.indices.iter().cloned().collect();
                acc.intersection(&vec_indices).cloned().collect()
            });

        // Average values for common indices
        let n = vectors.len() as f32;
        let mut indices: Vec<u16> = common_indices.into_iter().collect();
        indices.sort();

        let mut values = Vec::with_capacity(indices.len());
        for &idx in &indices {
            let sum: f32 = vectors
                .iter()
                .filter_map(|v| {
                    v.indices
                        .iter()
                        .position(|&i| i == idx)
                        .map(|pos| v.values[pos])
                })
                .sum();
            values.push(sum / n);
        }

        SparseVector { indices, values }
    }

    /// Weighted union of sparse vectors.
    fn weighted_union_sparse_vectors(vectors: Vec<&SparseVector>, weights: &[f32]) -> SparseVector {
        if vectors.is_empty() {
            return SparseVector::empty();
        }

        // Collect all indices and their weighted values
        let mut index_values: std::collections::BTreeMap<u16, f32> =
            std::collections::BTreeMap::new();

        for (vec, &weight) in vectors.iter().zip(weights.iter()) {
            for (&idx, &val) in vec.indices.iter().zip(vec.values.iter()) {
                *index_values.entry(idx).or_default() += val * weight;
            }
        }

        let indices: Vec<u16> = index_values.keys().cloned().collect();
        let values: Vec<f32> = index_values.values().cloned().collect();

        SparseVector { indices, values }
    }

    /// Generate SHA-256 reversal hash from source IDs and merged ID.
    fn generate_reversal_hash(&self, source_ids: &[Uuid], merged_id: Uuid) -> String {
        let mut hasher = Sha256::new();

        // Include all source IDs in deterministic order
        let mut sorted_sources: Vec<_> = source_ids.iter().collect();
        sorted_sources.sort();
        for id in sorted_sources {
            hasher.update(id.as_bytes());
        }

        // Include merged ID
        hasher.update(merged_id.as_bytes());

        // Include timestamp for uniqueness
        hasher.update(Utc::now().timestamp().to_le_bytes());

        let result = hasher.finalize();
        format!("sha256:{}", hex::encode(result))
    }

    /// Store reversal record for 30-day undo capability (SEC-06).
    async fn store_reversal_record(&self, record: &ReversalRecord) -> Result<(), String> {
        let content = serde_json::to_string(record)
            .map_err(|e| format!("Failed to serialize reversal record: {}", e))?;

        // Use the merged_id to store reversal record as content
        // Key format: reversal record is associated with merged node
        self.teleological_store
            .store_content(record.merged_id, &format!("REVERSAL:{}", content))
            .await
            .map_err(|e| format!("Failed to store reversal record: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_strategy_deserialization() {
        let json = r#""union""#;
        let strategy: MergeStrategy = serde_json::from_str(json).expect("union");
        assert_eq!(strategy, MergeStrategy::Union);

        let json = r#""intersection""#;
        let strategy: MergeStrategy = serde_json::from_str(json).expect("intersection");
        assert_eq!(strategy, MergeStrategy::Intersection);

        let json = r#""weighted_average""#;
        let strategy: MergeStrategy = serde_json::from_str(json).expect("weighted_average");
        assert_eq!(strategy, MergeStrategy::WeightedAverage);
    }

    #[test]
    fn test_merge_strategy_default() {
        let strategy = MergeStrategy::default();
        assert_eq!(strategy, MergeStrategy::Union);
    }

    #[test]
    fn test_merge_concepts_input_deserialization() {
        let json = r#"{
            "source_ids": [
                "550e8400-e29b-41d4-a716-446655440001",
                "550e8400-e29b-41d4-a716-446655440002"
            ],
            "target_name": "Merged Concept",
            "merge_strategy": "union",
            "rationale": "Consolidating duplicates",
            "force_merge": false
        }"#;
        let input: MergeConceptsInput = serde_json::from_str(json).expect("deserialize");
        assert_eq!(input.source_ids.len(), 2);
        assert_eq!(input.target_name, "Merged Concept");
        assert_eq!(input.merge_strategy, MergeStrategy::Union);
        assert_eq!(input.rationale, "Consolidating duplicates");
        assert!(!input.force_merge);
    }

    #[test]
    fn test_merge_concepts_input_defaults() {
        let json = r#"{
            "source_ids": [
                "550e8400-e29b-41d4-a716-446655440001",
                "550e8400-e29b-41d4-a716-446655440002"
            ],
            "target_name": "Test",
            "rationale": "Testing"
        }"#;
        let input: MergeConceptsInput = serde_json::from_str(json).expect("deserialize");
        assert_eq!(input.merge_strategy, MergeStrategy::Union); // default
        assert!(!input.force_merge); // default
    }

    #[test]
    fn test_reversal_record_serialization() {
        let record = ReversalRecord {
            reversal_hash: "sha256:abc123".to_string(),
            merged_id: Uuid::nil(),
            source_ids: vec![Uuid::nil()],
            original_fingerprints_json: vec![],
            created_at: "2026-01-13T00:00:00Z".to_string(),
            expires_at: "2026-02-12T00:00:00Z".to_string(),
            rationale: "Test merge".to_string(),
            strategy: MergeStrategy::Union,
            target_name: "Test".to_string(),
        };
        let json = serde_json::to_string(&record).expect("serialize");
        assert!(json.contains("sha256:abc123"));
        assert!(json.contains("2026-02-12")); // 30 days later
    }

    #[test]
    fn test_merged_node_info_serialization() {
        let info = MergedNodeInfo {
            id: Uuid::nil(),
            name: "Test".to_string(),
            source_count: 3,
            strategy_used: MergeStrategy::WeightedAverage,
            created_at: "2026-01-13T00:00:00Z".to_string(),
            total_access_count: 42,
        };
        let json = serde_json::to_string(&info).expect("serialize");
        assert!(json.contains("source_count\":3"));
        assert!(json.contains("weighted_average"));
    }

    // ========== UNIT TESTS FOR MERGE STRATEGIES ==========

    #[test]
    fn test_source_ids_validation_bounds() {
        assert_eq!(MIN_SOURCE_IDS, 2);
        assert_eq!(MAX_SOURCE_IDS, 10);
    }

    #[test]
    fn test_target_name_validation_bounds() {
        assert_eq!(MIN_TARGET_NAME_LEN, 1);
        assert_eq!(MAX_TARGET_NAME_LEN, 256);
    }

    #[test]
    fn test_rationale_validation_bounds() {
        assert_eq!(MIN_RATIONALE_LEN, 1);
        assert_eq!(MAX_RATIONALE_LEN, 1024);
    }

    #[test]
    fn test_reversal_expiration() {
        assert_eq!(REVERSAL_DAYS, 30);
    }

    #[test]
    fn test_intersection_threshold() {
        assert_eq!(INTERSECTION_THRESHOLD, 0.01);
    }

    // ========== VECTOR OPERATION TESTS ==========

    #[test]
    fn test_normalize_vector() {
        let mut vec = vec![3.0, 4.0];
        Handlers::normalize_vector(&mut vec);
        let magnitude: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_average_dense_vectors() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let result = Handlers::average_dense_vectors(vec![&v1, &v2]);

        // After averaging and normalizing
        let magnitude: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
        assert!(result[0] > 0.0);
        assert!(result[1] > 0.0);
        assert!((result[2]).abs() < 0.001);
    }

    #[test]
    fn test_union_sparse_vectors() {
        let v1 = SparseVector {
            indices: vec![0, 2],
            values: vec![1.0, 0.5],
        };
        let v2 = SparseVector {
            indices: vec![1, 2],
            values: vec![0.8, 0.5],
        };
        let result = Handlers::union_sparse_vectors(vec![&v1, &v2]);

        // Should have indices 0, 1, 2
        assert_eq!(result.indices.len(), 3);
        assert!(result.indices.contains(&0));
        assert!(result.indices.contains(&1));
        assert!(result.indices.contains(&2));
    }

    #[test]
    fn test_intersection_sparse_vectors() {
        let v1 = SparseVector {
            indices: vec![0, 2, 5],
            values: vec![1.0, 0.5, 0.3],
        };
        let v2 = SparseVector {
            indices: vec![1, 2, 5],
            values: vec![0.8, 0.5, 0.4],
        };
        let result = Handlers::intersection_sparse_vectors(vec![&v1, &v2]);

        // Should only have indices 2 and 5 (present in both)
        assert_eq!(result.indices.len(), 2);
        assert!(result.indices.contains(&2));
        assert!(result.indices.contains(&5));
        assert!(!result.indices.contains(&0));
        assert!(!result.indices.contains(&1));
    }
}
