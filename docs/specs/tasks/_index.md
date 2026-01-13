# Task Specification Index

**Spec Version**: 3.0.0
**Generated**: 2026-01-12
**Total Tasks**: 42

---

## 🤖 AI AGENT INSTRUCTIONS

**READ THIS SECTION FIRST IF YOU ARE AN AI AGENT WITH NO PRIOR CONTEXT.**

### What This Is
This is a task backlog for the `context-graph` project, a Rust-based system implementing Global Workspace Theory (GWT) with GPU acceleration. Tasks implement features across CUDA/FAISS FFI, async providers, Kuramoto oscillators for consciousness modeling, dream consolidation, identity continuity monitoring, and MCP (Model Context Protocol) tools.

### How to Execute Tasks
1. **Read the task file**: Each task is in `TASK-NN.md` (e.g., `TASK-01.md`)
2. **Check dependencies**: The `<depends_on>` tag lists prerequisite tasks (by TASK-NN number)
3. **Do NOT start a task until all dependencies are COMPLETE**
4. **Complete means**: All code written, `cargo test` passes, `cargo check` passes
5. **Execute tasks in order**: TASK-01, TASK-02, ..., TASK-42

### Naming Convention
- Tasks are numbered TASK-01 through TASK-42
- Original IDs (e.g., TASK-ARCH-001) are for historical reference only
- **Always use TASK-NN format** when referencing dependencies

### Key Concepts You Need to Know
| Concept | Description |
|---------|-------------|
| **GWT** | Global Workspace Theory - consciousness model |
| **Kuramoto** | Coupled oscillator network (13 oscillators) for coherence |
| **IC** | Identity Continuity - a metric < 0.5 triggers "dream" consolidation |
| **MCP** | Model Context Protocol - JSON-RPC 2.0 API for AI tools |
| **Dream** | Memory consolidation process triggered by low IC or idle GPU |
| **NVML** | NVIDIA Management Library - GPU monitoring |
| **FAISS** | Facebook AI Similarity Search - vector indexing |
| **FFI** | Foreign Function Interface - Rust binding to C libraries |

### Project Structure
```
crates/
├── context-graph-core/     # Core types, GWT, dream, identity
├── context-graph-cuda/     # GPU FFI (CUDA, FAISS) - created in TASK-01
├── context-graph-mcp/      # MCP server, tools, SSE transport
├── context-graph-embeddings/ # Vector embeddings, token pruning
├── context-graph-utl/      # Uncertainty Theory Logic (Johari)
└── context-graph-graph/    # Graph operations
```

### Verification After Each Task
```bash
# Check compilation
cargo check -p <crate-name>

# Run tests for the crate
cargo test -p <crate-name>

# Run all tests
cargo test --workspace
```

---

## ⚠️ MANDATORY EXECUTION ORDER

**EXECUTE TASKS IN THE EXACT ORDER BELOW. NO EXCEPTIONS.**

Tasks are numbered 1-42. You MUST complete each task before proceeding to the next.
A task is "complete" when all its acceptance criteria pass and tests are green.

---

## SEQUENTIAL EXECUTION ORDER (1-42)

### TASK-01: Create context-graph-cuda crate skeleton
**Original ID**: TASK-ARCH-001
**Layer**: Foundation | **Phase**: 1
**Est. Hours**: 2
**Dependencies**: NONE
**Blocks**: TASK-02, TASK-03

---

### TASK-02: Consolidate CUDA driver FFI bindings
**Original ID**: TASK-ARCH-002
**Layer**: Foundation | **Phase**: 1
**Est. Hours**: 4
**Dependencies**: TASK-01
**Blocks**: TASK-04

---

### TASK-03: Consolidate FAISS FFI bindings
**Original ID**: TASK-ARCH-003
**Layer**: Foundation | **Phase**: 1
**Est. Hours**: 4
**Dependencies**: TASK-01
**Blocks**: TASK-04

---

### TASK-04: Implement safe GpuDevice RAII wrapper
**Original ID**: TASK-ARCH-004
**Layer**: Foundation | **Phase**: 1
**Est. Hours**: 3
**Dependencies**: TASK-02, TASK-03
**Blocks**: TASK-05, TASK-13

---

### TASK-05: Add CI gate for FFI consolidation
**Original ID**: TASK-ARCH-005
**Layer**: Foundation | **Phase**: 1
**Est. Hours**: 1
**Dependencies**: TASK-04
**Blocks**: None

---

### TASK-06: Add async-trait to MCP crate
**Original ID**: TASK-PERF-001
**Layer**: Foundation | **Phase**: 1
**Est. Hours**: 0.5
**Dependencies**: NONE
**Blocks**: TASK-07, TASK-08

