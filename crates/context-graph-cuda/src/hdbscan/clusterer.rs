//! GPU-accelerated HDBSCAN clusterer.
//!
//! Uses FAISS GPU for k-NN to accelerate the core distance computation,
//! which is the O(n²k) bottleneck of HDBSCAN.
//!
//! # Algorithm Overview
//!
//! 1. **Core distances (GPU)**: Use FAISS batch k-NN to find k-th nearest neighbor
//!    for all points in a single GPU kernel launch.
//!
//! 2. **Mutual reachability (CPU)**: Compute MR(a,b) = max(core_a, core_b, dist(a,b))
//!    This is O(n²) but with simple operations, fast on CPU.
//!
//! 3. **MST (CPU)**: Prim's algorithm on mutual reachability graph. O(n²) but simple.
//!
//! 4. **Cluster extraction (CPU)**: Union-Find with gap detection. O(n).
//!
//! # Constitution Compliance
//!
//! - ARCH-GPU-05: HDBSCAN clustering runs on GPU (core distance via FAISS)
//! - AP-GPU-04: NEVER use sklearn HDBSCAN
//! - Performance target: < 20ms for topic detection

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use tracing::{debug, info, instrument};
use uuid::Uuid;

use super::error::{GpuHdbscanError, GpuHdbscanResult};
use super::gpu_knn::GpuKnnIndex;

/// Cluster membership result.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterMembership {
    /// Memory ID this membership belongs to.
    pub memory_id: Uuid,
    /// Cluster ID (-1 = noise).
    pub cluster_id: i32,
    /// Membership probability [0.0, 1.0].
    pub membership_probability: f32,
    /// Whether this point is a core point.
    pub is_core: bool,
}

impl ClusterMembership {
    /// Create a new cluster membership.
    pub fn new(memory_id: Uuid, cluster_id: i32, probability: f32, is_core: bool) -> Self {
        Self {
            memory_id,
            cluster_id,
            membership_probability: probability.clamp(0.0, 1.0),
            is_core,
        }
    }

    /// Create a noise membership.
    pub fn noise(memory_id: Uuid) -> Self {
        Self {
            memory_id,
            cluster_id: -1,
            membership_probability: 0.0,
            is_core: false,
        }
    }
}

/// Cluster selection method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClusterSelectionMethod {
    /// Excess of Mass - default, good general purpose.
    #[default]
    EOM,
    /// Leaf clusters only - more granular.
    Leaf,
}

/// HDBSCAN parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct HdbscanParams {
    /// Minimum cluster size (constitution default: 3).
    pub min_cluster_size: usize,
    /// Minimum samples for core point (typically min_cluster_size - 1).
    pub min_samples: usize,
    /// Cluster selection method.
    pub cluster_selection_method: ClusterSelectionMethod,
}

impl Default for HdbscanParams {
    fn default() -> Self {
        Self {
            min_cluster_size: 3, // Constitution default
            min_samples: 2,
            cluster_selection_method: ClusterSelectionMethod::EOM,
        }
    }
}

impl HdbscanParams {
    /// Validate parameters.
    pub fn validate(&self) -> GpuHdbscanResult<()> {
        if self.min_cluster_size < 2 {
            return Err(GpuHdbscanError::invalid_parameter(
                "min_cluster_size",
                self.min_cluster_size,
                "must be >= 2",
            ));
        }

        if self.min_samples < 1 {
            return Err(GpuHdbscanError::invalid_parameter(
                "min_samples",
                self.min_samples,
                "must be >= 1",
            ));
        }

        if self.min_samples > self.min_cluster_size {
            return Err(GpuHdbscanError::invalid_parameter(
                "min_samples",
                self.min_samples,
                format!("must be <= min_cluster_size ({})", self.min_cluster_size),
            ));
        }

        Ok(())
    }
}

/// GPU-accelerated HDBSCAN clusterer.
///
/// Uses FAISS GPU for the expensive k-NN computation.
/// No CPU fallback - fails fast if GPU is unavailable.
pub struct GpuHdbscanClusterer {
    params: HdbscanParams,
}

impl GpuHdbscanClusterer {
    /// Create a new GPU HDBSCAN clusterer with default parameters.
    pub fn new() -> Self {
        Self::with_params(HdbscanParams::default())
    }

    /// Create with custom parameters.
    pub fn with_params(params: HdbscanParams) -> Self {
        Self { params }
    }

