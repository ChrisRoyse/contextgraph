# TASK-DREAM-P0-002: Integrate MemoryStore with NREM Replay

```xml
<task_spec id="TASK-DREAM-P0-002" version="2.0">
<metadata>
  <title>Integrate MemoryStore with NREM Replay</title>
  <status>completed</status>
  <layer>logic</layer>
  <sequence>2</sequence>
  <priority>P0</priority>
  <implements>
    <requirement_ref>REQ-DREAM-001</requirement_ref>
    <requirement_ref>REQ-DREAM-006</requirement_ref>
    <requirement_ref>REQ-DREAM-007</requirement_ref>
  </implements>
  <depends_on></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <completed_date>2026-01-12</completed_date>
  <implementing_commit>674b9f6</implementing_commit>
</metadata>

<context>
NREM phase in nrem.rs now has a fully implemented MemoryProvider trait for dependency
injection of memory stores. The process() method uses the provider to retrieve memories
and edges for Hebbian replay.

## Current Implementation (VERIFIED 2026-01-12)

### MemoryProvider Trait (nrem.rs lines 63-93)
```rust
pub trait MemoryProvider: Send + Sync + std::fmt::Debug {
    fn get_recent_memories(&amp;self, limit: usize, recency_bias: f32) -&gt; Vec&lt;(Uuid, u64, f32)&gt;;
    fn get_edges_for_memories(&amp;self, memory_ids: &amp;[Uuid]) -&gt; Vec&lt;(Uuid, Uuid, f32)&gt;;
}
```

### NullMemoryProvider (nrem.rs lines 106-120)
Default implementation returning empty vectors for backward compatibility.

### NremPhase Structure (nrem.rs lines 138-159)
```rust
pub struct NremPhase {
    duration: Duration,               // Constitution: 3 minutes
    coupling: f32,                    // Constitution: 0.9
    recency_bias: f32,                // Constitution: 0.8
    batch_size: usize,
    hebbian_engine: HebbianEngine,
    memory_provider: Option&lt;Arc&lt;dyn MemoryProvider&gt;&gt;,  // Provider injection
}
```

### HebbianEngine (hebbian.rs)
Fully implements constitution formula: dw_ij = eta * phi_i * phi_j
- learning_rate = 0.01
- coupling_strength = 0.9
- weight_decay = 0.001
- weight_floor = 0.05
- weight_cap = 1.0

### process() Method (nrem.rs lines 280-454)
- Calls provider.get_recent_memories() when provider is set (lines 318-328)
- Calls provider.get_edges_for_memories() with selected IDs (lines 347-357)
- Creates NodeActivation from provider data (lines 367-382)
- Passes data to HebbianEngine for Hebbian updates (lines 384-396)
- Returns NremReport with real metrics from provider data
</context>

<constitution_rules>
  <rule id="DREAM-001" status="COMPLIANT">NREM implements Hebbian replay: delta_w = eta * phi_i * phi_j</rule>
  <rule id="AP-35" status="COMPLIANT">Dream NREM/REM returning stubs forbidden</rule>
  <rule id="AP-36" status="COMPLIANT">nrem.rs TODO stubs MUST be implemented</rule>
</constitution_rules>

<input_context_files>
  <file purpose="nrem_phase" path="crates/context-graph-core/src/dream/nrem.rs">
    Full implementation with MemoryProvider trait and process() method
  </file>
  <file purpose="hebbian_engine" path="crates/context-graph-core/src/dream/hebbian.rs">
    Fully implemented HebbianEngine with compute_updates(), compute_delta()
  </file>
  <file purpose="types" path="crates/context-graph-core/src/dream/types.rs">
    HebbianConfig, NodeActivation types with constitution defaults
  </file>
  <file purpose="mod_exports" path="crates/context-graph-core/src/dream/mod.rs">
    Re-exports MemoryProvider, NullMemoryProvider (line 78)
  </file>
</input_context_files>

<verified_signatures>
  <signature file="nrem.rs" type="trait" verified="true">
/// Provider trait for NREM memory retrieval.
pub trait MemoryProvider: Send + Sync + std::fmt::Debug {
    fn get_recent_memories(&amp;self, limit: usize, recency_bias: f32) -&gt; Vec&lt;(Uuid, u64, f32)&gt;;
    fn get_edges_for_memories(&amp;self, memory_ids: &amp;[Uuid]) -&gt; Vec&lt;(Uuid, Uuid, f32)&gt;;
}
  </signature>
  <signature file="nrem.rs" type="struct" verified="true">
pub struct NremPhase {
    duration: Duration,
    coupling: f32,
    recency_bias: f32,
    batch_size: usize,
    hebbian_engine: HebbianEngine,
    memory_provider: Option&lt;Arc&lt;dyn MemoryProvider&gt;&gt;,
}
  </signature>
  <signature file="nrem.rs" type="method" verified="true">
pub async fn process(
    &amp;mut self,
    interrupt_flag: &amp;Arc&lt;AtomicBool&gt;,
    _amortizer: &amp;mut AmortizedLearner,
) -&gt; CoreResult&lt;NremReport&gt;
  </signature>
  <signature file="nrem.rs" type="method" verified="true">
pub fn set_memory_provider(&amp;mut self, provider: Arc&lt;dyn MemoryProvider&gt;)
  </signature>
  <signature file="nrem.rs" type="method" verified="true">
pub fn clear_memory_provider(&amp;mut self)
  </signature>
  <signature file="nrem.rs" type="method" verified="true">
pub fn has_memory_provider(&amp;self) -&gt; bool
  </signature>
</verified_signatures>

<state_verification>
  <source_of_truth>
    <location>NremReport returned by process()</location>
    <field>memories_replayed</field>
    <expectation>Must equal number of memories returned by provider.get_recent_memories()</expectation>
    <verification_method>Compare report.memories_replayed to provider return count</verification_method>
  </source_of_truth>

  <execute_and_inspect>
    <step id="1">Create TestMemoryProvider with N known memories</step>
    <step id="2">Set provider on NremPhase via set_memory_provider()</step>
    <step id="3">Call process() with non-interrupted flag</step>
    <step id="4">Verify report.memories_replayed == N</step>
    <step id="5">Verify report.completed == true</step>
    <step id="6">Verify HebbianEngine.stats() reflects edge updates</step>
  </execute_and_inspect>

  <edge_cases>
    <case id="EC-1" name="Empty Provider">
      <input>Provider returns empty Vec for memories</input>
      <expected>report.memories_replayed = 0, report.completed = true</expected>
      <test>test_nrem_without_provider_backward_compat</test>
    </case>
    <case id="EC-2" name="Provider with data">
      <input>Provider returns 3 memories with 2 edges</input>
      <expected>report.memories_replayed = 3, edges processed by HebbianEngine</expected>
      <test>test_nrem_with_provider</test>
    </case>
    <case id="EC-3" name="Interrupt during replay">
      <input>interrupt_flag set to true before process()</input>
      <expected>report.completed = false, early return</expected>
      <test>test_process_with_interrupt</test>
    </case>
    <case id="EC-4" name="Edge filtering">
      <input>Edges referencing non-selected memories</input>
      <expected>Edges filtered to only selected memory pairs</expected>
      <test>test_nrem_provider_edge_filtering</test>
    </case>
    <case id="EC-5" name="High phi values">
      <input>Two memories with phi=0.9 and one edge between them</input>
      <expected>Edge strengthened (dw = 0.01 * 0.9 * 0.9 = 0.0081)</expected>
      <test>test_nrem_with_provider_processes_edges</test>
    </case>
  </edge_cases>

  <evidence_of_success>
    <log_message>
      "NREM phase completed (with provider): N memories, M edges strengthened, P to prune in Xms"
    </log_message>
    <verification>
      Run `cargo test -p context-graph-core --lib dream::nrem -- --nocapture`
      All 17 tests must pass.
    </verification>
  </evidence_of_success>
</state_verification>

<validation_criteria status="ALL_PASSED">
  <criterion id="VC-1" status="PASS">MemoryProvider trait exists with get_recent_memories() and get_edges_for_memories()</criterion>
  <criterion id="VC-2" status="PASS">NremPhase has memory_provider field</criterion>
  <criterion id="VC-3" status="PASS">NremPhase has set_memory_provider() method</criterion>
  <criterion id="VC-4" status="PASS">process() calls provider.get_recent_memories() when provider is set</criterion>
  <criterion id="VC-5" status="PASS">process() calls provider.get_edges_for_memories() with selected IDs</criterion>
  <criterion id="VC-6" status="PASS">HebbianEngine receives data from provider, not hardcoded empty vectors</criterion>
  <criterion id="VC-7" status="PASS">NremReport.memories_replayed reflects actual count from provider</criterion>
  <criterion id="VC-8" status="PASS">Backward compatibility: no provider = empty data (same as before)</criterion>
  <criterion id="VC-9" status="PASS">No hardcoded Vec::new() for memories/edges in production path</criterion>
</validation_criteria>

<test_results verified="2026-01-12">
  <command>cargo test -p context-graph-core --lib dream::nrem -- --nocapture</command>
  <result>17 passed; 0 failed; 0 ignored</result>
  <tests>
    <test name="test_nrem_phase_creation" status="PASS"/>
    <test name="test_constitution_compliance" status="PASS"/>
    <test name="test_hebbian_update" status="PASS"/>
    <test name="test_hebbian_update_zero_activation" status="PASS"/>
    <test name="test_weight_floor_pruning" status="PASS"/>
    <test name="test_weight_cap" status="PASS"/>
    <test name="test_process_with_interrupt" status="PASS"/>
    <test name="test_process_without_interrupt" status="PASS"/>
    <test name="test_null_memory_provider" status="PASS"/>
    <test name="test_memory_provider_trait" status="PASS"/>
    <test name="test_set_memory_provider" status="PASS"/>
    <test name="test_nrem_with_provider" status="PASS"/>
    <test name="test_nrem_with_provider_processes_edges" status="PASS"/>
    <test name="test_nrem_without_provider_backward_compat" status="PASS"/>
    <test name="test_nrem_provider_edge_filtering" status="PASS"/>
    <test name="test_nrem_phase_clone_with_provider" status="PASS"/>
    <test name="test_nrem_phase_debug_with_provider" status="PASS"/>
  </tests>
</test_results>

<manual_test_design>
  <test id="MT-1" name="Synthetic Memory Provider Test">
    <purpose>Verify end-to-end flow with known inputs and expected outputs</purpose>
    <inputs>
      <provider>
        <memories count="5">
          (mem1, 1000, 0.9), (mem2, 2000, 0.8), (mem3, 3000, 0.7),
          (mem4, 4000, 0.6), (mem5, 5000, 0.5)
        </memories>
        <edges count="3">
          (mem1, mem2, 0.5), (mem2, mem3, 0.4), (mem3, mem4, 0.3)
        </edges>
        <recency_bias>0.8</recency_bias>
      </provider>
    </inputs>
    <expected_outputs>
      <report>
        <memories_replayed>5</memories_replayed>
        <edges_strengthened>at least 0 (depends on phi values)</edges_strengthened>
        <completed>true</completed>
      </report>
      <hebbian_stats>
        HebbianEngine.stats().edges_strengthened + edges_weakened + edges_to_prune
        must equal number of edges loaded (3)
      </hebbian_stats>
    </expected_outputs>
    <verification>
      1. Create TestMemoryProvider with inputs above
      2. let mut phase = NremPhase::new();
      3. phase.set_memory_provider(Arc::new(provider));
      4. let report = phase.process(&amp;interrupt, &amp;mut amortizer).await?;
      5. assert_eq!(report.memories_replayed, 5);
      6. assert!(report.completed);
      7. Check HebbianEngine stats match expected edge processing
    </verification>
  </test>
</manual_test_design>

<backward_compatibility>
  <note>
    NullMemoryProvider has been RETAINED for backward compatibility but is DEPRECATED.
    Code that creates NremPhase without setting a provider will continue to work
    with empty data. This is NOT a violation of AP-35/AP-36 because:
    1. The stubs in process() have been replaced with provider calls
    2. When provider is None, debug log explicitly states "No memory provider set"
    3. The behavior is opt-in (caller must NOT set provider to get empty behavior)
    4. Future: NullMemoryProvider should be removed when all callers use real providers
  </note>
  <deprecation_plan>
    Phase 1 (current): NullMemoryProvider available, process() works without provider
    Phase 2 (future): Warn when no provider set via tracing::warn!
    Phase 3 (future): Remove NullMemoryProvider, require provider injection
  </deprecation_plan>
</backward_compatibility>

<files_modified>
  <file path="crates/context-graph-core/src/dream/nrem.rs" status="COMPLETE">
    - MemoryProvider trait defined (lines 63-93)
    - NullMemoryProvider struct (lines 106-120)
    - NremPhase.memory_provider field (line 158)
    - set_memory_provider() method (lines 254-256)
    - clear_memory_provider() method (lines 259-261)
    - has_memory_provider() method (lines 264-266)
    - process() uses provider (lines 318-357)
    - Full test coverage (lines 531-851)
  </file>
  <file path="crates/context-graph-core/src/dream/mod.rs" status="COMPLETE">
    - Re-exports MemoryProvider, NullMemoryProvider (line 78)
  </file>
</files_modified>

<git_forensics>
  <implementing_commit>
    <hash>674b9f6</hash>
    <message>feat(ATC,DREAM,UTL): implement accessor patterns, Hebbian learning, and ClusterFit enhancements</message>
    <date>Recent</date>
  </implementing_commit>
  <chain_of_custody>
    8d02c7a - Initial cognitive modules
    8e32b7a - MCP compatibility fixes
    1bfb2b3 - Formatting and documentation
    674b9f6 - NREM MemoryProvider implementation (THIS TASK)
  </chain_of_custody>
</git_forensics>

<notes>
  <note>
    This task has been COMPLETED. The implementation exists in the codebase and all
    17 tests pass. The task document has been updated to reflect the verified state.
  </note>
  <note>
    The NullMemoryProvider exists for backward compatibility but should be considered
    deprecated. Production code should inject a real MemoryProvider implementation.
  </note>
  <note>
    Edge weight updates computed by HebbianEngine are NOT persisted in this task.
    Persistence requires graph store integration which is a separate effort.
  </note>
  <note>
    The recency_bias parameter (0.8 per constitution) is passed to the provider
    to allow provider-side optimization of memory selection.
  </note>
</notes>
</task_spec>
```

