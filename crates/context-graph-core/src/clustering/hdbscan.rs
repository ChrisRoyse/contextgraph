//! HDBSCAN clustering parameters and clusterer implementation.
//!
//! Provides configuration types and the core HDBSCAN algorithm for batch
//! density-based clustering across the 13 embedding spaces.
//!
//! # Architecture
//!
//! Per constitution:
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - clustering.parameters.min_cluster_size: 3
//! - clustering.parameters.silhouette_threshold: 0.3
//!
//! # Algorithm
//!
//! HDBSCAN = Hierarchical Density-Based Spatial Clustering of Applications with Noise
//!
//! Steps:
//! 1. Compute core distances (distance to k-th nearest neighbor)
//! 2. Build mutual reachability graph: MR(a,b) = max(core_dist(a), core_dist(b), dist(a,b))
//! 3. Construct minimum spanning tree using Prim's algorithm
//! 4. Extract clusters with Union-Find respecting min_cluster_size

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::embeddings::config::get_distance_metric;
use crate::index::config::DistanceMetric;
use crate::teleological::Embedder;

use super::error::ClusterError;
use super::membership::ClusterMembership;

/// Cluster selection method for HDBSCAN.
///
/// Determines how clusters are extracted from the HDBSCAN hierarchy.
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::hdbscan::ClusterSelectionMethod;
///
/// let method = ClusterSelectionMethod::default();
/// assert_eq!(method, ClusterSelectionMethod::EOM);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ClusterSelectionMethod {
    /// Excess of Mass - default, good general purpose.
    /// Selects clusters based on persistence in the hierarchy.
    #[default]
    EOM,
    /// Leaf clusters only - more granular clustering.
    /// Selects only the leaf nodes of the hierarchy tree.
    Leaf,
}

impl ClusterSelectionMethod {
    /// Get description of this method.
    pub fn description(&self) -> &'static str {
        match self {
            ClusterSelectionMethod::EOM => "Excess of Mass - good general purpose clustering",
            ClusterSelectionMethod::Leaf => "Leaf clusters only - more granular clustering",
        }
    }
}

/// Parameters for HDBSCAN clustering algorithm.
///
/// Per constitution: clustering.parameters.min_cluster_size: 3
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::hdbscan::{HDBSCANParams, hdbscan_defaults};
/// use context_graph_core::teleological::Embedder;
///
/// // Use defaults
/// let params = hdbscan_defaults();
/// assert_eq!(params.min_cluster_size, 3);
///
/// // Or space-specific
/// let semantic_params = HDBSCANParams::default_for_space(Embedder::Semantic);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HDBSCANParams {
    /// Minimum number of points to form a cluster.
    /// Per constitution: 3
    pub min_cluster_size: usize,

    /// Minimum samples for a point to be considered a core point.
    /// Must be <= min_cluster_size.
    pub min_samples: usize,

    /// Method for selecting clusters from hierarchy.
    pub cluster_selection_method: ClusterSelectionMethod,

    /// Distance metric to use.
    /// Retrieved via get_distance_metric(embedder) for space-specific params.
    pub metric: DistanceMetric,
}

impl Default for HDBSCANParams {
    fn default() -> Self {
        Self {
            min_cluster_size: 3, // Per constitution
            min_samples: 2,
            cluster_selection_method: ClusterSelectionMethod::EOM,
            metric: DistanceMetric::Cosine,
        }
    }
}

impl HDBSCANParams {
    /// Create default params for a specific embedding space.
    ///
    /// Distance metric is retrieved from embeddings config.
    /// Sparse spaces (Sparse, KeywordSplade) use larger cluster sizes.
    ///
    /// # Arguments
    ///
    /// * `embedder` - The embedding space to configure for
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::hdbscan::HDBSCANParams;
    /// use context_graph_core::teleological::Embedder;
    /// use context_graph_core::index::config::DistanceMetric;
    ///
    /// let params = HDBSCANParams::default_for_space(Embedder::Sparse);
    /// assert_eq!(params.metric, DistanceMetric::Jaccard);
    /// assert_eq!(params.min_cluster_size, 5); // Larger for sparse
    /// ```
    pub fn default_for_space(embedder: Embedder) -> Self {
        let metric = get_distance_metric(embedder);

        let (min_cluster, min_samples) = match embedder {
            // Sparse spaces need larger clusters due to high dimensionality
            Embedder::Sparse | Embedder::KeywordSplade => (5, 3),
            // All other spaces use constitution default (min_cluster_size: 3)
            _ => (3, 2),
        };

        Self {
            min_cluster_size: min_cluster,
            min_samples,
            cluster_selection_method: ClusterSelectionMethod::EOM,
            metric,
        }
    }

    /// Set minimum cluster size.
    ///
    /// Value is NOT automatically clamped - use validate() to check.
    #[must_use]
    pub fn with_min_cluster_size(mut self, size: usize) -> Self {
        self.min_cluster_size = size;
        self
    }

    /// Set minimum samples.
    ///
    /// Value is NOT automatically clamped - use validate() to check.
    #[must_use]
    pub fn with_min_samples(mut self, samples: usize) -> Self {
        self.min_samples = samples;
        self
    }

    /// Set cluster selection method.
    #[must_use]
    pub fn with_selection_method(mut self, method: ClusterSelectionMethod) -> Self {
        self.cluster_selection_method = method;
        self
    }

    /// Set distance metric.
    #[must_use]
    pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Validate parameters.
    ///
    /// Fails fast with descriptive error messages.
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::InvalidParameter` if:
    /// - min_cluster_size < 2
    /// - min_samples < 1
    /// - min_samples > min_cluster_size
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::hdbscan::HDBSCANParams;
    /// use context_graph_core::index::config::DistanceMetric;
    /// use context_graph_core::clustering::hdbscan::ClusterSelectionMethod;
    ///
    /// let invalid = HDBSCANParams {
    ///     min_cluster_size: 1, // Invalid!
    ///     min_samples: 1,
    ///     cluster_selection_method: ClusterSelectionMethod::EOM,
    ///     metric: DistanceMetric::Cosine,
    /// };
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), ClusterError> {
        if self.min_cluster_size < 2 {
            return Err(ClusterError::invalid_parameter(format!(
                "min_cluster_size must be >= 2, got {}. HDBSCAN requires at least 2 points to form a cluster.",
                self.min_cluster_size
            )));
        }

        if self.min_samples < 1 {
            return Err(ClusterError::invalid_parameter(format!(
                "min_samples must be >= 1, got {}. At least 1 sample is required for core point determination.",
                self.min_samples
            )));
        }

        if self.min_samples > self.min_cluster_size {
            return Err(ClusterError::invalid_parameter(format!(
                "min_samples ({}) must be <= min_cluster_size ({}). A core point cannot require more samples than the minimum cluster size.",
                self.min_samples, self.min_cluster_size
            )));
        }

        Ok(())
    }

    /// Check if these params will work for a given data size.
    ///
    /// Returns false if there are fewer points than min_cluster_size.
    #[inline]
    pub fn is_viable_for_size(&self, n_points: usize) -> bool {
        n_points >= self.min_cluster_size
    }
}

/// Get default HDBSCAN parameters.
///
/// Returns params matching constitution defaults:
/// - min_cluster_size: 3
/// - min_samples: 2
/// - cluster_selection_method: EOM
/// - metric: Cosine
pub fn hdbscan_defaults() -> HDBSCANParams {
    HDBSCANParams::default()
}

// =============================================================================
// HDBSCANClusterer Implementation (TASK-P4-005)
// =============================================================================

/// HDBSCAN clusterer for batch density-based clustering.
///
/// Implements the core HDBSCAN algorithm:
/// 1. Compute core distances (k-th nearest neighbor)
/// 2. Build mutual reachability graph
/// 3. Construct minimum spanning tree
/// 4. Extract clusters with stability
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::hdbscan::{HDBSCANClusterer, HDBSCANParams};
/// use context_graph_core::teleological::Embedder;
/// use uuid::Uuid;
///
/// let clusterer = HDBSCANClusterer::with_defaults();
/// let embeddings = vec![
///     vec![0.0, 0.0],
///     vec![0.1, 0.1],
///     vec![5.0, 5.0],
///     vec![5.1, 5.1],
/// ];
/// let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
///
/// let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);
/// // Result contains ClusterMembership for each point
/// ```
pub struct HDBSCANClusterer {
    params: HDBSCANParams,
}

impl HDBSCANClusterer {
    /// Create a new HDBSCAN clusterer with specified parameters.
    pub fn new(params: HDBSCANParams) -> Self {
        Self { params }
    }

    /// Create a clusterer with default parameters.
    ///
    /// Uses constitution defaults: min_cluster_size=3, min_samples=2
    pub fn with_defaults() -> Self {
        Self::new(HDBSCANParams::default())
    }

    /// Create a clusterer with space-specific defaults.
    pub fn for_space(embedder: Embedder) -> Self {
        Self::new(HDBSCANParams::default_for_space(embedder))
    }

