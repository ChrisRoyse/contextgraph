//! Weight profile configuration for multi-embedding search.
//!
//! This module provides predefined weight profiles for 13 embedding spaces.
//! Moved from MCP crate to Core crate to allow Storage layer access.
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
//! FAIL FAST: Invalid weights or unknown profiles return detailed errors immediately.

use crate::types::fingerprint::NUM_EMBEDDERS;

/// Weight profile error types.
///
/// Provides detailed context for FAIL FAST error handling.
#[derive(Debug, Clone)]
pub enum WeightProfileError {
    /// Unknown profile name requested.
    UnknownProfile {
        /// The requested profile name.
        name: String,
        /// List of available profile names.
        available: Vec<&'static str>,
    },
    /// A weight is outside the valid range [0.0, 1.0].
    OutOfRange {
        /// Index of the problematic weight.
        index: usize,
        /// Name of the embedding space.
        space_name: &'static str,
        /// The invalid value.
        value: f32,
    },
    /// Weights do not sum to ~1.0.
    InvalidSum {
        /// The actual sum.
        actual: f32,
    },
}

impl std::fmt::Display for WeightProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownProfile { name, available } => {
                write!(
                    f,
                    "Unknown weight profile '{}'. Available profiles: {}",
                    name,
                    available.join(", ")
                )
            }
            Self::OutOfRange {
                index,
                space_name,
                value,
            } => {
                write!(
                    f,
                    "Weight for space {} ({}) is out of range [0.0, 1.0]: {}",
                    index, space_name, value
                )
            }
            Self::InvalidSum { actual } => {
                write!(f, "Weights must sum to ~1.0, got {}", actual)
            }
        }
    }
}

impl std::error::Error for WeightProfileError {}

