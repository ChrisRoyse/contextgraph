//! BatchComparator for parallel processing of fingerprint comparisons.
//!
//! Provides efficient parallel comparison using rayon for scenarios where
//! many fingerprints need to be compared simultaneously.

use rayon::prelude::*;
use tracing::warn;

use crate::teleological::{ComparisonValidationResult, MatrixSearchConfig};
use crate::types::SemanticFingerprint;

use super::result::ComparisonResult;
use super::teleological::TeleologicalComparator;

/// Batch comparator for parallel processing using rayon.
///
/// Provides efficient parallel comparison for scenarios where
/// many fingerprints need to be compared simultaneously.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::teleological::BatchComparator;
///
/// let batch = BatchComparator::new();
/// let results = batch.compare_one_to_many(&reference, &targets);
/// ```
#[derive(Debug, Clone)]
pub struct BatchComparator {
    comparator: TeleologicalComparator,
}

impl Default for BatchComparator {
    fn default() -> Self {
        Self::new()
    }
}

impl BatchComparator {
    /// Create with default configuration.
    pub fn new() -> Self {
        Self {
            comparator: TeleologicalComparator::new(),
        }
    }

    /// Create with specific configuration.
    pub fn with_config(config: MatrixSearchConfig) -> Self {
        Self {
            comparator: TeleologicalComparator::with_config(config),
        }
    }

    /// Get the underlying comparator.
    pub fn comparator(&self) -> &TeleologicalComparator {
        &self.comparator
    }

    /// Compare one reference against many targets in parallel.
    ///
    /// Uses rayon for parallel iteration, distributing comparisons
    /// across all available CPU cores.
    pub fn compare_one_to_many(
        &self,
        reference: &SemanticFingerprint,
        targets: &[SemanticFingerprint],
    ) -> Vec<ComparisonValidationResult<ComparisonResult>> {
        targets
            .par_iter()
            .map(|target| self.comparator.compare(reference, target))
            .collect()
    }

    /// Compare many-to-many in parallel, returns similarity matrix.
    ///
    /// Returns a Vec<Vec<f32>> where result[i][j] is the similarity
    /// between fingerprints[i] and fingerprints[j].
    ///
    /// The matrix is symmetric (result[i][j] == result[j][i]).
    /// Diagonal elements are 1.0 (self-similarity).
    pub fn compare_all_pairs(&self, fingerprints: &[SemanticFingerprint]) -> Vec<Vec<f32>> {
        let n = fingerprints.len();
        if n == 0 {
            return Vec::new();
        }

        // Initialize matrix with 1.0 on diagonal
        let mut matrix: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
        for (i, row) in matrix.iter_mut().enumerate() {
            row[i] = 1.0;
        }

        // Compute upper triangle in parallel, then mirror
        let pairs: Vec<(usize, usize)> = (0..n)
            .flat_map(|i| ((i + 1)..n).map(move |j| (i, j)))
            .collect();

        let similarities: Vec<((usize, usize), f32)> = pairs
            .par_iter()
            .map(|&(i, j)| {
                let sim = self
                    .comparator
                    .compare(&fingerprints[i], &fingerprints[j])
                    .map(|r| r.overall)
                    .unwrap_or(0.0);
                ((i, j), sim)
            })
            .collect();

        // Fill matrix (symmetric)
        for ((i, j), sim) in similarities {
            matrix[i][j] = sim;
            matrix[j][i] = sim;
        }

        matrix
    }

    /// Compare one reference against many, returning only scores above threshold.
    ///
    /// Returns pairs of (index, similarity) for targets exceeding min_similarity.
    pub fn compare_above_threshold(
        &self,
        reference: &SemanticFingerprint,
        targets: &[SemanticFingerprint],
        min_similarity: f32,
    ) -> Vec<(usize, f32)> {
        targets
            .par_iter()
            .enumerate()
            .filter_map(|(idx, target)| {
                match self.comparator.compare(reference, target) {
                    Ok(r) if r.overall >= min_similarity => Some((idx, r.overall)),
                    Ok(_) => None, // Below threshold
                    Err(e) => {
                        warn!(index = idx, error = %e, "Batch comparison failed — skipping pair");
                        None
                    }
                }
            })
            .collect()
    }
}
