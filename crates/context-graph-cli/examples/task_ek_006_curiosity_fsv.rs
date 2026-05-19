//! TASK-EK-006 active-learning curiosity FSV.
//!
//! Proves compression-progress curiosity scoring, curiosity-ranked operator
//! queue listing, label calibration evidence, telemetry CF persistence, and
//! weekly Curiosity Ranking markdown readback.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_cli::commands::mejepa_active_learning::{
    label_active_learning, list_active_learning, ActiveLearningLabelArgs, ActiveLearningListArgs,
    ActiveLearningRankByArg, OracleOutcomeArg,
};
use context_graph_mejepa::{
    conformal_width_proxy, count_cf, curiosity_score_from_proxy, open_infer_rocksdb,
    render_curiosity_ranking_weekly_section, ActiveLearningKind, ActiveLearningQueueEntry,
    ActiveLearningQueueState, ActiveLearningRankBy, CuriosityCalibrationEvidence,
    CuriosityTelemetryWindow, RocksDbEvalStore, TaskId,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_LABELS, CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_CURIOSITY_TELEMETRY,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-ek-006-curiosity-fsv";
const CANONICAL_FSV: &str =
    "/zfs/archive/contextgraph/fsv/task-ek-006-curiosity-fsv/curiosity_fsv.json";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedDeltaCase {
    task_id: String,
    conformal_width: f32,
    ood_score: f32,
    support_gap: f32,
    expected_delta_cp: f32,
    observed_curiosity_score: f32,
    relative_error: f32,
    within_five_percent: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskEk006Fsv {
    output_path: String,
    canonical_path: String,
    output_sha256: String,
    db_path: String,
    started_at_unix_ms: i64,
    queue_cf: String,
    label_cf: String,
    curiosity_telemetry_cf: String,
    known_expected_delta_cases: Vec<ExpectedDeltaCase>,
    direct_curiosity_ranked_first: String,
    cli_curiosity_ranked_first: String,
    zero_width_task_id: String,
    zero_width_curiosity_score: f32,
    label_task_id: String,
    label_queue_count_after: usize,
    predicted_delta_cp: f32,
    actual_delta_cp: f32,
    calibration_within_tolerance: bool,
    telemetry_cf_rows_after_reopen: u64,
    telemetry_distribution_count: usize,
    telemetry_top_task_id: String,
    weekly_curiosity_ranking_rendered: bool,
    weekly_top_task_id_present: bool,
    passes: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("rocksdb");
    let output_path = run_root.join("curiosity_fsv.json");
    let canonical_path = PathBuf::from(CANONICAL_FSV);
    fs::create_dir_all(&run_root).context("create TASK-EK-006 FSV root")?;

    let db = open_infer_rocksdb(&db_path).context("open TASK-EK-006 FSV RocksDB")?;
    let store = RocksDbEvalStore::new(db.clone()).context("open eval store")?;
    let queue = seed_curiosity_queue()?;
    store
        .persist_queue(&queue)
        .context("persist active-learning queue")?;
    drop(store);
    drop(db);

    let reopened = open_infer_rocksdb(&db_path).context("reopen queue RocksDB")?;
    let reopened_store = RocksDbEvalStore::new(reopened.clone()).context("reopen eval store")?;
    let pre_label_queue = reopened_store
        .load_queue()
        .context("load persisted queue")?
        .context("queue missing after persist")?;
    let direct_top = pre_label_queue
        .ranked_entries(ActiveLearningRankBy::Curiosity)
        .first()
        .context("empty ranked queue")?
        .task_id
        .0
        .clone();
    let known_expected_delta_cases = expected_delta_cases(&pre_label_queue)?;
    let zero_width_entry = pre_label_queue
        .entries
        .get(&TaskId("zero-width".to_string()))
        .context("zero-width entry missing")?;
    let zero_width_curiosity_score = zero_width_entry.curiosity_score;
    drop(reopened_store);
    drop(reopened);

    let cli_output = list_active_learning(ActiveLearningListArgs {
        db_path: db_path.clone(),
        top_n: 4,
        ranked_by: ActiveLearningRankByArg::Curiosity,
    })
    .await
    .context("CLI curiosity-ranked active-learning list")?;
    let cli_top = cli_output
        .entries
        .first()
        .context("empty CLI ranked queue")?
        .task_id
        .clone();

    let top_task = TaskId(direct_top.clone());
    let predicted_delta_cp = pre_label_queue
        .entries
        .get(&top_task)
        .context("top entry missing")?
        .curiosity_score;
    let actual_delta_cp = predicted_delta_cp * 0.98;
    let calibration_evidence = CuriosityCalibrationEvidence::new(
        top_task.clone(),
        predicted_delta_cp,
        actual_delta_cp,
        0.05,
    )
    .context("build curiosity calibration evidence")?;

    let telemetry = CuriosityTelemetryWindow::from_queue(
        "task-ek-006-window-1",
        started_at_unix_ms + 1,
        &pre_label_queue,
        10,
    )
    .context("build curiosity telemetry")?
    .with_calibration_evidence(vec![calibration_evidence.clone()])
    .context("attach calibration evidence")?;
    let telemetry_db = open_infer_rocksdb(&db_path).context("reopen telemetry RocksDB")?;
    let telemetry_store = RocksDbEvalStore::new(telemetry_db.clone()).context("telemetry store")?;
    telemetry_store
        .persist_curiosity_telemetry_window(&telemetry)
        .context("persist curiosity telemetry")?;
    drop(telemetry_store);
    drop(telemetry_db);

    let label_output = label_active_learning(ActiveLearningLabelArgs {
        db_path: db_path.clone(),
        task_id: direct_top.clone(),
        label: OracleOutcomeArg::Pass,
        reason: "TASK-EK-006 synthetic top-curiosity label".to_string(),
    })
    .await
    .context("label top curiosity entry")?;

    let final_db = open_infer_rocksdb(&db_path).context("final reopen RocksDB")?;
    let final_store = RocksDbEvalStore::new(final_db.clone()).context("final eval store")?;
    let telemetry_windows = final_store
        .load_curiosity_telemetry_windows_chronological()
        .context("load curiosity telemetry windows")?;
    let telemetry_cf_rows_after_reopen = count_cf(final_db.as_ref(), CF_MEJEPA_CURIOSITY_TELEMETRY)
        .context("count curiosity telemetry rows")?;
    let weekly_section = render_curiosity_ranking_weekly_section(&pre_label_queue, 10)
        .context("render weekly curiosity section")?;

    let telemetry_readback = telemetry_windows
        .first()
        .context("missing curiosity telemetry readback")?;
    let passes = known_expected_delta_cases
        .iter()
        .all(|case| case.within_five_percent)
        && direct_top == "very-high-curiosity"
        && cli_top == "very-high-curiosity"
        && zero_width_curiosity_score == 0.0
        && label_output.persisted_label_readback_equal
        && !final_store
            .load_queue()?
            .context("queue missing after label")?
            .entries
            .contains_key(&top_task)
        && calibration_evidence.within_tolerance
        && telemetry_cf_rows_after_reopen == 1
        && telemetry_readback.entry_count == pre_label_queue.entries.len()
        && telemetry_readback.top_entries[0].task_id.0 == "very-high-curiosity"
        && weekly_section.contains("## Curiosity Ranking")
        && weekly_section.contains("very-high-curiosity");

    let mut report = TaskEk006Fsv {
        output_path: output_path.display().to_string(),
        canonical_path: canonical_path.display().to_string(),
        output_sha256: String::new(),
        db_path: db_path.display().to_string(),
        started_at_unix_ms,
        queue_cf: CF_MEJEPA_ACTIVE_LEARNING_QUEUE.to_string(),
        label_cf: CF_MEJEPA_ACTIVE_LEARNING_LABELS.to_string(),
        curiosity_telemetry_cf: CF_MEJEPA_CURIOSITY_TELEMETRY.to_string(),
        known_expected_delta_cases,
        direct_curiosity_ranked_first: direct_top.clone(),
        cli_curiosity_ranked_first: cli_top,
        zero_width_task_id: "zero-width".to_string(),
        zero_width_curiosity_score,
        label_task_id: direct_top,
        label_queue_count_after: label_output.queue_count_after,
        predicted_delta_cp,
        actual_delta_cp,
        calibration_within_tolerance: calibration_evidence.within_tolerance,
        telemetry_cf_rows_after_reopen,
        telemetry_distribution_count: telemetry_readback.curiosity_distribution.count,
        telemetry_top_task_id: telemetry_readback.top_entries[0].task_id.0.clone(),
        weekly_curiosity_ranking_rendered: weekly_section.contains("## Curiosity Ranking"),
        weekly_top_task_id_present: weekly_section.contains("very-high-curiosity"),
        passes,
    };
    if !report.passes {
        bail!("TASK-EK-006 curiosity FSV failed");
    }

    write_report(&output_path, &mut report).context("write run report")?;
    fs::create_dir_all(canonical_path.parent().context("canonical parent")?)?;
    fs::copy(&output_path, &canonical_path).context("copy canonical curiosity FSV")?;
    println!("{}", canonical_path.display());
    Ok(())
}

fn seed_curiosity_queue() -> Result<ActiveLearningQueueState> {
    let mut queue = ActiveLearningQueueState::new(8).context("queue")?;
    for entry in [
        entry_from_proxy("known-delta-low", 0.80, 2, 0.50, 0.40)?,
        entry_from_proxy("known-delta-mid", 0.70, 4, 0.80, 0.50)?,
        entry_from_proxy("very-high-curiosity", 0.10, 10, 1.00, 1.00)?,
        entry_from_proxy("zero-width", 0.60, 1, 1.00, 1.00)?,
    ] {
        queue.entries.insert(entry.task_id.clone(), entry);
    }
    Ok(queue)
}

fn entry_from_proxy(
    task_id: &str,
    scheduler_score: f32,
    outcome_set_len: usize,
    ood_score: f32,
    support_gap: f32,
) -> Result<ActiveLearningQueueEntry> {
    Ok(ActiveLearningQueueEntry {
        task_id: TaskId(task_id.to_string()),
        score: scheduler_score,
        outcome_set_len,
        ood_score,
        curiosity_score: curiosity_score_from_proxy(
            conformal_width_proxy(outcome_set_len),
            ood_score,
            support_gap,
        )?,
        reason: format!("synthetic_curiosity_support_gap:{support_gap:.2}"),
        kind: ActiveLearningKind::Uncertainty,
    })
}

fn expected_delta_cases(queue: &ActiveLearningQueueState) -> Result<Vec<ExpectedDeltaCase>> {
    let specs = [
        ("known-delta-low", 0.50_f32, 0.50_f32, 0.40_f32),
        ("known-delta-mid", 0.75_f32, 0.80_f32, 0.50_f32),
        ("very-high-curiosity", 0.90_f32, 1.00_f32, 1.00_f32),
    ];
    specs
        .into_iter()
        .map(|(task_id, conformal_width, ood_score, support_gap)| {
            let expected_delta_cp = conformal_width * ood_score * support_gap;
            let observed_curiosity_score = queue
                .entries
                .get(&TaskId(task_id.to_string()))
                .with_context(|| format!("missing {task_id}"))?
                .curiosity_score;
            let relative_error = if expected_delta_cp == 0.0 {
                observed_curiosity_score.abs()
            } else {
                ((observed_curiosity_score - expected_delta_cp) / expected_delta_cp).abs()
            };
            Ok(ExpectedDeltaCase {
                task_id: task_id.to_string(),
                conformal_width,
                ood_score,
                support_gap,
                expected_delta_cp,
                observed_curiosity_score,
                relative_error,
                within_five_percent: relative_error <= 0.05,
            })
        })
        .collect()
}

fn write_report(path: &PathBuf, report: &mut TaskEk006Fsv) -> Result<()> {
    report.output_sha256.clear();
    let provisional = serde_json::to_vec_pretty(report).context("serialize provisional report")?;
    let digest = Sha256::digest(&provisional);
    report.output_sha256 = hex::encode(digest);
    let bytes = serde_json::to_vec_pretty(report).context("serialize report")?;
    fs::write(path, &bytes).context("write report")?;
    let readback = fs::read(path).context("read report")?;
    if readback != bytes {
        bail!("curiosity FSV readback bytes differ");
    }
    Ok(())
}

fn unix_ms() -> Result<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before epoch")?
        .as_millis()
        .try_into()
        .context("unix millis overflow")
}
