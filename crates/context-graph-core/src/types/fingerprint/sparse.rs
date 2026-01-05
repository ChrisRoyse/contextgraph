//! Sparse vector representation for E6 (SPLADE) embeddings.
//!
//! This module provides a memory-efficient sparse vector implementation for storing
//! SPLADE (Sparse Lexical and Expansion) embeddings. SPLADE produces sparse vectors
//! where only ~5% of vocabulary positions have non-zero activations.
//!
//! # Design Decisions
//!
//! - **u16 indices**: SPLADE vocabulary size is 30,522, which fits in u16 (max 65,535)
//! - **Sorted indices**: Required for efficient dot product computation via merge-join
//! - **Validation on construction**: Fail fast with detailed error types
//! - **No unwrap() in production**: All errors propagated with context

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

/// SPLADE vocabulary size (matches SPARSE_NATIVE in context-graph-embeddings)
///
/// This constant represents the BERT vocabulary size used by SPLADE models.
/// All indices in a SparseVector must be strictly less than this value.
pub const SPARSE_VOCAB_SIZE: usize = 30_522;

/// Maximum expected active indices (~5% sparsity)
///
/// SPLADE typically activates about 5% of vocabulary positions.
/// This constant is useful for pre-allocating vectors.
pub const MAX_SPARSE_ACTIVE: usize = 1_526; // floor(30522 * 0.05)

/// Sparse vector for lexical (SPLADE) embeddings.
///
/// Stores (index, value) pairs where indices are into the SPLADE vocabulary.
/// Indices must be:
/// - Sorted in ascending order
/// - Unique (no duplicates)
/// - Within bounds [0, 30521]
///
/// # Example
///
/// ```
/// use context_graph_core::types::fingerprint::sparse::{SparseVector, SPARSE_VOCAB_SIZE};
///
/// // Create a sparse vector with 3 active positions
/// let sv = SparseVector::new(vec![10, 100, 500], vec![0.5, 0.3, 0.8]).unwrap();
/// assert_eq!(sv.nnz(), 3);
/// assert_eq!(sv.memory_size(), 18); // 3*2 + 3*4 bytes
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SparseVector {
    /// Sorted ascending indices into vocabulary [0, 30521]
    pub indices: Vec<u16>,
    /// Corresponding activation values (same length as indices)
    pub values: Vec<f32>,
}

impl SparseVector {
    /// Create new SparseVector with validation.
    ///
    /// # Arguments
    ///
    /// * `indices` - Vocabulary indices, must be sorted ascending without duplicates
    /// * `values` - Activation values, must have same length as indices
    ///
    /// # Errors
    ///
    /// Returns `Err(SparseVectorError)` if:
    /// - `indices.len() != values.len()` (LengthMismatch)
    /// - Any index >= SPARSE_VOCAB_SIZE (IndexOutOfBounds)
    /// - Indices are not sorted ascending (UnsortedOrDuplicate)
    /// - Any duplicate indices exist (UnsortedOrDuplicate)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::types::fingerprint::sparse::SparseVector;
    ///
    /// // Valid construction
    /// let sv = SparseVector::new(vec![1, 10, 100], vec![0.1, 0.2, 0.3]).unwrap();
    ///
    /// // Invalid: mismatched lengths
    /// let err = SparseVector::new(vec![1, 2], vec![0.1]).unwrap_err();
    /// ```
    pub fn new(indices: Vec<u16>, values: Vec<f32>) -> Result<Self, SparseVectorError> {
        // Check 1: Lengths must match
        if indices.len() != values.len() {
            return Err(SparseVectorError::LengthMismatch {
                indices_len: indices.len(),
                values_len: values.len(),
            });
        }

        // Check 2 & 3 & 4: Validate indices are sorted, unique, and in-bounds
        // Using a single pass for efficiency
        let mut prev: Option<u16> = None;
        for &idx in &indices {
            // Bounds check: idx must be < 30522
            if idx as usize >= SPARSE_VOCAB_SIZE {
                return Err(SparseVectorError::IndexOutOfBounds {
                    index: idx as usize,
                    max: SPARSE_VOCAB_SIZE - 1,
                });
            }

            // Sorted and unique check
            if let Some(p) = prev {
                if idx <= p {
                    // idx == p means duplicate, idx < p means unsorted
                    return Err(SparseVectorError::UnsortedOrDuplicate { index: idx });
                }
            }
            prev = Some(idx);
        }

        Ok(Self { indices, values })
    }

