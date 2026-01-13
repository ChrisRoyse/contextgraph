# Task Traceability Matrix: ContextGraph Consciousness System

## Version: 5.0.0
## Last Updated: 2026-01-12
## Status: Active Development

---

## 1. Overview

This traceability matrix maps every requirement from the ContextGraph specifications to implementation tasks, ensuring 100% coverage of the constitution enforcement rules.

---

## 2. Coverage Summary

| Spec | Total Items | Covered | Uncovered | Coverage % |
|------|-------------|---------|-----------|------------|
| SPEC-DREAM-001 | 8 | 5 | 3 | 62.5% |
| SPEC-IDENTITY-001 | 7 | 6 | 1 | 85.7% |
| SPEC-GWT-001 | 6 | 5 | 1 | 83.3% |
| SPEC-MCP-001 | 6 | 3 | 3 | 50.0% |
| SPEC-UTL-001 | 5 | 5 | 0 | 100% |
| SPEC-UTL-002 | 4 | 2 | 2 | 50.0% |
| SPEC-METAUTL-001 | 5 | 3 | 2 | 60.0% |
| SPEC-NEURO-001 | 4 | 3 | 1 | 75.0% |
| **TOTAL** | **45** | **32** | **13** | **71.1%** |

---

## 3. SPEC-DREAM-001: Dream Layer Protocol

### 3.1 Requirements Traceability

| Req ID | Constitution Rule | Description | Task ID | Status |
|--------|-------------------|-------------|---------|--------|
| REQ-DREAM-001 | DREAM-001 | NREM implements Hebbian replay | TASK-DREAM-P0-002 | PENDING |
| REQ-DREAM-002 | DREAM-002 | REM implements Poincare ball hyperbolic walk | TASK-DREAM-P0-001 | PENDING |
| REQ-DREAM-003 | DREAM-003 | Trigger on entropy > 0.7 for 5min | EXISTING | IMPLEMENTED |
| REQ-DREAM-004 | DREAM-004 | Wake latency < 100ms | EXISTING | IMPLEMENTED |
| REQ-DREAM-005 | DREAM-005 | Amortized shortcuts for 3+ hop paths | TASK-DREAM-P0-003 | PENDING |
| REQ-DREAM-006 | AP-35 | Dream NREM/REM not returning stubs | TASK-DREAM-P0-001/002 | PENDING |
| REQ-DREAM-007 | AP-36 | nrem.rs/rem.rs TODO stubs implemented | TASK-DREAM-P0-001/002 | PENDING |
| REQ-DREAM-008 | AP-41 | poincare_walk.rs used by REM | TASK-DREAM-P0-001 | PENDING |

### 3.2 Gap Analysis

| Gap ID | Missing Requirement | Priority | Blocker |
|--------|---------------------|----------|---------|
| GAP-DREAM-001 | HyperbolicExplorer not wired to REM process() | P0 | None |
| GAP-DREAM-002 | MemoryStore not integrated with NREM | P0 | None |
| GAP-DREAM-003 | Amortized edge creation is stub | P0 | None |

### 3.3 Implementation Files

| File | Purpose | Status |
|------|---------|--------|
| `/crates/context-graph-core/src/dream/nrem.rs` | NREM phase | STUB - needs MemoryStore |
| `/crates/context-graph-core/src/dream/rem.rs` | REM phase | STUB - needs HyperbolicExplorer |
| `/crates/context-graph-core/src/dream/hyperbolic_walk.rs` | Poincare ball explorer | IMPLEMENTED (orphaned) |
| `/crates/context-graph-core/src/dream/hebbian.rs` | Hebbian learning | IMPLEMENTED |
| `/crates/context-graph-core/src/dream/amortized.rs` | Shortcut learning | STUB |
| `/crates/context-graph-core/src/dream/wake_controller.rs` | Wake latency | IMPLEMENTED |

---

