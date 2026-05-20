use anyhow::{bail, ensure, Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::loss::cross_entropy;
use context_graph_mejepa::calibration::open_infer_rocksdb;
use context_graph_mejepa::cli::{
    build_slot_preserving_cuda_compiler, write_json_0600,
    NATIVE_ACTIVE_CONSTELLATION_ADAPTER_ATTESTATION_KEY, TRAINED_CHECKPOINT_ATTESTATION_KEY,
};
use context_graph_mejepa::conformal::{non_conformity_score, CalibrationExample};
use context_graph_mejepa::{
    export_trained_predictor_checkpoint, fixture_patch_context,
    load_verified_trained_predictor_checkpoint, materialize_inference_panels, panel_sha,
    predictor_weight_content_sha256, valid_witness_segment, AstDiff, CalibrationStore, DiffHunk,
    FrozenTargetAdapter, Language, MeJepaInferConfig, MeJepaPredictor, MejepaStore,
    NativeActiveConstellationAdapterEvidence, PatchBundle, PredictorCheckpointExportMetadata,
    PredictorConfig, RocksDbInferStore, TaskContext, TaskEnvironment, TaskId, TestId, PANEL_DIM,
};
use context_graph_mejepa_instruments::{
    EOracleInstrument, Instrument, InstrumentSlot, OracleVerdict as InstrumentOracleVerdict, Panel,
    PerTestOutcome, TestOutcome,
};
use context_graph_mejepa_tct::{
    panel_slot_for_embedder, Centroid, ConstellationStore, CorpusProvenance, EmbedderId,
    EntityType, Language as TctLanguage, MutationCategory, OracleOutcome, ShrinkageOrigin,
    TctConstellation, Thresholds,
};
use context_graph_mejepa_train::eval::regression::promote_trained_predictor_checkpoint;
use context_graph_mejepa_train::loss::predict::l_predict;
use context_graph_mejepa_train::optim::build_adamw;
use context_graph_mejepa_train::TrainingConfig;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-127-trained-checkpoint-export-fsv";
const MODEL_ROOT: &str =
    "/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv";
const REPORT_NAME: &str = "trained_checkpoint_export_fsv.json";
const CONSTELLATION_ARTIFACT_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/artifacts";
const AST_CHUNKS_JSONL: &str =
    "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl";
const NATIVE_ACTIVE_ADAPTER_MODEL_ROOT: &str =
    "/zfs/archive/contextgraph/models/task-py-g-152-native-active-constellation-training-fsv";
const NATIVE_ACTIVE_ADAPTER_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-152-native-active-constellation-training-fsv";
const REAL_CORPUS_BATCH_SIZE: usize = 256;
const REAL_CORPUS_OPTIMIZER_STEPS: usize = 256;
const ORACLE_LOSS_WEIGHT: f64 = 4.0;

struct RealCorpusTrainingBatch {
    panel_t0: Tensor,
    panel_t1: Tensor,
    target_panel: Tensor,
    oracle_targets: Tensor,
    corpus_sha256: String,
    summary: Value,
    direct_pack_edge: Value,
}

#[derive(Clone, Debug, serde::Serialize)]
struct LossScalars {
    total: f32,
    panel_predict: f32,
    oracle_ce: f32,
    oracle_loss_weight: f64,
}

impl LossScalars {
    fn is_finite(&self) -> bool {
        self.total.is_finite()
            && self.panel_predict.is_finite()
            && self.oracle_ce.is_finite()
            && self.oracle_loss_weight.is_finite()
    }
}

#[derive(Clone, Debug, serde::Serialize)]
struct OracleTrainingDiagnostics {
    sample_count: usize,
    actual_pass_count: usize,
    predicted_pass_count: usize,
    accuracy: f32,
    score_min: f32,
    score_max: f32,
    score_mean: f32,
    non_degenerate: bool,
}

#[derive(Clone, Debug)]
struct ChunkPair {
    before: Value,
    after: Value,
}

#[derive(Clone, Debug)]
struct NativeAdapterEvidenceReadback {
    evidence: NativeActiveConstellationAdapterEvidence,
    summary: Value,
}

fn main() -> Result<()> {
    let fsv_root = PathBuf::from(FSV_ROOT);
    let model_root = PathBuf::from(MODEL_ROOT);
    remove_if_exists(&fsv_root)?;
    remove_if_exists(&model_root)?;
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&model_root)?;

    let predictor_config = PredictorConfig::default();
    let device = Device::new_cuda(0).context("TASK-PY-G-127 requires aiwonder CUDA device 0")?;
    let predictor = MeJepaPredictor::new(
        predictor_config.clone(),
        FrozenTargetAdapter::empty_for_test(),
        device.clone(),
        1,
    )?;
    let initial_weight_sha256 = predictor_weight_content_sha256(&predictor)?;
    let unchanged_export_edge =
        unchanged_weights_export_edge(&predictor, &model_root, &initial_weight_sha256)?;

    let training_config = TrainingConfig {
        lr: 1e-3,
        batch_size: REAL_CORPUS_BATCH_SIZE,
        epochs: 1,
        warmup_steps: 1,
        max_grad_norm: 5.0,
        full_finetune: true,
        ..TrainingConfig::default()
    };
    let real_batch = real_corpus_training_batch(
        Path::new(CONSTELLATION_ARTIFACT_ROOT),
        Path::new(AST_CHUNKS_JSONL),
        REAL_CORPUS_BATCH_SIZE,
        &device,
    )?;
    let RealCorpusTrainingBatch {
        panel_t0,
        panel_t1,
        target_panel,
        oracle_targets,
        corpus_sha256,
        summary: real_corpus_summary,
        direct_pack_edge,
    } = real_batch;
    let native_adapter = native_active_constellation_adapter_evidence(
        Path::new(NATIVE_ACTIVE_ADAPTER_MODEL_ROOT),
        Path::new(NATIVE_ACTIVE_ADAPTER_FSV_ROOT),
    )?;
    let (_loss_tensor, loss_before) = predictor_training_loss(
        &predictor,
        &panel_t0,
        &panel_t1,
        &target_panel,
        &oracle_targets,
    )?;
    let mut optimizer = build_adamw(
        &training_config,
        REAL_CORPUS_OPTIMIZER_STEPS as u64,
        predictor.trainable_parameters(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )?;
    for _ in 0..REAL_CORPUS_OPTIMIZER_STEPS {
        let (step_loss, _step_scalar) = predictor_training_loss(
            &predictor,
            &panel_t0,
            &panel_t1,
            &target_panel,
            &oracle_targets,
        )?;
        optimizer.adamw.step(&step_loss)?;
        device.synchronize()?;
    }
    let trained_weight_sha256 = predictor_weight_content_sha256(&predictor)?;
    let (_loss_after_tensor, loss_after) = predictor_training_loss(
        &predictor,
        &panel_t0,
        &panel_t1,
        &target_panel,
        &oracle_targets,
    )?;
    let oracle_diagnostics =
        oracle_training_diagnostics(&predictor, &panel_t0, &panel_t1, &oracle_targets)?;
    let weight_delta_observed = trained_weight_sha256 != initial_weight_sha256;
    ensure!(
        weight_delta_observed,
        "optimizer step did not change predictor VarMap hash"
    );

    let training_certificate = json!({
        "schema_version": 1,
        "issue": 441,
        "task_id": "TASK-PY-G-127",
        "optimizer": "AdamW",
        "optimizer_steps": optimizer.adamw.global_step(),
        "loss": {
            "kind": "L_predict(predicted_panel, target_panel) + oracle_cross_entropy(class0=pass,class1=fail)",
            "oracle_loss_weight": ORACLE_LOSS_WEIGHT,
            "before": loss_before,
            "after": loss_after
        },
        "oracle_training_diagnostics": oracle_diagnostics.clone(),
        "real_corpus_batch": real_corpus_summary,
        "initial_weight_sha256": initial_weight_sha256,
        "trained_weight_sha256": trained_weight_sha256,
        "weight_delta_observed": weight_delta_observed,
        "corpus_sha256": corpus_sha256,
        "native_active_constellation_adapter": native_adapter.summary.clone(),
        "training_config_sha256": sha256_json(&training_config)?,
        "predictor_config_sha256": context_graph_mejepa::predictor_config_sha256(&predictor_config)?,
    });
    let training_certificate_sha256 = sha256_json(&training_certificate)?;
    write_json_0600(
        &model_root.join("training_certificate.json"),
        &training_certificate,
    )?;

    let export = promote_trained_predictor_checkpoint(
        &model_root,
        1,
        1.0,
        0.0,
        0.005,
        &predictor,
        PredictorCheckpointExportMetadata {
            payload_step: 1,
            optimizer_steps: optimizer.adamw.global_step() as u64,
            training_mode: "full_finetune_predictor_oracle_supervised".to_string(),
            initial_weight_sha256: initial_weight_sha256.clone(),
            training_certificate_sha256: training_certificate_sha256.clone(),
            native_active_constellation_adapter: Some(native_adapter.evidence.clone()),
            corpus_sha256: corpus_sha256.clone(),
            config_sha256: sha256_json(&training_config)?,
            code_version: option_env!("GIT_HASH")
                .unwrap_or("task-py-g-127-fsv")
                .to_string(),
            created_at_unix_ms: now_unix_ms()?,
        },
    )?
    .context("agreement delta should promote trained predictor checkpoint")?;

    let manifest_readback: Value = serde_json::from_slice(&fs::read(&export.manifest_path)?)?;
    let checkpoint_bytes = fs::read(&export.checkpoint_path)?;
    let checkpoint_sha_readback = sha256_hex(&checkpoint_bytes);
    let mut reloaded = MeJepaPredictor::new(
        predictor_config.clone(),
        FrozenTargetAdapter::empty_for_test(),
        device.clone(),
        1,
    )?;
    let loaded = load_verified_trained_predictor_checkpoint(
        &mut reloaded,
        &export.manifest_path,
        &predictor_config,
    )?;
    let reloaded_weight_sha256 = predictor_weight_content_sha256(&reloaded)?;
    let config_mismatch_edge = predictor_config_mismatch_edge(&fsv_root, &export.manifest_path)?;
    let stub_rejection_edge = stub_checkpoint_edge(&fsv_root)?;
    let runtime = runtime_loaded_checkpoint_edge(&fsv_root, &export.manifest_path)?;

    let all_passed = loss_before.is_finite()
        && loss_after.is_finite()
        && weight_delta_observed
        && export.checkpoint_bytes > 0
        && export.checkpoint_sha256 == checkpoint_sha_readback
        && loaded.trained_weight_sha256 == trained_weight_sha256
        && loaded.training_certificate_sha256 == training_certificate_sha256
        && reloaded_weight_sha256 == trained_weight_sha256
        && runtime["prediction_has_loaded_checkpoint_attestation"] == true
        && runtime["prediction_has_checkpoint_sha_attestation"] == true
        && runtime["prediction_has_native_active_constellation_adapter_attestation"] == true
        && runtime["prediction_has_native_active_constellation_adapter_manifest_sha_attestation"] == true
        && runtime["prediction_has_native_active_constellation_adapter_checkpoint_sha_attestation"] == true
        && runtime["prediction_has_native_active_constellation_adapter_training_certificate_sha_attestation"] == true
        && runtime["prediction_persisted"] == true
        && optimizer.adamw.global_step() as usize == REAL_CORPUS_OPTIMIZER_STEPS
        && oracle_diagnostics.non_degenerate
        && oracle_diagnostics.predicted_pass_count > 0
        && real_corpus_summary["label_diversity_passed"] == true
        && real_corpus_summary["batch_size"] == REAL_CORPUS_BATCH_SIZE
        && native_adapter.summary["adapter_fsv_all_passed"] == true
        && real_corpus_summary["synthetic_seed_features_used"] == false
        && direct_pack_edge["passed"] == true
        && unchanged_export_edge["passed"] == true
        && config_mismatch_edge["passed"] == true
        && stub_rejection_edge["passed"] == true;

    let report = json!({
        "schema_version": 1,
        "task_id": "TASK-PY-G-127",
        "issue": 441,
        "source_of_truth": {
            "fsv_root": FSV_ROOT,
            "model_root": MODEL_ROOT,
            "report_path": fsv_root.join(REPORT_NAME),
            "manifest_path": export.manifest_path,
            "checkpoint_path": export.checkpoint_path
        },
        "training": {
            "optimizer_steps": optimizer.adamw.global_step(),
            "loss_before": loss_before,
            "loss_after": loss_after,
            "initial_weight_sha256": initial_weight_sha256,
            "trained_weight_sha256": trained_weight_sha256,
            "weight_delta_observed": weight_delta_observed,
            "training_certificate_sha256": training_certificate_sha256,
            "corpus_sha256": corpus_sha256,
            "oracle_training_diagnostics": oracle_diagnostics,
            "native_active_constellation_adapter": native_adapter.summary.clone(),
            "real_corpus_batch": real_corpus_summary
        },
        "export": {
            "manifest_sha256": export.manifest_sha256,
            "checkpoint_sha256": export.checkpoint_sha256,
            "checkpoint_sha256_readback": checkpoint_sha_readback,
            "checkpoint_bytes": export.checkpoint_bytes,
            "manifest_readback": manifest_readback
        },
        "load": {
            "loaded_manifest_path": loaded.manifest_path,
            "loaded_checkpoint_path": loaded.checkpoint_path,
            "loaded_payload_step": loaded.payload_step,
            "loaded_optimizer_steps": loaded.optimizer_steps,
            "loaded_training_certificate_sha256": loaded.training_certificate_sha256,
            "reloaded_weight_sha256": reloaded_weight_sha256
        },
        "runtime": runtime,
        "edge_cases": [
            direct_pack_edge,
            unchanged_export_edge,
            config_mismatch_edge,
            stub_rejection_edge
        ],
        "all_passed": all_passed
    });
    let report_path = fsv_root.join(REPORT_NAME);
    write_json_0600(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        readback["all_passed"] == true,
        "TASK-PY-G-127 FSV failed: {}",
        serde_json::to_string_pretty(&readback)?
    );
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn predictor_training_loss(
    predictor: &MeJepaPredictor,
    panel_t0: &Tensor,
    panel_t1: &Tensor,
    target_panel: &Tensor,
    oracle_targets: &Tensor,
) -> Result<(Tensor, LossScalars)> {
    let predicted = predictor.forward(panel_t0, panel_t1)?;
    let target = target_panel.to_dtype(predicted.tensor.dtype())?;
    let panel_loss = l_predict(&predicted.tensor, &target)?.to_dtype(DType::F32)?;
    let logits = predictor.oracle_head().predict_logits(&predicted)?;
    let oracle_loss = cross_entropy(&logits.tensor.to_dtype(DType::F32)?, oracle_targets)?;
    let weighted_oracle_loss = oracle_loss.affine(ORACLE_LOSS_WEIGHT, 0.0)?;
    let total_loss = (&panel_loss + &weighted_oracle_loss)?;
    let scalars = LossScalars {
        total: total_loss.to_scalar::<f32>()?,
        panel_predict: panel_loss.to_scalar::<f32>()?,
        oracle_ce: oracle_loss.to_scalar::<f32>()?,
        oracle_loss_weight: ORACLE_LOSS_WEIGHT,
    };
    Ok((total_loss, scalars))
}

fn oracle_training_diagnostics(
    predictor: &MeJepaPredictor,
    panel_t0: &Tensor,
    panel_t1: &Tensor,
    oracle_targets: &Tensor,
) -> Result<OracleTrainingDiagnostics> {
    let predicted = predictor.forward(panel_t0, panel_t1)?;
    let logits = predictor.oracle_head().predict_logits(&predicted)?;
    let probabilities = predictor.oracle_head().predict_probabilities(&logits)?;
    let dims = probabilities.tensor.dims();
    ensure!(
        dims.len() == 2 && dims[0] > 0 && dims[1] >= 2,
        "TASK-PY-G-127_ORACLE_PROBABILITY_SHAPE_INVALID: {:?}",
        dims
    );
    let batch = dims[0];
    let classes = dims[1];
    let scores = probabilities
        .tensor
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let targets = oracle_targets
        .to_dtype(DType::U32)?
        .flatten_all()?
        .to_vec1::<u32>()?;
    ensure!(
        targets.len() == batch,
        "TASK-PY-G-127_ORACLE_TARGET_COUNT_MISMATCH"
    );
    let mut actual_pass_count = 0usize;
    let mut predicted_pass_count = 0usize;
    let mut correct = 0usize;
    let mut pass_scores = Vec::with_capacity(batch);
    for row_idx in 0..batch {
        let pass_score = scores[row_idx * classes];
        ensure!(
            pass_score.is_finite() && (0.0..=1.0).contains(&pass_score),
            "TASK-PY-G-127_ORACLE_SCORE_INVALID: {pass_score}"
        );
        let actual_pass = targets[row_idx] == 0;
        let predicted_pass = pass_score >= 0.5;
        if actual_pass {
            actual_pass_count += 1;
        }
        if predicted_pass {
            predicted_pass_count += 1;
        }
        if actual_pass == predicted_pass {
            correct += 1;
        }
        pass_scores.push(pass_score);
    }
    let score_min = pass_scores.iter().copied().fold(f32::INFINITY, f32::min);
    let score_max = pass_scores
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let score_mean = pass_scores.iter().sum::<f32>() / pass_scores.len() as f32;
    Ok(OracleTrainingDiagnostics {
        sample_count: batch,
        actual_pass_count,
        predicted_pass_count,
        accuracy: correct as f32 / batch as f32,
        score_min,
        score_max,
        score_mean,
        non_degenerate: predicted_pass_count > 0
            && predicted_pass_count < batch
            && (score_max - score_min) > 0.05,
    })
}

fn real_corpus_training_batch(
    artifact_root: &Path,
    ast_chunks_jsonl: &Path,
    batch_size: usize,
    device: &Device,
) -> Result<RealCorpusTrainingBatch> {
    ensure!(batch_size >= 2, "real corpus batch must contain >=2 rows");
    let training_rows_path = artifact_root.join("constellation_training_rows.jsonl");
    let slot_vectors_path = artifact_root.join("constellation_slot_vectors.jsonl");
    let certificate_path = artifact_root.join("constellation_training_certificate.json");
    for path in [
        &training_rows_path,
        &slot_vectors_path,
        &certificate_path,
        ast_chunks_jsonl,
    ] {
        ensure!(
            path.is_file(),
            "TASK-PY-G-127_REAL_CORPUS_ARTIFACT_MISSING: {}",
            path.display()
        );
    }

    let certificate: Value = serde_json::from_slice(&fs::read(&certificate_path)?)?;
    ensure!(
        certificate["passes"] == true
            && certificate["objective_policy"]["flat_vector_concat_used"] == false,
        "TASK-PY-G-127_REAL_CORPUS_CERTIFICATE_INVALID: {}",
        certificate_path.display()
    );

    let all_rows = read_jsonl_values(&training_rows_path)?;
    let candidate_keys = all_rows
        .iter()
        .filter(|row| row["partition"] == "train")
        .map(row_key)
        .collect::<Result<BTreeSet<_>>>()?;
    let candidate_pairs = read_candidate_chunk_pairs(ast_chunks_jsonl, &candidate_keys)?;
    let selected_rows = select_real_corpus_rows(&all_rows, &candidate_pairs, batch_size)?;
    let selected_keys = selected_rows
        .iter()
        .map(row_key)
        .collect::<Result<BTreeSet<_>>>()?;
    let slot_readback = read_selected_slot_vectors(&slot_vectors_path, &selected_keys)?;

    let mut panel_t0_values = Vec::with_capacity(batch_size * PANEL_DIM);
    let mut panel_t1_values = Vec::with_capacity(batch_size * PANEL_DIM);
    let mut target_values = Vec::with_capacity(batch_size * PANEL_DIM);
    let mut oracle_target_values = Vec::with_capacity(batch_size);
    let mut selected_summary = Vec::with_capacity(batch_size);
    let mut label_counts: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, row) in selected_rows.iter().enumerate() {
        let key = row_key(row)?;
        let chunk_pair = candidate_pairs
            .get(&key)
            .with_context(|| format!("TASK-PY-G-127_AST_CHUNK_PAIR_MISSING: {key}"))?;
        let oracle_all_passed = row["labels"]["oracle_all_passed"]
            .as_bool()
            .with_context(|| format!("TASK-PY-G-127_ORACLE_LABEL_MISSING: {key}"))?;
        *label_counts
            .entry(if oracle_all_passed { "pass" } else { "fail" }.to_string())
            .or_default() += 1;

        let (patch, context) = patch_context_from_real_row(row, chunk_pair, idx)?;
        let (panel_t0, panel_t1, panel_t2) = materialize_inference_panels(&patch, &context)?;
        let target_panel = panel_with_oracle_label(&panel_t2, oracle_all_passed)?;
        panel_t0_values.extend_from_slice(panel_t0.data());
        panel_t1_values.extend_from_slice(panel_t1.data());
        target_values.extend_from_slice(target_panel.data());
        oracle_target_values.push(if oracle_all_passed { 0u32 } else { 1u32 });
        selected_summary.push(json!({
            "row_key": key,
            "partition": row["partition"],
            "cell_key": row["cell_key"],
            "oracle_all_passed": oracle_all_passed,
            "present_embedder_count": row["slot_policy"]["present_embedder_count"],
            "slot_vector_readback": slot_readback.row_summaries.get(&row_key(row)?),
            "source": {
                "task_instance_id": chunk_pair.after["task_instance_id"],
                "before_workspace_state": chunk_pair.before["workspace_state"],
                "after_workspace_state": chunk_pair.after["workspace_state"],
                "relative_path": chunk_pair.after["relative_path"],
                "before_chunk_id": chunk_pair.before["chunk_id"],
                "after_chunk_id": chunk_pair.after["chunk_id"],
                "before_source_sha256": chunk_pair.before["source_sha256"],
                "after_source_sha256": chunk_pair.after["source_sha256"],
            },
            "panel_hashes": {
                "t0_sha256": sha256_f32s(panel_t0.data()),
                "t1_sha256": sha256_f32s(panel_t1.data()),
                "target_sha256": sha256_f32s(target_panel.data()),
                "target_oracle_slot_sha256": sha256_f32s(target_panel.slot(InstrumentSlot::EOracle)),
            }
        }));
    }

    ensure!(
        label_counts.contains_key("pass") && label_counts.contains_key("fail"),
        "TASK-PY-G-127_REAL_CORPUS_LABELS_DEGENERATE: {:?}",
        label_counts
    );
    ensure!(
        panel_t0_values.len() == batch_size * PANEL_DIM
            && panel_t1_values.len() == batch_size * PANEL_DIM
            && target_values.len() == batch_size * PANEL_DIM,
        "TASK-PY-G-127_PANEL_BATCH_DIM_MISMATCH"
    );

    let artifact_hashes = json!({
        "constellation_training_rows": sha256_file(&training_rows_path)?,
        "constellation_slot_vectors": sha256_file(&slot_vectors_path)?,
        "constellation_training_certificate": sha256_file(&certificate_path)?,
        "python_ast_chunks": sha256_file(ast_chunks_jsonl)?,
    });
    let direct_pack_edge = active_embedder_direct_pack_edge(&slot_readback)?;
    let summary = json!({
        "schema_version": 1,
        "source": {
            "artifact_root": artifact_root,
            "training_rows": training_rows_path,
            "slot_vectors": slot_vectors_path,
            "training_certificate": certificate_path,
            "ast_chunks_jsonl": ast_chunks_jsonl,
        },
        "artifact_hashes": artifact_hashes,
        "batch_size": batch_size,
        "panel_dim": PANEL_DIM,
        "optimizer_steps": REAL_CORPUS_OPTIMIZER_STEPS,
        "selected_rows": selected_summary,
        "label_counts": label_counts,
        "label_diversity_passed": true,
        "slot_vector_rows_read": slot_readback.slot_vector_rows_read,
        "slot_vector_hashes_verified": slot_readback.vector_hashes_verified,
        "max_active_embedder_total_dim": slot_readback.max_total_dim,
        "synthetic_seed_features_used": false,
        "flat_vector_concat_used": false,
        "candidate_chunk_pairs_with_real_source_change": candidate_pairs.len(),
        "chunk_pair_policy": "after row key from #396 joined to base chunk by task/path/chunk_kind/node_kind/name/qualname/line span; no-op same-source rows rejected",
        "panels_materialized_from": "real #396 row keys + base-vs-after AST chunk source_text + frozen instrument encoders + oracle_all_passed target slot",
        "active_embedder_vectors_directly_packed_into_5120_panel": false,
    });
    let corpus_sha256 = sha256_json(&summary)?;
    Ok(RealCorpusTrainingBatch {
        panel_t0: Tensor::from_slice(&panel_t0_values, (batch_size, PANEL_DIM), device)?
            .to_dtype(DType::BF16)?,
        panel_t1: Tensor::from_slice(&panel_t1_values, (batch_size, PANEL_DIM), device)?
            .to_dtype(DType::BF16)?,
        target_panel: Tensor::from_slice(&target_values, (batch_size, PANEL_DIM), device)?
            .to_dtype(DType::BF16)?,
        oracle_targets: Tensor::from_slice(&oracle_target_values, batch_size, device)?,
        corpus_sha256,
        summary,
        direct_pack_edge,
    })
}

#[derive(Debug)]
struct SlotVectorReadback {
    slot_vector_rows_read: usize,
    vector_hashes_verified: usize,
    max_total_dim: usize,
    row_summaries: BTreeMap<String, Value>,
}

fn read_jsonl_values(path: &Path) -> Result<Vec<Value>> {
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut rows = Vec::new();
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(
            serde_json::from_str(&line)
                .with_context(|| format!("parse {} line {}", path.display(), idx + 1))?,
        );
    }
    ensure!(!rows.is_empty(), "JSONL is empty: {}", path.display());
    Ok(rows)
}

