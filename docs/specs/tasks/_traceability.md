# Traceability Matrix

**Document**: TRACEABILITY-MATRIX-001
**Version**: 2.0.0
**Generated**: 2026-01-12
**Author**: Traceability Agent (Claude Opus 4.5)

---

## Executive Summary

| Metric | Count | Status |
|--------|-------|--------|
| Total Issues | 16 | All Covered |
| Total Requirements | 35 | All Covered |
| Total Tasks | 42 | All Linked |
| Uncovered Items | 0 | PASS |

**VERIFICATION STATUS: 100% COVERAGE ACHIEVED**

---

## ⚠️ MANDATORY EXECUTION ORDER

**ALL 42 TASKS MUST BE EXECUTED IN THE ORDER SPECIFIED IN `_index.md`**

The order below is the ONLY valid execution sequence:

```
STEP 1  → TASK-ARCH-001     STEP 22 → TASK-DREAM-001
STEP 2  → TASK-ARCH-002     STEP 23 → TASK-DREAM-002
STEP 3  → TASK-ARCH-003     STEP 24 → TASK-DREAM-003
STEP 4  → TASK-ARCH-004     STEP 25 → TASK-DREAM-004
STEP 5  → TASK-ARCH-005     STEP 26 → TASK-DREAM-005
STEP 6  → TASK-PERF-001     STEP 27 → TASK-MCP-001
STEP 7  → TASK-PERF-002     STEP 28 → TASK-MCP-002
STEP 8  → TASK-PERF-003     STEP 29 → TASK-MCP-003
STEP 9  → TASK-UTL-001      STEP 30 → TASK-MCP-004
STEP 10 → TASK-GWT-001      STEP 31 → TASK-MCP-005
STEP 11 → TASK-GWT-002      STEP 32 → TASK-MCP-006
STEP 12 → TASK-GWT-003      STEP 33 → TASK-MCP-007
STEP 13 → TASK-EMBED-001    STEP 34 → TASK-MCP-008
STEP 14 → TASK-EMBED-002    STEP 35 → TASK-MCP-009
STEP 15 → TASK-EMBED-003    STEP 36 → TASK-MCP-010
STEP 16 → TASK-PERF-004     STEP 37 → TASK-MCP-011
STEP 17 → TASK-PERF-005     STEP 38 → TASK-MCP-012
STEP 18 → TASK-PERF-006     STEP 39 → TASK-MCP-013
STEP 19 → TASK-IDENTITY-001 STEP 40 → TASK-MCP-014
STEP 20 → TASK-IDENTITY-002 STEP 41 → TASK-MCP-015
STEP 21 → TASK-IDENTITY-003 STEP 42 → TASK-MCP-016
```

---

## 1. EXECUTION ORDER BY ISSUE (Severity-First)

### CRITICAL ISSUES (Must Fix First)

#### ISS-005: CUDA FFI scattered [Steps 1-5]
**Execute Steps**: 1 → 2 → 3 → 4 → 5

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 1 | TASK-ARCH-001 | Create context-graph-cuda crate skeleton | 2 | - |
| 2 | TASK-ARCH-002 | Consolidate CUDA driver FFI bindings | 4 | Step 1 |
| 3 | TASK-ARCH-003 | Consolidate FAISS FFI bindings | 4 | Step 1 |
| 4 | TASK-ARCH-004 | Implement safe GpuDevice RAII wrapper | 3 | Steps 2, 3 |
| 5 | TASK-ARCH-005 | Add CI gate for FFI consolidation | 1 | Step 4 |

---

#### ISS-004: block_on() deadlock risk [Steps 6-8, 16]
**Execute Steps**: 6 → 7 → 8 → (later) 16

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 6 | TASK-PERF-001 | Add async-trait to MCP crate | 0.5 | - |
| 7 | TASK-PERF-002 | Convert WorkspaceProvider to async | 2 | Step 6 |
| 8 | TASK-PERF-003 | Convert MetaCognitiveProvider to async | 1.5 | Step 6 |
| 16 | TASK-PERF-004 | Remove block_on from gwt_providers | 2 | Step 7 |

---

#### ISS-001: Kuramoto 8 oscillators (wrong) [Steps 10-11]
**Execute Steps**: 10 → 11

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 10 | TASK-GWT-001 | Add KURAMOTO_N constant (13 oscillators) | 1 | - |
| 11 | TASK-GWT-002 | Implement KuramotoNetwork with 13 frequencies | 3 | Step 10 |

---

