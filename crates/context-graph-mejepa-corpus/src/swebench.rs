// SWE-bench Lite Phase 0 oracle bridge.
//
// Wraps the official `swebench.harness.run_evaluation` Python module so a
// Rust corpus generator can:
//
//   1. Build a canonical SWE-bench predictions JSONL line for ONE instance.
//   2. Invoke the harness against the SWE-bench Lite Docker image.
//   3. Read the per-instance `report.json` produced under
//      `<run_root>/logs/run_evaluation/<run_id>/<model_name_or_path>/<instance_id>/report.json`.
//   4. Convert SWE-bench's `tests_status` (FAIL_TO_PASS / PASS_TO_PASS /
//      FAIL_TO_FAIL / PASS_TO_FAIL, each with `success` and `failure` lists)
//      to the canonical `OracleVerdict` consumed by `EOracleInstrument`.
//   5. Surface every failure mode (harness crash, image missing, patch did
//      not apply, schema mismatch, missing required artifact) as a typed
//      fail-closed error with `.code()`.
//
// This module is the structural complement to
// `scripts/swebench_lite_ops.py gold-smoke`: that script proves the harness
// + Docker work; this module makes that workflow programmatically callable
// from Rust so corpus generation can persist to RocksDB.
//
// References:
//   - `swebench.harness.run_evaluation` (CLI entrypoint).
//   - `swebench.harness.grading.get_eval_report` (per-instance report
//     producer; inspected at
//     `~/.cache/contextgraph-swebench/SWE-bench/swebench/harness/grading.py`
//     during this implementation).
//   - `docs/ruvectorfindings/sitrep/04_implementation_plans/plan_phase0_docker_oracle.md`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use context_graph_mejepa_instruments::{
    ExceptionClass, OracleVerdict, PerTestOutcome, TestOutcome,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::oracle::{tail_text, wait_with_timeout, OracleError, OracleResult};

// ---------- public configuration types ----------

/// One row of a SWE-bench predictions JSONL. The official harness expects
/// these three fields and rejects unknown fields silently. We always set
/// `model_name_or_path` to a non-empty stable string because the harness
/// uses it as a directory name under `logs/run_evaluation/<run_id>/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwebenchPrediction {
    pub instance_id: String,
    pub model_name_or_path: String,
    pub model_patch: String,
}

/// Either the official gold patches (no JSONL is written; harness reads
/// `patch` from the dataset row) or a custom prediction the caller wants
/// evaluated against the Docker image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwebenchPredictionMode {
    Gold,
    Custom(SwebenchPrediction),
}

impl SwebenchPredictionMode {
    /// The string value passed to `--predictions_path`. For Gold this is
    /// the literal `"gold"`; for Custom this is the path to the JSONL we
    /// write before invoking the harness.
    fn predictions_path_arg(&self, jsonl_path: &Path) -> String {
        match self {
            Self::Gold => "gold".to_string(),
            Self::Custom(_) => jsonl_path.display().to_string(),
        }
    }

    /// The directory name the harness uses under
    /// `logs/run_evaluation/<run_id>/` to namespace per-instance artifacts.
    /// The harness defaults to `gold` for `--predictions_path gold` and to
    /// the prediction's `model_name_or_path` field otherwise.
    fn artifact_namespace(&self) -> &str {
        match self {
            Self::Gold => "gold",
            Self::Custom(prediction) => prediction.model_name_or_path.as_str(),
        }
    }

    pub fn instance_id(&self) -> Option<&str> {
        match self {
            Self::Gold => None,
            Self::Custom(prediction) => Some(prediction.instance_id.as_str()),
        }
    }
}

/// Configuration for a single SWE-bench harness invocation.
///
/// Defaults align with `scripts/swebench_lite_ops.py`:
/// - `dataset_name`: `"princeton-nlp/SWE-bench_Lite"`
/// - `split`: `"test"`
/// - `namespace`: `"swebench"` (Docker namespace prefix)
/// - `instance_timeout`: 1800 seconds (per-instance harness timeout)
/// - `overall_timeout`: 3600 seconds (we kill the subprocess if it runs longer)
#[derive(Debug, Clone)]
pub struct SwebenchOracleConfig {
    pub instance_id: String,
    pub mode: SwebenchPredictionMode,
    pub run_id: String,
    pub run_root: PathBuf,
    pub venv_python: PathBuf,
    pub dataset_name: String,
    pub split: String,
    pub namespace: String,
    pub instance_timeout: Duration,
    pub overall_timeout: Duration,
}

impl SwebenchOracleConfig {
    pub fn defaults_for(
        instance_id: impl Into<String>,
        run_id: impl Into<String>,
        run_root: impl Into<PathBuf>,
        venv_python: impl Into<PathBuf>,
        mode: SwebenchPredictionMode,
    ) -> Self {
        Self {
            instance_id: instance_id.into(),
            mode,
            run_id: run_id.into(),
            run_root: run_root.into(),
            venv_python: venv_python.into(),
            dataset_name: "princeton-nlp/SWE-bench_Lite".to_string(),
            split: "test".to_string(),
            namespace: "swebench".to_string(),
            instance_timeout: Duration::from_secs(1800),
            overall_timeout: Duration::from_secs(3600),
        }
    }
}

/// One side of a transition bucket in the SWE-bench tests_status report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionBucket {
    pub success: Vec<String>,
    pub failure: Vec<String>,
}

impl TransitionBucket {
    pub fn total(&self) -> usize {
        self.success.len() + self.failure.len()
    }
}