/// Predefined weight profiles per query type.
///
/// Each profile has 13 weights corresponding to:
/// [E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13]
///
/// # IMPORTANT: Temporal Embedders (E2-E4)
///
/// Per AP-71 and research findings:
/// **Temporal embedders (E2-E4) have weight 0.0 in semantic search profiles.**
///
/// Temporal proximity != topical similarity. Documents created at the same time
/// are NOT necessarily on the same topic. Time should be a post-retrieval boost,
/// not a similarity measure.
///
/// # Profile Categories
///
/// - **Semantic Profiles** (E2-E4 = 0.0): semantic_search, code_search, causal_reasoning, fact_checking
/// - **Special Profiles**: temporal_navigation (for explicit time-based queries)
/// - **Category-Weighted**: category_weighted (constitution-compliant)
///
/// # IMPORTANT: Pipeline-Stage Embedders (E12, E13) - per ARCH-13
///
/// E12 (Late Interaction) and E13 (SPLADE) have weight 0.0 in ALL semantic scoring profiles
/// because they're used in specific pipeline stages, NOT for similarity scoring:
/// - E13: Stage 1 recall ONLY (inverted index) - per AP-74
/// - E12: Stage 3 re-ranking ONLY (MaxSim) - per AP-73
pub const WEIGHT_PROFILES: &[(&str, [f32; NUM_EMBEDDERS])] = &[
    // =========================================================================
    // SEMANTIC PROFILES - Temporal (E2-E4) = 0.0 per AP-71
    // =========================================================================

    // Semantic Search: General queries - E1 primary, E5/E7/E10 supporting
    (
        "semantic_search",
        [
            0.33, // E1_Semantic (primary)
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.15, // E5_Causal
            0.05, // E6_Sparse (keyword backup)
            0.20, // E7_Code
            0.05, // E8_Graph (relational)
            0.02, // E9_HDC (noise-robust backup for typo tolerance)
            0.15, // E10_Multimodal
            0.05, // E11_Entity (relational)
            0.0,  // E12_Late_Interaction (Stage 3 rerank only)
            0.0,  // E13_SPLADE (Stage 1 recall only)
        ],
    ),

    // Causal Reasoning: "Why" questions - E5 primary
    (
        "causal_reasoning",
        [
            0.20, // E1_Semantic
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.45, // E5_Causal (primary)
            0.05, // E6_Sparse
            0.10, // E7_Code
            0.10, // E8_Graph (causal chains)
            0.0,  // E9_HDC
            0.05, // E10_Multimodal
            0.05, // E11_Entity
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
        ],
    ),

    // Code Search: Programming queries - E7 primary
    (
        "code_search",
        [
            0.20, // E1_Semantic
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional
            0.10, // E5_Causal
            0.10, // E6_Sparse (keywords)
            0.40, // E7_Code (primary)
            0.0,  // E8_Graph
            0.0,  // E9_HDC
            0.10, // E10_Multimodal
            0.10, // E11_Entity (function names, etc.)
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
        ],
    ),

    // Fact Checking: Entity/fact queries - E11 primary, E6 for keywords
    (
        "fact_checking",
        [
            0.15, // E1_Semantic
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.15, // E5_Causal
            0.15, // E6_Sparse (keyword match)
            0.05, // E7_Code
            0.05, // E8_Graph
            0.0,  // E9_HDC
            0.05, // E10_Multimodal
            0.40, // E11_Entity (primary - named entities)
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
        ],
    ),

    // Intent Search: "What was the goal?" queries - E1 primary, E10 via multiplicative boost
    // ARCH-17 COMPLIANT: E10 is 0.0 in weighted fusion (uses multiplicative boost instead)
    (
        "intent_search",
        [
            0.55, // E1_Semantic (foundation per ARCH-12, boosted)
            0.0,  // E2_Temporal_Recent - NOT for semantic search per AP-71
            0.0,  // E3_Temporal_Periodic - NOT for semantic search per AP-71
            0.0,  // E4_Temporal_Positional - NOT for semantic search per AP-71
            0.12, // E5_Causal (intent often has causal structure)
            0.05, // E6_Sparse (keyword backup)
            0.18, // E7_Code (code intent/purpose, boosted)
            0.05, // E8_Graph (relational)
            0.0,  // E9_HDC
            0.0,  // E10_Multimodal - NOW VIA MULTIPLICATIVE BOOST (ARCH-17)
            0.05, // E11_Entity (entities in intent)
            0.0,  // E12_Late_Interaction (Stage 3 rerank only per AP-73)
            0.0,  // E13_SPLADE (Stage 1 recall only per AP-74)
        ],
    ),

    // Intent Enhanced: E1 primary foundation, E10 via stronger multiplicative boost
    // ARCH-17 COMPLIANT: E10 uses IntentBoostConfig::aggressive()
    (
        "intent_enhanced",
        [
            0.55, // E1_Semantic (foundation per ARCH-12, boosted)
            0.0,  // E2_Temporal_Recent - NOT for semantic search per AP-71
            0.0,  // E3_Temporal_Periodic - NOT for semantic search per AP-71
            0.0,  // E4_Temporal_Positional - NOT for semantic search per AP-71
            0.12, // E5_Causal (intent often has causal structure)
            0.05, // E6_Sparse (keyword backup)
            0.18, // E7_Code (code intent/purpose, boosted)
            0.05, // E8_Graph (relational)
            0.0,  // E9_HDC
            0.0,  // E10_Multimodal - NOW VIA MULTIPLICATIVE BOOST (ARCH-17)
            0.05, // E11_Entity (entities in intent)
            0.0,  // E12_Late_Interaction (Stage 3 rerank only per AP-73)
            0.0,  // E13_SPLADE (Stage 1 recall only per AP-74)
        ],
    ),

    // =========================================================================
    // GRAPH REASONING PROFILE - E8 primary for structural queries
    // =========================================================================

    // Graph Reasoning: Structural/connectivity queries - E8 primary
    // Use for: "what imports X?", "what uses X?", "what connects to X?"
    (
        "graph_reasoning",
        [
            0.15, // E1_Semantic
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.10, // E5_Causal
            0.10, // E6_Sparse
            0.0,  // E7_Code
            0.40, // E8_Graph (primary)
            0.0,  // E9_HDC
            0.05, // E10_Multimodal
            0.20, // E11_Entity
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
        ],
    ),

    // =========================================================================
    // SPECIAL PROFILES
    // =========================================================================

    // Temporal Navigation: EXPLICIT time-based queries only
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

    // =========================================================================
    // SEQUENCE NAVIGATION PROFILES - E4 focused
    // =========================================================================

    // Sequence Navigation: For explicit sequence traversal queries
    (
        "sequence_navigation",
        [
            0.20, // E1_Semantic (semantic backup)
            0.05, // E2_Temporal_Recent (mild recency signal)
            0.0,  // E3_Temporal_Periodic (no periodic patterns for sequence)
            0.55, // E4_Temporal_Positional (PRIMARY - sequence ordering)
            0.03, // E5_Causal
            0.02, // E6_Sparse
            0.03, // E7_Code
            0.02, // E8_Graph
            0.03, // E9_HDC
            0.03, // E10_Multimodal
            0.02, // E11_Entity
            0.0,  // E12_Late_Interaction (pipeline stage only)
            0.02, // E13_SPLADE
        ],
    ),

    // Conversation History: Balanced E4 + E1 for contextual recall
    (
        "conversation_history",
        [
            0.30, // E1_Semantic (topic matching)
            0.05, // E2_Temporal_Recent (recent context helps)
            0.0,  // E3_Temporal_Periodic
            0.35, // E4_Temporal_Positional (conversation ordering)
            0.10, // E5_Causal (causal chains in conversation)
            0.03, // E6_Sparse
            0.05, // E7_Code
            0.02, // E8_Graph
            0.0,  // E9_HDC
            0.05, // E10_Multimodal
            0.03, // E11_Entity
            0.0,  // E12_Late_Interaction (pipeline stage only)
            0.02, // E13_SPLADE
        ],
    ),

    // Category-Weighted: Constitution-compliant weights per CLAUDE.md and ARCH-13
    // max_weighted_agreement = 6.5 (5*1.0 + 3*0.5 = 6.5)
    (
        "category_weighted",
        [
            1.0 / 6.5,   // E1_Semantic (SEMANTIC)
            0.0,         // E2_Temporal_Recent (TEMPORAL - excluded per AP-60)
            0.0,         // E3_Temporal_Periodic (TEMPORAL - excluded per AP-60)
            0.0,         // E4_Temporal_Positional (TEMPORAL - excluded per AP-60)
            1.0 / 6.5,   // E5_Causal (SEMANTIC)
            1.0 / 6.5,   // E6_Sparse (SEMANTIC)
            1.0 / 6.5,   // E7_Code (SEMANTIC)
            0.5 / 6.5,   // E8_Graph (RELATIONAL)
            0.5 / 6.5,   // E9_HDC (STRUCTURAL)
            1.0 / 6.5,   // E10_Multimodal (SEMANTIC)
            0.5 / 6.5,   // E11_Entity (RELATIONAL)
            0.0,         // E12_Late_Interaction (PIPELINE-STAGE)
            0.0,         // E13_SPLADE (PIPELINE-STAGE)
        ],
    ),

    // =========================================================================
    // TYPO-TOLERANT PROFILE - E9 primary for noisy queries
    // =========================================================================

    // Typo Tolerant: For queries with potential spelling errors
    (
        "typo_tolerant",
        [
            0.30, // E1_Semantic (reduced - query might be noisy)
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.10, // E5_Causal
            0.05, // E6_Sparse (keyword backup for exact matches)
            0.15, // E7_Code (reduced to make room for E9)
            0.03, // E8_Graph (relational)
            0.15, // E9_HDC (PRIMARY for typo tolerance)
            0.12, // E10_Multimodal
            0.05, // E11_Entity (relational)
            0.03, // E12_Late_Interaction (can help with phrase matching)
            0.02, // E13_SPLADE (term expansion helps with variations)
        ],
    ),

    // =========================================================================
    // PIPELINE-AWARE PROFILES - Phase 5 E12/E13 Integration
    // =========================================================================

    // Pipeline Stage 1 Recall: E13-heavy for sparse retrieval
    (
        "pipeline_stage1_recall",
        [
            0.20, // E1_Semantic (backup for semantic overlap)
            0.0,  // E2_Temporal_Recent - NOT for recall stage
            0.0,  // E3_Temporal_Periodic - NOT for recall stage
            0.0,  // E4_Temporal_Positional - NOT for recall stage
            0.05, // E5_Causal (minimal)
            0.25, // E6_Sparse (keyword matching, supports E13)
            0.10, // E7_Code (for code queries)
            0.0,  // E8_Graph
            0.05, // E9_HDC (typo tolerance helps recall)
            0.05, // E10_Multimodal
            0.05, // E11_Entity (entity names)
            0.0,  // E12_Late_Interaction (Stage 3 rerank only per AP-73)
            0.25, // E13_SPLADE (PRIMARY - term expansion for recall)
        ],
    ),

    // Pipeline Stage 2 Scoring: E1-heavy for dense candidate scoring
    (
        "pipeline_stage2_scoring",
        [
            0.50, // E1_Semantic (PRIMARY - semantic foundation per ARCH-12)
            0.0,  // E2_Temporal_Recent - NOT for scoring stage
            0.0,  // E3_Temporal_Periodic - NOT for scoring stage
            0.0,  // E4_Temporal_Positional - NOT for scoring stage
            0.12, // E5_Causal (causal relationships)
            0.05, // E6_Sparse (keyword precision)
            0.15, // E7_Code (code understanding)
            0.03, // E8_Graph (relational)
            0.02, // E9_HDC (noise tolerance)
            0.08, // E10_Multimodal (intent alignment via boost)
            0.05, // E11_Entity (entity matching)
            0.0,  // E12_Late_Interaction (Stage 3 rerank only per AP-73)
            0.0,  // E13_SPLADE (Stage 1 recall only per AP-74)
        ],
    ),

    // Pipeline Full: Combined profile for complete pipeline execution
    (
        "pipeline_full",
        [
            0.40, // E1_Semantic (strong foundation)
            0.0,  // E2_Temporal_Recent - NOT for pipeline
            0.0,  // E3_Temporal_Periodic - NOT for pipeline
            0.0,  // E4_Temporal_Positional - NOT for pipeline
            0.10, // E5_Causal
            0.10, // E6_Sparse (keyword precision)
            0.15, // E7_Code
            0.03, // E8_Graph
            0.02, // E9_HDC
            0.08, // E10_Multimodal
            0.05, // E11_Entity
            0.0,  // E12_Late_Interaction (applied via MaxSim, not fusion)
            0.07, // E13_SPLADE (mild weight for fusion awareness)
        ],
    ),

    // Balanced: Equal weights across all 13 spaces (for testing/comparison)
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
/// The 13-element weight array if found.
///
/// # Errors
/// Returns `WeightProfileError::UnknownProfile` if the profile name is not found.
/// This is a FAIL FAST behavior - invalid profile names are rejected immediately.
pub fn get_weight_profile(name: &str) -> Result<[f32; NUM_EMBEDDERS], WeightProfileError> {
    WEIGHT_PROFILES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, w)| *w)
        .ok_or_else(|| WeightProfileError::UnknownProfile {
            name: name.to_string(),
            available: get_profile_names(),
        })
}

