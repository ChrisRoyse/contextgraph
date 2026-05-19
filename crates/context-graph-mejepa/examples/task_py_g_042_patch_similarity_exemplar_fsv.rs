use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::patch_similarity::{
    build_patch_similarity_index, closest_exemplars, load_patch_similarity_index,
    PatchExemplarRecord, PatchSimilarityQuery, PatchSimilaritySearchStatus,
    PATCH_SIMILARITY_DEFAULT_K, PATCH_SIMILARITY_META_KEY, PATCH_SIMILARITY_SCHEMA_VERSION,
    PATCH_SIMILARITY_STRUCTURAL_DIM,
};
use context_graph_mejepa::store::RocksDbInferStore;
use context_graph_mejepa::{
    open_infer_rocksdb, write_json_0600, AgentClaimGraph, ConformalInterval, ConformalMethod,
    ConformalSet, ExemplarMatch, FailureModeClass, Language, MejepaStore, MutationCategory,
    OracleOutcome, PredictionProvenance, RealityPrediction, ReasoningClass, TaskId, Verdict,
    WitnessHash,
};
use context_graph_mejepa_cf::{CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_PATCH_SIMILARITY_META};
use context_graph_mejepa_instruments::{OracleVerdict, PerTestOutcome, TestOutcome};
use serde::Serialize;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-042-patch-similarity-exemplar-fsv";
const INDEX_ROOT: &str = "/mnt/d/contextgraph/storage/patch-similarity-index-v1";

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct HoldoutReport {
    query_count: usize,
    same_failure_mode_top1: usize,
    top1_same_failure_mode_accuracy: f32,
    threshold: f32,
    passed: bool,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct EdgeCaseReport {
    zero_norm_error_code: String,
    stale_hash_error_code: String,
    no_neighborhood_status: PatchSimilaritySearchStatus,
    no_neighborhood_returned: usize,
    k_gt_row_count_requested: usize,
    k_gt_row_count_returned: usize,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct LivePredictionReport {
    prediction_id_hex: String,
    reopened_live_prediction_count: usize,
    closest_exemplars_count: usize,
    top_exemplar_prediction_id_hex: String,
    top_exemplar_failure_mode_class: Option<FailureModeClass>,
    top_exemplar_evidence_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct TaskPyG042FsvReport {
    task_id: String,
    issue: String,
    status: String,
    schema_version: u32,
    corpus_snapshot_hash: String,
    index_dir: PathBuf,
    graph_file: PathBuf,
    records_file: PathBuf,
    metadata_file: PathBuf,
    metadata_cf: String,
    metadata_cf_key_hex: String,
    metadata_cf_readback_equal: bool,
    metadata_cf_reopened_readback_equal: bool,
    live_prediction_cf: String,
    row_count: usize,
    vector_dim: usize,
    graph_sha256: String,
    records_sha256: String,
    holdout: HoldoutReport,
    edge_cases: EdgeCaseReport,
    live_prediction: LivePredictionReport,
}

fn main() -> Result<()> {
    let run_id = chrono::Utc::now().timestamp_millis();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{run_id}"));
    let index_dir = PathBuf::from(INDEX_ROOT).join(format!("task-py-g-042-fsv-{run_id}"));
    let db_path = run_root.join("rocksdb");
    fs::create_dir_all(&run_root).context("create FSV run root")?;
    fs::create_dir_all(&index_dir).context("create index dir")?;

    let corpus_snapshot_hash = sha256_hex(b"task-py-g-042-patch-similarity-corpus-v1");
    let records = exemplar_records();

    let session_id = [0x42; 16];
    let live_prediction_id = [0x61; 16];
    let (
        metadata_cf_readback_equal,
        metadata_cf_reopened_readback_equal,
        build_report,
        holdout,
        edge_cases,
        live_prediction,
    ) = {
        let db = open_infer_rocksdb(&db_path).context("open FSV RocksDB")?;
        let build_report = build_patch_similarity_index(
            records,
            &index_dir,
            corpus_snapshot_hash.clone(),
            Some(db.as_ref()),
        )
        .context("build patch-similarity index")?;
        let metadata_bytes =
            serde_json::to_vec_pretty(&build_report.metadata).context("encode metadata")?;
        let metadata_cf_readback_equal = read_cf_bytes(
            db.as_ref(),
            CF_MEJEPA_PATCH_SIMILARITY_META,
            PATCH_SIMILARITY_META_KEY,
        )? == metadata_bytes;

        let index = load_patch_similarity_index(&index_dir, &corpus_snapshot_hash)
            .context("load built patch-similarity index")?;
        let holdout = run_holdout(&index).context("run holdout")?;
        ensure!(
            holdout.passed,
            "holdout did not pass TASK-PY-G-042 threshold"
        );
        let edge_cases =
            run_edge_cases(&index_dir, &corpus_snapshot_hash, &index).context("edge cases")?;

        let live_query =
            PatchSimilarityQuery::new(cluster_vector(0, 999), PATCH_SIMILARITY_DEFAULT_K, true)
                .context("build live exemplar query")?;
        let live_exemplars = closest_exemplars(&index, live_query)
            .context("live closest_exemplars query")?
            .exemplars;
        let prediction = live_prediction_row(live_prediction_id, session_id, live_exemplars)
            .context("build live prediction row")?;
        let store = RocksDbInferStore::new(db.clone());
        store
            .write_live_prediction(&prediction)
            .context("write live prediction with closest_exemplars")?;
        let live_cf = db
            .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
            .context("missing live predictions CF")?;
        db.flush_cf(live_cf).context("flush live predictions CF")?;
        drop(store);
        drop(db);

        let reopened = open_infer_rocksdb(&db_path).context("reopen FSV RocksDB")?;
        let reopened_metadata_equal = read_cf_bytes(
            reopened.as_ref(),
            CF_MEJEPA_PATCH_SIMILARITY_META,
            PATCH_SIMILARITY_META_KEY,
        )? == metadata_bytes;
        let reopened_store = RocksDbInferStore::new(reopened.clone());
        let live_rows = reopened_store
            .read_live_predictions(session_id, 10)
            .context("read reopened live predictions")?;
        let live_prediction = live_prediction_report(&live_rows, live_prediction_id)
            .context("live prediction readback report")?;

        (
            metadata_cf_readback_equal,
            reopened_metadata_equal,
            build_report,
            holdout,
            edge_cases,
            live_prediction,
        )
    };

    ensure!(
        metadata_cf_readback_equal && metadata_cf_reopened_readback_equal,
        "metadata CF readback did not match written metadata bytes"
    );
    ensure!(
        live_prediction.closest_exemplars_count == PATCH_SIMILARITY_DEFAULT_K,
        "live prediction closest_exemplars count mismatch"
    );

    let report = TaskPyG042FsvReport {
        task_id: "TASK-PY-G-042".to_string(),
        issue: "https://github.com/ChrisRoyse/contextgraph/issues/261".to_string(),
        status: "pass".to_string(),
        schema_version: PATCH_SIMILARITY_SCHEMA_VERSION,
        corpus_snapshot_hash,
        index_dir: build_report.index_dir.clone(),
        graph_file: build_report.graph_file.clone(),
        records_file: build_report.records_file.clone(),
        metadata_file: build_report.metadata_file.clone(),
        metadata_cf: CF_MEJEPA_PATCH_SIMILARITY_META.to_string(),
        metadata_cf_key_hex: hex::encode(PATCH_SIMILARITY_META_KEY),
        metadata_cf_readback_equal,
        metadata_cf_reopened_readback_equal,
        live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        row_count: build_report.metadata.row_count,
        vector_dim: build_report.metadata.vector_dim,
        graph_sha256: build_report.metadata.graph_sha256,
        records_sha256: build_report.metadata.records_sha256,
        holdout,
        edge_cases,
        live_prediction,
    };

    let report_path = run_root.join("patch_similarity_exemplar_fsv.json");
    write_json_0600(&report_path, &report).context("write FSV report")?;
    let report_sha256 = sha256_file(&report_path).context("hash FSV report")?;
    let canonical_path = PathBuf::from(FSV_ROOT).join("patch_similarity_exemplar_fsv.latest.json");
    write_json_0600(&canonical_path, &report).context("write latest FSV report")?;

    println!("TASK-PY-G-042 FSV PASS");
    println!("report_path={}", report_path.display());
    println!("report_sha256={report_sha256}");
    println!("index_dir={}", report.index_dir.display());
    println!(
        "holdout_top1_same_failure_mode_accuracy={:.3}",
        report.holdout.top1_same_failure_mode_accuracy
    );
    Ok(())
}

fn exemplar_records() -> Vec<PatchExemplarRecord> {
    let mut records = Vec::new();
    for idx in 0..20_u8 {
        records.push(record(
            idx + 1,
            0,
            MutationCategory::OffByOne,
            FailureModeClass::OffByOne,
        ));
    }
    for idx in 0..20_u8 {
        records.push(record(
            idx + 81,
            1,
            MutationCategory::SwapVariable,
            FailureModeClass::WrongAlgorithm,
        ));
    }
    records
}

fn record(
    id: u8,
    cluster: usize,
    mutation_kind: MutationCategory,
    failure_mode_class: FailureModeClass,
) -> PatchExemplarRecord {
    PatchExemplarRecord {
        exemplar_prediction_id: [id; 16],
        task_id: TaskId(format!("py-g-042-exemplar-{id:03}")),
        mutation_kind,
        oracle_outcome: oracle_verdict(id),
        failure_mode_class,
        witness_hash: WitnessHash([id; 32]),
        evidence_path: format!(
            "/mnt/d/contextgraph/fsv/task-py-g-042/evidence/exemplar-{id:03}.json"
        ),
        diff_summary: format!("cluster={cluster} mutation={}", mutation_kind.slug()),
        vector: cluster_vector(cluster, id as u64),
    }
}

fn oracle_verdict(id: u8) -> OracleVerdict {
    OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: format!("tests/test_py_g_042.py::case_{id:03}"),
            outcome: TestOutcome::Fail,
            runtime_ms: 10 + i64::from(id),
        }],
        exception: None,
    }
}

