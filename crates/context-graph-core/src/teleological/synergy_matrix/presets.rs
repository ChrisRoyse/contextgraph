//! Predefined matrix constructors for specific use cases (TASK-CORE-004).
//!
//! These constructors create SynergyMatrix instances optimized for specific
//! use cases. Each modifies the base synergies to emphasize certain
//! embedder pairs.

use chrono::Utc;

use super::types::SynergyMatrix;

impl SynergyMatrix {
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

    /// Create a graph reasoning synergy matrix.
    ///
    /// Emphasizes E8_Spatial (Graph) relationships for structural and
    /// connectivity queries. E8 captures module dependencies, code structure,
    /// and connectivity patterns.
    ///
    /// Per E8 upgrade specification (Phase 5):
    /// - E8 Graph: PRIMARY for structural relationships
    /// - E1 Semantic: Supporting context
    /// - E11 Abstract (Entity): Entity relationships
    /// - E6 Code: Code structure connections
    ///
    /// Use for: module dependencies ("what imports X?"), code structure
    /// ("what extends BaseClass?"), connectivity ("what connects to X?").
    pub fn graph_reasoning() -> Self {
        let mut matrix = Self::with_base_synergies();

        // Boost E8_Spatial (Graph) pairs - PRIMARY for structural reasoning
        // E8 + E1_Semantic: graph structure with semantic context
        matrix.values[7][0] = 0.95;
        matrix.values[0][7] = 0.95;
        // E8 + E11_Abstract: graph relationships with entities
        matrix.values[7][10] = 0.95;
        matrix.values[10][7] = 0.95;
        // E8 + E6_Code: code structure graphs (imports, dependencies)
        matrix.values[7][5] = 0.95;
        matrix.values[5][7] = 0.95;
        // E8 + E7_Procedural: call graphs, flow structure
        matrix.values[7][6] = 0.90;
        matrix.values[6][7] = 0.90;
        // E8 + E5_Analogical: structural analogies
        matrix.values[7][4] = 0.85;
        matrix.values[4][7] = 0.85;

        // Also boost E6_Code + E11_Abstract (code entities)
        matrix.values[5][10] = 0.85;
        matrix.values[10][5] = 0.85;

        matrix.computed_at = Utc::now();
        matrix
    }
}