fn select_real_corpus_rows(
    rows: &[Value],
    chunk_pairs: &BTreeMap<String, ChunkPair>,
    batch_size: usize,
) -> Result<Vec<Value>> {
    let eligible = rows
        .iter()
        .filter(|row| row["partition"] == "train")
        .filter(|row| row_key(row).is_ok_and(|key| chunk_pairs.contains_key(&key)))
        .collect::<Vec<_>>();
    ensure!(
        eligible.len() >= batch_size,
        "TASK-PY-G-127_REAL_SOURCE_CHANGE_ROWS_TOO_SMALL: eligible {} expected {}",
        eligible.len(),
        batch_size
    );
    let pass_rows = eligible
        .iter()
        .copied()
        .filter(|row| row["labels"]["oracle_all_passed"].as_bool() == Some(true))
        .collect::<Vec<_>>();
    let fail_rows = eligible
        .iter()
        .copied()
        .filter(|row| row["labels"]["oracle_all_passed"].as_bool() == Some(false))
        .collect::<Vec<_>>();
    ensure!(
        !pass_rows.is_empty() && !fail_rows.is_empty(),
        "TASK-PY-G-127_REAL_CORPUS_LABELS_DEGENERATE: pass={} fail={}",
        pass_rows.len(),
        fail_rows.len()
    );
    let mut selected = Vec::with_capacity(batch_size);
    let mut selected_keys = BTreeSet::new();
    if let Some(max_coverage) = eligible
        .iter()
        .copied()
        .max_by_key(|row| row["slots"].as_array().map_or(0, Vec::len))
    {
        push_selected_row(max_coverage, &mut selected_keys, &mut selected)?;
    }
    let max_label_len = pass_rows.len().max(fail_rows.len());
    for idx in 0..max_label_len {
        if selected.len() >= batch_size {
            break;
        }
        if let Some(row) = fail_rows.get(idx) {
            push_selected_row(row, &mut selected_keys, &mut selected)?;
        }
        if selected.len() >= batch_size {
            break;
        }
        if let Some(row) = pass_rows.get(idx) {
            push_selected_row(row, &mut selected_keys, &mut selected)?;
        }
    }
    for row in eligible {
        if selected.len() >= batch_size {
            break;
        }
        push_selected_row(row, &mut selected_keys, &mut selected)?;
    }
    ensure!(
        selected.len() == batch_size,
        "TASK-PY-G-127_REAL_CORPUS_BATCH_TOO_SMALL: selected {} expected {}",
        selected.len(),
        batch_size
    );
    let labels = selected
        .iter()
        .map(|row| row["labels"]["oracle_all_passed"].as_bool())
        .collect::<BTreeSet<_>>();
    ensure!(
        labels.contains(&Some(true)) && labels.contains(&Some(false)),
        "TASK-PY-G-127_REAL_CORPUS_LABELS_DEGENERATE: {:?}",
        labels
    );
    Ok(selected)
}

