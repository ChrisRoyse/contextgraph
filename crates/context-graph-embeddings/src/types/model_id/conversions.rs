//! Type conversions for ModelId.

use super::core::ModelId;

impl TryFrom<u8> for ModelId {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Semantic),
            1 => Ok(Self::TemporalRecent),
            2 => Ok(Self::TemporalPeriodic),
            3 => Ok(Self::TemporalPositional),
            4 => Ok(Self::Causal),
            5 => Ok(Self::Sparse),
            6 => Ok(Self::Code),
            7 => Ok(Self::Graph),
            8 => Ok(Self::Hdc),
            9 => Ok(Self::Multimodal),
            10 => Ok(Self::Entity),
            11 => Ok(Self::LateInteraction),
            12 => Ok(Self::Splade),
            _ => Err("Invalid ModelId: must be 0-12"),
        }
    }
}

impl TryFrom<&str> for ModelId {
    type Error = &'static str;

    /// Parses a model ID string (e.g., "E1_Semantic") into a ModelId enum.
    ///
    /// # Supported formats
    /// - "E1_Semantic", "E2_TemporalRecent", etc. (canonical format)
    /// - "Semantic", "TemporalRecent", etc. (short format)
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // Strip "E{N}_" prefix if present
        let name = if value.starts_with('E') && value.contains('_') {
            value.split('_').skip(1).collect::<Vec<_>>().join("_")
        } else {
            value.to_string()
        };

        match name.as_str() {
            "Semantic" => Ok(Self::Semantic),
            "TemporalRecent" => Ok(Self::TemporalRecent),
            "TemporalPeriodic" => Ok(Self::TemporalPeriodic),
            "TemporalPositional" => Ok(Self::TemporalPositional),
            "Causal" => Ok(Self::Causal),
            "Sparse" => Ok(Self::Sparse),
            "Code" => Ok(Self::Code),
            "Graph" => Ok(Self::Graph),
            "Hdc" | "HDC" => Ok(Self::Hdc),
            "Multimodal" => Ok(Self::Multimodal),
            "Entity" => Ok(Self::Entity),
            "LateInteraction" => Ok(Self::LateInteraction),
            "Splade" | "SPLADE" => Ok(Self::Splade),
            _ => Err("Invalid ModelId string"),
        }
    }
}
