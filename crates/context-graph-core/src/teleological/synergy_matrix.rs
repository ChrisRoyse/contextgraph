//! 13x13 Cross-embedding synergy matrix for teleological fusion.
//!
//! The synergy matrix captures the strength of relationships between different
//! embedding spaces. High synergy pairs should have their cross-correlations
//! amplified in teleological fusion.
//!
//! From teleoplan.md:
//! - Diagonal is always 1.0 (self-synergy)
//! - Matrix must be symmetric
//! - Values in [0.0, 1.0]
//! - Base synergies: weak (0.3), moderate (0.6), strong (0.9)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::comparison_error::{ComparisonValidationError, ComparisonValidationResult};

/// Number of embedding dimensions in the synergy matrix.
pub const SYNERGY_DIM: usize = 13;

/// Number of unique cross-embedding pairs (upper triangle excluding diagonal).
/// Formula: n * (n - 1) / 2 = 13 * 12 / 2 = 78
pub const CROSS_CORRELATION_COUNT: usize = 78;

/// 13x13 cross-embedding synergy matrix per teleoplan.md.
///
/// Captures the strength of relationships between different embedding spaces.
/// Used for weighting cross-correlations in teleological fusion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SynergyMatrix {
    /// 13x13 matrix of synergy values, symmetric with diagonal = 1.0
    pub values: [[f32; SYNERGY_DIM]; SYNERGY_DIM],
    /// Per-cell weights for adaptive learning
    pub weights: [[f32; SYNERGY_DIM]; SYNERGY_DIM],
    /// When this matrix was computed/updated
    pub computed_at: DateTime<Utc>,
    /// Number of samples used to compute/refine the matrix
    pub sample_count: u64,
}

impl SynergyMatrix {
    /// Base synergies from teleoplan.md synergy matrix.
    ///
    /// Values: weak (0.3), moderate (0.6), strong (0.9)
    ///
    /// Index mapping:
    /// - 0: E1_Semantic
    /// - 1: E2_Episodic
    /// - 2: E3_Temporal
    /// - 3: E4_Causal
    /// - 4: E5_Analogical
    /// - 5: E6_Code
    /// - 6: E7_Procedural
    /// - 7: E8_Spatial
    /// - 8: E9_Social
    /// - 9: E10_Emotional
    /// - 10: E11_Abstract
    /// - 11: E12_Factual
    /// - 12: E13_Sparse
    #[rustfmt::skip]
    pub const BASE_SYNERGIES: [[f32; SYNERGY_DIM]; SYNERGY_DIM] = [
        // E1_Semantic
        [1.0, 0.6, 0.3, 0.6, 0.9, 0.6, 0.6, 0.3, 0.3, 0.6, 0.9, 0.9, 0.6],
        // E2_Episodic
        [0.6, 1.0, 0.9, 0.6, 0.3, 0.3, 0.3, 0.6, 0.9, 0.9, 0.3, 0.6, 0.3],
        // E3_Temporal
        [0.3, 0.9, 1.0, 0.9, 0.3, 0.3, 0.9, 0.3, 0.6, 0.3, 0.3, 0.6, 0.3],
        // E4_Causal
        [0.6, 0.6, 0.9, 1.0, 0.6, 0.9, 0.9, 0.3, 0.6, 0.3, 0.9, 0.9, 0.3],
        // E5_Analogical
        [0.9, 0.3, 0.3, 0.6, 1.0, 0.6, 0.3, 0.6, 0.6, 0.6, 0.9, 0.3, 0.3],
        // E6_Code
        [0.6, 0.3, 0.3, 0.9, 0.6, 1.0, 0.9, 0.9, 0.3, 0.3, 0.6, 0.6, 0.9],
        // E7_Procedural
        [0.6, 0.3, 0.9, 0.9, 0.3, 0.9, 1.0, 0.6, 0.3, 0.3, 0.3, 0.6, 0.6],
        // E8_Spatial
        [0.3, 0.6, 0.3, 0.3, 0.6, 0.9, 0.6, 1.0, 0.6, 0.3, 0.6, 0.3, 0.3],
        // E9_Social
        [0.3, 0.9, 0.6, 0.6, 0.6, 0.3, 0.3, 0.6, 1.0, 0.9, 0.3, 0.6, 0.3],
        // E10_Emotional
        [0.6, 0.9, 0.3, 0.3, 0.6, 0.3, 0.3, 0.3, 0.9, 1.0, 0.6, 0.3, 0.3],
        // E11_Abstract
        [0.9, 0.3, 0.3, 0.9, 0.9, 0.6, 0.3, 0.6, 0.3, 0.6, 1.0, 0.6, 0.3],
        // E12_Factual
        [0.9, 0.6, 0.6, 0.9, 0.3, 0.6, 0.6, 0.3, 0.6, 0.3, 0.6, 1.0, 0.9],
        // E13_Sparse
        [0.6, 0.3, 0.3, 0.3, 0.3, 0.9, 0.6, 0.3, 0.3, 0.3, 0.3, 0.9, 1.0],
    ];

    /// Create a new empty synergy matrix with identity diagonal.
    ///
    /// All synergy values are 0.0 except diagonal which is 1.0.
    pub fn new() -> Self {
        let mut values = [[0.0f32; SYNERGY_DIM]; SYNERGY_DIM];
        let mut weights = [[1.0f32; SYNERGY_DIM]; SYNERGY_DIM];

        // Set diagonal to 1.0
        for (i, (val_row, wgt_row)) in values.iter_mut().zip(weights.iter_mut()).enumerate() {
            val_row[i] = 1.0;
            wgt_row[i] = 1.0;
        }

        Self {
            values,
            weights,
            computed_at: Utc::now(),
            sample_count: 0,
        }
    }

    /// Create a synergy matrix initialized with base synergies from teleoplan.md.
    pub fn with_base_synergies() -> Self {
        Self {
            values: Self::BASE_SYNERGIES,
            weights: [[1.0f32; SYNERGY_DIM]; SYNERGY_DIM],
            computed_at: Utc::now(),
            sample_count: 0,
        }
    }