fn push_selected_row(
    row: &Value,
    selected_keys: &mut BTreeSet<String>,
    selected: &mut Vec<Value>,
) -> Result<()> {
    let key = row_key(row)?;
    if selected_keys.insert(key) {
        selected.push(row.clone());
    }
    Ok(())
}

fn read_candidate_chunk_pairs(
    path: &Path,
    candidate_keys: &BTreeSet<String>,
) -> Result<BTreeMap<String, ChunkPair>> {
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut after_chunks = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(&line)
            .with_context(|| format!("parse {} line {}", path.display(), idx + 1))?;
        let key = chunk_row_key(&row)?;
        if candidate_keys.contains(&key) {
            identities.insert(chunk_pair_identity(&row)?);
            after_chunks.insert(key, row);
            if after_chunks.len() == candidate_keys.len() {
                break;
            }
        }
    }
    ensure!(
        !after_chunks.is_empty(),
        "TASK-PY-G-127_AST_CHUNK_READBACK_EMPTY"
    );

    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut before_by_identity = BTreeMap::new();
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(&line)
            .with_context(|| format!("parse {} line {}", path.display(), idx + 1))?;
        if row["workspace_state"] != "base" {
            continue;
        }
        let identity = chunk_pair_identity(&row)?;
        if identities.contains(&identity) {
            before_by_identity.entry(identity).or_insert(row);
        }
    }

    let mut pairs = BTreeMap::new();
    for (key, after) in after_chunks {
        if after["workspace_state"] == "base" {
            continue;
        }
        let identity = chunk_pair_identity(&after)?;
        let Some(before) = before_by_identity.get(&identity) else {
            continue;
        };
        let before_source = value_string(before, "source_text")?;
        let after_source = value_string(&after, "source_text")?;
        if before_source == after_source {
            continue;
        }
        pairs.insert(
            key,
            ChunkPair {
                before: before.clone(),
                after,
            },
        );
    }
    ensure!(
        !pairs.is_empty(),
        "TASK-PY-G-127_REAL_SOURCE_CHANGE_PAIR_READBACK_EMPTY"
    );
    Ok(pairs)
}

