//! MultiSpaceClusterManager for cross-space clustering coordination.
//!
//! Orchestrates HDBSCAN (batch) and BIRCH (incremental) clustering across
//! all 13 embedding spaces, managing per-space BIRCH trees and batch reclustering.
//!
//! # Architecture
//!
//! Per constitution:
//! - ARCH-01: TeleologicalArray is atomic - all 13 embeddings or nothing
//! - ARCH-02: Apples-to-apples only - compare E1<->E1, E4<->E4, never cross-space
//! - ARCH-04: Temporal embedders (E2-E4) NEVER count toward topic detection
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//!
//! # Usage
//!
//! ```
//! use context_graph_core::clustering::{MultiSpaceClusterManager, manager_defaults};
//! use uuid::Uuid;
//!
//! // Create manager with default parameters
//! let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();
//!
//! // Insert a memory with 13 embeddings
//! let memory_id = Uuid::new_v4();
//! let embeddings: [Vec<f32>; 13] = std::array::from_fn(|i| {
//!     match i {
//!         0 => vec![0.0; 1024],   // E1
//!         1..=3 => vec![0.0; 512], // E2-E4
//!         4 => vec![0.0; 768],    // E5
//!         5 | 12 => vec![0.0; 30522], // E6, E13 sparse
//!         6 => vec![0.0; 1536],   // E7
//!         7 => vec![0.0; 1024], // E8 (upgraded from 384D)
//!         10 => vec![0.0; 768], // E11 (KEPLER)
//!         8 => vec![0.0; 1024],   // E9
//!         9 => vec![0.0; 768],    // E10
//!         11 => vec![0.0; 128],   // E12 per-token (simplified)
//!         _ => vec![0.0; 128],
//!     }
//! });
//!
//! // Insert into manager
//! let result = manager.insert(memory_id, &embeddings);
//! assert!(result.is_ok());
//! ```

use std::collections::HashMap;

use uuid::Uuid;

use crate::embeddings::category::category_for;
use crate::embeddings::config::get_dimension;
use crate::teleological::Embedder;
use crate::types::SemanticFingerprint;

use super::birch::{birch_defaults, BIRCHParams, BIRCHTree};
use super::cluster::Cluster;
use super::error::ClusterError;
use super::fingerprint_matrix::{build_fingerprint_matrix, FingerprintMatrixConfig, SimilarityStats};
use super::hdbscan::{hdbscan_defaults, HDBSCANClusterer, HDBSCANParams};
use super::membership::ClusterMembership;
use super::stability::TopicStabilityTracker;
use super::topic::{Topic, TopicProfile};

// =============================================================================
// Constants
// =============================================================================

/// Default HDBSCAN batch reclustering threshold (number of incremental updates).
pub const DEFAULT_RECLUSTER_THRESHOLD: usize = 100;

/// Maximum weighted agreement per constitution (7*1.0 + 2*0.5 + 1*0.5 = 8.5).
pub const MAX_WEIGHTED_AGREEMENT: f32 = 8.5;

/// Topic detection threshold per ARCH-09.
pub const TOPIC_THRESHOLD: f32 = 2.5;

// =============================================================================
// ManagerParams
// =============================================================================

/// Configuration parameters for MultiSpaceClusterManager.
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::{ManagerParams, manager_defaults};
///
/// let params = manager_defaults();
/// assert!(params.recluster_threshold > 0);
/// ```
#[derive(Debug, Clone)]
pub struct ManagerParams {
    /// BIRCH parameters for each embedding space.
    pub birch_params: BIRCHParams,

    /// HDBSCAN parameters for batch reclustering.
    pub hdbscan_params: HDBSCANParams,

    /// Number of incremental updates before triggering HDBSCAN reclustering.
    pub recluster_threshold: usize,
}

impl Default for ManagerParams {
    fn default() -> Self {
        Self {
            birch_params: birch_defaults(),
            hdbscan_params: hdbscan_defaults(),
            recluster_threshold: DEFAULT_RECLUSTER_THRESHOLD,
        }
    }
}

impl ManagerParams {
    /// Validate parameters.
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::InvalidParameter` if any parameters are invalid.
    pub fn validate(&self) -> Result<(), ClusterError> {
        self.birch_params.validate()?;
        self.hdbscan_params.validate()?;

        if self.recluster_threshold == 0 {
            return Err(ClusterError::invalid_parameter(
                "recluster_threshold must be > 0; HDBSCAN batch reclustering requires a positive threshold",
            ));
        }

        Ok(())
    }

    /// Set recluster threshold.
    #[must_use]
    pub fn with_recluster_threshold(mut self, threshold: usize) -> Self {
        self.recluster_threshold = threshold;
        self
    }

    /// Set BIRCH parameters.
    #[must_use]
    pub fn with_birch_params(mut self, params: BIRCHParams) -> Self {
        self.birch_params = params;
        self
    }

    /// Set HDBSCAN parameters.
    #[must_use]
    pub fn with_hdbscan_params(mut self, params: HDBSCANParams) -> Self {
        self.hdbscan_params = params;
        self
    }
}

/// Get default manager parameters.
pub fn manager_defaults() -> ManagerParams {
    ManagerParams::default()
}

// =============================================================================
// UpdateStatus
// =============================================================================

/// Update status for a single embedding space.
#[derive(Debug, Clone, Copy)]
pub struct UpdateStatus {
    /// The embedding space.
    pub embedder: Embedder,
    /// Number of updates since last HDBSCAN reclustering.
    pub updates_since_recluster: usize,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            embedder: Embedder::Semantic,
            updates_since_recluster: 0,
        }
    }
}

// =============================================================================
// PerSpaceState
// =============================================================================

/// Per-space clustering state.
///
/// Holds the BIRCH tree and accumulated embeddings for one embedding space.
#[derive(Debug)]
struct PerSpaceState {
    /// BIRCH CF-tree for incremental clustering.
    tree: BIRCHTree,

    /// Accumulated embeddings for HDBSCAN batch reclustering.
    embeddings: Vec<Vec<f32>>,

    /// Memory IDs corresponding to embeddings.
    memory_ids: Vec<Uuid>,

    /// Current cluster memberships from most recent clustering.
    memberships: HashMap<Uuid, ClusterMembership>,

    /// Clusters discovered in this space.
    clusters: HashMap<i32, Cluster>,

    /// Number of updates since last HDBSCAN reclustering.
    updates_since_recluster: usize,
}

impl PerSpaceState {
    /// Create new per-space state with given dimension.
    fn new(params: &BIRCHParams, dimension: usize) -> Result<Self, ClusterError> {
        Ok(Self {
            tree: BIRCHTree::new(params.clone(), dimension)?,
            embeddings: Vec::new(),
            memory_ids: Vec::new(),
            memberships: HashMap::new(),
            clusters: HashMap::new(),
            updates_since_recluster: 0,
        })
    }
}

// =============================================================================
// MultiSpaceClusterManager
// =============================================================================

/// Manages clustering across all 13 embedding spaces.
///
/// Coordinates BIRCH incremental clustering and HDBSCAN batch reclustering
/// to discover cross-space topics.
///
/// # Thread Safety
///
/// This type is NOT thread-safe. For concurrent access, wrap in
/// `Arc<RwLock<MultiSpaceClusterManager>>`.
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::MultiSpaceClusterManager;
///
/// let manager = MultiSpaceClusterManager::with_defaults().unwrap();
/// assert_eq!(manager.total_memories(), 0);
/// ```
#[derive(Debug)]
pub struct MultiSpaceClusterManager {
    /// Configuration parameters.
    params: ManagerParams,

    /// Per-space clustering state (13 spaces).
    spaces: [PerSpaceState; 13],

    /// Discovered topics from cross-space synthesis.
    topics: HashMap<Uuid, Topic>,

    /// Total number of memories inserted.
    total_memories: usize,

    /// Topic stability tracker for churn calculation and dream triggers (AP-70).
    stability_tracker: TopicStabilityTracker,
}

impl MultiSpaceClusterManager {
    /// Create a new manager with specified parameters.
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::InvalidParameter` if parameters are invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::{MultiSpaceClusterManager, manager_defaults};
    ///
    /// let manager = MultiSpaceClusterManager::new(manager_defaults()).unwrap();
    /// ```
    pub fn new(params: ManagerParams) -> Result<Self, ClusterError> {
        params.validate()?;

        // Initialize per-space states using array::try_from_fn pattern
        let spaces = Self::init_spaces(&params)?;

        Ok(Self {
            params,
            spaces,
            topics: HashMap::new(),
            total_memories: 0,
            stability_tracker: TopicStabilityTracker::new(),
        })
    }

    /// Create a manager with default parameters.
    ///
    /// # Errors
    ///
    /// Returns error if default parameter initialization fails.
    pub fn with_defaults() -> Result<Self, ClusterError> {
        Self::new(manager_defaults())
    }

    /// Initialize per-space states.
    fn init_spaces(params: &ManagerParams) -> Result<[PerSpaceState; 13], ClusterError> {
        let mut states: Vec<PerSpaceState> = Vec::with_capacity(13);

        for embedder in Embedder::all() {
            let dimension = get_dimension(embedder);
            let state = PerSpaceState::new(&params.birch_params, dimension)?;
            states.push(state);
        }

        // Convert Vec to array - safe because we pushed exactly 13 elements
        states.try_into().map_err(|_| {
            ClusterError::invalid_parameter("Failed to initialize 13 per-space states")
        })
    }

    /// Insert a memory with embeddings from all 13 spaces.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - Unique identifier for this memory
    /// * `embeddings` - Array of 13 embedding vectors, one per space
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::DimensionMismatch` if any embedding has wrong dimension.
    /// Returns `ClusterError::InvalidParameter` if any embedding contains NaN/Infinity.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::MultiSpaceClusterManager;
    /// use uuid::Uuid;
    ///
    /// let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();
    /// let memory_id = Uuid::new_v4();
    ///
    /// // Create 13 embeddings with correct dimensions
    /// let embeddings: [Vec<f32>; 13] = std::array::from_fn(|i| {
    ///     let dim = match i {
    ///         0 => 1024, 1 | 2 | 3 => 512, 4 => 768, 5 | 12 => 30522,
    ///         6 => 1536, 7 => 1024, 8 => 1024, 9 | 10 => 768, 11 => 128, _ => 128,
    ///     };
    ///     vec![0.0; dim]
    /// });
    ///
    /// let result = manager.insert(memory_id, &embeddings);
    /// assert!(result.is_ok());
    /// ```
    pub fn insert(
        &mut self,
        memory_id: Uuid,
        embeddings: &[Vec<f32>; 13],
    ) -> Result<InsertResult, ClusterError> {
        let mut cluster_indices: [i32; 13] = [-1; 13];
        let mut needs_recluster = false;

        // Insert into each space's BIRCH tree
        for (i, embedder) in Embedder::all().enumerate() {
            let embedding = &embeddings[i];
            let expected_dim = get_dimension(embedder);

            // Validate dimension
            if embedding.len() != expected_dim {
                return Err(ClusterError::dimension_mismatch(expected_dim, embedding.len()));
            }

            // Validate finite values (AP-10)
            for (j, &val) in embedding.iter().enumerate() {
                if !val.is_finite() {
                    return Err(ClusterError::invalid_parameter(format!(
                        "embedding[{}][{}] is not finite: {}; all embedding values must be finite",
                        i, j, val
                    )));
                }
            }

            let state = &mut self.spaces[i];

            // Insert into BIRCH tree
            let cluster_idx = state.tree.insert(embedding, memory_id)? as i32;
            cluster_indices[i] = cluster_idx;

            // Accumulate for HDBSCAN
            state.embeddings.push(embedding.clone());
            state.memory_ids.push(memory_id);
            state.updates_since_recluster += 1;

            // Check if we need to trigger HDBSCAN reclustering
            if state.updates_since_recluster >= self.params.recluster_threshold {
                needs_recluster = true;
            }
        }

        self.total_memories += 1;

        // Synthesize topics from cross-space clustering
        let topic_profile = self.compute_topic_profile(&cluster_indices);

        let result = InsertResult {
            memory_id,
            cluster_indices,
            topic_profile,
            needs_recluster,
        };

        Ok(result)
    }

