//! Teleological profiles for task-specific embedding fusion.
//!
//! From teleoplan.md Section 6.3 Meta-Learning Across Tasks:
//!
//! Profiles define task-specific configurations for:
//! - Embedding weights (which embeddings matter most)
//! - Fusion strategy (how to combine embeddings)
//! - Task type classification
//!
//! Example profiles:
//! - code_implementation: boosts E6 (Code), E7 (Procedural)
//! - conceptual_research: boosts E11 (Abstract), E5 (Analogical)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::types::{ProfileId, NUM_EMBEDDERS};

/// Fusion strategy for combining embeddings in a teleological profile.
///
/// Different strategies optimize for different use cases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum FusionStrategy {
    /// Simple weighted average of all embeddings.
    #[default]
    WeightedAverage,

    /// Cross-correlation matrix fusion (captures inter-embedding relationships).
    CrossCorrelation,

    /// Tucker tensor decomposition for compact representation.
    /// Ranks: (mode1_rank, mode2_rank, mode3_rank)
    TuckerDecomposition {
        /// Tensor decomposition ranks
        ranks: (usize, usize, usize),
    },

    /// Attention-weighted combination using query context.
    Attention {
        /// Number of attention heads
        heads: usize,
    },

    /// Hierarchical group-then-domain fusion.
    Hierarchical,

    /// Use only primary embeddings (fast path).
    PrimaryOnly,
}

impl FusionStrategy {
    /// Default Tucker ranks from teleoplan.md.
    pub const DEFAULT_TUCKER_RANKS: (usize, usize, usize) = (4, 4, 128);

    /// Default number of attention heads.
    pub const DEFAULT_ATTENTION_HEADS: usize = 4;

    /// Create Tucker decomposition with default ranks.
    pub fn tucker_default() -> Self {
        Self::TuckerDecomposition {
            ranks: Self::DEFAULT_TUCKER_RANKS,
        }
    }

    /// Create attention fusion with default heads.
    pub fn attention_default() -> Self {
        Self::Attention {
            heads: Self::DEFAULT_ATTENTION_HEADS,
        }
    }

    /// Human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            FusionStrategy::WeightedAverage => "Simple weighted average fusion",
            FusionStrategy::CrossCorrelation => "Cross-correlation matrix fusion",
            FusionStrategy::TuckerDecomposition { .. } => "Tucker tensor decomposition",
            FusionStrategy::Attention { .. } => "Attention-weighted fusion",
            FusionStrategy::Hierarchical => "Hierarchical group-then-domain fusion",
            FusionStrategy::PrimaryOnly => "Primary embeddings only (fast)",
        }
    }

    /// Estimated computational cost (1=low, 5=high).
    pub fn cost(&self) -> u8 {
        match self {
            FusionStrategy::PrimaryOnly => 1,
            FusionStrategy::WeightedAverage => 2,
            FusionStrategy::Hierarchical => 2,
            FusionStrategy::CrossCorrelation => 3,
            FusionStrategy::Attention { .. } => 4,
            FusionStrategy::TuckerDecomposition { .. } => 5,
        }
    }
}

/// Task type for automatic profile selection.
///
/// From teleoplan.md query routing examples:
/// - "How do I implement X?" -> Code search
/// - "Why did X happen?" -> Causal search
/// - "What is similar to X?" -> Semantic search
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TaskType {
    /// Code implementation tasks.
    /// Primary: E6 (Code), E7 (Procedural)
    CodeSearch,

    /// General semantic similarity search.
    /// Primary: E1 (Semantic), E5 (Analogical)
    SemanticSearch,

    /// Temporal/sequence queries.
    /// Primary: E2 (Episodic), E3 (Temporal)
    TemporalSearch,

    /// Causal reasoning queries.
    /// Primary: E4 (Causal), E7 (Procedural)
    CausalSearch,

    /// Factual/knowledge retrieval.
    /// Primary: E12 (Factual), E13 (Sparse)
    FactualSearch,

    /// Social/interpersonal context.
    /// Primary: E9 (Social), E10 (Emotional)
    SocialSearch,

    /// Abstract/conceptual queries.
    /// Primary: E11 (Abstract), E5 (Analogical)
    AbstractSearch,

    /// General balanced search.
    #[default]
    General,
}