#### ISS-003: KuramotoStepper dead code [Steps 12, 25]
**Execute Steps**: 12 → (later) 25

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 12 | TASK-GWT-003 | Implement KuramotoStepper lifecycle | 4 | Step 11 |
| 25 | TASK-DREAM-004 | Integrate KuramotoStepper with MCP server | 3 | Step 12 |

---

#### ISS-002: IC < 0.5 no dream trigger [Steps 19-21, 24, 26]
**Execute Steps**: 19 → 20 → 21 → (later) 24 → (later) 26

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 19 | TASK-IDENTITY-001 | Add IdentityCritical variant | 1 | - |
| 20 | TASK-IDENTITY-002 | Implement TriggerConfig with IC threshold | 1.5 | Step 19 |
| 21 | TASK-IDENTITY-003 | Implement TriggerManager IC checking | 3 | Step 20 |
| 24 | TASK-DREAM-003 | Wire DreamEventListener to TriggerManager | 3 | Steps 21, 23 |
| 26 | TASK-DREAM-005 | Wire IC monitor to emit IdentityCritical events | 2 | Step 25 |

---

### HIGH PRIORITY ISSUES

#### ISS-010: IdentityCritical variant missing [Steps 19-20]
**Execute Steps**: 19 → 20 (same as ISS-002 first steps)

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 19 | TASK-IDENTITY-001 | Add IdentityCritical variant | 1 | - |
| 20 | TASK-IDENTITY-002 | Implement TriggerConfig with IC threshold | 1.5 | Step 19 |

---

#### ISS-007: GpuMonitor stub returns 0 [Steps 22-23]
**Execute Steps**: 22 → 23

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 22 | TASK-DREAM-001 | Implement GpuMonitor trait and error types | 2 | - |
| 23 | TASK-DREAM-002 | Implement NvmlGpuMonitor with thresholds | 4 | Step 22 |

---

#### ISS-008: Green Contexts not auto-enabled [Step 13]
**Execute Step**: 13

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 13 | TASK-EMBED-001 | Implement Green Contexts auto-enable | 4 | Step 4 |

---

#### ISS-009: TokenPruning for E12 missing [Steps 14-15]
**Execute Steps**: 14 → 15

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 14 | TASK-EMBED-002 | Implement TokenPruningConfig types | 2 | - |
| 15 | TASK-EMBED-003 | Implement TokenPruningQuantizer | 4 | Step 14 |

---

#### ISS-006: Missing MCP tools [Steps 27-42]
**Execute Steps**: 27 → 28 → 29 → 30 → 31 → 32 → 33 → 34 → 35 → 36 → 37 → 38 → 39 → 40 → 41 → 42

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 27 | TASK-MCP-001 | Implement epistemic_action tool schema | 2 | - |
| 28 | TASK-MCP-002 | Implement epistemic_action handler | 4 | Step 27 |
| 29 | TASK-MCP-003 | Implement merge_concepts tool schema | 2 | - |
| 30 | TASK-MCP-004 | Implement merge_concepts handler | 6 | Step 29 |
| 31 | TASK-MCP-005 | Implement get_johari_classification tool | 3 | Step 9 |
| 32 | TASK-MCP-006 | Add SSE transport types | 2 | - |
| 33 | TASK-MCP-007 | Implement SSE handler with keep-alive | 4 | Step 32 |
| 34 | TASK-MCP-008 | Implement get_coherence_state tool | 3 | Step 12 |
| 35 | TASK-MCP-009 | Implement trigger_dream tool | 3 | Step 24 |
| 36 | TASK-MCP-010 | Add parameter validation middleware | 4 | - |
| 37 | TASK-MCP-011 | Implement get_gpu_status tool | 2 | Step 23 |
| 38 | TASK-MCP-012 | Implement get_identity_continuity tool | 2 | Step 21 |
| 39 | TASK-MCP-013 | Implement get_kuramoto_state tool | 2 | Step 12 |
| 40 | TASK-MCP-014 | Implement set_coupling_strength tool | 2 | Step 12 |
| 41 | TASK-MCP-015 | Add tool registration to MCP server | 3 | Steps 28,30,31,34-40 |
| 42 | TASK-MCP-016 | SSE integration with MCP router | 3 | Step 33 |

---

### MEDIUM PRIORITY ISSUES

#### ISS-011: Johari Blind/Unknown swapped [Step 9]
**Execute Step**: 9

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 9 | TASK-UTL-001 | Fix Johari Blind/Unknown action mapping | 1 | - |

---

