# TASK-CORE-010: Module Integration

```xml
<task_spec id="TASK-CORE-010" version="2.0">
<metadata>
  <title>Verify Teleology Module Integration into Crate Structure</title>
  <status>completed</status>
  <layer>foundation</layer>
  <sequence>10</sequence>
  <implements>
    <requirement_ref>REQ-MODULE-STRUCTURE-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="DONE">TASK-CORE-005</task_ref>
    <task_ref status="DONE">TASK-CORE-008</task_ref>
    <task_ref status="DONE">TASK-CORE-009</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <audited_at>2025-01-09</audited_at>
</metadata>

<current_state_audit>
  <!-- VERIFIED: 2025-01-09 via cargo check --workspace and code inspection -->

  <finding id="F1" status="ALREADY_EXISTS">
    <description>teleological/ module EXISTS in context-graph-core with extensive re-exports</description>
    <evidence>
      File: crates/context-graph-core/src/teleological/mod.rs (114 lines)
      Submodules: embedder, types, synergy_matrix, groups, resolution, vector, meaning, profile, matrix_search, comparison_error, services
      Re-exports: Embedder, EmbedderDims, EmbedderGroup, EmbedderMask, GroupAlignments, GroupType, SynergyMatrix, TeleologicalVector, TeleologicalProfile, TaskType, etc.
    </evidence>
    <action>VERIFY exports match task spec requirements</action>
  </finding>

  <finding id="F2" status="ALREADY_EXISTS">
    <description>teleological/ module EXISTS in context-graph-storage with column families and RocksDB store</description>
    <evidence>
      File: crates/context-graph-storage/src/teleological/mod.rs (113 lines)
      Submodules: column_families, indexes, quantized, rocksdb_store, schema, serialization
      Re-exports: RocksDbTeleologicalStore, TeleologicalStoreConfig, TeleologicalStoreError, HnswConfig, etc.
      Column families: 20 CFs (7 core + 13 quantized embedder)
    </evidence>
    <action>VERIFY exports match task spec requirements</action>
  </finding>

  <finding id="F3" status="ALREADY_EXISTS">
    <description>purpose/ module EXISTS in context-graph-core with GoalNode using TeleologicalArray</description>
    <evidence>
      File: crates/context-graph-core/src/purpose/mod.rs (76 lines)
      Submodules: goals, computer, default_computer, splade
      Re-exports: GoalNode, GoalLevel, GoalHierarchy, GoalDiscoveryMetadata, DiscoveryMethod, etc.
      Doc comments reference ARCH-02 (apples-to-apples comparison)
    </evidence>
    <action>VERIFY GoalNode, GoalLevel exported from lib.rs</action>
  </finding>

  <finding id="F4" status="ALREADY_EXISTS">
    <description>alignment/ module EXISTS in context-graph-core</description>
    <evidence>
      Module declared: crates/context-graph-core/src/lib.rs line 29
      TASK-CORE-009 completed: project_embedding removed from calculator.rs
    </evidence>
    <action>VERIFY alignment module functional</action>
  </finding>

  <finding id="F5" status="PARTIAL">
    <description>lib.rs re-exports exist but may not match task spec exactly</description>
    <evidence>
      context-graph-core/src/lib.rs lines 68-73 re-export teleological types
      context-graph-storage/src/lib.rs lines 66-106 re-export teleological storage types
      Missing from spec: GoalNode, GoalLevel not re-exported from core lib.rs root
    </evidence>
    <action>VERIFY and ADD missing re-exports if needed</action>
  </finding>

  <finding id="F6" status="VERIFIED_COMPILES">
    <description>Entire workspace compiles successfully</description>
    <evidence>
      Command: cargo check --workspace
      Result: Finished `dev` profile (only warnings, no errors)
      Warnings: 24 warnings in context-graph-mcp (unused methods - unrelated to this task)
    </evidence>
    <action>VERIFY cross-crate imports work in integration tests</action>
  </finding>
</current_state_audit>

<context>
This task VERIFIES (not creates) the integration of teleology modules into the crate structure.
The modules already exist and compile. This task confirms all public types are properly exported
and accessible from crate roots, and that cross-crate dependencies work correctly.

From the audit: The teleological/ and purpose/ modules are fully implemented with extensive
re-exports. The original task spec was written BEFORE implementation, assuming modules needed
creation. They now exist and work.
</context>

<objective>
1. Verify teleology/purpose/alignment modules exist in context-graph-core
2. Verify teleological module exists in context-graph-storage
3. Verify all public types accessible from crate lib.rs roots
4. Verify cross-crate imports work (storage depends on core)
5. Add any missing re-exports identified in audit (GoalNode, GoalLevel from purpose)
6. Run cargo doc to ensure documentation generates
</objective>

<rationale>
Per constitution.yaml ARCH-05:
  - "All 13 embedders must be present in teleological_array"
  - The TeleologicalArray is the atomic storage unit

Proper module organization:
1. Enables clean imports from dependent crates
2. Establishes public API surface
3. Validates all foundation work integrates correctly
4. Sets up for logic layer implementation
</rationale>

<source_of_truth>
  <primary>
    <item>cargo check --workspace (compilation status)</item>
    <item>cargo doc --workspace --no-deps (documentation generation)</item>
    <item>crates/context-graph-core/src/lib.rs (core re-exports)</item>
    <item>crates/context-graph-storage/src/lib.rs (storage re-exports)</item>
  </primary>
  <secondary>
    <item>crates/context-graph-core/src/teleological/mod.rs</item>
    <item>crates/context-graph-core/src/purpose/mod.rs</item>
    <item>crates/context-graph-storage/src/teleological/mod.rs</item>
  </secondary>
</source_of_truth>

<prerequisites>
  <check status="VERIFIED">TASK-CORE-005 complete (GoalNode uses TeleologicalArray)</check>
  <check status="VERIFIED">TASK-CORE-008 complete (RocksDbTeleologicalStore implemented)</check>
  <check status="VERIFIED">TASK-CORE-009 complete (project_embedding removed)</check>
  <check status="VERIFIED">cargo check --workspace succeeds</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Verify teleological/mod.rs exists in context-graph-core (EXISTS)</item>
    <item>Verify teleological/mod.rs exists in context-graph-storage (EXISTS)</item>
    <item>Verify purpose/mod.rs exists with GoalNode, GoalLevel exports (EXISTS)</item>
    <item>Verify alignment module exists (EXISTS)</item>
    <item>Verify all public types exported from crate lib.rs</item>
    <item>Add missing re-exports: GoalNode, GoalLevel from purpose module</item>
    <item>Run cargo doc --workspace --no-deps</item>
    <item>Verify cross-crate imports (storage uses core types)</item>
  </in_scope>
  <out_of_scope>
    <item>Creating new modules (they already exist)</item>
    <item>New logic implementation (Layer 2 tasks)</item>
    <item>MCP handler updates (Layer 3 tasks)</item>
    <item>Modifying existing functionality</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/lib.rs" status="TO_VERIFY">
      // Current exports (lines 68-73):
      pub use teleological::{
          DomainAlignments, DomainType, Embedder, EmbedderDims, EmbedderGroup, EmbedderMask,
          GroupAlignments, GroupType, MultiResolutionHierarchy, ProfileId, ProfileMetrics, SynergyMatrix,
          TaskType, TeleologicalProfile, TeleologicalVector, TuckerCore,
      };

      // MISSING - need to add:
      pub use purpose::{GoalNode, GoalLevel};
    </signature>

    <signature file="crates/context-graph-storage/src/lib.rs" status="VERIFIED">
      // Already exports (lines 66-106):
      pub use teleological::{
          RocksDbTeleologicalStore, TeleologicalStoreConfig, TeleologicalStoreError,
          TeleologicalStoreResult, HnswConfig, EmbedderIndex, DistanceMetric,
          // ... extensive re-exports ...
      };
    </signature>

    <signature file="crates/context-graph-core/src/teleological/mod.rs" status="VERIFIED">
      // Already has all required exports (114 lines of re-exports)
      pub use embedder::{Embedder, EmbedderDims, EmbedderGroup, EmbedderMask};
      pub use comparison_error::{ComparisonValidationError, ComparisonValidationResult, WeightValues};
      // ... etc ...
    </signature>

    <signature file="crates/context-graph-storage/src/teleological/mod.rs" status="VERIFIED">
      // Already has all required exports (113 lines)
      pub use rocksdb_store::{RocksDbTeleologicalStore, TeleologicalStoreConfig, TeleologicalStoreError};
      pub use indexes::{EmbedderIndex, HnswConfig, InvertedIndexConfig, DistanceMetric};
      // ... etc ...
    </signature>
  </signatures>

  <constraints>
    <constraint>All modules compile without errors (VERIFIED)</constraint>
    <constraint>Public types accessible from crate root</constraint>
    <constraint>Cross-crate dependencies work</constraint>
    <constraint>cargo doc generates valid documentation</constraint>
    <constraint>NO BACKWARDS COMPATIBILITY - fail fast if types missing</constraint>
    <constraint>NO MOCK DATA in verification - use real type imports</constraint>
  </constraints>
</definition_of_done>

<full_state_verification>
  <execute_and_inspect>
    <step order="1">
      <action>Verify core crate exports TeleologicalArray types</action>
      <command>rg "^pub use teleological::" crates/context-graph-core/src/lib.rs</command>
      <expected_output>
        pub use teleological::{
            DomainAlignments, DomainType, Embedder, EmbedderDims, EmbedderGroup, EmbedderMask,
            GroupAlignments, GroupType, MultiResolutionHierarchy, ProfileId, ProfileMetrics, SynergyMatrix,
            TaskType, TeleologicalProfile, TeleologicalVector, TuckerCore,
        };
      </expected_output>
      <fail_if>No output or missing key types</fail_if>
    </step>

    <step order="2">
      <action>Verify purpose module exports GoalNode and GoalLevel</action>
      <command>rg "^pub use goals::" crates/context-graph-core/src/purpose/mod.rs</command>
      <expected_output>
        pub use goals::{
            DiscoveryMethod, GoalDiscoveryMetadata, GoalHierarchy, GoalHierarchyError, GoalLevel,
            GoalNode, GoalNodeError,
        };
      </expected_output>
      <fail_if>GoalNode or GoalLevel missing</fail_if>
    </step>

    <step order="3">
      <action>Check if GoalNode/GoalLevel exported from core lib.rs root</action>
      <command>rg "pub use purpose::" crates/context-graph-core/src/lib.rs</command>
      <expected_before>NO OUTPUT (not currently re-exported)</expected_before>
      <expected_after>pub use purpose::{GoalNode, GoalLevel};</expected_after>
      <fail_if>After edit, still no output</fail_if>
    </step>

    <step order="4">
      <action>Verify storage crate exports RocksDbTeleologicalStore</action>
      <command>rg "RocksDbTeleologicalStore" crates/context-graph-storage/src/lib.rs | head -5</command>
      <expected_output>Contains RocksDbTeleologicalStore export</expected_output>
      <fail_if>RocksDbTeleologicalStore not found</fail_if>
    </step>

    <step order="5">
      <action>Verify cross-crate dependency (storage imports from core)</action>
      <command>rg "use context_graph_core::" crates/context-graph-storage/src/lib.rs</command>
      <expected_output>
        pub use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
        pub use context_graph_core::types::{...};
      </expected_output>
      <fail_if>No cross-crate imports</fail_if>
    </step>

    <step order="6">
      <action>Run cargo check for all crates</action>
      <command>cargo check --workspace 2>&amp;1 | tail -5</command>
      <expected_output>Finished `dev` profile</expected_output>
      <fail_if>Contains "error" or compilation failure</fail_if>
    </step>

    <step order="7">
      <action>Run cargo doc to verify documentation builds</action>
      <command>cargo doc --workspace --no-deps 2>&amp;1 | tail -10</command>
      <expected_output>Finished `dev` profile or similar success message</expected_output>
      <fail_if>Contains "error" or documentation failure</fail_if>
    </step>

    <step order="8">
      <action>Run all workspace tests</action>
      <command>cargo test --workspace 2>&amp;1 | grep "test result"</command>
      <expected_output>test result: ok. N passed; 0 failed</expected_output>
      <fail_if>Any failed tests</fail_if>
    </step>
  </execute_and_inspect>
</full_state_verification>

<edge_cases>
  <edge_case id="EC1" priority="HIGH">
    <scenario>GoalNode/GoalLevel re-export causes name collision</scenario>
    <before_state>
      // No purpose re-exports at lib.rs root
      // Users import: use context_graph_core::purpose::{GoalNode, GoalLevel};
    </before_state>
    <after_state>
      // Added to lib.rs:
      pub use purpose::{GoalNode, GoalLevel};
      // Users can now import: use context_graph_core::{GoalNode, GoalLevel};
    </after_state>
    <verification>
      cargo check --workspace succeeds after adding re-exports
    </verification>
    <risk>Low - adding re-exports shouldn't break existing imports</risk>
    <mitigation>
      If collision occurs, check for duplicate type names in crate root.
      Remove conflicting export or rename at definition site.
    </mitigation>
  </edge_case>

  <edge_case id="EC2" priority="MEDIUM">
    <scenario>Documentation generation fails for specific types</scenario>
    <before_state>
      // cargo doc completes but may have warnings
      // Some types may have invalid doc links
    </before_state>
    <after_state>
      // cargo doc --workspace --no-deps completes without errors
      // All public types documented
    </after_state>
    <verification>
      cargo doc --workspace --no-deps 2>&amp;1 | grep -c "error"
      Expected: 0
    </verification>
    <risk>Low - documentation warnings don't block compilation</risk>
    <mitigation>
      Fix broken intra-doc links if cargo doc fails.
      Use `#[doc(hidden)]` for internal-only types if needed.
    </mitigation>
  </edge_case>

  <edge_case id="EC3" priority="LOW">
    <scenario>New re-exports increase compile time significantly</scenario>
    <before_state>
      // cargo check time: ~6 seconds
    </before_state>
    <after_state>
      // cargo check time should remain ~6 seconds
      // Adding 2 re-exports has negligible impact
    </after_state>
    <verification>
      time cargo check --workspace
      Should complete in similar time to baseline
    </verification>
    <risk>Very low - adding 2 symbol re-exports has no measurable impact</risk>
    <mitigation>
      If compile time increases significantly, check for accidental
      recursive re-exports or circular dependencies.
    </mitigation>
  </edge_case>
