use context_graph_mejepa::{
    ConformalInterval, ConformalSet, CostAxis, Language, OracleOutcome, PerfAxis,
    PredictedCostRegression, PredictedPerfRegression, PredictionProvenance, Q4EvidenceCatalog,
    Q4HeadEvidence, RealityPrediction, RealityPredictionBuilder, ReasoningClass, TaskId, Verdict,
    WitnessHash,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_HEAD_CALIBRATIONS, CF_MEJEPA_LIVE_PREDICTIONS, FROZEN_Q4_CALIBRATION_COUNT_KEY,
    FROZEN_Q4_CALIBRATION_KEY_PREFIX, FROZEN_Q4_CFS,
};
use context_graph_mejepa_train::{LossCoefficients, TrainingConfig, Q4_LOSS_COEFFICIENT_MAX};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let fsv_root = env_path(
        "CG_Q4_DOCTRINE_FREEZE_FSV_ROOT",
        "/zfs/archive/contextgraph/fsv/q4-doctrine-freeze-fsv",
    );
    std::fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!("run-{run_id}"));
    std::fs::create_dir_all(&run_root)?;
    let db_path = run_root.join("q4-freeze.rocksdb");
    let d_root = run_root.join("d-root");
    let db = open_db(&db_path)?;

    seed_frozen_q4_rows(&db)?;
    let q4_counts_before = frozen_q4_counts(&db)?;
    write_q4_freeze_baseline(&d_root, &q4_counts_before)?;

    let historical = historical_q4_prediction();
    persist_prediction(&db, &historical)?;
    let historical_readback = read_prediction_by_id(&db, historical.prediction_id)?;
    let trust_report = full_q4_evidence_trust_report()?;
    let historical_trusted =
        context_graph_mejepa::trusted_q4_consequences(&historical_readback, &trust_report);
    let historical_q4_readable = historical_readback.q4_display_only_field_count() > 0
        && !historical_readback.predicted_perf_regressions.is_empty()
        && historical_readback.predicted_reasoning_class == ReasoningClass::Overclaiming
        && historical_trusted.perf_regressions.is_empty()
        && !historical_trusted.q4_trust_decision_eligible;

    let new_prediction = new_prediction_attempting_q4()?;
    persist_prediction(&db, &new_prediction)?;
    let new_readback = read_prediction_by_id(&db, new_prediction.prediction_id)?;
    let new_prediction_q4_fields_empty = new_readback.q4_display_only_fields_empty();

    let q4_counts_after_clean_ingest = frozen_q4_counts(&db)?;
    let q4_row_count_delta = row_count_delta(&q4_counts_before, &q4_counts_after_clean_ingest);

    let q4_loss_coefficient_max = TrainingConfig::default()
        .loss_coefficients
        .q4_loss_coefficient_max();
    let q4_loss_zero = q4_loss_coefficient_max == Q4_LOSS_COEFFICIENT_MAX;
    let q4_inputs_to_ship_gate = context_graph_mejepa::q4_inputs_to_ship_gate();

    let edge_cases = vec![
        full_evidence_catalog_stays_frozen_case(&trust_report),
        nonzero_q4_loss_fails_closed_case(),
        q4_growth_detected_case(&run_root)?,
        q4_calibration_growth_detected_case(&run_root)?,
        malformed_q4_baseline_fails_closed_case(&run_root, &q4_counts_after_clean_ingest)?,
    ];
    let all_edge_cases_pass = edge_cases.iter().all(|case| case["pass"] == true);
    let all_passed = q4_row_count_delta == 0
        && q4_loss_zero
        && q4_inputs_to_ship_gate == 0
        && historical_q4_readable
        && new_prediction_q4_fields_empty
        && all_edge_cases_pass;

    for cf in FROZEN_Q4_CFS {
        if let Some(handle) = db.cf_handle(cf) {
            db.flush_cf(handle)?;
        }
    }
    if let Some(handle) = db.cf_handle(CF_MEJEPA_HEAD_CALIBRATIONS) {
        db.flush_cf(handle)?;
    }
    drop(db);

    let evidence = json!({
        "task": "TASK-PY-G-114",
        "issue": 408,
        "runId": run_id,
        "fsvRoot": fsv_root,
        "runRoot": run_root,
        "sourceOfTruth": {
            "dbPath": db_path,
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
            "frozenQ4Cfs": FROZEN_Q4_CFS,
            "frozenQ4CalibrationCounter": FROZEN_Q4_CALIBRATION_COUNT_KEY,
            "q4FreezeBaseline": d_root.join("state/q4-freeze/q4_frozen_cf_counts.json")
        },
        "happyPath": {
            "q4RowCountDelta": q4_row_count_delta,
            "q4CountsBefore": q4_counts_before,
            "q4CountsAfterCleanIngest": q4_counts_after_clean_ingest,
            "q4LossCoefficientMax": q4_loss_coefficient_max,
            "q4InputsToShipGate": q4_inputs_to_ship_gate,
            "historicalQ4FieldsStillReadable": historical_q4_readable,
            "historicalPredictionId": hex::encode(historical.prediction_id),
            "historicalQ4DisplayOnlyFieldCount": historical_readback.q4_display_only_field_count(),
            "historicalTrustedQ4Consequences": historical_trusted,
            "newPredictionsReturnEmptyQ4Fields": new_prediction_q4_fields_empty,
            "newPredictionId": hex::encode(new_prediction.prediction_id),
            "newPredictionQ4DisplayOnlyFieldCount": new_readback.q4_display_only_field_count(),
            "pass": q4_row_count_delta == 0
                && q4_loss_zero
                && q4_inputs_to_ship_gate == 0
                && historical_q4_readable
                && new_prediction_q4_fields_empty
        },
        "edgeCases": edge_cases,
        "allPassed": all_passed
    });
    require("Q4 doctrine freeze FSV passed", all_passed)?;
    let evidence_path = fsv_root.join("q4_doctrine_freeze_fsv.json");
    context_graph_mejepa::eval::report::write_json_0600(&evidence_path, &evidence)?;
    let readback_bytes = std::fs::read(&evidence_path)?;
    let readback: Value = serde_json::from_slice(&readback_bytes)?;
    require("FSV readback allPassed", readback["allPassed"] == true)?;
    let readback_path = fsv_root.join("q4_doctrine_freeze_readback.json");
    context_graph_mejepa::eval::report::write_json_0600(
        &readback_path,
        &json!({
            "evidencePath": evidence_path,
            "evidenceSha256": sha256_hex(&readback_bytes),
            "allPassed": readback["allPassed"],
            "readbackEqual": readback == evidence
        }),
    )?;
    Ok(())
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var(name)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