    /// Create empty sparse vector.
    ///
    /// Returns a sparse vector with zero active entries.
    /// This is useful for initialization or representing zero vectors.
    #[inline]
    pub fn empty() -> Self {
        Self {
            indices: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Number of active (non-zero) entries.
    ///
    /// This is the number of (index, value) pairs stored, also known as "nnz"
    /// (number of non-zeros) in sparse matrix terminology.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Compute sparse dot product (intersection-based).
    ///
    /// Uses merge-join algorithm to efficiently compute the dot product
    /// of two sparse vectors. Only indices present in both vectors
    /// contribute to the result.
    ///
    /// # Algorithm
    ///
    /// Since both vectors have sorted indices, we can use a two-pointer
    /// technique to find matching indices in O(n + m) time where n and m
    /// are the number of non-zeros in each vector.
    ///
    /// # Returns
    ///
    /// Sum of `self.values[i] * other.values[j]` for all index pairs where
    /// `self.indices[i] == other.indices[j]`.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::types::fingerprint::sparse::SparseVector;
    ///
    /// let a = SparseVector::new(vec![1, 3, 5], vec![1.0, 2.0, 3.0]).unwrap();
    /// let b = SparseVector::new(vec![2, 3, 5], vec![4.0, 5.0, 6.0]).unwrap();
    /// // Intersection at indices 3 and 5: 2.0*5.0 + 3.0*6.0 = 28.0
    /// assert!((a.dot(&b) - 28.0).abs() < 1e-6);
    /// ```
    pub fn dot(&self, other: &Self) -> f32 {
        let mut result = 0.0_f32;
        let mut i = 0;
        let mut j = 0;

        // Two-pointer merge-join on sorted indices
        while i < self.indices.len() && j < other.indices.len() {
            match self.indices[i].cmp(&other.indices[j]) {
                Ordering::Less => i += 1,
                Ordering::Greater => j += 1,
                Ordering::Equal => {
                    result += self.values[i] * other.values[j];
                    i += 1;
                    j += 1;
                }
            }
        }
        result
    }

    /// Memory size in bytes (heap allocation only).
    ///
    /// Calculates the heap memory used by the indices and values vectors.
    /// Does not include the stack size of the SparseVector struct itself.
    ///
    /// # Calculation
    ///
    /// - indices: `nnz * sizeof(u16)` = `nnz * 2` bytes
    /// - values: `nnz * sizeof(f32)` = `nnz * 4` bytes
    /// - total: `nnz * 6` bytes
    #[inline]
    pub fn memory_size(&self) -> usize {
        self.indices.len() * std::mem::size_of::<u16>()
            + self.values.len() * std::mem::size_of::<f32>()
    }

    /// Check if the sparse vector is empty (has no active entries).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Get the value at a specific vocabulary index, if present.
    ///
    /// Uses binary search for O(log n) lookup.
    ///
    /// # Returns
    ///
    /// `Some(value)` if the index is active, `None` otherwise.
    pub fn get(&self, vocab_index: u16) -> Option<f32> {
        self.indices
            .binary_search(&vocab_index)
            .ok()
            .map(|pos| self.values[pos])
    }

    /// Compute the L2 (Euclidean) norm of the sparse vector.
    ///
    /// This is the square root of the sum of squared values.
    pub fn l2_norm(&self) -> f32 {
        self.values.iter().map(|&v| v * v).sum::<f32>().sqrt()
    }

    /// Compute cosine similarity with another sparse vector.
    ///
    /// # Returns
    ///
    /// The cosine similarity in range [-1, 1], or 0.0 if either vector has zero norm.
    pub fn cosine_similarity(&self, other: &Self) -> f32 {
        let dot = self.dot(other);
        let norm_self = self.l2_norm();
        let norm_other = other.l2_norm();

        if norm_self == 0.0 || norm_other == 0.0 {
            0.0
        } else {
            dot / (norm_self * norm_other)
        }
    }
}

impl Default for SparseVector {
    fn default() -> Self {
        Self::empty()
    }
}

/// Errors for SparseVector construction.
///
/// These errors are returned by `SparseVector::new()` when validation fails.
/// All errors contain detailed information about what went wrong.
#[derive(Debug, Clone, PartialEq)]
pub enum SparseVectorError {
    /// The indices and values vectors have different lengths.
    LengthMismatch {
        /// Length of the indices vector
        indices_len: usize,
        /// Length of the values vector
        values_len: usize,
    },

    /// An index exceeds the maximum valid vocabulary index (30521).
    IndexOutOfBounds {
        /// The invalid index value
        index: usize,
        /// The maximum valid index (30521)
        max: usize,
    },

    /// Indices are not in sorted ascending order, or contain duplicates.
    ///
    /// The `index` field contains the problematic index value that violated
    /// the ordering constraint.
    UnsortedOrDuplicate {
        /// The index where the violation was detected
        index: u16,
    },
}

impl fmt::Display for SparseVectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch {
                indices_len,
                values_len,
            } => {
                write!(
                    f,
                    "indices length ({}) != values length ({})",
                    indices_len, values_len
                )
            }
            Self::IndexOutOfBounds { index, max } => {
                write!(f, "index {} exceeds maximum {}", index, max)
            }
            Self::UnsortedOrDuplicate { index } => {
                write!(
                    f,
                    "indices must be sorted ascending without duplicates, failed at {}",
                    index
                )
            }
        }
    }
}