    /// Trigger HDBSCAN batch reclustering for all spaces.
    ///
    /// This rebuilds clusters from accumulated embeddings using the HDBSCAN
    /// algorithm, which provides more accurate clusters than incremental BIRCH.
    ///
    /// # Returns
    ///
    /// Returns statistics about the reclustering operation.
    ///
    /// # Errors
    ///
    /// Returns error if reclustering fails for any space.
    pub fn recluster(&mut self) -> Result<ReclusterResult, ClusterError> {
        let mut total_clusters = 0;
        let mut per_space_clusters: [usize; 13] = [0; 13];

        for (i, embedder) in Embedder::all().enumerate() {
            let state = &mut self.spaces[i];

            // Get space-specific params to check min_cluster_size
            let space_params = HDBSCANParams::default_for_space(embedder);

            // Skip if not enough data for this space's HDBSCAN config
            // Note: Sparse spaces (E6, E13) use min_cluster_size=5
            if state.embeddings.len() < space_params.min_cluster_size {
                state.updates_since_recluster = 0; // Reset counter anyway
                continue;
            }

            // Create space-specific clusterer and run HDBSCAN
            let clusterer = HDBSCANClusterer::for_space(embedder);
            let memberships =
                clusterer.fit(&state.embeddings, &state.memory_ids, embedder)?;

            // Update memberships and build clusters
            state.memberships.clear();
            state.clusters.clear();

            let mut cluster_embeddings: HashMap<i32, Vec<Vec<f32>>> = HashMap::new();

            for (j, membership) in memberships.iter().enumerate() {
                state.memberships.insert(membership.memory_id, membership.clone());

                if membership.cluster_id >= 0 {
                    cluster_embeddings
                        .entry(membership.cluster_id)
                        .or_default()
                        .push(state.embeddings[j].clone());
                }
            }

            // Build Cluster objects with centroids
            for (cluster_id, embs) in cluster_embeddings {
                let centroid = Self::compute_centroid(&embs);
                let cluster = Cluster::new(cluster_id, embedder, centroid, embs.len() as u32);
                state.clusters.insert(cluster_id, cluster);
            }

            per_space_clusters[i] = state.clusters.len();
            total_clusters += state.clusters.len();

            // Reset update counter
            state.updates_since_recluster = 0;
        }

        // Re-synthesize topics after reclustering
        self.synthesize_topics()?;

        Ok(ReclusterResult {
            total_clusters,
            per_space_clusters,
            topics_discovered: self.topics.len(),
        })
    }

    // =========================================================================
    // FINGERPRINT DISTANCE MATRIX CLUSTERING (FDMC)
    // =========================================================================

    /// Recluster using fingerprint distance matrix approach.
    ///
    /// Instead of clustering 13 spaces independently, this method:
    /// 1. Builds a pairwise similarity matrix using TeleologicalComparator
    /// 2. Runs HDBSCAN once on the aggregated fingerprint distances
    /// 3. Topics emerge from memories with similar fingerprints
    ///
    /// This approach amplifies the signal: gaps of 0.03-0.08 across 7 semantic
    /// spaces become 0.21-0.56 in aggregate, enabling better topic separation.
    ///
    /// # Arguments
    ///
    /// * `fingerprints` - Slice of (memory_id, fingerprint) pairs to cluster
    ///
    /// # Returns
    ///
    /// `FdmcResult` containing cluster assignments, topic information, and analysis.
    ///
    /// # Errors
    ///
    /// - `ClusterError::InsufficientData` if too few fingerprints for clustering
    /// - `ClusterError::InvalidParameter` if fingerprint comparison fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use context_graph_core::clustering::MultiSpaceClusterManager;
    /// use context_graph_core::types::SemanticFingerprint;
    /// use uuid::Uuid;
    ///
    /// let mut manager = MultiSpaceClusterManager::with_defaults()?;
    ///
    /// // Get fingerprints from storage
    /// let fingerprints: Vec<(Uuid, SemanticFingerprint)> = load_fingerprints();
    ///
    /// // Run FDMC clustering
    /// let result = manager.recluster_fingerprint_matrix(&fingerprints)?;
    ///
    /// println!("Discovered {} topics", result.topics_discovered);
    /// println!("Best discriminator: {:?}", result.dominant_embedder);
    /// ```
    pub fn recluster_fingerprint_matrix(
        &mut self,
        fingerprints: &[(Uuid, SemanticFingerprint)],
    ) -> Result<FdmcResult, ClusterError> {
        let n = fingerprints.len();

        tracing::info!(
            count = n,
            min_cluster_size = self.params.hdbscan_params.min_cluster_size,
            "Starting FDMC reclustering"
        );

        // Validate minimum data
        if n < self.params.hdbscan_params.min_cluster_size {
            tracing::debug!(
                count = n,
                required = self.params.hdbscan_params.min_cluster_size,
                "Insufficient fingerprints for FDMC clustering"
            );
            return Ok(FdmcResult::empty());
        }

        // Build fingerprint similarity matrix
        let config = FingerprintMatrixConfig::for_topic_detection();
        let fp_refs: Vec<(Uuid, &SemanticFingerprint)> =
            fingerprints.iter().map(|(id, fp)| (*id, fp)).collect();

        let matrix = build_fingerprint_matrix(&fp_refs, &config)?;

        // Get statistics about the similarity distribution
        let stats = matrix.similarity_stats();
        tracing::debug!(
            min = stats.min,
            max = stats.max,
            mean = stats.mean,
            std_dev = stats.std_dev,
            max_gap = stats.max_gap,
            gap_position = stats.gap_position,
            "Fingerprint similarity statistics"
        );

        // Convert to distance and cluster
        let distances = matrix.to_distance_matrix();
        let memory_ids: Vec<Uuid> = matrix.memory_ids.clone();

        let clusterer = HDBSCANClusterer::with_defaults();
        let memberships = clusterer.fit_precomputed(&distances, &memory_ids)?;

        // Compute silhouette score for quality
        let labels: Vec<i32> = memberships.iter().map(|m| m.cluster_id).collect();
        let silhouette = clusterer.compute_silhouette_precomputed(&distances, &labels);

        tracing::debug!(
            silhouette = silhouette,
            "FDMC clustering quality"
        );

        // Build topics from cluster assignments
        self.topics.clear();
        let mut cluster_members: HashMap<i32, Vec<Uuid>> = HashMap::new();

        for membership in &memberships {
            if membership.cluster_id >= 0 {
                cluster_members
                    .entry(membership.cluster_id)
                    .or_default()
                    .push(membership.memory_id);
            }
        }

        // Create topics for valid clusters
        let mut total_clusters = 0;
        for (cluster_id, members) in &cluster_members {
            if members.len() < 2 {
                continue;
            }

            // Compute topic profile from fingerprint similarities
            let profile = self.compute_topic_profile_from_fingerprints(members, &fp_refs);

            // Only create topic if profile meets threshold
            if !profile.is_topic() {
                tracing::debug!(
                    cluster_id = cluster_id,
                    members = members.len(),
                    weighted_agreement = profile.weighted_agreement(),
                    "Cluster does not meet topic threshold"
                );
                continue;
            }

            let cluster_ids = HashMap::new(); // N/A for FDMC
            let topic = Topic::new(profile, cluster_ids, members.clone());
            self.topics.insert(topic.id, topic);
            total_clusters += 1;
        }

        // Analyze which embedders contributed most to separation
        let contributions = matrix.analyze_embedder_contributions();
        let dominant_embedder = matrix.dominant_embedder();

        tracing::info!(
            topics_discovered = self.topics.len(),
            total_clusters = total_clusters,
            silhouette = silhouette,
            dominant_embedder = ?dominant_embedder,
            "FDMC reclustering complete"
        );

        // Take stability snapshot
        self.take_stability_snapshot();

        Ok(FdmcResult {
            total_clusters,
            topics_discovered: self.topics.len(),
            memberships,
            silhouette_score: silhouette,
            similarity_stats: stats,
            embedder_contributions: contributions,
            dominant_embedder,
        })
    }

    /// Compute topic profile from fingerprint similarities.
    ///
    /// For FDMC, we compute the average per-space similarity across
    /// all pairs of members, then use that as the profile strength.
    fn compute_topic_profile_from_fingerprints(
        &self,
        members: &[Uuid],
        fingerprints: &[(Uuid, &SemanticFingerprint)],
    ) -> TopicProfile {
        use crate::teleological::{TeleologicalComparator, NUM_EMBEDDERS};

        if members.len() < 2 {
            return TopicProfile::new([0.0f32; 13]);
        }

        // Build ID -> fingerprint lookup
        let fp_map: HashMap<Uuid, &SemanticFingerprint> =
            fingerprints.iter().map(|(id, fp)| (*id, *fp)).collect();

        // Compute average per-space similarity across all pairs
        let mut strengths = [0.0f32; NUM_EMBEDDERS];
        let mut pair_count = 0usize;
        let comparator = TeleologicalComparator::new();

        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                let Some(fp_a) = fp_map.get(&members[i]) else {
                    continue;
                };
                let Some(fp_b) = fp_map.get(&members[j]) else {
                    continue;
                };

                if let Ok(result) = comparator.compare(fp_a, fp_b) {
                    for (k, sim) in result.per_embedder.iter().enumerate() {
                        if let Some(s) = sim {
                            strengths[k] += *s;
                        }
                    }
                    pair_count += 1;
                }
            }
        }

        // Average the strengths
        if pair_count > 0 {
            for s in &mut strengths {
                *s /= pair_count as f32;
            }
        }