</edge_cases>

<evidence_of_success>
  <log id="LOG1" phase="VERIFICATION">
    <command>rg "^pub mod" crates/context-graph-core/src/lib.rs | wc -l</command>
    <expected_output>20+ (many modules declared)</expected_output>
    <description>Confirms all modules declared at crate root</description>
  </log>

  <log id="LOG2" phase="VERIFICATION">
    <command>rg "TeleologicalArray|TeleologicalVector" crates/context-graph-core/src/lib.rs</command>
    <expected_output>At least one match showing re-export</expected_output>
    <description>Confirms teleological types accessible from crate root</description>
  </log>

  <log id="LOG3" phase="AFTER">
    <command>rg "pub use purpose::" crates/context-graph-core/src/lib.rs</command>
    <expected_output>pub use purpose::{GoalNode, GoalLevel};</expected_output>
    <description>Confirms purpose types re-exported after edit</description>
  </log>

  <log id="LOG4" phase="AFTER">
    <command>cargo test --workspace 2>&amp;1 | grep -E "^test result:"</command>
    <expected_output>test result: ok. N passed; 0 failed; 0 ignored</expected_output>
    <description>All tests pass after integration</description>
  </log>

  <log id="LOG5" phase="AFTER">
    <command>cargo doc --workspace --no-deps 2>&amp;1 | grep -E "(Finished|error)"</command>
    <expected_output>Finished `dev` profile or similar (no errors)</expected_output>
    <description>Documentation generation succeeds</description>
  </log>
