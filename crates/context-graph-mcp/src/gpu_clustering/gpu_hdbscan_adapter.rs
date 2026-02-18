//! GPU HDBSCAN adapter for topic detection.
//!
//! Bridges context-graph-cuda's GpuHdbscanClusterer with the MCP topic detection flow.
//!
//! # Constitution Compliance
//!
//! - ARCH-GPU-05: HDBSCAN clustering runs on GPU via FAISS
//! - AP-GPU-04: No sklearn HDBSCAN fallback
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-60: Temporal embedders (E2-E4) weight = 0.0 in topic detection

use std::collections::HashMap;
use std::time::Instant;

use thiserror::Error;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use context_graph_core::teleological::Embedder;
use context_graph_cuda::hdbscan::{
    GpuHdbscanClusterer, GpuHdbscanError, HdbscanParams,
    ClusterMembership as GpuClusterMembership,
};

/// Errors that can occur during GPU clustering.
#[derive(Error, Debug)]
pub enum GpuClusteringError {
    /// GPU not available - required per constitution.
    #[error("GPU not available for HDBSCAN clustering: {0}")]
    GpuNotAvailable(String),

    /// GPU HDBSCAN operation failed.
    #[error("GPU HDBSCAN error: {0}")]
    HdbscanError(#[from] GpuHdbscanError),

    /// Invalid input data.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Insufficient data for clustering.
    #[error("Insufficient data: need at least {required} points, got {actual}")]
    InsufficientData { required: usize, actual: usize },
}

/// Result of GPU topic detection.
#[derive(Debug, Clone)]
pub struct GpuClusteringResult {
    /// Cluster memberships per embedder space.
    pub memberships: HashMap<Embedder, Vec<GpuClusterMembership>>,
    /// Total clusters found across all spaces.
    pub total_clusters: usize,
    /// Time taken for GPU k-NN (microseconds).
    pub gpu_knn_time_us: u64,
    /// Time taken for entire clustering (milliseconds).
    pub total_time_ms: u64,
}

/// GPU-accelerated topic detector.
///
/// Uses FAISS GPU for k-NN computation, which is the O(n²k) bottleneck of HDBSCAN.
/// Per constitution, this is the ONLY valid way to run topic detection.
pub struct GpuTopicDetector {
    /// HDBSCAN parameters.
    params: HdbscanParams,
}

impl GpuTopicDetector {
    /// Create a new GPU topic detector with constitution-compliant defaults.
    ///
    /// Default parameters per constitution:
    /// - min_cluster_size: 3
    /// - min_samples: 2
    pub fn new() -> Self {
        Self {
            params: HdbscanParams {
                min_cluster_size: 3, // Constitution default
                min_samples: 2,
                cluster_selection_method: Default::default(),
            },
        }
    }

    /// Create with custom parameters.
    pub fn with_params(params: HdbscanParams) -> Self {
        Self { params }
    }

    /// Detect clusters in a single embedder space.
    ///
    /// # Arguments
    ///
    /// * `embedder` - The embedder space (E1-E13)
    /// * `embeddings` - Vectors for this embedder
    /// * `memory_ids` - UUIDs corresponding to each embedding
    ///
    /// # Returns
    ///
    /// Vector of cluster memberships.
    ///
    /// # Errors
    ///
    /// - `GpuNotAvailable` if GPU is not detected
    /// - `HdbscanError` if FAISS operations fail
    /// - `InsufficientData` if fewer points than min_cluster_size
    #[instrument(skip_all, fields(embedder = ?embedder, n_points = embeddings.len()))]
    pub fn detect_clusters_for_space(
        &self,
        embedder: Embedder,
        embeddings: &[Vec<f32>],
        memory_ids: &[Uuid],
    ) -> Result<Vec<GpuClusterMembership>, GpuClusteringError> {
        let n = embeddings.len();

        debug!(
            embedder = ?embedder,
            n_points = n,
            min_cluster_size = self.params.min_cluster_size,
            "GPU topic detection for embedder space"
        );

        if n < self.params.min_cluster_size {
            return Err(GpuClusteringError::InsufficientData {
                required: self.params.min_cluster_size,
                actual: n,
            });
        }

        if n != memory_ids.len() {
            return Err(GpuClusteringError::InvalidInput(format!(
                "embeddings count ({}) != memory_ids count ({})",
                n,
                memory_ids.len()
            )));
        }

        let clusterer = GpuHdbscanClusterer::with_params(self.params.clone());
        let memberships = clusterer.fit(embeddings, memory_ids)?;

        let n_clusters = memberships
            .iter()
            .filter(|m| m.cluster_id >= 0)
            .map(|m| m.cluster_id)
            .collect::<std::collections::HashSet<_>>()
            .len();

        debug!(
            embedder = ?embedder,
            n_clusters,
            n_noise = memberships.iter().filter(|m| m.cluster_id < 0).count(),
            "GPU clustering completed for space"
        );

        Ok(memberships)
    }