impl TaskType {
    /// All task types.
    pub const ALL: [TaskType; 8] = [
        TaskType::CodeSearch,
        TaskType::SemanticSearch,
        TaskType::TemporalSearch,
        TaskType::CausalSearch,
        TaskType::FactualSearch,
        TaskType::SocialSearch,
        TaskType::AbstractSearch,
        TaskType::General,
    ];

    /// Get primary embedder indices for this task type.
    ///
    /// Returns the indices of embedders that should be weighted higher.
    pub fn primary_embedders(self) -> &'static [usize] {
        match self {
            TaskType::CodeSearch => &[5, 6],      // E6, E7
            TaskType::SemanticSearch => &[0, 4],  // E1, E5
            TaskType::TemporalSearch => &[1, 2],  // E2, E3
            TaskType::CausalSearch => &[3, 6],    // E4, E7
            TaskType::FactualSearch => &[11, 12], // E12, E13
            TaskType::SocialSearch => &[8, 9],    // E9, E10
            TaskType::AbstractSearch => &[10, 4], // E11, E5
            TaskType::General => &[0, 3, 11],     // E1, E4, E12 (balanced)
        }
    }

    /// Get secondary embedder indices for this task type.
    pub fn secondary_embedders(self) -> &'static [usize] {
        match self {
            TaskType::CodeSearch => &[3, 11],      // E4, E12
            TaskType::SemanticSearch => &[10, 7],  // E11, E8
            TaskType::TemporalSearch => &[11, 8],  // E12, E9
            TaskType::CausalSearch => &[11, 8],    // E12, E9
            TaskType::FactualSearch => &[0, 3],    // E1, E4
            TaskType::SocialSearch => &[0, 1],     // E1, E2
            TaskType::AbstractSearch => &[0, 3],   // E1, E4
            TaskType::General => &[4, 5, 6, 7, 8], // Middle embedders
        }
    }

    /// Human-readable description.
    pub fn description(self) -> &'static str {
        match self {
            TaskType::CodeSearch => "Code implementation and programming",
            TaskType::SemanticSearch => "General semantic similarity",
            TaskType::TemporalSearch => "Temporal and sequence patterns",
            TaskType::CausalSearch => "Causal reasoning and explanations",
            TaskType::FactualSearch => "Factual knowledge retrieval",
            TaskType::SocialSearch => "Social and interpersonal context",
            TaskType::AbstractSearch => "Abstract and conceptual queries",
            TaskType::General => "Balanced general-purpose search",
        }
    }

    /// Suggested fusion strategy for this task type.
    pub fn suggested_strategy(self) -> FusionStrategy {
        match self {
            TaskType::CodeSearch => FusionStrategy::PrimaryOnly,
            TaskType::SemanticSearch => FusionStrategy::CrossCorrelation,
            TaskType::TemporalSearch => FusionStrategy::Hierarchical,
            TaskType::CausalSearch => FusionStrategy::Hierarchical,
            TaskType::FactualSearch => FusionStrategy::WeightedAverage,
            TaskType::SocialSearch => FusionStrategy::attention_default(),
            TaskType::AbstractSearch => FusionStrategy::CrossCorrelation,
            TaskType::General => FusionStrategy::WeightedAverage,
        }
    }
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskType::CodeSearch => write!(f, "Code"),
            TaskType::SemanticSearch => write!(f, "Semantic"),
            TaskType::TemporalSearch => write!(f, "Temporal"),
            TaskType::CausalSearch => write!(f, "Causal"),
            TaskType::FactualSearch => write!(f, "Factual"),
            TaskType::SocialSearch => write!(f, "Social"),
            TaskType::AbstractSearch => write!(f, "Abstract"),
            TaskType::General => write!(f, "General"),
        }
    }
}