---

### TASK-07: Convert WorkspaceProvider to async
**Original ID**: TASK-PERF-002
**Layer**: Foundation | **Phase**: 1
**Est. Hours**: 2
**Dependencies**: TASK-06
**Blocks**: TASK-16

---

### TASK-08: Convert MetaCognitiveProvider to async
**Original ID**: TASK-PERF-003
**Layer**: Foundation | **Phase**: 1
**Est. Hours**: 1.5
**Dependencies**: TASK-06
**Blocks**: None

---

### TASK-09: Fix Johari Blind/Unknown action mapping
**Original ID**: TASK-UTL-001
**Layer**: Foundation | **Phase**: 1
**Est. Hours**: 1
**Dependencies**: NONE
**Blocks**: TASK-31

---

### TASK-10: Add KURAMOTO_N constant (13 oscillators)
**Original ID**: TASK-GWT-001
**Layer**: Core Logic | **Phase**: 2
**Est. Hours**: 1
**Dependencies**: NONE
**Blocks**: TASK-11

---

### TASK-11: Implement KuramotoNetwork with 13 frequencies
**Original ID**: TASK-GWT-002
**Layer**: Core Logic | **Phase**: 2
**Est. Hours**: 3
**Dependencies**: TASK-10
**Blocks**: TASK-12

---

### TASK-12: Implement KuramotoStepper lifecycle
**Original ID**: TASK-GWT-003
**Layer**: Core Logic | **Phase**: 2
**Est. Hours**: 4
**Dependencies**: TASK-11
**Blocks**: TASK-25, TASK-34, TASK-39, TASK-40

---

### TASK-13: Implement Green Contexts auto-enable
**Original ID**: TASK-EMBED-001
**Layer**: Core Logic | **Phase**: 2
**Est. Hours**: 4
**Dependencies**: TASK-04
**Blocks**: None

---

### TASK-14: Implement TokenPruningConfig types
**Original ID**: TASK-EMBED-002
**Layer**: Core Logic | **Phase**: 2
**Est. Hours**: 2
**Dependencies**: NONE
**Blocks**: TASK-15

---

### TASK-15: Implement TokenPruningQuantizer
**Original ID**: TASK-EMBED-003
**Layer**: Core Logic | **Phase**: 2
**Est. Hours**: 4
**Dependencies**: TASK-14
**Blocks**: None

---

### TASK-16: Remove block_on from gwt_providers
**Original ID**: TASK-PERF-004
**Layer**: Core Logic | **Phase**: 2
**Est. Hours**: 2
**Dependencies**: TASK-07
**Blocks**: None

---

### TASK-17: Add parking_lot::RwLock to wake_controller
**Original ID**: TASK-PERF-005
**Layer**: Core Logic | **Phase**: 2
**Est. Hours**: 1
**Dependencies**: NONE
**Blocks**: None

---

### TASK-18: Pre-allocate HashMap capacity in hot paths
**Original ID**: TASK-PERF-006
**Layer**: Core Logic | **Phase**: 2
**Est. Hours**: 1
**Dependencies**: NONE
**Blocks**: None

---

### TASK-19: Add IdentityCritical variant to ExtendedTriggerReason
**Original ID**: TASK-IDENTITY-001
**Layer**: Integration | **Phase**: 3
**Est. Hours**: 1
**Dependencies**: NONE
**Blocks**: TASK-20

---

### TASK-20: Implement TriggerConfig with IC threshold
**Original ID**: TASK-IDENTITY-002
**Layer**: Integration | **Phase**: 3
**Est. Hours**: 1.5
**Dependencies**: TASK-19
**Blocks**: TASK-21

---

### TASK-21: Implement TriggerManager IC checking
**Original ID**: TASK-IDENTITY-003
**Layer**: Integration | **Phase**: 3
**Est. Hours**: 3
**Dependencies**: TASK-20
**Blocks**: TASK-24, TASK-38

---

### TASK-22: Implement GpuMonitor trait and error types
**Original ID**: TASK-DREAM-001
**Layer**: Integration | **Phase**: 3
**Est. Hours**: 2
**Dependencies**: NONE
**Blocks**: TASK-23

---

### TASK-23: Implement NvmlGpuMonitor with thresholds
**Original ID**: TASK-DREAM-002
**Layer**: Integration | **Phase**: 3
**Est. Hours**: 4
**Dependencies**: TASK-22
**Blocks**: TASK-24, TASK-37

---

### TASK-24: Wire DreamEventListener to TriggerManager
**Original ID**: TASK-DREAM-003
**Layer**: Integration | **Phase**: 3
**Est. Hours**: 3
**Dependencies**: TASK-21, TASK-23
**Blocks**: TASK-35

