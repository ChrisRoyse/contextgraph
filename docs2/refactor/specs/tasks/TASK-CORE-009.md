# TASK-CORE-009: Remove Projection Code

```xml
<task_spec id="TASK-CORE-009" version="2.0">
<metadata>
  <title>Remove Broken Projection Code from Alignment Calculator</title>
  <status>completed</status>
  <layer>foundation</layer>
  <sequence>9</sequence>
  <implements>
    <requirement_ref>REQ-NORTHSTAR-REMOVE-03</requirement_ref>
    <requirement_ref>REQ-APPLES-TO-APPLES-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="DONE">TASK-CORE-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <audited_at>2025-01-09</audited_at>
</metadata>

<current_state_audit>
  <!-- VERIFIED: 2025-01-09 via compiler diagnostics and code inspection -->

  <finding id="F1" status="CONFIRMED">
    <description>project_embedding function exists and is UNUSED</description>
    <evidence>
      Compiler warning: "calculator.rs: Line 458:8 associated function 'project_embedding' is never used"
      Location: crates/context-graph-core/src/alignment/calculator.rs:458-477
    </evidence>
    <action>DELETE function entirely</action>
  </finding>

  <finding id="F2" status="CONFIRMED">
    <description>Test exists that calls project_embedding</description>
    <evidence>
      Test function: test_project_embedding_dimensions at lines 1411-1428
      Calls DefaultAlignmentCalculator::project_embedding at lines 1415, 1419, 1424
    </evidence>
    <action>DELETE test entirely</action>
  </finding>

  <finding id="F3" status="VERIFIED_CORRECT">
    <description>compute_all_space_alignments already uses apples-to-apples comparison</description>
    <evidence>
      Lines 393-442 implement correct E1 vs E1, E2 vs E2, etc. comparison.
      Comments explicitly reference ARCH-02 (apples-to-apples).
      Does NOT call project_embedding anywhere.
    </evidence>
    <action>NO CHANGE - already correct</action>
  </finding>

  <finding id="F4" status="VERIFIED_ABSENT">
    <description>reduce_dimensions and align_via_projection do NOT exist</description>
    <evidence>
      grep -r "reduce_dimensions\|align_via_projection" returns 0 results
      Original task spec was based on outdated assumptions
    </evidence>
    <action>NO ACTION needed - functions never existed</action>
  </finding>

  <non_existent_in_task_spec>
    <!-- The original task incorrectly assumed these existed -->
    <item>fn reduce_dimensions(...) - DOES NOT EXIST</item>
    <item>fn align_via_projection(...) - DOES NOT EXIST</item>
    <item>fn cosine_similarity_projected(...) - DOES NOT EXIST</item>
  </non_existent_in_task_spec>
</current_state_audit>

<context>
The alignment calculator at crates/context-graph-core/src/alignment/calculator.rs contains
a single unused projection function (project_embedding at line 458) that violates AP-03
(No dimension projection). The rest of the calculator already correctly implements
ARCH-02 (apples-to-apples comparison) via compute_all_space_alignments.

This task removes ONLY the project_embedding function and its associated test.
No placeholder is needed - the existing implementation is CORRECT.
</context>

<objective>
Remove the project_embedding function (lines 458-477) and its test
(test_project_embedding_dimensions at lines 1411-1428) from calculator.rs.
That's it. Nothing else needs to change.
</objective>

<rationale>
Per constitution.yaml AP-03:
  - "Projection across embedding spaces" is an anti-pattern
  - "Linear interpolation to resize embeddings loses semantic information"

The project_embedding function attempts to resize embeddings via linear interpolation,
which destroys semantic relationships. It's currently unused (compiler warning confirms)
but its presence violates AP-03 and could mislead future developers.

The existing compute_all_space_alignments (lines 393-442) already correctly implements
ARCH-02 by comparing E1 to E1, E2 to E2, etc. using the goal's TeleologicalArray.
</rationale>

<exact_code_to_remove>
  <removal id="R1" file="crates/context-graph-core/src/alignment/calculator.rs">
    <start_line>455</start_line>
    <end_line>477</end_line>
    <reason>Violates AP-03 - dimension projection anti-pattern</reason>
    <code><![CDATA[
    /// Project goal embedding to target dimension using linear interpolation.
    ///
    /// This allows a 1024D goal embedding to be compared against any dimension.
    fn project_embedding(source: &[f32], target_dim: usize) -> Vec<f32> {
        if source.is_empty() || target_dim == 0 {
            return vec![0.0; target_dim];
        }

        if source.len() == target_dim {
            return source.to_vec();
        }

        let mut result = Vec::with_capacity(target_dim);
        let ratio = source.len() as f32 / target_dim as f32;

        for i in 0..target_dim {
            let src_idx = (i as f32 * ratio) as usize;
            let src_idx = src_idx.min(source.len() - 1);
            result.push(source[src_idx]);
        }

        result
    }
]]></code>
  </removal>

  <removal id="R2" file="crates/context-graph-core/src/alignment/calculator.rs">
    <start_line>1410</start_line>
    <end_line>1428</end_line>
    <reason>Test for removed function - becomes dead code</reason>
    <code><![CDATA[
    #[test]
    fn test_project_embedding_dimensions() {
        let source = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]; // 8D

        // Project to smaller dimension
        let projected_4d = DefaultAlignmentCalculator::project_embedding(&source, 4);
        assert_eq!(projected_4d.len(), 4);

        // Project to same dimension
        let projected_8d = DefaultAlignmentCalculator::project_embedding(&source, 8);
        assert_eq!(projected_8d.len(), 8);
        assert_eq!(projected_8d, source);

        // Project to larger dimension
        let projected_16d = DefaultAlignmentCalculator::project_embedding(&source, 16);
        assert_eq!(projected_16d.len(), 16);

        println!("[VERIFIED] project_embedding handles different dimensions correctly");
    }
]]></code>
  </removal>
</exact_code_to_remove>

<what_NOT_to_do>
  <!-- CRITICAL: The original task spec was WRONG about several things -->
  <item>DO NOT add a standalone calculate_alignment function - the existing trait implementation works</item>
  <item>DO NOT add AlignmentInterpretation enum - already exists as AlignmentThreshold</item>
  <item>DO NOT add placeholder todo!() calls - existing code is functional</item>
  <item>DO NOT modify compute_all_space_alignments - it's already correct</item>
  <item>DO NOT touch the GoalAlignmentCalculator trait - out of scope</item>
  <item>DO NOT add any "backwards compatibility" shims</item>
  <item>DO NOT remove ANY other tests - they validate correct behavior</item>
</what_NOT_to_do>

<prerequisites>
  <check status="VERIFIED">TASK-CORE-001 complete (protocol constants removed)</check>
  <check status="VERIFIED">calculator.rs exists at crates/context-graph-core/src/alignment/calculator.rs</check>
  <check status="VERIFIED">project_embedding exists at line 458 (compiler warning confirms)</check>
  <check status="VERIFIED">compute_all_space_alignments already uses ARCH-02 correctly</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Remove project_embedding function (lines 455-477)</item>
    <item>Remove test_project_embedding_dimensions test (lines 1410-1428)</item>
  </in_scope>
  <out_of_scope>
    <item>Any other changes to calculator.rs</item>
    <item>TeleologicalComparator implementation (TASK-LOGIC-004)</item>
    <item>GoalAlignmentCalculator trait refactor (TASK-LOGIC-010)</item>
    <item>Adding new functions or structs</item>
    <item>Modifying existing tests (except the one being removed)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <constraints>
    <constraint>project_embedding function deleted from calculator.rs</constraint>
    <constraint>test_project_embedding_dimensions test deleted</constraint>
    <constraint>No compilation errors</constraint>
    <constraint>All remaining tests pass</constraint>
    <constraint>No new warnings introduced</constraint>
  </constraints>
</definition_of_done>

<full_state_verification>
  <source_of_truth>
    <item>compiler diagnostics (cargo check -p context-graph-core)</item>
    <item>test output (cargo test -p context-graph-core --lib)</item>
    <item>grep results for projection patterns</item>
  </source_of_truth>

  <execute_and_inspect>
    <step order="1">
      <command>cargo check -p context-graph-core 2>&1 | grep -i "project_embedding"</command>
      <expected_before>warning: associated function `project_embedding` is never used</expected_before>
      <expected_after>NO OUTPUT (function removed, warning gone)</expected_after>
      <fail_if>Any output mentioning project_embedding</fail_if>
    </step>

    <step order="2">
      <command>rg "project_embedding" --type rust</command>
      <expected_before>crates/context-graph-core/src/alignment/calculator.rs (5 matches)</expected_before>
      <expected_after>NO OUTPUT (all references removed)</expected_after>
      <fail_if>Any file paths returned</fail_if>
    </step>

    <step order="3">
      <command>rg "reduce_dimensions|align_via_projection" --type rust</command>
      <expected_before>NO OUTPUT</expected_before>
      <expected_after>NO OUTPUT</expected_after>
      <fail_if>Any output (these functions never existed)</fail_if>
    </step>

    <step order="4">
      <command>cargo test -p context-graph-core --lib -- alignment::calculator --nocapture 2>&1 | tail -20</command>
      <expected_before>test alignment::calculator::tests::test_project_embedding_dimensions ... ok</expected_before>
      <expected_after>NO test_project_embedding_dimensions in output (test removed)</expected_after>
      <fail_if>test_project_embedding_dimensions appears</fail_if>
    </step>

    <step order="5">
      <command>cargo test -p context-graph-core --lib 2>&1 | grep -E "^test result:"</command>
      <expected_after>test result: ok. (all tests pass, count reduced by 1)</expected_after>
      <fail_if>FAILED or error in output</fail_if>
    </step>

    <step order="6">
      <command>cargo clippy -p context-graph-core -- -D warnings 2>&1 | grep -c "error\|warning"</command>
      <expected_after>0 (no new warnings)</expected_after>
      <fail_if>Count greater than 0</fail_if>
    </step>
  </execute_and_inspect>
</full_state_verification>

<edge_cases>
  <edge_case id="EC1" priority="CRITICAL">
    <scenario>Removing function changes line numbers in file</scenario>
    <risk>Line numbers in this spec become stale after edit</risk>
    <mitigation>
      Use grep/search to locate code rather than relying on line numbers.
      The function signature "fn project_embedding(source: &[f32], target_dim: usize)" is unique.
    </mitigation>
    <verification>grep -n "fn project_embedding" before and after edit</verification>
  </edge_case>

  <edge_case id="EC2" priority="HIGH">
    <scenario>Some code path still references project_embedding</scenario>
    <risk>Compilation fails with "cannot find function project_embedding"</risk>
    <mitigation>
      VERIFIED: Current compiler shows "never used" warning, proving no callers exist.
      If compilation fails, it means hidden reference - search entire codebase.
    </mitigation>
    <verification>cargo check -p context-graph-core must succeed with 0 errors</verification>
  </edge_case>

  <edge_case id="EC3" priority="MEDIUM">
    <scenario>Test removal affects test count expectations</scenario>
    <risk>CI/CD checks test count and fails</risk>
    <mitigation>Test count will decrease by 1 - this is expected and correct.</mitigation>
    <verification>Confirm test suite still reports ok, even if count decreases</verification>
  </edge_case>
</edge_cases>

<evidence_of_success>
  <log id="LOG1" phase="BEFORE">
    <command>rg "project_embedding" --type rust -c</command>
    <expected_output>crates/context-graph-core/src/alignment/calculator.rs:5</expected_output>
  </log>

  <log id="LOG2" phase="AFTER">
    <command>rg "project_embedding" --type rust -c</command>
    <expected_output>(no output - empty result)</expected_output>
  </log>

  <log id="LOG3" phase="AFTER">
    <command>cargo check -p context-graph-core 2>&1 | grep -c warning</command>
    <expected_output>0 (or fewer than before)</expected_output>
  </log>

  <log id="LOG4" phase="AFTER">
    <command>cargo test -p context-graph-core --lib 2>&1 | grep "test result"</command>
    <expected_output>test result: ok. N passed; 0 failed; 0 ignored</expected_output>
  </log>
</evidence_of_success>

<implementation_steps>
  <step order="1">
    <action>Locate project_embedding function</action>
    <command>grep -n "fn project_embedding" crates/context-graph-core/src/alignment/calculator.rs</command>
    <expected>Line 458 (approximately)</expected>
  </step>

  <step order="2">
    <action>Delete project_embedding function (including doc comments)</action>
    <details>Remove from "/// Project goal embedding" comment through closing brace of function</details>
  </step>

  <step order="3">
    <action>Locate test_project_embedding_dimensions test</action>
    <command>grep -n "test_project_embedding_dimensions" crates/context-graph-core/src/alignment/calculator.rs</command>
    <expected>Line 1411 (approximately, will shift after step 2)</expected>
  </step>

  <step order="4">
    <action>Delete test_project_embedding_dimensions test</action>
    <details>Remove from #[test] attribute through closing brace</details>
  </step>

  <step order="5">
    <action>Verify compilation</action>
    <command>cargo check -p context-graph-core</command>
    <expected>Compiles with 0 errors, fewer warnings than before</expected>
  </step>

  <step order="6">
    <action>Run tests</action>
    <command>cargo test -p context-graph-core --lib</command>
    <expected>All tests pass (one fewer test than before)</expected>
  </step>

  <step order="7">
    <action>Verify no projection references remain</action>
    <command>rg "project_embedding|reduce_dimensions|dimension.*reduction" --type rust -i</command>
    <expected>No output</expected>
  </step>
</implementation_steps>

<constitution_compliance>
  <rule id="AP-03" status="ENFORCED">
    <text>No dimension projection - Linear interpolation to resize embeddings loses semantic information</text>
    <action>Removing project_embedding eliminates this anti-pattern</action>
  </rule>

  <rule id="ARCH-02" status="ALREADY_COMPLIANT">
    <text>Apples-to-apples comparison - Same embedder to same embedder only</text>
    <evidence>compute_all_space_alignments at lines 393-442 already implements this correctly</evidence>
  </rule>
</constitution_compliance>

<files_to_modify>
  <file path="crates/context-graph-core/src/alignment/calculator.rs">
    <change type="DELETE">project_embedding function (lines ~455-477)</change>
    <change type="DELETE">test_project_embedding_dimensions test (lines ~1410-1428)</change>
  </file>
</files_to_modify>

<files_to_create>
  <!-- NONE - this is a pure removal task -->
</files_to_create>

<validation_commands>
  <command purpose="verify_removal">rg "project_embedding" --type rust</command>
  <command purpose="verify_no_dimension_reduction">rg "reduce_dimensions|dimension.*reduction" --type rust -i</command>
  <command purpose="verify_compilation">cargo check -p context-graph-core</command>
  <command purpose="verify_tests">cargo test -p context-graph-core --lib</command>
  <command purpose="verify_no_warnings">cargo clippy -p context-graph-core -- -D warnings</command>
</validation_commands>

<failure_modes>
  <failure id="FM1">
    <symptom>Compilation error: cannot find function `project_embedding`</symptom>
    <cause>Hidden caller not detected by compiler warning</cause>
    <resolution>Search entire workspace: rg "project_embedding" and remove caller</resolution>
  </failure>

  <failure id="FM2">
    <symptom>Test failure in unrelated test</symptom>
    <cause>Accidental deletion of adjacent code</cause>
    <resolution>git diff to inspect changes, revert and redo carefully</resolution>
  </failure>

  <failure id="FM3">
    <symptom>New warning about unused code</symptom>
    <cause>project_embedding was the only caller of some helper</cause>
    <resolution>Review warning, delete newly-orphaned helper if appropriate</resolution>
  </failure>
</failure_modes>
</task_spec>
```
