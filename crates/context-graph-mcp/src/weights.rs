//! Weight profile configuration for multi-embedding search.
//!
//! TASK-S002: Provides predefined weight profiles for 13 embedding spaces.
//!
//! # 13 Embedding Spaces (NOT 12)
//!
//! | Index | Name | Purpose |
//! |-------|------|---------|
//! | 0 | E1_Semantic | General semantic similarity |
//! | 1 | E2_Temporal_Recent | Recent time proximity |
//! | 2 | E3_Temporal_Periodic | Recurring patterns |
//! | 3 | E4_Temporal_Positional | Document position encoding |
//! | 4 | E5_Causal | Cause-effect relationships |
//! | 5 | E6_Sparse | Keyword-level matching |
//! | 6 | E7_Code | Source code similarity |
//! | 7 | E8_Graph | Node2Vec structural |
//! | 8 | E9_HDC | Hyperdimensional computing |
//! | 9 | E10_Multimodal | Cross-modal alignment |
//! | 10 | E11_Entity | Named entity matching |
//! | 11 | E12_Late_Interaction | ColBERT-style token matching |
//! | 12 | E13_SPLADE | Sparse learned expansion (Stage 1) |
//!
//! # Error Handling
//!
//! FAIL FAST: Invalid weights return detailed error immediately.

use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

/// Predefined weight profiles per query type.
///
/// Each profile has 13 weights corresponding to:
/// [E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13]
///
/// E13 (SPLADE) is typically weighted lower in final aggregation
/// since it's primarily used in Stage 1 pre-filtering.
pub const WEIGHT_PROFILES: &[(&str, [f32; NUM_EMBEDDERS])] = &[
    // Semantic Search: Heavy E1 (general semantic), moderate E7 (code), E5 (causal)
    (
        "semantic_search",
        [
            0.28, // E1_Semantic (primary)
            0.05, // E2_Temporal_Recent
            0.05, // E3_Temporal_Periodic
            0.05, // E4_Temporal_Positional
            0.10, // E5_Causal
            0.04, // E6_Sparse
            0.18, // E7_Code
            0.05, // E8_Graph
            0.05, // E9_HDC
            0.05, // E10_Multimodal
            0.03, // E11_Entity
            0.05, // E12_Late_Interaction
            0.02, // E13_SPLADE (low - used in Stage 1 filtering)
        ],
    ),
    // Causal Reasoning: Heavy E5 (causal), moderate E1, E8 (graph)
    (
        "causal_reasoning",
        [
            0.15, // E1_Semantic
            0.03, // E2_Temporal_Recent
            0.03, // E3_Temporal_Periodic
            0.03, // E4_Temporal_Positional
            0.40, // E5_Causal (primary)
            0.03, // E6_Sparse
            0.10, // E7_Code
            0.08, // E8_Graph
            0.03, // E9_HDC
            0.05, // E10_Multimodal
            0.03, // E11_Entity
            0.02, // E12_Late_Interaction
            0.02, // E13_SPLADE
        ],
    ),
    // Code Search: Heavy E7 (code), E4 (positional), E1
    (
        "code_search",
        [
            0.15, // E1_Semantic
            0.02, // E2_Temporal_Recent
            0.02, // E3_Temporal_Periodic
            0.15, // E4_Temporal_Positional (line numbers, structure)
            0.05, // E5_Causal
            0.05, // E6_Sparse
            0.35, // E7_Code (primary)
            0.02, // E8_Graph
            0.02, // E9_HDC
            0.05, // E10_Multimodal
            0.05, // E11_Entity
            0.05, // E12_Late_Interaction
            0.02, // E13_SPLADE
        ],
    ),
    // Temporal Navigation: Heavy E2, E3, E4 (all temporal)
    (
        "temporal_navigation",
        [
            0.12, // E1_Semantic
            0.22, // E2_Temporal_Recent (primary)
            0.22, // E3_Temporal_Periodic (primary)
            0.22, // E4_Temporal_Positional (primary)
            0.03, // E5_Causal
            0.02, // E6_Sparse
            0.03, // E7_Code
            0.02, // E8_Graph
            0.03, // E9_HDC
            0.03, // E10_Multimodal
            0.02, // E11_Entity
            0.02, // E12_Late_Interaction
            0.02, // E13_SPLADE
        ],
    ),
    // Fact Checking: Heavy E11 (entity), E5 (causal), E6 (sparse)
    (
        "fact_checking",
        [
            0.10, // E1_Semantic
            0.02, // E2_Temporal_Recent
            0.02, // E3_Temporal_Periodic
            0.02, // E4_Temporal_Positional
            0.18, // E5_Causal
            0.10, // E6_Sparse (keyword matching)
            0.05, // E7_Code
            0.05, // E8_Graph
            0.02, // E9_HDC
            0.05, // E10_Multimodal
            0.35, // E11_Entity (primary - named entities)
            0.02, // E12_Late_Interaction
            0.02, // E13_SPLADE
        ],
    ),
    // Balanced: Equal weights across all 13 spaces
    (
        "balanced",
        [
            0.077, 0.077, 0.077, 0.077, 0.077, 0.077, 0.077, 0.077, 0.077, 0.077, 0.077, 0.077,
            0.076, // E13 slightly lower to sum to 1.0
        ],
    ),
];