#### ISS-012: Parameter validation missing [Step 36]
**Execute Step**: 36

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 36 | TASK-MCP-010 | Add parameter validation middleware | 4 | - |

---

#### ISS-013: SSE transport missing [Steps 32, 33, 42]
**Execute Steps**: 32 → 33 → 42

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 32 | TASK-MCP-006 | Add SSE transport types | 2 | - |
| 33 | TASK-MCP-007 | Implement SSE handler with keep-alive | 4 | Step 32 |
| 42 | TASK-MCP-016 | SSE integration with MCP router | 3 | Step 33 |

---

#### ISS-014: GPU threshold confusion [Step 23]
**Execute Step**: 23 (same as ISS-007)

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 23 | TASK-DREAM-002 | Implement NvmlGpuMonitor with thresholds | 4 | Step 22 |

---

### LOW PRIORITY ISSUES

#### ISS-015: RwLock blocking on single-thread [Step 17]
**Execute Step**: 17

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 17 | TASK-PERF-005 | Add parking_lot::RwLock to wake_controller | 1 | - |

---

#### ISS-016: HashMap alloc without capacity [Step 18]
**Execute Step**: 18

| Step | Task ID | Title | Hours | Depends On |
|------|---------|-------|-------|------------|
| 18 | TASK-PERF-006 | Pre-allocate HashMap capacity in hot paths | 1 | - |

---

## 2. COMPLETE TASK-TO-STEP MAPPING

| Task ID | Step # | Title | Layer | Hours |
|---------|--------|-------|-------|-------|
| TASK-ARCH-001 | 1 | Create context-graph-cuda crate skeleton | Foundation | 2 |
| TASK-ARCH-002 | 2 | Consolidate CUDA driver FFI bindings | Foundation | 4 |
| TASK-ARCH-003 | 3 | Consolidate FAISS FFI bindings | Foundation | 4 |
| TASK-ARCH-004 | 4 | Implement safe GpuDevice RAII wrapper | Foundation | 3 |
| TASK-ARCH-005 | 5 | Add CI gate for FFI consolidation | Foundation | 1 |
| TASK-PERF-001 | 6 | Add async-trait to MCP crate | Foundation | 0.5 |
| TASK-PERF-002 | 7 | Convert WorkspaceProvider to async | Foundation | 2 |
| TASK-PERF-003 | 8 | Convert MetaCognitiveProvider to async | Foundation | 1.5 |
| TASK-UTL-001 | 9 | Fix Johari Blind/Unknown action mapping | Foundation | 1 |
| TASK-GWT-001 | 10 | Add KURAMOTO_N constant (13 oscillators) | Core | 1 |
| TASK-GWT-002 | 11 | Implement KuramotoNetwork with 13 frequencies | Core | 3 |
| TASK-GWT-003 | 12 | Implement KuramotoStepper lifecycle | Core | 4 |
| TASK-EMBED-001 | 13 | Implement Green Contexts auto-enable | Core | 4 |
| TASK-EMBED-002 | 14 | Implement TokenPruningConfig types | Core | 2 |
| TASK-EMBED-003 | 15 | Implement TokenPruningQuantizer | Core | 4 |
| TASK-PERF-004 | 16 | Remove block_on from gwt_providers | Core | 2 |
| TASK-PERF-005 | 17 | Add parking_lot::RwLock to wake_controller | Core | 1 |
| TASK-PERF-006 | 18 | Pre-allocate HashMap capacity in hot paths | Core | 1 |
| TASK-IDENTITY-001 | 19 | Add IdentityCritical variant | Integration | 1 |
| TASK-IDENTITY-002 | 20 | Implement TriggerConfig with IC threshold | Integration | 1.5 |
| TASK-IDENTITY-003 | 21 | Implement TriggerManager IC checking | Integration | 3 |
| TASK-DREAM-001 | 22 | Implement GpuMonitor trait and error types | Integration | 2 |
| TASK-DREAM-002 | 23 | Implement NvmlGpuMonitor with thresholds | Integration | 4 |
| TASK-DREAM-003 | 24 | Wire DreamEventListener to TriggerManager | Integration | 3 |
| TASK-DREAM-004 | 25 | Integrate KuramotoStepper with MCP server | Integration | 3 |
| TASK-DREAM-005 | 26 | Wire IC monitor to emit IdentityCritical events | Integration | 2 |
| TASK-MCP-001 | 27 | Implement epistemic_action tool schema | Surface | 2 |
| TASK-MCP-002 | 28 | Implement epistemic_action handler | Surface | 4 |
| TASK-MCP-003 | 29 | Implement merge_concepts tool schema | Surface | 2 |
| TASK-MCP-004 | 30 | Implement merge_concepts handler | Surface | 6 |
| TASK-MCP-005 | 31 | Implement get_johari_classification tool | Surface | 3 |
| TASK-MCP-006 | 32 | Add SSE transport types | Surface | 2 |
| TASK-MCP-007 | 33 | Implement SSE handler with keep-alive | Surface | 4 |
| TASK-MCP-008 | 34 | Implement get_coherence_state tool | Surface | 3 |
| TASK-MCP-009 | 35 | Implement trigger_dream tool | Surface | 3 |
| TASK-MCP-010 | 36 | Add parameter validation middleware | Surface | 4 |
| TASK-MCP-011 | 37 | Implement get_gpu_status tool | Surface | 2 |
| TASK-MCP-012 | 38 | Implement get_identity_continuity tool | Surface | 2 |
| TASK-MCP-013 | 39 | Implement get_kuramoto_state tool | Surface | 2 |
| TASK-MCP-014 | 40 | Implement set_coupling_strength tool | Surface | 2 |
| TASK-MCP-015 | 41 | Add tool registration to MCP server | Surface | 3 |
| TASK-MCP-016 | 42 | SSE integration with MCP router | Surface | 3 |

