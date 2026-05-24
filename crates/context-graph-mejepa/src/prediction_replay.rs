//! Byte-for-byte replay for persisted ME-JEPA `RealityPrediction` rows.
//!
//! The source of truth is `CF_MEJEPA_LIVE_PREDICTIONS`. A replay succeeds only
//! when the RocksDB key matches the decoded payload and reserializing the
//! decoded `RealityPrediction` reproduces the exact persisted bytes.

use std::path::Path;

use rocksdb::{IteratorMode, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::{cf, count_cf, open_infer_rocksdb, CF_MEJEPA_LIVE_PREDICTIONS};
use crate::error::MejepaInferError;
use crate::types::{decode_reality_prediction, PredictionId, RealityPrediction};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredictionReplaySourceOfTruth {
    pub db_path: Option<String>,
    pub column_family: String,
    pub key_hex: String,
    pub value_sha256: String,
    pub value_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredictionReplayReport {
    pub prediction_id: String,
    pub task_id: String,
    pub session_id: String,
    pub created_at_unix_ms: i64,
    pub column_family: String,
    pub live_prediction_count: u64,
    pub stored_value_sha256: String,
    pub persisted_value_sha256: String,
    pub replayed_value_sha256: String,
    pub byte_equal: bool,
    pub semantic_equal: bool,
    pub byte_for_byte_equal: bool,
    pub key_payload_equal: bool,
    pub prediction: RealityPrediction,
    pub source_of_truth: PredictionReplaySourceOfTruth,
}

pub fn parse_prediction_id_hex(raw: &str) -> Result<PredictionId, MejepaInferError> {
    if raw.len() != 32 || !raw.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(MejepaInferError::InvalidInput {
            field: "prediction_id".to_string(),
            detail: "MEJEPA_PREDICTION_REPLAY_ID_INVALID: predictionId must be exactly 32 hexadecimal characters".to_string(),
        });
    }
    let mut bytes = [0u8; 16];
    hex::decode_to_slice(raw, &mut bytes).map_err(|err| MejepaInferError::InvalidInput {
        field: "prediction_id".to_string(),
        detail: format!("MEJEPA_PREDICTION_REPLAY_ID_DECODE_FAILED: {err}"),
    })?;
    Ok(PredictionId(bytes))
}

pub fn replay_prediction_from_db(
    db_path: &Path,
    prediction_id: PredictionId,
) -> Result<PredictionReplayReport, MejepaInferError> {
    let db = open_infer_rocksdb(db_path)?;
    replay_prediction_by_id(db.as_ref(), Some(db_path), prediction_id)
}

#[derive(Debug, Clone)]
struct DecodedLivePredictionRow {
    prediction: RealityPrediction,
    key_hex: String,
    value_sha256: String,
    value_bytes: usize,
    reserialized_sha256: String,
}

pub fn replay_prediction_by_id(
    db: &DB,
    db_path: Option<&Path>,
    prediction_id: PredictionId,
) -> Result<PredictionReplayReport, MejepaInferError> {
    let live_prediction_count = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    let cf = cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        let row = decode_live_prediction_row(&key, &value)?;
        if row.prediction.prediction_id == prediction_id.0 {
            return Ok(PredictionReplayReport {
                prediction_id: hex::encode(row.prediction.prediction_id),
                task_id: row.prediction.task_id.0.clone(),
                session_id: hex::encode(row.prediction.session_id),
                created_at_unix_ms: row.prediction.created_at_unix_ms,
                column_family: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
                live_prediction_count,
                stored_value_sha256: row.value_sha256.clone(),
                persisted_value_sha256: row.value_sha256.clone(),
                replayed_value_sha256: row.reserialized_sha256.clone(),
                byte_equal: true,
                semantic_equal: true,
                byte_for_byte_equal: true,
                key_payload_equal: true,
                source_of_truth: PredictionReplaySourceOfTruth {
                    db_path: db_path.map(|path| path.display().to_string()),
                    column_family: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
                    key_hex: row.key_hex,
                    value_sha256: row.value_sha256,
                    value_bytes: row.value_bytes,
                },
                prediction: row.prediction,
            });
        }
    }
    Err(MejepaInferError::InvalidInput {
        field: "prediction_id".to_string(),
        detail: format!(
            "MEJEPA_PREDICTION_REPLAY_NOT_FOUND: predictionId={} source_of_truth={}",
            hex::encode(prediction_id.0),
            CF_MEJEPA_LIVE_PREDICTIONS
        ),
    })
}

