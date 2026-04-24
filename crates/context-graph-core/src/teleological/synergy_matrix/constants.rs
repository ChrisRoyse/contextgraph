//! Constants for the 14x14 cross-embedding synergy matrix.
//!
//! From teleoplan.md (extended for E14 BGE-M3 Dense):
//! - Diagonal is always 1.0 (self-synergy)
//! - Matrix must be symmetric
//! - Values in [0.0, 1.0]
//! - Base synergies: weak (0.3), moderate (0.6), strong (0.9)

/// Number of embedding dimensions in the synergy matrix.
pub const SYNERGY_DIM: usize = 14;

/// Number of unique cross-embedding pairs (upper triangle excluding diagonal).
/// Formula: n * (n - 1) / 2 = 14 * 13 / 2 = 91
pub const CROSS_CORRELATION_COUNT: usize = 91;

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
/// - 13: E14_BgeM3Dense (multilingual semantic/style; strong with E1/E5 semantic, weak elsewhere)
#[rustfmt::skip]
pub const BASE_SYNERGIES: [[f32; SYNERGY_DIM]; SYNERGY_DIM] = [
    // E1_Semantic
    [1.0, 0.6, 0.3, 0.6, 0.9, 0.6, 0.6, 0.3, 0.3, 0.6, 0.9, 0.9, 0.6, 0.9],
    // E2_Episodic
    [0.6, 1.0, 0.9, 0.6, 0.3, 0.3, 0.3, 0.6, 0.9, 0.9, 0.3, 0.6, 0.3, 0.3],
    // E3_Temporal
    [0.3, 0.9, 1.0, 0.9, 0.3, 0.3, 0.9, 0.3, 0.6, 0.3, 0.3, 0.6, 0.3, 0.3],
    // E4_Causal
    [0.6, 0.6, 0.9, 1.0, 0.6, 0.9, 0.9, 0.3, 0.6, 0.3, 0.9, 0.9, 0.3, 0.6],
    // E5_Analogical
    [0.9, 0.3, 0.3, 0.6, 1.0, 0.6, 0.3, 0.6, 0.6, 0.6, 0.9, 0.3, 0.3, 0.9],
    // E6_Code
    [0.6, 0.3, 0.3, 0.9, 0.6, 1.0, 0.9, 0.9, 0.3, 0.3, 0.6, 0.6, 0.9, 0.6],
    // E7_Procedural
    [0.6, 0.3, 0.9, 0.9, 0.3, 0.9, 1.0, 0.6, 0.3, 0.3, 0.3, 0.6, 0.6, 0.6],
    // E8_Spatial
    [0.3, 0.6, 0.3, 0.3, 0.6, 0.9, 0.6, 1.0, 0.6, 0.3, 0.6, 0.3, 0.3, 0.3],
    // E9_Social
    [0.3, 0.9, 0.6, 0.6, 0.6, 0.3, 0.3, 0.6, 1.0, 0.9, 0.3, 0.6, 0.3, 0.3],
    // E10_Emotional
    [0.6, 0.9, 0.3, 0.3, 0.6, 0.3, 0.3, 0.3, 0.9, 1.0, 0.6, 0.3, 0.3, 0.6],
    // E11_Abstract
    [0.9, 0.3, 0.3, 0.9, 0.9, 0.6, 0.3, 0.6, 0.3, 0.6, 1.0, 0.6, 0.3, 0.9],
    // E12_Factual
    [0.9, 0.6, 0.6, 0.9, 0.3, 0.6, 0.6, 0.3, 0.6, 0.3, 0.6, 1.0, 0.9, 0.9],
    // E13_Sparse
    [0.6, 0.3, 0.3, 0.3, 0.3, 0.9, 0.6, 0.3, 0.3, 0.3, 0.3, 0.9, 1.0, 0.6],
    // E14_BgeM3Dense (multilingual dense head — strong semantic pairing across languages)
    [0.9, 0.3, 0.3, 0.6, 0.9, 0.6, 0.6, 0.3, 0.3, 0.6, 0.9, 0.9, 0.6, 1.0],
];
