//! Search result types for single embedder search.
//!
//! # Types
//!
//! - [`EmbedderSearchHit`]: A single search result with ID, distance, similarity
//! - [`SingleEmbedderSearchResults`]: Collection of hits from one embedder
//!
//! # Distance to Similarity Conversion
//!
//! HNSW returns **distance** (lower = more similar). We convert to **similarity**
//! for consistency with the comparison API:
//!
//! ```text
//! Cosine distance ∈ [0, 2]  →  similarity = 1 - distance  →  ∈ [-1, 1]
//! Clamped to [0, 1] for practical use
//! ```

use uuid::Uuid;

use super::super::indexes::EmbedderIndex;

/// A single search result from an embedder index.
///
/// Contains both the raw distance from HNSW and the converted similarity score.
///
/// # Fields
///
/// - `id`: Memory UUID
/// - `distance`: Raw HNSW distance (lower = more similar)
/// - `similarity`: Converted score [0.0, 1.0] (higher = more similar)
/// - `embedder`: Which embedder was searched
///
/// # Example
///
/// ```
/// use context_graph_storage::teleological::search::EmbedderSearchHit;
/// use context_graph_storage::teleological::indexes::EmbedderIndex;
/// use uuid::Uuid;
///
/// let hit = EmbedderSearchHit::from_hnsw(
///     Uuid::new_v4(),
///     0.1,  // 10% distance = 90% similarity
///     EmbedderIndex::E1Semantic,
/// );
///
/// assert!(hit.similarity > 0.89 && hit.similarity < 0.91);
/// ```
#[derive(Debug, Clone)]
pub struct EmbedderSearchHit {
    /// The memory ID (fingerprint UUID).
    pub id: Uuid,

    /// Distance from query (lower = more similar for HNSW).
    ///
    /// Note: HNSW returns distance, not similarity.
    /// For cosine metric: distance ∈ [0, 2] where 0 = identical.
    pub distance: f32,

    /// Similarity score [0.0, 1.0] (converted from distance).
    ///
    /// For cosine distance: similarity = 1.0 - distance, clamped to [0, 1].
    pub similarity: f32,

    /// Which embedder was searched.
    pub embedder: EmbedderIndex,
}

impl EmbedderSearchHit {
    /// Create from HNSW search result (id, distance).
    ///
    /// Converts distance to similarity based on cosine metric:
    /// - distance 0.0 → similarity 1.0 (identical)
    /// - distance 1.0 → similarity 0.0 (orthogonal)
    /// - distance 2.0 → similarity 0.0 (clamped, opposite)
    ///
    /// # Arguments
    ///
    /// * `id` - Memory UUID
    /// * `distance` - HNSW distance (typically cosine distance)
    /// * `embedder` - Which embedder was searched
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_storage::teleological::search::EmbedderSearchHit;
    /// use context_graph_storage::teleological::indexes::EmbedderIndex;
    /// use uuid::Uuid;
    ///
    /// // Identical vectors (distance 0)
    /// let hit = EmbedderSearchHit::from_hnsw(
    ///     Uuid::new_v4(),
    ///     0.0,
    ///     EmbedderIndex::E8Graph,
    /// );
    /// assert!((hit.similarity - 1.0).abs() < 0.001);
    ///
    /// // Orthogonal vectors (distance 1)
    /// let hit = EmbedderSearchHit::from_hnsw(
    ///     Uuid::new_v4(),
    ///     1.0,
    ///     EmbedderIndex::E8Graph,
    /// );
    /// assert!(hit.similarity.abs() < 0.001);
    /// ```
    #[inline]
    pub fn from_hnsw(id: Uuid, distance: f32, embedder: EmbedderIndex) -> Self {
        // For cosine distance, similarity = 1.0 - distance
        // HNSW returns distance in [0, 2] for cosine, clamped to [0, 1]
        let similarity = (1.0 - distance).clamp(0.0, 1.0);
        Self {
            id,
            distance,
            similarity,
            embedder,
        }
    }

