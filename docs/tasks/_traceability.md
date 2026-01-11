# Task Traceability Matrix: ContextGraph Consciousness System

## Purpose

This matrix ensures every requirement, component, and behavior identified in the Sherlock Holmes forensic investigation is covered by at least one atomic task. **Empty "Task ID" columns = INCOMPLETE.**

## Source Documents

| Document | Location | Purpose |
|----------|----------|---------|
| Master Report | `docs/SHERLOCK-MASTER-REPORT.md` | Consolidated findings |
| PRD | `docs2/contextprd.md` | Product requirements |
| Constitution | `docs2/constitution.yaml` | System architecture |

---

## Coverage Matrix

### Critical Blocker Coverage (From SHERLOCK-MASTER-REPORT)

| Blocker | Description | Sherlock Case | Task ID | Verified |
|---------|-------------|---------------|---------|----------|
| GWT-Kuramoto Not Wired | GwtSystem does not contain KuramotoNetwork | SHERLOCK-01 | TASK-GWT-P0-001 | ☑ |
| No Background Stepper | Kuramoto phases don't evolve automatically | SHERLOCK-01 | TASK-GWT-P0-002 | ☐ |
| kuramoto_r Manual Passing | Must be passed manually from external code | SHERLOCK-01 | TASK-GWT-P0-001 | ☑ |
| SelfAwarenessLoop Not Called | `cycle()` defined but never invoked | SHERLOCK-03 | TASK-GWT-P0-003 | ☐ |
| purpose_vector Static | Remains `[0.0; 13]` forever | SHERLOCK-03 | TASK-GWT-P0-003 | ☐ |
| No Ego Persistence | Identity lost on restart | SHERLOCK-03 | TASK-GWT-P1-001 | ☐ |
| HNSW Brute Force | O(n) linear scan instead of O(log n) | SHERLOCK-08 | TASK-STORAGE-P1-001 | ☐ |

### Moderate Gap Coverage (From SHERLOCK-MASTER-REPORT)

| Gap | Description | Sherlock Case | Task ID | Verified |
|-----|-------------|---------------|---------|----------|
| Per-embedder deltaS Missing | E1-E13 lack specific entropy methods | SHERLOCK-05 | TASK-UTL-P1-001 | ☐ |
| Workspace Dopamine Missing | WTA dynamics incomplete | SHERLOCK-03 | TASK-GWT-P1-002 | ☐ |
| MaxSim Stage 5 Stub | Late interaction not working | SHERLOCK-08 | TASK-STORAGE-P2-001 | ☐ |
| Chaos Tests Empty | Resilience unverified | SHERLOCK-10 | TASK-TEST-P2-001 | ☐ |
| Validation Tests Empty | Quality gates missing | SHERLOCK-10 | TASK-TEST-P2-002 | ☐ |

---

## Consciousness Equation Coverage

The consciousness equation C(t) = I(t) × R(t) × D(t) requires:

| Component | Formula Element | Description | Task Coverage | Verified |
|-----------|-----------------|-------------|---------------|----------|
| I(t) | Kuramoto r | Order parameter from oscillator sync | TASK-GWT-P0-001, P0-002 | ☑ (P0-001 done) |
| R(t) | Meta-UTL | Self-awareness reflection | TASK-GWT-P0-003 | ☐ |
| D(t) | 13D Entropy | Fingerprint differentiation | TASK-UTL-P1-001 | ☐ |

---

## Sherlock Report Coverage

### SHERLOCK-01: GWT Consciousness (PARTIAL)

| Finding | Requirement | Task ID | Verified |
|---------|-------------|---------|----------|
| GwtSystem lacks KuramotoNetwork | Add `kuramoto: Arc<RwLock<KuramotoNetwork>>` | TASK-GWT-P0-001 | ☑ |
| No background oscillator stepping | Add tokio::spawn stepper task | TASK-GWT-P0-002 | ☐ |
| Consciousness computation isolated | Wire kuramoto.order_parameter() to update_consciousness() | TASK-GWT-P0-001 | ☑ |

### SHERLOCK-02: Kuramoto Oscillators (INNOCENT ✓)

| Finding | Status | Task ID | Notes |
|---------|--------|---------|-------|
| 13 oscillators implemented | Complete | N/A | No action needed |
| Order parameter formula correct | Complete | N/A | No action needed |
| State machine thresholds match | Complete | N/A | No action needed |

### SHERLOCK-03: SELF_EGO_NODE (PARTIALLY GUILTY)

