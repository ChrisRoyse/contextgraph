use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use context_graph_mejepa_instruments::materialize::{
    materialize_panel, PanelVectorInput, TimeStep,
};
use context_graph_mejepa_instruments::{InstrumentSlot, Panel};
use context_graph_mejepa_tct::{
    gtau_check, EntityType as TctEntityType, Language as TctLanguage,
    MutationCategory as TctMutationCategory, TctConstellation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::CalibrationStore;
use crate::calibration_types::CalibrationRecord;
use crate::config::MeJepaInferConfig;
use crate::conformal::{build_outcome_set, conformal_set, CalibrationExample};
use crate::degraded::{
    calibrated_confidence, calibrated_confidence_multiplier, compute_train_health, evidence_factor,
};
use crate::error::MejepaInferError;
use crate::gates::{check_source_sha_drift, replay_witness_segment};
use crate::ood::{compute_ood_score, squared_l2};
use crate::prediction_surfaces::{covered_chunks_for_patch, infer_phase_b_surfaces};
use crate::types::{
    validate_probability, AgentClaimGraph, ConformalInterval, ConformalMethod, FailedGate,
    PatchBundle, PredictionProvenance, RealityPrediction, RealityPredictionBuilder, TaskContext,
    VerifyVerdict, WitnessHash,
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

    fn session_known(&self, session_id: [u8; 16]) -> Result<bool, MejepaInferError>;

    fn read_recent_calibration_examples(
        &self,
        limit: usize,
    ) -> Result<Vec<CalibrationExample>, MejepaInferError>;
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
        })
    }

    pub fn calibration(&self) -> &CalibrationStore {
        &self.calibration
    }

    pub fn store(&self) -> &dyn MejepaStore {
        self.store.as_ref()
    }

    pub fn compile(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
    ) -> Result<RealityPrediction, MejepaInferError> {
        self.compile_with_panel(patch, context)
            .map(|(prediction, _predicted_panel)| prediction)
    }

    pub fn compile_with_panel(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
    ) -> Result<(RealityPrediction, Panel), MejepaInferError> {
        patch.validate()?;
        context.validate()?;
        let calibration = self.calibration.load_active()?;
        let (panel_t0, panel_t1, panel_t2) = materialize_inference_panels(patch, context)?;
        let predicted = self.predictor.predict(&panel_t0, &panel_t1)?;
        let target = self.frozen_target.target(&panel_t2)?;
        let oracle = self.oracle_head.score(&predicted)?;
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
        let phase_b_surfaces = infer_phase_b_surfaces(patch, context, &oracle.predicted_test_pass)?;
        let covered_chunks = covered_chunks_for_patch(patch)?;
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
            guard_violation_count: phase_b_surfaces.guard_violations.len(),
            ood_score,
            confidence_interval: &confidence_interval,
            pass_threshold: self.config.p_test_threshold,
            ood_threshold: self.config.ood_refuse_threshold,
            interval_width_threshold: 0.8,
        });
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
        .source_panel_sha(panel_sha(&predicted))
        .calibration_version(calibration.version.clone())
        .build()
        .map(|prediction| (prediction, predicted))
    }

    pub fn verify(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
    ) -> Result<VerifyVerdict, MejepaInferError> {
        patch.validate()?;
        context.validate()?;
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

        let (prediction, predicted_panel) = match self.compile_with_panel(patch, context) {
            Ok(value) => value,
            Err(MejepaInferError::OodRefuse {
                ood_score,
                threshold,
                ..
            }) => {
                return Ok(VerifyVerdict::EscalateToHuman {
                    reality_prediction: None,
                    failed_gate: FailedGate::OutOfDistribution {
                        ood_score,
                        threshold,
                    },
                    gates_passed: 2,
                });
            }
            Err(err) => return Err(err),
        };

        if prediction.ood_score > self.config.ood_refuse_threshold {
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
            return Ok(VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction.clone()),
                failed_gate: FailedGate::ConstellationGuardRejected {
                    reason: constellation.reason,
                },
                gates_passed: 6,
            });
        }

        self.store.write_live_prediction(&prediction)?;
        Ok(VerifyVerdict::Approve {
            reality_prediction: prediction,
            gates_passed: 7,
        })
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

fn witness_hash(patch: &PatchBundle) -> WitnessHash {
    let mut hasher = Sha256::new();
    hasher.update(&patch.witness_chain_segment);
    WitnessHash(hasher.finalize().into())
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
}

impl Default for DeterministicConstellationGuard {
    fn default() -> Self {
        Self {
            version_id: "tct-fsv-v1".to_string(),
            age_days: 0,
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
