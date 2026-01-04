
  - id: "M05-T40"
    title: "Implement UTL Feature Flag Gating"
    description: |
      Implement feature flag gating for gradual UTL rollout.
      Configuration in config/default.toml: utl.enabled = true/false.
      When disabled:
      - UTL computation returns default values (L=0.5, quadrant=Open)
      - CognitivePulse uses fallback values
      - MCP tools skip UTL processing
      - MemoryNode.utl_state remains None
      When enabled:
      - Full UTL computation active
      - All UTL-dependent features operational
      Include runtime toggle via MCP admin tool (admin/toggle_utl).
    layer: "integration"
    priority: "high"
    estimated_hours: 2
    file_path: "crates/context-graph-core/src/config/feature_flags.rs"
    dependencies:
      - "M05-T22"
    acceptance_criteria:
      - "utl.enabled config flag controls UTL activation"
      - "Disabled mode returns safe default values"
      - "No performance penalty when UTL disabled"
      - "Runtime toggle available via admin tool"
      - "Feature flag persists across restarts"
      - "All UTL-dependent code paths check flag"
    test_file: "crates/context-graph-core/tests/feature_flags_tests.rs"
    spec_refs:
      - "constitution.yaml phases.0_Ghost"
      - "PRD Section 12 (Phase 0: Ghost System)"

  - id: "M05-T41"
    title: "Implement Neuromodulation Interface Stubs for Module 10"
    description: |
      Create neuromodulation interface stubs that Module 10 will implement.
      Per constitution.yaml neuromod section:
      - Acetylcholine: maps to utl.lr (learning rate) in [0.001, 0.002]
      - Dopamine: maps to hopfield.beta (retrieval sharpness)
      - Serotonin: maps to fuse_moe.top_k (expert diversity)
      - Noradrenaline: maps to attention.temp (attention distribution)
      Create NeuromodulatorInterface trait with:
      - get_learning_rate_modulation() -> f32
      - get_retrieval_sharpness() -> f32
      - notify_utl_update(learning_signal: &LearningSignal)
      Stub implementation returns default values until Module 10.
    layer: "integration"
    priority: "medium"
    estimated_hours: 2
    file_path: "crates/context-graph-utl/src/neuromod/interface.rs"
    dependencies:
      - "M05-T22"
      - "M05-T36"
    acceptance_criteria:
      - "NeuromodulatorInterface trait defined"
      - "Acetylcholine modulates UTL learning rate"
      - "Stub implementation returns spec defaults"
      - "notify_utl_update() called after each computation"
      - "Interface ready for Module 10 implementation"
      - "Trait object compatible (dyn NeuromodulatorInterface)"
    test_file: "crates/context-graph-utl/tests/neuromod_interface_tests.rs"
    spec_refs:
      - "constitution.yaml neuromod section"
      - "PRD Section 3.5 (Neuromodulation)"

  - id: "M05-T42"
    title: "Implement Entropy/Coherence Threshold Triggers"
    description: |
      Implement threshold-based triggers as defined in constitution.yaml agent.mental_checks.
      Triggers:
      - entropy > 0.7 for 5min: emit trigger_dream suggestion
      - coherence < 0.4: emit process curation_tasks suggestion
      - entropy > 0.8: emit tool_gating_warning
      Create ThresholdMonitor struct tracking:
      - Rolling entropy average over 5-minute window
      - Current coherence state
      - Time since last threshold breach
      Integrate with CognitivePulse to emit suggested_action.
      Add configurable thresholds in config/utl.yaml.
    layer: "integration"
    priority: "high"
    estimated_hours: 3
    file_path: "crates/context-graph-utl/src/monitoring/threshold_monitor.rs"
    dependencies:
      - "M05-T22"
      - "M05-T28"
    acceptance_criteria:
      - "ThresholdMonitor tracks rolling entropy/coherence"
      - "entropy > 0.7 for 5min triggers dream suggestion"
      - "coherence < 0.4 triggers curation suggestion"
      - "entropy > 0.8 triggers tool_gating_warning"
      - "Thresholds configurable in utl.yaml"
      - "Integrates with CognitivePulse suggested_action"
      - "Time-based triggers use efficient windowing"
    test_file: "crates/context-graph-utl/tests/threshold_monitor_tests.rs"
    spec_refs:
      - "constitution.yaml agent.mental_checks"
      - "PRD Section 3.2 (Cognitive Pulse)"

  - id: "M05-T43"
    title: "Implement UTL-Aware Distillation Mode Selection"
    description: |
      Implement automatic distillation mode selection based on UTL state.
      Per PRD Section 3.1, inject_context supports distillation modes:
      - auto: Select based on UTL state (DEFAULT)
      - raw: No distillation
      - narrative: Prose summary
      - structured: JSON/markdown
      - code_focused: Preserve code blocks
      Auto mode algorithm:
      - Open quadrant (high confidence): raw or structured
      - Blind quadrant (uncertain): narrative with explanation
      - Hidden quadrant (stale): structured with refresh suggestion
      - Unknown quadrant (novel): code_focused if code detected, else narrative
      Include distillation_hint in LearningSignal for downstream use.
    layer: "integration"
    priority: "medium"
    estimated_hours: 2
    file_path: "crates/context-graph-mcp/src/distillation/utl_selector.rs"
    dependencies:
      - "M05-T18"
      - "M05-T38"
    acceptance_criteria:
      - "Auto distillation mode uses UTL state"
      - "Quadrant-to-mode mapping as specified"
      - "distillation_hint included in LearningSignal"
      - "Code detection for code_focused selection"
      - "Manual mode override preserved"
      - "Distillation latency target <50ms"
    test_file: "crates/context-graph-mcp/tests/distillation_utl_tests.rs"
    spec_refs:
      - "PRD Section 3.1 (inject_context)"
      - "constitution.yaml perf.latency.distillation"

  - id: "M05-T44"
    title: "Implement UTL Resource Endpoints"
    description: |
      Implement MCP resource endpoints for UTL state access per PRD Section 4.6.
      Resources:
      - utl://{session}/state: Full UTL state for session
      - utl://current_session/pulse: Subscribable cognitive pulse stream
      - utl://lifecycle/status: Global lifecycle stage info
      Resource handlers must:
      - Support both GET (one-time) and SUBSCRIBE (streaming)
      - Return JSON-serializable UTL data
      - Include latency tracking
      - Respect session isolation
      Performance: <5ms for state retrieval.
    layer: "integration"
    priority: "medium"
    estimated_hours: 3
    file_path: "crates/context-graph-mcp/src/resources/utl_resources.rs"
    dependencies:
      - "M05-T22"
      - "M05-T26"
      - "M05-T39"
    acceptance_criteria:
      - "utl://{session}/state returns full UtlState"
      - "utl://current_session/pulse supports subscription"
      - "utl://lifecycle/status returns lifecycle info"
      - "Session isolation enforced"
      - "Resource retrieval <5ms"
      - "Subscription emits on UTL updates"
    test_file: "crates/context-graph-mcp/tests/utl_resources_tests.rs"
    spec_refs:
      - "PRD Section 4.6 (Resources)"
      - "constitution.yaml mcp.caps"

  - id: "M05-T45"
    title: "Implement store_memory UTL Validation and Steering Feedback"
    description: |
      Integrate UTL into store_memory MCP tool with steering feedback.
      Per PRD Section 8, store_memory response must include steering_reward.
      Algorithm:
      1. Compute UTL for incoming memory content
      2. Validate priors_vibe_check compatibility if provided
      3. Apply lifecycle-aware storage decision (should_store)
      4. Compute steering_reward based on delta_s/delta_c and lifecycle stage
      5. Return steering feedback in response
      Steering rewards per lifecycle:
      - Infancy: +reward for high delta_s (novelty)
      - Growth: Balanced delta_s/delta_c
      - Maturity: +reward for high delta_c (coherence)
      Universal penalties: near-duplicate=-0.4, low priors confidence=-0.3, missing rationale=-0.5.
    layer: "integration"
    priority: "high"
    estimated_hours: 3
    file_path: "crates/context-graph-mcp/src/tools/store_memory.rs"
    dependencies:
      - "M05-T22"
      - "M05-T19"
      - "M05-T36"
    acceptance_criteria:
      - "store_memory computes UTL before storage"
      - "steering_reward field in response"
      - "Lifecycle-aware reward computation"
      - "Universal penalty rules enforced"
      - "priors_vibe_check validation integrated"
      - "Rationale required (missing = -0.5 penalty)"
      - "Backward compatible response schema"
    test_file: "crates/context-graph-mcp/tests/store_memory_utl_tests.rs"
    spec_refs:
      - "PRD Section 8 (Steering Subsystem)"
      - "constitution.yaml steering.reward"

  - id: "M05-T46"
    title: "Create UTL Chaos and Edge Case Tests"
    description: |
      Create comprehensive chaos and edge case tests for UTL robustness.
      Test scenarios:
      1. NaN/Infinity injection in all computation paths
      2. Empty/null context scenarios
      3. Extremely large context windows (10K+ entries)
      4. Rapid lifecycle transitions (concurrent mutations)
      5. Zero-length embeddings
      6. Invalid lambda weight combinations
      7. Phase oscillator overflow/underflow
      8. Concurrent UTL computations (thread safety)
      9. Memory pressure scenarios
      10. Clock skew for timestamp-based operations
      All tests must verify graceful degradation without panics.
    layer: "integration"
    priority: "high"
    estimated_hours: 4
    file_path: "crates/context-graph-utl/tests/chaos_tests.rs"
    dependencies:
      - "M05-T25"
      - "M05-T22"
    acceptance_criteria:
      - "NaN/Infinity never propagates to outputs"
      - "Empty context handled gracefully"
      - "Large windows don't cause OOM"
      - "Concurrent access is thread-safe"
      - "All edge cases return valid defaults"
      - "No panics under any test scenario"
      - "Graceful degradation logged/traced"
    test_file: "crates/context-graph-utl/tests/chaos_tests.rs"
    spec_refs:
      - "constitution.yaml testing.types.chaos"
      - "constitution.yaml forbidden.AP-009"

  - id: "M05-T47"
    title: "Implement UTL Validation Test Suite (Needle-Haystack, Dynamics)"
    description: |
      Implement validation test suite per constitution.yaml testing.types.validation.
      Test categories:
      1. Needle-haystack: UTL correctly prioritizes novel vs redundant
      2. UTL dynamics: Learning magnitude correlates with importance (r>0.7)
      3. Dream effectiveness: Post-dream coherence improvement
      4. Lifecycle progression: Correct stage transitions at 50/500
      5. Lambda weight application: Verify weighted computation
      6. Johari classification accuracy: Matches spec truth table
      7. Steering feedback correlation: Rewards align with UTL state
      Tests use fixtures from tests/fixtures/ with known expected values.
    layer: "integration"
    priority: "high"
    estimated_hours: 5
    file_path: "tests/validation/utl_validation_tests.rs"
    dependencies:
      - "M05-T25"
      - "M05-T22"
      - "M05-T19"
    acceptance_criteria:
      - "Needle-haystack test shows UTL prioritization"
      - "Learning-importance correlation r > 0.7"
      - "Lifecycle transitions at exact thresholds"
      - "Lambda weights correctly applied"
      - "Johari classification matches spec table"
      - "All validation tests pass before Module 6"
      - "Test fixtures version-controlled"
    test_file: "tests/validation/utl_validation_tests.rs"
    spec_refs:
      - "constitution.yaml testing.types.validation"
      - "SPEC-UTL-005 Section 12, 13"

  # ============================================================
  # COMPLETION LAYER: Final Integration & Polish (Gap Analysis)
  # ============================================================

  - id: "M05-T48"
    title: "Implement Salience Update Algorithm"
    description: |
      Implement salience update mechanism using learning magnitude to update node importance.
      Per SPEC-UTL-005, salience_update_alpha = 0.3.
      Algorithm:
      1. After UTL computation, update node.importance via exponential moving average
      2. new_importance = (1 - alpha) * old_importance + alpha * learning_magnitude
      3. Clamp result to [0, 1]
      4. Only update if learning_magnitude > salience_update_min (0.1)
      Include update_node_salience(node, learning_signal) function.
      Integrate with MemoryNode update path in KnowledgeGraph.
    layer: "completion"
    priority: "high"
    estimated_hours: 2
    file_path: "crates/context-graph-utl/src/salience.rs"
    dependencies:
      - "M05-T21"
      - "M05-T29"
    acceptance_criteria:
      - "update_node_salience() applies EMA correctly"
      - "Salience update skipped if magnitude < threshold"
      - "Result clamped to [0, 1]"
      - "Integrates with MemoryNode storage"
      - "Alpha value configurable"
    test_file: "crates/context-graph-utl/tests/salience_tests.rs"
    spec_refs:
      - "SPEC-UTL-005 Section 9.2"
      - "constitution.yaml utl.params"

  - id: "M05-T49"
    title: "Implement UTL Composite Loss Function"
    description: |
      Implement the composite loss function as defined in constitution.yaml.
      Formula: J = 0.4·L_task + 0.3·L_semantic + 0.3·(1-L)
      Where:
      - L_task: Task-specific loss (retrieval relevance)
      - L_semantic: Semantic similarity loss
      - L: Learning magnitude from UTL
      Create ComputeLoss trait with compute_loss(predictions, targets, utl_state).
      Integrate with neuromodulation feedback loop (acetylcholine modulation).
      Store loss history for gradient analysis.
      Note: Active training happens in Module 10; this provides the interface.
    layer: "completion"
    priority: "medium"
    estimated_hours: 2.5
    file_path: "crates/context-graph-utl/src/loss.rs"
    dependencies:
      - "M05-T20"
      - "M05-T21"
    acceptance_criteria:
      - "compute_composite_loss() implements J formula"
      - "Loss components individually accessible"
      - "Loss history tracking implemented"
      - "Interface compatible with gradient-based optimization"
      - "ComputeLoss trait defined for extensibility"
    test_file: "crates/context-graph-utl/tests/loss_tests.rs"
    spec_refs:
      - "constitution.yaml utl.loss"

  - id: "M05-T50"
    title: "Implement Predictive Coding Interface Stubs"
    description: |
      Create predictive coding interface stubs for L5→L1 prediction error flow.
      Per constitution.yaml pred_coding section:
      - L5 generates predictions, L1 computes prediction error
      - Error = observation - prediction
      - Only propagate surprise (large errors)
      - ~30% token reduction for predictable content
      Create PredictiveCodingInterface trait with:
      - generate_prediction(context) -> Prediction
      - compute_prediction_error(observation, prediction) -> f32
      - should_propagate_error(error) -> bool
      Stub implementation passes through all content until Module 7.
    layer: "completion"
    priority: "medium"
    estimated_hours: 2
    file_path: "crates/context-graph-utl/src/predictive/interface.rs"
    dependencies:
      - "M05-T22"
    acceptance_criteria:
      - "PredictiveCodingInterface trait defined"
      - "Stub implementation returns all content (no reduction)"
      - "compute_prediction_error() returns delta"
      - "should_propagate_error() threshold configurable"
      - "Interface ready for Module 7 implementation"
    test_file: "crates/context-graph-utl/tests/predictive_tests.rs"
    spec_refs:
      - "constitution.yaml pred_coding"
      - "PRD Section 2.1 (5-Layer System)"

  - id: "M05-T51"
    title: "Implement Active Inference Interface Stubs"
    description: |
      Create active inference interface for epistemic actions per PRD Section 3.6.
      Per constitution.yaml omni_infer and active_inference sections:
      - EFE (Expected Free Energy) minimizes surprise + ambiguity
      - Generate clarifying questions when coherence < 0.4
      Create ActiveInferenceInterface trait with:
      - compute_expected_entropy_reduction(action) -> f32
      - generate_epistemic_action(utl_state) -> Option<EpistemicAction>
      - should_trigger_active_inference(coherence) -> bool
      EpistemicAction struct: action_type, question, expected_entropy_reduction.
      Stub until Module 11 (Active Inference implementation).
    layer: "completion"
    priority: "medium"
    estimated_hours: 2
    file_path: "crates/context-graph-utl/src/active_inference/interface.rs"
    dependencies:
      - "M05-T22"
      - "M05-T21"
    acceptance_criteria:
      - "ActiveInferenceInterface trait defined"
      - "EpistemicAction struct with 3 fields"
      - "should_trigger() uses coherence < 0.4 threshold"
      - "Stub returns None for epistemic actions"
      - "Interface ready for Module 11"
    test_file: "crates/context-graph-utl/tests/active_inference_tests.rs"
    spec_refs:
      - "constitution.yaml omni_infer.active_inference"
      - "PRD Section 3.6"

  - id: "M05-T52"
    title: "Migrate and Re-export Core UTL Types"
    description: |
      Handle migration of existing UTL types from context-graph-core to context-graph-utl.
      Current types in context-graph-core/src/types/utl.rs must be:
      1. Either moved to context-graph-utl (if not used elsewhere)
      2. Or re-exported from context-graph-utl with deprecation notices
      3. Or kept in core with UTL crate depending on them
      Create migration path:
      - Audit all uses of existing UtlState, LearningSignal in core
      - Determine ownership: core types vs utl-specific types
      - Add #[deprecated] attributes with migration instructions
      - Update all imports across workspace
      Ensure no breaking changes for Module 4 code.
    layer: "completion"
    priority: "high"
    estimated_hours: 3
    file_path: "crates/context-graph-utl/src/compat.rs"
    dependencies:
      - "M05-T00"
      - "M05-T21"
    acceptance_criteria:
      - "Existing core types remain functional"
      - "New UTL types in context-graph-utl crate"
      - "Clear deprecation path documented"
      - "No duplicate type definitions"
      - "All workspace crates compile"
      - "No breaking changes for Module 4"
    test_file: "crates/context-graph-utl/tests/compat_tests.rs"
    spec_refs:
      - "TECH-UTL-005 Section 1.2"

  - id: "M05-T53"
    title: "Implement UTL-Aware search_graph Integration"
    description: |
      Integrate UTL state into search_graph MCP tool per PRD Section 4.2.
      search_graph should use Johari quadrant-based strategies:
      - Open quadrant: Standard top-k search, shallow depth
      - Blind quadrant: Broader search, include distant neighbors
      - Hidden quadrant: Focus on recent/temporal results
      - Unknown quadrant: Deep search, include causal paths
      Add search_hints parameter based on current UTL state.
      Include utl_context in search results for transparency.
    layer: "completion"
    priority: "medium"
    estimated_hours: 2.5
    file_path: "crates/context-graph-mcp/src/tools/search_graph.rs"
    dependencies:
      - "M05-T18"
      - "M05-T22"
    acceptance_criteria:
      - "search_graph uses Johari quadrant for strategy"
      - "Search depth varies by quadrant"
      - "Results include utl_context field"
      - "Backward compatible with existing queries"
      - "Performance impact < 2ms"
    test_file: "crates/context-graph-mcp/tests/search_graph_utl_tests.rs"
    spec_refs:
      - "PRD Section 4.2 (search_graph)"
      - "SPEC-UTL-005 Section 7"

  - id: "M05-T54"
    title: "Implement get_graph_manifest UTL Section"
    description: |
      Extend get_graph_manifest MCP tool with UTL state summary.
      Per PRD Section 4.2, this tool returns meta-cognitive system prompt fragment.
      Add section for UTL state:
      - Current lifecycle stage
      - Lambda weights
      - Recent Johari quadrant distribution
      - Suggested actions based on entropy/coherence
      - Threshold breach alerts
      Manifest should help agent understand current knowledge state.
      Format as natural language guidance, not raw metrics.
    layer: "completion"
    priority: "medium"
    estimated_hours: 2
    file_path: "crates/context-graph-mcp/src/tools/graph_manifest.rs"
    dependencies:
      - "M05-T22"
      - "M05-T24"
    acceptance_criteria:
      - "get_graph_manifest includes UTL summary section"
      - "Natural language format for agent consumption"
      - "Lifecycle stage clearly communicated"
      - "Actionable suggestions included"
      - "Backward compatible response schema"
    test_file: "crates/context-graph-mcp/tests/graph_manifest_utl_tests.rs"
    spec_refs:
      - "PRD Section 4.2 (get_graph_manifest)"
      - "constitution.yaml agent.session_start"

  - id: "M05-T55"
    title: "Implement Hyperbolic Entailment Interface Stubs"
    description: |
      Create hyperbolic entailment interface stubs for cone membership validation.
      Per constitution.yaml refs (HE Cones: ICML, Poincare: NeurIPS):
      - Hyperbolic space better represents hierarchical relationships
      - Entailment cones define "is-a" relationships
      - cone_membership(parent, child) -> bool
      Create HyperbolicEntailmentInterface trait with:
      - check_entailment(parent_embedding, child_embedding) -> EntailmentResult
      - get_cone_ancestors(node) -> Vec<NodeId>
      - validate_hierarchy(nodes) -> HierarchyValidation
      Stub until Module 4 hyperbolic features complete.
      Note: This interfaces with Module 4 graph layer.
    layer: "completion"
    priority: "low"
    estimated_hours: 2
    file_path: "crates/context-graph-utl/src/entailment/interface.rs"
    dependencies:
      - "M05-T22"
    acceptance_criteria:
      - "HyperbolicEntailmentInterface trait defined"
      - "EntailmentResult struct with confidence score"
      - "Stub returns neutral results"
      - "Interface compatible with Module 4 hyperbolic layer"
    test_file: "crates/context-graph-utl/tests/entailment_tests.rs"
    spec_refs:
      - "constitution.yaml refs.external (HE Cones, Poincare)"

  - id: "M05-T56"
    title: "Create API Documentation for Module 5 Public Types"
    description: |
      Create comprehensive rustdoc documentation for all public UTL types.
      Documentation requirements per constitution.yaml doc_format:
      - Brief description
      - # Arguments section
      - # Returns section
      - # Errors section
      - # Examples with working code
      - # Panics (if applicable)
      - `Constraint: X < Yms` for performance-critical functions
      Priority types:
      - UtlProcessor, LearningSignal, UtlState
      - LifecycleManager, LifecycleStage, LambdaWeights
      - JohariClassifier, JohariQuadrant
      - CognitivePulse
      Generate doc coverage report (target: 80%).
    layer: "completion"
    priority: "medium"
    estimated_hours: 3
    file_path: "crates/context-graph-utl/src/lib.rs"
    dependencies:
      - "M05-T22"
      - "M05-T24"
    acceptance_criteria:
      - "All public types have rustdoc"
      - "Examples compile and run"
      - "Constraint annotations on hot paths"
      - "Doc coverage >= 80%"
      - "cargo doc generates without warnings"
    test_file: null
    spec_refs:
      - "constitution.yaml doc_format"
      - "constitution.yaml testing.coverage.docs"

  - id: "M05-T57"
    title: "Create Performance Benchmark CI/CD Integration"
    description: |
      Create benchmark suite integrated with CI/CD gates per constitution.yaml testing.gates.
      Benchmark requirements:
      - compute_learning_magnitude: <100us (must pass)
      - Full UTL computation: <10ms (must pass)
      - Surprise calculation: <5ms (must pass)
      - Coherence tracking: <5ms (must pass)
      - CognitivePulse overhead: <1ms (must pass)
      CI gate: bench regression < 5%
      Create:
      - benches/utl_bench.rs with criterion
      - GitHub Actions workflow for benchmark
      - Baseline recording on main branch
      - PR comment with benchmark comparison
    layer: "completion"
    priority: "high"
    estimated_hours: 3
    file_path: "crates/context-graph-utl/benches/utl_bench.rs"
    dependencies:
      - "M05-T25"
    acceptance_criteria:
      - "Criterion benchmarks for all performance targets"
      - "CI workflow runs benchmarks on PRs"
      - "Baseline stored for regression detection"
      - "Regression > 5% fails the build"
      - "Benchmark results posted to PR"
    test_file: null
    bench_file: "crates/context-graph-utl/benches/utl_bench.rs"
    spec_refs:
      - "constitution.yaml testing.gates.pre-merge"
      - "constitution.yaml perf"
