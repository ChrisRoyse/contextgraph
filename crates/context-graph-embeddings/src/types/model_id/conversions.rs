//! Type conversions for ModelId.
//!
//! Includes bidirectional conversion between `ModelId` (embeddings crate)
//! and `Embedder` (core crate) for cross-crate interoperability.

use super::core::ModelId;
use context_graph_core::teleological::embedder::Embedder;

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
            9 => Ok(Self::Contextual),
            10 => Ok(Self::Entity),
            11 => Ok(Self::LateInteraction),
            12 => Ok(Self::Splade),
            13 => Ok(Self::Kepler),
            _ => Err("Invalid ModelId: must be 0-13"),
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
            "Contextual" | "Multimodal" => Ok(Self::Contextual),
            "Entity" => Ok(Self::Entity),
            "LateInteraction" => Ok(Self::LateInteraction),
            "Splade" | "SPLADE" => Ok(Self::Splade),
            "Kepler" | "KEPLER" => Ok(Self::Kepler),
            _ => Err("Invalid ModelId string"),
        }
    }
}

// =============================================================================
// Embedder <-> ModelId conversions (TASK-CORE-012)
// =============================================================================

impl From<Embedder> for ModelId {
    /// Convert from core crate's `Embedder` to embeddings crate's `ModelId`.
    ///
    /// Both enums have identical variant ordering (0-12), so conversion is direct.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::ModelId;
    /// use context_graph_core::teleological::embedder::Embedder;
    ///
    /// let embedder = Embedder::Semantic;
    /// let model_id: ModelId = embedder.into();
    /// assert_eq!(model_id, ModelId::Semantic);
    /// ```
    fn from(embedder: Embedder) -> Self {
        match embedder {
            Embedder::Semantic => ModelId::Semantic,
            Embedder::TemporalRecent => ModelId::TemporalRecent,
            Embedder::TemporalPeriodic => ModelId::TemporalPeriodic,
            Embedder::TemporalPositional => ModelId::TemporalPositional,
            Embedder::Causal => ModelId::Causal,
            Embedder::Sparse => ModelId::Sparse,
            Embedder::Code => ModelId::Code,
            Embedder::Graph => ModelId::Graph,
            Embedder::Hdc => ModelId::Hdc,
            Embedder::Contextual => ModelId::Contextual,
            Embedder::Entity => ModelId::Kepler,  // Production E11 is KEPLER (768D), not legacy Entity (384D)
            Embedder::LateInteraction => ModelId::LateInteraction,
            Embedder::KeywordSplade => ModelId::Splade,
        }
    }
}

