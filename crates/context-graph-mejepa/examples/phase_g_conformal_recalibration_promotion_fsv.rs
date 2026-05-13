use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{ensure, Context, Result};
use clap::Parser;
use context_graph_mejepa::heal::{
    decode_value, encode_active_pointer_key, write_json_atomic_readback, AbcPromoter,
    ActivePointerValue, HealError, HealRocksStore, HoldoutDataset, HoldoutExample, PromotionGate,
    PromotionLockState, RetrainPromoteRequest, TriggerReason, WitnessChainAppender,
    CF_MEJEPA_ACTIVE_POINTERS, CF_MEJEPA_CALIBRATION_HISTORY,
};
use context_graph_mejepa::{CalibrationRecord, OracleOutcome};
use rocksdb::IteratorMode;
use serde_json::{json, Value};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        default_value = "/mnt/d/contextgraph/fsv/phase-g-conformal-recalibration-fsv"
    )]
    output_root: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let report = run(&args.output_root)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run(output_root: &Path) -> Result<Value> {
    if output_root.exists() {
        fs::remove_dir_all(output_root)
            .with_context(|| format!("remove {}", output_root.display()))?;
    }
    fs::create_dir_all(output_root).with_context(|| format!("create {}", output_root.display()))?;

    let report_path = output_root.join("conformal_recalibration_promotion_fsv.json");
    let storage = HealRocksStore::open(output_root.join("rocksdb"))?;
    let lock = Arc::new(Mutex::new(PromotionLockState::default()));
    let mut witness_chain = WitnessChainAppender::new(output_root.join("witness-chain.bin"))?;
    let mut promoter = AbcPromoter::try_new(0.1, PromotionGate::default())?;
    let current_weights = vec![0.10; 16];

    let cases = [
        ("accepted_90pct", 72usize, true, 0.90f32),
        ("accepted_91pct", 73usize, true, 0.91f32),
        ("blocked_93pct", 75usize, false, 0.93f32),
    ];

    let mut case_reports = Vec::new();
    for (idx, (name, covered, expect_promoted, expected_empirical)) in cases.iter().enumerate() {
        let active_before = active_weights_pointer(&storage)?;
        let calibration_count_before = storage.count_cf(CF_MEJEPA_CALIBRATION_HISTORY)?;
        let result = promoter.retrain_and_promote(RetrainPromoteRequest {
            trigger_reason: TriggerReason::DriftHard,
            current_weights: &current_weights,
            storage: storage.clone(),
            witness_chain: &mut witness_chain,
            holdout: holdout_with_base_coverage(*covered, 100)?,
            lock: lock.clone(),
            calibration_version: "phase-g-before-promotion",
        });
        let active_after = active_weights_pointer(&storage)?;
        let calibration_count_after = storage.count_cf(CF_MEJEPA_CALIBRATION_HISTORY)?;
        let promoted = result.is_ok();
        let error_code = result.as_ref().err().map(HealError::code);
        let empirical = match &result {
            Ok(_) => promoter
                .fsv_snapshot()
                .map(|evidence| evidence.conformal_recalibration.empirical_coverage),
            Err(HealError::ConformalCoverageOutOfBand {
                empirical_coverage, ..
            }) => Some(*empirical_coverage),
            Err(_) => None,
        };
        case_reports.push(json!({
            "case": name,
            "attempt_index": idx,
            "covered_examples": covered,
            "expected_promoted": expect_promoted,
            "promoted": promoted,
            "error_code": error_code,
            "expected_empirical_coverage": expected_empirical,
            "empirical_coverage": empirical,
            "calibration_rows_before": calibration_count_before,
            "calibration_rows_after": calibration_count_after,
            "active_pointer_changed": active_before != active_after,
        }));
        ensure!(
            promoted == *expect_promoted,
            "case {name} promotion outcome mismatch"
        );
        ensure!(
            approx_eq(empirical.unwrap_or(-1.0), *expected_empirical, 0.0001),
            "case {name} empirical coverage mismatch"
        );
        if !expect_promoted {
            ensure!(
                error_code == Some("MEJEPA_CONFORMAL_COVERAGE_OUT_OF_BAND"),
                "blocked case did not return MEJEPA_CONFORMAL_COVERAGE_OUT_OF_BAND"
            );
            ensure!(
                active_before == active_after,
                "blocked case changed active weights pointer"
            );
        }
    }

    let records = calibration_records(&storage)?;
    let active_weights = active_weights_pointer(&storage)?;
    let active_calibration = active_calibration_pointer(&storage)?;
    let last_accepted_calibration = records
        .iter()
        .find(|record| approx_eq(record.empirical_coverage, 0.91, 0.0001));
    let assertions = json!({
        "three_calibration_results_persisted": records.len() == 3,
        "target_coverage_persisted_090": records.iter().all(|record| approx_eq(record.target_coverage, 0.90, 0.0001)),
        "empirical_coverage_measured": contains_coverage(&records, 0.90)
            && contains_coverage(&records, 0.91)
            && contains_coverage(&records, 0.93),
        "third_blocked_with_required_code": case_reports[2]["error_code"] == "MEJEPA_CONFORMAL_COVERAGE_OUT_OF_BAND",
        "third_blocked_before_active_weight_flip": case_reports[2]["active_pointer_changed"] == false,
        "accepted_promotions_flipped_active_weights": case_reports[0]["active_pointer_changed"] == true
            && case_reports[1]["active_pointer_changed"] == true,
        "active_calibration_points_to_last_accepted_record": active_calibration.as_ref().map(|pointer| {
            last_accepted_calibration.map(|record| {
                String::from_utf8_lossy(&pointer.theta_sha_or_version).contains(&record.version)
            }).unwrap_or(false)
        }).unwrap_or(false),
        "active_weights_pointer_exists": active_weights.is_some(),
    });
    ensure!(
        assertions
            .as_object()
            .unwrap()
            .values()
            .all(|value| value.as_bool() == Some(true)),
        "one or more FSV assertions failed: {assertions}"
    );

    let report = json!({
        "fsv": "phase_g_conformal_recalibration_promotion",
        "all_passed": true,
        "artifact_path": report_path,
        "build_release_sha": git_sha(),
        "source_of_truth_cf": CF_MEJEPA_CALIBRATION_HISTORY,
        "cases": case_reports,
        "calibration_history": records.iter().map(|record| json!({
            "version": record.version,
            "alpha": record.alpha,
            "target_coverage": record.target_coverage,
            "empirical_coverage": record.empirical_coverage,
            "tau": record.tau,
            "sample_count": record.sample_count,
        })).collect::<Vec<_>>(),
        "assertions": assertions,
    });
    write_json_atomic_readback(&report_path, &report)?;
    Ok(report)
}

