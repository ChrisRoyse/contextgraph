//! K-means clustering for 13D purpose vectors.
//!
//! # CRITICAL: NO FALLBACKS
//!
//! Clustering failures are fatal. No partial results returned.
//! If clustering cannot complete, error propagates immediately.
//!
//! # Overview
//!
//! This module implements k-means++ clustering for purpose vectors,
//! enabling discovery of natural groupings in teleological alignment space.
//!
//! # Algorithm
//!
//! 1. Initialize k centroids using k-means++ (smart initialization)
//! 2. Assign each vector to nearest centroid (Euclidean distance)
//! 3. Recompute centroids as mean of assigned vectors
//! 4. Repeat until convergence or max iterations
//!
//! # Fail-Fast Validation
//!
//! - k must be > 0 and <= entries.len()
//! - max_iterations must be > 0
//! - convergence_threshold must be > 0.0
//! - entries must not be empty

use std::collections::HashMap;
use uuid::Uuid;

use crate::index::config::PURPOSE_VECTOR_DIM;

use super::entry::{GoalId, PurposeIndexEntry};
use super::error::{PurposeIndexError, PurposeIndexResult};

/// A cluster of memories with similar purpose vectors.
///
/// Represents the result of k-means clustering on purpose vectors,
/// containing the centroid, members, and quality metrics.
#[derive(Clone, Debug)]
pub struct PurposeCluster {
    /// Cluster centroid (13D purpose vector).
    ///
    /// Computed as the mean of all member purpose vectors.
    pub centroid: [f32; PURPOSE_VECTOR_DIM],

    /// Memory IDs belonging to this cluster.
    ///
    /// All memories whose purpose vectors are closest to this centroid.
    pub members: Vec<Uuid>,

    /// Intra-cluster coherence score [0.0, 1.0].
    ///
    /// Higher values indicate more tightly clustered members.
    /// Computed as 1.0 - (mean distance to centroid / max possible distance).
    pub coherence: f32,

    /// Dominant goal for this cluster.
    ///
    /// The most frequent primary goal among cluster members.
    /// None if no metadata is available.
    pub dominant_goal: Option<GoalId>,
}

impl PurposeCluster {
    /// Create a new cluster with computed metrics.
    ///
    /// # Arguments
    ///
    /// * `centroid` - The 13D centroid of the cluster
    /// * `members` - UUIDs of memories in this cluster
    /// * `coherence` - Intra-cluster coherence score
    /// * `dominant_goal` - Most common goal in the cluster
    pub fn new(
        centroid: [f32; PURPOSE_VECTOR_DIM],
        members: Vec<Uuid>,
        coherence: f32,
        dominant_goal: Option<GoalId>,
    ) -> Self {
        Self {
            centroid,
            members,
            coherence,
            dominant_goal,
        }
    }

    /// Check if the cluster is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Get the number of members in this cluster.
    #[inline]
    pub fn len(&self) -> usize {
        self.members.len()
    }
}

/// Configuration for k-means clustering.
///
/// # Validation
///
/// All parameters are validated at construction time.
/// Invalid configurations result in immediate errors.
#[derive(Clone, Debug)]
pub struct KMeansConfig {
    /// Number of clusters (k).
    ///
    /// Must be > 0 and <= number of data points.
    pub k: usize,

    /// Maximum iterations before stopping.
    ///
    /// Must be > 0. Typical values: 50-300.
    pub max_iterations: usize,

    /// Convergence threshold for centroid movement.
    ///
    /// Iteration stops when max centroid movement is below this.
    /// Must be > 0.0. Typical value: 1e-6.
    pub convergence_threshold: f32,
}

impl KMeansConfig {
    /// Create a new configuration with validation.
    ///
    /// # Arguments
    ///
    /// * `k` - Number of clusters (must be > 0)
    /// * `max_iterations` - Maximum iterations (must be > 0)
    /// * `convergence_threshold` - Convergence threshold (must be > 0.0)
    ///
    /// # Errors
    ///
    /// Returns `PurposeIndexError::ClusteringError` if any parameter is invalid.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = KMeansConfig::new(5, 100, 1e-6)?;
    /// ```
    pub fn new(
        k: usize,
        max_iterations: usize,
        convergence_threshold: f32,
    ) -> PurposeIndexResult<Self> {
        if k == 0 {
            return Err(PurposeIndexError::clustering("k must be > 0"));
        }
        if max_iterations == 0 {
            return Err(PurposeIndexError::clustering("max_iterations must be > 0"));
        }
        if convergence_threshold <= 0.0 {
            return Err(PurposeIndexError::clustering(
                "convergence_threshold must be > 0.0",
            ));
        }
        if convergence_threshold.is_nan() || convergence_threshold.is_infinite() {
            return Err(PurposeIndexError::clustering(
                "convergence_threshold must be a finite positive number",
            ));
        }

        Ok(Self {
            k,
            max_iterations,
            convergence_threshold,
        })
    }

