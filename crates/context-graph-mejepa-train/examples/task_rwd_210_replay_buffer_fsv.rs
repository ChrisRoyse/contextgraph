use context_graph_mejepa::{open_infer_rocksdb, PredictionId};
use context_graph_mejepa_cf::CF_MEJEPA_REPLAY_BUFFER;
use context_graph_mejepa_train::replay_buffer::{
    count_replay_rows, enforce_replay_buffer_quota, prediction_hex, write_replay_row_sync_readback,
    ReplayBufferRow, ReplayBufferSource, ReplayConsolidationWindow, ReplayPriorityConfig,
    ReplayRetentionTier,
};
use context_graph_mejepa_train::sampler::BatchSampler;
use context_graph_mejepa_train::ReplayBatchRequest;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const NOW_MS: i64 = 1_779_049_500_000;
const DAY_MS: i64 = 86_400_000;

fn main() -> Result<()> {
    let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/task-py-g-068-replay-buffer-fsv");
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!(
        "run-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    ));
    fs::create_dir_all(&run_root)?;

    let db_path = run_root.join("infer-db");
    let db = open_infer_rocksdb(&db_path)?;
    let counts_before = count_replay_rows(db.as_ref())?;
    seed_priority_ladder(db.as_ref(), NOW_MS)?;
    let seeded_count = count_replay_rows(db.as_ref())?;

    let happy = BatchSampler::next_replay_batch(
        db.as_ref(),
        ReplayBatchRequest {
            top_k: 20,
            window: ReplayConsolidationWindow::Normal,
            now_unix_ms: NOW_MS + 1_000,
        },
        ReplayPriorityConfig::default(),
    )?;
    let high_ids = (1_u8..=10).map(prediction).collect::<Vec<_>>();
    let first_ten_ids = happy
        .selected_rows
        .iter()
        .take(10)
        .map(|row| row.row.prediction_id)
        .collect::<Vec<_>>();
    let high_readback_counts = high_ids
        .iter()
        .map(|prediction_id| {
            let row = context_graph_mejepa_train::read_replay_row(db.as_ref(), *prediction_id)?
                .ok_or("missing high-priority replay row")?;
            Ok((prediction_hex(*prediction_id), row.replay_count))
        })
        .collect::<Result<Vec<_>>>()?;

    let empty_case = run_empty_case(&run_root)?;
    let quota_case = run_quota_case(&run_root)?;
    let nan_case = run_nan_case(db.as_ref());
    let k4_case = BatchSampler::next_replay_batch(
        db.as_ref(),
        ReplayBatchRequest {
            top_k: 5,
            window: ReplayConsolidationWindow::CatastrophicPromotion {
                ticks_since_promotion: 42,
            },
            now_unix_ms: NOW_MS + 2_000,
        },
        ReplayPriorityConfig::default(),
    )?;

    let counts_after = count_replay_rows(db.as_ref())?;
    drop(db);
    let reopened = open_infer_rocksdb(&db_path)?;
    let reopened_count = count_replay_rows(reopened.as_ref())?;

    let happy_pass = seeded_count == 100
        && happy.selected_rows.len() == 20
        && first_ten_ids == high_ids
        && high_readback_counts
            .iter()
            .all(|(_, replay_count)| *replay_count == 1);
    let k4_counter_sum = k4_case
        .replay_rows_sampled_total
        .values()
        .copied()
        .sum::<u64>();
    let k4_pass = k4_case.k_replay == 4
        && k4_case.requested_rows == 20
        && k4_case.selected_rows.len() == 20
        && k4_counter_sum == 20;
    let all_passed = happy_pass
        && empty_case["pass"].as_bool() == Some(true)
        && quota_case["pass"].as_bool() == Some(true)
        && nan_case["pass"].as_bool() == Some(true)
        && k4_pass
        && counts_after == reopened_count;

    let artifact_path = run_root.join("replay_buffer_fsv.json");
    let mut report = json!({
        "task_id": "TASK-RWD-210",
        "github_issue": 287,
        "source_of_truth": {
            "db_path": db_path.display().to_string(),
            "cf": CF_MEJEPA_REPLAY_BUFFER,
            "row_key": "prediction_id"
        },
        "trigger": "BatchSampler::next_replay_batch",
        "counts_before": counts_before,
        "seeded_count": seeded_count,
        "counts_after": counts_after,
        "reopened_count": reopened_count,
        "happy_path": {
            "case": "100_rows_top_k_20_prioritizes_high_surprise",
            "expected": {
                "seeded_rows": 100,
                "selected_rows": 20,
                "first_ten": "10 high-surprise rows",
                "high_replay_count": 1
            },
            "actual": {
                "selected_rows": happy.selected_rows.len(),
                "first_ten_ids": first_ten_ids.iter().map(|id| prediction_hex(*id)).collect::<Vec<_>>(),
                "high_ids": high_ids.iter().map(|id| prediction_hex(*id)).collect::<Vec<_>>(),
                "high_readback_counts": high_readback_counts,
                "counter": happy.replay_rows_sampled_total
            },
            "pass": happy_pass
        },
        "edge_cases": [
            empty_case,
            quota_case,
            nan_case,
            json!({
                "case": "catastrophic_promotion_k_replay_4",
                "expected": {"k_replay": 4, "requested_rows": 20, "counter_sum": 20},
                "actual": {
                    "k_replay": k4_case.k_replay,
                    "requested_rows": k4_case.requested_rows,
                    "selected_rows": k4_case.selected_rows.len(),
                    "counter": k4_case.replay_rows_sampled_total,
                    "counter_sum": k4_counter_sum
                },
                "pass": k4_pass
            })
        ],
        "physical_artifacts": physical_artifacts(&run_root)?,
        "all_passed": all_passed
    });
    write_report_readback(&artifact_path, &report)?;
    report["artifact_path"] = json!(artifact_path.display().to_string());
    report["artifact_sha256"] = json!(sha256_file(&artifact_path)?);
    write_report_readback(&artifact_path, &report)?;
    if !all_passed {
        return Err("TASK_RWD_210_REPLAY_BUFFER_FSV_FAILED".into());
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn seed_priority_ladder(db: &rocksdb::DB, now_ms: i64) -> Result<()> {
    for byte in 1_u8..=10 {
        write_replay_row_sync_readback(
            db,
            &row(byte, 9.0, 1.0, "off_by_one::python", false, now_ms)?,
        )?;
    }
    for byte in 20_u8..70 {
        write_replay_row_sync_readback(
            db,
            &row(byte, 2.0, 0.5, "subtle_flip::python", false, now_ms)?,
        )?;
    }
    for byte in 100_u8..140 {
        write_replay_row_sync_readback(
            db,
            &row(byte, 0.1, 0.1, "swap_variable::python", false, now_ms)?,
        )?;
    }
    Ok(())
}

fn run_empty_case(run_root: &Path) -> Result<serde_json::Value> {
    let db_path = run_root.join("empty-db");
    let db = open_infer_rocksdb(&db_path)?;
    let report = BatchSampler::next_replay_batch(
        db.as_ref(),
        ReplayBatchRequest {
            top_k: 20,
            window: ReplayConsolidationWindow::Normal,
            now_unix_ms: NOW_MS + 3_000,
        },
        ReplayPriorityConfig::default(),
    )?;
    Ok(json!({
        "case": "empty_buffer_fresh_only",
        "expected": {"empty_buffer_fresh_only": true, "selected_rows": 0},
        "actual": report,
        "pass": report.empty_buffer_fresh_only && report.selected_rows.is_empty()
    }))
}

fn run_quota_case(run_root: &Path) -> Result<serde_json::Value> {
    let db_path = run_root.join("quota-db");
    let db = open_infer_rocksdb(&db_path)?;
    for (byte, surprise, protected) in [
        (150_u8, 0.01_f32, true),
        (151_u8, 8.0_f32, false),
        (152_u8, 7.0_f32, false),
        (153_u8, 0.2_f32, false),
        (154_u8, 0.1_f32, false),
    ] {
        let mut replay_row = row(
            byte,
            surprise,
            0.2,
            "quota_cell::python",
            protected,
            NOW_MS - 31 * DAY_MS,
        )?;
        replay_row.source = if protected {
            ReplayBufferSource::AdversarialCorpus
        } else {
            ReplayBufferSource::DriftFlaggedCell
        };
        write_replay_row_sync_readback(db.as_ref(), &replay_row)?;
    }
    let report = enforce_replay_buffer_quota(
        db.as_ref(),
        3,
        NOW_MS + 4_000,
        ReplayPriorityConfig::default(),
    )?;
    let remaining = context_graph_mejepa_train::read_all_replay_rows(db.as_ref())?;
    let remaining_ids = remaining
        .iter()
        .map(|row| prediction_hex(row.prediction_id))
        .collect::<Vec<_>>();
    let protected_id = prediction_hex(prediction(150));
    let high_ids = vec![
        prediction_hex(prediction(151)),
        prediction_hex(prediction(152)),
    ];
    let pass = report.rows_before == 5
        && report.rows_after == 3
        && report.quota_deleted.len() == 2
        && remaining_ids.contains(&protected_id)
        && high_ids.iter().all(|id| remaining_ids.contains(id))
        && remaining
            .iter()
            .all(|row| row.retention_tier == ReplayRetentionTier::Cold);
    Ok(json!({
        "case": "quota_eviction_preserves_protected_and_drops_lowest_priority",
        "expected": {
            "rows_before": 5,
            "rows_after": 3,
            "protected_preserved": protected_id,
            "kept_high_unprotected": high_ids,
            "cold_tiered_after_30_days": true
        },
        "actual": {
            "report": report,
            "remaining_ids": remaining_ids
        },
        "pass": pass
    }))
}

fn run_nan_case(db: &rocksdb::DB) -> serde_json::Value {
    let mut bad = row(200, f32::NAN, 0.2, "bad_cell::python", false, NOW_MS)
        .expect("literal row construction");
    bad.surprise_z = f32::NAN;
    let err = write_replay_row_sync_readback(db, &bad).unwrap_err();
    json!({
        "case": "nan_surprise_z_fails_closed",
        "expected": {"error_code": "REPLAY_BUFFER_DEGENERATE_PRIORITY"},
        "actual": {"error_code": err.code(), "message": err.to_string()},
        "pass": err.code() == "REPLAY_BUFFER_DEGENERATE_PRIORITY"
    })
}

fn row(
    byte: u8,
    surprise_z: f32,
    coverage_gap_score: f32,
    cell_id: &str,
    protected: bool,
    now_ms: i64,
) -> Result<ReplayBufferRow> {
    let row = ReplayBufferRow {
        prediction_id: prediction(byte),
        surprise_z,
        cell_id: cell_id.to_string(),
        coverage_gap_score,
        last_replayed_ts: None,
        replay_count: 0,
        retention_weight: 1.0,
        protected,
        retention_tier: ReplayRetentionTier::Hot,
        source: ReplayBufferSource::AgentSurprise,
        created_at_unix_ms: now_ms,
        updated_at_unix_ms: now_ms,
    };
    Ok(row)
}

fn prediction(byte: u8) -> PredictionId {
    PredictionId([byte; 16])
}

fn physical_artifacts(run_root: &Path) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for name in ["infer-db", "empty-db", "quota-db"] {
        let path = run_root.join(name);
        if path.join("CURRENT").exists() {
            paths.push(path.display().to_string());
        }
    }
    Ok(paths)
}

fn write_report_readback(path: &Path, report: &serde_json::Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    fs::write(path, bytes)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    if readback != *report {
        return Err("TASK_RWD_210_REPLAY_BUFFER_FSV_READBACK_MISMATCH".into());
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}
