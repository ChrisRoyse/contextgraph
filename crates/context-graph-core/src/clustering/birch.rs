//! BIRCH clustering parameters and ClusteringFeature.
//!
//! Provides configuration types for BIRCH (Balanced Iterative Reducing and
//! Clustering using Hierarchies) algorithm. BIRCH enables O(log n) incremental
//! clustering for real-time memory insertion.
//!
//! # Constitution Defaults
//!
//! Per constitution BIRCH_DEFAULTS:
//! - branching_factor: 50
//! - threshold: 0.3 (adaptive)
//! - max_node_entries: 50
//!
//! # Clustering Feature (CF)
//!
//! The CF is a triple (n, LS, SS) that summarizes a set of points:
//! - n: number of data points
//! - LS: linear sum (vector sum of all points)
//! - SS: squared sum (scalar sum of squared norms)
//!
//! Key property: CFs are additive. CF(A ∪ B) = CF(A) + CF(B)
//!
//! # BIRCHTree
//!
//! The BIRCH CF-tree for incremental clustering:
//! - O(log n) insertion via tree traversal
//! - Automatic node splitting when exceeding max_node_entries
//! - Memory ID tracking for cluster membership queries
//! - Threshold adaptation for target cluster count

use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use crate::teleological::Embedder;

use super::error::ClusterError;

// =============================================================================
// BIRCHParams
// =============================================================================

/// Parameters for BIRCH clustering algorithm.
///
/// Per constitution: branching_factor=50, threshold=0.3, max_node_entries=50
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::birch::{BIRCHParams, birch_defaults};
/// use context_graph_core::teleological::Embedder;
///
/// // Use defaults
/// let params = birch_defaults();
/// assert_eq!(params.branching_factor, 50);
///
/// // Or space-specific
/// let code_params = BIRCHParams::default_for_space(Embedder::Code);
/// assert!(code_params.threshold < 0.3); // Code embeddings more specific
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BIRCHParams {
    /// Maximum number of children per non-leaf node.
    /// Controls tree width. Higher = flatter tree but more work per node.
    pub branching_factor: usize,

    /// Threshold for cluster radius.
    /// Points within this radius merge into same CF. Adaptive in practice.
    pub threshold: f32,

    /// Maximum entries per leaf node.
    /// When exceeded, node splits.
    pub max_node_entries: usize,
}

impl Default for BIRCHParams {
    fn default() -> Self {
        Self {
            branching_factor: 50, // Per constitution
            threshold: 0.3,       // Per constitution
            max_node_entries: 50, // Per constitution
        }
    }
}

impl BIRCHParams {
    /// Create new BIRCH params.
    ///
    /// Values are NOT automatically validated - call validate() to check.
    pub fn new(branching_factor: usize, threshold: f32, max_entries: usize) -> Self {
        Self {
            branching_factor,
            threshold,
            max_node_entries: max_entries,
        }
    }

    /// Create params for a specific embedding space.
    ///
    /// Adjusts threshold based on space characteristics:
    /// - Sparse spaces (Sparse, KeywordSplade): 0.4 (looser for high dimensionality)
    /// - Code embeddings: 0.25 (tighter for specificity)
    /// - All other spaces: 0.3 (constitution default)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::birch::BIRCHParams;
    /// use context_graph_core::teleological::Embedder;
    ///
    /// let sparse_params = BIRCHParams::default_for_space(Embedder::Sparse);
    /// assert_eq!(sparse_params.threshold, 0.4);
    ///
    /// let code_params = BIRCHParams::default_for_space(Embedder::Code);
    /// assert_eq!(code_params.threshold, 0.25);
    /// ```
    pub fn default_for_space(embedder: Embedder) -> Self {
        let threshold = match embedder {
            // Sparse spaces need looser threshold due to high dimensionality
            Embedder::Sparse | Embedder::KeywordSplade => 0.4,
            // Code embeddings are more specific, need tighter threshold
            Embedder::Code => 0.25,
            // All other spaces use constitution default
            _ => 0.3,
        };

        Self {
            branching_factor: 50,
            threshold,
            max_node_entries: 50,
        }
    }

    /// Set branching factor.
    ///
    /// Value is NOT automatically clamped - use validate() to check.
    #[must_use]
    pub fn with_branching_factor(mut self, bf: usize) -> Self {
        self.branching_factor = bf;
        self
    }

    /// Set threshold.
    ///
    /// Value is NOT automatically clamped - use validate() to check.
    #[must_use]
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set max node entries.
    ///
    /// Value is NOT automatically clamped - use validate() to check.
    #[must_use]
    pub fn with_max_node_entries(mut self, entries: usize) -> Self {
        self.max_node_entries = entries;
        self
    }