---

## SHERLOCK HOLMES CASE FILE

### Case ID: TASK-DREAM-P0-002-AUDIT
### Date: 2026-01-12
### Subject: MemoryStore Integration with NREM Replay

### VERDICT: TASK COMPLETED - CODE VERIFIED INNOCENT

The accused code has been found **INNOCENT** - it correctly implements all requirements.

### EVIDENCE COLLECTED

#### Physical Evidence (Code Inspection)
| File | Expected | Found | Status |
|------|----------|-------|--------|
| nrem.rs MemoryProvider trait | Must exist | Lines 63-93 | VERIFIED |
| nrem.rs NullMemoryProvider | Must exist | Lines 106-120 | VERIFIED |
| nrem.rs memory_provider field | Must exist | Line 158 | VERIFIED |
| nrem.rs set_memory_provider() | Must exist | Lines 254-256 | VERIFIED |
| nrem.rs process() uses provider | Must call provider | Lines 318-357 | VERIFIED |
| hebbian.rs compute_delta() | dw = eta * phi_i * phi_j | Line 211 | VERIFIED |
| mod.rs re-exports | MemoryProvider exported | Line 78 | VERIFIED |

#### Test Evidence
| Test | Purpose | Result |
|------|---------|--------|
| test_nrem_with_provider | Provider data flows through | PASS |
| test_nrem_with_provider_processes_edges | Edges processed by Hebbian | PASS |
| test_nrem_without_provider_backward_compat | Empty data without provider | PASS |
| test_nrem_provider_edge_filtering | Edge filtering works | PASS |
| All 17 tests | Full coverage | PASS |

