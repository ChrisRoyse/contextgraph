use crate::artifact::{
    compute_artifact_hashes, compute_file_sha256, ensure_clean_run_dir, hex,
    load_artifact_for_inference, verify_artifact_files, ArtifactHashVerification, LoadedArtifact,
};
use crate::config::TrainConfig;
use crate::model::{train_tiny_jepa, TrainExample};
use candle_core::{Device, Tensor};
use context_graph_core::dynamicjepa::{
    ActionId, ActionOrigin, ArtifactStatus, CandidateActionConfig, DatasetId, DatasetShardRecord,
    DjRecordHeader, DomainPack, DynamicJepaError, DynamicJepaRecord, DynamicJepaResult, EventId,
    FieldKind, FieldSpec, FieldValue, GuardDecision, GuardDecisionId, GuardDecisionRecord,
    InstrumentKind, InstrumentSpec, LatentPanel, ModelArtifactId, ModelArtifactRecord,
    Normalization, NormalizedAction, OutcomeId, PanelId, PanelSlotKind, PlanTraceId,
    PlanTraceRecord, PlanTraceStatus, PredictionId, PredictionRecord, SkillId, SkillPolicyRecord,
    SkillStrategy, SurpriseEventId, SurpriseEventRecord, SurpriseKind, TrainingRunId,
    TrainingRunRecord, TrainingRunStatus, GUARD_DECISION_RECORD_VERSION,
    MODEL_ARTIFACT_RECORD_VERSION, NORMALIZED_ACTION_RECORD_VERSION, PLAN_TRACE_RECORD_VERSION,
    PREDICTION_RECORD_VERSION, SKILL_POLICY_RECORD_VERSION, SURPRISE_EVENT_RECORD_VERSION,
    TRAINING_RUN_RECORD_VERSION,
};
use context_graph_storage::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_AUDIT_LOG, CF_DJ_DATASET_SHARDS, CF_DJ_GUARD_DECISIONS,
    CF_DJ_LATENT_PANELS, CF_DJ_MODEL_ARTIFACTS, CF_DJ_OUTCOMES, CF_DJ_PLAN_TRACES,
    CF_DJ_PREDICTIONS, CF_DJ_SKILL_POLICIES, CF_DJ_SURPRISE_EVENTS, CF_DJ_TRAINING_RUNS,
};
use context_graph_storage::dynamicjepa::{
    get_action, get_domain_pack, get_guard_decision, get_latent_panel, get_model_artifact,
    get_normalized_state, get_outcome, get_plan_trace, get_prediction, get_skill_policy,
    get_surprise_event, get_training_run, list_dataset_shards, put_model_artifact_completion_batch,
    put_plan_batch, put_prediction_with_audit_batch, put_surprise_event_with_audit_batch,
    put_training_run_with_audit_batch, AuditStatus, DjAuditRecord,
};
use rocksdb::DB;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct TrainServiceOutcome {
    pub training_run_id: TrainingRunId,
    pub artifact_id: ModelArtifactId,
    pub artifact_root: PathBuf,
    pub metrics: BTreeMap<String, f64>,
    pub artifact_hashes: Vec<ArtifactHashVerification>,
    pub row_count_by_split: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PredictServiceOutcome {
    pub prediction_id: PredictionId,
    pub prediction: PredictionRecord,
    pub artifact: ModelArtifactRecord,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanServiceOutcome {
    pub plan_trace_id: PlanTraceId,
    pub plan_trace: PlanTraceRecord,
    pub skill_policy: SkillPolicyRecord,
    pub candidate_actions: Vec<NormalizedAction>,
    pub predictions: Vec<PredictionRecord>,
    pub guard_decisions: Vec<GuardDecisionRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SurpriseServiceOutcome {
    pub surprise_event_id: Option<SurpriseEventId>,
    pub surprise_event: Option<SurpriseEventRecord>,
    pub cosine: f32,
    pub threshold: f32,
    pub error_norm: f32,
    pub wrote_surprise: bool,
}

struct InferenceArtifact {
    loaded: LoadedArtifact,
    config: TrainConfig,
    tensors: HashMap<String, Tensor>,
}

pub fn train_dataset(
    db: &DB,
    dataset_id: DatasetId,
    config: &TrainConfig,
    artifact_root: &Path,
) -> DynamicJepaResult<TrainServiceOutcome> {
    let started_at = now_unix_ms()?;
    let training_run_id = TrainingRunId::new_v4();
    let config_bytes = config.canonical_bytes()?;
    let config_hash = sha256(&config_bytes);
    let (shards, row_count_by_split) = load_dataset_shards(db, dataset_id)?;
    let domain_id = shards[0].header.domain_pack_id.clone();
    let domain_version = shards[0].header.domain_pack_version.clone();
    get_domain_pack(db, &domain_id, &domain_version)?.ok_or_else(|| {
        DynamicJepaError::DomainPackNotFound {
            id: domain_id.to_string(),
        }
    })?;
    let objective_ids = objective_ids(&shards)?;
    let mut started = TrainingRunRecord {
        header: DjRecordHeader::new(
            training_run_id.0,
            TRAINING_RUN_RECORD_VERSION,
            domain_id.clone(),
            domain_version.clone(),
            started_at,
            Some(training_run_id.0),
        ),
        training_run_id,
        domain_pack_id: domain_id.clone(),
        dataset_id,
        started_at_unix_ms: started_at,
        finished_at_unix_ms: None,
        status: TrainingRunStatus::Started,
        training_config_hash: config_hash,
        objective_ids: objective_ids.clone(),
        metrics: BTreeMap::new(),
        artifact_ids: Vec::new(),
    };
    started.refresh_content_hash()?;
    let start_audit = build_audit(
        "start_training_run",
        AuditStatus::Ok,
        vec![dataset_id.to_string()],
        vec![training_run_id.to_string()],
        vec![CF_DJ_TRAINING_RUNS.to_string(), CF_DJ_AUDIT_LOG.to_string()],
        vec![started.header.content_hash],
    )?;
    put_training_run_with_audit_batch(db, &started, &start_audit)?;
    let read_started = get_training_run(db, training_run_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_TRAINING_RUNS.to_string(),
            key: training_run_id.into_bytes().to_vec(),
        }
    })?;
    if read_started != started {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!("started training run readback mismatch for {training_run_id}"),
        });
    }

    let after_start_result = (|| -> DynamicJepaResult<TrainServiceOutcome> {
        let examples = collect_examples(db, &shards)?;
        let trained = train_tiny_jepa(config, &objective_ids[0], &examples)
            .map_err(|err| attach_run_id(err, training_run_id))?;
        let base_root = prepare_artifact_root(artifact_root)?;
        let run_dir = ensure_clean_run_dir(&base_root, domain_id.as_str(), training_run_id.0)?;
        write_artifact_files(&run_dir, &config_bytes, &trained)?;
        let files = compute_artifact_hashes(&run_dir)?;
        let model_config_hash = compute_file_sha256(&run_dir.join("config.json"))?;
        let evaluation_report_hash = compute_file_sha256(&run_dir.join("evaluation_report.json"))?;
        let artifact_id = ModelArtifactId::new_v4();
        let created_at = now_unix_ms()?;
        let mut artifact = ModelArtifactRecord {
            header: DjRecordHeader::new(
                artifact_id.0,
                MODEL_ARTIFACT_RECORD_VERSION,
                domain_id.clone(),
                domain_version.clone(),
                created_at,
                Some(training_run_id.0),
            ),
            artifact_id,
            training_run_id,
            domain_pack_id: domain_id.clone(),
            domain_pack_version: domain_version.clone(),
            dataset_id,
            artifact_root: run_dir.clone(),
            files,
            model_config_hash,
            evaluation_report_hash,
            created_at_unix_ms: created_at,
            status: ArtifactStatus::Active,
        };
        artifact.refresh_content_hash()?;
        let finished_at = now_unix_ms()?.max(started_at);
        let mut completed = TrainingRunRecord {
            finished_at_unix_ms: Some(finished_at),
            status: TrainingRunStatus::Completed,
            metrics: trained.metrics.clone(),
            artifact_ids: vec![artifact_id],
            ..started.clone()
        };
        completed.header.created_at_unix_ms = finished_at;
        completed.refresh_content_hash()?;
        let completion_audit = build_audit(
            "register_model_artifact",
            AuditStatus::Ok,
            vec![training_run_id.to_string(), dataset_id.to_string()],
            vec![artifact_id.to_string()],
            vec![
                CF_DJ_MODEL_ARTIFACTS.to_string(),
                CF_DJ_TRAINING_RUNS.to_string(),
                CF_DJ_AUDIT_LOG.to_string(),
            ],
            vec![artifact.header.content_hash, completed.header.content_hash],
        )?;
        put_model_artifact_completion_batch(db, &artifact, &completed, &completion_audit)?;
        let stored_run = get_training_run(db, training_run_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_TRAINING_RUNS.to_string(),
                key: training_run_id.into_bytes().to_vec(),
            }
        })?;
        let stored_artifact = get_model_artifact(db, artifact_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_MODEL_ARTIFACTS.to_string(),
                key: artifact_id.into_bytes().to_vec(),
            }
        })?;
        if stored_run != completed || stored_artifact != artifact {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "training/artifact readback mismatch for run {training_run_id} artifact {artifact_id}"
                ),
            });
        }
        let artifact_hashes = verify_artifact_files(&stored_artifact)?;
        if artifact_hashes.iter().any(|check| !check.equal) {
            return Err(DynamicJepaError::ArtifactHashMismatch {
                artifact_id: stored_artifact.artifact_id.0,
                file: "artifact_root".to_string(),
                expected: "all registry hashes equal recomputed hashes".to_string(),
                actual: "one or more files differed".to_string(),
            });
        }
        info!(
            operation = "dynamicjepa_train",
            run_id = %training_run_id,
            domain_pack_id = %domain_id,
            source_of_truth_cf = "dj_training_runs,dj_model_artifacts",
            status = "ok",
            "DynamicJEPA training completed and artifact hashes verified"
        );
        Ok(TrainServiceOutcome {
            training_run_id,
            artifact_id,
            artifact_root: stored_artifact.artifact_root,
            metrics: stored_run.metrics,
            artifact_hashes,
            row_count_by_split,
        })
    })();

    match after_start_result {
        Ok(outcome) => Ok(outcome),
        Err(err) => {
            let err = attach_run_id(err, training_run_id);
            error!(
                operation = "dynamicjepa_train",
                run_id = %training_run_id,
                domain_pack_id = %domain_id,
                source_of_truth_cf = "dj_training_runs",
                status = "error",
                error_code = err.code(),
                error_message = %err,
                "DynamicJEPA training failed after started run was persisted"
            );
            let mut failed = TrainingRunRecord {
                finished_at_unix_ms: Some(now_unix_ms()?.max(started_at)),
                status: TrainingRunStatus::Failed {
                    error: err.to_string(),
                },
                ..started
            };
            failed.refresh_content_hash()?;
            let failed_audit = build_audit(
                "training_run_failed",
                AuditStatus::Failed {
                    error_code: err.code().to_string(),
                },
                vec![dataset_id.to_string()],
                vec![training_run_id.to_string()],
                vec![CF_DJ_TRAINING_RUNS.to_string(), CF_DJ_AUDIT_LOG.to_string()],
                vec![failed.header.content_hash],
            )?;
            put_training_run_with_audit_batch(db, &failed, &failed_audit)?;
            Err(err)
        }
    }
}

