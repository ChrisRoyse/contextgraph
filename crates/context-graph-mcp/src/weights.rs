//! Weight profile configuration for multi-embedding search.
//!
//! Re-exports core weight profiles and adds MCP-specific utilities.
//!
//! # 13 Embedding Spaces
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

// Re-export core weight profiles and functions
pub use context_graph_core::weights::{
    WEIGHT_PROFILES, WeightProfileError,
    get_profile_names, validate_weights as core_validate_weights, space_name,
};

/// Get weight profile by name.
///
/// # Arguments
/// * `name` - Profile name (e.g., "semantic_search", "code_search")
///
/// # Returns
/// The 13-element weight array if found, None otherwise.
///
/// # Note
/// This is a thin wrapper over `context_graph_core::weights::get_weight_profile`
/// that converts `Result<...>` to `Option<...>` for backwards compatibility.
pub fn get_weight_profile(name: &str) -> Option<[f32; NUM_EMBEDDERS]> {
    context_graph_core::weights::get_weight_profile(name).ok()
}

/// Validate that weights sum to ~1.0 and all are in [0.0, 1.0].
///
/// # FAIL FAST
/// Returns detailed error on validation failure.
pub(crate) fn validate_weights(weights: &[f32; NUM_EMBEDDERS]) -> Result<(), WeightValidationError> {
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

/// Get snake_case key name for JSON serialization.
pub(crate) fn space_json_key(idx: usize) -> &'static str {
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
pub(crate) enum WeightValidationError {
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
    /// Invalid weight value (not a number).
    InvalidValue {
        index: usize,
        space_name: &'static str,
        value: serde_json::Value,
    },
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
            Self::InvalidValue { index, space_name, value } => {
                write!(
                    f,
                    "Invalid weight at index {} ({}): {:?} is not a number",
                    index, space_name, value
                )
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
/// # Errors (FAIL FAST)
/// - `WrongCount`: Array has wrong number of elements
/// - `InvalidValue`: A value is not a number (NO SILENT 0.0 FALLBACK)
/// - `OutOfRange`: A weight is outside [0.0, 1.0]
/// - `InvalidSum`: Weights don't sum to 1.0
pub(crate) fn parse_weights_from_json(
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
        // FAIL FAST: Reject non-numeric values instead of silently using 0.0
        weights[i] = v.as_f64()
            .ok_or_else(|| WeightValidationError::InvalidValue {
                index: i,
                space_name: space_name(i),
                value: v.clone(),
            })? as f32;
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
            (semantic.unwrap()[0] - 0.33).abs() < 0.001,
            "E1 should be 0.33 in semantic_search profile"
        );

        let missing = get_weight_profile("nonexistent");
        assert!(missing.is_none(), "Unknown profile should return None");

        println!("[VERIFIED] get_weight_profile works correctly");
    }

    #[test]
    fn test_graph_reasoning_profile_exists() {
        let weights = get_weight_profile("graph_reasoning");
        assert!(weights.is_some(), "graph_reasoning profile should exist");

        let weights = weights.unwrap();
        assert!((weights[7] - 0.40).abs() < 0.001, "E8 Graph should be 0.40");
        println!("[VERIFIED] graph_reasoning profile exists with E8={:.2}", weights[7]);
    }

    #[test]
    fn test_typo_tolerant_profile_exists() {
        let weights = get_weight_profile("typo_tolerant");
        assert!(weights.is_some(), "typo_tolerant profile should exist");
        println!("[VERIFIED] typo_tolerant profile exists");
    }

    #[test]
    fn test_typo_tolerant_e9_is_primary() {
        // E9 should have significant weight in typo_tolerant profile
        let weights = get_weight_profile("typo_tolerant").unwrap();

        // E9 should be >= 0.10 (substantial contribution)
        assert!(
            weights[8] >= 0.10,
            "E9 should be >= 0.10 in typo_tolerant (got {})",
            weights[8]
        );

        // E9 should be one of the highest weighted (top 3)
        let mut indexed_weights: Vec<(usize, f32)> = weights.iter().cloned().enumerate().collect();
        indexed_weights.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top_3_indices: Vec<usize> = indexed_weights.iter().take(3).map(|(i, _)| *i).collect();
        assert!(
            top_3_indices.contains(&8),
            "E9 (index 8) should be in top 3 weights for typo_tolerant. Top 3: {:?}",
            top_3_indices
        );

        println!(
            "[VERIFIED] typo_tolerant has E9={:.2} as primary structural embedder",
            weights[8]
        );
    }

    #[test]
    fn test_temporal_embedders_excluded_from_semantic_profiles() {
        let semantic_profiles = [
            "semantic_search", "causal_reasoning", "code_search", "fact_checking",
            "category_weighted", "intent_search", "intent_enhanced", "typo_tolerant",
            "graph_reasoning"
        ];

        for profile_name in semantic_profiles {
            let weights = get_weight_profile(profile_name).expect(&format!(
                "Profile '{}' should exist",
                profile_name
            ));

            assert_eq!(
                weights[1], 0.0,
                "E2 (temporal recent) should be 0.0 in '{}' profile per AP-71",
                profile_name
            );
            assert_eq!(
                weights[2], 0.0,
                "E3 (temporal periodic) should be 0.0 in '{}' profile per AP-71",
                profile_name
            );
            assert_eq!(
                weights[3], 0.0,
                "E4 (temporal positional) should be 0.0 in '{}' profile per AP-71",
                profile_name
            );

            println!(
                "[VERIFIED] Profile '{}' has temporal embedders (E2-E4) = 0.0",
                profile_name
            );
        }
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

    #[test]
    fn test_sequence_navigation_profile_exists() {
        let weights = get_weight_profile("sequence_navigation");
        assert!(
            weights.is_some(),
            "sequence_navigation profile should exist"
        );
        println!("[VERIFIED] sequence_navigation profile exists");
    }

    #[test]
    fn test_sequence_navigation_e4_is_primary() {
        // E4 should be the PRIMARY embedder for sequence navigation
        let weights = get_weight_profile("sequence_navigation").unwrap();

        // E4 should be >= 0.50 (dominant)
        assert!(
            weights[3] >= 0.50,
            "E4 should be >= 0.50 in sequence_navigation (got {})",
            weights[3]
        );

        // E4 should be the highest weighted embedder
        let max_weight = weights.iter().cloned().fold(0.0f32, f32::max);
        assert!(
            (weights[3] - max_weight).abs() < 0.001,
            "E4 should be highest weighted in sequence_navigation"
        );

        println!(
            "[VERIFIED] sequence_navigation has E4={:.2} as primary",
            weights[3]
        );
    }

    #[test]
    fn test_intent_search_profile_exists() {
        let weights = get_weight_profile("intent_search");
        assert!(weights.is_some(), "intent_search profile should exist");
        println!("[VERIFIED] intent_search profile exists");
    }

    #[test]
    fn test_intent_enhanced_profile_exists() {
        let weights = get_weight_profile("intent_enhanced");
        assert!(weights.is_some(), "intent_enhanced profile should exist");
        println!("[VERIFIED] intent_enhanced profile exists");
    }

    #[test]
    fn test_pipeline_stage1_recall_profile_exists() {
        let weights = get_weight_profile("pipeline_stage1_recall");
        assert!(
            weights.is_some(),
            "pipeline_stage1_recall profile should exist"
        );
        println!("[VERIFIED] pipeline_stage1_recall profile exists");
    }

    #[test]
    fn test_pipeline_stage2_scoring_profile_exists() {
        let weights = get_weight_profile("pipeline_stage2_scoring");
        assert!(
            weights.is_some(),
            "pipeline_stage2_scoring profile should exist"
        );
        println!("[VERIFIED] pipeline_stage2_scoring profile exists");
    }

    #[test]
    fn test_pipeline_full_profile_exists() {
        let weights = get_weight_profile("pipeline_full");
        assert!(weights.is_some(), "pipeline_full profile should exist");
        println!("[VERIFIED] pipeline_full profile exists");
    }
}