fn cluster_vector(cluster: usize, seed: u64) -> Vec<f32> {
    let mut vector = vec![0.0_f32; PATCH_SIMILARITY_STRUCTURAL_DIM];
    let primary_axis = if cluster == 0 { 0 } else { 64 };
    vector[primary_axis] = 1.0;
    for offset in 0..8 {
        let idx = 128
            + ((seed as usize * 17 + offset * 31 + cluster * 7)
                % (PATCH_SIMILARITY_STRUCTURAL_DIM - 128));
        vector[idx] = 0.001 * (offset as f32 + 1.0);
    }
    vector
}

fn run_holdout(index: &context_graph_mejepa::PatchSimilarityIndex) -> Result<HoldoutReport> {
    let mut same_failure_mode_top1 = 0usize;
    let query_count = 100usize;
    for idx in 0..query_count {
        let cluster = idx % 2;
        let expected = if cluster == 0 {
            FailureModeClass::OffByOne
        } else {
            FailureModeClass::WrongAlgorithm
        };
        let query =
            PatchSimilarityQuery::new(cluster_vector(cluster, 10_000 + idx as u64), 5, true)
                .context("holdout query")?;
        let outcome = closest_exemplars(index, query).context("holdout search")?;
        let top1 = outcome
            .exemplars
            .first()
            .context("holdout search returned no exemplars")?;
        if top1.failure_mode_class == Some(expected) {
            same_failure_mode_top1 += 1;
        }
    }
    let accuracy = same_failure_mode_top1 as f32 / query_count as f32;
    Ok(HoldoutReport {
        query_count,
        same_failure_mode_top1,
        top1_same_failure_mode_accuracy: accuracy,
        threshold: 0.70,
        passed: accuracy >= 0.70,
    })
}