pub fn predict_next_panel(
    db: &DB,
    artifact_id: ModelArtifactId,
    panel_id: PanelId,
    action_id: ActionId,
) -> DynamicJepaResult<PredictServiceOutcome> {
    let artifact = load_inference_artifact(db, artifact_id)?;
    let domain = domain_for_artifact(db, &artifact.loaded.registry)?;
    let panel = get_latent_panel(db, panel_id)?.ok_or_else(|| {
        DynamicJepaError::PredictionInputMissing {
            target_id: panel_id.to_string(),
            cf: CF_DJ_LATENT_PANELS.to_string(),
        }
    })?;
    ensure_same_domain(
        &domain,
        panel.header.domain_pack_id.as_str(),
        "predict.panel",
    )?;
    let action =
        get_action(db, action_id)?.ok_or_else(|| DynamicJepaError::PredictionInputMissing {
            target_id: action_id.to_string(),
            cf: CF_DJ_ACTIONS.to_string(),
        })?;
    ensure_same_domain(
        &domain,
        action.header.domain_pack_id.as_str(),
        "predict.action",
    )?;
    let predicted = predict_vector(&artifact, &domain, &panel, &action)?;
    let uncertainty = vector_norm(&predicted) / (predicted.len().max(1) as f32).sqrt();
    if !uncertainty.is_finite() {
        return Err(DynamicJepaError::TrainingFailed {
            training_run_id: artifact.loaded.registry.training_run_id.0,
            message: "model produced non-finite uncertainty during prediction".to_string(),
            remediation: "retrain from a clean artifact root; inference must not persist NaN/Inf"
                .to_string(),
        });
    }
    let objective_id = domain
        .objective_specs
        .first()
        .map(|objective| objective.id.clone())
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "DomainPack.objective_specs",
                "domain pack has no objective for prediction scoring",
                "register a valid v1 domain pack before prediction",
            )
        })?;
    let created_at = now_unix_ms()?;
    let prediction_id = PredictionId::new_v4();
    let mut record = PredictionRecord {
        header: DjRecordHeader::new(
            prediction_id.0,
            PREDICTION_RECORD_VERSION,
            domain.id.clone(),
            domain.version.clone(),
            created_at,
            Some(prediction_id.0),
        ),
        prediction_id,
        model_artifact_id: artifact_id,
        model_artifact_hash_at_inference: artifact.loaded.model_file_sha256,
        input_panel_id: panel_id,
        candidate_action_id: action_id,
        predicted_next_panel_vec: predicted,
        uncertainty,
        objective_scores: BTreeMap::from([(objective_id, -uncertainty)]),
        created_at_unix_ms: created_at,
    };
    record.refresh_content_hash()?;
    let audit = build_audit(
        "run_prediction",
        AuditStatus::Ok,
        vec![
            artifact_id.to_string(),
            panel_id.to_string(),
            action_id.to_string(),
        ],
        vec![prediction_id.to_string()],
        vec![CF_DJ_PREDICTIONS.to_string(), CF_DJ_AUDIT_LOG.to_string()],
        vec![record.header.content_hash],
    )?;
    put_prediction_with_audit_batch(db, &record, &audit)?;
    let stored = get_prediction(db, prediction_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_PREDICTIONS.to_string(),
            key: prediction_id.into_bytes().to_vec(),
        }
    })?;
    if stored != record {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!("prediction readback mismatch for {prediction_id}"),
        });
    }
    info!(
        operation = "dynamicjepa_predict",
        run_id = %prediction_id,
        domain_pack_id = %domain.id,
        target_id = %panel_id,
        source_of_truth_cf = "dj_predictions",
        status = "ok",
        "DynamicJEPA prediction persisted and read back"
    );
    Ok(PredictServiceOutcome {
        prediction_id,
        prediction: stored,
        artifact: artifact.loaded.registry,
    })
}