/// Performance metrics for a teleological profile.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProfileMetrics {
    /// Mean Reciprocal Rank (position of first relevant result).
    pub mrr: f32,

    /// Recall at position 5.
    pub recall_at_5: f32,

    /// Recall at position 10.
    pub recall_at_10: f32,

    /// Precision at position 5.
    pub precision_at_5: f32,

    /// Precision at position 10.
    pub precision_at_10: f32,

    /// Number of retrievals used to compute these metrics.
    pub retrieval_count: u64,

    /// Average latency in milliseconds.
    pub avg_latency_ms: f32,
}

impl ProfileMetrics {
    /// Create metrics with all values.
    pub fn new(
        mrr: f32,
        recall_at_5: f32,
        recall_at_10: f32,
        precision_at_5: f32,
        precision_at_10: f32,
    ) -> Self {
        Self {
            mrr,
            recall_at_5,
            recall_at_10,
            precision_at_5,
            precision_at_10,
            retrieval_count: 0,
            avg_latency_ms: 0.0,
        }
    }

    /// Overall quality score (weighted combination of metrics).
    pub fn quality_score(&self) -> f32 {
        // MRR (30%) + Recall@10 (30%) + Precision@10 (40%)
        0.3 * self.mrr + 0.3 * self.recall_at_10 + 0.4 * self.precision_at_10
    }

    /// F1 score at position 10.
    pub fn f1_at_10(&self) -> f32 {
        let p = self.precision_at_10;
        let r = self.recall_at_10;

        if p + r < f32::EPSILON {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }

    /// Update metrics with EWMA.
    pub fn update_ewma(
        &mut self,
        new_mrr: f32,
        new_recall_5: f32,
        new_recall_10: f32,
        new_precision_5: f32,
        new_precision_10: f32,
        latency_ms: f32,
        alpha: f32,
    ) {
        self.mrr = alpha * new_mrr + (1.0 - alpha) * self.mrr;
        self.recall_at_5 = alpha * new_recall_5 + (1.0 - alpha) * self.recall_at_5;
        self.recall_at_10 = alpha * new_recall_10 + (1.0 - alpha) * self.recall_at_10;
        self.precision_at_5 = alpha * new_precision_5 + (1.0 - alpha) * self.precision_at_5;
        self.precision_at_10 = alpha * new_precision_10 + (1.0 - alpha) * self.precision_at_10;
        self.avg_latency_ms = alpha * latency_ms + (1.0 - alpha) * self.avg_latency_ms;
        self.retrieval_count += 1;
    }
}

/// A teleological profile: task-specific configuration for embedding fusion.
///
/// Profiles are learned from retrieval feedback and can be:
/// - Pre-defined for common task types
/// - Learned automatically from user behavior
/// - Merged/split based on performance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeleologicalProfile {
    /// Unique profile identifier.
    pub id: ProfileId,

    /// Human-readable profile name.
    pub name: String,

    /// Per-embedder weights [0.0, 1.0].
    /// Higher weight = more contribution to final score.
    pub embedding_weights: [f32; NUM_EMBEDDERS],

    /// Fusion strategy for this profile.
    pub fusion_strategy: FusionStrategy,

    /// Task type this profile is optimized for.
    pub task_type: TaskType,

    /// When this profile was created.
    pub created_at: DateTime<Utc>,

    /// When this profile was last updated.
    pub updated_at: DateTime<Utc>,

    /// Number of samples used to learn this profile.
    pub sample_count: u64,

    /// Performance metrics for this profile.
    pub metrics: ProfileMetrics,

    /// Whether this is a system-defined profile (not learned).
    pub is_system: bool,

    /// Description of the profile's purpose.
    pub description: Option<String>,
}

impl TeleologicalProfile {
    /// Create a new profile with uniform weights.
    pub fn new(id: impl Into<String>, name: impl Into<String>, task_type: TaskType) -> Self {
        let now = Utc::now();
        Self {
            id: ProfileId::new(id),
            name: name.into(),
            embedding_weights: [1.0 / NUM_EMBEDDERS as f32; NUM_EMBEDDERS],
            fusion_strategy: task_type.suggested_strategy(),
            task_type,
            created_at: now,
            updated_at: now,
            sample_count: 0,
            metrics: ProfileMetrics::default(),
            is_system: false,
            description: None,
        }
    }