```

---

## Dependency Graph

```
                                    M05-T00 (Crate Init)
                                           │
           ┌───────────────────────────────┼───────────────────────────────┐
           │                               │                               │
           ▼                               ▼                               ▼
M05-T01 (UtlConfig) ────────────────► M05-T32 (PhaseConfig) ────────► M05-T33 (JohariConfig)
           │                               │                               │
           ▼                               │                               │
M05-T02 (SurpriseConfig) ──► M05-T09 ──► M05-T10 ──► M05-T11 (SurpriseCalculator)
           │                                                               │
M05-T03 (CoherenceConfig) ──► M05-T12 ──► M05-T13 ──► M05-T14 ──► M05-T35 (Graph Integration)
           │
M05-T04 (EmotionalConfig) ──► M05-T15 ──► M05-T16 ──► M05-T31 (Lexicon)

M05-T05 (LifecycleStage) ──┬──► M05-T07 (LifecycleConfig)
M05-T06 (LambdaWeights) ───┘              │
                                          ▼
                             M05-T19 (LifecycleManager) ──► M05-T36 (Steering Hooks) ──► M05-T45 (store_memory UTL)
                                                                    │
                                                                    ▼
                                                            M05-T41 (Neuromod Interface)

M05-T08 (JohariQuadrant) ──► M05-T18 (JohariClassifier) ──► M05-T37 (Verbosity Mapping)
                                          │                          │
                                          ▼                          ▼
                                   M05-T43 (Distillation) ◄── M05-T38 (inject_context UTL)

