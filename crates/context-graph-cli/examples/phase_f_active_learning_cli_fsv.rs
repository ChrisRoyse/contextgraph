use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, ensure, Context, Result};
use chrono::Utc;
use context_graph_cli::commands::mejepa_active_learning::{
    dismiss_active_learning, label_active_learning, list_active_learning,
    ActiveLearningDismissArgs, ActiveLearningLabelArgs, ActiveLearningListArgs, OracleOutcomeArg,
};
use context_graph_mejepa::eval::store::{
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS, CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
};
use context_graph_mejepa::{
    count_cf, open_infer_rocksdb, ActiveLearningKind, ActiveLearningQueueEntry,
    ActiveLearningQueueState, OracleOutcome, PredictionId, RocksDbEvalStore, TaskId,
};
use rocksdb::DB;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-active-learning-cli-fsv";

#[tokio::main]
async fn main() -> Result<()> {
    let started_at_unix_ms = Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root).context("create FSV root")?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root).context("create run root")?;
    let db_path = run_root.join("rocksdb");
    let build_release_sha = git_head_short()?;

    let db = open_infer_rocksdb(&db_path).context("open active-learning FSV DB")?;
    let store = RocksDbEvalStore::new(db.clone()).context("construct eval store")?;
    let cf_counts_before = cf_counts(db.as_ref())?;

    seed_queue(&store).context("seed active-learning queue")?;
    flush_active_learning_cfs(db.as_ref()).context("flush seeded active-learning CFs")?;
    drop(store);
    drop(db);

    let list_output = list_active_learning(ActiveLearningListArgs {
        db_path: db_path.clone(),
        top_n: 2,
    })
    .await
    .context("list active-learning queue")?;
    ensure!(
        list_output.queued_count == 4,
        "expected 4 queued entries after seed"
    );
    ensure!(
        list_output
            .entries
            .first()
            .map(|entry| entry.task_id.as_str())
            == Some("phase-f-agent-surprise"),
        "agent surprise must sort ahead of uncertainty rows"
    );

    let label_output = label_active_learning(ActiveLearningLabelArgs {
        db_path: db_path.clone(),
        task_id: "phase-f-label-me".to_string(),
        label: OracleOutcomeArg::Pass,
        reason: "operator verified tests pass".to_string(),
    })
    .await
    .context("label active-learning item")?;
    ensure!(
        label_output.persisted_label_readback_equal,
        "label readback equality must be true"
    );

    let dismiss_output = dismiss_active_learning(ActiveLearningDismissArgs {
        db_path: db_path.clone(),
        task_id: "phase-f-dismiss-me".to_string(),
        reason: "duplicate of already triaged surprise".to_string(),
    })
    .await
    .context("dismiss active-learning item")?;
    ensure!(
        dismiss_output.persisted_eviction_readback_equal,
        "dismissal readback equality must be true"
    );

    let boundary_cases = run_boundary_cases(&db_path).await?;
    ensure!(
        boundary_cases
            .iter()
            .all(|case| case["pass"].as_bool() == Some(true)),
        "all boundary cases must pass"
    );

    let post_db = open_infer_rocksdb(&db_path).context("open post-operation FSV DB")?;
    let post_store = RocksDbEvalStore::new(post_db.clone()).context("post-operation eval store")?;
    flush_active_learning_cfs(post_db.as_ref()).context("flush active-learning CFs")?;
    let cf_counts_after = cf_counts(post_db.as_ref())?;
    drop(post_store);
    drop(post_db);

    let reopened = open_infer_rocksdb(&db_path).context("reopen active-learning FSV DB")?;
    let reopened_store = RocksDbEvalStore::new(reopened.clone()).context("reopened eval store")?;
    let reopened_queue = reopened_store
        .load_queue()
        .context("load reopened queue")?
        .ok_or_else(|| anyhow!("reopened queue missing"))?;
    let reopened_label = reopened_store
        .load_label(&TaskId("phase-f-label-me".to_string()))
        .context("load reopened label")?
        .ok_or_else(|| anyhow!("reopened label missing"))?;
    let reopened_eviction = reopened_store
        .load_evicted_entry(&TaskId("phase-f-dismiss-me".to_string()))
        .context("load reopened eviction")?
        .ok_or_else(|| anyhow!("reopened eviction missing"))?;
    let readback_equal = reopened_queue.entries.len() == 2
        && !reopened_queue
            .entries
            .contains_key(&TaskId("phase-f-label-me".to_string()))
        && !reopened_queue
            .entries
            .contains_key(&TaskId("phase-f-dismiss-me".to_string()))
        && reopened_label.oracle_outcome == OracleOutcome::Pass
        && reopened_eviction
            .reason
            .starts_with("operator_dismissed:duplicate");
    ensure!(
        readback_equal,
        "reopened readback did not match expected state"
    );

    let physical_artifacts = physical_artifacts(&db_path)?;
    ensure!(
        physical_artifacts["sst_file_count"].as_u64().unwrap_or(0) > 0,
        "expected at least one RocksDB SST file after flush"
    );

    let artifact_path = fsv_root.join("active_learning_cli_fsv.json");
    let artifact = json!({
        "fsv_root": fsv_root,
        "task_id": "TASK-OBS-010",
        "issue": 45,
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": build_release_sha,
        "trigger": "cargo run --release -p context-graph-cli --example phase_f_active_learning_cli_fsv",
        "db_path": db_path,
        "happy_path": [{
            "case": "list_label_dismiss_round_trip",
            "sot": [
                "CF_MEJEPA_ACTIVE_LEARNING_QUEUE",
                "CF_MEJEPA_ACTIVE_LEARNING_LABELS",
                "CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS"
            ],
            "before": cf_counts_before,
            "after": cf_counts_after,
            "expected": {
                "queue_count_after": 2,
                "label_task": "phase-f-label-me",
                "label": "pass",
                "dismissed_task": "phase-f-dismiss-me"
            },
            "actual": {
                "list_output": list_output,
                "label_output": label_output,
                "dismiss_output": dismiss_output,
                "reopened_queue_count": reopened_queue.entries.len(),
                "reopened_label": reopened_label,
                "reopened_eviction": reopened_eviction,
            },
            "pass": true,
            "evidence_path": artifact_path,
        }],
        "boundary_cases": boundary_cases,
        "all_passed": true,
        "cf_counts_before": cf_counts_before,
        "cf_counts_after": cf_counts_after,
        "readback_equal": readback_equal,
        "physical_artifacts": physical_artifacts,
    });
    let bytes = serde_json::to_vec_pretty(&artifact).context("serialize FSV artifact")?;
    fs::write(&artifact_path, &bytes).context("write FSV artifact")?;
    let artifact_sha256 = format!("{:x}", Sha256::digest(&bytes));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_path": artifact_path,
            "artifact_sha256": artifact_sha256,
            "all_passed": true,
            "readback_equal": readback_equal,
        }))?
    );
    Ok(())
}

