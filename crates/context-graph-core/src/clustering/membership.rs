//! ClusterMembership type for tracking memory cluster assignments.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::teleological::Embedder;

/// Confidence threshold for high-confidence cluster assignments.
pub const CONFIDENT_THRESHOLD: f32 = 0.8;

/// Cluster ID indicating noise (not assigned to any cluster).
pub const NOISE_CLUSTER_ID: i32 = -1;

/// Represents a memory's cluster assignment in a specific embedding space.
///
/// Each memory can have different cluster assignments in different embedding
/// spaces. The cluster_id of -1 indicates the memory is noise (outlier) in
/// that space.
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::ClusterMembership;
/// use context_graph_core::teleological::Embedder;
/// use uuid::Uuid;
///
/// // Create a normal cluster membership
/// let mem_id = Uuid::new_v4();
/// let membership = ClusterMembership::new(mem_id, Embedder::Semantic, 5, 0.95, true);
/// assert!(!membership.is_noise());
/// assert!(membership.is_confident());
///
/// // Create a noise membership
/// let noise = ClusterMembership::noise(mem_id, Embedder::Semantic);
/// assert!(noise.is_noise());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterMembership {
    /// The memory this membership belongs to.
    pub memory_id: Uuid,

    /// The embedding space this assignment is for.
    pub space: Embedder,

    /// Cluster ID (-1 = noise, not in any cluster).
    pub cluster_id: i32,

    /// Probability of belonging to this cluster (0.0..=1.0).
    /// Computed by HDBSCAN based on density.
    pub membership_probability: f32,

    /// Whether this point is a core point of the cluster.
    /// Core points are central to cluster density.
    pub is_core_point: bool,
}

impl ClusterMembership {
    /// Create a new cluster membership.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - UUID of the memory
    /// * `space` - Embedding space this membership is for
    /// * `cluster_id` - Cluster ID (-1 for noise)
    /// * `probability` - Membership probability (will be clamped to 0.0..=1.0)
    /// * `is_core` - Whether this is a core point
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::ClusterMembership;
    /// use context_graph_core::teleological::Embedder;
    /// use uuid::Uuid;
    ///
    /// let membership = ClusterMembership::new(
    ///     Uuid::new_v4(),
    ///     Embedder::Semantic,
    ///     5,
    ///     0.95,
    ///     true,
    /// );
    /// ```
    pub fn new(
        memory_id: Uuid,
        space: Embedder,
        cluster_id: i32,
        probability: f32,
        is_core: bool,
    ) -> Self {
        Self {
            memory_id,
            space,
            cluster_id,
            membership_probability: probability.clamp(0.0, 1.0),
            is_core_point: is_core,
        }
    }

    /// Create a noise membership (not in any cluster).
    ///
    /// Noise points have cluster_id = -1, probability = 0.0,
    /// and are never core points.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::ClusterMembership;
    /// use context_graph_core::teleological::Embedder;
    /// use uuid::Uuid;
    ///
    /// let noise = ClusterMembership::noise(Uuid::new_v4(), Embedder::Semantic);
    /// assert!(noise.is_noise());
    /// assert_eq!(noise.cluster_id, -1);
    /// ```
    pub fn noise(memory_id: Uuid, space: Embedder) -> Self {
        Self {
            memory_id,
            space,
            cluster_id: NOISE_CLUSTER_ID,
            membership_probability: 0.0,
            is_core_point: false,
        }
    }

    /// Check if this is a noise point (not in any cluster).
    #[inline]
    pub fn is_noise(&self) -> bool {
        self.cluster_id == NOISE_CLUSTER_ID
    }