</evidence_of_success>

<implementation_steps>
  <step order="1">
    <action>Verify current state of lib.rs exports</action>
    <command>rg "^pub use" crates/context-graph-core/src/lib.rs</command>
    <details>Check what's already exported from crate root</details>
  </step>

  <step order="2">
    <action>Check if GoalNode/GoalLevel already re-exported</action>
    <command>rg "GoalNode|GoalLevel" crates/context-graph-core/src/lib.rs</command>
    <expected>No output (not currently re-exported from root)</expected>
  </step>

  <step order="3">
    <action>Add GoalNode and GoalLevel re-exports to lib.rs</action>
    <file>crates/context-graph-core/src/lib.rs</file>
    <edit>
      After line 73 (after teleological re-exports), add:

      // Purpose module re-exports (goal hierarchy types)
      pub use purpose::{GoalNode, GoalLevel};
    </edit>
  </step>

  <step order="4">
    <action>Verify compilation after edit</action>
    <command>cargo check --workspace</command>
    <expected>Finished `dev` profile (no errors)</expected>
  </step>

  <step order="5">
    <action>Verify documentation generation</action>
    <command>cargo doc --workspace --no-deps</command>
    <expected>Documentation builds successfully</expected>
  </step>

  <step order="6">
    <action>Run all tests to confirm no regressions</action>
    <command>cargo test --workspace</command>
    <expected>All tests pass</expected>
  </step>

  <step order="7">
    <action>Verify cross-crate access works</action>
    <command>rg "use context_graph_core::" --type rust | head -10</command>
    <expected>Multiple files importing from context_graph_core</expected>
  </step>