/// Get all available profile names.
pub fn get_profile_names() -> Vec<&'static str> {
    WEIGHT_PROFILES.iter().map(|(n, _)| *n).collect()
}

/// Validate that weights sum to ~1.0 and all are in [0.0, 1.0].
///
/// # FAIL FAST
/// Returns detailed error on validation failure.
pub fn validate_weights(weights: &[f32; NUM_EMBEDDERS]) -> Result<(), WeightProfileError> {
    // Check each weight is in range
    for (i, &w) in weights.iter().enumerate() {
        if !(0.0..=1.0).contains(&w) {
            return Err(WeightProfileError::OutOfRange {
                index: i,
                space_name: space_name(i),
                value: w,
            });
        }
    }

    // Check sum is ~1.0
    let sum: f32 = weights.iter().sum();
    if (sum - 1.0).abs() > 0.01 {
        return Err(WeightProfileError::InvalidSum { actual: sum });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_profile_fails_fast() {
        let result = get_weight_profile("nonexistent");
        assert!(matches!(result, Err(WeightProfileError::UnknownProfile { .. })));

        if let Err(WeightProfileError::UnknownProfile { name, available }) = result {
            assert_eq!(name, "nonexistent");
            assert!(!available.is_empty());
            println!("[VERIFIED] Unknown profile fails fast with available profiles list");
        }
    }

    #[test]
    fn test_code_search_profile_weights() {
        let weights = get_weight_profile("code_search").unwrap();
        assert!((weights[6] - 0.40).abs() < 0.001, "E7 Code should be 0.40");
        assert!(weights[6] > weights[0], "E7 > E1 for code search");
        println!("[VERIFIED] code_search has E7={:.2} as primary", weights[6]);
    }

    #[test]
    fn test_all_profiles_sum_to_one() {
        for (name, weights) in WEIGHT_PROFILES {
            let sum: f32 = weights.iter().sum();
            assert!(
                (sum - 1.0).abs() < 0.01,
                "Profile '{}' sums to {} (expected ~1.0)",
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
                NUM_EMBEDDERS,
                "Profile '{}' should have {} weights",
                name,
                NUM_EMBEDDERS
            );
        }
        println!("[VERIFIED] All profiles have exactly {} weights", NUM_EMBEDDERS);
    }

    #[test]
    fn test_graph_reasoning_profile_exists() {
        let weights = get_weight_profile("graph_reasoning");
        assert!(weights.is_ok(), "graph_reasoning profile should exist");

        let weights = weights.unwrap();
        assert!((weights[7] - 0.40).abs() < 0.001, "E8 Graph should be 0.40");
        assert!(weights[7] > weights[0], "E8 > E1 for graph reasoning");
        println!("[VERIFIED] graph_reasoning has E8={:.2} as primary", weights[7]);
    }

    #[test]
    fn test_temporal_embedders_excluded_from_semantic_profiles() {
        let semantic_profiles = [
            "semantic_search", "causal_reasoning", "code_search",
            "fact_checking", "graph_reasoning", "intent_search", "intent_enhanced"
        ];

        for profile_name in semantic_profiles {
            let weights = get_weight_profile(profile_name)
                .expect(&format!("Profile '{}' should exist", profile_name));

            assert_eq!(
                weights[1], 0.0,
                "E2 should be 0.0 in '{}' profile per AP-71",
                profile_name
            );
            assert_eq!(
                weights[2], 0.0,
                "E3 should be 0.0 in '{}' profile per AP-71",
                profile_name
            );
            assert_eq!(
                weights[3], 0.0,
                "E4 should be 0.0 in '{}' profile per AP-71",
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
            WeightProfileError::OutOfRange { index, .. } => {
                assert_eq!(index, 0);
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
            WeightProfileError::InvalidSum { actual } => {
                assert!((actual - 6.5).abs() < 0.01);
            }
            _ => panic!("Expected InvalidSum error"),
        }
        println!("[VERIFIED] Invalid sum fails fast");
    }

    #[test]
    fn test_space_names() {
        assert_eq!(space_name(0), "E1_Semantic");
        assert_eq!(space_name(6), "E7_Code");
        assert_eq!(space_name(7), "E8_Graph");
        assert_eq!(space_name(12), "E13_SPLADE");
        assert_eq!(space_name(13), "Unknown");
        println!("[VERIFIED] space_name returns correct names");
    }

    #[test]
    fn test_get_profile_names() {
        let names = get_profile_names();
        assert!(names.contains(&"semantic_search"));
        assert!(names.contains(&"code_search"));
        assert!(names.contains(&"causal_reasoning"));
        assert!(names.contains(&"graph_reasoning"));
        println!("[VERIFIED] get_profile_names returns {} profiles", names.len());
    }
}