pub fn plan_next_action(
    db: &DB,
    artifact_id: ModelArtifactId,
    panel_id: PanelId,
    skill_id: SkillId,
) -> DynamicJepaResult<PlanServiceOutcome> {
    let artifact = load_inference_artifact(db, artifact_id)?;
    let domain = domain_for_artifact(db, &artifact.loaded.registry)?;
    let panel = get_latent_panel(db, panel_id)?.ok_or_else(|| {
        DynamicJepaError::PredictionInputMissing {
            target_id: panel_id.to_string(),
            cf: CF_DJ_LATENT_PANELS.to_string(),
        }
    })?;
    ensure_same_domain(&domain, panel.header.domain_pack_id.as_str(), "plan.panel")?;
    let source_state = get_normalized_state(db, panel.state_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: "dj_normalized_states".to_string(),
            key: panel.state_id.into_bytes().to_vec(),
        }
    })?;
    let plan_trace_id = PlanTraceId::new_v4();
    let (skill_policy, write_skill_policy) =
        ensure_skill_policy(db, &domain, skill_id, plan_trace_id.0)?;
    let actions = enumerate_candidate_actions(&domain, &source_state, plan_trace_id.0)?;

    let mut predictions = Vec::with_capacity(actions.len());
    let mut guard_decisions =
        Vec::with_capacity(actions.len() * domain.planner_policy.guards.len());
    let mut utility_scores = Vec::with_capacity(actions.len());
    for action in &actions {
        let predicted = predict_vector(&artifact, &domain, &panel, action)?;
        let uncertainty = vector_norm(&predicted) / (predicted.len().max(1) as f32).sqrt();
        if !uncertainty.is_finite() {
            return Err(DynamicJepaError::TrainingFailed {
                training_run_id: artifact.loaded.registry.training_run_id.0,
                message: "model produced non-finite uncertainty during planning".to_string(),
                remediation:
                    "retrain from a clean artifact root; planning must not persist NaN/Inf"
                        .to_string(),
            });
        }
        let objective_id = domain
            .objective_specs
            .first()
            .map(|objective| objective.id.clone())
            .ok_or_else(|| {
                DynamicJepaError::validation(
                    "DomainPack.objective_specs",
                    "domain pack has no objective for plan scoring",
                    "register a valid v1 domain pack before planning",
                )
            })?;
        let prediction_id = PredictionId::new_v4();
        let created_at = now_unix_ms()?;
        let mut prediction = PredictionRecord {
            header: DjRecordHeader::new(
                prediction_id.0,
                PREDICTION_RECORD_VERSION,
                domain.id.clone(),
                domain.version.clone(),
                created_at,
                Some(plan_trace_id.0),
            ),
            prediction_id,
            model_artifact_id: artifact_id,
            model_artifact_hash_at_inference: artifact.loaded.model_file_sha256,
            input_panel_id: panel_id,
            candidate_action_id: action.action_id,
            predicted_next_panel_vec: predicted,
            uncertainty,
            objective_scores: BTreeMap::from([(objective_id, -uncertainty)]),
            created_at_unix_ms: created_at,
        };
        prediction.refresh_content_hash()?;
        let predicted_state = predicted_state_after_action(&domain, &source_state, action)?;
        for guard_id in &domain.planner_policy.guards {
            guard_decisions.push(build_guard_decision(
                &domain,
                plan_trace_id,
                guard_id,
                action.action_id,
                prediction.prediction_id,
                &predicted_state,
            )?);
        }
        utility_scores.push(utility_score(&domain, &predicted_state, uncertainty)?);
        predictions.push(prediction);
    }

    let selected_action_id = select_allowed_action(
        &actions,
        &guard_decisions,
        &utility_scores,
        &domain.planner_policy.guards,
    );
    let status = if selected_action_id.is_some() {
        PlanTraceStatus::Selected
    } else {
        PlanTraceStatus::Rejected
    };
    let created_at = now_unix_ms()?;
    let mut trace = PlanTraceRecord {
        header: DjRecordHeader::new(
            plan_trace_id.0,
            PLAN_TRACE_RECORD_VERSION,
            domain.id.clone(),
            domain.version.clone(),
            created_at,
            Some(plan_trace_id.0),
        ),
        plan_trace_id,
        domain_pack_id: domain.id.clone(),
        current_panel_id: panel_id,
        model_artifact_id: artifact_id,
        model_artifact_hash_at_plan: artifact.loaded.model_file_sha256,
        skill_policy_id: skill_id,
        candidate_action_ids: actions.iter().map(|action| action.action_id).collect(),
        prediction_ids: predictions
            .iter()
            .map(|prediction| prediction.prediction_id)
            .collect(),
        guard_decision_ids: guard_decisions
            .iter()
            .map(|guard| guard.guard_decision_id.0)
            .collect(),
        utility_scores,
        selected_action_id,
        status,
        created_at_unix_ms: created_at,
    };
    trace.refresh_content_hash()?;
    let mut touched = vec![
        CF_DJ_ACTIONS.to_string(),
        CF_DJ_PREDICTIONS.to_string(),
        CF_DJ_GUARD_DECISIONS.to_string(),
        CF_DJ_PLAN_TRACES.to_string(),
        CF_DJ_AUDIT_LOG.to_string(),
    ];
    if write_skill_policy {
        touched.push(CF_DJ_SKILL_POLICIES.to_string());
    }
    let mut content_hashes = actions
        .iter()
        .map(|action| action.header.content_hash)
        .collect::<Vec<_>>();
    content_hashes.extend(
        predictions
            .iter()
            .map(|prediction| prediction.header.content_hash),
    );
    content_hashes.extend(
        guard_decisions
            .iter()
            .map(|guard| guard.header.content_hash),
    );
    content_hashes.push(trace.header.content_hash);
    let audit = build_audit(
        "run_plan",
        AuditStatus::Ok,
        vec![
            artifact_id.to_string(),
            panel_id.to_string(),
            skill_id.to_string(),
        ],
        vec![plan_trace_id.to_string()],
        touched,
        content_hashes,
    )?;
    put_plan_batch(
        db,
        write_skill_policy.then_some(&skill_policy),
        &actions,
        &predictions,
        &guard_decisions,
        &trace,
        &audit,
    )?;
    let stored_trace = get_plan_trace(db, plan_trace_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_PLAN_TRACES.to_string(),
            key: plan_trace_id.into_bytes().to_vec(),
        }
    })?;
    if stored_trace != trace {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!("plan trace readback mismatch for {plan_trace_id}"),
        });
    }
    for prediction in &predictions {
        let stored = get_prediction(db, prediction.prediction_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_PREDICTIONS.to_string(),
                key: prediction.prediction_id.into_bytes().to_vec(),
            }
        })?;
        if stored != *prediction {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "plan prediction readback mismatch for {}",
                    prediction.prediction_id
                ),
            });
        }
    }
    for guard in &guard_decisions {
        let stored = get_guard_decision(db, guard.guard_decision_id)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_GUARD_DECISIONS.to_string(),
                key: guard.guard_decision_id.into_bytes().to_vec(),
            }
        })?;
        if stored != *guard {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "guard decision readback mismatch for {}",
                    guard.guard_decision_id
                ),
            });
        }
    }
    info!(
        operation = "dynamicjepa_plan",
        run_id = %plan_trace_id,
        domain_pack_id = %domain.id,
        source_of_truth_cf = "dj_actions,dj_predictions,dj_guard_decisions,dj_plan_traces",
        status = "ok",
        "DynamicJEPA plan persisted and read back"
    );
    Ok(PlanServiceOutcome {
        plan_trace_id,
        plan_trace: stored_trace,
        skill_policy,
        candidate_actions: actions,
        predictions,
        guard_decisions,
    })
}