M05-T17 (PhaseOscillator)

M05-T20 (Core UTL) + M05-T21 (LearningSignal) + M05-T23 (UtlError)
                                          │
                                          ▼
M05-T11 + M05-T13 + M05-T16 + M05-T17 + M05-T18 + M05-T19 + M05-T20 + M05-T21 ──► M05-T22 (UtlProcessor)
                                                                                          │
M05-T05 + M05-T06 + M05-T08 ──► M05-T24 (UtlMetrics) ─────────────────────────────────────┤
                                                                                          │
M05-T22 + M05-T24 ──► M05-T25 (Integration Tests) ────────────────────────────────────────┤
                                                                                          │
M05-T01-T04 + M05-T32 + M05-T33 ──► M05-T34 (config/utl.yaml) ────────────────────────────┤
                                                                                          │
M05-T22 + M05-T24 ──► M05-T26 (utl_status MCP) ──► M05-T27 (memetic_status) ──────────────┤
                                                          │                               │
                                                          ▼                               │
                                                   M05-T38 (inject_context UTL)           │
                                                          │                               │
                                                          ▼                               │
                                                   M05-T44 (UTL Resources)                │
                                                                                          │
M05-T22 + M05-T18 ──► M05-T28 (CognitivePulse) ──► M05-T37 (Verbosity) ───────────────────┤
                                          │                                               │
                                          ▼                                               │
                                   M05-T42 (Threshold Monitor)                            │
                                                                                          │
