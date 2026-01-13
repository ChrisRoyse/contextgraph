# TASK-DREAM-P0-003: Complete Amortized Edge Creation

```xml
<task_spec id="TASK-DREAM-P0-003" version="2.0">
<metadata>
  <title>Complete Amortized Edge Creation</title>
  <status>complete-pending-strictness</status>
  <layer>logic</layer>
  <sequence>3</sequence>
  <priority>P0</priority>
  <implements>
    <requirement_ref>REQ-DREAM-005</requirement_ref>
  </implements>
  <depends_on></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <last_audit>2026-01-12</last_audit>
  <auditor>SHERLOCK-HOLMES</auditor>
</metadata>

<context>
## Current Implementation Status (Audited 2026-01-12)

The AmortizedLearner in `amortized.rs` is **FULLY IMPLEMENTED** with all required components:

### Implemented Components (VERIFIED)

| Component | Status | Location |
|-----------|--------|----------|
| `ShortcutEdge` struct | COMPLETE | Lines 66-80 |
| `ShortcutEdge::from_candidate()` | COMPLETE | Lines 82-97 |
| `EdgeCreator` trait | COMPLETE | Lines 99-127 |
| `NullEdgeCreator` struct | COMPLETE | Lines 129-146 |
| `AmortizedLearner.edge_creator` field | COMPLETE | Line 215 |
| `AmortizedLearner::set_edge_creator()` | COMPLETE | Lines 271-273 |
| `AmortizedLearner::with_edge_creator()` | COMPLETE | Lines 278-289 |
| `create_shortcut()` with EdgeCreator call | COMPLETE | Lines 398-440 |

### Git History
```
0a2cc1b feat(dream): migrate dream layer thresholds to domain-aware ATC system
1bfb2b3 refactor: format codebase and update TASK-CORE-010 documentation
8d02c7a feat: add cognitive modules (causal, dream, neuromod, steering) with MCP handlers
```

### Test Results (22/22 PASSED)
```
test dream::amortized::tests::test_constitution_compliance ... ok
test dream::amortized::tests::test_shortcut_creation_calls_creator ... ok
test dream::amortized::tests::test_shortcut_edge_from_candidate ... ok
test dream::amortized::tests::test_create_shortcut_creator_error_propagates ... ok
test dream::amortized::tests::test_quality_gate_enforcement_with_creator ... ok
test dream::amortized::tests::test_multiple_shortcuts_with_creator ... ok
... (22 total tests passed)
```

### TODO/STUB Markers
**NONE FOUND** - All stub markers have been removed from amortized.rs.

### Remaining Issue: Backwards Compatibility Mode

The current implementation silently skips edge persistence when no EdgeCreator is set:

```rust
// Lines 429-434 in amortized.rs
} else {
    debug!(
        "No edge creator set, shortcut {} -> {} tracked but not persisted",
        edge.source, edge.target
    );
}
```

**REQUIREMENT**: Per user directive, this backwards compatibility MUST BE REMOVED.
When no EdgeCreator is set, the system MUST return an error instead of silently skipping.

</context>

<constitution_rules>
  <rule id="DREAM-005">Amortized shortcuts for 3+ hop paths traversed 5+ times</rule>
  <rule_verification>
    <check>min_hops >= 3 (enforced in meets_quality_gate())</check>
    <check>min_traversals >= 5 (enforced in meets_quality_gate())</check>
    <check>min_confidence >= 0.7 (enforced in meets_quality_gate())</check>
  </rule_verification>
</constitution_rules>

<input_context_files>
  <file purpose="implementation" path="crates/context-graph-core/src/dream/amortized.rs">
    COMPLETE implementation with EdgeCreator integration
  </file>
  <file purpose="re-exports" path="crates/context-graph-core/src/dream/mod.rs">
    Re-exports: EdgeCreator, NullEdgeCreator, ShortcutEdge
  </file>
  <file purpose="constants" path="crates/context-graph-core/src/dream/mod.rs">
    Constitution constants: MIN_SHORTCUT_HOPS=3, MIN_SHORTCUT_TRAVERSALS=5, SHORTCUT_CONFIDENCE_THRESHOLD=0.7
  </file>
</input_context_files>

<prerequisites>
  <check type="struct_exists" status="VERIFIED">ShortcutEdge struct with is_shortcut and original_path fields</check>
  <check type="trait_exists" status="VERIFIED">EdgeCreator trait with create_edge() method</check>
  <check type="struct_exists" status="VERIFIED">NullEdgeCreator implements EdgeCreator</check>
  <check type="field_exists" status="VERIFIED">AmortizedLearner.edge_creator: Option&lt;Arc&lt;dyn EdgeCreator&gt;&gt;</check>
  <check type="method_exists" status="VERIFIED">AmortizedLearner::set_edge_creator()</check>
  <check type="method_exists" status="VERIFIED">AmortizedLearner::with_edge_creator()</check>
  <check type="method_verified" status="VERIFIED">create_shortcut() calls EdgeCreator.create_edge()</check>
  <check type="no_stubs" status="VERIFIED">No TODO/STUB/Agent 2 markers in amortized.rs</check>
</prerequisites>

<full_state_verification>
  <source_of_truth>
    <description>EdgeCreator.create_edge() receives ShortcutEdge with correct data</description>
    <verification_method>Use RecordingEdgeCreator to capture created edges</verification_method>
    <expected_state>
      - edge.source == candidate.source (first node in path)
      - edge.target == candidate.target (last node in path)
      - edge.is_shortcut == true (always)
      - edge.original_path == candidate.path_nodes (full path)
      - edge.weight == candidate.combined_weight
      - edge.confidence == candidate.min_confidence
    </expected_state>
  </source_of_truth>

  <execute_and_inspect>
    <step>Create AmortizedLearner with RecordingEdgeCreator</step>
    <step>Record 6 traversals of a 5-node path (4 hops)</step>
    <step>Call process_candidates()</step>
    <step>Verify: creator.get_created_edges().len() == 1</step>
    <step>Verify: learner.total_shortcuts_created() == 1</step>
    <step>Verify: Both counters match</step>
  </execute_and_inspect>

  <edge_cases>
    <case id="EC-1" name="candidate_fails_quality_gate">
      <input>Candidate with hop_count=2 (below threshold)</input>
      <expected>create_shortcut() returns Ok(false), EdgeCreator NOT called</expected>
      <test>test_quality_gate_enforcement_with_creator</test>
      <status>VERIFIED</status>
    </case>
    <case id="EC-2" name="edge_already_exists">
      <input>EdgeCreator returns Ok(false)</input>
      <expected>create_shortcut() returns Ok(false), counters NOT incremented</expected>
      <test>test_create_shortcut_creator_returns_false</test>
      <status>VERIFIED</status>
    </case>
    <case id="EC-3" name="creator_returns_error">
      <input>EdgeCreator returns Err(...)</input>
      <expected>Error propagates up, counters NOT incremented</expected>
      <test>test_create_shortcut_creator_error_propagates</test>
      <status>VERIFIED</status>
    </case>
    <case id="EC-4" name="no_creator_set">
      <input>AmortizedLearner with no edge_creator</input>
      <expected_current>Silent skip with debug log (BACKWARDS COMPAT)</expected_current>
      <expected_new>Return Err(CoreError::Internal("No EdgeCreator configured"))</expected_new>
      <test>test_create_shortcut_without_creator (needs update)</test>
      <status>REQUIRES_CHANGE</status>
    </case>
  </edge_cases>

  <evidence_of_success>
    <log_pattern>INFO "Creating shortcut: {source} -> {target} (hops={N}, traversals={M}, confidence={C})"</log_pattern>
    <counter_check>learner.total_shortcuts_created() == creator.get_created_edges().len()</counter_check>
    <field_check>All created edges have is_shortcut=true</field_check>
    <field_check>All created edges have original_path.len() >= 4 (3+ hops means 4+ nodes)</field_check>
  </evidence_of_success>
</full_state_verification>

<scope>
  <in_scope>
    <item status="COMPLETE">Define ShortcutEdge struct with is_shortcut and original_path fields</item>
    <item status="COMPLETE">Define EdgeCreator trait with create_edge() method</item>
    <item status="COMPLETE">Add edge_creator field to AmortizedLearner</item>
    <item status="COMPLETE">Add set_edge_creator() method</item>
    <item status="COMPLETE">Add with_edge_creator() constructor</item>
    <item status="COMPLETE">Modify create_shortcut() to call EdgeCreator</item>
    <item status="COMPLETE">Create NullEdgeCreator for testing/logging</item>
    <item status="COMPLETE">Remove TODO/stub comments</item>
    <item status="COMPLETE">Add comprehensive EdgeCreator tests</item>
    <item status="COMPLETE">Re-export types in mod.rs</item>
    <item status="PENDING">Remove backwards compatibility (error when no creator set)</item>
  </in_scope>
  <out_of_scope>
    <item>Implementing a real graph store EdgeCreator</item>
    <item>REM integration (TASK-DREAM-P0-001)</item>
    <item>NREM integration (TASK-DREAM-P0-002)</item>
    <item>Actual graph persistence</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures status="ALL_VERIFIED">
    <signature file="amortized.rs" type="struct" status="VERIFIED">
/// Edge to be created as a shortcut
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutEdge {
    pub source: Uuid,
    pub target: Uuid,
    pub weight: f32,
    pub confidence: f32,
    pub is_shortcut: bool,
    pub original_path: Vec&lt;Uuid&gt;,
}
    </signature>
    <signature file="amortized.rs" type="trait" status="VERIFIED">
/// Trait for creating shortcut edges in storage
pub trait EdgeCreator: Send + Sync {
    fn create_edge(&amp;self, edge: &amp;ShortcutEdge) -&gt; CoreResult&lt;bool&gt;;
}
    </signature>
    <signature file="amortized.rs" type="struct_field" status="VERIFIED">
pub struct AmortizedLearner {
    edge_creator: Option&lt;Arc&lt;dyn EdgeCreator&gt;&gt;,
}
    </signature>
    <signature file="amortized.rs" type="method" status="VERIFIED">
pub fn set_edge_creator(&amp;mut self, creator: Arc&lt;dyn EdgeCreator&gt;)
    </signature>
    <signature file="amortized.rs" type="method" status="VERIFIED">
pub fn with_edge_creator(creator: Arc&lt;dyn EdgeCreator&gt;) -&gt; Self
    </signature>
    <signature file="amortized.rs" type="method" status="NEEDS_STRICTNESS_UPDATE">
pub fn create_shortcut(&amp;mut self, candidate: &amp;ShortcutCandidate) -&gt; CoreResult&lt;bool&gt;
// MUST call edge_creator.create_edge() when creator is set
// MUST return error when creator is NOT set (NO BACKWARDS COMPAT)
    </signature>
  </signatures>

  <constraints>
    <constraint status="VERIFIED">When edge_creator is set, create_edge() must be called for qualifying candidates</constraint>
    <constraint status="VERIFIED">ShortcutEdge.is_shortcut must always be true</constraint>
    <constraint status="VERIFIED">ShortcutEdge.original_path must contain the full path from candidate</constraint>
    <constraint status="VERIFIED">Quality gate enforces: hops >= 3, traversals >= 5, confidence >= 0.7</constraint>
    <constraint status="VERIFIED">No TODO, stub, or "Agent 2" comments in amortized.rs</constraint>
    <constraint status="PENDING">When edge_creator is NOT set, return error (no silent skip)</constraint>
  </constraints>

  <verification>
    <command description="Run amortized tests">cargo test -p context-graph-core --lib dream::amortized -- --nocapture</command>
    <command description="Verify no TODO markers">grep -r "TODO" crates/context-graph-core/src/dream/amortized.rs | wc -l</command>
    <command description="Verify no stub markers">grep -r "stub\|STUB\|Agent 2" crates/context-graph-core/src/dream/amortized.rs | wc -l</command>
    <result status="PASS">22 tests passed, 0 TODO markers, 0 stub markers</result>
  </verification>
</definition_of_done>

<manual_test_design>
  <test id="MT-1" name="End-to-End Shortcut Creation">
    <description>
      Verify that a 5-node path traversed 6 times produces a correct ShortcutEdge
    </description>
    <setup>
      <step>Create RecordingEdgeCreator</step>
      <step>Create AmortizedLearner with edge creator</step>
    </setup>
    <input>
      <param name="nodes">5-node path (4 hops): [N0, N1, N2, N3, N4]</param>
      <param name="traversals">6 (exceeds threshold of 5)</param>
      <param name="weights">[0.8, 0.9, 0.7, 0.85] (combined: 0.4284)</param>
      <param name="confidences">[0.9, 0.8, 0.75, 0.85] (min: 0.75)</param>
    </input>
    <expected_output>
      <assertion>learner.process_candidates() returns Ok(1)</assertion>
      <assertion>creator.get_created_edges().len() == 1</assertion>
      <assertion>edge.is_shortcut == true</assertion>
      <assertion>edge.original_path.len() == 5</assertion>
      <assertion>edge.source == nodes[0]</assertion>
      <assertion>edge.target == nodes[4]</assertion>
      <assertion>edge.confidence == 0.75</assertion>
    </expected_output>
    <verification_code>
```rust
#[test]
fn test_manual_shortcut_creation() {
    let creator = Arc::new(RecordingEdgeCreator::new());
    let mut learner = AmortizedLearner::with_edge_creator(creator.clone());

    let nodes = vec![
        Uuid::new_v4(), // N0
        Uuid::new_v4(), // N1
        Uuid::new_v4(), // N2
        Uuid::new_v4(), // N3
        Uuid::new_v4(), // N4
    ];
    let weights = vec![0.8, 0.9, 0.7, 0.85];
    let confidences = vec![0.9, 0.8, 0.75, 0.85];

    // Record 6 traversals
    for _ in 0..6 {
        learner.record_traversal(&amp;nodes, &amp;weights, &amp;confidences);
    }

    // Process
    let created = learner.process_candidates().unwrap();
    assert_eq!(created, 1);

    // Verify source of truth
    let edges = creator.get_created_edges();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges.len(), learner.total_shortcuts_created());

    let edge = &amp;edges[0];
    assert!(edge.is_shortcut);
    assert_eq!(edge.original_path.len(), 5);
    assert_eq!(edge.source, nodes[0]);
    assert_eq!(edge.target, nodes[4]);
    assert_eq!(edge.confidence, 0.75); // min of [0.9, 0.8, 0.75, 0.85]
}
```
    </verification_code>
    <status>VERIFIED (test_shortcut_creation_calls_creator)</status>
  </test>

  <test id="MT-2" name="No Creator Strict Mode">
    <description>
      Verify that create_shortcut() returns error when no EdgeCreator is set
    </description>
    <input>
      <param name="learner">AmortizedLearner::new() (no creator)</param>
      <param name="candidate">Valid candidate passing quality gate</param>
    </input>
    <expected_output>
      <assertion>create_shortcut() returns Err(CoreError::Internal(...))</assertion>
      <assertion>Error message contains "EdgeCreator" or "creator"</assertion>
      <assertion>learner.shortcuts_created_this_cycle() == 0</assertion>
    </expected_output>
    <status>PENDING - Requires code change to remove backwards compat</status>
  </test>
</manual_test_design>

<pending_strictness_change>
  <description>
    Remove backwards compatibility mode where missing EdgeCreator is silently skipped.
  </description>
  <current_behavior>
    Lines 429-434: When edge_creator is None, logs debug message and returns Ok(true)
  </current_behavior>
  <required_behavior>
    When edge_creator is None, return Err(CoreError::Internal("No EdgeCreator configured.
    Shortcut persistence requires an EdgeCreator implementation."))
  </required_behavior>
  <code_change>
```rust
// CURRENT (amortized.rs lines 419-434):
if let Some(creator) = &amp;self.edge_creator {
    let created = creator.create_edge(&amp;edge)?;
    if !created {
        debug!(...);
        return Ok(false);
    }
} else {
    debug!("No edge creator set, shortcut {} -> {} tracked but not persisted", ...);
}

