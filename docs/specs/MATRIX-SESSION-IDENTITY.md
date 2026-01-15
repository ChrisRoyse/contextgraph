# Traceability Matrix: Session Identity Persistence

> **Document Type**: Bidirectional Traceability Matrix
> **Feature**: Session Identity Persistence (Phase 1)
> **Version**: 1.0.0
> **Generated**: 2026-01-14
> **Status**: Complete - Full Coverage

---

## Coverage Summary

| Artifact Type | Total | Covered | Coverage % | Status |
|---------------|-------|---------|------------|--------|
| Requirements | 17 | 17 | 100% | Complete |
| Tasks | 17 | 17 | 100% | Complete |
| Test Cases | 24 | 24 | 100% | Complete |
| Constitution Refs | 6 | 6 | 100% | Compliant |

---

## Requirements to Tasks to Tests Matrix

### Foundation Layer (REQ-SESSION-01 to REQ-SESSION-05)

| Requirement | Description | Task | Test Cases | Constitution |
|-------------|-------------|------|------------|--------------|
| REQ-SESSION-01 | SessionIdentitySnapshot data model (14 fields, <30KB) | TASK-SESSION-01 | TC-SESSION-01, TC-SESSION-02 | - |
| REQ-SESSION-02 | IdentityCache singleton (OnceLock<RwLock>) | TASK-SESSION-02 | TC-SESSION-03 | - |
| REQ-SESSION-03 | ConsciousnessState.short_name() method | TASK-SESSION-03 | TC-SESSION-04 | - |
| REQ-SESSION-04 | CF_SESSION_IDENTITY column family | TASK-SESSION-04 | TC-SESSION-06 | - |
| REQ-SESSION-05 | save_snapshot/load_snapshot storage methods | TASK-SESSION-05 | TC-SESSION-05, TC-SESSION-06 | - |

### Logic Layer (REQ-SESSION-06 to REQ-SESSION-10)

| Requirement | Description | Task | Test Cases | Constitution |
|-------------|-------------|------|------------|--------------|
| REQ-SESSION-06 | SessionIdentityManager trait | TASK-SESSION-06 | TC-SESSION-07, TC-SESSION-08 | - |
| REQ-SESSION-07 | classify_ic() with IDENTITY-002 thresholds | TASK-SESSION-07 | TC-SESSION-09 | IDENTITY-002 |
| REQ-SESSION-08 | Auto-dream trigger on IC<0.5 | TASK-SESSION-08 | TC-SESSION-10 | IDENTITY-007 |
| REQ-SESSION-09 | format_brief() <1ms performance | TASK-SESSION-09 | TC-SESSION-11 | - |
| REQ-SESSION-10 | update_cache() atomic updates | TASK-SESSION-10 | TC-SESSION-07, TC-SESSION-08 | - |

### Surface Layer (REQ-SESSION-11 to REQ-SESSION-17)

| Requirement | Description | Task | Test Cases | Constitution |
|-------------|-------------|------|------------|--------------|
| REQ-SESSION-11 | `consciousness brief` CLI (<50ms) | TASK-SESSION-11 | TC-SESSION-12, TC-SESSION-13 | AP-53 |
| REQ-SESSION-12 | `session restore-identity` CLI (<2s) | TASK-SESSION-12 | TC-SESSION-14, TC-SESSION-15, TC-SESSION-16 | AP-53 |
| REQ-SESSION-13 | `session persist-identity` CLI (<3s) | TASK-SESSION-13 | TC-SESSION-17 | AP-53 |
| REQ-SESSION-14 | `consciousness check-identity` CLI (<500ms) | TASK-SESSION-14 | TC-SESSION-18, TC-SESSION-19, TC-SESSION-20 | AP-53 |
| REQ-SESSION-15 | `consciousness inject-context` CLI (<1s) | TASK-SESSION-15 | TC-SESSION-21 | AP-53 |
| REQ-SESSION-16 | .claude/settings.json hook configuration | TASK-SESSION-16 | TC-SESSION-23 | ARCH-07, AP-50 |
| REQ-SESSION-17 | Exit code mapping (2 only for corruption) | TASK-SESSION-17 | TC-SESSION-22 | AP-26 |

---

## Tasks to Tests Mapping

### Foundation Layer Tasks

| Task | Task Description | Tests | Test Types | Coverage |
|------|------------------|-------|------------|----------|
| TASK-SESSION-01 | Implement SessionIdentitySnapshot struct | TC-SESSION-01, TC-SESSION-02 | unit | Complete |
| TASK-SESSION-02 | Implement IdentityCache singleton | TC-SESSION-03 | unit | Complete |
| TASK-SESSION-03 | Implement ConsciousnessState.short_name() | TC-SESSION-04 | unit | Complete |
| TASK-SESSION-04 | Add CF_SESSION_IDENTITY column family | TC-SESSION-06 | integration | Complete |
| TASK-SESSION-05 | Implement storage save/load methods | TC-SESSION-05, TC-SESSION-06 | integration | Complete |