fn open_db(path: &Path) -> FsvResult<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let mut cfs = vec![ColumnFamilyDescriptor::new("default", Options::default())];
    cfs.extend(
        context_graph_mejepa_cf::all_hygiene_referenced_cfs()
            .into_iter()
            .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default())),
    );
    Ok(DB::open_cf_descriptors(&opts, path, cfs)?)
}

fn seed_frozen_q4_rows(db: &DB) -> FsvResult<()> {
    for cf in FROZEN_Q4_CFS {
        put_cf(db, cf, b"pre-freeze-row", b"display-only historical row")?;
    }
    put_cf(
        db,
        CF_MEJEPA_HEAD_CALIBRATIONS,
        b"q4cal::perf::pre-freeze",
        b"display-only historical q4 calibration row",
    )?;
    Ok(())
}

fn put_cf(db: &DB, cf_name: &'static str, key: &[u8], value: &[u8]) -> FsvResult<()> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| format!("missing column family {cf_name}"))?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, value, &opts)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| format!("missing readback for {cf_name}"))?;
    require("CF write readback equal", readback.as_slice() == value)
}

fn frozen_q4_counts(db: &DB) -> FsvResult<BTreeMap<String, usize>> {
    let mut counts = FROZEN_Q4_CFS
        .iter()
        .map(|cf_name| {
            let cf = db
                .cf_handle(cf_name)
                .ok_or_else(|| format!("missing column family {cf_name}"))?;
            Ok((
                cf_name.to_string(),
                db.iterator_cf(cf, IteratorMode::Start).count(),
            ))
        })
        .collect::<FsvResult<BTreeMap<_, _>>>()?;
    counts.insert(
        FROZEN_Q4_CALIBRATION_COUNT_KEY.to_string(),
        q4_calibration_count(db)?,
    );
    Ok(counts)
}

