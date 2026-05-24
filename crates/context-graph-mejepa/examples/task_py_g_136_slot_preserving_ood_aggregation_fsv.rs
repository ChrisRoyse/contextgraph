use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, ensure, Context, Result};
use chrono::Utc;
use context_graph_mejepa::types::PerSlotOodReasonKind;
use context_graph_mejepa::{
    count_cf, open_infer_rocksdb, sha256_bytes, valid_witness_segment, AstDiff, CalibrationExample,
    CalibrationStore, DeterministicConstellationGuard, DeterministicOracleHead, DiffHunk,
    FrozenTarget, Language, MeJepaCompiler, MeJepaInferConfig, MejepaInferError, MejepaStore,
    PatchBundle, PatchWitnessReader, Predictor, RocksDbInferStore, TaskContext, TaskEnvironment,
    TaskId, TestId, TrainCertSummary, Verdict,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_CALIBRATION_HISTORY, CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_instruments::{InstrumentSlot, Panel, PANEL_DIM};
use serde_json::{json, Value};

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

fn main() -> Result<()> {
    let run_root = output_root()?.join(format!("run-{}", Utc::now().format("%Y%m%dT%H%M%SZ")));
    fs::create_dir_all(&run_root)?;
    let db_path = run_root.join("infer-rocksdb");
    let db = open_infer_rocksdb(&db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let calibration_readback = seed_calibration_and_train_cert(&db, &calibration)?;

    let single_slot = run_case(
        db.clone(),
        "single_slot_e_reasoning",
        [0x36; 16],
        panel_with_slot_delta(InstrumentSlot::EReasoning, 3.0),
        panel_with_slot_delta(InstrumentSlot::EReasoning, 0.0),
        Verdict::GuardRejected,
        InstrumentSlot::EReasoning.slug(),
        PerSlotOodReasonKind::SlotThresholdExceeded,
    )?;
    let diffuse = run_case(
        db.clone(),
        "diffuse_all_slots",
        [0x37; 16],
        panel_with_all_slot_deltas(3.0),
        panel_with_all_slot_deltas(0.0),
        Verdict::OutOfDistribution,
        "diffuse",
        PerSlotOodReasonKind::DiffuseSlotThresholdExceeded,
    )?;
    let missing_per_slot_calibrator = run_missing_per_slot_calibrator_case(db.clone())?;

    let report = json!({
        "schema_version": 1,
        "task": "TASK-PY-G-136",
        "issue": 618,
        "run_dir": run_root,
        "db_path": db_path,
        "calibration_cf": CF_MEJEPA_CALIBRATION_HISTORY,
        "live_prediction_cf": CF_MEJEPA_LIVE_PREDICTIONS,
        "calibration_readback": calibration_readback,
        "single_slot": single_slot,
        "diffuse": diffuse,
        "missing_per_slot_calibrator": missing_per_slot_calibrator,
        "cf_counts_after": {
            "calibration_history": count_cf(db.as_ref(), CF_MEJEPA_CALIBRATION_HISTORY)?,
            "live_predictions": count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?,
        },
    });
    let report_json = run_root.join("report.json");
    fs::write(&report_json, serde_json::to_vec_pretty(&report)?)?;
    fs::write(run_root.join("report.md"), report_markdown(&report))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn output_root() -> Result<PathBuf> {
    let mut root = std::env::var_os("CONTEXTGRAPH_FSV_OUTPUT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join("task_py_g_136_slot_preserving_ood_aggregation_fsv")
        });
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == OsStr::new("--output-root") {
            let value = args
                .next()
                .context("--output-root requires a path argument")?;
            root = PathBuf::from(value);
        } else {
            bail!("unknown argument {:?}; expected --output-root <path>", arg);
        }
    }
    Ok(root)
}

fn seed_calibration_and_train_cert(
    db: &Arc<rocksdb::DB>,
    calibration: &CalibrationStore,
) -> Result<Value> {
    let before_calibration_rows = count_cf(db.as_ref(), CF_MEJEPA_CALIBRATION_HISTORY)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![if idx % 10 == 0 { 0.2 } else { 0.95 }],
            actual_test_pass: vec![if idx % 10 == 0 { 0.0 } else { 1.0 }],
        })
        .collect::<Vec<_>>();
    calibration.calibrate(
        &examples,
        &[0.01; 40],
        0.10,
        30,
        0.30,
        [7; 32],
        BTreeMap::new(),
    )?;
    let active = calibration.load_active()?;
    let per_slot_sigma_squared = active
        .per_slot_sigma_squared
        .as_ref()
        .context("fresh calibration row did not persist per_slot_sigma_squared")?;
    ensure!(
        per_slot_sigma_squared.len() == InstrumentSlot::all().len(),
        "fresh calibration row should cover every slot; got {}",
        per_slot_sigma_squared.len()
    );
    for slot in InstrumentSlot::all() {
        let sigma_squared = *per_slot_sigma_squared
            .get(&slot)
            .with_context(|| format!("fresh calibration row missing slot {slot:?}"))?;
        ensure!(
            sigma_squared.is_finite() && sigma_squared > 0.0,
            "fresh calibration row has invalid sigma for {slot:?}: {sigma_squared}"
        );
    }
    let after_calibration_rows = calibration.count_history()?;
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .context("missing CF_MEJEPA_TRAIN_CERTS")?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.9,
        delta_xi: 0.9,
        witness_offset: 88,
        // #699: FSV example simulates a "trained" cert; bump to 1 so the
        // downstream confidence multiplier path exercises the Measured arm
        // rather than DiagnosticCertificateOnlyNeutral.
        predictor_parameter_update_count: 1,
    };
    db.put_cf(cf, b"cert:task-py-g-136:0001", bincode::serialize(&cert)?)?;
    Ok(json!({
        "before_calibration_rows": before_calibration_rows,
        "after_calibration_rows": after_calibration_rows,
        "active_version": active.version.clone(),
        "active_per_slot_sigma_count": per_slot_sigma_squared.len(),
        "active_per_slot_sigma_slots": per_slot_sigma_squared
            .keys()
            .map(|slot| slot.slug())
            .collect::<Vec<_>>(),
        "active_per_slot_sigma_all_positive": per_slot_sigma_squared
            .values()
            .all(|sigma| sigma.is_finite() && *sigma > 0.0),
        "train_cert_cf": CF_MEJEPA_TRAIN_CERTS,
        "train_cert_key": "cert:task-py-g-136:0001",
    }))
}

