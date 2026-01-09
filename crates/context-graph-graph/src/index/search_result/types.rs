//! Core types for FAISS search results.
//!
//! Contains the primary data structures for representing search output.

/// Result from FAISS k-NN search.
///
/// Encapsulates the raw output from `FaissGpuIndex::search()` with methods
/// for extracting per-query results and filtering sentinel IDs.
///
/// # Memory Layout
///
/// FAISS returns flat arrays where results for query `i` start at index `i * k`.
/// For n queries with k neighbors each:
/// - `ids.len() == n * k`
/// - `distances.len() == n * k`
///
/// # Sentinel Handling
///
/// FAISS uses `-1` to indicate "no match found" for a given position.
/// This happens when:
/// - The index has fewer than k vectors
/// - Some IVF cells have fewer than nprobe neighbors
///
/// All `query_results*` methods automatically filter out `-1` sentinels.
#[derive(Clone, Debug, Default)]
pub struct SearchResult {
    /// Vector IDs for all queries (flattened, k per query)
    /// -1 indicates no match found for that position
    pub ids: Vec<i64>,
    /// Distances for all queries (flattened, k per query)
    /// L2 squared distance: lower = more similar
    pub distances: Vec<f32>,
    /// Number of neighbors requested per query
    pub k: usize,
    /// Number of queries in this result
    pub num_queries: usize,
}

impl SearchResult {
    /// Create a new SearchResult from raw FAISS output.
    ///
    /// # Arguments
    ///
    /// * `ids` - Vector IDs (flattened, k per query)
    /// * `distances` - L2 squared distances (flattened, k per query)
    /// * `k` - Number of neighbors per query
    /// * `num_queries` - Number of queries
    ///
    /// # Panics (debug only)
    ///
    /// Debug assertions verify array lengths match `k * num_queries`.
    #[inline]
    pub fn new(ids: Vec<i64>, distances: Vec<f32>, k: usize, num_queries: usize) -> Self {
        debug_assert_eq!(
            ids.len(),
            k * num_queries,
            "ids.len() ({}) != k ({}) * num_queries ({})",
            ids.len(),
            k,
            num_queries
        );
        debug_assert_eq!(
            distances.len(),
            k * num_queries,
            "distances.len() ({}) != k ({}) * num_queries ({})",
            distances.len(),
            k,
            num_queries
        );
        Self {
            ids,
            distances,
            k,
            num_queries,
        }
    }

    /// Get the number of queries in this result.
    #[inline]
    pub fn len(&self) -> usize {
        self.num_queries
    }

    /// Get the k value (neighbors per query).
    #[inline]
    pub fn k(&self) -> usize {
        self.k
    }
}

/// Single search result item with additional metadata.
///
/// Provides both L2 distance and cosine similarity for convenience.
/// Useful when downstream code needs similarity scores.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResultItem {
    /// Vector ID from the index
    pub id: i64,
    /// L2 distance from query (lower = more similar)
    pub distance: f32,
    /// Cosine similarity (derived from L2 for normalized vectors)
    /// Higher = more similar, range [-1, 1] for normalized vectors
    pub similarity: f32,
}

impl SearchResultItem {
    /// Create from ID and L2 distance.
    ///
    /// Converts L2 distance to cosine similarity assuming normalized vectors.
    ///
    /// # Math
    ///
    /// For normalized vectors (||a|| = ||b|| = 1):
    /// - L2 distance: d = ||a - b|| = sqrt(2 - 2*cos(theta))
    /// - Therefore: d^2 = 2 - 2*cos(theta)
    /// - Solving: cos(theta) = 1 - d^2/2
    ///
    /// # Arguments
    ///
    /// * `id` - Vector ID
    /// * `distance` - L2 distance (NOT squared - FAISS returns squared L2)
    ///
    /// # Note
    ///
    /// FAISS IVF-PQ with L2 metric returns squared L2 distances.
    /// The input `distance` should be the raw FAISS output.
    #[inline]
    pub fn from_l2(id: i64, distance: f32) -> Self {
        // FAISS returns squared L2 distance for efficiency
        // For normalized vectors: d^2 = 2(1 - cos(theta))
        // Therefore: similarity = 1 - d^2/2
        let similarity = 1.0 - (distance / 2.0);
        Self {
            id,
            distance,
            similarity,
        }
    }

    /// Create from ID and cosine similarity.
    ///
    /// Computes L2 distance from similarity assuming normalized vectors.
    ///
    /// # Arguments
    ///
    /// * `id` - Vector ID
    /// * `similarity` - Cosine similarity in range [-1, 1]
    #[inline]
    pub fn from_similarity(id: i64, similarity: f32) -> Self {
        // d^2 = 2(1 - cos(theta))
        let distance = 2.0 * (1.0 - similarity);
        Self {
            id,
            distance,
            similarity,
        }
    }
}