impl From<ModelId> for Embedder {
    /// Convert from embeddings crate's `ModelId` to core crate's `Embedder`.
    ///
    /// Both enums have identical variant ordering (0-12), so conversion is direct.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::types::ModelId;
    /// use context_graph_core::teleological::embedder::Embedder;
    ///
    /// let model_id = ModelId::Code;
    /// let embedder: Embedder = model_id.into();
    /// assert_eq!(embedder, Embedder::Code);
    /// ```
    fn from(model_id: ModelId) -> Self {
        match model_id {
            ModelId::Semantic => Embedder::Semantic,
            ModelId::TemporalRecent => Embedder::TemporalRecent,
            ModelId::TemporalPeriodic => Embedder::TemporalPeriodic,
            ModelId::TemporalPositional => Embedder::TemporalPositional,
            ModelId::Causal => Embedder::Causal,
            ModelId::Sparse => Embedder::Sparse,
            ModelId::Code => Embedder::Code,
            ModelId::Graph => Embedder::Graph,
            ModelId::Hdc => Embedder::Hdc,
            ModelId::Contextual => Embedder::Contextual,
            ModelId::Entity => Embedder::Entity,
            ModelId::Kepler => Embedder::Entity, // KEPLER is the new E11, maps to Entity embedder
            ModelId::LateInteraction => Embedder::LateInteraction,
            ModelId::Splade => Embedder::KeywordSplade,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_to_model_id_all_variants() {
        // Test all 13 conversions Embedder -> ModelId
        let mappings = [
            (Embedder::Semantic, ModelId::Semantic),
            (Embedder::TemporalRecent, ModelId::TemporalRecent),
            (Embedder::TemporalPeriodic, ModelId::TemporalPeriodic),
            (Embedder::TemporalPositional, ModelId::TemporalPositional),
            (Embedder::Causal, ModelId::Causal),
            (Embedder::Sparse, ModelId::Sparse),
            (Embedder::Code, ModelId::Code),
            (Embedder::Graph, ModelId::Graph),
            (Embedder::Hdc, ModelId::Hdc),
            (Embedder::Contextual, ModelId::Contextual),
            (Embedder::Entity, ModelId::Kepler),  // Production E11 is KEPLER (768D)
            (Embedder::LateInteraction, ModelId::LateInteraction),
            (Embedder::KeywordSplade, ModelId::Splade),
        ];

        for (embedder, expected_model_id) in mappings {
            let model_id: ModelId = embedder.into();
            assert_eq!(
                model_id, expected_model_id,
                "Embedder::{:?} should map to ModelId::{:?}",
                embedder, expected_model_id
            );
        }
        println!("[PASS] All 13 Embedder -> ModelId conversions correct");
    }

    #[test]
    fn test_model_id_to_embedder_all_variants() {
        // Test all 13 conversions ModelId -> Embedder
        let mappings = [
            (ModelId::Semantic, Embedder::Semantic),
            (ModelId::TemporalRecent, Embedder::TemporalRecent),
            (ModelId::TemporalPeriodic, Embedder::TemporalPeriodic),
            (ModelId::TemporalPositional, Embedder::TemporalPositional),
            (ModelId::Causal, Embedder::Causal),
            (ModelId::Sparse, Embedder::Sparse),
            (ModelId::Code, Embedder::Code),
            (ModelId::Graph, Embedder::Graph),
            (ModelId::Hdc, Embedder::Hdc),
            (ModelId::Contextual, Embedder::Contextual),
            (ModelId::Entity, Embedder::Entity),
            (ModelId::LateInteraction, Embedder::LateInteraction),
            (ModelId::Splade, Embedder::KeywordSplade),
        ];

        for (model_id, expected_embedder) in mappings {
            let embedder: Embedder = model_id.into();
            assert_eq!(
                embedder, expected_embedder,
                "ModelId::{:?} should map to Embedder::{:?}",
                model_id, expected_embedder
            );
        }
        println!("[PASS] All 13 ModelId -> Embedder conversions correct");
    }

    #[test]
    fn test_roundtrip_embedder_model_id() {
        // Test roundtrip: Embedder -> ModelId -> Embedder
        for embedder in Embedder::all() {
            let model_id: ModelId = embedder.into();
            let back: Embedder = model_id.into();
            assert_eq!(
                embedder, back,
                "Roundtrip failed for Embedder::{:?}",
                embedder
            );
        }
        println!("[PASS] Embedder <-> ModelId roundtrip preserves all 13 variants");
    }

    #[test]
    fn test_index_preservation() {
        // Verify that index() is preserved across conversion for most embedders.
        // Exception: Embedder::Entity (index 10) maps to ModelId::Kepler (index 13)
        // because Kepler is the production E11 replacement for legacy Entity.
        for embedder in Embedder::all() {
            let model_id: ModelId = embedder.into();
            if embedder == Embedder::Entity {
                // Embedder::Entity -> ModelId::Kepler (production E11, index 13)
                assert_eq!(
                    model_id,
                    ModelId::Kepler,
                    "Embedder::Entity should map to ModelId::Kepler (production E11)"
                );
                assert_eq!(model_id as usize, 13, "Kepler repr index is 13");
            } else {
                assert_eq!(
                    embedder.index(),
                    model_id as usize,
                    "Index mismatch for {:?}",
                    embedder
                );
            }
        }
        println!("[PASS] Index values correctly mapped across Embedder <-> ModelId conversion");
    }
}