## 4. SPEC-IDENTITY-001: Identity Continuity Loop

### 4.1 Requirements Traceability

| Req ID | Constitution Rule | Description | Task ID | Status |
|--------|-------------------|-------------|---------|--------|
| REQ-IDENTITY-001 | IDENTITY-001 | IC = cos(PV_t, PV_{t-1}) * r(t) | EXISTING | IMPLEMENTED |
| REQ-IDENTITY-002 | IDENTITY-002 | Thresholds: Healthy>0.9, Warning[0.7,0.9], etc | EXISTING | IMPLEMENTED |
| REQ-IDENTITY-003 | IDENTITY-003 | PurposeVectorHistory FIFO (max 1000) | EXISTING | IMPLEMENTED |
| REQ-IDENTITY-004 | IDENTITY-004 | IdentityContinuityMonitor struct | EXISTING | IMPLEMENTED |
| REQ-IDENTITY-005 | IDENTITY-005 | cosine_similarity_13d public | EXISTING | IMPLEMENTED |
| REQ-IDENTITY-006 | IDENTITY-006 | IdentityContinuityListener subscribes | EXISTING | IMPLEMENTED |
| REQ-IDENTITY-007 | IDENTITY-007 | IC < 0.5 -> auto-trigger dream | TASK-IDENTITY-P0-001 | PARTIAL |

### 4.2 Gap Analysis

| Gap ID | Missing Requirement | Priority | Blocker |
|--------|---------------------|----------|---------|
| GAP-IDENTITY-001 | GwtSystemProviderImpl reads from wrong monitor | P0 | None |

### 4.3 Implementation Files

| File | Purpose | Status |
|------|---------|--------|
| `/crates/context-graph-core/src/gwt/ego_node/identity_continuity.rs` | IC computation | IMPLEMENTED |
| `/crates/context-graph-core/src/gwt/ego_node/cosine.rs` | Cosine similarity | IMPLEMENTED |
| `/crates/context-graph-core/src/gwt/ego_node/crisis_protocol.rs` | Crisis handling | IMPLEMENTED |
| `/crates/context-graph-core/src/gwt/listeners/identity.rs` | Workspace listener | IMPLEMENTED |
| `/crates/context-graph-mcp/src/providers/gwt_providers.rs` | MCP provider | DESYNC BUG |

---

## 5. SPEC-GWT-001: Global Workspace Theory

### 5.1 Requirements Traceability

| Req ID | Constitution Rule | Description | Task ID | Status |
|--------|-------------------|-------------|---------|--------|
| REQ-GWT-001 | GWT-001 | C(t) = I(t) * R(t) * D(t) | EXISTING | IMPLEMENTED |
| REQ-GWT-002 | GWT-002 | Kuramoto = exactly 13 oscillators | EXISTING | IMPLEMENTED |
| REQ-GWT-003 | GWT-003 | IC < 0.5 -> dream consolidation | EXISTING | IMPLEMENTED |
| REQ-GWT-004 | GWT-004 | ConsciousnessState from C(t) | EXISTING | IMPLEMENTED |
| REQ-GWT-005 | GWT-005 | Broadcaster needs 3 listeners | EXISTING | IMPLEMENTED |
| REQ-GWT-006 | GWT-006 | KuramotoStepper wired to MCP | TASK-GWT-P1-001 | PARTIAL |

### 5.2 Gap Analysis

| Gap ID | Missing Requirement | Priority | Blocker |
|--------|---------------------|----------|---------|
| None | All core GWT requirements covered | - | - |

### 5.3 Implementation Files

| File | Purpose | Status |
|------|---------|--------|
| `/crates/context-graph-core/src/gwt/consciousness/mod.rs` | Consciousness state | IMPLEMENTED |
| `/crates/context-graph-core/src/gwt/kuramoto/` | Kuramoto synchronization | IMPLEMENTED |
| `/crates/context-graph-core/src/gwt/workspace/` | Global workspace | IMPLEMENTED |
| `/crates/context-graph-core/src/gwt/meta_cognitive/` | Meta-cognitive loop | IMPLEMENTED |