fn seed_queue(store: &RocksDbEvalStore) -> Result<()> {
    let mut queue = ActiveLearningQueueState::new(16)?;
    for entry in [
        queue_entry(
            "phase-f-agent-surprise",
            0.2,
            "catastrophic operator surprise",
            ActiveLearningKind::AgentSurprise {
                prediction_id: PredictionId([0x45; 16]),
                severity_score: 1.0,
            },
        ),
        queue_entry(
            "phase-f-label-me",
            0.7,
            "needs human label",
            ActiveLearningKind::Uncertainty,
        ),
        queue_entry(
            "phase-f-dismiss-me",
            0.6,
            "duplicate uncertainty",
            ActiveLearningKind::Uncertainty,
        ),
        queue_entry(
            "phase-f-uncertain-low",
            0.1,
            "low score uncertainty",
            ActiveLearningKind::Uncertainty,
        ),
    ] {
        queue.entries.insert(entry.task_id.clone(), entry);
    }
    store.persist_queue(&queue)?;
    Ok(())
}

fn queue_entry(
    task_id: &str,
    score: f32,
    reason: &str,
    kind: ActiveLearningKind,
) -> ActiveLearningQueueEntry {
    ActiveLearningQueueEntry {
        task_id: TaskId(task_id.to_string()),
        score,
        outcome_set_len: 3,
        ood_score: 0.2,
        reason: reason.to_string(),
        kind,
    }
}