fn run_edge_cases(
    index_dir: &Path,
    corpus_snapshot_hash: &str,
    index: &context_graph_mejepa::PatchSimilarityIndex,
) -> Result<EdgeCaseReport> {
    let zero_norm_error_code =
        PatchSimilarityQuery::new(vec![0.0_f32; PATCH_SIMILARITY_STRUCTURAL_DIM], 1, true)
            .expect_err("zero-norm query must fail")
            .code()
            .to_string();

    let stale_hash = sha256_hex(b"task-py-g-042-stale-corpus");
    ensure!(
        stale_hash != corpus_snapshot_hash,
        "stale hash fixture unexpectedly equals corpus hash"
    );
    let stale_hash_error_code = match load_patch_similarity_index(index_dir, &stale_hash) {
        Ok(_) => anyhow::bail!("stale corpus hash unexpectedly loaded"),
        Err(err) => err.code().to_string(),
    };

    let no_neighborhood = closest_exemplars(
        index,
        PatchSimilarityQuery::new(cluster_vector(0, 777), 5, false)?,
    )?;

    let k_gt_row_count_requested = index.row_count() + 17;
    let oversized = closest_exemplars(
        index,
        PatchSimilarityQuery::new(cluster_vector(1, 778), k_gt_row_count_requested, true)?,
    )?;
    ensure!(
        oversized.returned == index.row_count(),
        "k > row_count should return every indexed row"
    );

    Ok(EdgeCaseReport {
        zero_norm_error_code,
        stale_hash_error_code,
        no_neighborhood_status: no_neighborhood.status,
        no_neighborhood_returned: no_neighborhood.returned,
        k_gt_row_count_requested,
        k_gt_row_count_returned: oversized.returned,
    })
}

