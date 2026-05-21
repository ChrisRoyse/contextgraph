use anyhow::{ensure, Context, Result};
use context_graph_mejepa::calibration::open_infer_rocksdb;
use context_graph_mejepa::cli::{
    build_slot_preserving_cuda_compiler, write_json_0600, FIXTURE_COMPILER_EVIDENCE_CLASS,
    NATIVE_ACTIVE_CONSTELLATION_ADAPTER_ATTESTATION_KEY,
    SLOT_PRESERVING_CUDA_COMPILER_EVIDENCE_CLASS,
    SLOT_PRESERVING_CUDA_TRAINED_COMPILER_EVIDENCE_CLASS, TRAINED_CHECKPOINT_ATTESTATION_KEY,
};
use context_graph_mejepa::conformal::{non_conformity_score, CalibrationExample};
use context_graph_mejepa::{
    fixture_patch_context, materialize_inference_panels, panel_sha, CalibrationStore,
    MeJepaInferConfig, MejepaStore, RocksDbInferStore,
};
use context_graph_mejepa_tct::{
    panel_slot_for_embedder, Centroid, ConstellationStore, CorpusProvenance, EmbedderId,
    EntityType, Language as TctLanguage, MutationCategory, OracleOutcome, ShrinkageOrigin,
    TctConstellation, Thresholds,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-126-deterministic-harness-guard-fsv";
const REPORT_NAME: &str = "deterministic_harness_guard_fsv.json";
const TRAINED_CHECKPOINT_MANIFEST: &str =
    "/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json";

#[derive(Debug, Clone)]
struct CheckpointEvidence {
    manifest_path: PathBuf,
    manifest_sha256: String,
    checkpoint_sha256: String,
    training_certificate_sha256: String,
    native_adapter_manifest_sha256: String,
    native_adapter_checkpoint_sha256: String,
    native_adapter_training_certificate_sha256: String,
    summary: Value,
}

fn main() -> Result<()> {
    let root = std::env::var("CG_MEJEPA_TASK_PY_G_126_FSV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_FSV_ROOT));
    remove_if_exists(&root)?;
    fs::create_dir_all(&root)?;
    let repo_root = root.join("repo");
    let db_path = root.join("infer.rocksdb");
    let report_path = root.join(REPORT_NAME);

    let db = open_infer_rocksdb(&db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    seed_calibration(&calibration)?;
    let tct_seed = seed_tct_constellation(db.clone())?;
    let checkpoint = read_checkpoint_evidence(Path::new(TRAINED_CHECKPOINT_MANIFEST))?;
    let store = Arc::new(RocksDbInferStore::new(db));
    let compiler = build_slot_preserving_cuda_compiler(
        repo_root.clone(),
        Arc::clone(&store),
        calibration,
        MeJepaInferConfig {
            outcome_set_max: 1,
            require_ood_calibrator: false,
            pause_state_path: None,
            trained_checkpoint_manifest_path: Some(checkpoint.manifest_path.clone()),
            ..MeJepaInferConfig::default()
        },
    )?;

    let (patch, context) = fixture_patch_context(&repo_root, "real-compiler")?;
    let (_panel_t0, panel_t1, panel_t2) = materialize_inference_panels(&patch, &context)?;
    let before_rows =
        MejepaStore::read_live_predictions(store.as_ref(), context.session_id, 100)?.len();
    let (prediction, predicted_panel) = compiler.compile_with_panel(&patch, &context)?;
    store.write_live_prediction(&prediction)?;
    let readback = MejepaStore::read_live_predictions(store.as_ref(), context.session_id, 100)?;
    let persisted = readback
        .iter()
        .any(|row| row.prediction_id == prediction.prediction_id);

    let happy_path = json!({
        "before_live_prediction_rows": before_rows,
        "after_live_prediction_rows": readback.len(),
        "prediction_persisted": persisted,
        "source_panel_sha_matches_predicted_panel": prediction.source_panel_sha == panel_sha(&predicted_panel),
        "prediction_has_real_compiler_attestation": prediction.granger_attestations.contains_key("MEJEPA_REAL_COMPILER"),
        "prediction_has_trained_checkpoint_attestation": prediction.granger_attestations.get(TRAINED_CHECKPOINT_ATTESTATION_KEY) == Some(&1.0),
        "prediction_has_checkpoint_sha_attestation": prediction.granger_attestations.contains_key(&format!("checkpoint_sha256:{}", checkpoint.checkpoint_sha256)),
        "prediction_has_checkpoint_manifest_sha_attestation": prediction.granger_attestations.contains_key(&format!("checkpoint_manifest_sha256:{}", checkpoint.manifest_sha256)),
        "prediction_has_training_certificate_attestation": prediction.granger_attestations.contains_key(&format!("checkpoint_training_certificate_sha256:{}", checkpoint.training_certificate_sha256)),
        "prediction_has_native_active_constellation_adapter_attestation": prediction.granger_attestations.get(NATIVE_ACTIVE_CONSTELLATION_ADAPTER_ATTESTATION_KEY) == Some(&1.0),
        "prediction_has_native_active_constellation_adapter_manifest_sha_attestation": prediction.granger_attestations.contains_key(&format!("native_active_constellation_adapter_manifest_sha256:{}", checkpoint.native_adapter_manifest_sha256)),
        "prediction_has_native_active_constellation_adapter_checkpoint_sha_attestation": prediction.granger_attestations.contains_key(&format!("native_active_constellation_adapter_checkpoint_sha256:{}", checkpoint.native_adapter_checkpoint_sha256)),
        "prediction_has_native_active_constellation_adapter_training_certificate_sha_attestation": prediction.granger_attestations.contains_key(&format!("native_active_constellation_adapter_training_certificate_sha256:{}", checkpoint.native_adapter_training_certificate_sha256)),
        "prediction_lacks_fixture_skill_attestation": !prediction.granger_attestations.contains_key("skill:python_unit"),
        "predicted_panel_filled_mask": predicted_panel.filled_mask(),
        "t1_panel_sha": hex::encode(panel_sha(&panel_t1)),
        "t2_target_panel_sha": hex::encode(panel_sha(&panel_t2)),
        "predicted_panel_sha": hex::encode(panel_sha(&predicted_panel)),
        "predicted_oracle_pass": prediction.predicted_oracle_pass,
        "predicted_test_pass_len": prediction.predicted_test_pass.len(),
    });

    let source_scan = source_scan()?;
    let edge_cases = vec![
        missing_calibration_edge(&root)?,
        missing_checkpoint_edge(&root)?,
        stub_checkpoint_edge(&root)?,
        empty_witness_edge(&repo_root, &compiler)?,
        source_guard_edge(&source_scan),
    ];
    let all_passed = persisted
        && truthy(&happy_path, "source_panel_sha_matches_predicted_panel")
        && truthy(&happy_path, "prediction_has_real_compiler_attestation")
        && truthy(&happy_path, "prediction_has_trained_checkpoint_attestation")
        && truthy(&happy_path, "prediction_has_checkpoint_sha_attestation")
        && truthy(&happy_path, "prediction_has_checkpoint_manifest_sha_attestation")
        && truthy(&happy_path, "prediction_has_training_certificate_attestation")
        && truthy(&happy_path, "prediction_has_native_active_constellation_adapter_attestation")
        && truthy(
            &happy_path,
            "prediction_has_native_active_constellation_adapter_manifest_sha_attestation",
        )
        && truthy(
            &happy_path,
            "prediction_has_native_active_constellation_adapter_checkpoint_sha_attestation",
        )
        && truthy(
            &happy_path,
            "prediction_has_native_active_constellation_adapter_training_certificate_sha_attestation",
        )
        && truthy(&happy_path, "prediction_lacks_fixture_skill_attestation")
        && source_scan["legacy_builder_name_occurrences"] == 0
        && source_scan["product_fixture_builder_occurrences"] == 0
        && source_scan["real_builder_uses_tct_guard"] == true
        && source_scan["real_builder_uses_checkpoint_loader"] == true
        && source_scan["real_builder_deterministic_guard_occurrences"] == 0
        && source_scan["product_real_compiler_occurrences"]
            .as_u64()
            .unwrap_or_default()
            >= 4
        && edge_cases
            .iter()
            .all(|case| case["passed"].as_bool() == Some(true));

    let report = json!({
        "schema_version": 1,
        "task_id": "TASK-PY-G-126",
        "issue": 438,
        "source_of_truth": {
            "db_path": db_path,
            "report_path": report_path,
            "prediction_cf": "CF_MEJEPA_LIVE_PREDICTIONS"
        },
        "evidence_classes": {
            "fixture_compiler": FIXTURE_COMPILER_EVIDENCE_CLASS,
            "real_compiler_untrained": SLOT_PRESERVING_CUDA_COMPILER_EVIDENCE_CLASS,
            "real_compiler": SLOT_PRESERVING_CUDA_TRAINED_COMPILER_EVIDENCE_CLASS,
            "fixture_ship_gate_countable": false,
            "real_compiler_uses_slot_preserving_cuda_path": true,
            "real_compiler_uses_persisted_tct_guard": true,
            "trained_checkpoint_loaded": true,
            "real_compiler_ship_gate_countable": true
        },
        "trained_checkpoint": checkpoint.summary,
        "tct_seed": tct_seed,
        "happy_path": happy_path,
        "source_scan": source_scan,
        "edge_cases": edge_cases,
        "all_passed": all_passed,
    });
    write_json_0600(&report_path, &report)?;
    let readback_report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        readback_report["all_passed"] == Value::Bool(true),
        "TASK-PY-G-126 FSV report readback failed: {}",
        serde_json::to_string_pretty(&readback_report)?
    );
    println!("{}", serde_json::to_string_pretty(&readback_report)?);
    Ok(())
}

fn read_checkpoint_evidence(manifest_path: &Path) -> Result<CheckpointEvidence> {
    ensure!(
        manifest_path.starts_with("/zfs/archive/contextgraph/")
            || manifest_path.starts_with("/zfs/hot/contextgraph/"),
        "TASK-PY-G-126_TRAINED_CHECKPOINT_NOT_ON_AIWONDER_ZFS: {}",
        manifest_path.display()
    );
    let manifest_bytes = fs::read(manifest_path).with_context(|| {
        format!(
            "read trained checkpoint manifest {}",
            manifest_path.display()
        )
    })?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let manifest: Value = serde_json::from_slice(&manifest_bytes)?;
    ensure!(
        manifest["training_status"] == "trained",
        "TASK-PY-G-126_TRAINED_CHECKPOINT_STATUS_INVALID"
    );
    let checkpoint_file = manifest["checkpoint_file"]
        .as_str()
        .context("checkpoint_file")?;
    ensure!(
        checkpoint_file == "best.safetensors",
        "TASK-PY-G-126_TRAINED_CHECKPOINT_FILE_INVALID"
    );
    let checkpoint_path = manifest_path
        .parent()
        .context("checkpoint manifest parent")?
        .join(checkpoint_file);
    let checkpoint_bytes = fs::read(&checkpoint_path)
        .with_context(|| format!("read trained checkpoint {}", checkpoint_path.display()))?;
    let checkpoint_sha256 = sha256_hex(&checkpoint_bytes);
    let manifest_checkpoint_sha256 = string_field(&manifest, "checkpoint_sha256")?;
    ensure!(
        checkpoint_sha256 == manifest_checkpoint_sha256,
        "TASK-PY-G-126_TRAINED_CHECKPOINT_HASH_MISMATCH"
    );
    ensure!(
        checkpoint_bytes.len() as u64 == manifest["checkpoint_bytes"].as_u64().unwrap_or_default(),
        "TASK-PY-G-126_TRAINED_CHECKPOINT_BYTES_MISMATCH"
    );
    let training_certificate_sha256 = string_field(&manifest, "training_certificate_sha256")?;
    let native_adapter = manifest["native_active_constellation_adapter"]
        .as_object()
        .context("native_active_constellation_adapter evidence missing")?;
    let native_adapter_manifest_sha256 = native_adapter
        .get("manifest_sha256")
        .and_then(Value::as_str)
        .context("native adapter manifest sha")?
        .to_string();
    let native_adapter_checkpoint_sha256 = native_adapter
        .get("checkpoint_sha256")
        .and_then(Value::as_str)
        .context("native adapter checkpoint sha")?
        .to_string();
    let native_adapter_training_certificate_sha256 = native_adapter
        .get("training_certificate_sha256")
        .and_then(Value::as_str)
        .context("native adapter training certificate sha")?
        .to_string();
    let summary = json!({
        "manifest_path": manifest_path.display().to_string(),
        "checkpoint_path": checkpoint_path.display().to_string(),
        "manifest_sha256": manifest_sha256.clone(),
        "checkpoint_sha256": checkpoint_sha256.clone(),
        "checkpoint_bytes": checkpoint_bytes.len(),
        "training_certificate_sha256": training_certificate_sha256.clone(),
        "native_active_constellation_adapter": {
            "manifest_sha256": native_adapter_manifest_sha256.clone(),
            "checkpoint_sha256": native_adapter_checkpoint_sha256.clone(),
            "training_certificate_sha256": native_adapter_training_certificate_sha256.clone(),
        },
        "training_status": "trained",
        "source_issue": 441,
        "source_task_id": "TASK-PY-G-127",
    });
    Ok(CheckpointEvidence {
        manifest_path: manifest_path.to_path_buf(),
        manifest_sha256,
        checkpoint_sha256,
        training_certificate_sha256,
        native_adapter_manifest_sha256,
        native_adapter_checkpoint_sha256,
        native_adapter_training_certificate_sha256,
        summary,
    })
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

fn missing_calibration_edge(root: &Path) -> Result<Value> {
    let edge_root = root.join("missing-calibration-edge");
    fs::create_dir_all(&edge_root)?;
    let db = open_infer_rocksdb(edge_root.join("infer.rocksdb"))?;
    seed_tct_constellation(db.clone())?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let store = Arc::new(RocksDbInferStore::new(db));
    let compiler = build_slot_preserving_cuda_compiler(
        edge_root.join("repo"),
        store,
        calibration,
        MeJepaInferConfig {
            pause_state_path: None,
            ..MeJepaInferConfig::default()
        },
    )?;
    let (patch, context) = fixture_patch_context(&edge_root.join("repo"), "missing-calibration")?;
    let before = json!({"active_calibration": false});
    let err = compiler.compile(&patch, &context).unwrap_err();
    let err_code = err.code();
    let passed = matches!(
        err_code,
        "MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES"
            | "MEJEPA_INFER_CALIBRATION_STALE"
            | "MEJEPA_INFER_INVALID_INPUT"
            | "MEJEPA_INFER_ROCKSDB"
    );
    let after = json!({"error_code": err_code, "error": err.to_string()});
    Ok(json!({
        "name": "missing_calibration_fails_closed",
        "before": before,
        "after": after,
        "passed": passed
    }))
}

fn missing_checkpoint_edge(root: &Path) -> Result<Value> {
    let edge_root = root.join("missing-checkpoint-edge");
    fs::create_dir_all(&edge_root)?;
    let db = open_infer_rocksdb(edge_root.join("infer.rocksdb"))?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    seed_calibration(&calibration)?;
    seed_tct_constellation(db.clone())?;
    let store = Arc::new(RocksDbInferStore::new(db));
    let manifest_path = edge_root.join("models/missing.manifest.json");
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
        Ok(_) => anyhow::bail!("missing checkpoint unexpectedly built a compiler"),
        Err(err) => err,
    };
    Ok(json!({
        "name": "missing_trained_checkpoint_fails_non_countable",
        "before": {"manifest_path": manifest_path},
        "after": {"error_code": err.code(), "error": err.to_string()},
        "passed": err.code() == "MEJEPA_INFER_PREDICTOR"
            && err.to_string().contains("No such file")
    }))
}

fn stub_checkpoint_edge(root: &Path) -> Result<Value> {
    let edge_root = root.join("stub-checkpoint-edge");
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
    let checkpoint_sha256 = sha256_hex(checkpoint_bytes);
    let manifest_path = model_root.join("best.manifest.json");
    let manifest = json!({
        "schema_version": 1,
        "checkpoint_file": "best.safetensors",
        "checkpoint_sha256": checkpoint_sha256,
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
        "code_version": "task-py-g-126-fsv",
        "created_at_unix_ms": 1u64
    });
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
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
        Ok(_) => anyhow::bail!("stub checkpoint unexpectedly built a compiler"),
        Err(err) => err,
    };
    Ok(json!({
        "name": "stub_best_safetensors_rejected",
        "before": {
            "manifest_path": manifest_path,
            "checkpoint_path": checkpoint_path,
            "checkpoint_sha256": checkpoint_sha256
        },
        "after": {"error_code": err.code(), "error": err.to_string()},
        "passed": err.code() == "MEJEPA_INFER_PREDICTOR"
            && err.to_string().contains("MEJEPA_BEST_STUB_V1")
    }))
}

fn empty_witness_edge(
    repo_root: &Path,
    compiler: &context_graph_mejepa::MeJepaCompiler,
) -> Result<Value> {
    let (mut patch, context) = fixture_patch_context(repo_root, "empty-witness")?;
    patch.witness_chain_segment.clear();
    let before = json!({"witness_len": 0});
    let err = compiler.compile(&patch, &context).unwrap_err();
    let after = json!({"error_code": err.code(), "error": err.to_string()});
    Ok(json!({
        "name": "empty_witness_fails_closed",
        "before": before,
        "after": after,
        "passed": err.code() == "MEJEPA_INFER_WITNESS_CHAIN_BROKEN"
            || err.code() == "MEJEPA_INFER_INSTRUMENT"
            || err.code() == "MEJEPA_INFER_INVALID_INPUT"
    }))
}

fn source_guard_edge(source_scan: &Value) -> Value {
    json!({
        "name": "product_paths_have_no_fixture_compiler_refs",
        "before": {
            "legacy_builder_name_occurrences": source_scan["legacy_builder_name_occurrences"],
            "product_fixture_builder_occurrences": source_scan["product_fixture_builder_occurrences"],
            "real_builder_deterministic_guard_occurrences": source_scan["real_builder_deterministic_guard_occurrences"]
        },
        "after": {
            "product_real_compiler_occurrences": source_scan["product_real_compiler_occurrences"],
            "fixture_builder_occurrences": source_scan["fixture_builder_occurrences"],
            "real_builder_uses_tct_guard": source_scan["real_builder_uses_tct_guard"]
        },
        "passed": source_scan["legacy_builder_name_occurrences"] == 0
            && source_scan["product_fixture_builder_occurrences"] == 0
        && source_scan["real_builder_uses_tct_guard"] == true
        && source_scan["real_builder_uses_checkpoint_loader"] == true
        && source_scan["real_builder_deterministic_guard_occurrences"] == 0
    })
}

fn source_scan() -> Result<Value> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .context("resolve repository root from CARGO_MANIFEST_DIR")?;
    let mut files = collect_rs_files(&repo.join("crates"))?;
    let tests_root = repo.join("tests");
    if tests_root.exists() {
        files.extend(collect_rs_files(&tests_root)?);
    }
    let mut legacy_builder_name_occurrences = 0u64;
    let mut fixture_builder_occurrences = 0u64;
    let mut product_fixture_builder_occurrences = 0u64;
    let mut product_real_compiler_occurrences = 0u64;
    let legacy_builder_symbol = ["build_", "deterministic_compiler"].concat();
    for file in files {
        let text = fs::read_to_string(&file)?;
        let path = file
            .strip_prefix(repo)
            .unwrap_or(&file)
            .display()
            .to_string();
        legacy_builder_name_occurrences += text.matches(&legacy_builder_symbol).count() as u64;
        let fixture_count = text.matches("build_fixture_deterministic_compiler").count() as u64;
        fixture_builder_occurrences += fixture_count;
        if is_product_runtime_path(&path) {
            product_fixture_builder_occurrences += fixture_count;
            product_real_compiler_occurrences +=
                text.matches("build_slot_preserving_cuda_compiler").count() as u64;
        }
    }
    let real_builder_source = real_builder_source(repo)?;
    let real_builder_deterministic_guard_occurrences = real_builder_source
        .matches("DeterministicConstellationGuard")
        .count() as u64;
    let real_builder_uses_tct_guard = real_builder_source.contains("ConstellationStore::new")
        && real_builder_source.contains("load_without_runtime_checks")
        && real_builder_source.contains("TctConstellationGuard::new");
    let real_builder_uses_checkpoint_loader =
        real_builder_source.contains("load_verified_trained_predictor_checkpoint");
    Ok(json!({
        "legacy_builder_name_occurrences": legacy_builder_name_occurrences,
        "fixture_builder_occurrences": fixture_builder_occurrences,
        "product_fixture_builder_occurrences": product_fixture_builder_occurrences,
        "product_real_compiler_occurrences": product_real_compiler_occurrences,
        "real_builder_uses_tct_guard": real_builder_uses_tct_guard,
        "real_builder_uses_checkpoint_loader": real_builder_uses_checkpoint_loader,
        "real_builder_deterministic_guard_occurrences": real_builder_deterministic_guard_occurrences,
    }))
}