impl Error for SparseVectorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_vector_new_valid() {
        let sv = SparseVector::new(vec![10, 100, 500], vec![0.5, 0.3, 0.8]).unwrap();
        assert_eq!(sv.nnz(), 3);
        assert_eq!(sv.indices, vec![10, 100, 500]);
        assert_eq!(sv.values, vec![0.5, 0.3, 0.8]);
    }

    #[test]
    fn test_sparse_vector_new_empty() {
        let sv = SparseVector::new(vec![], vec![]).unwrap();
        assert_eq!(sv.nnz(), 0);
        assert!(sv.is_empty());
    }

    #[test]
    fn test_sparse_vector_length_mismatch() {
        let result = SparseVector::new(vec![1, 2, 3], vec![0.1, 0.2]);
        assert!(matches!(
            result,
            Err(SparseVectorError::LengthMismatch {
                indices_len: 3,
                values_len: 2,
            })
        ));
    }

    #[test]
    fn test_sparse_vector_index_out_of_bounds() {
        // 30522 is out of bounds (max valid is 30521)
        let result = SparseVector::new(vec![30522], vec![0.5]);
        assert!(matches!(
            result,
            Err(SparseVectorError::IndexOutOfBounds {
                index: 30522,
                max: 30521,
            })
        ));

        // 30521 should be valid (maximum valid index)
        let valid = SparseVector::new(vec![30521], vec![1.0]);
        assert!(valid.is_ok());
    }

    #[test]
    fn test_sparse_vector_unsorted() {
        let result = SparseVector::new(vec![100, 50], vec![0.1, 0.2]);
        assert!(matches!(
            result,
            Err(SparseVectorError::UnsortedOrDuplicate { index: 50 })
        ));
    }

    #[test]
    fn test_sparse_vector_duplicate() {
        let result = SparseVector::new(vec![50, 50], vec![0.1, 0.2]);
        assert!(matches!(
            result,
            Err(SparseVectorError::UnsortedOrDuplicate { index: 50 })
        ));
    }

    #[test]
    fn test_sparse_vector_dot() {
        let a = SparseVector::new(vec![1, 3, 5], vec![1.0, 2.0, 3.0]).unwrap();
        let b = SparseVector::new(vec![2, 3, 5], vec![4.0, 5.0, 6.0]).unwrap();
        // Intersection at indices 3 and 5: 2.0*5.0 + 3.0*6.0 = 10.0 + 18.0 = 28.0
        let dot = a.dot(&b);
        assert!(
            (dot - 28.0).abs() < 1e-6,
            "Expected 28.0, got {}",
            dot
        );
    }

    #[test]
    fn test_sparse_vector_dot_empty() {
        let empty = SparseVector::empty();
        let non_empty = SparseVector::new(vec![1, 2, 3], vec![1.0, 2.0, 3.0]).unwrap();

        // Empty dot anything = 0
        assert_eq!(empty.dot(&empty), 0.0);
        assert_eq!(empty.dot(&non_empty), 0.0);
        assert_eq!(non_empty.dot(&empty), 0.0);
    }

    #[test]
    fn test_sparse_vector_dot_no_intersection() {
        let a = SparseVector::new(vec![1, 3, 5], vec![1.0, 2.0, 3.0]).unwrap();
        let b = SparseVector::new(vec![2, 4, 6], vec![4.0, 5.0, 6.0]).unwrap();
        // No common indices
        assert_eq!(a.dot(&b), 0.0);
    }

    #[test]
    fn test_sparse_vector_memory_size() {
        let sv = SparseVector::new(vec![1, 2, 3], vec![0.1, 0.2, 0.3]).unwrap();
        // 3 u16 = 6 bytes, 3 f32 = 12 bytes, total = 18 bytes
        assert_eq!(sv.memory_size(), 18);

        let empty = SparseVector::empty();
        assert_eq!(empty.memory_size(), 0);
    }

    #[test]
    fn test_sparse_vector_serialization_roundtrip() {
        let sv = SparseVector::new(vec![10, 100, 1000], vec![0.1, 0.5, 0.9]).unwrap();
        let bytes = bincode::serialize(&sv).expect("serialize failed");
        let restored: SparseVector = bincode::deserialize(&bytes).expect("deserialize failed");
        assert_eq!(sv, restored);
    }

    #[test]
    fn test_sparse_vector_empty_serialization() {
        let sv = SparseVector::empty();
        let bytes = bincode::serialize(&sv).expect("serialize failed");
        let restored: SparseVector = bincode::deserialize(&bytes).expect("deserialize failed");
        assert_eq!(sv, restored);
        assert!(restored.is_empty());
    }

    #[test]
    fn test_sparse_vector_get() {
        let sv = SparseVector::new(vec![10, 100, 500], vec![0.5, 0.3, 0.8]).unwrap();

        assert_eq!(sv.get(10), Some(0.5));
        assert_eq!(sv.get(100), Some(0.3));
        assert_eq!(sv.get(500), Some(0.8));
        assert_eq!(sv.get(50), None);
        assert_eq!(sv.get(0), None);
    }

    #[test]
    fn test_sparse_vector_l2_norm() {
        let sv = SparseVector::new(vec![1, 2], vec![3.0, 4.0]).unwrap();
        // sqrt(9 + 16) = sqrt(25) = 5
        let norm = sv.l2_norm();
        assert!((norm - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_sparse_vector_cosine_similarity() {
        // Identical vectors should have cosine similarity 1.0
        let a = SparseVector::new(vec![1, 2, 3], vec![1.0, 2.0, 3.0]).unwrap();
        let cos = a.cosine_similarity(&a);
        assert!((cos - 1.0).abs() < 1e-6);

        // Orthogonal vectors (no overlap) should have cosine similarity 0.0
        let b = SparseVector::new(vec![4, 5, 6], vec![1.0, 2.0, 3.0]).unwrap();
        assert_eq!(a.cosine_similarity(&b), 0.0);
    }

    #[test]
    fn test_sparse_vector_max_index() {
        // Test with maximum valid index (30521)
        let sv = SparseVector::new(vec![0, 30521], vec![0.1, 0.2]).unwrap();
        assert_eq!(sv.nnz(), 2);
        assert_eq!(sv.get(30521), Some(0.2));
    }

    #[test]
    fn test_sparse_vector_error_display() {
        let e1 = SparseVectorError::LengthMismatch {
            indices_len: 3,
            values_len: 2,
        };
        assert_eq!(e1.to_string(), "indices length (3) != values length (2)");

        let e2 = SparseVectorError::IndexOutOfBounds {
            index: 30522,
            max: 30521,
        };
        assert_eq!(e2.to_string(), "index 30522 exceeds maximum 30521");

        let e3 = SparseVectorError::UnsortedOrDuplicate { index: 50 };
        assert_eq!(
            e3.to_string(),
            "indices must be sorted ascending without duplicates, failed at 50"
        );
    }

    #[test]
    fn test_sparse_vector_constants() {
        // Verify constants match expected values from specification
        assert_eq!(SPARSE_VOCAB_SIZE, 30_522);
        assert_eq!(MAX_SPARSE_ACTIVE, 1_526);

        // Verify MAX_SPARSE_ACTIVE is approximately 5% of SPARSE_VOCAB_SIZE
        let five_percent = (SPARSE_VOCAB_SIZE as f64 * 0.05).floor() as usize;
        assert_eq!(MAX_SPARSE_ACTIVE, five_percent);
    }

    #[test]
    fn test_sparse_vector_typical_sparsity() {
        // Create a vector with typical sparsity (~5% active)
        let indices: Vec<u16> = (0..1500_u16).map(|i| i * 20).collect();
        let values: Vec<f32> = vec![0.1; 1500];
        let sv = SparseVector::new(indices, values).unwrap();

        assert_eq!(sv.nnz(), 1500);
        // Memory: 1500 * 2 + 1500 * 4 = 3000 + 6000 = 9000 bytes
        assert_eq!(sv.memory_size(), 9000);
    }
}
