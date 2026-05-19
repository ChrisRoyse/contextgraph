use context_graph_mejepa::prediction_surfaces::covered_chunks_for_patch;
use context_graph_mejepa::{
    build_deterministic_compiler, materialize_inference_panels, open_infer_rocksdb, panel_sha,
    CalibrationExample, CalibrationStore, DdaSignals, DeterministicPredictor, MeJepaInferConfig,
    MejepaStore, PanelId, Predictor, RocksDbInferStore,
};
use context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS;
use context_graph_mejepa_train::dda::{
    count_persisted_dda_signals, encode_dda_signal_key, persist_dda_signals, read_dda_signals,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/zfs/archive/contextgraph/fsv/dda-prediction-consumption-fsv");
    reset_secure_dir(&root)?;
    let happy = run_happy_path(&root)?;
    let edges = json!({
        "missing_dda_row": run_edge_missing_row(&root)?,
        "malformed_dda_row": run_edge_malformed_row(&root)?,
        "shape_mismatch": run_edge_shape_mismatch(&root)?,
    });
    let evidence_path = root.join("dda-prediction-consumption-fsv.json");
    let evidence = json!({
        "source_of_truth": {
            "dda_cf": CF_MEJEPA_DDA_SIGNALS,
            "live_prediction_cf": context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
            "artifact_dir": root.display().to_string(),
        },
        "happy_path": happy,
        "edge_cases": edges,
    });
    write_secure_json(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        "happy path persisted DDA-backed prediction",
        readback["happy_path"]["passes"] == true,
    )?;
    require(
        "edge cases fail closed",
        readback["edge_cases"]
            .as_object()
            .ok_or("edge_cases not object")?
            .values()
            .all(|edge| edge["passes"] == true),
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn run_happy_path(root: &Path) -> FsvResult<Value> {
    let case = "happy_path";
    let state = setup(root, case, true, 3)?;
    let before = state_counts(&state)?;
    println!("before {case}: {}", serde_json::to_string(&before)?);
    let (panel_id, chunks) = prediction_identity(&state.patch, &state.context)?;
    for chunk in &chunks {
        persist_dda_signals(state.db.as_ref(), &panel_id, chunk, &signals_three())?;
    }
    let prediction = state.compiler.compile(&state.patch, &state.context)?;
    state.store.write_live_prediction(&prediction)?;
    let predictions = state
        .store
        .read_live_predictions(state.context.session_id, 1000)?;
    let readback = predictions
        .iter()
        .find(|row| row.prediction_id == prediction.prediction_id)
        .ok_or("DDA-backed prediction missing from live prediction readback")?;
    let dda_readback = read_dda_signals(state.db.as_ref(), &panel_id, &chunks[0])?
        .ok_or("DDA row missing after persist")?;
    let dda_keys = readback
        .granger_attestations
        .keys()
        .filter(|key| key.starts_with("dda:"))
        .cloned()
        .collect::<Vec<_>>();
    let after = json!({
        "counts": state_counts(&state)?,
        "panel_id": hex::encode(panel_id.0),
        "chunk_ids": chunks.iter().map(|chunk| chunk.0.clone()).collect::<Vec<_>>(),
        "dda_row_readback": dda_readback,
        "prediction_id": hex::encode(readback.prediction_id),
        "persisted_granger_keys": readback.granger_attestations.keys().cloned().collect::<Vec<_>>(),
        "dda_granger_keys": dda_keys,
    });
    println!("after {case}: {}", serde_json::to_string(&after)?);
    let passes = before["dda_rows"] == 0
        && after["counts"]["dda_rows"] == chunks.len()
        && after["counts"]["live_predictions"] == 1
        && after["dda_granger_keys"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0)
            >= 6;
    Ok(json!({ "before_state": before, "after_state": after, "passes": passes }))
}

fn run_edge_missing_row(root: &Path) -> FsvResult<Value> {
    let case = "missing_dda_row";
    let state = setup(root, case, true, 3)?;
    let before = state_counts(&state)?;
    println!("before {case}: {}", serde_json::to_string(&before)?);
    let err = state
        .compiler
        .compile(&state.patch, &state.context)
        .unwrap_err();
    let after = json!({
        "counts": state_counts(&state)?,
        "error_code": err.code(),
        "error": err.to_string(),
    });
    println!("after {case}: {}", serde_json::to_string(&after)?);
    Ok(json!({
        "before_state": before,
        "after_state": after,
        "expected_code": "MEJEPA_INFER_DDA_FEATURE_MISSING",
        "passes": err.code() == "MEJEPA_INFER_DDA_FEATURE_MISSING"
            && after["counts"]["live_predictions"] == before["live_predictions"],
    }))
}

fn run_edge_malformed_row(root: &Path) -> FsvResult<Value> {
    let case = "malformed_dda_row";
    let state = setup(root, case, true, 3)?;
    let before = state_counts(&state)?;
    println!("before {case}: {}", serde_json::to_string(&before)?);
    let (panel_id, chunks) = prediction_identity(&state.patch, &state.context)?;
    let cf = state
        .db
        .cf_handle(CF_MEJEPA_DDA_SIGNALS)
        .ok_or("missing DDA CF")?;
    let key = encode_dda_signal_key(&panel_id, &chunks[0])?;
    state.db.put_cf(cf, key, b"{ not valid json")?;
    let err = state
        .compiler
        .compile(&state.patch, &state.context)
        .unwrap_err();
    let after = json!({
        "counts": state_counts(&state)?,
        "error_code": err.code(),
        "error": err.to_string(),
    });
    println!("after {case}: {}", serde_json::to_string(&after)?);
    Ok(json!({
        "before_state": before,
        "after_state": after,
        "expected_code": "MEJEPA_INFER_JSON",
        "passes": err.code() == "MEJEPA_INFER_JSON"
            && after["counts"]["live_predictions"] == before["live_predictions"],
    }))
}

fn run_edge_shape_mismatch(root: &Path) -> FsvResult<Value> {
    let case = "shape_mismatch";
    let state = setup(root, case, true, 3)?;
    let before = state_counts(&state)?;
    println!("before {case}: {}", serde_json::to_string(&before)?);
    let (panel_id, chunks) = prediction_identity(&state.patch, &state.context)?;
    persist_dda_signals(state.db.as_ref(), &panel_id, &chunks[0], &signals_two())?;
    let err = state
        .compiler
        .compile(&state.patch, &state.context)
        .unwrap_err();
    let after = json!({
        "counts": state_counts(&state)?,
        "error_code": err.code(),
        "error": err.to_string(),
    });
    println!("after {case}: {}", serde_json::to_string(&after)?);
    Ok(json!({
        "before_state": before,
        "after_state": after,
        "expected_code": "MEJEPA_INFER_DIM_MISMATCH",
        "passes": err.code() == "MEJEPA_INFER_DIM_MISMATCH"
            && after["counts"]["live_predictions"] == before["live_predictions"],
    }))
}

struct State {
    db: Arc<rocksdb::DB>,
    store: Arc<RocksDbInferStore>,
    compiler: context_graph_mejepa::MeJepaCompiler,
    patch: context_graph_mejepa::PatchBundle,
    context: context_graph_mejepa::TaskContext,
}

fn setup(
    root: &Path,
    case: &str,
    require_dda: bool,
    expected_embedders: usize,
) -> FsvResult<State> {
    let db_path = root.join(format!("{case}-rocksdb"));
    let repo_root = root.join(format!("{case}-repo"));
    if db_path.exists() {
        fs::remove_dir_all(&db_path)?;
    }
    if repo_root.exists() {
        fs::remove_dir_all(&repo_root)?;
    }
    fs::create_dir_all(&repo_root)?;
    let db = open_infer_rocksdb(&db_path)?;
    let calibration_store = CalibrationStore::new(db.clone(), 30)?;
    let examples = calibration_examples();
    calibration_store.calibrate(
        &examples,
        &[0.01; 40],
        0.10,
        30,
        0.30,
        [7; 32],
        BTreeMap::new(),
    )?;
    let store = Arc::new(RocksDbInferStore::new(db.clone()));
    let compiler = build_deterministic_compiler(
        repo_root.clone(),
        store.clone(),
        calibration_store,
        MeJepaInferConfig {
            outcome_set_max: 1,
            require_dda_features: require_dda,
            dda_expected_embedder_count: expected_embedders,
            ..MeJepaInferConfig::default()
        },
    )?;
    let (patch, context) = context_graph_mejepa::fixture_patch_context(&repo_root, case)?;
    Ok(State {
        db,
        store,
        compiler,
        patch,
        context,
    })
}

fn prediction_identity(
    patch: &context_graph_mejepa::PatchBundle,
    context: &context_graph_mejepa::TaskContext,
) -> FsvResult<(PanelId, Vec<context_graph_mejepa::ChunkId>)> {
    let (panel_t0, panel_t1, _) = materialize_inference_panels(patch, context)?;
    let predicted = DeterministicPredictor.predict(&panel_t0, &panel_t1)?;
    let panel_id = PanelId(panel_sha(&predicted));
    Ok((panel_id, covered_chunks_for_patch(patch)?))
}

fn state_counts(state: &State) -> FsvResult<Value> {
    Ok(json!({
        "dda_rows": count_persisted_dda_signals(state.db.as_ref())?,
        "live_predictions": state.store.read_live_predictions(state.context.session_id, 1000)?.len(),
    }))
}

fn signals_three() -> DdaSignals {
    DdaSignals::try_new(DdaSignals {
        per_embedder_cosine: vec![1.0, 0.5, 0.25],
        pairwise_cosine_upper: vec![0.10, 0.20, 0.30],
        pairwise_mi_upper: vec![0.01, 0.02, 0.03],
        blind_spot_z_scores: vec![0.5, 0.0, -0.5],
    })
    .unwrap()
}

fn signals_two() -> DdaSignals {
    DdaSignals::try_new(DdaSignals {
        per_embedder_cosine: vec![1.0, 0.5],
        pairwise_cosine_upper: vec![0.10],
        pairwise_mi_upper: vec![0.01],
        blind_spot_z_scores: vec![0.5],
    })
    .unwrap()
}

fn calibration_examples() -> Vec<CalibrationExample> {
    (0..40)
        .map(|idx| CalibrationExample {
            language: context_graph_mejepa::Language::Python,
            predicted_test_pass: vec![0.90],
            actual_test_pass: vec![if idx % 2 == 0 { 1.0 } else { 0.0 }],
        })
        .collect()
}

fn reset_secure_dir(path: &Path) -> FsvResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_secure_json(path: &Path, value: &Value) -> FsvResult<()> {
    let mut file = File::create(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV assertion failed: {label}").into())
    }
}