/// Mirror of the SWE-bench harness `tests_status` block from
/// `<instance_id>` -> `tests_status` -> {FAIL_TO_PASS, PASS_TO_PASS,
/// FAIL_TO_FAIL, PASS_TO_FAIL}. All four keys are always present; F2F and
/// P2F may have empty lists when `calculate_to_fail=False` (the harness
/// default for `EvalType.PASS_AND_FAIL`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestsStatus {
    pub fail_to_pass: TransitionBucket,
    pub pass_to_pass: TransitionBucket,
    pub fail_to_fail: TransitionBucket,
    pub pass_to_fail: TransitionBucket,
}

/// Per-instance SWE-bench harness report.json contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwebenchInstanceReport {
    pub instance_id: String,
    pub patch_is_none: bool,
    pub patch_exists: bool,
    pub patch_successfully_applied: bool,
    pub resolved: bool,
    pub tests_status: Option<TestsStatus>,
}

/// Filesystem locations the harness produces for one instance run. The
/// caller can read each of these for FSV or further analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwebenchOracleArtifacts {
    pub overall_report: PathBuf,
    pub instance_report: PathBuf,
    pub run_log: PathBuf,
    pub test_output: PathBuf,
    pub patch: PathBuf,
    pub predictions_jsonl: Option<PathBuf>,
}

/// Output of one harness invocation: the parsed instance report, a
/// canonical `OracleVerdict` derived from it, and the artifact paths.
#[derive(Debug, Clone)]
pub struct SwebenchOracleResult {
    pub instance_report: SwebenchInstanceReport,
    pub verdict: OracleVerdict,
    pub artifacts: SwebenchOracleArtifacts,
    pub command: Vec<String>,
    pub duration_ms: u128,
}

// ---------- public API ----------

/// Run the official SWE-bench Lite harness for ONE instance and return a
/// parsed result + canonical OracleVerdict.
///
/// Fail-closed if:
///   - any required path (venv-python, run_root parent) is missing or wrong;
///   - the harness exit code is non-zero;
///   - the per-instance report.json is absent / invalid / has the wrong
///     instance_id;
///   - the instance row is missing required fields;
///   - the prediction's `instance_id` does not match `config.instance_id`;
///   - the instance run timed out (we kill the harness after
///     `overall_timeout` and return `MEJEPA_ORACLE_TIMEOUT`).
pub fn run_swebench_lite_oracle(
    config: &SwebenchOracleConfig,
) -> OracleResult<SwebenchOracleResult> {
    validate_config(config)?;

    let predictions_jsonl_path = if matches!(config.mode, SwebenchPredictionMode::Custom(_)) {
        Some(write_predictions_jsonl(config)?)
    } else {
        None
    };

    let predictions_path_arg = config.mode.predictions_path_arg(
        predictions_jsonl_path
            .as_deref()
            .unwrap_or_else(|| Path::new("gold")),
    );

    let command = build_harness_command(config, &predictions_path_arg);

    fs::create_dir_all(&config.run_root).map_err(|err| {
        OracleError::io(
            &config.run_root,
            err.to_string(),
            "ensure the run_root parent directory is writable",
        )
    })?;

    let started = Instant::now();
    let mut cmd = Command::new(&command[0]);
    cmd.args(&command[1..])
        .current_dir(&config.run_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|err| {
        OracleError::command_failed(
            command[0].clone(),
            format!("failed to spawn swebench harness: {err}"),
            "verify swebench venv python path and PATH; do not modify .env to bypass",
        )
    })?;
    let output = wait_with_timeout(child, config.overall_timeout, "swebench-harness")?;
    let duration_ms = started.elapsed().as_millis();
    if !output.status.success() {
        return Err(OracleError::command_failed(
            "swebench-harness".to_string(),
            format!(
                "exit_code={}, stdout_tail={}, stderr_tail={}",
                output.status.code().unwrap_or(-1),
                tail_text(&output.stdout),
                tail_text(&output.stderr),
            ),
            "inspect harness output and Docker daemon state; fix root cause before retrying",
        ));
    }

    let artifacts = locate_artifacts(config, predictions_jsonl_path.clone())?;
    let report_text = fs::read_to_string(&artifacts.instance_report).map_err(|err| {
        OracleError::io(
            &artifacts.instance_report,
            err.to_string(),
            "ensure the harness wrote per-instance report.json before reading the verdict",
        )
    })?;
    let instance_report = parse_swebench_instance_report(&config.instance_id, &report_text)?;

    let test_output_text = fs::read_to_string(&artifacts.test_output).map_err(|err| {
        OracleError::io(
            &artifacts.test_output,
            err.to_string(),
            "ensure the harness produced test_output.txt for exception-class extraction",
        )
    })?;
    let exception_hint = extract_exception_class(&test_output_text);

    let verdict = swebench_report_to_verdict(&instance_report, exception_hint)?;

    Ok(SwebenchOracleResult {
        instance_report,
        verdict,
        artifacts,
        command,
        duration_ms,
    })
}