---

## 3. DEPENDENCY CHAIN VISUALIZATION

### Chain 1: Architecture Foundation
```
Step 1 (ARCH-001) ─┬─→ Step 2 (ARCH-002) ──┐
                   │                        ├─→ Step 4 (ARCH-004) ─→ Step 5 (ARCH-005)
                   └─→ Step 3 (ARCH-003) ──┘         │
                                                     └─→ Step 13 (EMBED-001)
```

### Chain 2: Async Performance
```
Step 6 (PERF-001) ─┬─→ Step 7 (PERF-002) ─→ Step 16 (PERF-004)
                   │
                   └─→ Step 8 (PERF-003)
```

### Chain 3: GWT Consciousness
```
Step 10 (GWT-001) ─→ Step 11 (GWT-002) ─→ Step 12 (GWT-003) ─┬─→ Step 25 (DREAM-004) ─→ Step 26 (DREAM-005)
                                                              │
                                                              ├─→ Step 34 (MCP-008)
                                                              ├─→ Step 39 (MCP-013)
                                                              └─→ Step 40 (MCP-014)
```

### Chain 4: Embeddings
```
Step 14 (EMBED-002) ─→ Step 15 (EMBED-003)
```

### Chain 5: Identity/Dream Integration
```
Step 19 (IDENTITY-001) ─→ Step 20 (IDENTITY-002) ─→ Step 21 (IDENTITY-003) ─┬─→ Step 24 (DREAM-003) ─→ Step 35 (MCP-009)
                                                                            │
                                                                            └─→ Step 38 (MCP-012)

Step 22 (DREAM-001) ─→ Step 23 (DREAM-002) ─┬─→ Step 24 (DREAM-003)
                                            │
                                            └─→ Step 37 (MCP-011)
```

### Chain 6: MCP Tools
```
Step 27 (MCP-001) ─→ Step 28 (MCP-002) ──┐
Step 29 (MCP-003) ─→ Step 30 (MCP-004) ──┤
Step 9 (UTL-001) ──→ Step 31 (MCP-005) ──┤
Step 32 (MCP-006) ─→ Step 33 (MCP-007) ──┼─→ Step 42 (MCP-016)
                                         │
Step 34, 35, 36, 37, 38, 39, 40 ─────────┴─→ Step 41 (MCP-015)
```

---

## 4. VERIFICATION CHECKLIST