    // =========================================================================
    // Predefined Matrix Constructors (TASK-CORE-004)
    // =========================================================================
    //
    // These constructors create SynergyMatrix instances optimized for specific
    // use cases. Each modifies the base synergies to emphasize certain
    // embedder pairs.

    /// Create a semantic-focused synergy matrix.
    ///
    /// Emphasizes E1_Semantic relationships with E5_Analogical, E11_Abstract,
    /// and E12_Factual (strong synergies boosted to 0.95).
    ///
    /// Use for: semantic similarity search, meaning extraction, concept matching.
    pub fn semantic_focused() -> Self {
        let mut matrix = Self::with_base_synergies();

        // Boost E1_Semantic pairs
        // E1 + E5_Analogical: strong semantic relationship
        matrix.values[0][4] = 0.95;
        matrix.values[4][0] = 0.95;
        // E1 + E11_Abstract: semantic abstractions
        matrix.values[0][10] = 0.95;
        matrix.values[10][0] = 0.95;
        // E1 + E12_Factual: semantic facts
        matrix.values[0][11] = 0.95;
        matrix.values[11][0] = 0.95;

        // Also boost E5_Analogical + E11_Abstract relationship
        matrix.values[4][10] = 0.95;
        matrix.values[10][4] = 0.95;

        matrix.computed_at = Utc::now();
        matrix
    }

    /// Create a code-heavy synergy matrix.
    ///
    /// Emphasizes E6_Code relationships with E4_Causal, E7_Procedural,
    /// E8_Spatial, and E13_Sparse (code analysis embedders).
    ///
    /// Use for: code search, implementation matching, algorithm similarity.
    pub fn code_heavy() -> Self {
        let mut matrix = Self::with_base_synergies();

        // Boost E6_Code pairs
        // E6 + E4_Causal: code causes effects
        matrix.values[5][3] = 0.95;
        matrix.values[3][5] = 0.95;
        // E6 + E7_Procedural: code is procedural
        matrix.values[5][6] = 0.95;
        matrix.values[6][5] = 0.95;
        // E6 + E8_Spatial: code structure
        matrix.values[5][7] = 0.95;
        matrix.values[7][5] = 0.95;
        // E6 + E13_Sparse: code tokens
        matrix.values[5][12] = 0.95;
        matrix.values[12][5] = 0.95;

        // Also boost E4_Causal + E7_Procedural (logic flow)
        matrix.values[3][6] = 0.95;
        matrix.values[6][3] = 0.95;

        matrix.computed_at = Utc::now();
        matrix
    }

    /// Create a temporal-focused synergy matrix.
    ///
    /// Emphasizes E2_Episodic and E3_Temporal relationships for
    /// sequence-aware retrieval.
    ///
    /// Use for: timeline queries, event sequences, historical context.
    pub fn temporal_focused() -> Self {
        let mut matrix = Self::with_base_synergies();

        // Boost E2_Episodic + E3_Temporal
        matrix.values[1][2] = 0.95;
        matrix.values[2][1] = 0.95;

        // Boost E3_Temporal + E4_Causal (temporal causation)
        matrix.values[2][3] = 0.95;
        matrix.values[3][2] = 0.95;

        // Boost E2_Episodic + E9_Social (episodic social events)
        matrix.values[1][8] = 0.95;
        matrix.values[8][1] = 0.95;

        // Boost E3_Temporal + E7_Procedural (procedure timing)
        matrix.values[2][6] = 0.95;
        matrix.values[6][2] = 0.95;

        matrix.computed_at = Utc::now();
        matrix
    }

    /// Create a causal reasoning synergy matrix.
    ///
    /// Emphasizes E4_Causal relationships for understanding cause-effect.
    ///
    /// Use for: debugging, root cause analysis, impact assessment.
    pub fn causal_reasoning() -> Self {
        let mut matrix = Self::with_base_synergies();

        // Boost E4_Causal pairs
        // E4 + E3_Temporal: causation is temporal
        matrix.values[3][2] = 0.95;
        matrix.values[2][3] = 0.95;
        // E4 + E6_Code: code causes behavior
        matrix.values[3][5] = 0.95;
        matrix.values[5][3] = 0.95;
        // E4 + E7_Procedural: procedures have effects
        matrix.values[3][6] = 0.95;
        matrix.values[6][3] = 0.95;
        // E4 + E11_Abstract: abstract causation
        matrix.values[3][10] = 0.95;
        matrix.values[10][3] = 0.95;
        // E4 + E12_Factual: factual consequences
        matrix.values[3][11] = 0.95;
        matrix.values[11][3] = 0.95;

        matrix.computed_at = Utc::now();
        matrix
    }

    /// Create a relational synergy matrix.
    ///
    /// Emphasizes E5_Analogical, E8_Spatial, and E9_Social for
    /// understanding relationships between entities.
    ///
    /// Use for: knowledge graph queries, entity relationships, social context.
    pub fn relational() -> Self {
        let mut matrix = Self::with_base_synergies();

        // Boost relational group pairs
        // E5_Analogical + E8_Spatial
        matrix.values[4][7] = 0.9;
        matrix.values[7][4] = 0.9;
        // E5_Analogical + E9_Social
        matrix.values[4][8] = 0.9;
        matrix.values[8][4] = 0.9;
        // E8_Spatial + E9_Social
        matrix.values[7][8] = 0.9;
        matrix.values[8][7] = 0.9;

        // Also boost E9_Social + E10_Emotional (social emotions)
        matrix.values[8][9] = 0.95;
        matrix.values[9][8] = 0.95;

        matrix.computed_at = Utc::now();
        matrix
    }

    /// Create a qualitative reasoning synergy matrix.
    ///
    /// Emphasizes E10_Emotional and E11_Abstract for understanding
    /// subjective and abstract concepts.
    ///
    /// Use for: sentiment analysis, opinion mining, conceptual reasoning.
    pub fn qualitative() -> Self {
        let mut matrix = Self::with_base_synergies();

        // Boost qualitative group pairs
        // E10_Emotional + E11_Abstract
        matrix.values[9][10] = 0.9;
        matrix.values[10][9] = 0.9;

        // Also boost E1_Semantic + E10_Emotional (semantic sentiment)
        matrix.values[0][9] = 0.85;
        matrix.values[9][0] = 0.85;

        // E5_Analogical + E10_Emotional (emotional analogies)
        matrix.values[4][9] = 0.85;
        matrix.values[9][4] = 0.85;

        // E9_Social + E10_Emotional (social emotions)
        matrix.values[8][9] = 0.95;
        matrix.values[9][8] = 0.95;

        matrix.computed_at = Utc::now();
        matrix
    }