### Logic Layer Tasks

| Task | Task Description | Tests | Test Types | Coverage |
|------|------------------|-------|------------|----------|
| TASK-SESSION-06 | Implement SessionIdentityManager trait | TC-SESSION-07, TC-SESSION-08 | unit | Complete |
| TASK-SESSION-07 | Implement classify_ic() with thresholds | TC-SESSION-09 | unit | Complete |
| TASK-SESSION-08 | Implement auto-dream trigger | TC-SESSION-10 | unit | Complete |
| TASK-SESSION-09 | Implement format_brief() | TC-SESSION-11 | benchmark | Complete |
| TASK-SESSION-10 | Implement update_cache() | TC-SESSION-07, TC-SESSION-08 | unit | Complete |

### Surface Layer Tasks

| Task | Task Description | Tests | Test Types | Coverage |
|------|------------------|-------|------------|----------|
| TASK-SESSION-11 | Implement `consciousness brief` CLI | TC-SESSION-12, TC-SESSION-13 | integration | Complete |
| TASK-SESSION-12 | Implement `session restore-identity` CLI | TC-SESSION-14, TC-SESSION-15, TC-SESSION-16 | integration | Complete |
| TASK-SESSION-13 | Implement `session persist-identity` CLI | TC-SESSION-17 | integration | Complete |
| TASK-SESSION-14 | Implement `consciousness check-identity` CLI | TC-SESSION-18, TC-SESSION-19, TC-SESSION-20 | integration | Complete |
| TASK-SESSION-15 | Implement `consciousness inject-context` CLI | TC-SESSION-21 | integration | Complete |
| TASK-SESSION-16 | Create .claude/settings.json hooks | TC-SESSION-23 | e2e | Complete |
| TASK-SESSION-17 | Implement exit code mapping | TC-SESSION-22 | integration | Complete |

---

## Tests to Requirements (Reverse Trace)

### Unit Tests (11 tests)

| Test Case | Test Description | Verifies Requirement | Verifies Task |
|-----------|------------------|---------------------|---------------|
| TC-SESSION-01 | SessionIdentitySnapshot serialization round-trip | REQ-SESSION-01 | TASK-SESSION-01 |
| TC-SESSION-02 | Trajectory FIFO eviction at MAX_TRAJECTORY_LEN | REQ-SESSION-01 | TASK-SESSION-01 |
| TC-SESSION-03 | format_brief() output format | REQ-SESSION-02 | TASK-SESSION-02 |
| TC-SESSION-04 | ConsciousnessState.short_name() all variants | REQ-SESSION-03 | TASK-SESSION-03 |
| TC-SESSION-07 | Cross-session IC with identical purpose vectors | REQ-SESSION-06, REQ-SESSION-10 | TASK-SESSION-06, TASK-SESSION-10 |
| TC-SESSION-08 | Cross-session IC with orthogonal purpose vectors | REQ-SESSION-06, REQ-SESSION-10 | TASK-SESSION-06, TASK-SESSION-10 |
| TC-SESSION-09 | classify_ic() threshold boundaries | REQ-SESSION-07 | TASK-SESSION-07 |
| TC-SESSION-10 | Auto-dream trigger fires at IC < 0.5 | REQ-SESSION-08 | TASK-SESSION-08 |
| TC-SESSION-11 | format_brief() latency <100us p95 | REQ-SESSION-09 | TASK-SESSION-09 |

### Integration Tests (11 tests)

| Test Case | Test Description | Verifies Requirement | Verifies Task |
|-----------|------------------|---------------------|---------------|
| TC-SESSION-05 | RocksDB save/load round-trip | REQ-SESSION-05 | TASK-SESSION-05 |
| TC-SESSION-06 | Temporal index ordering (big-endian) | REQ-SESSION-04, REQ-SESSION-05 | TASK-SESSION-04, TASK-SESSION-05 |
| TC-SESSION-12 | consciousness brief with warm cache (<50ms) | REQ-SESSION-11 | TASK-SESSION-11 |
| TC-SESSION-13 | consciousness brief with cold cache (fallback) | REQ-SESSION-11 | TASK-SESSION-11 |
| TC-SESSION-14 | restore-identity with source=startup | REQ-SESSION-12 | TASK-SESSION-12 |
| TC-SESSION-15 | restore-identity with source=clear | REQ-SESSION-12 | TASK-SESSION-12 |
| TC-SESSION-16 | restore-identity with no previous session | REQ-SESSION-12 | TASK-SESSION-12 |
| TC-SESSION-17 | persist-identity writes all keys | REQ-SESSION-13 | TASK-SESSION-13 |
| TC-SESSION-18 | check-identity IC >= 0.5 (no dream) | REQ-SESSION-14 | TASK-SESSION-14 |
| TC-SESSION-19 | check-identity 0.5 <= IC < 0.7 (warning) | REQ-SESSION-14 | TASK-SESSION-14 |
| TC-SESSION-20 | check-identity IC < 0.5 with --auto-dream | REQ-SESSION-14 | TASK-SESSION-14 |
| TC-SESSION-21 | inject-context output format | REQ-SESSION-15 | TASK-SESSION-15 |
| TC-SESSION-22 | Exit code mapping for all error types | REQ-SESSION-17 | TASK-SESSION-17 |