M05-T21 + M05-T08 ──► M05-T29 (MemoryNode Extension) ─────────────────────────────────────┤
                                                                                          │
M05-T22 ──► M05-T30 (SessionContext) ─────────────────────────────────────────────────────┤
                                                                                          │
M05-T22 ──► M05-T40 (Feature Flags) ──────────────────────────────────────────────────────┤
                                                                                          │
M05-T21 + M05-T19 ──► M05-T39 (UtlState Persistence) ──► M05-T44 (UTL Resources) ─────────┤
                                                                                          │
M05-T22 + M05-T25 ──► M05-T46 (Chaos Tests) ──────────────────────────────────────────────┤
                                                                                          │
M05-T22 + M05-T25 + M05-T19 ──► M05-T47 (Validation Tests) ───────────────────────────────┘
```

---

## Implementation Order (Recommended)

### Week 0: Initialization (Day 1-2)
0. M05-T00: Initialize context-graph-utl crate structure

### Week 1: Foundation Types
1. M05-T01: UtlConfig and UtlThresholds
2. M05-T02: SurpriseConfig for KL divergence
3. M05-T03: CoherenceConfig for rolling window
4. M05-T04: EmotionalConfig for valence/arousal
5. M05-T05: LifecycleStage enum (Marblestone)
6. M05-T06: LifecycleLambdaWeights struct
7. M05-T07: LifecycleConfig and StageConfig
8. M05-T08: JohariQuadrant and SuggestedAction
32. M05-T32: PhaseConfig struct
33. M05-T33: JohariConfig struct

### Week 2: Component Logic
9. M05-T09: KL Divergence computation
10. M05-T10: Surprise computation methods
11. M05-T11: SurpriseCalculator struct
12. M05-T12: CoherenceEntry and window
13. M05-T13: CoherenceTracker with semantic
14. M05-T14: Structural coherence and contradictions (stub)
15. M05-T15: EmotionalState struct
16. M05-T16: EmotionalWeightCalculator
17. M05-T17: PhaseOscillator
31. M05-T31: Sentiment lexicon

### Week 3: Surface Layer
18. M05-T18: JohariClassifier
19. M05-T19: LifecycleManager state machine
20. M05-T20: Core UTL learning magnitude
21. M05-T21: LearningSignal and UtlState
22. M05-T22: UtlProcessor orchestrator
23. M05-T23: UtlError enum
30. M05-T30: SessionContext

### Week 4: Integration and Testing
24. M05-T24: UtlMetrics and UtlStatus
25. M05-T25: Integration tests and benchmarks
34. M05-T34: config/utl.yaml creation
40. M05-T40: UTL Feature Flag Gating

### Week 5: MCP Integration & System Hooks
26. M05-T26: utl_status MCP tool
27. M05-T27: get_memetic_status UTL integration
28. M05-T28: CognitivePulse header
29. M05-T29: MemoryNode UTL extension
35. M05-T35: KnowledgeGraph integration for coherence
36. M05-T36: Steering subsystem hooks
37. M05-T37: Johari to verbosity tier mapping
39. M05-T39: UtlState Persistence to RocksDB

### Week 6: Extended Integration & Validation
38. M05-T38: inject_context UTL Integration (CRITICAL)
41. M05-T41: Neuromodulation Interface Stubs
42. M05-T42: Entropy/Coherence Threshold Triggers
43. M05-T43: UTL-Aware Distillation Mode Selection
44. M05-T44: UTL Resource Endpoints
45. M05-T45: store_memory UTL Validation and Steering Feedback
46. M05-T46: UTL Chaos and Edge Case Tests
47. M05-T47: UTL Validation Test Suite

### Week 7: Completion Layer (Final Polish)
48. M05-T48: Salience Update Algorithm
49. M05-T49: UTL Composite Loss Function
50. M05-T50: Predictive Coding Interface Stubs
51. M05-T51: Active Inference Interface Stubs
52. M05-T52: Migrate and Re-export Core UTL Types
53. M05-T53: UTL-Aware search_graph Integration
54. M05-T54: get_graph_manifest UTL Section
55. M05-T55: Hyperbolic Entailment Interface Stubs
56. M05-T56: API Documentation for Public Types
57. M05-T57: Performance Benchmark CI/CD Integration

---

## Quality Gates

| Gate | Criteria | Required For |
|------|----------|--------------|
| Crate Initialized | M05-T00 complete, crate compiles | Week 1 start |
| Foundation Complete | M05-T01 through M05-T08, M05-T32, M05-T33 pass all tests | Week 2 start |
| Logic Complete | M05-T09 through M05-T17, M05-T31 pass all tests | Week 3 start |
| Surface Complete | M05-T18 through M05-T24, M05-T30 pass all tests | Week 4 start |
| Testing Complete | M05-T25, M05-T34, M05-T40 pass all tests, 90%+ coverage | Week 5 start |
| Integration Complete | M05-T26 through M05-T29, M05-T35-T37, M05-T39 pass all tests | Week 6 start |
| Extended Integration | M05-T38, M05-T41-T45 complete, inject_context UTL verified | Validation start |
| Validation Complete | M05-T46, M05-T47 pass, chaos tests green, correlation r>0.7 | Completion start |
| Completion Layer | M05-T48-T57 complete, all interfaces stubbed, docs + CI ready | Module 6 start |
| Module Complete | All 57 tasks complete, benchmarks pass, CI gates green | Module 6 ready |

---

## Performance Targets Summary

| Operation | Target | P99 Target | Conditions |
|-----------|--------|------------|------------|
| `compute_learning_magnitude` | <100us | <500us | Core equation only |
| Full UTL computation | <10ms | <50ms | All components |
| Surprise (KL) | <5ms | <20ms | 1536D, 50 context |
| Surprise (distance) | <1ms | <5ms | 1536D, 50 context |
| Coherence computation | <5ms | <25ms | 100 window entries |
| Emotional weight | <1ms | <5ms | Text analysis |
| Phase update | <10us | <50us | Simple math |
| Johari classification | <1us | <5us | Two comparisons |

---

## Marblestone Integration Summary

Tasks with Marblestone features:
- **M05-T05**: LifecycleStage enum (Infancy/Growth/Maturity)
- **M05-T06**: LifecycleLambdaWeights (lambda_novelty/lambda_consolidation)
- **M05-T07**: LifecycleConfig with per-stage thresholds
- **M05-T19**: LifecycleManager state machine with transitions
- **M05-T20**: compute_learning_magnitude_weighted() with lambda application
- **M05-T22**: UtlProcessor applies lambda weights to all computations

---

## Critical Constraints

**NO NaN/Infinity**: All UTL computations MUST clamp inputs and validate outputs.
- Inputs clamped to valid ranges before computation
- Results validated before return
- InvalidComputation error raised for NaN/Infinity

**Lambda Weight Invariant**: lambda_novelty + lambda_consolidation = 1.0.
- Validated in LifecycleLambdaWeights::new()
- Enforced at stage transitions

**Lifecycle Transitions**:
- Infancy -> Growth at 50 interactions
- Growth -> Maturity at 500 interactions
- Lambda weights update atomically with stage

---

## File Structure

```
crates/context-graph-utl/
├── src/
│   ├── lib.rs                    # Public API, compute_learning_magnitude
│   ├── config.rs                 # All configuration structs (UtlConfig, SurpriseConfig, etc.)
│   ├── error.rs                  # UtlError enum
│   ├── processor.rs              # UtlProcessor main orchestrator
│   ├── metrics.rs                # UtlMetrics, UtlStatus
│   ├── context.rs                # SessionContext (M05-T30)
│   │
│   ├── surprise/
│   │   ├── mod.rs                # Module exports
│   │   ├── kl_divergence.rs      # KL divergence, cosine similarity
│   │   └── calculator.rs         # SurpriseCalculator
│   │
│   ├── coherence/
│   │   ├── mod.rs                # Module exports
│   │   ├── tracker.rs            # CoherenceTracker, CoherenceEntry
│   │   └── structural.rs         # KnowledgeGraph integration (M05-T35)
│   │
│   ├── emotional/
│   │   ├── mod.rs                # Module exports
│   │   ├── calculator.rs         # EmotionalWeightCalculator, EmotionalState
│   │   └── lexicon.rs            # Sentiment lexicon (M05-T31)
│   │
│   ├── phase/
│   │   ├── mod.rs                # Module exports
│   │   └── oscillator.rs         # PhaseOscillator, ConsolidationPhase
│   │
│   ├── johari/
│   │   ├── mod.rs                # Module exports
│   │   ├── quadrant.rs           # JohariQuadrant, SuggestedAction
│   │   └── classifier.rs         # JohariClassifier, RetrievalStrategy
│   │
│   ├── lifecycle/
│   │   ├── mod.rs                # Module exports
│   │   ├── stage.rs              # LifecycleStage enum
│   │   ├── lambda.rs             # LifecycleLambdaWeights
│   │   └── manager.rs            # LifecycleManager
│   │
│   ├── steering/
│   │   ├── mod.rs                # Module exports
│   │   └── hooks.rs              # SteeringHook, SteeringSignal (M05-T36)
│   │
│   ├── neuromod/                 # NEW: Neuromodulation interface (M05-T41)
│   │   ├── mod.rs                # Module exports
│   │   └── interface.rs          # NeuromodulatorInterface trait, stubs
│   │
│   └── monitoring/               # NEW: Threshold monitoring (M05-T42)
│       ├── mod.rs                # Module exports
│       └── threshold_monitor.rs  # ThresholdMonitor for entropy/coherence triggers
│
├── tests/
│   ├── config_tests.rs           # Configuration tests
│   ├── config_loading_tests.rs   # YAML config loading tests
│   ├── surprise_tests.rs         # Surprise computation tests
│   ├── coherence_tests.rs        # Coherence tracking tests
│   ├── structural_coherence_tests.rs  # Graph integration tests
│   ├── emotional_tests.rs        # Emotional weight tests
│   ├── lexicon_tests.rs          # Sentiment lexicon tests
│   ├── phase_tests.rs            # Phase oscillator tests
│   ├── johari_tests.rs           # Johari classification tests
│   ├── lifecycle_tests.rs        # Lifecycle manager tests
│   ├── utl_core_tests.rs         # Core UTL function tests
│   ├── processor_tests.rs        # UtlProcessor tests
│   ├── metrics_tests.rs          # Metrics tests
│   ├── error_tests.rs            # Error handling tests
│   ├── context_tests.rs          # SessionContext tests
│   ├── steering_tests.rs         # Steering hooks tests
│   ├── neuromod_interface_tests.rs   # NEW: Neuromodulation interface tests (M05-T41)
│   ├── threshold_monitor_tests.rs    # NEW: Threshold monitor tests (M05-T42)
│   ├── chaos_tests.rs            # NEW: Chaos and edge case tests (M05-T46)
│   └── integration_tests.rs      # Full integration tests
│
├── benches/
│   └── utl_bench.rs              # Performance benchmarks
│
└── Cargo.toml