    /// Create a balanced synergy matrix.
    ///
    /// Uses moderate synergies (0.6) across all pairs for unbiased retrieval.
    ///
    /// Use for: general-purpose search, exploration, discovery.
    pub fn balanced() -> Self {
        let mut values = [[0.6f32; SYNERGY_DIM]; SYNERGY_DIM];

        // Set diagonal to 1.0
        for (i, row) in values.iter_mut().enumerate() {
            row[i] = 1.0;
        }

        Self {
            values,
            weights: [[1.0f32; SYNERGY_DIM]; SYNERGY_DIM],
            computed_at: Utc::now(),
            sample_count: 0,
        }
    }

    /// Create an identity synergy matrix.
    ///
    /// Diagonal is 1.0, all off-diagonal is 0.0 (no cross-embedder synergy).
    ///
    /// Use for: per-embedder independent search, testing, baseline comparison.
    pub fn identity() -> Self {
        Self::new()
    }

    /// Get synergy value between embeddings i and j.
    ///
    /// # Panics
    ///
    /// Panics if `i >= SYNERGY_DIM` or `j >= SYNERGY_DIM` (FAIL FAST).
    #[inline]
    pub fn get_synergy(&self, i: usize, j: usize) -> f32 {
        assert!(
            i < SYNERGY_DIM,
            "FAIL FAST: synergy index i={} out of bounds (max {})",
            i,
            SYNERGY_DIM - 1
        );
        assert!(
            j < SYNERGY_DIM,
            "FAIL FAST: synergy index j={} out of bounds (max {})",
            j,
            SYNERGY_DIM - 1
        );
        self.values[i][j]
    }

    /// Set synergy value between embeddings i and j.
    ///
    /// Automatically maintains symmetry (sets both [i][j] and [j][i]).
    ///
    /// # Panics
    ///
    /// - Panics if `i >= SYNERGY_DIM` or `j >= SYNERGY_DIM` (FAIL FAST)
    /// - Panics if `value < 0.0` or `value > 1.0` (FAIL FAST)
    /// - Panics if attempting to set diagonal to non-1.0 value (FAIL FAST)
    #[inline]
    pub fn set_synergy(&mut self, i: usize, j: usize, value: f32) {
        assert!(
            i < SYNERGY_DIM,
            "FAIL FAST: synergy index i={} out of bounds (max {})",
            i,
            SYNERGY_DIM - 1
        );
        assert!(
            j < SYNERGY_DIM,
            "FAIL FAST: synergy index j={} out of bounds (max {})",
            j,
            SYNERGY_DIM - 1
        );
        assert!(
            (0.0..=1.0).contains(&value),
            "FAIL FAST: synergy value {} must be in [0.0, 1.0]",
            value
        );
        if i == j {
            assert!(
                (value - 1.0).abs() < f32::EPSILON,
                "FAIL FAST: diagonal synergy must be 1.0, got {}",
                value
            );
        }

        self.values[i][j] = value;
        self.values[j][i] = value; // Maintain symmetry
        self.computed_at = Utc::now();
    }

    /// Get weight for synergy between embeddings i and j.
    ///
    /// # Panics
    ///
    /// Panics if `i >= SYNERGY_DIM` or `j >= SYNERGY_DIM` (FAIL FAST).
    #[inline]
    pub fn get_weight(&self, i: usize, j: usize) -> f32 {
        assert!(
            i < SYNERGY_DIM,
            "FAIL FAST: weight index i={} out of bounds (max {})",
            i,
            SYNERGY_DIM - 1
        );
        assert!(
            j < SYNERGY_DIM,
            "FAIL FAST: weight index j={} out of bounds (max {})",
            j,
            SYNERGY_DIM - 1
        );
        self.weights[i][j]
    }

    /// Set weight for synergy between embeddings i and j.
    ///
    /// Automatically maintains symmetry.
    ///
    /// # Panics
    ///
    /// - Panics if indices out of bounds (FAIL FAST)
    /// - Panics if weight is negative (FAIL FAST)
    #[inline]
    pub fn set_weight(&mut self, i: usize, j: usize, weight: f32) {
        assert!(
            i < SYNERGY_DIM,
            "FAIL FAST: weight index i={} out of bounds (max {})",
            i,
            SYNERGY_DIM - 1
        );
        assert!(
            j < SYNERGY_DIM,
            "FAIL FAST: weight index j={} out of bounds (max {})",
            j,
            SYNERGY_DIM - 1
        );
        assert!(
            weight >= 0.0,
            "FAIL FAST: weight {} must be non-negative",
            weight
        );

        self.weights[i][j] = weight;
        self.weights[j][i] = weight; // Maintain symmetry
    }

    /// Check if the matrix is symmetric within tolerance.
    pub fn is_symmetric(&self, tolerance: f32) -> bool {
        for (i, row) in self.values.iter().enumerate() {
            for (j, &value) in row.iter().enumerate().skip(i + 1) {
                if (value - self.values[j][i]).abs() > tolerance {
                    return false;
                }
            }
        }
        true
    }

    /// Check if diagonal values are all 1.0 within tolerance.
    pub fn has_unit_diagonal(&self, tolerance: f32) -> bool {
        for (i, row) in self.values.iter().enumerate() {
            if (row[i] - 1.0).abs() > tolerance {
                return false;
            }
        }
        true
    }

    /// Check if all values are in [0.0, 1.0].
    pub fn values_in_range(&self) -> bool {
        for row in &self.values {
            for &value in row {
                if !(0.0..=1.0).contains(&value) {
                    return false;
                }
            }
        }
        true
    }

    /// Default tolerance for matrix validation
    pub const VALIDATION_TOLERANCE: f32 = 1e-6;