fn read_selected_slot_vectors(
    path: &Path,
    selected_keys: &BTreeSet<String>,
) -> Result<SlotVectorReadback> {
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut dims_by_row: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
    let mut vector_hashes_verified = 0usize;
    let mut slot_vector_rows_read = 0usize;
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(&line)
            .with_context(|| format!("parse {} line {}", path.display(), idx + 1))?;
        let key = value_string(&row, "row_key")?;
        if !selected_keys.contains(&key) {
            continue;
        }
        let embedder_id = value_string(&row, "embedder_id")?;
        let dim = row["dim"]
            .as_u64()
            .with_context(|| format!("slot vector missing dim for {key}:{embedder_id}"))?
            as usize;
        let vector_values = row["vector"]
            .as_array()
            .with_context(|| format!("slot vector missing vector for {key}:{embedder_id}"))?;
        ensure!(
            vector_values.len() == dim,
            "TASK-PY-G-127_SLOT_VECTOR_DIM_MISMATCH: {key}:{embedder_id} len={} dim={dim}",
            vector_values.len()
        );
        let mut vector = Vec::with_capacity(dim);
        for value in vector_values {
            let value = value
                .as_f64()
                .with_context(|| format!("non-number slot vector value for {key}:{embedder_id}"))?
                as f32;
            ensure!(
                value.is_finite(),
                "TASK-PY-G-127_SLOT_VECTOR_NONFINITE: {key}:{embedder_id}"
            );
            vector.push(value);
        }
        let expected_hash = value_string(&row, "vector_sha256")?;
        let expected_hash = expected_hash
            .strip_prefix("sha256:")
            .unwrap_or(&expected_hash)
            .to_string();
        let observed_hash = sha256_f32s(&vector);
        ensure!(
            observed_hash == expected_hash,
            "TASK-PY-G-127_SLOT_VECTOR_HASH_MISMATCH: {key}:{embedder_id}"
        );
        vector_hashes_verified += 1;
        slot_vector_rows_read += 1;
        dims_by_row.entry(key).or_default().push((embedder_id, dim));
    }
    let mut row_summaries = BTreeMap::new();
    let mut max_total_dim = 0usize;
    for key in selected_keys {
        let mut dims = dims_by_row
            .remove(key)
            .with_context(|| format!("TASK-PY-G-127_SLOT_VECTOR_READBACK_MISSING: {key}"))?;
        dims.sort();
        let total_dim = dims.iter().map(|(_, dim)| *dim).sum::<usize>();
        max_total_dim = max_total_dim.max(total_dim);
        row_summaries.insert(
            key.clone(),
            json!({
                "embedder_count": dims.len(),
                "total_active_embedder_dim": total_dim,
                "dims": dims.iter().map(|(embedder_id, dim)| json!({"embedder_id": embedder_id, "dim": dim})).collect::<Vec<_>>(),
            }),
        );
    }
    Ok(SlotVectorReadback {
        slot_vector_rows_read,
        vector_hashes_verified,
        max_total_dim,
        row_summaries,
    })
}