pub fn record_surprise(
    db: &DB,
    prediction_id: PredictionId,
    observed_outcome_id: OutcomeId,
    observed_panel_id: PanelId,
) -> DynamicJepaResult<SurpriseServiceOutcome> {
    let prediction = get_prediction(db, prediction_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_PREDICTIONS.to_string(),
            key: prediction_id.into_bytes().to_vec(),
        }
    })?;
    get_outcome(db, observed_outcome_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_OUTCOMES.to_string(),
            key: observed_outcome_id.into_bytes().to_vec(),
        }
    })?;
    let observed_panel = get_latent_panel(db, observed_panel_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_LATENT_PANELS.to_string(),
            key: observed_panel_id.into_bytes().to_vec(),
        }
    })?;
    let artifact = load_inference_artifact(db, prediction.model_artifact_id)?;
    let domain = domain_for_artifact(db, &artifact.loaded.registry)?;
    ensure_same_domain(
        &domain,
        observed_panel.header.domain_pack_id.as_str(),
        "record-surprise.observed_panel",
    )?;
    let observed_vec = encode_panel_vector(&artifact, &observed_panel)?;
    if observed_vec.len() != prediction.predicted_next_panel_vec.len() {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: domain.id.to_string(),
            expected: vec![prediction.predicted_next_panel_vec.len()],
            actual: vec![observed_vec.len()],
        });
    }
    let cosine = cosine_f32(&prediction.predicted_next_panel_vec, &observed_vec);
    let error_norm = l2_distance(&prediction.predicted_next_panel_vec, &observed_vec);
    let threshold = domain.planner_policy.surprise_threshold;
    if cosine >= threshold {
        return Ok(SurpriseServiceOutcome {
            surprise_event_id: None,
            surprise_event: None,
            cosine,
            threshold,
            error_norm,
            wrote_surprise: false,
        });
    }
    let created_at = now_unix_ms()?;
    let surprise_event_id = SurpriseEventId::new_v4();
    let mut record = SurpriseEventRecord {
        header: DjRecordHeader::new(
            surprise_event_id.0,
            SURPRISE_EVENT_RECORD_VERSION,
            domain.id.clone(),
            domain.version.clone(),
            created_at,
            Some(surprise_event_id.0),
        ),
        surprise_event_id,
        prediction_id,
        observed_outcome_id,
        observed_panel_id,
        surprise_kind: SurpriseKind::UnexpectedOutcome,
        cosine,
        threshold,
        error_norm,
        created_at_unix_ms: created_at,
    };
    record.refresh_content_hash()?;
    let audit = build_audit(
        "record_surprise",
        AuditStatus::Ok,
        vec![
            prediction_id.to_string(),
            observed_outcome_id.to_string(),
            observed_panel_id.to_string(),
        ],
        vec![surprise_event_id.to_string()],
        vec![
            CF_DJ_SURPRISE_EVENTS.to_string(),
            CF_DJ_AUDIT_LOG.to_string(),
        ],
        vec![record.header.content_hash],
    )?;
    put_surprise_event_with_audit_batch(db, &record, &audit)?;
    let stored = get_surprise_event(db, surprise_event_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_SURPRISE_EVENTS.to_string(),
            key: surprise_event_id.into_bytes().to_vec(),
        }
    })?;
    if stored != record {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!("surprise event readback mismatch for {surprise_event_id}"),
        });
    }
    Ok(SurpriseServiceOutcome {
        surprise_event_id: Some(surprise_event_id),
        surprise_event: Some(stored),
        cosine,
        threshold,
        error_norm,
        wrote_surprise: true,
    })
}

fn load_inference_artifact(
    db: &DB,
    artifact_id: ModelArtifactId,
) -> DynamicJepaResult<InferenceArtifact> {
    let loaded = load_artifact_for_inference(db, artifact_id)?;
    let config_path = loaded.registry.artifact_root.join("config.json");
    let config = TrainConfig::from_path(&config_path)?;
    let model_path = loaded.registry.artifact_root.join("model.safetensors");
    let tensors = candle_core::safetensors::load(&model_path, &Device::Cpu).map_err(|err| {
        DynamicJepaError::TrainingFailed {
            training_run_id: loaded.registry.training_run_id.0,
            message: format!(
                "failed to load verified model tensors from {}: {err}",
                model_path.display()
            ),
            remediation:
                "rerun training; inference must not continue without the registered tensor artifact"
                    .to_string(),
        }
    })?;
    Ok(InferenceArtifact {
        loaded,
        config,
        tensors,
    })
}

fn domain_for_artifact(db: &DB, artifact: &ModelArtifactRecord) -> DynamicJepaResult<DomainPack> {
    get_domain_pack(db, &artifact.domain_pack_id, &artifact.domain_pack_version)?.ok_or_else(|| {
        DynamicJepaError::DomainPackNotFound {
            id: artifact.domain_pack_id.to_string(),
        }
    })
}

fn ensure_same_domain(domain: &DomainPack, actual: &str, field: &str) -> DynamicJepaResult<()> {
    if domain.id.as_str() == actual {
        return Ok(());
    }
    Err(DynamicJepaError::validation(
        field,
        format!("expected domain {} but found {actual}", domain.id),
        "use artifact, panel, action, and outcome rows from the same registered domain pack",
    ))
}

fn predict_vector(
    artifact: &InferenceArtifact,
    domain: &DomainPack,
    panel: &LatentPanel,
    action: &NormalizedAction,
) -> DynamicJepaResult<Vec<f32>> {
    let input = flatten_panel(panel)?;
    let action = action_dense_vector(domain, action)?;
    if artifact.config.model.predictor.in_action_dim != action.len() {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: domain.id.to_string(),
            expected: vec![artifact.config.model.predictor.in_action_dim],
            actual: vec![action.len()],
        });
    }
    let z = mlp_forward(
        &artifact.tensors,
        "online.encoder",
        artifact.config.model.encoder.hidden.len() + 1,
        &input,
        artifact.loaded.registry.training_run_id,
    )?;
    if z.len() != artifact.config.model.encoder.out_dim {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: domain.id.to_string(),
            expected: vec![artifact.config.model.encoder.out_dim],
            actual: vec![z.len()],
        });
    }
    let mut joined = z;
    joined.extend_from_slice(&action);
    mlp_forward(
        &artifact.tensors,
        "predictor",
        artifact.config.model.predictor.hidden.len() + 1,
        &joined,
        artifact.loaded.registry.training_run_id,
    )
}

fn encode_panel_vector(
    artifact: &InferenceArtifact,
    panel: &LatentPanel,
) -> DynamicJepaResult<Vec<f32>> {
    mlp_forward(
        &artifact.tensors,
        "online.encoder",
        artifact.config.model.encoder.hidden.len() + 1,
        &flatten_panel(panel)?,
        artifact.loaded.registry.training_run_id,
    )
}

fn mlp_forward(
    tensors: &HashMap<String, Tensor>,
    prefix: &str,
    layer_count: usize,
    input: &[f32],
    training_run_id: TrainingRunId,
) -> DynamicJepaResult<Vec<f32>> {
    let mut x = input.to_vec();
    for layer_idx in 0..layer_count {
        let weight_name = format!("{prefix}.layer{layer_idx}.weight");
        let bias_name = format!("{prefix}.layer{layer_idx}.bias");
        let weight = tensors
            .get(&weight_name)
            .ok_or_else(|| DynamicJepaError::TrainingFailed {
                training_run_id: training_run_id.0,
                message: format!("verified artifact is missing tensor {weight_name}"),
                remediation: "rerun training; model artifacts must contain every MLP tensor"
                    .to_string(),
            })?;
        let bias = tensors
            .get(&bias_name)
            .ok_or_else(|| DynamicJepaError::TrainingFailed {
                training_run_id: training_run_id.0,
                message: format!("verified artifact is missing tensor {bias_name}"),
                remediation: "rerun training; model artifacts must contain every MLP tensor"
                    .to_string(),
            })?;
        let weight = weight
            .to_vec2::<f32>()
            .map_err(|err| DynamicJepaError::TrainingFailed {
                training_run_id: training_run_id.0,
                message: format!("failed to decode tensor {weight_name} as f32 matrix: {err}"),
                remediation: "rerun training; inference requires finite f32 MLP weights"
                    .to_string(),
            })?;
        let bias = bias
            .to_vec1::<f32>()
            .map_err(|err| DynamicJepaError::TrainingFailed {
                training_run_id: training_run_id.0,
                message: format!("failed to decode tensor {bias_name} as f32 vector: {err}"),
                remediation: "rerun training; inference requires finite f32 MLP weights"
                    .to_string(),
            })?;
        if weight.len() != bias.len() {
            return Err(DynamicJepaError::TrainingFailed {
                training_run_id: training_run_id.0,
                message: format!(
                    "tensor shape mismatch for {prefix}.layer{layer_idx}: weight rows {} != bias len {}",
                    weight.len(),
                    bias.len()
                ),
                remediation: "rerun training; model tensor dimensions must match config".to_string(),
            });
        }
        let mut next = Vec::with_capacity(weight.len());
        for (row_idx, row) in weight.iter().enumerate() {
            if row.len() != x.len() {
                return Err(DynamicJepaError::PanelShapeMismatch {
                    domain_pack_id: format!("{prefix}.layer{layer_idx}"),
                    expected: vec![row.len()],
                    actual: vec![x.len()],
                });
            }
            let mut acc = bias[row_idx];
            for (w, v) in row.iter().zip(x.iter()) {
                acc += *w * *v;
            }
            if layer_idx + 1 != layer_count {
                acc = acc.max(0.0);
            }
            if !acc.is_finite() {
                return Err(DynamicJepaError::TrainingFailed {
                    training_run_id: training_run_id.0,
                    message: format!("{prefix}.layer{layer_idx} produced non-finite value"),
                    remediation:
                        "retrain from a clean artifact root; inference must not persist NaN/Inf"
                            .to_string(),
                });
            }
            next.push(acc);
        }
        x = next;
    }
    if x.is_empty() {
        return Err(DynamicJepaError::TrainingFailed {
            training_run_id: training_run_id.0,
            message: format!("{prefix} produced an empty output vector"),
            remediation: "rerun training with a positive encoder/predictor output dimension"
                .to_string(),
        });
    }
    Ok(x)
}

