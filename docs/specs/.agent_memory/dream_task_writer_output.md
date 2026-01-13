# Dream Layer Task Spec Writer Output

## Session: 2026-01-12
## Agent: Dream Layer Task Spec Writer
## Status: COMPLETE

---

## Files Created

| File | Absolute Path | Purpose |
|------|---------------|---------|
| TASK-DREAM-P0-001.md | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-DREAM-P0-001.md` | Wire HyperbolicExplorer to REM Phase |
| TASK-DREAM-P0-002.md | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-DREAM-P0-002.md` | Integrate MemoryStore with NREM Replay |
| TASK-DREAM-P0-003.md | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-DREAM-P0-003.md` | Complete Amortized Edge Creation |

---

## Task Summary

### TASK-DREAM-P0-001: Wire HyperbolicExplorer to REM Phase

**Key Insight**: The HyperbolicExplorer (830+ lines) is FULLY IMPLEMENTED but NEVER CALLED from rem.rs.

**Current State**:
- `rem.rs::process()` (lines 157-238) returns stub/placeholder data
- Contains TODO comment "Agent 2 will implement actual processing"
- Simulates queries and blind spots with fake data

**Fix Required**:
1. Add `explorer: HyperbolicExplorer` field to `RemPhase` struct
2. Rewrite `process()` to call `self.explorer.explore()`
3. Convert `ExplorationResult` to `RemReport` with real metrics

**Files to Modify**: `crates/context-graph-core/src/dream/rem.rs`

**Constitution Rules**: DREAM-002, AP-35, AP-41

---

### TASK-DREAM-P0-002: Integrate MemoryStore with NREM Replay

**Key Insight**: HebbianEngine is correct but operates on EMPTY VECTORS.

**Current State**:
- `nrem.rs::process()` lines 152-153:
  ```rust
  let memories: Vec<(Uuid, u64, f32)> = Vec::new();
  let edges: Vec<(Uuid, Uuid, f32)> = Vec::new();
  ```
- HebbianEngine computes updates on empty data (always returns zeros)

**Fix Required**:
1. Define `MemoryProvider` trait for memory/edge retrieval
2. Add `memory_provider: Option<Arc<dyn MemoryProvider>>` field
3. Modify `process()` to call provider instead of empty vectors

**Files to Modify**: `crates/context-graph-core/src/dream/nrem.rs`, `mod.rs`

**Constitution Rules**: DREAM-001, AP-35, AP-36

---

### TASK-DREAM-P0-003: Complete Amortized Edge Creation

**Key Insight**: AmortizedLearner tracks paths correctly but `create_shortcut()` is a stub.

**Current State**:
- `amortized.rs::create_shortcut()` (lines 238-271) contains TODO
- Quality gate works (hops >= 3, traversals >= 5, confidence >= 0.7)
- Edge is NOT actually created

**Fix Required**:
1. Define `ShortcutEdge` struct with `is_shortcut`, `original_path`
2. Define `EdgeCreator` trait
3. Add `edge_creator: Option<Arc<dyn EdgeCreator>>` field
4. Modify `create_shortcut()` to call creator

**Files to Modify**: `crates/context-graph-core/src/dream/amortized.rs`, `mod.rs`

**Constitution Rules**: DREAM-005

---

## Key Implementation Insights

### 1. Dependency Injection Pattern

All three tasks use the same pattern:
- Define a trait for the external dependency (Explorer, MemoryProvider, EdgeCreator)
- Add `Option<Arc<dyn Trait>>` field to the struct
- Add `set_X()` method for injection
- Check `if let Some(x) = &self.x` before calling
- Provide `NullX` implementation for backward compatibility

This pattern allows:
- Unit testing with mock implementations
- Gradual integration (set provider when ready)
- Backward compatibility (no provider = current behavior)

### 2. Constitution Compliance Points

| Parameter | Constitution Value | Location in Code |
|-----------|-------------------|------------------|
| REM temperature | 2.0 | `HyperbolicWalkConfig::default()` |
| Semantic leap | >= 0.7 | `HyperbolicWalkConfig::min_blind_spot_distance` |
| Query limit | 100 | `HyperbolicExplorer::query_limit` |
| NREM coupling | 0.9 | `HebbianConfig::coupling_strength` |
| NREM recency bias | 0.8 | `NremPhase::recency_bias` |
| Hebbian formula | dw = eta * phi_i * phi_j | `HebbianEngine::compute_delta()` |
| Min shortcut hops | 3 | `AmortizedLearner::min_hops` |
| Min shortcut traversals | 5 | `AmortizedLearner::min_traversals` |
| Shortcut confidence | >= 0.7 | `AmortizedLearner::confidence_threshold` |

### 3. What's NOT in Scope

These tasks create interfaces but NOT implementations for:
- Real MemoryStore (requires graph store integration)
- Real EdgeCreator (requires graph store integration)
- Real starting positions for REM (requires MemoryStore)
- Persisting HebbianEngine updates (requires graph store)

These require separate tasks once graph store integration is available.

---

## Blockers Discovered

### No Major Blockers

All three tasks are **READY** with no dependencies:
- HyperbolicExplorer code exists and is tested
- HebbianEngine code exists and is tested
- AmortizedLearner quality gate code exists and is tested

### Minor Integration Notes

1. **Starting Positions**: TASK-DREAM-P0-001 hardcodes `[0.0; 64]` as starting position. Real positions require MemoryStore integration from TASK-DREAM-P0-002.

2. **Edge Persistence**: TASK-DREAM-P0-003 creates ShortcutEdge but real persistence requires graph store integration (out of scope).

3. **Test Data**: All tasks need mock providers for unit tests. Test implementations included in task specs.

---

## Recommended Execution Order

```
1. TASK-DREAM-P0-001 (REM Phase) - No dependencies
2. TASK-DREAM-P0-002 (NREM Phase) - No dependencies
3. TASK-DREAM-P0-003 (Amortized) - No dependencies
```

All three can be implemented in parallel if desired, but sequential is safer to verify each component works before adding complexity.

---

## Verification Commands

```bash
# After TASK-DREAM-P0-001
cargo test -p context-graph-core --lib dream::rem -- --nocapture
grep -r "TODO\|STUB" crates/context-graph-core/src/dream/rem.rs

# After TASK-DREAM-P0-002
cargo test -p context-graph-core --lib dream::nrem -- --nocapture
grep -r "Vec::new()" crates/context-graph-core/src/dream/nrem.rs

# After TASK-DREAM-P0-003
cargo test -p context-graph-core --lib dream::amortized -- --nocapture
grep -r "TODO\|Agent 2" crates/context-graph-core/src/dream/amortized.rs

# Full dream suite
cargo test -p context-graph-core --lib dream -- --nocapture
```

---

## Reference Documents Used

- `/home/cabdru/contextgraph/docs/specs/_index.md` - Gap inventory
- `/home/cabdru/contextgraph/docs/specs/_traceability.md` - Requirements mapping
- `/home/cabdru/contextgraph/docs/specs/.agent_memory/architect_output.md` - Architecture context
- Source files in `crates/context-graph-core/src/dream/`

---

*Dream Layer Task Spec Writer Agent Complete*
*Format: prdtospec.md v1.0*
