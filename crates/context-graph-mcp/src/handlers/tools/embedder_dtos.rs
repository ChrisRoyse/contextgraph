//! DTOs for embedder-first search tools.
//!
//! Per Constitution v6.3, these DTOs support searching using any of the
//! 13 embedders as the primary perspective.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Embedder identifier enum matching the 13 embedders in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmbedderId {
    E1,  // V_meaning - Semantic similarity (foundation)
    E2,  // V_freshness - Recency
    E3,  // V_periodicity - Time-of-day patterns
    E4,  // V_ordering - Sequence
    E5,  // V_causality - Causal chains
    E6,  // V_selectivity - Keyword matches (sparse)
    E7,  // V_correctness - Code patterns
    E8,  // V_connectivity - Graph structure
    E9,  // V_robustness - Noise-robust structure
    E10, // V_multimodality - Intent alignment
    E11, // V_factuality - Entity knowledge (KEPLER)
    E12, // V_precision - Exact phrase matches
    E13, // V_keyword_precision - Term expansion (sparse)
}

impl EmbedderId {
    /// Convert from string to EmbedderId.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "E1" => Some(EmbedderId::E1),
            "E2" => Some(EmbedderId::E2),
            "E3" => Some(EmbedderId::E3),
            "E4" => Some(EmbedderId::E4),
            "E5" => Some(EmbedderId::E5),
            "E6" => Some(EmbedderId::E6),
            "E7" => Some(EmbedderId::E7),
            "E8" => Some(EmbedderId::E8),
            "E9" => Some(EmbedderId::E9),
            "E10" => Some(EmbedderId::E10),
            "E11" => Some(EmbedderId::E11),
            "E12" => Some(EmbedderId::E12),
            "E13" => Some(EmbedderId::E13),
            _ => None,
        }
    }

    /// Get the index in the TeleologicalArray (0-12).
    pub fn to_index(&self) -> usize {
        match self {
            EmbedderId::E1 => 0,
            EmbedderId::E2 => 1,
            EmbedderId::E3 => 2,
            EmbedderId::E4 => 3,
            EmbedderId::E5 => 4,
            EmbedderId::E6 => 5,
            EmbedderId::E7 => 6,
            EmbedderId::E8 => 7,
            EmbedderId::E9 => 8,
            EmbedderId::E10 => 9,
            EmbedderId::E11 => 10,
            EmbedderId::E12 => 11,
            EmbedderId::E13 => 12,
        }
    }

    /// Get the human-readable name for this embedder.
    pub fn name(&self) -> &'static str {
        match self {
            EmbedderId::E1 => "V_meaning (Semantic)",
            EmbedderId::E2 => "V_freshness (Recency)",
            EmbedderId::E3 => "V_periodicity (Periodic)",
            EmbedderId::E4 => "V_ordering (Sequence)",
            EmbedderId::E5 => "V_causality (Causal)",
            EmbedderId::E6 => "V_selectivity (Keyword)",
            EmbedderId::E7 => "V_correctness (Code)",
            EmbedderId::E8 => "V_connectivity (Graph)",
            EmbedderId::E9 => "V_robustness (HDC)",
            EmbedderId::E10 => "V_multimodality (Intent)",
            EmbedderId::E11 => "V_factuality (Entity)",
            EmbedderId::E12 => "V_precision (Phrase)",
            EmbedderId::E13 => "V_keyword_precision (Expansion)",
        }
    }

    /// Get what this embedder finds that others miss.
    pub fn finds(&self) -> &'static str {
        match self {
            EmbedderId::E1 => "Dense semantic similarity",
            EmbedderId::E2 => "Recent memories first",
            EmbedderId::E3 => "Time-of-day patterns",
            EmbedderId::E4 => "Conversation sequence (before/after)",
            EmbedderId::E5 => "Cause-effect relationships",
            EmbedderId::E6 => "Exact keyword matches",
            EmbedderId::E7 => "Code patterns, function signatures",
            EmbedderId::E8 => "Structural relationships (imports, deps)",
            EmbedderId::E9 => "Noise-robust structure",
            EmbedderId::E10 => "Goal alignment (different words, same intent)",
            EmbedderId::E11 => "Entity knowledge via KEPLER",
            EmbedderId::E12 => "Exact phrase matches (token-level)",
            EmbedderId::E13 => "Term expansion (synonyms)",
        }
    }

    /// Get the dimension of this embedder (or "sparse" for sparse embedders).
    pub fn dimension(&self) -> &'static str {
        match self {
            EmbedderId::E1 => "1024",
            EmbedderId::E2 => "512",
            EmbedderId::E3 => "512",
            EmbedderId::E4 => "512",
            EmbedderId::E5 => "768",
            EmbedderId::E6 => "sparse (~30K)",
            EmbedderId::E7 => "1536",
            EmbedderId::E8 => "1024",
            EmbedderId::E9 => "1024",
            EmbedderId::E10 => "768",
            EmbedderId::E11 => "768",
            EmbedderId::E12 => "128/token",
            EmbedderId::E13 => "sparse (~30K)",
        }
    }

    /// Check if this embedder uses sparse representation.
    pub fn is_sparse(&self) -> bool {
        matches!(self, EmbedderId::E6 | EmbedderId::E13)
    }

    /// Check if this is a temporal embedder (E2-E4).
    pub fn is_temporal(&self) -> bool {
        matches!(self, EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4)
    }

    /// Check if this is a semantic embedder.
    pub fn is_semantic(&self) -> bool {
        matches!(
            self,
            EmbedderId::E1
                | EmbedderId::E5
                | EmbedderId::E6
                | EmbedderId::E7
                | EmbedderId::E10
                | EmbedderId::E12
                | EmbedderId::E13
        )
    }
}