fn action_dense_vector(
    domain: &DomainPack,
    action: &NormalizedAction,
) -> DynamicJepaResult<Vec<f32>> {
    let mut out = Vec::new();
    for spec in &domain.instrument_specs {
        if !spec
            .input_fields
            .iter()
            .any(|field| field.starts_with("action."))
        {
            continue;
        }
        out.extend(action_instrument_vector(spec, action, domain)?);
    }
    if out.is_empty() {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: domain.id.to_string(),
            expected: vec![1],
            actual: vec![0],
        });
    }
    Ok(out)
}

fn action_instrument_vector(
    spec: &InstrumentSpec,
    action: &NormalizedAction,
    domain: &DomainPack,
) -> DynamicJepaResult<Vec<f32>> {
    let values = spec
        .input_fields
        .iter()
        .map(|field| {
            let name = action_field_name(field)?;
            action.fields.get(name).cloned().ok_or_else(|| {
                DynamicJepaError::PredictionInputMissing {
                    target_id: format!("{}.{}", action.action_id, field),
                    cf: CF_DJ_ACTIONS.to_string(),
                }
            })
        })
        .collect::<DynamicJepaResult<Vec<_>>>()?;
    let output = match &spec.kind {
        InstrumentKind::Scalar => {
            if values.len() != 1 {
                return Err(DynamicJepaError::validation(
                    "predict.action_scalar",
                    format!(
                        "scalar action instrument {} has {} inputs",
                        spec.instrument_id,
                        values.len()
                    ),
                    "declare scalar action instruments with exactly one action field",
                ));
            }
            let raw = numeric_field_value(&values[0]).ok_or_else(|| {
                DynamicJepaError::validation(
                    "predict.action_scalar",
                    format!(
                        "action instrument {} expected numeric input",
                        spec.instrument_id
                    ),
                    "use the same typed action fields that panel materialization uses",
                )
            })?;
            vec![normalize_value(raw, &spec.normalization)? as f32]
        }
        InstrumentKind::Categorical { variants } => {
            if values.len() != 1 {
                return Err(DynamicJepaError::validation(
                    "predict.action_categorical",
                    format!(
                        "categorical action instrument {} has {} inputs",
                        spec.instrument_id,
                        values.len()
                    ),
                    "declare categorical action instruments with exactly one action field",
                ));
            }
            let variant = match &values[0] {
                FieldValue::Categorical { variant } | FieldValue::String(variant) => variant,
                other => {
                    return Err(DynamicJepaError::validation(
                        "predict.action_categorical",
                        format!(
                            "expected categorical/string action, got {}",
                            field_value_kind(other)
                        ),
                        "use declared action variants for hypothetical and observed actions",
                    ));
                }
            };
            let idx = variants
                .iter()
                .position(|item| item == variant)
                .ok_or_else(|| {
                    DynamicJepaError::validation(
                        "predict.action_categorical",
                        format!(
                            "variant {variant:?} is not declared for {}",
                            spec.instrument_id
                        ),
                        "generate candidates from the registered domain pack",
                    )
                })?;
            onehot(variants.len(), idx)
        }
        other => {
            return Err(DynamicJepaError::validation(
                "predict.action_instrument",
                format!("unsupported action instrument kind {other:?}"),
                "Phase 8 supports scalar and categorical action instruments",
            ));
        }
    };
    let expected = spec.output_shape.iter().product::<usize>();
    if output.len() != expected {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: domain.id.to_string(),
            expected: vec![expected],
            actual: vec![output.len()],
        });
    }
    Ok(output)
}

fn action_field_name(field: &str) -> DynamicJepaResult<&str> {
    field.strip_prefix("action.").ok_or_else(|| {
        DynamicJepaError::validation(
            "action_instrument.input_fields",
            format!("expected action.<field>, got {field:?}"),
            "action vectors can only be built from action-scoped instrument inputs",
        )
    })
}

fn ensure_skill_policy(
    db: &DB,
    domain: &DomainPack,
    skill_id: SkillId,
    plan_run_id: Uuid,
) -> DynamicJepaResult<(SkillPolicyRecord, bool)> {
    if let Some(existing) = get_skill_policy(db, skill_id)? {
        if existing.domain_pack_id != domain.id
            || existing.strategy != SkillStrategy::EnumerateDeclaredActions
        {
            return Err(DynamicJepaError::validation(
                "plan.skill_id",
                format!("skill policy {skill_id} exists but does not match domain {}", domain.id),
                "use a fresh skill id or the existing enumerate_declared_actions skill for this domain",
            ));
        }
        return Ok((existing, false));
    }
    let now = now_unix_ms()?;
    let mut record = SkillPolicyRecord {
        header: DjRecordHeader::new(
            skill_id.0,
            SKILL_POLICY_RECORD_VERSION,
            domain.id.clone(),
            domain.version.clone(),
            now,
            Some(plan_run_id),
        ),
        skill_id,
        domain_pack_id: domain.id.clone(),
        skill_name: "enumerate_declared_actions".to_string(),
        strategy: SkillStrategy::EnumerateDeclaredActions,
        version: 1,
    };
    record.refresh_content_hash()?;
    Ok((record, true))
}

fn enumerate_candidate_actions(
    domain: &DomainPack,
    source_state: &context_graph_core::dynamicjepa::NormalizedState,
    plan_run_id: Uuid,
) -> DynamicJepaResult<Vec<NormalizedAction>> {
    let mut actions = Vec::new();
    match &domain.planner_policy.candidate_actions {
        CandidateActionConfig::EnumeratedDeltas { deltas } => {
            let field = single_action_field(domain)?;
            for delta in deltas {
                actions.push(build_hypothetical_action(
                    domain,
                    source_state.source_event_id,
                    plan_run_id,
                    field,
                    FieldValue::I64(*delta),
                )?);
            }
        }
        CandidateActionConfig::EnumeratedMoves { moves } => {
            let field = single_action_field(domain)?;
            for movement in moves {
                actions.push(build_hypothetical_action(
                    domain,
                    source_state.source_event_id,
                    plan_run_id,
                    field,
                    FieldValue::Categorical {
                        variant: movement.clone(),
                    },
                )?);
            }
        }
    }
    Ok(actions)
}

fn build_hypothetical_action(
    domain: &DomainPack,
    source_event_id: EventId,
    plan_run_id: Uuid,
    field: &str,
    value: FieldValue,
) -> DynamicJepaResult<NormalizedAction> {
    validate_action_field(domain, field, &value)?;
    let action_id = ActionId::new_v4();
    let now = now_unix_ms()?;
    let mut record = NormalizedAction {
        header: DjRecordHeader::new(
            action_id.0,
            NORMALIZED_ACTION_RECORD_VERSION,
            domain.id.clone(),
            domain.version.clone(),
            now,
            Some(plan_run_id),
        ),
        action_id,
        fields: BTreeMap::from([(field.to_string(), value)]),
        source_event_id,
        action_origin: ActionOrigin::Hypothetical,
    };
    record.refresh_content_hash()?;
    Ok(record)
}