    /// Validate parameters.
    ///
    /// Fails fast with descriptive error messages.
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::InvalidParameter` if:
    /// - branching_factor < 2
    /// - threshold <= 0.0 or threshold is NaN/Infinity
    /// - max_node_entries < branching_factor
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::birch::BIRCHParams;
    ///
    /// let invalid = BIRCHParams::new(1, 0.3, 50);
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), ClusterError> {
        if self.branching_factor < 2 {
            return Err(ClusterError::invalid_parameter(format!(
                "branching_factor must be >= 2, got {}. BIRCH tree nodes need at least 2 children.",
                self.branching_factor
            )));
        }

        if self.threshold <= 0.0 || self.threshold.is_nan() || self.threshold.is_infinite() {
            return Err(ClusterError::invalid_parameter(format!(
                "threshold must be > 0.0 and finite, got {}. Threshold controls cluster compactness.",
                self.threshold
            )));
        }

        if self.max_node_entries < self.branching_factor {
            return Err(ClusterError::invalid_parameter(format!(
                "max_node_entries ({}) must be >= branching_factor ({}). Leaf nodes must hold at least branching_factor entries.",
                self.max_node_entries, self.branching_factor
            )));
        }

        Ok(())
    }
}

/// Get default BIRCH parameters.
///
/// Returns params matching constitution defaults:
/// - branching_factor: 50
/// - threshold: 0.3
/// - max_node_entries: 50
pub fn birch_defaults() -> BIRCHParams {
    BIRCHParams::default()
}

// =============================================================================
// ClusteringFeature
// =============================================================================

/// Clustering Feature - statistical summary for BIRCH.
///
/// A CF is a triple (n, LS, SS) that summarizes a set of d-dimensional points:
/// - n: number of data points
/// - LS: linear sum, d-dimensional vector = Σ Xi
/// - SS: squared sum, scalar = Σ ||Xi||²
///
/// # Key Properties
///
/// 1. **Additivity**: CF(A ∪ B) = CF(A) + CF(B)
/// 2. **Sufficient Statistics**: Can compute centroid, radius, diameter
/// 3. **Compact**: O(d) space regardless of n
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::birch::ClusteringFeature;
///
/// let mut cf = ClusteringFeature::from_point(&[1.0, 2.0, 3.0]);
/// cf.add_point(&[2.0, 3.0, 4.0]).unwrap();
///
/// assert_eq!(cf.n, 2);
/// assert_eq!(cf.centroid(), vec![1.5, 2.5, 3.5]);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusteringFeature {
    /// Number of data points summarized.
    pub n: u32,
    /// Linear sum: Σ Xi (d-dimensional vector).
    pub ls: Vec<f32>,
    /// Squared sum: Σ ||Xi||² (scalar).
    pub ss: f32,
}

impl ClusteringFeature {
    /// Create empty CF with given dimension.
    pub fn new(dimension: usize) -> Self {
        Self {
            n: 0,
            ls: vec![0.0; dimension],
            ss: 0.0,
        }
    }

    /// Create CF from a single point.
    pub fn from_point(point: &[f32]) -> Self {
        let ss: f32 = point.iter().map(|x| x * x).sum();
        Self {
            n: 1,
            ls: point.to_vec(),
            ss,
        }
    }

    /// Get dimension of this CF.
    #[inline]
    pub fn dimension(&self) -> usize {
        self.ls.len()
    }

    /// Check if CF is empty (no points).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Compute centroid (mean point).
    ///
    /// centroid = LS / n
    ///
    /// Returns zero vector if n=0.
    pub fn centroid(&self) -> Vec<f32> {
        if self.n == 0 {
            return self.ls.clone(); // Zero vector
        }
        let n_f32 = self.n as f32;
        self.ls.iter().map(|x| x / n_f32).collect()
    }

    /// Compute radius (RMS distance from centroid to points).
    ///
    /// radius = sqrt(SS/n - ||centroid||²)
    ///
    /// Returns 0.0 if n=0 or if variance is negative (numerical precision).
    pub fn radius(&self) -> f32 {
        if self.n == 0 {
            return 0.0;
        }

        let centroid = self.centroid();
        let centroid_norm_sq: f32 = centroid.iter().map(|x| x * x).sum();
        let variance = (self.ss / self.n as f32) - centroid_norm_sq;

        // Handle numerical precision issues
        if variance < 0.0 || variance.is_nan() {
            0.0
        } else {
            variance.sqrt()
        }
    }

    /// Compute diameter (average pairwise distance approximation).
    ///
    /// diameter ≈ 2 * radius (approximation for spherical clusters)
    ///
    /// Returns 0.0 if n <= 1.
    pub fn diameter(&self) -> f32 {
        if self.n <= 1 {
            return 0.0;
        }
        2.0 * self.radius()
    }

    /// Merge another CF into this one.
    ///
    /// CF(A ∪ B) = (n_A + n_B, LS_A + LS_B, SS_A + SS_B)
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::DimensionMismatch` if dimensions differ.
    pub fn merge(&mut self, other: &ClusteringFeature) -> Result<(), ClusterError> {
        if other.n == 0 {
            return Ok(()); // Merging empty CF is no-op
        }

        if self.n == 0 {
            // Self is empty, just copy other
            self.n = other.n;
            self.ls = other.ls.clone();
            self.ss = other.ss;
            return Ok(());
        }

        // Check dimension match
        if self.ls.len() != other.ls.len() {
            return Err(ClusterError::dimension_mismatch(
                self.ls.len(),
                other.ls.len(),
            ));
        }

        // Additive merge
        self.n += other.n;
        for (a, b) in self.ls.iter_mut().zip(other.ls.iter()) {
            *a += b;
        }
        self.ss += other.ss;

        Ok(())
    }

    /// Add a single point to this CF.
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::DimensionMismatch` if point dimension differs.
    pub fn add_point(&mut self, point: &[f32]) -> Result<(), ClusterError> {
        // Initialize dimension if empty
        if self.ls.is_empty() {
            self.ls = vec![0.0; point.len()];
        }

        // Check dimension match
        if self.ls.len() != point.len() {
            return Err(ClusterError::dimension_mismatch(self.ls.len(), point.len()));
        }

        self.n += 1;
        for (a, b) in self.ls.iter_mut().zip(point.iter()) {
            *a += b;
        }
        self.ss += point.iter().map(|x| x * x).sum::<f32>();

        Ok(())
    }

    /// Compute Euclidean distance between centroids of two CFs.
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::DimensionMismatch` if dimensions differ.
    pub fn distance(&self, other: &ClusteringFeature) -> Result<f32, ClusterError> {
        if self.ls.len() != other.ls.len() {
            return Err(ClusterError::dimension_mismatch(
                self.ls.len(),
                other.ls.len(),
            ));
        }

        let c1 = self.centroid();
        let c2 = other.centroid();

        let dist_sq: f32 = c1
            .iter()
            .zip(c2.iter())
            .map(|(a, b)| (a - b) * (a - b))
            .sum();

        Ok(dist_sq.sqrt())
    }