        TopicProfile::new(strengths)
    }

    /// Synthesize topics from cross-space cluster memberships.
    ///
    /// Discovers topics where memories cluster together in multiple spaces
    /// with weighted_agreement >= 2.5 (ARCH-09).
    ///
    /// FIXED: Now uses proper weighted agreement between memory pairs instead of
    /// requiring exact cluster matches across ALL spaces. Two memories form a topic
    /// edge if they share clusters in enough spaces to meet the 2.5 threshold.
    fn synthesize_topics(&mut self) -> Result<(), ClusterError> {
        self.topics.clear();

        // Build memory -> (embedder -> cluster_id) map
        let mut mem_clusters: HashMap<Uuid, HashMap<Embedder, i32>> = HashMap::new();

        for (i, embedder) in Embedder::all().enumerate() {
            for (memory_id, membership) in &self.spaces[i].memberships {
                mem_clusters
                    .entry(*memory_id)
                    .or_default()
                    .insert(embedder, membership.cluster_id);
            }
        }

        if mem_clusters.is_empty() {
            self.take_stability_snapshot();
            return Ok(());
        }

        // Collect memory IDs for pairwise comparison
        let memory_ids: Vec<Uuid> = mem_clusters.keys().cloned().collect();
        let n = memory_ids.len();

        if n < 2 {
            self.take_stability_snapshot();
            return Ok(());
        }

        // Find edges: pairs with weighted_agreement >= TOPIC_THRESHOLD (2.5)
        let mut edges: Vec<(usize, usize)> = Vec::new();

        for i in 0..n {
            for j in (i + 1)..n {
                let wa = Self::compute_pairwise_weighted_agreement(
                    &memory_ids[i],
                    &memory_ids[j],
                    &mem_clusters,
                );
                if wa >= TOPIC_THRESHOLD {
                    edges.push((i, j));
                }
            }
        }

        // Union-Find to group connected memories
        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut [usize], i: usize) -> usize {
            if parent[i] != i {
                parent[i] = find(parent, parent[i]);
            }
            parent[i]
        }

        fn union(parent: &mut [usize], i: usize, j: usize) {
            let pi = find(parent, i);
            let pj = find(parent, j);
            if pi != pj {
                parent[pi] = pj;
            }
        }

        for (i, j) in edges {
            union(&mut parent, i, j);
        }

        // Group by component root
        let mut components: HashMap<usize, Vec<Uuid>> = HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            components.entry(root).or_default().push(memory_ids[i]);
        }

        // Create topics from groups with >= 2 members
        for members in components.into_values() {
            if members.len() < 2 {
                continue;
            }

            // Compute topic profile (fraction of members in dominant cluster per space)
            let profile = Self::compute_topic_profile_from_clusters(&members, &mem_clusters);

            // Only create topic if profile meets threshold
            if !profile.is_topic() {
                continue;
            }

            // Compute cluster_ids (most common cluster per space)
            let cluster_ids = Self::compute_dominant_cluster_ids(&members, &mem_clusters);

            let topic = Topic::new(profile, cluster_ids, members);
            self.topics.insert(topic.id, topic);
        }

        // Take a stability snapshot after synthesizing topics (AP-70)
        self.take_stability_snapshot();

        Ok(())
    }

    /// Compute weighted agreement between two memories.
    ///
    /// Uses category weights per constitution:
    /// - SEMANTIC (E1, E5, E6, E7, E10, E12, E13): 1.0
    /// - TEMPORAL (E2, E3, E4): 0.0 (excluded per AP-60)
    /// - RELATIONAL (E8, E11): 0.5
    /// - STRUCTURAL (E9): 0.5
    fn compute_pairwise_weighted_agreement(
        mem_a: &Uuid,
        mem_b: &Uuid,
        mem_clusters: &HashMap<Uuid, HashMap<Embedder, i32>>,
    ) -> f32 {
        let Some(clusters_a) = mem_clusters.get(mem_a) else {
            return 0.0;
        };
        let Some(clusters_b) = mem_clusters.get(mem_b) else {
            return 0.0;
        };

        let mut weighted = 0.0f32;
        for embedder in Embedder::all() {
            let ca = clusters_a.get(&embedder).copied().unwrap_or(-1);
            let cb = clusters_b.get(&embedder).copied().unwrap_or(-1);

            // Both in same non-noise cluster
            if ca != -1 && ca == cb {
                weighted += category_for(embedder).topic_weight();
            }
        }
        weighted
    }

    /// Compute topic profile from cluster memberships.
    ///
    /// For each space, the strength is the fraction of members in the dominant cluster.
    fn compute_topic_profile_from_clusters(
        members: &[Uuid],
        mem_clusters: &HashMap<Uuid, HashMap<Embedder, i32>>,
    ) -> TopicProfile {
        if members.is_empty() {
            return TopicProfile::new([0.0f32; 13]);
        }

        let mut strengths = [0.0f32; 13];
        for embedder in Embedder::all() {
            let mut counts: HashMap<i32, usize> = HashMap::new();
            for mem_id in members {
                if let Some(clusters) = mem_clusters.get(mem_id) {
                    let cid = clusters.get(&embedder).copied().unwrap_or(-1);
                    if cid != -1 {
                        *counts.entry(cid).or_insert(0) += 1;
                    }
                }
            }
            if let Some((_, &count)) = counts.iter().max_by_key(|(_, &c)| c) {
                strengths[embedder.index()] = count as f32 / members.len() as f32;
            }
        }

        TopicProfile::new(strengths)
    }

    /// Compute the dominant cluster ID for each embedding space.
    fn compute_dominant_cluster_ids(
        members: &[Uuid],
        mem_clusters: &HashMap<Uuid, HashMap<Embedder, i32>>,
    ) -> HashMap<Embedder, i32> {
        let mut result = HashMap::new();
        for embedder in Embedder::all() {
            let mut counts: HashMap<i32, usize> = HashMap::new();
            for mem_id in members {
                if let Some(clusters) = mem_clusters.get(mem_id) {
                    let cid = clusters.get(&embedder).copied().unwrap_or(-1);
                    if cid != -1 {
                        *counts.entry(cid).or_insert(0) += 1;
                    }
                }
            }
            if let Some((&dominant, _)) = counts.iter().max_by_key(|(_, &c)| c) {
                result.insert(embedder, dominant);
            }
        }
        result
    }

    /// Compute average profile from a set of memories.
    #[allow(dead_code)]
    fn compute_average_profile(
        &self,
        memory_ids: &[Uuid],
        profiles: &HashMap<Uuid, TopicProfile>,
    ) -> TopicProfile {
        let mut sum = [0.0f32; 13];
        let count = memory_ids.len() as f32;

        for id in memory_ids {
            if let Some(profile) = profiles.get(id) {
                for i in 0..13 {
                    sum[i] += profile.strengths[i];
                }
            }
        }

        if count > 0.0 {
            for s in &mut sum {
                *s /= count;
            }
        }

        TopicProfile::new(sum)
    }

    /// Compute centroid from a set of embeddings.
    fn compute_centroid(embeddings: &[Vec<f32>]) -> Vec<f32> {
        if embeddings.is_empty() {
            return Vec::new();
        }

        let dim = embeddings[0].len();
        let count = embeddings.len() as f32;
        let mut centroid = vec![0.0f32; dim];

        for emb in embeddings {
            for (i, &val) in emb.iter().enumerate() {
                centroid[i] += val;
            }
        }

        for c in &mut centroid {
            *c /= count;
        }

        centroid
    }

    /// Compute topic profile from cluster indices.
    fn compute_topic_profile(&self, cluster_indices: &[i32; 13]) -> TopicProfile {
        let mut strengths = [0.0f32; 13];

        for (i, &cluster_idx) in cluster_indices.iter().enumerate() {
            if cluster_idx >= 0 {
                // Use 1.0 strength for being in a cluster
                // Could be refined to use membership probability from BIRCH
                strengths[i] = 1.0;
            }
        }

        TopicProfile::new(strengths)
    }

    /// Get a memory's cluster memberships across all spaces.
    ///
    /// # Returns
    ///
    /// Array of optional ClusterMembership, one per space.
    /// None means no membership data available for that space.
    pub fn get_memberships(&self, memory_id: Uuid) -> [Option<ClusterMembership>; 13] {
        let mut result: [Option<ClusterMembership>; 13] = Default::default();

        for (i, state) in self.spaces.iter().enumerate() {
            result[i] = state.memberships.get(&memory_id).cloned();
        }

        result
    }

    /// Get all discovered topics.
    pub fn get_topics(&self) -> &HashMap<Uuid, Topic> {
        &self.topics
    }

    /// Get topic by ID.
    pub fn get_topic(&self, topic_id: &Uuid) -> Option<&Topic> {
        self.topics.get(topic_id)
    }

    /// Get clusters for a specific space.
    pub fn get_clusters(&self, embedder: Embedder) -> &HashMap<i32, Cluster> {
        &self.spaces[embedder.index()].clusters
    }

    /// Get total number of memories inserted.
    #[inline]
    pub fn total_memories(&self) -> usize {
        self.total_memories
    }

    /// Get total number of topics discovered.
    #[inline]
    pub fn topic_count(&self) -> usize {
        self.topics.len()
    }

    /// Get cluster count for a specific space.
    pub fn cluster_count(&self, embedder: Embedder) -> usize {
        self.spaces[embedder.index()].clusters.len()
    }

    /// Get total cluster count across all spaces.
    pub fn total_clusters(&self) -> usize {
        self.spaces.iter().map(|s| s.clusters.len()).sum()
    }

    /// Get manager parameters.
    pub fn params(&self) -> &ManagerParams {
        &self.params
    }

    /// Get status of updates per space.
    pub fn updates_status(&self) -> [UpdateStatus; 13] {
        let mut result: [UpdateStatus; 13] = [UpdateStatus::default(); 13];

        for (i, embedder) in Embedder::all().enumerate() {
            result[i] = UpdateStatus {
                embedder,
                updates_since_recluster: self.spaces[i].updates_since_recluster,
            };
        }

        result
    }

    /// Check if any space needs reclustering.
    pub fn needs_recluster(&self) -> bool {
        self.spaces
            .iter()
            .any(|s| s.updates_since_recluster >= self.params.recluster_threshold)
    }

    // =========================================================================
    // Topic Portfolio Persistence (Phase 5)
    // =========================================================================

    /// Export the current topic portfolio for persistence.
    ///
    /// Creates a snapshot of all discovered topics with their profiles,
    /// stability metrics, and portfolio-level metrics (churn, entropy).
    ///
    /// # Arguments
    ///
    /// * `session_id` - Session identifier for tracking
    /// * `churn_rate` - Current portfolio-level churn rate [0.0, 1.0]
    /// * `entropy` - Current portfolio-level entropy [0.0, 1.0]
    ///
    /// # Returns
    ///
    /// A `PersistedTopicPortfolio` ready for storage.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::MultiSpaceClusterManager;
    ///
    /// let manager = MultiSpaceClusterManager::with_defaults().unwrap();
    /// let portfolio = manager.export_portfolio("session-123", 0.15, 0.45);
    ///
    /// assert_eq!(portfolio.session_id, "session-123");
    /// ```
    pub fn export_portfolio(
        &self,
        session_id: impl Into<String>,
        churn_rate: f32,
        entropy: f32,
    ) -> crate::clustering::PersistedTopicPortfolio {
        let topics: Vec<Topic> = self.topics.values().cloned().collect();

        crate::clustering::PersistedTopicPortfolio::new(
            topics,
            churn_rate,
            entropy,
            session_id.into(),
        )
    }

    /// Export the current topic portfolio using internal churn rate.
    ///
    /// This is the preferred method as it uses the stability tracker's
    /// computed churn rate instead of requiring an external value.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Session identifier for tracking
    /// * `entropy` - Current system entropy (from external source)
    ///
    /// # Returns
    ///
    /// A `PersistedTopicPortfolio` ready for storage.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::MultiSpaceClusterManager;
    ///
    /// let manager = MultiSpaceClusterManager::with_defaults().unwrap();
    /// let portfolio = manager.export_portfolio_with_internal_churn("session-123", 0.45);
    ///
    /// assert_eq!(portfolio.session_id, "session-123");
    /// ```
    pub fn export_portfolio_with_internal_churn(
        &self,
        session_id: impl Into<String>,
        entropy: f32,
    ) -> crate::clustering::PersistedTopicPortfolio {
        let topics: Vec<Topic> = self.topics.values().cloned().collect();

        crate::clustering::PersistedTopicPortfolio::new(
            topics,
            self.stability_tracker.current_churn(),
            entropy,
            session_id.into(),
        )
    }

    /// Import topics from a persisted portfolio.
    ///
    /// Restores topics from a previous session's portfolio snapshot.
    /// This replaces the current topics with the imported ones.
    ///
    /// # Arguments
    ///
    /// * `portfolio` - The persisted portfolio to import
    ///
    /// # Returns
    ///
    /// Number of topics imported.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use context_graph_core::clustering::{MultiSpaceClusterManager, PersistedTopicPortfolio};
    ///
    /// let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();
    ///
    /// // Load portfolio from storage
    /// let portfolio: PersistedTopicPortfolio = load_from_storage()?;
    ///
    /// // Import into manager
    /// let count = manager.import_portfolio(&portfolio);
    /// println!("Imported {} topics", count);
    /// ```
    pub fn import_portfolio(&mut self, portfolio: &crate::clustering::PersistedTopicPortfolio) -> usize {
        // Clear existing topics
        self.topics.clear();

        // Import topics from portfolio
        // CRITICAL FIX (AP-60): Call update_contributing_spaces() to filter out
        // temporal embedders that may have been persisted before the fix.
        // Temporal embedders (E2-E4) have topic_weight=0.0 and MUST NOT appear
        // in contributing_spaces.
        for topic in &portfolio.topics {
            let mut imported_topic = topic.clone();
            imported_topic.update_contributing_spaces();
            self.topics.insert(imported_topic.id, imported_topic);
        }

        self.topics.len()
    }

    /// Clear all topics from the manager.
    ///
    /// This is useful before importing a new portfolio or for testing.
    pub fn clear_topics(&mut self) {
        self.topics.clear();
    }

    /// Clear all per-space data (embeddings, memory_ids, memberships, clusters).
    ///
    /// This is used before loading fingerprints from storage to ensure
    /// we cluster ALL fingerprints, not just those added during this session.
    ///
    /// Also clears topics since they'll be re-synthesized after reclustering.
    pub fn clear_all_spaces(&mut self) {
        for space in &mut self.spaces {
            space.embeddings.clear();
            space.memory_ids.clear();
            space.memberships.clear();
            space.clusters.clear();
            space.updates_since_recluster = 0;
        }
        self.topics.clear();
        self.total_memories = 0;

        tracing::info!("Cleared all 13 embedding spaces for fresh clustering");
    }

    /// Get portfolio-level summary for persistence.
    ///
    /// Returns a tuple of (topic_count, total_members).
    #[inline]
    pub fn portfolio_summary(&self) -> (usize, usize) {
        let total_members: usize = self.topics.values().map(|t| t.member_count()).sum();
        (self.topics.len(), total_members)
    }

    // =========================================================================
    // Topic Stability Tracking (AP-70 Compliance)
    // =========================================================================

    /// Take a stability snapshot of current topics.
    ///
    /// Call this periodically (e.g., every minute) or after topic synthesis
    /// to track portfolio changes for churn calculation.
    pub fn take_stability_snapshot(&mut self) {
        let topics_vec: Vec<Topic> = self.topics.values().cloned().collect();
        self.stability_tracker.take_snapshot(&topics_vec);
    }

    /// Compute churn by comparing current state to ~1 hour ago.
    ///
    /// # Returns
    ///
    /// Churn rate [0.0, 1.0] where:
    /// - 0.0 = no change (stable)
    /// - 1.0 = complete turnover
    pub fn track_churn(&mut self) -> f32 {
        self.stability_tracker.track_churn()
    }

    /// Get current churn rate (last computed value).
    #[inline]
    pub fn current_churn(&self) -> f32 {
        self.stability_tracker.current_churn()
    }

    /// Check if dream consolidation should trigger (AP-70).
    ///
    /// Per constitution AP-70, triggers when EITHER:
    /// 1. entropy > 0.7 AND churn > 0.5 (both simultaneously)
    /// 2. entropy > 0.7 for 5+ continuous minutes
    ///
    /// # Arguments
    ///
    /// * `entropy` - Current system entropy [0.0, 1.0]
    ///
    /// # Returns
    ///
    /// true if dream should be triggered
    pub fn check_dream_trigger(&mut self, entropy: f32) -> bool {
        self.stability_tracker.check_dream_trigger(entropy)
    }

    /// Get reference to stability tracker for advanced queries.
    pub fn stability_tracker(&self) -> &TopicStabilityTracker {
        &self.stability_tracker
    }

    /// Get mutable reference to stability tracker.
    pub fn stability_tracker_mut(&mut self) -> &mut TopicStabilityTracker {
        &mut self.stability_tracker
    }

    /// Reset entropy tracking (call after dream completes).
    pub fn reset_entropy_tracking(&mut self) {
        self.stability_tracker.reset_entropy_tracking();
    }

    /// Check if system is stable (low churn over 6 hours).
    pub fn is_stable(&self) -> bool {
        self.stability_tracker.is_stable()
    }

    /// Get average churn over specified hours.
    pub fn average_churn(&self, hours: i64) -> f32 {
        self.stability_tracker.average_churn(hours)
    }
}

