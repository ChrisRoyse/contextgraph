use std::collections::BTreeMap;
use std::time::Instant;

use context_graph_mejepa_instruments::Panel;

use crate::constellation::TctConstellation;
use crate::error::TctError;
use crate::hinge_types::ChunkGtauOutput;
use crate::panel_slots::panel_slice_for_embedder;
use crate::types::{
    ChunkId, EmbedderId, EntityType, GtauOutput, GtauViolation, Language, MutationCategory,
};

pub fn gtau_check(
    predicted_panel: &Panel,
    predicted_class: MutationCategory,
    language: Language,
    entity_type: EntityType,
    constellation: &TctConstellation,
) -> Result<GtauOutput, TctError> {
    let start = Instant::now();
    let mut violations = Vec::new();
    let mut centroid_origin = BTreeMap::new();
    let mut min_margin = f32::INFINITY;

    for embedder in EmbedderId::all() {
        let slot = panel_slice_for_embedder(predicted_panel, embedder)?;
        let (centroid, origin, _n) = constellation
            .lookup_centroid(predicted_class, language, entity_type, embedder)
            .ok_or_else(|| TctError::MissingCentroid {
                detail: format!(
                    "missing centroid for {:?}/{:?}/{:?}/{embedder}",
                    predicted_class, language, entity_type
                ),
            })?;
        let observed = cosine_similarity(slot, centroid)?;
        let threshold = constellation.threshold(embedder, Some(entity_type))?;
        let margin = observed - threshold;
        min_margin = min_margin.min(margin);
        centroid_origin.insert(embedder, origin);
        if observed < threshold {
            violations.push(GtauViolation::try_new(
                embedder, observed, threshold, None, origin,
            )?);
        }
    }

    GtauOutput::try_new(
        violations,
        centroid_origin,
        start.elapsed().as_secs_f32() * 1_000.0,
        EmbedderId::all().len(),
        min_margin,
    )
}

pub fn gtau_check_chunks(
    chunk_panels: &[(ChunkId, Panel)],
    predicted_class: MutationCategory,
    constellation: &TctConstellation,
) -> Result<ChunkGtauOutput, TctError> {
    if chunk_panels.is_empty() {
        return Err(TctError::InsufficientSamples {
            cell: "chunk_panels".to_string(),
            observed: 0,
            required: 1,
        });
    }
    let mut per_chunk_results = Vec::with_capacity(chunk_panels.len());
    let mut violating_chunks = Vec::new();
    for (chunk_id, panel) in chunk_panels {
        let mut output = gtau_check(
            panel,
            predicted_class,
            chunk_id.language,
            chunk_id.entity_type,
            constellation,
        )?;
        for violation in &mut output.violations {
            violation.violating_chunk = Some(chunk_id.clone());
        }
        if !output.gtau_satisfied {
            violating_chunks.push(chunk_id.clone());
        }
        per_chunk_results.push((chunk_id.clone(), output));
    }
    ChunkGtauOutput::try_new(per_chunk_results, violating_chunks)
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, TctError> {
    if a.len() != b.len() {
        return Err(TctError::dim(
            b.len(),
            a.len(),
            "cosine_similarity dimension mismatch",
        ));
    }
    if a.is_empty() {
        return Err(TctError::dim(1, 0, "cosine_similarity empty vectors"));
    }
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for idx in 0..a.len() {
        if !a[idx].is_finite() {
            return Err(TctError::nan(
                "cosine_similarity.a",
                format!("a[{idx}] is {}", a[idx]),
            ));
        }
        if !b[idx].is_finite() {
            return Err(TctError::nan(
                "cosine_similarity.b",
                format!("b[{idx}] is {}", b[idx]),
            ));
        }
        dot += a[idx] as f64 * b[idx] as f64;
        na += a[idx] as f64 * a[idx] as f64;
        nb += b[idx] as f64 * b[idx] as f64;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom < 1.0e-12 {
        return Err(TctError::ConstellationViolation {
            detail: format!(
                "cosine_similarity zero-norm vector: norm_a={:.6e} norm_b={:.6e}",
                na.sqrt(),
                nb.sqrt()
            ),
        });
    }
    let value = (dot / denom) as f32;
    if !value.is_finite() {
        return Err(TctError::nan(
            "cosine_similarity.output",
            format!("cosine output is {value}"),
        ));
    }
    Ok(value.clamp(-1.0, 1.0))
}