    /// Check if a point would fit within threshold after merging.
    ///
    /// Computes the hypothetical radius without allocating a new CF.
    pub fn would_fit(&self, point: &[f32], threshold: f32) -> bool {
        if self.n == 0 {
            return true; // Empty CF accepts anything
        }

        if self.ls.len() != point.len() {
            return false; // Dimension mismatch
        }

        // Compute merged statistics inline without cloning
        let new_n = self.n + 1;
        let new_n_f32 = new_n as f32;

        // Compute new centroid and check radius
        let point_ss: f32 = point.iter().map(|x| x * x).sum();
        let new_ss = self.ss + point_ss;

        // centroid_norm_sq = ||new_ls / new_n||^2 = (1/new_n^2) * ||new_ls||^2
        let new_centroid_norm_sq: f32 = self
            .ls
            .iter()
            .zip(point.iter())
            .map(|(a, b)| {
                let sum = a + b;
                (sum / new_n_f32) * (sum / new_n_f32)
            })
            .sum();

        let variance = (new_ss / new_n_f32) - new_centroid_norm_sq;

        if variance < 0.0 || variance.is_nan() {
            return true; // Numerical precision: treat as zero radius
        }

        variance.sqrt() <= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // BIRCHParams DEFAULT VALUES TESTS
    // =========================================================================

    #[test]
    fn test_birch_defaults_match_constitution() {
        let params = birch_defaults();

        // Per constitution: BIRCH_DEFAULTS
        assert_eq!(
            params.branching_factor, 50,
            "branching_factor must be 50 per constitution"
        );
        assert!(
            (params.threshold - 0.3).abs() < f32::EPSILON,
            "threshold must be 0.3 per constitution"
        );
        assert_eq!(
            params.max_node_entries, 50,
            "max_node_entries must be 50 per constitution"
        );

        // Validate should pass for defaults
        assert!(params.validate().is_ok(), "Default params must be valid");

        println!("[PASS] test_birch_defaults_match_constitution - defaults verified");
    }

    // =========================================================================
    // BIRCHParams VALIDATION TESTS - FAIL FAST
    // =========================================================================

    #[test]
    fn test_validation_rejects_branching_factor_below_2() {
        let params = BIRCHParams::new(1, 0.3, 50);
        let result = params.validate();
        assert!(result.is_err(), "branching_factor=1 must be rejected");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("branching_factor"),
            "Error must mention field name"
        );
        assert!(err_msg.contains("2"), "Error must mention minimum value");

        println!(
            "[PASS] test_validation_rejects_branching_factor_below_2 - error: {}",
            err_msg
        );
    }

    #[test]
    fn test_validation_rejects_zero_threshold() {
        let params = BIRCHParams::new(50, 0.0, 50);
        let result = params.validate();
        assert!(result.is_err(), "threshold=0.0 must be rejected");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("threshold"),
            "Error must mention field name"
        );

        println!(
            "[PASS] test_validation_rejects_zero_threshold - error: {}",
            err_msg
        );
    }

    // =========================================================================
    // BIRCHParams BUILDER TESTS
    // =========================================================================

    #[test]
    fn test_builder_pattern() {
        let params = BIRCHParams::default()
            .with_branching_factor(100)
            .with_threshold(0.5)
            .with_max_node_entries(200);

        assert_eq!(params.branching_factor, 100);
        assert!((params.threshold - 0.5).abs() < f32::EPSILON);
        assert_eq!(params.max_node_entries, 200);
        assert!(params.validate().is_ok());

        println!("[PASS] test_builder_pattern - all builder methods work");
    }

    // =========================================================================
    // BIRCHParams SERIALIZATION TESTS
    // =========================================================================

    #[test]
    fn test_birch_params_serialization_roundtrip() {
        let params = BIRCHParams::default_for_space(Embedder::Code)
            .with_branching_factor(75)
            .with_max_node_entries(100);

        let json = serde_json::to_string(&params).expect("serialize must succeed");
        let restored: BIRCHParams = serde_json::from_str(&json).expect("deserialize must succeed");

        assert_eq!(params.branching_factor, restored.branching_factor);
        assert!((params.threshold - restored.threshold).abs() < f32::EPSILON);
        assert_eq!(params.max_node_entries, restored.max_node_entries);

        println!(
            "[PASS] test_birch_params_serialization_roundtrip - JSON: {}",
            json
        );
    }

    // =========================================================================
    // ClusteringFeature SERIALIZATION TESTS
    // =========================================================================

    #[test]
    fn test_cf_serialization_roundtrip() {
        let mut cf = ClusteringFeature::from_point(&[1.0, 2.0, 3.0]);
        cf.add_point(&[4.0, 5.0, 6.0]).unwrap();

        let json = serde_json::to_string(&cf).expect("serialize must succeed");
        let restored: ClusteringFeature =
            serde_json::from_str(&json).expect("deserialize must succeed");

        assert_eq!(cf.n, restored.n);
        assert_eq!(cf.ls, restored.ls);
        assert!((cf.ss - restored.ss).abs() < f32::EPSILON);

        println!("[PASS] test_cf_serialization_roundtrip - JSON: {}", json);
    }
}

// =============================================================================
// BIRCHEntry (TASK-P4-006)
// =============================================================================

/// Entry in a BIRCH node.
///
/// Each entry contains:
/// - A clustering feature (CF) summarizing points
/// - An optional child node (None for leaf entries)
/// - Memory IDs for leaf entries (empty for internal entries)
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::birch::BIRCHEntry;
/// use uuid::Uuid;
///
/// let id = Uuid::new_v4();
/// let entry = BIRCHEntry::from_point(&[1.0, 2.0, 3.0], id);
/// assert!(entry.is_leaf());
/// assert_eq!(entry.memory_ids.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct BIRCHEntry {
    /// Clustering feature summary.
    pub cf: ClusteringFeature,
    /// Child node (None for leaf entries).
    pub child: Option<Box<BIRCHNode>>,
    /// Memory IDs in this entry (leaf only).
    pub memory_ids: Vec<Uuid>,
}