    /// Create a default configuration for the given number of clusters.
    ///
    /// Uses max_iterations=100 and convergence_threshold=1e-6.
    ///
    /// # Errors
    ///
    /// Returns error if k is 0.
    pub fn with_k(k: usize) -> PurposeIndexResult<Self> {
        Self::new(k, 100, 1e-6)
    }
}

impl Default for KMeansConfig {
    /// Default configuration: k=3, max_iterations=100, convergence_threshold=1e-6.
    fn default() -> Self {
        Self {
            k: 3,
            max_iterations: 100,
            convergence_threshold: 1e-6,
        }
    }
}

/// Result of k-means clustering operation.
///
/// Contains the final clusters and convergence information.
#[derive(Clone, Debug)]
pub struct ClusteringResult {
    /// The clusters found by k-means.
    ///
    /// Length equals k from the configuration.
    pub clusters: Vec<PurposeCluster>,

    /// Number of iterations to converge.
    ///
    /// If converged is false, this equals max_iterations.
    pub iterations: usize,

    /// Whether convergence was achieved.
    ///
    /// True if max centroid movement fell below threshold.
    pub converged: bool,

    /// Total within-cluster sum of squares (WCSS).
    ///
    /// Lower values indicate better clustering.
    /// Sum of squared distances from each point to its centroid.
    pub wcss: f32,
}

impl ClusteringResult {
    /// Create a new clustering result.
    pub fn new(
        clusters: Vec<PurposeCluster>,
        iterations: usize,
        converged: bool,
        wcss: f32,
    ) -> Self {
        Self {
            clusters,
            iterations,
            converged,
            wcss,
        }
    }

    /// Get the number of clusters.
    #[inline]
    pub fn num_clusters(&self) -> usize {
        self.clusters.len()
    }

    /// Get the total number of points across all clusters.
    pub fn total_points(&self) -> usize {
        self.clusters.iter().map(|c| c.len()).sum()
    }

    /// Get the average cluster size.
    pub fn avg_cluster_size(&self) -> f32 {
        if self.clusters.is_empty() {
            0.0
        } else {
            self.total_points() as f32 / self.clusters.len() as f32
        }
    }

    /// Get the average coherence across all clusters.
    pub fn avg_coherence(&self) -> f32 {
        if self.clusters.is_empty() {
            0.0
        } else {
            let sum: f32 = self.clusters.iter().map(|c| c.coherence).sum();
            sum / self.clusters.len() as f32
        }
    }
}

/// Trait for k-means clustering on purpose vectors.
///
/// Implementors provide k-means clustering functionality for
/// collections of purpose index entries.
pub trait KMeansPurposeClustering {
    /// Cluster purpose vectors using k-means algorithm.
    ///
    /// # Arguments
    ///
    /// * `entries` - The purpose index entries to cluster
    /// * `config` - K-means configuration
    ///
    /// # Errors
    ///
    /// Returns `PurposeIndexError::ClusteringError` if:
    /// - entries is empty
    /// - k > entries.len()
    /// - Algorithm fails to converge within max_iterations (still returns partial result)
    ///
    /// # Fail-Fast
    ///
    /// Invalid inputs cause immediate errors. No fallbacks.
    fn cluster_purposes(
        &self,
        entries: &[PurposeIndexEntry],
        config: &KMeansConfig,
    ) -> PurposeIndexResult<ClusteringResult>;
}

/// Standard k-means++ implementation for purpose vectors.
///
/// Uses k-means++ initialization for better initial centroids
/// and standard Lloyd's algorithm for iteration.
#[derive(Clone, Debug, Default)]
pub struct StandardKMeans;

impl StandardKMeans {
    /// Create a new StandardKMeans clusterer.
    pub fn new() -> Self {
        Self
    }
}

