//! Amortized Learning - Shortcut Creation for Frequently Traversed Paths
//!
//! Implements amortized shortcut creation during dream cycles for paths
//! that are traversed frequently (5+ times) and span 3+ hops.
//!
//! ## Constitution Reference (Section dream.amortized, line 452)
//!
//! - Minimum hops: 3
//! - Minimum traversals: 5
//! - Confidence threshold: 0.7
//!
//! ## Shortcut Creation Algorithm
//!
//! 1. **Path Tracking**: Monitor traversal counts during NREM replay
//! 2. **Candidate Detection**: Identify paths with 3+ hops and 5+ traversals
//! 3. **Quality Gate**: Require minimum confidence of 0.7
//! 4. **Shortcut Edge**: Create direct source->target edge with combined weight
//!
//! The shortcut edge stores:
//! - `is_shortcut: true`
//! - `original_path`: Reference to the original multi-hop path
//! - `weight`: Product of path edge weights
//! - `confidence`: Minimum confidence along the path
//!
//! ## Edge Creation
//!
//! The [`EdgeCreator`] trait abstracts edge persistence. Implementations can:
//! - Persist edges to a graph store
//! - Log edges for debugging (via [`NullEdgeCreator`])
//! - Record edges for testing (via [`RecordingEdgeCreator`])
//!
//! When no edge creator is set, shortcuts are tracked internally but not persisted.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use uuid::Uuid;

use super::constants;
use crate::error::CoreResult;

/// Signature for identifying unique paths (hash of ordered node IDs)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathSignature(u64);

impl PathSignature {
    /// Create a signature from a path of node IDs
    pub fn from_path(nodes: &[Uuid]) -> Self {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        for node in nodes {
            node.hash(&mut hasher);
        }
        PathSignature(hasher.finish())
    }
}

/// Edge to be created as a shortcut
///
/// Represents a direct edge that replaces a multi-hop path that was
/// traversed frequently enough to warrant optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutEdge {
    /// Source node ID
    pub source: Uuid,
    /// Target node ID
    pub target: Uuid,
    /// Combined weight (product of path edge weights)
    pub weight: f32,
    /// Minimum confidence along the original path
    pub confidence: f32,
    /// Flag indicating this is a shortcut edge (always true)
    pub is_shortcut: bool,
    /// Original multi-hop path that this shortcut replaces
    pub original_path: Vec<Uuid>,
}

impl ShortcutEdge {
    /// Create a ShortcutEdge from a ShortcutCandidate
    ///
    /// The `is_shortcut` flag is always set to `true` to distinguish
    /// shortcut edges from regular edges in the graph store.
    pub fn from_candidate(candidate: &ShortcutCandidate) -> Self {
        Self {
            source: candidate.source,
            target: candidate.target,
            weight: candidate.combined_weight,
            confidence: candidate.min_confidence,
            is_shortcut: true, // Always true for shortcuts
            original_path: candidate.path_nodes.clone(),
        }
    }
}

/// Trait for creating shortcut edges in storage
///
/// Implementations of this trait handle the persistence of shortcut edges
/// to a graph store or other storage mechanism.
///
/// # Contract
///
/// - `create_edge` is called only for candidates that pass the quality gate
/// - The edge's `is_shortcut` flag is always `true`
/// - The `original_path` contains the full multi-hop path
///
/// # Error Handling
///
/// Implementations must return errors (not panic) on failure.
/// Returning `Ok(false)` indicates the edge already exists.
pub trait EdgeCreator: Send + Sync {
    /// Create a shortcut edge in the graph store
    ///
    /// # Arguments
    ///
    /// * `edge` - The shortcut edge to create
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Edge was successfully created
    /// * `Ok(false)` - Edge already exists (no-op)
    /// * `Err(_)` - Error during creation
    fn create_edge(&self, edge: &ShortcutEdge) -> CoreResult<bool>;
}

/// Null implementation of EdgeCreator for backward compatibility
///
/// This implementation logs edge creation attempts but does not persist
/// them. Useful for testing and when no graph store is available.
pub struct NullEdgeCreator;

impl EdgeCreator for NullEdgeCreator {
    fn create_edge(&self, edge: &ShortcutEdge) -> CoreResult<bool> {
        debug!(
            "NullEdgeCreator: would create edge {} -> {} (is_shortcut={}, path_len={})",
            edge.source,
            edge.target,
            edge.is_shortcut,
            edge.original_path.len()
        );
        Ok(true) // Pretend success for backward compatibility
    }
}