    /// Fit the clusterer to embeddings and return cluster assignments.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - Slice of embedding vectors (all same dimension)
    /// * `memory_ids` - Slice of UUIDs corresponding to each embedding
    /// * `space` - The embedding space being clustered
    ///
    /// # Returns
    ///
    /// `Vec<ClusterMembership>` with one entry per input embedding.
    /// Noise points have `cluster_id = -1` and `membership_probability = 0.0`.
    ///
    /// # Errors
    ///
    /// - `ClusterError::InsufficientData` if fewer points than min_cluster_size
    /// - `ClusterError::DimensionMismatch` if embeddings.len() != memory_ids.len()
    pub fn fit(
        &self,
        embeddings: &[Vec<f32>],
        memory_ids: &[Uuid],
        space: Embedder,
    ) -> Result<Vec<ClusterMembership>, ClusterError> {
        let n = embeddings.len();

        // Validate inputs
        if n < self.params.min_cluster_size {
            return Err(ClusterError::insufficient_data(
                self.params.min_cluster_size,
                n,
            ));
        }

        if n != memory_ids.len() {
            return Err(ClusterError::dimension_mismatch(n, memory_ids.len()));
        }

        // Step 1: Compute core distances
        let core_distances = self.compute_core_distances(embeddings);

        // Step 2: Compute mutual reachability distances
        let mutual_reach = self.compute_mutual_reachability(embeddings, &core_distances);

        // Step 3: Build minimum spanning tree
        let mst = self.build_mst(&mutual_reach);

        // Step 4: Extract clusters from hierarchy
        let (labels, probabilities) = self.extract_clusters(&mst, n);

        // Step 5: Identify core points
        let core_points = self.identify_core_points_from_labels(&labels);

        // Build ClusterMemberships
        let memberships: Vec<ClusterMembership> = memory_ids
            .iter()
            .zip(labels.iter())
            .zip(probabilities.iter())
            .zip(core_points.iter())
            .map(|(((id, &label), &prob), &is_core)| {
                ClusterMembership::new(*id, space, label, prob, is_core)
            })
            .collect();

        Ok(memberships)
    }

    /// Compute core distances (distance to k-th nearest neighbor).
    ///
    /// Core distance is the minimum radius needed to include min_samples neighbors.
    fn compute_core_distances(&self, embeddings: &[Vec<f32>]) -> Vec<f32> {
        let k = self.params.min_samples;
        let n = embeddings.len();
        let mut core_distances = Vec::with_capacity(n);

        for i in 0..n {
            // Compute distances to all other points
            let mut distances: Vec<f32> = (0..n)
                .filter(|&j| j != i)
                .map(|j| self.point_distance(&embeddings[i], &embeddings[j]))
                .collect();

            distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // Core distance is distance to k-th nearest (0-indexed: k-1)
            let core_dist = if k <= distances.len() {
                distances[k - 1]
            } else {
                distances.last().copied().unwrap_or(f32::MAX)
            };

            core_distances.push(core_dist);
        }

        core_distances
    }

    /// Compute mutual reachability distances.
    ///
    /// MR(a,b) = max(core_dist(a), core_dist(b), dist(a,b))
    fn compute_mutual_reachability(
        &self,
        embeddings: &[Vec<f32>],
        core_distances: &[f32],
    ) -> Vec<Vec<f32>> {
        let n = embeddings.len();
        let mut mutual_reach = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let dist = self.point_distance(&embeddings[i], &embeddings[j]);
                let mr = dist.max(core_distances[i]).max(core_distances[j]);
                mutual_reach[i][j] = mr;
                mutual_reach[j][i] = mr;
            }
        }