impl BIRCHEntry {
    /// Create a new leaf entry from a single point.
    pub fn from_point(embedding: &[f32], memory_id: Uuid) -> Self {
        Self {
            cf: ClusteringFeature::from_point(embedding),
            child: None,
            memory_ids: vec![memory_id],
        }
    }

    /// Create a new non-leaf entry with a child node.
    pub fn with_child(cf: ClusteringFeature, child: BIRCHNode) -> Self {
        Self {
            cf,
            child: Some(Box::new(child)),
            memory_ids: Vec::new(),
        }
    }

    /// Check if this is a leaf entry (no child).
    #[inline]
    #[must_use]
    pub fn is_leaf(&self) -> bool {
        self.child.is_none()
    }

    /// Merge a point into this entry (leaf only).
    ///
    /// Adds the point to the CF and tracks the memory ID.
    pub fn merge_point(&mut self, embedding: &[f32], memory_id: Uuid) {
        // Safe to ignore error since we validate dimensions at tree level
        let _ = self.cf.add_point(embedding);
        self.memory_ids.push(memory_id);
    }

    /// Get the number of points in this entry.
    #[inline]
    #[must_use]
    pub fn n(&self) -> u32 {
        self.cf.n
    }
}

// =============================================================================
// BIRCHNode (TASK-P4-006)
// =============================================================================

/// Node in the BIRCH CF-tree.
///
/// A node is either:
/// - A leaf node: entries contain CFs and memory IDs directly
/// - An internal node: entries contain CFs and child node pointers
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::birch::BIRCHNode;
///
/// let leaf = BIRCHNode::new_leaf();
/// assert!(leaf.is_leaf);
/// assert!(leaf.entries.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct BIRCHNode {
    /// Whether this is a leaf node.
    pub is_leaf: bool,
    /// Entries in this node.
    pub entries: Vec<BIRCHEntry>,
}

impl BIRCHNode {
    /// Create a new empty leaf node.
    #[must_use]
    pub fn new_leaf() -> Self {
        Self {
            is_leaf: true,
            entries: Vec::new(),
        }
    }

    /// Create a new empty internal (non-leaf) node.
    #[must_use]
    pub fn new_internal() -> Self {
        Self {
            is_leaf: false,
            entries: Vec::new(),
        }
    }

    /// Compute total CF for all entries in this node.
    ///
    /// Returns an empty CF if node has no entries.
    #[must_use]
    pub fn total_cf(&self) -> ClusteringFeature {
        let dim = self.entries.first().map(|e| e.cf.dimension()).unwrap_or(0);

        let mut total = ClusteringFeature::new(dim);
        for entry in &self.entries {
            // Safe to ignore error - all entries should have same dimension
            let _ = total.merge(&entry.cf);
        }
        total
    }

    /// Find index of closest entry to a point.
    ///
    /// Returns None if node has no entries.
    #[must_use]
    pub fn find_closest(&self, point: &[f32]) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }

        let point_cf = ClusteringFeature::from_point(point);
        let mut min_dist = f32::INFINITY;
        let mut min_idx = 0;

        for (i, entry) in self.entries.iter().enumerate() {
            match entry.cf.distance(&point_cf) {
                Ok(dist) => {
                    if dist < min_dist {
                        min_dist = dist;
                        min_idx = i;
                    }
                }
                Err(e) => {
                    warn!(entry_idx = i, error = %e, "BIRCH find_closest: distance error, skipping entry");
                }
            }
        }

        Some(min_idx)
    }

    /// Get number of entries in this node.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if node has no entries.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// =============================================================================
// BIRCHTree (TASK-P4-006)
// =============================================================================

/// BIRCH CF-tree for incremental clustering.
///
/// Implements O(log n) insertion via tree traversal. When nodes overflow
/// (exceed max_node_entries), they are split using the farthest-pair algorithm.
///
/// # Architecture
///
/// Per constitution:
/// - branching_factor: 50
/// - threshold: 0.3 (adaptive)
/// - max_node_entries: 50
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::birch::{BIRCHTree, birch_defaults};
/// use uuid::Uuid;
///
/// let mut tree = BIRCHTree::new(birch_defaults(), 128).unwrap();
/// let id = Uuid::new_v4();
/// let cluster_idx = tree.insert(&vec![0.0; 128], id).unwrap();
///
/// assert_eq!(tree.total_points(), 1);
/// assert!(tree.cluster_count() >= 1);
/// ```
#[derive(Debug)]
pub struct BIRCHTree {
    params: BIRCHParams,
    root: BIRCHNode,
    dimension: usize,
    total_points: usize,
}

impl BIRCHTree {
    /// Create a new empty BIRCH tree.
    ///
    /// # Arguments
    ///
    /// * `params` - BIRCH parameters (branching_factor, threshold, max_node_entries)
    /// * `dimension` - Expected embedding dimension
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::InvalidParameter` if:
    /// - dimension is 0
    /// - params.validate() fails
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::birch::{BIRCHTree, birch_defaults};
    ///
    /// let tree = BIRCHTree::new(birch_defaults(), 128).unwrap();
    /// assert_eq!(tree.total_points(), 0);
    /// ```
    pub fn new(params: BIRCHParams, dimension: usize) -> Result<Self, ClusterError> {
        if dimension == 0 {
            return Err(ClusterError::invalid_parameter(
                "dimension must be > 0; BIRCH tree requires positive embedding dimension",
            ));
        }

        params.validate()?;

        Ok(Self {
            params,
            root: BIRCHNode::new_leaf(),
            dimension,
            total_points: 0,
        })
    }