</implementation_steps>

<what_NOT_to_do>
  <item>DO NOT create teleological/mod.rs - it already exists (114 lines)</item>
  <item>DO NOT create purpose/mod.rs - it already exists (76 lines)</item>
  <item>DO NOT create storage teleological/mod.rs - it already exists (113 lines)</item>
  <item>DO NOT modify existing module implementations</item>
  <item>DO NOT add backwards compatibility shims</item>
  <item>DO NOT add placeholder functions or todo!() macros</item>
  <item>DO NOT modify Cargo.toml dependencies (already correct)</item>
  <item>DO NOT touch test files unless test fails</item>
</what_NOT_to_do>

<constitution_compliance>
  <rule id="ARCH-02" status="VERIFIED_COMPLIANT">
    <text>Apples-to-apples comparison - Same embedder to same embedder only</text>
    <evidence>
      GoalNode uses TeleologicalArray per purpose/goals.rs
      compute_all_space_alignments in calculator.rs uses per-embedder comparison
    </evidence>
  </rule>

  <rule id="ARCH-05" status="VERIFIED_COMPLIANT">
    <text>All 13 embedders must be present in teleological_array</text>
    <evidence>
      TeleologicalVector has 13 embedders per teleological/vector.rs
      Embedder enum has 13 variants per teleological/embedder.rs
    </evidence>
  </rule>

  <rule id="AP-07" status="ENFORCED">
    <text>Stubs are test only</text>
    <evidence>
      lib.rs line 65-66: #[cfg(test)] guards stub imports
    </evidence>
  </rule>