/// A candidate path for shortcut creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutCandidate {
    /// Source node (start of path)
    pub source: Uuid,

    /// Target node (end of path)
    pub target: Uuid,

    /// Number of hops in the original path
    pub hop_count: usize,

    /// Number of times this path was traversed
    pub traversal_count: usize,

    /// Combined weight (product of edge weights)
    pub combined_weight: f32,

    /// Minimum confidence along the path
    pub min_confidence: f32,

    /// Full path nodes for reference
    pub path_nodes: Vec<Uuid>,
}

impl ShortcutCandidate {
    /// Check if this candidate meets the quality gate requirements
    ///
    /// NOTE: Uses legacy constants. Will be migrated to DreamThresholds in consumer update task.
    #[allow(deprecated)]
    pub fn meets_quality_gate(&self) -> bool {
        self.hop_count >= constants::MIN_SHORTCUT_HOPS
            && self.traversal_count >= constants::MIN_SHORTCUT_TRAVERSALS
            && self.min_confidence >= constants::SHORTCUT_CONFIDENCE_THRESHOLD
    }
}

/// Amortized learning system for shortcut creation
///
/// The learner tracks path traversals and creates shortcut edges when
/// paths meet the quality gate requirements (3+ hops, 5+ traversals, 0.7+ confidence).
///
/// # Edge Creation
///
/// When an `EdgeCreator` is set via [`set_edge_creator`](Self::set_edge_creator),
/// shortcut edges are persisted to the graph store. Without a creator,
/// shortcuts are tracked internally but not persisted.
pub struct AmortizedLearner {
    /// Path traversal counts
    path_counts: HashMap<PathSignature, PathInfo>,

    /// Minimum hops required for shortcut (Constitution: 3)
    min_hops: usize,

    /// Minimum traversals required (Constitution: 5)
    min_traversals: usize,

    /// Confidence threshold (Constitution: 0.7)
    confidence_threshold: f32,

    /// Shortcuts created in current cycle
    shortcuts_created_this_cycle: usize,

    /// Total shortcuts created
    total_shortcuts_created: usize,

    /// Optional edge creator for persisting shortcut edges
    edge_creator: Option<Arc<dyn EdgeCreator>>,
}

// Manual Debug impl because Arc<dyn EdgeCreator> doesn't implement Debug
impl std::fmt::Debug for AmortizedLearner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmortizedLearner")
            .field("path_counts", &self.path_counts)
            .field("min_hops", &self.min_hops)
            .field("min_traversals", &self.min_traversals)
            .field("confidence_threshold", &self.confidence_threshold)
            .field("shortcuts_created_this_cycle", &self.shortcuts_created_this_cycle)
            .field("total_shortcuts_created", &self.total_shortcuts_created)
            .field("edge_creator", &self.edge_creator.is_some())
            .finish()
    }
}

/// Internal tracking info for a path
#[derive(Debug, Clone)]
struct PathInfo {
    /// Path nodes
    nodes: Vec<Uuid>,
    /// Traversal count
    count: usize,
    /// Combined weight
    weight: f32,
    /// Minimum confidence
    min_confidence: f32,
}

impl AmortizedLearner {
    /// Create a new AmortizedLearner with constitution-mandated defaults
    ///
    /// NOTE: Uses legacy constants. Will be migrated to DreamThresholds in consumer update task.
    #[allow(deprecated)]
    pub fn new() -> Self {
        Self {
            path_counts: HashMap::new(),
            min_hops: constants::MIN_SHORTCUT_HOPS,
            min_traversals: constants::MIN_SHORTCUT_TRAVERSALS,
            confidence_threshold: constants::SHORTCUT_CONFIDENCE_THRESHOLD,
            shortcuts_created_this_cycle: 0,
            total_shortcuts_created: 0,
            edge_creator: None,
        }
    }

    /// Set the edge creator for shortcut persistence
    ///
    /// When set, the learner will call the creator to persist shortcut edges
    /// to the graph store when candidates pass the quality gate.
    ///
    /// # Arguments
    ///
    /// * `creator` - The edge creator implementation
    pub fn set_edge_creator(&mut self, creator: Arc<dyn EdgeCreator>) {
        self.edge_creator = Some(creator);
    }

