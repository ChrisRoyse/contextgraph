//! Core type definitions for the teleological module.
//!
//! Contains ProfileId, TuckerCore, and other foundational types used
//! across the teleological fusion system.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a teleological profile.
///
/// Profiles represent task-specific configurations for embedding fusion,
/// e.g., "code_implementation", "conceptual_research".
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(String);

impl ProfileId {
    /// Create a new ProfileId.
    ///
    /// # Panics
    ///
    /// Panics if the id is empty (FAIL FAST).
    pub fn new(id: impl Into<String>) -> Self {
        let id_str = id.into();
        assert!(!id_str.is_empty(), "FAIL FAST: ProfileId cannot be empty");
        Self(id_str)
    }

    /// Get the ID as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner string.
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ProfileId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ProfileId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for ProfileId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Tucker decomposition core tensor.
///
/// From teleoplan.md: Tucker decomposition for compact representation
/// `(Core, U1, U2, U3) = tucker_decomposition(T, ranks=[4, 4, 128])`
///
/// The core tensor captures the essential structure of the full 13x13x1024
/// teleological tensor in compressed form.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TuckerCore {
    /// Core tensor dimensions (rank_1, rank_2, rank_3).
    /// Default: (4, 4, 128) per teleoplan.md
    pub ranks: (usize, usize, usize),

    /// Flattened core tensor data.
    /// Size: rank_1 * rank_2 * rank_3
    pub data: Vec<f32>,

    /// Factor matrix U1 (SYNERGY_DIM x rank_1) - embedding space compression
    pub u1: Vec<f32>,

    /// Factor matrix U2 (SYNERGY_DIM x rank_2) - embedding space compression
    pub u2: Vec<f32>,

    /// Factor matrix U3 (1024 x rank_3) - dimension compression
    pub u3: Vec<f32>,
}

impl TuckerCore {
    /// Default ranks from teleoplan.md: (4, 4, 128)
    pub const DEFAULT_RANKS: (usize, usize, usize) = (4, 4, 128);

    /// Create a new TuckerCore with specified ranks.
    ///
    /// # Arguments
    /// * `ranks` - Tuple of (rank_1, rank_2, rank_3) dimensions
    ///
    /// # Panics
    ///
    /// Panics if any rank is 0 (FAIL FAST).
    pub fn new(ranks: (usize, usize, usize)) -> Self {
        assert!(
            ranks.0 > 0 && ranks.1 > 0 && ranks.2 > 0,
            "FAIL FAST: Tucker ranks must be positive, got {:?}",
            ranks
        );

        let core_size = ranks.0 * ranks.1 * ranks.2;
        let u1_size = NUM_EMBEDDERS * ranks.0;
        let u2_size = NUM_EMBEDDERS * ranks.1;
        let u3_size = EMBEDDING_DIM * ranks.2;

        Self {
            ranks,
            data: vec![0.0; core_size],
            u1: vec![0.0; u1_size],
            u2: vec![0.0; u2_size],
            u3: vec![0.0; u3_size],
        }
    }

    /// Create a TuckerCore with default ranks (4, 4, 128).
    pub fn with_default_ranks() -> Self {
        Self::new(Self::DEFAULT_RANKS)
    }

    /// Get the total compressed size in floats.
    pub fn compressed_size(&self) -> usize {
        self.data.len() + self.u1.len() + self.u2.len() + self.u3.len()
    }

    /// Get the original uncompressed size in floats.
    /// Original: NUM_EMBEDDERS × NUM_EMBEDDERS × EMBEDDING_DIM (14 × 14 × 1024 = 200,704 floats post-E14).
    pub fn original_size(&self) -> usize {
        NUM_EMBEDDERS * NUM_EMBEDDERS * EMBEDDING_DIM
    }

    /// Compression ratio (original / compressed).
    pub fn compression_ratio(&self) -> f32 {
        self.original_size() as f32 / self.compressed_size() as f32
    }

    /// Access core tensor element at (i, j, k).
    ///
    /// # Panics
    ///
    /// Panics if indices out of bounds (FAIL FAST).
    #[inline]
    pub fn get_core(&self, i: usize, j: usize, k: usize) -> f32 {
        assert!(
            i < self.ranks.0,
            "FAIL FAST: core index i={} out of bounds (max {})",
            i,
            self.ranks.0 - 1
        );
        assert!(
            j < self.ranks.1,
            "FAIL FAST: core index j={} out of bounds (max {})",
            j,
            self.ranks.1 - 1
        );
        assert!(
            k < self.ranks.2,
            "FAIL FAST: core index k={} out of bounds (max {})",
            k,
            self.ranks.2 - 1
        );

        let idx = i * self.ranks.1 * self.ranks.2 + j * self.ranks.2 + k;
        self.data[idx]
    }

