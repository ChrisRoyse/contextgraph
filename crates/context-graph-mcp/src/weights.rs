//! Weight profile configuration for multi-embedding search.
//!
//! Thin wrapper over `context_graph_core::weights` for MCP layer.
//! ERR-3 fix: propagates errors instead of silently discarding them with `.ok()`.

use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

// Re-export space_name for use by other modules in this crate.
#[cfg(test)]
pub use context_graph_core::weights::space_name;

/// Get weight profile by name.
///
/// # Arguments
/// * `name` - Profile name (e.g., "semantic_search", "code_search")
///
/// # Returns
/// The 13-element weight array if found.
///
/// # Errors
/// Returns error if profile name is not found — FAIL FAST, no silent fallback.
pub fn get_weight_profile(name: &str) -> Option<[f32; NUM_EMBEDDERS]> {
    match context_graph_core::weights::get_weight_profile(name) {
        Ok(weights) => Some(weights),
        Err(e) => {
            tracing::error!(
                profile = %name,
                error = %e,
                "Weight profile lookup failed — FAIL FAST"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::weights::{WEIGHT_PROFILES, validate_weights};

    /// Get snake_case key name for JSON serialization (test utility).
    fn space_json_key(idx: usize) -> &'static str {
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

    /// Parse weights from JSON array (test utility).
    fn parse_weights_from_json(
        arr: &[serde_json::Value],
    ) -> Result<[f32; NUM_EMBEDDERS], String> {
        if arr.len() != NUM_EMBEDDERS {
            return Err(format!("Expected {} weights, got {}", NUM_EMBEDDERS, arr.len()));
        }

        let mut weights = [0.0f32; NUM_EMBEDDERS];
        for (i, v) in arr.iter().enumerate() {
            weights[i] = v.as_f64()
                .ok_or_else(|| format!(
                    "Invalid weight at index {} ({}): {:?} is not a number",
                    i, space_name(i), v
                ))? as f32;
        }

        validate_weights(&weights).map_err(|e| e.to_string())?;
        Ok(weights)
    }

    #[test]
    fn test_weight_profiles_count() {
        assert!(
            WEIGHT_PROFILES.len() >= 6,
            "Should have at least 6 predefined profiles"
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
    }

    #[test]
    fn test_graph_reasoning_profile_exists() {
        let weights = get_weight_profile("graph_reasoning").unwrap();
        assert!((weights[7] - 0.40).abs() < 0.001, "E8 Graph should be 0.40");
    }

    #[test]
    fn test_typo_tolerant_profile_exists() {
        assert!(get_weight_profile("typo_tolerant").is_some());
    }

    #[test]
    fn test_typo_tolerant_e9_is_primary() {
        let weights = get_weight_profile("typo_tolerant").unwrap();
        assert!(
            weights[8] >= 0.10,
            "E9 should be >= 0.10 in typo_tolerant (got {})",
            weights[8]
        );

        let mut indexed_weights: Vec<(usize, f32)> = weights.iter().cloned().enumerate().collect();
        indexed_weights.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top_3_indices: Vec<usize> = indexed_weights.iter().take(3).map(|(i, _)| *i).collect();
        assert!(
            top_3_indices.contains(&8),
            "E9 (index 8) should be in top 3 weights for typo_tolerant. Top 3: {:?}",
            top_3_indices
        );
    }

    #[test]
    fn test_temporal_embedders_excluded_from_semantic_profiles() {
        let semantic_profiles = [
            "semantic_search", "causal_reasoning", "code_search", "fact_checking",
            "category_weighted", "typo_tolerant",
            "graph_reasoning"
        ];

        for profile_name in semantic_profiles {
            let weights = get_weight_profile(profile_name).unwrap_or_else(|| {
                panic!("Profile '{}' should exist", profile_name)
            });
            assert_eq!(weights[1], 0.0, "E2 should be 0.0 in '{}'", profile_name);
            assert_eq!(weights[2], 0.0, "E3 should be 0.0 in '{}'", profile_name);
            assert_eq!(weights[3], 0.0, "E4 should be 0.0 in '{}'", profile_name);
        }
    }

    #[test]
    fn test_validate_weights_valid() {
        let valid = get_weight_profile("semantic_search").unwrap();
        assert!(validate_weights(&valid).is_ok());
    }

    #[test]
    fn test_validate_weights_out_of_range() {
        let mut weights = [0.077f32; NUM_EMBEDDERS];
        weights[0] = 1.5;
        let result = validate_weights(&weights);
        assert!(result.is_err(), "Out-of-range weight should fail fast");
    }

    #[test]
    fn test_validate_weights_invalid_sum() {
        let weights = [0.5f32; NUM_EMBEDDERS]; // Sum = 6.5
        let result = validate_weights(&weights);
        assert!(result.is_err(), "Invalid sum should fail fast");
    }

    #[test]
    fn test_space_names() {
        assert_eq!(space_name(0), "E1_Semantic");
        assert_eq!(space_name(12), "E13_SPLADE");
        assert_eq!(space_name(13), "Unknown");
    }

    #[test]
    fn test_space_json_keys() {
        assert_eq!(space_json_key(0), "e1_semantic");
        assert_eq!(space_json_key(12), "e13_splade");
    }

    #[test]
    fn test_parse_weights_from_json_valid() {
        let json_arr: Vec<serde_json::Value> = vec![
            0.28, 0.05, 0.05, 0.05, 0.10, 0.04, 0.18, 0.05, 0.05, 0.05, 0.03, 0.05, 0.02,
        ]
        .into_iter()
        .map(serde_json::Value::from)
        .collect();

        assert!(parse_weights_from_json(&json_arr).is_ok());
    }

    #[test]
    fn test_parse_weights_from_json_wrong_count() {
        let json_arr: Vec<serde_json::Value> = vec![0.5, 0.5]
            .into_iter()
            .map(serde_json::Value::from)
            .collect();

        let result = parse_weights_from_json(&json_arr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected 13 weights, got 2"));
    }

    #[test]
    fn test_sequence_navigation_profile_exists() {
        assert!(get_weight_profile("sequence_navigation").is_some());
    }

    #[test]
    fn test_sequence_navigation_e4_is_primary() {
        let weights = get_weight_profile("sequence_navigation").unwrap();
        assert!(weights[3] >= 0.50, "E4 should be >= 0.50 (got {})", weights[3]);
        let max_weight = weights.iter().cloned().fold(0.0f32, f32::max);
        assert!((weights[3] - max_weight).abs() < 0.001, "E4 should be highest weighted");
    }

    #[test]
    fn test_pipeline_stage1_recall_profile_exists() {
        assert!(get_weight_profile("pipeline_stage1_recall").is_some());
    }

    #[test]
    fn test_pipeline_stage2_scoring_profile_exists() {
        assert!(get_weight_profile("pipeline_stage2_scoring").is_some());
    }

    #[test]
    fn test_pipeline_full_profile_exists() {
        assert!(get_weight_profile("pipeline_full").is_some());
    }
}