fn run_case(
    db: Arc<rocksdb::DB>,
    name: &str,
    session_id: [u8; 16],
    predicted: Panel,
    target: Panel,
    expected_verdict: Verdict,
    expected_embedder: &str,
    expected_reason: PerSlotOodReasonKind,
) -> Result<Value> {
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let store = Arc::new(RocksDbInferStore::new(db));
    let config = MeJepaInferConfig {
        pause_state_path: None,
        ..MeJepaInferConfig::default()
    };
    let compiler = MeJepaCompiler::new(
        config,
        Arc::new(FixedPanelPredictor { panel: predicted }),
        Arc::new(FixedPanelTarget { panel: target }),
        Arc::new(DeterministicOracleHead),
        Arc::new(DeterministicConstellationGuard::default()),
        Arc::new(PatchWitnessReader),
        store.clone(),
        calibration,
        PathBuf::from("/zfs/archive/contextgraph/test-repo"),
    )?;
    let (patch, context) = sample_patch_context(name, session_id)?;
    let (prediction, _) = compiler.compile_with_panel(&patch, &context)?;
    ensure!(
        prediction.verdict == expected_verdict,
        "{name}: expected {:?}, got {:?}",
        expected_verdict,
        prediction.verdict
    );
    ensure!(
        prediction.per_slot_ood_reasons.len() == 1,
        "{name}: expected one per-slot OOD reason, got {}",
        prediction.per_slot_ood_reasons.len()
    );
    let reason = &prediction.per_slot_ood_reasons[0];
    ensure!(
        reason.embedder.0 == expected_embedder,
        "{name}: expected embedder {expected_embedder}, got {}",
        reason.embedder.0
    );
    ensure!(
        reason.reason == expected_reason,
        "{name}: expected reason {:?}, got {:?}",
        expected_reason,
        reason.reason
    );
    store.write_live_prediction(&prediction)?;
    let readback = store.read_live_predictions(context.session_id, 1)?;
    let readback_prediction = readback
        .first()
        .context("CF_MEJEPA_LIVE_PREDICTIONS readback returned no rows")?;
    ensure!(
        readback_prediction.prediction_id == prediction.prediction_id,
        "{name}: readback prediction_id did not match compiled prediction"
    );
    Ok(json!({
        "case": name,
        "verdict": prediction.verdict,
        "prediction_id": hex::encode(prediction.prediction_id),
        "ood_score": prediction.ood_score,
        "expected_embedder": expected_embedder,
        "per_slot_ood_reasons": prediction.per_slot_ood_reasons,
        "cf_readback_prediction_id": hex::encode(readback_prediction.prediction_id),
        "cf_readback_equal": readback_prediction == &prediction,
    }))
}