/// Parse the JSON text of a SWE-bench per-instance `report.json` file.
/// The text must contain exactly one top-level key matching `instance_id`.
pub fn parse_swebench_instance_report(
    instance_id: &str,
    text: &str,
) -> OracleResult<SwebenchInstanceReport> {
    if text.trim().is_empty() {
        return Err(OracleError::json_parse(
            "<swebench-report>",
            "report text is empty",
            "configure the harness to write a non-empty report.json",
        ));
    }
    let root: Value = serde_json::from_str(text).map_err(|err| {
        OracleError::json_parse(
            "<swebench-report>",
            err.to_string(),
            "preserve the harness report.json file for inspection",
        )
    })?;
    let obj = root.as_object().ok_or_else(|| {
        OracleError::report_invalid(
            "root",
            "swebench report.json root must be a JSON object keyed by instance_id",
            "the harness produces a single-key map; do not edit it",
        )
    })?;
    if obj.len() != 1 {
        return Err(OracleError::report_invalid(
            "root",
            format!(
                "swebench report.json root must contain exactly one key; got {} keys: {:?}",
                obj.len(),
                obj.keys().collect::<Vec<_>>(),
            ),
            "do not concatenate multiple per-instance reports into one file",
        ));
    }
    let (key, body) = obj
        .iter()
        .next()
        .expect("map with len==1 has one entry per BTreeMap invariant");
    if key != instance_id {
        return Err(OracleError::report_invalid(
            "root.<key>",
            format!(
                "swebench report.json instance_id mismatch: expected `{instance_id}`, found `{key}`",
            ),
            "make sure you read the report.json under .../<instance_id>/report.json for the right instance",
        ));
    }
    let body_obj = body.as_object().ok_or_else(|| {
        OracleError::report_invalid(
            "root.<instance_id>",
            "instance row must be a JSON object",
            "do not edit harness output by hand",
        )
    })?;

    let patch_is_none = read_bool(body_obj, "patch_is_None")?;
    let patch_exists = read_bool(body_obj, "patch_exists")?;
    let patch_successfully_applied = read_bool(body_obj, "patch_successfully_applied")?;
    let resolved = read_bool(body_obj, "resolved")?;
    let tests_status = match body_obj.get("tests_status") {
        None => None,
        Some(Value::Null) => None,
        Some(value) => Some(parse_tests_status(value)?),
    };

    Ok(SwebenchInstanceReport {
        instance_id: instance_id.to_string(),
        patch_is_none,
        patch_exists,
        patch_successfully_applied,
        resolved,
        tests_status,
    })
}

/// Convert a SWE-bench per-instance report into the canonical
/// `OracleVerdict` consumed by `EOracleInstrument`.
///
/// Mapping rules (see `swebench/harness/grading.py::get_eval_tests_report`):
///   - F2P.success / P2P.success / F2F.failure / P2F.failure → final state Pass
///   - F2P.failure / P2P.failure / F2F.success / P2F.success → final state Fail
///   - patch did not apply (patch_successfully_applied=false) → exception=Other
///     and zero per_test rows (the harness produces an empty tests_status).
///   - patch_is_None or patch_exists=false → exception=Other and zero per_test
///     rows.
///   - exception_hint (parsed from test_output.txt) is preferred over the
///     synthetic Other when the harness ran but reported failures.
pub fn swebench_report_to_verdict(
    report: &SwebenchInstanceReport,
    exception_hint: Option<ExceptionClass>,
) -> OracleResult<OracleVerdict> {
    if report.patch_is_none {
        return Ok(OracleVerdict {
            per_test: Vec::new(),
            exception: Some(ExceptionClass::Other),
        });
    }
    if !report.patch_exists {
        return Ok(OracleVerdict {
            per_test: Vec::new(),
            exception: Some(ExceptionClass::Other),
        });
    }
    if !report.patch_successfully_applied {
        return Ok(OracleVerdict {
            per_test: Vec::new(),
            exception: Some(ExceptionClass::Other),
        });
    }

    let Some(tests) = report.tests_status.as_ref() else {
        return Err(OracleError::report_invalid(
            "tests_status",
            "patch_successfully_applied=true but tests_status is missing or null",
            "harness ran without --include_tests_status; rerun with the default flags",
        ));
    };

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut per_test: Vec<PerTestOutcome> = Vec::new();

    fn extend(
        bucket: &TransitionBucket,
        success_outcome: TestOutcome,
        failure_outcome: TestOutcome,
        seen: &mut BTreeSet<String>,
        per_test: &mut Vec<PerTestOutcome>,
    ) -> OracleResult<()> {
        for case in &bucket.success {
            validate_test_id(case)?;
            if !seen.insert(case.clone()) {
                continue;
            }
            per_test.push(PerTestOutcome {
                test_id: case.clone(),
                outcome: success_outcome,
                runtime_ms: -1,
            });
        }
        for case in &bucket.failure {
            validate_test_id(case)?;
            if !seen.insert(case.clone()) {
                continue;
            }
            per_test.push(PerTestOutcome {
                test_id: case.clone(),
                outcome: failure_outcome,
                runtime_ms: -1,
            });
        }
        Ok(())
    }

    extend(
        &tests.fail_to_pass,
        TestOutcome::Pass,
        TestOutcome::Fail,
        &mut seen,
        &mut per_test,
    )?;
    extend(
        &tests.pass_to_pass,
        TestOutcome::Pass,
        TestOutcome::Fail,
        &mut seen,
        &mut per_test,
    )?;
    // F2F.success means the test stayed failing (final state: Fail).
    // F2F.failure means the test transitioned to passing (final state: Pass).
    extend(
        &tests.fail_to_fail,
        TestOutcome::Fail,
        TestOutcome::Pass,
        &mut seen,
        &mut per_test,
    )?;
    // P2F.success means the test transitioned to failing (final state: Fail).
    // P2F.failure means the test stayed passing (final state: Pass).
    extend(
        &tests.pass_to_fail,
        TestOutcome::Fail,
        TestOutcome::Pass,
        &mut seen,
        &mut per_test,
    )?;

    if per_test.is_empty() {
        // No tests at all: the harness completed but the spec had no F2P or
        // P2P tests. Fail-closed because that means the source-of-truth
        // signal is empty for this instance.
        return Err(OracleError::report_invalid(
            "tests_status",
            "tests_status produced zero per-test rows; nothing to learn from",
            "verify the SWE-bench task spec has FAIL_TO_PASS or PASS_TO_PASS tests",
        ));
    }

    // Stable order so persisted verdicts compare cleanly.
    per_test.sort_by(|a, b| a.test_id.cmp(&b.test_id));

    let any_failure = per_test
        .iter()
        .any(|t| matches!(t.outcome, TestOutcome::Fail));
    let exception = if any_failure {
        Some(exception_hint.unwrap_or(ExceptionClass::Other))
    } else {
        None
    };

    Ok(OracleVerdict {
        per_test,
        exception,
    })
}

