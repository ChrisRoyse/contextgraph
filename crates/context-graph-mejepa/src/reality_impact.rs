use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::error::MejepaInferError;
use crate::types::{
    ObservedTestOutcome, PredictedTestOutcome, PredictionCorrectness, RealityImpact,
    RealityPrediction, ShiftEntry, TestOutcome,
};

pub fn reconcile_reality_impact(
    prediction: &RealityPrediction,
    observed_shifts: Vec<ShiftEntry>,
    observed_test_outcomes: Vec<ObservedTestOutcome>,
) -> Result<RealityImpact, MejepaInferError> {
    let predicted_files_changed = prediction_covered_files(prediction);
    let observed_files_changed = observed_shifts
        .iter()
        .map(|shift| shift.file.clone())
        .collect::<Vec<_>>();
    let predicted_set = predicted_files_changed.iter().collect::<BTreeSet<_>>();
    let unexpected_files_changed = observed_files_changed
        .iter()
        .filter(|file| !predicted_set.contains(file))
        .cloned()
        .collect::<Vec<_>>();
    let prediction_correctness = classify_correctness(
        &prediction.predicted_failed_tests,
        &observed_test_outcomes,
        unexpected_files_changed.is_empty(),
    );
    let impact = RealityImpact {
        observed_shifts,
        predicted_files_changed,
        observed_files_changed,
        unexpected_files_changed,
        predicted_test_outcomes: prediction.predicted_failed_tests.clone(),
        observed_test_outcomes,
        prediction_correctness,
    };
    impact.validate()?;
    Ok(impact)
}

pub fn prediction_covered_files(prediction: &RealityPrediction) -> Vec<PathBuf> {
    let mut out = prediction
        .covered_chunks
        .iter()
        .filter_map(|chunk| chunk.0.split('#').next())
        .filter(|raw| !raw.is_empty())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}

fn classify_correctness(
    predicted: &[PredictedTestOutcome],
    observed: &[ObservedTestOutcome],
    files_aligned: bool,
) -> PredictionCorrectness {
    let predicted_failures = predicted
        .iter()
        .filter(|item| {
            matches!(
                item.predicted_outcome,
                TestOutcome::Fail | TestOutcome::Error
            )
        })
        .count();
    let observed_failures = observed
        .iter()
        .filter(|item| matches!(item.outcome, TestOutcome::Fail | TestOutcome::Error))
        .count();
    if !files_aligned || observed_failures > predicted_failures + 2 {
        return PredictionCorrectness::Surprise;
    }
    match predicted_failures.cmp(&observed_failures) {
        std::cmp::Ordering::Equal => PredictionCorrectness::Aligned,
        std::cmp::Ordering::Less => PredictionCorrectness::UnderPredicted,
        std::cmp::Ordering::Greater => PredictionCorrectness::OverPredicted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChunkId, ConformalSet, Language, OracleOutcome, RealityPrediction, TaskId};
    use std::collections::BTreeMap;

    #[test]
    fn covered_files_dedup_by_chunk_prefix() {
        let prediction = RealityPrediction::try_new(RealityPrediction {
            prediction_id: [1; 16],
            witness_hash: crate::types::WitnessHash([2; 32]),
            task_id: TaskId("task".to_string()),
            session_id: [3; 16],
            language: Language::Rust,
            covered_chunks: vec![
                ChunkId("src/lib.rs#0".to_string()),
                ChunkId("src/lib.rs#1".to_string()),
            ],
            verdict: crate::types::Verdict::Pass,
            confidence_interval: crate::types::ConformalInterval::default(),
            predicted_oracle_pass: 0.9,
            predicted_test_pass: vec![0.9],
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.1,
            outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
            calibrated_confidence: 0.8,
            degraded_status: false,
            granger_attestations: BTreeMap::new(),
            predicted_failure_modes: Vec::new(),
            predicted_failed_tests: Vec::new(),
            predicted_works: Vec::new(),
            predicted_uncovered_paths: Vec::new(),
            predicted_flaky_tests: Vec::new(),
            guard_violations: Vec::new(),
            per_slot_ood_reasons: Vec::new(),
            closest_exemplars: Vec::new(),
            predicted_edge_cases: Vec::new(),
            predicted_latent_bugs: Vec::new(),
            predicted_tech_debt_added: Vec::new(),
            predicted_dead_code: Vec::new(),
            predicted_redundant_code: Vec::new(),
            predicted_perf_regressions: Vec::new(),
            predicted_security_concerns: Vec::new(),
            predicted_accuracy_degradations: Vec::new(),
            predicted_cost_regressions: Vec::new(),
            predicted_reasoning_class: crate::types::ReasoningClass::Mute,
            agent_claim_graph: crate::types::AgentClaimGraph::default(),
            claim_reconciliation: Vec::new(),
            reality_impact: None,
            provenance: crate::types::PredictionProvenance::default(),
            source_panel_sha: [4; 32],
            calibration_version: "cal-v1".to_string(),
            created_at_unix_ms: 1,
            matched_fingerprint: None,
            unknown_candidate_id: None,
        })
        .unwrap();
        assert_eq!(
            prediction_covered_files(&prediction),
            vec![PathBuf::from("src/lib.rs")]
        );
    }
}