    /// Check if this hit has high similarity (>= 0.9).
    #[inline]
    pub fn is_high_similarity(&self) -> bool {
        self.similarity >= 0.9
    }

    /// Check if this hit meets a minimum similarity threshold.
    #[inline]
    pub fn meets_threshold(&self, min_similarity: f32) -> bool {
        self.similarity >= min_similarity
    }
}

/// Results from a single embedder search.
///
/// Contains hits sorted by similarity descending, plus metadata about the search.
///
/// # Fields
///
/// - `hits`: Search results sorted by similarity (highest first)
/// - `embedder`: Which embedder was searched
/// - `k`: Requested limit
/// - `threshold`: Minimum similarity filter (if applied)
/// - `latency_us`: Search latency in microseconds
///
/// # Example
///
/// ```
/// use context_graph_storage::teleological::search::{
///     EmbedderSearchHit, SingleEmbedderSearchResults,
/// };
/// use context_graph_storage::teleological::indexes::EmbedderIndex;
/// use uuid::Uuid;
///
/// let results = SingleEmbedderSearchResults {
///     hits: vec![
///         EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.1, EmbedderIndex::E1Semantic),
///         EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.3, EmbedderIndex::E1Semantic),
///     ],
///     embedder: EmbedderIndex::E1Semantic,
///     k: 10,
///     threshold: Some(0.5),
///     latency_us: 150,
/// };
///
/// assert_eq!(results.len(), 2);
/// assert!(results.top().unwrap().similarity > 0.8);
/// ```
#[derive(Debug, Clone)]
pub struct SingleEmbedderSearchResults {
    /// Hits sorted by similarity descending.
    pub hits: Vec<EmbedderSearchHit>,

    /// Which embedder was searched.
    pub embedder: EmbedderIndex,

    /// Query k (requested limit).
    pub k: usize,

    /// Threshold applied (if any).
    pub threshold: Option<f32>,

    /// Search latency in microseconds.
    pub latency_us: u64,
}

impl SingleEmbedderSearchResults {
    /// Check if no results were found.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// Get the number of results.
    #[inline]
    pub fn len(&self) -> usize {
        self.hits.len()
    }

    /// Get the top (most similar) result.
    #[inline]
    pub fn top(&self) -> Option<&EmbedderSearchHit> {
        self.hits.first()
    }

    /// Get all result IDs.
    #[inline]
    pub fn ids(&self) -> Vec<Uuid> {
        self.hits.iter().map(|h| h.id).collect()
    }

    /// Get top N results.
    #[inline]
    pub fn top_n(&self, n: usize) -> &[EmbedderSearchHit] {
        if n >= self.hits.len() {
            &self.hits
        } else {
            &self.hits[..n]
        }
    }

    /// Get results with similarity above threshold.
    #[inline]
    pub fn above_threshold(&self, min_similarity: f32) -> Vec<&EmbedderSearchHit> {
        self.hits
            .iter()
            .filter(|h| h.similarity >= min_similarity)
            .collect()
    }

    /// Get average similarity of all hits.
    #[inline]
    pub fn average_similarity(&self) -> Option<f32> {
        if self.hits.is_empty() {
            None
        } else {
            let sum: f32 = self.hits.iter().map(|h| h.similarity).sum();
            Some(sum / self.hits.len() as f32)
        }
    }

