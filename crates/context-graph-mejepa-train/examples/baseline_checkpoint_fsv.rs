use context_graph_mejepa_train::checkpoint::{
    AdamWStateBlob, CheckpointKind, CheckpointPayload, Checkpointer, TensorSnapshot,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/baseline-checkpoint-fsv";

#[derive(Debug, Serialize)]
struct FileEvidence {
    path: String,
    bytes: u64,
    sha256: String,
    json: Option<Value>,
}

#[derive(Debug, Serialize)]
struct DirState {
    label: String,
    root: String,
    files: Vec<FileEvidence>,
}

#[derive(Debug, Serialize)]
struct EdgeReport {
    case: String,
    trigger: String,
    expected_error: String,
    before: DirState,
    error_code: String,
    error_message: String,
    after: DirState,
}

#[derive(Debug, Serialize)]
struct FsvReport {
    source_of_truth: Value,
    before: DirState,
    after_write: DirState,
    loaded_payload: Value,
    manifest_path: String,
    checkpoint_path: String,
    edge_cases: Vec<EdgeReport>,
    evidence_path: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(FSV_ROOT)?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{}", unix_ms()?));
    let happy_root = run_root.join("happy");
    let checkpointer = Checkpointer::new(happy_root.clone(), 1);
    let payload = synthetic_payload();
    let corpus_sha = sha256_hex(b"phase-c-corpus:synthetic-known-good");
    let config_sha = sha256_hex(b"train-config:baseline-fsv");
    let before = dir_state("happy_before", &happy_root)?;
    let (manifest_path, manifest) = checkpointer.write_with_manifest(
        &payload,
        CheckpointKind::Best,
        corpus_sha.clone(),
        config_sha.clone(),
    )?;
    let (loaded_manifest, loaded_payload) = Checkpointer::load_verified(&manifest_path)?;
    if loaded_manifest != manifest || loaded_payload.step != payload.step {
        return Err("verified checkpoint load did not match the written payload".into());
    }
    let after_write = dir_state("happy_after_write", &happy_root)?;
    let checkpoint_path = happy_root.join(&manifest.checkpoint_file);
    let report = FsvReport {
        source_of_truth: json!({
            "type": "checkpoint_artifact_plus_manifest",
            "checkpoint_path": checkpoint_path,
            "manifest_path": manifest_path,
            "verification": [
                "write checkpoint atomically",
                "read manifest JSON from disk",
                "read checkpoint bytes from disk",
                "recompute SHA-256 and byte length",
                "load payload through Checkpointer::load_verified"
            ]
        }),
        before,
        after_write,
        loaded_payload: json!({
            "step": loaded_payload.step,
            "training_mode": loaded_payload.training_mode,
            "predictor_weight_keys": loaded_payload.predictor_weights.keys().cloned().collect::<Vec<_>>(),
            "aux_head_keys": loaded_payload.aux_heads.keys().cloned().collect::<Vec<_>>(),
            "sampler_rng_state": loaded_payload.sampler_rng_state
        }),
        manifest_path: manifest_path.display().to_string(),
        checkpoint_path: checkpoint_path.display().to_string(),
        edge_cases: vec![
            missing_artifact_edge(&run_root, &payload, &corpus_sha, &config_sha)?,
            hash_mismatch_edge(&run_root, &payload, &corpus_sha, &config_sha)?,
            schema_mismatch_edge(&run_root, &payload, &corpus_sha, &config_sha)?,
        ],
        evidence_path: PathBuf::from(FSV_ROOT)
            .join("baseline-checkpoint-fsv.json")
            .display()
            .to_string(),
    };
    fs::write(&report.evidence_path, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn missing_artifact_edge(
    run_root: &Path,
    payload: &CheckpointPayload,
    corpus_sha: &str,
    config_sha: &str,
) -> Result<EdgeReport, Box<dyn Error>> {
    let root = run_root.join("edge-missing-artifact");
    let checkpointer = Checkpointer::new(root.clone(), 1);
    let (manifest_path, manifest) = checkpointer.write_with_manifest(
        payload,
        CheckpointKind::Best,
        corpus_sha.to_string(),
        config_sha.to_string(),
    )?;
    let checkpoint_path = root.join(manifest.checkpoint_file);
    fs::remove_file(&checkpoint_path)?;
    let before = dir_state("missing_artifact_before_load", &root)?;
    let err = Checkpointer::load_verified(&manifest_path)
        .expect_err("missing checkpoint artifact unexpectedly loaded");
    let after = dir_state("missing_artifact_after_load", &root)?;
    Ok(edge_report(
        "missing_artifact_fails_closed",
        "delete the checkpoint file referenced by a valid manifest",
        "MEJEPA_TRAIN_CHECKPOINT_CORRUPT",
        before,
        err,
        after,
    ))
}

fn hash_mismatch_edge(
    run_root: &Path,
    payload: &CheckpointPayload,
    corpus_sha: &str,
    config_sha: &str,
) -> Result<EdgeReport, Box<dyn Error>> {
    let root = run_root.join("edge-hash-mismatch");
    let checkpointer = Checkpointer::new(root.clone(), 1);
    let (manifest_path, manifest) = checkpointer.write_with_manifest(
        payload,
        CheckpointKind::Best,
        corpus_sha.to_string(),
        config_sha.to_string(),
    )?;
    fs::write(root.join(manifest.checkpoint_file), b"{\"tampered\":true}")?;
    let before = dir_state("hash_mismatch_before_load", &root)?;
    let err = Checkpointer::load_verified(&manifest_path)
        .expect_err("tampered checkpoint unexpectedly loaded");
    let after = dir_state("hash_mismatch_after_load", &root)?;
    Ok(edge_report(
        "hash_mismatch_fails_closed",
        "replace the checkpoint bytes after manifest creation",
        "MEJEPA_TRAIN_CHECKPOINT_CORRUPT",
        before,
        err,
        after,
    ))
}

fn schema_mismatch_edge(
    run_root: &Path,
    payload: &CheckpointPayload,
    corpus_sha: &str,
    config_sha: &str,
) -> Result<EdgeReport, Box<dyn Error>> {
    let root = run_root.join("edge-schema-mismatch");
    let checkpointer = Checkpointer::new(root.clone(), 1);
    let (manifest_path, _) = checkpointer.write_with_manifest(
        payload,
        CheckpointKind::Best,
        corpus_sha.to_string(),
        config_sha.to_string(),
    )?;
    let mut manifest_json: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    manifest_json["schema_version"] = json!(999);
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest_json)?)?;
    let before = dir_state("schema_mismatch_before_load", &root)?;
    let err = Checkpointer::load_verified(&manifest_path)
        .expect_err("schema-mismatched manifest unexpectedly loaded");
    let after = dir_state("schema_mismatch_after_load", &root)?;
    Ok(edge_report(
        "schema_mismatch_fails_closed",
        "set manifest schema_version to 999",
        "MEJEPA_TRAIN_CHECKPOINT_CORRUPT",
        before,
        err,
        after,
    ))
}

fn edge_report(
    case: &str,
    trigger: &str,
    expected_error: &str,
    before: DirState,
    err: context_graph_mejepa_train::TrainerError,
    after: DirState,
) -> EdgeReport {
    EdgeReport {
        case: case.to_string(),
        trigger: trigger.to_string(),
        expected_error: expected_error.to_string(),
        before,
        error_code: err.code().to_string(),
        error_message: err.to_string(),
        after,
    }
}

fn synthetic_payload() -> CheckpointPayload {
    let tensor = TensorSnapshot {
        shape: vec![2, 2],
        values_f32: vec![0.25, -0.5, 0.75, 1.0],
    };
    CheckpointPayload {
        predictor_weights: HashMap::from([("prediction_head.weight".to_string(), tensor.clone())]),
        lora_adapters: None,
        aux_heads: HashMap::from([("test_pass.bias".to_string(), tensor.clone())]),
        adamw_state: AdamWStateBlob {
            m: HashMap::from([("prediction_head.weight".to_string(), tensor.clone())]),
            v: HashMap::from([("prediction_head.weight".to_string(), tensor)]),
            step: 42,
            lr_schedule_state: json!({"schedule": "constant", "lr": 0.0003}),
        },
        sampler_rng_state: 0xC0FFEE,
        step: 42,
        training_mode: "bootstrap_baseline_release_fsv".to_string(),
    }
}

fn dir_state(label: &str, root: &Path) -> Result<DirState, Box<dyn Error>> {
    let mut files = Vec::new();
    if root.exists() {
        for entry in fs::read_dir(root)? {
            let path = entry?.path();
            if path.is_file() {
                let bytes = fs::read(&path)?;
                files.push(FileEvidence {
                    path: path.display().to_string(),
                    bytes: bytes.len() as u64,
                    sha256: sha256_hex(&bytes),
                    json: serde_json::from_slice(&bytes).ok(),
                });
            }
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(DirState {
        label: label.to_string(),
        root: root.display().to_string(),
        files,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn unix_ms() -> Result<u128, Box<dyn Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}