// ============================================================================
// search_by_embedder DTOs
// ============================================================================

/// Request for search_by_embedder tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchByEmbedderRequest {
    /// Which embedder to use as primary (E1-E13).
    pub embedder: String,
    /// Search query.
    pub query: String,
    /// Maximum number of results (default: 10).
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Minimum similarity threshold (default: 0.0).
    #[serde(default)]
    pub min_similarity: f32,
    /// Include full content in results (default: false).
    #[serde(default)]
    pub include_content: bool,
    /// Include scores from all 13 embedders (default: false).
    #[serde(default)]
    pub include_all_scores: bool,
}

fn default_top_k() -> usize {
    10
}

impl SearchByEmbedderRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        if self.query.is_empty() {
            return Err("Query cannot be empty".to_string());
        }
        if EmbedderId::from_str(&self.embedder).is_none() {
            return Err(format!(
                "Invalid embedder '{}'. Must be E1-E13.",
                self.embedder
            ));
        }
        if self.top_k == 0 || self.top_k > 100 {
            return Err("topK must be between 1 and 100".to_string());
        }
        if self.min_similarity < 0.0 || self.min_similarity > 1.0 {
            return Err("minSimilarity must be between 0 and 1".to_string());
        }
        Ok(())
    }

    /// Get the parsed embedder ID.
    pub fn embedder_id(&self) -> Option<EmbedderId> {
        EmbedderId::from_str(&self.embedder)
    }
}

/// A single search result from search_by_embedder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderSearchResult {
    /// Memory ID.
    pub memory_id: Uuid,
    /// Similarity score in the primary embedder's space.
    pub similarity: f32,
    /// Content (if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Scores from all 13 embedders (if includeAllScores=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_scores: Option<AllEmbedderScores>,
}

/// Scores from all 13 embedders for a single memory.
#[derive(Debug, Clone, Serialize)]
pub struct AllEmbedderScores {
    pub e1: f32,
    pub e2: f32,
    pub e3: f32,
    pub e4: f32,
    pub e5: f32,
    pub e6: f32,
    pub e7: f32,
    pub e8: f32,
    pub e9: f32,
    pub e10: f32,
    pub e11: f32,
    pub e12: f32,
    pub e13: f32,
}