fn run_missing_per_slot_calibrator_case(db: Arc<rocksdb::DB>) -> Result<Value> {
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let active_before = calibration.load_active()?;
    let before_per_slot_sigma_count = active_before
        .per_slot_sigma_squared
        .as_ref()
        .map(|values| values.len())
        .unwrap_or(0);
    ensure!(
        before_per_slot_sigma_count == InstrumentSlot::all().len(),
        "missing-calibrator edge requires a complete calibration before mutation; got {before_per_slot_sigma_count}"
    );
    let store = Arc::new(RocksDbInferStore::new(db));
    let session_id = [0x38; 16];
    let before_live_prediction_rows = store.read_live_predictions(session_id, 1000)?.len();

    let mut missing = (*active_before).clone();
    missing.version = format!("{}-missing-per-slot-edge", missing.version);
    missing.frozen_at = Utc::now().timestamp() + 1;
    missing.per_slot_sigma_squared = None;
    calibration.persist(&missing)?;
    let active_after = calibration.load_active()?;
    ensure!(
        active_after.version == missing.version,
        "active calibration did not advance to missing-per-slot edge record"
    );
    ensure!(
        active_after.per_slot_sigma_squared.is_none(),
        "missing-per-slot edge record should read back with no per-slot calibrator"
    );

    let compiler = MeJepaCompiler::new(
        MeJepaInferConfig {
            pause_state_path: None,
            ..MeJepaInferConfig::default()
        },
        Arc::new(FixedPanelPredictor {
            panel: panel_with_slot_delta(InstrumentSlot::EReasoning, 3.0),
        }),
        Arc::new(FixedPanelTarget {
            panel: panel_with_slot_delta(InstrumentSlot::EReasoning, 0.0),
        }),
        Arc::new(DeterministicOracleHead),
        Arc::new(DeterministicConstellationGuard::default()),
        Arc::new(PatchWitnessReader),
        store.clone(),
        calibration,
        PathBuf::from("/zfs/archive/contextgraph/test-repo"),
    )?;
    let (patch, context) = sample_patch_context("missing_per_slot_calibrator", session_id)?;
    let err = match compiler.compile_with_panel(&patch, &context) {
        Ok((prediction, _)) => bail!(
            "missing per-slot calibrator should fail closed, got prediction {:?}",
            prediction.verdict
        ),
        Err(err) => err,
    };
    ensure!(
        err.code() == "MEJEPA_INFER_OOD_PER_SLOT_CALIBRATOR_MISSING",
        "expected MEJEPA_INFER_OOD_PER_SLOT_CALIBRATOR_MISSING, got {}",
        err.code()
    );
    let after_live_prediction_rows = store.read_live_predictions(session_id, 1000)?.len();
    ensure!(
        after_live_prediction_rows == before_live_prediction_rows,
        "missing per-slot calibrator must not write live predictions"
    );

    Ok(json!({
        "case": "missing_per_slot_calibrator",
        "before_active_calibration_version": active_before.version.clone(),
        "before_per_slot_sigma_count": before_per_slot_sigma_count,
        "after_active_calibration_version": active_after.version.clone(),
        "after_per_slot_sigma_present": active_after.per_slot_sigma_squared.is_some(),
        "error_code": err.code(),
        "error_message": err.to_string(),
        "before_live_prediction_rows": before_live_prediction_rows,
        "after_live_prediction_rows": after_live_prediction_rows,
        "cf_readback_no_live_prediction_written": after_live_prediction_rows == before_live_prediction_rows,
    }))
}