---

### TASK-25: Integrate KuramotoStepper with MCP server
**Original ID**: TASK-DREAM-004
**Layer**: Integration | **Phase**: 3
**Est. Hours**: 3
**Dependencies**: TASK-12
**Blocks**: TASK-26

---

### TASK-26: Wire IC monitor to emit IdentityCritical events
**Original ID**: TASK-DREAM-005
**Layer**: Integration | **Phase**: 3
**Est. Hours**: 2
**Dependencies**: TASK-25
**Blocks**: None

---

### TASK-27: Implement epistemic_action tool schema
**Original ID**: TASK-MCP-001
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 2
**Dependencies**: NONE
**Blocks**: TASK-28

---

### TASK-28: Implement epistemic_action handler
**Original ID**: TASK-MCP-002
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 4
**Dependencies**: TASK-27
**Blocks**: TASK-41

---

### TASK-29: Implement merge_concepts tool schema
**Original ID**: TASK-MCP-003
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 2
**Dependencies**: NONE
**Blocks**: TASK-30

---

### TASK-30: Implement merge_concepts handler
**Original ID**: TASK-MCP-004
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 6
**Dependencies**: TASK-29
**Blocks**: TASK-41

---

### TASK-31: Implement get_johari_classification tool
**Original ID**: TASK-MCP-005
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 3
**Dependencies**: TASK-09
**Blocks**: TASK-41

---

### TASK-32: Add SSE transport types
**Original ID**: TASK-MCP-006
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 2
**Dependencies**: NONE
**Blocks**: TASK-33

---

### TASK-33: Implement SSE handler with keep-alive
**Original ID**: TASK-MCP-007
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 4
**Dependencies**: TASK-32
**Blocks**: TASK-42

---

### TASK-34: Implement get_coherence_state tool
**Original ID**: TASK-MCP-008
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 3
**Dependencies**: TASK-12
**Blocks**: TASK-41

---

### TASK-35: Implement trigger_dream tool
**Original ID**: TASK-MCP-009
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 3
**Dependencies**: TASK-24
**Blocks**: TASK-41

---

### TASK-36: Add parameter validation middleware
**Original ID**: TASK-MCP-010
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 4
**Dependencies**: NONE
**Blocks**: TASK-41

---

### TASK-37: Implement get_gpu_status tool
**Original ID**: TASK-MCP-011
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 2
**Dependencies**: TASK-23
**Blocks**: TASK-41

---

### TASK-38: Implement get_identity_continuity tool
**Original ID**: TASK-MCP-012
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 2
**Dependencies**: TASK-21
**Blocks**: TASK-41

---

### TASK-39: Implement get_kuramoto_state tool
**Original ID**: TASK-MCP-013
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 2
**Dependencies**: TASK-12
**Blocks**: TASK-41

---

### TASK-40: Implement set_coupling_strength tool
**Original ID**: TASK-MCP-014
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 2
**Dependencies**: TASK-12
**Blocks**: TASK-41

---

### TASK-41: Add tool registration to MCP server
**Original ID**: TASK-MCP-015
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 3
**Dependencies**: TASK-28, TASK-30, TASK-31, TASK-34, TASK-35, TASK-36, TASK-37, TASK-38, TASK-39, TASK-40
**Blocks**: None

---

### TASK-42: SSE integration with MCP router
**Original ID**: TASK-MCP-016
**Layer**: Surface | **Phase**: 4
**Est. Hours**: 3
**Dependencies**: TASK-33
**Blocks**: None (FINAL TASK)

---

## QUICK REFERENCE: EXECUTION CHECKLIST

Copy this checklist and mark tasks as you complete them:

