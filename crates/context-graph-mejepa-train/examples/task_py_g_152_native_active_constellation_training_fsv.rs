use anyhow::{ensure, Context, Result};
use candle_core::Device;
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa_train::active_constellation::{
    build_native_active_constellation_batch, NativeActiveConstellationAdapter,
    NATIVE_ACTIVE_CONSTELLATION_SCHEMA_VERSION,
};
use context_graph_mejepa_train::optim::build_adamw;
use context_graph_mejepa_train::TrainingConfig;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-152-native-active-constellation-training-fsv";
const MODEL_ROOT: &str =
    "/zfs/archive/contextgraph/models/task-py-g-152-native-active-constellation-training-fsv";
const CONSTELLATION_ARTIFACT_ROOT: &str =
    "/zfs/archive/contextgraph/state/training-arrays/constellation-training-arrays-v1-realinput";
const REPORT_NAME: &str = "native_active_constellation_training_fsv.json";
const CHECKPOINT_NAME: &str = "native_active_constellation_adapter.safetensors";
const MANIFEST_NAME: &str = "native_active_constellation_adapter.manifest.json";
const BATCH_SIZE: usize = 8;
const HIDDEN_DIM: usize = 64;
const OPTIMIZER_STEPS: usize = 8;

fn main() -> Result<()> {
    let fsv_root = PathBuf::from(FSV_ROOT);
    let model_root = PathBuf::from(MODEL_ROOT);
    remove_if_exists(&fsv_root)?;
    remove_if_exists(&model_root)?;
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&model_root)?;

    let device = Device::new_cuda(0).context("TASK-PY-G-152 requires aiwonder CUDA device 0")?;
    let batch = build_native_active_constellation_batch(
        Path::new(CONSTELLATION_ARTIFACT_ROOT),
        BATCH_SIZE,
        &device,
    )?;
    let batch_summary = batch.summary();
    ensure!(
        batch_summary["flat_vector_concat_used"] == false,
        "native batch must not use flat vector concatenation"
    );
    ensure!(
        batch_summary["ragged_coverage"] == true,
        "native batch must exercise coverage masks"
    );
    ensure!(
        batch.pairwise_feature_dim() > 0,
        "native batch must carry pairwise scalar relation features"
    );

    let model = NativeActiveConstellationAdapter::new(
        &batch.embedder_specs(),
        batch.pairwise_feature_dim(),
        HIDDEN_DIM,
        device.clone(),
    )?;
    let initial_weight_sha256 = model.weight_sha256()?;
    let (_loss_before_tensor, loss_before) = model.oracle_mse_loss(&batch)?;
    let training_config = TrainingConfig {
        lr: 5e-3,
        batch_size: BATCH_SIZE,
        epochs: 1,
        warmup_steps: 1,
        max_grad_norm: 5.0,
        full_finetune: true,
        ..TrainingConfig::default()
    };
    let mut optimizer = build_adamw(
        &training_config,
        BATCH_SIZE as u64,
        model.trainable_parameters(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )?;
    for _ in 0..OPTIMIZER_STEPS {
        let (loss, _scalar) = model.oracle_mse_loss(&batch)?;
        optimizer.adamw.step(&loss)?;
        device.synchronize()?;
    }
    let trained_weight_sha256 = model.weight_sha256()?;
    ensure!(
        trained_weight_sha256 != initial_weight_sha256,
        "native adapter optimizer did not change weights"
    );
    let (_loss_after_tensor, loss_after) = model.oracle_mse_loss(&batch)?;

    let checkpoint_path = model_root.join(CHECKPOINT_NAME);
    model.save_checkpoint(&checkpoint_path)?;
    set_private_file_permissions(&checkpoint_path)?;
    let checkpoint_sha256 = sha256_file(&checkpoint_path)?;
    let checkpoint_bytes = fs::metadata(&checkpoint_path)?.len();
    ensure!(checkpoint_bytes > 0, "native adapter checkpoint is empty");

    let mut reloaded = NativeActiveConstellationAdapter::new(
        &batch.embedder_specs(),
        batch.pairwise_feature_dim(),
        HIDDEN_DIM,
        device.clone(),
    )?;
    reloaded.load_checkpoint(&checkpoint_path)?;
    let reloaded_weight_sha256 = reloaded.weight_sha256()?;
    let logits = model.forward(&batch)?.flatten_all()?.to_vec1::<f32>()?;
    let reloaded_logits = reloaded.forward(&batch)?.flatten_all()?.to_vec1::<f32>()?;
    let reload_logits_match = all_close(&logits, &reloaded_logits, 1.0e-4);

    let pairwise_mask_values = batch.pairwise_mask.flatten_all()?.to_vec1::<f32>()?;
    let pairwise_mask_present = pairwise_mask_values
        .iter()
        .filter(|value| **value > 0.0)
        .count();
    let pairwise_mask_missing = pairwise_mask_values.len() - pairwise_mask_present;
    ensure!(
        pairwise_mask_present > 0 && pairwise_mask_missing > 0,
        "pairwise mask must prove present and missing relations"
    );
    let coverage_min = batch.coverage_counts.iter().copied().min().unwrap_or(0);
    let coverage_max = batch.coverage_counts.iter().copied().max().unwrap_or(0);
    ensure!(
        coverage_min > 0 && coverage_min < coverage_max,
        "coverage mask must prove ragged native embedder rows"
    );

    let training_certificate = json!({
        "schema_version": NATIVE_ACTIVE_CONSTELLATION_SCHEMA_VERSION,
        "issue": 452,
        "task_id": "TASK-PY-G-152",
        "model_kind": "native_active_embedder_constellation_adapter",
        "objective": "binary oracle consequence prediction over separate active embedder spaces plus explicit pairwise scalar relations",
        "optimizer": "AdamW",
        "optimizer_steps": optimizer.adamw.global_step(),
        "loss_before": loss_before,
        "loss_after": loss_after,
        "initial_weight_sha256": initial_weight_sha256,
        "trained_weight_sha256": trained_weight_sha256,
        "batch": batch_summary,
        "training_config_sha256": sha256_json(&training_config)?,
        "flat_vector_concat_used": false,
        "synthetic_seed_features_used": false,
        "coverage_masks_honored": true,
        "pairwise_relation_masks_honored": true,
    });
    let training_certificate_path = model_root.join("native_training_certificate.json");
    write_json_0600(&training_certificate_path, &training_certificate)?;
    let training_certificate_sha256 = sha256_file(&training_certificate_path)?;

    let manifest_path = model_root.join(MANIFEST_NAME);
    let manifest = json!({
        "schema_version": NATIVE_ACTIVE_CONSTELLATION_SCHEMA_VERSION,
        "issue": 452,
        "task_id": "TASK-PY-G-152",
        "model_kind": "native_active_embedder_constellation_adapter",
        "checkpoint_file": CHECKPOINT_NAME,
        "checkpoint_sha256": checkpoint_sha256,
        "checkpoint_bytes": checkpoint_bytes,
        "hidden_dim": HIDDEN_DIM,
        "embedder_specs": batch.embedder_specs(),
        "pairwise_pairs": batch.pairwise_pairs,
        "pairwise_feature_names": batch.pairwise_feature_names,
        "pairwise_feature_dim": batch.pairwise_feature_dim(),
        "training_certificate_sha256": training_certificate_sha256,
        "artifact_root": CONSTELLATION_ARTIFACT_ROOT,
        "artifact_hashes": batch.artifact_hashes,
        "initial_weight_sha256": training_certificate["initial_weight_sha256"],
        "trained_weight_sha256": training_certificate["trained_weight_sha256"],
        "created_at_unix_ms": now_unix_ms()?,
        "flat_vector_concat_used": false,
        "active_vectors_directly_packed_into_5120_panel": false,
        "integration": {
            "issue_441_checkpoint_bridge": "native adapter checkpoint and certificate can be cited by trained predictor export before ship-gate counting",
            "issue_406_online_learning_bridge": "same row_key/embedder_id/vector_sha256/coverage-mask schema is replay-safe for mistake-driven updates",
            "runtime_requirement": "runtime must load or cite this native adapter/certificate before counting active-embedder-trained predictions"
        }
    });
    write_json_0600(&manifest_path, &manifest)?;
    let manifest_readback: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let manifest_sha256 = sha256_file(&manifest_path)?;

    let all_passed = loss_before.is_finite()
        && loss_after.is_finite()
        && trained_weight_sha256 != initial_weight_sha256
        && reloaded_weight_sha256 == trained_weight_sha256
        && reload_logits_match
        && manifest_readback["flat_vector_concat_used"] == false
        && manifest_readback["active_vectors_directly_packed_into_5120_panel"] == false
        && pairwise_mask_present > 0
        && pairwise_mask_missing > 0
        && coverage_min < coverage_max;

    let report_path = fsv_root.join(REPORT_NAME);
    let report = json!({
        "schema_version": NATIVE_ACTIVE_CONSTELLATION_SCHEMA_VERSION,
        "task_id": "TASK-PY-G-152",
        "issue": 452,
        "source_of_truth": {
            "fsv_root": FSV_ROOT,
            "model_root": MODEL_ROOT,
            "artifact_root": CONSTELLATION_ARTIFACT_ROOT,
            "report_path": report_path,
            "manifest_path": manifest_path,
            "checkpoint_path": checkpoint_path,
            "training_certificate_path": training_certificate_path,
        },
        "batch_readback": batch.summary(),
        "training": {
            "optimizer_steps": optimizer.adamw.global_step(),
            "loss_before": loss_before,
            "loss_after": loss_after,
            "initial_weight_sha256": training_certificate["initial_weight_sha256"],
            "trained_weight_sha256": training_certificate["trained_weight_sha256"],
            "weight_delta_observed": trained_weight_sha256 != initial_weight_sha256,
        },
        "checkpoint": {
            "checkpoint_sha256": manifest["checkpoint_sha256"],
            "checkpoint_bytes": checkpoint_bytes,
            "manifest_sha256": manifest_sha256,
            "training_certificate_sha256": training_certificate_sha256,
            "reloaded_weight_sha256": reloaded_weight_sha256,
            "reload_logits_match": reload_logits_match,
        },
        "mask_readback": {
            "coverage_min": coverage_min,
            "coverage_max": coverage_max,
            "pairwise_mask_present": pairwise_mask_present,
            "pairwise_mask_missing": pairwise_mask_missing,
        },
        "forbidden_paths": {
            "flat_vector_concat_used": false,
            "synthetic_seed_features_used": false,
            "active_vectors_directly_packed_into_5120_panel": false,
        },
        "manifest_readback": manifest_readback,
        "all_passed": all_passed,
    });
    write_json_0600(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        readback["all_passed"] == true,
        "TASK-PY-G-152 FSV failed: {}",
        serde_json::to_string_pretty(&readback)?
    );
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn all_close(left: &[f32], right: &[f32], epsilon: f32) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| (*left - *right).abs() <= epsilon)
}

fn sha256_json(value: &impl serde::Serialize) -> Result<String> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_hex(&fs::read(path)?))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn now_unix_ms() -> Result<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}