    /// Insert a point into the tree.
    ///
    /// Returns the cluster index (0-based) for this point.
    ///
    /// # Arguments
    ///
    /// * `embedding` - The embedding vector
    /// * `memory_id` - UUID to track this memory
    ///
    /// # Errors
    ///
    /// Returns `ClusterError::DimensionMismatch` if embedding dimension doesn't match.
    /// Returns `ClusterError::InvalidParameter` if embedding contains NaN/Infinity.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::birch::{BIRCHTree, birch_defaults};
    /// use uuid::Uuid;
    ///
    /// let mut tree = BIRCHTree::new(birch_defaults(), 3).unwrap();
    /// let id = Uuid::new_v4();
    /// let cluster_idx = tree.insert(&[1.0, 2.0, 3.0], id).unwrap();
    /// ```
    pub fn insert(&mut self, embedding: &[f32], memory_id: Uuid) -> Result<usize, ClusterError> {
        // Validate dimension
        if embedding.len() != self.dimension {
            return Err(ClusterError::dimension_mismatch(
                self.dimension,
                embedding.len(),
            ));
        }

        // Validate no NaN/Infinity (AP-10: No NaN/Infinity in similarity scores)
        for (i, &val) in embedding.iter().enumerate() {
            if !val.is_finite() {
                return Err(ClusterError::invalid_parameter(format!(
                    "embedding[{}] is not finite: {}; BIRCH requires finite values",
                    i, val
                )));
            }
        }

        // Insert into tree
        let cluster_idx = self.insert_recursive(embedding, memory_id);

        // Check if root needs splitting
        if self.root.entries.len() > self.params.max_node_entries {
            self.split_root();
        }

        self.total_points += 1;
        Ok(cluster_idx)
    }

    /// Recursive insertion into a node.
    ///
    /// Returns the cluster index for the inserted point.
    fn insert_recursive(&mut self, embedding: &[f32], memory_id: Uuid) -> usize {
        if self.root.is_leaf {
            // Find closest entry or create new one
            if let Some(idx) = self.find_closest_fitting_entry(&self.root, embedding) {
                // Merge into existing entry - need to work around borrow checker
                let entry = &mut self.root.entries[idx];
                entry.merge_point(embedding, memory_id);
                idx
            } else {
                // Create new entry
                let new_entry = BIRCHEntry::from_point(embedding, memory_id);
                let new_idx = self.root.entries.len();
                self.root.entries.push(new_entry);

                // Check if leaf needs splitting (handled at caller level for root)
                new_idx
            }
        } else {
            // Non-leaf: find closest child and descend
            let closest_idx = self.root.find_closest(embedding).unwrap_or(0);

            // Take child out temporarily to avoid borrow conflicts
            let mut child_opt = self.root.entries[closest_idx].child.take();

            if let Some(ref mut child) = child_opt {
                // Insert into child (now we have exclusive access)
                let threshold = self.params.threshold;
                let cluster_idx =
                    Self::insert_into_node_owned(child, embedding, memory_id, threshold);

                // Check if child needs splitting before putting back
                let needs_split = child.entries.len() > self.params.max_node_entries;

                // Put child back
                self.root.entries[closest_idx].child = child_opt;

                // Update CF in parent
                let _ = self.root.entries[closest_idx].cf.add_point(embedding);

                // Handle split if needed
                if needs_split {
                    self.split_child(closest_idx);
                }

                cluster_idx
            } else {
                // Put None back (shouldn't happen in well-formed tree)
                self.root.entries[closest_idx].child = child_opt;
                0
            }
        }
    }

    /// Insert into a node without requiring &self (avoids borrow conflicts).
    /// Uses explicit threshold parameter instead of self.params.
    fn insert_into_node_owned(
        node: &mut BIRCHNode,
        embedding: &[f32],
        memory_id: Uuid,
        threshold: f32,
    ) -> usize {
        if node.is_leaf {
            // Find closest fitting entry or create new
            if let Some(idx) = Self::find_closest_fitting_in_node(node, embedding, threshold) {
                node.entries[idx].merge_point(embedding, memory_id);
                idx
            } else {
                let new_entry = BIRCHEntry::from_point(embedding, memory_id);
                let new_idx = node.entries.len();
                node.entries.push(new_entry);
                new_idx
            }
        } else {
            // Non-leaf: descend to closest child
            let closest_idx = node.find_closest(embedding).unwrap_or(0);

            if let Some(ref mut child) = node.entries[closest_idx].child {
                let cluster_idx =
                    Self::insert_into_node_owned(child, embedding, memory_id, threshold);
                let _ = node.entries[closest_idx].cf.add_point(embedding);
                cluster_idx
            } else {
                0
            }
        }
    }

    /// Find closest fitting entry in a node (static version, no &self).
    fn find_closest_fitting_in_node(
        node: &BIRCHNode,
        embedding: &[f32],
        threshold: f32,
    ) -> Option<usize> {
        if node.entries.is_empty() {
            return None;
        }

        let mut best_idx = None;
        let mut best_dist = f32::INFINITY;

        for (i, entry) in node.entries.iter().enumerate() {
            if entry.cf.would_fit(embedding, threshold) {
                let point_cf = ClusteringFeature::from_point(embedding);
                match entry.cf.distance(&point_cf) {
                    Ok(dist) => {
                        if dist < best_dist {
                            best_dist = dist;
                            best_idx = Some(i);
                        }
                    }
                    Err(e) => {
                        warn!(entry_idx = i, error = %e, "BIRCH find_closest_fitting_entry: distance error, skipping entry");
                    }
                }
            }
        }

        best_idx
    }

    /// Find closest entry that can fit the point within threshold.
    #[allow(dead_code)]
    fn find_closest_fitting_entry(&self, node: &BIRCHNode, embedding: &[f32]) -> Option<usize> {
        self.find_closest_fitting_entry_in_node(node, embedding)
    }