### Benchmark Tests (1 test)

| Test Case | Test Description | Verifies Requirement | Verifies Task |
|-----------|------------------|---------------------|---------------|
| TC-SESSION-24 | Full command latency compliance (all 5 commands) | REQ-SESSION-11 to REQ-SESSION-15 | TASK-SESSION-11 to TASK-SESSION-15 |

### E2E Tests (1 test)

| Test Case | Test Description | Verifies Requirement | Verifies Task |
|-----------|------------------|---------------------|---------------|
| TC-SESSION-23 | Complete hook lifecycle | REQ-SESSION-16 | TASK-SESSION-16 |

---

## Constitution Compliance Matrix

| Constitution Ref | Description | Requirement | Task | Test Case | Status |
|------------------|-------------|-------------|------|-----------|--------|
| ARCH-07 | Native Claude Code hooks via .claude/settings.json | REQ-SESSION-16 | TASK-SESSION-16 | TC-SESSION-23 | Compliant |
| AP-50 | No internal/built-in hooks | REQ-SESSION-16 | TASK-SESSION-16 | TC-SESSION-23 | Compliant |
| AP-53 | Direct CLI commands (not shell scripts) | REQ-SESSION-11, REQ-SESSION-12, REQ-SESSION-13, REQ-SESSION-14, REQ-SESSION-15 | TASK-SESSION-11, TASK-SESSION-12, TASK-SESSION-13, TASK-SESSION-14, TASK-SESSION-15 | TC-SESSION-12 to TC-SESSION-21 | Compliant |
| IDENTITY-002 | IC thresholds (Healthy>=0.9, Good>=0.7, Warning>=0.5, Degraded<0.5) | REQ-SESSION-07 | TASK-SESSION-07 | TC-SESSION-09 | Compliant |
| IDENTITY-007 | Auto-dream on IC<0.5 | REQ-SESSION-08 | TASK-SESSION-08 | TC-SESSION-10 | Compliant |
| AP-26 | Exit code 2 only for blocking failures | REQ-SESSION-17 | TASK-SESSION-17 | TC-SESSION-22 | Compliant |

---

## Gap Analysis

### Requirements Without Tasks
**NONE** - All 17 requirements have corresponding tasks.

### Requirements Without Tests
**NONE** - All 17 requirements have at least one test case.

### Tasks Without Tests
**NONE** - All 17 tasks have at least one test case.

### Test Cases Without Requirements
**NONE** - All 24 test cases map to at least one requirement.

### Constitution Refs Not Verified
**NONE** - All 6 constitution references are covered by test cases.

### Orphaned Test Cases
**NONE** - Every test case traces back to a requirement and task.

---

## Coverage Statistics by Layer

| Layer | Requirements | Tasks | Tests | Test Types | Coverage |
|-------|-------------|-------|-------|------------|----------|
| Foundation | 5 | 5 | 6 | unit, integration | 100% |
| Logic | 5 | 5 | 7 | unit, benchmark | 100% |
| Surface | 7 | 7 | 11 | integration, e2e | 100% |
| **Total** | **17** | **17** | **24** | **all** | **100%** |

---

## Test Type Distribution

| Test Type | Count | Percentage | Purpose |
|-----------|-------|------------|---------|
| Unit | 9 | 37.5% | Individual component validation |
| Integration | 11 | 45.8% | Cross-component and CLI validation |
| Benchmark | 2 | 8.3% | Performance target validation |
| E2E | 2 | 8.3% | Full system lifecycle validation |
| **Total** | **24** | **100%** | - |

---

## Performance Targets Verified

| Target | Test Case | Expected | Verified |
|--------|-----------|----------|----------|
| format_brief() | TC-SESSION-11 | <100us p95 | Pending |
| consciousness brief | TC-SESSION-12, TC-SESSION-24 | <50ms p95 | Pending |
| restore-identity | TC-SESSION-14, TC-SESSION-24 | <2s p95 | Pending |
| persist-identity | TC-SESSION-17, TC-SESSION-24 | <3s p95 | Pending |
| check-identity | TC-SESSION-18, TC-SESSION-24 | <500ms p95 | Pending |
| inject-context | TC-SESSION-21, TC-SESSION-24 | <1s p95 | Pending |
| Full lifecycle | TC-SESSION-23 | <30s total | Pending |