    /// Create a new AmortizedLearner with an edge creator
    ///
    /// Convenience constructor for creating a learner with edge persistence.
    #[allow(deprecated)]
    pub fn with_edge_creator(creator: Arc<dyn EdgeCreator>) -> Self {
        Self {
            path_counts: HashMap::new(),
            min_hops: constants::MIN_SHORTCUT_HOPS,
            min_traversals: constants::MIN_SHORTCUT_TRAVERSALS,
            confidence_threshold: constants::SHORTCUT_CONFIDENCE_THRESHOLD,
            shortcuts_created_this_cycle: 0,
            total_shortcuts_created: 0,
            edge_creator: Some(creator),
        }
    }

    /// Record a path traversal during NREM replay
    ///
    /// # Arguments
    ///
    /// * `nodes` - The nodes in the traversed path
    /// * `edge_weights` - Weights of edges along the path
    /// * `edge_confidences` - Confidences of edges along the path
    pub fn record_traversal(
        &mut self,
        nodes: &[Uuid],
        edge_weights: &[f32],
        edge_confidences: &[f32],
    ) {
        if nodes.len() < 2 {
            return; // Need at least 2 nodes for a path
        }

        let signature = PathSignature::from_path(nodes);
        let combined_weight: f32 = edge_weights.iter().product();
        let min_confidence = edge_confidences
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min);

        let entry = self
            .path_counts
            .entry(signature)
            .or_insert_with(|| PathInfo {
                nodes: nodes.to_vec(),
                count: 0,
                weight: combined_weight,
                min_confidence,
            });

        entry.count += 1;
        // Update weight and confidence (take the latest values)
        entry.weight = combined_weight;
        entry.min_confidence = entry.min_confidence.min(min_confidence);

        debug!(
            "Path traversal recorded: {} nodes, count={}",
            nodes.len(),
            entry.count
        );
    }

    /// Get candidates that meet the shortcut creation criteria
    ///
    /// Returns paths with:
    /// - 3+ hops
    /// - 5+ traversals
    /// - Confidence >= 0.7
    pub fn get_candidates(&self) -> Vec<ShortcutCandidate> {
        self.path_counts
            .values()
            .filter_map(|info| {
                let hop_count = info.nodes.len() - 1;

                // Check minimum hops
                if hop_count < self.min_hops {
                    return None;
                }

                // Check minimum traversals
                if info.count < self.min_traversals {
                    return None;
                }

                // Check confidence threshold
                if info.min_confidence < self.confidence_threshold {
                    return None;
                }

                Some(ShortcutCandidate {
                    source: *info.nodes.first()?,
                    target: *info.nodes.last()?,
                    hop_count,
                    traversal_count: info.count,
                    combined_weight: info.weight,
                    min_confidence: info.min_confidence,
                    path_nodes: info.nodes.clone(),
                })
            })
            .collect()
    }

    /// Create a shortcut for a candidate
    ///
    /// Creates a [`ShortcutEdge`] from the candidate and persists it via the
    /// configured [`EdgeCreator`] (if set).
    ///
    /// # Arguments
    ///
    /// * `candidate` - The shortcut candidate to create
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Shortcut was created successfully
    /// * `Ok(false)` - Candidate failed quality gate or edge already exists
    /// * `Err(_)` - Error during edge creation
    ///
    /// # Quality Gate
    ///
    /// The candidate must meet these requirements (Constitution DREAM-005):
    /// - hops >= 3
    /// - traversals >= 5
    /// - confidence >= 0.7
    pub fn create_shortcut(&mut self, candidate: &ShortcutCandidate) -> CoreResult<bool> {
        if !candidate.meets_quality_gate() {
            debug!(
                "Candidate failed quality gate: hops={}, traversals={}, confidence={}",
                candidate.hop_count, candidate.traversal_count, candidate.min_confidence
            );
            return Ok(false);
        }

        info!(
            "Creating shortcut: {} -> {} (hops={}, traversals={}, confidence={})",
            candidate.source,
            candidate.target,
            candidate.hop_count,
            candidate.traversal_count,
            candidate.min_confidence
        );

        // Create ShortcutEdge from candidate
        let edge = ShortcutEdge::from_candidate(candidate);

        // Call edge creator if set
        if let Some(creator) = &self.edge_creator {
            let created = creator.create_edge(&edge)?;
            if !created {
                debug!(
                    "Edge creator returned false for {} -> {} (edge may already exist)",
                    edge.source, edge.target
                );
                return Ok(false);
            }
        } else {
            debug!(
                "No edge creator set, shortcut {} -> {} tracked but not persisted",
                edge.source, edge.target
            );
        }

        self.shortcuts_created_this_cycle += 1;
        self.total_shortcuts_created += 1;

        Ok(true)
    }

    /// Process all candidates and create shortcuts
    ///
    /// Returns the number of shortcuts created
    pub fn process_candidates(&mut self) -> CoreResult<usize> {
        let candidates = self.get_candidates();
        let mut created = 0;

        let candidate_count = candidates.len();
        for candidate in candidates {
            if self.create_shortcut(&candidate)? {
                created += 1;
            }
        }

        info!(
            "Processed {} candidates, created {} shortcuts",
            candidate_count, created
        );

        Ok(created)
    }

    /// Get the number of shortcuts created this cycle
    pub fn shortcuts_created_this_cycle(&self) -> usize {
        self.shortcuts_created_this_cycle
    }

    /// Get the total number of shortcuts created
    pub fn total_shortcuts_created(&self) -> usize {
        self.total_shortcuts_created
    }

    /// Reset the cycle counter (called at start of new dream cycle)
    pub fn reset_cycle_counter(&mut self) {
        self.shortcuts_created_this_cycle = 0;
    }

    /// Clear all path tracking data
    pub fn clear(&mut self) {
        self.path_counts.clear();
        self.shortcuts_created_this_cycle = 0;
    }

    /// Get the number of tracked paths
    pub fn tracked_paths(&self) -> usize {
        self.path_counts.len()
    }

    /// Get configuration values
    pub fn min_hops(&self) -> usize {
        self.min_hops
    }

    pub fn min_traversals(&self) -> usize {
        self.min_traversals
    }

    pub fn confidence_threshold(&self) -> f32 {
        self.confidence_threshold
    }
}