/// Get weight profile by name.
///
/// # Arguments
/// * `name` - Profile name (e.g., "semantic_search", "code_search")
///
/// # Returns
/// The 13-element weight array if found, None otherwise.
pub fn get_weight_profile(name: &str) -> Option<[f32; NUM_EMBEDDERS]> {
    WEIGHT_PROFILES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, w)| *w)
}

/// Get all available profile names.
pub fn get_profile_names() -> Vec<&'static str> {
    WEIGHT_PROFILES.iter().map(|(n, _)| *n).collect()
}

/// Validate that weights sum to ~1.0 and all are in [0.0, 1.0].
///
/// # FAIL FAST
/// Returns detailed error on validation failure.
pub fn validate_weights(weights: &[f32; NUM_EMBEDDERS]) -> Result<(), WeightValidationError> {
    // Check each weight is in range
    for (i, &w) in weights.iter().enumerate() {
        if !(0.0..=1.0).contains(&w) {
            return Err(WeightValidationError::OutOfRange {
                space_index: i,
                space_name: space_name(i),
                value: w,
            });
        }
    }

    // Check sum is ~1.0
    let sum: f32 = weights.iter().sum();
    if (sum - 1.0).abs() > 0.01 {
        return Err(WeightValidationError::InvalidSum {
            expected: 1.0,
            actual: sum,
            weights: weights.to_vec(),
        });
    }

    Ok(())
}

/// Get space name by index.
pub fn space_name(idx: usize) -> &'static str {
    match idx {
        0 => "E1_Semantic",
        1 => "E2_Temporal_Recent",
        2 => "E3_Temporal_Periodic",
        3 => "E4_Temporal_Positional",
        4 => "E5_Causal",
        5 => "E6_Sparse",
        6 => "E7_Code",
        7 => "E8_Graph",
        8 => "E9_HDC",
        9 => "E10_Multimodal",
        10 => "E11_Entity",
        11 => "E12_Late_Interaction",
        12 => "E13_SPLADE",
        _ => "Unknown",
    }
}

/// Get snake_case key name for JSON serialization.
pub fn space_json_key(idx: usize) -> &'static str {
    match idx {
        0 => "e1_semantic",
        1 => "e2_temporal_recent",
        2 => "e3_temporal_periodic",
        3 => "e4_temporal_positional",
        4 => "e5_causal",
        5 => "e6_sparse",
        6 => "e7_code",
        7 => "e8_graph",
        8 => "e9_hdc",
        9 => "e10_multimodal",
        10 => "e11_entity",
        11 => "e12_late_interaction",
        12 => "e13_splade",
        _ => "unknown",
    }
}

/// Weight validation error.
///
/// Provides detailed context for FAIL FAST error handling.
#[derive(Debug, Clone)]
pub enum WeightValidationError {
    /// A weight is outside the valid range [0.0, 1.0].
    OutOfRange {
        space_index: usize,
        space_name: &'static str,
        value: f32,
    },
    /// Weights do not sum to 1.0.
    InvalidSum {
        expected: f32,
        actual: f32,
        weights: Vec<f32>,
    },
    /// Wrong number of weights provided.
    WrongCount { expected: usize, actual: usize },
}

impl std::fmt::Display for WeightValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange {
                space_index,
                space_name,
                value,
            } => {
                write!(
                    f,
                    "Weight for space {} ({}) is out of range [0.0, 1.0]: {}",
                    space_index, space_name, value
                )
            }
            Self::InvalidSum {
                expected,
                actual,
                weights,
            } => {
                write!(
                    f,
                    "Weights must sum to {}, got {}. Weights: {:?}",
                    expected, actual, weights
                )
            }
            Self::WrongCount { expected, actual } => {
                write!(f, "Expected {} weights, got {}", expected, actual)
            }
        }
    }
}

impl std::error::Error for WeightValidationError {}

