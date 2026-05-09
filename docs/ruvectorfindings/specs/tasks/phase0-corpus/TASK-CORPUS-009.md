# TASK-CORPUS-009 — Docker Run-ID Validator

```xml
<task_spec id="TASK-CORPUS-009" version="1.0">
<metadata>
  <title>Docker Run-ID Validator (validate_run_id)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>9</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-05</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Foundation task. Defense-in-depth validator for Docker container
names. A real bug found 2026-05-09e: `+` in `run_id` is rejected
by Docker's container-name regex; the subprocess spawn succeeds
but produces no artifacts. Validating BEFORE spawn prevents 2,400
silent failures. Per BR-CORPUS-03 + SEC-MEJEPA-05.
</context>

<input_context_files>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-06, EC-CORPUS-..., BR-CORPUS-03)</file>
  <file purpose="error_taxonomy">../../technical/TECH-CORPUS.md#error_taxonomy (MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE)</file>
  <file purpose="security_rule">../../constitution.md#security_requirements (SEC-MEJEPA-05)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/swebench.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-001 complete (OracleError::DockerRunIdUnsafe variant exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `validate_run_id(run_id: &amp;str) -&gt;
      OracleResult&lt;()&gt;` per TECH-CORPUS spec:
      1. Empty -&gt; DockerRunIdUnsafe { position: 0, character: 0 }.
      2. First char not in `[a-zA-Z0-9]` -&gt; same error
         with position 0 and the offending char.
      3. Remaining chars first violator of `[a-zA-Z0-9_.-]`
         -&gt; same error with the position cited.
      4. Else Ok(()).
    - Make the function callable from CLI argument parsing AND
      from `run_swebench_lite_oracle` (defense in depth).
    - Add unit tests covering: valid name `mejepa-corpus`,
      empty, leading hyphen, leading dot, leading digit (allowed
      per regex — first char `[a-zA-Z0-9]`), embedded `+`,
      embedded space, embedded `\n`, embedded `/`,
      embedded `:`, control character (0x01).
    - Verify against TC-CORPUS-05's test inputs:
      "mejepa+v1", "mejepa v1", "mejepa\\nv1", "mejepa/v1",
      ".mejepa-v1", "1mejepa-v1" (last is ALLOWED — first char is
      digit which IS in `[a-zA-Z0-9]`); the test fixture says
      ".mejepa-v1" is invalid (correct: leading `.`) and
      "1mejepa-v1" is VALID (digit first char). Confirm and
      adjust the spec test description.
  </in_scope>
  <out_of_scope>
    - Subprocess spawn (TASK-CORPUS-020).
    - CLI argument parsing (TASK-CORPUS-026).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/swebench.rs">
    pub fn validate_run_id(run_id: &amp;str) -&gt; OracleResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - Pure function; no allocations beyond String error formatting.
    - Regex spelling MUST be exactly `^[a-zA-Z0-9][a-zA-Z0-9_.-]*$`
      so the error message can cite it verbatim.
    - Error variant: `OracleError::DockerRunIdUnsafe { run_id,
      character, position }`.
    - 0-indexed position consistent with byte offset (run_id is
      ASCII per the regex; non-ASCII is rejected at first violator).
    - No use of the `regex` crate; hand-rolled char predicate.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- validate_run_id`
      passes (≥ 8 cases including the 6 from TC-CORPUS-05).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn validate_run_id(run_id: &amp;str) -&gt; OracleResult&lt;()&gt; {
    let mut iter = run_id.chars().enumerate();
    let (pos, first) = iter.next().ok_or_else(|| OracleError::DockerRunIdUnsafe {
        run_id: run_id.to_string(), character: '\0', position: 0,
    })?;
    if !first.is_ascii_alphanumeric() {
        return Err(OracleError::DockerRunIdUnsafe {
            run_id: run_id.to_string(), character: first, position: pos,
        });
    }
    for (i, c) in iter {
        let ok = c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-';
        if !ok {
            return Err(OracleError::DockerRunIdUnsafe {
                run_id: run_id.to_string(), character: c, position: i,
            });
        }
    }
    Ok(())
}

#[test]
fn validate_run_id_accepts_canonical() {
    assert!(validate_run_id("mejepa-corpus").is_ok());
    assert!(validate_run_id("1mejepa-v1").is_ok()); // digit first char OK
    assert!(validate_run_id("a").is_ok());
}

#[test]
fn validate_run_id_rejects_documented_unsafe() {
    for bad in ["mejepa+v1", "mejepa v1", "mejepa\nv1", "mejepa/v1",
                ".mejepa-v1", "mejepa:v1", ""] {
        assert!(validate_run_id(bad).is_err(),
                "expected {bad:?} to be rejected");
    }
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/swebench.rs">Add validate_run_id function.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add validate_run_id tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>Empty / leading-non-alnum / embedded special char all rejected with correct character + position</criterion>
  <criterion>"1mejepa-v1" (digit first) accepted</criterion>
  <criterion>".mejepa-v1" (leading dot) rejected at position 0</criterion>
  <criterion>"mejepa+v1" rejected at position 6 with character '+'</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- validate_run_id</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