// REQUIRED:
if let Some(creator) = &amp;self.edge_creator {
    let created = creator.create_edge(&amp;edge)?;
    if !created {
        debug!(...);
        return Ok(false);
    }
} else {
    return Err(CoreError::Internal(
        "No EdgeCreator configured. Shortcut persistence requires an EdgeCreator implementation."
            .into(),
    ));
}
```
  </code_change>
  <test_updates>
    <update>
      test_create_shortcut_without_creator must be updated to expect Err instead of Ok(true)
    </update>
    <update>
      test_create_shortcut (without creator) must be removed or updated
    </update>
  </test_updates>
</pending_strictness_change>

<error_handling>
  <error case="EdgeCreator returns error">
    Propagate error up via ? operator (IMPLEMENTED)
  </error>
  <error case="EdgeCreator returns Ok(false)">
    Edge may already exist, return Ok(false) without incrementing counters (IMPLEMENTED)
  </error>
  <error case="No EdgeCreator set">
    CURRENT: Log debug message, return Ok(true) (BACKWARDS COMPAT)
    REQUIRED: Return Err(CoreError::Internal(...)) (STRICT MODE)
  </error>
  <error case="Candidate fails quality gate">
    Return Ok(false) without calling EdgeCreator (IMPLEMENTED)
  </error>
</error_handling>

<test_commands>
  <command description="Run amortized learning tests">cargo test -p context-graph-core --lib dream::amortized -- --nocapture</command>
  <command description="Run all dream tests">cargo test -p context-graph-core --lib dream -- --nocapture</command>
  <command description="Verify no TODO markers">grep -r "TODO" crates/context-graph-core/src/dream/amortized.rs | wc -l</command>
  <command description="Verify no stub markers">grep -r "stub\|STUB\|Agent 2" crates/context-graph-core/src/dream/amortized.rs | wc -l</command>
  <result>22 passed, 0 TODO, 0 STUB markers</result>
</test_commands>

<validation_criteria>
  <criterion id="VC-1" status="VERIFIED">ShortcutEdge struct exists with source, target, weight, confidence, is_shortcut, original_path</criterion>
  <criterion id="VC-2" status="VERIFIED">EdgeCreator trait exists with create_edge() method</criterion>
  <criterion id="VC-3" status="VERIFIED">NullEdgeCreator struct implements EdgeCreator</criterion>
  <criterion id="VC-4" status="VERIFIED">AmortizedLearner has edge_creator field</criterion>
  <criterion id="VC-5" status="VERIFIED">AmortizedLearner has set_edge_creator() method</criterion>
  <criterion id="VC-6" status="VERIFIED">create_shortcut() creates ShortcutEdge::from_candidate()</criterion>
  <criterion id="VC-7" status="VERIFIED">create_shortcut() calls edge_creator.create_edge() when creator is set</criterion>
  <criterion id="VC-8" status="VERIFIED">ShortcutEdge.is_shortcut is always true</criterion>
  <criterion id="VC-9" status="VERIFIED">ShortcutEdge.original_path contains candidate.path_nodes</criterion>
  <criterion id="VC-10" status="VERIFIED">No TODO, stub, or "Agent 2" markers remain in amortized.rs</criterion>
  <criterion id="VC-11" status="VERIFIED">Quality gate still enforces: hops >= 3, traversals >= 5, confidence >= 0.7</criterion>
  <criterion id="VC-12" status="PENDING">create_shortcut() errors when no EdgeCreator is set (no backwards compat)</criterion>
</validation_criteria>

<notes>
  <note>
    AUDIT FINDING: The implementation is COMPLETE except for the strict mode change.
    All original requirements from v1.0 have been implemented and verified.
  </note>
  <note>
    The NullEdgeCreator exists for testing/logging purposes but should NOT be used
    as a "silent skip" mechanism. Production code MUST configure a real EdgeCreator.
  </note>
  <note>
    ShortcutEdge.is_shortcut is always true. This flag allows graph stores to
    distinguish shortcut edges from regular edges for different query behaviors.
  </note>
  <note>
    ShortcutEdge.original_path stores the full multi-hop path. This allows the
    system to explain why a shortcut exists and potentially remove it if the
    underlying path becomes invalid.
  </note>
  <note>
    PENDING CHANGE: Remove backwards compatibility mode (lines 429-434) to enforce
    strict EdgeCreator requirement per user directive.
  </note>
</notes>
</task_spec>
```

