//! Goal hierarchy types for teleological alignment.
//!
//! Provides the goal tree structure that defines the North Star and sub-goals
//! for purpose vector computation.
//!
//! # Architecture (constitution.yaml)
//!
//! - **ARCH-02**: Goals use TeleologicalArray for apples-to-apples comparison
//! - **ARCH-03**: Goals are discovered autonomously, not manually created
//! - **ARCH-05**: All 13 embedders must be present in teleological_array
//!
//! # Example
//!
//! ```ignore
//! use context_graph_core::purpose::{GoalNode, GoalLevel, DiscoveryMethod, GoalDiscoveryMetadata};
//! use context_graph_core::types::fingerprint::SemanticFingerprint;
//!
//! // Goals are created from clustering analysis
//! let discovery = GoalDiscoveryMetadata::new(
//!     DiscoveryMethod::Clustering,
//!     0.85,  // confidence
//!     42,    // cluster_size
//!     0.78,  // coherence
//! ).unwrap();
//!
//! let centroid_fingerprint = SemanticFingerprint::zeroed();
//! let goal = GoalNode::autonomous_goal(
//!     "Emergent ML mastery goal".to_string(),
//!     GoalLevel::NorthStar,
//!     centroid_fingerprint,
//!     discovery,
//! ).unwrap();
//! ```

use crate::types::fingerprint::{TeleologicalArray, ValidationError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Error when creating or validating a GoalNode.
#[derive(Debug, Clone, Error)]
pub enum GoalNodeError {
    /// The teleological array failed validation.
    #[error("Invalid teleological array: {0}")]
    InvalidArray(#[from] ValidationError),

    /// Discovery confidence is out of range [0.0, 1.0].
    #[error("Discovery confidence must be in [0.0, 1.0], got {0}")]
    InvalidConfidence(f32),

    /// Discovery coherence is out of range [0.0, 1.0].
    #[error("Discovery coherence must be in [0.0, 1.0], got {0}")]
    InvalidCoherence(f32),

    /// Cluster size must be > 0 for discovered goals.
    #[error("Cluster size must be > 0 for discovered goals")]
    EmptyCluster,
}

/// How a goal was discovered.
///
/// Goals are discovered AUTONOMOUSLY from memory patterns.
/// Manual goal creation is forbidden per ARCH-03.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    /// Discovered via k-means or HDBSCAN clustering of fingerprints.
    Clustering,
    /// Discovered via pattern recognition in purpose vectors.
    PatternRecognition,
    /// Created by decomposing a parent goal into sub-goals.
    Decomposition,
    /// Bootstrapped from initial memory analysis (first North Star).
    Bootstrap,
}

/// Metadata about how a goal was autonomously discovered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalDiscoveryMetadata {
    /// How this goal was discovered.
    pub method: DiscoveryMethod,
    /// Confidence score [0.0, 1.0].
    pub confidence: f32,
    /// Number of memories in the cluster that formed this goal.
    pub cluster_size: usize,
    /// Coherence score of the cluster [0.0, 1.0].
    pub coherence: f32,
    /// Timestamp when discovery occurred.
    pub discovered_at: DateTime<Utc>,
}

impl GoalDiscoveryMetadata {
    /// Create new discovery metadata with validation.
    ///
    /// # Errors
    ///
    /// Returns `GoalNodeError` if:
    /// - Confidence is not in [0.0, 1.0]
    /// - Coherence is not in [0.0, 1.0]
    /// - Cluster size is 0 for non-Bootstrap methods
    pub fn new(
        method: DiscoveryMethod,
        confidence: f32,
        cluster_size: usize,
        coherence: f32,
    ) -> Result<Self, GoalNodeError> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(GoalNodeError::InvalidConfidence(confidence));
        }
        if !(0.0..=1.0).contains(&coherence) {
            return Err(GoalNodeError::InvalidCoherence(coherence));
        }
        if cluster_size == 0 && method != DiscoveryMethod::Bootstrap {
            return Err(GoalNodeError::EmptyCluster);
        }
        Ok(Self {
            method,
            confidence,
            cluster_size,
            coherence,
            discovered_at: Utc::now(),
        })
    }

    /// Create bootstrap metadata (for initial North Star).
    ///
    /// Bootstrap goals start with zero confidence and coherence,
    /// which will be computed after more data is available.
    pub fn bootstrap() -> Self {
        Self {
            method: DiscoveryMethod::Bootstrap,
            confidence: 0.0,
            cluster_size: 0,
            coherence: 0.0,
            discovered_at: Utc::now(),
        }
    }
}

