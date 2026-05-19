use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::compiler::MejepaStore;
use context_graph_mejepa::store::RocksDbInferStore;
use context_graph_mejepa::{
    count_cf, open_infer_rocksdb, persist_operator_override,
    write_operator_contribution_sync_readback, ChunkId, ConformalInterval, ConformalSet,
    DownstreamOutcomeKind, DownstreamOutcomeRef, Language, OperatorContribution,
    OperatorContributionEventKind, OperatorContributionPayload, OperatorOverride, OracleOutcome,
    OverrideVerdict, PredictionId, PredictionProvenance, RealityPrediction,
    RealityPredictionBuilder, TaskId, Verdict,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_OPERATOR_CONTRIBUTIONS, CF_MEJEPA_OPERATOR_OVERRIDES,
};
use context_graph_mejepa_train::config::TrainingConfig;
use context_graph_mejepa_train::sampler::{
    BatchSampler, OperatorOverrideSamplerInput, PatchSimilarityGraph, SamplerExampleSource,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-ek-015-credibility-override-fsv";
const BASE_TS: i64 = 1_800_100_000_000;

fn main() -> Result<()> {
    let started_at = chrono::Utc::now().timestamp_millis();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at}"));
    let db_path = run_root.join("infer-rocksdb");
    let fsv_path = run_root.join("credibility_override_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("credibility_override_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-015 FSV dir")?;

    let db = open_infer_rocksdb(&db_path).context("open credibility override DB")?;
    let store = RocksDbInferStore::new(db.clone());
    let high_prediction = PredictionId([0x91; 16]);
    let low_prediction = PredictionId([0x92; 16]);
    let high_task = TaskId("task-ek-015-high".to_string());
    let low_task = TaskId("task-ek-015-low".to_string());
    let high_chunks = vec![
        ChunkId("src/high.rs#fn#a".to_string()),
        ChunkId("src/high.rs#fn#b".to_string()),
    ];
    let low_chunks = vec![
        ChunkId("src/low.rs#fn#a".to_string()),
        ChunkId("src/low.rs#fn#b".to_string()),
    ];
    store.write_live_prediction(&prediction(
        high_prediction,
        high_task.clone(),
        high_chunks.clone(),
        BASE_TS,
    )?)?;
    store.write_live_prediction(&prediction(
        low_prediction,
        low_task.clone(),
        low_chunks.clone(),
        BASE_TS + 1,
    )?)?;

    for row in high_quality_operator_rows("operator-high") {
        write_operator_contribution_sync_readback(db.as_ref(), &row)
            .context("persist high-quality operator contribution")?;
    }

    persist_operator_override(
        db.as_ref(),
        &OperatorOverride::new(
            high_prediction,
            OverrideVerdict::Fail,
            "high-quality operator correction".to_string(),
            "operator-high".to_string(),
            BASE_TS + 100,
        )?,
    )?;
    persist_operator_override(
        db.as_ref(),
        &OperatorOverride::new(
            low_prediction,
            OverrideVerdict::Fail,
            "low-quality operator correction".to_string(),
            "operator-low".to_string(),
            BASE_TS + 101,
        )?,
    )?;
    flush_cfs(db.as_ref())?;

    let cfg = TrainingConfig {
        random_seed: 915,
        batch_size: 4,
        operator_override_min_multiplier: 4.0,
        operator_override_max_multiplier: 12.0,
        ..TrainingConfig::default()
    };
    let input = OperatorOverrideSamplerInput {
        config: cfg,
        l_steps: vec![0.2, 0.2, 0.2, 0.2],
        overrides: vec![false; 4],
        ages_days: vec![0.0; 4],
        task_ids: vec![
            high_task.0.clone(),
            high_task.0.clone(),
            low_task.0.clone(),
            low_task.0.clone(),
        ],
        example_sources: vec![
            SamplerExampleSource {
                prediction_id: high_prediction,
                chunk_id: high_chunks[0].clone(),
            },
            SamplerExampleSource {
                prediction_id: high_prediction,
                chunk_id: high_chunks[1].clone(),
            },
            SamplerExampleSource {
                prediction_id: low_prediction,
                chunk_id: low_chunks[0].clone(),
            },
            SamplerExampleSource {
                prediction_id: low_prediction,
                chunk_id: low_chunks[1].clone(),
            },
        ],
        patch_similarity_graph: PatchSimilarityGraph {
            neighbors: vec![Vec::new(); 4],
        },
    };

    let (mut sampler, report) =
        BatchSampler::new_with_operator_override_db(input, db.as_ref(), None)
            .context("build credibility-weighted sampler")?;
    let batch = sampler.next_batch(None, 4).context("draw audited batch")?;
    drop(store);
    drop(db);
    let reopened = open_infer_rocksdb(&db_path).context("reopen credibility override DB")?;

    let base_weight = 0.2_f32;
    let high_multiplier = sampler.weights[0] / base_weight;
    let low_multiplier = sampler.weights[2] / base_weight;
    let high_multiplier_ok =
        approx_eq(high_multiplier, 12.0) && approx_eq(sampler.weights[1] / base_weight, 12.0);
    let low_multiplier_ok =
        approx_eq(low_multiplier, 4.0) && approx_eq(sampler.weights[3] / base_weight, 4.0);
    let audit_reconstructable = report.boost_audit.len() == 4
        && batch.operator_override_boost_audit.len() == 4
        && report.boost_audit.iter().any(|row| {
            row.operator_id == "operator-high"
                && row.operator_quality_score_micros == 1_000_000
                && row.sampling_weight_multiplier_micros == 12_000_000
        })
        && report.boost_audit.iter().any(|row| {
            row.operator_id == "operator-low"
                && row.operator_quality_score_micros == 0
                && row.sampling_weight_multiplier_micros == 4_000_000
        });
    let reopen_counts_ok = count_cf(reopened.as_ref(), CF_MEJEPA_OPERATOR_CONTRIBUTIONS)? == 8
        && count_cf(reopened.as_ref(), CF_MEJEPA_OPERATOR_OVERRIDES)? == 2
        && count_cf(reopened.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)? == 2;
    let all_passed =
        high_multiplier_ok && low_multiplier_ok && audit_reconstructable && reopen_counts_ok;

    let evidence = json!({
        "task": "TASK-EK-015",
        "issue": 352,
        "passes": all_passed,
        "all_passed": all_passed,
        "sot": {
            "db_path": db_path,
            "operator_contribution_cf": CF_MEJEPA_OPERATOR_CONTRIBUTIONS,
            "operator_override_cf": CF_MEJEPA_OPERATOR_OVERRIDES,
            "live_prediction_cf": CF_MEJEPA_LIVE_PREDICTIONS,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "inner_llm_invoked": false
        },
        "sot_cases": {
            "high_quality_operator_gets_12x": {
                "passed": high_multiplier_ok,
                "expected_multiplier": 12.0,
                "actual_multipliers": [high_multiplier, sampler.weights[1] / base_weight]
            },
            "low_quality_operator_gets_4x": {
                "passed": low_multiplier_ok,
                "expected_multiplier": 4.0,
                "actual_multipliers": [low_multiplier, sampler.weights[3] / base_weight]
            },
            "per_batch_audit_row_reconstructable": {
                "passed": audit_reconstructable,
                "sampler_report_audit": &report.boost_audit,
                "batch_audit": &batch.operator_override_boost_audit
            },
            "independent_reopen_counts": {
                "passed": reopen_counts_ok,
                "counts": {
                    CF_MEJEPA_OPERATOR_CONTRIBUTIONS: count_cf(reopened.as_ref(), CF_MEJEPA_OPERATOR_CONTRIBUTIONS)?,
                    CF_MEJEPA_OPERATOR_OVERRIDES: count_cf(reopened.as_ref(), CF_MEJEPA_OPERATOR_OVERRIDES)?,
                    CF_MEJEPA_LIVE_PREDICTIONS: count_cf(reopened.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?
                }
            }
        },
        "report": &report,
        "batch": &batch,
        "weights": &sampler.weights
    });
    context_graph_mejepa::write_json_0600(&fsv_path, &evidence)
        .context("write TASK-EK-015 run FSV")?;
    context_graph_mejepa::write_json_0600(&canonical_path, &evidence)
        .context("write TASK-EK-015 canonical FSV")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-015 FSV")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "fsv_sha256": fsv_sha256,
            "high_multiplier": high_multiplier,
            "low_multiplier": low_multiplier
        }))?
    );
    ensure!(all_passed, "TASK-EK-015 credibility override FSV failed");
    Ok(())
}