/// Best-effort scan of a SWE-bench `test_output.txt` for the dominant
/// Python exception class (the test framework prints exception class names
/// in tracebacks on failure). Returns the first exact match against a
/// curated set of class tokens; no substring fuzziness because that
/// produces false positives on test names that contain those tokens.
pub fn extract_exception_class(text: &str) -> Option<ExceptionClass> {
    // Keep this list synchronised with `ExceptionClass` variants. Order
    // here is precedence-by-distinctiveness so a `KeyError` line in a
    // traceback dominates a generic `Error` mention elsewhere.
    let classes: &[(ExceptionClass, &str)] = &[
        (ExceptionClass::AssertionError, "AssertionError"),
        (ExceptionClass::TypeError, "TypeError"),
        (ExceptionClass::ValueError, "ValueError"),
        (ExceptionClass::KeyError, "KeyError"),
        (ExceptionClass::IndexError, "IndexError"),
        (ExceptionClass::AttributeError, "AttributeError"),
        (ExceptionClass::ImportError, "ImportError"),
        (ExceptionClass::NameError, "NameError"),
        (ExceptionClass::RuntimeError, "RuntimeError"),
    ];
    for (class, token) in classes {
        if has_word(text, token) {
            return Some(*class);
        }
    }
    None
}

// ---------- internals ----------

fn validate_config(config: &SwebenchOracleConfig) -> OracleResult<()> {
    validate_instance_id(&config.instance_id)?;
    if config.run_id.trim().is_empty() {
        return Err(OracleError::invalid(
            "config.run_id",
            "run_id must be non-empty; the harness uses it as a directory name",
            "supply a unique run_id, e.g. mejepa-corpus-<utc_iso8601>",
        ));
    }
    // Docker container names accept only [a-zA-Z0-9][a-zA-Z0-9_.-]+ and
    // the harness embeds run_id in the container name. Reject anything
    // outside that alphabet up front so the harness cannot fail late.
    let mut chars = config.run_id.chars();
    let first = chars.next().expect("non-empty (checked above)");
    if !first.is_ascii_alphanumeric() {
        return Err(OracleError::invalid(
            "config.run_id",
            "run_id must start with an ASCII alphanumeric character",
            "Docker container-name regex is [a-zA-Z0-9][a-zA-Z0-9_.-]+",
        ));
    }
    for c in std::iter::once(first).chain(chars) {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-') {
            return Err(OracleError::invalid(
                "config.run_id",
                format!(
                    "run_id contains disallowed character `{c}`; allowed alphabet is [a-zA-Z0-9_.-]",
                ),
                "Docker container-name regex is [a-zA-Z0-9][a-zA-Z0-9_.-]+; do not use `+`, `:`, `/`, spaces, or any other punctuation",
            ));
        }
    }
    if config.dataset_name.trim().is_empty() {
        return Err(OracleError::invalid(
            "config.dataset_name",
            "dataset_name must be non-empty",
            "use 'princeton-nlp/SWE-bench_Lite' for Lite eval",
        ));
    }
    if config.split.trim().is_empty() {
        return Err(OracleError::invalid(
            "config.split",
            "split must be non-empty",
            "use 'test' for SWE-bench Lite",
        ));
    }
    if config.namespace.trim().is_empty() {
        return Err(OracleError::invalid(
            "config.namespace",
            "namespace must be non-empty",
            "use 'swebench' for the official Docker namespace",
        ));
    }
    if config.instance_timeout.is_zero() {
        return Err(OracleError::invalid(
            "config.instance_timeout",
            "instance_timeout must be > 0",
            "use the harness default of 1800 seconds or higher",
        ));
    }
    if config.overall_timeout < config.instance_timeout {
        return Err(OracleError::invalid(
            "config.overall_timeout",
            "overall_timeout must be >= instance_timeout to allow harness setup overhead",
            "set overall_timeout >= instance_timeout + 600 seconds",
        ));
    }
    if !config.venv_python.is_file() {
        return Err(OracleError::invalid(
            "config.venv_python",
            format!(
                "venv_python is not a file: {}",
                config.venv_python.display()
            ),
            "install the SWE-bench harness venv (see scripts/swebench_lite_ops.py)",
        ));
    }

    if let SwebenchPredictionMode::Custom(prediction) = &config.mode {
        if prediction.instance_id != config.instance_id {
            return Err(OracleError::invalid(
                "config.mode.prediction.instance_id",
                format!(
                    "prediction.instance_id `{}` must match config.instance_id `{}`",
                    prediction.instance_id, config.instance_id,
                ),
                "build one Custom prediction per instance and align both fields",
            ));
        }
        validate_instance_id(&prediction.instance_id)?;
        validate_model_name(&prediction.model_name_or_path)?;
        if prediction.model_patch.is_empty() {
            return Err(OracleError::invalid(
                "config.mode.prediction.model_patch",
                "model_patch is empty; harness will report patch_is_None",
                "supply a non-empty unified diff or use SwebenchPredictionMode::Gold",
            ));
        }
        if !prediction.model_patch.is_ascii()
            && !prediction
                .model_patch
                .chars()
                .all(|c| !c.is_control() || c == '\n' || c == '\r' || c == '\t')
        {
            // model_patch is encoded as a string in JSONL; reject control bytes
            // other than the standard whitespace set so the harness JSON parser
            // receives well-formed input.
            return Err(OracleError::invalid(
                "config.mode.prediction.model_patch",
                "model_patch contains disallowed control characters",
                "strip non-printable bytes (other than \\n, \\r, \\t) before predicting",
            ));
        }
    }
    Ok(())
}