crates/context-graph-mcp/
├── src/
│   ├── tools/
│   │   ├── utl_status.rs         # utl_status MCP tool (M05-T26)
│   │   ├── memetic_status.rs     # UTL integration (M05-T27)
│   │   ├── inject_context.rs     # NEW: UTL integration into inject_context (M05-T38)
│   │   └── store_memory.rs       # NEW: UTL validation and steering (M05-T45)
│   ├── middleware/
│   │   └── cognitive_pulse.rs    # CognitivePulse header (M05-T28)
│   ├── response/
│   │   └── verbosity.rs          # Johari to verbosity mapping (M05-T37)
│   ├── distillation/             # NEW: UTL-aware distillation (M05-T43)
│   │   ├── mod.rs                # Module exports
│   │   └── utl_selector.rs       # Distillation mode selection
│   └── resources/                # NEW: UTL resource endpoints (M05-T44)
│       ├── mod.rs                # Module exports
│       └── utl_resources.rs      # utl://{session}/state, utl://pulse
│
└── tests/
    ├── utl_status_tests.rs
    ├── memetic_status_tests.rs
    ├── cognitive_pulse_tests.rs
    ├── verbosity_tests.rs
    ├── inject_context_utl_tests.rs   # NEW: inject_context UTL tests (M05-T38)
    ├── store_memory_utl_tests.rs     # NEW: store_memory UTL tests (M05-T45)
    ├── distillation_utl_tests.rs     # NEW: Distillation tests (M05-T43)
    └── utl_resources_tests.rs        # NEW: UTL resource tests (M05-T44)