fn active_embedder_direct_pack_edge(readback: &SlotVectorReadback) -> Result<Value> {
    let max_total_dim = readback.max_total_dim;
    Ok(json!({
        "name": "active_embedder_vectors_are_not_directly_packed_into_5120_panel",
        "before": {
            "panel_dim": PANEL_DIM,
            "max_total_active_embedder_dim_in_batch": max_total_dim,
            "row_summaries": &readback.row_summaries,
        },
        "after": {
            "direct_pack_used": false,
            "training_panel_source": "frozen instrument encoders over real #396 AST rows; oracle target slot from oracle_all_passed",
        },
        "passed": max_total_dim != PANEL_DIM && max_total_dim > PANEL_DIM && readback.vector_hashes_verified > 0,
    }))
}

fn patch_context_from_real_row(
    row: &Value,
    chunk_pair: &ChunkPair,
    idx: usize,
) -> Result<(PatchBundle, TaskContext)> {
    let before_source = value_string(&chunk_pair.before, "source_text")?;
    let after_source = value_string(&chunk_pair.after, "source_text")?;
    ensure!(
        !before_source.trim().is_empty() && !after_source.trim().is_empty(),
        "TASK-PY-G-127_EMPTY_AST_CHUNK_SOURCE_TEXT_PAIR: {}",
        row_key(row)?
    );
    ensure!(
        before_source != after_source,
        "TASK-PY-G-127_NOOP_AST_CHUNK_PAIR: {}",
        row_key(row)?
    );
    let relative_path = PathBuf::from(value_string(&chunk_pair.after, "relative_path")?);
    let pre_sha = sha256_array(before_source.as_bytes());
    let post_sha = sha256_array(after_source.as_bytes());
    let patch_sha = sha256_array(row_key(row)?.as_bytes());
    let oracle_all_passed = row["labels"]["oracle_all_passed"]
        .as_bool()
        .context("oracle_all_passed")?;
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: relative_path.clone(),
                pre_sha,
                post_sha,
                before: before_source,
                after: after_source,
            }],
        },
        valid_witness_segment(),
        format!(
            "TASK-PY-G-127 real corpus checkpoint row {} oracle_all_passed={}",
            value_string(&chunk_pair.after, "task_instance_id")?,
            oracle_all_passed
        ),
        patch_sha,
    )?;
    let mut session_id = [7u8; 16];
    session_id[0] = (idx as u8).saturating_add(1);
    let context = TaskContext {
        task_id: TaskId(format!(
            "task-py-g-127-{}-{}",
            idx,
            value_string(&chunk_pair.after, "task_instance_id")?
        )),
        session_id,
        language: Language::Python,
        problem_statement: format!(
            "Predict observed SWE-bench oracle consequence for {} {} {}.",
            value_string(&chunk_pair.after, "task_instance_id")?,
            value_string(&chunk_pair.after, "mutation_category")?,
            relative_path.display()
        ),
        tests: vec![TestId(format!(
            "oracle_label::{}::{}",
            value_string(&chunk_pair.after, "task_instance_id")?,
            value_string(&chunk_pair.after, "mutation_category")?
        ))],
        environment: TaskEnvironment {
            repo_root: PathBuf::from("/zfs/hot/contextgraph/task-py-g-127-real-corpus-panel"),
            python_version: Some("3.11".to_string()),
            os: "linux".to_string(),
        },
        claim_graph: None,
        skill_citations: vec![],
    };
    Ok((patch, context))
}