    /// Set core tensor element at (i, j, k).
    ///
    /// # Panics
    ///
    /// Panics if indices out of bounds (FAIL FAST).
    #[inline]
    pub fn set_core(&mut self, i: usize, j: usize, k: usize, value: f32) {
        assert!(
            i < self.ranks.0,
            "FAIL FAST: core index i={} out of bounds (max {})",
            i,
            self.ranks.0 - 1
        );
        assert!(
            j < self.ranks.1,
            "FAIL FAST: core index j={} out of bounds (max {})",
            j,
            self.ranks.1 - 1
        );
        assert!(
            k < self.ranks.2,
            "FAIL FAST: core index k={} out of bounds (max {})",
            k,
            self.ranks.2 - 1
        );

        let idx = i * self.ranks.1 * self.ranks.2 + j * self.ranks.2 + k;
        self.data[idx] = value;
    }
}

impl Default for TuckerCore {
    fn default() -> Self {
        Self::with_default_ranks()
    }
}

/// Embedding dimension constant (1024D vectors per teleoplan.md).
pub const EMBEDDING_DIM: usize = 1024;

/// Number of embedders in the system.
pub const NUM_EMBEDDERS: usize = 14;

/// TopicProfile: 14D profile for topic/embedding weights.
///
/// This type represents embedder-level weights or alignment scores for topics.
/// Each element corresponds to one of the 14 embedders in the system.
///
/// # Usage
///
/// Used for:
/// - Topic weight profiles (how much each embedder contributes to a topic)
/// - Per-embedder alignment scores in teleological vectors
/// - Similarity comparisons between topic profiles
///
/// # Constitution Compliance
///
/// TopicProfile focuses on embedder-level weighting for topic detection and
/// multi-space clustering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TopicProfile {
    /// 14D alignment scores, one per embedder [0.0, 1.0]
    pub alignments: [f32; NUM_EMBEDDERS],
}

impl TopicProfile {
    /// Create a new TopicProfile from an array of alignments.
    ///
    /// # Arguments
    /// * `alignments` - Array of NUM_EMBEDDERS alignment values
    pub fn new(alignments: [f32; NUM_EMBEDDERS]) -> Self {
        Self { alignments }
    }

    /// Create a TopicProfile with all alignments set to the same value.
    pub fn uniform(value: f32) -> Self {
        Self {
            alignments: [value; NUM_EMBEDDERS],
        }
    }

    /// Get the aggregate alignment score (mean of all alignments).
    #[inline]
    pub fn aggregate_alignment(&self) -> f32 {
        self.alignments.iter().sum::<f32>() / NUM_EMBEDDERS as f32
    }

    /// Compute cosine similarity between two TopicProfiles.
    ///
    /// Returns a value in [-1.0, 1.0], where 1.0 means identical direction.
    pub fn similarity(&self, other: &Self) -> f32 {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..NUM_EMBEDDERS {
            dot += self.alignments[i] * other.alignments[i];
            norm_a += self.alignments[i] * self.alignments[i];
            norm_b += other.alignments[i] * other.alignments[i];
        }

        let denom = (norm_a.sqrt()) * (norm_b.sqrt());
        if denom < f32::EPSILON {
            0.0
        } else {
            dot / denom
        }
    }

    /// Get alignment for a specific embedder index.
    ///
    /// # Panics
    ///
    /// Panics if index >= NUM_EMBEDDERS (FAIL FAST).
    #[inline]
    pub fn get(&self, embedder_idx: usize) -> f32 {
        assert!(
            embedder_idx < NUM_EMBEDDERS,
            "FAIL FAST: embedder index {} out of bounds (max {})",
            embedder_idx,
            NUM_EMBEDDERS - 1
        );
        self.alignments[embedder_idx]
    }

    /// Set alignment for a specific embedder index.
    ///
    /// # Panics
    ///
    /// Panics if index >= NUM_EMBEDDERS (FAIL FAST).
    #[inline]
    pub fn set(&mut self, embedder_idx: usize, value: f32) {
        assert!(
            embedder_idx < NUM_EMBEDDERS,
            "FAIL FAST: embedder index {} out of bounds (max {})",
            embedder_idx,
            NUM_EMBEDDERS - 1
        );
        self.alignments[embedder_idx] = value;
    }

    /// Count how many embedders have alignment above the given threshold.
    pub fn active_count(&self, threshold: f32) -> usize {
        self.alignments.iter().filter(|&&a| a > threshold).count()
    }
}

impl Default for TopicProfile {
    fn default() -> Self {
        Self {
            alignments: [0.0; NUM_EMBEDDERS],
        }
    }
}

impl From<[f32; NUM_EMBEDDERS]> for TopicProfile {
    fn from(alignments: [f32; NUM_EMBEDDERS]) -> Self {
        Self::new(alignments)
    }
}