    /// Find closest entry in a node that can fit the point.
    fn find_closest_fitting_entry_in_node(
        &self,
        node: &BIRCHNode,
        embedding: &[f32],
    ) -> Option<usize> {
        if node.entries.is_empty() {
            return None;
        }

        let mut best_idx = None;
        let mut best_dist = f32::INFINITY;

        for (i, entry) in node.entries.iter().enumerate() {
            if entry.cf.would_fit(embedding, self.params.threshold) {
                let point_cf = ClusteringFeature::from_point(embedding);
                match entry.cf.distance(&point_cf) {
                    Ok(dist) => {
                        if dist < best_dist {
                            best_dist = dist;
                            best_idx = Some(i);
                        }
                    }
                    Err(e) => {
                        warn!(entry_idx = i, error = %e, "BIRCH find_closest_in_tree: distance error, skipping entry");
                    }
                }
            }
        }

        best_idx
    }

    /// Split root node, increasing tree height.
    fn split_root(&mut self) {
        if self.root.entries.len() <= self.params.max_node_entries {
            return;
        }

        let (seed1, seed2) = self.find_farthest_pair(&self.root.entries);

        // Create two new nodes
        let mut node1 = BIRCHNode {
            is_leaf: self.root.is_leaf,
            entries: Vec::new(),
        };
        let mut node2 = BIRCHNode {
            is_leaf: self.root.is_leaf,
            entries: Vec::new(),
        };

        // Distribute entries
        for (i, entry) in self.root.entries.drain(..).enumerate() {
            if i == seed1 {
                node1.entries.push(entry);
            } else if i == seed2 {
                node2.entries.push(entry);
            } else {
                // Assign to closer seed
                let cf1 = if node1.entries.is_empty() {
                    ClusteringFeature::new(self.dimension)
                } else {
                    node1.total_cf()
                };
                let cf2 = if node2.entries.is_empty() {
                    ClusteringFeature::new(self.dimension)
                } else {
                    node2.total_cf()
                };

                let dist1 = entry.cf.distance(&cf1).unwrap_or(f32::MAX);
                let dist2 = entry.cf.distance(&cf2).unwrap_or(f32::MAX);

                if dist1 <= dist2 {
                    node1.entries.push(entry);
                } else {
                    node2.entries.push(entry);
                }
            }
        }

        // Create new root with two children
        let cf1 = node1.total_cf();
        let cf2 = node2.total_cf();

        let entry1 = BIRCHEntry::with_child(cf1, node1);
        let entry2 = BIRCHEntry::with_child(cf2, node2);

        self.root = BIRCHNode::new_internal();
        self.root.entries.push(entry1);
        self.root.entries.push(entry2);
    }

    /// Split a child node at the given index.
    fn split_child(&mut self, child_idx: usize) {
        let child = match self.root.entries[child_idx].child.take() {
            Some(c) => c,
            None => return,
        };

        if child.entries.len() <= self.params.max_node_entries {
            self.root.entries[child_idx].child = Some(child);
            return;
        }

        let (seed1, seed2) = self.find_farthest_pair(&child.entries);

        let mut node1 = BIRCHNode {
            is_leaf: child.is_leaf,
            entries: Vec::new(),
        };
        let mut node2 = BIRCHNode {
            is_leaf: child.is_leaf,
            entries: Vec::new(),
        };

        // Distribute entries
        for (i, entry) in child.entries.into_iter().enumerate() {
            if i == seed1 {
                node1.entries.push(entry);
            } else if i == seed2 {
                node2.entries.push(entry);
            } else {
                let cf1 = if node1.entries.is_empty() {
                    ClusteringFeature::new(self.dimension)
                } else {
                    node1.total_cf()
                };
                let cf2 = if node2.entries.is_empty() {
                    ClusteringFeature::new(self.dimension)
                } else {
                    node2.total_cf()
                };

                let dist1 = entry.cf.distance(&cf1).unwrap_or(f32::MAX);
                let dist2 = entry.cf.distance(&cf2).unwrap_or(f32::MAX);

                if dist1 <= dist2 {
                    node1.entries.push(entry);
                } else {
                    node2.entries.push(entry);
                }
            }
        }

        // Update existing entry and add new one
        let cf1 = node1.total_cf();
        let cf2 = node2.total_cf();

        self.root.entries[child_idx].cf = cf1;
        self.root.entries[child_idx].child = Some(Box::new(node1));

        let new_entry = BIRCHEntry::with_child(cf2, node2);
        self.root.entries.push(new_entry);
    }