fn validate_instance_id(instance_id: &str) -> OracleResult<()> {
    if instance_id.trim() != instance_id || instance_id.is_empty() {
        return Err(OracleError::invalid(
            "instance_id",
            "instance_id must be a non-empty trimmed string",
            "use the canonical SWE-bench instance_id, e.g. sympy__sympy-20590",
        ));
    }
    if !instance_id.contains("__") {
        return Err(OracleError::invalid(
            "instance_id",
            "instance_id must contain `__` (SWE-bench owner__repo-issue convention)",
            "supply an id from tasks/swebench-lite or the official Lite dataset",
        ));
    }
    if instance_id
        .chars()
        .any(|c| c.is_control() || matches!(c, '/' | '\\' | '\0' | ' '))
    {
        return Err(OracleError::invalid(
            "instance_id",
            "instance_id must not contain whitespace, path separators, or control characters",
            "use exact ids from the Lite manifest",
        ));
    }
    Ok(())
}

fn validate_model_name(name: &str) -> OracleResult<()> {
    if name.trim().is_empty() {
        return Err(OracleError::invalid(
            "prediction.model_name_or_path",
            "model_name_or_path must be non-empty (used as a directory name)",
            "use a stable slug like `mejepa-corpus-known-good`",
        ));
    }
    if name
        .chars()
        .any(|c| c.is_control() || matches!(c, '/' | '\\' | '\0'))
    {
        return Err(OracleError::invalid(
            "prediction.model_name_or_path",
            "model_name_or_path must not contain control characters or path separators",
            "use ASCII alphanumerics + hyphens + underscores",
        ));
    }
    Ok(())
}

fn validate_test_id(test_id: &str) -> OracleResult<()> {
    if test_id.trim().is_empty() {
        return Err(OracleError::report_invalid(
            "tests_status.<bucket>",
            "test id is empty or whitespace-only in tests_status",
            "the harness should never emit empty test ids; investigate harness output",
        ));
    }
    if test_id.chars().any(char::is_control) {
        return Err(OracleError::report_invalid(
            "tests_status.<bucket>",
            "test id contains a control character",
            "investigate harness output for malformed pytest ids",
        ));
    }
    Ok(())
}

fn write_predictions_jsonl(config: &SwebenchOracleConfig) -> OracleResult<PathBuf> {
    let SwebenchPredictionMode::Custom(prediction) = &config.mode else {
        return Err(OracleError::invalid(
            "config.mode",
            "write_predictions_jsonl called for Gold mode",
            "internal error: only call this helper for Custom",
        ));
    };
    fs::create_dir_all(&config.run_root).map_err(|err| {
        OracleError::io(
            &config.run_root,
            err.to_string(),
            "ensure the run_root parent directory is writable",
        )
    })?;
    let path = config.run_root.join("predictions.jsonl");
    let mut json = serde_json::Map::new();
    json.insert(
        "instance_id".to_string(),
        Value::String(prediction.instance_id.clone()),
    );
    json.insert(
        "model_name_or_path".to_string(),
        Value::String(prediction.model_name_or_path.clone()),
    );
    json.insert(
        "model_patch".to_string(),
        Value::String(prediction.model_patch.clone()),
    );
    let mut text = serde_json::to_string(&Value::Object(json)).map_err(|err| {
        OracleError::json_parse(
            path.display().to_string(),
            err.to_string(),
            "the prediction record failed to serialise; this is an internal defect",
        )
    })?;
    text.push('\n');
    fs::write(&path, &text).map_err(|err| {
        OracleError::io(
            &path,
            err.to_string(),
            "ensure the predictions.jsonl path is writable",
        )
    })?;
    let readback = fs::read_to_string(&path).map_err(|err| {
        OracleError::io(
            &path,
            err.to_string(),
            "filesystem readback after write failed; check the disk",
        )
    })?;
    if readback != text {
        return Err(OracleError::io(
            &path,
            "predictions.jsonl readback did not match what we wrote",
            "investigate the filesystem for partial writes or concurrent writers",
        ));
    }
    Ok(path)
}

fn build_harness_command(config: &SwebenchOracleConfig, predictions_path_arg: &str) -> Vec<String> {
    vec![
        config.venv_python.display().to_string(),
        "-m".to_string(),
        "swebench.harness.run_evaluation".to_string(),
        "--dataset_name".to_string(),
        config.dataset_name.clone(),
        "--split".to_string(),
        config.split.clone(),
        "--predictions_path".to_string(),
        predictions_path_arg.to_string(),
        "--max_workers".to_string(),
        "1".to_string(),
        "--instance_ids".to_string(),
        config.instance_id.clone(),
        "--run_id".to_string(),
        config.run_id.clone(),
        "--timeout".to_string(),
        config.instance_timeout.as_secs().to_string(),
        "--cache_level".to_string(),
        "instance".to_string(),
        "--clean".to_string(),
        "False".to_string(),
        "--namespace".to_string(),
        config.namespace.clone(),
        "--report_dir".to_string(),
        config.run_root.display().to_string(),
    ]
}