/// Parse weights from JSON array.
///
/// # Arguments
/// * `arr` - JSON array of 13 numeric weights
///
/// # Returns
/// Validated weight array.
///
/// # Errors
/// - `WrongCount`: Array has wrong number of elements
/// - `OutOfRange`: A weight is outside [0.0, 1.0]
/// - `InvalidSum`: Weights don't sum to 1.0
pub fn parse_weights_from_json(
    arr: &[serde_json::Value],
) -> Result<[f32; NUM_EMBEDDERS], WeightValidationError> {
    if arr.len() != NUM_EMBEDDERS {
        return Err(WeightValidationError::WrongCount {
            expected: NUM_EMBEDDERS,
            actual: arr.len(),
        });
    }

    let mut weights = [0.0f32; NUM_EMBEDDERS];
    for (i, v) in arr.iter().enumerate() {
        weights[i] = v.as_f64().unwrap_or(0.0) as f32;
    }

    validate_weights(&weights)?;
    Ok(weights)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_profiles_count() {
        assert!(
            WEIGHT_PROFILES.len() >= 6,
            "Should have at least 6 predefined profiles"
        );
        println!(
            "[VERIFIED] WEIGHT_PROFILES has {} profiles",
            WEIGHT_PROFILES.len()
        );
    }

    #[test]
    fn test_all_profiles_sum_to_one() {
        for (name, weights) in WEIGHT_PROFILES {
            let sum: f32 = weights.iter().sum();
            assert!(
                (sum - 1.0).abs() < 0.01,
                "Profile '{}' weights sum to {} (expected ~1.0)",
                name,
                sum
            );
            println!("[VERIFIED] Profile '{}' sums to {:.4}", name, sum);
        }
    }

    #[test]
    fn test_all_profiles_have_13_weights() {
        for (name, weights) in WEIGHT_PROFILES {
            assert_eq!(
                weights.len(),
                13,
                "Profile '{}' should have 13 weights",
                name
            );
        }
        println!("[VERIFIED] All profiles have exactly 13 weights");
    }

    #[test]
    fn test_get_weight_profile() {
        let semantic = get_weight_profile("semantic_search");
        assert!(semantic.is_some(), "semantic_search profile should exist");
        assert!(
            (semantic.unwrap()[0] - 0.28).abs() < 0.001,
            "E1 should be 0.28"
        );

        let missing = get_weight_profile("nonexistent");
        assert!(missing.is_none(), "Unknown profile should return None");

        println!("[VERIFIED] get_weight_profile works correctly");
    }

    #[test]
    fn test_validate_weights_valid() {
        let valid = get_weight_profile("semantic_search").unwrap();
        assert!(
            validate_weights(&valid).is_ok(),
            "Valid profile should pass validation"
        );
        println!("[VERIFIED] Valid weights pass validation");
    }

    #[test]
    fn test_validate_weights_out_of_range() {
        let mut weights = [0.077f32; NUM_EMBEDDERS];
        weights[0] = 1.5; // Out of range

        let result = validate_weights(&weights);
        assert!(result.is_err());

        match result.unwrap_err() {
            WeightValidationError::OutOfRange { space_index, .. } => {
                assert_eq!(space_index, 0);
            }
            _ => panic!("Expected OutOfRange error"),
        }
        println!("[VERIFIED] Out-of-range weight fails fast");
    }

    #[test]
    fn test_validate_weights_invalid_sum() {
        let weights = [0.5f32; NUM_EMBEDDERS]; // Sum = 6.5

        let result = validate_weights(&weights);
        assert!(result.is_err());

        match result.unwrap_err() {
            WeightValidationError::InvalidSum { actual, .. } => {
                assert!((actual - 6.5).abs() < 0.01);
            }
            _ => panic!("Expected InvalidSum error"),
        }
        println!("[VERIFIED] Invalid sum fails fast");
    }

    #[test]
    fn test_space_names() {
        assert_eq!(space_name(0), "E1_Semantic");
        assert_eq!(space_name(12), "E13_SPLADE");
        assert_eq!(space_name(13), "Unknown");
        println!("[VERIFIED] space_name returns correct names");
    }

    #[test]
    fn test_space_json_keys() {
        assert_eq!(space_json_key(0), "e1_semantic");
        assert_eq!(space_json_key(12), "e13_splade");
        println!("[VERIFIED] space_json_key returns correct keys");
    }

    #[test]
    fn test_parse_weights_from_json_valid() {
        let json_arr: Vec<serde_json::Value> = vec![
            0.28, 0.05, 0.05, 0.05, 0.10, 0.04, 0.18, 0.05, 0.05, 0.05, 0.03, 0.05, 0.02,
        ]
        .into_iter()
        .map(serde_json::Value::from)
        .collect();

        let result = parse_weights_from_json(&json_arr);
        assert!(result.is_ok());
        println!("[VERIFIED] parse_weights_from_json works for valid input");
    }

    #[test]
    fn test_parse_weights_from_json_wrong_count() {
        let json_arr: Vec<serde_json::Value> = vec![0.5, 0.5]
            .into_iter()
            .map(serde_json::Value::from)
            .collect();

        let result = parse_weights_from_json(&json_arr);
        assert!(result.is_err());

        match result.unwrap_err() {
            WeightValidationError::WrongCount { expected, actual } => {
                assert_eq!(expected, 13);
                assert_eq!(actual, 2);
            }
            _ => panic!("Expected WrongCount error"),
        }
        println!("[VERIFIED] Wrong count fails with clear error");
    }
}