fn high_quality_operator_rows(operator_id: &str) -> Vec<OperatorContribution> {
    OperatorContributionEventKind::all()
        .into_iter()
        .enumerate()
        .map(|(idx, kind)| {
            let contribution_id = format!("high-quality-{idx}");
            OperatorContribution::new(
                contribution_id.clone(),
                kind,
                operator_id,
                BASE_TS + 10 + idx as i64,
                OperatorContributionPayload::new(
                    format!("subject:{contribution_id}"),
                    format!("high quality contribution {idx}"),
                    "fsv:task-ek-015",
                    BTreeMap::new(),
                ),
                Some(DownstreamOutcomeRef::new(
                    DownstreamOutcomeKind::CpPhi,
                    format!("cp-phi:{idx}"),
                    None,
                    0.01,
                    Some(0.90),
                    Some(0.91),
                    1.0,
                )),
            )
            .expect("valid high-quality operator row")
        })
        .collect()
}

fn prediction(
    prediction_id: PredictionId,
    task_id: TaskId,
    covered_chunks: Vec<ChunkId>,
    created_at_unix_ms: i64,
) -> Result<RealityPrediction> {
    let mut prediction = RealityPredictionBuilder::from_parts(
        task_id,
        [0x91; 16],
        Language::Rust,
        ConformalSet::try_new(vec![OracleOutcome::Fail], 0.1, 0.2)?,
    )
    .prediction_id(prediction_id.0)
    .covered_chunks(covered_chunks)
    .verdict(Verdict::Fail)
    .confidence_interval(ConformalInterval::default())
    .predicted_oracle_pass(0.2)
    .predicted_test_pass(vec![0.2])
    .predicted_runtime_trace([0.0; 32])
    .ood_score(0.1)
    .calibrated_confidence(0.8)
    .provenance(PredictionProvenance::default())
    .calibration_version("task-ek-015-credibility-override-fsv")
    .build()?;
    prediction.created_at_unix_ms = created_at_unix_ms;
    prediction.validate()?;
    Ok(prediction)
}

fn flush_cfs(db: &rocksdb::DB) -> Result<()> {
    for cf_name in [
        CF_MEJEPA_OPERATOR_CONTRIBUTIONS,
        CF_MEJEPA_OPERATOR_OVERRIDES,
        CF_MEJEPA_LIVE_PREDICTIONS,
    ] {
        let cf = db
            .cf_handle(cf_name)
            .with_context(|| format!("missing {cf_name}"))?;
        db.flush_cf(cf)?;
    }
    Ok(())
}

fn approx_eq(left: f32, right: f32) -> bool {
    (left - right).abs() <= 0.0001
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}