fn real_builder_source(repo: &Path) -> Result<String> {
    let source = fs::read_to_string(repo.join("crates/context-graph-mejepa/src/cli.rs"))?;
    let start = source
        .find("pub fn build_slot_preserving_cuda_compiler")
        .context("build_slot_preserving_cuda_compiler not found in cli.rs")?;
    let tail = &source[start..];
    let end = tail
        .find("struct SlotPreservingCudaPredictor")
        .context("SlotPreservingCudaPredictor marker not found in cli.rs")?;
    Ok(tail[..end].to_string())
}

fn collect_rs_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_rs_files_inner(root, &mut out)?;
    Ok(out)
}

fn collect_rs_files_inner(path: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files_inner(&path, out)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn is_product_runtime_path(path: &str) -> bool {
    path.starts_with("crates/context-graph-mcp/src/")
        || path.starts_with("crates/context-graph-mejepa-shift-subscriber/src/")
        || path == "crates/context-graph-mejepa/src/verify_cli.rs"
}

fn truthy(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool) == Some(true)
}

fn string_field(value: &Value, key: &str) -> Result<String> {
    value[key]
        .as_str()
        .map(ToString::to_string)
        .with_context(|| format!("missing string field {key}"))
}

fn seed_tct_constellation(db: Arc<rocksdb::DB>) -> Result<Value> {
    let store = ConstellationStore::new(db)?;
    let constellation = fixture_tct_constellation(SystemTime::now() - Duration::from_secs(10))?;
    let version = store.persist(&constellation)?;
    let readback = store.load_without_runtime_checks(version)?;
    Ok(json!({
        "cf": context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION,
        "version_hex": hex::encode(version),
        "readback_version_hex": hex::encode(readback.version_id()),
        "centroid_embedder_count": EmbedderId::all().len(),
        "readback_matches": readback.version_id() == version
    }))
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
            "task-py-g-126-tct-centroid",
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
        [0x56; 32],
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

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