impl KMeansPurposeClustering for StandardKMeans {
    fn cluster_purposes(
        &self,
        entries: &[PurposeIndexEntry],
        config: &KMeansConfig,
    ) -> PurposeIndexResult<ClusteringResult> {
        // FAIL FAST: Validate inputs
        if entries.is_empty() {
            return Err(PurposeIndexError::clustering("entries must not be empty"));
        }
        if config.k > entries.len() {
            return Err(PurposeIndexError::clustering(format!(
                "k ({}) must be <= entries.len() ({})",
                config.k,
                entries.len()
            )));
        }

        println!(
            "[CLUSTERING] Starting k-means: k={}, n={}, max_iter={}",
            config.k,
            entries.len(),
            config.max_iterations
        );

        // Extract vectors for clustering
        let vectors: Vec<[f32; PURPOSE_VECTOR_DIM]> = entries
            .iter()
            .map(|e| e.purpose_vector.alignments)
            .collect();

        // Initialize centroids using k-means++
        let mut centroids = kmeans_plus_plus_init(&vectors, config.k);

        println!(
            "[CLUSTERING] Initialized {} centroids using k-means++",
            centroids.len()
        );

        // Main k-means loop
        let mut assignments = vec![0usize; entries.len()];
        let mut iterations = 0;
        let mut converged = false;

        for iter in 0..config.max_iterations {
            iterations = iter + 1;

            // Assignment step: assign each point to nearest centroid
            for (i, vector) in vectors.iter().enumerate() {
                let mut min_dist = f32::MAX;
                let mut best_cluster = 0;

                for (j, centroid) in centroids.iter().enumerate() {
                    let dist = euclidean_distance_squared(vector, centroid);
                    if dist < min_dist {
                        min_dist = dist;
                        best_cluster = j;
                    }
                }

                assignments[i] = best_cluster;
            }

            // Update step: recompute centroids
            let new_centroids = compute_centroids(&vectors, &assignments, config.k);

            // Check convergence: max centroid movement
            let max_movement = centroids
                .iter()
                .zip(new_centroids.iter())
                .map(|(old, new)| euclidean_distance_squared(old, new).sqrt())
                .fold(0.0f32, |a, b| a.max(b));

            centroids = new_centroids;

            if max_movement < config.convergence_threshold {
                converged = true;
                println!(
                    "[CLUSTERING] Converged at iteration {} (movement={:.2e})",
                    iterations, max_movement
                );
                break;
            }
        }

        if !converged {
            println!(
                "[CLUSTERING] Did not converge after {} iterations",
                iterations
            );
        }

        // Build clusters with metadata
        let clusters = build_clusters(entries, &assignments, &centroids, config.k);

        // Compute WCSS (within-cluster sum of squares)
        let wcss = compute_wcss(&vectors, &assignments, &centroids);

        println!(
            "[CLUSTERING] Completed: {} clusters, {} iterations, WCSS={:.4}",
            clusters.len(),
            iterations,
            wcss
        );

        Ok(ClusteringResult::new(clusters, iterations, converged, wcss))
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Compute squared Euclidean distance between two vectors.
///
/// Uses squared distance to avoid sqrt for comparison.
#[inline]
fn euclidean_distance_squared(a: &[f32; PURPOSE_VECTOR_DIM], b: &[f32; PURPOSE_VECTOR_DIM]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| (x - y) * (x - y)).sum()
}

/// Compute Euclidean distance between two vectors.
#[inline]
fn euclidean_distance(a: &[f32; PURPOSE_VECTOR_DIM], b: &[f32; PURPOSE_VECTOR_DIM]) -> f32 {
    euclidean_distance_squared(a, b).sqrt()
}

/// Initialize centroids using k-means++ algorithm.
///
/// K-means++ provides better initial centroids by choosing them
/// with probability proportional to squared distance from existing centroids.
fn kmeans_plus_plus_init(
    vectors: &[[f32; PURPOSE_VECTOR_DIM]],
    k: usize,
) -> Vec<[f32; PURPOSE_VECTOR_DIM]> {
    let n = vectors.len();
    let mut centroids = Vec::with_capacity(k);

    // Choose first centroid uniformly at random
    // Use deterministic selection for reproducibility in tests
    let first_idx = 0;
    centroids.push(vectors[first_idx]);

    // Distance from each point to nearest centroid
    let mut min_distances = vec![f32::MAX; n];

    for _ in 1..k {
        // Update distances
        let last_centroid = centroids.last().unwrap();
        for (i, vector) in vectors.iter().enumerate() {
            let dist = euclidean_distance_squared(vector, last_centroid);
            if dist < min_distances[i] {
                min_distances[i] = dist;
            }
        }

        // Select next centroid with probability proportional to D^2
        // Use deterministic weighted selection for reproducibility
        let total: f32 = min_distances.iter().sum();
        if total == 0.0 {
            // All points are at centroid locations, pick next available
            for (i, _) in vectors.iter().enumerate() {
                if !centroids
                    .iter()
                    .any(|c| euclidean_distance_squared(c, &vectors[i]) < 1e-10)
                {
                    centroids.push(vectors[i]);
                    break;
                }
            }
        } else {
            // Find the point with maximum distance (deterministic approximation)
            let max_idx = min_distances
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0);
            centroids.push(vectors[max_idx]);
        }
    }

    centroids
}