---

## Sherlock Holmes Forensic Investigation Report

### Case ID: TASK-DREAM-P0-003-AUDIT-2026-01-12
### Investigator: SHERLOCK-HOLMES
### Date: 2026-01-12

### Executive Summary

| Field | Value |
|-------|-------|
| Task ID | TASK-DREAM-P0-003 |
| Title | Complete Amortized Edge Creation |
| Previous Status | ready |
| **Current Status** | **complete-pending-strictness** |
| Layer | Logic |
| Priority | P0 (Critical) |
| Constitution Rule | DREAM-005 |

### Investigation Verdict: IMPLEMENTATION COMPLETE

The task document claimed status "ready" (not yet implemented), but forensic investigation reveals the implementation is **FULLY COMPLETE**:

| Component | Status | Evidence |
|-----------|--------|----------|
| ShortcutEdge struct | VERIFIED | Lines 66-80, all fields present |
| EdgeCreator trait | VERIFIED | Lines 99-127, create_edge() method |
| NullEdgeCreator | VERIFIED | Lines 129-146 |
| edge_creator field | VERIFIED | Line 215 in AmortizedLearner |
| set_edge_creator() | VERIFIED | Lines 271-273 |
| with_edge_creator() | VERIFIED | Lines 278-289 |
| create_shortcut() integration | VERIFIED | Lines 398-440, calls EdgeCreator |
| TODO/STUB markers | VERIFIED CLEAN | grep found 0 matches |
| Test coverage | VERIFIED | 22/22 tests passing |