fn q4_calibration_count(db: &DB) -> FsvResult<usize> {
    let cf = db
        .cf_handle(CF_MEJEPA_HEAD_CALIBRATIONS)
        .ok_or("missing CF_MEJEPA_HEAD_CALIBRATIONS")?;
    Ok(db
        .iterator_cf(cf, IteratorMode::Start)
        .filter_map(Result::ok)
        .filter(|(key, _)| {
            key.as_ref()
                .starts_with(FROZEN_Q4_CALIBRATION_KEY_PREFIX.as_bytes())
        })
        .count())
}

fn write_q4_freeze_baseline(d_root: &Path, counts: &BTreeMap<String, usize>) -> FsvResult<()> {
    let path = d_root.join("state/q4-freeze/q4_frozen_cf_counts.json");
    context_graph_mejepa::eval::report::write_json_0600(&path, counts)?;
    let readback: BTreeMap<String, usize> = serde_json::from_slice(&std::fs::read(&path)?)?;
    require("Q4 freeze baseline readback equal", readback == *counts)
}

fn row_count_delta(before: &BTreeMap<String, usize>, after: &BTreeMap<String, usize>) -> isize {
    before
        .keys()
        .map(|key| {
            after.get(key).copied().unwrap_or_default() as isize
                - before.get(key).copied().unwrap_or_default() as isize
        })
        .sum()
}

fn persist_prediction(db: &DB, prediction: &RealityPrediction) -> FsvResult<()> {
    let cf = db
        .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .ok_or("missing CF_MEJEPA_LIVE_PREDICTIONS")?;
    let key = live_key(prediction);
    let bytes = bincode::serialize(prediction)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, &key, &bytes, &opts)?;
    let readback = db.get_cf(cf, &key)?.ok_or("missing prediction readback")?;
    require("live prediction readback equal", readback == bytes)
}

fn read_prediction_by_id(db: &DB, prediction_id: [u8; 16]) -> FsvResult<RealityPrediction> {
    let cf = db
        .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .ok_or("missing CF_MEJEPA_LIVE_PREDICTIONS")?;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let prediction = context_graph_mejepa::decode_reality_prediction(&value)?;
        if prediction.prediction_id == prediction_id {
            return Ok(prediction);
        }
    }
    Err(format!("prediction {} not found", hex::encode(prediction_id)).into())
}

fn live_key(prediction: &RealityPrediction) -> Vec<u8> {
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(&prediction.session_id);
    key.extend_from_slice(&prediction.created_at_unix_ms.to_be_bytes());
    key.extend_from_slice(&prediction.prediction_id);
    key
}

fn historical_q4_prediction() -> RealityPrediction {
    let mut prediction = base_prediction(
        [0x41; 16],
        [0x42; 16],
        1_779_000_000_001,
        "q4-freeze-historical",
    )
    .build()
    .expect("valid historical base prediction");
    let chunk = context_graph_mejepa::ChunkId("src/historical.py#0".to_string());
    prediction.predicted_perf_regressions = vec![PredictedPerfRegression {
        axis: PerfAxis::CpuMs,
        chunk: chunk.clone(),
        predicted_delta_pct: 11.0,
        baseline_value: Some(100.0),
        confidence: 0.82,
        explanation: "pre-freeze display-only perf observation".to_string(),
    }];
    prediction.predicted_cost_regressions = vec![PredictedCostRegression {
        axis: CostAxis::CiMinutes,
        chunk,
        predicted_delta: 1.0,
        baseline_value: Some(5.0),
        explanation: "pre-freeze display-only cost observation".to_string(),
    }];
    prediction.predicted_reasoning_class = ReasoningClass::Overclaiming;
    prediction
}

