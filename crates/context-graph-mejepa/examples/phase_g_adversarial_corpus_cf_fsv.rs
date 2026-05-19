use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::{
    load_adversarial_fixture_cases, open_infer_rocksdb, read_adversarial_corpus,
    validate_adversarial_corpus, write_adversarial_case_sync_readback,
    write_adversarial_patch_artifacts, AdversarialCase, AdversarialKind, ADVERSARIAL_SOURCE_CORPUS,
    CF_MEJEPA_ADVERSARIAL_CORPUS, EXPECTED_ADVERSARIAL_CASES, EXPECTED_ADVERSARIAL_CASES_PER_KIND,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-060-adversarial-corpus-fsv";
const CORPUS_ROOT: &str = "/zfs/archive/contextgraph/corpus/python-adversarial-v1";

fn main() -> Result<()> {
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{}", now_ms()));
    fs::create_dir_all(&run_root)?;
    let report_path = run_root.join("adversarial_corpus_cf_fsv.json");
    let db_path = run_root.join("infer-rocksdb");
    let fixture_root = fixture_root()?;
    let cases = load_adversarial_fixture_cases(&fixture_root)?;
    let corpus_counts = counts_by_kind(&cases);
    let db = open_infer_rocksdb(&db_path)?;
    let cf_count_before = count_adversarial_cf(db.as_ref())?;
    let patch_paths = write_adversarial_patch_artifacts(&cases, Path::new(CORPUS_ROOT))?;
    for case in &cases {
        write_adversarial_case_sync_readback(db.as_ref(), case)?;
    }
    flush_cf(db.as_ref(), CF_MEJEPA_ADVERSARIAL_CORPUS)?;
    let readback = read_adversarial_corpus(db.as_ref())?;
    validate_adversarial_corpus(&readback)?;
    let cf_count_after = count_adversarial_cf(db.as_ref())?;
    db.cancel_all_background_work(true);
    drop(db);
    let reopened = open_infer_rocksdb(&db_path)?;
    let reopened_cases = read_adversarial_corpus(reopened.as_ref())?;
    let reopened_count = count_adversarial_cf(reopened.as_ref())?;
    let boundary_cases = boundary_cases(&cases);
    let all_passed = cases.len() == EXPECTED_ADVERSARIAL_CASES
        && readback == cases
        && reopened_cases == cases
        && cf_count_after == EXPECTED_ADVERSARIAL_CASES as u64
        && reopened_count == cf_count_after
        && patch_paths.len() == EXPECTED_ADVERSARIAL_CASES
        && corpus_counts
            .values()
            .all(|count| *count == EXPECTED_ADVERSARIAL_CASES_PER_KIND as u64)
        && boundary_cases.iter().all(|case| case["pass"] == true);
    let report = json!({
        "task": "TASK-PY-G-060",
        "issue": 279,
        "all_passed": all_passed,
        "fsv_root": run_root,
        "source_of_truth": {
            "db_path": db_path,
            "cf": CF_MEJEPA_ADVERSARIAL_CORPUS,
            "fixture_root": fixture_root,
            "durable_corpus_root": CORPUS_ROOT,
            "source_corpus": ADVERSARIAL_SOURCE_CORPUS
        },
        "counts": {
            "loaded_cases": cases.len(),
            "cf_count_before": cf_count_before,
            "cf_count_after": cf_count_after,
            "reopened_count": reopened_count,
            "patch_artifacts": patch_paths.len(),
            "by_kind": corpus_counts
        },
        "sample_cases": cases.iter().take(5).map(|case| json!({
            "case_id": case.case_id,
            "kind": case.kind,
            "expected_verdict": case.expected_verdict,
            "patch_sha256": hex::encode(case.patch_sha256)
        })).collect::<Vec<_>>(),
        "boundary_cases": boundary_cases,
    });
    write_json_readback(&report_path, &report)?;
    ensure!(all_passed, "TASK-PY-G-060 adversarial corpus CF FSV failed");
    println!("all_passed: {all_passed}");
    println!("artifact_path: {}", report_path.display());
    println!("cf_count_after: {cf_count_after}");
    println!("sha256: {}", file_sha256(&report_path)?);
    Ok(())
}

fn boundary_cases(cases: &[AdversarialCase]) -> Vec<Value> {
    let mut short = cases.to_vec();
    short.pop();
    let count_err = validate_adversarial_corpus(&short).expect_err("29 cases must fail closed");

    let mut duplicate = cases.to_vec();
    duplicate[1] = duplicate[0].clone();
    let duplicate_err =
        validate_adversarial_corpus(&duplicate).expect_err("duplicate id must fail closed");

    let mut empty_patch = cases[0].clone();
    empty_patch.patch_text.clear();
    let empty_patch_err = empty_patch
        .validate()
        .expect_err("empty patch must fail closed");

    let mut unsafe_target = cases[0].clone();
    unsafe_target.target_path = PathBuf::from("../escape.py");
    let unsafe_target_err = unsafe_target
        .validate()
        .expect_err("unsafe target must fail closed");

    vec![
        boundary("row_count_not_30_rejected", &count_err, "case_count"),
        boundary("duplicate_case_id_rejected", &duplicate_err, "case_id"),
        boundary("empty_patch_rejected", &empty_patch_err, "patch_text"),
        boundary(
            "unsafe_target_rejected",
            &unsafe_target_err,
            "UNSAFE_TARGET",
        ),
    ]
}

fn boundary(case: &str, err: &context_graph_mejepa::MejepaInferError, needle: &str) -> Value {
    json!({
        "case": case,
        "expected_error_code": "MEJEPA_INFER_INVALID_INPUT",
        "actual_error_code": err.code(),
        "error": err.to_string(),
        "pass": err.code() == "MEJEPA_INFER_INVALID_INPUT" && err.to_string().contains(needle)
    })
}

fn counts_by_kind(cases: &[AdversarialCase]) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for kind in AdversarialKind::all() {
        counts.insert(kind.slug().to_string(), 0);
    }
    for case in cases {
        *counts.entry(case.kind.slug().to_string()).or_insert(0) += 1;
    }
    counts
}

fn fixture_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/adversarial")
        .canonicalize()
        .context("canonicalize adversarial fixture root")
}

fn count_adversarial_cf(db: &rocksdb::DB) -> Result<u64> {
    context_graph_mejepa::calibration::count_cf(db, CF_MEJEPA_ADVERSARIAL_CORPUS)
        .map_err(Into::into)
}

fn flush_cf(db: &rocksdb::DB, cf_name: &str) -> Result<()> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing CF {cf_name}"))?;
    db.flush_cf(cf)?;
    Ok(())
}

fn write_json_readback(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, &bytes)?;
    let readback: Value = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(readback == *value, "json readback mismatch");
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
    Ok(hex::encode(Sha256::digest(fs::read(path)?)))
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_millis()
}