/// Response for search_by_embedder tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchByEmbedderResponse {
    /// The embedder used for search.
    pub embedder: String,
    /// What this embedder finds.
    pub embedder_finds: String,
    /// Search results.
    pub results: Vec<EmbedderSearchResult>,
    /// Total candidates searched.
    pub total_searched: usize,
    /// Search time in milliseconds.
    pub search_time_ms: u64,
}

// ============================================================================
// get_embedder_clusters DTOs
// ============================================================================

/// Request for get_embedder_clusters tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetEmbedderClustersRequest {
    /// Which embedder's clusters to explore (E1-E13).
    pub embedder: String,
    /// Minimum memories per cluster (default: 3).
    #[serde(default = "default_min_cluster_size")]
    pub min_cluster_size: usize,
    /// Maximum clusters to return (default: 10).
    #[serde(default = "default_top_clusters")]
    pub top_clusters: usize,
    /// Include sample memories from each cluster (default: true).
    #[serde(default = "default_true")]
    pub include_samples: bool,
    /// Number of samples per cluster (default: 3).
    #[serde(default = "default_samples_per_cluster")]
    pub samples_per_cluster: usize,
}

fn default_min_cluster_size() -> usize {
    3
}

fn default_top_clusters() -> usize {
    10
}

fn default_true() -> bool {
    true
}

fn default_samples_per_cluster() -> usize {
    3
}

impl GetEmbedderClustersRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        if EmbedderId::from_str(&self.embedder).is_none() {
            return Err(format!(
                "Invalid embedder '{}'. Must be E1-E13.",
                self.embedder
            ));
        }
        if self.min_cluster_size < 2 || self.min_cluster_size > 50 {
            return Err("minClusterSize must be between 2 and 50".to_string());
        }
        if self.top_clusters == 0 || self.top_clusters > 50 {
            return Err("topClusters must be between 1 and 50".to_string());
        }
        Ok(())
    }

    /// Get the parsed embedder ID.
    pub fn embedder_id(&self) -> Option<EmbedderId> {
        EmbedderId::from_str(&self.embedder)
    }
}

/// A single cluster from get_embedder_clusters.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderCluster {
    /// Cluster ID (0-indexed).
    pub cluster_id: usize,
    /// Number of memories in this cluster.
    pub size: usize,
    /// Sample memory IDs from this cluster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_ids: Option<Vec<Uuid>>,
    /// Sample content snippets (if include_samples=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_snippets: Option<Vec<String>>,
    /// Cluster centroid description (auto-generated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Response for get_embedder_clusters tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetEmbedderClustersResponse {
    /// The embedder whose clusters were explored.
    pub embedder: String,
    /// What this embedder sees.
    pub embedder_perspective: String,
    /// Clusters found.
    pub clusters: Vec<EmbedderCluster>,
    /// Total memories clustered.
    pub total_memories: usize,
    /// Total clusters found (before limiting to topClusters).
    pub total_clusters: usize,
    /// Time in milliseconds.
    pub time_ms: u64,
}

// ============================================================================
// compare_embedder_views DTOs
// ============================================================================

/// Request for compare_embedder_views tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareEmbedderViewsRequest {
    /// Search query.
    pub query: String,
    /// Which embedders to compare (2-5 embedders).
    pub embedders: Vec<String>,
    /// Number of top results per embedder (default: 5).
    #[serde(default = "default_compare_top_k")]
    pub top_k: usize,
    /// Include content in results (default: false).
    #[serde(default)]
    pub include_content: bool,
}

fn default_compare_top_k() -> usize {
    5
}