    /// Create a system profile for a task type with optimized weights.
    pub fn system(task_type: TaskType) -> Self {
        let id = format!("system_{}", task_type);
        let name = format!("{} (System)", task_type.description());

        let mut profile = Self::new(id, name, task_type);
        profile.is_system = true;

        // Set weights based on task type
        let primary = task_type.primary_embedders();
        let secondary = task_type.secondary_embedders();

        // Reset to low baseline
        for w in profile.embedding_weights.iter_mut() {
            *w = 0.05;
        }

        // Boost primary embedders
        for &idx in primary {
            profile.embedding_weights[idx] = 0.2;
        }

        // Moderate boost for secondary
        for &idx in secondary {
            if profile.embedding_weights[idx] < 0.1 {
                profile.embedding_weights[idx] = 0.1;
            }
        }

        // Normalize weights
        profile.normalize_weights();

        profile
    }

    /// Create the code implementation profile.
    ///
    /// From teleoplan.md example:
    /// ```json
    /// "code_implementation": {
    ///   "weights": [0.05, 0.02, 0.05, 0.15, 0.08, 0.25, 0.18, 0.05, 0.02, 0.02, 0.05, 0.05, 0.03],
    ///   "primary_embeddings": [6, 7, 4],
    ///   "fusion_method": "attention_weighted"
    /// }
    /// ```
    pub fn code_implementation() -> Self {
        let mut profile = Self::system(TaskType::CodeSearch);
        profile.id = ProfileId::new("code_implementation");
        profile.name = "Code Implementation".to_string();
        profile.embedding_weights = [
            0.05, 0.02, 0.05, 0.15, 0.08, 0.25, 0.18, 0.05, 0.02, 0.02, 0.05, 0.05, 0.03,
        ];
        profile.fusion_strategy = FusionStrategy::attention_default();
        profile.description = Some("Optimized for code implementation queries".to_string());
        profile
    }

    /// Create the conceptual research profile.
    ///
    /// From teleoplan.md example:
    /// ```json
    /// "conceptual_research": {
    ///   "weights": [0.12, 0.05, 0.03, 0.10, 0.15, 0.03, 0.02, 0.05, 0.05, 0.05, 0.20, 0.12, 0.03],
    ///   "primary_embeddings": [11, 5, 1, 12],
    ///   "fusion_method": "hierarchical_group"
    /// }
    /// ```
    pub fn conceptual_research() -> Self {
        let mut profile = Self::system(TaskType::AbstractSearch);
        profile.id = ProfileId::new("conceptual_research");
        profile.name = "Conceptual Research".to_string();
        profile.embedding_weights = [
            0.12, 0.05, 0.03, 0.10, 0.15, 0.03, 0.02, 0.05, 0.05, 0.05, 0.20, 0.12, 0.03,
        ];
        profile.fusion_strategy = FusionStrategy::Hierarchical;
        profile.description = Some("Optimized for conceptual and research queries".to_string());
        profile
    }

    /// Normalize weights to sum to 1.0.
    pub fn normalize_weights(&mut self) {
        let sum: f32 = self.embedding_weights.iter().sum();
        if sum > f32::EPSILON {
            for w in self.embedding_weights.iter_mut() {
                *w /= sum;
            }
        }
    }

    /// Get weight for a specific embedder.
    ///
    /// # Panics
    ///
    /// Panics if index >= NUM_EMBEDDERS (FAIL FAST).
    #[inline]
    pub fn get_weight(&self, embedder_idx: usize) -> f32 {
        assert!(
            embedder_idx < NUM_EMBEDDERS,
            "FAIL FAST: embedder index {} out of bounds (max {})",
            embedder_idx,
            NUM_EMBEDDERS - 1
        );
        self.embedding_weights[embedder_idx]
    }

