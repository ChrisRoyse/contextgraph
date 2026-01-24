//! Display implementation for ModelId.

use super::core::ModelId;

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Semantic => "Semantic (E1)",
            Self::TemporalRecent => "TemporalRecent (E2)",
            Self::TemporalPeriodic => "TemporalPeriodic (E3)",
            Self::TemporalPositional => "TemporalPositional (E4)",
            Self::Causal => "Causal (E5)",
            Self::Sparse => "Sparse (E6)",
            Self::Code => "Code (E7)",
            Self::Graph => "Graph (E8)",
            Self::Hdc => "Hdc (E9)",
            Self::Multimodal => "Multimodal (E10)",
            Self::Entity => "Entity (E11-deprecated)",
            Self::Kepler => "Kepler (E11)",
            Self::LateInteraction => "LateInteraction (E12)",
            Self::Splade => "Splade (E13)",
        };
        write!(f, "{name}")
    }
}