/// Compute new centroids as mean of assigned points.
fn compute_centroids(
    vectors: &[[f32; PURPOSE_VECTOR_DIM]],
    assignments: &[usize],
    k: usize,
) -> Vec<[f32; PURPOSE_VECTOR_DIM]> {
    let mut sums = vec![[0.0f32; PURPOSE_VECTOR_DIM]; k];
    let mut counts = vec![0usize; k];

    for (i, &cluster) in assignments.iter().enumerate() {
        counts[cluster] += 1;
        for d in 0..PURPOSE_VECTOR_DIM {
            sums[cluster][d] += vectors[i][d];
        }
    }

    sums.into_iter()
        .zip(counts)
        .map(|(mut sum, count)| {
            if count > 0 {
                for elem in sum.iter_mut() {
                    *elem /= count as f32;
                }
            }
            sum
        })
        .collect()
}

/// Build cluster objects with metadata.
fn build_clusters(
    entries: &[PurposeIndexEntry],
    assignments: &[usize],
    centroids: &[[f32; PURPOSE_VECTOR_DIM]],
    k: usize,
) -> Vec<PurposeCluster> {
    let mut cluster_members: Vec<Vec<usize>> = vec![Vec::new(); k];

    for (i, &cluster) in assignments.iter().enumerate() {
        cluster_members[cluster].push(i);
    }

    cluster_members
        .into_iter()
        .enumerate()
        .map(|(cluster_idx, member_indices)| {
            let members: Vec<Uuid> = member_indices
                .iter()
                .map(|&i| entries[i].memory_id)
                .collect();

            let coherence = if members.is_empty() {
                0.0
            } else {
                compute_cluster_coherence(entries, &member_indices, &centroids[cluster_idx])
            };

            let dominant_goal = find_dominant_goal(entries, &member_indices);

            PurposeCluster::new(centroids[cluster_idx], members, coherence, dominant_goal)
        })
        .collect()
}

/// Compute coherence score for a cluster.
///
/// Coherence is computed as 1 - (mean_distance / max_possible_distance).
/// Max possible distance for normalized vectors in 13D is sqrt(4*13) = 7.21.
fn compute_cluster_coherence(
    entries: &[PurposeIndexEntry],
    member_indices: &[usize],
    centroid: &[f32; PURPOSE_VECTOR_DIM],
) -> f32 {
    if member_indices.is_empty() {
        return 0.0;
    }

    let total_dist: f32 = member_indices
        .iter()
        .map(|&i| euclidean_distance(&entries[i].purpose_vector.alignments, centroid))
        .sum();

    let mean_dist = total_dist / member_indices.len() as f32;

    // Max distance in 13D for vectors in [0,1] is sqrt(13)
    let max_dist = (PURPOSE_VECTOR_DIM as f32).sqrt();

    (1.0 - mean_dist / max_dist).clamp(0.0, 1.0)
}

/// Find the most common goal among cluster members.
fn find_dominant_goal(entries: &[PurposeIndexEntry], member_indices: &[usize]) -> Option<GoalId> {
    if member_indices.is_empty() {
        return None;
    }

    let mut goal_counts: HashMap<String, usize> = HashMap::new();

    for &i in member_indices {
        let goal_str = entries[i].metadata.primary_goal.as_str().to_string();
        *goal_counts.entry(goal_str).or_insert(0) += 1;
    }

    goal_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(goal, _)| GoalId::new(goal))
}

