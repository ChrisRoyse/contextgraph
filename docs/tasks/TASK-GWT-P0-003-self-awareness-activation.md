# TASK-GWT-P0-003: Self-Awareness Loop Activation

```xml
<task_spec id="TASK-GWT-P0-003" version="1.0">
<metadata>
  <title>Activate SelfAwarenessLoop in Production Code Paths</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>3</sequence>
  <implements>
    <item>Constitution v4.0.0 Section gwt.self_ego_node (lines 371-392)</item>
    <item>Self-awareness loop: Retrieve->A(action,PV)->if&lt;0.55 self_reflect->update fingerprint->store evolution</item>
    <item>Identity continuity: IC = cos(PV_t, PV_{t-1}) x r(t); healthy>0.9, warning<0.7, dream<0.5</item>
    <item>SHERLOCK-03 finding: SelfAwarenessLoop::cycle() defined but NEVER CALLED in production</item>
  </implements>
  <depends_on>
    <task_ref>TASK-GWT-P0-001</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
</metadata>

<context>
The SelfAwarenessLoop is fully implemented and tested but never invoked in production code paths.
As documented in SHERLOCK-03-SELF-EGO-NODE, the system has "anatomy but no physiology for self-awareness."
The loop exists in ego_node.rs with correct algorithms (alignment threshold 0.55, identity continuity
formula IC = cos(PV_t, PV_{t-1}) x r(t), status thresholds), but no production code calls cycle().
The purpose_vector remains [0.0; 13] forever because it is never updated from TeleologicalFingerprint
values. This task wires the existing loop into the action processing path.
</context>

<input_context_files>
  <file purpose="Contains SelfAwarenessLoop::cycle() implementation to be activated">
    /home/cabdru/contextgraph/crates/context-graph-core/src/gwt/ego_node.rs
  </file>
  <file purpose="GwtSystem orchestrator where self_ego_node lives">
    /home/cabdru/contextgraph/crates/context-graph-core/src/gwt/mod.rs
  </file>
  <file purpose="MCP handlers where fingerprints are processed (action entry point)">
    /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/teleological.rs
  </file>
  <file purpose="TeleologicalFingerprint with purpose_vector.alignments source">
    /home/cabdru/contextgraph/crates/context-graph-core/src/types/fingerprint/teleological/types.rs
  </file>
  <file purpose="PurposeVector struct with alignments: [f32; 13]">
    /home/cabdru/contextgraph/crates/context-graph-core/src/types/fingerprint/purpose.rs
  </file>
  <file purpose="SHERLOCK investigation documenting the gap">
    /home/cabdru/contextgraph/docs/sherlock-03-self-ego-node.md
  </file>
  <file purpose="Constitution specification for self_ego_node">
    /home/cabdru/contextgraph/docs2/constitution.yaml
  </file>
</input_context_files>

<prerequisites>
  <check>TASK-GWT-P0-001 (Kuramoto Integration) is complete - provides kuramoto_r parameter</check>
  <check>SelfAwarenessLoop struct exists in ego_node.rs with cycle() method</check>
  <check>SelfEgoNode struct exists with purpose_vector: [f32; 13] field</check>
  <check>TeleologicalFingerprint has purpose_vector.alignments: [f32; 13]</check>
  <check>GwtSystem has self_ego_node: Arc&lt;RwLock&lt;SelfEgoNode&gt;&gt;</check>
</prerequisites>

<scope>
  <in_scope>
    - Wire SelfAwarenessLoop::cycle() into action processing path (GwtSystem)
    - Add method to update SelfEgoNode.purpose_vector from TeleologicalFingerprint
    - Connect Critical identity status to dream trigger (actual invocation, not just comment)
    - Add write operations to self_ego_node in production code
    - Compute action_embedding from fingerprint for alignment check
    - Record purpose snapshots on each cycle
    - Unit tests for the new integration points
    - Integration test verifying loop executes on action processing
  </in_scope>
  <out_of_scope>
    - MCP tools for ego state updates (separate task)
    - Persistence layer for SelfEgoNode (separate task)
    - Dream controller implementation (assumed to exist or stubbed)
    - Kuramoto network integration (handled by TASK-GWT-P0-001)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/gwt/mod.rs">
      impl GwtSystem {
          /// Process an action through the self-awareness loop
          /// Updates purpose_vector from fingerprint, runs cycle(), triggers dream if Critical
          pub async fn process_action_awareness(
              &amp;self,
              fingerprint: &amp;TeleologicalFingerprint,
              kuramoto_r: f32,
          ) -> crate::CoreResult&lt;SelfReflectionResult&gt;
      }
    </signature>
    <signature file="crates/context-graph-core/src/gwt/ego_node.rs">
      impl SelfEgoNode {
          /// Update purpose_vector from a TeleologicalFingerprint's purpose alignments
          pub fn update_from_fingerprint(&amp;mut self, fingerprint: &amp;TeleologicalFingerprint)
      }
    </signature>
    <signature file="crates/context-graph-core/src/gwt/mod.rs">
      impl GwtSystem {
          /// Trigger dream consolidation when identity is Critical
          async fn trigger_identity_dream(&amp;self, reason: &amp;str) -> crate::CoreResult&lt;()&gt;
      }
    </signature>
  </signatures>

  <constraints>
    - SelfAwarenessLoop::cycle() MUST be called on every action that produces a fingerprint
    - purpose_vector MUST be updated from fingerprint.purpose_vector.alignments before cycle()
    - Critical identity status (IC &lt; 0.5) MUST trigger dream (not just log)
    - alignment_threshold MUST remain 0.55 per constitution
    - Identity continuity formula IC = cos(PV_t, PV_{t-1}) x r(t) MUST be preserved
    - Must not block the main action processing path (async)
    - Must handle concurrent access to self_ego_node via RwLock
    - Must record purpose snapshot after each cycle
  </constraints>

  <verification>
    - cargo test -p context-graph-core gwt::tests::test_process_action_awareness
    - cargo test -p context-graph-core gwt::tests::test_purpose_vector_updates_from_fingerprint
    - cargo test -p context-graph-core gwt::tests::test_critical_identity_triggers_dream
    - cargo test --test gwt_integration test_self_awareness_loop_production_invocation
    - Grep confirms no remaining "cycle() defined but never called" pattern
    - purpose_vector changes from [0.0; 13] after first fingerprint processed
  </verification>
</definition_of_done>

<pseudo_code>
GwtSystem::process_action_awareness (crates/context-graph-core/src/gwt/mod.rs):
  1. Extract action_embedding from fingerprint.purpose_vector.alignments
  2. Get kuramoto_r from parameter (provided by caller who has Kuramoto integration)
  3. Acquire write lock on self_ego_node
  4. Call self_ego_node.update_from_fingerprint(fingerprint)
  5. Create SelfAwarenessLoop instance (or use persistent one)
  6. Call loop.cycle(&amp;mut ego_node, &amp;action_embedding, kuramoto_r).await
  7. If result.identity_status == Critical:
       Call self.trigger_identity_dream("Identity coherence critical").await
  8. If result.needs_reflection:
       (Optional) trigger self-reflection subsystem or log warning
  9. Return SelfReflectionResult

SelfEgoNode::update_from_fingerprint (crates/context-graph-core/src/gwt/ego_node.rs):
  1. Copy fingerprint.purpose_vector.alignments to self.purpose_vector
  2. Update self.fingerprint = Some(fingerprint.clone())
  3. Update self.last_updated = Utc::now()
  4. Update coherence_with_actions from fingerprint.purpose_vector.coherence

GwtSystem::trigger_identity_dream (crates/context-graph-core/src/gwt/mod.rs):
  1. Log warning about Critical identity state
  2. If dream_controller is available:
       Call dream_controller.trigger_dream_with_reason(reason)
  3. Else:
       Log that dream would be triggered (graceful degradation)
  4. Record purpose snapshot with "Dream triggered: {reason}"
</pseudo_code>

<files_to_modify>
  <file path="crates/context-graph-core/src/gwt/mod.rs">
    Add process_action_awareness() method to GwtSystem
    Add trigger_identity_dream() method to GwtSystem
    Add use statement for TeleologicalFingerprint
    Add use statement for SelfReflectionResult
    Add SelfAwarenessLoop field to GwtSystem (persistent loop instance)
  </file>
  <file path="crates/context-graph-core/src/gwt/ego_node.rs">
    Add update_from_fingerprint() method to SelfEgoNode
    Add use statement for TeleologicalFingerprint if not present
  </file>
</files_to_modify>

<files_to_create>
  <file path="crates/context-graph-core/src/gwt/tests/action_awareness_test.rs">
    Unit tests for process_action_awareness
    Unit tests for update_from_fingerprint
    Unit tests for Critical identity dream trigger
  </file>
</files_to_create>

<validation_criteria>
  <criterion>SelfAwarenessLoop::cycle() is called in production code path (not just tests)</criterion>
  <criterion>purpose_vector updates from TeleologicalFingerprint.purpose_vector.alignments</criterion>
  <criterion>Critical identity status (IC &lt; 0.5) triggers dream consolidation</criterion>
  <criterion>identity_trajectory grows with purpose snapshots after cycles</criterion>
  <criterion>coherence_with_actions is non-zero after first fingerprint</criterion>
  <criterion>All existing tests continue to pass</criterion>
  <criterion>New tests verify production invocation</criterion>
  <criterion>SHERLOCK-03 gaps GAP 1 (loop never invoked) and GAP 5 (dream disconnected) are addressed</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core gwt::</command>
  <command>cargo test --test gwt_integration</command>
  <command>cargo clippy -p context-graph-core -- -D warnings</command>
  <command>cargo doc -p context-graph-core --no-deps</command>
</test_commands>

<integration_points>
  <point>
    <location>MCP teleological handlers</location>
    <description>
      After compute_teleological_vector or store operations complete,
      call GwtSystem::process_action_awareness() with the resulting fingerprint.
      This wires the self-awareness loop into the main action path.
    </description>
  </point>
  <point>
    <location>Kuramoto network (TASK-GWT-P0-001)</location>
    <description>
      The kuramoto_r parameter comes from KuramotoNetwork::order_parameter().
      Ensure GwtSystem has access to Kuramoto state for the cycle() call.
    </description>
  </point>
  <point>
    <location>Dream controller</location>
    <description>
      When IdentityStatus::Critical is detected, invoke dream consolidation.
      If dream controller not yet implemented, log and gracefully degrade.
    </description>
  </point>
</integration_points>

<risks>
  <risk severity="medium">
    <description>Performance impact of running cycle() on every action</description>
    <mitigation>Measure latency; cycle() is lightweight (cosine + update). If needed, batch or debounce.</mitigation>
  </risk>
  <risk severity="low">
    <description>RwLock contention on self_ego_node during concurrent actions</description>
    <mitigation>Write lock is brief (vector copy + snapshot). Use try_write with fallback if needed.</mitigation>
  </risk>
  <risk severity="low">
    <description>Dream controller may not be implemented</description>
    <mitigation>Graceful degradation: log warning, record snapshot, continue. Add TODO for full implementation.</mitigation>
  </risk>
</risks>

<acceptance_tests>
  <test name="test_loop_executes_on_fingerprint_creation">
    Given: A new TeleologicalFingerprint with purpose_vector.alignments = [0.8, 0.7, ...]
    When: process_action_awareness() is called
    Then: SelfAwarenessLoop::cycle() executes
    And: self_ego_node.purpose_vector equals fingerprint.purpose_vector.alignments
    And: identity_trajectory.len() increases by 1
  </test>
  <test name="test_critical_identity_triggers_dream">
    Given: self_ego_node with purpose_vector set
    And: Previous purpose_vector that differs significantly (cosine &lt; 0.5)
    And: kuramoto_r = 0.3 (so IC = cos * r &lt; 0.5)
    When: cycle() computes identity_coherence &lt; 0.5
    Then: IdentityStatus::Critical is returned
    And: trigger_identity_dream() is called
  </test>
  <test name="test_purpose_vector_no_longer_frozen">
    Given: Fresh SelfEgoNode with purpose_vector = [0.0; 13]
    When: update_from_fingerprint() called with real fingerprint
    Then: purpose_vector != [0.0; 13]
    And: coherence_with_actions > 0.0
    And: last_updated is recent
  </test>
  <test name="test_low_alignment_triggers_reflection_flag">
    Given: self_ego_node.purpose_vector = [1.0, 0.0, 0.0, ...]
    And: action_embedding = [0.0, 1.0, 0.0, ...] (orthogonal)
    When: cycle() computes alignment
    Then: alignment &lt; 0.55
    And: result.needs_reflection = true
  </test>
</acceptance_tests>
</task_spec>
```

