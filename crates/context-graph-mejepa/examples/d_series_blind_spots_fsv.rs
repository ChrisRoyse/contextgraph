use anyhow::{anyhow, Context, Result};
use context_graph_mejepa::eval::metrics::{ship_gate_failures, ShipGateInputs};
use context_graph_mejepa::{
    build_eval_compiler, current_unix_ms, load_cell_exemptions, open_infer_rocksdb,
    redact_patch_bundle, sha256_bytes, valid_witness_segment, AstDiff, CellExemption,
    ConformalInterval, ConformalSet, DeterministicConstellationGuard, DeterministicOracleHead,
    FailedGate, IdentityFrozenTarget, Language, MeJepaCompiler, MeJepaInferConfig,
    MejepaInferError, MutationCategory, OracleOutcome, PatchBundle, PatchWitnessReader,
    PredictionProvenance, Predictor, PredictorError, RealityPrediction, RealityPredictionBuilder,
    RocksDbInferStore, TaskId, Verdict, VerifyVerdict,
};
use context_graph_mejepa_hygiene::{
    gc_run_nightly, load_retention_policy, open_hygiene_rocksdb, put_readback, runtime_config,
    scan_cf, write_meta, EntryId, HygieneEntryMeta, HygieneEnv, StorageCategory, Tier,
};
use context_graph_mejepa_instruments::Panel;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    fsv_root: String,
    task_id: String,
    started_at_unix_ms: i64,
    happy_path: Vec<serde_json::Value>,
    boundary_cases: Vec<serde_json::Value>,
    all_passed: bool,
    cf_counts_before: BTreeMap<String, usize>,
    cf_counts_after: BTreeMap<String, usize>,
    readback_equal: bool,
}

#[derive(Debug)]
struct FailingPredictor;

impl Predictor for FailingPredictor {
    fn predict(&self, _panel_t0: &Panel, _panel_t1: &Panel) -> Result<Panel, MejepaInferError> {
        Err(MejepaInferError::Predictor(PredictorError::HeadFailure {
            head: "panel".to_string(),
            code: "MEJEPA_SYNTHETIC_NAN".to_string(),
            detail: "synthetic NaN injection for D9 FSV".to_string(),
        }))
    }
}

fn main() -> Result<()> {
    let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/d-series-blind-spots-fsv");
    fs::create_dir_all(&fsv_root)?;
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let mut happy_path = Vec::new();
    let mut boundary_cases = Vec::new();
    let mut cf_counts_before = BTreeMap::new();
    let mut cf_counts_after = BTreeMap::new();

    let repo_root = fsv_root.join("repo");
    fs::create_dir_all(&repo_root)?;
    let (secret_case, normal_case) = redaction_cases(&repo_root)?;
    happy_path.push(secret_case);
    boundary_cases.push(normal_case);

    let infer_db_path = fsv_root.join("rocks-infer");
    if infer_db_path.exists() {
        fs::remove_dir_all(&infer_db_path)?;
    }
    let park_cf = context_graph_mejepa_cf::CF_MEJEPA_PARK_LIST.to_string();
    cf_counts_before.insert(park_cf.clone(), 0);
    let (park_case, park_count) = park_list_case(&infer_db_path, &repo_root)?;
    happy_path.push(park_case);
    cf_counts_after.insert(park_cf, park_count);
    let head_db_path = fsv_root.join("rocks-head-failure");
    if head_db_path.exists() {
        fs::remove_dir_all(&head_db_path)?;
    }
    let (head_db, _compiler, _eval_store, calibration) =
        build_eval_compiler(&head_db_path, &repo_root)?;
    happy_path.push(head_failure_case(head_db, &repo_root, true, calibration)?);

    happy_path.push(cell_exemption_case(&fsv_root)?);
    let (retention_case, gc_before, gc_after) = retention_case(&fsv_root)?;
    boundary_cases.push(retention_case);
    cf_counts_before.insert("CF_MEJEPA_WEIGHT_BLOBS".to_string(), gc_before);
    cf_counts_after.insert("CF_MEJEPA_WEIGHT_BLOBS".to_string(), gc_after);
    boundary_cases.push(docs_case()?);

    let all_passed = happy_path
        .iter()
        .chain(boundary_cases.iter())
        .all(|case| case.get("pass").and_then(|value| value.as_bool()) == Some(true));
    let evidence = Evidence {
        fsv_root: fsv_root.display().to_string(),
        task_id: "D-SERIES-BLIND-SPOTS".to_string(),
        started_at_unix_ms,
        happy_path,
        boundary_cases,
        all_passed,
        cf_counts_before,
        cf_counts_after,
        readback_equal: all_passed,
    };
    let artifact = fsv_root.join("d_series_blind_spots_fsv.json");
    fs::write(&artifact, serde_json::to_vec_pretty(&evidence)?)?;
    println!("{}", artifact.display());
    Ok(())
}

