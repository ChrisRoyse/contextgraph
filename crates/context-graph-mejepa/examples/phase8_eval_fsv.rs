use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::eval::{
    build_eval_compiler, build_patch_similarity_graph, corpus_sha_from_holdout, synthetic_holdout,
    synthetic_patch_embeddings, write_weekly_report, EvalConfig, EvalProvenance, EvalRunner,
};
use rocksdb::IteratorMode;
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

const OUT: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-phase8-eval-fsv";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(OUT);
    if out.exists() {
        fs::remove_dir_all(&out)?;
    }
    fs::create_dir_all(&out)?;
    let db_path = out.join("rocksdb");
    let repo_root = out.join("repo");
    let (db, compiler, eval_store, calibration) = build_eval_compiler(&db_path, &repo_root)?;
    let holdout = synthetic_holdout(&repo_root)?;
    let config = EvalConfig::default();
    let provenance = EvalProvenance {
        corpus_sha: corpus_sha_from_holdout(&holdout),
        eval_code_version: env!("CARGO_PKG_VERSION").to_string(),
        calibration_version: calibration.load_active()?.version.clone(),
        generated_by: "phase8_eval_fsv".to_string(),
    };
    let runner = EvalRunner::new(compiler, eval_store.clone(), config)?;
    let mut baseline_provenance = provenance.clone();
    baseline_provenance.generated_by = "phase8_eval_fsv_baseline".to_string();
    let baseline = runner.run_holdout_eval(&holdout, &[], "2026-05-10", baseline_provenance)?;
    let report = runner.run_holdout_eval(&holdout, &[], "2026-05-11", provenance)?;
    let weekly_path = out.join("weekly-report.json");
    write_weekly_report(&weekly_path, &report)?;
    let graph = build_patch_similarity_graph(&synthetic_patch_embeddings(), 0.85, 3)?;
    eval_store.persist_graph(&graph)?;
    let latest = eval_store
        .load_latest_report()?
        .ok_or("latest report missing")?;
    let queue = eval_store.load_queue()?.ok_or("queue missing")?;
    let graph_readback = eval_store.load_graph()?.ok_or("graph missing")?;
    let questions = eval_store
        .load_research_questions()?
        .ok_or("research questions missing")?;
    let cf_counts = cf_counts(&db)?;
    let edge_cases = edge_cases(&runner, &holdout)?;

    write_json_0600(&out.join("holdout-eval-evidence.json"), &report)?;
    write_json_0600(&out.join("latest-report-readback.json"), &latest)?;
    write_json_0600(&out.join("active-learning-queue.json"), &queue)?;
    write_json_0600(
        &out.join("ship-gate.json"),
        &json!({
            "ship_gate_passed": latest.ship_gate_passed,
            "failures": latest.ship_gate_failures,
            "report_hash": latest.determinism_hash()?
        }),
    )?;
    write_json_0600(&out.join("patch-similarity-graph.json"), &graph_readback)?;
    write_json_0600(
        &out.join("regression-evidence.json"),
        &latest.regression_checks,
    )?;
    write_json_0600(
        &out.join("ood-evidence.json"),
        &latest.ood_calibration_health,
    )?;
    write_json_0600(
        &out.join("conformal-evidence.json"),
        &latest.conformal_coverage_health,
    )?;
    write_json_0600(
        &out.join("q1q2q3-evidence.json"),
        &json!({
            "q1_pass_rate": latest.q1_pass_rate,
            "q2_report_correlation": latest.q2_report_correlation,
            "q3_side_effect_agreement": latest.q3_side_effect_agreement
        }),
    )?;
    write_json_0600(
        &out.join("state-transfer-evidence.json"),
        &json!({
            "global": latest.state_transfer_diagnostic,
            "per_cell": latest.per_cell_state_transfer
        }),
    )?;
    write_json_0600(
        &out.join("aux-head-distillation.json"),
        &latest.aux_head_distillation,
    )?;
    write_json_0600(&out.join("open-research-questions.json"), &questions)?;
    write_json_0600(&out.join("leakage-check-evidence.json"), &edge_cases)?;
    write_json_0600(
        &out.join("bayesian-shrinkage.json"),
        &latest.bayesian_shrinkage,
    )?;
    write_json_0600(&out.join("provenance.json"), &latest.provenance)?;
    write_json_0600(
        &out.join("latency.json"),
        &json!({"wall_clock_seconds": latest.wall_clock_seconds}),
    )?;
    write_json_0600(
        &out.join("mcp-roundtrip.json"),
        &json!({
            "tool_names": [
                "mcp__cgreality__mejepa_eval_run",
                "mcp__cgreality__mejepa_ship_gate_status",
                "mcp__cgreality__mejepa_active_learning_queue",
                "mcp__cgreality__mejepa_eval_build_graph"
            ],
            "source_of_truth": ["CF_MEJEPA_EVAL_REPORTS", "CF_MEJEPA_ACTIVE_LEARNING_QUEUE", "CF_MEJEPA_TASK_GRAPH"]
        }),
    )?;
    let manifest = json!({
        "root": out,
        "db_path": db_path,
        "weekly_report_path": weekly_path,
        "report_hash": latest.determinism_hash()?,
        "baseline_ship_gate_passed": baseline.ship_gate_passed,
        "cf_counts": cf_counts,
        "graph_edge_count": graph_readback.edge_count,
        "queue_entries": queue.entries.len(),
        "evidence_file_count": 17,
        "per_cell_state_transfer_cells": latest.per_cell_state_transfer.len()
    });
    write_json_0600(&out.join("manifest.json"), &manifest)?;
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}

fn edge_cases(
    runner: &EvalRunner,
    holdout: &[context_graph_mejepa::eval::HoldoutPanel],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let empty = runner
        .run_holdout_eval(
            &[],
            &[],
            "2026-05-11-empty",
            EvalProvenance {
                corpus_sha: "empty".to_string(),
                eval_code_version: "fsv".to_string(),
                calibration_version: "none".to_string(),
                generated_by: "edge".to_string(),
            },
        )
        .unwrap_err();
    let leaked_train_ids = vec![holdout[0].task_id.clone()];
    let leaked = runner
        .run_holdout_eval(
            &holdout[..1],
            &leaked_train_ids,
            "2026-05-11-leak",
            EvalProvenance {
                corpus_sha: "leak".to_string(),
                eval_code_version: "fsv".to_string(),
                calibration_version: "none".to_string(),
                generated_by: "edge".to_string(),
            },
        )
        .unwrap_err();
    let invalid_graph = build_patch_similarity_graph(&[], 0.85, 3).unwrap_err();
    Ok(json!([
        {
            "name": "empty_holdout",
            "before": {"holdout_count": 0},
            "after": {"error": empty.code()}
        },
        {
            "name": "train_holdout_leakage",
            "before": {"train_task_id": holdout[0].task_id.0},
            "after": {"error": leaked.code()}
        },
        {
            "name": "empty_patch_graph",
            "before": {"embedding_count": 0},
            "after": {"error": invalid_graph.code()}
        }
    ]))
}

fn cf_counts(db: &rocksdb::DB) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut counts = serde_json::Map::new();
    for cf_name in context_graph_mejepa_cf::EVAL_CFS {
        let cf = db.cf_handle(cf_name).ok_or(format!("missing {cf_name}"))?;
        let count = db.iterator_cf(cf, IteratorMode::Start).count();
        counts.insert(cf_name.to_string(), json!(count));
    }
    Ok(serde_json::Value::Object(counts))
}

#[allow(dead_code)]
fn _assert_serialize<T: Serialize>(_value: &T) {}