fn panel_with_oracle_label(panel: &Panel, oracle_all_passed: bool) -> Result<Panel> {
    let outcome = if oracle_all_passed {
        TestOutcome::Pass
    } else {
        TestOutcome::Fail
    };
    let verdict = InstrumentOracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: "task_py_g_127_real_corpus_oracle_label".to_string(),
            outcome,
            runtime_ms: -1,
        }],
        exception: None,
        evidence_unavailable: false,
    };
    let oracle_vector = EOracleInstrument.encode(&verdict)?;
    let (offset, dim) = InstrumentSlot::EOracle.extent();
    ensure!(
        oracle_vector.len() == dim,
        "TASK-PY-G-127_ORACLE_VECTOR_DIM_MISMATCH"
    );
    let mut data = panel.data().to_vec();
    data[offset..offset + dim].copy_from_slice(&oracle_vector);
    Ok(Panel::try_new(data, panel.filled_mask())?)
}

fn row_key(row: &Value) -> Result<String> {
    value_string(row, "row_key")
}

fn chunk_row_key(chunk: &Value) -> Result<String> {
    Ok(format!(
        "{}|{}|{}|{}",
        value_string(chunk, "task_instance_id")?,
        value_string(chunk, "workspace_state")?,
        value_string(chunk, "chunk_id")?,
        value_string(chunk, "source_sha256")?
    ))
}

fn chunk_pair_identity(chunk: &Value) -> Result<String> {
    Ok(format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        value_string(chunk, "task_instance_id")?,
        value_string(chunk, "relative_path")?,
        value_string(chunk, "chunk_kind")?,
        value_string(chunk, "node_kind")?,
        chunk["name"].as_str().unwrap_or(""),
        chunk["qualname"].as_str().unwrap_or(""),
        chunk["line_start"].as_u64().context("missing line_start")?,
        chunk["line_end"].as_u64().context("missing line_end")?,
    ))
}

fn value_string(value: &Value, field: &str) -> Result<String> {
    value[field]
        .as_str()
        .map(ToString::to_string)
        .with_context(|| format!("missing string field {field}"))
}

fn unchanged_weights_export_edge(
    predictor: &MeJepaPredictor,
    model_root: &Path,
    initial_weight_sha256: &str,
) -> Result<Value> {
    let edge_dir = model_root.join("unchanged-weights-edge");
    let err = export_trained_predictor_checkpoint(
        predictor,
        &edge_dir,
        predictor.config(),
        PredictorCheckpointExportMetadata {
            payload_step: 1,
            optimizer_steps: 1,
            training_mode: "unchanged_edge".to_string(),
            initial_weight_sha256: initial_weight_sha256.to_string(),
            training_certificate_sha256: "a".repeat(64),
            native_active_constellation_adapter: None,
            corpus_sha256: "b".repeat(64),
            config_sha256: "c".repeat(64),
            code_version: "task-py-g-127-unchanged-edge".to_string(),
            created_at_unix_ms: now_unix_ms()?,
        },
    )
    .expect_err("unchanged predictor weights must not export as trained");
    Ok(json!({
        "name": "unchanged_weights_export_rejected",
        "after": {
            "error_code": err.code(),
            "error": err.to_string(),
            "checkpoint_exists": edge_dir.join("best.safetensors").exists()
        },
        "passed": err.code() == "MEJEPA_PRED_CONFIG_INVALID"
            && err.to_string().contains("equals initial_weight_sha256")
            && !edge_dir.join("best.safetensors").exists()
    }))
}

fn predictor_config_mismatch_edge(fsv_root: &Path, manifest_path: &Path) -> Result<Value> {
    let edge_root = fsv_root.join("config-mismatch-edge");
    fs::create_dir_all(&edge_root)?;
    let mut manifest: Value = serde_json::from_slice(&fs::read(manifest_path)?)?;
    manifest["predictor_config_sha256"] = json!("9".repeat(64));
    let bad_manifest_path = manifest_path
        .parent()
        .context("manifest parent")?
        .join("bad-config.manifest.json");
    write_json_0600(&bad_manifest_path, &manifest)?;
    let db = open_infer_rocksdb(edge_root.join("infer.rocksdb"))?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    seed_calibration(&calibration)?;
    seed_tct_constellation(db.clone())?;
    let store = Arc::new(RocksDbInferStore::new(db));
    let err = match build_slot_preserving_cuda_compiler(
        edge_root.join("repo"),
        store,
        calibration,
        MeJepaInferConfig {
            pause_state_path: None,
            trained_checkpoint_manifest_path: Some(bad_manifest_path.clone()),
            ..MeJepaInferConfig::default()
        },
    ) {
        Ok(_) => bail!("predictor config mismatch unexpectedly built a compiler"),
        Err(err) => err,
    };
    Ok(json!({
        "name": "predictor_config_mismatch_rejected",
        "before": {"manifest_path": bad_manifest_path},
        "after": {"error_code": err.code(), "error": err.to_string()},
        "passed": err.code() == "MEJEPA_INFER_PREDICTOR"
            && err.to_string().contains("predictor_config_sha256 mismatch")
    }))
}

