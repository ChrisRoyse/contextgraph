# TASK-CORPUS-020 — run_swebench_lite_oracle + wait_with_timeout

```xml
<task_spec id="TASK-CORPUS-020" version="1.0">
<metadata>
  <title>run_swebench_lite_oracle + wait_with_timeout</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `run_swebench_lite_oracle(config) -> OracleResult<SwebenchOracleResult>` (swebench.rs:215-301) — validates config (including run_id alphabet), writes the predictions JSONL when `Custom`, spawns the venv Python process invoking `python -m swebench.harness.run_evaluation`, applies `wait_with_timeout` (oracle.rs) for `overall_timeout`, locates the per-instance `report.json` under `<run_root>/logs/run_evaluation/<run_id>/<artifact_namespace>/<instance_id>/report.json`, parses, converts via `swebench_report_to_verdict`, returns the verdict + the `SwebenchOracleArtifacts` paths (predictions JSONL, run-evaluation log dir, instance log dir, instance report path). Real Docker FSV gold-mode 23.7s → resolved=true 22-tests-all-pass; custom-mode 23.0s identical.
  </implementation_state>
  <layer>logic</layer>
  <sequence>20</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-03</requirement_ref>
    <requirement_ref>REQ-CORPUS-04</requirement_ref>
    <requirement_ref>REQ-CORPUS-05</requirement_ref>
    <requirement_ref>REQ-CORPUS-14</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-006</task_ref>
    <task_ref>TASK-CORPUS-009</task_ref>
    <task_ref>TASK-CORPUS-018</task_ref>
    <task_ref>TASK-CORPUS-019</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
</metadata>

<context>
Logic task. The Rust-callable bridge to the official SWE-bench
Docker harness. Validates config (run_id regex), writes
predictions JSONL when in Custom mode, spawns
`python -m swebench.harness.run_evaluation` with timeouts,
locates artifacts, parses report.json, extracts exception class,
and returns a canonical `OracleVerdict` plus all artifact paths.
Already shipped with one-instance Docker FSV against
`sympy__sympy-20590` (gold 23.7s, custom 23.0s); this task
verifies the shipped state, locks the timeout SIGKILL contract,
and adds the EC-CORPUS-06 (Docker daemon unreachable)
regression.
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (DockerOracle, validate_run_id, extract_exception_class)</file>
  <file purpose="api_contract">../../technical/TECH-CORPUS.md#api_contracts (run_swebench_lite_oracle)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-02, AC-CORPUS-02-A/B, EC-CORPUS-06)</file>
  <file purpose="grading_reference">swebench/harness/grading.py (read-only — DO NOT EDIT)</file>
  <file purpose="reality_boundary">../../constitution.md#anti_patterns (Reality-Boundary)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/swebench.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-006 complete (Swebench types exist)</check>
  <check>TASK-CORPUS-009 complete (validate_run_id exists)</check>
  <check>TASK-CORPUS-018 complete (parse + verdict mapper exist)</check>
  <check>TASK-CORPUS-019 complete (extract_exception_class exists)</check>
  <check>Docker daemon reachable for integration test</check>
  <check>swebench Python venv at operator-supplied path</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn run_swebench_lite_oracle(config:
      &amp;SwebenchOracleConfig) -&gt;
      OracleResult&lt;SwebenchOracleResult&gt;` per
      TECH-CORPUS step 1..11.
    - Steps:
      1. validate_config(config): validate_run_id; venv_python
         exists; run_root parent writable.
      2. If Custom mode: write_predictions_jsonl into
         `run_root/predictions/<run_id>.jsonl`.
      3. Build harness argv with all required flags including
         `--timeout`, `--instance_ids`, `--namespace`,
         `--predictions_path`, etc.
      4. fs::create_dir_all(run_root)?
      5. spawn `Command` with stdout/stderr piped.
      6. wait_with_timeout(child, overall_timeout, "swebench-harness").
      7. If exit non-zero: MEJEPA_ORACLE_COMMAND_FAILED with last
         8 KB of stdout / stderr tails.
      8. locate_artifacts(config, jsonl_path?) -&gt;
         SwebenchOracleArtifacts.
      9. parse_swebench_instance_report on
         fs::read_to_string(artifacts.instance_report)?.
      10. extract_exception_class on
          fs::read_to_string(artifacts.test_output)?.
      11. swebench_report_to_verdict(&amp;report,
          exception_hint).
      12. Populate verdict.runtime_seconds = elapsed.as_secs_f64().
      13. Return SwebenchOracleResult { ... }.
    - Implement `pub(crate) fn wait_with_timeout(child:
      &amp;mut std::process::Child, overall_timeout:
      std::time::Duration, label: &amp;'static str) -&gt;
      OracleResult&lt;std::process::ExitStatus&gt;` per
      REQ-CORPUS-14:
      - Poll `child.try_wait()` in a tight loop with
        exponential backoff (start 50 ms, cap 1 s).
      - Track elapsed; on timeout call `child.kill()` then
        `child.wait()` (zombie reap), return
        `MEJEPA_ORACLE_TIMEOUT { context: label, elapsed }`.
    - Implement `pub(crate) fn write_predictions_jsonl(config:
      &amp;SwebenchOracleConfig) -&gt; OracleResult&lt;PathBuf&gt;`:
      - Only for Custom mode; one JSON line per
        SwebenchPrediction; fs::create_dir_all on parent.
    - Implement `pub(crate) fn locate_artifacts(config:
      &amp;SwebenchOracleConfig, predictions_jsonl:
      Option&lt;PathBuf&gt;) -&gt;
      OracleResult&lt;SwebenchOracleArtifacts&gt;`:
      - Path layout per AC-CORPUS-02-A:
        `<run_root>/logs/run_evaluation/<run_id>/<model_name_or_path>/<instance_id>/{report.json,run.log,test_output.txt,patch.diff}`.
      - In Gold mode `model_name_or_path = "gold"`; in Custom
        mode it's the SwebenchPrediction.model_name_or_path.
      - Each artifact MUST exist on disk; missing one is
        `MEJEPA_ORACLE_REPORT_INVALID { field: <name> }`.
    - Add tests:
      - Unit: write_predictions_jsonl creates file with one
        line per prediction.
      - Unit: locate_artifacts builds correct path for both
        Gold and Custom modes.
      - Unit: validate_config rejects bad run_id, missing
        venv_python, non-writable run_root.
      - Unit: wait_with_timeout SIGKILLs after the elapsed
        budget (TC-CORPUS-17, synthetic sleep command).
      - Integration: full FSV against the
        `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`
        image (gated behind a `--ignored` test flag so CI without
        Docker doesn't fail).
  </in_scope>
  <out_of_scope>
    - The OracleStore (TASK-CORPUS-021).
    - The CLI (TASK-CORPUS-026).
    - Any modification under `swebench/**` (Reality-Boundary).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/swebench.rs">
    pub fn run_swebench_lite_oracle(
        config: &amp;SwebenchOracleConfig,
    ) -&gt; OracleResult&lt;SwebenchOracleResult&gt;;

    pub(crate) fn wait_with_timeout(
        child: &amp;mut std::process::Child,
        overall_timeout: std::time::Duration,
        label: &amp;'static str,
    ) -&gt; OracleResult&lt;std::process::ExitStatus&gt;;

    pub(crate) fn write_predictions_jsonl(
        config: &amp;SwebenchOracleConfig,
    ) -&gt; OracleResult&lt;std::path::PathBuf&gt;;

    pub(crate) fn locate_artifacts(
        config: &amp;SwebenchOracleConfig,
        predictions_jsonl: Option&lt;std::path::PathBuf&gt;,
    ) -&gt; OracleResult&lt;SwebenchOracleArtifacts&gt;;
    </signature>
  </signatures>

  <constraints>
    - validate_run_id is called BEFORE any subprocess spawn
      (defense in depth even if CLI also called it).
    - Subprocess inherits the operator's environment; no
      CG_* env vars are stripped (the harness needs typical
      Python env vars).
    - The Command is `Command::new(&amp;config.venv_python)
      .arg("-m").arg("swebench.harness.run_evaluation")...`
      — the venv_python is what the operator specifies; we do
      NOT search PATH for `python`.
    - The FSV evidence path is /tmp/contextgraph-mejepa-corpus-fsv/
      (mode 0o600 per SEC-MEJEPA-07).
    - File MUST stay under 500 lines; if oracle.rs +
      swebench.rs together exceed, split into
      `swebench/{mod, child, artifacts, predictions}.rs`.
    - No `swebench/**` write touches; we only READ from it via
      the venv interpreter.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- run_swebench_lite_oracle` passes (≥ 4 unit tests).
    - `cargo test -p context-graph-mejepa-corpus -- wait_with_timeout` passes (TC-CORPUS-17).
    - `cargo test -p context-graph-mejepa-corpus -- locate_artifacts` passes (≥ 2 tests).
    - `cargo test -p context-graph-mejepa-corpus -- write_predictions_jsonl` passes.
    - Integration test `--ignored swebench_oracle_real_docker_smoke` passes when Docker is up.
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run_swebench_lite_oracle(config: &amp;SwebenchOracleConfig) -&gt; OracleResult&lt;SwebenchOracleResult&gt; {
    validate_config(config)?;
    let predictions_jsonl = match &amp;config.mode {
        SwebenchPredictionMode::Custom(_) =&gt; Some(write_predictions_jsonl(config)?),
        SwebenchPredictionMode::Gold =&gt; None,
    };
    let argv = build_argv(config, predictions_jsonl.as_deref());
    fs::create_dir_all(&amp;config.run_root).map_err(|e| OracleError::Io { ... })?;
    let start = std::time::Instant::now();
    let mut child = std::process::Command::new(&amp;config.venv_python)
        .args(&amp;argv[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| OracleError::Io { context: "spawn swebench harness".into(), source: e })?;
    let status = wait_with_timeout(&amp;mut child, config.overall_timeout, "swebench-harness")?;
    let elapsed = start.elapsed();
    if !status.success() {
        return Err(OracleError::CommandFailed { command: argv, exit_code: status.code(), stdout_tail: ..., stderr_tail: ... });
    }
    let artifacts = locate_artifacts(config, predictions_jsonl)?;
    let instance_report_text = fs::read_to_string(&amp;artifacts.instance_report).map_err(...)?;
    let report = parse_swebench_instance_report(&amp;config.instance_id, &amp;instance_report_text)?;
    let test_output_text = fs::read_to_string(&amp;artifacts.test_output).map_err(...)?;
    let exc = extract_exception_class(&amp;test_output_text);
    let mut verdict = swebench_report_to_verdict(&amp;report, exc)?;
    verdict.runtime_seconds = elapsed.as_secs_f64();
    Ok(SwebenchOracleResult { instance_report: report, verdict, artifacts, command: argv, duration_ms: elapsed.as_millis() })
}

pub(crate) fn wait_with_timeout(child: &amp;mut Child, overall: Duration, label: &amp;'static str) -&gt; OracleResult&lt;ExitStatus&gt; {
    let start = Instant::now();
    let mut backoff = Duration::from_millis(50);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| OracleError::Io { ... })? {
            return Ok(status);
        }
        if start.elapsed() &gt; overall {
            let _ = child.kill();
            let _ = child.wait(); // reap zombie
            return Err(OracleError::Timeout { context: label, elapsed: start.elapsed() });
        }
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(Duration::from_secs(1));
    }
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/swebench.rs">Add 4 functions; verify shipped state.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add unit tests + #[ignore] integration smoke.</file>
</files_to_modify>

<validation_criteria>
  <criterion>validate_config rejects bad run_id, missing venv, non-writable run_root</criterion>
  <criterion>locate_artifacts builds correct path for Gold and Custom modes</criterion>
  <criterion>wait_with_timeout SIGKILLs at the documented elapsed budget</criterion>
  <criterion>Real Docker FSV passes against sympy__sympy-20590 in both modes</criterion>
  <criterion>No edits to swebench/** under any code path</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- run_swebench_lite_oracle</command>
  <command>cargo test -p context-graph-mejepa-corpus -- wait_with_timeout</command>
  <command>cargo test -p context-graph-mejepa-corpus -- --ignored swebench_oracle_real_docker_smoke</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
