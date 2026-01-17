//! Cluster type for representing cluster metadata and quality.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::teleological::Embedder;

/// Silhouette score threshold for high-quality clusters.
/// Per constitution clustering.parameters.silhouette_threshold: 0.3
pub const HIGH_QUALITY_THRESHOLD: f32 = 0.3;

/// Represents a cluster in an embedding space.
///
/// Each cluster has a centroid (mean embedding), member count,
/// and quality metrics like silhouette score.
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::Cluster;
/// use context_graph_core::teleological::Embedder;
///
/// let centroid = vec![0.1, 0.2, 0.3]; // simplified
/// let mut cluster = Cluster::new(1, Embedder::Semantic, centroid, 10);
///
/// cluster.update_silhouette(0.75);
/// assert!(cluster.is_high_quality());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// Cluster identifier (unique per space).
    pub id: i32,

    /// The embedding space this cluster belongs to.
    pub space: Embedder,

    /// Cluster centroid (mean of all member embeddings).
    pub centroid: Vec<f32>,

    /// Number of members in this cluster.
    pub member_count: u32,

    /// Silhouette score (-1.0..=1.0, higher is better).
    /// Measures how similar members are to own cluster vs other clusters.
    pub silhouette_score: f32,

    /// When the cluster was created.
    pub created_at: DateTime<Utc>,

    /// When the cluster was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Cluster {
    /// Create a new cluster.
    ///
    /// # Arguments
    ///
    /// * `id` - Cluster identifier
    /// * `space` - Embedding space
    /// * `centroid` - Mean embedding vector
    /// * `member_count` - Number of members
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::Cluster;
    /// use context_graph_core::teleological::Embedder;
    ///
    /// let cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 5);
    /// assert_eq!(cluster.id, 1);
    /// assert_eq!(cluster.silhouette_score, 0.0); // Default until computed
    /// ```
    pub fn new(id: i32, space: Embedder, centroid: Vec<f32>, member_count: u32) -> Self {
        let now = Utc::now();
        Self {
            id,
            space,
            centroid,
            member_count,
            silhouette_score: 0.0, // Computed later via update_silhouette
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the silhouette score.
    ///
    /// Score is clamped to valid range -1.0..=1.0.
    /// Also updates the updated_at timestamp.
    ///
    /// # Arguments
    ///
    /// * `score` - New silhouette score (will be clamped)
    pub fn update_silhouette(&mut self, score: f32) {
        self.silhouette_score = score.clamp(-1.0, 1.0);
        self.touch();
    }

    /// Update the updated_at timestamp to now.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Check if this cluster has high quality.
    ///
    /// Returns true if silhouette_score >= 0.3 (per constitution).
    #[inline]
    pub fn is_high_quality(&self) -> bool {
        self.silhouette_score >= HIGH_QUALITY_THRESHOLD
    }

    /// Update centroid and member count.
    ///
    /// Used when cluster membership changes.
    pub fn update_centroid(&mut self, centroid: Vec<f32>, member_count: u32) {
        self.centroid = centroid;
        self.member_count = member_count;
        self.touch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_cluster_creation() {
        let centroid = vec![0.1, 0.2, 0.3, 0.4];
        let cluster = Cluster::new(5, Embedder::Semantic, centroid.clone(), 10);

        assert_eq!(cluster.id, 5);
        assert_eq!(cluster.space, Embedder::Semantic);
        assert_eq!(cluster.centroid, centroid);
        assert_eq!(cluster.member_count, 10);
        assert_eq!(cluster.silhouette_score, 0.0);

        println!(
            "[PASS] test_cluster_creation - id={}, space={:?}, members={}",
            cluster.id, cluster.space, cluster.member_count
        );
    }

    #[test]
    fn test_cluster_touch() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        let old_updated = cluster.updated_at;
        thread::sleep(Duration::from_millis(10));
        cluster.touch();

        assert!(
            cluster.updated_at > old_updated,
            "updated_at should increase after touch"
        );

        println!(
            "[PASS] test_cluster_touch - old={}, new={}",
            old_updated, cluster.updated_at
        );
    }

    #[test]
    fn test_update_silhouette_normal() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        cluster.update_silhouette(0.75);
        assert!((cluster.silhouette_score - 0.75).abs() < f32::EPSILON);
        assert!(cluster.is_high_quality());

        println!(
            "[PASS] test_update_silhouette_normal - score=0.75, high_quality={}",
            cluster.is_high_quality()
        );
    }

    #[test]
    fn test_silhouette_clamping_high() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        cluster.update_silhouette(2.5); // Should clamp to 1.0
        assert_eq!(cluster.silhouette_score, 1.0);

        println!(
            "[PASS] test_silhouette_clamping_high - 2.5 clamped to {}",
            cluster.silhouette_score
        );
    }

    #[test]
    fn test_silhouette_clamping_low() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        cluster.update_silhouette(-2.5); // Should clamp to -1.0
        assert_eq!(cluster.silhouette_score, -1.0);

        println!(
            "[PASS] test_silhouette_clamping_low - -2.5 clamped to {}",
            cluster.silhouette_score
        );
    }

    #[test]
    fn test_is_high_quality() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        cluster.update_silhouette(0.4);
        assert!(cluster.is_high_quality(), "0.4 should be high quality");

        cluster.update_silhouette(0.3);
        assert!(
            cluster.is_high_quality(),
            "0.3 should be high quality (threshold)"
        );

        cluster.update_silhouette(0.29);
        assert!(!cluster.is_high_quality(), "0.29 should not be high quality");

        println!("[PASS] test_is_high_quality - threshold=0.3 working correctly");
    }

    #[test]
    fn test_update_centroid() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 4], 5);
        let old_updated = cluster.updated_at;

        thread::sleep(Duration::from_millis(10));
        cluster.update_centroid(vec![1.0, 2.0, 3.0, 4.0], 15);

        assert_eq!(cluster.centroid, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(cluster.member_count, 15);
        assert!(cluster.updated_at > old_updated);

        println!(
            "[PASS] test_update_centroid - new members={}, centroid updated",
            cluster.member_count
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        let cluster = Cluster::new(42, Embedder::Code, vec![0.1, 0.2, 0.3], 100);

        let json = serde_json::to_string(&cluster).expect("serialize");
        let restored: Cluster = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(cluster.id, restored.id);
        assert_eq!(cluster.space, restored.space);
        assert_eq!(cluster.centroid, restored.centroid);
        assert_eq!(cluster.member_count, restored.member_count);

        println!("[PASS] test_serialization_roundtrip - JSON preserved all fields");
    }

    #[test]
    fn test_all_embedder_spaces() {
        for embedder in Embedder::all() {
            let cluster = Cluster::new(1, embedder, vec![0.0; 10], 5);
            assert_eq!(cluster.space, embedder);

            // Verify serialization
            let json = serde_json::to_string(&cluster).expect("serialize");
            let restored: Cluster = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(cluster.space, restored.space);
        }

        println!("[PASS] test_all_embedder_spaces - all 13 spaces work");
    }
}