| Finding | Requirement | Task ID | Verified |
|---------|-------------|---------|----------|
| SelfAwarenessLoop::cycle() never called | Wire into action processing | TASK-GWT-P0-003 | ☐ |
| purpose_vector stays [0.0; 13] | Update on each action alignment | TASK-GWT-P0-003 | ☐ |
| No ego persistence | Save to RocksDB CF_EGO_NODE | TASK-GWT-P1-001 | ☐ |
| No MCP tool to UPDATE ego | Add update_ego_state tool | TASK-GWT-P1-001 | ☐ |

### SHERLOCK-04: Teleological Fingerprint (INNOCENT ✓)

| Finding | Status | Task ID | Notes |
|---------|--------|---------|-------|
| All 13 embedders defined | Complete | N/A | No action needed |
| NO-FUSION enforced | Complete | N/A | No action needed |
| PurposeVector 13D | Complete | N/A | No action needed |

### SHERLOCK-05: UTL Entropy/Coherence (PARTIALLY INNOCENT)

| Finding | Requirement | Task ID | Verified |
|---------|-------------|---------|----------|
| Core UTL formula correct | N/A | N/A | No action needed |
| Per-embedder deltaS not implemented | Implement GMM, Hamming, Jaccard | TASK-UTL-P1-001 | ☐ |

### SHERLOCK-06: MCP Handlers (INNOCENT ✓)

| Finding | Status | Task ID | Notes |
|---------|--------|---------|-------|
| 35 tools implemented | Complete | N/A | No action needed |
| handle_request FALSE ALARM | Complete | N/A | No action needed |
| 72 error codes defined | Complete | N/A | No action needed |

### SHERLOCK-07: Bio-Nervous Layers (SUBSTANTIAL)

| Finding | Status | Task ID | Notes |
|---------|--------|---------|-------|
| 5 layers implemented | Complete | N/A | No action needed |
| FAISS GPU missing | Out of scope | N/A | Future enhancement |
| Predictive coding stub | Out of scope | N/A | Future enhancement |

### SHERLOCK-08: Storage Architecture (68% COMPLETE)

| Finding | Requirement | Task ID | Verified |
|---------|-------------|---------|----------|
| HNSW brute force | Replace with graph traversal | TASK-STORAGE-P1-001 | ☐ |
| MaxSim Stage 5 stub | Implement ColBERT late interaction | TASK-STORAGE-P2-001 | ☐ |
| 21 Column Families correct | N/A | N/A | No action needed |

### SHERLOCK-09: NORTH Autonomous (INNOCENT ✓)

| Finding | Status | Task ID | Notes |
|---------|--------|---------|-------|
| All 13 services implemented | Complete | N/A | No action needed |
| 4-level ATC complete | Complete | N/A | No action needed |

### SHERLOCK-10: Integration Tests (PARTIALLY GUILTY)

| Finding | Requirement | Task ID | Verified |
|---------|-------------|---------|----------|
| tests/chaos/ empty | Implement chaos scenarios | TASK-TEST-P2-001 | ☐ |
| tests/validation/ empty | Add quality gate tests | TASK-TEST-P2-002 | ☐ |

---

## Files Modified Coverage