impl AsRef<[f32; NUM_EMBEDDERS]> for TopicProfile {
    fn as_ref(&self) -> &[f32; NUM_EMBEDDERS] {
        &self.alignments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ProfileId Tests =====

    #[test]
    fn test_profile_id_new() {
        let id = ProfileId::new("code_implementation");
        assert_eq!(id.as_str(), "code_implementation");

        println!("[PASS] ProfileId::new creates valid ID");
    }

    #[test]
    fn test_profile_id_from_str() {
        let id: ProfileId = "research_task".into();
        assert_eq!(id.as_str(), "research_task");

        println!("[PASS] ProfileId From<&str> works");
    }

    #[test]
    fn test_profile_id_from_string() {
        let s = String::from("creative_writing");
        let id: ProfileId = s.into();
        assert_eq!(id.as_str(), "creative_writing");

        println!("[PASS] ProfileId From<String> works");
    }

    #[test]
    fn test_profile_id_display() {
        let id = ProfileId::new("test_profile");
        assert_eq!(format!("{}", id), "test_profile");

        println!("[PASS] ProfileId Display works");
    }

    #[test]
    fn test_profile_id_into_inner() {
        let id = ProfileId::new("test");
        let inner = id.into_inner();
        assert_eq!(inner, "test");

        println!("[PASS] ProfileId::into_inner works");
    }

    #[test]
    fn test_profile_id_equality() {
        let id1 = ProfileId::new("same");
        let id2 = ProfileId::new("same");
        let id3 = ProfileId::new("different");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);

        println!("[PASS] ProfileId equality works");
    }

    #[test]
    fn test_profile_id_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ProfileId::new("a"));
        set.insert(ProfileId::new("b"));
        set.insert(ProfileId::new("a")); // Duplicate

        assert_eq!(set.len(), 2);

        println!("[PASS] ProfileId hashing works");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_profile_id_empty_panics() {
        let _ = ProfileId::new("");
    }

    #[test]
    fn test_profile_id_serialization() {
        let id = ProfileId::new("serialization_test");
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: ProfileId = serde_json::from_str(&json).unwrap();

        assert_eq!(id, deserialized);

        println!("[PASS] ProfileId serialization roundtrip works");
    }

    // ===== TuckerCore Tests =====

    #[test]
    fn test_tucker_core_new() {
        let core = TuckerCore::new((4, 4, 128));

        assert_eq!(core.ranks, (4, 4, 128));
        assert_eq!(core.data.len(), 4 * 4 * 128);
        assert_eq!(core.u1.len(), 14 * 4);
        assert_eq!(core.u2.len(), 14 * 4);
        assert_eq!(core.u3.len(), 1024 * 128);

        println!("[PASS] TuckerCore::new creates correct sizes");
    }

    #[test]
    fn test_tucker_core_default_ranks() {
        let core = TuckerCore::with_default_ranks();
        assert_eq!(core.ranks, TuckerCore::DEFAULT_RANKS);

        println!("[PASS] TuckerCore::with_default_ranks uses (4, 4, 128)");
    }

    #[test]
    fn test_tucker_core_compression_ratio() {
        let core = TuckerCore::with_default_ranks();

        let original = core.original_size();
        let compressed = core.compressed_size();
        let ratio = core.compression_ratio();

        assert_eq!(original, NUM_EMBEDDERS * NUM_EMBEDDERS * EMBEDDING_DIM);
        assert!(ratio > 1.0); // Should compress

        println!(
            "[PASS] Compression: {} -> {} (ratio {:.2}x)",
            original, compressed, ratio
        );
    }

    #[test]
    fn test_tucker_core_get_set() {
        let mut core = TuckerCore::new((2, 3, 4));

        core.set_core(1, 2, 3, 0.5);
        assert!((core.get_core(1, 2, 3) - 0.5).abs() < f32::EPSILON);

        println!("[PASS] TuckerCore get/set works");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_tucker_core_zero_rank_panics() {
        let _ = TuckerCore::new((0, 4, 128));
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_tucker_core_get_out_of_bounds() {
        let core = TuckerCore::new((2, 2, 2));
        let _ = core.get_core(2, 0, 0);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_tucker_core_set_out_of_bounds() {
        let mut core = TuckerCore::new((2, 2, 2));
        core.set_core(0, 2, 0, 1.0);
    }

    #[test]
    fn test_tucker_core_default() {
        let core = TuckerCore::default();
        assert_eq!(core.ranks, TuckerCore::DEFAULT_RANKS);

        println!("[PASS] TuckerCore::default works");
    }

    #[test]
    fn test_tucker_core_serialization() {
        let mut core = TuckerCore::new((2, 2, 4));
        core.set_core(1, 1, 3, 0.75);

        let json = serde_json::to_string(&core).unwrap();
        let deserialized: TuckerCore = serde_json::from_str(&json).unwrap();

        assert_eq!(core.ranks, deserialized.ranks);
        assert!((core.get_core(1, 1, 3) - deserialized.get_core(1, 1, 3)).abs() < f32::EPSILON);

        println!("[PASS] TuckerCore serialization roundtrip works");
    }

    #[test]
    fn test_constants() {
        assert_eq!(EMBEDDING_DIM, 1024);
        assert_eq!(NUM_EMBEDDERS, 14);

        println!("[PASS] Constants match teleoplan.md");
    }
}
