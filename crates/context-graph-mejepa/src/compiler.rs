use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use context_graph_mejepa_instruments::materialize::{
    materialize_panel, PanelVectorInput, TimeStep,
};
use context_graph_mejepa_instruments::{
    CodeInstrumentInput, Diagnostic, DiagnosticSeverity, DiffInstrumentInput, EAstInstrument,
    ECfgInstrument, ECommitMsgInstrument, EDataFlowInstrument, EDiffInstrument, EOracleInstrument,
    EProblemInstrument, EReasoningInstrument, ERuntimeInstrument, EStaticAnalysisInstrument,
    ETestInstrument, ETraceInstrument, ETypeGraphInstrument, EWitnessInstrument, Instrument,
    InstrumentSlot, OracleVerdict as InstrumentOracleVerdict, Panel, ReasoningEvent,
    ReasoningInput, RuntimeInput, ScalarInput, ScalarsInstrument, StaticAnalysisInput,
    TextInstrumentInput, TraceInput, WitnessChainInput, CANONICAL_WITNESS_FORMAT_VERSION,
};
use context_graph_mejepa_tct::{
    gtau_check, EmbedderId as TctEmbedderId, EntityType as TctEntityType, Language as TctLanguage,
    MutationCategory as TctMutationCategory, TctConstellation,
};
use ruff_text_size::Ranged;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::CalibrationStore;
use crate::calibration_types::CalibrationRecord;
use crate::config::MeJepaInferConfig;
use crate::conformal::{build_outcome_set, conformal_set, CalibrationExample};
use crate::constellation_intelligence::{
    default_active_embedder_slot_ids, granger_attestations_from_constellation_intelligence,
    summarize_dda_constellation_intelligence, ConstellationIntelligenceEvidence,
};
use crate::contradiction::{
    ContradictionDecision, ContradictionDecisionKind, ContradictionThresholds,
    CONTRADICTION_THRESHOLD_MISSING, MULTI_HEAD_CONTRADICTION,
};
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
use crate::ood::{per_slot_residual_scores, SlotResidualScore};
use crate::ood_harvest::{
    apply_ood_gate_decision, OodCalibrationReport, OodGateDecision, OOD_CALIBRATOR_MISSING,
};
use crate::park_list::ParkListEntry;
use crate::patch_similarity::{
    closest_exemplars, load_patch_similarity_index, patch_structural_signature_from_panel,
    PatchSimilarityQuery, PATCH_SIMILARITY_DEFAULT_K,
};
use crate::pause_state::{
    current_unix_ms, read_pause_state, PauseReadOutcome, PauseState, PAUSE_REASON_CODE,
};
const PARK_LIST_REASON_CODE: &str = "MEJEPA_PREDICTION_PARKED";
pub const COLD_CELL_INSUFFICIENT_SUPPORT: &str = "COLD_CELL_INSUFFICIENT_SUPPORT";
pub const COLD_CELL_LOOKUP_FAILURE: &str = "COLD_CELL_LOOKUP_FAILURE";
pub const COLD_CELL_Q4_NOTE: &str = "emitted_under_abstain";
use crate::prediction_surfaces::{
    covered_chunks_for_patch, enrich_predicted_works_with_known_good_exemplars,
    infer_phase_b_surfaces,
};
use crate::secret_redaction::{redact_patch_bundle, SECRET_REDACTION_ATTESTATION_KEY};
use crate::system_cost::SystemCostCounters;
use crate::types::{
    validate_probability, AgentClaimGraph, ChunkId, ConformalInterval, ConformalMethod, DdaSignals,
    EmbedderId, ExemplarMatch, FailedGate, HierarchicalPredictionRecord,
    MatchedFingerprintEvidence, OracleOutcome, PanelId, PatchBundle, PerSlotOodReason,
    PerSlotOodReasonKind, PhaseBPredictionSurfaces, PredictionProvenance, RealityPrediction,
    RealityPredictionBuilder, TaskContext, Verdict, VerifyVerdict, WitnessHash,
};
use crate::verdict_assembly::{assemble_verdict_with_evidence, VerdictAssemblyInput};

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

    fn read_contradiction_thresholds(
        &self,
        cell_id: &str,
    ) -> Result<Option<ContradictionThresholds>, MejepaInferError> {
        let _ = cell_id;
        Ok(None)
    }

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

    fn read_latest_ood_calibration_report(
        &self,
    ) -> Result<Option<OodCalibrationReport>, MejepaInferError> {
        Ok(None)
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
        let mut constellation_intelligence: Option<ConstellationIntelligenceEvidence> = None;
        if self.config.require_dda_features {
            let dda_rows = self.load_dda_rows(&source_panel_sha, &covered_chunks)?;
            let dda_projection =
                project_dda_features(&dda_rows, self.config.dda_expected_embedder_count)?;
            oracle
                .granger_attestations
                .extend(dda_projection.to_granger_attestations());
            let slot_ids = default_active_embedder_slot_ids(dda_projection.embedder_count);
            let intelligence = summarize_dda_constellation_intelligence(
                &slot_ids,
                &dda_rows,
                format!("live_inference:{}", hex::encode(source_panel_sha)),
            )?;
            oracle.granger_attestations.extend(
                granger_attestations_from_constellation_intelligence(&intelligence)?,
            );
            constellation_intelligence = Some(intelligence);
        }
        oracle.validate()?;
        let ood_threshold = self.ood_threshold_for_compilation()?;
        let slot_ood_scores =
            per_slot_residual_scores(&predicted, &target, calibration.sigma_squared)?;
        let ood_score = slot_ood_scores
            .iter()
            .map(|score| score.score)
            .fold(0.0_f32, f32::max);
        let per_slot_ood_assessment = per_slot_ood_assessment_from_residual_scores(
            &slot_ood_scores,
            ood_threshold,
            &calibration.version,
        )?;
        let mut per_slot_ood_reasons = per_slot_ood_assessment.reasons;
        let slot_specific_ood_guard_count = per_slot_ood_assessment.slot_specific_guard_count;
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
        let mut phase_b_surfaces = run_infer_head("phase_b_surfaces", || {
            infer_phase_b_surfaces(inference_patch, context, &oracle.predicted_test_pass)
        })?;
        phase_b_surfaces.closest_exemplars = self.closest_exemplars_for_panel(&predicted)?;
        enrich_predicted_works_with_known_good_exemplars(&mut phase_b_surfaces);
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
        let contradiction_cell = self.constellation_guard.target_cell_support(context)?;
        let contradiction_thresholds = self
            .store
            .read_contradiction_thresholds(&contradiction_cell.cell_id)?;
        let verdict_output = assemble_verdict_with_evidence(VerdictAssemblyInput {
            contradiction_cell_id: Some(&contradiction_cell.cell_id),
            contradiction_thresholds: contradiction_thresholds.as_ref(),
            oracle_pass_confidence: oracle.predicted_oracle_pass,
            failure_modes: &phase_b_surfaces.predicted_failure_modes,
            predicted_failed_test_count: phase_b_surfaces.predicted_failed_tests.len(),
            predicted_works: &phase_b_surfaces.predicted_works,
            security_concern_count: phase_b_surfaces.predicted_security_concerns.len(),
            guard_violation_count: phase_b_surfaces.guard_violations.len()
                + slot_specific_ood_guard_count,
            ood_score,
            confidence_interval: &confidence_interval,
            pass_threshold: self.config.p_test_threshold,
            ood_threshold,
            interval_width_threshold: 0.8,
            safety_constraint_violation_count: objective_report.constraint_violations.len(),
            objective_total_cost: objective_report.cost.total_cost,
            objective_cost_ceiling: objective_report.objective.pass_cost_ceiling,
            // F-003 fix: when DDA features are required (the doctrinal Q2-pressure
            // configuration), `constellation_intelligence` MUST be `Some` because it is
            // unconditionally populated above when `require_dda_features = true`. A `None`
            // here is therefore an internal invariant violation; we fail closed rather
            // than silently substitute `0.0` for the Q2 verdict pressure feature.
            // When DDA features are explicitly disabled (`require_dda_features = false`),
            // `0.0` is the documented contract — the Q2-pressure surface is opt-in.
            constellation_verdict_pressure: match (
                &constellation_intelligence,
                self.config.require_dda_features,
            ) {
                (Some(evidence), _) => evidence.pressures.q2_verdict_pressure,
                (None, false) => 0.0,
                (None, true) => {
                    return Err(MejepaInferError::ConstellationIntelligenceUnavailable {
                        prediction_id_hex: hex::encode(prediction_id),
                    });
                }
            },
        });
        let verdict = verdict_output.verdict;
        annotate_contradiction_decision(
            &mut oracle.granger_attestations,
            &verdict_output.contradiction,
        )?;
        per_slot_ood_reasons.extend(per_slot_ood_reasons_from_guard_violations(
            &phase_b_surfaces.guard_violations,
            &calibration.version,
        )?);
        let fingerprint_evidence =
            self.live_fingerprint_evidence_for_panel(&predicted, verdict, context, prediction_id)?;
        // F-005 fix: chain-of-custody requires a reproducible constellation version_id.
        // Both production `ConstellationGuard` impls (DeterministicConstellationGuard and
        // TctConstellationGuard) unconditionally return `Some(_)`; the previous
        // `unwrap_or_else(|| "unversioned".to_string())` masked a future contract drift
        // (e.g. a stub guard accidentally returning None) by stamping
        // `PredictionProvenance::constellation_version = "unversioned"` and silently
        // breaking reproducibility of the witness chain.
        let constellation_version = self.constellation_guard.version_id().ok_or(
            MejepaInferError::ConstellationVersionIdMissing {
                detail: "ConstellationGuard::version_id() returned None; PredictionProvenance.constellation_version must be reproducible (CLAUDE.md §1 Q2 chain-of-custody)".to_string(),
            },
        )?;
        let provenance = PredictionProvenance {
            predictor_version: env!("CARGO_PKG_VERSION").to_string(),
            constellation_version,
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
        .per_slot_ood_reasons(per_slot_ood_reasons)
        .agent_claim_graph(AgentClaimGraph::default())
        .provenance(provenance)
        .source_panel_sha(source_panel_sha)
        .calibration_version(calibration.version.clone())
        .matched_fingerprint(fingerprint_evidence.matched_fingerprint)
        .unknown_candidate_id(fingerprint_evidence.unknown_candidate_id)
        .constellation_intelligence(constellation_intelligence)
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

    fn closest_exemplars_for_panel(
        &self,
        predicted: &Panel,
    ) -> Result<Vec<ExemplarMatch>, MejepaInferError> {
        let Some(index_dir) = self.config.patch_similarity_index_dir.as_ref() else {
            return Ok(Vec::new());
        };
        let expected_hash = self
            .config
            .patch_similarity_corpus_snapshot_hash
            .as_deref()
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "patch_similarity_corpus_snapshot_hash".to_string(),
                detail: "configured patch-similarity index requires an expected snapshot hash"
                    .to_string(),
            })?;
        let index = load_patch_similarity_index(index_dir, expected_hash)?;
        let query_vector = patch_structural_signature_from_panel(predicted)?;
        let query = PatchSimilarityQuery::new(query_vector, PATCH_SIMILARITY_DEFAULT_K, true)?;
        Ok(closest_exemplars(&index, query)?.exemplars)
    }

    fn load_dda_rows(
        &self,
        source_panel_sha: &[u8; 32],
        covered_chunks: &[ChunkId],
    ) -> Result<Vec<(ChunkId, DdaSignals)>, MejepaInferError> {
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
        Ok(rows)
    }

    fn ood_threshold_for_compilation(&self) -> Result<f32, MejepaInferError> {
        ood_threshold_from_selected_report(
            self.load_selected_ood_calibration_report()?.as_ref(),
            self.config.require_ood_calibrator,
            self.config.ood_refuse_threshold,
        )
    }

    fn ood_gate_decision(
        &self,
        prediction: &RealityPrediction,
    ) -> Result<OodGateDecision, MejepaInferError> {
        let report = self.load_selected_ood_calibration_report()?;
        if report.is_none() && !self.config.require_ood_calibrator {
            if prediction.ood_score > self.config.ood_refuse_threshold {
                return Ok(OodGateDecision {
                    verdict: Verdict::OutOfDistribution,
                    reason: "OOD_SCORE_ABOVE_STATIC_THRESHOLD".to_string(),
                    ood_score: prediction.ood_score,
                    threshold: Some(self.config.ood_refuse_threshold),
                });
            }
            return Ok(OodGateDecision {
                verdict: prediction.verdict,
                reason: "OOD_SCORE_WITHIN_STATIC_THRESHOLD".to_string(),
                ood_score: prediction.ood_score,
                threshold: Some(self.config.ood_refuse_threshold),
            });
        }
        apply_ood_gate_decision(
            prediction.verdict,
            prediction.ood_score,
            report.as_ref(),
            None,
        )
        .map_err(eval_to_infer_error)
    }

    fn load_selected_ood_calibration_report(
        &self,
    ) -> Result<Option<OodCalibrationReport>, MejepaInferError> {
        let latest = match self.store.read_latest_ood_calibration_report() {
            Ok(value) => value,
            Err(err) if self.config.require_ood_calibrator => {
                tracing::warn!(
                    error = %err,
                    reason = OOD_CALIBRATOR_MISSING,
                    "OOD calibrator read failed; strict verifier will fail closed"
                );
                return Ok(None);
            }
            Err(err) => return Err(err),
        };
        let Some(report) = latest else {
            return Ok(None);
        };
        if report.selected_for_serving {
            return Ok(Some(report));
        }
        if self.config.require_ood_calibrator {
            tracing::warn!(
                report_id = %report.report_id,
                flags = ?report.flags,
                reason = OOD_CALIBRATOR_MISSING,
                "latest OOD calibrator is not selected for serving; strict verifier will fail closed"
            );
            return Ok(None);
        }
        Ok(None)
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

        let (mut prediction, predicted_panel) = match self.compile_with_panel(patch, context) {
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

        let ood_decision = self.ood_gate_decision(&prediction)?;
        apply_ood_gate_to_prediction(&mut prediction, &ood_decision, context)?;

        // Persist compiled predictions before downstream gates so escalations remain replayable.
        self.store.write_live_prediction(&prediction)?;
        let hierarchy = build_hierarchical_prediction(&prediction, patch, &predicted_panel)?;
        self.store.write_hierarchical_prediction(&hierarchy)?;

        if ood_gate_failed(&ood_decision) {
            self.store.record_park_list_failure(
                prediction.prediction_id,
                current_unix_ms(),
                &ood_decision.reason,
            )?;
            let failed_gate = match ood_decision.verdict {
                Verdict::OutOfDistribution => FailedGate::OutOfDistribution {
                    ood_score: ood_decision.ood_score,
                    threshold: ood_decision
                        .threshold
                        .unwrap_or(self.config.ood_refuse_threshold),
                },
                Verdict::GuardRejected => FailedGate::OodGateRejected {
                    reason: ood_decision.reason.clone(),
                    ood_score: ood_decision.ood_score,
                    threshold: ood_decision.threshold,
                },
                _ => unreachable!("OOD gate only escalates OOD or guard-rejected verdicts"),
            };
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction.clone()),
                failed_gate,
                gates_passed: 2,
            });
        }

        if let Some(failed_gate) = self.contradiction_failed_gate(&prediction, context)? {
            let reason = match &failed_gate {
                FailedGate::MultiHeadContradiction { reason, .. } => reason.as_str(),
                _ => unreachable!("contradiction_failed_gate only returns MultiHeadContradiction"),
            };
            self.store.record_park_list_failure(
                prediction.prediction_id,
                current_unix_ms(),
                reason,
            )?;
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction.clone()),
                failed_gate,
                gates_passed: 2,
            });
        }

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

    fn contradiction_failed_gate(
        &self,
        prediction: &RealityPrediction,
        context: &TaskContext,
    ) -> Result<Option<FailedGate>, MejepaInferError> {
        if prediction.verdict != Verdict::Abstain {
            return Ok(None);
        }
        let reason = prediction
            .granger_attestations
            .keys()
            .find_map(|key| key.strip_prefix("contradiction:"))
            .filter(|reason| {
                *reason == MULTI_HEAD_CONTRADICTION
                    || *reason == CONTRADICTION_THRESHOLD_MISSING
                    || reason.starts_with("CONTRADICTION_CALIBRATION_INVALID:")
            })
            .map(ToString::to_string);
        let Some(reason) = reason else {
            return Ok(None);
        };
        let support = self.constellation_guard.target_cell_support(context)?;
        let thresholds = self.store.read_contradiction_thresholds(&support.cell_id)?;
        let high_severity_failure_count = prediction
            .predicted_failure_modes
            .iter()
            .filter(|mode| {
                matches!(
                    mode.severity,
                    crate::types::Severity::High | crate::types::Severity::Critical
                ) && mode.confidence > 0.6
            })
            .count() as u32;
        Ok(Some(FailedGate::MultiHeadContradiction {
            cell_id: support.cell_id,
            reason,
            oracle_pass_confidence: prediction.predicted_oracle_pass,
            tau_oracle: thresholds.as_ref().map(|thresholds| thresholds.tau_oracle),
            high_severity_failure_count,
            tau_failure_count: thresholds
                .as_ref()
                .map(|thresholds| thresholds.tau_failure_count),
            security_concern_count: prediction.predicted_security_concerns.len() as u32,
        }))
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
        let panel = materialize_inference_panel(TimeStep::T0, patch, context)?;
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
        let panel = materialize_inference_panel(TimeStep::T0, &redacted.patch, context)?;
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
        let panel = materialize_inference_panel(TimeStep::T0, &redacted.patch, context)?;
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
    patch.validate()?;
    context.validate()?;
    let inputs = build_inference_panel_inputs(patch, context)?;
    Ok((
        materialize_panel(TimeStep::T0, vectors_for(TimeStep::T0, &inputs)?)?.panel,
        materialize_panel(TimeStep::T1, vectors_for(TimeStep::T1, &inputs)?)?.panel,
        materialize_panel(TimeStep::T2, vectors_for(TimeStep::T2, &inputs)?)?.panel,
    ))
}