impl Default for AmortizedLearner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amortized_learner_creation() {
        let learner = AmortizedLearner::new();

        assert_eq!(learner.min_hops, 3);
        assert_eq!(learner.min_traversals, 5);
        assert_eq!(learner.confidence_threshold, 0.7);
        assert_eq!(learner.shortcuts_created_this_cycle, 0);
    }

    #[test]
    #[allow(deprecated)]
    fn test_constitution_compliance() {
        let learner = AmortizedLearner::new();

        // Constitution mandates: 3 hops, 5 traversals, 0.7 confidence
        assert_eq!(learner.min_hops, constants::MIN_SHORTCUT_HOPS);
        assert_eq!(learner.min_traversals, constants::MIN_SHORTCUT_TRAVERSALS);
        assert_eq!(
            learner.confidence_threshold,
            constants::SHORTCUT_CONFIDENCE_THRESHOLD
        );
    }

    #[test]
    fn test_path_signature() {
        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();
        let node3 = Uuid::new_v4();

        let sig1 = PathSignature::from_path(&[node1, node2, node3]);
        let sig2 = PathSignature::from_path(&[node1, node2, node3]);
        let sig3 = PathSignature::from_path(&[node3, node2, node1]);

        // Same path should give same signature
        assert_eq!(sig1, sig2);

        // Different order should give different signature
        assert_ne!(sig1, sig3);
    }

    #[test]
    fn test_record_traversal() {
        let mut learner = AmortizedLearner::new();

        let nodes = vec![
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        let weights = vec![0.8, 0.9, 0.7];
        let confidences = vec![0.9, 0.8, 0.75];

        learner.record_traversal(&nodes, &weights, &confidences);

        assert_eq!(learner.tracked_paths(), 1);
    }

    #[test]
    fn test_get_candidates_insufficient_hops() {
        let mut learner = AmortizedLearner::new();

        // Only 2 hops (needs 3+)
        let nodes = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let weights = vec![0.8, 0.9];
        let confidences = vec![0.9, 0.8];

        // Record 5 times
        for _ in 0..5 {
            learner.record_traversal(&nodes, &weights, &confidences);
        }

        let candidates = learner.get_candidates();
        assert!(
            candidates.is_empty(),
            "2-hop paths should not be candidates"
        );
    }

    #[test]
    fn test_get_candidates_insufficient_traversals() {
        let mut learner = AmortizedLearner::new();

        // 3 hops
        let nodes = vec![
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        let weights = vec![0.8, 0.9, 0.7];
        let confidences = vec![0.9, 0.8, 0.75];

        // Only 3 traversals (needs 5+)
        for _ in 0..3 {
            learner.record_traversal(&nodes, &weights, &confidences);
        }

        let candidates = learner.get_candidates();
        assert!(
            candidates.is_empty(),
            "Paths with <5 traversals should not be candidates"
        );
    }

    #[test]
    fn test_get_candidates_insufficient_confidence() {
        let mut learner = AmortizedLearner::new();

        // 3 hops
        let nodes = vec![
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        let weights = vec![0.8, 0.9, 0.7];
        let confidences = vec![0.5, 0.6, 0.65]; // All below 0.7

        // 5 traversals
        for _ in 0..5 {
            learner.record_traversal(&nodes, &weights, &confidences);
        }

        let candidates = learner.get_candidates();
        assert!(
            candidates.is_empty(),
            "Paths with low confidence should not be candidates"
        );
    }

    #[test]
    fn test_get_candidates_success() {
        let mut learner = AmortizedLearner::new();

        // 4 hops (> 3)
        let nodes = vec![
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        let weights = vec![0.8, 0.9, 0.7, 0.85];
        let confidences = vec![0.9, 0.8, 0.75, 0.85]; // All >= 0.7

        // 6 traversals (> 5)
        for _ in 0..6 {
            learner.record_traversal(&nodes, &weights, &confidences);
        }

        let candidates = learner.get_candidates();
        assert_eq!(candidates.len(), 1, "Should have one candidate");

        let candidate = &candidates[0];
        assert_eq!(candidate.hop_count, 4);
        assert_eq!(candidate.traversal_count, 6);
        assert!(candidate.min_confidence >= 0.7);
        assert!(candidate.meets_quality_gate());
    }

    #[test]
    fn test_shortcut_candidate_quality_gate() {
        let good_candidate = ShortcutCandidate {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            hop_count: 4,
            traversal_count: 6,
            combined_weight: 0.5,
            min_confidence: 0.8,
            path_nodes: vec![],
        };
        assert!(good_candidate.meets_quality_gate());

        let bad_hops = ShortcutCandidate {
            hop_count: 2, // < 3
            ..good_candidate.clone()
        };
        assert!(!bad_hops.meets_quality_gate());

        let bad_traversals = ShortcutCandidate {
            traversal_count: 3, // < 5
            ..good_candidate.clone()
        };
        assert!(!bad_traversals.meets_quality_gate());

        let bad_confidence = ShortcutCandidate {
            min_confidence: 0.6, // < 0.7
            ..good_candidate.clone()
        };
        assert!(!bad_confidence.meets_quality_gate());
    }

    #[test]
    fn test_create_shortcut() {
        let mut learner = AmortizedLearner::new();

        let good_candidate = ShortcutCandidate {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            hop_count: 4,
            traversal_count: 6,
            combined_weight: 0.5,
            min_confidence: 0.8,
            path_nodes: vec![],
        };

        let result = learner.create_shortcut(&good_candidate).unwrap();
        assert!(result);
        assert_eq!(learner.shortcuts_created_this_cycle, 1);
        assert_eq!(learner.total_shortcuts_created, 1);
    }

    #[test]
    fn test_reset_cycle_counter() {
        let mut learner = AmortizedLearner::new();

        let candidate = ShortcutCandidate {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            hop_count: 4,
            traversal_count: 6,
            combined_weight: 0.5,
            min_confidence: 0.8,
            path_nodes: vec![],
        };

        learner.create_shortcut(&candidate).unwrap();
        assert_eq!(learner.shortcuts_created_this_cycle, 1);

        learner.reset_cycle_counter();
        assert_eq!(learner.shortcuts_created_this_cycle, 0);
        assert_eq!(learner.total_shortcuts_created, 1); // Total unchanged
    }

    // =====================================================================
    // EdgeCreator Tests
    // =====================================================================

    use std::sync::Mutex;

    /// Recording edge creator for testing - tracks all created edges
    pub struct RecordingEdgeCreator {
        created_edges: Mutex<Vec<ShortcutEdge>>,
    }

    impl RecordingEdgeCreator {
        pub fn new() -> Self {
            Self {
                created_edges: Mutex::new(Vec::new()),
            }
        }

        pub fn get_created_edges(&self) -> Vec<ShortcutEdge> {
            self.created_edges
                .lock()
                .expect("RecordingEdgeCreator lock poisoned")
                .clone()
        }
    }

    impl EdgeCreator for RecordingEdgeCreator {
        fn create_edge(&self, edge: &ShortcutEdge) -> CoreResult<bool> {
            self.created_edges
                .lock()
                .expect("RecordingEdgeCreator lock poisoned")
                .push(edge.clone());
            Ok(true)
        }
    }

    #[test]
    fn test_shortcut_edge_from_candidate() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let path_nodes = vec![source, Uuid::new_v4(), Uuid::new_v4(), target];

        let candidate = ShortcutCandidate {
            source,
            target,
            hop_count: 3,
            traversal_count: 5,
            combined_weight: 0.5,
            min_confidence: 0.8,
            path_nodes: path_nodes.clone(),
        };

        let edge = ShortcutEdge::from_candidate(&candidate);

        assert_eq!(edge.source, source);
        assert_eq!(edge.target, target);
        assert_eq!(edge.weight, 0.5);
        assert_eq!(edge.confidence, 0.8);
        assert!(edge.is_shortcut, "is_shortcut must be true");
        assert_eq!(edge.original_path, path_nodes);
    }

    #[test]
    fn test_shortcut_edge_is_shortcut_always_true() {
        let candidate = ShortcutCandidate {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            hop_count: 3,
            traversal_count: 5,
            combined_weight: 0.5,
            min_confidence: 0.8,
            path_nodes: vec![],
        };

        let edge = ShortcutEdge::from_candidate(&candidate);
        assert!(edge.is_shortcut, "ShortcutEdge.is_shortcut must always be true");
    }

    #[test]
    fn test_null_edge_creator() {
        let creator = NullEdgeCreator;
        let edge = ShortcutEdge {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            weight: 0.5,
            confidence: 0.8,
            is_shortcut: true,
            original_path: vec![],
        };

        let result = creator.create_edge(&edge).unwrap();
        assert!(result, "NullEdgeCreator should return true");
    }

    #[test]
    fn test_set_edge_creator() {
        let creator = Arc::new(RecordingEdgeCreator::new());
        let mut learner = AmortizedLearner::new();

        // Initially no creator
        assert!(learner.edge_creator.is_none());

        // Set creator
        learner.set_edge_creator(creator.clone());
        assert!(learner.edge_creator.is_some());
    }

    #[test]
    fn test_with_edge_creator_constructor() {
        let creator = Arc::new(RecordingEdgeCreator::new());
        let learner = AmortizedLearner::with_edge_creator(creator);

        assert!(learner.edge_creator.is_some());
        assert_eq!(learner.min_hops, 3);
        assert_eq!(learner.min_traversals, 5);
    }

    #[test]
    fn test_shortcut_creation_calls_creator() {
        let creator = Arc::new(RecordingEdgeCreator::new());
        let mut learner = AmortizedLearner::new();
        learner.set_edge_creator(creator.clone());

        // Create a candidate that passes quality gate
        // 5 nodes = 4 hops (>= 3)
        let nodes = vec![
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        let weights = vec![0.8, 0.9, 0.7, 0.85];
        let confidences = vec![0.9, 0.8, 0.75, 0.85];

        // Record 6 traversals (>= 5)
        for _ in 0..6 {
            learner.record_traversal(&nodes, &weights, &confidences);
        }

        // Process candidates
        let created = learner.process_candidates().unwrap();

        assert_eq!(created, 1, "Should create 1 shortcut");

        let edges = creator.get_created_edges();
        assert_eq!(edges.len(), 1, "Creator should have 1 edge");

        let edge = &edges[0];
        assert!(edge.is_shortcut, "Edge must have is_shortcut=true");
        assert_eq!(edge.original_path.len(), 5, "Edge must have full path");
        assert_eq!(edge.source, nodes[0], "Source must be first node");
        assert_eq!(edge.target, nodes[4], "Target must be last node");
        assert!(edge.confidence >= 0.7, "Confidence must meet threshold");
    }

    #[test]
    fn test_create_shortcut_without_creator() {
        let mut learner = AmortizedLearner::new();

        let candidate = ShortcutCandidate {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            hop_count: 4,
            traversal_count: 6,
            combined_weight: 0.5,
            min_confidence: 0.8,
            path_nodes: vec![Uuid::new_v4(); 5],
        };

        // Should succeed even without creator
        let result = learner.create_shortcut(&candidate).unwrap();
        assert!(result, "Shortcut creation should succeed without creator");
        assert_eq!(learner.shortcuts_created_this_cycle, 1);
    }

    /// Edge creator that returns false (edge already exists)
    struct RejectingEdgeCreator;

    impl EdgeCreator for RejectingEdgeCreator {
        fn create_edge(&self, _edge: &ShortcutEdge) -> CoreResult<bool> {
            Ok(false) // Edge already exists
        }
    }

    #[test]
    fn test_create_shortcut_creator_returns_false() {
        let creator = Arc::new(RejectingEdgeCreator);
        let mut learner = AmortizedLearner::with_edge_creator(creator);

        let candidate = ShortcutCandidate {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            hop_count: 4,
            traversal_count: 6,
            combined_weight: 0.5,
            min_confidence: 0.8,
            path_nodes: vec![Uuid::new_v4(); 5],
        };

        // Should return false when creator returns false
        let result = learner.create_shortcut(&candidate).unwrap();
        assert!(!result, "Should return false when creator returns false");
        assert_eq!(learner.shortcuts_created_this_cycle, 0, "Counter should not increment");
    }

    /// Edge creator that returns an error
    struct FailingEdgeCreator;

    impl EdgeCreator for FailingEdgeCreator {
        fn create_edge(&self, _edge: &ShortcutEdge) -> CoreResult<bool> {
            Err(crate::error::CoreError::Internal("Edge creation failed".into()))
        }
    }

    #[test]
    fn test_create_shortcut_creator_error_propagates() {
        let creator = Arc::new(FailingEdgeCreator);
        let mut learner = AmortizedLearner::with_edge_creator(creator);

        let candidate = ShortcutCandidate {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            hop_count: 4,
            traversal_count: 6,
            combined_weight: 0.5,
            min_confidence: 0.8,
            path_nodes: vec![Uuid::new_v4(); 5],
        };

        // Error should propagate
        let result = learner.create_shortcut(&candidate);
        assert!(result.is_err(), "Error should propagate from creator");
    }

    #[test]
    fn test_quality_gate_enforcement_with_creator() {
        let creator = Arc::new(RecordingEdgeCreator::new());
        let mut learner = AmortizedLearner::with_edge_creator(creator.clone());

        // Candidate that fails quality gate (insufficient hops)
        let bad_candidate = ShortcutCandidate {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            hop_count: 2, // < 3
            traversal_count: 6,
            combined_weight: 0.5,
            min_confidence: 0.8,
            path_nodes: vec![],
        };

        let result = learner.create_shortcut(&bad_candidate).unwrap();
        assert!(!result, "Should fail quality gate");

        let edges = creator.get_created_edges();
        assert!(edges.is_empty(), "Creator should NOT be called for failed quality gate");
    }

    #[test]
    fn test_multiple_shortcuts_with_creator() {
        let creator = Arc::new(RecordingEdgeCreator::new());
        let mut learner = AmortizedLearner::with_edge_creator(creator.clone());

        // Create two different paths
        let nodes1 = vec![
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        let nodes2 = vec![
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        let weights = vec![0.8, 0.9, 0.7, 0.85];
        let confidences = vec![0.9, 0.8, 0.75, 0.85];

        // Record both paths 6 times
        for _ in 0..6 {
            learner.record_traversal(&nodes1, &weights[..3], &confidences[..3]);
            learner.record_traversal(&nodes2, &weights, &confidences);
        }

        let created = learner.process_candidates().unwrap();
        assert_eq!(created, 2, "Should create 2 shortcuts");

        let edges = creator.get_created_edges();
        assert_eq!(edges.len(), 2, "Creator should have 2 edges");

        // Verify all edges have is_shortcut=true
        for edge in &edges {
            assert!(edge.is_shortcut, "All edges must have is_shortcut=true");
        }
    }
}