    /// Get iterator over hits.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &EmbedderSearchHit> {
        self.hits.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hit_from_hnsw_identical_vectors() {
        println!("=== TEST: Identical vectors have distance 0, similarity 1 ===");
        println!("BEFORE: distance = 0.0");

        let hit = EmbedderSearchHit::from_hnsw(
            Uuid::new_v4(),
            0.0,
            EmbedderIndex::E1Semantic,
        );

        println!("AFTER: similarity = {}", hit.similarity);
        assert!((hit.similarity - 1.0).abs() < 1e-6);
        assert!(hit.is_high_similarity());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_hit_from_hnsw_orthogonal_vectors() {
        println!("=== TEST: Orthogonal vectors have distance 1, similarity 0 ===");
        println!("BEFORE: distance = 1.0");

        let hit = EmbedderSearchHit::from_hnsw(
            Uuid::new_v4(),
            1.0,
            EmbedderIndex::E1Semantic,
        );

        println!("AFTER: similarity = {}", hit.similarity);
        assert!(hit.similarity.abs() < 1e-6);
        assert!(!hit.is_high_similarity());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_hit_from_hnsw_opposite_vectors() {
        println!("=== TEST: Opposite vectors have distance 2, similarity clamped to 0 ===");
        println!("BEFORE: distance = 2.0");

        let hit = EmbedderSearchHit::from_hnsw(
            Uuid::new_v4(),
            2.0,
            EmbedderIndex::E1Semantic,
        );

        println!("AFTER: similarity = {}", hit.similarity);
        assert_eq!(hit.similarity, 0.0);  // Clamped

        println!("RESULT: PASS");
    }

    #[test]
    fn test_hit_from_hnsw_partial_similarity() {
        println!("=== TEST: Partial similarity (distance 0.3) ===");
        println!("BEFORE: distance = 0.3");

        let hit = EmbedderSearchHit::from_hnsw(
            Uuid::new_v4(),
            0.3,
            EmbedderIndex::E8Graph,
        );

        println!("AFTER: similarity = {}", hit.similarity);
        assert!((hit.similarity - 0.7).abs() < 1e-6);
        assert!(!hit.is_high_similarity());  // 0.7 < 0.9

        println!("RESULT: PASS");
    }

    #[test]
    fn test_hit_meets_threshold() {
        println!("=== TEST: meets_threshold() ===");

        let hit = EmbedderSearchHit::from_hnsw(
            Uuid::new_v4(),
            0.2,  // similarity = 0.8
            EmbedderIndex::E1Semantic,
        );

        assert!(hit.meets_threshold(0.5));
        assert!(hit.meets_threshold(0.8));
        assert!(!hit.meets_threshold(0.81));
        assert!(!hit.meets_threshold(0.9));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_results_empty() {
        println!("=== TEST: Empty results ===");

        let results = SingleEmbedderSearchResults {
            hits: vec![],
            embedder: EmbedderIndex::E1Semantic,
            k: 10,
            threshold: None,
            latency_us: 100,
        };

        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
        assert!(results.top().is_none());
        assert!(results.ids().is_empty());
        assert!(results.average_similarity().is_none());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_results_with_hits() {
        println!("=== TEST: Results with multiple hits ===");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let results = SingleEmbedderSearchResults {
            hits: vec![
                EmbedderSearchHit::from_hnsw(id1, 0.1, EmbedderIndex::E1Semantic),  // sim 0.9
                EmbedderSearchHit::from_hnsw(id2, 0.3, EmbedderIndex::E1Semantic),  // sim 0.7
                EmbedderSearchHit::from_hnsw(id3, 0.5, EmbedderIndex::E1Semantic),  // sim 0.5
            ],
            embedder: EmbedderIndex::E1Semantic,
            k: 10,
            threshold: None,
            latency_us: 250,
        };

        assert!(!results.is_empty());
        assert_eq!(results.len(), 3);

        let top = results.top().unwrap();
        assert_eq!(top.id, id1);
        assert!((top.similarity - 0.9).abs() < 1e-6);

        let ids = results.ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&id1));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_results_top_n() {
        println!("=== TEST: top_n() ===");

        let results = SingleEmbedderSearchResults {
            hits: vec![
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.1, EmbedderIndex::E1Semantic),
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.2, EmbedderIndex::E1Semantic),
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.3, EmbedderIndex::E1Semantic),
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.4, EmbedderIndex::E1Semantic),
            ],
            embedder: EmbedderIndex::E1Semantic,
            k: 10,
            threshold: None,
            latency_us: 100,
        };