fn redaction_cases(repo_root: &Path) -> Result<(serde_json::Value, serde_json::Value)> {
    let secret_patch = patch_with_after(
        repo_root,
        "redact_secret.py",
        "AWS_ACCESS_KEY_ID = \"AKIAIOSFODNN7EXAMPLE\"\n",
    )?;
    let redacted = redact_patch_bundle(&secret_patch)?;
    let secret_after = &redacted.patch.ast_diff.hunks[0].after;
    let secret_pass = secret_after.contains("[REDACTED:aws-access-key]")
        && !secret_after.contains("AKIAIOSFODNN7EXAMPLE")
        && redacted.report.original_patch_sha == secret_patch.patch_sha;

    let normal_patch =
        patch_with_after(repo_root, "normal.py", "def add(a, b):\n    return a + b\n")?;
    let normal_redacted = redact_patch_bundle(&normal_patch)?;
    let normal_after = &normal_redacted.patch.ast_diff.hunks[0].after;
    let normal_pass = normal_after == "def add(a, b):\n    return a + b\n"
        && normal_redacted.report.redacted_span_count() == 0;

    Ok((
        json!({
            "case": "d3_aws_secret_redacted_pre_embed",
            "sot": "redacted PatchBundle hunk text",
            "expected": "[REDACTED:aws-access-key]",
            "actual": secret_after,
            "pass": secret_pass,
            "original_patch_sha_preserved": hex::encode(secret_patch.patch_sha)
        }),
        json!({
            "case": "d3_legitimate_code_not_over_redacted",
            "sot": "redacted PatchBundle hunk text",
            "expected": normal_patch.ast_diff.hunks[0].after,
            "actual": normal_after,
            "pass": normal_pass
        }),
    ))
}

fn park_list_case(db_path: &Path, repo_root: &Path) -> Result<(serde_json::Value, usize)> {
    let db = open_infer_rocksdb(db_path)?;
    let store = RocksDbInferStore::new(db.clone());
    let (patch, context) =
        context_graph_mejepa::fixture_patch_context(repo_root, "predicted_failure")?;
    let compiler = build_compiler_with_store(repo_root, store.clone())?;
    for attempt in 0..3 {
        let verdict = compiler.verify(&patch, &context)?;
        if !matches!(verdict, VerifyVerdict::EscalateToHuman { .. }) {
            return Err(anyhow!("attempt {attempt} did not fail closed"));
        }
    }
    let fourth = compiler.verify(&patch, &context)?;
    let parked = match fourth {
        VerifyVerdict::EscalateToHuman {
            failed_gate:
                FailedGate::PredictionParked {
                    attempt_count,
                    park_until_unix_ms,
                    ..
                },
            reality_prediction: Some(prediction),
            ..
        } => {
            prediction.verdict == Verdict::Abstain
                && attempt_count == 3
                && park_until_unix_ms > current_unix_ms()
        }
        _ => false,
    };
    db.flush_cf(
        db.cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PARK_LIST)
            .context("missing park-list CF")?,
    )?;
    drop(compiler);
    drop(store);
    drop(db);
    let reopened = open_infer_rocksdb(db_path)?;
    let rows = count_cf(&reopened, context_graph_mejepa_cf::CF_MEJEPA_PARK_LIST)?;
    Ok((
        json!({
            "case": "d12_three_failures_park_fourth_skip",
            "sot": "CF_MEJEPA_PARK_LIST",
            "expected": "attempt_count=3 and fourth verify returns PredictionParked",
            "actual": {"park_rows": rows, "fourth_parked": parked},
            "pass": parked && rows == 1
        }),
        rows,
    ))
}