    /// Validate all matrix invariants.
    ///
    /// Returns `Ok(())` if:
    /// - Matrix is symmetric (within tolerance)
    /// - All diagonal values are 1.0 (within tolerance)
    /// - All values are in [0.0, 1.0]
    ///
    /// Returns detailed error describing exactly what failed.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::teleological::SynergyMatrix;
    ///
    /// let valid = SynergyMatrix::with_base_synergies();
    /// assert!(valid.validate().is_ok());
    /// ```
    pub fn validate(&self) -> ComparisonValidationResult<()> {
        self.validate_with_tolerance(Self::VALIDATION_TOLERANCE)
    }

    /// Validate matrix invariants with custom tolerance.
    pub fn validate_with_tolerance(&self, tolerance: f32) -> ComparisonValidationResult<()> {
        // Check symmetry
        for i in 0..SYNERGY_DIM {
            for j in (i + 1)..SYNERGY_DIM {
                let diff = (self.values[i][j] - self.values[j][i]).abs();
                if diff > tolerance {
                    return Err(ComparisonValidationError::MatrixNotSymmetric {
                        row: i,
                        col: j,
                        value_ij: self.values[i][j],
                        value_ji: self.values[j][i],
                        tolerance,
                    });
                }
            }
        }

        // Check diagonal is 1.0
        for i in 0..SYNERGY_DIM {
            if (self.values[i][i] - 1.0).abs() > tolerance {
                return Err(ComparisonValidationError::DiagonalNotUnity {
                    index: i,
                    actual: self.values[i][i],
                    expected: 1.0,
                    tolerance,
                });
            }
        }

        // Check values in range
        for i in 0..SYNERGY_DIM {
            for j in 0..SYNERGY_DIM {
                let value = self.values[i][j];
                if !(0.0..=1.0).contains(&value) {
                    return Err(ComparisonValidationError::SynergyOutOfRange {
                        row: i,
                        col: j,
                        value,
                        min: 0.0,
                        max: 1.0,
                    });
                }
            }
        }

        Ok(())
    }

    /// Check if matrix is valid (returns bool for simple checks).
    ///
    /// For detailed error information, use `validate()` instead.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Assert matrix is valid, panicking with detailed error on failure.
    ///
    /// Use this for cases where validation failure is a programmer error.
    pub fn assert_valid(&self) {
        if let Err(e) = self.validate() {
            panic!("FAIL FAST: SynergyMatrix validation failed: {}", e);
        }
    }

    /// Get weighted synergy value (value * weight).
    #[inline]
    pub fn get_weighted_synergy(&self, i: usize, j: usize) -> f32 {
        self.get_synergy(i, j) * self.get_weight(i, j)
    }

    /// Compute average synergy across all pairs (excluding diagonal).
    pub fn average_synergy(&self) -> f32 {
        let mut sum = 0.0f32;
        let mut count = 0;

        for (i, row) in self.values.iter().enumerate() {
            for &value in row.iter().skip(i + 1) {
                sum += value;
                count += 1;
            }
        }

        if count > 0 {
            sum / count as f32
        } else {
            0.0
        }
    }

    /// Get indices of high synergy pairs (value >= threshold).
    pub fn high_synergy_pairs(&self, threshold: f32) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();
        for (i, row) in self.values.iter().enumerate() {
            for (j, &value) in row.iter().enumerate().skip(i + 1) {
                if value >= threshold {
                    pairs.push((i, j));
                }
            }
        }
        pairs
    }

    /// Flatten upper triangle to cross-correlation array.
    ///
    /// Returns 78 values corresponding to unique pairs (i, j) where i < j.
    /// Order: (0,1), (0,2), ..., (0,12), (1,2), (1,3), ..., (11,12)
    pub fn to_cross_correlations(&self) -> [f32; CROSS_CORRELATION_COUNT] {
        let mut result = [0.0f32; CROSS_CORRELATION_COUNT];
        let mut idx = 0;

        for (i, row) in self.values.iter().enumerate() {
            for &value in row.iter().skip(i + 1) {
                result[idx] = value;
                idx += 1;
            }
        }

        result
    }

    /// Convert flat index (0-77) to matrix indices (i, j).
    ///
    /// # Panics
    ///
    /// Panics if `flat_idx >= CROSS_CORRELATION_COUNT` (FAIL FAST).
    pub fn flat_to_indices(flat_idx: usize) -> (usize, usize) {
        assert!(
            flat_idx < CROSS_CORRELATION_COUNT,
            "FAIL FAST: flat index {} out of bounds (max {})",
            flat_idx,
            CROSS_CORRELATION_COUNT - 1
        );

        // Inverse of triangular number formula
        let mut i = 0;
        let mut offset = 0;

        while offset + (SYNERGY_DIM - 1 - i) <= flat_idx {
            offset += SYNERGY_DIM - 1 - i;
            i += 1;
        }

        let j = flat_idx - offset + i + 1;
        (i, j)
    }

    /// Convert matrix indices (i, j) to flat index (0-77).
    ///
    /// # Panics
    ///
    /// Panics if indices out of bounds or i >= j (FAIL FAST).
    pub fn indices_to_flat(i: usize, j: usize) -> usize {
        assert!(
            i < SYNERGY_DIM && j < SYNERGY_DIM,
            "FAIL FAST: indices ({}, {}) out of bounds",
            i,
            j
        );
        assert!(i < j, "FAIL FAST: i ({}) must be less than j ({})", i, j);

        let mut flat_idx = 0;
        for row in 0..i {
            flat_idx += SYNERGY_DIM - 1 - row;
        }
        flat_idx + (j - i - 1)
    }
}