/// Goal hierarchy level.
///
/// Defines the position of a goal in the hierarchical tree structure.
/// Each level has a different propagation weight for alignment computation.
///
/// From constitution.yaml:
/// - NorthStar: 1.0 weight (global goal)
/// - Strategic: 0.7 weight (mid-level)
/// - Tactical: 0.4 weight (short-term)
/// - Immediate: 0.2 weight (per-operation)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum GoalLevel {
    /// Top-level aspirational goal (the "North Star").
    /// Only one allowed per hierarchy.
    NorthStar = 0,

    /// Mid-term strategic objectives.
    /// Children of NorthStar.
    Strategic = 1,

    /// Short-term tactical goals.
    /// Children of Strategic goals.
    Tactical = 2,

    /// Immediate context goals.
    /// Lowest level, most specific.
    Immediate = 3,
}

impl GoalLevel {
    /// Weight factor for hierarchical propagation.
    ///
    /// From constitution.yaml:
    /// - NorthStar: 1.0
    /// - Strategic: 0.7
    /// - Tactical: 0.4
    /// - Immediate: 0.2
    #[inline]
    pub fn propagation_weight(&self) -> f32 {
        match self {
            GoalLevel::NorthStar => 1.0,
            GoalLevel::Strategic => 0.7,
            GoalLevel::Tactical => 0.4,
            GoalLevel::Immediate => 0.2,
        }
    }

    /// Get numeric depth (0 = NorthStar, 3 = Immediate).
    #[inline]
    pub fn depth(&self) -> u8 {
        *self as u8
    }
}