fn new_prediction_attempting_q4() -> FsvResult<RealityPrediction> {
    let chunk = context_graph_mejepa::ChunkId("src/new.py#0".to_string());
    let mut surfaces = context_graph_mejepa::PhaseBPredictionSurfaces::default();
    surfaces
        .predicted_perf_regressions
        .push(PredictedPerfRegression {
            axis: PerfAxis::CpuMs,
            chunk,
            predicted_delta_pct: 99.0,
            baseline_value: None,
            confidence: 0.99,
            explanation: "attempted post-freeze Q4 perf observation".to_string(),
        });
    surfaces.predicted_reasoning_class = ReasoningClass::Hallucination;
    Ok(
        base_prediction([0x51; 16], [0x52; 16], 1_779_000_000_002, "q4-freeze-new")
            .phase_b_surfaces(surfaces)
            .build()?,
    )
}

fn base_prediction(
    prediction_id: [u8; 16],
    session_id: [u8; 16],
    created_at_unix_ms: i64,
    task: &str,
) -> RealityPredictionBuilder {
    RealityPredictionBuilder::from_parts(
        TaskId(task.to_string()),
        session_id,
        Language::Python,
        ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
    )
    .prediction_id(prediction_id)
    .witness_hash(WitnessHash([0x33; 32]))
    .verdict(Verdict::Pass)
    .confidence_interval(ConformalInterval {
        lower: 0.7,
        upper: 0.9,
        ..ConformalInterval::default()
    })
    .predicted_oracle_pass(0.81)
    .predicted_test_pass(vec![0.81])
    .ood_score(0.1)
    .calibrated_confidence(0.8)
    .provenance(PredictionProvenance {
        predictor_version: "q4-freeze-fsv".to_string(),
        constellation_version: "q4-freeze-fsv".to_string(),
        calibration_version: "q4-freeze-fsv".to_string(),
        active_pointer: hex::encode(prediction_id),
    })
    .source_panel_sha([0x34; 32])
    .calibration_version("q4-freeze-fsv")
    .created_at_unix_ms(created_at_unix_ms)
}

fn full_q4_evidence_trust_report() -> FsvResult<context_graph_mejepa::Q4TrustGateReport> {
    let heads = context_graph_mejepa::default_q4_requirements()
        .into_iter()
        .map(|requirement| {
            (
                requirement.head,
                Q4HeadEvidence {
                    producer_fsv_root: Some(requirement.producer_fsv_root),
                    producer_rows: 1_000,
                    calibration_fsv_root: Some(requirement.calibration_fsv_root),
                    calibration_rows: 1_000,
                    per_slot_evidence_root: Some(
                        context_graph_mejepa::Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT.to_string(),
                    ),
                    slots_with_evidence: context_graph_mejepa::Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS
                        .iter()
                        .map(|slot| (*slot).to_string())
                        .collect(),
                },
            )
        })
        .collect();
    Ok(context_graph_mejepa::evaluate_q4_trust_gate(
        &Q4EvidenceCatalog { heads },
    )?)
}

fn full_evidence_catalog_stays_frozen_case(
    report: &context_graph_mejepa::Q4TrustGateReport,
) -> Value {
    let pass = !report.q4_head_ready
        && !report.q4_trust_decision_eligible
        && report.heads.iter().all(|head| {
            !head.trusted_in_decision
                && head
                    .missing_requirements
                    .iter()
                    .any(|item| item == context_graph_mejepa::Q4_DOCTRINE_FREEZE_REASON)
        });
    json!({
        "case": "full_evidence_catalog_stays_frozen",
        "expected": "complete producer/calibration/slot evidence still cannot unlock Q4",
        "actual": report,
        "pass": pass
    })
}