    /// Check if this is a high-confidence assignment.
    ///
    /// Returns true if membership_probability >= 0.8.
    #[inline]
    pub fn is_confident(&self) -> bool {
        self.membership_probability >= CONFIDENT_THRESHOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_membership() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::noise(mem_id, Embedder::Semantic);

        assert!(membership.is_noise(), "noise() should create noise membership");
        assert_eq!(membership.cluster_id, -1, "noise cluster_id should be -1");
        assert_eq!(
            membership.membership_probability, 0.0,
            "noise probability should be 0.0"
        );
        assert!(!membership.is_core_point, "noise should not be core point");
        assert!(!membership.is_confident(), "noise should not be confident");

        println!(
            "[PASS] test_noise_membership - cluster_id={}, prob={}, is_noise={}",
            membership.cluster_id,
            membership.membership_probability,
            membership.is_noise()
        );
    }

    #[test]
    fn test_cluster_membership() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::new(mem_id, Embedder::Semantic, 5, 0.95, true);

        assert!(!membership.is_noise(), "non-noise should not be noise");
        assert_eq!(membership.cluster_id, 5, "cluster_id should be 5");
        assert!(membership.is_confident(), "0.95 should be confident");
        assert!(membership.is_core_point, "should be core point");

        println!(
            "[PASS] test_cluster_membership - cluster_id={}, confident={}",
            membership.cluster_id,
            membership.is_confident()
        );
    }

    #[test]
    fn test_probability_clamping_high() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::new(
            mem_id,
            Embedder::Semantic,
            1,
            1.5, // Should be clamped to 1.0
            false,
        );

        assert_eq!(
            membership.membership_probability, 1.0,
            "probability > 1.0 should clamp to 1.0"
        );
        println!(
            "[PASS] test_probability_clamping_high - 1.5 clamped to {}",
            membership.membership_probability
        );
    }

    #[test]
    fn test_probability_clamping_low() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::new(
            mem_id,
            Embedder::Semantic,
            1,
            -0.5, // Should be clamped to 0.0
            false,
        );

        assert_eq!(
            membership.membership_probability, 0.0,
            "probability < 0.0 should clamp to 0.0"
        );
        println!(
            "[PASS] test_probability_clamping_low - -0.5 clamped to {}",
            membership.membership_probability
        );
    }

    #[test]
    fn test_is_confident_threshold() {
        let mem_id = Uuid::new_v4();

        let confident = ClusterMembership::new(mem_id, Embedder::Semantic, 1, 0.9, false);
        assert!(confident.is_confident(), "0.9 should be confident");

        let borderline = ClusterMembership::new(mem_id, Embedder::Semantic, 1, 0.8, false);
        assert!(
            borderline.is_confident(),
            "0.8 should be confident (threshold)"
        );

        let not_confident = ClusterMembership::new(mem_id, Embedder::Semantic, 1, 0.79, false);
        assert!(!not_confident.is_confident(), "0.79 should not be confident");

        println!(
            "[PASS] test_is_confident_threshold - 0.9={}, 0.8={}, 0.79={}",
            confident.is_confident(),
            borderline.is_confident(),
            not_confident.is_confident()
        );
    }

    #[test]
    fn test_all_embedders() {
        let mem_id = Uuid::new_v4();

        for embedder in Embedder::all() {
            let membership = ClusterMembership::new(mem_id, embedder, 1, 0.5, false);
            assert_eq!(membership.space, embedder, "space should match embedder");

            // Verify serialization works
            let json = serde_json::to_string(&membership).expect("serialize should work");
            let restored: ClusterMembership =
                serde_json::from_str(&json).expect("deserialize should work");
            assert_eq!(membership, restored, "roundtrip should preserve data");
        }

        println!("[PASS] test_all_embedders - all 13 variants work with serialization");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::new(mem_id, Embedder::Code, 42, 0.87, true);

        let json = serde_json::to_string(&membership).expect("serialize");
        let restored: ClusterMembership = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(membership.memory_id, restored.memory_id);
        assert_eq!(membership.space, restored.space);
        assert_eq!(membership.cluster_id, restored.cluster_id);
        assert!(
            (membership.membership_probability - restored.membership_probability).abs()
                < f32::EPSILON
        );
        assert_eq!(membership.is_core_point, restored.is_core_point);

        println!("[PASS] test_serialization_roundtrip - JSON: {}", json);
    }
}