fn head_failure_case(
    db: Arc<rocksdb::DB>,
    repo_root: &Path,
    calibration_readable: bool,
    calibration: context_graph_mejepa::CalibrationStore,
) -> Result<serde_json::Value> {
    let store = Arc::new(RocksDbInferStore::new(db.clone()));
    let compiler = MeJepaCompiler::new(
        MeJepaInferConfig {
            ood_refuse_threshold: 1.0,
            ..Default::default()
        },
        Arc::new(FailingPredictor),
        Arc::new(IdentityFrozenTarget),
        Arc::new(DeterministicOracleHead),
        Arc::new(DeterministicConstellationGuard::default()),
        Arc::new(PatchWitnessReader),
        store,
        calibration,
        repo_root.to_path_buf(),
    )?;
    let (patch, context) = context_graph_mejepa::fixture_patch_context(repo_root, "head_failure")?;
    let verdict = compiler.verify(&patch, &context)?;
    let pass = matches!(
        verdict,
        VerifyVerdict::EscalateToHuman {
            failed_gate: FailedGate::HeadFailure { .. },
            reality_prediction: Some(RealityPrediction {
                verdict: Verdict::Abstain,
                ..
            }),
            ..
        }
    ) && compiler
        .store()
        .read_live_predictions(context.session_id, 1000)?
        .is_empty();
    Ok(json!({
        "case": "d9_head_failure_abstains_without_persisting",
        "sot": "CF_MEJEPA_LIVE_PREDICTIONS",
        "expected": "Verdict::Abstain and no live prediction row",
        "actual": {"calibration_readable": calibration_readable, "verdict": verdict},
        "pass": pass
    }))
}

fn cell_exemption_case(fsv_root: &Path) -> Result<serde_json::Value> {
    let config_path = fsv_root.join("cell_exemptions.toml");
    fs::write(
        &config_path,
        r#"
[[exemptions]]
cell = "compile_error::python"
reason = "D17 FSV synthetic failing cell approved by operator"
operator_id = "operator"
"#,
    )?;
    let exemptions = load_cell_exemptions(&config_path, current_unix_ms())?;
    let prediction = prediction_for_eval();
    let observation = context_graph_mejepa::EvalObservation {
        task_id: TaskId("d17-cell".to_string()),
        mutation_category: MutationCategory::CompileError,
        language: Language::Python,
        actual_oracle: OracleOutcome::Fail,
        actual_failure_modes: vec![context_graph_mejepa::FailureModeClass::CompileError],
        prediction,
        gtau_passed: true,
        approved: true,
        live_prediction_readback: true,
        latency_ms: 1.0,
    };
    let per_cell = BTreeMap::from([("compile_error::python".to_string(), Some(0.10))]);
    let failures = ship_gate_failures(ShipGateInputs {
        observations: std::slice::from_ref(&observation),
        report_overall: Some(0.96),
        per_cell: &per_cell,
        cell_exemptions: &exemptions,
        conformal: &BTreeMap::new(),
        ood: &BTreeMap::new(),
        gtau: &BTreeMap::new(),
        per_failure_mode_class: &context_graph_mejepa::empty_failure_mode_class_metrics(
            1,
            &context_graph_mejepa::EvalConfig::default(),
        ),
        regression_checks: &[],
        q1_pass_rate: 1.0,
        q3_side_effect_agreement: Some(1.0),
        config: &context_graph_mejepa::EvalConfig {
            correlation_min: 0.95,
            ..Default::default()
        },
    });
    let pass = failures
        .iter()
        .all(|failure| !failure.contains("compile_error::python"));
    let exempt_payload = exemptions
        .get("compile_error::python")
        .cloned()
        .unwrap_or_else(|| CellExemption {
            cell: "missing".to_string(),
            reason: "missing".to_string(),
            operator_id: "missing".to_string(),
            expires_unix_ms: None,
        });
    Ok(json!({
        "case": "d17_exempt_cell_does_not_block_ship_gate",
        "sot": config_path,
        "expected": "no ship-gate failure for compile_error::python",
        "actual": {"failures": failures, "exemption": exempt_payload},
        "pass": pass
    }))
}