/// A goal node in the purpose hierarchy.
///
/// Goals are discovered AUTONOMOUSLY from memory patterns.
/// They represent emergent purpose from stored teleological fingerprints.
///
/// # Architectural Rules (constitution.yaml)
///
/// - ARCH-02: Goals use TeleologicalArray for apples-to-apples comparison
/// - ARCH-03: Goals are discovered, not manually created
/// - ARCH-05: All 13 embedders must be present in teleological_array
///
/// # Example
///
/// ```ignore
/// use context_graph_core::purpose::{GoalNode, GoalLevel, DiscoveryMethod, GoalDiscoveryMetadata};
/// use context_graph_core::types::fingerprint::SemanticFingerprint;
///
/// // Goals are created from clustering analysis
/// let discovery = GoalDiscoveryMetadata::new(
///     DiscoveryMethod::Clustering,
///     0.85,  // confidence
///     42,    // cluster_size
///     0.78,  // coherence
/// ).unwrap();
///
/// let centroid_fingerprint = SemanticFingerprint::zeroed();
/// let goal = GoalNode::autonomous_goal(
///     "Emergent ML mastery goal".to_string(),
///     GoalLevel::NorthStar,
///     centroid_fingerprint,
///     discovery,
/// ).unwrap();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalNode {
    /// Unique identifier (UUID).
    pub id: Uuid,

    /// Human-readable description.
    pub description: String,

    /// Hierarchical level.
    pub level: GoalLevel,

    /// The teleological array representing this goal.
    ///
    /// This is a SemanticFingerprint containing all 13 embeddings.
    /// Goals can be compared apples-to-apples with memories:
    /// - Goal.E1 vs Memory.E1 (semantic)
    /// - Goal.E5 vs Memory.E5 (causal)
    /// - Goal.E7 vs Memory.E7 (code)
    ///   etc.
    pub teleological_array: TeleologicalArray,

    /// Parent goal (None for NorthStar).
    pub parent_id: Option<Uuid>,

    /// Child goal IDs.
    pub child_ids: Vec<Uuid>,

    /// How this goal was discovered.
    pub discovery: GoalDiscoveryMetadata,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl GoalNode {
    /// Create a new autonomously discovered goal.
    ///
    /// This is the ONLY way to create goals. Manual goal creation is forbidden
    /// per ARCH-03.
    ///
    /// # Arguments
    ///
    /// * `description` - Human-readable goal description
    /// * `level` - Position in goal hierarchy
    /// * `teleological_array` - The 13-embedder fingerprint from clustering centroid
    /// * `discovery` - Metadata about how this goal was discovered
    ///
    /// # Errors
    ///
    /// Returns `GoalNodeError::InvalidArray` if:
    /// - The teleological array fails validation (incomplete embeddings, wrong dimensions)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let goal = GoalNode::autonomous_goal(
    ///     "Strategic code quality goal".into(),
    ///     GoalLevel::Strategic,
    ///     centroid_fingerprint,
    ///     discovery,
    /// )?;
    /// ```
    pub fn autonomous_goal(
        description: String,
        level: GoalLevel,
        teleological_array: TeleologicalArray,
        discovery: GoalDiscoveryMetadata,
    ) -> Result<Self, GoalNodeError> {
        // Fail fast if array is invalid
        teleological_array.validate_strict()?;

        Ok(Self {
            id: Uuid::new_v4(),
            description,
            level,
            teleological_array,
            parent_id: None,
            child_ids: Vec::new(),
            discovery,
            created_at: Utc::now(),
        })
    }

    /// Create a child goal with a parent reference.
    ///
    /// Used when decomposing a parent goal into sub-goals.
    ///
    /// # Panics
    ///
    /// Panics if `level` is `GoalLevel::NorthStar` (child goals cannot be North Star).
    ///
    /// # Errors
    ///
    /// Returns `GoalNodeError::InvalidArray` if the teleological array fails validation.
    pub fn child_goal(
        description: String,
        level: GoalLevel,
        parent_id: Uuid,
        teleological_array: TeleologicalArray,
        discovery: GoalDiscoveryMetadata,
    ) -> Result<Self, GoalNodeError> {
        assert!(
            level != GoalLevel::NorthStar,
            "Child goal cannot be NorthStar level"
        );

        teleological_array.validate_strict()?;

        Ok(Self {
            id: Uuid::new_v4(),
            description,
            level,
            teleological_array,
            parent_id: Some(parent_id),
            child_ids: Vec::new(),
            discovery,
            created_at: Utc::now(),
        })
    }

    /// Get the teleological array for comparison.
    #[inline]
    pub fn array(&self) -> &TeleologicalArray {
        &self.teleological_array
    }

    /// Check if this is a North Star goal.
    #[inline]
    pub fn is_north_star(&self) -> bool {
        self.level == GoalLevel::NorthStar
    }

    /// Check if this goal has the given ancestor.
    ///
    /// Note: This only checks the immediate parent. For full ancestry check,
    /// use `GoalHierarchy::path_to_north_star()`.
    #[inline]
    pub fn has_parent(&self, ancestor_id: Uuid) -> bool {
        self.parent_id == Some(ancestor_id)
    }

    /// Add a child goal ID.
    pub fn add_child(&mut self, child_id: Uuid) {
        if !self.child_ids.contains(&child_id) {
            self.child_ids.push(child_id);
        }
    }

    /// Remove a child goal ID.
    pub fn remove_child(&mut self, child_id: Uuid) {
        self.child_ids.retain(|id| *id != child_id);
    }
}

// NOTE: The following constructors are REMOVED per ARCH-03 (autonomous-first):
// - north_star() - Manual goal creation forbidden
// - child() - Use autonomous_goal() or child_goal() instead

/// Goal hierarchy tree structure.
///
/// Manages a tree of goals with a single North Star at the root.
/// Used for hierarchical alignment propagation in purpose vector computation.
///
/// # Invariants
///
/// - At most one North Star goal
/// - All child goals have valid parent references
/// - No cycles in the hierarchy
#[derive(Clone, Debug, Default)]
pub struct GoalHierarchy {
    nodes: HashMap<Uuid, GoalNode>,
    north_star: Option<Uuid>,
}