fn decode_live_prediction_row(
    key: &[u8],
    value: &[u8],
) -> Result<DecodedLivePredictionRow, MejepaInferError> {
    if key.len() != 40 {
        return Err(MejepaInferError::DimMismatch {
            expected: 40,
            actual: key.len(),
            context: "MEJEPA_REPLAY_KEY_LENGTH: live prediction key must be session_id || created_at || prediction_id".to_string(),
        });
    }
    let prediction = decode_reality_prediction(value)?;

    let mut key_session_id = [0u8; 16];
    key_session_id.copy_from_slice(&key[0..16]);
    if prediction.session_id != key_session_id {
        return Err(MejepaInferError::InvalidInput {
            field: "live_predictions.session_id".to_string(),
            detail:
                "MEJEPA_PREDICTION_REPLAY_KEY_MISMATCH: key session prefix does not match payload"
                    .to_string(),
        });
    }

    let mut key_created_at = [0u8; 8];
    key_created_at.copy_from_slice(&key[16..24]);
    if prediction.created_at_unix_ms != i64::from_be_bytes(key_created_at) {
        return Err(MejepaInferError::InvalidInput {
            field: "live_predictions.created_at_unix_ms".to_string(),
            detail: "MEJEPA_PREDICTION_REPLAY_KEY_MISMATCH: key timestamp does not match payload"
                .to_string(),
        });
    }

    let mut key_prediction_id = [0u8; 16];
    key_prediction_id.copy_from_slice(&key[24..40]);
    if prediction.prediction_id != key_prediction_id {
        return Err(MejepaInferError::InvalidInput {
            field: "live_predictions.prediction_id".to_string(),
            detail:
                "MEJEPA_PREDICTION_REPLAY_KEY_MISMATCH: key prediction suffix does not match payload"
                    .to_string(),
        });
    }

    let reserialized = bincode::serialize(&prediction)?;
    if reserialized != value {
        return Err(MejepaInferError::InvalidInput {
            field: "live_predictions.value".to_string(),
            detail: "MEJEPA_PREDICTION_REPLAY_BYTE_MISMATCH: reserialized RealityPrediction bytes differ from persisted payload".to_string(),
        });
    }

    Ok(DecodedLivePredictionRow {
        prediction,
        key_hex: hex::encode(key),
        value_sha256: hex::encode(Sha256::digest(value)),
        value_bytes: value.len(),
        reserialized_sha256: hex::encode(Sha256::digest(&reserialized)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::open_infer_rocksdb;
    use crate::compiler::MejepaStore;
    use crate::store::RocksDbInferStore;
    use crate::types::{
        AgentClaimGraph, ConformalInterval, ConformalSet, Language, OracleOutcome,
        PredictionProvenance, ReasoningClass, TaskId, Verdict, WitnessHash,
    };

    fn prediction(prediction_id: [u8; 16], created_at_unix_ms: i64) -> RealityPrediction {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id,
            witness_hash: WitnessHash([0x44; 32]),
            task_id: TaskId(format!("replay-test-{}", prediction_id[0])),
            session_id: [0x21; 16],
            language: Language::Rust,
            covered_chunks: Vec::new(),
            verdict: Verdict::Pass,
            confidence_interval: ConformalInterval {
                lower: 0.61,
                upper: 0.83,
                ..ConformalInterval::default()
            },
            predicted_oracle_pass: 0.91,
            predicted_test_pass: vec![0.88],
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.04,
            outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
            calibrated_confidence: 0.77,
            degraded_status: false,
            granger_attestations: std::collections::BTreeMap::from([(
                "replay:test".to_string(),
                0.9,
            )]),
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
            predicted_reasoning_class: ReasoningClass::MostlyCorrect,
            agent_claim_graph: AgentClaimGraph::default(),
            claim_reconciliation: Vec::new(),
            reality_impact: None,
            provenance: PredictionProvenance {
                predictor_version: "replay-test-predictor-v1".to_string(),
                constellation_version: "replay-test-constellation-v1".to_string(),
                calibration_version: "replay-test-calibration-v1".to_string(),
                active_pointer: hex::encode(prediction_id),
                train_health_source: String::new(),
            },
            source_panel_sha: [0x55; 32],
            calibration_version: "replay-test-calibration-v1".to_string(),
            created_at_unix_ms,
            matched_fingerprint: None,
            unknown_candidate_id: None,
            constellation_intelligence: None,
            slot_attributions: Vec::new(),
            label_context: Default::default(),
        })
        .unwrap()
    }

    #[test]
    fn replay_prediction_reads_back_byte_identical_payload() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let store = RocksDbInferStore::new(db.clone());
        let prediction = prediction([0x11; 16], 1_778_650_000_000);
        store.write_live_prediction(&prediction).unwrap();

        let report =
            replay_prediction_by_id(db.as_ref(), Some(temp.path()), PredictionId([0x11; 16]))
                .unwrap();

        assert!(report.byte_for_byte_equal);
        assert!(report.key_payload_equal);
        assert_eq!(report.prediction, prediction);
        assert_eq!(
            report.source_of_truth.column_family,
            CF_MEJEPA_LIVE_PREDICTIONS
        );
    }

    #[test]
    fn replay_prediction_missing_id_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let err = replay_prediction_by_id(db.as_ref(), Some(temp.path()), PredictionId([0xee; 16]))
            .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");
        assert!(err
            .to_string()
            .contains("MEJEPA_PREDICTION_REPLAY_NOT_FOUND"));
    }
}