</constitution_compliance>

<files_to_modify>
  <file path="crates/context-graph-core/src/lib.rs">
    <change type="ADD">
      Add after line 73:
      // Purpose module re-exports (goal hierarchy types)
      pub use purpose::{GoalNode, GoalLevel};
    </change>
  </file>
</files_to_modify>

<files_to_create>
  <!-- NONE - all modules already exist -->
</files_to_create>

<validation_commands>
  <command purpose="verify_compilation">cargo check --workspace</command>
  <command purpose="verify_tests">cargo test --workspace</command>
  <command purpose="verify_docs">cargo doc --workspace --no-deps</command>
  <command purpose="verify_purpose_export">rg "pub use purpose::" crates/context-graph-core/src/lib.rs</command>
  <command purpose="verify_no_new_warnings">cargo clippy --workspace -- -D warnings 2>&amp;1 | grep -c "error"</command>
</validation_commands>

<rollback_plan>
  If adding re-exports causes compilation errors:
  1. git checkout crates/context-graph-core/src/lib.rs
  2. Investigate name collision with: rg "GoalNode|GoalLevel" --type rust
  3. Check for circular dependencies if compilation fails
</rollback_plan>

<research_recommendations>
  <recommendation source="rust-api-guidelines">
    Use `pub use` sparingly at crate root - only for primary types.
    GoalNode and GoalLevel are primary user-facing types, so re-export is appropriate.
    Reference: https://rust-lang.github.io/api-guidelines/
  </recommendation>

  <recommendation source="cargo-best-practices">
    Run `cargo doc --open` locally to verify documentation links work.
    Broken intra-doc links are a common issue with re-exports.
  </recommendation>
</research_recommendations>
</task_spec>
```