pub fn panel_sha(panel: &Panel) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for value in panel.data() {
        hasher.update(value.to_le_bytes());
    }
    hasher.finalize().into()
}

fn materialize_inference_panel(
    time_step: TimeStep,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<Panel, MejepaInferError> {
    patch.validate()?;
    context.validate()?;
    let inputs = build_inference_panel_inputs(patch, context)?;
    Ok(materialize_panel(time_step, vectors_for(time_step, &inputs)?)?.panel)
}

#[derive(Debug, Clone)]
struct InferencePanelInputs {
    before_code: CodeInstrumentInput,
    after_code: CodeInstrumentInput,
    before_invalid_source: Option<InvalidSourceDiagnostic>,
    after_invalid_source: Option<InvalidSourceDiagnostic>,
    diff: DiffInstrumentInput,
    tests: TextInstrumentInput,
    problem: TextInstrumentInput,
    commit: TextInstrumentInput,
    witness: WitnessChainInput,
    oracle: InstrumentOracleVerdict,
    trace: TraceInput,
    static_analysis: StaticAnalysisInput,
    runtime: RuntimeInput,
    reasoning: ReasoningInput,
    scalars: ScalarInput,
}

#[derive(Debug, Clone)]
struct InvalidSourceDiagnostic {
    side: SourceSide,
    language: String,
    path: String,
    kind: String,
    detail: String,
    line: u32,
    column: u32,
    source_len_bytes: usize,
    source_line_count: usize,
    empty_source: bool,
}

fn build_inference_panel_inputs(
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<InferencePanelInputs, MejepaInferError> {
    let language = language_slug(context.language).to_string();
    let path = panel_source_path(patch);
    let before_source = panel_source_text(patch, SourceSide::Before)?;
    let after_source = panel_source_text(patch, SourceSide::After)?;
    let before_code = CodeInstrumentInput {
        language: language.clone(),
        path: path.clone(),
        source: before_source.clone(),
    };
    let after_code = CodeInstrumentInput {
        language: language.clone(),
        path: path.clone(),
        source: after_source.clone(),
    };
    let before_invalid_source = invalid_source_diagnostic(&before_code, SourceSide::Before);
    let after_invalid_source = invalid_source_diagnostic(&after_code, SourceSide::After);
    let tests_text = context
        .tests
        .iter()
        .map(|test| test.0.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let changed_lines = changed_line_count(patch)?;
    let file_count = patch
        .ast_diff
        .hunks
        .iter()
        .map(|hunk| hunk.path.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let static_analysis = if let Some(diagnostic) = &after_invalid_source {
        StaticAnalysisInput {
            source_text: after_source.clone(),
            diagnostics: vec![static_analysis_diagnostic(diagnostic)],
            churn_30d: None,
            evidence_unavailable: false,
        }
    } else {
        StaticAnalysisInput {
            source_text: after_source.clone(),
            diagnostics: Vec::new(),
            churn_30d: None,
            evidence_unavailable: true,
        }
    };
    Ok(InferencePanelInputs {
        before_code,
        after_code,
        before_invalid_source,
        after_invalid_source,
        diff: DiffInstrumentInput {
            language: language.clone(),
            path: path.clone(),
            before_source,
            after_source: after_source.clone(),
        },
        tests: TextInstrumentInput {
            text: tests_text,
            source_id: format!("{}:tests", context.task_id.0),
            language: Some(language.clone()),
        },
        problem: TextInstrumentInput {
            text: context.problem_statement.clone(),
            source_id: format!("{}:problem", context.task_id.0),
            language: Some(language.clone()),
        },
        commit: TextInstrumentInput {
            text: patch.commit_message.clone(),
            source_id: format!(
                "{}:commit:{}",
                context.task_id.0,
                hex::encode(patch.patch_sha)
            ),
            language: Some(language.clone()),
        },
        witness: WitnessChainInput {
            format_version: CANONICAL_WITNESS_FORMAT_VERSION,
            chain_bytes: patch.witness_chain_segment.clone(),
        },
        oracle: InstrumentOracleVerdict {
            per_test: Vec::new(),
            exception: None,
            evidence_unavailable: true,
        },
        trace: TraceInput {
            events: Vec::new(),
            evidence_unavailable: true,
        },
        static_analysis,
        runtime: RuntimeInput {
            wall_time_ms: 0,
            peak_rss_bytes: 0,
            exit_code: -1,
            timed_out: false,
            coverage_percent: None,
            network_events: 0,
            filesystem_writes: 0,
            evidence_unavailable: true,
        },
        reasoning: ReasoningInput {
            task_id: context.task_id.0.clone(),
            transcript: format!(
                "task_id={}; explicit_problem={}; commit_message={}; tests={}",
                context.task_id.0,
                context.problem_statement,
                patch.commit_message,
                context
                    .tests
                    .iter()
                    .map(|test| test.0.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            events: vec![ReasoningEvent {
                actor: "operator_or_agent".to_string(),
                event_type: "explicit_task_context".to_string(),
                text: context.problem_statement.clone(),
            }],
        },
        scalars: ScalarInput {
            bfs_depth: max_path_depth(patch),
            blame_age_days: 0,
            churn_lines_30d: changed_lines.min(u32::MAX as usize) as u32,
            coverage_delta: 0.0,
            repo_health_score: 0.5,
            files_touched: file_count.min(u32::MAX as usize) as u32,
            hunks_touched: patch.ast_diff.hunks.len().min(u32::MAX as usize) as u32,
            evidence_unavailable: true,
        },
    })
}

fn vectors_for(
    time_step: TimeStep,
    inputs: &InferencePanelInputs,
) -> Result<Vec<PanelVectorInput>, MejepaInferError> {
    let mut vectors = Vec::with_capacity(
        context_graph_mejepa_instruments::materialize::active_slots(time_step).len(),
    );
    match time_step {
        TimeStep::T0 => {
            push_code_vectors(
                &mut vectors,
                &inputs.before_code,
                inputs.before_invalid_source.as_ref(),
            )?;
            vectors.push(encode_vector(&ETestInstrument, &inputs.tests)?);
            vectors.push(encode_vector(&EProblemInstrument, &inputs.problem)?);
        }
        TimeStep::T1 => {
            push_code_vectors(
                &mut vectors,
                &inputs.after_code,
                inputs.after_invalid_source.as_ref(),
            )?;
            vectors.push(encode_vector(&ETestInstrument, &inputs.tests)?);
            push_diff_vector(&mut vectors, inputs)?;
            vectors.push(encode_vector(&EWitnessInstrument, &inputs.witness)?);
            vectors.push(encode_vector(&EProblemInstrument, &inputs.problem)?);
            vectors.push(encode_vector(&ECommitMsgInstrument, &inputs.commit)?);
        }
        TimeStep::T2 => {
            push_code_vectors(
                &mut vectors,
                &inputs.after_code,
                inputs.after_invalid_source.as_ref(),
            )?;
            vectors.push(encode_vector(&ETestInstrument, &inputs.tests)?);
            vectors.push(encode_vector(&ETraceInstrument, &inputs.trace)?);
            push_diff_vector(&mut vectors, inputs)?;
            vectors.push(encode_vector(&EWitnessInstrument, &inputs.witness)?);
            vectors.push(encode_vector(&EOracleInstrument, &inputs.oracle)?);
            vectors.push(encode_vector(&EProblemInstrument, &inputs.problem)?);
            vectors.push(encode_vector(&ECommitMsgInstrument, &inputs.commit)?);
            vectors.push(encode_vector(
                &EStaticAnalysisInstrument,
                &inputs.static_analysis,
            )?);
            vectors.push(encode_vector(&ERuntimeInstrument, &inputs.runtime)?);
            vectors.push(encode_vector(&EReasoningInstrument, &inputs.reasoning)?);
            vectors.push(encode_vector(&ScalarsInstrument, &inputs.scalars)?);
        }
    }
    Ok(vectors)
}

fn push_code_vectors(
    vectors: &mut Vec<PanelVectorInput>,
    code: &CodeInstrumentInput,
    invalid_source: Option<&InvalidSourceDiagnostic>,
) -> Result<(), MejepaInferError> {
    if let Some(diagnostic) = invalid_source {
        for slot in [
            InstrumentSlot::EAst,
            InstrumentSlot::ECfg,
            InstrumentSlot::EDataFlow,
            InstrumentSlot::ETypeGraph,
        ] {
            vectors.push(PanelVectorInput {
                slot,
                vector: invalid_source_vector(slot, diagnostic, None)?,
            });
        }
        return Ok(());
    }
    vectors.push(encode_vector(&EAstInstrument, code)?);
    vectors.push(encode_vector(&ECfgInstrument, code)?);
    vectors.push(encode_vector(&EDataFlowInstrument, code)?);
    vectors.push(encode_vector(&ETypeGraphInstrument, code)?);
    Ok(())
}

fn push_diff_vector(
    vectors: &mut Vec<PanelVectorInput>,
    inputs: &InferencePanelInputs,
) -> Result<(), MejepaInferError> {
    let primary_invalid = inputs
        .after_invalid_source
        .as_ref()
        .or(inputs.before_invalid_source.as_ref());
    if let Some(primary) = primary_invalid {
        let secondary = if matches!(primary.side, SourceSide::After) {
            inputs.before_invalid_source.as_ref()
        } else {
            inputs.after_invalid_source.as_ref()
        };
        vectors.push(PanelVectorInput {
            slot: InstrumentSlot::EDiff,
            vector: invalid_source_vector(InstrumentSlot::EDiff, primary, secondary)?,
        });
        return Ok(());
    }
    vectors.push(encode_vector(&EDiffInstrument, &inputs.diff)?);
    Ok(())
}

fn encode_vector<I, In>(instrument: &I, input: &In) -> Result<PanelVectorInput, MejepaInferError>
where
    I: Instrument<Input = In>,
{
    Ok(PanelVectorInput {
        slot: instrument.slot(),
        vector: instrument.encode(input)?,
    })
}

#[derive(Debug, Clone, Copy)]
enum SourceSide {
    Before,
    After,
}

fn panel_source_text(patch: &PatchBundle, side: SourceSide) -> Result<String, MejepaInferError> {
    if patch.ast_diff.hunks.len() == 1 {
        let source = match side {
            SourceSide::Before => &patch.ast_diff.hunks[0].before,
            SourceSide::After => &patch.ast_diff.hunks[0].after,
        };
        return Ok(source.clone());
    }

    let mut out = String::new();
    for hunk in &patch.ast_diff.hunks {
        let source = match side {
            SourceSide::Before => &hunk.before,
            SourceSide::After => &hunk.after,
        };
        out.push_str(&format!(
            "\n# MEJEPA_BEGIN_HUNK path={}\n",
            path_text(&hunk.path)
        ));
        out.push_str(source);
        if !source.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("# MEJEPA_END_HUNK\n");
    }
    Ok(out)
}

fn invalid_source_diagnostic(
    code: &CodeInstrumentInput,
    side: SourceSide,
) -> Option<InvalidSourceDiagnostic> {
    let source_line_count = code.source.lines().count().max(1);
    let empty_source = code.source.trim().is_empty();
    if empty_source {
        return Some(InvalidSourceDiagnostic {
            side,
            language: code.language.clone(),
            path: code.path.clone(),
            kind: "empty_source".to_string(),
            detail: "source text is empty or whitespace-only".to_string(),
            line: 1,
            column: 1,
            source_len_bytes: code.source.len(),
            source_line_count,
            empty_source: true,
        });
    }
    if code.language != "python" {
        return None;
    }
    match ruff_python_parser::parse_module(&code.source) {
        Ok(_) => None,
        Err(err) => {
            let start_byte = err.range().start().to_usize();
            let line = line_for_byte(code.source.as_bytes(), start_byte)
                .clamp(1, source_line_count.min(u32::MAX as usize) as u32);
            Some(InvalidSourceDiagnostic {
                side,
                language: code.language.clone(),
                path: code.path.clone(),
                kind: sanitize_diagnostic_text(&format!("{:?}", err.error)),
                detail: sanitize_diagnostic_text(&err.to_string()),
                line,
                column: column_for_byte(code.source.as_bytes(), start_byte),
                source_len_bytes: code.source.len(),
                source_line_count,
                empty_source: false,
            })
        }
    }
}

fn static_analysis_diagnostic(diagnostic: &InvalidSourceDiagnostic) -> Diagnostic {
    Diagnostic {
        tool: "ruff_python_parser".to_string(),
        severity: DiagnosticSeverity::Error,
        category: diagnostic.kind.clone(),
        line: diagnostic.line.max(1),
        column: diagnostic.column.max(1),
    }
}

fn invalid_source_vector(
    slot: InstrumentSlot,
    primary: &InvalidSourceDiagnostic,
    secondary: Option<&InvalidSourceDiagnostic>,
) -> Result<Vec<f32>, MejepaInferError> {
    let mut out = vec![0.0_f32; slot.dim()];
    out[0] = 10.0;
    out[1] = source_side_feature(primary.side);
    out[2] = bounded_ratio(primary.line as f32, primary.source_line_count as f32);
    out[3] = bounded_ratio(primary.column as f32, 240.0);
    out[4] = bounded_ratio(primary.source_len_bytes as f32, 5_000_000.0);
    out[5] = bounded_ratio(primary.source_line_count as f32, 100_000.0);
    out[6] = if primary.empty_source { 1.0 } else { 0.0 };
    out[7] = if primary.kind == "empty_source" {
        0.0
    } else {
        1.0
    };
    out[8] = invalid_source_slot_feature(slot);
    if let Some(secondary) = secondary {
        out[9] = 1.0;
        out[10] = source_side_feature(secondary.side);
        out[11] = bounded_ratio(secondary.line as f32, secondary.source_line_count as f32);
        out[12] = bounded_ratio(secondary.column as f32, 240.0);
        out[13] = if secondary.empty_source { 1.0 } else { 0.0 };
    }
    add_invalid_source_hashes(&mut out, primary, 1.0);
    if let Some(secondary) = secondary {
        add_invalid_source_hashes(&mut out, secondary, 0.5);
    }
    normalize_l2(&mut out);
    if let Some((idx, value)) = out.iter().enumerate().find(|(_, value)| !value.is_finite()) {
        return Err(MejepaInferError::NanDetected {
            nan_source: "invalid_source_vector".to_string(),
            detail: format!("slot={slot:?} vector[{idx}] is non-finite: {value}"),
        });
    }
    Ok(out)
}

fn add_invalid_source_hashes(out: &mut [f32], diagnostic: &InvalidSourceDiagnostic, weight: f32) {
    add_feature_bin(out, 32, 32, &diagnostic.language, weight);
    add_feature_bin(out, 64, 64, &diagnostic.path, weight);
    add_feature_bin(out, 128, 64, &diagnostic.kind, weight);
    add_feature_bin(out, 192, 64, &diagnostic.detail, weight);
}

fn add_feature_bin(out: &mut [f32], offset: usize, span: usize, key: &str, weight: f32) {
    if offset >= out.len() || span == 0 {
        return;
    }
    let available = span.min(out.len() - offset);
    let idx = offset + (fnv1a64(key.as_bytes()) as usize % available);
    out[idx] += weight;
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

fn bounded_ratio(value: f32, denom: f32) -> f32 {
    if denom <= 0.0 {
        0.0
    } else {
        (value / denom).clamp(0.0, 1.0)
    }
}

fn normalize_l2(out: &mut [f32]) {
    let norm = out
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt();
    if norm > 0.0 {
        for value in out {
            *value = (*value as f64 / norm) as f32;
        }
    }
}

fn source_side_feature(side: SourceSide) -> f32 {
    match side {
        SourceSide::Before => 0.25,
        SourceSide::After => 0.75,
    }
}

fn invalid_source_slot_feature(slot: InstrumentSlot) -> f32 {
    match slot {
        InstrumentSlot::EAst => 0.1,
        InstrumentSlot::ECfg => 0.2,
        InstrumentSlot::EDataFlow => 0.3,
        InstrumentSlot::ETypeGraph => 0.4,
        InstrumentSlot::EDiff => 0.5,
        _ => 0.0,
    }
}

fn line_for_byte(source: &[u8], byte: usize) -> u32 {
    let bounded = byte.min(source.len());
    source[..bounded]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        .saturating_add(1)
        .min(u32::MAX as usize) as u32
}

fn column_for_byte(source: &[u8], byte: usize) -> u32 {
    let bounded = byte.min(source.len());
    let line_start = source[..bounded]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    bounded
        .saturating_sub(line_start)
        .saturating_add(1)
        .min(u32::MAX as usize) as u32
}

fn sanitize_diagnostic_text(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    sanitized.trim().chars().take(240).collect()
}

fn panel_source_path(patch: &PatchBundle) -> String {
    if patch.ast_diff.hunks.len() == 1 {
        return path_text(&patch.ast_diff.hunks[0].path);
    }
    "mejepa_aggregate_patch.py".to_string()
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn changed_line_count(patch: &PatchBundle) -> Result<usize, MejepaInferError> {
    let mut total = 0usize;
    for hunk in &patch.ast_diff.hunks {
        let before = hunk.before.lines().count();
        let after = hunk.after.lines().count();
        total =
            total
                .checked_add(before.max(after))
                .ok_or_else(|| MejepaInferError::InvalidInput {
                    field: "ast_diff.hunks".to_string(),
                    detail: "changed-line count overflow".to_string(),
                })?;
    }
    Ok(total)
}

fn max_path_depth(patch: &PatchBundle) -> u32 {
    patch
        .ast_diff
        .hunks
        .iter()
        .map(|hunk| hunk.path.components().count())
        .max()
        .unwrap_or(0)
        .min(u32::MAX as usize) as u32
}

fn language_slug(language: crate::types::Language) -> &'static str {
    match language {
        crate::types::Language::Rust => "rust",
        crate::types::Language::Python => "python",
        crate::types::Language::Javascript => "javascript",
        crate::types::Language::Typescript => "typescript",
        crate::types::Language::Go => "go",
        crate::types::Language::Java => "java",
        crate::types::Language::C => "c",
        crate::types::Language::Cpp => "cpp",
        crate::types::Language::CSharp => "csharp",
        crate::types::Language::Ruby => "ruby",
        crate::types::Language::Php => "php",
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

fn apply_ood_gate_to_prediction(
    prediction: &mut RealityPrediction,
    decision: &OodGateDecision,
    context: &TaskContext,
) -> Result<(), MejepaInferError> {
    let previous_verdict = prediction.verdict;
    prediction.verdict = decision.verdict;
    prediction
        .granger_attestations
        .insert(format!("ood_gate:{}", decision.reason), 1.0);
    if matches!(
        decision.verdict,
        Verdict::OutOfDistribution | Verdict::GuardRejected
    ) {
        prediction.matched_fingerprint = None;
    }
    if decision.verdict == Verdict::OutOfDistribution {
        prediction.unknown_candidate_id = Some(unknown_candidate_id_for_prediction(
            &context.task_id,
            prediction.prediction_id,
            &context.session_id,
        ));
    } else if previous_verdict == Verdict::OutOfDistribution
        || prediction.unknown_candidate_id.is_some()
    {
        prediction.unknown_candidate_id = None;
    }
    prediction.provenance.active_pointer = format!(
        "ood_gate:{}:score={:.6}:threshold={}",
        decision.reason,
        decision.ood_score,
        decision
            .threshold
            .map(|threshold| format!("{threshold:.6}"))
            .unwrap_or_else(|| "missing".to_string())
    );
    prediction.validate()
}

fn ood_gate_failed(decision: &OodGateDecision) -> bool {
    matches!(
        decision.verdict,
        Verdict::OutOfDistribution | Verdict::GuardRejected
    ) || decision.reason == OOD_CALIBRATOR_MISSING
        || decision.reason == crate::ood_harvest::OOD_SCORE_ABOVE_THRESHOLD
        || decision.reason == "OOD_SCORE_ABOVE_STATIC_THRESHOLD"
}

fn eval_to_infer_error(err: crate::eval::EvalError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "ood_gate".to_string(),
        detail: err.to_string(),
    }
}

fn ood_threshold_from_selected_report(
    report: Option<&OodCalibrationReport>,
    require_ood_calibrator: bool,
    fallback_threshold: f32,
) -> Result<f32, MejepaInferError> {
    let Some(report) = report else {
        if require_ood_calibrator {
            return Err(MejepaInferError::OodCalibratorMissing {
                detail: "strict mode requires a selected OOD calibrator before compilation"
                    .to_string(),
            });
        }
        return Ok(fallback_threshold);
    };
    Ok(report.threshold)
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

fn annotate_contradiction_decision(
    attestations: &mut BTreeMap<String, f32>,
    decision: &ContradictionDecision,
) -> Result<(), MejepaInferError> {
    match decision.kind {
        ContradictionDecisionKind::NoContradiction => return Ok(()),
        ContradictionDecisionKind::SingleHeadOnly
        | ContradictionDecisionKind::MultiHeadContradiction
        | ContradictionDecisionKind::ThresholdMissing => {}
    }
    validate_probability(
        "contradiction.oracle_pass_confidence",
        decision.oracle_pass_confidence,
    )?;
    attestations.insert(format!("contradiction:{}", decision.reason), 1.0);
    attestations.insert(
        "contradiction:oracle_pass_confidence".to_string(),
        decision.oracle_pass_confidence,
    );
    if let Some(tau_oracle) = decision.tau_oracle {
        validate_probability("contradiction.tau_oracle", tau_oracle)?;
        attestations.insert("contradiction:tau_oracle".to_string(), tau_oracle);
    }
    if let Some(cell_id) = &decision.cell_id {
        attestations.insert(format!("contradiction_cell:{}", short_sha(cell_id)), 1.0);
    }
    Ok(())
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

fn per_slot_ood_reasons_from_guard_violations(
    guard_violations: &[crate::types::GuardViolation],
    calibration_version: &str,
) -> Result<Vec<PerSlotOodReason>, MejepaInferError> {
    guard_violations
        .iter()
        .map(|violation| {
            let reason = PerSlotOodReason {
                embedder: violation.embedder.clone(),
                chunk: Some(violation.chunk.clone()),
                reason: PerSlotOodReasonKind::GtauViolation,
                observed_score: violation.cosine,
                threshold: violation.threshold_tau_m,
                margin: violation.deficit,
                calibration_version: calibration_version.to_string(),
                evidence: format!(
                    "Gtau slot violation: embedder={} chunk={} centroid={} cosine={} threshold_tau_m={} deficit={}",
                    violation.embedder.0,
                    violation.chunk.0,
                    violation.centroid_id,
                    violation.cosine,
                    violation.threshold_tau_m,
                    violation.deficit
                ),
            };
            reason.validate()?;
            Ok(reason)
        })
        .collect()
}

struct PerSlotOodAssessment {
    reasons: Vec<PerSlotOodReason>,
    slot_specific_guard_count: usize,
}

fn per_slot_ood_assessment_from_residual_scores(
    scores: &[SlotResidualScore],
    threshold: f32,
    calibration_version: &str,
) -> Result<PerSlotOodAssessment, MejepaInferError> {
    let triggered = scores
        .iter()
        .filter(|score| score.score >= threshold)
        .collect::<Vec<_>>();
    if triggered.is_empty() {
        return Ok(PerSlotOodAssessment {
            reasons: Vec::new(),
            slot_specific_guard_count: 0,
        });
    }
    if let [score] = triggered.as_slice() {
        let reason = PerSlotOodReason {
            embedder: EmbedderId(score.slot.slug().to_string()),
            chunk: None,
            reason: PerSlotOodReasonKind::SlotThresholdExceeded,
            observed_score: score.score,
            threshold,
            margin: (score.score - threshold).max(0.0),
            calibration_version: calibration_version.to_string(),
            evidence: format!(
                "slot-preserving OOD residual: slot={} norm_sq={} score={} threshold={}",
                score.slot.slug(),
                score.norm_sq,
                score.score,
                threshold
            ),
        };
        reason.validate()?;
        return Ok(PerSlotOodAssessment {
            reasons: vec![reason],
            slot_specific_guard_count: 1,
        });
    }
    let max_score = triggered
        .iter()
        .max_by(|left, right| left.score.total_cmp(&right.score))
        .expect("triggered is non-empty");
    let triggered_slots = triggered
        .iter()
        .map(|score| score.slot.slug())
        .collect::<Vec<_>>()
        .join(",");
    let reason = PerSlotOodReason {
        embedder: EmbedderId("diffuse".to_string()),
        chunk: None,
        reason: PerSlotOodReasonKind::DiffuseSlotThresholdExceeded,
        observed_score: max_score.score,
        threshold,
        margin: (max_score.score - threshold).max(0.0),
        calibration_version: calibration_version.to_string(),
        evidence: format!(
            "slot-preserving OOD residual: slot_attribution=diffuse triggered_slots={} max_slot={} max_norm_sq={} max_score={} threshold={}",
            triggered_slots,
            max_score.slot.slug(),
            max_score.norm_sq,
            max_score.score,
            threshold
        ),
    };
    reason.validate()?;
    Ok(PerSlotOodAssessment {
        reasons: vec![reason],
        slot_specific_guard_count: 0,
    })
}

fn objective_report_for_prediction(
    patch: &PatchBundle,
    prediction: &RealityPrediction,
) -> Result<ObjectiveSafetyReport, MejepaInferError> {
    let surfaces = PhaseBPredictionSurfaces {
        predicted_failure_modes: prediction.predicted_failure_modes.clone(),
        predicted_failed_tests: prediction.predicted_failed_tests.clone(),
        predicted_works: prediction.predicted_works.clone(),
        predicted_uncovered_paths: prediction.predicted_uncovered_paths.clone(),
        predicted_flaky_tests: prediction.predicted_flaky_tests.clone(),
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
    pub approved: bool,
}

impl Default for DeterministicConstellationGuard {
    fn default() -> Self {
        Self {
            version_id: "tct-fsv-v1".to_string(),
            age_days: 0,
            cell_id: "mutation=known_good:language=python:entity=function".to_string(),
            n_supporting: Some(100),
            approved: true,
        }
    }
}

impl ConstellationGuard for DeterministicConstellationGuard {
    fn verify(
        &self,
        _prediction: &RealityPrediction,
        _predicted_panel: &Panel,
        _context: &TaskContext,
    ) -> Result<ConstellationDecision, MejepaInferError> {
        Ok(ConstellationDecision {
            approved: self.approved,
            reason: if self.approved {
                "constellation accepted".to_string()
            } else {
                "explicit deterministic guard rejection".to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::{sha256_bytes, valid_witness_segment};
    use crate::store::RocksDbInferStore;
    use crate::types::{AstDiff, DiffHunk, Language, TaskEnvironment, TaskId, TestId, Verdict};
    use context_graph_mejepa_instruments::PANEL_DIM;
    use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

    #[derive(Clone)]
    struct FixedPanelPredictor {
        panel: Panel,
    }

    impl Predictor for FixedPanelPredictor {
        fn predict(&self, _panel_t0: &Panel, _panel_t1: &Panel) -> Result<Panel, MejepaInferError> {
            Ok(self.panel.clone())
        }
    }

    #[derive(Clone)]
    struct FixedPanelTarget {
        panel: Panel,
    }

    impl FrozenTarget for FixedPanelTarget {
        fn target(&self, _panel_t2: &Panel) -> Result<Panel, MejepaInferError> {
            Ok(self.panel.clone())
        }
    }

    fn sample_patch_context(commit_message: &str) -> (PatchBundle, TaskContext) {
        let before = "def answer(value: int) -> int:\n    return value + 1\n";
        let after = "def answer(value: int) -> int:\n    return value + 2\n";
        let patch = PatchBundle::try_new(
            AstDiff {
                hunks: vec![DiffHunk {
                    path: PathBuf::from("src/example.py"),
                    pre_sha: sha256_bytes(before.as_bytes()),
                    post_sha: sha256_bytes(after.as_bytes()),
                    before: before.to_string(),
                    after: after.to_string(),
                }],
            },
            valid_witness_segment(),
            commit_message.to_string(),
            sha256_bytes(b"stable-patch-sha"),
        )
        .expect("sample patch is valid");
        let context = TaskContext {
            task_id: TaskId("task-real-panel".to_string()),
            session_id: [7; 16],
            language: Language::Python,
            problem_statement: "verify the answer increment changes by one".to_string(),
            tests: vec![TestId("tests/test_example.py::test_answer".to_string())],
            environment: TaskEnvironment {
                repo_root: PathBuf::from("/zfs/archive/contextgraph/test-repo"),
                python_version: Some("3.11".to_string()),
                os: "linux".to_string(),
            },
            claim_graph: None,
            skill_citations: vec![],
        };
        (patch, context)
    }

    fn panel_with_slot_delta(slot: InstrumentSlot, delta: f32) -> Panel {
        let mut data = vec![0.0_f32; PANEL_DIM];
        data[slot.offset()] = delta;
        Panel::try_new(data, (1u16 << InstrumentSlot::all().len()) - 1).unwrap()
    }

    fn panel_with_all_slot_deltas(delta: f32) -> Panel {
        let mut data = vec![0.0_f32; PANEL_DIM];
        for slot in InstrumentSlot::all() {
            data[slot.offset()] = delta;
        }
        Panel::try_new(data, (1u16 << InstrumentSlot::all().len()) - 1).unwrap()
    }

    fn seed_calibration(calibration: &CalibrationStore) {
        let examples = (0..40)
            .map(|idx| CalibrationExample {
                language: Language::Python,
                predicted_test_pass: vec![if idx % 10 == 0 { 0.2 } else { 0.95 }],
                actual_test_pass: vec![if idx % 10 == 0 { 0.0 } else { 1.0 }],
            })
            .collect::<Vec<_>>();
        calibration
            .calibrate(
                &examples,
                &[0.01; 40],
                0.10,
                30,
                0.30,
                [7; 32],
                BTreeMap::new(),
            )
            .unwrap();
    }

    fn compiler_with_fixed_panels(
        db: Arc<rocksdb::DB>,
        predicted: Panel,
        target: Panel,
    ) -> MeJepaCompiler {
        let calibration = CalibrationStore::new(db.clone(), 30).unwrap();
        seed_calibration(&calibration);
        let config = MeJepaInferConfig {
            pause_state_path: None,
            ..MeJepaInferConfig::default()
        };
        MeJepaCompiler::new(
            config,
            Arc::new(FixedPanelPredictor { panel: predicted }),
            Arc::new(FixedPanelTarget { panel: target }),
            Arc::new(DeterministicOracleHead),
            Arc::new(DeterministicConstellationGuard::default()),
            Arc::new(PatchWitnessReader),
            Arc::new(RocksDbInferStore::new(db)),
            calibration,
            PathBuf::from("/zfs/archive/contextgraph/test-repo"),
        )
        .unwrap()
    }

    #[test]
    fn inference_panels_match_direct_frozen_encoder_outputs() {
        let (patch, context) = sample_patch_context("change answer increment");
        let inputs = build_inference_panel_inputs(&patch, &context).unwrap();
        let (panel_t0, panel_t1, panel_t2) =
            materialize_inference_panels(&patch, &context).unwrap();

        let direct_ast = EAstInstrument.encode(&inputs.before_code).unwrap();
        let direct_test = ETestInstrument.encode(&inputs.tests).unwrap();
        let direct_diff = EDiffInstrument.encode(&inputs.diff).unwrap();
        let direct_problem = EProblemInstrument.encode(&inputs.problem).unwrap();
        let direct_commit = ECommitMsgInstrument.encode(&inputs.commit).unwrap();
        let direct_oracle = EOracleInstrument.encode(&inputs.oracle).unwrap();

        assert_eq!(panel_t0.slot(InstrumentSlot::EAst), direct_ast.as_slice());
        assert_eq!(panel_t0.slot(InstrumentSlot::ETest), direct_test.as_slice());
        assert_eq!(panel_t1.slot(InstrumentSlot::EDiff), direct_diff.as_slice());
        assert_eq!(
            panel_t1.slot(InstrumentSlot::EProblem),
            direct_problem.as_slice()
        );
        assert_eq!(
            panel_t1.slot(InstrumentSlot::ECommitMsg),
            direct_commit.as_slice()
        );
        assert_eq!(
            panel_t2.slot(InstrumentSlot::EOracle),
            direct_oracle.as_slice()
        );
    }

    #[test]
    fn t2_unavailable_evidence_uses_typed_absent_state() {
        let (patch, context) = sample_patch_context("change answer increment");
        let inputs = build_inference_panel_inputs(&patch, &context).unwrap();
        assert!(inputs.oracle.evidence_unavailable);
        assert!(inputs.oracle.per_test.is_empty());
        assert!(inputs.oracle.exception.is_none());
        assert!(inputs.trace.evidence_unavailable);
        assert!(inputs.trace.events.is_empty());
        assert!(inputs.static_analysis.evidence_unavailable);
        assert!(inputs.static_analysis.diagnostics.is_empty());
        assert!(inputs.runtime.evidence_unavailable);
        assert!(!inputs.runtime.timed_out);
        assert!(inputs.scalars.evidence_unavailable);

        let (_, _, panel_t2) = materialize_inference_panels(&patch, &context).unwrap();
        assert_eq!(panel_t2.slot(InstrumentSlot::EOracle)[16], 1.0);
        assert_eq!(panel_t2.slot(InstrumentSlot::EOracle)[2], 0.0);
        assert_eq!(panel_t2.slot(InstrumentSlot::ETrace)[16], 1.0);
        assert_eq!(panel_t2.slot(InstrumentSlot::ETrace)[3], 0.0);
        assert_eq!(panel_t2.slot(InstrumentSlot::EStaticAnalysis)[16], 1.0);
        assert_eq!(panel_t2.slot(InstrumentSlot::EStaticAnalysis)[67], 0.0);
        assert_eq!(panel_t2.slot(InstrumentSlot::ERuntime)[16], 1.0);
        assert_eq!(panel_t2.slot(InstrumentSlot::ERuntime)[3], 0.0);
        assert_eq!(panel_t2.slot(InstrumentSlot::Scalars)[7], 1.0);
    }

    #[test]
    fn ood_gate_failed_escalates_base_ood_verdict_even_when_static_score_within_threshold() {
        let decision = OodGateDecision {
            verdict: Verdict::OutOfDistribution,
            reason: "OOD_SCORE_WITHIN_STATIC_THRESHOLD".to_string(),
            ood_score: 0.0,
            threshold: Some(1.0),
        };

        assert!(ood_gate_failed(&decision));
    }

    fn selected_ood_report(threshold: f32) -> OodCalibrationReport {
        OodCalibrationReport {
            schema_version: crate::ood_harvest::OOD_CALIBRATION_SCHEMA_VERSION,
            report_id: "test-selected-ood-report".to_string(),
            generated_at_unix_ms: 1_774_000_000_000,
            window_start_unix_ms: 1_773_999_000_000,
            window_end_unix_ms: 1_774_000_000_000,
            threshold,
            harvested_rows: 8,
            synthetic_ood_rows: 4,
            id_rows: 4,
            ood_rows: 4,
            true_positive: 4,
            false_positive: 0,
            true_negative: 4,
            false_negative: 0,
            global_auc: Some(1.0),
            ood_recall: 1.0,
            false_positive_rate: 0.0,
            min_required_auc: 0.80,
            selected_for_serving: true,
            flags: Vec::new(),
            cell_reports: Vec::new(),
            source_harvest_cf: context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST.to_string(),
            source_synthetic_cf: context_graph_mejepa_cf::CF_MEJEPA_SYNTHETIC_STRESS_RESULTS
                .to_string(),
        }
    }

    #[test]
    fn strict_missing_ood_calibrator_fails_closed_before_compilation() {
        println!("FSV before: report=None require_ood_calibrator=true fallback_threshold=0.42");
        let err = ood_threshold_from_selected_report(None, true, 0.42).unwrap_err();
        println!("FSV after: outcome=Err code={} message={}", err.code(), err);
        assert!(matches!(err, MejepaInferError::OodCalibratorMissing { .. }));
        assert_eq!(err.code(), "MEJEPA_INFER_OOD_CALIBRATOR_MISSING");
        assert!(err.to_string().contains("strict mode requires"));
    }

    #[test]
    fn non_strict_missing_ood_calibrator_uses_configured_static_threshold() {
        println!("FSV before: report=None require_ood_calibrator=false fallback_threshold=0.42");
        let threshold = ood_threshold_from_selected_report(None, false, 0.42).unwrap();
        println!("FSV after: outcome=Ok threshold={threshold}");
        assert_eq!(threshold, 0.42);
    }

    #[test]
    fn strict_selected_ood_calibrator_uses_promoted_threshold() {
        let report = selected_ood_report(0.67);
        report.validate().unwrap();
        println!(
            "FSV before: report=Some(selected_for_serving={}) require_ood_calibrator=true fallback_threshold=0.42 report_threshold={}",
            report.selected_for_serving, report.threshold
        );

        let threshold = ood_threshold_from_selected_report(Some(&report), true, 0.42).unwrap();
        println!("FSV after: outcome=Ok threshold={threshold}");
        assert_eq!(threshold, 0.67);
    }

    #[test]
    fn compile_with_panel_reports_single_slot_ood_as_guard_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let db = crate::calibration::open_infer_rocksdb(temp.path()).unwrap();
        let compiler = compiler_with_fixed_panels(
            db,
            panel_with_slot_delta(InstrumentSlot::EReasoning, 3.0),
            panel_with_slot_delta(InstrumentSlot::EReasoning, 0.0),
        );
        let (patch, context) = sample_patch_context("slot isolated reasoning residual");

        let (prediction, _) = compiler.compile_with_panel(&patch, &context).unwrap();

        assert_eq!(prediction.verdict, Verdict::GuardRejected);
        assert_eq!(prediction.per_slot_ood_reasons.len(), 1);
        let reason = &prediction.per_slot_ood_reasons[0];
        assert_eq!(reason.embedder.0, InstrumentSlot::EReasoning.slug());
        assert_eq!(reason.reason, PerSlotOodReasonKind::SlotThresholdExceeded);
        assert!(reason.evidence.contains("slot=e_reasoning"));
        assert!(reason.evidence.contains("norm_sq=9"));
        assert!(prediction.ood_score >= compiler.config.ood_refuse_threshold);
    }

    #[test]
    fn compile_with_panel_reports_multi_slot_ood_as_diffuse_generic_ood() {
        let temp = tempfile::tempdir().unwrap();
        let db = crate::calibration::open_infer_rocksdb(temp.path()).unwrap();
        let compiler = compiler_with_fixed_panels(
            db,
            panel_with_all_slot_deltas(3.0),
            panel_with_all_slot_deltas(0.0),
        );
        let (patch, context) = sample_patch_context("diffuse residual across all slots");

        let (prediction, _) = compiler.compile_with_panel(&patch, &context).unwrap();

        assert_eq!(prediction.verdict, Verdict::OutOfDistribution);
        assert_eq!(prediction.per_slot_ood_reasons.len(), 1);
        let reason = &prediction.per_slot_ood_reasons[0];
        assert_eq!(reason.embedder.0, "diffuse");
        assert_eq!(
            reason.reason,
            PerSlotOodReasonKind::DiffuseSlotThresholdExceeded
        );
        assert!(reason.evidence.contains("slot_attribution=diffuse"));
        assert!(reason.evidence.contains("triggered_slots=e_ast"));
        assert!(prediction.ood_score >= compiler.config.ood_refuse_threshold);
    }

    #[test]
    fn changed_commit_message_alters_only_commit_slot_in_t1() {
        let (patch_a, context) = sample_patch_context("change answer increment");
        let (patch_b, _) = sample_patch_context("document answer increment precisely");
        let (_, panel_a, _) = materialize_inference_panels(&patch_a, &context).unwrap();
        let (_, panel_b, _) = materialize_inference_panels(&patch_b, &context).unwrap();

        for slot in context_graph_mejepa_instruments::materialize::active_slots(TimeStep::T1) {
            if slot == InstrumentSlot::ECommitMsg {
                assert_ne!(panel_a.slot(slot), panel_b.slot(slot));
            } else {
                assert_eq!(panel_a.slot(slot), panel_b.slot(slot), "slot {slot:?}");
            }
        }
    }

    #[test]
    fn empty_minimal_source_materializes_invalid_source_panel() {
        let (mut patch, context) = sample_patch_context("change answer increment");
        patch.ast_diff.hunks[0].before.clear();
        patch.ast_diff.hunks[0].after.clear();
        let (panel_t0, panel_t1, panel_t2) =
            materialize_inference_panels(&patch, &context).unwrap();
        assert!(panel_t0.slot(InstrumentSlot::EAst)[0] > 0.90);
        assert!(panel_t1.slot(InstrumentSlot::EAst)[0] > 0.90);
        assert!(panel_t1.slot(InstrumentSlot::EDiff)[0] > 0.90);
        assert!(panel_t2.slot(InstrumentSlot::EStaticAnalysis)[0] > 0.0);
    }

    #[test]
    fn malformed_source_materializes_invalid_source_panel() {
        let (mut patch, context) = sample_patch_context("change answer increment");
        patch.ast_diff.hunks[0].after = "def broken(:\n".to_string();
        let (_, panel_t1, panel_t2) = materialize_inference_panels(&patch, &context).unwrap();
        assert!(panel_t1.slot(InstrumentSlot::EAst)[0] > 0.90);
        assert!(panel_t1.slot(InstrumentSlot::EDiff)[0] > 0.90);
        assert!(panel_t2.slot(InstrumentSlot::EStaticAnalysis)[0] > 0.0);
    }

    #[test]
    fn eof_parse_error_line_is_clamped_to_source_line_count() {
        let (mut patch, context) = sample_patch_context("change answer increment");
        let mut after = String::new();
        for _ in 0..29 {
            after.push_str("x = 1\n");
        }
        after.push_str("def broken(\n");
        patch.ast_diff.hunks[0].after = after;
        let (_, panel_t1, panel_t2) = materialize_inference_panels(&patch, &context).unwrap();
        assert!(panel_t1.slot(InstrumentSlot::EAst)[0] > 0.90);
        assert!(panel_t2.slot(InstrumentSlot::EStaticAnalysis)[0] > 0.0);
    }

    #[test]
    fn code_instruments_still_reject_malformed_source_directly() {
        let input = CodeInstrumentInput {
            language: "python".to_string(),
            path: "src/broken.py".to_string(),
            source: "def broken(:\n".to_string(),
        };
        let err = EAstInstrument.encode(&input).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    }

    #[test]
    fn mixed_validity_multifile_patch_materializes_invalid_source_panel() {
        let (mut patch, context) = sample_patch_context("change answer increment");
        patch.ast_diff.hunks.push(DiffHunk {
            path: PathBuf::from("src/broken.py"),
            pre_sha: sha256_bytes(b"def ok():\n    return 1\n"),
            post_sha: sha256_bytes(b"def broken(:\n"),
            before: "def ok():\n    return 1\n".to_string(),
            after: "def broken(:\n".to_string(),
        });
        let (_, panel_t1, panel_t2) = materialize_inference_panels(&patch, &context).unwrap();
        assert!(panel_t1.slot(InstrumentSlot::EAst)[0] > 0.90);
        assert!(panel_t1.slot(InstrumentSlot::EDiff)[0] > 0.90);
        assert!(panel_t2.slot(InstrumentSlot::EStaticAnalysis)[0] > 0.0);
    }

    #[test]
    fn missing_witness_fails_closed_when_witness_slot_becomes_active() {
        let (mut patch, context) = sample_patch_context("change answer increment");
        patch.witness_chain_segment.clear();
        let err = materialize_inference_panels(&patch, &context).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_INSTRUMENT");
    }

    #[test]
    fn scenario_strings_do_not_steer_diff_slots() {
        let (patch, neutral_context) = sample_patch_context("change answer increment");
        let mut scenario_context = neutral_context.clone();
        scenario_context.problem_statement =
            "scenario: ood\nscenario: constellation_reject".to_string();
        let (_, neutral_t1, neutral_t2) =
            materialize_inference_panels(&patch, &neutral_context).unwrap();
        let (_, scenario_t1, scenario_t2) =
            materialize_inference_panels(&patch, &scenario_context).unwrap();

        assert_eq!(
            neutral_t1.slot(InstrumentSlot::EDiff),
            scenario_t1.slot(InstrumentSlot::EDiff)
        );
        assert_eq!(
            neutral_t2.slot(InstrumentSlot::EDiff),
            scenario_t2.slot(InstrumentSlot::EDiff)
        );
        assert_ne!(
            neutral_t1.slot(InstrumentSlot::EProblem),
            scenario_t1.slot(InstrumentSlot::EProblem),
            "problem text still remains typed evidence; it is just not a control channel"
        );
    }
}