fn stub_checkpoint_edge(fsv_root: &Path) -> Result<Value> {
    let edge_root = fsv_root.join("stub-checkpoint-edge");
    let model_root = edge_root.join("models");
    fs::create_dir_all(&model_root)?;
    let db = open_infer_rocksdb(edge_root.join("infer.rocksdb"))?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    seed_calibration(&calibration)?;
    seed_tct_constellation(db.clone())?;
    let store = Arc::new(RocksDbInferStore::new(db));
    let checkpoint_path = model_root.join("best.safetensors");
    let checkpoint_bytes = b"MEJEPA_BEST_STUB_V1\nstep=1\nagreement=0.990000\n";
    fs::write(&checkpoint_path, checkpoint_bytes)?;
    let manifest_path = model_root.join("best.manifest.json");
    let manifest = json!({
        "schema_version": 1,
        "checkpoint_file": "best.safetensors",
        "checkpoint_sha256": sha256_hex(checkpoint_bytes),
        "checkpoint_bytes": checkpoint_bytes.len(),
        "architecture_sha256": "0".repeat(64),
        "predictor_config_sha256": "1".repeat(64),
        "payload_step": 1,
        "optimizer_steps": 1,
        "training_mode": "full_finetune",
        "training_status": "trained",
        "initial_weight_sha256": "4".repeat(64),
        "trained_weight_sha256": "5".repeat(64),
        "training_certificate_sha256": "6".repeat(64),
        "corpus_sha256": "2".repeat(64),
        "config_sha256": "3".repeat(64),
        "code_version": "task-py-g-127-stub-edge",
        "created_at_unix_ms": 1u64
    });
    write_json_0600(&manifest_path, &manifest)?;
    let err = match build_slot_preserving_cuda_compiler(
        edge_root.join("repo"),
        store,
        calibration,
        MeJepaInferConfig {
            pause_state_path: None,
            trained_checkpoint_manifest_path: Some(manifest_path.clone()),
            ..MeJepaInferConfig::default()
        },
    ) {
        Ok(_) => bail!("stub checkpoint unexpectedly built a compiler"),
        Err(err) => err,
    };
    Ok(json!({
        "name": "stub_best_safetensors_rejected",
        "before": {"manifest_path": manifest_path, "checkpoint_path": checkpoint_path},
        "after": {"error_code": err.code(), "error": err.to_string()},
        "passed": err.code() == "MEJEPA_INFER_PREDICTOR"
            && err.to_string().contains("MEJEPA_BEST_STUB_V1")
    }))
}

fn runtime_loaded_checkpoint_edge(fsv_root: &Path, manifest_path: &Path) -> Result<Value> {
    let edge_root = fsv_root.join("runtime-loaded-edge");
    fs::create_dir_all(&edge_root)?;
    let repo_root = edge_root.join("repo");
    let db_path = edge_root.join("infer.rocksdb");
    let db = open_infer_rocksdb(&db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    seed_calibration(&calibration)?;
    seed_tct_constellation(db.clone())?;
    let store = Arc::new(RocksDbInferStore::new(db));
    let compiler = build_slot_preserving_cuda_compiler(
        repo_root.clone(),
        Arc::clone(&store),
        calibration,
        MeJepaInferConfig {
            outcome_set_max: 1,
            require_ood_calibrator: false,
            pause_state_path: None,
            trained_checkpoint_manifest_path: Some(manifest_path.to_path_buf()),
            ..MeJepaInferConfig::default()
        },
    )?;
    let (patch, context) = fixture_patch_context(&repo_root, "trained-checkpoint")?;
    let (_panel_t0, _panel_t1, _panel_t2) = materialize_inference_panels(&patch, &context)?;
    let before_rows =
        MejepaStore::read_live_predictions(store.as_ref(), context.session_id, 100)?.len();
    let (prediction, predicted_panel) = compiler.compile_with_panel(&patch, &context)?;
    store.write_live_prediction(&prediction)?;
    let readback = MejepaStore::read_live_predictions(store.as_ref(), context.session_id, 100)?;
    let prediction_persisted = readback
        .iter()
        .any(|row| row.prediction_id == prediction.prediction_id);
    let manifest: Value = serde_json::from_slice(&fs::read(manifest_path)?)?;
    let checkpoint_sha = manifest["checkpoint_sha256"]
        .as_str()
        .context("manifest checkpoint_sha256")?;
    let training_cert_sha = manifest["training_certificate_sha256"]
        .as_str()
        .context("manifest training_certificate_sha256")?;
    let adapter = manifest["native_active_constellation_adapter"]
        .as_object()
        .context("manifest native_active_constellation_adapter")?;
    let adapter_manifest_sha = adapter
        .get("manifest_sha256")
        .and_then(Value::as_str)
        .context("manifest native adapter manifest_sha256")?;
    let adapter_checkpoint_sha = adapter
        .get("checkpoint_sha256")
        .and_then(Value::as_str)
        .context("manifest native adapter checkpoint_sha256")?;
    let adapter_training_cert_sha = adapter
        .get("training_certificate_sha256")
        .and_then(Value::as_str)
        .context("manifest native adapter training_certificate_sha256")?;
    Ok(json!({
        "name": "runtime_compiler_loads_manifest_backed_trained_checkpoint",
        "source_of_truth": {
            "db_path": db_path,
            "prediction_cf": "CF_MEJEPA_LIVE_PREDICTIONS"
        },
        "before_live_prediction_rows": before_rows,
        "after_live_prediction_rows": readback.len(),
        "prediction_persisted": prediction_persisted,
        "source_panel_sha_matches_predicted_panel": prediction.source_panel_sha == panel_sha(&predicted_panel),
        "prediction_has_loaded_checkpoint_attestation": prediction
            .granger_attestations
            .get(TRAINED_CHECKPOINT_ATTESTATION_KEY) == Some(&1.0),
        "prediction_has_checkpoint_sha_attestation": prediction
            .granger_attestations
            .contains_key(&format!("checkpoint_sha256:{checkpoint_sha}")),
        "prediction_has_training_certificate_attestation": prediction
            .granger_attestations
            .contains_key(&format!("checkpoint_training_certificate_sha256:{training_cert_sha}")),
        "prediction_has_native_active_constellation_adapter_attestation": prediction
            .granger_attestations
            .get(NATIVE_ACTIVE_CONSTELLATION_ADAPTER_ATTESTATION_KEY) == Some(&1.0),
        "prediction_has_native_active_constellation_adapter_manifest_sha_attestation": prediction
            .granger_attestations
            .contains_key(&format!(
                "native_active_constellation_adapter_manifest_sha256:{adapter_manifest_sha}"
            )),
        "prediction_has_native_active_constellation_adapter_checkpoint_sha_attestation": prediction
            .granger_attestations
            .contains_key(&format!(
                "native_active_constellation_adapter_checkpoint_sha256:{adapter_checkpoint_sha}"
            )),
        "prediction_has_native_active_constellation_adapter_training_certificate_sha_attestation": prediction
            .granger_attestations
            .contains_key(&format!(
                "native_active_constellation_adapter_training_certificate_sha256:{adapter_training_cert_sha}"
            )),
        "predicted_oracle_pass": prediction.predicted_oracle_pass
    }))
}

fn native_active_constellation_adapter_evidence(
    model_root: &Path,
    fsv_root: &Path,
) -> Result<NativeAdapterEvidenceReadback> {
    let manifest_path = model_root.join("native_active_constellation_adapter.manifest.json");
    let checkpoint_path = model_root.join("native_active_constellation_adapter.safetensors");
    let certificate_path = model_root.join("native_training_certificate.json");
    let report_path = fsv_root.join("native_active_constellation_training_fsv.json");
    for path in [
        &manifest_path,
        &checkpoint_path,
        &certificate_path,
        &report_path,
    ] {
        ensure!(
            path.is_file(),
            "TASK-PY-G-127_NATIVE_ACTIVE_ADAPTER_EVIDENCE_MISSING: {}",
            path.display()
        );
    }

    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let certificate: Value = serde_json::from_slice(&fs::read(&certificate_path)?)?;
    let report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    let manifest_sha256 = sha256_file(&manifest_path)?;
    let checkpoint_sha256 = sha256_file(&checkpoint_path)?;
    let training_certificate_sha256 = sha256_file(&certificate_path)?;
    ensure!(
        report["all_passed"] == true,
        "TASK-PY-G-127_NATIVE_ACTIVE_ADAPTER_FSV_NOT_PASSED: {}",
        report_path.display()
    );
    ensure!(
        manifest["model_kind"] == "native_active_embedder_constellation_adapter"
            && certificate["model_kind"] == "native_active_embedder_constellation_adapter",
        "TASK-PY-G-127_NATIVE_ACTIVE_ADAPTER_KIND_MISMATCH"
    );
    ensure!(
        manifest["flat_vector_concat_used"] == false
            && manifest["active_vectors_directly_packed_into_5120_panel"] == false
            && certificate["flat_vector_concat_used"] == false
            && certificate["synthetic_seed_features_used"] == false,
        "TASK-PY-G-127_NATIVE_ACTIVE_ADAPTER_FORBIDDEN_PATH"
    );
    ensure!(
        certificate["coverage_masks_honored"] == true
            && certificate["pairwise_relation_masks_honored"] == true,
        "TASK-PY-G-127_NATIVE_ACTIVE_ADAPTER_MASKS_NOT_HONORED"
    );
    let manifest_checkpoint_file = manifest["checkpoint_file"]
        .as_str()
        .context("native adapter checkpoint_file")?;
    ensure!(
        manifest_checkpoint_file == "native_active_constellation_adapter.safetensors",
        "TASK-PY-G-127_NATIVE_ACTIVE_ADAPTER_CHECKPOINT_FILE_MISMATCH"
    );
    ensure!(
        manifest["checkpoint_sha256"].as_str() == Some(checkpoint_sha256.as_str())
            && manifest["training_certificate_sha256"].as_str()
                == Some(training_certificate_sha256.as_str())
            && report["checkpoint"]["checkpoint_sha256"].as_str()
                == Some(checkpoint_sha256.as_str())
            && report["checkpoint"]["training_certificate_sha256"].as_str()
                == Some(training_certificate_sha256.as_str()),
        "TASK-PY-G-127_NATIVE_ACTIVE_ADAPTER_HASH_MISMATCH"
    );
    let evidence = NativeActiveConstellationAdapterEvidence {
        manifest_sha256,
        checkpoint_sha256,
        training_certificate_sha256,
    };
    let summary = json!({
        "schema_version": 1,
        "source_issue": 452,
        "source_task_id": "TASK-PY-G-152",
        "model_root": model_root.display().to_string(),
        "fsv_root": fsv_root.display().to_string(),
        "manifest_path": manifest_path.display().to_string(),
        "checkpoint_path": checkpoint_path.display().to_string(),
        "training_certificate_path": certificate_path.display().to_string(),
        "report_path": report_path.display().to_string(),
        "adapter_fsv_all_passed": true,
        "model_kind": "native_active_embedder_constellation_adapter",
        "flat_vector_concat_used": false,
        "active_vectors_directly_packed_into_5120_panel": false,
        "coverage_masks_honored": true,
        "pairwise_relation_masks_honored": true,
        "manifest_sha256": evidence.manifest_sha256.clone(),
        "checkpoint_sha256": evidence.checkpoint_sha256.clone(),
        "training_certificate_sha256": evidence.training_certificate_sha256.clone(),
        "runtime_policy": "trained predictor manifests must cite this native separate-slot adapter evidence before active-embedder-trained predictions can count toward ship-gate accounting",
    });
    Ok(NativeAdapterEvidenceReadback { evidence, summary })
}

fn seed_calibration(calibration: &CalibrationStore) -> Result<()> {
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: context_graph_mejepa::Language::Python,
            predicted_test_pass: vec![if idx % 10 == 0 { 0.2 } else { 0.95 }],
            actual_test_pass: vec![if idx % 10 == 0 { 0.0 } else { 1.0 }],
        })
        .collect::<Vec<_>>();
    let norm_sq = examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, context_graph_mejepa::MejepaInferError>>()?;
    calibration.calibrate(
        &examples,
        &norm_sq,
        0.10,
        30,
        0.30,
        [8; 32],
        BTreeMap::new(),
    )?;
    Ok(())
}

