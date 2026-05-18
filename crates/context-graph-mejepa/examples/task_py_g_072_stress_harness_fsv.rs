use anyhow::{ensure, Context, Result};
use context_graph_mejepa::{
    count_stress_runs, finalize_stress_run, load_stress_run, open_infer_rocksdb,
    persist_stress_run_path, run_project_stress, write_json_0600, ProjectStressRequest,
    ProjectStressSize, StressFaultInjection, StressRecallStatus, StressRunVerdict,
};
use context_graph_mejepa_cf::CF_MEJEPA_STRESS_RUNS;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-072-stress-harness-fsv";
const REPORT_NAME: &str = "stress_harness_fsv.json";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT).join(format!(
        "run-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    remove_if_exists(&root)?;
    fs::create_dir_all(&root)?;
    let trace_root = root.join("runtime");

    let mut normal_cases = Vec::new();
    let mut large_run = None;
    for size in ProjectStressSize::ALL {
        let run = run_project_stress(ProjectStressRequest {
            size,
            run_id: Some(format!("task-py-g-072-fsv-{}", size.as_str())),
            seed: 72,
            trace_root: trace_root.clone(),
            fault: StressFaultInjection::None,
        })?;
        if size == ProjectStressSize::Large {
            large_run = Some(run.clone());
        }
        normal_cases.push(case_value("normal_project_size", &run)?);
    }

    let oom = run_project_stress(ProjectStressRequest {
        size: ProjectStressSize::Tiny,
        run_id: Some("task-py-g-072-fsv-oom".to_string()),
        seed: 73,
        trace_root: trace_root.clone(),
        fault: StressFaultInjection::OomBeforeIngest,
    })?;
    let network = run_project_stress(ProjectStressRequest {
        size: ProjectStressSize::Small,
        run_id: Some("task-py-g-072-fsv-network-latency".to_string()),
        seed: 74,
        trace_root: trace_root.clone(),
        fault: StressFaultInjection::NetworkLatencySpike,
    })?;
    let cold = run_project_stress(ProjectStressRequest {
        size: ProjectStressSize::Tiny,
        run_id: Some("task-py-g-072-fsv-cold-abstain".to_string()),
        seed: 75,
        trace_root: trace_root.clone(),
        fault: StressFaultInjection::ColdAllAbstain,
    })?;
    let performance = forced_large_performance_regression(
        &root,
        large_run.context("large normal stress run missing")?,
    )?;
    let edge_cases = vec![
        edge_case_value("oom_mid_ingest_writes_aborted_row", &oom, |run| {
            run.verdict == StressRunVerdict::AbortedOom
                && run.flags.iter().any(|flag| flag == "STRESS_RUN_OOM")
                && !run.acceptance_passed
        })?,
        edge_case_value(
            "network_latency_spike_flagged_not_failed",
            &network,
            |run| {
                run.verdict == StressRunVerdict::CompletedWithFlags
                    && run
                        .flags
                        .iter()
                        .any(|flag| flag == "NETWORK_FILESYSTEM_LATENCY_SPIKE")
                    && run.acceptance_passed
            },
        )?,
        edge_case_value("all_abstain_reports_insufficient_support", &cold, |run| {
            run.verdict == StressRunVerdict::Completed
                && run
                    .defect_recall_per_class
                    .values()
                    .all(|row| row.status == StressRecallStatus::InsufficientSupport)
                && run.acceptance_passed
        })?,
        edge_case_value(
            "large_project_performance_regression_flagged",
            &performance,
            |run| {
                run.project_size == ProjectStressSize::Large
                    && run.generated_file_count == 5_000
                    && run
                        .flags
                        .iter()
                        .any(|flag| flag == "PERFORMANCE_REGRESSION")
                    && run.verdict == StressRunVerdict::CompletedWithFlags
            },
        )?,
    ];

    let all_passed = normal_cases
        .iter()
        .all(|case| case["pass"].as_bool() == Some(true))
        && edge_cases
            .iter()
            .all(|case| case["pass"].as_bool() == Some(true));
    let report_path = root.join(REPORT_NAME);
    let report = json!({
        "schemaVersion": 1,
        "taskId": "TASK-PY-G-072",
        "issue": 291,
        "sourceOfTruth": {
            "stressRunCf": CF_MEJEPA_STRESS_RUNS,
            "runtimeTraceRoot": trace_root,
            "fsvReport": report_path,
        },
        "normalCases": normal_cases,
        "edgeCases": edge_cases,
        "allPassed": all_passed,
    });
    write_json_0600(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(readback == report, "FSV report readback mismatch");
    ensure!(all_passed, "one or more stress harness FSV cases failed");
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn case_value(case: &str, run: &context_graph_mejepa::StressRun) -> Result<Value> {
    let readback = readback(run)?;
    let recall_pass = run.defect_recall_per_class.values().all(|row| {
        matches!(
            row.status,
            StressRecallStatus::Satisfied
                | StressRecallStatus::CappedByReport
                | StressRecallStatus::NotApplicable
        )
    });
    let pass = run.acceptance_passed
        && run.generated_file_count == run.project_size.file_count()
        && run.total_predictions == run.generated_file_count
        && run.reingest_time_ms <= 5_000
        && recall_pass
        && readback["pass"].as_bool() == Some(true)
        && Path::new(&run.project_manifest_path).exists()
        && Path::new(&run.project_report_json_path).exists()
        && Path::new(&run.project_report_markdown_path).exists();
    Ok(json!({
        "case": case,
        "size": run.project_size,
        "runId": run.run_id,
        "fileCount": run.generated_file_count,
        "seededCounts": &run.seeded_counts,
        "timing": {
            "ingestTimeMs": run.ingest_time_ms,
            "reingestTimeMs": run.reingest_time_ms,
            "reportTimeMs": run.report_time_ms,
            "ingestBudgetMs": run.ingest_budget_ms,
        },
        "defectRecallPerClass": &run.defect_recall_per_class,
        "sourceOfTruthReadback": readback,
        "pass": pass,
    }))
}

fn edge_case_value<F>(
    case: &str,
    run: &context_graph_mejepa::StressRun,
    predicate: F,
) -> Result<Value>
where
    F: Fn(&context_graph_mejepa::StressRun) -> bool,
{
    let readback = readback(run)?;
    let pass = predicate(run) && readback["pass"].as_bool() == Some(true);
    Ok(json!({
        "case": case,
        "size": run.project_size,
        "runId": run.run_id,
        "verdict": run.verdict,
        "flags": &run.flags,
        "defectRecallPerClass": &run.defect_recall_per_class,
        "sourceOfTruthReadback": readback,
        "pass": pass,
    }))
}

fn readback(run: &context_graph_mejepa::StressRun) -> Result<Value> {
    let db = open_infer_rocksdb(Path::new(&run.stress_db_path))?;
    let loaded = load_stress_run(db.as_ref(), &run.run_id, run.project_size)?
        .context("stress run missing from CF readback")?;
    let cf_count = count_stress_runs(db.as_ref())?;
    let pass = loaded == *run && cf_count >= 1;
    Ok(json!({
        "dbPath": run.stress_db_path,
        "cf": CF_MEJEPA_STRESS_RUNS,
        "cfRows": cf_count,
        "loadedRunId": loaded.run_id,
        "loadedProjectId": loaded.project_id,
        "loadedFileCount": loaded.generated_file_count,
        "pass": pass,
    }))
}

fn forced_large_performance_regression(
    root: &Path,
    mut run: context_graph_mejepa::StressRun,
) -> Result<context_graph_mejepa::StressRun> {
    let trace_dir = root.join("forced-large-performance-regression");
    fs::create_dir_all(&trace_dir)?;
    run.run_id = "task-py-g-072-fsv-large-performance-regression".to_string();
    run.trace_dir = trace_dir.display().to_string();
    run.stress_db_path = trace_dir.join("stress-runs.rocksdb").display().to_string();
    run.ingest_time_ms = run.ingest_budget_ms + 1;
    run.flags.clear();
    run.verdict = StressRunVerdict::Completed;
    finalize_stress_run(&mut run);
    persist_stress_run_path(Path::new(&run.stress_db_path), &run)?;
    Ok(run)
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