---

## Dependency Chain Validation

### Foundation Layer (Independent Start)
```
TASK-SESSION-01 (Snapshot) ─┬─> TASK-SESSION-02 (Cache)
TASK-SESSION-03 (State)     │
TASK-SESSION-04 (ColumnFam) ─┴─> TASK-SESSION-05 (Storage)
```

### Logic Layer (Depends on Foundation)
```
TASK-SESSION-01,02,05 ──> TASK-SESSION-06 (Manager)
TASK-SESSION-07 (IC) ───> TASK-SESSION-08 (Dream)
TASK-SESSION-02 ───────> TASK-SESSION-09 (Brief)
TASK-SESSION-02 ───────> TASK-SESSION-10 (UpdateCache)
```

### Surface Layer (Depends on Logic)
```
TASK-SESSION-02,03,09 ──> TASK-SESSION-11 (CLI brief)
TASK-SESSION-06,07,10 ──> TASK-SESSION-12 (CLI restore)
TASK-SESSION-05,06 ─────> TASK-SESSION-13 (CLI persist)
TASK-SESSION-07,08,10 ──> TASK-SESSION-14 (CLI check)
TASK-SESSION-02,07 ─────> TASK-SESSION-15 (CLI inject)
TASK-SESSION-11-15 ─────> TASK-SESSION-16 (Hooks)
TASK-SESSION-17 (Exit codes - independent)
```

---

## Implementation Order (Inside-Out)

1. **Phase 1: Foundation** (7.5 hours estimated)
   - TASK-SESSION-01, TASK-SESSION-03, TASK-SESSION-04 (parallel)
   - TASK-SESSION-02 (after TASK-SESSION-01)
   - TASK-SESSION-05 (after TASK-SESSION-01, TASK-SESSION-04)

2. **Phase 2: Logic** (6.5 hours estimated)
   - TASK-SESSION-07 (independent)
   - TASK-SESSION-06, TASK-SESSION-09, TASK-SESSION-10 (after foundation)
   - TASK-SESSION-08 (after TASK-SESSION-07)

3. **Phase 3: Surface** (8 hours estimated)
   - TASK-SESSION-11-15 (after logic layer complete)
   - TASK-SESSION-16 (after all CLI commands)
   - TASK-SESSION-17 (independent)

**Total Estimated Time**: 22 hours

---

## File Locations Summary

### Source Files
| Component | Location |
|-----------|----------|
| Types/Snapshot | `crates/context-graph-core/src/gwt/session_identity/types.rs` |
| Cache | `crates/context-graph-core/src/gwt/session_identity/cache.rs` |
| Manager | `crates/context-graph-core/src/gwt/session_identity/manager.rs` |
| Dream Trigger | `crates/context-graph-core/src/gwt/session_identity/dream_trigger.rs` |
| Error | `crates/context-graph-core/src/gwt/session_identity/error.rs` |
| Storage | `crates/context-graph-storage/src/session_identity.rs` |
| Column Families | `crates/context-graph-storage/src/column_families.rs` |
| CLI Commands | `crates/context-graph-mcp/src/cli/commands/` |
| Hooks Config | `.claude/settings.json` |

### Test Files
| Test Type | Location |
|-----------|----------|
| Unit Tests | `crates/context-graph-core/src/gwt/session_identity/tests/` |
| Integration Tests | `crates/context-graph-storage/tests/session_identity_integration.rs` |
| CLI Integration | `tests/integration/session_hooks_test.rs` |
| Benchmarks | `crates/context-graph-core/benches/session_identity.rs` |

---

## Acceptance Criteria Checklist

- [x] All 17 requirements have corresponding tasks
- [x] All 17 tasks have corresponding test cases
- [x] All 24 test cases map back to requirements
- [x] All 6 constitution references are verified by tests
- [x] No orphaned artifacts exist
- [x] Dependency chain is validated
- [x] Performance targets are defined with test coverage
- [x] File locations are documented
- [x] Implementation order follows inside-out pattern

---

## Matrix Metadata

```yaml
document_type: traceability_matrix
feature: session_identity_persistence
phase: 1
version: 1.0.0
generated_by: traceability_matrix_agent
generation_date: 2026-01-14
source_documents:
  - SPEC-SESSION-IDENTITY.md (17 requirements)
  - TECH-SESSION-IDENTITY.md (technical implementation)
  - TASKS-SESSION-IDENTITY.md (17 tasks)
  - TESTS-SESSION-IDENTITY.md (24 test cases)
coverage:
  requirements: 100%
  tasks: 100%
  tests: 100%
  constitution: 100%
gaps: none
status: complete
```