    /// Get all weights for all embedders as a slice.
    #[inline]
    pub fn get_all_weights(&self) -> &[f32; NUM_EMBEDDERS] {
        &self.embedding_weights
    }

    /// Set weight for a specific embedder.
    ///
    /// # Panics
    ///
    /// - Panics if index >= NUM_EMBEDDERS (FAIL FAST)
    /// - Panics if weight < 0 (FAIL FAST)
    pub fn set_weight(&mut self, embedder_idx: usize, weight: f32) {
        assert!(
            embedder_idx < NUM_EMBEDDERS,
            "FAIL FAST: embedder index {} out of bounds (max {})",
            embedder_idx,
            NUM_EMBEDDERS - 1
        );
        assert!(
            weight >= 0.0,
            "FAIL FAST: weight must be non-negative, got {}",
            weight
        );

        self.embedding_weights[embedder_idx] = weight;
        self.updated_at = Utc::now();
    }

    /// Get indices of top N weighted embedders.
    pub fn top_embedders(&self, n: usize) -> Vec<usize> {
        let mut indexed: Vec<(usize, f32)> = self
            .embedding_weights
            .iter()
            .enumerate()
            .map(|(i, &w)| (i, w))
            .collect();

        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        indexed.into_iter().take(n).map(|(i, _)| i).collect()
    }

    /// Calculate similarity between two profiles (weight vector cosine).
    pub fn similarity(&self, other: &Self) -> f32 {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..NUM_EMBEDDERS {
            dot += self.embedding_weights[i] * other.embedding_weights[i];
            norm_a += self.embedding_weights[i] * self.embedding_weights[i];
            norm_b += other.embedding_weights[i] * other.embedding_weights[i];
        }

        let denom = (norm_a.sqrt()) * (norm_b.sqrt());
        if denom < f32::EPSILON {
            0.0
        } else {
            dot / denom
        }
    }

    /// Set description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== FusionStrategy Tests =====

    #[test]
    fn test_fusion_strategy_default() {
        assert_eq!(FusionStrategy::default(), FusionStrategy::WeightedAverage);

        println!("[PASS] FusionStrategy::default is WeightedAverage");
    }

    #[test]
    fn test_fusion_strategy_tucker_default() {
        let strategy = FusionStrategy::tucker_default();

        match strategy {
            FusionStrategy::TuckerDecomposition { ranks } => {
                assert_eq!(ranks, FusionStrategy::DEFAULT_TUCKER_RANKS);
            }
            _ => panic!("Expected TuckerDecomposition"),
        }

        println!("[PASS] tucker_default uses correct ranks");
    }

    #[test]
    fn test_fusion_strategy_attention_default() {
        let strategy = FusionStrategy::attention_default();

        match strategy {
            FusionStrategy::Attention { heads } => {
                assert_eq!(heads, FusionStrategy::DEFAULT_ATTENTION_HEADS);
            }
            _ => panic!("Expected Attention"),
        }

        println!("[PASS] attention_default uses correct heads");
    }

    #[test]
    fn test_fusion_strategy_cost() {
        assert!(FusionStrategy::PrimaryOnly.cost() < FusionStrategy::WeightedAverage.cost());
        assert!(FusionStrategy::WeightedAverage.cost() < FusionStrategy::tucker_default().cost());

        println!("[PASS] FusionStrategy costs are ordered correctly");
    }

    #[test]
    fn test_fusion_strategy_serialization() {
        let strategy = FusionStrategy::TuckerDecomposition { ranks: (2, 3, 64) };
        let json = serde_json::to_string(&strategy).unwrap();
        let deserialized: FusionStrategy = serde_json::from_str(&json).unwrap();

        assert_eq!(strategy, deserialized);

        println!("[PASS] FusionStrategy serialization works");
    }

    // ===== TaskType Tests =====

    #[test]
    fn test_task_type_default() {
        assert_eq!(TaskType::default(), TaskType::General);

        println!("[PASS] TaskType::default is General");
    }