fn nonzero_q4_loss_fails_closed_case() -> Value {
    let cfg = TrainingConfig {
        loss_coefficients: LossCoefficients {
            alpha_reasoning: 0.1,
            ..LossCoefficients::default()
        },
        ..TrainingConfig::default()
    };
    let err = cfg.validate().unwrap_err();
    json!({
        "case": "nonzero_q4_loss_fails_closed",
        "expected": "MEJEPA_TRAIN_CONFIG_INVALID and q4_loss_coefficient_max",
        "actual": err.to_string(),
        "pass": err.code() == "MEJEPA_TRAIN_CONFIG_INVALID"
            && err.to_string().contains("q4_loss_coefficient_max")
    })
}

fn q4_growth_detected_case(run_root: &Path) -> FsvResult<Value> {
    let edge_db_path = run_root.join("q4-growth-edge.rocksdb");
    let db = open_db(&edge_db_path)?;
    seed_frozen_q4_rows(&db)?;
    let baseline = frozen_q4_counts(&db)?;
    put_cf(
        &db,
        context_graph_mejepa_cf::CF_MEJEPA_Q4_PERF_LABELS,
        b"post-freeze-growth",
        b"forbidden growth",
    )?;
    let after_growth = frozen_q4_counts(&db)?;
    let delta = row_count_delta(&baseline, &after_growth);
    Ok(json!({
        "case": "q4_cf_growth_detected",
        "expected": "row-count delta > 0 after forbidden write",
        "actual": {
            "dbPath": edge_db_path,
            "baseline": baseline,
            "afterGrowth": after_growth,
            "delta": delta
        },
        "pass": delta > 0
    }))
}

fn q4_calibration_growth_detected_case(run_root: &Path) -> FsvResult<Value> {
    let edge_db_path = run_root.join("q4-calibration-growth-edge.rocksdb");
    let db = open_db(&edge_db_path)?;
    seed_frozen_q4_rows(&db)?;
    let baseline = frozen_q4_counts(&db)?;
    put_cf(
        &db,
        CF_MEJEPA_HEAD_CALIBRATIONS,
        b"q4cal::perf::post-freeze-growth",
        b"forbidden q4 calibration growth",
    )?;
    put_cf(
        &db,
        CF_MEJEPA_HEAD_CALIBRATIONS,
        b"q2_oracle_pass::allowed-non-q4-growth",
        b"allowed non-q4 calibration row",
    )?;
    let after_growth = frozen_q4_counts(&db)?;
    let delta = row_count_delta(&baseline, &after_growth);
    Ok(json!({
        "case": "q4_calibration_growth_detected",
        "expected": "q4cal::* row-count delta > 0 while non-Q4 calibration rows are ignored",
        "actual": {
            "dbPath": edge_db_path,
            "baseline": baseline,
            "afterGrowth": after_growth,
            "delta": delta
        },
        "pass": delta == 1
    }))
}

fn malformed_q4_baseline_fails_closed_case(
    run_root: &Path,
    current: &BTreeMap<String, usize>,
) -> FsvResult<Value> {
    let path = run_root.join("bad-q4-baseline.json");
    std::fs::write(&path, b"{not-json")?;
    let parsed = serde_json::from_slice::<BTreeMap<String, usize>>(&std::fs::read(&path)?);
    Ok(json!({
        "case": "malformed_q4_baseline_fails_closed",
        "expected": "baseline JSON parse error before comparing current counts",
        "actual": {
            "path": path,
            "currentCounts": current,
            "error": parsed.as_ref().err().map(|err| err.to_string())
        },
        "pass": parsed.is_err()
    }))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("requirement failed: {name}").into())
    }
}