fn sample_patch_context(name: &str, session_id: [u8; 16]) -> Result<(PatchBundle, TaskContext)> {
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
        format!("task py g 136 {name}"),
        sha256_bytes(format!("task-py-g-136-{name}").as_bytes()),
    )?;
    let context = TaskContext {
        task_id: TaskId(format!("task-py-g-136-{name}")),
        session_id,
        language: Language::Python,
        problem_statement: "verify slot-preserving OOD aggregation".to_string(),
        tests: vec![TestId("tests/test_example.py::test_answer".to_string())],
        environment: TaskEnvironment {
            repo_root: PathBuf::from("/zfs/archive/contextgraph/test-repo"),
            python_version: Some("3.11".to_string()),
            os: "linux".to_string(),
        },
        claim_graph: None,
        skill_citations: vec![],
    };
    Ok((patch, context))
}

fn panel_with_slot_delta(slot: InstrumentSlot, delta: f32) -> Panel {
    let mut data = vec![0.0_f32; PANEL_DIM];
    data[slot.offset()] = delta;
    Panel::try_new(data, (1u16 << InstrumentSlot::all().len()) - 1)
        .expect("slot-delta panel is valid")
}

fn panel_with_all_slot_deltas(delta: f32) -> Panel {
    let mut data = vec![0.0_f32; PANEL_DIM];
    for slot in InstrumentSlot::all() {
        data[slot.offset()] = delta;
    }
    Panel::try_new(data, (1u16 << InstrumentSlot::all().len()) - 1).expect("diffuse panel is valid")
}

fn report_markdown(report: &Value) -> String {
    format!(
        "# TASK-PY-G-136 Slot-Preserving OOD FSV\n\n- run_dir: `{}`\n- calibration_cf: `{}`\n- live_prediction_cf: `{}`\n- calibration per-slot sigma count: `{}`\n- single_slot verdict: `{}`\n- single_slot readback: `{}`\n- diffuse verdict: `{}`\n- diffuse readback: `{}`\n- missing per-slot calibrator error: `{}`\n- missing per-slot write blocked: `{}`\n",
        report["run_dir"].as_str().unwrap_or_default(),
        report["calibration_cf"].as_str().unwrap_or_default(),
        report["live_prediction_cf"].as_str().unwrap_or_default(),
        report["calibration_readback"]["active_per_slot_sigma_count"]
            .as_u64()
            .unwrap_or_default(),
        report["single_slot"]["verdict"]
            .as_str()
            .unwrap_or_default(),
        report["single_slot"]["cf_readback_equal"]
            .as_bool()
            .unwrap_or(false),
        report["diffuse"]["verdict"].as_str().unwrap_or_default(),
        report["diffuse"]["cf_readback_equal"]
            .as_bool()
            .unwrap_or(false),
        report["missing_per_slot_calibrator"]["error_code"]
            .as_str()
            .unwrap_or_default(),
        report["missing_per_slot_calibrator"]["cf_readback_no_live_prediction_written"]
            .as_bool()
            .unwrap_or(false)
    )
}