    /// Find farthest pair of entries (for split seeds).
    fn find_farthest_pair(&self, entries: &[BIRCHEntry]) -> (usize, usize) {
        if entries.len() < 2 {
            return (0, entries.len().saturating_sub(1));
        }

        let mut max_dist = 0.0f32;
        let mut pair = (0, 1);

        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                match entries[i].cf.distance(&entries[j].cf) {
                    Ok(dist) => {
                        if dist > max_dist {
                            max_dist = dist;
                            pair = (i, j);
                        }
                    }
                    Err(e) => {
                        warn!(i = i, j = j, error = %e, "BIRCH find_farthest_pair: distance error, skipping pair");
                    }
                }
            }
        }

        pair
    }

    /// Get all leaf CFs as cluster summaries.
    ///
    /// Returns one ClusteringFeature per cluster (leaf entry).
    #[must_use]
    pub fn get_clusters(&self) -> Vec<ClusteringFeature> {
        let mut clusters = Vec::new();
        self.collect_leaf_cfs(&self.root, &mut clusters);
        clusters
    }

    /// Recursively collect leaf CFs.
    fn collect_leaf_cfs(&self, node: &BIRCHNode, clusters: &mut Vec<ClusteringFeature>) {
        if node.is_leaf {
            for entry in &node.entries {
                clusters.push(entry.cf.clone());
            }
        } else {
            for entry in &node.entries {
                if let Some(ref child) = entry.child {
                    self.collect_leaf_cfs(child, clusters);
                }
            }
        }
    }

    /// Get cluster members: (cluster_index, memory_ids).
    ///
    /// Returns a vector of tuples, where each tuple contains:
    /// - The cluster index (0-based)
    /// - The list of memory IDs in that cluster
    #[must_use]
    pub fn get_cluster_members(&self) -> Vec<(usize, Vec<Uuid>)> {
        let mut members = Vec::new();
        let mut idx = 0;
        self.collect_members(&self.root, &mut members, &mut idx);
        members
    }

    /// Recursively collect cluster members.
    fn collect_members(
        &self,
        node: &BIRCHNode,
        members: &mut Vec<(usize, Vec<Uuid>)>,
        idx: &mut usize,
    ) {
        if node.is_leaf {
            for entry in &node.entries {
                members.push((*idx, entry.memory_ids.clone()));
                *idx += 1;
            }
        } else {
            for entry in &node.entries {
                if let Some(ref child) = entry.child {
                    self.collect_members(child, members, idx);
                }
            }
        }
    }

    /// Adapt threshold to achieve target cluster count.
    ///
    /// Uses binary search to find appropriate threshold.
    /// Note: This modifies params but does NOT rebuild the tree.
    /// For full effect, rebuild by reinserting all points.
    pub fn adapt_threshold(&mut self, target_cluster_count: usize) {
        let current_count = self.cluster_count();

        if current_count == target_cluster_count || target_cluster_count == 0 {
            return;
        }

        // Binary search for appropriate threshold
        let mut low = 0.01f32;
        let mut high = 2.0f32;

        for _ in 0..10 {
            let mid = (low + high) / 2.0;

            // Estimate: lower threshold = more clusters
            if current_count < target_cluster_count {
                high = mid; // Need more clusters, decrease threshold
            } else {
                low = mid; // Need fewer clusters, increase threshold
            }
        }

        let new_threshold = (low + high) / 2.0;
        self.params = self.params.clone().with_threshold(new_threshold);
    }

    /// Get current cluster count.
    #[inline]
    #[must_use]
    pub fn cluster_count(&self) -> usize {
        self.get_clusters().len()
    }

    /// Get total points in tree.
    #[inline]
    #[must_use]
    pub fn total_points(&self) -> usize {
        self.total_points
    }

    /// Get tree parameters.
    #[inline]
    #[must_use]
    pub fn params(&self) -> &BIRCHParams {
        &self.params
    }

    /// Get tree dimension.
    #[inline]
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Get number of points tracked via ClusteringFeature.
    ///
    /// This sums all CF.n values in leaf entries.
    #[must_use]
    pub fn get_cf_total_points(&self) -> u32 {
        self.get_clusters().iter().map(|cf| cf.n).sum()
    }
}

// =============================================================================
// BIRCH Tree Tests (TASK-P4-006)
// =============================================================================

#[cfg(test)]
mod birch_tree_tests {
    use super::*;

    // =========================================================================
    // BIRCHTree INSERT TESTS
    // =========================================================================

