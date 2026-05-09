# TASK-CORPUS-018 — parse_swebench_instance_report + swebench_report_to_verdict

```xml
<task_spec id="TASK-CORPUS-018" version="1.0">
<metadata>
  <title>parse_swebench_instance_report + swebench_report_to_verdict</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>18</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-04</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-003</task_ref>
    <task_ref>TASK-CORPUS-006</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Logic task. Two pure functions that map upstream
`swebench/harness/grading.py` semantics into our canonical
`OracleVerdict`. parse_swebench_instance_report enforces the
single-key root invariant (EC-CORPUS-07); swebench_report_to_verdict
implements the FAIL_TO_PASS / PASS_TO_PASS / FAIL_TO_FAIL /
PASS_TO_FAIL final-status mapping plus the patch-failure cases
(BR-CORPUS-02). Already shipped; this task verifies the shipped
state and locks the 11-fixture test matrix (TC-CORPUS-04).
</context>

<input_context_files>
  <file purpose="api_contract">../../technical/TECH-CORPUS.md#api_contracts (parse_swebench_instance_report, swebench_report_to_verdict)</file>
  <file purpose="business_rule">../../functional/SPEC-CORPUS.md (BR-CORPUS-02)</file>
  <file purpose="test_plan">../../functional/SPEC-CORPUS.md (TC-CORPUS-04)</file>
  <file purpose="grading_reference">swebench/harness/grading.py (read-only — DO NOT EDIT)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/swebench.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-003 complete (OracleVerdict / PerTestOutcome / TestOutcome / ExceptionClass re-exported)</check>
  <check>TASK-CORPUS-006 complete (SwebenchInstanceReport / TestsStatus types exist)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn parse_swebench_instance_report(instance_id:
      &amp;str, text: &amp;str) -&gt; OracleResult&lt;SwebenchInstanceReport&gt;`:
      1. Parse `text` as JSON; on error
         `MEJEPA_ORACLE_JSON_PARSE`.
      2. Reject non-object root with
         `MEJEPA_ORACLE_REPORT_INVALID { field: "root" }`.
      3. Reject object with != 1 key (EC-CORPUS-07).
      4. Reject if the single key != `instance_id` parameter.
      5. Deserialize the inner value into `SwebenchInstanceReport`;
         deserialization errors map to `MEJEPA_ORACLE_REPORT_INVALID`.
      6. Verify `report.instance_id == instance_id`; else
         `MEJEPA_ORACLE_REPORT_INVALID { field: "instance_id" }`.
    - Implement `pub fn swebench_report_to_verdict(report:
      &amp;SwebenchInstanceReport, exception_hint:
      Option&lt;ExceptionClass&gt;) -&gt; OracleResult&lt;OracleVerdict&gt;`:
      1. If `patch_is_none || !patch_exists ||
         !patch_successfully_applied`:
         - per_test = vec![]
         - exception = exception_hint.or(Some(Other))
         - resolved = report.resolved
      2. Else (tests ran):
         - For each test_id in F2P.success: PerTestOutcome { id, Pass }
         - For each test_id in F2P.failure: PerTestOutcome { id, Fail }
         - For each test_id in P2P.success: PerTestOutcome { id, Pass }
         - For each test_id in P2P.failure: PerTestOutcome { id, Fail }
         - For each test_id in F2F.success: PerTestOutcome { id, Pass }   // unexpected pass — skip-or-error per harness
         - For each test_id in F2F.failure: PerTestOutcome { id, Fail }
         - For each test_id in P2F.success: PerTestOutcome { id, Pass }   // unexpected pass
         - For each test_id in P2F.failure: PerTestOutcome { id, Fail }
         - exception = exception_hint
         - resolved = report.resolved
    - Per BR-CORPUS-02: when the harness ran past patch-application
      and produced typed failure output (exception_hint
      `Some(specific)`), prefer the specific class over `Other`.
    - Add 11 unit tests covering all 8 transition quadrants + 3
      patch-failure cases (TC-CORPUS-04).
  </in_scope>
  <out_of_scope>
    - extract_exception_class (TASK-CORPUS-019).
    - run_swebench_lite_oracle (TASK-CORPUS-020).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/swebench.rs">
    pub fn parse_swebench_instance_report(
        instance_id: &amp;str,
        text: &amp;str,
    ) -&gt; OracleResult&lt;SwebenchInstanceReport&gt;;

    pub fn swebench_report_to_verdict(
        report: &amp;SwebenchInstanceReport,
        exception_hint: Option&lt;crate::ExceptionClass&gt;,
    ) -&gt; OracleResult&lt;crate::OracleVerdict&gt;;
    </signature>
  </signatures>

  <constraints>
    - The mapping table for the 8 transition quadrants comes
      DIRECTLY from `swebench/harness/grading.py::get_eval_tests_report`
      semantics; cite the upstream function in a doc comment.
    - The function MUST NOT silently fall back to text parsing
      when report.json is missing — that's the caller's error
      (`MEJEPA_ORACLE_IO`).
    - PerTestOutcome.test_id uniqueness is preserved in the
      same order the harness emits (success then failure within
      each transition bucket).
    - `runtime_seconds` field is populated by the caller
      (TASK-CORPUS-020); this function leaves it 0.0 since the
      report.json doesn't carry runtime.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- swebench_report_to_verdict` passes (11 fixtures).
    - `cargo test -p context-graph-mejepa-corpus -- parse_swebench_instance_report` passes (≥ 4 tests).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn parse_swebench_instance_report(instance_id: &amp;str, text: &amp;str) -&gt; OracleResult&lt;SwebenchInstanceReport&gt; {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| OracleError::JsonParse { ... })?;
    let obj = v.as_object().ok_or_else(|| OracleError::ReportInvalid { field: "root", message: "root must be a JSON object".into() })?;
    if obj.len() != 1 {
        return Err(OracleError::ReportInvalid { field: "root", message: format!("root must contain exactly one key; found {}", obj.len()) });
    }
    let (key, val) = obj.iter().next().unwrap();
    if key != instance_id {
        return Err(OracleError::ReportInvalid { field: "instance_id", message: format!("expected `{instance_id}`, got `{key}`") });
    }
    let report: SwebenchInstanceReport = serde_json::from_value(val.clone()).map_err(...)?;
    if report.instance_id != instance_id { return Err(...); }
    Ok(report)
}

