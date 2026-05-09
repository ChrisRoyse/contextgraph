// Manual Full State Verification for the Phase 0 SWE-bench oracle bridge.
//
// Trigger
//   python3 -m swebench.harness.run_evaluation --predictions_path gold
//   --instance_ids sympy__sympy-20590 ... (invoked from Rust via the new
//   `run_swebench_lite_oracle` API).
//
// Sources of truth on disk
//   - <run_root>/gold.<run_id>.json
//   - <run_root>/logs/run_evaluation/<run_id>/gold/sympy__sympy-20590/
//       report.json, run_instance.log, test_output.txt, patch.diff
//   - /tmp/contextgraph-mejepa-corpus-fsv/swebench-oracle/oracle-store/
//   - /tmp/contextgraph-mejepa-corpus-fsv/swebench-oracle-evidence.json
//
// Expected: with --predictions_path=gold the harness applies the dataset's
// canonical fix; the per-instance report.json reports
// `resolved=true`, `patch_successfully_applied=true`, every FAIL_TO_PASS
// test in `success`, and zero P2P failures. The OracleVerdict produced
// from that report has all-Pass per_test rows and `exception=None`.
// The verdict is persisted to RocksDB CF_MEJEPA_ORACLE_VERDICTS and read
// back; the evidence JSON records every relevant before/after value.
//
// Skip behaviour: this FSV is gated on three external preconditions
//   1. SWE-bench venv python at $CGSWEBENCH_VENV (or default path);
//   2. Docker daemon reachable;
//   3. The official Lite image for the smoke instance present locally.
// If any precondition is missing the FSV prints a SKIP message that names
// the missing precondition and exits 0. This ensures the binary runs in
// CI without Docker but shouts loudly when run in an environment that
// claims to satisfy the preconditions but does not.

use context_graph_mejepa_corpus::oracle::{
    OracleStore, CF_MEJEPA_CORPUS_PROVENANCE, CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS,
};
use context_graph_mejepa_corpus::swebench::{
    parse_swebench_instance_report, run_swebench_lite_oracle, swebench_report_to_verdict,
    SwebenchOracleConfig, SwebenchOracleResult, SwebenchPredictionMode,
};
use context_graph_mejepa_corpus::{
    compute_corpus_sha256, CorpusProvenance, CorpusProvenanceRow, Language, MutationCategory,
    MutationOutcome, MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1,
};
use context_graph_mejepa_instruments::{OracleVerdict, TestOutcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const SMOKE_INSTANCE: &str = "sympy__sympy-20590";
const FSV_ROOT: &str = "/tmp/contextgraph-mejepa-corpus-fsv";

fn default_venv_python() -> PathBuf {
    if let Ok(custom) = std::env::var("CGSWEBENCH_VENV") {
        return PathBuf::from(custom).join("bin").join("python");
    }
    PathBuf::from("/home/cabdru/.cache/contextgraph-swebench/venv/bin/python")
}

fn iso_now() -> String {
    // Docker container-name regex is [a-zA-Z0-9][a-zA-Z0-9_.-]+, which is
    // why this slug uses dot+dash instead of `+`. Run ids embedded in the
    // SWE-bench harness become container names; non-conforming run ids
    // surface as opaque docker.errors.APIError 400s, so we keep this safe.
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch.{secs}")
}

fn ensure(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("swebench-oracle FSV invariant failed: {label}").into())
    }
}

fn check_docker_image(image: &str) -> bool {
    let out = Command::new("docker")
        .args(["image", "inspect", image, "--format", "{{.Id}}"])
        .output();
    matches!(out, Ok(o) if o.status.success())
}

fn write_evidence(path: &Path, payload: &Value) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(payload)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &text)?;
    fs::rename(&tmp, path)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    if &readback != payload {
        return Err(format!("evidence readback mismatch at {}", path.display()).into());
    }
    Ok(())
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn sha256_json_value(value: &Value) -> String {
    sha256_text(&serde_json::to_string(value).expect("JSON value serialises"))
}

fn smoke_image_ref(instance_id: &str) -> String {
    format!(
        "swebench/sweb.eval.x86_64.{}:latest",
        instance_id.to_lowercase().replace("__", "_1776_")
    )
}

