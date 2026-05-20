use anyhow::{bail, ensure, Context, Result};
use candle_core::{DType, Device, Tensor};
use context_graph_mejepa::calibration::open_infer_rocksdb;
use context_graph_mejepa::cli::{
    build_slot_preserving_cuda_compiler, write_json_0600, TRAINED_CHECKPOINT_ATTESTATION_KEY,
};
use context_graph_mejepa::conformal::{non_conformity_score, CalibrationExample};
use context_graph_mejepa::{
    export_trained_predictor_checkpoint, fixture_patch_context,
    load_verified_trained_predictor_checkpoint, materialize_inference_panels, panel_sha,
    predictor_weight_content_sha256, CalibrationStore, FrozenTargetAdapter, MeJepaInferConfig,
    MeJepaPredictor, MejepaStore, PredictorCheckpointExportMetadata, PredictorConfig,
    RocksDbInferStore, PANEL_DIM,
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
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-127-trained-checkpoint-export-fsv";
const MODEL_ROOT: &str =
    "/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv";
const REPORT_NAME: &str = "trained_checkpoint_export_fsv.json";

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
        batch_size: 2,
        epochs: 1,
        warmup_steps: 1,
        max_grad_norm: 5.0,
        full_finetune: true,
        ..TrainingConfig::default()
    };
    let (panel_t0, panel_t1, target_panel, corpus_sha256) = deterministic_training_batch(&device)?;
    let (loss_tensor, loss_before) =
        predictor_training_loss(&predictor, &panel_t0, &panel_t1, &target_panel)?;
    let mut optimizer = build_adamw(
        &training_config,
        2,
        predictor.trainable_parameters(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )?;
    optimizer.adamw.step(&loss_tensor)?;
    device.synchronize()?;
    let trained_weight_sha256 = predictor_weight_content_sha256(&predictor)?;
    let (_loss_after_tensor, loss_after) =
        predictor_training_loss(&predictor, &panel_t0, &panel_t1, &target_panel)?;
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
            "kind": "L_predict(predicted_panel, target_panel)",
            "before": loss_before,
            "after": loss_after
        },
        "initial_weight_sha256": initial_weight_sha256,
        "trained_weight_sha256": trained_weight_sha256,
        "weight_delta_observed": weight_delta_observed,
        "corpus_sha256": corpus_sha256,
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
            training_mode: "full_finetune_predictor_smoke".to_string(),
            initial_weight_sha256: initial_weight_sha256.clone(),
            training_certificate_sha256: training_certificate_sha256.clone(),
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
        && runtime["prediction_persisted"] == true
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
            "corpus_sha256": corpus_sha256
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
) -> Result<(Tensor, f32)> {
    let predicted = predictor.forward(panel_t0, panel_t1)?;
    let target = target_panel.to_dtype(predicted.tensor.dtype())?;
    let loss = l_predict(&predicted.tensor, &target)?;
    let scalar = loss.to_dtype(DType::F32)?.to_scalar::<f32>()?;
    Ok((loss, scalar))
}

fn deterministic_training_batch(device: &Device) -> Result<(Tensor, Tensor, Tensor, String)> {
    let batch = 2usize;
    let mut t0 = Vec::with_capacity(batch * PANEL_DIM);
    let mut t1 = Vec::with_capacity(batch * PANEL_DIM);
    let mut target = Vec::with_capacity(batch * PANEL_DIM);
    for row in 0..batch {
        for dim in 0..PANEL_DIM {
            let x = (row * PANEL_DIM + dim) as f32;
            let a = ((x * 0.001).sin() * 0.25) + row as f32 * 0.05;
            let b = ((x * 0.0007).cos() * 0.20) - row as f32 * 0.03;
            t0.push(a);
            t1.push(b);
            target.push((a * 0.65 + b * 0.35).tanh());
        }
    }
    let corpus_sha256 = sha256_json(&json!({
        "schema_version": 1,
        "kind": "task-py-g-127-deterministic-panel-smoke",
        "batch": batch,
        "panel_dim": PANEL_DIM,
        "t0_sha256": sha256_f32s(&t0),
        "t1_sha256": sha256_f32s(&t1),
        "target_sha256": sha256_f32s(&target)
    }))?;
    Ok((
        Tensor::from_slice(&t0, (batch, PANEL_DIM), device)?.to_dtype(DType::BF16)?,
        Tensor::from_slice(&t1, (batch, PANEL_DIM), device)?.to_dtype(DType::BF16)?,
        Tensor::from_slice(&target, (batch, PANEL_DIM), device)?.to_dtype(DType::BF16)?,
        corpus_sha256,
    ))
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
        "predicted_oracle_pass": prediction.predicted_oracle_pass
    }))
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