    #[test]
    fn test_task_type_all() {
        assert_eq!(TaskType::ALL.len(), 8);

        println!("[PASS] TaskType::ALL contains 8 types");
    }

    #[test]
    fn test_task_type_primary_embedders() {
        let code = TaskType::CodeSearch.primary_embedders();
        assert!(code.contains(&5)); // E6
        assert!(code.contains(&6)); // E7

        let semantic = TaskType::SemanticSearch.primary_embedders();
        assert!(semantic.contains(&0)); // E1
        assert!(semantic.contains(&4)); // E5

        println!("[PASS] primary_embedders match teleoplan.md");
    }

    #[test]
    fn test_task_type_suggested_strategy() {
        assert_eq!(
            TaskType::CodeSearch.suggested_strategy(),
            FusionStrategy::PrimaryOnly
        );

        match TaskType::SocialSearch.suggested_strategy() {
            FusionStrategy::Attention { .. } => {}
            _ => panic!("Expected Attention for SocialSearch"),
        }

        println!("[PASS] suggested_strategy returns appropriate strategies");
    }

    #[test]
    fn test_task_type_display() {
        assert_eq!(format!("{}", TaskType::CodeSearch), "Code");
        assert_eq!(format!("{}", TaskType::General), "General");

        println!("[PASS] TaskType Display works");
    }

    // ===== ProfileMetrics Tests =====

    #[test]
    fn test_profile_metrics_default() {
        let metrics = ProfileMetrics::default();

        assert!((metrics.mrr - 0.0).abs() < f32::EPSILON);
        assert_eq!(metrics.retrieval_count, 0);

        println!("[PASS] ProfileMetrics::default creates zeros");
    }

    #[test]
    fn test_profile_metrics_quality_score() {
        let metrics = ProfileMetrics::new(0.8, 0.7, 0.9, 0.6, 0.85);

        // 0.3 * 0.8 + 0.3 * 0.9 + 0.4 * 0.85 = 0.24 + 0.27 + 0.34 = 0.85
        assert!((metrics.quality_score() - 0.85).abs() < 0.001);

        println!("[PASS] quality_score computes correctly");
    }

    #[test]
    fn test_profile_metrics_f1() {
        let metrics = ProfileMetrics::new(0.0, 0.0, 0.8, 0.0, 0.6);

        // F1 = 2 * 0.6 * 0.8 / (0.6 + 0.8) = 0.96 / 1.4 = 0.6857
        assert!((metrics.f1_at_10() - 0.6857).abs() < 0.01);

        // Zero case
        let zero_metrics = ProfileMetrics::default();
        assert!((zero_metrics.f1_at_10() - 0.0).abs() < f32::EPSILON);

        println!("[PASS] f1_at_10 computes correctly");
    }

    #[test]
    fn test_profile_metrics_update_ewma() {
        let mut metrics = ProfileMetrics::new(0.5, 0.5, 0.5, 0.5, 0.5);

        metrics.update_ewma(1.0, 1.0, 1.0, 1.0, 1.0, 50.0, 0.1);

        // After EWMA: 0.1 * 1.0 + 0.9 * 0.5 = 0.55
        assert!((metrics.mrr - 0.55).abs() < 0.001);
        assert_eq!(metrics.retrieval_count, 1);

        println!("[PASS] update_ewma applies EWMA correctly");
    }

    // ===== TeleologicalProfile Tests =====

    #[test]
    fn test_profile_new() {
        let profile = TeleologicalProfile::new("test", "Test Profile", TaskType::General);

        assert_eq!(profile.id.as_str(), "test");
        assert_eq!(profile.name, "Test Profile");
        assert_eq!(profile.task_type, TaskType::General);
        assert_eq!(profile.sample_count, 0);
        assert!(!profile.is_system);

        // Weights should be uniform
        let expected_weight = 1.0 / NUM_EMBEDDERS as f32;
        for &w in profile.embedding_weights.iter() {
            assert!((w - expected_weight).abs() < 0.001);
        }

        println!("[PASS] TeleologicalProfile::new creates valid profile");
    }

