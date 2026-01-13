# Meta-UTL Task Writer Agent Output

## Session: 2026-01-12
## Agent: Meta-UTL Task Spec Writer
## Status: COMPLETE

---

## Files Created

| File | Absolute Path | Purpose |
|------|---------------|---------|
| Per-Domain Accuracy Task | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-METAUTL-P1-001.md` | Implement per-domain accuracy tracking for lambda recalibration |
| Lambda Override Persistence Task | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-METAUTL-P2-001.md` | Add persistence for lambda_override field |
| Agent Memory | `/home/cabdru/contextgraph/docs/specs/.agent_memory/metautl_task_writer_output.md` | This file - coordination with next agent |

---

## Summary of Tasks Created

### TASK-METAUTL-P1-001: Implement Per-Domain Accuracy Tracking

**Priority:** P1 (High)
**Layer:** Logic
**Complexity:** Medium
**Status:** Ready

**Gap Addressed:** GAP-METAUTL-001 - Per-domain accuracy tracking not implemented

**Constitution Rules:**
- METAUTL-004: Domain-specific accuracy tracking required

**Key Implementation:**
1. Add `DomainAccuracyTracker` struct to `types.rs` with 100-sample rolling window
2. Add `domain_accuracy: HashMap<Domain, DomainAccuracyTracker>` to `MetaUtlTracker`
3. Implement `record_domain_accuracy()`, `get_domain_accuracy()`, `get_all_domain_accuracies()` methods

**Files Modified:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/core/types.rs`
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/core/meta_utl_tracker.rs`

**Critical Constraints:**
- Rolling window size = 100 samples (consistent with embedder tracking)
- Accuracy clamped to [0.0, 1.0]
- No panic on HashMap access - use entry API
- Domain enum already has Hash derive (verified in types.rs line 25)

---

### TASK-METAUTL-P2-001: Add Lambda Override Persistence

**Priority:** P2 (Medium)
**Layer:** Logic
**Complexity:** Medium
**Status:** Ready

**Gap Addressed:** GAP-METAUTL-002 - lambda_override lost on restart due to #[serde(skip)]

**Constitution Rules:**
- METAUTL-005: SelfCorrectingLambda trait must be implemented
- EC-08: Persistence across restart (from Sherlock report)

**Key Implementation:**
1. Remove `#[serde(skip)]` from lambda_override field
2. Add `#[serde(default, skip_serializing_if = "Option::is_none")]` for clean serialization
3. Add unit tests for serialization roundtrip and backwards compatibility

**Files Modified:**
- `/home/cabdru/contextgraph/crates/context-graph-utl/src/lifecycle/manager/types.rs`

**Critical Constraints:**
- No breaking changes to existing serialization format
- Backwards compatibility with old JSON (no lambda_override field)
- LifecycleLambdaWeights must be serializable (already has derives)

---

## Dependency Graph

```
TASK-METAUTL-P1-001 (domain accuracy)
    |
    | [No dependencies between these tasks]
    |
TASK-METAUTL-P2-001 (persistence)
```

Both tasks can be executed in parallel - they modify different files and have no interdependencies.

---

## Execution Order Recommendation

| Order | Task ID | Reason |
|-------|---------|--------|
| 1 | TASK-METAUTL-P1-001 | Higher priority (P1), enables domain-specific learning |
| 2 | TASK-METAUTL-P2-001 | Medium priority (P2), prevents loss of learned corrections |

---

## Verification Commands

```bash
# After TASK-METAUTL-P1-001
cargo test -p context-graph-mcp --lib handlers::core::meta_utl_tracker -- --nocapture
cargo test -p context-graph-mcp --lib test_domain_accuracy -- --nocapture

# After TASK-METAUTL-P2-001
cargo test -p context-graph-utl --lib lifecycle::manager -- --nocapture
cargo test -p context-graph-utl --lib test_lambda_override -- --nocapture

# Full verification
cargo clippy -p context-graph-mcp -p context-graph-utl -- -D warnings
cargo test -p context-graph-mcp -p context-graph-utl --lib -- --nocapture
```

---

## Source Analysis Summary

### Files Investigated

| File | Path | Key Findings |
|------|------|--------------|
| MetaUtlTracker | `crates/context-graph-mcp/src/handlers/core/meta_utl_tracker.rs` | Per-embedder tracking exists (line 26-30), NO domain tracking |
| Types | `crates/context-graph-mcp/src/handlers/core/types.rs` | Domain enum exists (line 23-40) with Hash derive |
| LifecycleManager Types | `crates/context-graph-utl/src/lifecycle/manager/types.rs` | lambda_override has #[serde(skip)] (line 69) |
| LifecycleManager Core | `crates/context-graph-utl/src/lifecycle/manager/core.rs` | Override methods work (line 232-300) |
| Sherlock Report | `docs/sherlock-meta-utl-report.md` | GAP-01: domain tracking, GAP-04: persistence |

### Constitution Rules Applied

| Rule ID | Rule Text | Applied To |
|---------|-----------|------------|
| METAUTL-001 | prediction_error > 0.2 -> lambda adjustment | Both tasks (data enablers) |
| METAUTL-002 | accuracy < 0.7 for 100 ops -> BayesianLambdaOptimizer | TASK-METAUTL-P1-001 |
| METAUTL-003 | dream_triggered -> lambda_adjustment | Both tasks |
| METAUTL-004 | Domain-specific accuracy tracking required | TASK-METAUTL-P1-001 |
| METAUTL-005 | SelfCorrectingLambda trait must be implemented | TASK-METAUTL-P2-001 |

---

## Key Insights for Next Agent (Coder)

1. **Domain enum is ready**: Already has `#[derive(Hash, Eq)]` (line 25 of types.rs), can be used as HashMap key directly.

2. **DomainAccuracyTracker design**: Pattern matches existing `embedder_accuracy` approach (100-sample rolling window). Keep implementation consistent.

3. **No mock data**: Tasks specify real data structures from codebase. Do NOT create mock/stub implementations.

4. **No backwards compatibility hacks**: The serde attributes (`default`, `skip_serializing_if`) handle backwards compatibility. Do NOT add migration code.

5. **No workarounds**: If serialization fails, error out loudly. Do NOT create alternative serialization paths.

6. **Test with real data**: Unit tests should use actual LifecycleLambdaWeights values (0.4, 0.6, etc.), not arbitrary floats.

---

## Traceability Update Required

After tasks are completed, update:
- `/home/cabdru/contextgraph/docs/specs/_index.md` - Mark tasks as COMPLETE
- `/home/cabdru/contextgraph/docs/specs/_traceability.md` - Update gap status

---

*Meta-UTL Task Spec Writer Agent Complete*