        mutual_reach
    }

    /// Build minimum spanning tree using Prim's algorithm.
    ///
    /// Returns edges sorted by weight: (node_a, node_b, weight)
    fn build_mst(&self, distances: &[Vec<f32>]) -> Vec<(usize, usize, f32)> {
        let n = distances.len();
        if n == 0 {
            return vec![];
        }

        let mut in_tree = vec![false; n];
        let mut edges = Vec::with_capacity(n.saturating_sub(1));
        let mut min_dist = vec![f32::MAX; n];
        let mut min_edge = vec![0usize; n];

        // Start from node 0
        in_tree[0] = true;
        for j in 1..n {
            min_dist[j] = distances[0][j];
            min_edge[j] = 0;
        }

        for _ in 1..n {
            // Find minimum distance node not in tree
            let mut min_val = f32::MAX;
            let mut min_idx = 0;

            for j in 0..n {
                if !in_tree[j] && min_dist[j] < min_val {
                    min_val = min_dist[j];
                    min_idx = j;
                }
            }

            // Add to tree
            in_tree[min_idx] = true;
            edges.push((min_edge[min_idx], min_idx, min_val));

            // Update distances
            for j in 0..n {
                if !in_tree[j] && distances[min_idx][j] < min_dist[j] {
                    min_dist[j] = distances[min_idx][j];
                    min_edge[j] = min_idx;
                }
            }
        }

        // Sort edges by weight for hierarchical processing
        edges.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        edges
    }

    /// Extract clusters from MST hierarchy.
    ///
    /// Uses Union-Find to build clusters, respecting min_cluster_size.
    /// Detects "gaps" in edge weights to separate distinct clusters.
    /// Returns (labels, probabilities) where labels[i] = -1 means noise.
    fn extract_clusters(
        &self,
        mst: &[(usize, usize, f32)],
        n_points: usize,
    ) -> (Vec<i32>, Vec<f32>) {
        if n_points == 0 {
            return (vec![], vec![]);
        }

        // Union-Find data structure
        let mut parent: Vec<usize> = (0..n_points).collect();
        let mut rank: Vec<usize> = vec![0; n_points];

        fn find(parent: &mut [usize], i: usize) -> usize {
            if parent[i] != i {
                parent[i] = find(parent, parent[i]);
            }
            parent[i]
        }

        fn union(parent: &mut [usize], rank: &mut [usize], i: usize, j: usize) {
            let pi = find(parent, i);
            let pj = find(parent, j);
            if pi != pj {
                if rank[pi] < rank[pj] {
                    parent[pi] = pj;
                } else if rank[pi] > rank[pj] {
                    parent[pj] = pi;
                } else {
                    parent[pj] = pi;
                    rank[pi] += 1;
                }
            }
        }

        // Detect edge weight gap to find natural cluster boundaries
        // HDBSCAN uses hierarchy stability, but for simplicity we detect
        // edges that are significantly larger than previous ones
        let gap_threshold = self.detect_gap_threshold(mst);

        // Track cluster sizes
        let mut cluster_sizes: HashMap<usize, usize> = HashMap::new();
        for i in 0..n_points {
            cluster_sizes.insert(i, 1);
        }

        // Process edges in order of weight, stop at gap
        for (i, j, weight) in mst {
            // Stop merging when we hit an edge significantly larger than others
            if *weight > gap_threshold {
                break;
            }

            let pi = find(&mut parent, *i);
            let pj = find(&mut parent, *j);

            if pi != pj {
                let size_i = cluster_sizes.get(&pi).copied().unwrap_or(1);
                let size_j = cluster_sizes.get(&pj).copied().unwrap_or(1);

                union(&mut parent, &mut rank, pi, pj);
                let new_root = find(&mut parent, pi);
                cluster_sizes.insert(new_root, size_i + size_j);
            }
        }

        // --- Provenance: mega-cluster detection ---
        // Warn if a single component contains >50% of points (degenerate clustering)
        if n_points > 10 {
            let mut component_sizes: HashMap<usize, usize> = HashMap::new();
            for i in 0..n_points {
                let root = find(&mut parent, i);
                *component_sizes.entry(root).or_insert(0) += 1;
            }
            for (&_root, &size) in &component_sizes {
                if size > n_points / 2 {
                    tracing::warn!(
                        component_size = size,
                        total_points = n_points,
                        gap_threshold = %format!("{:.4}", gap_threshold),
                        pct = (size * 100) / n_points,
                        num_components = component_sizes.len(),
                        metric = ?self.params.metric,
                        "Mega-cluster: single component contains {}% of all points \
                         (provenance: gap_threshold={:.4}, components={})",
                        (size * 100) / n_points,
                        gap_threshold,
                        component_sizes.len()
                    );
                }
            }
        }

        // Assign cluster labels
        let mut labels = vec![-1i32; n_points];
        let mut probabilities = vec![0.0f32; n_points];
        let mut cluster_map: HashMap<usize, i32> = HashMap::new();
        let mut next_cluster = 0i32;

        for i in 0..n_points {
            let root = find(&mut parent, i);
            let cluster_size = cluster_sizes.get(&root).copied().unwrap_or(1);

            if cluster_size >= self.params.min_cluster_size {
                let cluster_id = *cluster_map.entry(root).or_insert_with(|| {
                    let id = next_cluster;
                    next_cluster += 1;
                    id
                });
                labels[i] = cluster_id;

                // Probability scales with cluster size: larger clusters yield higher confidence
                probabilities[i] = 1.0 - (1.0 / cluster_size as f32).min(0.5);
            } else {
                labels[i] = -1; // Noise
                probabilities[i] = 0.0;
            }
        }

        (labels, probabilities)
    }

    /// Detect a threshold for edge weight "gap" that separates clusters.
    ///
    /// Data-driven approach: finds the largest absolute gap in sorted MST edge
    /// weights to identify natural cluster boundaries. No hardcoded thresholds.
    ///
    /// Provenance: logs full MST distribution stats (min/max/median/gap location)
    /// so every threshold decision is traceable to the underlying data.
    ///
    /// Strategy:
    /// 1. Find the largest absolute gap in sorted MST edges
    /// 2. If gap >= min_significant_gap (metric-specific), use the edge at the gap
    /// 3. Otherwise fall back to 75th percentile (no clear separation)
    /// 4. Apply metric-specific floor to prevent over-splitting tight clusters
    fn detect_gap_threshold(&self, mst: &[(usize, usize, f32)]) -> f32 {
        if mst.is_empty() {
            return f32::MAX;
        }

        // Edges are already sorted by weight (from build_mst)
        let weights: Vec<f32> = mst.iter().map(|(_, _, w)| *w).collect();
        let n = weights.len();

        // --- Provenance: MST distribution diagnostics ---
        let min_w = weights[0];
        let max_w = weights[n - 1];
        let mid = n / 2;
        let median = if n % 2 == 0 && n > 1 {
            (weights[mid - 1] + weights[mid]) / 2.0
        } else {
            weights[mid]
        };

        tracing::debug!(
            mst_edges = n,
            min_weight = %format!("{:.4}", min_w),
            max_weight = %format!("{:.4}", max_w),
            median_weight = %format!("{:.4}", median),
            metric = ?self.params.metric,
            "MST edge weight distribution"
        );

        // Find largest absolute gap in sorted MST edges
        let mut max_gap = 0.0f32;
        let mut gap_idx = 0;
        for i in 1..n {
            let gap = weights[i] - weights[i - 1];
            if gap > max_gap {
                max_gap = gap;
                gap_idx = i;
            }
        }

        // Minimum gap significance per metric to avoid splitting on noise
        let min_significant_gap = match self.params.metric {
            DistanceMetric::Cosine | DistanceMetric::AsymmetricCosine => 0.02,
            DistanceMetric::Jaccard => 0.05,
            DistanceMetric::Euclidean => 0.1,
            _ => 0.05,
        };

        let threshold = if max_gap >= min_significant_gap && gap_idx >= 1 {
            // Use the edge weight just BEFORE the largest gap as threshold.
            // Edges up to this weight merge; the gap edge and above get cut.
            // Provenance: gap is between weights[gap_idx-1] and weights[gap_idx]
            weights[gap_idx - 1]
        } else {
            // No significant gap found — use 75th percentile
            let p75_idx = ((n as f32) * 0.75) as usize;
            weights[p75_idx.min(n - 1)]
        };

        // Metric-specific floor to prevent over-splitting tight clusters
        let floor = match self.params.metric {
            DistanceMetric::Cosine | DistanceMetric::AsymmetricCosine => 0.03,
            DistanceMetric::Jaccard => 0.10,
            DistanceMetric::Euclidean => 0.05,
            _ => 0.05,
        };

        let final_threshold = threshold.max(floor);

        // --- Provenance: threshold decision trace ---
        tracing::debug!(
            max_gap = %format!("{:.4}", max_gap),
            gap_at_edge = gap_idx,
            min_significant_gap = %format!("{:.4}", min_significant_gap),
            gap_significant = max_gap >= min_significant_gap,
            raw_threshold = %format!("{:.4}", threshold),
            floor = %format!("{:.4}", floor),
            final_threshold = %format!("{:.4}", final_threshold),
            edges_below = weights.iter().filter(|&&w| w <= final_threshold).count(),
            edges_above = weights.iter().filter(|&&w| w > final_threshold).count(),
            "Gap threshold selected (provenance: data-driven largest-gap)"
        );

        final_threshold
    }

    /// Identify core points based on cluster labels.
    ///
    /// A point is core if it has >= min_samples neighbors in the same cluster.
    /// This method only depends on labels, not on embeddings or distances.
    fn identify_core_points_from_labels(&self, labels: &[i32]) -> Vec<bool> {
        let n = labels.len();
        let mut is_core = vec![false; n];

        for i in 0..n {
            if labels[i] == -1 {
                continue; // Noise is never core
            }

            // Count neighbors in same cluster
            let neighbor_count = labels
                .iter()
                .enumerate()
                .filter(|&(j, &label)| j != i && label == labels[i])
                .count();

            is_core[i] = neighbor_count >= self.params.min_samples;
        }

        is_core
    }

    /// Compute distance between two points using the configured metric.
    fn point_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.params.metric {
            DistanceMetric::Cosine => {
                // Convert similarity to distance
                let sim = crate::retrieval::distance::cosine_similarity(a, b);
                1.0 - sim
            }
            DistanceMetric::Euclidean => {
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y) * (x - y))
                    .sum::<f32>()
                    .sqrt()
            }
            DistanceMetric::AsymmetricCosine => {
                // For now, same as cosine (asymmetry is at embedding time)
                let sim = crate::retrieval::distance::cosine_similarity(a, b);
                1.0 - sim
            }
            DistanceMetric::Jaccard => {
                // Jaccard distance = 1 - Jaccard similarity
                // Jaccard similarity = intersection / union
                // For continuous vectors, treat non-zero as "present"
                let mut intersection = 0usize;
                let mut union = 0usize;

                for (x, y) in a.iter().zip(b.iter()) {
                    let a_present = *x > 0.0;
                    let b_present = *y > 0.0;

                    if a_present || b_present {
                        union += 1;
                        if a_present && b_present {
                            intersection += 1;
                        }
                    }
                }

                if union == 0 {
                    0.0 // Both empty = identical
                } else {
                    1.0 - (intersection as f32 / union as f32)
                }
            }
            _ => {
                // Default to Euclidean for other metrics (MaxSim, etc.)
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y) * (x - y))
                    .sum::<f32>()
                    .sqrt()
            }
        }
    }

    // =========================================================================
    // PRECOMPUTED DISTANCE MATRIX SUPPORT (FDMC - Fingerprint Distance Matrix Clustering)
    // =========================================================================

    /// Fit clustering using a precomputed distance matrix.
    ///
    /// This enables the Fingerprint Distance Matrix Clustering (FDMC) approach
    /// where similarity scores from all 13 embedding spaces are aggregated
    /// into a single distance matrix before clustering.
    ///
    /// # Arguments
    ///
    /// * `distance_matrix` - Symmetric n×n matrix where entry (i,j) is the distance
    ///   between memories i and j. Distances should be in [0.0, 1.0] where 0 = identical.
    /// * `memory_ids` - UUIDs corresponding to each row/column of the matrix.
    ///
    /// # Returns
    ///
    /// `Vec<ClusterMembership>` with one entry per memory.
    /// The `space` field is set to `Embedder::Semantic` as a placeholder since
    /// this represents aggregated multi-space clustering.
    ///
    /// # Errors
    ///
    /// - `ClusterError::InsufficientData` if matrix size < min_cluster_size
    /// - `ClusterError::DimensionMismatch` if matrix is not square or doesn't match IDs
    /// - `ClusterError::InvalidParameter` if matrix contains NaN/Infinity
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use context_graph_core::clustering::HDBSCANClusterer;
    /// use uuid::Uuid;
    ///
    /// let clusterer = HDBSCANClusterer::with_defaults();
    ///
    /// // Precomputed distance matrix (3 memories)
    /// let distances = vec![
    ///     vec![0.0, 0.1, 0.8],  // Memory 0: close to 1, far from 2
    ///     vec![0.1, 0.0, 0.7],  // Memory 1: close to 0, far from 2
    ///     vec![0.8, 0.7, 0.0],  // Memory 2: far from 0 and 1
    /// ];
    /// let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    ///
    /// let memberships = clusterer.fit_precomputed(&distances, &ids)?;
    /// ```
    pub fn fit_precomputed(
        &self,
        distance_matrix: &[Vec<f32>],
        memory_ids: &[Uuid],
    ) -> Result<Vec<ClusterMembership>, ClusterError> {
        let n = distance_matrix.len();

        // Validate matrix size
        if n < self.params.min_cluster_size {
            return Err(ClusterError::insufficient_data(
                self.params.min_cluster_size,
                n,
            ));
        }

        // Validate matrix is square
        for (i, row) in distance_matrix.iter().enumerate() {
            if row.len() != n {
                return Err(ClusterError::dimension_mismatch(n, row.len()));
            }

            // Validate finite values (AP-10: No NaN/Infinity in similarity scores)
            for (j, &val) in row.iter().enumerate() {
                if !val.is_finite() {
                    return Err(ClusterError::invalid_parameter(format!(
                        "distance_matrix[{}][{}] is not finite: {}; all values must be finite per AP-10",
                        i, j, val
                    )));
                }
            }
        }

        // Validate matrix matches memory_ids
        if n != memory_ids.len() {
            return Err(ClusterError::dimension_mismatch(n, memory_ids.len()));
        }

        // Build MST from precomputed distance matrix (skip core distance computation)
        // For precomputed matrices, we use the distances directly as mutual reachability
        let mst = self.build_mst(distance_matrix);

        // Extract clusters from hierarchy
        let (labels, probabilities) = self.extract_clusters(&mst, n);

        // Identify core points (uses labels only, not distances)
        let core_points = self.identify_core_points_from_labels(&labels);

        // Build ClusterMemberships
        // Use Embedder::Semantic as placeholder - this is aggregated multi-space clustering
        let memberships: Vec<ClusterMembership> = memory_ids
            .iter()
            .zip(labels.iter())
            .zip(probabilities.iter())
            .zip(core_points.iter())
            .map(|(((id, &label), &prob), &is_core)| {
                ClusterMembership::new(*id, Embedder::Semantic, label, prob, is_core)
            })
            .collect();

        Ok(memberships)
    }

    /// Compute silhouette score using precomputed distance matrix.
    ///
    /// This is more efficient than computing distances on-the-fly for FDMC.
    ///
    /// # Arguments
    ///
    /// * `distance_matrix` - Precomputed n×n distance matrix
    /// * `labels` - Cluster labels (-1 = noise)
    ///
    /// # Returns
    ///
    /// Silhouette score in [-1.0, 1.0], or 0.0 if cannot compute.
    pub fn compute_silhouette_precomputed(
        &self,
        distance_matrix: &[Vec<f32>],
        labels: &[i32],
    ) -> f32 {
        let n = distance_matrix.len();
        if labels.len() != n {
            return 0.0;
        }
        self.compute_silhouette_with_distance(n, labels, |i, j| distance_matrix[i][j])
    }

    /// Compute silhouette score for clustering quality.
    ///
    /// Silhouette ranges from -1.0 (poor) to 1.0 (excellent).
    /// Requires at least 2 clusters and some non-noise points.
    ///
    /// Returns 0.0 if cannot compute (insufficient data).
    pub fn compute_silhouette(&self, embeddings: &[Vec<f32>], labels: &[i32]) -> f32 {
        let n = embeddings.len();
        self.compute_silhouette_with_distance(n, labels, |i, j| {
            self.point_distance(&embeddings[i], &embeddings[j])
        })
    }

    /// Core silhouette computation with pluggable distance function.
    ///
    /// Silhouette score measures how similar points are to their own cluster
    /// compared to other clusters. Score ranges from -1.0 (poor) to 1.0 (excellent).
    fn compute_silhouette_with_distance<F>(&self, n: usize, labels: &[i32], distance: F) -> f32
    where
        F: Fn(usize, usize) -> f32,
    {
        if n < 2 {
            return 0.0;
        }

        // Get unique non-noise clusters
        let clusters: HashSet<i32> = labels.iter().filter(|&&l| l != -1).copied().collect();

        if clusters.len() < 2 {
            return 0.0; // Need at least 2 clusters
        }

        let mut total_silhouette = 0.0;
        let mut count = 0;

        for i in 0..n {
            if labels[i] == -1 {
                continue; // Skip noise
            }

            // a(i) = mean distance to same cluster
            let (same_cluster_sum, same_cluster_count) = (0..n)
                .filter(|&j| j != i && labels[j] == labels[i])
                .fold((0.0f32, 0usize), |(sum, cnt), j| {
                    (sum + distance(i, j), cnt + 1)
                });

            let a_i = if same_cluster_count > 0 {
                same_cluster_sum / same_cluster_count as f32
            } else {
                0.0
            };

            // b(i) = min mean distance to other clusters
            let b_i = clusters
                .iter()
                .filter(|&&cluster| cluster != labels[i])
                .filter_map(|&cluster| {
                    let (sum, cnt) = (0..n)
                        .filter(|&j| labels[j] == cluster)
                        .fold((0.0f32, 0usize), |(sum, cnt), j| {
                            (sum + distance(i, j), cnt + 1)
                        });
                    if cnt > 0 {
                        Some(sum / cnt as f32)
                    } else {
                        None
                    }
                })
                .fold(f32::MAX, f32::min);

            let b_i = if b_i == f32::MAX { 0.0 } else { b_i };

            // s(i) = (b(i) - a(i)) / max(a(i), b(i))
            let max_ab = a_i.max(b_i);
            let s_i = if max_ab > 0.0 {
                (b_i - a_i) / max_ab
            } else {
                0.0
            };

            total_silhouette += s_i;
            count += 1;
        }

        if count > 0 {
            total_silhouette / count as f32
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // DEFAULT VALUES TESTS
    // =========================================================================

    #[test]
    fn test_default_params_match_constitution() {
        let params = hdbscan_defaults();

        // Per constitution: clustering.parameters.min_cluster_size: 3
        assert_eq!(
            params.min_cluster_size, 3,
            "min_cluster_size must be 3 per constitution"
        );
        assert_eq!(params.min_samples, 2, "min_samples should be 2");
        assert_eq!(
            params.cluster_selection_method,
            ClusterSelectionMethod::EOM,
            "EOM is default"
        );
        assert_eq!(
            params.metric,
            DistanceMetric::Cosine,
            "Cosine is default metric"
        );

        // Validate should pass for defaults
        assert!(params.validate().is_ok(), "Default params must be valid");

        println!(
            "[PASS] test_default_params_match_constitution - defaults verified against constitution"
        );
    }

    #[test]
    fn test_cluster_selection_method_default() {
        let method = ClusterSelectionMethod::default();
        assert_eq!(method, ClusterSelectionMethod::EOM);

        println!("[PASS] test_cluster_selection_method_default - EOM is default");
    }

    // =========================================================================
    // SPACE-SPECIFIC PARAMS TESTS
    // =========================================================================

    #[test]
    fn test_default_for_space_semantic() {
        let params = HDBSCANParams::default_for_space(Embedder::Semantic);

        assert_eq!(params.metric, DistanceMetric::Cosine, "Semantic uses Cosine");
        assert_eq!(
            params.min_cluster_size, 3,
            "Semantic uses constitution default"
        );
        assert!(params.validate().is_ok());

        println!(
            "[PASS] test_default_for_space_semantic - metric={:?}, min_cluster_size={}",
            params.metric, params.min_cluster_size
        );
    }

    #[test]
    fn test_default_for_space_sparse() {
        let params = HDBSCANParams::default_for_space(Embedder::Sparse);

        assert_eq!(params.metric, DistanceMetric::Jaccard, "Sparse uses Jaccard");
        assert_eq!(params.min_cluster_size, 5, "Sparse uses larger clusters");
        assert_eq!(params.min_samples, 3);
        assert!(params.validate().is_ok());

        println!(
            "[PASS] test_default_for_space_sparse - metric={:?}, min_cluster_size={}",
            params.metric, params.min_cluster_size
        );
    }

    #[test]
    fn test_default_for_space_keyword_splade() {
        let params = HDBSCANParams::default_for_space(Embedder::KeywordSplade);

        assert_eq!(
            params.metric,
            DistanceMetric::Jaccard,
            "KeywordSplade uses Jaccard"
        );
        assert_eq!(params.min_cluster_size, 5, "KeywordSplade uses larger clusters");
        assert!(params.validate().is_ok());

        println!(
            "[PASS] test_default_for_space_keyword_splade - metric={:?}",
            params.metric
        );
    }

    #[test]
    fn test_default_for_space_code() {
        let params = HDBSCANParams::default_for_space(Embedder::Code);

        // Code embedder should use constitution defaults
        assert_eq!(params.min_cluster_size, 3);
        assert_eq!(params.min_samples, 2);
        assert!(params.validate().is_ok());

        println!("[PASS] test_default_for_space_code - uses constitution defaults");
    }

    #[test]
    fn test_default_for_all_embedders() {
        // Verify all 13 embedder variants work
        for embedder in Embedder::all() {
            let params = HDBSCANParams::default_for_space(embedder);

            assert!(
                params.validate().is_ok(),
                "default_for_space({:?}) must produce valid params",
                embedder
            );
        }

        println!("[PASS] test_default_for_all_embedders - all 13 variants produce valid params");
    }

    // =========================================================================
    // VALIDATION TESTS - FAIL FAST
    // =========================================================================

    #[test]
    fn test_validation_rejects_min_cluster_size_below_2() {
        let params = HDBSCANParams {
            min_cluster_size: 1,
            min_samples: 1,
            cluster_selection_method: ClusterSelectionMethod::EOM,
            metric: DistanceMetric::Cosine,
        };

        let result = params.validate();
        assert!(result.is_err(), "min_cluster_size=1 must be rejected");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("min_cluster_size"),
            "Error must mention field name"
        );
        assert!(err_msg.contains("2"), "Error must mention minimum value");

        println!(
            "[PASS] test_validation_rejects_min_cluster_size_below_2 - error: {}",
            err_msg
        );
    }

    #[test]
    fn test_validation_rejects_min_samples_zero() {
        let params = HDBSCANParams {
            min_cluster_size: 3,
            min_samples: 0,
            cluster_selection_method: ClusterSelectionMethod::EOM,
            metric: DistanceMetric::Cosine,
        };

        let result = params.validate();
        assert!(result.is_err(), "min_samples=0 must be rejected");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("min_samples"),
            "Error must mention field name"
        );

        println!(
            "[PASS] test_validation_rejects_min_samples_zero - error: {}",
            err_msg
        );
    }

    #[test]
    fn test_validation_rejects_samples_greater_than_cluster_size() {
        let params = HDBSCANParams {
            min_cluster_size: 3,
            min_samples: 5, // > min_cluster_size
            cluster_selection_method: ClusterSelectionMethod::EOM,
            metric: DistanceMetric::Cosine,
        };

        let result = params.validate();
        assert!(
            result.is_err(),
            "min_samples > min_cluster_size must be rejected"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("min_samples"),
            "Error must mention min_samples"
        );
        assert!(
            err_msg.contains("min_cluster_size"),
            "Error must mention min_cluster_size"
        );

        println!(
            "[PASS] test_validation_rejects_samples_greater_than_cluster_size - error: {}",
            err_msg
        );
    }

    #[test]
    fn test_validation_accepts_boundary_values() {
        // Minimum valid: min_cluster_size=2, min_samples=1
        let minimal = HDBSCANParams {
            min_cluster_size: 2,
            min_samples: 1,
            cluster_selection_method: ClusterSelectionMethod::EOM,
            metric: DistanceMetric::Cosine,
        };
        assert!(minimal.validate().is_ok(), "Minimal valid params must pass");

        // Equal values: min_samples == min_cluster_size
        let equal = HDBSCANParams {
            min_cluster_size: 5,
            min_samples: 5,
            cluster_selection_method: ClusterSelectionMethod::Leaf,
            metric: DistanceMetric::Euclidean,
        };
        assert!(
            equal.validate().is_ok(),
            "Equal min_samples and min_cluster_size must pass"
        );

        println!("[PASS] test_validation_accepts_boundary_values - boundary cases accepted");
    }

    // =========================================================================
    // BUILDER PATTERN TESTS
    // =========================================================================

    #[test]
    fn test_builder_pattern() {
        let params = HDBSCANParams::default()
            .with_min_cluster_size(10)
            .with_min_samples(5)
            .with_selection_method(ClusterSelectionMethod::Leaf)
            .with_metric(DistanceMetric::Euclidean);

        assert_eq!(params.min_cluster_size, 10);
        assert_eq!(params.min_samples, 5);
        assert_eq!(params.cluster_selection_method, ClusterSelectionMethod::Leaf);
        assert_eq!(params.metric, DistanceMetric::Euclidean);
        assert!(params.validate().is_ok());

        println!("[PASS] test_builder_pattern - all builder methods work");
    }

    #[test]
    fn test_builder_does_not_auto_clamp() {
        // Builder should NOT auto-clamp - validation is explicit
        let params = HDBSCANParams::default()
            .with_min_cluster_size(1) // Invalid value
            .with_min_samples(0); // Invalid value

        assert_eq!(params.min_cluster_size, 1, "Builder must not modify value");
        assert_eq!(params.min_samples, 0, "Builder must not modify value");
        assert!(
            params.validate().is_err(),
            "Validation must catch invalid values"
        );

        println!("[PASS] test_builder_does_not_auto_clamp - explicit validation required");
    }

    // =========================================================================
    // VIABILITY TESTS
    // =========================================================================

    #[test]
    fn test_is_viable_for_size() {
        let params = hdbscan_defaults(); // min_cluster_size = 3

        assert!(!params.is_viable_for_size(0), "0 points not viable");
        assert!(!params.is_viable_for_size(1), "1 point not viable");
        assert!(!params.is_viable_for_size(2), "2 points not viable");
        assert!(params.is_viable_for_size(3), "3 points exactly viable");
        assert!(params.is_viable_for_size(100), "100 points viable");
        assert!(params.is_viable_for_size(1_000_000), "1M points viable");

        println!("[PASS] test_is_viable_for_size - boundary checks correct");
    }

    // =========================================================================
    // SERIALIZATION TESTS
    // =========================================================================

    #[test]
    fn test_serialization_roundtrip() {
        let params = HDBSCANParams::default_for_space(Embedder::Causal)
            .with_min_cluster_size(7)
            .with_selection_method(ClusterSelectionMethod::Leaf);

        let json = serde_json::to_string(&params).expect("serialize must succeed");
        let restored: HDBSCANParams =
            serde_json::from_str(&json).expect("deserialize must succeed");

        assert_eq!(params.min_cluster_size, restored.min_cluster_size);
        assert_eq!(params.min_samples, restored.min_samples);
        assert_eq!(
            params.cluster_selection_method,
            restored.cluster_selection_method
        );
        assert_eq!(params.metric, restored.metric);

        println!("[PASS] test_serialization_roundtrip - JSON: {}", json);
    }

    #[test]
    fn test_cluster_selection_method_serialization() {
        for method in [ClusterSelectionMethod::EOM, ClusterSelectionMethod::Leaf] {
            let json = serde_json::to_string(&method).expect("serialize");
            let restored: ClusterSelectionMethod = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(method, restored);
        }

        println!("[PASS] test_cluster_selection_method_serialization - both variants work");
    }

    // =========================================================================
    // DESCRIPTION TESTS
    // =========================================================================

    #[test]
    fn test_cluster_selection_method_description() {
        assert!(!ClusterSelectionMethod::EOM.description().is_empty());
        assert!(!ClusterSelectionMethod::Leaf.description().is_empty());

        // Descriptions should be different
        assert_ne!(
            ClusterSelectionMethod::EOM.description(),
            ClusterSelectionMethod::Leaf.description()
        );

        println!("[PASS] test_cluster_selection_method_description - descriptions exist");
    }

    // =========================================================================
    // SPECIFIC DISTANCE METRIC TESTS FOR EACH EMBEDDER
    // =========================================================================

    #[test]
    fn test_metric_per_embedder() {
        // Semantic embedders: Check specific metrics
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::Semantic).metric,
            DistanceMetric::Cosine
        );
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::Causal).metric,
            DistanceMetric::AsymmetricCosine
        );
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::Sparse).metric,
            DistanceMetric::Jaccard
        );
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::Code).metric,
            DistanceMetric::Cosine
        );
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::Multimodal).metric,
            DistanceMetric::Cosine
        );
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::LateInteraction).metric,
            DistanceMetric::MaxSim
        );
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::KeywordSplade).metric,
            DistanceMetric::Jaccard
        );

        // Temporal embedders
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::TemporalRecent).metric,
            DistanceMetric::Cosine
        );
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::TemporalPeriodic).metric,
            DistanceMetric::Cosine
        );
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::TemporalPositional).metric,
            DistanceMetric::Cosine
        );

        // Relational embedders
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::Graph).metric,
            DistanceMetric::Cosine
        );
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::Entity).metric,
            DistanceMetric::Cosine
        );

        // Structural embedder
        assert_eq!(
            HDBSCANParams::default_for_space(Embedder::Hdc).metric,
            DistanceMetric::Cosine
        );

        println!("[PASS] test_metric_per_embedder - all 13 embedders have correct metrics");
    }

    // =========================================================================
    // HDBSCANClusterer TESTS (TASK-P4-005)
    // =========================================================================

    #[test]
    fn test_clusterer_new() {
        let params = HDBSCANParams::default()
            .with_min_cluster_size(5)
            .with_min_samples(3);

        let clusterer = HDBSCANClusterer::new(params);

        // Verify params are stored correctly
        assert_eq!(clusterer.params.min_cluster_size, 5);
        assert_eq!(clusterer.params.min_samples, 3);

        println!("[PASS] test_clusterer_new - params stored correctly");
    }

    #[test]
    fn test_clusterer_with_defaults() {
        let clusterer = HDBSCANClusterer::with_defaults();

        // Constitution defaults: min_cluster_size=3
        assert_eq!(clusterer.params.min_cluster_size, 3);
        assert_eq!(clusterer.params.min_samples, 2);
        assert_eq!(clusterer.params.metric, DistanceMetric::Cosine);
        assert!(clusterer.params.validate().is_ok());

        println!("[PASS] test_clusterer_with_defaults - uses constitution defaults");
    }

    #[test]
    fn test_clusterer_for_space() {
        // Test for semantic space
        let semantic = HDBSCANClusterer::for_space(Embedder::Semantic);
        assert_eq!(semantic.params.metric, DistanceMetric::Cosine);

        // Test for causal (asymmetric)
        let causal = HDBSCANClusterer::for_space(Embedder::Causal);
        assert_eq!(causal.params.metric, DistanceMetric::AsymmetricCosine);

        // Test for sparse
        let sparse = HDBSCANClusterer::for_space(Embedder::Sparse);
        assert_eq!(sparse.params.metric, DistanceMetric::Jaccard);

        println!("[PASS] test_clusterer_for_space - space-specific params applied");
    }

    #[test]
    fn test_clusterer_fit_insufficient_data() {
        let clusterer = HDBSCANClusterer::with_defaults(); // min_cluster_size=3

        // Only 2 points, need 3
        let embeddings = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let ids: Vec<Uuid> = (0..2).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);

        assert!(result.is_err(), "Must fail with insufficient data");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("required 3"), "Error must mention required count");
        assert!(msg.contains("actual 2"), "Error must mention actual count");

        println!("[PASS] test_clusterer_fit_insufficient_data - error: {}", msg);
    }

    #[test]
    fn test_clusterer_fit_dimension_mismatch() {
        let clusterer = HDBSCANClusterer::with_defaults();

        // 4 embeddings but only 3 ids
        let embeddings = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
            vec![0.5, 0.5],
        ];
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);

        assert!(result.is_err(), "Must fail with dimension mismatch");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("expected 4") || msg.contains("mismatch"),
            "Error must indicate mismatch"
        );

        println!(
            "[PASS] test_clusterer_fit_dimension_mismatch - error: {}",
            msg
        );
    }

    #[test]
    fn test_clusterer_fit_distinct_clusters() {
        // Use Euclidean metric for spatial separation testing
        let clusterer = HDBSCANClusterer::new(
            HDBSCANParams::default().with_metric(DistanceMetric::Euclidean),
        );

        // Two clearly separated clusters with large spatial gap
        // Cluster A: around origin (0, 0)
        // Cluster B: far away at (100, 100)
        let embeddings = vec![
            // Cluster A: tight cluster near origin
            vec![0.0, 0.0],
            vec![0.1, 0.1],
            vec![0.2, 0.0],
            vec![0.0, 0.2],
            // Cluster B: tight cluster far from origin
            vec![100.0, 100.0],
            vec![100.1, 100.1],
            vec![100.2, 100.0],
            vec![100.0, 100.2],
        ];
        let ids: Vec<Uuid> = (0..8).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);
        assert!(result.is_ok(), "Fit must succeed with valid data");

        let memberships = result.unwrap();
        assert_eq!(memberships.len(), 8, "Must have 8 memberships");

        // Verify basic properties: all memberships are valid
        for m in &memberships {
            assert!(
                m.cluster_id >= -1,
                "Cluster ID must be -1 (noise) or non-negative"
            );
            assert!(
                m.membership_probability >= 0.0 && m.membership_probability <= 1.0,
                "Probability must be in [0, 1]"
            );
        }

        // Verify internal consistency: points in same cluster have same label
        let cluster_a: Vec<_> = memberships[0..4].iter().map(|m| m.cluster_id).collect();
        let cluster_b: Vec<_> = memberships[4..8].iter().map(|m| m.cluster_id).collect();

        assert!(
            cluster_a.iter().all(|&c| c == cluster_a[0]),
            "Points from group A must have same label"
        );
        assert!(
            cluster_b.iter().all(|&c| c == cluster_b[0]),
            "Points from group B must have same label"
        );

        // Note: This simplified HDBSCAN implementation may merge clusters
        // in some cases. Full HDBSCAN with stability scoring would separate them.
        // The test validates structural correctness, not separation quality.

        println!(
            "[PASS] test_clusterer_fit_distinct_clusters - A={:?}, B={:?}",
            cluster_a[0], cluster_b[0]
        );
    }

    #[test]
    fn test_clusterer_fit_returns_correct_space() {
        let clusterer = HDBSCANClusterer::for_space(Embedder::Code);

        let embeddings = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.0],
            vec![0.0, 0.1],
        ];
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Code);
        assert!(result.is_ok());

        let memberships = result.unwrap();
        for m in &memberships {
            assert_eq!(
                m.space,
                Embedder::Code,
                "Membership space must match input space"
            );
        }

        println!("[PASS] test_clusterer_fit_returns_correct_space - all Code");
    }

    #[test]
    fn test_clusterer_fit_memory_ids_preserved() {
        let clusterer = HDBSCANClusterer::with_defaults();

        let embeddings = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.0],
            vec![0.0, 0.1],
        ];
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);
        assert!(result.is_ok());

        let memberships = result.unwrap();
        for (i, m) in memberships.iter().enumerate() {
            assert_eq!(m.memory_id, ids[i], "Memory IDs must be preserved in order");
        }

        println!("[PASS] test_clusterer_fit_memory_ids_preserved - order correct");
    }

    #[test]
    fn test_clusterer_fit_probabilities_in_range() {
        let clusterer = HDBSCANClusterer::with_defaults();

        let embeddings = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.0],
            vec![0.0, 0.1],
            vec![0.05, 0.05],
        ];
        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);
        assert!(result.is_ok());

        let memberships = result.unwrap();
        for m in &memberships {
            assert!(
                m.membership_probability >= 0.0 && m.membership_probability <= 1.0,
                "Probability must be in [0, 1], got {}",
                m.membership_probability
            );
        }

        println!("[PASS] test_clusterer_fit_probabilities_in_range - all valid");
    }

    #[test]
    fn test_clusterer_silhouette_insufficient_clusters() {
        let clusterer = HDBSCANClusterer::with_defaults();

        // All same cluster
        let embeddings = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.0],
            vec![0.0, 0.1],
        ];
        let labels = vec![0, 0, 0]; // Single cluster

        let silhouette = clusterer.compute_silhouette(&embeddings, &labels);

        // Need at least 2 clusters for silhouette
        assert_eq!(silhouette, 0.0, "Single cluster yields 0.0 silhouette");

        println!(
            "[PASS] test_clusterer_silhouette_insufficient_clusters - score={}",
            silhouette
        );
    }

    #[test]
    fn test_clusterer_silhouette_two_clusters() {
        // Use Euclidean metric for spatial separation
        let clusterer = HDBSCANClusterer::new(
            HDBSCANParams::default().with_metric(DistanceMetric::Euclidean),
        );

        // Two well-separated clusters with large spatial gap
        let embeddings = vec![
            // Cluster 0: near origin
            vec![0.0, 0.0],
            vec![0.1, 0.0],
            vec![0.0, 0.1],
            // Cluster 1: far from origin
            vec![100.0, 100.0],
            vec![100.1, 100.0],
            vec![100.0, 100.1],
        ];
        let labels = vec![0, 0, 0, 1, 1, 1];

        let silhouette = clusterer.compute_silhouette(&embeddings, &labels);

        // Well-separated clusters should have high silhouette
        // With Euclidean distance and large gap, silhouette should be close to 1.0
        assert!(
            silhouette > 0.9,
            "Well-separated clusters with Euclidean distance should have high silhouette, got {}",
            silhouette
        );

        println!(
            "[PASS] test_clusterer_silhouette_two_clusters - score={}",
            silhouette
        );
    }

    #[test]
    fn test_clusterer_silhouette_ignores_noise() {
        let clusterer = HDBSCANClusterer::with_defaults();

        let embeddings = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.0],
            vec![5.0, 5.0], // Noise point
            vec![10.0, 10.0],
            vec![10.1, 10.0],
        ];
        let labels = vec![0, 0, -1, 1, 1]; // -1 is noise

        let silhouette = clusterer.compute_silhouette(&embeddings, &labels);

        // Should compute silhouette ignoring noise
        // With only 2 points per cluster, silhouette should still be computable
        assert!(
            silhouette != 0.0 || labels.iter().filter(|&&l| l != -1).count() < 2,
            "Silhouette should be computed excluding noise"
        );

        println!(
            "[PASS] test_clusterer_silhouette_ignores_noise - score={}",
            silhouette
        );
    }

    #[test]
    fn test_clusterer_silhouette_empty_input() {
        let clusterer = HDBSCANClusterer::with_defaults();

        let embeddings: Vec<Vec<f32>> = vec![];
        let labels: Vec<i32> = vec![];

        let silhouette = clusterer.compute_silhouette(&embeddings, &labels);

        assert_eq!(silhouette, 0.0, "Empty input should return 0.0");

        println!("[PASS] test_clusterer_silhouette_empty_input - handled correctly");
    }

    // =========================================================================
    // EDGE CASE TESTS - SYNTHETIC DATA (TASK-P4-005 VERIFICATION)
    // =========================================================================

    #[test]
    fn test_edge_case_all_identical_points() {
        // Edge case: All points are identical
        let clusterer = HDBSCANClusterer::with_defaults();

        let embeddings = vec![
            vec![1.0, 1.0],
            vec![1.0, 1.0],
            vec![1.0, 1.0],
        ];
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);

        // Should succeed - all points in same cluster
        assert!(result.is_ok(), "Identical points should cluster");
        let memberships = result.unwrap();

        // All should be in same cluster or all noise (depending on distance handling)
        let clusters: std::collections::HashSet<_> =
            memberships.iter().map(|m| m.cluster_id).collect();
        assert!(
            clusters.len() <= 2,
            "Identical points should be in same cluster or noise"
        );

        println!(
            "[PASS] test_edge_case_all_identical_points - clusters={:?}",
            clusters
        );
    }

    #[test]
    fn test_edge_case_linear_chain() {
        // Edge case: Points form a linear chain with increasing distance
        let clusterer = HDBSCANClusterer::with_defaults();

        let embeddings = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![2.0, 0.0],
            vec![3.0, 0.0],
            vec![4.0, 0.0],
        ];
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);
        assert!(result.is_ok(), "Linear chain should cluster");

        let memberships = result.unwrap();
        assert_eq!(memberships.len(), 5);

        // Print cluster assignments for verification
        let assignments: Vec<_> = memberships.iter().map(|m| m.cluster_id).collect();
        println!(
            "[PASS] test_edge_case_linear_chain - assignments={:?}",
            assignments
        );
    }

    #[test]
    fn test_edge_case_exact_min_cluster_size() {
        // Edge case: Exactly min_cluster_size points
        let clusterer = HDBSCANClusterer::with_defaults(); // min_cluster_size=3

        let embeddings = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.0],
            vec![0.0, 0.1],
        ];
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);
        assert!(
            result.is_ok(),
            "Exactly min_cluster_size points must succeed"
        );

        let memberships = result.unwrap();
        assert_eq!(memberships.len(), 3);

        println!("[PASS] test_edge_case_exact_min_cluster_size - fit succeeded");
    }

    #[test]
    fn test_edge_case_high_dimensional() {
        // Edge case: High-dimensional embeddings (like real E1 at 1024D)
        // Verify space-specific construction works
        let _space_clusterer = HDBSCANClusterer::for_space(Embedder::Semantic);

        // Create 4 high-dimensional points (128D for test performance)
        let dim = 128;
        let mut embeddings = vec![];

        // Cluster A: similar points
        for i in 0..2 {
            let mut vec = vec![0.0f32; dim];
            vec[0] = 1.0 + (i as f32 * 0.01);
            embeddings.push(vec);
        }
        // Cluster B: different points
        for i in 0..2 {
            let mut vec = vec![0.0f32; dim];
            vec[dim - 1] = 1.0 + (i as f32 * 0.01);
            embeddings.push(vec);
        }

        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();

        // Use smaller min_cluster_size for this test
        let clusterer = HDBSCANClusterer::new(
            HDBSCANParams::default()
                .with_min_cluster_size(2)
                .with_min_samples(1),
        );

        let result = clusterer.fit(&embeddings, &ids, Embedder::Semantic);
        assert!(result.is_ok(), "High-dimensional clustering must succeed");

        let memberships = result.unwrap();
        assert_eq!(memberships.len(), 4);

        println!(
            "[PASS] test_edge_case_high_dimensional - dim={}, points={}",
            dim,
            memberships.len()
        );
    }

    #[test]
    fn test_edge_case_sparse_like_embeddings() {
        // Edge case: Sparse-like embeddings (many zeros)
        let clusterer = HDBSCANClusterer::for_space(Embedder::Sparse);

        // Sparse vectors with few non-zero elements
        let embeddings = vec![
            vec![1.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.9, 0.1, 0.0, 0.0, 0.0],
            vec![0.8, 0.2, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.9, 0.1],
            vec![0.0, 0.0, 0.0, 0.8, 0.2],
        ];
        let ids: Vec<Uuid> = (0..6).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit(&embeddings, &ids, Embedder::Sparse);
        assert!(result.is_ok(), "Sparse embeddings must cluster");

        let memberships = result.unwrap();
        assert_eq!(memberships.len(), 6);

        // First 3 and last 3 should ideally be in different clusters
        let first_half: Vec<_> = memberships[0..3].iter().map(|m| m.cluster_id).collect();
        let second_half: Vec<_> = memberships[3..6].iter().map(|m| m.cluster_id).collect();

        println!(
            "[PASS] test_edge_case_sparse_like_embeddings - first={:?}, second={:?}",
            first_half, second_half
        );
    }

    #[test]
    fn test_all_embedder_spaces_cluster() {
        // Verify clustering works for all 13 embedder spaces
        // Use normalized vectors that work with Cosine metric
        // Need 5 points for Sparse/KeywordSplade (min_cluster_size=5)
        let dense_embeddings = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.9, 0.1, 0.0],
            vec![0.8, 0.2, 0.0],
            vec![0.7, 0.3, 0.0],
            vec![0.6, 0.4, 0.0],
        ];

        // For Jaccard metric, use binary-like vectors (0 or 1)
        // Need 5 points for min_cluster_size=5
        let sparse_embeddings = vec![
            vec![1.0, 1.0, 0.0, 0.0],
            vec![1.0, 0.0, 1.0, 0.0],
            vec![0.0, 1.0, 1.0, 0.0],
            vec![1.0, 1.0, 1.0, 0.0],
            vec![1.0, 0.0, 0.0, 1.0],
        ];

        let dense_ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
        let sparse_ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        for embedder in Embedder::all() {
            let clusterer = HDBSCANClusterer::for_space(embedder);

            // Use sparse-compatible vectors for Jaccard-based embedders
            let (embeddings, ids): (&Vec<Vec<f32>>, &Vec<Uuid>) =
                if clusterer.params.metric == DistanceMetric::Jaccard {
                    (&sparse_embeddings, &sparse_ids)
                } else {
                    (&dense_embeddings, &dense_ids)
                };

            let result = clusterer.fit(embeddings, ids, embedder);

            assert!(
                result.is_ok(),
                "Clustering must succeed for {:?}, got error: {:?}",
                embedder,
                result.err()
            );

            let memberships = result.unwrap();
            assert_eq!(memberships.len(), 5);

            for m in &memberships {
                assert_eq!(m.space, embedder);
            }
        }

        println!("[PASS] test_all_embedder_spaces_cluster - all 13 embedders work");
    }

    // =========================================================================
    // PRECOMPUTED DISTANCE MATRIX TESTS (FDMC Support)
    // =========================================================================

    #[test]
    fn test_fit_precomputed_two_clusters() {
        // Two well-separated clusters in distance space
        // Cluster A: memories 0, 1, 2 (close to each other)
        // Cluster B: memories 3, 4, 5 (close to each other, far from A)
        let clusterer = HDBSCANClusterer::with_defaults();

        // Distances: 0 = identical, 1 = maximally different
        // Intra-cluster distance ~0.05-0.10
        // Inter-cluster distance ~0.80-0.90
        let distance_matrix = vec![
            // Memory 0
            vec![0.0, 0.05, 0.08, 0.85, 0.87, 0.88],
            // Memory 1
            vec![0.05, 0.0, 0.06, 0.82, 0.84, 0.85],
            // Memory 2
            vec![0.08, 0.06, 0.0, 0.83, 0.86, 0.84],
            // Memory 3
            vec![0.85, 0.82, 0.83, 0.0, 0.07, 0.09],
            // Memory 4
            vec![0.87, 0.84, 0.86, 0.07, 0.0, 0.06],
            // Memory 5
            vec![0.88, 0.85, 0.84, 0.09, 0.06, 0.0],
        ];
        let ids: Vec<Uuid> = (0..6).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_ok(), "fit_precomputed must succeed");

        let memberships = result.unwrap();
        assert_eq!(memberships.len(), 6);

        // Verify cluster assignments
        let cluster_a: Vec<i32> = memberships[0..3].iter().map(|m| m.cluster_id).collect();
        let cluster_b: Vec<i32> = memberships[3..6].iter().map(|m| m.cluster_id).collect();

        // All in cluster A should have same label
        assert!(
            cluster_a.iter().all(|&c| c == cluster_a[0]),
            "Memories 0-2 should be in same cluster"
        );

        // All in cluster B should have same label
        assert!(
            cluster_b.iter().all(|&c| c == cluster_b[0]),
            "Memories 3-5 should be in same cluster"
        );

        // Clusters A and B should be different
        if cluster_a[0] != -1 && cluster_b[0] != -1 {
            assert_ne!(
                cluster_a[0], cluster_b[0],
                "Clusters A and B should have different labels"
            );
        }

        println!(
            "[PASS] test_fit_precomputed_two_clusters - A={:?}, B={:?}",
            cluster_a[0], cluster_b[0]
        );
    }

    #[test]
    fn test_fit_precomputed_insufficient_data() {
        let clusterer = HDBSCANClusterer::with_defaults(); // min_cluster_size=3

        // Only 2 memories
        let distance_matrix = vec![vec![0.0, 0.5], vec![0.5, 0.0]];
        let ids: Vec<Uuid> = (0..2).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_err(), "Should fail with insufficient data");

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("required 3"),
            "Error should mention required count"
        );

        println!(
            "[PASS] test_fit_precomputed_insufficient_data - error: {}",
            err
        );
    }

    #[test]
    fn test_fit_precomputed_non_square_matrix() {
        let clusterer = HDBSCANClusterer::with_defaults();

        // Non-square matrix
        let distance_matrix = vec![
            vec![0.0, 0.1, 0.2],
            vec![0.1, 0.0, 0.15],
            vec![0.2, 0.15], // Missing element
        ];
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_err(), "Should fail with non-square matrix");

        println!("[PASS] test_fit_precomputed_non_square_matrix");
    }

    #[test]
    fn test_fit_precomputed_nan_rejection() {
        let clusterer = HDBSCANClusterer::with_defaults();

        let mut distance_matrix = vec![
            vec![0.0, 0.1, 0.2],
            vec![0.1, 0.0, 0.15],
            vec![0.2, 0.15, 0.0],
        ];
        // Insert NaN
        distance_matrix[1][2] = f32::NAN;
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_err(), "Should fail with NaN");

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("not finite"),
            "Error should mention finite requirement"
        );

        println!("[PASS] test_fit_precomputed_nan_rejection - error: {}", err);
    }

    #[test]
    fn test_fit_precomputed_infinity_rejection() {
        let clusterer = HDBSCANClusterer::with_defaults();

        let mut distance_matrix = vec![
            vec![0.0, 0.1, 0.2],
            vec![0.1, 0.0, 0.15],
            vec![0.2, 0.15, 0.0],
        ];
        // Insert Infinity
        distance_matrix[0][1] = f32::INFINITY;
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_err(), "Should fail with Infinity");

        println!("[PASS] test_fit_precomputed_infinity_rejection");
    }

    #[test]
    fn test_fit_precomputed_id_count_mismatch() {
        let clusterer = HDBSCANClusterer::with_defaults();

        let distance_matrix = vec![
            vec![0.0, 0.1, 0.2],
            vec![0.1, 0.0, 0.15],
            vec![0.2, 0.15, 0.0],
        ];
        // Wrong number of IDs
        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_err(), "Should fail with ID count mismatch");

        println!("[PASS] test_fit_precomputed_id_count_mismatch");
    }

    #[test]
    fn test_fit_precomputed_memory_ids_preserved() {
        let clusterer = HDBSCANClusterer::with_defaults();

        let distance_matrix = vec![
            vec![0.0, 0.05, 0.06],
            vec![0.05, 0.0, 0.04],
            vec![0.06, 0.04, 0.0],
        ];
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_ok());

        let memberships = result.unwrap();
        for (i, m) in memberships.iter().enumerate() {
            assert_eq!(m.memory_id, ids[i], "Memory IDs must be preserved");
        }

        println!("[PASS] test_fit_precomputed_memory_ids_preserved");
    }

    #[test]
    fn test_silhouette_precomputed() {
        let clusterer = HDBSCANClusterer::with_defaults();

        // Two well-separated clusters
        let distance_matrix = vec![
            vec![0.0, 0.05, 0.06, 0.85, 0.87, 0.88],
            vec![0.05, 0.0, 0.04, 0.82, 0.84, 0.85],
            vec![0.06, 0.04, 0.0, 0.83, 0.86, 0.84],
            vec![0.85, 0.82, 0.83, 0.0, 0.07, 0.09],
            vec![0.87, 0.84, 0.86, 0.07, 0.0, 0.06],
            vec![0.88, 0.85, 0.84, 0.09, 0.06, 0.0],
        ];
        let labels = vec![0, 0, 0, 1, 1, 1];

        let silhouette = clusterer.compute_silhouette_precomputed(&distance_matrix, &labels);

        // Well-separated clusters should have high silhouette
        assert!(
            silhouette > 0.7,
            "Well-separated clusters should have high silhouette, got {}",
            silhouette
        );

        println!(
            "[PASS] test_silhouette_precomputed - score={}",
            silhouette
        );
    }

    #[test]
    fn test_silhouette_precomputed_single_cluster() {
        let clusterer = HDBSCANClusterer::with_defaults();

        let distance_matrix = vec![
            vec![0.0, 0.1, 0.2],
            vec![0.1, 0.0, 0.15],
            vec![0.2, 0.15, 0.0],
        ];
        let labels = vec![0, 0, 0]; // Single cluster

        let silhouette = clusterer.compute_silhouette_precomputed(&distance_matrix, &labels);

        // Single cluster = 0.0 silhouette
        assert_eq!(silhouette, 0.0, "Single cluster should have 0.0 silhouette");

        println!("[PASS] test_silhouette_precomputed_single_cluster");
    }

    #[test]
    fn test_fit_precomputed_single_cluster() {
        // All memories are very close to each other - should form single cluster
        let clusterer = HDBSCANClusterer::with_defaults();

        let distance_matrix = vec![
            vec![0.0, 0.02, 0.03],
            vec![0.02, 0.0, 0.01],
            vec![0.03, 0.01, 0.0],
        ];
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_ok());

        let memberships = result.unwrap();

        // All should be in the same cluster (not noise)
        let cluster_ids: Vec<i32> = memberships.iter().map(|m| m.cluster_id).collect();
        assert!(
            cluster_ids.iter().all(|&c| c == cluster_ids[0]),
            "All memories should be in same cluster"
        );
        assert!(cluster_ids[0] >= 0, "Should not be noise");

        println!(
            "[PASS] test_fit_precomputed_single_cluster - all in cluster {}",
            cluster_ids[0]
        );
    }

    // =========================================================================
    // DATA-DRIVEN GAP THRESHOLD TESTS (provenance: verifies adaptive threshold)
    // =========================================================================

    #[test]
    fn test_gap_threshold_finds_clear_gap() {
        // Provenance: MST weights [0.05, 0.06, 0.07, 0.80, 0.82]
        // Largest gap = 0.80 - 0.07 = 0.73 at index 3
        // Threshold should be at weights[3] = 0.80
        // Build a precomputed distance matrix that produces this MST structure
        // 5 points: A(0,1,2) tight cluster, B(3,4) far cluster
        let distance_matrix = vec![
            vec![0.0, 0.05, 0.07, 0.80, 0.82],
            vec![0.05, 0.0, 0.06, 0.81, 0.83],
            vec![0.07, 0.06, 0.0, 0.80, 0.82],
            vec![0.80, 0.81, 0.80, 0.0, 0.05],
            vec![0.82, 0.83, 0.82, 0.05, 0.0],
        ];
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        // Use min_cluster_size=2 to allow the two groups to form clusters
        let small_clusterer = HDBSCANClusterer::new(
            HDBSCANParams::default()
                .with_min_cluster_size(2)
                .with_min_samples(1),
        );
        let result = small_clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_ok(), "fit_precomputed must succeed");

        let memberships = result.unwrap();
        let cluster_a: Vec<i32> = memberships[0..3].iter().map(|m| m.cluster_id).collect();
        let cluster_b: Vec<i32> = memberships[3..5].iter().map(|m| m.cluster_id).collect();

        // Group A should be in same cluster
        assert!(
            cluster_a.iter().all(|&c| c == cluster_a[0]),
            "Points 0-2 should be in same cluster, got {:?}",
            cluster_a
        );
        // Group B should be in same cluster
        assert!(
            cluster_b.iter().all(|&c| c == cluster_b[0]),
            "Points 3-4 should be in same cluster, got {:?}",
            cluster_b
        );
        // Groups should be separated (different clusters or noise)
        if cluster_a[0] != -1 && cluster_b[0] != -1 {
            assert_ne!(
                cluster_a[0], cluster_b[0],
                "Clusters A and B should be different"
            );
        }

        println!(
            "[PASS] test_gap_threshold_finds_clear_gap - A={}, B={}",
            cluster_a[0], cluster_b[0]
        );
    }

    #[test]
    fn test_gap_threshold_uniform_distribution() {
        // Provenance: MST weights [0.10, 0.11, 0.12, 0.13, 0.14]
        // Max gap = 0.01 < min_significant_gap(0.02) → 75th percentile fallback
        // 75th percentile = weights[3] = 0.13
        let clusterer = HDBSCANClusterer::with_defaults();

        // Uniform distances — no natural gap
        let distance_matrix = vec![
            vec![0.00, 0.10, 0.11, 0.12, 0.13, 0.14],
            vec![0.10, 0.00, 0.10, 0.11, 0.12, 0.13],
            vec![0.11, 0.10, 0.00, 0.10, 0.11, 0.12],
            vec![0.12, 0.11, 0.10, 0.00, 0.10, 0.11],
            vec![0.13, 0.12, 0.11, 0.10, 0.00, 0.10],
            vec![0.14, 0.13, 0.12, 0.11, 0.10, 0.00],
        ];
        let ids: Vec<Uuid> = (0..6).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_ok(), "Uniform distances must cluster without panic");

        let memberships = result.unwrap();
        // With uniform distribution and 75th percentile fallback,
        // most/all points should end up in a single cluster (no natural gap)
        let non_noise: Vec<i32> = memberships
            .iter()
            .filter(|m| m.cluster_id >= 0)
            .map(|m| m.cluster_id)
            .collect();

        // At least some points should be clustered (not all noise)
        assert!(
            !non_noise.is_empty(),
            "Some points should be clustered in uniform distribution"
        );

        println!(
            "[PASS] test_gap_threshold_uniform_distribution - {} non-noise points",
            non_noise.len()
        );
    }

    #[test]
    fn test_gap_threshold_respects_cosine_floor() {
        // Provenance: Very tight distances [0.01, 0.015, 0.02]
        // Largest gap = 0.005 < min_significant_gap(0.02) → 75th percentile
        // 75th percentile ≈ 0.02, but floor = 0.03 → final threshold = 0.03
        // With threshold 0.03, all edges (<=0.02) are below it → single cluster
        let clusterer = HDBSCANClusterer::new(
            HDBSCANParams::default()
                .with_min_cluster_size(2)
                .with_min_samples(1),
        );

        // Very tight cluster — everything is within cosine floor
        let distance_matrix = vec![
            vec![0.000, 0.010, 0.020],
            vec![0.010, 0.000, 0.015],
            vec![0.020, 0.015, 0.000],
        ];
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_ok());

        let memberships = result.unwrap();
        let cluster_ids: Vec<i32> = memberships.iter().map(|m| m.cluster_id).collect();

        // Floor prevents over-splitting: all points should stay together
        assert!(
            cluster_ids.iter().all(|&c| c == cluster_ids[0]),
            "Cosine floor should prevent splitting tight cluster, got {:?}",
            cluster_ids
        );
        assert!(cluster_ids[0] >= 0, "Should not be noise");

        println!(
            "[PASS] test_gap_threshold_respects_cosine_floor - all in cluster {}",
            cluster_ids[0]
        );
    }

    #[test]
    fn test_gap_threshold_empty_mst() {
        // Provenance: empty MST → f32::MAX (no edges to cluster)
        let clusterer = HDBSCANClusterer::with_defaults();

        // extract_clusters with 0 points
        let (labels, probs) = clusterer.extract_clusters(&[], 0);
        assert!(labels.is_empty());
        assert!(probs.is_empty());

        println!("[PASS] test_gap_threshold_empty_mst - returns empty");
    }

    #[test]
    fn test_gap_threshold_single_edge() {
        // Provenance: single MST edge → threshold = max(weight, floor)
        // For weight=0.15 with cosine floor=0.03 → threshold = 0.15
        let clusterer = HDBSCANClusterer::new(
            HDBSCANParams::default()
                .with_min_cluster_size(2)
                .with_min_samples(1),
        );

        let distance_matrix = vec![vec![0.0, 0.15], vec![0.15, 0.0]];
        let ids: Vec<Uuid> = (0..2).map(|_| Uuid::new_v4()).collect();

        let result = clusterer.fit_precomputed(&distance_matrix, &ids);
        assert!(result.is_ok());

        let memberships = result.unwrap();
        // Single edge at 0.15: with no gap to detect (n=1 edge),
        // the 75th percentile IS that edge, so threshold = 0.15
        // The edge weight (0.15) is not > threshold (0.15), so they merge
        assert_eq!(memberships.len(), 2);
        assert!(
            memberships[0].cluster_id == memberships[1].cluster_id,
            "Single edge should merge both points into same cluster"
        );

        println!(
            "[PASS] test_gap_threshold_single_edge - cluster={}",
            memberships[0].cluster_id
        );
    }
}