impl CompareEmbedderViewsRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        if self.query.is_empty() {
            return Err("Query cannot be empty".to_string());
        }
        if self.embedders.len() < 2 {
            return Err("Must compare at least 2 embedders".to_string());
        }
        if self.embedders.len() > 5 {
            return Err("Cannot compare more than 5 embedders".to_string());
        }
        for e in &self.embedders {
            if EmbedderId::from_str(e).is_none() {
                return Err(format!("Invalid embedder '{}'. Must be E1-E13.", e));
            }
        }
        if self.top_k == 0 || self.top_k > 20 {
            return Err("topK must be between 1 and 20".to_string());
        }
        Ok(())
    }

    /// Get the parsed embedder IDs.
    pub fn embedder_ids(&self) -> Vec<EmbedderId> {
        self.embedders
            .iter()
            .filter_map(|e| EmbedderId::from_str(e))
            .collect()
    }
}

/// Rankings from a single embedder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderRanking {
    /// Embedder ID (e.g., "E1").
    pub embedder: String,
    /// What this embedder finds.
    pub finds: String,
    /// Top results from this embedder.
    pub results: Vec<RankedMemory>,
}

/// A memory with its rank in a specific embedder's view.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedMemory {
    /// Memory ID.
    pub memory_id: Uuid,
    /// Rank (1-indexed) in this embedder's results.
    pub rank: usize,
    /// Similarity score.
    pub similarity: f32,
    /// Content snippet (if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Response for compare_embedder_views tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareEmbedderViewsResponse {
    /// Query that was compared.
    pub query: String,
    /// Rankings from each embedder.
    pub rankings: Vec<EmbedderRanking>,
    /// Memories found by all embedders.
    pub agreement: Vec<Uuid>,
    /// Memories found by only one embedder (unique finds).
    pub unique_finds: Vec<UniqueFind>,
    /// Agreement score (0-1): how much embedders agree.
    pub agreement_score: f32,
    /// Time in milliseconds.
    pub time_ms: u64,
}

/// A memory found by only one embedder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UniqueFind {
    /// Memory ID.
    pub memory_id: Uuid,
    /// Which embedder found it.
    pub found_by: String,
    /// What that embedder sees that others missed.
    pub why_unique: String,
}

// ============================================================================
// list_embedder_indexes DTOs
// ============================================================================

/// Request for list_embedder_indexes tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEmbedderIndexesRequest {
    /// Include detailed stats (default: true).
    #[serde(default = "default_true")]
    pub include_details: bool,
}

impl ListEmbedderIndexesRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Information about a single embedder's index.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderIndexInfo {
    /// Embedder ID (e.g., "E1").
    pub embedder: String,
    /// Human-readable name.
    pub name: String,
    /// What this embedder finds.
    pub finds: String,
    /// Vector dimension or "sparse".
    pub dimension: String,
    /// Index type (HNSW, Inverted, MaxSim).
    pub index_type: String,
    /// Number of vectors indexed.
    pub vector_count: usize,
    /// Approximate size in MB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_mb: Option<f32>,
    /// Whether the index is GPU-resident.
    /// None if cannot be verified at runtime.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_resident: Option<bool>,
    /// Topic weight for this embedder.
    pub topic_weight: f32,
    /// Category (Semantic, Relational, Temporal, Structural).
    pub category: String,
}

/// Response for list_embedder_indexes tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEmbedderIndexesResponse {
    /// Information for all 13 embedders.
    pub indexes: Vec<EmbedderIndexInfo>,
    /// Total memories in the system.
    pub total_memories: usize,
    /// Total VRAM usage (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_vram_mb: Option<f32>,
}

// ============================================================================
// get_memory_fingerprint DTOs
// ============================================================================

/// Request for get_memory_fingerprint tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMemoryFingerprintRequest {
    /// UUID of the memory to inspect.
    pub memory_id: String,
    /// Filter to specific embedders (default: all 13).
    #[serde(default)]
    pub embedders: Vec<String>,
    /// Include L2 norms (default: true).
    #[serde(default = "default_true")]
    pub include_vector_norms: bool,
    /// Include content text (default: false).
    #[serde(default)]
    pub include_content: bool,
}

