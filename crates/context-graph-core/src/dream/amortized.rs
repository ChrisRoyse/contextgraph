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

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

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
#[derive(Debug, Clone)]
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
    /// Note: This is a stub. Agent 2 will implement actual edge creation
    /// with graph store integration.
    ///
    /// # Arguments
    ///
    /// * `candidate` - The shortcut candidate to create
    ///
    /// # Returns
    ///
    /// True if shortcut was created, false if it failed quality gate
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

        // TODO: Agent 2 will implement actual edge creation:
        // let edge = Edge {
        //     source: candidate.source,
        //     target: candidate.target,
        //     weight: candidate.combined_weight,
        //     confidence: candidate.min_confidence,
        //     is_shortcut: true,
        //     original_path: Some(candidate.path_nodes.clone()),
        // };
        // graph.store_edge(&edge).await?;

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
}