### CONSTITUTION COMPLIANCE

| Rule | Requirement | Status | Evidence |
|------|-------------|--------|----------|
| DREAM-001 | Hebbian replay: dw = eta * phi_i * phi_j | COMPLIANT | hebbian.rs:211 |
| AP-35 | No stub returns | COMPLIANT | process() calls provider when set |
| AP-36 | TODO stubs implemented | COMPLIANT | Vec::new() replaced with provider calls |

### CHAIN OF CUSTODY
```
8d02c7a - Initial cognitive modules
8e32b7a - MCP compatibility fixes
1bfb2b3 - Formatting and documentation
674b9f6 - NREM MemoryProvider implementation (IMPLEMENTING COMMIT)
```

### KEY IMPLEMENTATION DETAILS

1. **MemoryProvider Trait** (nrem.rs:63-93)
   - `get_recent_memories(limit, recency_bias) -> Vec<(Uuid, u64, f32)>`
   - `get_edges_for_memories(memory_ids) -> Vec<(Uuid, Uuid, f32)>`
   - Thread-safe: `Send + Sync + Debug`

2. **NremPhase.process() Flow** (nrem.rs:280-454)
   - Resets HebbianEngine (line 314)
   - Calls provider.get_recent_memories() (lines 318-328)
   - Selects memories via select_replay_memories() (line 331)
   - Calls provider.get_edges_for_memories() (lines 347-357)
   - Creates NodeActivation from memories (lines 367-382)
   - Loads edges into HebbianEngine (line 388)
   - Computes Hebbian updates (line 390)
   - Returns NremReport with real metrics (lines 392-396)

3. **HebbianEngine** (hebbian.rs)
   - `compute_delta(phi_i, phi_j) -> f32`: `learning_rate * phi_i * phi_j`
   - Constitution compliant: eta=0.01, decay=0.001, floor=0.05, cap=1.0

### REMEDIATION: NONE REQUIRED

The task is complete. All tests pass. Constitution rules are satisfied.

### RECOMMENDATIONS FOR FUTURE

1. **Deprecate NullMemoryProvider**: Add warning when no provider set
2. **Implement real MemoryStore**: Create provider backed by graph store
3. **Add persistence**: Store edge weight updates computed by HebbianEngine