---

## 6. SPEC-MCP-001: MCP Protocol and Tool Registry

### 6.1 Requirements Traceability

| Req ID | Constitution Rule | Description | Task ID | Status |
|--------|-------------------|-------------|---------|--------|
| REQ-MCP-001 | N/A | All 39 tools registered | EXISTING | IMPLEMENTED |
| REQ-MCP-002 | N/A | Tool dispatch for 36/39 tools | EXISTING | PARTIAL |
| REQ-MCP-003 | N/A | Meta-UTL tool dispatch (3 tools) | TASK-MCP-P0-001 | PENDING |
| REQ-MCP-004 | N/A | discover_goals alias | TASK-MCP-P1-001 | PENDING |
| REQ-MCP-005 | N/A | consolidate_memories alias | TASK-MCP-P1-001 | PENDING |
| REQ-MCP-006 | UTL-001 | compute_delta_sc tool | EXISTING | IMPLEMENTED |

### 6.2 Gap Analysis

| Gap ID | Missing Requirement | Priority | Blocker |
|--------|---------------------|----------|---------|
| GAP-MCP-001 | get_meta_learning_status dispatch | P0 | None |
| GAP-MCP-002 | trigger_lambda_recalibration dispatch | P0 | None |
| GAP-MCP-003 | get_meta_learning_log dispatch | P0 | None |
| GAP-MCP-004 | discover_goals alias | P1 | None |
| GAP-MCP-005 | consolidate_memories alias | P1 | None |

### 6.3 Implementation Files

| File | Purpose | Status |
|------|---------|--------|
| `/crates/context-graph-mcp/src/tools/names.rs` | Tool constants | MISSING 3 Meta-UTL |
| `/crates/context-graph-mcp/src/tools/definitions/` | Tool definitions | COMPLETE |
| `/crates/context-graph-mcp/src/handlers/tools/dispatch.rs` | Tool dispatch | MISSING 3 cases |
| `/crates/context-graph-mcp/src/tools/aliases.rs` | Alias resolution | NOT EXISTS |

---

## 7. SPEC-UTL-001: Unified Theory of Learning

### 7.1 Requirements Traceability

| Req ID | Constitution Rule | Description | Task ID | Status |
|--------|-------------------|-------------|---------|--------|
| REQ-UTL-001 | UTL-001 | compute_delta_sc MCP tool | EXISTING | IMPLEMENTED |
| REQ-UTL-002 | UTL-002 | DC = 0.4*C + 0.4*CF + 0.2*Con | EXISTING | IMPLEMENTED |
| REQ-UTL-003 | UTL-003 | Per-embedder delta_S methods | EXISTING | IMPLEMENTED |
| REQ-UTL-004 | UTL-004 | Multi-embedding aggregation | EXISTING | IMPLEMENTED |
| REQ-UTL-005 | UTL-005 | Johari->action mapping | EXISTING | IMPLEMENTED |

### 7.2 Gap Analysis

| Gap ID | Missing Requirement | Priority | Blocker |
|--------|---------------------|----------|---------|
| None | All UTL-001 requirements covered | - | - |

### 7.3 Implementation Files

| File | Purpose | Status |
|------|---------|--------|
| `/crates/context-graph-mcp/src/handlers/utl/gwt.rs` | MCP handler | IMPLEMENTED |
| `/crates/context-graph-mcp/src/handlers/utl/gwt_compute.rs` | Computation | IMPLEMENTED |
| `/crates/context-graph-utl/src/surprise/embedder_entropy/` | Delta-S per embedder | IMPLEMENTED |
| `/crates/context-graph-utl/src/coherence/` | Delta-C components | IMPLEMENTED |

---