fn locate_artifacts(
    config: &SwebenchOracleConfig,
    predictions_jsonl: Option<PathBuf>,
) -> OracleResult<SwebenchOracleArtifacts> {
    let namespace = config.mode.artifact_namespace();
    let overall_report = config.run_root.join(format!(
        "{namespace}.{run_id}.json",
        namespace = namespace,
        run_id = config.run_id,
    ));
    let instance_dir = config
        .run_root
        .join("logs")
        .join("run_evaluation")
        .join(&config.run_id)
        .join(namespace)
        .join(&config.instance_id);
    let instance_report = instance_dir.join("report.json");
    let run_log = instance_dir.join("run_instance.log");
    let test_output = instance_dir.join("test_output.txt");
    let patch = instance_dir.join("patch.diff");

    for (path, label) in [
        (&overall_report, "overall report"),
        (&instance_report, "per-instance report.json"),
        (&run_log, "run_instance.log"),
        (&test_output, "test_output.txt"),
    ] {
        if !path.is_file() {
            return Err(OracleError::io(
                path,
                format!("{label} missing after harness run"),
                "investigate harness logs; the run did not produce the expected artifact",
            ));
        }
    }
    // patch.diff is only produced for non-gold runs OR when the harness can
    // recover the gold patch on its own; the gold-smoke evidence shows it
    // present for gold. We require it present so reproductions are
    // bit-faithful, but tolerate absence with a typed warning so the call
    // site can react.
    if !patch.is_file() {
        return Err(OracleError::io(
            &patch,
            "patch.diff missing after harness run",
            "investigate harness logs; the run did not record the predicted patch",
        ));
    }
    Ok(SwebenchOracleArtifacts {
        overall_report,
        instance_report,
        run_log,
        test_output,
        patch,
        predictions_jsonl,
    })
}

fn parse_tests_status(value: &Value) -> OracleResult<TestsStatus> {
    let obj = value.as_object().ok_or_else(|| {
        OracleError::report_invalid(
            "tests_status",
            "tests_status must be an object",
            "do not edit harness output by hand",
        )
    })?;
    let fail_to_pass = parse_bucket(obj, "FAIL_TO_PASS")?;
    let pass_to_pass = parse_bucket(obj, "PASS_TO_PASS")?;
    let fail_to_fail = parse_bucket(obj, "FAIL_TO_FAIL")?;
    let pass_to_fail = parse_bucket(obj, "PASS_TO_FAIL")?;
    Ok(TestsStatus {
        fail_to_pass,
        pass_to_pass,
        fail_to_fail,
        pass_to_fail,
    })
}

fn parse_bucket(
    parent: &serde_json::Map<String, Value>,
    key: &'static str,
) -> OracleResult<TransitionBucket> {
    let Some(value) = parent.get(key) else {
        return Err(OracleError::report_invalid(
            "tests_status.<bucket>",
            format!("missing required tests_status key: {key}"),
            "harness should always emit FAIL_TO_PASS, PASS_TO_PASS, FAIL_TO_FAIL, PASS_TO_FAIL",
        ));
    };
    let obj = value.as_object().ok_or_else(|| {
        OracleError::report_invalid(
            "tests_status.<bucket>",
            format!("tests_status.{key} must be an object"),
            "investigate harness output",
        )
    })?;
    let success = read_string_array(obj, "success", key)?;
    let failure = read_string_array(obj, "failure", key)?;
    Ok(TransitionBucket { success, failure })
}

fn read_string_array(
    parent: &serde_json::Map<String, Value>,
    key: &'static str,
    bucket_label: &'static str,
) -> OracleResult<Vec<String>> {
    let Some(value) = parent.get(key) else {
        return Err(OracleError::report_invalid(
            "tests_status.<bucket>.<list>",
            format!("missing tests_status.{bucket_label}.{key}"),
            "investigate harness output",
        ));
    };
    let array = value.as_array().ok_or_else(|| {
        OracleError::report_invalid(
            "tests_status.<bucket>.<list>",
            format!("tests_status.{bucket_label}.{key} must be a JSON array"),
            "investigate harness output",
        )
    })?;
    let mut out = Vec::with_capacity(array.len());
    for (i, item) in array.iter().enumerate() {
        let Some(s) = item.as_str() else {
            return Err(OracleError::report_invalid(
                "tests_status.<bucket>.<list>[i]",
                format!("tests_status.{bucket_label}.{key}[{i}] is not a JSON string"),
                "investigate harness output",
            ));
        };
        out.push(s.to_string());
    }
    Ok(out)
}

fn read_bool(parent: &serde_json::Map<String, Value>, key: &'static str) -> OracleResult<bool> {
    let Some(value) = parent.get(key) else {
        return Err(OracleError::report_invalid(
            "instance_row.<bool>",
            format!("missing required field {key}"),
            "the harness should always emit this field; investigate harness output",
        ));
    };
    value.as_bool().ok_or_else(|| {
        OracleError::report_invalid(
            "instance_row.<bool>",
            format!("field {key} is not a JSON boolean"),
            "investigate harness output",
        )
    })
}