fn single_action_field(domain: &DomainPack) -> DynamicJepaResult<&str> {
    if domain.action_schema.fields.len() == 1 {
        return Ok(domain.action_schema.fields[0].name.as_str());
    }
    Err(DynamicJepaError::validation(
        "DomainPack.action_schema",
        format!(
            "planner candidate enumeration expected one action field, got {}",
            domain.action_schema.fields.len()
        ),
        "Phase 8 demo domains declare one finite action field",
    ))
}

fn validate_action_field(
    domain: &DomainPack,
    field: &str,
    value: &FieldValue,
) -> DynamicJepaResult<()> {
    let spec = domain
        .action_schema
        .fields
        .iter()
        .find(|spec| spec.name == field)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "DomainPack.action_schema",
                format!("candidate action field {field:?} is not declared"),
                "generate candidates only from domain-pack action schema",
            )
        })?;
    validate_field_value_against_spec("candidate_action", spec, value)
}

fn validate_field_value_against_spec(
    context: &str,
    spec: &FieldSpec,
    value: &FieldValue,
) -> DynamicJepaResult<()> {
    match (&spec.kind, value) {
        (FieldKind::I64, FieldValue::I64(value)) => {
            check_min_max(context, &spec.name, *value as f64, spec.min, spec.max)
        }
        (FieldKind::F64, FieldValue::F64(value)) => {
            check_min_max(context, &spec.name, *value, spec.min, spec.max)
        }
        (FieldKind::Categorical { variants }, FieldValue::Categorical { variant })
        | (FieldKind::Categorical { variants }, FieldValue::String(variant)) => {
            if variants.iter().any(|item| item == variant) {
                Ok(())
            } else {
                Err(DynamicJepaError::validation(
                    format!("{context}.{}", spec.name),
                    format!("variant {variant:?} is not in declared variants {variants:?}"),
                    "generate candidates only from the registered domain pack",
                ))
            }
        }
        (expected, actual) => Err(DynamicJepaError::validation(
            format!("{context}.{}", spec.name),
            format!(
                "expected field kind {expected:?}, got {}",
                field_value_kind(actual)
            ),
            "write typed DynamicJEPA field values without coercion",
        )),
    }
}

fn check_min_max(
    context: &str,
    field: &str,
    value: f64,
    min: Option<f64>,
    max: Option<f64>,
) -> DynamicJepaResult<()> {
    if let Some(min) = min {
        if value < min {
            return Err(DynamicJepaError::validation(
                format!("{context}.{field}"),
                format!("value {value} is below min {min}"),
                "candidate action violates the registered domain schema",
            ));
        }
    }
    if let Some(max) = max {
        if value > max {
            return Err(DynamicJepaError::validation(
                format!("{context}.{field}"),
                format!("value {value} exceeds max {max}"),
                "candidate action violates the registered domain schema",
            ));
        }
    }
    Ok(())
}

fn build_guard_decision(
    domain: &DomainPack,
    plan_trace_id: PlanTraceId,
    guard_id: &str,
    action_id: ActionId,
    prediction_id: PredictionId,
    predicted_state: &BTreeMap<String, f64>,
) -> DynamicJepaResult<GuardDecisionRecord> {
    if guard_id != "bounds_check" {
        return Err(DynamicJepaError::validation(
            "PlannerPolicy.guards",
            format!("unsupported guard {guard_id:?}"),
            "Phase 8 demo supports bounds_check only",
        ));
    }
    let mut decision = GuardDecision::Allow;
    let mut thresholds = BTreeMap::new();
    for field in &domain.state_schema.fields {
        let Some(value) = predicted_state.get(&field.name) else {
            continue;
        };
        thresholds.insert(format!("predicted.{}", field.name), *value);
        let (effective_min, effective_max) = effective_state_bounds(domain, field);
        if let Some(min) = effective_min {
            thresholds.insert(format!("min.{}", field.name), min);
            if *value < min {
                decision = GuardDecision::Reject {
                    reason_code: "STATE_BELOW_MIN".to_string(),
                    reason_message: format!(
                        "predicted state {}={} is below declared min {}",
                        field.name, value, min
                    ),
                };
                break;
            }
        }
        if let Some(max) = effective_max {
            thresholds.insert(format!("max.{}", field.name), max);
            if *value > max {
                decision = GuardDecision::Reject {
                    reason_code: "STATE_ABOVE_MAX".to_string(),
                    reason_message: format!(
                        "predicted state {}={} exceeds declared max {}",
                        field.name, value, max
                    ),
                };
                break;
            }
        }
    }
    let now = now_unix_ms()?;
    let guard_decision_id = GuardDecisionId::new_v4();
    let mut record = GuardDecisionRecord {
        header: DjRecordHeader::new(
            guard_decision_id.0,
            GUARD_DECISION_RECORD_VERSION,
            domain.id.clone(),
            domain.version.clone(),
            now,
            Some(plan_trace_id.0),
        ),
        guard_decision_id,
        plan_trace_id: plan_trace_id.0,
        guard_id: guard_id.to_string(),
        candidate_action_id: action_id,
        decision,
        evidence_refs: vec![
            format!("prediction:{prediction_id}"),
            format!("action:{action_id}"),
        ],
        threshold_values: thresholds,
        created_at_unix_ms: now,
    };
    record.refresh_content_hash()?;
    Ok(record)
}

fn effective_state_bounds(domain: &DomainPack, field: &FieldSpec) -> (Option<f64>, Option<f64>) {
    let mut min = field.min;
    let mut max = field.max;
    for spec in &domain.instrument_specs {
        if !spec
            .input_fields
            .iter()
            .any(|input| input == &format!("state.{}", field.name))
        {
            continue;
        }
        if let Some(instrument_min) = spec.validation.min {
            min = Some(min.map_or(instrument_min, |schema_min| schema_min.max(instrument_min)));
        }
        if let Some(instrument_max) = spec.validation.max {
            max = Some(max.map_or(instrument_max, |schema_max| schema_max.min(instrument_max)));
        }
    }
    (min, max)
}

fn predicted_state_after_action(
    domain: &DomainPack,
    state: &context_graph_core::dynamicjepa::NormalizedState,
    action: &NormalizedAction,
) -> DynamicJepaResult<BTreeMap<String, f64>> {
    if state.fields.contains_key("counter") && action.fields.contains_key("delta") {
        let counter = numeric_field_value(state.fields.get("counter").expect("checked"))
            .ok_or_else(|| {
                DynamicJepaError::validation(
                    "bounds_check.counter",
                    "state.counter is not numeric",
                    "counter_world bounds guard requires numeric counter state",
                )
            })?;
        let delta =
            numeric_field_value(action.fields.get("delta").expect("checked")).ok_or_else(|| {
                DynamicJepaError::validation(
                    "bounds_check.delta",
                    "action.delta is not numeric",
                    "counter_world bounds guard requires numeric delta action",
                )
            })?;
        return Ok(BTreeMap::from([("counter".to_string(), counter + delta)]));
    }
    if state.fields.contains_key("x")
        && state.fields.contains_key("y")
        && action.fields.contains_key("move")
    {
        let mut x =
            numeric_field_value(state.fields.get("x").expect("checked")).ok_or_else(|| {
                DynamicJepaError::validation(
                    "bounds_check.x",
                    "state.x is not numeric",
                    "gridworld bounds guard requires numeric x state",
                )
            })?;
        let mut y =
            numeric_field_value(state.fields.get("y").expect("checked")).ok_or_else(|| {
                DynamicJepaError::validation(
                    "bounds_check.y",
                    "state.y is not numeric",
                    "gridworld bounds guard requires numeric y state",
                )
            })?;
        let max_x = state_field(domain, "x")
            .and_then(|field| field.max)
            .unwrap_or(x);
        let max_y = state_field(domain, "y")
            .and_then(|field| field.max)
            .unwrap_or(y);
        let min_x = state_field(domain, "x")
            .and_then(|field| field.min)
            .unwrap_or(x);
        let min_y = state_field(domain, "y")
            .and_then(|field| field.min)
            .unwrap_or(y);
        let movement = match action.fields.get("move").expect("checked") {
            FieldValue::Categorical { variant } | FieldValue::String(variant) => variant.as_str(),
            other => {
                return Err(DynamicJepaError::validation(
                    "bounds_check.move",
                    format!(
                        "action.move must be categorical/string, got {}",
                        field_value_kind(other)
                    ),
                    "gridworld bounds guard requires a declared move action",
                ));
            }
        };
        match movement {
            "up" => y -= 1.0,
            "down" => y += 1.0,
            "left" => x -= 1.0,
            "right" => x += 1.0,
            "noop" => {}
            other => {
                return Err(DynamicJepaError::validation(
                    "bounds_check.move",
                    format!("unsupported move {other:?}"),
                    "generate moves from the registered domain pack",
                ));
            }
        }
        x = x.clamp(min_x, max_x);
        y = y.clamp(min_y, max_y);
        return Ok(BTreeMap::from([("x".to_string(), x), ("y".to_string(), y)]));
    }
    Err(DynamicJepaError::validation(
        "bounds_check",
        "cannot infer predicted state from the registered state/action schema",
        "Phase 8 bounds_check supports counter/delta and x/y/move domain-pack schemas",
    ))
}

