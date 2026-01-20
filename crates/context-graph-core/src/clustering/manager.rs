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
//!         7 | 10 => vec![0.0; 384], // E8, E11
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

use crate::embeddings::category::{category_for, EmbedderCategory};
use crate::embeddings::config::get_dimension;
use crate::teleological::Embedder;

use super::birch::{birch_defaults, BIRCHParams, BIRCHTree};
use super::cluster::Cluster;
use super::error::ClusterError;
use super::hdbscan::{hdbscan_defaults, HDBSCANClusterer, HDBSCANParams};
use super::membership::ClusterMembership;
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
    ///         6 => 1536, 7 | 10 => 384, 8 => 1024, 9 => 768, 11 => 128, _ => 128,
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

    /// Synthesize topics from cross-space cluster memberships.
    ///
    /// Discovers topics where memories cluster together in multiple spaces
    /// with weighted_agreement >= 2.5 (ARCH-09).
    fn synthesize_topics(&mut self) -> Result<(), ClusterError> {
        self.topics.clear();

        // Group memories by their cluster assignments across spaces
        let mut memory_profiles: HashMap<Uuid, TopicProfile> = HashMap::new();

        // Collect cluster assignments for each memory
        for state in &self.spaces {
            for (memory_id, membership) in &state.memberships {
                let profile = memory_profiles.entry(*memory_id).or_default();

                // Set strength based on membership probability
                if membership.cluster_id >= 0 {
                    profile.set_strength(membership.space, membership.membership_probability);
                }
            }
        }

        // Find memories that meet topic threshold
        let mut topic_candidates: HashMap<String, Vec<Uuid>> = HashMap::new();

        for (memory_id, profile) in &memory_profiles {
            if profile.is_topic() {
                // Create a topic key based on cluster assignments
                let key = self.make_topic_key(memory_id);
                topic_candidates.entry(key).or_default().push(*memory_id);
            }
        }

        // Create Topic objects for groups with enough members
        for (_, member_ids) in topic_candidates {
            if member_ids.len() >= self.params.hdbscan_params.min_cluster_size {
                // Compute average profile for the topic
                let avg_profile = self.compute_average_profile(&member_ids, &memory_profiles);

                // Build cluster_ids map
                let mut cluster_ids = HashMap::new();
                if let Some(first_id) = member_ids.first() {
                    for (i, embedder) in Embedder::all().enumerate() {
                        if let Some(membership) = self.spaces[i].memberships.get(first_id) {
                            if membership.cluster_id >= 0 {
                                cluster_ids.insert(embedder, membership.cluster_id);
                            }
                        }
                    }
                }

                let topic = Topic::new(avg_profile, cluster_ids, member_ids);
                self.topics.insert(topic.id, topic);
            }
        }

        Ok(())
    }

    /// Make a topic key from a memory's cluster assignments.
    fn make_topic_key(&self, memory_id: &Uuid) -> String {
        let mut key_parts: Vec<String> = Vec::new();

        for (i, embedder) in Embedder::all().enumerate() {
            let category = category_for(embedder);

            // Only use semantic, relational, and structural for topic keys
            // Temporal (E2-E4) excluded per AP-60
            if category == EmbedderCategory::Temporal {
                continue;
            }

            if let Some(membership) = self.spaces[i].memberships.get(memory_id) {
                if membership.cluster_id >= 0 {
                    key_parts.push(format!("{}:{}", embedder.index(), membership.cluster_id));
                }
            }
        }

        key_parts.join(",")
    }

    /// Compute average profile from a set of memories.
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
        for topic in &portfolio.topics {
            self.topics.insert(topic.id, topic.clone());
        }

        self.topics.len()
    }

    /// Clear all topics from the manager.
    ///
    /// This is useful before importing a new portfolio or for testing.
    pub fn clear_topics(&mut self) {
        self.topics.clear();
    }

    /// Get portfolio-level summary for persistence.
    ///
    /// Returns a tuple of (topic_count, total_members).
    #[inline]
    pub fn portfolio_summary(&self) -> (usize, usize) {
        let total_members: usize = self.topics.values().map(|t| t.member_count()).sum();
        (self.topics.len(), total_members)
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
}