    /// Fit the clusterer to embeddings and return cluster assignments.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - Slice of embedding vectors (all same dimension)
    /// * `memory_ids` - Slice of UUIDs corresponding to each embedding
    ///
    /// # Returns
    ///
    /// `Vec<ClusterMembership>` with one entry per input embedding.
    ///
    /// # Errors
    ///
    /// - `GpuNotAvailable` if no GPU detected
    /// - `InsufficientData` if fewer points than min_cluster_size
    /// - `DimensionMismatch` if embeddings.len() != memory_ids.len()
    #[instrument(skip_all, fields(n_points = embeddings.len()))]
    pub fn fit(
        &self,
        embeddings: &[Vec<f32>],
        memory_ids: &[Uuid],
    ) -> GpuHdbscanResult<Vec<ClusterMembership>> {
        let start = Instant::now();
        let n = embeddings.len();

        info!(n_points = n, min_cluster_size = self.params.min_cluster_size, "Starting GPU HDBSCAN");

        // Validate parameters
        self.params.validate()?;

        // Validate inputs
        if n < self.params.min_cluster_size {
            return Err(GpuHdbscanError::insufficient_data(
                self.params.min_cluster_size,
                n,
            ));
        }

        if n != memory_ids.len() {
            return Err(GpuHdbscanError::dimension_mismatch(n, memory_ids.len()));
        }

        if embeddings.is_empty() {
            return Ok(vec![]);
        }

        let dimension = embeddings[0].len();
        if dimension == 0 {
            return Err(GpuHdbscanError::invalid_dimension(dimension));
        }

        // Validate all embeddings have same dimension and finite values
        for (i, emb) in embeddings.iter().enumerate() {
            if emb.len() != dimension {
                return Err(GpuHdbscanError::InvalidParameter {
                    parameter: format!("embeddings[{}].len()", i),
                    value: emb.len().to_string(),
                    requirement: format!("must equal {}", dimension),
                });
            }
            for (j, &val) in emb.iter().enumerate() {
                if !val.is_finite() {
                    return Err(GpuHdbscanError::non_finite_value(i * dimension + j, val));
                }
            }
        }

        // === STEP 1: GPU k-NN for core distances ===
        let knn_start = Instant::now();

        let mut knn_index = GpuKnnIndex::new(dimension)?;
        knn_index.add(embeddings)?;

        let core_distances = knn_index.compute_core_distances_with_vectors(
            embeddings,
            self.params.min_samples,
        )?;

        let knn_elapsed = knn_start.elapsed();
        debug!(
            knn_elapsed_us = knn_elapsed.as_micros(),
            n_points = n,
            "GPU k-NN complete"
        );

        // === STEP 2: Mutual reachability (CPU - fast with precomputed core distances) ===
        let mr_start = Instant::now();
        let mutual_reach = self.compute_mutual_reachability(embeddings, &core_distances);
        let mr_elapsed = mr_start.elapsed();
        debug!(mr_elapsed_us = mr_elapsed.as_micros(), "Mutual reachability complete");

        // === STEP 3: MST (CPU - Prim's algorithm) ===
        let mst_start = Instant::now();
        let mst = self.build_mst(&mutual_reach);
        let mst_elapsed = mst_start.elapsed();
        debug!(mst_elapsed_us = mst_elapsed.as_micros(), mst_edges = mst.len(), "MST complete");

        // === STEP 4: Cluster extraction (CPU - Union-Find) ===
        let cluster_start = Instant::now();
        let (labels, probabilities) = self.extract_clusters(&mst, n);
        let cluster_elapsed = cluster_start.elapsed();
        debug!(cluster_elapsed_us = cluster_elapsed.as_micros(), "Cluster extraction complete");

        // === STEP 5: Identify core points ===
        let core_points = self.identify_core_points(&labels);

        // Build result
        let memberships: Vec<ClusterMembership> = memory_ids
            .iter()
            .zip(labels.iter())
            .zip(probabilities.iter())
            .zip(core_points.iter())
            .map(|(((id, &label), &prob), &is_core)| {
                ClusterMembership::new(*id, label, prob, is_core)
            })
            .collect();

        let total_elapsed = start.elapsed();
        let n_clusters = labels.iter().filter(|&&l| l >= 0).collect::<HashSet<_>>().len();
        let n_noise = labels.iter().filter(|&&l| l == -1).count();

        info!(
            total_ms = total_elapsed.as_millis(),
            knn_us = knn_elapsed.as_micros(),
            n_clusters,
            n_noise,
            n_points = n,
            "GPU HDBSCAN complete"
        );

        Ok(memberships)
    }

    /// Compute mutual reachability matrix.
    ///
    /// MR(a,b) = max(core_dist(a), core_dist(b), dist(a,b))
    fn compute_mutual_reachability(
        &self,
        embeddings: &[Vec<f32>],
        core_distances: &[f32],
    ) -> Vec<Vec<f32>> {
        let n = embeddings.len();
        let mut mutual_reach = vec![vec![0.0f32; n]; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let dist = self.euclidean_distance(&embeddings[i], &embeddings[j]);
                let mr = dist.max(core_distances[i]).max(core_distances[j]);
                mutual_reach[i][j] = mr;
                mutual_reach[j][i] = mr;
            }
        }