pub fn swebench_report_to_verdict(report: &amp;SwebenchInstanceReport, hint: Option&lt;ExceptionClass&gt;) -&gt; OracleResult&lt;OracleVerdict&gt; {
    let patch_failed = report.patch_is_none || !report.patch_exists || !report.patch_successfully_applied;
    let exception = if patch_failed { hint.or(Some(ExceptionClass::Other)) } else { hint };
    let per_test = if patch_failed { vec![] } else if let Some(ts) = &amp;report.tests_status {
        let mut out = vec![];
        for id in &amp;ts.fail_to_pass.success { out.push(PerTestOutcome { test_id: id.clone(), outcome: TestOutcome::Pass }); }
        for id in &amp;ts.fail_to_pass.failure { out.push(PerTestOutcome { test_id: id.clone(), outcome: TestOutcome::Fail }); }
        // ... 6 more loops covering P2P/F2F/P2F success+failure
        out
    } else { vec![] };
    Ok(OracleVerdict { per_test, exception, resolved: report.resolved, runtime_seconds: 0.0 })
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/swebench.rs">Verify parse + map; add 11-fixture test matrix.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 11 + 4 unit tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>11 transition/patch-failure fixtures all map correctly per grading.py</criterion>
  <criterion>parse rejects multi-key root (EC-CORPUS-07)</criterion>
  <criterion>parse rejects mismatched instance_id</criterion>
  <criterion>BR-CORPUS-02: hint preferred over Other when present</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- swebench_report_to_verdict</command>
  <command>cargo test -p context-graph-mejepa-corpus -- parse_swebench_instance_report</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