---

## Summary

This task activates the dormant SelfAwarenessLoop by:

1. **Wiring cycle() into GwtSystem** - Adding `process_action_awareness()` method that calls `SelfAwarenessLoop::cycle()` on every fingerprint-producing action
2. **Updating purpose_vector from fingerprints** - New `update_from_fingerprint()` method copies `fingerprint.purpose_vector.alignments` to `self_ego_node.purpose_vector`
3. **Connecting dream trigger** - When `IdentityStatus::Critical` (IC < 0.5), actually invoke dream consolidation instead of just logging

## Dependencies

- **TASK-GWT-P0-001**: Provides `kuramoto_r` parameter needed for `cycle()` call

## Evidence from SHERLOCK-03

| Gap | Description | Resolution in This Task |
|-----|-------------|------------------------|
| GAP 1 | SelfAwarenessLoop::cycle() NEVER INVOKED | Wire into process_action_awareness() |
| GAP 2 | Ego Node NEVER WRITTEN TO | Add update_from_fingerprint() |
| GAP 5 | Dream Trigger DISCONNECTED | Add trigger_identity_dream() on Critical |

## Constitution Reference

From `docs2/constitution.yaml` lines 365-369:
```yaml
self_ego_node:
  id: "SELF_EGO_NODE"
  fields: [fingerprint, purpose_vector, identity_trajectory, coherence_with_actions]
  loop: "Retrieve->A(action,PV)->if<0.55 self_reflect->update fingerprint->store evolution"
  identity_continuity: "IC = cos(PV_t, PV_{t-1}) x r(t); healthy>0.9, warning<0.7, dream<0.5"
```

## Estimated Effort

- Implementation: 4-6 hours
- Testing: 2-3 hours
- Integration verification: 1-2 hours