```
PHASE 1: FOUNDATION (Tasks 1-9)
[ ] TASK-01 - Create context-graph-cuda crate skeleton
[ ] TASK-02 - Consolidate CUDA driver FFI bindings
[ ] TASK-03 - Consolidate FAISS FFI bindings
[ ] TASK-04 - Implement safe GpuDevice RAII wrapper
[ ] TASK-05 - Add CI gate for FFI consolidation
[ ] TASK-06 - Add async-trait to MCP crate
[ ] TASK-07 - Convert WorkspaceProvider to async
[ ] TASK-08 - Convert MetaCognitiveProvider to async
[ ] TASK-09 - Fix Johari Blind/Unknown action mapping

PHASE 2: CORE LOGIC (Tasks 10-18)
[ ] TASK-10 - Add KURAMOTO_N constant (13 oscillators)
[ ] TASK-11 - Implement KuramotoNetwork with 13 frequencies
[ ] TASK-12 - Implement KuramotoStepper lifecycle
[ ] TASK-13 - Implement Green Contexts auto-enable
[ ] TASK-14 - Implement TokenPruningConfig types
[ ] TASK-15 - Implement TokenPruningQuantizer
[ ] TASK-16 - Remove block_on from gwt_providers
[ ] TASK-17 - Add parking_lot::RwLock to wake_controller
[ ] TASK-18 - Pre-allocate HashMap capacity in hot paths

PHASE 3: INTEGRATION (Tasks 19-26)
[ ] TASK-19 - Add IdentityCritical variant
[ ] TASK-20 - Implement TriggerConfig with IC threshold
[ ] TASK-21 - Implement TriggerManager IC checking
[ ] TASK-22 - Implement GpuMonitor trait and error types
[ ] TASK-23 - Implement NvmlGpuMonitor with thresholds
[ ] TASK-24 - Wire DreamEventListener to TriggerManager
[ ] TASK-25 - Integrate KuramotoStepper with MCP server
[ ] TASK-26 - Wire IC monitor to emit IdentityCritical events

PHASE 4: SURFACE/MCP (Tasks 27-42)
[ ] TASK-27 - Implement epistemic_action tool schema
[ ] TASK-28 - Implement epistemic_action handler
[ ] TASK-29 - Implement merge_concepts tool schema
[ ] TASK-30 - Implement merge_concepts handler
[ ] TASK-31 - Implement get_johari_classification tool
[ ] TASK-32 - Add SSE transport types
[ ] TASK-33 - Implement SSE handler with keep-alive
[ ] TASK-34 - Implement get_coherence_state tool
[ ] TASK-35 - Implement trigger_dream tool
[ ] TASK-36 - Add parameter validation middleware
[ ] TASK-37 - Implement get_gpu_status tool
[ ] TASK-38 - Implement get_identity_continuity tool
[ ] TASK-39 - Implement get_kuramoto_state tool
[ ] TASK-40 - Implement set_coupling_strength tool
[ ] TASK-41 - Add tool registration to MCP server
[ ] TASK-42 - SSE integration with MCP router
```

---

## DEPENDENCY RULES (ENFORCED)

**RULE 1**: You CANNOT start a task until ALL its dependencies are COMPLETE.

**RULE 2**: "Complete" means:
- All code is written
- All tests pass (`cargo test`)
- No compiler errors
- Task spec acceptance criteria verified

**RULE 3**: If a task fails, you MUST fix it before proceeding. Do NOT skip.

**RULE 4**: Execute tasks in numerical order: 1, 2, 3, ... 42.

---

## CRITICAL BLOCKING DEPENDENCIES

These tasks block multiple downstream tasks. Prioritize quality:

| Task | Blocks | Impact if Delayed |
|------|--------|-------------------|
| TASK-04 | TASK-05, TASK-13 | GPU features blocked |
| TASK-12 | TASK-25, 34, 39, 40 | Consciousness features blocked |
| TASK-21 | TASK-24, TASK-38 | IC monitoring blocked |
| TASK-24 | TASK-35 | Dream triggering blocked |
| TASK-41 | None | All tools (28,30,31,34-40) must be ready first |

---

## ESTIMATED TIME BY PHASE

| Phase | Tasks | Hours |
|-------|-------|-------|
| Phase 1: Foundation | 1-9 | 19h |
| Phase 2: Core Logic | 10-18 | 18h |
| Phase 3: Integration | 19-26 | 19.5h |
| Phase 4: Surface | 27-42 | 42h |
| **TOTAL** | **1-42** | **98.5h** |

---

## PARALLEL EXECUTION OPPORTUNITIES

**If you support parallel execution**, these task groups can run concurrently:

### Phase 1 Parallel Groups
| Group | Tasks | After |
|-------|-------|-------|
| ARCH-FFI | TASK-02, TASK-03 | TASK-01 complete |
| ASYNC | TASK-07, TASK-08 | TASK-06 complete |
| STANDALONE | TASK-09 | Anytime (no deps) |

### Phase 2 Parallel Groups
| Group | Tasks | After |
|-------|-------|-------|
| EMBED | TASK-14, TASK-15 | No deps / TASK-14 |
| PERF-OPT | TASK-17, TASK-18 | Anytime (no deps) |

### Phase 3 Parallel Groups
| Group | Tasks | After |
|-------|-------|-------|
| IDENTITY | TASK-19, TASK-20, TASK-21 | Sequential chain |
| DREAM-GPU | TASK-22, TASK-23 | Sequential chain |