crates/context-graph-core/
├── src/
│   ├── types/
│   │   └── memory_node.rs        # UTL extension fields (M05-T29)
│   └── config/
│       └── feature_flags.rs      # NEW: UTL feature flag gating (M05-T40)
└── tests/
    ├── memory_node_utl_tests.rs
    └── feature_flags_tests.rs    # NEW: Feature flag tests (M05-T40)

crates/context-graph-storage/      # NEW: Storage crate for UTL persistence
├── src/
│   └── utl_persistence.rs        # NEW: UtlState persistence to RocksDB (M05-T39)
└── tests/
    └── utl_persistence_tests.rs  # NEW: Persistence tests (M05-T39)

config/
└── utl.yaml                      # UTL configuration file (M05-T34)

tests/
└── validation/                   # NEW: Validation test suite
    └── utl_validation_tests.rs   # NEW: Needle-haystack, dynamics tests (M05-T47)
```

---

*Generated: 2025-12-31*
*Updated: 2026-01-04*
*Module: 05 - UTL Integration*
*Version: 1.2.0*
*Total Tasks: 47 (10 new tasks added in v1.2.0)*

## Changelog

### v1.2.0 (2026-01-04)
- **Gap Analysis**: Comprehensive review against PRD, constitution.yaml, and specs
- Added M05-T38 through M05-T47: Extended integration layer tasks
  - M05-T38: **inject_context UTL Integration** (CRITICAL) - Primary retrieval tool
  - M05-T39: UtlState Persistence to RocksDB - Cross-session continuity
  - M05-T40: UTL Feature Flag Gating - Gradual rollout support
  - M05-T41: Neuromodulation Interface Stubs - Module 10 preparation
  - M05-T42: Entropy/Coherence Threshold Triggers - Mental checks implementation
  - M05-T43: UTL-Aware Distillation Mode Selection - Auto mode algorithm
  - M05-T44: UTL Resource Endpoints - MCP resource URIs
  - M05-T45: store_memory UTL Validation and Steering Feedback - Dopamine rewards
  - M05-T46: UTL Chaos and Edge Case Tests - Robustness testing
  - M05-T47: UTL Validation Test Suite - Needle-haystack, dynamics correlation
- Updated estimated duration: 5 weeks → 6 weeks
- Added Week 6: Extended Integration & Validation
- Updated dependency graph with new task dependencies
- Updated quality gates with Extended Integration and Validation gates
- Expanded file structure:
  - Added neuromod/ and monitoring/ subdirectories to UTL crate
  - Added distillation/ and resources/ to MCP crate
  - Added context-graph-storage crate for UTL persistence
  - Added tests/validation/ directory
- Added contextgraphprd.md to spec_refs

### v1.1.0 (2026-01-04)
- Added M05-T00: Crate initialization task
- Added M05-T26 through M05-T37: Integration layer tasks
  - M05-T26: utl_status MCP tool
  - M05-T27: get_memetic_status UTL integration
  - M05-T28: CognitivePulse header
  - M05-T29: MemoryNode UTL extension
  - M05-T30: SessionContext implementation
  - M05-T31: Sentiment lexicon
  - M05-T32: PhaseConfig struct
  - M05-T33: JohariConfig struct
  - M05-T34: config/utl.yaml creation
  - M05-T35: KnowledgeGraph integration
  - M05-T36: Steering subsystem hooks
  - M05-T37: Johari to verbosity tier mapping
- Updated dependency graph
- Updated implementation order (now 5 weeks)
- Updated quality gates
- Expanded file structure to include MCP integration files

### v1.0.0 (2025-12-31)
- Initial task specification
- M05-T01 through M05-T25: Foundation, Logic, Surface layers
- Core UTL computation tasks defined