impl GetMemoryFingerprintRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        if uuid::Uuid::parse_str(&self.memory_id).is_err() {
            return Err(format!(
                "Invalid memory_id '{}'. Must be a valid UUID.",
                self.memory_id
            ));
        }
        for e in &self.embedders {
            if EmbedderId::from_str(e).is_none() {
                return Err(format!(
                    "Invalid embedder '{}'. Must be E1-E13.",
                    e
                ));
            }
        }
        Ok(())
    }

    /// Get the set of requested embedder IDs (empty = all 13).
    pub fn embedder_filter(&self) -> Vec<EmbedderId> {
        self.embedders
            .iter()
            .filter_map(|e| EmbedderId::from_str(e))
            .collect()
    }
}

/// Per-embedder introspection data.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderVectorInfo {
    /// Embedder ID (e.g., "E1").
    pub embedder: String,
    /// Human-readable name.
    pub name: String,
    /// Vector dimension (or "sparse" for E6/E13).
    pub dimension: String,
    /// Whether the vector is present (non-empty).
    pub present: bool,
    /// Actual dimension of the stored vector.
    pub actual_dimension: usize,
    /// L2 norm of the vector (if includeVectorNorms=true and vector present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub l2_norm: Option<f32>,
    /// For asymmetric embedders: directional variant info.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants: Option<Vec<AsymmetricVariant>>,
}

/// Asymmetric variant info (for E5, E8, E10).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsymmetricVariant {
    /// Variant name (e.g., "cause", "effect", "source", "target", "intent", "context").
    pub variant: String,
    /// Whether this variant vector is present.
    pub present: bool,
    /// Dimension of this variant.
    pub dimension: usize,
    /// L2 norm if requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub l2_norm: Option<f32>,
}

/// Response for get_memory_fingerprint tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMemoryFingerprintResponse {
    /// Memory ID.
    pub memory_id: Uuid,
    /// Per-embedder vector info.
    pub embedders: Vec<EmbedderVectorInfo>,
    /// Total embedders with vectors present.
    pub embedders_present: usize,
    /// Content text (if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// When the fingerprint was created.
    pub created_at: String,
}

// ============================================================================
// create_weight_profile DTOs
// ============================================================================

/// Request for create_weight_profile tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWeightProfileRequest {
    /// Name for the custom profile (must not conflict with built-in profiles).
    pub name: String,
    /// Per-embedder weights as E1-E13 keys. Must sum to ~1.0.
    pub weights: HashMap<String, f64>,
    /// Optional description for the profile.
    #[serde(default)]
    pub description: Option<String>,
}

impl CreateWeightProfileRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Profile name cannot be empty".to_string());
        }
        if self.name.len() > 64 {
            return Err("Profile name must be at most 64 characters".to_string());
        }
        if self.weights.is_empty() {
            return Err("Weights cannot be empty".to_string());
        }
        // Validate all keys are valid embedder names
        for key in self.weights.keys() {
            if EmbedderId::from_str(key).is_none() {
                return Err(format!(
                    "Invalid embedder name '{}' in weights. Valid names: E1-E13.",
                    key
                ));
            }
        }
        Ok(())
    }

    /// Convert the weights map to a [f32; 13] array.
    pub fn to_weight_array(&self) -> [f32; 13] {
        let mut weights = [0.0f32; 13];
        for (key, &val) in &self.weights {
            if let Some(eid) = EmbedderId::from_str(key) {
                weights[eid.to_index()] = val as f32;
            }
        }
        weights
    }
}

/// Response for create_weight_profile tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWeightProfileResponse {
    /// Name of the created profile.
    pub name: String,
    /// The stored weights as E1-E13 map.
    pub weights: HashMap<String, f32>,
    /// Description if provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Total custom profiles now stored.
    pub total_custom_profiles: usize,
}