fn check_preconditions(venv: &Path, instance_id: &str) -> Option<String> {
    if !venv.is_file() {
        return Some(format!(
            "venv python missing at {} (set CGSWEBENCH_VENV or install per scripts/swebench_lite_ops.py)",
            venv.display()
        ));
    }
    if !Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some("docker daemon unreachable".to_string());
    }
    let image = smoke_image_ref(instance_id);
    if !check_docker_image(&image) {
        return Some(format!(
            "official Lite image missing locally: {image}; run `python3 scripts/swebench_lite_ops.py pull-images --instance-ids {instance_id}` first"
        ));
    }
    None
}

fn assert_all_pass_resolved(result: &SwebenchOracleResult) -> FsvResult<()> {
    let report = &result.instance_report;
    ensure("instance_report.resolved == true", report.resolved)?;
    ensure(
        "instance_report.patch_successfully_applied == true",
        report.patch_successfully_applied,
    )?;
    ensure("instance_report.patch_exists == true", report.patch_exists)?;
    ensure(
        "instance_report.patch_is_None == false",
        !report.patch_is_none,
    )?;

    let verdict = &result.verdict;
    ensure(
        "verdict.exception is None for resolved-gold run",
        verdict.exception.is_none(),
    )?;
    ensure(
        "verdict.per_test is non-empty",
        !verdict.per_test.is_empty(),
    )?;
    for row in &verdict.per_test {
        ensure(
            &format!(
                "verdict.per_test[{}].outcome == Pass for resolved gold run",
                row.test_id
            ),
            row.outcome == TestOutcome::Pass,
        )?;
    }
    ensure("verdict.all_passed() == true", verdict.all_passed())?;
    Ok(())
}

fn run_for_mode(
    mode: SwebenchPredictionMode,
) -> FsvResult<(SwebenchOracleResult, PathBuf, String)> {
    let venv = default_venv_python();
    if let Some(reason) = check_preconditions(&venv, SMOKE_INSTANCE) {
        return Err(format!("PRECONDITION: {reason}").into());
    }

    let run_id = format!(
        "mejepa-swebench-oracle-{}-{}",
        match mode {
            SwebenchPredictionMode::Gold => "gold",
            SwebenchPredictionMode::Custom(_) => "custom",
        },
        iso_now()
    );
    let run_root = PathBuf::from(FSV_ROOT)
        .join("swebench-oracle")
        .join(&run_id);
    if run_root.exists() {
        fs::remove_dir_all(&run_root)?;
    }
    fs::create_dir_all(&run_root)?;

    let config = SwebenchOracleConfig::defaults_for(
        SMOKE_INSTANCE,
        run_id.clone(),
        run_root.clone(),
        venv,
        mode,
    );
    let result = run_swebench_lite_oracle(&config)?;
    Ok((result, run_root, run_id))
}