fn has_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let needle = word.as_bytes();
    let mut i = 0usize;
    while i + needle.len() <= bytes.len() {
        if bytes[i..i + needle.len()] == *needle {
            let before_ok = i == 0 || !is_word_byte(bytes[i - 1]);
            let after_idx = i + needle.len();
            let after_ok = after_idx == bytes.len() || !is_word_byte(bytes[after_idx]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_resolved_report() -> &'static str {
        r#"{
          "sympy__sympy-20590": {
            "patch_is_None": false,
            "patch_exists": true,
            "patch_successfully_applied": true,
            "resolved": true,
            "tests_status": {
              "FAIL_TO_PASS": {
                "success": ["tests/test_a.py::test_alpha"],
                "failure": []
              },
              "PASS_TO_PASS": {
                "success": ["tests/test_b.py::test_beta", "tests/test_b.py::test_gamma"],
                "failure": []
              },
              "FAIL_TO_FAIL": {"success": [], "failure": []},
              "PASS_TO_FAIL": {"success": [], "failure": []}
            }
          }
        }"#
    }

    fn sample_unresolved_report() -> &'static str {
        r#"{
          "sympy__sympy-20590": {
            "patch_is_None": false,
            "patch_exists": true,
            "patch_successfully_applied": true,
            "resolved": false,
            "tests_status": {
              "FAIL_TO_PASS": {
                "success": [],
                "failure": ["tests/test_a.py::test_alpha"]
              },
              "PASS_TO_PASS": {
                "success": ["tests/test_b.py::test_beta"],
                "failure": ["tests/test_b.py::test_gamma"]
              },
              "FAIL_TO_FAIL": {"success": [], "failure": []},
              "PASS_TO_FAIL": {"success": [], "failure": []}
            }
          }
        }"#
    }

    #[test]
    fn parses_resolved_report_and_maps_to_all_pass_verdict() {
        let report =
            parse_swebench_instance_report("sympy__sympy-20590", sample_resolved_report()).unwrap();
        assert!(report.resolved);
        assert!(report.patch_successfully_applied);
        let verdict = swebench_report_to_verdict(&report, None).unwrap();
        assert!(verdict.exception.is_none());
        assert_eq!(verdict.per_test.len(), 3);
        for row in &verdict.per_test {
            assert_eq!(row.outcome, TestOutcome::Pass);
        }
    }

    #[test]
    fn parses_unresolved_report_and_maps_failures_with_exception() {
        let report =
            parse_swebench_instance_report("sympy__sympy-20590", sample_unresolved_report())
                .unwrap();
        assert!(!report.resolved);
        let verdict =
            swebench_report_to_verdict(&report, Some(ExceptionClass::AssertionError)).unwrap();
        assert_eq!(verdict.exception, Some(ExceptionClass::AssertionError));
        assert_eq!(verdict.per_test.len(), 3);
        let pass = verdict
            .per_test
            .iter()
            .filter(|t| t.outcome == TestOutcome::Pass)
            .count();
        let fail = verdict
            .per_test
            .iter()
            .filter(|t| t.outcome == TestOutcome::Fail)
            .count();
        assert_eq!(pass, 1);
        assert_eq!(fail, 2);
    }

    #[test]
    fn maps_fail_to_fail_extra_credit_to_pass() {
        let text = r#"{
          "sympy__sympy-20590": {
            "patch_is_None": false,
            "patch_exists": true,
            "patch_successfully_applied": true,
            "resolved": false,
            "tests_status": {
              "FAIL_TO_PASS": {"success": [], "failure": []},
              "PASS_TO_PASS": {"success": ["x::y"], "failure": []},
              "FAIL_TO_FAIL": {"success": ["s::t"], "failure": ["u::v"]},
              "PASS_TO_FAIL": {"success": [], "failure": []}
            }
          }
        }"#;
        let report = parse_swebench_instance_report("sympy__sympy-20590", text).unwrap();
        let verdict = swebench_report_to_verdict(&report, None).unwrap();
        let map: std::collections::BTreeMap<&str, TestOutcome> = verdict
            .per_test
            .iter()
            .map(|t| (t.test_id.as_str(), t.outcome))
            .collect();
        // F2F.success "s::t" => Fail (test stayed failing)
        assert_eq!(map.get("s::t").copied(), Some(TestOutcome::Fail));
        // F2F.failure "u::v" => Pass (test transitioned to passing — extra credit)
        assert_eq!(map.get("u::v").copied(), Some(TestOutcome::Pass));
        // P2P.success "x::y" => Pass
        assert_eq!(map.get("x::y").copied(), Some(TestOutcome::Pass));
    }

    #[test]
    fn rejects_instance_id_mismatch() {
        let err = parse_swebench_instance_report("other", sample_resolved_report()).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_REPORT_INVALID");
    }

    #[test]
    fn rejects_empty_report() {
        let err = parse_swebench_instance_report("sympy__sympy-20590", "").unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_JSON_PARSE");
    }

    #[test]
    fn rejects_multi_key_root() {
        let text = r#"{
          "a__b-1": {"patch_is_None": false, "patch_exists": true, "patch_successfully_applied": false, "resolved": false},
          "c__d-2": {"patch_is_None": false, "patch_exists": true, "patch_successfully_applied": false, "resolved": false}
        }"#;
        let err = parse_swebench_instance_report("a__b-1", text).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_REPORT_INVALID");
    }

    #[test]
    fn patch_not_applied_yields_other_exception_no_per_tests() {
        let text = r#"{
          "sympy__sympy-20590": {
            "patch_is_None": false,
            "patch_exists": true,
            "patch_successfully_applied": false,
            "resolved": false
          }
        }"#;
        let report = parse_swebench_instance_report("sympy__sympy-20590", text).unwrap();
        assert!(!report.patch_successfully_applied);
        let verdict = swebench_report_to_verdict(&report, None).unwrap();
        assert_eq!(verdict.exception, Some(ExceptionClass::Other));
        assert!(verdict.per_test.is_empty());
    }

    #[test]
    fn patch_is_none_yields_other_exception_no_per_tests() {
        let text = r#"{
          "sympy__sympy-20590": {
            "patch_is_None": true,
            "patch_exists": false,
            "patch_successfully_applied": false,
            "resolved": false
          }
        }"#;
        let report = parse_swebench_instance_report("sympy__sympy-20590", text).unwrap();
        let verdict = swebench_report_to_verdict(&report, None).unwrap();
        assert_eq!(verdict.exception, Some(ExceptionClass::Other));
        assert!(verdict.per_test.is_empty());
    }

    #[test]
    fn rejects_missing_required_bool_field() {
        // patch_successfully_applied missing
        let text = r#"{
          "sympy__sympy-20590": {
            "patch_is_None": false,
            "patch_exists": true,
            "resolved": false
          }
        }"#;
        let err = parse_swebench_instance_report("sympy__sympy-20590", text).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_REPORT_INVALID");
    }

    #[test]
    fn rejects_resolved_with_zero_tests() {
        let text = r#"{
          "sympy__sympy-20590": {
            "patch_is_None": false,
            "patch_exists": true,
            "patch_successfully_applied": true,
            "resolved": true,
            "tests_status": {
              "FAIL_TO_PASS": {"success": [], "failure": []},
              "PASS_TO_PASS": {"success": [], "failure": []},
              "FAIL_TO_FAIL": {"success": [], "failure": []},
              "PASS_TO_FAIL": {"success": [], "failure": []}
            }
          }
        }"#;
        let report = parse_swebench_instance_report("sympy__sympy-20590", text).unwrap();
        let err = swebench_report_to_verdict(&report, None).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_REPORT_INVALID");
    }

    #[test]
    fn extracts_assertion_error_word_match_only() {
        let text = "============ FAIL ============\nE   AssertionError: 1 != 2\n";
        assert_eq!(
            extract_exception_class(text),
            Some(ExceptionClass::AssertionError)
        );
        // Substring inside an identifier must NOT match.
        let text = "MyAssertionErrorHelper invoked";
        assert_eq!(extract_exception_class(text), None);
    }

    #[test]
    fn extracts_first_class_by_precedence() {
        // Both AssertionError and TypeError appear; AssertionError comes first
        // in the precedence table, so we should pick it.
        let text = "TypeError: bad\nAssertionError: also bad\n";
        assert_eq!(
            extract_exception_class(text),
            Some(ExceptionClass::AssertionError)
        );
    }

    #[test]
    fn validate_config_rejects_run_id_with_plus() {
        // 2026-05-09 reality bug: SWE-bench harness uses run_id as a Docker
        // container-name suffix. Docker rejects `+` with a 400. The new
        // validator catches it up front instead of at harness-execution time.
        let cfg = SwebenchOracleConfig {
            run_id: "epoch+1234567s".to_string(),
            ..SwebenchOracleConfig::defaults_for(
                "sympy__sympy-20590",
                "rid",
                std::env::temp_dir().join("nope"),
                std::env::temp_dir().join("nope-py"),
                SwebenchPredictionMode::Gold,
            )
        };
        let err = validate_config(&cfg).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_INVALID_INPUT");
    }

    #[test]
    fn validate_config_rejects_run_id_starting_with_punctuation() {
        let cfg = SwebenchOracleConfig {
            run_id: "-leading-dash".to_string(),
            ..SwebenchOracleConfig::defaults_for(
                "sympy__sympy-20590",
                "rid",
                std::env::temp_dir().join("nope"),
                std::env::temp_dir().join("nope-py"),
                SwebenchPredictionMode::Gold,
            )
        };
        let err = validate_config(&cfg).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_INVALID_INPUT");
    }

    #[test]
    fn validate_config_rejects_bad_instance_id() {
        let cfg = SwebenchOracleConfig::defaults_for(
            "no-double-underscore",
            "rid",
            std::env::temp_dir().join("nope"),
            std::env::temp_dir().join("nope-py"),
            SwebenchPredictionMode::Gold,
        );
        let err = validate_config(&cfg).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_INVALID_INPUT");
    }

    #[test]
    fn validate_config_rejects_prediction_id_mismatch() {
        let cfg = SwebenchOracleConfig::defaults_for(
            "sympy__sympy-20590",
            "rid",
            std::env::temp_dir().join("nope"),
            std::env::temp_dir().join("nope-py"),
            SwebenchPredictionMode::Custom(SwebenchPrediction {
                instance_id: "django__django-12286".to_string(),
                model_name_or_path: "test".to_string(),
                model_patch: "diff --git a b\n".to_string(),
            }),
        );
        let err = validate_config(&cfg).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_INVALID_INPUT");
    }

    #[test]
    fn build_harness_command_has_required_flags() {
        let cfg = SwebenchOracleConfig::defaults_for(
            "sympy__sympy-20590",
            "rid",
            std::path::PathBuf::from("/tmp/runroot"),
            std::path::PathBuf::from("/usr/bin/python3"),
            SwebenchPredictionMode::Gold,
        );
        let cmd = build_harness_command(&cfg, "gold");
        assert!(cmd.iter().any(|a| a == "swebench.harness.run_evaluation"));
        assert!(cmd.iter().any(|a| a == "--predictions_path"));
        assert!(cmd.iter().any(|a| a == "gold"));
        assert!(cmd.iter().any(|a| a == "sympy__sympy-20590"));
        assert!(cmd.iter().any(|a| a == "--cache_level"));
        assert!(cmd.iter().any(|a| a == "instance"));
    }

    #[test]
    fn locate_artifacts_namespaces_gold_vs_custom() {
        let cfg_gold = SwebenchOracleConfig::defaults_for(
            "sympy__sympy-20590",
            "rid",
            std::path::PathBuf::from("/tmp/runroot"),
            std::path::PathBuf::from("/usr/bin/python3"),
            SwebenchPredictionMode::Gold,
        );
        // Cannot call locate_artifacts because files don't exist; instead
        // assert the namespace logic directly.
        assert_eq!(cfg_gold.mode.artifact_namespace(), "gold");

        let cfg_custom = SwebenchOracleConfig::defaults_for(
            "sympy__sympy-20590",
            "rid",
            std::path::PathBuf::from("/tmp/runroot"),
            std::path::PathBuf::from("/usr/bin/python3"),
            SwebenchPredictionMode::Custom(SwebenchPrediction {
                instance_id: "sympy__sympy-20590".to_string(),
                model_name_or_path: "mejepa-known-good".to_string(),
                model_patch: "diff --git a b\n".to_string(),
            }),
        );
        assert_eq!(cfg_custom.mode.artifact_namespace(), "mejepa-known-good");
    }
}