// ============================================================================
// search_cross_embedder_anomalies DTOs
// ============================================================================

/// Request for search_cross_embedder_anomalies tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCrossEmbedderAnomaliesRequest {
    /// Search query.
    pub query: String,
    /// Embedder expected to score HIGH (E1-E13).
    pub high_embedder: String,
    /// Embedder expected to score LOW (E1-E13).
    pub low_embedder: String,
    /// Minimum score in high embedder (default: 0.5).
    #[serde(default = "default_high_threshold")]
    pub high_threshold: f32,
    /// Maximum score in low embedder (default: 0.3).
    #[serde(default = "default_low_threshold")]
    pub low_threshold: f32,
    /// Maximum results (default: 10).
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Include content in results (default: false).
    #[serde(default)]
    pub include_content: bool,
}

fn default_high_threshold() -> f32 {
    0.5
}

fn default_low_threshold() -> f32 {
    0.3
}

impl SearchCrossEmbedderAnomaliesRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        if self.query.is_empty() {
            return Err("Query cannot be empty".to_string());
        }
        if EmbedderId::from_str(&self.high_embedder).is_none() {
            return Err(format!(
                "Invalid highEmbedder '{}'. Must be E1-E13.",
                self.high_embedder
            ));
        }
        if EmbedderId::from_str(&self.low_embedder).is_none() {
            return Err(format!(
                "Invalid lowEmbedder '{}'. Must be E1-E13.",
                self.low_embedder
            ));
        }
        if self.high_embedder == self.low_embedder {
            return Err("highEmbedder and lowEmbedder must be different".to_string());
        }
        if self.high_threshold < 0.0 || self.high_threshold > 1.0 {
            return Err("highThreshold must be between 0 and 1".to_string());
        }
        if self.low_threshold < 0.0 || self.low_threshold > 1.0 {
            return Err("lowThreshold must be between 0 and 1".to_string());
        }
        if self.top_k == 0 || self.top_k > 100 {
            return Err("topK must be between 1 and 100".to_string());
        }
        Ok(())
    }
}

/// A single anomaly result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderAnomaly {
    /// Memory ID.
    pub memory_id: Uuid,
    /// Score in the high embedder.
    pub high_score: f32,
    /// Score in the low embedder.
    pub low_score: f32,
    /// Anomaly score = high_score - low_score.
    pub anomaly_score: f32,
    /// Content (if includeContent=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Response for search_cross_embedder_anomalies tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCrossEmbedderAnomaliesResponse {
    /// Query searched.
    pub query: String,
    /// Embedder that scored high.
    pub high_embedder: String,
    /// What the high embedder finds.
    pub high_embedder_finds: String,
    /// Embedder that scored low.
    pub low_embedder: String,
    /// What the low embedder finds.
    pub low_embedder_finds: String,
    /// Anomalous memories (high in one, low in other).
    pub anomalies: Vec<EmbedderAnomaly>,
    /// Total candidates searched.
    pub total_searched: usize,
    /// Search time in milliseconds.
    pub search_time_ms: u64,
    /// Scoring method used: "difference" (highScore - lowScore).
    pub scoring_method: String,
    /// The formula used to compute anomaly scores.
    pub scoring_formula: String,
    /// High score threshold used for filtering.
    pub high_threshold: f32,
    /// Low score threshold used for filtering.
    pub low_threshold: f32,
    /// Documents the over-fetch multiplier used when searching candidates.
    pub search_multiplier: usize,
}

// ============================================================================
// adaptive_search DTOs
// ============================================================================

/// Request for adaptive_search tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptiveSearchRequest {
    /// Search query - will be auto-classified.
    pub query: String,
    /// Maximum results (default: 10).
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Include strategy explanation (default: true).
    #[serde(default = "default_true")]
    pub explain_strategy: bool,
    /// Include content in results (default: false).
    #[serde(default)]
    pub include_content: bool,
}