fn run_gold() -> FsvResult<Value> {
    println!("[swebench-oracle-fsv] running gold mode against {SMOKE_INSTANCE}");
    let (result, run_root, run_id) = run_for_mode(SwebenchPredictionMode::Gold)?;

    assert_all_pass_resolved(&result)?;

    // Round-trip through the parser one more time, then through the verdict
    // converter, to assert the on-disk report.json is the SoT and the parser
    // is deterministic.
    let report_text = fs::read_to_string(&result.artifacts.instance_report)?;
    let reparsed = parse_swebench_instance_report(SMOKE_INSTANCE, &report_text)?;
    ensure(
        "reparsed report.json matches in-memory copy",
        reparsed == result.instance_report,
    )?;
    let reverdict = swebench_report_to_verdict(&reparsed, None)?;
    ensure(
        "reverdict matches in-memory verdict (deterministic conversion)",
        reverdict == result.verdict,
    )?;

    // Persist verdict to RocksDB and read back.
    let store_root = PathBuf::from(FSV_ROOT)
        .join("swebench-oracle")
        .join(&run_id)
        .join("oracle-store");
    if store_root.exists() {
        fs::remove_dir_all(&store_root)?;
    }
    let store = OracleStore::open(&store_root)?;
    // For KnownGood we record a marker that simply notes the gold patch was
    // used as-is. The mutated_source is intentionally a stable diagnostic
    // string here — KnownGood does not mutate, and the actual patch.diff
    // applied by the harness is stored on disk under artifacts.patch.
    let mutation = MutationOutcome {
        category: MutationCategory::KnownGood,
        seed: 0,
        mutated_source: format!(
            "swebench-gold-smoke run_id={run_id} duration_ms={}; see artifacts.patch on disk",
            result.duration_ms
        ),
        mutation_site: None,
    };
    let patch_sha256 = sha256_text(&mutation.mutated_source);
    let verdict_sha256 = sha256_json_value(&serde_json::to_value(&result.verdict)?);
    let corpus_sha256 = compute_corpus_sha256(&[CorpusProvenanceRow {
        task_id: SMOKE_INSTANCE.to_string(),
        category: MutationCategory::KnownGood,
        patch_sha256,
        oracle_verdict_sha256: verdict_sha256,
    }])?;
    let provenance = CorpusProvenance {
        corpus_version: MEJEPA_CORPUS_VERSION_V1.to_string(),
        embedder_version: MEJEPA_EMBEDDER_VERSION_V1.to_string(),
        generated_at_unix_ms: 1,
        generator_version: "swebench_oracle_fsv".to_string(),
        seed: 0,
        languages: vec![Language::Python],
        task_manifest_count: 1,
        mutation_count: 1,
        mutation_categories: vec![MutationCategory::KnownGood],
        source_patch_mode: "swebench-gold".to_string(),
        split_mode: "instance_atomic".to_string(),
        corpus_sha256,
        complete: true,
    };
    store.put_corpus_row(
        SMOKE_INSTANCE,
        MutationCategory::KnownGood,
        &mutation,
        &result.verdict,
    )?;
    store.put_provenance(&provenance)?;
    store.flush()?;
    drop(store);

    let store2 = OracleStore::open(&store_root)?;
    let mutation_count = store2.count_cf(CF_MEJEPA_MUTATION_CORPUS)?;
    let verdict_count = store2.count_cf(CF_MEJEPA_ORACLE_VERDICTS)?;
    let provenance_count = store2.count_cf(CF_MEJEPA_CORPUS_PROVENANCE)?;
    ensure(
        "OracleStore CF_MEJEPA_ORACLE_VERDICTS count == 1",
        verdict_count == 1,
    )?;
    ensure(
        "OracleStore CF_MEJEPA_MUTATION_CORPUS count == 1",
        mutation_count == 1,
    )?;
    ensure(
        "OracleStore CF_MEJEPA_CORPUS_PROVENANCE count == 1",
        provenance_count == 1,
    )?;
    let read_back: Option<OracleVerdict> =
        store2.get_verdict(SMOKE_INSTANCE, MutationCategory::KnownGood)?;
    let read_back = read_back.ok_or("verdict missing after RocksDB reopen")?;
    let corpus_row = store2
        .get_corpus_row(SMOKE_INSTANCE, MutationCategory::KnownGood)?
        .ok_or("atomic corpus row missing after RocksDB reopen")?;
    let provenance_readback = store2
        .get_provenance(MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1)?
        .ok_or("provenance missing after RocksDB reopen")?;
    ensure(
        "verdict round-trip through RocksDB == original verdict",
        read_back == result.verdict,
    )?;
    ensure(
        "atomic corpus row round-trip through RocksDB == original row",
        corpus_row == (mutation.clone(), result.verdict.clone()),
    )?;
    ensure(
        "provenance round-trip through RocksDB == original provenance",
        provenance_readback == provenance,
    )?;
    drop(store2);

    Ok(json!({
        "mode": "gold",
        "instance_id": SMOKE_INSTANCE,
        "run_root": run_root,
        "run_id": run_id,
        "duration_ms": result.duration_ms,
        "command": result.command,
        "instance_report": {
            "resolved": result.instance_report.resolved,
            "patch_is_None": result.instance_report.patch_is_none,
            "patch_exists": result.instance_report.patch_exists,
            "patch_successfully_applied": result.instance_report.patch_successfully_applied,
            "fail_to_pass_success_count": result.instance_report.tests_status.as_ref().map(|t| t.fail_to_pass.success.len()).unwrap_or(0),
            "fail_to_pass_failure_count": result.instance_report.tests_status.as_ref().map(|t| t.fail_to_pass.failure.len()).unwrap_or(0),
            "pass_to_pass_success_count": result.instance_report.tests_status.as_ref().map(|t| t.pass_to_pass.success.len()).unwrap_or(0),
            "pass_to_pass_failure_count": result.instance_report.tests_status.as_ref().map(|t| t.pass_to_pass.failure.len()).unwrap_or(0),
        },
        "verdict": {
            "exception": result.verdict.exception.map(|e| e.slug()),
            "per_test_count": result.verdict.per_test.len(),
            "all_pass": result.verdict.all_passed(),
        },
        "artifacts": {
            "overall_report_path": result.artifacts.overall_report,
            "instance_report_path": result.artifacts.instance_report,
            "run_log_path": result.artifacts.run_log,
            "test_output_path": result.artifacts.test_output,
            "patch_path": result.artifacts.patch,
        },
        "rocksdb_store_path": store_root,
        "rocksdb_counts": {
            CF_MEJEPA_MUTATION_CORPUS: mutation_count,
            CF_MEJEPA_ORACLE_VERDICTS: verdict_count,
            CF_MEJEPA_CORPUS_PROVENANCE: provenance_count,
        },
        "provenance": provenance_readback,
        "rocksdb_round_trip_verdict_matches": true,
    }))
}