fn seed_tct_constellation(db: Arc<rocksdb::DB>) -> Result<()> {
    let store = ConstellationStore::new(db)?;
    let constellation = fixture_tct_constellation(SystemTime::now() - Duration::from_secs(10))?;
    let version = store.persist(&constellation)?;
    let readback = store.load_without_runtime_checks(version)?;
    ensure!(
        readback.version_id() == version,
        "TCT constellation readback mismatch"
    );
    Ok(())
}

fn fixture_tct_constellation(frozen_at: SystemTime) -> Result<TctConstellation> {
    let mut thresholds = BTreeMap::new();
    let mut per_chunk_thresholds = BTreeMap::new();
    let mut category = BTreeMap::new();
    let mut language = BTreeMap::new();
    let mut outcome = BTreeMap::new();
    let mut chunk = BTreeMap::new();
    for (idx, embedder) in EmbedderId::all().iter().copied().enumerate() {
        thresholds.insert(embedder, -1.0);
        per_chunk_thresholds.insert((EntityType::Function, embedder), -1.0);
        let dim = panel_slot_for_embedder(embedder).dim();
        let centroid = Centroid::try_new(
            normalized_fixture_vector(dim, idx as f32 + 1.0),
            80,
            ShrinkageOrigin::OwnCell,
            "task-py-g-127-tct-centroid",
        )?;
        category.insert(embedder, centroid.clone());
        language.insert(embedder, centroid.clone());
        outcome.insert(embedder, centroid.clone());
        chunk.insert(
            (
                MutationCategory::KnownGood,
                EntityType::Function,
                TctLanguage::Python,
                embedder,
            ),
            centroid,
        );
    }
    let thresholds = Thresholds::try_new(thresholds, per_chunk_thresholds)?;
    let provenance = CorpusProvenance::try_new(
        [0x57; 32],
        EmbedderId::all()
            .iter()
            .copied()
            .map(|embedder| (embedder, [embedder as u8; 32]))
            .collect(),
        frozen_at,
        "0123456789abcdef0123456789abcdef01234567".to_string(),
    )?;
    Ok(TctConstellation::try_new(
        BTreeMap::from([(MutationCategory::KnownGood, category)]),
        BTreeMap::from([((TctLanguage::Python, MutationCategory::KnownGood), language)]),
        BTreeMap::from([(OracleOutcome::Pass, outcome)]),
        chunk,
        thresholds,
        provenance,
        frozen_at,
    )?)
}

fn normalized_fixture_vector(dim: usize, seed: f32) -> Vec<f32> {
    let mut values = (0..dim)
        .map(|idx| seed + idx as f32 * 0.01)
        .collect::<Vec<_>>();
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    for value in &mut values {
        *value /= norm;
    }
    values
}

fn now_unix_ms() -> Result<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn sha256_json(value: &impl serde::Serialize) -> Result<String> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_hex(&fs::read(path)?))
}

fn sha256_array(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn sha256_f32s(values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