fn state_field<'a>(domain: &'a DomainPack, name: &str) -> Option<&'a FieldSpec> {
    domain
        .state_schema
        .fields
        .iter()
        .find(|field| field.name == name)
}

fn utility_score(
    domain: &DomainPack,
    predicted_state: &BTreeMap<String, f64>,
    uncertainty: f32,
) -> DynamicJepaResult<f32> {
    if let Some(counter) = predicted_state.get("counter") {
        return Ok(*counter as f32);
    }
    if predicted_state.contains_key("x") && predicted_state.contains_key("y") {
        return Ok(-uncertainty);
    }
    Err(DynamicJepaError::validation(
        "plan.utility",
        format!("no v1 utility rule matched domain {}", domain.id),
        "Phase 8 demo supports counter utility and gridworld lowest-uncertainty utility",
    ))
}

fn select_allowed_action(
    actions: &[NormalizedAction],
    guards: &[GuardDecisionRecord],
    scores: &[f32],
    guard_ids: &[String],
) -> Option<ActionId> {
    let mut selected = None;
    let mut selected_score = f32::NEG_INFINITY;
    for (idx, action) in actions.iter().enumerate() {
        let allow = guards
            .iter()
            .filter(|guard| guard.candidate_action_id == action.action_id)
            .filter(|guard| guard_ids.iter().any(|id| id == &guard.guard_id))
            .all(|guard| matches!(guard.decision, GuardDecision::Allow));
        if allow && scores[idx] > selected_score {
            selected = Some(action.action_id);
            selected_score = scores[idx];
        }
    }
    selected
}

fn numeric_field_value(value: &FieldValue) -> Option<f64> {
    match value {
        FieldValue::I64(value) | FieldValue::UnixMs(value) => Some(*value as f64),
        FieldValue::F64(value) => Some(*value),
        _ => None,
    }
}

fn field_value_kind(value: &FieldValue) -> &'static str {
    match value {
        FieldValue::I64(_) => "i64",
        FieldValue::F64(_) => "f64",
        FieldValue::Bool(_) => "bool",
        FieldValue::String(_) => "string",
        FieldValue::Categorical { .. } => "categorical",
        FieldValue::Vector(_) => "vector",
        FieldValue::UnixMs(_) => "unix_ms",
    }
}

fn normalize_value(value: f64, normalization: &Normalization) -> DynamicJepaResult<f64> {
    match normalization {
        Normalization::None => Ok(value),
        Normalization::StandardScore { mean, std } => {
            if *std <= 0.0 || !std.is_finite() {
                return Err(DynamicJepaError::validation(
                    "instrument.normalization.std",
                    format!("std must be finite and > 0, got {std}"),
                    "fix the registered domain pack normalization",
                ));
            }
            Ok((value - *mean) / *std)
        }
    }
}

fn onehot(dim: usize, idx: usize) -> Vec<f32> {
    let mut values = vec![0.0; dim];
    values[idx] = 1.0;
    values
}

fn vector_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn cosine_f32(lhs: &[f32], rhs: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut ln = 0.0f32;
    let mut rn = 0.0f32;
    for (left, right) in lhs.iter().zip(rhs.iter()) {
        dot += *left * *right;
        ln += *left * *left;
        rn += *right * *right;
    }
    let denom = ln.sqrt() * rn.sqrt();
    if denom <= f32::EPSILON {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

fn l2_distance(lhs: &[f32], rhs: &[f32]) -> f32 {
    lhs.iter()
        .zip(rhs.iter())
        .map(|(left, right)| {
            let diff = *left - *right;
            diff * diff
        })
        .sum::<f32>()
        .sqrt()
}

fn load_dataset_shards(
    db: &DB,
    dataset_id: DatasetId,
) -> DynamicJepaResult<(Vec<DatasetShardRecord>, BTreeMap<String, u32>)> {
    let mut shards = list_dataset_shards(db, 1_000_000, 0)?
        .into_iter()
        .filter(|shard| shard.dataset_id == dataset_id)
        .collect::<Vec<_>>();
    if shards.is_empty() {
        return Err(DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_DATASET_SHARDS.to_string(),
            key: dataset_id.into_bytes().to_vec(),
        });
    }
    shards.sort_by(|left, right| left.split_name.cmp(&right.split_name));
    let domain = shards[0].header.domain_pack_id.clone();
    let version = shards[0].header.domain_pack_version.clone();
    let mut split_counts = BTreeMap::new();
    let mut seen_splits = BTreeSet::new();
    for shard in &shards {
        if shard.header.domain_pack_id != domain || shard.header.domain_pack_version != version {
            return Err(DynamicJepaError::DatasetLeakageDetected {
                message: format!(
                    "dataset {dataset_id} contains shards from multiple domain packs or versions"
                ),
                dataset_id: dataset_id.0,
            });
        }
        if !seen_splits.insert(shard.split_name.clone()) {
            return Err(DynamicJepaError::DatasetLeakageDetected {
                message: format!(
                    "dataset {dataset_id} contains duplicate split {}",
                    shard.split_name
                ),
                dataset_id: dataset_id.0,
            });
        }
        split_counts.insert(shard.split_name.clone(), shard.row_count);
    }
    if !split_counts.contains_key("train") {
        return Err(DynamicJepaError::DatasetLeakageDetected {
            message: format!("dataset {dataset_id} has no train split"),
            dataset_id: dataset_id.0,
        });
    }
    Ok((shards, split_counts))
}

fn objective_ids(shards: &[DatasetShardRecord]) -> DynamicJepaResult<Vec<String>> {
    let mut ids = BTreeSet::new();
    for shard in shards {
        for id in &shard.objective_ids {
            ids.insert(id.clone());
        }
    }
    if ids.is_empty() {
        return Err(DynamicJepaError::validation(
            "train.objective_ids",
            "dataset shards do not reference any objective ids",
            "compile dataset shards from a registered domain pack objective",
        ));
    }
    Ok(ids.into_iter().collect())
}

fn collect_examples(
    db: &DB,
    shards: &[DatasetShardRecord],
) -> DynamicJepaResult<Vec<TrainExample>> {
    let mut examples = Vec::new();
    for shard in shards {
        let row_count = shard.row_count as usize;
        if shard.source_hashes.len() != row_count * 2 {
            return Err(DynamicJepaError::DatasetLeakageDetected {
                message: format!(
                    "shard {} source_hashes length {} does not equal row_count*2",
                    shard.shard_id,
                    shard.source_hashes.len()
                ),
                dataset_id: shard.dataset_id.0,
            });
        }
        for row_idx in 0..row_count {
            let input_panel = get_panel_or_missing(db, shard.input_panel_ids[row_idx])?;
            let target_panel = get_panel_or_missing(db, shard.target_panel_ids[row_idx])?;
            let negative_panel = get_panel_or_missing(db, shard.negative_panel_ids[row_idx])?;
            let action_id = shard.action_ids[row_idx];
            get_action(db, action_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_ACTIONS.to_string(),
                key: action_id.into_bytes().to_vec(),
            })?;
            if shard.source_hashes[row_idx * 2] != input_panel.header.content_hash {
                return Err(DynamicJepaError::DatasetLeakageDetected {
                    message: format!(
                        "input panel source hash drift for shard {} row {} panel {}",
                        shard.shard_id, row_idx, input_panel.panel_id
                    ),
                    dataset_id: shard.dataset_id.0,
                });
            }
            if shard.source_hashes[row_idx * 2 + 1] != target_panel.header.content_hash {
                return Err(DynamicJepaError::DatasetLeakageDetected {
                    message: format!(
                        "target panel source hash drift for shard {} row {} panel {}",
                        shard.shard_id, row_idx, target_panel.panel_id
                    ),
                    dataset_id: shard.dataset_id.0,
                });
            }
            examples.push(TrainExample {
                split_name: shard.split_name.clone(),
                input_panel: flatten_panel(&input_panel)?,
                target_panel: flatten_panel(&target_panel)?,
                action: action_vector(&input_panel)?,
                negative_panel: flatten_panel(&negative_panel)?,
            });
        }
    }
    if examples.is_empty() {
        return Err(DynamicJepaError::DatasetLeakageDetected {
            message: "dataset shards produced zero training examples".to_string(),
            dataset_id: shards[0].dataset_id.0,
        });
    }
    Ok(examples)
}