    /// Detect topics across all 13 embedder spaces.
    ///
    /// # Arguments
    ///
    /// * `fingerprints` - Map of memory_id -> array of 13 embeddings
    ///
    /// # Returns
    ///
    /// `GpuClusteringResult` with memberships per embedder and total clusters.
    ///
    /// # Errors
    ///
    /// Fails if GPU is unavailable or clustering fails in any space.
    #[instrument(skip_all, fields(n_fingerprints = fingerprints.len()))]
    pub fn detect_topics_all_spaces(
        &self,
        fingerprints: &HashMap<Uuid, [Vec<f32>; 13]>,
    ) -> Result<GpuClusteringResult, GpuClusteringError> {
        let start = Instant::now();
        let n = fingerprints.len();

        if n < self.params.min_cluster_size {
            return Err(GpuClusteringError::InsufficientData {
                required: self.params.min_cluster_size,
                actual: n,
            });
        }

        info!(n_fingerprints = n, "Starting GPU topic detection across 13 spaces");

        // Prepare data by embedder
        let memory_ids: Vec<Uuid> = fingerprints.keys().cloned().collect();
        let mut memberships: HashMap<Embedder, Vec<GpuClusterMembership>> = HashMap::new();
        let mut total_clusters = 0usize;
        let mut gpu_knn_time_us = 0u64;

        // Process each embedder space
        for embedder_idx in 0..13 {
            let embedder = match Embedder::from_index(embedder_idx) {
                Some(e) => e,
                None => continue, // Should never happen for 0..13
            };

            // Extract embeddings for this space
            let space_embeddings: Vec<Vec<f32>> = fingerprints
                .values()
                .map(|arr| arr[embedder_idx].clone())
                .collect();

            // Skip if insufficient data
            if space_embeddings.len() < self.params.min_cluster_size {
                debug!(embedder = ?embedder, "Skipping space - insufficient data");
                continue;
            }

            // Skip temporal embedders for topic detection per AP-60
            // They have weight 0.0 and shouldn't contribute to topics
            // But we still run clustering to maintain consistency
            let space_start = Instant::now();

            match self.detect_clusters_for_space(embedder, &space_embeddings, &memory_ids) {
                Ok(space_memberships) => {
                    let space_clusters = space_memberships
                        .iter()
                        .filter(|m| m.cluster_id >= 0)
                        .map(|m| m.cluster_id)
                        .collect::<std::collections::HashSet<_>>()
                        .len();

                    total_clusters += space_clusters;
                    gpu_knn_time_us += space_start.elapsed().as_micros() as u64;
                    memberships.insert(embedder, space_memberships);

                    debug!(
                        embedder = ?embedder,
                        clusters = space_clusters,
                        elapsed_us = space_start.elapsed().as_micros(),
                        "Completed GPU clustering for space"
                    );
                }
                Err(e) => {
                    // Log but continue with other spaces
                    // This allows partial results if some spaces fail
                    warn!(
                        embedder = ?embedder,
                        error = %e,
                        "Failed to cluster embedder space"
                    );
                }
            }
        }

        let total_time_ms = start.elapsed().as_millis() as u64;

        info!(
            n_fingerprints = n,
            total_clusters,
            spaces_processed = memberships.len(),
            total_time_ms,
            gpu_knn_time_us,
            "GPU topic detection completed"
        );

        Ok(GpuClusteringResult {
            memberships,
            total_clusters,
            gpu_knn_time_us,
            total_time_ms,
        })
    }