fn holdout_with_base_coverage(covered: usize, total: usize) -> Result<HoldoutDataset> {
    ensure!(covered <= total, "covered count cannot exceed total");
    let examples = (0..total)
        .map(|idx| {
            let is_covered = idx < covered;
            HoldoutExample {
                predicted: vec![OracleOutcome::Pass],
                actual: if is_covered {
                    OracleOutcome::Pass
                } else {
                    OracleOutcome::Fail
                },
                ood_score: if is_covered { 0.05 } else { 0.15 },
                calibration_nonconformity_score: if is_covered { 0.05 } else { 0.95 },
                cell_key: "global_or_per_cell".to_string(),
            }
        })
        .collect();
    Ok(HoldoutDataset::try_new(examples, [7; 32])?)
}

fn calibration_records(storage: &HealRocksStore) -> Result<Vec<CalibrationRecord>> {
    let db = storage.db();
    let cf = db
        .cf_handle(CF_MEJEPA_CALIBRATION_HISTORY)
        .context("missing calibration history CF")?;
    let mut records = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        records.push(bincode::deserialize::<CalibrationRecord>(&value)?);
    }
    records.sort_by_key(|record| record.frozen_at);
    Ok(records)
}

fn active_weights_pointer(storage: &HealRocksStore) -> Result<Option<Vec<u8>>> {
    Ok(storage.get_cf(
        CF_MEJEPA_ACTIVE_POINTERS,
        &encode_active_pointer_key("active_weights")?,
    )?)
}

fn active_calibration_pointer(storage: &HealRocksStore) -> Result<Option<ActivePointerValue>> {
    let Some(bytes) = storage.get_cf(
        CF_MEJEPA_ACTIVE_POINTERS,
        &encode_active_pointer_key("active_calibration")?,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(decode_value(&bytes)?))
}

fn approx_eq(left: f32, right: f32, epsilon: f32) -> bool {
    (left - right).abs() <= epsilon
}

fn contains_coverage(records: &[CalibrationRecord], expected: f32) -> bool {
    records
        .iter()
        .any(|record| approx_eq(record.empirical_coverage, expected, 0.0001))
}

fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}
