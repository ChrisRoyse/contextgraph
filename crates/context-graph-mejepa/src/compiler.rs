use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use context_graph_mejepa_instruments::materialize::{
    materialize_panel, PanelVectorInput, TimeStep,
};
use context_graph_mejepa_instruments::{InstrumentSlot, Panel};
use context_graph_mejepa_tct::{
    gtau_check, EmbedderId as TctEmbedderId, EntityType as TctEntityType, Language as TctLanguage,
    MutationCategory as TctMutationCategory, TctConstellation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::CalibrationStore;
use crate::calibration_types::CalibrationRecord;
use crate::config::MeJepaInferConfig;
use crate::conformal::{build_outcome_set, conformal_set, CalibrationExample};
use crate::dda_features::{project_dda_features, DDA_FEATURE_PROJECTION_SCHEMA};
use crate::degraded::{
    calibrated_confidence, calibrated_confidence_multiplier, compute_train_health, evidence_factor,
};
use crate::error::{MejepaInferError, PredictorError};
use crate::failure_fingerprint::{
    classify_failure_fingerprint_observation, FailureShapeFingerprint, FingerprintClassification,
    FingerprintClassifierConfig, FingerprintDecisionReason,
};
use crate::gates::{check_source_sha_drift, replay_witness_segment};
use crate::hierarchical::build_hierarchical_prediction;
use crate::objective_safety::{evaluate_mejepa_objective, MejepaObjective, ObjectiveSafetyReport};
use crate::ood::{compute_ood_score, squared_l2};
use crate::park_list::ParkListEntry;
use crate::pause_state::{
    current_unix_ms, read_pause_state, PauseReadOutcome, PauseState, PAUSE_REASON_CODE,
};
const PARK_LIST_REASON_CODE: &str = "MEJEPA_PREDICTION_PARKED";
pub const COLD_CELL_INSUFFICIENT_SUPPORT: &str = "COLD_CELL_INSUFFICIENT_SUPPORT";
pub const COLD_CELL_LOOKUP_FAILURE: &str = "COLD_CELL_LOOKUP_FAILURE";
pub const COLD_CELL_Q4_NOTE: &str = "emitted_under_abstain";
use crate::prediction_surfaces::{covered_chunks_for_patch, infer_phase_b_surfaces};
use crate::secret_redaction::{redact_patch_bundle, SECRET_REDACTION_ATTESTATION_KEY};
use crate::system_cost::SystemCostCounters;
use crate::types::{
    validate_probability, AgentClaimGraph, ChunkId, ConformalInterval, ConformalMethod, DdaSignals,
    EmbedderId, FailedGate, HierarchicalPredictionRecord, MatchedFingerprintEvidence,
    OracleOutcome, PanelId, PatchBundle, PhaseBPredictionSurfaces, PredictionProvenance,
    RealityPrediction, RealityPredictionBuilder, TaskContext, Verdict, VerifyVerdict, WitnessHash,
};
use crate::verdict_assembly::{assemble_verdict, VerdictAssemblyInput};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainCertSummary {
    pub step: u64,
    pub delta_omega: f32,
    pub delta_xi: f32,
    pub witness_offset: u64,
}

impl TrainCertSummary {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_probability("train_cert.delta_omega", self.delta_omega)?;
        validate_probability("train_cert.delta_xi", self.delta_xi)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleScores {
    pub predicted_oracle_pass: f32,
    pub predicted_test_pass: Vec<f32>,
    pub predicted_runtime_trace: [f32; 32],
    pub granger_attestations: BTreeMap<String, f32>,
}

impl OracleScores {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_probability("oracle.predicted_oracle_pass", self.predicted_oracle_pass)?;
        if self.predicted_test_pass.is_empty() {
            return Err(MejepaInferError::DimMismatch {
                expected: 1,
                actual: 0,
                context: "oracle predicted_test_pass must be non-empty".to_string(),
            });
        }
        for (idx, value) in self.predicted_test_pass.iter().enumerate() {
            validate_probability(&format!("oracle.predicted_test_pass[{idx}]"), *value)?;
        }
        for (idx, value) in self.predicted_runtime_trace.iter().enumerate() {
            if !value.is_finite() {
                return Err(MejepaInferError::NanDetected {
                    nan_source: "oracle.predicted_runtime_trace".to_string(),
                    detail: format!("predicted_runtime_trace[{idx}] is {value}"),
                });
            }
        }
        for (key, value) in &self.granger_attestations {
            if key.trim().is_empty() {
                return Err(MejepaInferError::InvalidInput {
                    field: "oracle.granger_attestations".to_string(),
                    detail: "attestation keys must be non-empty".to_string(),
                });
            }
            validate_probability(&format!("oracle.granger_attestations[{key}]"), *value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationDecision {
    pub approved: bool,
    pub reason: String,
    pub version_id: String,
    pub age_days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationCellSupport {
    pub cell_id: String,
    pub n_supporting: Option<u32>,
}

impl ConstellationCellSupport {
    pub fn try_new(
        cell_id: impl Into<String>,
        n_supporting: Option<u32>,
    ) -> Result<Self, MejepaInferError> {
        let value = Self {
            cell_id: cell_id.into(),
            n_supporting,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_metric_text("constellation_cell_support.cell_id", &self.cell_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColdCellMetric {
    pub cell_id: String,
    pub reason: String,
    pub n_supporting: Option<u32>,
    pub threshold: u32,
    pub prediction_id: [u8; 16],
    pub task_id: String,
    pub session_id: [u8; 16],
    pub created_at_unix_ms: i64,
}

impl ColdCellMetric {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        cell_id: impl Into<String>,
        reason: impl Into<String>,
        n_supporting: Option<u32>,
        threshold: u32,
        prediction_id: [u8; 16],
        task_id: impl Into<String>,
        session_id: [u8; 16],
        created_at_unix_ms: i64,
    ) -> Result<Self, MejepaInferError> {
        let value = Self {
            cell_id: cell_id.into(),
            reason: reason.into(),
            n_supporting,
            threshold,
            prediction_id,
            task_id: task_id.into(),
            session_id,
            created_at_unix_ms,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_metric_text("cold_cell_metric.cell_id", &self.cell_id)?;
        validate_metric_text("cold_cell_metric.reason", &self.reason)?;
        validate_metric_text("cold_cell_metric.task_id", &self.task_id)?;
        if self.threshold == 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "cold_cell_metric.threshold".to_string(),
                detail: "threshold must be >= 1".to_string(),
            });
        }
        Ok(())
    }
}

pub trait Predictor: Send + Sync {
    fn predict(&self, panel_t0: &Panel, panel_t1: &Panel) -> Result<Panel, MejepaInferError>;
}

pub trait FrozenTarget: Send + Sync {
    fn target(&self, panel_t2: &Panel) -> Result<Panel, MejepaInferError>;
}

pub trait OracleHead: Send + Sync {
    fn score(&self, predicted_panel: &Panel) -> Result<OracleScores, MejepaInferError>;
}

pub trait ConstellationGuard: Send + Sync {
    fn verify(
        &self,
        prediction: &RealityPrediction,
        predicted_panel: &Panel,
        context: &TaskContext,
    ) -> Result<ConstellationDecision, MejepaInferError>;

    fn version_id(&self) -> Option<String>;

    fn target_cell_support(
        &self,
        context: &TaskContext,
    ) -> Result<ConstellationCellSupport, MejepaInferError>;
}

pub trait WitnessChainReader: Send + Sync {
    fn read_segment(&self, patch: &PatchBundle) -> Result<Vec<u8>, MejepaInferError>;
}

pub trait MejepaStore: Send + Sync {
    fn read_recent_train_certs(
        &self,
        limit: usize,
    ) -> Result<Vec<TrainCertSummary>, MejepaInferError>;

    fn write_live_prediction(&self, prediction: &RealityPrediction)
        -> Result<(), MejepaInferError>;

    fn read_live_predictions(
        &self,
        session_id: [u8; 16],
        limit: u32,
    ) -> Result<Vec<RealityPrediction>, MejepaInferError>;

    fn write_hierarchical_prediction(
        &self,
        record: &HierarchicalPredictionRecord,
    ) -> Result<(), MejepaInferError>;

    fn read_hierarchical_predictions(
        &self,
        session_id: [u8; 16],
        limit: u32,
    ) -> Result<Vec<HierarchicalPredictionRecord>, MejepaInferError>;

    fn session_known(&self, session_id: [u8; 16]) -> Result<bool, MejepaInferError>;

    fn read_recent_calibration_examples(
        &self,
        limit: usize,
    ) -> Result<Vec<CalibrationExample>, MejepaInferError>;

    fn read_dda_signals(
        &self,
        panel_id: &PanelId,
        chunk_id: &ChunkId,
    ) -> Result<Option<DdaSignals>, MejepaInferError>;

    fn read_park_list_entry(
        &self,
        prediction_id: [u8; 16],
    ) -> Result<Option<ParkListEntry>, MejepaInferError>;

    fn record_park_list_failure(
        &self,
        prediction_id: [u8; 16],
        now_unix_ms: i64,
        error_code: &str,
    ) -> Result<ParkListEntry, MejepaInferError>;

    fn clear_park_list_entry(&self, prediction_id: [u8; 16]) -> Result<(), MejepaInferError>;

    fn record_cold_cell_metric(&self, metric: &ColdCellMetric) -> Result<(), MejepaInferError>;

    fn read_failure_fingerprint_catalog(
        &self,
    ) -> Result<Vec<FailureShapeFingerprint>, MejepaInferError> {
        Ok(Vec::new())
    }
}

pub struct MeJepaCompiler {
    pub config: MeJepaInferConfig,
    predictor: Arc<dyn Predictor>,
    frozen_target: Arc<dyn FrozenTarget>,
    oracle_head: Arc<dyn OracleHead>,
    constellation_guard: Arc<dyn ConstellationGuard>,
    witness_reader: Arc<dyn WitnessChainReader>,
    store: Arc<dyn MejepaStore>,
    calibration: CalibrationStore,
    repo_root: PathBuf,
    system_cost_counters: Option<Arc<SystemCostCounters>>,
}

impl MeJepaCompiler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: MeJepaInferConfig,
        predictor: Arc<dyn Predictor>,
        frozen_target: Arc<dyn FrozenTarget>,
        oracle_head: Arc<dyn OracleHead>,
        constellation_guard: Arc<dyn ConstellationGuard>,
        witness_reader: Arc<dyn WitnessChainReader>,
        store: Arc<dyn MejepaStore>,
        calibration: CalibrationStore,
        repo_root: PathBuf,
    ) -> Result<Self, MejepaInferError> {
        Self::new_with_system_cost_counters(
            config,
            predictor,
            frozen_target,
            oracle_head,
            constellation_guard,
            witness_reader,
            store,
            calibration,
            repo_root,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_system_cost_counters(
        config: MeJepaInferConfig,
        predictor: Arc<dyn Predictor>,
        frozen_target: Arc<dyn FrozenTarget>,
        oracle_head: Arc<dyn OracleHead>,
        constellation_guard: Arc<dyn ConstellationGuard>,
        witness_reader: Arc<dyn WitnessChainReader>,
        store: Arc<dyn MejepaStore>,
        calibration: CalibrationStore,
        repo_root: PathBuf,
        system_cost_counters: Option<Arc<SystemCostCounters>>,
    ) -> Result<Self, MejepaInferError> {
        config.validate()?;
        Ok(Self {
            config,
            predictor,
            frozen_target,
            oracle_head,
            constellation_guard,
            witness_reader,
            store,
            calibration,
            repo_root,
            system_cost_counters,
        })
    }

    pub fn calibration(&self) -> &CalibrationStore {
        &self.calibration
    }

    pub fn store(&self) -> &dyn MejepaStore {
        self.store.as_ref()
    }

    pub fn classify_failure_fingerprints(
        &self,
        observation_by_embedder: &BTreeMap<crate::types::EmbedderId, Vec<f32>>,
    ) -> Result<FingerprintClassification, MejepaInferError> {
        let catalog = self.store.read_failure_fingerprint_catalog()?;
        classify_failure_fingerprint_observation(
            &catalog,
            observation_by_embedder,
            FingerprintClassifierConfig::default(),
        )
    }

    pub fn compile(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
    ) -> Result<RealityPrediction, MejepaInferError> {
        self.compile_with_panel(patch, context)
            .map(|(prediction, _predicted_panel)| prediction)
    }

    pub fn compile_with_fingerprint_observation(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
        observation_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
    ) -> Result<RealityPrediction, MejepaInferError> {
        let (mut prediction, _predicted_panel) = self.compile_with_panel(patch, context)?;
        let classification = self.classify_failure_fingerprints(observation_by_embedder)?;
        let evidence = evidence_from_fingerprint_classification(
            &classification,
            prediction.verdict,
            context,
            prediction.prediction_id,
        )?;
        prediction.matched_fingerprint = evidence.matched_fingerprint;
        prediction.unknown_candidate_id = evidence.unknown_candidate_id;
        RealityPrediction::try_new(prediction)
    }

    pub fn compile_with_panel(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
    ) -> Result<(RealityPrediction, Panel), MejepaInferError> {
        patch.validate()?;
        context.validate()?;
        if let Some(pause_state) = self.active_pause_state() {
            return self.paused_prediction_with_panel(patch, context, &pause_state);
        }
        let calibration = self.calibration.load_active()?;
        let redacted = redact_patch_bundle(patch)?;
        let inference_patch = &redacted.patch;
        let (panel_t0, panel_t1, panel_t2) =
            materialize_inference_panels(inference_patch, context)?;
        let predict_started = Instant::now();
        let predicted = run_infer_head("panel", || self.predictor.predict(&panel_t0, &panel_t1))?;
        self.record_predictor_elapsed(predict_started);
        let target = run_infer_head("frozen_target", || self.frozen_target.target(&panel_t2))?;
        let mut oracle = run_infer_head("oracle", || self.oracle_head.score(&predicted))?;
        let source_panel_sha = panel_sha(&predicted);
        let covered_chunks = covered_chunks_for_patch(inference_patch)?;
        if redacted.report.redacted_span_count() > 0 {
            oracle
                .granger_attestations
                .insert(SECRET_REDACTION_ATTESTATION_KEY.to_string(), 1.0);
        }
        if self.config.require_dda_features {
            let dda_projection = self.load_dda_projection(&source_panel_sha, &covered_chunks)?;
            oracle
                .granger_attestations
                .extend(dda_projection.to_granger_attestations());
        }
        oracle.validate()?;
        let ood_score =
            compute_ood_score(predicted.data(), target.data(), calibration.sigma_squared)?;
        if ood_score > self.config.ood_refuse_threshold {
            return Err(MejepaInferError::OodRefuse {
                ood_score,
                threshold: self.config.ood_refuse_threshold,
                reason: "predicted panel residual exceeded OOD threshold".to_string(),
            });
        }
        let train_certs = self
            .store
            .read_recent_train_certs(self.config.train_cert_window_steps)?;
        let train_health = compute_train_health(
            &train_certs,
            self.config.bootstrap_delta_omega,
            self.config.bootstrap_delta_xi,
        )?;
        let multiplier = calibrated_confidence_multiplier(
            train_health.delta_omega_mean,
            train_health.delta_xi_mean,
        )?;
        let outcomes = build_outcome_set(
            &oracle.predicted_test_pass,
            context,
            self.config.alpha,
            calibration.tau,
            multiplier,
        )?;
        let outcome_set = conformal_set(outcomes, self.config.alpha, calibration.tau)?;
        let confidence = calibrated_confidence(
            oracle.predicted_oracle_pass,
            1.0 - ood_score,
            mean_attestation(&oracle.granger_attestations, context)?,
            evidence_factor(patch.ast_diff.hunks.len() as u32, 4)?,
            train_health.delta_omega_mean,
            train_health.delta_xi_mean,
        )?;
        let prediction_id = prediction_id(context, patch, &calibration);
        let phase_b_surfaces = run_infer_head("phase_b_surfaces", || {
            infer_phase_b_surfaces(inference_patch, context, &oracle.predicted_test_pass)
        })?;
        let objective_report = evaluate_mejepa_objective(
            MejepaObjective::default(),
            inference_patch,
            &phase_b_surfaces,
            oracle.predicted_oracle_pass,
        )?;
        let confidence_interval = ConformalInterval {
            lower: (confidence - calibration.tau).clamp(0.0, 1.0),
            upper: (confidence + calibration.tau).clamp(0.0, 1.0),
            method: ConformalMethod::SplitConformal,
            coverage_target: 1.0 - self.config.alpha,
            empirical_coverage: calibration.empirical_coverage,
        };
        confidence_interval.validate("confidence_interval")?;
        let verdict = assemble_verdict(VerdictAssemblyInput {
            oracle_pass_confidence: oracle.predicted_oracle_pass,
            failure_modes: &phase_b_surfaces.predicted_failure_modes,
            security_concern_count: phase_b_surfaces.predicted_security_concerns.len(),
            guard_violation_count: phase_b_surfaces.guard_violations.len(),
            ood_score,
            confidence_interval: &confidence_interval,
            pass_threshold: self.config.p_test_threshold,
            ood_threshold: self.config.ood_refuse_threshold,
            interval_width_threshold: 0.8,
            safety_constraint_violation_count: objective_report.constraint_violations.len(),
            objective_total_cost: objective_report.cost.total_cost,
            objective_cost_ceiling: objective_report.objective.pass_cost_ceiling,
        });
        let fingerprint_evidence =
            self.live_fingerprint_evidence_for_panel(&predicted, verdict, context, prediction_id)?;
        let provenance = PredictionProvenance {
            predictor_version: env!("CARGO_PKG_VERSION").to_string(),
            constellation_version: self
                .constellation_guard
                .version_id()
                .unwrap_or_else(|| "unversioned".to_string()),
            calibration_version: calibration.version.clone(),
            active_pointer: hex::encode(prediction_id),
        };
        RealityPredictionBuilder::from_parts(
            context.task_id.clone(),
            context.session_id,
            context.language,
            outcome_set,
        )
        .prediction_id(prediction_id)
        .witness_hash(witness_hash(patch))
        .covered_chunks(covered_chunks)
        .verdict(verdict)
        .confidence_interval(confidence_interval)
        .predicted_oracle_pass(oracle.predicted_oracle_pass)
        .predicted_test_pass(oracle.predicted_test_pass)
        .predicted_runtime_trace(oracle.predicted_runtime_trace)
        .ood_score(ood_score)
        .calibrated_confidence(confidence)
        .degraded_status(train_health.degraded_status)
        .granger_attestations(oracle.granger_attestations)
        .phase_b_surfaces(phase_b_surfaces)
        .agent_claim_graph(AgentClaimGraph::default())
        .provenance(provenance)
        .source_panel_sha(source_panel_sha)
        .calibration_version(calibration.version.clone())
        .matched_fingerprint(fingerprint_evidence.matched_fingerprint)
        .unknown_candidate_id(fingerprint_evidence.unknown_candidate_id)
        .build()
        .map(|prediction| (prediction, predicted))
    }

    fn live_fingerprint_evidence_for_panel(
        &self,
        predicted: &Panel,
        verdict: Verdict,
        context: &TaskContext,
        prediction_id: [u8; 16],
    ) -> Result<FingerprintPredictionEvidence, MejepaInferError> {
        let catalog = self.store.read_failure_fingerprint_catalog()?;
        if catalog.is_empty() {
            return Ok(FingerprintPredictionEvidence::default());
        }
        let Some(observation_by_embedder) =
            panel_observation_for_fingerprint_catalog(predicted, &catalog)
        else {
            tracing::warn!(
                catalog_rows = catalog.len(),
                "failure-fingerprint catalog embedder ids are not panel-slot ids; live matched_fingerprint is unavailable for this prediction"
            );
            return Ok(FingerprintPredictionEvidence::default());
        };
        let classification = classify_failure_fingerprint_observation(
            &catalog,
            &observation_by_embedder,
            FingerprintClassifierConfig::default(),
        )?;
        evidence_from_fingerprint_classification(&classification, verdict, context, prediction_id)
    }

    fn load_dda_projection(
        &self,
        source_panel_sha: &[u8; 32],
        covered_chunks: &[ChunkId],
    ) -> Result<crate::dda_features::DdaFeatureProjection, MejepaInferError> {
        if covered_chunks.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "covered_chunks".to_string(),
                detail: "DDA-required inference needs at least one covered chunk".to_string(),
            });
        }
        let panel_id = PanelId(*source_panel_sha);
        let mut rows = Vec::with_capacity(covered_chunks.len());
        for chunk in covered_chunks {
            let signals = self
                .store
                .read_dda_signals(&panel_id, chunk)?
                .ok_or_else(|| MejepaInferError::DdaFeatureMissing {
                    schema: DDA_FEATURE_PROJECTION_SCHEMA.to_string(),
                    panel_id: hex::encode(panel_id.0),
                    chunk_id: chunk.0.clone(),
                })?;
            rows.push((chunk.clone(), signals));
        }
        project_dda_features(&rows, self.config.dda_expected_embedder_count)
    }

    fn record_predictor_elapsed(&self, started: Instant) {
        let Some(counters) = &self.system_cost_counters else {
            return;
        };
        let micros = started.elapsed().as_micros();
        let bounded = if micros > u128::from(u64::MAX) {
            u64::MAX
        } else {
            micros as u64
        };
        counters.record_cuda_microseconds(bounded.max(1));
    }

    pub fn verify(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
    ) -> Result<VerifyVerdict, MejepaInferError> {
        patch.validate()?;
        context.validate()?;
        if let Some(pause_state) = self.active_pause_state() {
            let (prediction, _panel) =
                self.paused_prediction_with_panel(patch, context, &pause_state)?;
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction),
                failed_gate: FailedGate::PredictionPaused {
                    paused_until_unix_ms: pause_state.paused_until_unix_ms,
                    reason: PAUSE_REASON_CODE.to_string(),
                },
                gates_passed: 0,
            });
        }
        if let Some(failed_gate) = check_source_sha_drift(patch, &self.repo_root)? {
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: None,
                failed_gate,
                gates_passed: 0,
            });
        }

        let witness_segment = self.witness_reader.read_segment(patch)?;
        if let Some(failed_gate) = replay_witness_segment(&witness_segment)? {
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: None,
                failed_gate,
                gates_passed: 1,
            });
        }

        let calibration = self.calibration.load_active()?;
        let candidate_prediction_id = prediction_id(context, patch, &calibration);
        if let Some(entry) = self.store.read_park_list_entry(candidate_prediction_id)? {
            if entry.is_parked_at(current_unix_ms()) {
                let (prediction, _panel) =
                    self.parked_prediction_with_panel(patch, context, &entry)?;
                let park_until_unix_ms =
                    entry
                        .park_until_unix_ms
                        .ok_or_else(|| MejepaInferError::InvalidInput {
                            field: "park_list.park_until_unix_ms".to_string(),
                            detail: "parked entry was missing park_until_unix_ms".to_string(),
                        })?;
                return Ok(VerifyVerdict::EscalateToHuman {
                    reality_prediction: Some(prediction),
                    failed_gate: FailedGate::PredictionParked {
                        attempt_count: entry.attempt_count,
                        park_until_unix_ms,
                        reason: PARK_LIST_REASON_CODE.to_string(),
                    },
                    gates_passed: 2,
                });
            }
        }

        let (prediction, predicted_panel) = match self.compile_with_panel(patch, context) {
            Ok(value) => value,
            Err(MejepaInferError::OodRefuse {
                ood_score,
                threshold,
                ..
            }) => {
                self.store.record_park_list_failure(
                    candidate_prediction_id,
                    current_unix_ms(),
                    "MEJEPA_INFER_OOD_REFUSE",
                )?;
                return Ok(VerifyVerdict::EscalateToHuman {
                    reality_prediction: None,
                    failed_gate: FailedGate::OutOfDistribution {
                        ood_score,
                        threshold,
                    },
                    gates_passed: 2,
                });
            }
            Err(MejepaInferError::Predictor(PredictorError::HeadFailure {
                head,
                code,
                detail,
            })) => {
                let (prediction, _panel) =
                    self.head_failure_prediction_with_panel(patch, context, &head, &code, &detail)?;
                self.store.record_park_list_failure(
                    candidate_prediction_id,
                    current_unix_ms(),
                    "MEJEPA_HEAD_FAILURE",
                )?;
                return Ok(VerifyVerdict::EscalateToHuman {
                    reality_prediction: Some(prediction),
                    failed_gate: FailedGate::HeadFailure { head, code, detail },
                    gates_passed: 2,
                });
            }
            Err(err) => return Err(err),
        };

        if let Some((cold_prediction, failed_gate)) =
            self.cold_cell_abstain_prediction(&prediction, context)?
        {
            let metric = match &failed_gate {
                FailedGate::ColdCell {
                    cell_id,
                    n_supporting,
                    threshold,
                    reason,
                } => ColdCellMetric::try_new(
                    cell_id.clone(),
                    reason.clone(),
                    *n_supporting,
                    *threshold,
                    cold_prediction.prediction_id,
                    cold_prediction.task_id.0.clone(),
                    cold_prediction.session_id,
                    cold_prediction.created_at_unix_ms,
                )?,
                _ => unreachable!("cold_cell_abstain_prediction only returns ColdCell gates"),
            };
            self.store.record_cold_cell_metric(&metric)?;
            self.store.write_live_prediction(&cold_prediction)?;
            let hierarchy =
                build_hierarchical_prediction(&cold_prediction, patch, &predicted_panel)?;
            self.store.write_hierarchical_prediction(&hierarchy)?;
            self.store.record_park_list_failure(
                cold_prediction.prediction_id,
                current_unix_ms(),
                match &failed_gate {
                    FailedGate::ColdCell { reason, .. } => reason,
                    _ => unreachable!("cold_cell_abstain_prediction only returns ColdCell gates"),
                },
            )?;
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(cold_prediction),
                failed_gate,
                gates_passed: 2,
            });
        }

        // Persist compiled predictions before downstream gates so escalations remain replayable.
        self.store.write_live_prediction(&prediction)?;
        let hierarchy = build_hierarchical_prediction(&prediction, patch, &predicted_panel)?;
        self.store.write_hierarchical_prediction(&hierarchy)?;

        let objective_report = objective_report_for_prediction(patch, &prediction)?;
        if objective_report.pass_blocked {
            self.store.record_park_list_failure(
                prediction.prediction_id,
                current_unix_ms(),
                "MEJEPA_SAFETY_CONSTRAINT_REJECTED",
            )?;
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction.clone()),
                failed_gate: FailedGate::SafetyConstraintViolation {
                    violation_count: objective_report.constraint_violations.len(),
                    total_cost: objective_report.cost.total_cost,
                    cost_ceiling: objective_report.objective.pass_cost_ceiling,
                    reason: if objective_report.constraint_violations.is_empty() {
                        "objective cost exceeds pass ceiling".to_string()
                    } else {
                        "hardwired safety constraint violation".to_string()
                    },
                },
                gates_passed: 2,
            });
        }

        if prediction.ood_score > self.config.ood_refuse_threshold {
            self.store.record_park_list_failure(
                prediction.prediction_id,
                current_unix_ms(),
                "MEJEPA_INFER_OOD_REFUSE",
            )?;
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction.clone()),
                failed_gate: FailedGate::OutOfDistribution {
                    ood_score: prediction.ood_score,
                    threshold: self.config.ood_refuse_threshold,
                },
                gates_passed: 2,
            });
        }
        if prediction.predicted_oracle_pass < self.config.p_test_threshold {
            self.store.record_park_list_failure(
                prediction.prediction_id,
                current_unix_ms(),
                "MEJEPA_PREDICTED_FAILURE",
            )?;
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction.clone()),
                failed_gate: FailedGate::PredictedFailure {
                    predicted_oracle_pass: prediction.predicted_oracle_pass,
                    threshold: self.config.p_test_threshold,
                },
                gates_passed: 3,
            });
        }
        if prediction.outcome_set.outcomes.len() > self.config.outcome_set_max {
            self.store.record_park_list_failure(
                prediction.prediction_id,
                current_unix_ms(),
                "MEJEPA_LOW_CONFIDENCE",
            )?;
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction.clone()),
                failed_gate: FailedGate::LowConfidence {
                    outcome_set_len: prediction.outcome_set.outcomes.len(),
                    max_len: self.config.outcome_set_max,
                },
                gates_passed: 4,
            });
        }
        let rejected = prediction
            .granger_attestations
            .iter()
            .filter(|(_, value)| **value < self.config.p_threshold)
            .map(|(key, value)| (key.clone(), *value))
            .collect::<BTreeMap<_, _>>();
        if !rejected.is_empty() {
            self.store.record_park_list_failure(
                prediction.prediction_id,
                current_unix_ms(),
                "MEJEPA_GRANGER_REJECTION",
            )?;
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction.clone()),
                failed_gate: FailedGate::GrangerRejection {
                    rejected,
                    threshold: self.config.p_threshold,
                },
                gates_passed: 5,
            });
        }

        let constellation =
            self.constellation_guard
                .verify(&prediction, &predicted_panel, context)?;
        if constellation.age_days > self.config.max_constellation_age_days {
            return Err(MejepaInferError::GtauStaleConstellation {
                version: constellation.version_id,
                age_days: constellation.age_days,
            });
        }
        if !constellation.approved {
            self.store.record_park_list_failure(
                prediction.prediction_id,
                current_unix_ms(),
                "MEJEPA_CONSTELLATION_REJECTED",
            )?;
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction.clone()),
                failed_gate: FailedGate::ConstellationGuardRejected {
                    reason: constellation.reason,
                },
                gates_passed: 6,
            });
        }

        self.store.clear_park_list_entry(prediction.prediction_id)?;
        Ok(VerifyVerdict::Approve {
            reality_prediction: prediction,
            gates_passed: 7,
        })
    }

    fn cold_cell_abstain_prediction(
        &self,
        prediction: &RealityPrediction,
        context: &TaskContext,
    ) -> Result<Option<(RealityPrediction, FailedGate)>, MejepaInferError> {
        let threshold = self.config.min_cell_support_for_verdict;
        let support = self.constellation_guard.target_cell_support(context)?;
        support.validate()?;
        let reason = match support.n_supporting {
            Some(n_supporting) if n_supporting >= threshold => return Ok(None),
            Some(_) => COLD_CELL_INSUFFICIENT_SUPPORT,
            None => COLD_CELL_LOOKUP_FAILURE,
        };
        let abstain =
            abstain_for_cold_cell(prediction, &support, threshold, reason, self.config.alpha)?;
        let gate = FailedGate::ColdCell {
            cell_id: support.cell_id,
            n_supporting: support.n_supporting,
            threshold,
            reason: reason.to_string(),
        };
        Ok(Some((abstain, gate)))
    }

    fn active_pause_state(&self) -> Option<PauseState> {
        let path = self.config.pause_state_path.as_ref()?;
        match read_pause_state(path, current_unix_ms()) {
            PauseReadOutcome::Active { state } => Some(state),
            PauseReadOutcome::Inactive { .. } => None,
            PauseReadOutcome::IgnoredInvalid { warning } => {
                tracing::warn!(
                    code = %warning.code,
                    state_path = %warning.state_path.display(),
                    message = %warning.message,
                    "ignoring invalid ME-JEPA pause-state file"
                );
                None
            }
        }
    }

    fn paused_prediction_with_panel(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
        pause_state: &PauseState,
    ) -> Result<(RealityPrediction, Panel), MejepaInferError> {
        let panel =
            materialize_panel(TimeStep::T0, vectors_for(TimeStep::T0, patch, context))?.panel;
        let prediction_id = paused_prediction_id(context, patch, pause_state);
        let outcome_set = crate::types::ConformalSet::try_new(
            vec![OracleOutcome::Abstain],
            self.config.alpha,
            1.0,
        )?;
        let mut source_panel_sha = [0u8; 32];
        source_panel_sha.copy_from_slice(&Sha256::digest(PAUSE_REASON_CODE.as_bytes())[..32]);
        let prediction = RealityPredictionBuilder::from_parts(
            context.task_id.clone(),
            context.session_id,
            context.language,
            outcome_set,
        )
        .prediction_id(prediction_id)
        .witness_hash(witness_hash(patch))
        .covered_chunks(covered_chunks_for_patch(patch)?)
        .verdict(Verdict::Abstain)
        .confidence_interval(ConformalInterval::default())
        .predicted_oracle_pass(0.0)
        .predicted_test_pass(vec![0.0])
        .predicted_runtime_trace([0.0; 32])
        .ood_score(0.0)
        .calibrated_confidence(0.0)
        .degraded_status(true)
        .granger_attestations(BTreeMap::from([(PAUSE_REASON_CODE.to_string(), 0.0)]))
        .provenance(PredictionProvenance {
            predictor_version: env!("CARGO_PKG_VERSION").to_string(),
            constellation_version: self
                .constellation_guard
                .version_id()
                .unwrap_or_else(|| "unversioned".to_string()),
            calibration_version: PAUSE_REASON_CODE.to_string(),
            active_pointer: format!("paused_until_{}", pause_state.paused_until_unix_ms),
        })
        .source_panel_sha(source_panel_sha)
        .calibration_version(PAUSE_REASON_CODE)
        .build()?;
        Ok((prediction, panel))
    }

    fn head_failure_prediction_with_panel(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
        head: &str,
        code: &str,
        detail: &str,
    ) -> Result<(RealityPrediction, Panel), MejepaInferError> {
        let redacted = redact_patch_bundle(patch)?;
        let panel = materialize_panel(
            TimeStep::T0,
            vectors_for(TimeStep::T0, &redacted.patch, context),
        )?
        .panel;
        let prediction_id = head_failure_prediction_id(context, patch, head, code);
        let outcome_set = crate::types::ConformalSet::try_new(
            vec![OracleOutcome::Abstain],
            self.config.alpha,
            1.0,
        )?;
        let mut source_panel_sha = [0u8; 32];
        source_panel_sha.copy_from_slice(&Sha256::digest(code.as_bytes())[..32]);
        let prediction = RealityPredictionBuilder::from_parts(
            context.task_id.clone(),
            context.session_id,
            context.language,
            outcome_set,
        )
        .prediction_id(prediction_id)
        .witness_hash(witness_hash(patch))
        .covered_chunks(covered_chunks_for_patch(&redacted.patch)?)
        .verdict(Verdict::Abstain)
        .confidence_interval(ConformalInterval::default())
        .predicted_oracle_pass(0.0)
        .predicted_test_pass(vec![0.0])
        .predicted_runtime_trace([0.0; 32])
        .ood_score(0.0)
        .calibrated_confidence(0.0)
        .degraded_status(true)
        .granger_attestations(BTreeMap::from([
            ("MEJEPA_HEAD_FAILURE".to_string(), 0.0),
            (format!("MEJEPA_HEAD_FAILURE_HEAD:{head}"), 0.0),
        ]))
        .provenance(PredictionProvenance {
            predictor_version: env!("CARGO_PKG_VERSION").to_string(),
            constellation_version: self
                .constellation_guard
                .version_id()
                .unwrap_or_else(|| "unversioned".to_string()),
            calibration_version: "MEJEPA_HEAD_FAILURE".to_string(),
            active_pointer: format!("head={head};code={code};detail_sha={}", short_sha(detail)),
        })
        .source_panel_sha(source_panel_sha)
        .calibration_version("MEJEPA_HEAD_FAILURE")
        .build()?;
        Ok((prediction, panel))
    }

    fn parked_prediction_with_panel(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
        entry: &ParkListEntry,
    ) -> Result<(RealityPrediction, Panel), MejepaInferError> {
        entry.validate()?;
        let redacted = redact_patch_bundle(patch)?;
        let panel = materialize_panel(
            TimeStep::T0,
            vectors_for(TimeStep::T0, &redacted.patch, context),
        )?
        .panel;
        let outcome_set = crate::types::ConformalSet::try_new(
            vec![OracleOutcome::Abstain],
            self.config.alpha,
            1.0,
        )?;
        let mut source_panel_sha = [0u8; 32];
        source_panel_sha.copy_from_slice(&Sha256::digest(PARK_LIST_REASON_CODE.as_bytes())[..32]);
        let prediction = RealityPredictionBuilder::from_parts(
            context.task_id.clone(),
            context.session_id,
            context.language,
            outcome_set,
        )
        .prediction_id(entry.prediction_id)
        .witness_hash(witness_hash(patch))
        .covered_chunks(covered_chunks_for_patch(&redacted.patch)?)
        .verdict(Verdict::Abstain)
        .confidence_interval(ConformalInterval::default())
        .predicted_oracle_pass(0.0)
        .predicted_test_pass(vec![0.0])
        .predicted_runtime_trace([0.0; 32])
        .ood_score(0.0)
        .calibrated_confidence(0.0)
        .degraded_status(true)
        .granger_attestations(BTreeMap::from([(PARK_LIST_REASON_CODE.to_string(), 0.0)]))
        .provenance(PredictionProvenance {
            predictor_version: env!("CARGO_PKG_VERSION").to_string(),
            constellation_version: self
                .constellation_guard
                .version_id()
                .unwrap_or_else(|| "unversioned".to_string()),
            calibration_version: PARK_LIST_REASON_CODE.to_string(),
            active_pointer: format!(
                "parked_until_{};attempts={}",
                entry
                    .park_until_unix_ms
                    .ok_or_else(|| MejepaInferError::InvalidInput {
                        field: "park_list.park_until_unix_ms".to_string(),
                        detail: "parked prediction missing park_until_unix_ms".to_string(),
                    })?,
                entry.attempt_count
            ),
        })
        .source_panel_sha(source_panel_sha)
        .calibration_version(PARK_LIST_REASON_CODE)
        .build()?;
        Ok((prediction, panel))
    }

    pub fn observe(
        &self,
        pipeline: &mut crate::heal::SelfHealingPipeline,
        panel: &Panel,
        oracle_outcome: &crate::types::OracleOutcome,
        signal_clarity: f32,
        witness_chain_offset_in: u64,
        session_id: &str,
    ) -> Result<crate::heal::ObserveOutput, crate::heal::HealError> {
        pipeline.observe(
            panel,
            oracle_outcome,
            signal_clarity,
            witness_chain_offset_in,
            session_id,
        )
    }

    pub fn constellation_version_id(&self) -> Option<String> {
        self.constellation_guard.version_id()
    }
}