fn get_panel_or_missing(
    db: &DB,
    id: context_graph_core::dynamicjepa::PanelId,
) -> DynamicJepaResult<LatentPanel> {
    get_latent_panel(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
        cf: CF_DJ_LATENT_PANELS.to_string(),
        key: id.into_bytes().to_vec(),
    })
}

fn flatten_panel(panel: &LatentPanel) -> DynamicJepaResult<Vec<f32>> {
    let mut out = Vec::new();
    for (idx, (slot, vector)) in panel
        .ordered_slots
        .iter()
        .zip(panel.slot_vectors.iter())
        .enumerate()
    {
        if !panel.slot_masks[idx] {
            return Err(DynamicJepaError::PanelShapeMismatch {
                domain_pack_id: panel.header.domain_pack_id.to_string(),
                expected: vec![slot.dim],
                actual: vec![0],
            });
        }
        if vector.len() != slot.dim {
            return Err(DynamicJepaError::PanelShapeMismatch {
                domain_pack_id: panel.header.domain_pack_id.to_string(),
                expected: vec![slot.dim],
                actual: vec![vector.len()],
            });
        }
        out.extend_from_slice(vector);
    }
    if out.is_empty() {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: panel.header.domain_pack_id.to_string(),
            expected: vec![1],
            actual: vec![0],
        });
    }
    Ok(out)
}

fn action_vector(panel: &LatentPanel) -> DynamicJepaResult<Vec<f32>> {
    let mut out = Vec::new();
    for (idx, (slot, vector)) in panel
        .ordered_slots
        .iter()
        .zip(panel.slot_vectors.iter())
        .enumerate()
    {
        if matches!(slot.kind, PanelSlotKind::Action) {
            if !panel.slot_masks[idx] || vector.len() != slot.dim {
                return Err(DynamicJepaError::PanelShapeMismatch {
                    domain_pack_id: panel.header.domain_pack_id.to_string(),
                    expected: vec![slot.dim],
                    actual: vec![vector.len()],
                });
            }
            out.extend_from_slice(vector);
        }
    }
    if out.is_empty() {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: panel.header.domain_pack_id.to_string(),
            expected: vec![1],
            actual: vec![0],
        });
    }
    Ok(out)
}

fn prepare_artifact_root(root: &Path) -> DynamicJepaResult<PathBuf> {
    fs::create_dir_all(root).map_err(|err| DynamicJepaError::Storage {
        operation: "artifact_root.create_dir_all".to_string(),
        cf: "artifact_root".to_string(),
        message: format!("failed to create {}: {err}", root.display()),
        remediation: "use a writable artifact root outside the repository source tree".to_string(),
    })?;
    fs::canonicalize(root).map_err(|err| {
        DynamicJepaError::validation(
            "artifact_root",
            format!("failed to canonicalize {}: {err}", root.display()),
            "use a writable artifact root path",
        )
    })
}

fn write_artifact_files(
    run_dir: &Path,
    config_bytes: &[u8],
    trained: &crate::model::TrainedTinyJepa,
) -> DynamicJepaResult<()> {
    fs::write(run_dir.join("config.json"), config_bytes).map_err(file_write_err)?;
    candle_core::safetensors::save(&trained.tensors, run_dir.join("model.safetensors")).map_err(
        |err| DynamicJepaError::TrainingFailed {
            training_run_id: Uuid::nil(),
            message: format!("failed to write model.safetensors: {err}"),
            remediation: "verify artifact_root is writable and rerun training".to_string(),
        },
    )?;
    let metrics_bytes = serde_json::to_vec(&trained.metrics).map_err(json_err)?;
    fs::write(run_dir.join("metrics.json"), metrics_bytes).map_err(file_write_err)?;
    let evaluation_bytes = serde_json::to_vec(&trained.evaluation_report).map_err(json_err)?;
    fs::write(run_dir.join("evaluation_report.json"), evaluation_bytes).map_err(file_write_err)?;
    let partial_files = compute_artifact_hashes(run_dir)?;
    let manifest = json!({
        "files_before_manifest": partial_files
            .iter()
            .filter(|file| file.relative_path != "manifest.json")
            .map(|file| json!({
                "relative_path": file.relative_path,
                "sha256": hex(&file.sha256),
                "size_bytes": file.size_bytes,
            }))
            .collect::<Vec<_>>()
    });
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(json_err)?;
    fs::write(run_dir.join("manifest.json"), manifest_bytes).map_err(file_write_err)?;
    Ok(())
}

fn json_err(err: serde_json::Error) -> DynamicJepaError {
    DynamicJepaError::TrainingFailed {
        training_run_id: Uuid::nil(),
        message: format!("failed to serialize training artifact JSON: {err}"),
        remediation: "inspect metric/evaluation report values for non-finite data".to_string(),
    }
}

fn file_write_err(err: std::io::Error) -> DynamicJepaError {
    DynamicJepaError::Storage {
        operation: "artifact_file.write".to_string(),
        cf: "artifact_file".to_string(),
        message: err.to_string(),
        remediation: "verify artifact_root is writable and has free disk space".to_string(),
    }
}

fn build_audit(
    operation: impl Into<String>,
    status: AuditStatus,
    input_ids: Vec<String>,
    output_ids: Vec<String>,
    cfs_touched: Vec<String>,
    content_hashes: Vec<[u8; 32]>,
) -> DynamicJepaResult<DjAuditRecord> {
    let record = DjAuditRecord {
        audit_id: Uuid::new_v4(),
        timestamp_unix_nanos: now_unix_ms()? as u64 * 1_000_000,
        operation: operation.into(),
        actor: "dynamicjepa_service".to_string(),
        input_ids,
        output_ids,
        cfs_touched,
        content_hashes,
        status,
        verification_run_id: None,
    };
    record.validate()?;
    Ok(record)
}

fn now_unix_ms() -> DynamicJepaResult<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            DynamicJepaError::validation(
                "system_clock",
                format!("system clock is before Unix epoch: {err}"),
                "fix host clock before writing timestamped DynamicJEPA records",
            )
        })?;
    Ok(duration.as_millis() as i64)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn attach_run_id(err: DynamicJepaError, training_run_id: TrainingRunId) -> DynamicJepaError {
    match err {
        DynamicJepaError::TrainingFailed {
            training_run_id: id,
            message,
            remediation,
        } if id.is_nil() => DynamicJepaError::TrainingFailed {
            training_run_id: training_run_id.0,
            message,
            remediation,
        },
        other => other,
    }
}