impl AdaptiveSearchRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        if self.query.is_empty() {
            return Err("Query cannot be empty".to_string());
        }
        if self.top_k == 0 || self.top_k > 100 {
            return Err("topK must be between 1 and 100".to_string());
        }
        Ok(())
    }
}

/// Classification result for a query.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryClassification {
    /// Detected query type.
    pub query_type: String,
    /// Selected weight profile name.
    pub selected_profile: String,
    /// Confidence in the classification (0-1).
    pub confidence: f32,
    /// Why this profile was selected.
    pub reason: String,
    /// Keywords that triggered the classification.
    pub trigger_keywords: Vec<String>,
}

/// Response for adaptive_search tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptiveSearchResponse {
    /// The original query.
    pub query: String,
    /// Query classification and strategy explanation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<QueryClassification>,
    /// Search results (same structure as search_graph).
    pub results: Vec<EmbedderSearchResult>,
    /// Total results found.
    pub total_results: usize,
    /// Search time in milliseconds.
    pub search_time_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_id_from_str() {
        assert_eq!(EmbedderId::from_str("E1"), Some(EmbedderId::E1));
        assert_eq!(EmbedderId::from_str("e11"), Some(EmbedderId::E11));
        assert_eq!(EmbedderId::from_str("E13"), Some(EmbedderId::E13));
        assert_eq!(EmbedderId::from_str("E14"), None);
        assert_eq!(EmbedderId::from_str("invalid"), None);
    }

    #[test]
    fn test_embedder_id_to_index() {
        assert_eq!(EmbedderId::E1.to_index(), 0);
        assert_eq!(EmbedderId::E11.to_index(), 10);
        assert_eq!(EmbedderId::E13.to_index(), 12);
    }

    #[test]
    fn test_embedder_categories() {
        assert!(EmbedderId::E1.is_semantic());
        assert!(EmbedderId::E2.is_temporal());
        assert!(EmbedderId::E6.is_sparse());
        assert!(!EmbedderId::E11.is_temporal());
    }

    #[test]
    fn test_search_request_validation() {
        let valid = SearchByEmbedderRequest {
            embedder: "E1".to_string(),
            query: "test query".to_string(),
            top_k: 10,
            min_similarity: 0.0,
            include_content: false,
            include_all_scores: false,
        };
        assert!(valid.validate().is_ok());

        let invalid_embedder = SearchByEmbedderRequest {
            embedder: "E14".to_string(),
            query: "test".to_string(),
            top_k: 10,
            min_similarity: 0.0,
            include_content: false,
            include_all_scores: false,
        };
        assert!(invalid_embedder.validate().is_err());

        let empty_query = SearchByEmbedderRequest {
            embedder: "E1".to_string(),
            query: "".to_string(),
            top_k: 10,
            min_similarity: 0.0,
            include_content: false,
            include_all_scores: false,
        };
        assert!(empty_query.validate().is_err());
    }

    #[test]
    fn test_compare_request_validation() {
        let valid = CompareEmbedderViewsRequest {
            query: "test".to_string(),
            embedders: vec!["E1".to_string(), "E11".to_string()],
            top_k: 5,
            include_content: false,
        };
        assert!(valid.validate().is_ok());

        let too_few = CompareEmbedderViewsRequest {
            query: "test".to_string(),
            embedders: vec!["E1".to_string()],
            top_k: 5,
            include_content: false,
        };
        assert!(too_few.validate().is_err());

        let too_many = CompareEmbedderViewsRequest {
            query: "test".to_string(),
            embedders: vec![
                "E1".to_string(),
                "E2".to_string(),
                "E3".to_string(),
                "E4".to_string(),
                "E5".to_string(),
                "E6".to_string(),
            ],
            top_k: 5,
            include_content: false,
        };
        assert!(too_many.validate().is_err());
    }
}