impl Default for SynergyMatrix {
    fn default() -> Self {
        Self::with_base_synergies()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synergy_matrix_new() {
        let matrix = SynergyMatrix::new();

        // Diagonal should be 1.0
        for i in 0..SYNERGY_DIM {
            assert!(
                (matrix.values[i][i] - 1.0).abs() < f32::EPSILON,
                "Diagonal [{i}][{i}] should be 1.0"
            );
        }

        // Off-diagonal should be 0.0
        for i in 0..SYNERGY_DIM {
            for j in 0..SYNERGY_DIM {
                if i != j {
                    assert!(
                        matrix.values[i][j].abs() < f32::EPSILON,
                        "Off-diagonal [{i}][{j}] should be 0.0"
                    );
                }
            }
        }

        println!("[PASS] SynergyMatrix::new creates identity-diagonal matrix");
    }

    #[test]
    fn test_synergy_matrix_with_base_synergies() {
        let matrix = SynergyMatrix::with_base_synergies();

        // Verify diagonal is 1.0
        for i in 0..SYNERGY_DIM {
            assert!(
                (matrix.values[i][i] - 1.0).abs() < f32::EPSILON,
                "Diagonal [{i}][{i}] should be 1.0"
            );
        }

        // Verify some known synergy values from teleoplan.md
        // E1_Semantic + E5_Analogical = strong (0.9)
        assert!((matrix.get_synergy(0, 4) - 0.9).abs() < f32::EPSILON);
        // E2_Episodic + E3_Temporal = strong (0.9)
        assert!((matrix.get_synergy(1, 2) - 0.9).abs() < f32::EPSILON);
        // E6_Code + E13_Sparse = strong (0.9)
        assert!((matrix.get_synergy(5, 12) - 0.9).abs() < f32::EPSILON);

        println!("[PASS] SynergyMatrix::with_base_synergies matches teleoplan.md");
    }

    #[test]
    fn test_synergy_matrix_symmetry() {
        let matrix = SynergyMatrix::with_base_synergies();

        for i in 0..SYNERGY_DIM {
            for j in 0..SYNERGY_DIM {
                assert!(
                    (matrix.values[i][j] - matrix.values[j][i]).abs() < f32::EPSILON,
                    "Matrix should be symmetric: [{i}][{j}] != [{j}][{i}]"
                );
            }
        }

        assert!(matrix.is_symmetric(f32::EPSILON));
        println!("[PASS] BASE_SYNERGIES matrix is symmetric");
    }

    #[test]
    fn test_synergy_matrix_values_in_range() {
        let matrix = SynergyMatrix::with_base_synergies();

        for i in 0..SYNERGY_DIM {
            for j in 0..SYNERGY_DIM {
                let v = matrix.values[i][j];
                assert!(
                    (0.0..=1.0).contains(&v),
                    "Value [{i}][{j}] = {v} out of range [0.0, 1.0]"
                );
            }
        }

        assert!(matrix.values_in_range());
        println!("[PASS] All synergy values in [0.0, 1.0]");
    }

    #[test]
    fn test_synergy_matrix_get_set_synergy() {
        let mut matrix = SynergyMatrix::new();

        matrix.set_synergy(0, 5, 0.7);

        assert!((matrix.get_synergy(0, 5) - 0.7).abs() < f32::EPSILON);
        assert!((matrix.get_synergy(5, 0) - 0.7).abs() < f32::EPSILON); // Symmetric

        println!("[PASS] set_synergy maintains symmetry");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_synergy_matrix_get_out_of_bounds_i() {
        let matrix = SynergyMatrix::new();
        let _ = matrix.get_synergy(13, 0);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_synergy_matrix_get_out_of_bounds_j() {
        let matrix = SynergyMatrix::new();
        let _ = matrix.get_synergy(0, 13);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_synergy_matrix_set_value_too_high() {
        let mut matrix = SynergyMatrix::new();
        matrix.set_synergy(0, 1, 1.5);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_synergy_matrix_set_value_negative() {
        let mut matrix = SynergyMatrix::new();
        matrix.set_synergy(0, 1, -0.1);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_synergy_matrix_set_diagonal_not_one() {
        let mut matrix = SynergyMatrix::new();
        matrix.set_synergy(3, 3, 0.5);
    }

    #[test]
    fn test_synergy_matrix_validate() {
        use crate::teleological::comparison_error::ComparisonValidationError;

        // Valid matrix should pass
        let matrix = SynergyMatrix::with_base_synergies();
        assert!(matrix.validate().is_ok(), "Base synergies should be valid");
        assert!(matrix.is_valid(), "is_valid() should return true");

        // All predefined matrices should be valid
        assert!(SynergyMatrix::semantic_focused().validate().is_ok());
        assert!(SynergyMatrix::code_heavy().validate().is_ok());
        assert!(SynergyMatrix::temporal_focused().validate().is_ok());
        assert!(SynergyMatrix::causal_reasoning().validate().is_ok());
        assert!(SynergyMatrix::relational().validate().is_ok());
        assert!(SynergyMatrix::qualitative().validate().is_ok());
        assert!(SynergyMatrix::balanced().validate().is_ok());
        assert!(SynergyMatrix::identity().validate().is_ok());

        // Test invalid matrix: asymmetric
        let mut asymmetric = SynergyMatrix::with_base_synergies();
        asymmetric.values[0][5] = 0.8; // Only change one direction
        let err = asymmetric.validate();
        assert!(err.is_err(), "Asymmetric matrix should fail");
        match err {
            Err(ComparisonValidationError::MatrixNotSymmetric { row, col, .. }) => {
                assert_eq!(row, 0);
                assert_eq!(col, 5);
                println!(
                    "  Got expected MatrixNotSymmetric error at [{}, {}]",
                    row, col
                );
            }
            _ => panic!("Expected MatrixNotSymmetric error"),
        }

        // Test invalid matrix: bad diagonal
        let mut bad_diag = SynergyMatrix::with_base_synergies();
        bad_diag.values[3][3] = 0.5;
        let err = bad_diag.validate();
        assert!(err.is_err(), "Bad diagonal should fail");
        match err {
            Err(ComparisonValidationError::DiagonalNotUnity { index, actual, .. }) => {
                assert_eq!(index, 3);
                assert!((actual - 0.5).abs() < f32::EPSILON);
                println!("  Got expected DiagonalNotUnity error at index {}", index);
            }
            _ => panic!("Expected DiagonalNotUnity error"),
        }

        // Test invalid matrix: out of range
        let mut out_of_range = SynergyMatrix::with_base_synergies();
        out_of_range.values[2][7] = 1.5;
        out_of_range.values[7][2] = 1.5; // Keep symmetric
        let err = out_of_range.validate();
        assert!(err.is_err(), "Out of range value should fail");
        match err {
            Err(ComparisonValidationError::SynergyOutOfRange {
                row, col, value, ..
            }) => {
                assert_eq!(value, 1.5);
                println!(
                    "  Got expected SynergyOutOfRange error at [{}, {}] = {}",
                    row, col, value
                );
            }
            _ => panic!("Expected SynergyOutOfRange error"),
        }

        println!("[PASS] validate() returns correct Results");
    }

    #[test]
    fn test_synergy_matrix_average_synergy() {
        let matrix = SynergyMatrix::with_base_synergies();
        let avg = matrix.average_synergy();

        // Average should be in (0, 1)
        assert!(avg > 0.0 && avg < 1.0);

        // Count unique pairs: 13 * 12 / 2 = 78
        // Verify by manual calculation
        let mut sum = 0.0f32;
        for i in 0..SYNERGY_DIM {
            for j in (i + 1)..SYNERGY_DIM {
                sum += matrix.values[i][j];
            }
        }
        let expected = sum / CROSS_CORRELATION_COUNT as f32;

        assert!((avg - expected).abs() < f32::EPSILON);
        println!("[PASS] average_synergy = {avg:.4}");
    }

    #[test]
    fn test_synergy_matrix_high_synergy_pairs() {
        let matrix = SynergyMatrix::with_base_synergies();
        let high_pairs = matrix.high_synergy_pairs(0.9);

        // Should include (0, 4) = E1_Semantic + E5_Analogical
        assert!(high_pairs.contains(&(0, 4)));
        // Should include (1, 2) = E2_Episodic + E3_Temporal
        assert!(high_pairs.contains(&(1, 2)));

        println!(
            "[PASS] high_synergy_pairs(0.9) found {} pairs",
            high_pairs.len()
        );
    }

    #[test]
    fn test_synergy_matrix_to_cross_correlations() {
        let matrix = SynergyMatrix::with_base_synergies();
        let cross = matrix.to_cross_correlations();

        assert_eq!(cross.len(), CROSS_CORRELATION_COUNT);

        // Verify first few values
        // (0,1) = E1_Semantic + E2_Episodic = 0.6
        assert!((cross[0] - 0.6).abs() < f32::EPSILON);
        // (0,2) = E1_Semantic + E3_Temporal = 0.3
        assert!((cross[1] - 0.3).abs() < f32::EPSILON);

        println!("[PASS] to_cross_correlations produces 78 values");
    }

    #[test]
    fn test_synergy_matrix_flat_to_indices() {
        // First pair (0, 1)
        assert_eq!(SynergyMatrix::flat_to_indices(0), (0, 1));
        // Second pair (0, 2)
        assert_eq!(SynergyMatrix::flat_to_indices(1), (0, 2));
        // Last of first row (0, 12)
        assert_eq!(SynergyMatrix::flat_to_indices(11), (0, 12));
        // First of second row (1, 2)
        assert_eq!(SynergyMatrix::flat_to_indices(12), (1, 2));
        // Last pair (11, 12)
        assert_eq!(SynergyMatrix::flat_to_indices(77), (11, 12));

        println!("[PASS] flat_to_indices conversions correct");
    }

    #[test]
    fn test_synergy_matrix_indices_to_flat() {
        assert_eq!(SynergyMatrix::indices_to_flat(0, 1), 0);
        assert_eq!(SynergyMatrix::indices_to_flat(0, 2), 1);
        assert_eq!(SynergyMatrix::indices_to_flat(0, 12), 11);
        assert_eq!(SynergyMatrix::indices_to_flat(1, 2), 12);
        assert_eq!(SynergyMatrix::indices_to_flat(11, 12), 77);

        println!("[PASS] indices_to_flat conversions correct");
    }

    #[test]
    fn test_synergy_matrix_roundtrip_indices() {
        // Test all 78 pairs roundtrip correctly
        for flat in 0..CROSS_CORRELATION_COUNT {
            let (i, j) = SynergyMatrix::flat_to_indices(flat);
            let back = SynergyMatrix::indices_to_flat(i, j);
            assert_eq!(
                flat, back,
                "Roundtrip failed: {} -> ({}, {}) -> {}",
                flat, i, j, back
            );
        }

        println!("[PASS] All 78 index roundtrips correct");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_synergy_matrix_flat_to_indices_out_of_bounds() {
        let _ = SynergyMatrix::flat_to_indices(78);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_synergy_matrix_indices_to_flat_invalid() {
        let _ = SynergyMatrix::indices_to_flat(5, 3); // i must be < j
    }

    #[test]
    fn test_synergy_matrix_weights() {
        let mut matrix = SynergyMatrix::new();

        // Default weight is 1.0
        assert!((matrix.get_weight(0, 1) - 1.0).abs() < f32::EPSILON);

        // Set weight
        matrix.set_weight(0, 1, 2.5);
        assert!((matrix.get_weight(0, 1) - 2.5).abs() < f32::EPSILON);
        assert!((matrix.get_weight(1, 0) - 2.5).abs() < f32::EPSILON); // Symmetric

        println!("[PASS] Weight get/set works correctly");
    }

    #[test]
    fn test_synergy_matrix_weighted_synergy() {
        let mut matrix = SynergyMatrix::with_base_synergies();
        matrix.set_weight(0, 4, 2.0);

        // E1_Semantic + E5_Analogical = 0.9, weight = 2.0
        let weighted = matrix.get_weighted_synergy(0, 4);
        assert!((weighted - 1.8).abs() < f32::EPSILON);

        println!("[PASS] get_weighted_synergy = {weighted}");
    }

    #[test]
    fn test_synergy_matrix_serialization() {
        let matrix = SynergyMatrix::with_base_synergies();
        let json = serde_json::to_string(&matrix).unwrap();
        let deserialized: SynergyMatrix = serde_json::from_str(&json).unwrap();

        assert_eq!(matrix.sample_count, deserialized.sample_count);
        assert!((matrix.get_synergy(0, 4) - deserialized.get_synergy(0, 4)).abs() < f32::EPSILON);

        println!("[PASS] Serialization roundtrip successful");
    }

    #[test]
    fn test_synergy_matrix_default() {
        let matrix = SynergyMatrix::default();

        // Default should use base synergies
        assert!((matrix.get_synergy(0, 4) - 0.9).abs() < f32::EPSILON);

        println!("[PASS] Default uses base synergies");
    }

    // =========================================================================
    // Predefined Constructor Tests (TASK-CORE-004)
    // =========================================================================

    #[test]
    fn test_semantic_focused_constructor() {
        let matrix = SynergyMatrix::semantic_focused();

        // Matrix should be valid
        assert!(
            matrix.validate().is_ok(),
            "semantic_focused should produce valid matrix"
        );

        // Verify boosted synergies
        // E1_Semantic (0) + E5_Analogical (4) should be 0.95
        assert!(
            (matrix.get_synergy(0, 4) - 0.95).abs() < f32::EPSILON,
            "E1+E5 should be boosted to 0.95"
        );
        // E1_Semantic (0) + E11_Abstract (10) should be 0.95
        assert!(
            (matrix.get_synergy(0, 10) - 0.95).abs() < f32::EPSILON,
            "E1+E11 should be boosted to 0.95"
        );
        // E1_Semantic (0) + E12_Factual (11) should be 0.95
        assert!(
            (matrix.get_synergy(0, 11) - 0.95).abs() < f32::EPSILON,
            "E1+E12 should be boosted to 0.95"
        );
        // E5_Analogical (4) + E11_Abstract (10) should be 0.95
        assert!(
            (matrix.get_synergy(4, 10) - 0.95).abs() < f32::EPSILON,
            "E5+E11 should be boosted to 0.95"
        );

        // Verify symmetry
        assert!(matrix.is_symmetric(f32::EPSILON));
        // Verify diagonal
        assert!(matrix.has_unit_diagonal(f32::EPSILON));

        println!("[PASS] semantic_focused constructor creates valid boosted matrix");
    }

    #[test]
    fn test_code_heavy_constructor() {
        let matrix = SynergyMatrix::code_heavy();

        // Matrix should be valid
        assert!(
            matrix.validate().is_ok(),
            "code_heavy should produce valid matrix"
        );

        // Verify boosted synergies
        // E6_Code (5) + E4_Causal (3) should be 0.95
        assert!(
            (matrix.get_synergy(5, 3) - 0.95).abs() < f32::EPSILON,
            "E6+E4 should be boosted to 0.95"
        );
        // E6_Code (5) + E7_Procedural (6) should be 0.95
        assert!(
            (matrix.get_synergy(5, 6) - 0.95).abs() < f32::EPSILON,
            "E6+E7 should be boosted to 0.95"
        );
        // E6_Code (5) + E8_Spatial (7) should be 0.95
        assert!(
            (matrix.get_synergy(5, 7) - 0.95).abs() < f32::EPSILON,
            "E6+E8 should be boosted to 0.95"
        );
        // E6_Code (5) + E13_Sparse (12) should be 0.95
        assert!(
            (matrix.get_synergy(5, 12) - 0.95).abs() < f32::EPSILON,
            "E6+E13 should be boosted to 0.95"
        );

        // Verify symmetry
        assert!(matrix.is_symmetric(f32::EPSILON));

        println!("[PASS] code_heavy constructor creates valid boosted matrix");
    }

    #[test]
    fn test_temporal_focused_constructor() {
        let matrix = SynergyMatrix::temporal_focused();

        assert!(
            matrix.validate().is_ok(),
            "temporal_focused should produce valid matrix"
        );

        // E2_Episodic (1) + E3_Temporal (2) should be 0.95
        assert!(
            (matrix.get_synergy(1, 2) - 0.95).abs() < f32::EPSILON,
            "E2+E3 should be boosted to 0.95"
        );
        // E3_Temporal (2) + E4_Causal (3) should be 0.95
        assert!(
            (matrix.get_synergy(2, 3) - 0.95).abs() < f32::EPSILON,
            "E3+E4 should be boosted to 0.95"
        );

        println!("[PASS] temporal_focused constructor creates valid boosted matrix");
    }

    #[test]
    fn test_causal_reasoning_constructor() {
        let matrix = SynergyMatrix::causal_reasoning();

        assert!(
            matrix.validate().is_ok(),
            "causal_reasoning should produce valid matrix"
        );

        // E4_Causal (3) should have multiple strong synergies
        assert!(
            (matrix.get_synergy(3, 2) - 0.95).abs() < f32::EPSILON,
            "E4+E3 should be boosted to 0.95"
        );
        assert!(
            (matrix.get_synergy(3, 5) - 0.95).abs() < f32::EPSILON,
            "E4+E6 should be boosted to 0.95"
        );
        assert!(
            (matrix.get_synergy(3, 6) - 0.95).abs() < f32::EPSILON,
            "E4+E7 should be boosted to 0.95"
        );
        assert!(
            (matrix.get_synergy(3, 10) - 0.95).abs() < f32::EPSILON,
            "E4+E11 should be boosted to 0.95"
        );
        assert!(
            (matrix.get_synergy(3, 11) - 0.95).abs() < f32::EPSILON,
            "E4+E12 should be boosted to 0.95"
        );

        println!("[PASS] causal_reasoning constructor creates valid boosted matrix");
    }

    #[test]
    fn test_relational_constructor() {
        let matrix = SynergyMatrix::relational();

        assert!(
            matrix.validate().is_ok(),
            "relational should produce valid matrix"
        );

        // E5_Analogical (4), E8_Spatial (7), E9_Social (8) should have boosted pairs
        assert!(
            (matrix.get_synergy(4, 7) - 0.9).abs() < f32::EPSILON,
            "E5+E8 should be 0.9"
        );
        assert!(
            (matrix.get_synergy(4, 8) - 0.9).abs() < f32::EPSILON,
            "E5+E9 should be 0.9"
        );
        assert!(
            (matrix.get_synergy(7, 8) - 0.9).abs() < f32::EPSILON,
            "E8+E9 should be 0.9"
        );
        // E9_Social (8) + E10_Emotional (9) should be 0.95
        assert!(
            (matrix.get_synergy(8, 9) - 0.95).abs() < f32::EPSILON,
            "E9+E10 should be boosted to 0.95"
        );

        println!("[PASS] relational constructor creates valid boosted matrix");
    }

    #[test]
    fn test_qualitative_constructor() {
        let matrix = SynergyMatrix::qualitative();

        assert!(
            matrix.validate().is_ok(),
            "qualitative should produce valid matrix"
        );

        // E10_Emotional (9) + E11_Abstract (10) should be 0.9
        assert!(
            (matrix.get_synergy(9, 10) - 0.9).abs() < f32::EPSILON,
            "E10+E11 should be 0.9"
        );
        // E9_Social (8) + E10_Emotional (9) should be 0.95
        assert!(
            (matrix.get_synergy(8, 9) - 0.95).abs() < f32::EPSILON,
            "E9+E10 should be boosted to 0.95"
        );

        println!("[PASS] qualitative constructor creates valid boosted matrix");
    }

    #[test]
    fn test_balanced_constructor() {
        let matrix = SynergyMatrix::balanced();

        assert!(
            matrix.validate().is_ok(),
            "balanced should produce valid matrix"
        );

        // All off-diagonal values should be 0.6
        for i in 0..SYNERGY_DIM {
            for j in 0..SYNERGY_DIM {
                if i == j {
                    assert!(
                        (matrix.get_synergy(i, j) - 1.0).abs() < f32::EPSILON,
                        "Diagonal should be 1.0"
                    );
                } else {
                    assert!(
                        (matrix.get_synergy(i, j) - 0.6).abs() < f32::EPSILON,
                        "Off-diagonal [{}, {}] should be 0.6",
                        i,
                        j
                    );
                }
            }
        }

        println!("[PASS] balanced constructor creates valid uniform 0.6 matrix");
    }

    #[test]
    fn test_identity_constructor() {
        let matrix = SynergyMatrix::identity();

        assert!(
            matrix.validate().is_ok(),
            "identity should produce valid matrix"
        );

        // Diagonal should be 1.0, off-diagonal should be 0.0
        for i in 0..SYNERGY_DIM {
            for j in 0..SYNERGY_DIM {
                if i == j {
                    assert!(
                        (matrix.get_synergy(i, j) - 1.0).abs() < f32::EPSILON,
                        "Diagonal should be 1.0"
                    );
                } else {
                    assert!(
                        matrix.get_synergy(i, j).abs() < f32::EPSILON,
                        "Off-diagonal [{}, {}] should be 0.0",
                        i,
                        j
                    );
                }
            }
        }

        println!("[PASS] identity constructor creates valid identity matrix");
    }

    #[test]
    fn test_all_predefined_matrices_are_valid() {
        // Comprehensive validation of all predefined matrices
        let matrices = [
            ("base", SynergyMatrix::with_base_synergies()),
            ("semantic_focused", SynergyMatrix::semantic_focused()),
            ("code_heavy", SynergyMatrix::code_heavy()),
            ("temporal_focused", SynergyMatrix::temporal_focused()),
            ("causal_reasoning", SynergyMatrix::causal_reasoning()),
            ("relational", SynergyMatrix::relational()),
            ("qualitative", SynergyMatrix::qualitative()),
            ("balanced", SynergyMatrix::balanced()),
            ("identity", SynergyMatrix::identity()),
        ];

        for (name, matrix) in matrices.iter() {
            // Validate returns Ok
            let result = matrix.validate();
            assert!(
                result.is_ok(),
                "{} matrix validation failed: {:?}",
                name,
                result
            );

            // is_valid returns true
            assert!(
                matrix.is_valid(),
                "{} matrix is_valid() returned false",
                name
            );

            // Symmetry check
            assert!(
                matrix.is_symmetric(f32::EPSILON),
                "{} matrix is not symmetric",
                name
            );

            // Diagonal check
            assert!(
                matrix.has_unit_diagonal(f32::EPSILON),
                "{} matrix has non-unity diagonal",
                name
            );

            // Range check
            assert!(
                matrix.values_in_range(),
                "{} matrix has values out of range",
                name
            );

            println!(
                "  {} matrix: valid, symmetric, unit diagonal, in range",
                name
            );
        }

        println!("[PASS] All predefined matrices pass comprehensive validation");
    }

    #[test]
    fn test_predefined_matrix_properties() {
        // Test that semantic_focused has higher average synergy for semantic embedders
        let semantic = SynergyMatrix::semantic_focused();
        let base = SynergyMatrix::with_base_synergies();

        // E1_Semantic row average should be higher in semantic_focused
        let semantic_e1_avg: f32 = (0..SYNERGY_DIM)
            .filter(|&j| j != 0)
            .map(|j| semantic.get_synergy(0, j))
            .sum::<f32>()
            / 12.0;
        let base_e1_avg: f32 = (0..SYNERGY_DIM)
            .filter(|&j| j != 0)
            .map(|j| base.get_synergy(0, j))
            .sum::<f32>()
            / 12.0;

        assert!(
            semantic_e1_avg > base_e1_avg,
            "semantic_focused E1 average ({}) should be higher than base ({})",
            semantic_e1_avg,
            base_e1_avg
        );

        // Test that code_heavy has higher average synergy for code embedders
        let code = SynergyMatrix::code_heavy();

        // E6_Code row average should be higher in code_heavy
        let code_e6_avg: f32 = (0..SYNERGY_DIM)
            .filter(|&j| j != 5)
            .map(|j| code.get_synergy(5, j))
            .sum::<f32>()
            / 12.0;
        let base_e6_avg: f32 = (0..SYNERGY_DIM)
            .filter(|&j| j != 5)
            .map(|j| base.get_synergy(5, j))
            .sum::<f32>()
            / 12.0;

        assert!(
            code_e6_avg > base_e6_avg,
            "code_heavy E6 average ({}) should be higher than base ({})",
            code_e6_avg,
            base_e6_avg
        );

        println!("[PASS] Predefined matrices have expected statistical properties");
    }
}