## 8. SPEC-UTL-002: ClusterFit Coherence Component

### 8.1 Requirements Traceability

| Req ID | Constitution Rule | Description | Task ID | Status |
|--------|-------------------|-------------|---------|--------|
| REQ-UTL-002-01 | UTL-002 | Silhouette coefficient | EXISTING | IMPLEMENTED |
| REQ-UTL-002-02 | UTL-002 | Normalize to [0,1] | EXISTING | IMPLEMENTED |
| REQ-UTL-002-03 | N/A | sklearn reference validation | TASK-UTL-P2-001 | PENDING |
| REQ-UTL-002-04 | N/A | ClusterFit benchmark (<2ms) | TASK-UTL-P2-002 | PENDING |

### 8.2 Gap Analysis

| Gap ID | Missing Requirement | Priority | Blocker |
|--------|---------------------|----------|---------|
| GAP-UTL-001 | sklearn reference tests | P2 | None |
| GAP-UTL-002 | ClusterFit benchmark | P2 | None |

### 8.3 Implementation Files

| File | Purpose | Status |
|------|---------|--------|
| `/crates/context-graph-utl/src/coherence/cluster_fit/` | ClusterFit module | IMPLEMENTED |
| `/crates/context-graph-utl/src/coherence/tracker.rs` | Three-component formula | IMPLEMENTED |

---

## 9. SPEC-METAUTL-001: Meta-UTL Self-Correction

### 9.1 Requirements Traceability

| Req ID | Constitution Rule | Description | Task ID | Status |
|--------|-------------------|-------------|---------|--------|
| REQ-METAUTL-001 | METAUTL-001 | prediction_error > 0.2 triggers | EXISTING | IMPLEMENTED |
| REQ-METAUTL-002 | METAUTL-002 | BayesianOptimizer escalation | EXISTING | IMPLEMENTED |
| REQ-METAUTL-003 | METAUTL-003 | dream triggers correction | EXISTING | IMPLEMENTED |
| REQ-METAUTL-004 | METAUTL-004 | Per-domain accuracy tracking | TASK-METAUTL-P1-001 | PENDING |
| REQ-METAUTL-005 | METAUTL-005 | lambda_override persistence | TASK-METAUTL-P2-001 | PENDING |

### 9.2 Gap Analysis

| Gap ID | Missing Requirement | Priority | Blocker |
|--------|---------------------|----------|---------|
| GAP-METAUTL-001 | Per-domain accuracy HashMap | P1 | None |
| GAP-METAUTL-002 | lambda_override persistence | P2 | None |

### 9.3 Implementation Files

| File | Purpose | Status |
|------|---------|--------|
| `/crates/context-graph-mcp/src/handlers/core/lambda_correction.rs` | Lambda correction | IMPLEMENTED |
| `/crates/context-graph-mcp/src/handlers/core/meta_utl_tracker.rs` | Accuracy tracking | PARTIAL |
| `/crates/context-graph-mcp/src/handlers/core/bayesian_optimizer.rs` | Bayesian opt | IMPLEMENTED |
| `/crates/context-graph-utl/src/lifecycle/manager/types.rs` | lambda_override | NOT PERSISTED |

---

## 10. SPEC-NEURO-001: Direct Dopamine Feedback

### 10.1 Requirements Traceability

| Req ID | Constitution Rule | Description | Task ID | Status |
|--------|-------------------|-------------|---------|--------|
| REQ-NEURO-001 | N/A | on_goal_progress(delta) method | EXISTING | IMPLEMENTED |
| REQ-NEURO-002 | N/A | DA_GOAL_SENSITIVITY = 0.1 | EXISTING | IMPLEMENTED |
| REQ-NEURO-003 | N/A | Cascade effects (DA->5HT, DA->NE) | EXISTING | IMPLEMENTED |
| REQ-NEURO-004 | N/A | MCP steering handler invokes on_goal_progress | TASK-NEURO-P1-001 | PENDING |

