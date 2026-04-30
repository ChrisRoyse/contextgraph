use crate::artifact::{
    compute_artifact_hashes, compute_file_sha256, ensure_clean_run_dir, hex, verify_artifact_files,
    ArtifactHashVerification,
};
use crate::config::TrainConfig;
use crate::model::{train_tiny_jepa, TrainExample};
use context_graph_core::dynamicjepa::{
    ArtifactStatus, DatasetId, DatasetShardRecord, DjRecordHeader, DynamicJepaError,
    DynamicJepaRecord, DynamicJepaResult, LatentPanel, ModelArtifactId, ModelArtifactRecord,
    PanelSlotKind, TrainingRunId, TrainingRunRecord, TrainingRunStatus,
    MODEL_ARTIFACT_RECORD_VERSION, TRAINING_RUN_RECORD_VERSION,
};
use context_graph_storage::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_AUDIT_LOG, CF_DJ_DATASET_SHARDS, CF_DJ_LATENT_PANELS,
    CF_DJ_MODEL_ARTIFACTS, CF_DJ_TRAINING_RUNS,
};
use context_graph_storage::dynamicjepa::{
    get_action, get_domain_pack, get_latent_panel, get_model_artifact, get_training_run,
    list_dataset_shards, put_model_artifact_completion_batch, put_training_run_with_audit_batch,
    AuditStatus, DjAuditRecord,
};
use rocksdb::DB;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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