### Phase 4 Parallel Groups
| Group | Tasks | After |
|-------|-------|-------|
| MCP-SCHEMAS | TASK-27, TASK-29 | Anytime (no deps) |
| SSE | TASK-32, TASK-33 | Sequential chain |
| KURAMOTO-TOOLS | TASK-34, TASK-39, TASK-40 | TASK-12 complete |
| MCP-VALIDATION | TASK-36 | Anytime (no deps) |

**IMPORTANT**: TASK-41 waits for ALL MCP tools (28, 30, 31, 34-40) to complete.

---

## NAMING CROSS-REFERENCE

**⚠️ IMPORTANT**: Original IDs (e.g., TASK-ARCH-001) appear in some documentation for historical context.
**ALWAYS USE THE NEW ID (TASK-NN) WHEN REFERENCING TASKS.**

| New ID | Original ID | Domain |
|--------|-------------|--------|
| TASK-01 | TASK-ARCH-001 | Architecture |
| TASK-02 | TASK-ARCH-002 | Architecture |
| TASK-03 | TASK-ARCH-003 | Architecture |
| TASK-04 | TASK-ARCH-004 | Architecture |
| TASK-05 | TASK-ARCH-005 | Architecture |
| TASK-06 | TASK-PERF-001 | Performance |
| TASK-07 | TASK-PERF-002 | Performance |
| TASK-08 | TASK-PERF-003 | Performance |
| TASK-09 | TASK-UTL-001 | UTL |
| TASK-10 | TASK-GWT-001 | GWT |
| TASK-11 | TASK-GWT-002 | GWT |
| TASK-12 | TASK-GWT-003 | GWT |
| TASK-13 | TASK-EMBED-001 | Embeddings |
| TASK-14 | TASK-EMBED-002 | Embeddings |
| TASK-15 | TASK-EMBED-003 | Embeddings |
| TASK-16 | TASK-PERF-004 | Performance |
| TASK-17 | TASK-PERF-005 | Performance |
| TASK-18 | TASK-PERF-006 | Performance |
| TASK-19 | TASK-IDENTITY-001 | Identity |
| TASK-20 | TASK-IDENTITY-002 | Identity |
| TASK-21 | TASK-IDENTITY-003 | Identity |
| TASK-22 | TASK-DREAM-001 | Dream |
| TASK-23 | TASK-DREAM-002 | Dream |
| TASK-24 | TASK-DREAM-003 | Dream |
| TASK-25 | TASK-DREAM-004 | Dream |
| TASK-26 | TASK-DREAM-005 | Dream |
| TASK-27 | TASK-MCP-001 | MCP |
| TASK-28 | TASK-MCP-002 | MCP |
| TASK-29 | TASK-MCP-003 | MCP |
| TASK-30 | TASK-MCP-004 | MCP |
| TASK-31 | TASK-MCP-005 | MCP |
| TASK-32 | TASK-MCP-006 | MCP |
| TASK-33 | TASK-MCP-007 | MCP |
| TASK-34 | TASK-MCP-008 | MCP |
| TASK-35 | TASK-MCP-009 | MCP |
| TASK-36 | TASK-MCP-010 | MCP |
| TASK-37 | TASK-MCP-011 | MCP |
| TASK-38 | TASK-MCP-012 | MCP |
| TASK-39 | TASK-MCP-013 | MCP |
| TASK-40 | TASK-MCP-014 | MCP |
| TASK-41 | TASK-MCP-015 | MCP |
| TASK-42 | TASK-MCP-016 | MCP |

---

## ISSUE TO TASK MAPPING

| Issue | Severity | Tasks |
|-------|----------|-------|
| ISS-001 | CRITICAL | 10, 11 |
| ISS-002 | CRITICAL | 21, 24, 26 |
| ISS-003 | CRITICAL | 12, 25 |
| ISS-004 | CRITICAL | 6, 7, 8, 16 |
| ISS-005 | CRITICAL | 1, 2, 3, 4, 5 |
| ISS-006 | HIGH | 27-42 (all MCP) |
| ISS-007 | HIGH | 22, 23 |
| ISS-008 | HIGH | 13 |
| ISS-009 | HIGH | 14, 15 |
| ISS-010 | HIGH | 19, 20 |
| ISS-011 | MEDIUM | 9 |
| ISS-012 | MEDIUM | 36 |
| ISS-013 | MEDIUM | 32, 33, 42 |
| ISS-014 | MEDIUM | 23 |
| ISS-015 | LOW | 17 |
| ISS-016 | LOW | 18 |

---

*Version 3.0.0 - Generated 2026-01-12 - SIMPLIFIED SEQUENTIAL NAMING*