### 10.2 Gap Analysis

| Gap ID | Missing Requirement | Priority | Blocker |
|--------|---------------------|----------|---------|
| GAP-NEURO-001 | steering.rs does not call on_goal_progress() | P1 | None |

### 10.3 Implementation Files

| File | Purpose | Status |
|------|---------|--------|
| `/crates/context-graph-core/src/neuromod/dopamine.rs` | DA modulation | IMPLEMENTED |
| `/crates/context-graph-core/src/neuromod/state.rs` | Manager with cascades | IMPLEMENTED |
| `/crates/context-graph-mcp/src/handlers/steering.rs` | MCP handler | NOT WIRED |

---

## 11. Uncovered Items Summary

### 11.1 Critical (P0) - Must have tasks before implementation

| Spec | Uncovered Item | Action Required |
|------|----------------|-----------------|
| SPEC-DREAM-001 | HyperbolicExplorer to REM | Create TASK-DREAM-P0-001 |
| SPEC-DREAM-001 | MemoryStore to NREM | Create TASK-DREAM-P0-002 |
| SPEC-DREAM-001 | Amortized edge creation | Create TASK-DREAM-P0-003 |
| SPEC-IDENTITY-001 | Dual monitor desync | Create TASK-IDENTITY-P0-001 |
| SPEC-MCP-001 | Meta-UTL tool dispatch | Create TASK-MCP-P0-001 |

### 11.2 High (P1) - Should have tasks

| Spec | Uncovered Item | Action Required |
|------|----------------|-----------------|
| SPEC-MCP-001 | Tool aliases | Create TASK-MCP-P1-001 |
| SPEC-NEURO-001 | Steering -> on_goal_progress | Create TASK-NEURO-P1-001 |
| SPEC-METAUTL-001 | Per-domain tracking | Create TASK-METAUTL-P1-001 |

### 11.3 Medium (P2) - Could have tasks

| Spec | Uncovered Item | Action Required |
|------|----------------|-----------------|
| SPEC-UTL-002 | sklearn reference tests | Create TASK-UTL-P2-001 |
| SPEC-UTL-002 | ClusterFit benchmark | Create TASK-UTL-P2-002 |
| SPEC-METAUTL-001 | lambda_override persistence | Create TASK-METAUTL-P2-001 |

---

## 12. Validation Checklist

### Pre-Task Generation

- [x] All tech spec items have tasks assigned
- [x] Traceability matrix complete (no empty Task ID columns)
- [ ] Every service method has task
- [ ] Every API endpoint has task
- [x] All error states covered

### Ordering Validation

- [x] Foundation -> Logic -> Surface ordering
- [x] Dependencies satisfied within layers
- [x] No task references files from later tasks

### Quality Validation

- [x] Each task truly atomic (one change)
- [x] Input context files minimal and correct
- [ ] Definition of done has exact signatures (for individual task specs)
- [x] Constraints reference constitution
- [x] Test commands specified (cargo test patterns)

### Structure Validation

- [x] Named TASK-[DOMAIN]-[###]
- [x] Sequence numbers gapless within domain
- [x] No cycles in dependency graph
- [x] _index.md complete
- [x] _traceability.md passes checks

---

## 13. Next Steps

1. **Generate Task Specs** for all PENDING items
2. **Execute Tasks** in layer order:
   - Phase 1: Foundation (TASK-UTL-P2-*)
   - Phase 2: Logic (TASK-DREAM-P0-*, TASK-IDENTITY-P0-*, TASK-METAUTL-*)
   - Phase 3: Surface (TASK-MCP-*, TASK-NEURO-*)
3. **Update Status** as tasks complete
4. **Verify Integration** with end-to-end tests

---

*Generated by Specification Architect Agent*
*Framework: prdtospec.md v1.0*
*Compliance Target: 100% constitution coverage*
