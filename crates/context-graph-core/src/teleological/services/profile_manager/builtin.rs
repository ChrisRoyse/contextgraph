//! Built-in profile factories
//!
//! Contains factory methods for creating built-in teleological profiles:
//! - code_implementation: Emphasizes E6 (Code) for programming tasks
//! - research_analysis: Emphasizes E1, E4, E7 for semantic/causal analysis
//! - creative_writing: Emphasizes E10, E11 for qualitative/abstract tasks

use crate::teleological::TeleologicalProfile;

/// Create the code_implementation built-in profile.
///
/// Emphasizes E6 (Code) at index 5.
pub fn code_implementation() -> TeleologicalProfile {
    // Weights: emphasize E6 (index 5) for code implementation
    let weights = [
        0.05, // E1_Semantic
        0.02, // E2_Episodic
        0.05, // E3_Temporal
        0.15, // E4_Causal
        0.08, // E5_Analogical
        0.25, // E6_Code (PRIMARY)
        0.18, // E7_Procedural
        0.05, // E8_Spatial
        0.02, // E9_Social
        0.02, // E10_Emotional
        0.05, // E11_Abstract
        0.05, // E12_Factual
        0.03, // E13_Sparse
        0.0,  // E14_BgeM3Dense
    ];

    let mut profile = TeleologicalProfile::new(
        "code_implementation",
        "Code Implementation",
        crate::teleological::TaskType::CodeSearch,
    );
    profile.embedding_weights = weights;
    profile.is_system = true;
    profile.description =
        Some("Optimized for programming and code implementation tasks".to_string());
    profile
}

/// Create the research_analysis built-in profile.
///
/// Emphasizes E1 (Semantic), E4 (Causal), E7 (Procedural).
pub fn research_analysis() -> TeleologicalProfile {
    // Weights: emphasize semantic (E1), causal (E4), procedural (E7)
    let weights = [
        0.20, // E1_Semantic (PRIMARY)
        0.05, // E2_Episodic
        0.08, // E3_Temporal
        0.18, // E4_Causal (PRIMARY)
        0.10, // E5_Analogical
        0.03, // E6_Code
        0.15, // E7_Procedural (PRIMARY)
        0.05, // E8_Spatial
        0.03, // E9_Social
        0.02, // E10_Emotional
        0.05, // E11_Abstract
        0.04, // E12_Factual
        0.02, // E13_Sparse
        0.0,  // E14_BgeM3Dense
    ];

    let mut profile = TeleologicalProfile::new(
        "research_analysis",
        "Research Analysis",
        crate::teleological::TaskType::SemanticSearch,
    );
    profile.embedding_weights = weights;
    profile.is_system = true;
    profile.description = Some("Optimized for research and analytical queries".to_string());
    profile
}

/// Create the creative_writing built-in profile.
///
/// Emphasizes E10 (Emotional), E11 (Abstract).
pub fn creative_writing() -> TeleologicalProfile {
    // Weights: emphasize qualitative embeddings (E10, E11)
    let weights = [
        0.08, // E1_Semantic
        0.05, // E2_Episodic
        0.07, // E3_Temporal
        0.05, // E4_Causal
        0.12, // E5_Analogical
        0.02, // E6_Code
        0.03, // E7_Procedural
        0.05, // E8_Spatial
        0.08, // E9_Social
        0.20, // E10_Emotional (PRIMARY)
        0.18, // E11_Abstract (PRIMARY)
        0.04, // E12_Factual
        0.03, // E13_Sparse
        0.0,  // E14_BgeM3Dense
    ];

    let mut profile = TeleologicalProfile::new(
        "creative_writing",
        "Creative Writing",
        crate::teleological::TaskType::AbstractSearch,
    );
    profile.embedding_weights = weights;
    profile.is_system = true;
    profile.description = Some("Optimized for creative and qualitative tasks".to_string());
    profile
}
