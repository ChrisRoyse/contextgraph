# TASK-CORPUS-006 — SwebenchOracleConfig + SwebenchInstanceReport Types

```xml
<task_spec id="TASK-CORPUS-006" version="1.0">
<metadata>
  <title>SwebenchOracleConfig + SwebenchInstanceReport + SwebenchOracleResult Types</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `SwebenchPrediction`, `SwebenchPredictionMode { Gold, Custom(SwebenchPrediction) }`, `SwebenchOracleConfig` (with `instance_timeout`, `overall_timeout`, `dataset_name`, `split`, `namespace`, `venv_python`, `run_root`), `TransitionBucket`, `TestsStatus`, `SwebenchInstanceReport`, `SwebenchOracleArtifacts`, `SwebenchOracleResult` (swebench.rs:51–215). `SwebenchOracleConfig::defaults_for` aligns with `scripts/swebench_lite_ops.py`. Real Docker FSV against `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest` (gold + custom modes).
  </implementation_state>
  <layer>foundation</layer>
  <sequence>6</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-03</requirement_ref>
    <requirement_ref>REQ-CORPUS-04</requirement_ref>
    <requirement_ref>REQ-CORPUS-14</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-001</task_ref>
    <task_ref>TASK-CORPUS-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Foundation task. The plain-data inputs and outputs of
`run_swebench_lite_oracle` (TASK-CORPUS-020). These are
serializable types (predictions go through JSONL on disk;
report.json maps directly into SwebenchInstanceReport) so the
oracle invoker can be written purely against the type contract
in TASK-CORPUS-020 without inventing ad-hoc shapes.
</context>

<input_context_files>
  <file purpose="data_models">../../technical/TECH-CORPUS.md#data_models (SwebenchPrediction, SwebenchPredictionMode, SwebenchOracleConfig, TransitionBucket, TestsStatus, SwebenchInstanceReport, SwebenchOracleArtifacts, SwebenchOracleResult)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-02, AC-CORPUS-02-A/B)</file>
  <file purpose="grading_reference">swebench/harness/grading.py (read-only — DO NOT EDIT)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/swebench.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-001 complete (OracleError exists)</check>
  <check>TASK-CORPUS-003 complete (OracleVerdict re-exported)</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `SwebenchPrediction { instance_id: String,
      model_name_or_path: String, model_patch: String }`.
    - Define `SwebenchPredictionMode { Gold,
      Custom(SwebenchPrediction) }` enum.
    - Define `SwebenchOracleConfig { instance_id, mode, run_id,
      run_root, venv_python, dataset_name, split, namespace,
      instance_timeout, overall_timeout }` per data model row.
    - Default fields: dataset_name = "princeton-nlp/SWE-bench_Lite",
      split = "test", namespace = "swebench",
      instance_timeout = Duration::from_secs(1800),
      overall_timeout = Duration::from_secs(3600).
    - Define `TransitionBucket { success: Vec&lt;String&gt;,
      failure: Vec&lt;String&gt; }`,
      `TestsStatus { fail_to_pass, pass_to_pass, fail_to_fail,
      pass_to_fail }`.
    - Define `SwebenchInstanceReport { instance_id,
      patch_is_none (#[serde(rename = "patch_is_None")]),
      patch_exists, patch_successfully_applied, resolved,
      tests_status: Option&lt;TestsStatus&gt; }`.
    - Define `SwebenchOracleArtifacts { overall_report,
      instance_report, run_log, test_output, patch,
      predictions_jsonl: Option&lt;PathBuf&gt; }`.
    - Define `SwebenchOracleResult { instance_report, verdict,
      artifacts, command, duration_ms }`.
    - Unit tests: default config sets the four documented defaults;
      SwebenchInstanceReport JSON parse maps `patch_is_None` field
      correctly; TransitionBucket round-trips.
  </in_scope>
  <out_of_scope>
    - run_swebench_lite_oracle implementation (TASK-CORPUS-020).
    - parse_swebench_instance_report (TASK-CORPUS-018).
    - swebench_report_to_verdict (TASK-CORPUS-018).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/swebench.rs">
    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct SwebenchPrediction {
        pub instance_id: String,
        pub model_name_or_path: String,
        pub model_patch: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum SwebenchPredictionMode { Gold, Custom(SwebenchPrediction) }

    #[derive(Clone, Debug)]
    pub struct SwebenchOracleConfig {
        pub instance_id: String,
        pub mode: SwebenchPredictionMode,
        pub run_id: String,
        pub run_root: std::path::PathBuf,
        pub venv_python: std::path::PathBuf,
        pub dataset_name: String,
        pub split: String,
        pub namespace: String,
        pub instance_timeout: std::time::Duration,
        pub overall_timeout: std::time::Duration,
    }

    impl SwebenchOracleConfig {
        pub fn defaults_for(instance_id: &amp;str, mode: SwebenchPredictionMode,
                            run_id: &amp;str, run_root: std::path::PathBuf,
                            venv_python: std::path::PathBuf) -&gt; Self;
    }

    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct TransitionBucket { pub success: Vec&lt;String&gt;, pub failure: Vec&lt;String&gt; }

    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct TestsStatus {
        pub fail_to_pass: TransitionBucket,
        pub pass_to_pass: TransitionBucket,
        pub fail_to_fail: TransitionBucket,
        pub pass_to_fail: TransitionBucket,
    }

    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct SwebenchInstanceReport {
        pub instance_id: String,
        #[serde(rename = "patch_is_None")] pub patch_is_none: bool,
        pub patch_exists: bool,
        pub patch_successfully_applied: bool,
        pub resolved: bool,
        #[serde(default)] pub tests_status: Option&lt;TestsStatus&gt;,
    }

    #[derive(Clone, Debug)]
    pub struct SwebenchOracleArtifacts {
        pub overall_report: std::path::PathBuf,
        pub instance_report: std::path::PathBuf,
        pub run_log: std::path::PathBuf,
        pub test_output: std::path::PathBuf,
        pub patch: std::path::PathBuf,
        pub predictions_jsonl: Option&lt;std::path::PathBuf&gt;,
    }

    pub struct SwebenchOracleResult {
        pub instance_report: SwebenchInstanceReport,
        pub verdict: crate::OracleVerdict,
        pub artifacts: SwebenchOracleArtifacts,
        pub command: Vec&lt;String&gt;,
        pub duration_ms: u128,
    }
    </signature>
  </signatures>

  <constraints>
    - `patch_is_None` field gets `#[serde(rename = "patch_is_None")]`
      (the harness emits the capitalized form; we MUST NOT bend
      Python's casing).
    - `tests_status` is `#[serde(default)]` so missing-key cases
      (patch did not run) deserialize as None per BR-CORPUS-02.
    - Default constructor constants are spelled in the function,
      not in ad-hoc strings: namespace = "swebench" via const.
    - `run_id` field is `String`; the validator (TASK-CORPUS-009)
      validates it before subprocess spawn.
    - `Duration` defaults: 1800s (instance), 3600s (overall) per
      REQ-CORPUS-14.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- swebench_types` passes.
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
impl SwebenchOracleConfig {
    pub fn defaults_for(instance_id: &amp;str, mode: SwebenchPredictionMode,
                        run_id: &amp;str, run_root: PathBuf, venv_python: PathBuf) -&gt; Self {
        Self {
            instance_id: instance_id.into(),
            mode, run_id: run_id.into(), run_root, venv_python,
            dataset_name: "princeton-nlp/SWE-bench_Lite".into(),
            split: "test".into(),
            namespace: "swebench".into(),
            instance_timeout: Duration::from_secs(1800),
            overall_timeout: Duration::from_secs(3600),
        }
    }
}

#[test]
fn instance_report_parses_patch_is_None_capitalized() {
    let json = r#"{"instance_id":"x","patch_is_None":false,
                    "patch_exists":true,"patch_successfully_applied":true,
                    "resolved":true}"#;
    let r: SwebenchInstanceReport = serde_json::from_str(json).unwrap();
    assert!(!r.patch_is_none);
    assert!(r.tests_status.is_none());
}

#[test]
fn defaults_for_sets_canonical_dataset_namespace_timeouts() {
    let c = SwebenchOracleConfig::defaults_for(
        "sympy__sympy-20590", SwebenchPredictionMode::Gold,
        "mejepa-corpus", PathBuf::from("/tmp/run"), PathBuf::from("/p/python"));
    assert_eq!(c.dataset_name, "princeton-nlp/SWE-bench_Lite");
    assert_eq!(c.namespace, "swebench");
    assert_eq!(c.instance_timeout, Duration::from_secs(1800));
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/swebench.rs">Add 9 type definitions + defaults_for constructor.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 3 swebench_types tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>9 types defined with required derives</criterion>
  <criterion>SwebenchOracleConfig::defaults_for sets the 4 documented defaults</criterion>
  <criterion>patch_is_None capitalized field deserializes correctly</criterion>
  <criterion>tests_status defaults to None when absent</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- swebench_types</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