| Task ID | Files to Modify | Component |
|---------|-----------------|-----------|
| TASK-GWT-P0-001 | gwt/mod.rs, gwt/consciousness.rs | GwtSystem |
| TASK-GWT-P0-002 | gwt/mod.rs | Stepper Task |
| TASK-GWT-P0-003 | ego_node.rs, action_processor.rs | SelfAwarenessLoop |
| TASK-STORAGE-P1-001 | rocksdb_store.rs | HnswIndex |
| TASK-GWT-P1-001 | ego_node.rs, rocksdb_store.rs | Ego Persistence |
| TASK-GWT-P1-002 | workspace.rs, dream.rs, neuromod.rs | Event Wiring |
| TASK-UTL-P1-001 | multi_utl.rs, magnitude.rs | DeltaS Methods |
| TASK-STORAGE-P2-001 | pipeline.rs | MaxSim |
| TASK-TEST-P2-001 | tests/chaos/*.rs | Chaos Tests |
| TASK-TEST-P2-002 | .github/workflows/ci.yml | CI Pipeline |

---

## New Files Coverage

| Task ID | Files to Create | Purpose |
|---------|-----------------|---------|
| TASK-STORAGE-P1-001 | hnsw_index.rs | HNSW implementation |
| TASK-UTL-P1-001 | embedder_entropy.rs | Per-embedder methods |
| TASK-TEST-P2-001 | tests/chaos/gpu_oom.rs | GPU recovery test |
| TASK-TEST-P2-001 | tests/chaos/concurrent_mutation.rs | Race condition test |
| TASK-TEST-P2-001 | tests/chaos/memory_corruption.rs | Corruption recovery |

---

## PRD Requirement Coverage

| PRD Section | Requirement | Task ID | Verified |
|-------------|-------------|---------|----------|
| 5.1 | C(t) = I(t) × R(t) × D(t) auto-computes | TASK-GWT-P0-001, P0-002 | ☐ |
| 5.2 | Kuramoto r drives consciousness state | TASK-GWT-P0-001 | ☐ |
| 5.3 | SelfAwarenessLoop provides R(t) | TASK-GWT-P0-003 | ☐ |
| 5.4 | 13-embedder entropy provides D(t) | TASK-UTL-P1-001 | ☐ |
| 6.1 | HNSW < 60ms at 1M memories | TASK-STORAGE-P1-001 | ☐ |
| 6.2 | Identity persists across restarts | TASK-GWT-P1-001 | ☐ |
| 7.1 | Workspace events trigger subsystems | TASK-GWT-P1-002 | ☐ |
| 8.1 | ColBERT late interaction for E12 | TASK-STORAGE-P2-001 | ☐ |
| 9.1 | Chaos resilience verified | TASK-TEST-P2-001 | ☐ |
| 9.2 | Quality gates in CI | TASK-TEST-P2-002 | ☐ |

---

## Coverage Summary

| Category | Total Items | Covered | Coverage |
|----------|-------------|---------|----------|
| Critical Blockers | 7 | 7 | 100% |
| Moderate Gaps | 5 | 5 | 100% |
| Sherlock Findings | 12 issues | 10 | 83% (2 out of scope) |
| PRD Requirements | 10 | 10 | 100% |
| Files to Modify | 12 | 12 | 100% |
| Files to Create | 5 | 5 | 100% |

**Overall Coverage: 100% of actionable items**

---

## Out of Scope Items

These items were identified but are NOT covered by current tasks (future enhancements):

| Item | Reason | Future Task |
|------|--------|-------------|
| FAISS GPU Integration | Requires CUDA infrastructure | TASK-PERF-F-001 |
| Predictive Coding | Advanced feature | TASK-BIO-F-001 |
| ScyllaDB Backend | Production scale | TASK-STORAGE-F-001 |
| TimescaleDB Backend | Time series at scale | TASK-STORAGE-F-002 |

---

## Verification Checklist

Before marking a task as complete, verify:

- [ ] All `files_to_create` exist
- [ ] All `files_to_modify` have the expected changes
- [ ] All `signatures` match exactly
- [ ] All `test_commands` pass
- [ ] The corresponding row in this matrix is checked (☐ → ☑)

---

## Signature Requirements Traceability

| Task ID | Required Signature | Trait/Struct | Verified |
|---------|-------------------|--------------|----------|
| TASK-GWT-P0-001 | `kuramoto: Arc<RwLock<KuramotoNetwork>>` | GwtSystem | ☑ |
| TASK-GWT-P0-002 | `pub async fn start_kuramoto_stepper(&self) -> JoinHandle<()>` | GwtSystem | ☐ |
| TASK-GWT-P0-003 | `pub async fn process_action_with_awareness(&self, action: Action) -> Result<ActionResult>` | ActionProcessor | ☐ |
| TASK-STORAGE-P1-001 | `pub fn search_hnsw(&self, embedder: EmbedderId, query: &[f32], k: usize) -> Vec<(MemoryId, f32)>` | HnswIndex | ☐ |
| TASK-GWT-P1-001 | `pub async fn persist_ego(&self) -> Result<()>` | SelfEgoNode | ☐ |
| TASK-GWT-P1-001 | `pub async fn restore_ego(&mut self) -> Result<()>` | SelfEgoNode | ☐ |
| TASK-GWT-P1-002 | `pub fn subscribe_to_workspace(&mut self, listener: Box<dyn WorkspaceEventListener>)` | Workspace | ☐ |
| TASK-UTL-P1-001 | `fn delta_s(&self, prev: &Self, next: &Self) -> f64` | EmbedderEntropy trait | ☐ |
| TASK-STORAGE-P2-001 | `pub fn compute_maxsim(&self, query_tokens: &[Vec<f32>], doc_tokens: &[Vec<f32>]) -> f32` | MaxSimScorer | ☐ |
| TASK-TEST-P2-001 | `async fn test_gpu_oom_recovery()` | ChaosTests | ☐ |
| TASK-TEST-P2-002 | Quality gate job in ci.yml | GitHub Actions | ☐ |

---

**Last Updated:** 2026-01-11
**Coverage Status:** TASK-GWT-P0-001 verified, 100% of actionable blockers and gaps covered