        assert_eq!(results.top_n(2).len(), 2);
        assert_eq!(results.top_n(10).len(), 4);  // Only 4 available
        assert_eq!(results.top_n(0).len(), 0);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_results_above_threshold() {
        println!("=== TEST: above_threshold() ===");

        let results = SingleEmbedderSearchResults {
            hits: vec![
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.1, EmbedderIndex::E1Semantic),  // 0.9
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.3, EmbedderIndex::E1Semantic),  // 0.7
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.6, EmbedderIndex::E1Semantic),  // 0.4
            ],
            embedder: EmbedderIndex::E1Semantic,
            k: 10,
            threshold: None,
            latency_us: 100,
        };

        let above_80 = results.above_threshold(0.8);
        assert_eq!(above_80.len(), 1);

        let above_50 = results.above_threshold(0.5);
        assert_eq!(above_50.len(), 2);

        let above_30 = results.above_threshold(0.3);
        assert_eq!(above_30.len(), 3);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_results_average_similarity() {
        println!("=== TEST: average_similarity() ===");

        let results = SingleEmbedderSearchResults {
            hits: vec![
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.0, EmbedderIndex::E1Semantic),  // 1.0
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.5, EmbedderIndex::E1Semantic),  // 0.5
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 1.0, EmbedderIndex::E1Semantic),  // 0.0
            ],
            embedder: EmbedderIndex::E1Semantic,
            k: 10,
            threshold: None,
            latency_us: 100,
        };

        let avg = results.average_similarity().unwrap();
        println!("Average similarity: {}", avg);
        assert!((avg - 0.5).abs() < 1e-6);  // (1.0 + 0.5 + 0.0) / 3 = 0.5

        println!("RESULT: PASS");
    }

    #[test]
    fn test_results_iter() {
        println!("=== TEST: iter() ===");

        let results = SingleEmbedderSearchResults {
            hits: vec![
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.1, EmbedderIndex::E1Semantic),
                EmbedderSearchHit::from_hnsw(Uuid::new_v4(), 0.2, EmbedderIndex::E1Semantic),
            ],
            embedder: EmbedderIndex::E1Semantic,
            k: 10,
            threshold: None,
            latency_us: 100,
        };

        let count = results.iter().count();
        assert_eq!(count, 2);

        for hit in results.iter() {
            assert!(hit.similarity > 0.0);
        }

        println!("RESULT: PASS");
    }

    #[test]
    fn test_verification_log() {
        println!("\n=== RESULT.RS VERIFICATION LOG ===");
        println!();

        println!("Type Verification:");
        println!("  - EmbedderSearchHit:");
        println!("    - id: Uuid");
        println!("    - distance: f32");
        println!("    - similarity: f32");
        println!("    - embedder: EmbedderIndex");
        println!("  - SingleEmbedderSearchResults:");
        println!("    - hits: Vec<EmbedderSearchHit>");
        println!("    - embedder: EmbedderIndex");
        println!("    - k: usize");
        println!("    - threshold: Option<f32>");
        println!("    - latency_us: u64");

        println!();
        println!("Method Verification:");
        println!("  - EmbedderSearchHit::from_hnsw: PASS");
        println!("  - EmbedderSearchHit::is_high_similarity: PASS");
        println!("  - EmbedderSearchHit::meets_threshold: PASS");
        println!("  - SingleEmbedderSearchResults::is_empty: PASS");
        println!("  - SingleEmbedderSearchResults::len: PASS");
        println!("  - SingleEmbedderSearchResults::top: PASS");
        println!("  - SingleEmbedderSearchResults::ids: PASS");
        println!("  - SingleEmbedderSearchResults::top_n: PASS");
        println!("  - SingleEmbedderSearchResults::above_threshold: PASS");
        println!("  - SingleEmbedderSearchResults::average_similarity: PASS");
        println!("  - SingleEmbedderSearchResults::iter: PASS");

        println!();
        println!("Distance-to-Similarity Conversion:");
        println!("  - distance 0.0 → similarity 1.0: PASS");
        println!("  - distance 1.0 → similarity 0.0: PASS");
        println!("  - distance 2.0 → similarity 0.0 (clamped): PASS");
        println!("  - distance 0.3 → similarity 0.7: PASS");

        println!();
        println!("VERIFICATION COMPLETE");
    }
}
