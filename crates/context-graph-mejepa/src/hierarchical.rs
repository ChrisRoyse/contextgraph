use std::collections::BTreeMap;
use std::path::Path;

use context_graph_mejepa_instruments::Panel;
use sha2::{Digest, Sha256};

use crate::error::MejepaInferError;
use crate::types::{
    ChunkId, HierarchicalPredictionLevel, HierarchicalPredictionRecord, PatchBundle,
    PredictionHierarchyLevel, RealityPrediction, HIERARCHICAL_PREDICTION_SCHEMA_VERSION,
};

pub fn build_hierarchical_prediction(
    prediction: &RealityPrediction,
    patch: &PatchBundle,
    predicted_panel: &Panel,
) -> Result<HierarchicalPredictionRecord, MejepaInferError> {
    prediction.validate()?;
    patch.validate()?;
    if prediction.covered_chunks.len() != patch.ast_diff.hunks.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: patch.ast_diff.hunks.len(),
            actual: prediction.covered_chunks.len(),
            context: "hierarchical prediction requires one covered chunk per AST diff hunk"
                .to_string(),
        });
    }

    let mut file_chunks = BTreeMap::<String, Vec<ChunkId>>::new();
    let mut descendants = Vec::with_capacity(patch.ast_diff.hunks.len() * 3);

    for (idx, hunk) in patch.ast_diff.hunks.iter().enumerate() {
        let chunk = prediction.covered_chunks[idx].clone();
        let file_scope = file_scope_id(&hunk.path);
        file_chunks
            .entry(file_scope.clone())
            .or_default()
            .push(chunk.clone());

        let function_scope = format!(
            "{file_scope}/function:{}#{}",
            function_slug(&hunk.after).unwrap_or_else(|| "module".to_string()),
            idx
        );
        descendants.push(level_record(
            PredictionHierarchyLevel::Function,
            function_scope.clone(),
            Some(file_scope),
            vec![chunk.clone()],
            prediction,
            latent_energy(predicted_panel.data(), idx, 0),
        )?);

        let ast_scope = format!(
            "{function_scope}/ast:{}:{}",
            ast_node_kind(&hunk.after),
            short_hash(hunk.after.as_bytes())
        );
        descendants.push(level_record(
            PredictionHierarchyLevel::AstNode,
            ast_scope.clone(),
            Some(function_scope),
            vec![chunk.clone()],
            prediction,
            latent_energy(predicted_panel.data(), idx, 1),
        )?);

        descendants.push(level_record(
            PredictionHierarchyLevel::Chunk,
            format!("{ast_scope}/chunk:{idx}"),
            Some(ast_scope),
            vec![chunk],
            prediction,
            latent_energy(predicted_panel.data(), idx, 2),
        )?);
    }

    let mut levels = Vec::with_capacity(file_chunks.len() + descendants.len());
    for (idx, (file_scope, chunks)) in file_chunks.into_iter().enumerate() {
        levels.push(level_record(
            PredictionHierarchyLevel::File,
            file_scope,
            None,
            chunks,
            prediction,
            latent_energy(predicted_panel.data(), idx, 3),
        )?);
    }
    levels.extend(descendants);

    HierarchicalPredictionRecord::try_new(HierarchicalPredictionRecord {
        schema_version: HIERARCHICAL_PREDICTION_SCHEMA_VERSION,
        prediction_id: prediction.prediction_id,
        task_id: prediction.task_id.clone(),
        session_id: prediction.session_id,
        language: prediction.language,
        source_panel_sha: prediction.source_panel_sha,
        calibration_version: prediction.calibration_version.clone(),
        created_at_unix_ms: prediction.created_at_unix_ms,
        slot_attributions: prediction.slot_attributions.clone(),
        levels,
    })
}

fn level_record(
    level: PredictionHierarchyLevel,
    scope_id: String,
    parent_scope_id: Option<String>,
    covered_chunks: Vec<ChunkId>,
    prediction: &RealityPrediction,
    latent_energy: f32,
) -> Result<HierarchicalPredictionLevel, MejepaInferError> {
    let scale = match level {
        PredictionHierarchyLevel::File => 0.98,
        PredictionHierarchyLevel::Function => 0.99,
        PredictionHierarchyLevel::AstNode => 1.0,
        PredictionHierarchyLevel::Chunk => 1.01,
    };
    let uncertainty_penalty = (latent_energy.sqrt() * 0.02).min(0.05);
    let predicted_oracle_pass =
        (prediction.predicted_oracle_pass * scale - uncertainty_penalty).clamp(0.0, 1.0);
    let calibrated_confidence =
        (prediction.calibrated_confidence * scale - uncertainty_penalty).clamp(0.0, 1.0);
    let ood_score = (prediction.ood_score + uncertainty_penalty).clamp(0.0, 1.0);
    let record = HierarchicalPredictionLevel {
        level,
        scope_id,
        parent_scope_id,
        covered_chunks,
        predicted_oracle_pass,
        calibrated_confidence,
        ood_score,
        verdict: prediction.verdict,
        latent_energy,
    };
    record.validate()?;
    Ok(record)
}

fn file_scope_id(path: &Path) -> String {
    format!("file:{}", path.display())
}

fn function_slug(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim_start();
        let candidate = trimmed
            .strip_prefix("async def ")
            .or_else(|| trimmed.strip_prefix("def "))
            .or_else(|| trimmed.strip_prefix("fn "))
            .or_else(|| trimmed.strip_prefix("function "))
            .or_else(|| trimmed.strip_prefix("class "));
        if let Some(rest) = candidate {
            let name = rest
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .next()
                .unwrap_or_default();
            if !name.is_empty() {
                return Some(name.to_ascii_lowercase());
            }
        }
    }
    None
}