impl GoalHierarchy {
    /// Create a new empty goal hierarchy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a goal to the hierarchy.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Adding a second North Star goal
    /// - Child goal's parent doesn't exist
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_core::purpose::{GoalHierarchy, GoalNode, GoalLevel, GoalDiscoveryMetadata};
    /// use context_graph_core::types::fingerprint::SemanticFingerprint;
    ///
    /// let mut hierarchy = GoalHierarchy::new();
    ///
    /// let discovery = GoalDiscoveryMetadata::bootstrap();
    /// let fp = SemanticFingerprint::zeroed();
    /// let ns = GoalNode::autonomous_goal(
    ///     "North Star".into(),
    ///     GoalLevel::NorthStar,
    ///     fp,
    ///     discovery,
    /// ).unwrap();
    /// hierarchy.add_goal(ns).unwrap();
    /// ```
    pub fn add_goal(&mut self, goal: GoalNode) -> Result<(), GoalHierarchyError> {
        // Validate parent exists (except for NorthStar)
        if let Some(ref parent_id) = goal.parent_id {
            if !self.nodes.contains_key(parent_id) {
                return Err(GoalHierarchyError::ParentNotFound(*parent_id));
            }
        }

        // Only one North Star allowed
        if goal.level == GoalLevel::NorthStar {
            if self.north_star.is_some() {
                return Err(GoalHierarchyError::MultipleNorthStars);
            }
            self.north_star = Some(goal.id);
        }

        // Update parent's child list
        if let Some(parent_id) = goal.parent_id {
            if let Some(parent) = self.nodes.get_mut(&parent_id) {
                parent.add_child(goal.id);
            }
        }

        self.nodes.insert(goal.id, goal);
        Ok(())
    }

    /// Get the North Star goal.
    ///
    /// Returns None if no North Star has been added.
    pub fn north_star(&self) -> Option<&GoalNode> {
        self.north_star.and_then(|id| self.nodes.get(&id))
    }

    /// Check if a North Star goal exists.
    #[inline]
    pub fn has_north_star(&self) -> bool {
        self.north_star.is_some()
    }

    /// Get a goal by ID.
    pub fn get(&self, id: &Uuid) -> Option<&GoalNode> {
        self.nodes.get(id)
    }