pub fn materialize_inference_panels(
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<(Panel, Panel, Panel), MejepaInferError> {
    Ok((
        materialize_panel(TimeStep::T0, vectors_for(TimeStep::T0, patch, context))?.panel,
        materialize_panel(TimeStep::T1, vectors_for(TimeStep::T1, patch, context))?.panel,
        materialize_panel(TimeStep::T2, vectors_for(TimeStep::T2, patch, context))?.panel,
    ))
}

pub fn panel_sha(panel: &Panel) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for value in panel.data() {
        hasher.update(value.to_le_bytes());
    }
    hasher.finalize().into()
}

fn vectors_for(
    time_step: TimeStep,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Vec<PanelVectorInput> {
    let mut seed = Sha256::new();
    seed.update(context.task_id.0.as_bytes());
    seed.update(context.problem_statement.as_bytes());
    seed.update(patch.patch_sha);
    seed.update(patch.commit_message.as_bytes());
    for hunk in &patch.ast_diff.hunks {
        seed.update(hunk.path.to_string_lossy().as_bytes());
        seed.update(hunk.before.as_bytes());
        seed.update(hunk.after.as_bytes());
    }
    seed.update(format!("{time_step:?}").as_bytes());
    let digest: [u8; 32] = seed.finalize().into();
    let scenario = scenario_code(context);
    context_graph_mejepa_instruments::materialize::active_slots(time_step)
        .into_iter()
        .map(|slot| {
            let mut vector = deterministic_vector(slot, &digest);
            if slot == InstrumentSlot::EDiff && !vector.is_empty() {
                vector[0] = scenario as f32;
            }
            PanelVectorInput { slot, vector }
        })
        .collect()
}

fn deterministic_vector(slot: InstrumentSlot, seed: &[u8; 32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(slot.dim());
    for idx in 0..slot.dim() {
        let byte = seed[idx % seed.len()] as f32 / 255.0;
        let wave = ((idx as f32 + 1.0) * 0.017 + byte).sin() * 0.05;
        out.push(byte * 0.10 + wave);
    }
    out
}

fn scenario_code(context: &TaskContext) -> u8 {
    let text = context.problem_statement.as_str();
    if text.contains("scenario: predicted_failure") {
        2
    } else if text.contains("scenario: low_confidence") {
        3
    } else if text.contains("scenario: granger_rejection") {
        4
    } else if text.contains("scenario: constellation_reject") {
        5
    } else if text.contains("scenario: ood") {
        6
    } else {
        1
    }
}

fn prediction_id(
    context: &TaskContext,
    patch: &PatchBundle,
    calibration: &CalibrationRecord,
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(context.task_id.0.as_bytes());
    hasher.update(context.session_id);
    hasher.update(patch.patch_sha);
    hasher.update(calibration.version.as_bytes());
    let digest = hasher.finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

fn head_failure_prediction_id(
    context: &TaskContext,
    patch: &PatchBundle,
    head: &str,
    code: &str,
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(context.task_id.0.as_bytes());
    hasher.update(context.session_id);
    hasher.update(patch.patch_sha);
    hasher.update(b"MEJEPA_HEAD_FAILURE");
    hasher.update(head.as_bytes());
    hasher.update(code.as_bytes());
    let digest = hasher.finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

fn paused_prediction_id(
    context: &TaskContext,
    patch: &PatchBundle,
    pause_state: &PauseState,
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(context.task_id.0.as_bytes());
    hasher.update(context.session_id);
    hasher.update(patch.patch_sha);
    hasher.update(PAUSE_REASON_CODE.as_bytes());
    hasher.update(pause_state.paused_until_unix_ms.to_le_bytes());
    let digest = hasher.finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

fn witness_hash(patch: &PatchBundle) -> WitnessHash {
    let mut hasher = Sha256::new();
    hasher.update(&patch.witness_chain_segment);
    WitnessHash(hasher.finalize().into())
}

fn run_infer_head<T, F>(head: &str, run: F) -> Result<T, MejepaInferError>
where
    F: FnOnce() -> Result<T, MejepaInferError>,
{
    match std::panic::catch_unwind(AssertUnwindSafe(run)) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(MejepaInferError::Predictor(PredictorError::HeadFailure {
            head,
            code,
            detail,
        }))) => Err(MejepaInferError::Predictor(PredictorError::HeadFailure {
            head,
            code,
            detail,
        })),
        Ok(Err(err)) => {
            let code = err.code().to_string();
            let detail = err.to_string();
            Err(MejepaInferError::Predictor(PredictorError::HeadFailure {
                head: head.to_string(),
                code,
                detail,
            }))
        }
        Err(payload) => Err(MejepaInferError::Predictor(PredictorError::HeadFailure {
            head: head.to_string(),
            code: "MEJEPA_HEAD_PANIC".to_string(),
            detail: panic_payload_to_string(payload),
        })),
    }
}

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "head panicked with non-string payload".to_string()
    }
}

fn short_sha(text: &str) -> String {
    hex::encode(&Sha256::digest(text.as_bytes())[..8])
}

fn mean_attestation(
    attestations: &BTreeMap<String, f32>,
    context: &TaskContext,
) -> Result<f32, MejepaInferError> {
    for citation in &context.skill_citations {
        if !attestations.contains_key(&citation.skill_id.0) {
            return Err(MejepaInferError::InvalidInput {
                field: "oracle.granger_attestations".to_string(),
                detail: format!(
                    "missing Granger attestation for cited skill {}",
                    citation.skill_id.0
                ),
            });
        }
    }
    if attestations.is_empty() {
        Ok(1.0)
    } else {
        Ok(attestations.values().sum::<f32>() / attestations.len() as f32)
    }
}

fn abstain_for_cold_cell(
    prediction: &RealityPrediction,
    support: &ConstellationCellSupport,
    threshold: u32,
    reason: &str,
    alpha: f32,
) -> Result<RealityPrediction, MejepaInferError> {
    let mut abstain = prediction.clone();
    abstain.verdict = Verdict::Abstain;
    abstain.confidence_interval = ConformalInterval::default();
    abstain.outcome_set =
        crate::types::ConformalSet::try_new(vec![OracleOutcome::Abstain], alpha, 1.0)?;
    abstain.calibrated_confidence = 0.0;
    abstain.degraded_status = true;
    abstain.granger_attestations.insert(reason.to_string(), 1.0);
    abstain
        .granger_attestations
        .insert(format!("cold_cell_q4_note:{COLD_CELL_Q4_NOTE}"), 1.0);
    abstain.provenance.active_pointer = format!(
        "cold_cell:{}:n={}:threshold={threshold}:note={COLD_CELL_Q4_NOTE}",
        short_sha(&support.cell_id),
        support
            .n_supporting
            .map(|value| value.to_string())
            .unwrap_or_else(|| "lookup_failure".to_string())
    );
    RealityPrediction::try_new(abstain)
}

#[derive(Debug, Clone, Default, PartialEq)]
struct FingerprintPredictionEvidence {
    matched_fingerprint: Option<MatchedFingerprintEvidence>,
    unknown_candidate_id: Option<[u8; 16]>,
}

fn evidence_from_fingerprint_classification(
    classification: &FingerprintClassification,
    prediction_verdict: Verdict,
    context: &TaskContext,
    prediction_id: [u8; 16],
) -> Result<FingerprintPredictionEvidence, MejepaInferError> {
    classification.validate()?;
    if matches!(
        prediction_verdict,
        Verdict::OutOfDistribution | Verdict::GuardRejected
    ) {
        let unknown_candidate_id = if prediction_verdict == Verdict::OutOfDistribution
            && classification.reason == FingerprintDecisionReason::NoKnownMatch
        {
            Some(unknown_candidate_id_for_prediction(
                &context.task_id,
                prediction_id,
                &context.session_id,
            ))
        } else {
            None
        };
        return Ok(FingerprintPredictionEvidence {
            matched_fingerprint: None,
            unknown_candidate_id,
        });
    }

    let matched_fingerprint = classification
        .primary_match
        .as_ref()
        .map(MatchedFingerprintEvidence::from_candidate)
        .transpose()?;
    Ok(FingerprintPredictionEvidence {
        matched_fingerprint,
        unknown_candidate_id: None,
    })
}

fn objective_report_for_prediction(
    patch: &PatchBundle,
    prediction: &RealityPrediction,
) -> Result<ObjectiveSafetyReport, MejepaInferError> {
    let surfaces = PhaseBPredictionSurfaces {
        predicted_failure_modes: prediction.predicted_failure_modes.clone(),
        predicted_failed_tests: prediction.predicted_failed_tests.clone(),
        guard_violations: prediction.guard_violations.clone(),
        closest_exemplars: prediction.closest_exemplars.clone(),
        predicted_edge_cases: prediction.predicted_edge_cases.clone(),
        predicted_latent_bugs: prediction.predicted_latent_bugs.clone(),
        predicted_tech_debt_added: prediction.predicted_tech_debt_added.clone(),
        predicted_dead_code: prediction.predicted_dead_code.clone(),
        predicted_redundant_code: prediction.predicted_redundant_code.clone(),
        predicted_perf_regressions: prediction.predicted_perf_regressions.clone(),
        predicted_security_concerns: prediction.predicted_security_concerns.clone(),
        predicted_accuracy_degradations: prediction.predicted_accuracy_degradations.clone(),
        predicted_cost_regressions: prediction.predicted_cost_regressions.clone(),
        predicted_reasoning_class: prediction.predicted_reasoning_class,
    };
    evaluate_mejepa_objective(
        MejepaObjective::default(),
        patch,
        &surfaces,
        prediction.predicted_oracle_pass,
    )
}

fn panel_observation_for_fingerprint_catalog(
    predicted: &Panel,
    catalog: &[FailureShapeFingerprint],
) -> Option<BTreeMap<EmbedderId, Vec<f32>>> {
    let mut observation = BTreeMap::new();
    for fingerprint in catalog {
        for embedder in fingerprint.centroid_by_embedder.keys() {
            if observation.contains_key(embedder) {
                continue;
            }
            let slot = InstrumentSlot::all()
                .iter()
                .copied()
                .find(|slot| slot.slug() == embedder.0.as_str())?;
            observation.insert(embedder.clone(), predicted.slot(slot).to_vec());
        }
    }
    if observation.is_empty() {
        None
    } else {
        Some(observation)
    }
}

fn unknown_candidate_id_for_prediction(
    task_id: &crate::types::TaskId,
    prediction_id: [u8; 16],
    session_id: &[u8; 16],
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_LIVE_UNKNOWN_FINGERPRINT_CANDIDATE_V1");
    hasher.update(task_id.0.as_bytes());
    hasher.update(prediction_id);
    hasher.update(session_id);
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn validate_metric_text(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "must be non-empty".to_string(),
        });
    }
    if value.len() > 512 {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "exceeds 512 bytes".to_string(),
        });
    }
    if value.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "contains a control character".to_string(),
        });
    }
    Ok(())
}