fn ast_node_kind(text: &str) -> &'static str {
    let lower = text.to_ascii_lowercase();
    if lower.contains("class ") {
        "class"
    } else if lower.contains("def ") || lower.contains("fn ") || lower.contains("function ") {
        "function"
    } else if lower.contains("if ") || lower.contains("match ") {
        "branch"
    } else if lower.contains("for ") || lower.contains("while ") {
        "loop"
    } else {
        "statement"
    }
}

fn latent_energy(data: &[f32], scope_index: usize, salt: usize) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let width = data.len().min(64);
    let start = (scope_index.wrapping_mul(97) + salt.wrapping_mul(31)) % data.len();
    let mut sum = 0.0_f32;
    for offset in 0..width {
        let value = data[(start + offset) % data.len()];
        sum += value * value;
    }
    (sum / width as f32).max(0.0)
}

fn short_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(&digest[..8])
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa_instruments::PANEL_DIM;

    use crate::types::{
        AstDiff, ConformalInterval, ConformalSet, DiffHunk, Language, OracleOutcome, PatchBundle,
        PredictionProvenance, RealityPredictionBuilder, TaskId, Verdict, WitnessHash,
    };

    #[test]
    fn hierarchical_prediction_contains_file_function_ast_and_chunk_levels() {
        let patch = patch_with_two_hunks();
        let prediction = prediction_for(&patch);
        let panel = panel();
        let record = build_hierarchical_prediction(&prediction, &patch, &panel).unwrap();
        assert_eq!(record.slot_attributions, prediction.slot_attributions);
        assert!(!record.slot_attributions.is_empty());
        let kinds = record
            .levels
            .iter()
            .map(|level| level.level)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(kinds.contains(&PredictionHierarchyLevel::File));
        assert!(kinds.contains(&PredictionHierarchyLevel::Function));
        assert!(kinds.contains(&PredictionHierarchyLevel::AstNode));
        assert!(kinds.contains(&PredictionHierarchyLevel::Chunk));
        assert_eq!(
            record
                .levels
                .iter()
                .filter(|level| level.level == PredictionHierarchyLevel::Chunk)
                .count(),
            2
        );
        record.validate().unwrap();
    }

    #[test]
    fn hierarchy_rejects_missing_parent() {
        let patch = patch_with_two_hunks();
        let prediction = prediction_for(&patch);
        let panel = panel();
        let mut record = build_hierarchical_prediction(&prediction, &patch, &panel).unwrap();
        let child = record
            .levels
            .iter_mut()
            .find(|level| level.level == PredictionHierarchyLevel::Chunk)
            .unwrap();
        child.parent_scope_id = Some("missing-parent".to_string());
        assert!(record.validate().is_err());
    }

    fn patch_with_two_hunks() -> PatchBundle {
        PatchBundle::try_new(
            AstDiff {
                hunks: vec![
                    DiffHunk {
                        path: "src/a.py".into(),
                        pre_sha: [1; 32],
                        post_sha: [2; 32],
                        before: String::new(),
                        after: "def alpha():\n    return 1\n".to_string(),
                    },
                    DiffHunk {
                        path: "src/a.py".into(),
                        pre_sha: [3; 32],
                        post_sha: [4; 32],
                        before: String::new(),
                        after: "def beta(values):\n    if values:\n        return values[0]\n"
                            .to_string(),
                    },
                ],
            },
            crate::gates::valid_witness_segment(),
            "hierarchical test".to_string(),
            [5; 32],
        )
        .unwrap()
    }

    fn prediction_for(patch: &PatchBundle) -> RealityPrediction {
        let covered_chunks = patch
            .ast_diff
            .hunks
            .iter()
            .enumerate()
            .map(|(idx, hunk)| ChunkId(format!("{}#{idx}", hunk.path.display())))
            .collect();
        RealityPredictionBuilder::from_parts(
            TaskId("task-fp-102-unit".to_string()),
            [7; 16],
            Language::Python,
            ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
        )
        .prediction_id([8; 16])
        .witness_hash(WitnessHash([9; 32]))
        .covered_chunks(covered_chunks)
        .verdict(Verdict::Pass)
        .confidence_interval(ConformalInterval {
            lower: 0.7,
            upper: 0.9,
            ..ConformalInterval::default()
        })
        .predicted_oracle_pass(0.88)
        .predicted_test_pass(vec![0.90])
        .predicted_runtime_trace([0.001; 32])
        .ood_score(0.05)
        .calibrated_confidence(0.82)
        .provenance(PredictionProvenance {
            predictor_version: "task-fp-102-unit".to_string(),
            constellation_version: "task-fp-102-unit".to_string(),
            calibration_version: "task-fp-102-unit".to_string(),
            active_pointer: "unit".to_string(),
            train_health_source: String::new(),
        })
        .source_panel_sha([10; 32])
        .calibration_version("task-fp-102-unit")
        .build()
        .unwrap()
    }

    fn panel() -> Panel {
        let data = (0..PANEL_DIM)
            .map(|idx| (idx as f32 % 19.0) * 0.001)
            .collect::<Vec<_>>();
        let filled_mask =
            (1u16 << context_graph_mejepa_instruments::InstrumentSlot::all().len()) - 1;
        Panel::try_new(data, filled_mask).unwrap()
    }
}