fn live_prediction_row(
    prediction_id: [u8; 16],
    session_id: [u8; 16],
    closest_exemplars: Vec<ExemplarMatch>,
) -> Result<RealityPrediction> {
    RealityPrediction::try_new(RealityPrediction {
        prediction_id,
        witness_hash: WitnessHash([0x77; 32]),
        task_id: TaskId("task-py-g-042-live-readback".to_string()),
        session_id,
        language: Language::Python,
        covered_chunks: Vec::new(),
        verdict: Verdict::Fail,
        confidence_interval: ConformalInterval {
            lower: 0.76,
            upper: 0.92,
            method: ConformalMethod::SplitConformal,
            coverage_target: 0.90,
            empirical_coverage: 0.91,
        },
        predicted_oracle_pass: 0.12,
        predicted_test_pass: vec![0.12],
        predicted_runtime_trace: [0.0; 32],
        ood_score: 0.03,
        outcome_set: ConformalSet::try_new(vec![OracleOutcome::Fail], 0.1, 0.2)?,
        calibrated_confidence: 0.84,
        degraded_status: false,
        granger_attestations: BTreeMap::from([("patch_similarity:fsv".to_string(), 0.93)]),
        predicted_failure_modes: Vec::new(),
        predicted_failed_tests: Vec::new(),
        predicted_works: Vec::new(),
        predicted_uncovered_paths: Vec::new(),
        predicted_flaky_tests: Vec::new(),
        guard_violations: Vec::new(),
        per_slot_ood_reasons: Vec::new(),
        closest_exemplars,
        predicted_edge_cases: Vec::new(),
        predicted_latent_bugs: Vec::new(),
        predicted_tech_debt_added: Vec::new(),
        predicted_dead_code: Vec::new(),
        predicted_redundant_code: Vec::new(),
        predicted_perf_regressions: Vec::new(),
        predicted_security_concerns: Vec::new(),
        predicted_accuracy_degradations: Vec::new(),
        predicted_cost_regressions: Vec::new(),
        predicted_reasoning_class: ReasoningClass::MostlyCorrect,
        agent_claim_graph: AgentClaimGraph::default(),
        claim_reconciliation: Vec::new(),
        reality_impact: None,
        provenance: PredictionProvenance {
            predictor_version: "task-py-g-042-fsv".to_string(),
            constellation_version: "task-py-g-042-fsv".to_string(),
            calibration_version: "task-py-g-042-fsv".to_string(),
            active_pointer: hex::encode(prediction_id),
        },
        source_panel_sha: sha256_bytes(b"task-py-g-042-source-panel"),
        calibration_version: "task-py-g-042-fsv".to_string(),
        created_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        matched_fingerprint: None,
        unknown_candidate_id: None,
        constellation_intelligence: None,
        slot_attributions: Vec::new(),
    })
    .context("validate live prediction")
}

fn live_prediction_report(
    rows: &[RealityPrediction],
    prediction_id: [u8; 16],
) -> Result<LivePredictionReport> {
    let row = rows
        .iter()
        .find(|row| row.prediction_id == prediction_id)
        .context("missing reopened live prediction row")?;
    let top = row
        .closest_exemplars
        .first()
        .context("reopened live prediction has no closest_exemplars")?;
    let top_id = top
        .exemplar_prediction_id
        .context("top exemplar missing exemplar_prediction_id")?;
    Ok(LivePredictionReport {
        prediction_id_hex: hex::encode(prediction_id),
        reopened_live_prediction_count: rows.len(),
        closest_exemplars_count: row.closest_exemplars.len(),
        top_exemplar_prediction_id_hex: hex::encode(top_id),
        top_exemplar_failure_mode_class: top.failure_mode_class,
        top_exemplar_evidence_path: top.evidence_path.clone(),
    })
}

fn read_cf_bytes(db: &rocksdb::DB, cf_name: &str, key: &[u8]) -> Result<Vec<u8>> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing CF {cf_name}"))?;
    db.get_cf(cf, key)?
        .with_context(|| format!("missing key {} in {cf_name}", hex::encode(key)))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(sha256_hex(&bytes))
}