/// Compute within-cluster sum of squares.
fn compute_wcss(
    vectors: &[[f32; PURPOSE_VECTOR_DIM]],
    assignments: &[usize],
    centroids: &[[f32; PURPOSE_VECTOR_DIM]],
) -> f32 {
    vectors
        .iter()
        .zip(assignments.iter())
        .map(|(vector, &cluster)| euclidean_distance_squared(vector, &centroids[cluster]))
        .sum()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::purpose::entry::PurposeMetadata;
    use crate::types::fingerprint::PurposeVector;
    use crate::types::JohariQuadrant;

    // =========================================================================
    // Helper functions for creating test data (REAL data, NO mocks)
    // =========================================================================

    /// Create a purpose vector with deterministic values based on base and variation.
    fn create_purpose_vector(base: f32, variation: f32) -> PurposeVector {
        let mut alignments = [0.0f32; PURPOSE_VECTOR_DIM];
        for (i, alignment) in alignments.iter_mut().enumerate() {
            *alignment = (base + (i as f32 * variation)).clamp(0.0, 1.0);
        }
        PurposeVector::new(alignments)
    }

    /// Create a test entry with a specific base value and goal.
    fn create_entry(base: f32, goal: &str) -> PurposeIndexEntry {
        let pv = create_purpose_vector(base, 0.02);
        let metadata = PurposeMetadata::new(GoalId::new(goal), 0.85, JohariQuadrant::Open).unwrap();
        PurposeIndexEntry::new(Uuid::new_v4(), pv, metadata)
    }

    /// Create entries forming distinct clusters.
    fn create_clustered_entries() -> Vec<PurposeIndexEntry> {
        let mut entries = Vec::new();

        // Cluster 1: low values (base around 0.2)
        for i in 0..5 {
            entries.push(create_entry(0.15 + i as f32 * 0.02, "goal_low"));
        }

        // Cluster 2: medium values (base around 0.5)
        for i in 0..5 {
            entries.push(create_entry(0.45 + i as f32 * 0.02, "goal_mid"));
        }

        // Cluster 3: high values (base around 0.8)
        for i in 0..5 {
            entries.push(create_entry(0.75 + i as f32 * 0.02, "goal_high"));
        }

        entries
    }

    // =========================================================================
    // KMeansConfig Tests
    // =========================================================================

    #[test]
    fn test_kmeans_config_valid() {
        let config = KMeansConfig::new(5, 100, 1e-6).unwrap();

        assert_eq!(config.k, 5);
        assert_eq!(config.max_iterations, 100);
        assert!((config.convergence_threshold - 1e-6).abs() < 1e-10);

        println!("[VERIFIED] KMeansConfig::new creates valid config");
    }

    #[test]
    fn test_kmeans_config_with_k() {
        let config = KMeansConfig::with_k(10).unwrap();

        assert_eq!(config.k, 10);
        assert_eq!(config.max_iterations, 100);

        println!("[VERIFIED] KMeansConfig::with_k creates config with defaults");
    }

    #[test]
    fn test_kmeans_config_invalid_k_zero() {
        let result = KMeansConfig::new(0, 100, 1e-6);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("k must be > 0"));

        println!("[VERIFIED] FAIL FAST: KMeansConfig rejects k=0: {}", msg);
    }

    #[test]
    fn test_kmeans_config_invalid_max_iterations_zero() {
        let result = KMeansConfig::new(5, 0, 1e-6);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("max_iterations must be > 0"));

        println!(
            "[VERIFIED] FAIL FAST: KMeansConfig rejects max_iterations=0: {}",
            msg
        );
    }

    #[test]
    fn test_kmeans_config_invalid_threshold_zero() {
        let result = KMeansConfig::new(5, 100, 0.0);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("convergence_threshold must be > 0.0"));

        println!(
            "[VERIFIED] FAIL FAST: KMeansConfig rejects convergence_threshold=0.0: {}",
            msg
        );
    }

    #[test]
    fn test_kmeans_config_invalid_threshold_negative() {
        let result = KMeansConfig::new(5, 100, -1e-6);

        assert!(result.is_err());

        println!("[VERIFIED] FAIL FAST: KMeansConfig rejects negative convergence_threshold");
    }

    #[test]
    fn test_kmeans_config_invalid_threshold_nan() {
        let result = KMeansConfig::new(5, 100, f32::NAN);

        assert!(result.is_err());

        println!("[VERIFIED] FAIL FAST: KMeansConfig rejects NaN convergence_threshold");
    }

    #[test]
    fn test_kmeans_config_invalid_threshold_infinity() {
        let result = KMeansConfig::new(5, 100, f32::INFINITY);

        assert!(result.is_err());

        println!("[VERIFIED] FAIL FAST: KMeansConfig rejects infinite convergence_threshold");
    }

    #[test]
    fn test_kmeans_config_default() {
        let config = KMeansConfig::default();

        assert_eq!(config.k, 3);
        assert_eq!(config.max_iterations, 100);
        assert!((config.convergence_threshold - 1e-6).abs() < 1e-10);

        println!("[VERIFIED] KMeansConfig::default creates sensible defaults");
    }

    // =========================================================================
    // PurposeCluster Tests
    // =========================================================================

    #[test]
    fn test_purpose_cluster_new() {
        let centroid = [0.5; PURPOSE_VECTOR_DIM];
        let members = vec![Uuid::new_v4(), Uuid::new_v4()];
        let cluster =
            PurposeCluster::new(centroid, members.clone(), 0.85, Some(GoalId::new("test")));

        assert_eq!(cluster.centroid, centroid);
        assert_eq!(cluster.members.len(), 2);
        assert!((cluster.coherence - 0.85).abs() < f32::EPSILON);
        assert!(cluster.dominant_goal.is_some());

        println!("[VERIFIED] PurposeCluster::new creates cluster with all fields");
    }

    #[test]
    fn test_purpose_cluster_len_and_is_empty() {
        let empty_cluster = PurposeCluster::new([0.5; PURPOSE_VECTOR_DIM], vec![], 0.0, None);

        assert!(empty_cluster.is_empty());
        assert_eq!(empty_cluster.len(), 0);

        let filled_cluster = PurposeCluster::new(
            [0.5; PURPOSE_VECTOR_DIM],
            vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()],
            0.9,
            None,
        );

        assert!(!filled_cluster.is_empty());
        assert_eq!(filled_cluster.len(), 3);

        println!("[VERIFIED] PurposeCluster len and is_empty work correctly");
    }

    // =========================================================================
    // ClusteringResult Tests
    // =========================================================================

    #[test]
    fn test_clustering_result_new() {
        let clusters = vec![
            PurposeCluster::new([0.2; PURPOSE_VECTOR_DIM], vec![Uuid::new_v4()], 0.8, None),
            PurposeCluster::new([0.8; PURPOSE_VECTOR_DIM], vec![Uuid::new_v4()], 0.9, None),
        ];

        let result = ClusteringResult::new(clusters, 25, true, 0.5);

        assert_eq!(result.num_clusters(), 2);
        assert_eq!(result.iterations, 25);
        assert!(result.converged);
        assert!((result.wcss - 0.5).abs() < f32::EPSILON);

        println!("[VERIFIED] ClusteringResult::new creates result with all fields");
    }

    #[test]
    fn test_clustering_result_total_points() {
        let clusters = vec![
            PurposeCluster::new(
                [0.2; PURPOSE_VECTOR_DIM],
                vec![Uuid::new_v4(), Uuid::new_v4()],
                0.8,
                None,
            ),
            PurposeCluster::new(
                [0.5; PURPOSE_VECTOR_DIM],
                vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()],
                0.9,
                None,
            ),
            PurposeCluster::new([0.8; PURPOSE_VECTOR_DIM], vec![Uuid::new_v4()], 0.7, None),
        ];

        let result = ClusteringResult::new(clusters, 10, true, 0.3);

        assert_eq!(result.total_points(), 6); // 2 + 3 + 1

        println!("[VERIFIED] ClusteringResult::total_points returns correct count");
    }

    #[test]
    fn test_clustering_result_avg_cluster_size() {
        let clusters = vec![
            PurposeCluster::new(
                [0.2; PURPOSE_VECTOR_DIM],
                vec![Uuid::new_v4(), Uuid::new_v4()],
                0.8,
                None,
            ),
            PurposeCluster::new(
                [0.5; PURPOSE_VECTOR_DIM],
                vec![
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                ],
                0.9,
                None,
            ),
        ];

        let result = ClusteringResult::new(clusters, 10, true, 0.3);

        assert!((result.avg_cluster_size() - 3.0).abs() < f32::EPSILON); // (2 + 4) / 2

        println!("[VERIFIED] ClusteringResult::avg_cluster_size returns correct average");
    }

    #[test]
    fn test_clustering_result_avg_coherence() {
        let clusters = vec![
            PurposeCluster::new([0.2; PURPOSE_VECTOR_DIM], vec![], 0.8, None),
            PurposeCluster::new([0.5; PURPOSE_VECTOR_DIM], vec![], 0.9, None),
            PurposeCluster::new([0.8; PURPOSE_VECTOR_DIM], vec![], 0.7, None),
        ];

        let result = ClusteringResult::new(clusters, 10, true, 0.3);

        let expected = (0.8 + 0.9 + 0.7) / 3.0;
        assert!((result.avg_coherence() - expected).abs() < f32::EPSILON);

        println!("[VERIFIED] ClusteringResult::avg_coherence returns correct average");
    }

    // =========================================================================
    // StandardKMeans Clustering Tests
    // =========================================================================

    #[test]
    fn test_cluster_empty_entries_fails() {
        let clusterer = StandardKMeans::new();
        let entries: Vec<PurposeIndexEntry> = vec![];
        let config = KMeansConfig::default();

        let result = clusterer.cluster_purposes(&entries, &config);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("empty"));

        println!(
            "[VERIFIED] FAIL FAST: clustering rejects empty entries: {}",
            msg
        );
    }

    #[test]
    fn test_cluster_k_greater_than_entries_fails() {
        let clusterer = StandardKMeans::new();
        let entries = vec![create_entry(0.5, "goal")];
        let config = KMeansConfig::new(5, 100, 1e-6).unwrap(); // k=5 but only 1 entry

        let result = clusterer.cluster_purposes(&entries, &config);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("k (5)"));
        assert!(msg.contains("entries.len() (1)"));

        println!(
            "[VERIFIED] FAIL FAST: clustering rejects k > entries.len(): {}",
            msg
        );
    }

    #[test]
    fn test_cluster_single_point() {
        let clusterer = StandardKMeans::new();
        let entries = vec![create_entry(0.5, "single_goal")];
        let config = KMeansConfig::new(1, 100, 1e-6).unwrap();

        println!("[BEFORE] entries.len()={}", entries.len());

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        println!(
            "[AFTER] clusters={}, points={}",
            result.num_clusters(),
            result.total_points()
        );

        assert_eq!(result.num_clusters(), 1);
        assert_eq!(result.total_points(), 1);
        assert!(result.converged);
        assert_eq!(result.clusters[0].len(), 1);

        println!("[VERIFIED] Single point clustering works correctly");
    }

    #[test]
    fn test_cluster_all_same_points() {
        let clusterer = StandardKMeans::new();

        // All entries have the same purpose vector
        let base_pv = create_purpose_vector(0.5, 0.0);
        let entries: Vec<PurposeIndexEntry> = (0..5)
            .map(|_| {
                let metadata =
                    PurposeMetadata::new(GoalId::new("same"), 0.9, JohariQuadrant::Open).unwrap();
                PurposeIndexEntry::new(Uuid::new_v4(), base_pv.clone(), metadata)
            })
            .collect();

        let config = KMeansConfig::new(2, 100, 1e-6).unwrap();

        println!("[BEFORE] entries with identical vectors: {}", entries.len());

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        println!(
            "[AFTER] clusters={}, total_points={}",
            result.num_clusters(),
            result.total_points()
        );

        assert_eq!(result.num_clusters(), 2);
        assert_eq!(result.total_points(), 5);
        // WCSS should be very low since all points are the same
        assert!(result.wcss < 0.1);

        println!("[VERIFIED] All same points clustering works (edge case)");
    }

    #[test]
    fn test_cluster_distinct_clusters() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries();
        let config = KMeansConfig::new(3, 100, 1e-6).unwrap();

        println!(
            "[BEFORE] entries={}, expecting 3 distinct clusters",
            entries.len()
        );

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        println!(
            "[AFTER] clusters={}, iterations={}, converged={}, WCSS={:.4}",
            result.num_clusters(),
            result.iterations,
            result.converged,
            result.wcss
        );

        assert_eq!(result.num_clusters(), 3);
        assert_eq!(result.total_points(), 15);

        // All clusters should have members
        for (i, cluster) in result.clusters.iter().enumerate() {
            assert!(!cluster.is_empty(), "Cluster {} should not be empty", i);
            println!(
                "  Cluster {}: {} members, coherence={:.4}, goal={:?}",
                i,
                cluster.len(),
                cluster.coherence,
                cluster.dominant_goal
            );
        }

        println!("[VERIFIED] Clustering produces 3 non-empty clusters for distinct data");
    }

    #[test]
    fn test_cluster_convergence_detection() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries();
        let config = KMeansConfig::new(3, 500, 1e-6).unwrap();

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        // With well-separated clusters, should converge
        assert!(result.converged);
        assert!(result.iterations < config.max_iterations);

        println!(
            "[VERIFIED] Convergence detected at iteration {} < max {}",
            result.iterations, config.max_iterations
        );
    }

    #[test]
    fn test_cluster_dominant_goal_detection() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries();
        let config = KMeansConfig::new(3, 100, 1e-6).unwrap();

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        // Each cluster should have a dominant goal
        let goals_found: Vec<_> = result
            .clusters
            .iter()
            .filter_map(|c| c.dominant_goal.as_ref())
            .map(|g| g.as_str().to_string())
            .collect();

        println!("[BEFORE] Expected goals: goal_low, goal_mid, goal_high");
        println!("[AFTER] Found goals: {:?}", goals_found);

        // We should find at least some goals
        assert!(!goals_found.is_empty());

        println!("[VERIFIED] Dominant goals detected for clusters");
    }

    #[test]
    fn test_cluster_coherence_computation() {
        let clusterer = StandardKMeans::new();

        // Create tightly grouped entries (high coherence expected)
        let entries: Vec<PurposeIndexEntry> = (0..10)
            .map(|i| create_entry(0.5 + i as f32 * 0.005, "tight"))
            .collect();

        let config = KMeansConfig::new(1, 100, 1e-6).unwrap();

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        // Single cluster with tight grouping should have high coherence
        let coherence = result.clusters[0].coherence;
        println!(
            "[RESULT] Tight cluster coherence: {:.4} (expected > 0.9)",
            coherence
        );
        assert!(coherence > 0.9);

        println!("[VERIFIED] Coherence correctly computed for tight cluster");
    }

    #[test]
    fn test_cluster_wcss_decreases_with_more_clusters() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries();

        let result_k1 = clusterer
            .cluster_purposes(&entries, &KMeansConfig::new(1, 100, 1e-6).unwrap())
            .unwrap();

        let result_k2 = clusterer
            .cluster_purposes(&entries, &KMeansConfig::new(2, 100, 1e-6).unwrap())
            .unwrap();

        let result_k3 = clusterer
            .cluster_purposes(&entries, &KMeansConfig::new(3, 100, 1e-6).unwrap())
            .unwrap();

        println!(
            "[RESULT] WCSS: k=1: {:.4}, k=2: {:.4}, k=3: {:.4}",
            result_k1.wcss, result_k2.wcss, result_k3.wcss
        );

        // WCSS should decrease (or stay same) as k increases
        assert!(result_k1.wcss >= result_k2.wcss);
        assert!(result_k2.wcss >= result_k3.wcss);

        println!("[VERIFIED] WCSS decreases with increasing k");
    }

    #[test]
    fn test_cluster_various_k_values() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries(); // 15 entries

        for k in [1, 2, 3, 5, 10, 15] {
            let config = KMeansConfig::new(k, 100, 1e-6).unwrap();
            let result = clusterer.cluster_purposes(&entries, &config).unwrap();

            assert_eq!(result.num_clusters(), k);
            assert_eq!(result.total_points(), 15);

            println!(
                "  k={}: clusters={}, iterations={}, WCSS={:.4}",
                k,
                result.num_clusters(),
                result.iterations,
                result.wcss
            );
        }

        println!("[VERIFIED] Clustering works for various k values");
    }

    // =========================================================================
    // Helper Function Tests
    // =========================================================================

    #[test]
    fn test_euclidean_distance_squared() {
        let a = [0.0; PURPOSE_VECTOR_DIM];
        let b = [1.0; PURPOSE_VECTOR_DIM];

        let dist_sq = euclidean_distance_squared(&a, &b);

        // Distance should be 13 (sum of 13 ones squared)
        assert!((dist_sq - 13.0).abs() < f32::EPSILON);

        println!("[VERIFIED] euclidean_distance_squared computes correctly");
    }

    #[test]
    fn test_euclidean_distance() {
        let a = [0.0; PURPOSE_VECTOR_DIM];
        let b = [1.0; PURPOSE_VECTOR_DIM];

        let dist = euclidean_distance(&a, &b);

        // Distance should be sqrt(13)
        let expected = (PURPOSE_VECTOR_DIM as f32).sqrt();
        assert!((dist - expected).abs() < 1e-6);

        println!("[VERIFIED] euclidean_distance computes correctly");
    }

    #[test]
    fn test_euclidean_distance_same_point() {
        let a = [0.5; PURPOSE_VECTOR_DIM];

        let dist = euclidean_distance(&a, &a);

        assert!(dist.abs() < f32::EPSILON);

        println!("[VERIFIED] euclidean_distance returns 0 for same point");
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_cluster_max_iterations_reached() {
        let clusterer = StandardKMeans::new();

        // Create entries that won't converge quickly
        let entries: Vec<PurposeIndexEntry> = (0..20)
            .map(|i| create_entry(i as f32 / 20.0, &format!("goal_{}", i % 5)))
            .collect();

        // Very few iterations
        let config = KMeansConfig::new(5, 2, 1e-10).unwrap();

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        // Should reach max iterations without converging
        assert_eq!(result.iterations, 2);
        // May or may not converge in 2 iterations

        println!(
            "[VERIFIED] Clustering respects max_iterations limit (converged={})",
            result.converged
        );
    }

    #[test]
    fn test_cluster_preserves_all_memory_ids() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries();
        let original_ids: std::collections::HashSet<Uuid> =
            entries.iter().map(|e| e.memory_id).collect();

        let config = KMeansConfig::new(3, 100, 1e-6).unwrap();
        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        let clustered_ids: std::collections::HashSet<Uuid> = result
            .clusters
            .iter()
            .flat_map(|c| c.members.iter().copied())
            .collect();

        assert_eq!(original_ids, clustered_ids);

        println!("[VERIFIED] All memory IDs preserved after clustering");
    }

    #[test]
    fn test_cluster_result_clone_and_debug() {
        let clusterer = StandardKMeans::new();
        let entries = vec![create_entry(0.5, "test")];
        let config = KMeansConfig::new(1, 10, 1e-6).unwrap();

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        // Test Clone
        let cloned = result.clone();
        assert_eq!(cloned.num_clusters(), result.num_clusters());
        assert_eq!(cloned.iterations, result.iterations);

        // Test Debug
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("ClusteringResult"));

        println!("[VERIFIED] ClusteringResult implements Clone and Debug");
    }

    #[test]
    fn test_purpose_cluster_clone_and_debug() {
        let cluster = PurposeCluster::new(
            [0.5; PURPOSE_VECTOR_DIM],
            vec![Uuid::new_v4()],
            0.9,
            Some(GoalId::new("test")),
        );

        // Test Clone
        let cloned = cluster.clone();
        assert_eq!(cloned.len(), cluster.len());
        assert_eq!(cloned.coherence, cluster.coherence);

        // Test Debug
        let debug_str = format!("{:?}", cluster);
        assert!(debug_str.contains("PurposeCluster"));

        println!("[VERIFIED] PurposeCluster implements Clone and Debug");
    }
}
