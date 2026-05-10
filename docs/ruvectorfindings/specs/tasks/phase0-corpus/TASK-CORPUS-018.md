# TASK-CORPUS-018 — parse_swebench_instance_report + swebench_report_to_verdict

```xml
<task_spec id="TASK-CORPUS-018" version="1.0">
<metadata>
  <title>parse_swebench_instance_report + swebench_report_to_verdict</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `parse_swebench_instance_report(report_path) -> OracleResult<SwebenchInstanceReport>` (swebench.rs) and `swebench_report_to_verdict(report, exception_hint) -> OracleResult<OracleVerdict>` (swebench.rs) — convert the SWE-bench `tests_status` buckets (FAIL_TO_PASS / PASS_TO_PASS / FAIL_TO_FAIL / PASS_TO_FAIL each with `success` and `failure` lists) per the canonical `swebench/harness/grading.py::get_eval_report` semantics into per-test `PerTestOutcome` rows + the cumulative `OracleVerdict.all_passed` flag. `exception_hint` is retained for API compatibility but intentionally excluded from the persisted canonical verdict hash because raw `test_output.txt` rendering can differ across equivalent unresolved runs; the durable source of truth is structured `report.json` / `tests_status`. End-to-end FSV against `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest` confirms round-trip behavior, and production verification targeted `sympy__sympy-13146|wrong_file` after this normalization fix.
  </implementation_state>
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
         - exception = Some(Other)
         - resolved = report.resolved
      2. Else (tests ran):
         - For each test_id in F2P.success: PerTestOutcome { id, Pass }
         - For each test_id in F2P.failure: PerTestOutcome { id, Fail }
         - For each test_id in P2P.success: PerTestOutcome { id, Pass }
         - For each test_id in P2P.failure: PerTestOutcome { id, Fail }
         - For each test_id in F2F.success: PerTestOutcome { id, Fail }   // stayed failing
         - For each test_id in F2F.failure: PerTestOutcome { id, Pass }   // transitioned to passing
         - For each test_id in P2F.success: PerTestOutcome { id, Fail }   // transitioned to failing
         - For each test_id in P2F.failure: PerTestOutcome { id, Pass }   // stayed passing
         - exception = None unless this is a deterministic ContextGraph timeout report
         - resolved = report.resolved
    - Per the 2026-05-09 production root-cause fix: when the harness
      ran past patch application, use structured `tests_status` as the
      canonical verdict and do not hash `test_output.txt`-scraped
      exception hints. Raw log exception hints remain diagnostic only.
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
    - PerTestOutcome.test_id uniqueness is preserved and the persisted
      verdict is sorted by test id so equivalent SWE-bench runs hash
      identically.
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
    let exception = if patch_failed { Some(ExceptionClass::Other) } else { None };
    let per_test = if patch_failed { vec![] } else if let Some(ts) = &amp;report.tests_status {
        let mut out = vec![];
        for id in &amp;ts.fail_to_pass.success { out.push(PerTestOutcome { test_id: id.clone(), outcome: TestOutcome::Pass }); }
        for id in &amp;ts.fail_to_pass.failure { out.push(PerTestOutcome { test_id: id.clone(), outcome: TestOutcome::Fail }); }
        // ... 6 more loops covering P2P/F2F/P2F success+failure
        out
    } else { vec![] };
    Ok(OracleVerdict { per_test, exception })
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
  <criterion>Raw test_output exception hints are not included in the canonical persisted verdict hash</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- swebench_report_to_verdict</command>
  <command>cargo test -p context-graph-mejepa-corpus -- parse_swebench_instance_report</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