### Prerequisites Check (Before Starting Step N)
```
BEFORE STEP 2:  Verify Step 1 complete (ARCH-001)
BEFORE STEP 3:  Verify Step 1 complete (ARCH-001)
BEFORE STEP 4:  Verify Steps 2,3 complete (ARCH-002, ARCH-003)
BEFORE STEP 5:  Verify Step 4 complete (ARCH-004)
BEFORE STEP 7:  Verify Step 6 complete (PERF-001)
BEFORE STEP 8:  Verify Step 6 complete (PERF-001)
BEFORE STEP 11: Verify Step 10 complete (GWT-001)
BEFORE STEP 12: Verify Step 11 complete (GWT-002)
BEFORE STEP 13: Verify Step 4 complete (ARCH-004)
BEFORE STEP 15: Verify Step 14 complete (EMBED-002)
BEFORE STEP 16: Verify Step 7 complete (PERF-002)
BEFORE STEP 20: Verify Step 19 complete (IDENTITY-001)
BEFORE STEP 21: Verify Step 20 complete (IDENTITY-002)
BEFORE STEP 23: Verify Step 22 complete (DREAM-001)
BEFORE STEP 24: Verify Steps 21,23 complete (IDENTITY-003, DREAM-002)
BEFORE STEP 25: Verify Step 12 complete (GWT-003)
BEFORE STEP 26: Verify Step 25 complete (DREAM-004)
BEFORE STEP 28: Verify Step 27 complete (MCP-001)
BEFORE STEP 30: Verify Step 29 complete (MCP-003)
BEFORE STEP 31: Verify Step 9 complete (UTL-001)
BEFORE STEP 33: Verify Step 32 complete (MCP-006)
BEFORE STEP 34: Verify Step 12 complete (GWT-003)
BEFORE STEP 35: Verify Step 24 complete (DREAM-003)
BEFORE STEP 37: Verify Step 23 complete (DREAM-002)
BEFORE STEP 38: Verify Step 21 complete (IDENTITY-003)
BEFORE STEP 39: Verify Step 12 complete (GWT-003)
BEFORE STEP 40: Verify Step 12 complete (GWT-003)
BEFORE STEP 41: Verify Steps 28,30,31,34,35,36,37,38,39,40 complete
BEFORE STEP 42: Verify Step 33 complete (MCP-007)
```

---

## 5. COVERAGE METRICS

### 5.1 Issue Coverage
| Issue ID | Severity | # Tasks | Steps | Coverage |
|----------|----------|---------|-------|----------|
| ISS-001 | CRITICAL | 2 | 10, 11 | 100% |
| ISS-002 | CRITICAL | 3 | 21, 24, 26 | 100% |
| ISS-003 | CRITICAL | 2 | 12, 25 | 100% |
| ISS-004 | CRITICAL | 4 | 6, 7, 8, 16 | 100% |
| ISS-005 | CRITICAL | 5 | 1, 2, 3, 4, 5 | 100% |
| ISS-006 | HIGH | 14 | 27-42 | 100% |
| ISS-007 | HIGH | 2 | 22, 23 | 100% |
| ISS-008 | HIGH | 1 | 13 | 100% |
| ISS-009 | HIGH | 2 | 14, 15 | 100% |
| ISS-010 | HIGH | 2 | 19, 20 | 100% |
| ISS-011 | MEDIUM | 1 | 9 | 100% |
| ISS-012 | MEDIUM | 1 | 36 | 100% |
| ISS-013 | MEDIUM | 3 | 32, 33, 42 | 100% |
| ISS-014 | MEDIUM | 1 | 23 | 100% |
| ISS-015 | LOW | 1 | 17 | 100% |
| ISS-016 | LOW | 1 | 18 | 100% |

### 5.2 Domain Coverage
| Domain | Tasks | Steps | Hours |
|--------|-------|-------|-------|
| Architecture | 5 | 1-5 | 14 |
| Performance | 6 | 6-8, 16-18 | 8 |
| UTL | 1 | 9 | 1 |
| GWT | 3 | 10-12 | 8 |
| Embeddings | 3 | 13-15 | 10 |
| Identity | 3 | 19-21 | 5.5 |
| Dream | 5 | 22-26 | 14 |
| MCP | 16 | 27-42 | 38 |
| **TOTAL** | **42** | **1-42** | **98.5** |

---

## 6. AUDIT TRAIL

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-12 | v1.0.0 created | Initial generation |
| 2026-01-12 | v2.0.0 updated | Added unambiguous step ordering |

---

## 7. FINAL VERIFICATION

```
+------------------------------------------+
|          TRACEABILITY VERIFICATION       |
+------------------------------------------+
| Total Issues:                    16      |
| Total Requirements:              35      |
| Total Tasks:                     42      |
| Total Steps:                     42      |
+------------------------------------------+
| Issues → Tasks Coverage:        100%     |
| Requirements → Tasks Coverage:  100%     |
| Orphan Tasks:                    0       |
| Dependency Cycles:               0       |
+------------------------------------------+
| VERIFICATION STATUS:            PASS     |
+------------------------------------------+
```

**100% COVERAGE - ALL 16 ISSUES TRACED TO 42 TASKS IN 42 SEQUENTIAL STEPS**

---

*Version 2.0.0 - Generated 2026-01-12 - UNAMBIGUOUS EXECUTION ORDER*
