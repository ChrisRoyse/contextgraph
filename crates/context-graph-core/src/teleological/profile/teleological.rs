//! TeleologicalProfile: task-specific configuration for embedding fusion.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::teleological::types::{ProfileId, NUM_EMBEDDERS};

use super::{FusionStrategy, ProfileMetrics, TaskType};

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
            0.05, 0.02, 0.05, 0.15, 0.08, 0.25, 0.18, 0.05, 0.02, 0.02, 0.05, 0.05, 0.03, 0.0,
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
            0.12, 0.05, 0.03, 0.10, 0.15, 0.03, 0.02, 0.05, 0.05, 0.05, 0.20, 0.12, 0.03, 0.0,
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
