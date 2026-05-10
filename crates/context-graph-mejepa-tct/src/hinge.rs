use std::collections::BTreeMap;

use context_graph_mejepa_instruments::Panel;

use crate::constellation::TctConstellation;
use crate::error::TctError;
use crate::gtau::cosine_similarity;
use crate::hinge_types::ConstellationHingeOutput;
use crate::panel_slots::panel_slice_for_embedder;
use crate::types::{EmbedderId, EntityType, Language, MutationCategory};

pub fn constellation_hinge_loss(
    predicted_panel: &Panel,
    predicted_class: MutationCategory,
    language: Language,
    entity_type: EntityType,
    constellation: &TctConstellation,
    margin: f32,
) -> Result<ConstellationHingeOutput, TctError> {
    if !margin.is_finite() || margin < 0.0 {
        return Err(TctError::invalid(
            "margin",
            format!("hinge margin must be finite and non-negative, got {margin}"),
        ));
    }
    let mut hinges = BTreeMap::new();
    let mut origins = BTreeMap::new();
    for embedder in EmbedderId::all() {
        let slot = panel_slice_for_embedder(predicted_panel, embedder)?;
        let (centroid, origin, _n) = constellation
            .lookup_centroid(predicted_class, language, entity_type, embedder)
            .ok_or_else(|| TctError::MissingCentroid {
                detail: format!(
                    "missing hinge centroid for {:?}/{:?}/{:?}/{embedder}",
                    predicted_class, language, entity_type
                ),
            })?;
        let cos = cosine_similarity(slot, centroid)?;
        let tau = constellation.threshold(embedder, Some(entity_type))?;
        let hinge = (tau + margin - cos).max(0.0);
        hinges.insert(embedder, hinge);
        origins.insert(embedder, origin);
    }
    ConstellationHingeOutput::try_new(hinges, origins)
}