    #[test]
    fn test_birch_insert_single_point() {
        let mut tree = BIRCHTree::new(birch_defaults(), 3).expect("valid tree");
        let id = Uuid::new_v4();

        let result = tree.insert(&[1.0, 2.0, 3.0], id);
        assert!(result.is_ok(), "Insert should succeed");

        assert_eq!(tree.total_points(), 1);
        let clusters = tree.get_clusters();
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].n, 1);

        let members = tree.get_cluster_members();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].1.len(), 1);
        assert_eq!(members[0].1[0], id);

        println!("[PASS] test_birch_insert_single_point");
    }

    #[test]
    fn test_birch_insert_dimension_mismatch() {
        let mut tree = BIRCHTree::new(birch_defaults(), 3).expect("valid tree");

        let result = tree.insert(&[1.0, 2.0], Uuid::new_v4()); // Wrong dimension

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            ClusterError::DimensionMismatch {
                expected: 3,
                actual: 2
            }
        ));

        println!("[PASS] test_birch_insert_dimension_mismatch");
    }

    #[test]
    fn test_birch_insert_nan_rejected() {
        let mut tree = BIRCHTree::new(birch_defaults(), 3).expect("valid tree");

        let result = tree.insert(&[1.0, f32::NAN, 3.0], Uuid::new_v4());

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ClusterError::InvalidParameter { .. }));
        let msg = err.to_string();
        assert!(msg.contains("not finite") || msg.contains("NaN"));

        println!("[PASS] test_birch_insert_nan_rejected");
    }

    // =========================================================================
    // BIRCHTree CLUSTERING BEHAVIOR TESTS
    // =========================================================================

    #[test]
    fn test_birch_merge_close_points() {
        // High threshold to encourage merging
        let params = BIRCHParams::default().with_threshold(10.0);
        let mut tree = BIRCHTree::new(params, 2).expect("valid tree");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        tree.insert(&[0.0, 0.0], id1).expect("insert 1");
        tree.insert(&[0.1, 0.1], id2).expect("insert 2");

        assert_eq!(tree.total_points(), 2);

        // With high threshold, close points should merge
        let clusters = tree.get_clusters();
        assert_eq!(
            clusters.len(),
            1,
            "Close points should merge into one cluster"
        );
        assert_eq!(clusters[0].n, 2, "Cluster should have 2 points");

        let members = tree.get_cluster_members();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].1.len(), 2);
        assert!(members[0].1.contains(&id1));
        assert!(members[0].1.contains(&id2));

        println!("[PASS] test_birch_merge_close_points");
    }

    #[test]
    fn test_birch_separate_distant_points() {
        // Low threshold to keep points separate
        let params = BIRCHParams::default().with_threshold(0.01);
        let mut tree = BIRCHTree::new(params, 2).expect("valid tree");

        tree.insert(&[0.0, 0.0], Uuid::new_v4()).expect("insert 1");
        tree.insert(&[100.0, 100.0], Uuid::new_v4())
            .expect("insert 2");

        let clusters = tree.get_clusters();
        assert_eq!(
            clusters.len(),
            2,
            "Distant points should be in separate clusters"
        );

        println!("[PASS] test_birch_separate_distant_points");
    }

    #[test]
    fn test_birch_memory_id_tracking() {
        let params = birch_defaults();
        let mut tree = BIRCHTree::new(params, 2).expect("valid tree");

        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        for (i, id) in ids.iter().enumerate() {
            tree.insert(&[(i as f32) * 10.0, 0.0], *id).expect("insert");
        }

        let members = tree.get_cluster_members();
        let all_ids: Vec<Uuid> = members.iter().flat_map(|(_, m)| m.clone()).collect();

        assert_eq!(all_ids.len(), 5, "All 5 IDs should be tracked");
        for id in &ids {
            assert!(all_ids.contains(id), "ID {} should be tracked", id);
        }

        println!("[PASS] test_birch_memory_id_tracking");
    }

    // =========================================================================
    // EDGE CASES
    // =========================================================================

    #[test]
    fn test_birch_many_insertions() {
        let params = BIRCHParams::default().with_threshold(0.1);
        let mut tree = BIRCHTree::new(params, 2).expect("valid tree");

        let n = 100;
        let mut ids = Vec::new();

        for i in 0..n {
            let id = Uuid::new_v4();
            ids.push(id);
            let point = [(i as f32) * 0.5, (i as f32) * 0.3];
            tree.insert(&point, id).expect("insert");
        }

        assert_eq!(tree.total_points(), n);

        let members = tree.get_cluster_members();
        let tracked: Vec<Uuid> = members.iter().flat_map(|(_, m)| m.clone()).collect();
        assert_eq!(tracked.len(), n, "All {} IDs should be tracked", n);

        for id in &ids {
            assert!(tracked.contains(id), "ID should be tracked");
        }

        println!("[PASS] test_birch_many_insertions - {} points", n);
    }

    #[test]
    fn test_birch_node_split() {
        // Low max_node_entries to trigger splits
        let params = BIRCHParams::new(2, 0.01, 3); // max 3 entries per node
        let mut tree = BIRCHTree::new(params, 2).expect("valid tree");

        // Insert more points than max_node_entries
        for i in 0..10 {
            let point = [(i as f32) * 100.0, 0.0]; // Far apart to prevent merging
            tree.insert(&point, Uuid::new_v4()).expect("insert");
        }

        assert_eq!(tree.total_points(), 10);

        // Tree should have split (not all entries in root if internal)
        let clusters = tree.get_clusters();
        assert!(!clusters.is_empty(), "Should have clusters after splits");

        // Verify all points are tracked
        let members = tree.get_cluster_members();
        let total: usize = members.iter().map(|(_, m)| m.len()).sum();
        assert_eq!(total, 10, "All 10 points should be tracked");

        println!("[PASS] test_birch_node_split - {} clusters", clusters.len());
    }

    // =========================================================================
    // PHYSICAL OUTPUT VERIFICATION
    // =========================================================================

    #[test]
    fn test_physical_output_verification() {
        // Create tree and insert known data
        let params = BIRCHParams::default().with_threshold(1.0);
        let mut tree = BIRCHTree::new(params, 2).expect("valid tree");

        let id1 = Uuid::from_u128(1);
        let id2 = Uuid::from_u128(2);
        let id3 = Uuid::from_u128(3);

        tree.insert(&[0.0, 0.0], id1).expect("insert 1");
        tree.insert(&[0.1, 0.1], id2).expect("insert 2");
        tree.insert(&[100.0, 100.0], id3).expect("insert 3");

        // Physical verification 1: Memory IDs are ACTUALLY stored
        let members = tree.get_cluster_members();
        let all_ids: Vec<Uuid> = members.iter().flat_map(|(_, ids)| ids.clone()).collect();

        println!("[PHYSICAL] All tracked IDs: {:?}", all_ids);
        assert_eq!(all_ids.len(), 3, "Must have 3 IDs");
        assert!(all_ids.contains(&id1), "id1 must be stored");
        assert!(all_ids.contains(&id2), "id2 must be stored");
        assert!(all_ids.contains(&id3), "id3 must be stored");

        // Physical verification 2: Sum of members equals total_points
        let member_sum: usize = members.iter().map(|(_, ids)| ids.len()).sum();
        assert_eq!(
            member_sum,
            tree.total_points(),
            "Member sum must equal total_points"
        );
        println!(
            "[PHYSICAL] Member sum: {}, total_points: {}",
            member_sum,
            tree.total_points()
        );

        // Physical verification 3: CF values are mathematically correct
        let clusters = tree.get_clusters();
        for (i, cf) in clusters.iter().enumerate() {
            println!(
                "[PHYSICAL] Cluster {}: n={}, centroid={:?}, radius={}",
                i,
                cf.n,
                cf.centroid(),
                cf.radius()
            );

            assert!(cf.n > 0, "Cluster {} must have points", i);
            assert!(
                cf.centroid().iter().all(|v| v.is_finite()),
                "Centroid must be finite"
            );
            assert!(cf.radius() >= 0.0, "Radius must be non-negative");
        }

        // Physical verification 4: Tree structure is valid
        assert!(
            tree.root.entries.iter().all(|e| {
                if e.is_leaf() {
                    e.child.is_none()
                } else {
                    e.child.is_some() && e.memory_ids.is_empty()
                }
            }),
            "Tree structure must be valid"
        );

        println!("[PASS] test_physical_output_verification");
    }
}