    #[test]
    fn test_profile_system() {
        let profile = TeleologicalProfile::system(TaskType::CodeSearch);

        assert!(profile.is_system);
        assert!(profile.id.as_str().contains("system"));

        // Primary embedders should have higher weights
        let primary = TaskType::CodeSearch.primary_embedders();
        for &idx in primary {
            assert!(
                profile.embedding_weights[idx] > 0.1,
                "Primary embedder {} should have high weight",
                idx
            );
        }

        println!("[PASS] TeleologicalProfile::system creates optimized profile");
    }

    #[test]
    fn test_profile_code_implementation() {
        let profile = TeleologicalProfile::code_implementation();

        // Check specific weights from teleoplan.md
        assert!((profile.embedding_weights[5] - 0.25).abs() < 0.001); // E6
        assert!((profile.embedding_weights[6] - 0.18).abs() < 0.001); // E7

        println!("[PASS] code_implementation matches teleoplan.md");
    }

    #[test]
    fn test_profile_conceptual_research() {
        let profile = TeleologicalProfile::conceptual_research();

        // Check specific weights from teleoplan.md
        assert!((profile.embedding_weights[10] - 0.20).abs() < 0.001); // E11

        match profile.fusion_strategy {
            FusionStrategy::Hierarchical => {}
            _ => panic!("Expected Hierarchical fusion"),
        }

        println!("[PASS] conceptual_research matches teleoplan.md");
    }

    #[test]
    fn test_profile_normalize_weights() {
        let mut profile = TeleologicalProfile::new("test", "Test", TaskType::General);

        // Set non-normalized weights
        profile.embedding_weights = [1.0; NUM_EMBEDDERS];
        profile.normalize_weights();

        let sum: f32 = profile.embedding_weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);

        println!("[PASS] normalize_weights sums to 1.0");
    }

    #[test]
    fn test_profile_get_set_weight() {
        let mut profile = TeleologicalProfile::new("test", "Test", TaskType::General);

        profile.set_weight(5, 0.5);
        assert!((profile.get_weight(5) - 0.5).abs() < f32::EPSILON);

        println!("[PASS] get/set weight work correctly");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_profile_get_weight_out_of_bounds() {
        let profile = TeleologicalProfile::new("test", "Test", TaskType::General);
        let _ = profile.get_weight(13);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_profile_set_negative_weight() {
        let mut profile = TeleologicalProfile::new("test", "Test", TaskType::General);
        profile.set_weight(0, -0.1);
    }

    #[test]
    fn test_profile_top_embedders() {
        let profile = TeleologicalProfile::code_implementation();
        let top3 = profile.top_embedders(3);

        assert_eq!(top3.len(), 3);
        assert!(top3.contains(&5)); // E6 has highest weight (0.25)
        assert!(top3.contains(&6)); // E7 has second highest (0.18)

        println!("[PASS] top_embedders returns highest weighted");
    }

    #[test]
    fn test_profile_similarity() {
        let p1 = TeleologicalProfile::code_implementation();
        let p2 = TeleologicalProfile::code_implementation();

        let sim = p1.similarity(&p2);
        assert!((sim - 1.0).abs() < 0.001);

        let p3 = TeleologicalProfile::conceptual_research();
        let sim2 = p1.similarity(&p3);
        assert!(sim2 < 0.95); // Should be different

        println!("[PASS] profile similarity works correctly");
    }

    #[test]
    fn test_profile_serialization() {
        let profile = TeleologicalProfile::code_implementation();

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: TeleologicalProfile = serde_json::from_str(&json).unwrap();

        assert_eq!(profile.id, deserialized.id);
        assert_eq!(profile.embedding_weights, deserialized.embedding_weights);

        println!("[PASS] Profile serialization roundtrip works");
    }

    #[test]
    fn test_profile_with_description() {
        let profile = TeleologicalProfile::new("test", "Test", TaskType::General)
            .with_description("A test profile");

        assert_eq!(profile.description, Some("A test profile".to_string()));

        println!("[PASS] with_description works");
    }
}