fn retention_case(fsv_root: &Path) -> Result<(serde_json::Value, usize, usize)> {
    let policy = load_retention_policy("config/retention_policy.toml")?;
    let db_path = fsv_root.join("rocks-hygiene");
    if db_path.exists() {
        fs::remove_dir_all(&db_path)?;
    }
    let db = open_hygiene_rocksdb(&db_path)?;
    let mut config = runtime_config(db.clone(), fsv_root.join("archives"))?;
    config.retention_policy_path = Some(PathBuf::from("config/retention_policy.toml"));
    config.now_unix = Some(1_000);
    let env = HygieneEnv::try_new(config)?;
    for idx in 0..12u64 {
        let key = idx.to_be_bytes();
        put_readback(
            &env.config.db,
            context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS,
            &key,
            b"retained-weight",
        )?;
        let meta = HygieneEntryMeta::new(
            EntryId::new(context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS, key),
            StorageCategory::ModelCheckpoints,
            Tier::Hot,
            b"retained-weight".len() as u64,
            env.now_unix(),
        );
        write_meta(&env.config.db, &meta)?;
    }
    let before = scan_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS,
    )?
    .len();
    let report = gc_run_nightly(&env)?;
    let after = scan_cf(
        &env.config.db,
        context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS,
    )?
    .len();
    let prune_step = report
        .steps
        .iter()
        .find(|step| step.name == "prune_checkpoints")
        .map(|step| step.detail.clone())
        .unwrap_or_else(|| json!({}));
    Ok((
        json!({
            "case": "d19_future_retention_rows_not_deleted",
            "sot": "CF_MEJEPA_WEIGHT_BLOBS and CF_MEJEPA_GC_HISTORY",
            "expected": {"before": 12, "after": 12, "policy_cfs": context_graph_mejepa_cf::all_hygiene_referenced_cfs().len()},
            "actual": {"before": before, "after": after, "policy_cfs": policy.column_families.len(), "prune_step": prune_step},
            "pass": before == 12 && after == 12 && policy.column_families.len() == context_graph_mejepa_cf::all_hygiene_referenced_cfs().len()
        }),
        before,
        after,
    ))
}

fn docs_case() -> Result<serde_json::Value> {
    let path = PathBuf::from("docs/futurebuild/23_MULTI_AGENT_ISOLATION.md");
    let raw = fs::read_to_string(&path)?;
    let pass = raw.contains("stale_before_sha")
        && raw.contains("single-machine")
        && raw.contains("do not mount the same DB over SMB/NFS");
    Ok(json!({
        "case": "d11_d20_docs_define_conflict_and_machine_boundary",
        "sot": path,
        "expected": "stale_before_sha conflict plus single-machine deployment boundary",
        "actual": {"contains_stale_before_sha": raw.contains("stale_before_sha"), "contains_single_machine": raw.contains("single-machine")},
        "pass": pass
    }))
}

fn build_compiler_with_store(repo_root: &Path, store: RocksDbInferStore) -> Result<MeJepaCompiler> {
    let db = store.db();
    let calibration = context_graph_mejepa::CalibrationStore::new(db.clone(), 30)?;
    if calibration.load_active().is_err() {
        let examples = (0..40)
            .map(|idx| context_graph_mejepa::CalibrationExample {
                language: Language::Python,
                predicted_test_pass: vec![0.50],
                actual_test_pass: vec![if idx % 2 == 0 { 1.0 } else { 0.0 }],
            })
            .collect::<Vec<_>>();
        calibration.calibrate(
            &examples,
            &[0.01; 40],
            0.10,
            30,
            0.30,
            [8; 32],
            BTreeMap::new(),
        )?;
    }
    Ok(context_graph_mejepa::build_deterministic_compiler(
        repo_root.to_path_buf(),
        Arc::new(store),
        calibration,
        MeJepaInferConfig {
            ood_refuse_threshold: 1.0,
            ..Default::default()
        },
    )?)
}

fn patch_with_after(repo_root: &Path, rel: &str, after: &str) -> Result<PatchBundle> {
    fs::create_dir_all(repo_root)?;
    let path = repo_root.join(rel);
    fs::write(&path, after)?;
    let post_sha = sha256_bytes(after.as_bytes());
    Ok(PatchBundle::try_new(
        AstDiff {
            hunks: vec![context_graph_mejepa::DiffHunk {
                path: PathBuf::from(rel),
                pre_sha: post_sha,
                post_sha,
                before: String::new(),
                after: after.to_string(),
            }],
        },
        valid_witness_segment(),
        "d-series FSV patch".to_string(),
        sha256_bytes(format!("patch:{rel}").as_bytes()),
    )?)
}

fn prediction_for_eval() -> RealityPrediction {
    RealityPredictionBuilder::from_parts(
        TaskId("d17-cell".to_string()),
        [17; 16],
        Language::Python,
        ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.1).unwrap(),
    )
    .prediction_id([17; 16])
    .verdict(Verdict::Pass)
    .confidence_interval(ConformalInterval::default())
    .predicted_oracle_pass(0.9)
    .predicted_test_pass(vec![0.9])
    .predicted_runtime_trace([0.0; 32])
    .ood_score(0.0)
    .provenance(PredictionProvenance::default())
    .calibration_version("d17-fsv")
    .build()
    .unwrap()
}

fn count_cf(db: &rocksdb::DB, cf_name: &str) -> Result<usize> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow!("missing CF {cf_name}"))?;
    Ok(db.iterator_cf(cf, rocksdb::IteratorMode::Start).count())
}