fn run_custom_with_gold_diff(gold_diff_path: &Path) -> FsvResult<Value> {
    println!(
        "[swebench-oracle-fsv] running custom mode (mejepa-known-good) against {SMOKE_INSTANCE}"
    );
    let model_patch = fs::read_to_string(gold_diff_path)?;
    if model_patch.trim().is_empty() {
        return Err(format!(
            "expected non-empty patch.diff at {}",
            gold_diff_path.display()
        )
        .into());
    }
    let prediction = context_graph_mejepa_corpus::swebench::SwebenchPrediction {
        instance_id: SMOKE_INSTANCE.to_string(),
        model_name_or_path: "mejepa-corpus-known-good".to_string(),
        model_patch,
    };
    let (result, run_root, run_id) = run_for_mode(SwebenchPredictionMode::Custom(prediction))?;
    assert_all_pass_resolved(&result)?;
    Ok(json!({
        "mode": "custom",
        "instance_id": SMOKE_INSTANCE,
        "run_root": run_root,
        "run_id": run_id,
        "duration_ms": result.duration_ms,
        "predictions_jsonl_path": result.artifacts.predictions_jsonl,
        "instance_report_resolved": result.instance_report.resolved,
        "verdict_all_pass": result.verdict.all_passed(),
        "verdict_per_test_count": result.verdict.per_test.len(),
    }))
}

fn main() -> FsvResult<()> {
    println!("[swebench-oracle-fsv] checked_at = {}", iso_now());
    let venv = default_venv_python();
    if let Some(reason) = check_preconditions(&venv, SMOKE_INSTANCE) {
        let payload = json!({
            "checked_at": iso_now(),
            "ok": false,
            "skipped": true,
            "reason": reason,
            "remediation": "Run scripts/swebench_lite_ops.py preflight then pull the smoke image.",
        });
        let path = Path::new(FSV_ROOT).join("swebench-oracle-evidence.json");
        write_evidence(&path, &payload)?;
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    let gold_payload = run_gold()?;
    // Re-use the patch.diff produced by the gold run as a Custom prediction
    // input. KnownGood mutations are the gold patch applied verbatim, so this
    // Custom run must still resolve. The dual-mode test confirms predictions
    // JSONL writing + path computation work end-to-end.
    let gold_patch_path = gold_payload
        .get("artifacts")
        .and_then(|a| a.get("patch_path"))
        .and_then(|p| p.as_str())
        .map(PathBuf::from)
        .ok_or("gold payload missing artifacts.patch_path")?;
    let custom_payload = run_custom_with_gold_diff(&gold_patch_path)?;

    let evidence = json!({
        "checked_at": iso_now(),
        "ok": true,
        "instance_id": SMOKE_INSTANCE,
        "image_ref": smoke_image_ref(SMOKE_INSTANCE),
        "gold": gold_payload,
        "custom": custom_payload,
    });
    let path = Path::new(FSV_ROOT).join("swebench-oracle-evidence.json");
    write_evidence(&path, &evidence)?;
    println!("[swebench-oracle-fsv] wrote {}", path.display());
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

#[allow(dead_code)]
fn _ignore_dead_code(_: Duration) {}