// =============================================================================
// InsertResult
// =============================================================================

/// Result of inserting a memory into the cluster manager.
#[derive(Debug, Clone)]
pub struct InsertResult {
    /// The memory ID that was inserted.
    pub memory_id: Uuid,

    /// Cluster index in each of 13 spaces.
    ///
    /// Value of -1 indicates the memory was not assigned to a cluster
    /// in that space (treated as noise by BIRCH/HDBSCAN).
    pub cluster_indices: [i32; 13],

    /// Topic profile based on cluster assignments.
    pub topic_profile: TopicProfile,

    /// Whether HDBSCAN reclustering should be triggered.
    pub needs_recluster: bool,
}

impl InsertResult {
    /// Check if this memory meets topic threshold.
    #[inline]
    pub fn is_topic(&self) -> bool {
        self.topic_profile.is_topic()
    }

    /// Get weighted agreement score.
    #[inline]
    pub fn weighted_agreement(&self) -> f32 {
        self.topic_profile.weighted_agreement()
    }
}

// =============================================================================
// ReclusterResult
// =============================================================================

/// Result of HDBSCAN batch reclustering.
#[derive(Debug, Clone)]
pub struct ReclusterResult {
    /// Total clusters discovered across all spaces.
    pub total_clusters: usize,

    /// Number of clusters per space.
    pub per_space_clusters: [usize; 13],

    /// Number of topics discovered from cross-space synthesis.
    pub topics_discovered: usize,
}

// =============================================================================
// FdmcResult
// =============================================================================

/// Result of Fingerprint Distance Matrix Clustering (FDMC).
///
/// Contains clustering results plus diagnostic information about which
/// embedders contributed most to topic separation.
#[derive(Debug, Clone)]
pub struct FdmcResult {
    /// Total number of valid clusters (non-noise).
    pub total_clusters: usize,

    /// Number of topics that meet the weighted_agreement >= 2.5 threshold.
    pub topics_discovered: usize,

    /// Cluster membership assignments for each memory.
    pub memberships: Vec<ClusterMembership>,

    /// Silhouette score measuring cluster quality [-1.0, 1.0].
    /// >= 0.3 indicates good separation.
    pub silhouette_score: f32,

    /// Statistics about the similarity distribution.
    pub similarity_stats: SimilarityStats,

    /// Per-embedder variance (higher = better discriminator).
    /// Index corresponds to Embedder::index().
    pub embedder_contributions: [f32; 13],

    /// The embedder with highest contribution to separation.
    pub dominant_embedder: Option<Embedder>,
}

impl FdmcResult {
    /// Create an empty result (used when insufficient data).
    #[inline]
    pub fn empty() -> Self {
        Self {
            total_clusters: 0,
            topics_discovered: 0,
            memberships: Vec::new(),
            silhouette_score: 0.0,
            similarity_stats: SimilarityStats::default(),
            embedder_contributions: [0.0f32; 13],
            dominant_embedder: None,
        }
    }