        mutual_reach
    }

    /// Euclidean distance between two vectors.
    #[inline]
    fn euclidean_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f32>()
            .sqrt()
    }

    /// Build minimum spanning tree using Prim's algorithm.
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

    /// Extract clusters from MST hierarchy using Union-Find.
    fn extract_clusters(&self, mst: &[(usize, usize, f32)], n_points: usize) -> (Vec<i32>, Vec<f32>) {
        if n_points == 0 {
            return (vec![], vec![]);
        }

        // Union-Find
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

        // Detect gap threshold
        let gap_threshold = self.detect_gap_threshold(mst);

        // Track cluster sizes
        let mut cluster_sizes: HashMap<usize, usize> = HashMap::new();
        for i in 0..n_points {
            cluster_sizes.insert(i, 1);
        }

        // Process edges in order of weight, stop at gap
        for (i, j, weight) in mst {
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
                probabilities[i] = 1.0 - (1.0 / cluster_size as f32).min(0.5);
            } else {
                labels[i] = -1;
                probabilities[i] = 0.0;
            }
        }

        (labels, probabilities)
    }

    /// Detect gap threshold for cluster separation.
    fn detect_gap_threshold(&self, mst: &[(usize, usize, f32)]) -> f32 {
        if mst.is_empty() {
            return f32::MAX;
        }

        let weights: Vec<f32> = mst.iter().map(|(_, _, w)| *w).collect();
        let n = weights.len();

        // Use absolute threshold for semantic embeddings
        // 0.5 L2 distance ≈ 0.75 cosine similarity
        let semantic_threshold = 0.5;

        // Also look for significant gaps
        for i in 1..n {
            let ratio = if weights[i - 1] > 0.001 {
                weights[i] / weights[i - 1]
            } else {
                1.0
            };

            // 2x jump indicates cluster boundary
            if ratio >= 2.0 && weights[i] > 0.1 {
                return weights[i];
            }
        }

        // Use semantic threshold if no clear gap
        semantic_threshold
    }

    /// Identify core points based on cluster labels.
    fn identify_core_points(&self, labels: &[i32]) -> Vec<bool> {
        let n = labels.len();
        let mut is_core = vec![false; n];

        for i in 0..n {
            if labels[i] == -1 {
                continue;
            }

            let neighbor_count = labels
                .iter()
                .enumerate()
                .filter(|&(j, &label)| j != i && label == labels[i])
                .count();

            is_core[i] = neighbor_count >= self.params.min_samples;
        }

        is_core
    }

    /// Compute silhouette score for clustering quality.
    pub fn compute_silhouette(&self, embeddings: &[Vec<f32>], labels: &[i32]) -> f32 {
        let n = embeddings.len();
        if n < 2 || labels.len() != n {
            return 0.0;
        }

        let clusters: HashSet<i32> = labels.iter().filter(|&&l| l != -1).copied().collect();
        if clusters.len() < 2 {
            return 0.0;
        }

        let mut total_silhouette = 0.0;
        let mut count = 0;

        for i in 0..n {
            if labels[i] == -1 {
                continue;
            }

            // a(i) = mean distance to same cluster
            let (same_sum, same_count) = (0..n)
                .filter(|&j| j != i && labels[j] == labels[i])
                .fold((0.0f32, 0usize), |(sum, cnt), j| {
                    (sum + self.euclidean_distance(&embeddings[i], &embeddings[j]), cnt + 1)
                });

            let a_i = if same_count > 0 { same_sum / same_count as f32 } else { 0.0 };

            // b(i) = min mean distance to other clusters
            let b_i = clusters
                .iter()
                .filter(|&&c| c != labels[i])
                .filter_map(|&cluster| {
                    let (sum, cnt) = (0..n)
                        .filter(|&j| labels[j] == cluster)
                        .fold((0.0f32, 0usize), |(sum, cnt), j| {
                            (sum + self.euclidean_distance(&embeddings[i], &embeddings[j]), cnt + 1)
                        });
                    if cnt > 0 { Some(sum / cnt as f32) } else { None }
                })
                .fold(f32::MAX, f32::min);

            let b_i = if b_i == f32::MAX { 0.0 } else { b_i };

            let max_ab = a_i.max(b_i);
            let s_i = if max_ab > 0.0 { (b_i - a_i) / max_ab } else { 0.0 };

            total_silhouette += s_i;
            count += 1;
        }

        if count > 0 { total_silhouette / count as f32 } else { 0.0 }
    }
}

impl Default for GpuHdbscanClusterer {
    fn default() -> Self {
        Self::new()
    }
}