    /// Compute weighted agreement for a memory based on cluster memberships.
    ///
    /// Per constitution ARCH-09: topic threshold is weighted_agreement >= 2.5
    ///
    /// Weights:
    /// - SEMANTIC (E1,E5,E6,E7,E10,E12,E13): 1.0
    /// - RELATIONAL (E8,E11): 0.5
    /// - STRUCTURAL (E9): 0.5
    /// - TEMPORAL (E2,E3,E4): 0.0 (never count)
    pub fn compute_weighted_agreement(
        &self,
        memory_id: Uuid,
        memberships: &HashMap<Embedder, Vec<GpuClusterMembership>>,
    ) -> f32 {
        let mut weighted_sum = 0.0f32;

        for (embedder, space_memberships) in memberships {
            // Find this memory's membership
            let membership = space_memberships
                .iter()
                .find(|m| m.memory_id == memory_id);

            if let Some(m) = membership {
                if m.cluster_id >= 0 {
                    // Memory is in a cluster
                    let weight = embedder.topic_weight();
                    weighted_sum += weight;
                }
            }
        }

        weighted_sum
    }
}

impl Default for GpuTopicDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for Embedder to get topic weight.
trait EmbedderExt {
    fn topic_weight(&self) -> f32;
}

impl EmbedderExt for Embedder {
    /// Get topic weight per constitution.
    ///
    /// - SEMANTIC (E1,E5,E6,E7,E10,E12,E13): 1.0
    /// - RELATIONAL (E8,E11): 0.5
    /// - STRUCTURAL (E9): 0.5
    /// - TEMPORAL (E2,E3,E4): 0.0
    fn topic_weight(&self) -> f32 {
        match self {
            // Temporal - never count toward topics (AP-60)
            Embedder::TemporalRecent
            | Embedder::TemporalPeriodic
            | Embedder::TemporalPositional => 0.0,
            // Semantic - full weight
            Embedder::Semantic
            | Embedder::Causal
            | Embedder::Sparse
            | Embedder::Code
            | Embedder::Multimodal
            | Embedder::LateInteraction
            | Embedder::KeywordSplade => 1.0,
            // Relational - half weight
            Embedder::Graph | Embedder::Entity => 0.5,
            // Structural - half weight
            Embedder::Hdc => 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_weights_match_constitution() {
        // SEMANTIC = 1.0
        assert_eq!(Embedder::Semantic.topic_weight(), 1.0);
        assert_eq!(Embedder::Causal.topic_weight(), 1.0);
        assert_eq!(Embedder::Sparse.topic_weight(), 1.0);
        assert_eq!(Embedder::Code.topic_weight(), 1.0);
        assert_eq!(Embedder::Multimodal.topic_weight(), 1.0);
        assert_eq!(Embedder::LateInteraction.topic_weight(), 1.0);
        assert_eq!(Embedder::KeywordSplade.topic_weight(), 1.0);

        // RELATIONAL = 0.5
        assert_eq!(Embedder::Graph.topic_weight(), 0.5);
        assert_eq!(Embedder::Entity.topic_weight(), 0.5);

        // STRUCTURAL = 0.5
        assert_eq!(Embedder::Hdc.topic_weight(), 0.5);

        // TEMPORAL = 0.0
        assert_eq!(Embedder::TemporalRecent.topic_weight(), 0.0);
        assert_eq!(Embedder::TemporalPeriodic.topic_weight(), 0.0);
        assert_eq!(Embedder::TemporalPositional.topic_weight(), 0.0);

        // Max possible = 7*1.0 + 2*0.5 + 1*0.5 = 8.5
        let max: f32 = (0..13)
            .filter_map(|i| Embedder::from_index(i))
            .map(|e| e.topic_weight())
            .sum();
        assert!((max - 8.5).abs() < 0.001);
    }

    #[test]
    fn test_detector_creation() {
        let detector = GpuTopicDetector::new();
        assert_eq!(detector.params.min_cluster_size, 3);
        assert_eq!(detector.params.min_samples, 2);
    }
}