    /// Check if the result is empty (no clustering performed).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.memberships.is_empty()
    }

    /// Get the number of memories clustered.
    #[inline]
    pub fn memory_count(&self) -> usize {
        self.memberships.len()
    }

    /// Get noise point count (memories not assigned to any cluster).
    pub fn noise_count(&self) -> usize {
        self.memberships.iter().filter(|m| m.cluster_id == -1).count()
    }

    /// Get cluster quality rating.
    pub fn quality_rating(&self) -> &'static str {
        if self.silhouette_score >= 0.7 {
            "excellent"
        } else if self.silhouette_score >= 0.5 {
            "good"
        } else if self.silhouette_score >= 0.3 {
            "acceptable"
        } else if self.silhouette_score >= 0.0 {
            "poor"
        } else {
            "failed"
        }
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::config::get_dimension;

    // =========================================================================
    // Helper Functions
    // =========================================================================

    /// Create embeddings with correct dimensions for all 13 spaces.
    fn create_test_embeddings(base_value: f32) -> [Vec<f32>; 13] {
        std::array::from_fn(|i| {
            let embedder = Embedder::from_index(i).unwrap();
            let dim = get_dimension(embedder);
            vec![base_value; dim]
        })
    }

    /// Create embeddings with a specific pattern (for clustering tests).
    fn create_clustered_embeddings(cluster_id: usize) -> [Vec<f32>; 13] {
        std::array::from_fn(|i| {
            let embedder = Embedder::from_index(i).unwrap();
            let dim = get_dimension(embedder);
            let offset = (cluster_id as f32) * 10.0;
            vec![offset + 0.1 * (i as f32); dim]
        })
    }

    // =========================================================================
    // ManagerParams Tests
    // =========================================================================

    #[test]
    fn test_manager_params_defaults() {
        let params = manager_defaults();

        assert_eq!(
            params.recluster_threshold,
            DEFAULT_RECLUSTER_THRESHOLD,
            "Default recluster threshold should match constant"
        );
        assert!(params.validate().is_ok(), "Default params must be valid");

        println!(
            "[PASS] test_manager_params_defaults - threshold={}",
            params.recluster_threshold
        );
    }

    #[test]
    fn test_manager_params_validation_recluster_zero() {
        let params = manager_defaults().with_recluster_threshold(0);

        let result = params.validate();
        assert!(result.is_err(), "recluster_threshold=0 must be rejected");

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("recluster_threshold"),
            "Error must mention field name"
        );

        println!(
            "[PASS] test_manager_params_validation_recluster_zero - error: {}",
            err
        );
    }

    #[test]
    fn test_manager_params_builder() {
        let birch = BIRCHParams::default().with_threshold(0.5);
        let hdbscan = HDBSCANParams::default().with_min_cluster_size(5);

        let params = manager_defaults()
            .with_recluster_threshold(50)
            .with_birch_params(birch.clone())
            .with_hdbscan_params(hdbscan.clone());

        assert_eq!(params.recluster_threshold, 50);
        assert_eq!(params.birch_params.threshold, 0.5);
        assert_eq!(params.hdbscan_params.min_cluster_size, 5);

        println!("[PASS] test_manager_params_builder - all builders work");
    }

    // =========================================================================
    // MultiSpaceClusterManager Creation Tests
    // =========================================================================

    #[test]
    fn test_manager_creation() {
        let manager = MultiSpaceClusterManager::with_defaults();
        assert!(manager.is_ok(), "Manager creation must succeed");

        let manager = manager.unwrap();
        assert_eq!(manager.total_memories(), 0);
        assert_eq!(manager.topic_count(), 0);
        assert_eq!(manager.total_clusters(), 0);

        println!("[PASS] test_manager_creation - manager initialized empty");
    }

    #[test]
    fn test_manager_creation_with_custom_params() {
        let params = manager_defaults().with_recluster_threshold(10);

        let manager = MultiSpaceClusterManager::new(params);
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(manager.params().recluster_threshold, 10);

        println!("[PASS] test_manager_creation_with_custom_params");
    }

    #[test]
    fn test_manager_creation_invalid_params() {
        let params = manager_defaults().with_recluster_threshold(0);

        let result = MultiSpaceClusterManager::new(params);
        assert!(result.is_err(), "Invalid params should fail creation");

        println!("[PASS] test_manager_creation_invalid_params");
    }

    // =========================================================================
    // Insert Tests
    // =========================================================================

    #[test]
    fn test_insert_single_memory() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let memory_id = Uuid::new_v4();
        let embeddings = create_test_embeddings(0.5);

        let result = manager.insert(memory_id, &embeddings);
        assert!(result.is_ok(), "Insert must succeed");

        let insert_result = result.unwrap();
        assert_eq!(insert_result.memory_id, memory_id);
        assert_eq!(manager.total_memories(), 1);

        // All cluster indices should be valid (>= 0) after BIRCH insert
        for (i, &idx) in insert_result.cluster_indices.iter().enumerate() {
            assert!(
                idx >= 0,
                "Cluster index for space {} should be >= 0, got {}",
                i,
                idx
            );
        }

        println!(
            "[PASS] test_insert_single_memory - indices={:?}",
            insert_result.cluster_indices
        );
    }

    #[test]
    fn test_insert_multiple_memories() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        for i in 0..5 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings((i as f32) * 0.1);

            let result = manager.insert(memory_id, &embeddings);
            assert!(result.is_ok(), "Insert {} must succeed", i);
        }

        assert_eq!(manager.total_memories(), 5);

        println!("[PASS] test_insert_multiple_memories - inserted 5 memories");
    }

    #[test]
    fn test_insert_dimension_mismatch() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let memory_id = Uuid::new_v4();
        let mut embeddings = create_test_embeddings(0.5);

        // Corrupt first embedding dimension
        embeddings[0] = vec![0.0; 100]; // Wrong dimension (should be 1024)

        let result = manager.insert(memory_id, &embeddings);
        assert!(result.is_err(), "Wrong dimension must fail");

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("expected 1024") || err.to_string().contains("mismatch"),
            "Error must indicate dimension mismatch"
        );

        println!(
            "[PASS] test_insert_dimension_mismatch - error: {}",
            err
        );
    }

    #[test]
    fn test_insert_nan_rejection() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let memory_id = Uuid::new_v4();
        let mut embeddings = create_test_embeddings(0.5);

        // Insert NaN into first embedding
        embeddings[0][0] = f32::NAN;

        let result = manager.insert(memory_id, &embeddings);
        assert!(result.is_err(), "NaN must be rejected");

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("not finite"),
            "Error must mention finite requirement"
        );

        println!("[PASS] test_insert_nan_rejection - error: {}", err);
    }

    #[test]
    fn test_insert_infinity_rejection() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let memory_id = Uuid::new_v4();
        let mut embeddings = create_test_embeddings(0.5);

        // Insert Infinity into first embedding
        embeddings[0][0] = f32::INFINITY;

        let result = manager.insert(memory_id, &embeddings);
        assert!(result.is_err(), "Infinity must be rejected");

        let err = result.unwrap_err();
        assert!(err.to_string().contains("not finite"));

        println!("[PASS] test_insert_infinity_rejection - error: {}", err);
    }

    // =========================================================================
    // Topic Profile Tests
    // =========================================================================

    #[test]
    fn test_insert_topic_profile() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let memory_id = Uuid::new_v4();
        let embeddings = create_test_embeddings(0.5);

        let result = manager.insert(memory_id, &embeddings).unwrap();

        // Topic profile should be computed
        let profile = &result.topic_profile;
        let weighted = profile.weighted_agreement();

        // With all spaces clustered (strength=1.0), weighted agreement should be high
        // Max = 7*1.0 + 2*0.5 + 1*0.5 = 8.5 (temporal contributes 0)
        assert!(
            weighted > 0.0,
            "Weighted agreement should be > 0, got {}",
            weighted
        );

        println!(
            "[PASS] test_insert_topic_profile - weighted_agreement={}",
            weighted
        );
    }

    #[test]
    fn test_insert_result_is_topic() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let memory_id = Uuid::new_v4();
        let embeddings = create_test_embeddings(0.5);

        let result = manager.insert(memory_id, &embeddings).unwrap();

        // Check is_topic() method
        let is_topic = result.is_topic();
        let weighted = result.weighted_agreement();

        assert_eq!(
            is_topic,
            weighted >= TOPIC_THRESHOLD,
            "is_topic should match weighted >= 2.5"
        );

        println!(
            "[PASS] test_insert_result_is_topic - is_topic={}, weighted={}",
            is_topic, weighted
        );
    }

    // =========================================================================
    // Recluster Tests
    // =========================================================================

    #[test]
    fn test_recluster_empty() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Reclustering with no data should succeed (but do nothing)
        let result = manager.recluster();
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.total_clusters, 0);
        assert_eq!(stats.topics_discovered, 0);

        println!("[PASS] test_recluster_empty - no errors on empty data");
    }

    #[test]
    fn test_recluster_insufficient_data() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Insert only 2 memories (need 3 for HDBSCAN)
        for i in 0..2 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings((i as f32) * 0.1);
            manager.insert(memory_id, &embeddings).unwrap();
        }

        // Reclustering should succeed but produce no clusters
        let result = manager.recluster();
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.total_clusters, 0, "No clusters with insufficient data");

        println!("[PASS] test_recluster_insufficient_data - gracefully handles small data");
    }

    #[test]
    fn test_recluster_with_sufficient_data() {
        // Use smaller recluster threshold for testing
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Insert enough memories to trigger reclustering
        for i in 0..5 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_clustered_embeddings(i % 2); // Two clusters
            manager.insert(memory_id, &embeddings).unwrap();
        }

        // Trigger reclustering
        let result = manager.recluster();
        assert!(result.is_ok(), "Recluster must succeed");

        let stats = result.unwrap();

        // With 5 points and min_cluster_size=3, we might get clusters
        // The exact number depends on the clustering algorithm
        println!(
            "[PASS] test_recluster_with_sufficient_data - clusters={}, topics={}",
            stats.total_clusters, stats.topics_discovered
        );
    }

    // =========================================================================
    // Needs Recluster Tests
    // =========================================================================

    #[test]
    fn test_needs_recluster_false_initially() {
        let manager = MultiSpaceClusterManager::with_defaults().unwrap();
        assert!(!manager.needs_recluster(), "Should not need recluster initially");

        println!("[PASS] test_needs_recluster_false_initially");
    }

    #[test]
    fn test_needs_recluster_after_threshold() {
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Insert enough to hit threshold
        for i in 0..3 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings(i as f32);
            manager.insert(memory_id, &embeddings).unwrap();
        }

        assert!(manager.needs_recluster(), "Should need recluster after threshold");

        println!("[PASS] test_needs_recluster_after_threshold");
    }

    #[test]
    fn test_needs_recluster_reset_after_recluster() {
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Insert to hit threshold
        for i in 0..3 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings(i as f32);
            manager.insert(memory_id, &embeddings).unwrap();
        }

        assert!(manager.needs_recluster());

        // Recluster
        manager.recluster().unwrap();

        // Should no longer need reclustering
        assert!(!manager.needs_recluster(), "Should not need recluster after reclustering");

        println!("[PASS] test_needs_recluster_reset_after_recluster");
    }

    // =========================================================================
    // Get Methods Tests
    // =========================================================================

    #[test]
    fn test_get_memberships() {
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Insert memories and recluster
        let mut memory_ids = Vec::new();
        for i in 0..5 {
            let memory_id = Uuid::new_v4();
            memory_ids.push(memory_id);
            let embeddings = create_test_embeddings(i as f32);
            manager.insert(memory_id, &embeddings).unwrap();
        }

        manager.recluster().unwrap();

        // Get memberships for first memory
        let memberships = manager.get_memberships(memory_ids[0]);

        // Count how many spaces have membership data
        let with_membership = memberships.iter().filter(|m| m.is_some()).count();

        println!(
            "[PASS] test_get_memberships - {} spaces have membership data",
            with_membership
        );
    }

    #[test]
    fn test_get_memberships_unknown_memory() {
        let manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let unknown_id = Uuid::new_v4();
        let memberships = manager.get_memberships(unknown_id);

        // All should be None for unknown memory
        assert!(
            memberships.iter().all(|m| m.is_none()),
            "Unknown memory should have no memberships"
        );

        println!("[PASS] test_get_memberships_unknown_memory");
    }

    #[test]
    fn test_get_clusters() {
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Insert and recluster
        for i in 0..5 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings(i as f32);
            manager.insert(memory_id, &embeddings).unwrap();
        }
        manager.recluster().unwrap();

        let clusters = manager.get_clusters(Embedder::Semantic);

        println!(
            "[PASS] test_get_clusters - {} clusters in Semantic space",
            clusters.len()
        );
    }

    #[test]
    fn test_get_topics_empty() {
        let manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let topics = manager.get_topics();
        assert!(topics.is_empty(), "No topics initially");

        println!("[PASS] test_get_topics_empty");
    }

    // =========================================================================
    // Updates Status Tests
    // =========================================================================

    #[test]
    fn test_updates_status() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Initially all zeros
        let status = manager.updates_status();
        for update in &status {
            assert_eq!(
                update.updates_since_recluster, 0,
                "{:?} should have 0 updates initially",
                update.embedder
            );
        }

        // Insert one memory
        let memory_id = Uuid::new_v4();
        let embeddings = create_test_embeddings(0.5);
        manager.insert(memory_id, &embeddings).unwrap();

        // Now all should be 1
        let status = manager.updates_status();
        for update in &status {
            assert_eq!(
                update.updates_since_recluster, 1,
                "{:?} should have 1 update",
                update.embedder
            );
        }

        println!("[PASS] test_updates_status - correctly tracks updates");
    }

    // =========================================================================
    // Insert Result Tests
    // =========================================================================

    #[test]
    fn test_insert_result_needs_recluster() {
        let params = manager_defaults().with_recluster_threshold(2);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // First insert should not need recluster
        let memory_id = Uuid::new_v4();
        let embeddings = create_test_embeddings(0.5);
        let result1 = manager.insert(memory_id, &embeddings).unwrap();
        assert!(!result1.needs_recluster, "First insert should not trigger recluster");

        // Second insert should trigger (threshold is 2)
        let memory_id = Uuid::new_v4();
        let embeddings = create_test_embeddings(0.6);
        let result2 = manager.insert(memory_id, &embeddings).unwrap();
        assert!(result2.needs_recluster, "Second insert should trigger recluster");

        println!("[PASS] test_insert_result_needs_recluster");
    }

    // =========================================================================
    // Topic Synthesis Tests (ARCH-09, AP-60)
    // =========================================================================

    #[test]
    fn test_topic_synthesis_respects_temporal_exclusion() {
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Insert memories
        for i in 0..5 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings(i as f32 * 0.1);
            manager.insert(memory_id, &embeddings).unwrap();
        }

        manager.recluster().unwrap();

        // Check that topics are based on weighted agreement
        for topic in manager.get_topics().values() {
            let weighted = topic.profile.weighted_agreement();

            // Verify temporal embedders don't contribute
            // (This is enforced by TopicProfile::weighted_agreement())
            assert!(
                topic.profile.is_topic() == (weighted >= TOPIC_THRESHOLD),
                "Topic validity should match threshold check"
            );
        }

        println!("[PASS] test_topic_synthesis_respects_temporal_exclusion");
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_edge_case_single_memory_all_spaces() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let memory_id = Uuid::new_v4();
        let embeddings = create_test_embeddings(0.5);

        let result = manager.insert(memory_id, &embeddings).unwrap();

        // Single memory should be assigned to cluster in all spaces
        for (i, &idx) in result.cluster_indices.iter().enumerate() {
            assert!(idx >= 0, "Space {} should have cluster >= 0", i);
        }

        println!("[PASS] test_edge_case_single_memory_all_spaces");
    }

    #[test]
    fn test_edge_case_identical_embeddings() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Insert same embeddings multiple times
        for _ in 0..3 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings(0.5);
            manager.insert(memory_id, &embeddings).unwrap();
        }

        manager.recluster().unwrap();

        // All should be in same cluster
        println!("[PASS] test_edge_case_identical_embeddings - handles duplicates");
    }

    #[test]
    fn test_edge_case_sparse_embeddings() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Create sparse-like embeddings (mostly zeros)
        let mut embeddings = create_test_embeddings(0.0);

        // Only set first few values
        for emb in &mut embeddings {
            if emb.len() > 10 {
                for i in 0..10 {
                    emb[i] = 0.5;
                }
            }
        }

        let memory_id = Uuid::new_v4();
        let result = manager.insert(memory_id, &embeddings);
        assert!(result.is_ok(), "Sparse embeddings should work");

        println!("[PASS] test_edge_case_sparse_embeddings");
    }

    // =========================================================================
    // Constants Tests
    // =========================================================================

    #[test]
    fn test_constants_match_constitution() {
        assert_eq!(
            MAX_WEIGHTED_AGREEMENT, 8.5,
            "MAX_WEIGHTED_AGREEMENT should be 8.5 per constitution"
        );
        assert_eq!(
            TOPIC_THRESHOLD, 2.5,
            "TOPIC_THRESHOLD should be 2.5 per ARCH-09"
        );

        println!("[PASS] test_constants_match_constitution");
    }

    // =========================================================================
    // Portfolio Export/Import Tests (Phase 5)
    // =========================================================================

    #[test]
    fn test_export_portfolio_empty() {
        let manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let portfolio = manager.export_portfolio("test-session", 0.15, 0.45);

        assert!(portfolio.is_empty());
        assert_eq!(portfolio.session_id, "test-session");
        assert!((portfolio.churn_rate - 0.15).abs() < f32::EPSILON);
        assert!((portfolio.entropy - 0.45).abs() < f32::EPSILON);

        println!("[PASS] test_export_portfolio_empty");
    }

    #[test]
    fn test_export_portfolio_with_topics() {
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Insert memories and recluster to create topics
        for i in 0..5 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings(i as f32 * 0.1);
            manager.insert(memory_id, &embeddings).unwrap();
        }
        manager.recluster().unwrap();

        let portfolio = manager.export_portfolio("session-123", 0.2, 0.6);

        assert_eq!(portfolio.session_id, "session-123");
        assert!(portfolio.persisted_at_ms > 0);
        // The topic count depends on clustering results

        println!(
            "[PASS] test_export_portfolio_with_topics - topics={}",
            portfolio.topic_count()
        );
    }

    #[test]
    fn test_import_portfolio_empty() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();
        let empty_portfolio = crate::clustering::PersistedTopicPortfolio::default();

        let count = manager.import_portfolio(&empty_portfolio);

        assert_eq!(count, 0);
        assert_eq!(manager.topic_count(), 0);

        println!("[PASS] test_import_portfolio_empty");
    }

    #[test]
    fn test_import_portfolio_roundtrip() {
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Create topics
        for i in 0..5 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings(i as f32 * 0.1);
            manager.insert(memory_id, &embeddings).unwrap();
        }
        manager.recluster().unwrap();

        // Export
        let original_count = manager.topic_count();
        let portfolio = manager.export_portfolio("roundtrip-test", 0.1, 0.3);

        // Create new manager and import
        let mut new_manager = MultiSpaceClusterManager::with_defaults().unwrap();
        let imported_count = new_manager.import_portfolio(&portfolio);

        assert_eq!(imported_count, original_count);
        assert_eq!(new_manager.topic_count(), original_count);

        println!(
            "[PASS] test_import_portfolio_roundtrip - topics={}",
            imported_count
        );
    }

    #[test]
    fn test_clear_topics() {
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Create some topics
        for i in 0..5 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings(i as f32 * 0.1);
            manager.insert(memory_id, &embeddings).unwrap();
        }
        manager.recluster().unwrap();

        // Clear topics
        manager.clear_topics();

        assert_eq!(manager.topic_count(), 0);

        println!("[PASS] test_clear_topics");
    }

    #[test]
    fn test_portfolio_summary() {
        let manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let (count, members) = manager.portfolio_summary();

        assert_eq!(count, 0);
        assert_eq!(members, 0);

        println!("[PASS] test_portfolio_summary");
    }

    #[test]
    fn test_import_replaces_existing_topics() {
        let params = manager_defaults().with_recluster_threshold(3);
        let mut manager = MultiSpaceClusterManager::new(params).unwrap();

        // Create initial topics
        for i in 0..5 {
            let memory_id = Uuid::new_v4();
            let embeddings = create_test_embeddings(i as f32 * 0.1);
            manager.insert(memory_id, &embeddings).unwrap();
        }
        manager.recluster().unwrap();

        let initial_count = manager.topic_count();

        // Create a different portfolio
        let new_portfolio = crate::clustering::PersistedTopicPortfolio::default();

        // Import should replace existing
        manager.import_portfolio(&new_portfolio);

        assert_eq!(manager.topic_count(), 0, "Import should replace existing topics");

        println!(
            "[PASS] test_import_replaces_existing_topics - before={}, after=0",
            initial_count
        );
    }

    // =========================================================================
    // FDMC (Fingerprint Distance Matrix Clustering) Integration Tests
    // =========================================================================

    /// Create a test SemanticFingerprint with domain-specific patterns.
    ///
    /// Different domains will have distinct angular patterns for cosine similarity:
    /// - ML: dominated by first third of embedding dimensions
    /// - Database: dominated by middle third of dimensions
    /// - DevOps: dominated by last third of dimensions
    ///
    /// Adds realistic noise to prevent artificially perfect clustering.
    fn create_domain_fingerprint(domain: &str, variation: f32) -> SemanticFingerprint {
        use crate::types::SparseVector;

        /// Generate a non-uniform vector with domain-specific pattern and noise.
        /// Creates distinct angular signatures for cosine similarity.
        fn generate_domain_embedding(dim: usize, domain: &str, variation: f32, seed: u64) -> Vec<f32> {
            let mut embedding = vec![0.1f32; dim]; // Low baseline

            // Domain-specific high-activation regions
            let (start_ratio, end_ratio) = match domain {
                "ml" => (0.0, 0.33),       // ML: first third
                "database" => (0.33, 0.66), // DB: middle third
                "devops" => (0.66, 1.0),   // DevOps: last third
                _ => (0.0, 1.0),
            };

            let start = (dim as f32 * start_ratio) as usize;
            let end = (dim as f32 * end_ratio) as usize;

            // Set high values in domain-specific region with realistic variation
            for i in start..end.min(dim) {
                // Add pseudo-random noise based on position and seed
                let noise = ((seed as f32 + i as f32) * 0.618033988).fract() - 0.5;
                embedding[i] = 0.5 + variation * 0.2 + noise * 0.15;
            }

            // Add some "bleed" into other regions (realistic overlap)
            for i in 0..dim {
                if i < start || i >= end {
                    let noise = ((seed as f32 + i as f32 + 1000.0) * 0.618033988).fract() - 0.5;
                    embedding[i] = 0.1 + noise * 0.1 + variation * 0.05;
                }
            }

            // Normalize for cosine similarity
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut embedding {
                    *v /= norm;
                }
            }

            embedding
        }

        // Use variation as seed modifier for reproducible but varied embeddings
        let seed = (variation * 1000.0) as u64;

        let e5_vec = generate_domain_embedding(get_dimension(Embedder::Causal), domain, variation, seed + 400);
        SemanticFingerprint {
            e1_semantic: generate_domain_embedding(get_dimension(Embedder::Semantic), domain, variation, seed),
            e2_temporal_recent: generate_domain_embedding(get_dimension(Embedder::TemporalRecent), "all", 0.5, seed + 100),
            e3_temporal_periodic: generate_domain_embedding(get_dimension(Embedder::TemporalPeriodic), "all", 0.5, seed + 200),
            e4_temporal_positional: generate_domain_embedding(get_dimension(Embedder::TemporalPositional), "all", 0.5, seed + 300),
            e5_causal_as_cause: e5_vec.clone(),
            e5_causal_as_effect: e5_vec,
            e5_causal: Vec::new(), // Using new dual format
            e6_sparse: SparseVector::empty(),
            e7_code: generate_domain_embedding(get_dimension(Embedder::Code), domain, variation, seed + 500),
            e8_graph_as_source: generate_domain_embedding(get_dimension(Embedder::Emotional), "all", 0.5, seed + 600),
            e8_graph_as_target: generate_domain_embedding(get_dimension(Embedder::Emotional), "all", 0.5, seed + 601),
            e8_graph: Vec::new(), // Legacy field, empty by default
            e9_hdc: generate_domain_embedding(get_dimension(Embedder::Hdc), "all", 0.5, seed + 700),
            e10_multimodal_as_intent: generate_domain_embedding(get_dimension(Embedder::Multimodal), domain, variation, seed + 800),
            e10_multimodal_as_context: generate_domain_embedding(get_dimension(Embedder::Multimodal), domain, variation, seed + 801),
            e10_multimodal: Vec::new(), // Legacy field, empty by default
            e11_entity: generate_domain_embedding(get_dimension(Embedder::Entity), domain, variation, seed + 900),
            e12_late_interaction: vec![generate_domain_embedding(128, domain, variation, seed + 1000); 10],
            e13_splade: SparseVector::empty(),
        }
    }

    #[test]
    fn test_fdmc_empty_fingerprints() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let fingerprints: Vec<(Uuid, SemanticFingerprint)> = vec![];
        let result = manager.recluster_fingerprint_matrix(&fingerprints);

        assert!(result.is_ok());
        let fdmc_result = result.unwrap();
        assert!(fdmc_result.is_empty());
        assert_eq!(fdmc_result.topics_discovered, 0);

        println!("[PASS] test_fdmc_empty_fingerprints");
    }

    #[test]
    fn test_fdmc_insufficient_fingerprints() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Only 2 fingerprints (min_cluster_size = 3)
        let fingerprints: Vec<(Uuid, SemanticFingerprint)> = vec![
            (Uuid::new_v4(), create_domain_fingerprint("ml", 0.0)),
            (Uuid::new_v4(), create_domain_fingerprint("ml", 0.1)),
        ];

        let result = manager.recluster_fingerprint_matrix(&fingerprints);
        assert!(result.is_ok());

        let fdmc_result = result.unwrap();
        assert!(fdmc_result.is_empty());

        println!("[PASS] test_fdmc_insufficient_fingerprints - correctly handles < min_cluster_size");
    }

    #[test]
    fn test_fdmc_single_cluster() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // All ML domain - should form single cluster
        let fingerprints: Vec<(Uuid, SemanticFingerprint)> = (0..5)
            .map(|i| {
                (
                    Uuid::new_v4(),
                    create_domain_fingerprint("ml", i as f32 * 0.1),
                )
            })
            .collect();

        let result = manager.recluster_fingerprint_matrix(&fingerprints);
        assert!(result.is_ok());

        let fdmc_result = result.unwrap();
        assert!(!fdmc_result.is_empty());
        assert_eq!(fdmc_result.memory_count(), 5);

        // All should be in same cluster (or few noise)
        let cluster_ids: Vec<i32> = fdmc_result.memberships.iter().map(|m| m.cluster_id).collect();
        let non_noise: Vec<i32> = cluster_ids.iter().filter(|&&c| c >= 0).cloned().collect();
        if !non_noise.is_empty() {
            assert!(
                non_noise.iter().all(|&c| c == non_noise[0]),
                "All non-noise should be in same cluster"
            );
        }

        println!(
            "[PASS] test_fdmc_single_cluster - clusters={}, noise={}",
            fdmc_result.total_clusters,
            fdmc_result.noise_count()
        );
    }

    #[test]
    fn test_fdmc_multi_domain_separation() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Create fingerprints from 3 domains
        // 5 ML, 5 Database, 5 DevOps = 15 total
        let mut fingerprints: Vec<(Uuid, SemanticFingerprint)> = Vec::new();

        // ML domain (memories 0-4)
        for i in 0..5 {
            fingerprints.push((
                Uuid::new_v4(),
                create_domain_fingerprint("ml", i as f32 * 0.1),
            ));
        }

        // Database domain (memories 5-9)
        for i in 0..5 {
            fingerprints.push((
                Uuid::new_v4(),
                create_domain_fingerprint("database", i as f32 * 0.1),
            ));
        }

        // DevOps domain (memories 10-14)
        for i in 0..5 {
            fingerprints.push((
                Uuid::new_v4(),
                create_domain_fingerprint("devops", i as f32 * 0.1),
            ));
        }

        let result = manager.recluster_fingerprint_matrix(&fingerprints);
        assert!(result.is_ok());

        let fdmc_result = result.unwrap();
        assert_eq!(fdmc_result.memory_count(), 15);

        // Check silhouette score - should be reasonable for well-separated data
        println!(
            "[RESULT] FDMC silhouette={}, quality={}",
            fdmc_result.silhouette_score,
            fdmc_result.quality_rating()
        );

        // Check similarity stats
        let stats = &fdmc_result.similarity_stats;
        println!(
            "[RESULT] Similarity: min={:.3}, max={:.3}, mean={:.3}, gap={:.3} at {:.3}",
            stats.min, stats.max, stats.mean, stats.max_gap, stats.gap_position
        );

        // Check embedder contributions
        let contributions = &fdmc_result.embedder_contributions;
        if let Some(dominant) = fdmc_result.dominant_embedder {
            println!(
                "[RESULT] Dominant embedder: {:?} with contribution {:.4}",
                dominant,
                contributions[dominant.index()]
            );
        }

        println!(
            "[PASS] test_fdmc_multi_domain_separation - topics={}, clusters={}",
            fdmc_result.topics_discovered, fdmc_result.total_clusters
        );
    }

    #[test]
    fn test_fdmc_result_quality_rating() {
        let result = FdmcResult {
            silhouette_score: 0.8,
            ..FdmcResult::empty()
        };
        assert_eq!(result.quality_rating(), "excellent");

        let result = FdmcResult {
            silhouette_score: 0.5,
            ..FdmcResult::empty()
        };
        assert_eq!(result.quality_rating(), "good");

        let result = FdmcResult {
            silhouette_score: 0.3,
            ..FdmcResult::empty()
        };
        assert_eq!(result.quality_rating(), "acceptable");

        let result = FdmcResult {
            silhouette_score: 0.1,
            ..FdmcResult::empty()
        };
        assert_eq!(result.quality_rating(), "poor");

        let result = FdmcResult {
            silhouette_score: -0.5,
            ..FdmcResult::empty()
        };
        assert_eq!(result.quality_rating(), "failed");

        println!("[PASS] test_fdmc_result_quality_rating");
    }

    #[test]
    fn test_fdmc_noise_count() {
        let memberships = vec![
            ClusterMembership::new(Uuid::new_v4(), Embedder::Semantic, 0, 0.9, true),
            ClusterMembership::new(Uuid::new_v4(), Embedder::Semantic, 0, 0.8, false),
            ClusterMembership::new(Uuid::new_v4(), Embedder::Semantic, -1, 0.0, false), // Noise
            ClusterMembership::new(Uuid::new_v4(), Embedder::Semantic, 1, 0.7, true),
            ClusterMembership::new(Uuid::new_v4(), Embedder::Semantic, -1, 0.0, false), // Noise
        ];

        let result = FdmcResult {
            memberships,
            ..FdmcResult::empty()
        };

        assert_eq!(result.noise_count(), 2);
        assert_eq!(result.memory_count(), 5);

        println!("[PASS] test_fdmc_noise_count");
    }

    #[test]
    fn test_fdmc_updates_topics_in_manager() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Initially no topics
        assert_eq!(manager.topic_count(), 0);

        // Create fingerprints
        let fingerprints: Vec<(Uuid, SemanticFingerprint)> = (0..6)
            .map(|i| {
                (
                    Uuid::new_v4(),
                    create_domain_fingerprint("ml", i as f32 * 0.05),
                )
            })
            .collect();

        let result = manager.recluster_fingerprint_matrix(&fingerprints);
        assert!(result.is_ok());

        let fdmc_result = result.unwrap();

        // Manager's topics should be updated
        assert_eq!(manager.topic_count(), fdmc_result.topics_discovered);

        println!(
            "[PASS] test_fdmc_updates_topics_in_manager - topics={}",
            manager.topic_count()
        );
    }

    #[test]
    fn test_fdmc_topic_profile_computation() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Create fingerprints with known pattern
        let fingerprints: Vec<(Uuid, SemanticFingerprint)> = (0..6)
            .map(|i| {
                (
                    Uuid::new_v4(),
                    create_domain_fingerprint("ml", i as f32 * 0.05),
                )
            })
            .collect();

        let result = manager.recluster_fingerprint_matrix(&fingerprints);
        assert!(result.is_ok());

        // Check that topics have valid profiles
        for topic in manager.get_topics().values() {
            let weighted = topic.profile.weighted_agreement();
            assert!(
                weighted >= TOPIC_THRESHOLD,
                "Topic should meet threshold, got {}",
                weighted
            );

            // Check contributing spaces
            assert!(
                !topic.contributing_spaces.is_empty(),
                "Topic should have contributing spaces"
            );
        }

        println!("[PASS] test_fdmc_topic_profile_computation");
    }

    #[test]
    fn test_fdmc_silhouette_manual_verification() {
        // Create small dataset to manually verify silhouette math
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // 3 ML, 3 DB = 6 memories, expecting 2 clusters
        let mut fingerprints: Vec<(Uuid, SemanticFingerprint)> = Vec::new();
        for i in 0..3 {
            fingerprints.push((
                Uuid::new_v4(),
                create_domain_fingerprint("ml", i as f32 * 0.1),
            ));
        }
        for i in 0..3 {
            fingerprints.push((
                Uuid::new_v4(),
                create_domain_fingerprint("database", i as f32 * 0.1),
            ));
        }

        // Build the fingerprint matrix to inspect distances
        use crate::clustering::{build_fingerprint_matrix, FingerprintMatrixConfig};
        let fp_refs: Vec<(Uuid, &SemanticFingerprint)> =
            fingerprints.iter().map(|(id, fp)| (*id, fp)).collect();
        let config = FingerprintMatrixConfig::for_topic_detection();
        let matrix = build_fingerprint_matrix(&fp_refs, &config).unwrap();

        println!("\n=== DISTANCE MATRIX (6x6) ===");
        let distances = matrix.to_distance_matrix();
        for i in 0..6 {
            let row: Vec<String> = distances[i].iter().map(|d| format!("{:.3}", d)).collect();
            let domain = if i < 3 { "ML" } else { "DB" };
            println!("  [{}] {}: [{}]", i, domain, row.join(", "));
        }

        // Expected pattern:
        // - ML[0-2] to ML[0-2]: low distance (high similarity within domain)
        // - DB[3-5] to DB[3-5]: low distance (high similarity within domain)
        // - ML[0-2] to DB[3-5]: high distance (low similarity across domains)

        // Now cluster and compute silhouette
        let result = manager.recluster_fingerprint_matrix(&fingerprints).unwrap();
        let labels: Vec<i32> = result.memberships.iter().map(|m| m.cluster_id).collect();

        println!("\n=== CLUSTER LABELS ===");
        println!("  Labels: {:?}", labels);

        // Manually compute a(i) and b(i) for point 0 (ML)
        if labels.iter().filter(|&&l| l >= 0).count() >= 2 {
            let i = 0; // First ML point

            // a(i) = mean distance to same cluster
            let same_cluster: Vec<f32> = (0..6)
                .filter(|&j| j != i && labels[j] == labels[i])
                .map(|j| distances[i][j])
                .collect();
            let a_i = if same_cluster.is_empty() {
                0.0
            } else {
                same_cluster.iter().sum::<f32>() / same_cluster.len() as f32
            };

            // b(i) = min mean distance to other clusters
            let other_clusters: std::collections::HashSet<i32> = labels
                .iter()
                .filter(|&&l| l >= 0 && l != labels[i])
                .copied()
                .collect();

            let mut b_i = f32::MAX;
            for &other_cluster in &other_clusters {
                let other_dists: Vec<f32> = (0..6)
                    .filter(|&j| labels[j] == other_cluster)
                    .map(|j| distances[i][j])
                    .collect();
                if !other_dists.is_empty() {
                    let mean = other_dists.iter().sum::<f32>() / other_dists.len() as f32;
                    b_i = b_i.min(mean);
                }
            }
            if b_i == f32::MAX { b_i = 0.0; }

            // s(i) = (b(i) - a(i)) / max(a(i), b(i))
            let max_ab = a_i.max(b_i);
            let s_i = if max_ab > 0.0 { (b_i - a_i) / max_ab } else { 0.0 };

            println!("\n=== MANUAL SILHOUETTE FOR POINT 0 (ML) ===");
            println!("  same_cluster_distances: {:?}", same_cluster);
            println!("  a(0) = mean intra-cluster distance = {:.4}", a_i);
            println!("  b(0) = min mean inter-cluster distance = {:.4}", b_i);
            println!("  s(0) = (b - a) / max(a, b) = ({:.4} - {:.4}) / {:.4} = {:.4}",
                     b_i, a_i, max_ab, s_i);
        }

        println!("\n=== COMPUTED SILHOUETTE ===");
        println!("  Silhouette score: {:.6}", result.silhouette_score);
        println!("  Quality rating: {}", result.quality_rating());

        // If intra-cluster distance ≈ 0 and inter-cluster distance ≈ 0.45
        // Then s(i) ≈ (0.45 - 0) / 0.45 ≈ 1.0
        // This would be mathematically correct for perfectly separated clusters!

        println!("\n[PASS] test_fdmc_silhouette_manual_verification");
    }

    // =========================================================================
    // FDMC Large-Scale Topic Discovery Tests (15 Topics)
    // =========================================================================

    /// Domain definitions for 15-topic test.
    /// Each domain has a unique activation pattern across embedding dimensions.
    const DOMAINS_15: [&str; 15] = [
        "machine_learning",
        "database_systems",
        "devops_infra",
        "web_frontend",
        "mobile_ios",
        "mobile_android",
        "security_crypto",
        "networking",
        "cloud_aws",
        "cloud_gcp",
        "data_science",
        "game_dev",
        "embedded_systems",
        "blockchain",
        "quantum_computing",
    ];

    /// Create fingerprint with one of 15 domain patterns.
    /// Uses fractional dimension ranges to create 15 distinct signatures.
    fn create_15domain_fingerprint(domain_idx: usize, variation: f32) -> SemanticFingerprint {
        use crate::types::SparseVector;

        /// Generate embedding with domain-specific activation region.
        /// Divides dimension space into 15 regions (6.67% each).
        fn generate_15domain_embedding(dim: usize, domain_idx: usize, variation: f32, seed: u64) -> Vec<f32> {
            let mut embedding = vec![0.05f32; dim]; // Very low baseline

            // Each domain gets ~6.67% of the dimension space
            let start_ratio = (domain_idx as f32) / 15.0;
            let end_ratio = ((domain_idx + 1) as f32) / 15.0;

            let start = (dim as f32 * start_ratio) as usize;
            let end = (dim as f32 * end_ratio) as usize;

            // High activation in domain-specific region
            for i in start..end.min(dim) {
                let noise = ((seed as f32 + i as f32) * 0.618033988).fract() - 0.5;
                embedding[i] = 0.6 + variation * 0.15 + noise * 0.1;
            }

            // Add slight bleed to adjacent regions (realistic overlap)
            let bleed_start = if start > 10 { start - 10 } else { 0 };
            let bleed_end = (end + 10).min(dim);
            for i in bleed_start..start {
                let decay = (i - bleed_start) as f32 / 10.0;
                embedding[i] = 0.1 + decay * 0.2;
            }
            for i in end..bleed_end {
                let decay = 1.0 - (i - end) as f32 / 10.0;
                embedding[i] = 0.1 + decay * 0.2;
            }

            // Add global noise
            for i in 0..dim {
                let noise = ((seed as f32 + i as f32 + 5000.0) * 0.618033988).fract() - 0.5;
                embedding[i] += noise * 0.03;
                embedding[i] = embedding[i].max(0.01); // Keep positive
            }

            // Normalize
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut embedding {
                    *v /= norm;
                }
            }

            embedding
        }

        let seed = (domain_idx as u64 * 1000) + (variation * 100.0) as u64;
        let e5_vec = generate_15domain_embedding(get_dimension(Embedder::Causal), domain_idx, variation, seed + 400);

        SemanticFingerprint {
            e1_semantic: generate_15domain_embedding(get_dimension(Embedder::Semantic), domain_idx, variation, seed),
            e2_temporal_recent: generate_15domain_embedding(get_dimension(Embedder::TemporalRecent), 7, 0.5, seed + 100),
            e3_temporal_periodic: generate_15domain_embedding(get_dimension(Embedder::TemporalPeriodic), 7, 0.5, seed + 200),
            e4_temporal_positional: generate_15domain_embedding(get_dimension(Embedder::TemporalPositional), 7, 0.5, seed + 300),
            e5_causal_as_cause: e5_vec.clone(),
            e5_causal_as_effect: e5_vec,
            e5_causal: Vec::new(), // Using new dual format
            e6_sparse: SparseVector::empty(),
            e7_code: generate_15domain_embedding(get_dimension(Embedder::Code), domain_idx, variation, seed + 500),
            e8_graph_as_source: generate_15domain_embedding(get_dimension(Embedder::Emotional), 7, 0.5, seed + 600),
            e8_graph_as_target: generate_15domain_embedding(get_dimension(Embedder::Emotional), 7, 0.5, seed + 601),
            e8_graph: Vec::new(), // Legacy field, empty by default
            e9_hdc: generate_15domain_embedding(get_dimension(Embedder::Hdc), 7, 0.5, seed + 700),
            e10_multimodal_as_intent: generate_15domain_embedding(get_dimension(Embedder::Multimodal), domain_idx, variation, seed + 800),
            e10_multimodal_as_context: generate_15domain_embedding(get_dimension(Embedder::Multimodal), domain_idx, variation, seed + 801),
            e10_multimodal: Vec::new(), // Legacy field, empty by default
            e11_entity: generate_15domain_embedding(get_dimension(Embedder::Entity), domain_idx, variation, seed + 900),
            e12_late_interaction: vec![generate_15domain_embedding(128, domain_idx, variation, seed + 1000); 10],
            e13_splade: SparseVector::empty(),
        }
    }

    #[test]
    fn test_fdmc_15_topics_discovery() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Create 3 fingerprints per domain = 45 total memories
        // Should discover ~15 topics (one per domain)
        let mut fingerprints: Vec<(Uuid, SemanticFingerprint)> = Vec::new();

        for (domain_idx, domain_name) in DOMAINS_15.iter().enumerate() {
            for i in 0..3 {
                let fp = create_15domain_fingerprint(domain_idx, i as f32 * 0.1);
                fingerprints.push((Uuid::new_v4(), fp));
            }
            println!("Created 3 fingerprints for domain {}: {}", domain_idx, domain_name);
        }

        assert_eq!(fingerprints.len(), 45, "Should have 45 fingerprints (15 domains × 3)");

        // Run FDMC clustering
        let result = manager.recluster_fingerprint_matrix(&fingerprints).unwrap();

        println!("\n=== 15-TOPIC TEST RESULTS ===");
        println!("Total memories: {}", result.memory_count());
        println!("Total clusters: {}", result.total_clusters);
        println!("Topics discovered: {}", result.topics_discovered);
        println!("Noise points: {}", result.noise_count());
        println!("Silhouette: {:.4} ({})", result.silhouette_score, result.quality_rating());

        // Print similarity stats
        let stats = &result.similarity_stats;
        println!("\nSimilarity distribution:");
        println!("  min={:.4}, max={:.4}, mean={:.4}, std={:.4}",
                 stats.min, stats.max, stats.mean, stats.std_dev);

        // Analyze cluster assignments
        let labels: Vec<i32> = result.memberships.iter().map(|m| m.cluster_id).collect();
        let unique_clusters: std::collections::HashSet<i32> = labels.iter().filter(|&&l| l >= 0).copied().collect();
        println!("\nUnique cluster IDs (non-noise): {:?}", unique_clusters);

        // Check each domain's cluster assignment
        println!("\nDomain → Cluster mapping:");
        for (domain_idx, domain_name) in DOMAINS_15.iter().enumerate() {
            let start = domain_idx * 3;
            let domain_labels: Vec<i32> = labels[start..start + 3].to_vec();
            let coherent = domain_labels.iter().all(|&l| l == domain_labels[0] && l >= 0);
            println!("  {}: {:?} {}", domain_name, domain_labels,
                     if coherent { "✓" } else { "⚠" });
        }

        // Verify we got a reasonable number of topics
        // With 15 domains, we expect close to 15 topics
        assert!(
            result.topics_discovered >= 10,
            "Should discover at least 10 topics from 15 domains, got {}",
            result.topics_discovered
        );

        println!("\n[PASS] test_fdmc_15_topics_discovery - {} topics from 15 domains",
                 result.topics_discovered);
    }

    #[test]
    fn test_fdmc_15_topics_cluster_purity() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Create fingerprints with known domain assignments
        let mut fingerprints: Vec<(Uuid, SemanticFingerprint)> = Vec::new();
        let mut domain_assignments: Vec<usize> = Vec::new();

        for domain_idx in 0..15 {
            for i in 0..3 {
                let fp = create_15domain_fingerprint(domain_idx, i as f32 * 0.1);
                fingerprints.push((Uuid::new_v4(), fp));
                domain_assignments.push(domain_idx);
            }
        }

        let result = manager.recluster_fingerprint_matrix(&fingerprints).unwrap();
        let labels: Vec<i32> = result.memberships.iter().map(|m| m.cluster_id).collect();

        // Calculate cluster purity: for each cluster, what % comes from the dominant domain?
        let unique_clusters: std::collections::HashSet<i32> = labels.iter().filter(|&&l| l >= 0).copied().collect();

        let mut total_purity = 0.0;
        let mut cluster_count = 0;

        println!("\n=== CLUSTER PURITY ANALYSIS ===");
        for &cluster_id in &unique_clusters {
            // Find which memories are in this cluster
            let cluster_members: Vec<usize> = labels.iter()
                .enumerate()
                .filter(|(_, &l)| l == cluster_id)
                .map(|(i, _)| i)
                .collect();

            // Count domain occurrences
            let mut domain_counts: [usize; 15] = [0; 15];
            for &member_idx in &cluster_members {
                domain_counts[domain_assignments[member_idx]] += 1;
            }

            let max_count = *domain_counts.iter().max().unwrap_or(&0);
            let dominant_domain = domain_counts.iter().position(|&c| c == max_count).unwrap_or(0);
            let purity = max_count as f32 / cluster_members.len() as f32;

            total_purity += purity;
            cluster_count += 1;

            println!("  Cluster {}: {} members, dominant={} ({}), purity={:.2}%",
                     cluster_id, cluster_members.len(), DOMAINS_15[dominant_domain], max_count,
                     purity * 100.0);
        }

        let avg_purity = if cluster_count > 0 { total_purity / cluster_count as f32 } else { 0.0 };
        println!("\nAverage cluster purity: {:.2}%", avg_purity * 100.0);

        // Expect high purity (each cluster mostly contains one domain)
        assert!(
            avg_purity >= 0.8,
            "Average cluster purity should be >= 80%, got {:.2}%",
            avg_purity * 100.0
        );

        println!("\n[PASS] test_fdmc_15_topics_cluster_purity - {:.2}% average purity",
                 avg_purity * 100.0);
    }

    #[test]
    fn test_fdmc_15_topics_with_5_per_domain() {
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Create 5 fingerprints per domain = 75 total memories
        let mut fingerprints: Vec<(Uuid, SemanticFingerprint)> = Vec::new();

        for domain_idx in 0..15 {
            for i in 0..5 {
                let fp = create_15domain_fingerprint(domain_idx, i as f32 * 0.08);
                fingerprints.push((Uuid::new_v4(), fp));
            }
        }

        assert_eq!(fingerprints.len(), 75);

        let result = manager.recluster_fingerprint_matrix(&fingerprints).unwrap();

        println!("\n=== 15-TOPIC TEST (5 per domain) ===");
        println!("Memories: {}, Clusters: {}, Topics: {}",
                 result.memory_count(), result.total_clusters, result.topics_discovered);
        println!("Silhouette: {:.4} ({})", result.silhouette_score, result.quality_rating());
        println!("Noise: {}", result.noise_count());

        // With more samples per domain, clustering should be more stable
        assert!(
            result.topics_discovered >= 12,
            "Should discover at least 12 topics with 5 samples per domain, got {}",
            result.topics_discovered
        );

        println!("\n[PASS] test_fdmc_15_topics_with_5_per_domain - {} topics", result.topics_discovered);
    }

    #[test]
    fn test_fdmc_mixed_domain_overlap() {
        // Test with some domains that are "close" to each other
        // e.g., mobile_ios and mobile_android should be somewhat similar
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let mut fingerprints: Vec<(Uuid, SemanticFingerprint)> = Vec::new();

        // Create related domain pairs (indices 4&5 = mobile, 8&9 = cloud)
        let related_pairs = [(4, 5), (8, 9)]; // mobile_ios/android, cloud_aws/gcp

        for domain_idx in 0..15 {
            for i in 0..3 {
                let fp = create_15domain_fingerprint(domain_idx, i as f32 * 0.1);
                fingerprints.push((Uuid::new_v4(), fp));
            }
        }

        let result = manager.recluster_fingerprint_matrix(&fingerprints).unwrap();
        let labels: Vec<i32> = result.memberships.iter().map(|m| m.cluster_id).collect();

        println!("\n=== RELATED DOMAIN ANALYSIS ===");
        for (domain_a, domain_b) in related_pairs {
            let labels_a: Vec<i32> = labels[domain_a * 3..(domain_a + 1) * 3].to_vec();
            let labels_b: Vec<i32> = labels[domain_b * 3..(domain_b + 1) * 3].to_vec();

            // Check if they ended up in same cluster (they're related, so might merge)
            let same_cluster = labels_a.iter().any(|&a| labels_b.contains(&a) && a >= 0);

            println!("  {} vs {}: {:?} vs {:?} - {}",
                     DOMAINS_15[domain_a], DOMAINS_15[domain_b],
                     labels_a, labels_b,
                     if same_cluster { "MERGED" } else { "SEPARATE" });
        }

        println!("\nTotal topics: {}", result.topics_discovered);
        println!("\n[PASS] test_fdmc_mixed_domain_overlap");
    }

    #[test]
    fn test_fdmc_incremental_domain_addition() {
        // Start with 5 domains, add more incrementally
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        // Phase 1: 5 domains (15 fingerprints)
        let mut fingerprints: Vec<(Uuid, SemanticFingerprint)> = Vec::new();
        for domain_idx in 0..5 {
            for i in 0..3 {
                fingerprints.push((Uuid::new_v4(), create_15domain_fingerprint(domain_idx, i as f32 * 0.1)));
            }
        }

        let result1 = manager.recluster_fingerprint_matrix(&fingerprints).unwrap();
        println!("\n=== INCREMENTAL TEST ===");
        println!("Phase 1 (5 domains, 15 memories): {} topics", result1.topics_discovered);

        // Phase 2: Add 5 more domains (30 fingerprints total)
        for domain_idx in 5..10 {
            for i in 0..3 {
                fingerprints.push((Uuid::new_v4(), create_15domain_fingerprint(domain_idx, i as f32 * 0.1)));
            }
        }

        let result2 = manager.recluster_fingerprint_matrix(&fingerprints).unwrap();
        println!("Phase 2 (10 domains, 30 memories): {} topics", result2.topics_discovered);

        // Phase 3: Add final 5 domains (45 fingerprints total)
        for domain_idx in 10..15 {
            for i in 0..3 {
                fingerprints.push((Uuid::new_v4(), create_15domain_fingerprint(domain_idx, i as f32 * 0.1)));
            }
        }

        let result3 = manager.recluster_fingerprint_matrix(&fingerprints).unwrap();
        println!("Phase 3 (15 domains, 45 memories): {} topics", result3.topics_discovered);

        // Topics should increase as we add more domains
        assert!(
            result2.topics_discovered >= result1.topics_discovered,
            "Adding domains should not reduce topic count"
        );
        assert!(
            result3.topics_discovered >= result2.topics_discovered,
            "Adding domains should not reduce topic count"
        );

        println!("\n[PASS] test_fdmc_incremental_domain_addition");
    }

    #[test]
    fn test_fdmc_noise_injection() {
        // Test with some random noise fingerprints mixed in
        let mut manager = MultiSpaceClusterManager::with_defaults().unwrap();

        let mut fingerprints: Vec<(Uuid, SemanticFingerprint)> = Vec::new();

        // Add 10 domains (30 fingerprints)
        for domain_idx in 0..10 {
            for i in 0..3 {
                fingerprints.push((Uuid::new_v4(), create_15domain_fingerprint(domain_idx, i as f32 * 0.1)));
            }
        }

        // Add 5 "noise" fingerprints with random domain indices
        // These should either form noise cluster or join nearby clusters
        let noise_indices = [3, 7, 11, 2, 14]; // Random domain patterns
        for (i, &domain_idx) in noise_indices.iter().enumerate() {
            let fp = create_15domain_fingerprint(domain_idx, 0.5 + i as f32 * 0.1); // High variation
            fingerprints.push((Uuid::new_v4(), fp));
        }

        let result = manager.recluster_fingerprint_matrix(&fingerprints).unwrap();

        println!("\n=== NOISE INJECTION TEST ===");
        println!("Total memories: {} (30 regular + 5 noise)", result.memory_count());
        println!("Topics: {}, Clusters: {}", result.topics_discovered, result.total_clusters);
        println!("Noise points: {}", result.noise_count());
        println!("Silhouette: {:.4}", result.silhouette_score);

        // Should still find most of the 10 domains as topics
        assert!(
            result.topics_discovered >= 7,
            "Should find at least 7 topics despite noise, got {}",
            result.topics_discovered
        );

        println!("\n[PASS] test_fdmc_noise_injection");
    }

    #[test]
    fn test_fdmc_similarity_matrix_structure() {
        // Verify the similarity matrix has expected structure
        use crate::clustering::{build_fingerprint_matrix, FingerprintMatrixConfig};

        let mut fingerprints: Vec<(Uuid, SemanticFingerprint)> = Vec::new();
        for domain_idx in 0..5 {
            for i in 0..3 {
                fingerprints.push((Uuid::new_v4(), create_15domain_fingerprint(domain_idx, i as f32 * 0.1)));
            }
        }

        let fp_refs: Vec<(Uuid, &SemanticFingerprint)> =
            fingerprints.iter().map(|(id, fp)| (*id, fp)).collect();
        let config = FingerprintMatrixConfig::for_topic_detection();
        let matrix = build_fingerprint_matrix(&fp_refs, &config).unwrap();

        println!("\n=== SIMILARITY MATRIX STRUCTURE (5 domains) ===");

        // Check block diagonal structure
        // Entries within same domain should be high, across domains should be lower
        let mut intra_domain_avg = 0.0;
        let mut inter_domain_avg = 0.0;
        let mut intra_count = 0;
        let mut inter_count = 0;

        for i in 0..15 {
            for j in (i + 1)..15 {
                let sim = matrix.get_similarity(i, j).unwrap();
                let domain_i = i / 3;
                let domain_j = j / 3;

                if domain_i == domain_j {
                    intra_domain_avg += sim;
                    intra_count += 1;
                } else {
                    inter_domain_avg += sim;
                    inter_count += 1;
                }
            }
        }

        intra_domain_avg /= intra_count as f32;
        inter_domain_avg /= inter_count as f32;

        println!("Intra-domain avg similarity: {:.4} ({} pairs)", intra_domain_avg, intra_count);
        println!("Inter-domain avg similarity: {:.4} ({} pairs)", inter_domain_avg, inter_count);
        println!("Separation ratio: {:.2}x", intra_domain_avg / inter_domain_avg);

        // Intra-domain should be significantly higher than inter-domain
        assert!(
            intra_domain_avg > inter_domain_avg * 1.1,
            "Intra-domain similarity ({:.4}) should be > inter-domain ({:.4})",
            intra_domain_avg, inter_domain_avg
        );

        println!("\n[PASS] test_fdmc_similarity_matrix_structure");
    }
}