    /// Get direct children of a goal.
    pub fn children(&self, parent_id: &Uuid) -> Vec<&GoalNode> {
        self.get(parent_id)
            .map(|parent| {
                parent
                    .child_ids
                    .iter()
                    .filter_map(|id| self.nodes.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all goals at a specific level.
    pub fn at_level(&self, level: GoalLevel) -> Vec<&GoalNode> {
        self.nodes.values().filter(|n| n.level == level).collect()
    }

    /// Total number of goals in the hierarchy.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the hierarchy is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Iterate over all goals.
    pub fn iter(&self) -> impl Iterator<Item = &GoalNode> {
        self.nodes.values()
    }

    /// Get path from a goal to the North Star.
    ///
    /// Returns the sequence of goal IDs from the given goal up to (and including)
    /// the North Star. Returns empty vec if goal not found.
    pub fn path_to_north_star(&self, goal_id: &Uuid) -> Vec<Uuid> {
        let mut path = Vec::new();
        let mut current = self.nodes.get(goal_id);

        while let Some(node) = current {
            path.push(node.id);
            current = node.parent_id.and_then(|pid| self.nodes.get(&pid));
        }

        path
    }

    /// Validate hierarchy integrity.
    ///
    /// Checks:
    /// - North Star exists if hierarchy is not empty
    /// - All parent references are valid
    pub fn validate(&self) -> Result<(), GoalHierarchyError> {
        if self.north_star.is_none() && !self.nodes.is_empty() {
            return Err(GoalHierarchyError::NoNorthStar);
        }

        // Check all parents exist
        for node in self.nodes.values() {
            if let Some(ref parent_id) = node.parent_id {
                if !self.nodes.contains_key(parent_id) {
                    return Err(GoalHierarchyError::ParentNotFound(*parent_id));
                }
            }
        }

        Ok(())
    }
}

/// Errors for goal hierarchy operations.
#[derive(Debug, Error)]
pub enum GoalHierarchyError {
    /// No North Star goal has been defined.
    #[error("No North Star goal defined")]
    NoNorthStar,

    /// Attempted to add a second North Star goal.
    #[error("Multiple North Star goals not allowed")]
    MultipleNorthStars,

    /// Referenced parent goal does not exist.
    #[error("Parent goal not found: {0}")]
    ParentNotFound(Uuid),

    /// Referenced goal does not exist.
    #[error("Goal not found: {0}")]
    GoalNotFound(Uuid),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::fingerprint::SemanticFingerprint;

    // Helper function to create a valid zeroed fingerprint for testing
    fn test_fingerprint() -> SemanticFingerprint {
        SemanticFingerprint::zeroed()
    }

    // Helper function to create bootstrap discovery metadata
    fn test_discovery() -> GoalDiscoveryMetadata {
        GoalDiscoveryMetadata::bootstrap()
    }

    // Helper function to create clustering discovery metadata
    fn clustering_discovery(confidence: f32, cluster_size: usize, coherence: f32) -> Result<GoalDiscoveryMetadata, GoalNodeError> {
        GoalDiscoveryMetadata::new(DiscoveryMethod::Clustering, confidence, cluster_size, coherence)
    }

    #[test]
    fn test_goal_level_propagation_weights() {
        assert_eq!(GoalLevel::NorthStar.propagation_weight(), 1.0);
        assert_eq!(GoalLevel::Strategic.propagation_weight(), 0.7);
        assert_eq!(GoalLevel::Tactical.propagation_weight(), 0.4);
        assert_eq!(GoalLevel::Immediate.propagation_weight(), 0.2);
        println!("[VERIFIED] GoalLevel propagation weights match constitution.yaml");
    }

    #[test]
    fn test_goal_level_depth() {
        assert_eq!(GoalLevel::NorthStar.depth(), 0);
        assert_eq!(GoalLevel::Strategic.depth(), 1);
        assert_eq!(GoalLevel::Tactical.depth(), 2);
        assert_eq!(GoalLevel::Immediate.depth(), 3);
        println!("[VERIFIED] GoalLevel depth values are correct");
    }

    #[test]
    fn test_discovery_metadata_valid() {
        let discovery = clustering_discovery(0.85, 42, 0.78).unwrap();
        assert_eq!(discovery.method, DiscoveryMethod::Clustering);
        assert_eq!(discovery.confidence, 0.85);
        assert_eq!(discovery.cluster_size, 42);
        assert_eq!(discovery.coherence, 0.78);
        println!("[VERIFIED] GoalDiscoveryMetadata::new creates valid metadata");
    }

    #[test]
    fn test_discovery_metadata_bootstrap() {
        let discovery = GoalDiscoveryMetadata::bootstrap();
        assert_eq!(discovery.method, DiscoveryMethod::Bootstrap);
        assert_eq!(discovery.confidence, 0.0);
        assert_eq!(discovery.cluster_size, 0);
        assert_eq!(discovery.coherence, 0.0);
        println!("[VERIFIED] GoalDiscoveryMetadata::bootstrap creates correct defaults");
    }

    #[test]
    fn test_discovery_metadata_invalid_confidence() {
        let result = clustering_discovery(1.5, 10, 0.8);
        assert!(matches!(result, Err(GoalNodeError::InvalidConfidence(1.5))));

        let result = clustering_discovery(-0.1, 10, 0.8);
        assert!(matches!(result, Err(GoalNodeError::InvalidConfidence(_))));
        println!("[VERIFIED] GoalDiscoveryMetadata rejects invalid confidence");
    }

    #[test]
    fn test_discovery_metadata_invalid_coherence() {
        let result = clustering_discovery(0.8, 10, 1.5);
        assert!(matches!(result, Err(GoalNodeError::InvalidCoherence(1.5))));

        let result = clustering_discovery(0.8, 10, -0.1);
        assert!(matches!(result, Err(GoalNodeError::InvalidCoherence(_))));
        println!("[VERIFIED] GoalDiscoveryMetadata rejects invalid coherence");
    }

    #[test]
    fn test_discovery_metadata_empty_cluster() {
        let result = clustering_discovery(0.8, 0, 0.7);
        assert!(matches!(result, Err(GoalNodeError::EmptyCluster)));
        println!("[VERIFIED] GoalDiscoveryMetadata rejects empty cluster for non-Bootstrap");
    }

    #[test]
    fn test_goal_node_autonomous_creation() {
        let fp = test_fingerprint();
        let discovery = test_discovery();

        let goal = GoalNode::autonomous_goal(
            "Test North Star".into(),
            GoalLevel::NorthStar,
            fp,
            discovery,
        )
        .expect("Should create goal");

        assert_eq!(goal.level, GoalLevel::NorthStar);
        assert!(goal.parent_id.is_none());
        assert!(goal.child_ids.is_empty());
        assert!(goal.is_north_star());
        println!("[VERIFIED] GoalNode::autonomous_goal creates correct structure");
    }

    #[test]
    fn test_goal_node_child_creation() {
        let fp = test_fingerprint();
        let discovery = test_discovery();
        let parent_id = Uuid::new_v4();

        let child = GoalNode::child_goal(
            "Test Strategic Goal".into(),
            GoalLevel::Strategic,
            parent_id,
            fp,
            discovery,
        )
        .expect("Should create child goal");

        assert_eq!(child.level, GoalLevel::Strategic);
        assert_eq!(child.parent_id, Some(parent_id));
        assert!(!child.is_north_star());
        println!("[VERIFIED] GoalNode::child_goal creates correct structure");
    }

    #[test]
    #[should_panic(expected = "Child goal cannot be NorthStar")]
    fn test_child_goal_cannot_be_north_star() {
        let fp = test_fingerprint();
        let discovery = test_discovery();
        let parent_id = Uuid::new_v4();

        let _ = GoalNode::child_goal(
            "Bad goal".into(),
            GoalLevel::NorthStar, // Should panic
            parent_id,
            fp,
            discovery,
        );
    }

    #[test]
    fn test_goal_node_invalid_fingerprint() {
        let mut fp = test_fingerprint();
        fp.e1_semantic = vec![]; // Invalid - empty

        let discovery = test_discovery();
        let result = GoalNode::autonomous_goal(
            "Test".into(),
            GoalLevel::NorthStar,
            fp,
            discovery,
        );

        assert!(result.is_err());
        assert!(matches!(result, Err(GoalNodeError::InvalidArray(_))));
        println!("[VERIFIED] GoalNode rejects invalid teleological array");
    }

    #[test]
    fn test_goal_node_array_access() {
        let fp = test_fingerprint();
        let discovery = test_discovery();

        let goal = GoalNode::autonomous_goal(
            "Test".into(),
            GoalLevel::NorthStar,
            fp,
            discovery,
        )
        .unwrap();

        let array = goal.array();
        assert_eq!(array.e1_semantic.len(), 1024);
        println!("[VERIFIED] GoalNode::array() provides access to teleological array");
    }

    #[test]
    fn test_goal_node_child_management() {
        let fp = test_fingerprint();
        let discovery = test_discovery();

        let mut goal = GoalNode::autonomous_goal(
            "Test".into(),
            GoalLevel::NorthStar,
            fp,
            discovery,
        )
        .unwrap();

        let child_id = Uuid::new_v4();
        goal.add_child(child_id);
        assert!(goal.child_ids.contains(&child_id));
        assert_eq!(goal.child_ids.len(), 1);

        // Adding same child again should not duplicate
        goal.add_child(child_id);
        assert_eq!(goal.child_ids.len(), 1);

        goal.remove_child(child_id);
        assert!(!goal.child_ids.contains(&child_id));
        println!("[VERIFIED] GoalNode child management works correctly");
    }

    #[test]
    fn test_goal_hierarchy_single_north_star() {
        let mut hierarchy = GoalHierarchy::new();

        let ns1 = GoalNode::autonomous_goal(
            "NS1".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();

        let ns2 = GoalNode::autonomous_goal(
            "NS2".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();

        assert!(hierarchy.add_goal(ns1).is_ok());
        let result = hierarchy.add_goal(ns2);
        assert!(matches!(result, Err(GoalHierarchyError::MultipleNorthStars)));
        println!("[VERIFIED] GoalHierarchy enforces single North Star");
    }

    #[test]
    fn test_goal_hierarchy_parent_validation() {
        let mut hierarchy = GoalHierarchy::new();

        let fake_parent = Uuid::new_v4();
        let child = GoalNode::child_goal(
            "Orphan".into(),
            GoalLevel::Strategic,
            fake_parent,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();

        let result = hierarchy.add_goal(child);
        assert!(matches!(result, Err(GoalHierarchyError::ParentNotFound(_))));
        println!("[VERIFIED] GoalHierarchy validates parent existence");
    }

    #[test]
    fn test_goal_hierarchy_full_tree() {
        let mut hierarchy = GoalHierarchy::new();

        // Add North Star
        let ns = GoalNode::autonomous_goal(
            "Master ML".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            clustering_discovery(0.9, 100, 0.85).unwrap(),
        )
        .unwrap();
        let ns_id = ns.id;
        hierarchy.add_goal(ns).unwrap();

        // Add Strategic child
        let strategic = GoalNode::child_goal(
            "Learn PyTorch".into(),
            GoalLevel::Strategic,
            ns_id,
            test_fingerprint(),
            clustering_discovery(0.8, 50, 0.75).unwrap(),
        )
        .unwrap();
        let strategic_id = strategic.id;
        hierarchy.add_goal(strategic).unwrap();

        // Add Tactical child
        let tactical = GoalNode::child_goal(
            "Complete tutorial".into(),
            GoalLevel::Tactical,
            strategic_id,
            test_fingerprint(),
            clustering_discovery(0.7, 20, 0.65).unwrap(),
        )
        .unwrap();
        let tactical_id = tactical.id;
        hierarchy.add_goal(tactical).unwrap();

        assert_eq!(hierarchy.len(), 3);
        assert!(!hierarchy.is_empty());
        assert!(hierarchy.has_north_star());
        assert!(hierarchy.north_star().is_some());
        assert_eq!(hierarchy.at_level(GoalLevel::Strategic).len(), 1);
        assert_eq!(hierarchy.at_level(GoalLevel::Tactical).len(), 1);
        assert_eq!(hierarchy.children(&ns_id).len(), 1);
        assert!(hierarchy.validate().is_ok());

        // Verify path to north star
        let path = hierarchy.path_to_north_star(&tactical_id);
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], tactical_id);
        assert_eq!(path[1], strategic_id);
        assert_eq!(path[2], ns_id);

        println!("[VERIFIED] GoalHierarchy full tree structure works correctly");
    }

    #[test]
    fn test_goal_hierarchy_validate_no_north_star() {
        let mut hierarchy = GoalHierarchy::new();

        // Manually insert a node without North Star (bypass add_goal validation)
        let goal = GoalNode {
            id: Uuid::new_v4(),
            description: "Orphan".into(),
            level: GoalLevel::Strategic,
            teleological_array: test_fingerprint(),
            parent_id: None,
            child_ids: vec![],
            discovery: test_discovery(),
            created_at: Utc::now(),
        };
        hierarchy.nodes.insert(goal.id, goal);

        let result = hierarchy.validate();
        assert!(matches!(result, Err(GoalHierarchyError::NoNorthStar)));
        println!("[VERIFIED] validate detects missing North Star");
    }

    #[test]
    fn test_goal_hierarchy_iter() {
        let mut hierarchy = GoalHierarchy::new();

        let ns = GoalNode::autonomous_goal(
            "NS".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
        let ns_id = ns.id;
        hierarchy.add_goal(ns).unwrap();

        let child = GoalNode::child_goal(
            "C1".into(),
            GoalLevel::Strategic,
            ns_id,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
        hierarchy.add_goal(child).unwrap();

        let count = hierarchy.iter().count();
        assert_eq!(count, 2);
        println!("[VERIFIED] GoalHierarchy iter works correctly");
    }

    #[test]
    fn test_goal_serialization_roundtrip() {
        let fp = test_fingerprint();
        let discovery = clustering_discovery(0.85, 42, 0.78).unwrap();

        let goal = GoalNode::autonomous_goal(
            "Test goal".into(),
            GoalLevel::Strategic,
            fp,
            discovery,
        )
        .unwrap();

        // Serialize
        let json = serde_json::to_string(&goal).expect("Serialize");

        // Deserialize
        let restored: GoalNode = serde_json::from_str(&json).expect("Deserialize");

        // Verify
        assert_eq!(goal.id, restored.id);
        assert_eq!(goal.level, restored.level);
        assert_eq!(goal.description, restored.description);
        assert_eq!(
            goal.teleological_array.e1_semantic.len(),
            restored.teleological_array.e1_semantic.len()
        );
        println!("[VERIFIED] GoalNode survives JSON serialization roundtrip");
    }

    #[test]
    fn test_discovery_method_serialization() {
        let methods = vec![
            DiscoveryMethod::Clustering,
            DiscoveryMethod::PatternRecognition,
            DiscoveryMethod::Decomposition,
            DiscoveryMethod::Bootstrap,
        ];

        for method in methods {
            let json = serde_json::to_string(&method).expect("Serialize");
            let restored: DiscoveryMethod = serde_json::from_str(&json).expect("Deserialize");
            assert_eq!(method, restored);
        }
        println!("[VERIFIED] DiscoveryMethod serialization works correctly");
    }

    // Edge Case Tests from Task Spec

    #[test]
    fn test_edge_case_incomplete_fingerprint_rejected() {
        let mut fp = test_fingerprint();
        fp.e1_semantic = vec![]; // Invalid - empty

        let discovery = test_discovery();
        let result = GoalNode::autonomous_goal(
            "Test".into(),
            GoalLevel::NorthStar,
            fp,
            discovery,
        );

        assert!(result.is_err());
        match result {
            Err(GoalNodeError::InvalidArray(ValidationError::DimensionMismatch { .. })) => {
                println!("[EDGE CASE 1 PASSED] Incomplete fingerprint rejected");
            }
            _ => panic!("Wrong error type: {:?}", result),
        }
    }

    #[test]
    fn test_edge_case_invalid_confidence_rejected() {
        let result = GoalDiscoveryMetadata::new(
            DiscoveryMethod::Clustering,
            1.5, // Invalid
            10,
            0.8,
        );

        assert!(matches!(result, Err(GoalNodeError::InvalidConfidence(1.5))));
        println!("[EDGE CASE 2 PASSED] Invalid confidence rejected");
    }

    #[test]
    fn test_edge_case_multiple_north_stars_rejected() {
        let mut hierarchy = GoalHierarchy::new();
        let fp = test_fingerprint();
        let discovery = test_discovery();

        let ns1 = GoalNode::autonomous_goal(
            "NS1".into(),
            GoalLevel::NorthStar,
            fp.clone(),
            discovery.clone(),
        )
        .unwrap();

        let ns2 = GoalNode::autonomous_goal(
            "NS2".into(),
            GoalLevel::NorthStar,
            fp,
            discovery,
        )
        .unwrap();

        hierarchy.add_goal(ns1).unwrap();
        let result = hierarchy.add_goal(ns2);

        assert!(matches!(result, Err(GoalHierarchyError::MultipleNorthStars)));
        println!("[EDGE CASE 3 PASSED] Multiple North Stars rejected");
    }

    #[test]
    fn test_edge_case_orphan_child_rejected() {
        let mut hierarchy = GoalHierarchy::new();
        let fp = test_fingerprint();
        let discovery = test_discovery();

        let fake_parent = Uuid::new_v4();
        let child = GoalNode::child_goal(
            "Orphan".into(),
            GoalLevel::Strategic,
            fake_parent,
            fp,
            discovery,
        )
        .unwrap();

        let result = hierarchy.add_goal(child);
        assert!(matches!(result, Err(GoalHierarchyError::ParentNotFound(_))));
        println!("[EDGE CASE 4 PASSED] Orphan child rejected");
    }

    #[test]
    fn test_edge_case_empty_cluster_rejected() {
        let result = GoalDiscoveryMetadata::new(
            DiscoveryMethod::Clustering,
            0.8,
            0, // Invalid for Clustering
            0.7,
        );

        assert!(matches!(result, Err(GoalNodeError::EmptyCluster)));
        println!("[EDGE CASE 5 PASSED] Empty cluster rejected");
    }
}