async fn run_boundary_cases(db_path: &Path) -> Result<Vec<Value>> {
    let missing_db = db_path
        .parent()
        .ok_or_else(|| anyhow!("db_path has no parent"))?
        .join("missing-queue-rocksdb");
    let missing_queue = list_active_learning(ActiveLearningListArgs {
        db_path: missing_db,
        top_n: 1,
    })
    .await
    .expect_err("missing queue must fail closed");

    let zero_top_n = list_active_learning(ActiveLearningListArgs {
        db_path: db_path.to_path_buf(),
        top_n: 0,
    })
    .await
    .expect_err("zero top_n must fail closed");

    let unknown_label = label_active_learning(ActiveLearningLabelArgs {
        db_path: db_path.to_path_buf(),
        task_id: "phase-f-missing-task".to_string(),
        label: OracleOutcomeArg::Fail,
        reason: "should fail".to_string(),
    })
    .await
    .expect_err("unknown label target must fail closed");

    let empty_dismiss_reason = dismiss_active_learning(ActiveLearningDismissArgs {
        db_path: db_path.to_path_buf(),
        task_id: "phase-f-uncertain-low".to_string(),
        reason: " ".to_string(),
    })
    .await
    .expect_err("empty dismiss reason must fail closed");

    Ok(vec![
        boundary_case(
            "missing_queue",
            "MEJEPA_ACTIVE_LEARNING_QUEUE_MISSING",
            &missing_queue,
        ),
        boundary_case(
            "zero_top_n",
            "MEJEPA_ACTIVE_LEARNING_TOP_N_INVALID",
            &zero_top_n,
        ),
        boundary_case(
            "unknown_label_task",
            "MEJEPA_ACTIVE_LEARNING_ENTRY_NOT_FOUND",
            &unknown_label,
        ),
        boundary_case(
            "empty_dismiss_reason",
            "MEJEPA_ACTIVE_LEARNING_REASON_EMPTY",
            &empty_dismiss_reason,
        ),
    ])
}

fn boundary_case(case: &str, expected: &str, err: &anyhow::Error) -> Value {
    let actual = format!("{err:#}");
    json!({
        "case": case,
        "before": null,
        "after": null,
        "expected": expected,
        "actual": actual,
        "pass": actual.contains(expected),
    })
}

fn flush_active_learning_cfs(db: &DB) -> Result<()> {
    for cf_name in [
        CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
        CF_MEJEPA_ACTIVE_LEARNING_LABELS,
        CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    ] {
        let cf = db
            .cf_handle(cf_name)
            .ok_or_else(|| anyhow!("missing CF {cf_name}"))?;
        db.flush_cf(cf)
            .with_context(|| format!("flush column family {cf_name}"))?;
    }
    Ok(())
}

fn cf_counts(db: &DB) -> Result<Value> {
    Ok(json!({
        "CF_MEJEPA_ACTIVE_LEARNING_QUEUE": count_cf(db, CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?,
        "CF_MEJEPA_ACTIVE_LEARNING_LABELS": count_cf(db, CF_MEJEPA_ACTIVE_LEARNING_LABELS)?,
        "CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS": count_cf(db, CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS)?,
    }))
}

fn physical_artifacts(db_path: &Path) -> Result<Value> {
    let mut total_bytes = 0u64;
    let mut sst_file_count = 0u64;
    let mut sst_files = Vec::new();
    collect_physical_artifacts(
        db_path,
        &mut total_bytes,
        &mut sst_file_count,
        &mut sst_files,
    )?;
    Ok(json!({
        "db_path": db_path,
        "total_file_bytes": total_bytes,
        "sst_file_count": sst_file_count,
        "sst_files": sst_files,
    }))
}

fn collect_physical_artifacts(
    path: &Path,
    total_bytes: &mut u64,
    sst_file_count: &mut u64,
    sst_files: &mut Vec<Value>,
) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read dir {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            collect_physical_artifacts(&path, total_bytes, sst_file_count, sst_files)?;
            continue;
        }
        *total_bytes += meta.len();
        if path.extension().and_then(|ext| ext.to_str()) == Some("sst") {
            *sst_file_count += 1;
            sst_files.push(json!({
                "path": path,
                "bytes": meta.len(),
            }));
        }
    }
    Ok(())
}

fn git_head_short() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .context("git rev-parse --short HEAD")?;
    ensure!(
        output.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