### Constitution Compliance: DREAM-005

| Requirement | Threshold | Enforcement | Status |
|-------------|-----------|-------------|--------|
| Minimum hops | >= 3 | meets_quality_gate() | VERIFIED |
| Minimum traversals | >= 5 | meets_quality_gate() | VERIFIED |
| Minimum confidence | >= 0.7 | meets_quality_gate() | VERIFIED |

### Pending Change: Strict Mode

One user requirement remains unimplemented:

**CURRENT BEHAVIOR** (backwards compatible):
```rust
// When no EdgeCreator is set, silently skip persistence
} else {
    debug!("No edge creator set, shortcut {} -> {} tracked but not persisted", ...);
}
```

**REQUIRED BEHAVIOR** (strict mode):
```rust
// When no EdgeCreator is set, return error
} else {
    return Err(CoreError::Internal("No EdgeCreator configured...".into()));
}
```

This change requires:
1. Modify lines 429-434 in amortized.rs
2. Update test_create_shortcut_without_creator to expect Err

### Source of Truth Verification

| Check | Expected | Actual | Verdict |
|-------|----------|--------|---------|
| RecordingEdgeCreator receives edge | edge.source == nodes[0] | MATCH | PASS |
| RecordingEdgeCreator receives edge | edge.target == nodes[4] | MATCH | PASS |
| RecordingEdgeCreator receives edge | edge.is_shortcut == true | MATCH | PASS |
| RecordingEdgeCreator receives edge | edge.original_path.len() == 5 | MATCH | PASS |
| Counter consistency | creator.edges.len() == learner.total_shortcuts_created | MATCH | PASS |

### Files Verified

| File | Path | Status |
|------|------|--------|
| Implementation | crates/context-graph-core/src/dream/amortized.rs | COMPLETE |
| Re-exports | crates/context-graph-core/src/dream/mod.rs | COMPLETE |

### Recommendations

1. **IMMEDIATE**: Update task status to "complete-pending-strictness"
2. **FOLLOW-UP TASK**: Create TASK-DREAM-P0-003a to implement strict mode (error on missing creator)
3. **TEST UPDATE**: Update test_create_shortcut_without_creator when strict mode is implemented

---

*"The case is closed, Watson, though one small detail remains - the strictness enforcement. The criminal elements have been identified and documented."*

-- SHERLOCK HOLMES, Forensic Code Investigator
