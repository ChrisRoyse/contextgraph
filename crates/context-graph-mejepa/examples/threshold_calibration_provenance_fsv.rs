use context_graph_mejepa::{
    audit_threshold_calibration_provenance_db_with_config,
    require_threshold_calibration_ship_eligible, write_threshold_calibration_provenance,
    ThresholdCalibrationAuditConfig, ThresholdCalibrationProvenanceRecord,
    ThresholdPartitionMember, ThresholdSurfaceKind, THRESHOLD_CALIBRATION_NONDISJOINT,
    THRESHOLD_CALIBRATION_SCHEMA_VERSION,
};
use context_graph_mejepa_cf::{
    ALL_HYGIENE_REFERENCED_CFS, CF_MEJEPA_ACCEPTED_LABEL_REGISTRY, CF_MEJEPA_CONSTELLATION,
    CF_MEJEPA_FAILURE_FINGERPRINTS, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
    CF_MEJEPA_HEAD_CALIBRATIONS, CF_MEJEPA_OOD_CALIBRATIONS, CF_MEJEPA_Q5_CALIBRATIONS,
    CF_MEJEPA_THRESHOLD_CALIBRATION_PROVENANCE,
};
use rocksdb::{ColumnFamilyDescriptor, Options, WriteOptions, DB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const NOW: i64 = 1_779_400_000_000;
const LABEL_REGISTRY_HASH: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const USEFULNESS_METRICS_HASH: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn main() -> FsvResult<()> {
    let fsv_root = env_path(
        "CG_THRESHOLD_CALIBRATION_FSV_ROOT",
        "/zfs/archive/contextgraph/fsv/threshold-calibration-provenance-fsv",
    );
    let run_dir = fsv_root.join(format!(
        "run-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    std::fs::create_dir_all(&run_dir)?;

    let pass_db_path = run_dir.join("threshold-calibration-pass.rocksdb");
    let missing_source_db_path = run_dir.join("threshold-calibration-missing-source.rocksdb");
    let config = ThresholdCalibrationAuditConfig::from_expected_lineage(
        LABEL_REGISTRY_HASH,
        USEFULNESS_METRICS_HASH,
    );

    let pass_case = passing_db_case(&pass_db_path, &config)?;
    let missing_source_case = missing_source_case(&missing_source_db_path, &config)?;
    let fail_closed_cases = fail_closed_cases(&config)?;
    let readback = independent_readback(&pass_db_path, &config)?;
    let no_rows = {
        let empty_db_path = run_dir.join("threshold-calibration-empty.rocksdb");
        let db = open_db(&empty_db_path)?;
        let report = audit_threshold_calibration_provenance_db_with_config(&db, Some(&config))?;
        let ship_error = require_threshold_calibration_ship_eligible(&report).unwrap_err();
        json!({
            "passed": !report.all_passed && ship_error.to_string().contains(THRESHOLD_CALIBRATION_NONDISJOINT),
            "failureCount": report.failures.len(),
            "shipEligibilityError": ship_error.to_string(),
        })
    };

    let all_passed = pass_case["passed"].as_bool() == Some(true)
        && missing_source_case["passed"].as_bool() == Some(true)
        && no_rows["passed"].as_bool() == Some(true)
        && fail_closed_cases.as_object().map(|cases| {
            cases
                .values()
                .all(|case| case["passed"].as_bool() == Some(true))
        }) == Some(true)
        && readback["allPassed"].as_bool() == Some(true);

    let report = json!({
        "artifactKind": "threshold_calibration_provenance_fsv",
        "task": "TASK-PY-G-113",
        "coveredGithubIssues": [407],
        "schemaVersion": THRESHOLD_CALIBRATION_SCHEMA_VERSION,
        "allPassed": all_passed,
        "sourceOfTruth": {
            "fsvRoot": run_dir,
            "thresholdProvenanceCf": CF_MEJEPA_THRESHOLD_CALIBRATION_PROVENANCE,
            "auditedCfs": [
                CF_MEJEPA_FAILURE_FINGERPRINTS,
                CF_MEJEPA_OOD_CALIBRATIONS,
                CF_MEJEPA_HEAD_CALIBRATIONS,
                CF_MEJEPA_Q5_CALIBRATIONS,
                CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
                CF_MEJEPA_CONSTELLATION,
                CF_MEJEPA_ACCEPTED_LABEL_REGISTRY
            ],
            "labelRegistryHash": LABEL_REGISTRY_HASH,
            "usefulnessMetricsHash": USEFULNESS_METRICS_HASH,
        },
        "cases": {
            "passingDisjointP10Rows": pass_case,
            "missingSourceThresholdRowFailsClosed": missing_source_case,
            "emptyProvenanceFailsClosed": no_rows,
            "failClosedEdges": fail_closed_cases
        },
        "independentReadback": readback,
        "activePathPolicy": {
            "mntDActiveReadsOrWrites": false
        }
    });
    let report_path = run_dir.join("threshold_calibration_provenance_fsv.json");
    context_graph_mejepa::eval::report::write_json_0600(&report_path, &report)?;
    let readback_path = run_dir.join("threshold_calibration_provenance_readback.json");
    let report_readback: Value = serde_json::from_slice(&std::fs::read(&report_path)?)?;
    let readback_report = json!({
        "reportPath": report_path,
        "allPassed": report_readback["allPassed"],
        "reportSha256": sha256_file(&report_path)?,
        "readbackAllPassed": report_readback["allPassed"] == true,
    });
    context_graph_mejepa::eval::report::write_json_0600(&readback_path, &readback_report)?;
    if report_readback["allPassed"] != true {
        return Err(format!("TASK-PY-G-113 FSV failed at {}", report_path.display()).into());
    }
    println!(
        "threshold calibration provenance FSV allPassed=true report={} report_sha256={} readback={} readback_sha256={}",
        report_path.display(),
        sha256_file(&report_path)?,
        readback_path.display(),
        sha256_file(&readback_path)?,
    );
    Ok(())
}

fn passing_db_case(db_path: &Path, config: &ThresholdCalibrationAuditConfig) -> FsvResult<Value> {
    let db = open_db(db_path)?;
    let records = passing_records();
    for record in &records {
        seed_source_row(&db, &record.source_cf, &record.source_row_key)?;
        write_threshold_calibration_provenance(&db, record)?;
    }
    drop(db);

    let reopened = open_db(db_path)?;
    let report = audit_threshold_calibration_provenance_db_with_config(&reopened, Some(config))?;
    let ship_eligible = require_threshold_calibration_ship_eligible(&report).is_ok();
    Ok(json!({
        "passed": report.all_passed && ship_eligible && report.threshold_rows_audited == records.len(),
        "shipEligible": ship_eligible,
        "thresholdRowsAudited": report.threshold_rows_audited,
        "sourceCfsAudited": report.source_cfs_audited,
        "failures": report.failures,
    }))
}

fn missing_source_case(
    db_path: &Path,
    config: &ThresholdCalibrationAuditConfig,
) -> FsvResult<Value> {
    let db = open_db(db_path)?;
    let record = base_record(
        ThresholdSurfaceKind::FailureFingerprint,
        CF_MEJEPA_FAILURE_FINGERPRINTS,
        "fingerprint:missing-source",
        "gtau:e1",
        0.70,
    );
    write_threshold_calibration_provenance(&db, &record)?;
    let report = audit_threshold_calibration_provenance_db_with_config(&db, Some(config))?;
    let ship_error = require_threshold_calibration_ship_eligible(&report).unwrap_err();
    Ok(json!({
        "passed": !report.all_passed && ship_error.to_string().contains(THRESHOLD_CALIBRATION_NONDISJOINT),
        "failureCount": report.failures.len(),
        "firstFailure": report.failures.first(),
        "shipEligibilityError": ship_error.to_string(),
    }))
}

fn fail_closed_cases(config: &ThresholdCalibrationAuditConfig) -> FsvResult<Value> {
    let mut missing_partition = base_record(
        ThresholdSurfaceKind::FailureFingerprint,
        CF_MEJEPA_FAILURE_FINGERPRINTS,
        "fingerprint:missing-partition",
        "gtau:e1",
        0.70,
    );
    missing_partition.calibration_partition_id.clear();

    let mut train_overlap = base_record(
        ThresholdSurfaceKind::OodCalibration,
        CF_MEJEPA_OOD_CALIBRATIONS,
        "ood:train-overlap",
        "tau_ood",
        0.70,
    );
    train_overlap.train_members = train_overlap.calibration_members.clone();

    let mut eval_overlap = base_record(
        ThresholdSurfaceKind::HeadCalibration,
        CF_MEJEPA_HEAD_CALIBRATIONS,
        "head:eval-overlap",
        "tau:q2",
        0.70,
    );
    eval_overlap.eval_members = eval_overlap.calibration_members.clone();

    let wrong_p10 = base_record(
        ThresholdSurfaceKind::Constellation,
        CF_MEJEPA_CONSTELLATION,
        "constellation:wrong-p10",
        "tau:e1",
        0.95,
    );

    let mut target_leakage = base_record(
        ThresholdSurfaceKind::FailureModeSkill,
        CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
        "skill:target-leakage",
        "confidence_cutoff",
        0.70,
    );
    target_leakage
        .live_input_label_ids
        .push("oracle:pass".to_string());

    let mut stale_hash = base_record(
        ThresholdSurfaceKind::LabelRegistry,
        CF_MEJEPA_ACCEPTED_LABEL_REGISTRY,
        "label:stale-hash",
        "usefulness_lift_floor",
        0.70,
    );
    stale_hash.label_registry_hash = "c".repeat(64);

    Ok(json!({
        "missingPartitionId": fail_case(&[missing_partition], config),
        "trainOverlap": fail_case(&[train_overlap], config),
        "evalOverlap": fail_case(&[eval_overlap], config),
        "wrongP10": fail_case(&[wrong_p10], config),
        "targetSideLabelLeakage": fail_case(&[target_leakage], config),
        "staleLabelRegistryHash": fail_case(&[stale_hash], config),
    }))
}

fn fail_case(
    records: &[ThresholdCalibrationProvenanceRecord],
    config: &ThresholdCalibrationAuditConfig,
) -> Value {
    let report = context_graph_mejepa::audit_threshold_calibration_provenance_with_config(
        records,
        Some(config),
    );
    let ship_error = require_threshold_calibration_ship_eligible(&report).unwrap_err();
    json!({
        "passed": !report.all_passed && ship_error.to_string().contains(THRESHOLD_CALIBRATION_NONDISJOINT),
        "failureCount": report.failures.len(),
        "firstFailure": report.failures.first(),
        "shipEligibilityError": ship_error.to_string(),
    })
}

fn independent_readback(
    db_path: &Path,
    config: &ThresholdCalibrationAuditConfig,
) -> FsvResult<Value> {
    let db = open_db(db_path)?;
    let report = audit_threshold_calibration_provenance_db_with_config(&db, Some(config))?;
    let cfs = report
        .source_cfs_audited
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    Ok(json!({
        "allPassed": report.all_passed
            && report.threshold_rows_audited == passing_records().len()
            && cfs.contains(CF_MEJEPA_FAILURE_FINGERPRINTS)
            && cfs.contains(CF_MEJEPA_OOD_CALIBRATIONS)
            && cfs.contains(CF_MEJEPA_HEAD_CALIBRATIONS)
            && cfs.contains(CF_MEJEPA_Q5_CALIBRATIONS)
            && cfs.contains(CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS)
            && cfs.contains(CF_MEJEPA_CONSTELLATION)
            && cfs.contains(CF_MEJEPA_ACCEPTED_LABEL_REGISTRY),
        "thresholdRowsAudited": report.threshold_rows_audited,
        "sourceCfsAudited": report.source_cfs_audited,
        "failures": report.failures,
    }))
}

fn passing_records() -> Vec<ThresholdCalibrationProvenanceRecord> {
    vec![
        base_record(
            ThresholdSurfaceKind::FailureFingerprint,
            CF_MEJEPA_FAILURE_FINGERPRINTS,
            "fingerprint:api-contract",
            "gtau:e1",
            0.70,
        ),
        base_record(
            ThresholdSurfaceKind::OodCalibration,
            CF_MEJEPA_OOD_CALIBRATIONS,
            "ood:python-bool-flip",
            "tau_ood",
            0.70,
        ),
        base_record(
            ThresholdSurfaceKind::HeadCalibration,
            CF_MEJEPA_HEAD_CALIBRATIONS,
            "head:q2-oracle",
            "tau:q2",
            0.70,
        ),
        base_record(
            ThresholdSurfaceKind::HeadCalibration,
            CF_MEJEPA_Q5_CALIBRATIONS,
            "q5:shift-event",
            "tau:q5",
            0.70,
        ),
        base_record(
            ThresholdSurfaceKind::FailureModeSkill,
            CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
            "skill:patch-breaks-test-isolation",
            "confidence_cutoff",
            0.70,
        ),
        base_record(
            ThresholdSurfaceKind::Constellation,
            CF_MEJEPA_CONSTELLATION,
            "constellation:python-wrong-file",
            "tau:e1",
            0.70,
        ),
        base_record(
            ThresholdSurfaceKind::LabelRegistry,
            CF_MEJEPA_ACCEPTED_LABEL_REGISTRY,
            "label:api",
            "usefulness_lift_floor",
            0.70,
        ),
    ]
}

fn base_record(
    surface: ThresholdSurfaceKind,
    source_cf: &str,
    source_row_key: &str,
    threshold_name: &str,
    tau: f32,
) -> ThresholdCalibrationProvenanceRecord {
    ThresholdCalibrationProvenanceRecord {
        schema_version: THRESHOLD_CALIBRATION_SCHEMA_VERSION,
        surface,
        source_cf: source_cf.to_string(),
        source_row_key: source_row_key.to_string(),
        threshold_name: threshold_name.to_string(),
        tau,
        calibration_partition_id: "partition:calibration".to_string(),
        train_partition_id: "partition:train".to_string(),
        eval_partition_id: "partition:eval".to_string(),
        calibration_members: vec![
            member("row:calibration:1", "state:calibration:1"),
            member("row:calibration:2", "state:calibration:2"),
        ],
        train_members: vec![member("row:train:1", "state:train:1")],
        eval_members: vec![member("row:eval:1", "state:eval:1")],
        authentic_sample_cosines: vec![0.70, 0.72, 0.74, 0.76, 0.78],
        live_input_label_ids: vec!["label:api".to_string(), "label:slot:e1".to_string()],
        target_side_label_ids: vec!["oracle:pass".to_string(), "docker:passed".to_string()],
        label_registry_hash: LABEL_REGISTRY_HASH.to_string(),
        usefulness_metrics_hash: USEFULNESS_METRICS_HASH.to_string(),
        created_at_unix_ms: NOW,
    }
}

fn member(row_key: &str, code_state_key: &str) -> ThresholdPartitionMember {
    ThresholdPartitionMember {
        row_key: row_key.to_string(),
        accepted_label_id: Some("label:api".to_string()),
        code_state_key: code_state_key.to_string(),
    }
}

fn seed_source_row(db: &DB, cf_name: &str, key: &str) -> FsvResult<()> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| format!("missing CF {cf_name}"))?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key.as_bytes(), b"source-threshold-row", &opts)?;
    db.flush_cf(cf)?;
    Ok(())
}

fn open_db(path: &Path) -> FsvResult<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let descriptors = ALL_HYGIENE_REFERENCED_CFS
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
        .collect::<Vec<_>>();
    Ok(DB::open_cf_descriptors(&opts, path, descriptors)?)
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