#[derive(Default)]
pub struct PatchWitnessReader;

impl WitnessChainReader for PatchWitnessReader {
    fn read_segment(&self, patch: &PatchBundle) -> Result<Vec<u8>, MejepaInferError> {
        Ok(patch.witness_chain_segment.clone())
    }
}

#[derive(Default)]
pub struct DeterministicPredictor;

impl Predictor for DeterministicPredictor {
    fn predict(&self, _panel_t0: &Panel, panel_t1: &Panel) -> Result<Panel, MejepaInferError> {
        let scenario = panel_t1.slot(InstrumentSlot::EDiff)[0].round() as u8;
        synthetic_panel_for_scenario(scenario, scenario == 6)
    }
}

#[derive(Default)]
pub struct IdentityFrozenTarget;

impl FrozenTarget for IdentityFrozenTarget {
    fn target(&self, panel_t2: &Panel) -> Result<Panel, MejepaInferError> {
        let scenario = panel_t2.slot(InstrumentSlot::EDiff)[0].round() as u8;
        synthetic_panel_for_scenario(scenario, false)
    }
}

#[derive(Default)]
pub struct DeterministicOracleHead;

impl OracleHead for DeterministicOracleHead {
    fn score(&self, predicted_panel: &Panel) -> Result<OracleScores, MejepaInferError> {
        let scenario = predicted_panel.slot(InstrumentSlot::EDiff)[0].round() as u8;
        let mut granger = BTreeMap::from([("skill:python_unit".to_string(), 0.90)]);
        let (predicted_oracle_pass, predicted_test_pass) = match scenario {
            2 => (0.20, vec![0.20]),
            3 => (0.95, vec![0.50]),
            4 => {
                granger.insert("skill:python_unit".to_string(), 0.001);
                (0.95, vec![0.95])
            }
            _ => (0.95, vec![0.95]),
        };
        Ok(OracleScores {
            predicted_oracle_pass,
            predicted_test_pass,
            predicted_runtime_trace: [0.001; 32],
            granger_attestations: granger,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DeterministicConstellationGuard {
    pub version_id: String,
    pub age_days: u32,
    pub cell_id: String,
    pub n_supporting: Option<u32>,
}

impl Default for DeterministicConstellationGuard {
    fn default() -> Self {
        Self {
            version_id: "tct-fsv-v1".to_string(),
            age_days: 0,
            cell_id: "mutation=known_good:language=python:entity=function".to_string(),
            n_supporting: Some(100),
        }
    }
}

impl ConstellationGuard for DeterministicConstellationGuard {
    fn verify(
        &self,
        _prediction: &RealityPrediction,
        _predicted_panel: &Panel,
        context: &TaskContext,
    ) -> Result<ConstellationDecision, MejepaInferError> {
        let approved = !context
            .problem_statement
            .contains("scenario: constellation_reject");
        Ok(ConstellationDecision {
            approved,
            reason: if approved {
                "constellation accepted".to_string()
            } else {
                "scenario requested constellation rejection".to_string()
            },
            version_id: self.version_id.clone(),
            age_days: self.age_days,
        })
    }

    fn version_id(&self) -> Option<String> {
        Some(self.version_id.clone())
    }

    fn target_cell_support(
        &self,
        _context: &TaskContext,
    ) -> Result<ConstellationCellSupport, MejepaInferError> {
        ConstellationCellSupport::try_new(self.cell_id.clone(), self.n_supporting)
    }
}

#[derive(Debug, Clone)]
pub struct TctConstellationGuard {
    constellation: TctConstellation,
    predicted_class: TctMutationCategory,
    entity_type: TctEntityType,
}

impl TctConstellationGuard {
    pub fn new(
        constellation: TctConstellation,
        predicted_class: TctMutationCategory,
        entity_type: TctEntityType,
    ) -> Result<Self, MejepaInferError> {
        constellation.validate_integrity()?;
        Ok(Self {
            constellation,
            predicted_class,
            entity_type,
        })
    }
}

impl ConstellationGuard for TctConstellationGuard {
    fn verify(
        &self,
        _prediction: &RealityPrediction,
        predicted_panel: &Panel,
        context: &TaskContext,
    ) -> Result<ConstellationDecision, MejepaInferError> {
        let language = tct_language(context.language);
        let (max_age_days, allow_stale) = context_graph_mejepa_tct::read_freshness_config()?;
        let target_cells = context_graph_mejepa_tct::materialize_target_cells(
            &self.constellation,
            self.predicted_class,
            language,
            self.entity_type,
            &BTreeMap::new(),
        )?;
        let freshness_audit = context_graph_mejepa_tct::build_freshness_audit(
            self.constellation.version_id(),
            &target_cells,
            std::time::SystemTime::now(),
            context_graph_mejepa_tct::RefreshPolicyConfig {
                max_age_days,
                ..context_graph_mejepa_tct::RefreshPolicyConfig::default()
            },
        )?;
        if !allow_stale {
            if let Some(stale) = freshness_audit
                .rows
                .iter()
                .find(|row| row.decision.is_refit())
            {
                return Err(MejepaInferError::GtauStaleConstellation {
                    version: hex::encode(self.constellation.version_id()),
                    age_days: stale.age_days,
                });
            }
        }
        let output = gtau_check(
            predicted_panel,
            self.predicted_class,
            language,
            self.entity_type,
            &self.constellation,
        )?;
        let age_days = constellation_age_days(self.constellation.frozen_at)?;
        Ok(ConstellationDecision {
            approved: output.gtau_satisfied,
            reason: if output.gtau_satisfied {
                format!(
                    "Gtau accepted all {} embedders; min_margin={:.6}",
                    output.evaluated_embedder_count, output.min_margin
                )
            } else {
                let first = output
                    .violations
                    .first()
                    .map(|violation| {
                        format!(
                            "{} observed_cos={:.6} threshold={:.6}",
                            violation.embedder, violation.observed_cos, violation.threshold
                        )
                    })
                    .unwrap_or_else(|| "unknown violation".to_string());
                format!(
                    "Gtau rejected {} violating embedders; first={first}",
                    output.violations.len()
                )
            },
            version_id: hex::encode(self.constellation.version_id()),
            age_days,
        })
    }

    fn version_id(&self) -> Option<String> {
        Some(hex::encode(self.constellation.version_id()))
    }

    fn target_cell_support(
        &self,
        context: &TaskContext,
    ) -> Result<ConstellationCellSupport, MejepaInferError> {
        let language = tct_language(context.language);
        let mut min_support: Option<u32> = None;
        for embedder in TctEmbedderId::all() {
            let Some((_centroid, _origin, sample_count)) = self.constellation.lookup_centroid(
                self.predicted_class,
                language,
                self.entity_type,
                embedder,
            ) else {
                return ConstellationCellSupport::try_new(
                    tct_cell_id(self.predicted_class, language, self.entity_type),
                    None,
                );
            };
            let sample_count =
                u32::try_from(sample_count).map_err(|_| MejepaInferError::InvalidInput {
                    field: "constellation.sample_count".to_string(),
                    detail: format!("sample_count {sample_count} exceeds u32::MAX"),
                })?;
            min_support = Some(match min_support {
                Some(current) => current.min(sample_count),
                None => sample_count,
            });
        }
        ConstellationCellSupport::try_new(
            tct_cell_id(self.predicted_class, language, self.entity_type),
            min_support,
        )
    }
}

pub fn residual_norm_sq(predicted: &Panel, target: &Panel) -> Result<f32, MejepaInferError> {
    squared_l2(predicted.data(), target.data())
}

fn synthetic_panel_for_scenario(scenario: u8, force_ood: bool) -> Result<Panel, MejepaInferError> {
    let base = if force_ood {
        2.0
    } else {
        scenario as f32 * 0.001
    };
    let mut data = vec![base; context_graph_mejepa_instruments::PANEL_DIM];
    let offset = InstrumentSlot::EDiff.offset();
    data[offset] = scenario as f32;
    let filled_mask = (1u16 << InstrumentSlot::all().len()) - 1;
    Ok(Panel::try_new(data, filled_mask)?)
}

fn tct_language(language: crate::types::Language) -> TctLanguage {
    match language {
        crate::types::Language::Rust => TctLanguage::Rust,
        crate::types::Language::Python => TctLanguage::Python,
        crate::types::Language::Javascript => TctLanguage::Javascript,
        crate::types::Language::Typescript => TctLanguage::Typescript,
        crate::types::Language::Go => TctLanguage::Go,
        crate::types::Language::Java => TctLanguage::Java,
        crate::types::Language::C => TctLanguage::C,
        crate::types::Language::Cpp => TctLanguage::Cpp,
        crate::types::Language::CSharp => TctLanguage::CSharp,
        crate::types::Language::Ruby => TctLanguage::Ruby,
        crate::types::Language::Php => TctLanguage::Php,
    }
}

fn constellation_age_days(frozen_at: std::time::SystemTime) -> Result<u32, MejepaInferError> {
    let age = std::time::SystemTime::now()
        .duration_since(frozen_at)
        .map_err(|_| MejepaInferError::InvalidInput {
            field: "constellation.frozen_at".to_string(),
            detail: "constellation frozen_at is in the future".to_string(),
        })?;
    Ok((age.as_secs() / 86_400) as u32)
}

fn tct_cell_id(
    mutation: TctMutationCategory,
    language: TctLanguage,
    entity_type: TctEntityType,
) -> String {
    format!("mutation={mutation:?}:language={language:?}:entity={entity_type:?}")
}
