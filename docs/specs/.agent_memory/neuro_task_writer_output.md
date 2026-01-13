# Neuromodulation Task Spec Writer Agent Output

## Session: 2026-01-12
## Agent: Neuromodulation Task Spec Writer
## Status: COMPLETE

---

## Files Created

| File | Absolute Path | Purpose |
|------|---------------|---------|
| Task Spec | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-NEURO-P1-001.md` | Wire steering handler to on_goal_progress |
| Agent Memory | `/home/cabdru/contextgraph/docs/specs/.agent_memory/neuro_task_writer_output.md` | This file - coordination with next agent |

---

## Summary of Task

### TASK-NEURO-P1-001: Wire Steering Handler to on_goal_progress

**Gap Identified:** The `on_goal_progress()` method exists in `NeuromodulationManager` but is NOT called from the MCP steering handler when `get_steering_feedback` is invoked.

**Solution:** After computing steering feedback via `SteeringSystem::compute_feedback()`, call `neuromod_manager.on_goal_progress_with_cascades(feedback.reward.value)` to propagate the steering reward to the dopamine modulator.

---

## Key Files Analyzed

### Source Files

| File | Absolute Path | Key Findings |
|------|---------------|--------------|
| steering.rs | `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/steering.rs` | Computes feedback but does NOT call neuromod |
| state.rs | `/home/cabdru/contextgraph/crates/context-graph-core/src/neuromod/state.rs` | Has `on_goal_progress()` at line 352-354 and `on_goal_progress_with_cascades()` at lines 370-424 |
| dopamine.rs | `/home/cabdru/contextgraph/crates/context-graph-core/src/neuromod/dopamine.rs` | DA_GOAL_SENSITIVITY = 0.1 at line 39 |
| handlers.rs | `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/core/handlers.rs` | neuromod_manager field at line 131 |

### Constitution Reference

From `/home/cabdru/contextgraph/docs2/constitution.yaml`:
- `neuromod.Dopamine`: `{ param: hopfield.beta, range: "[1,5]", effect: "=sharp retrieval" }`

---

## Implementation Constants

| Constant | Value | Source |
|----------|-------|--------|
| DA_GOAL_SENSITIVITY | 0.1 | dopamine.rs:39 |
| DA_MIN | 1.0 | dopamine.rs:26 |
| DA_MAX | 5.0 | dopamine.rs:29 |
| DA_BASELINE | 3.0 | dopamine.rs:23 |
| DA_HIGH_THRESHOLD | 4.0 | state.rs:36 (cascade trigger for 5HT boost) |
| DA_LOW_THRESHOLD | 2.0 | state.rs:38 (cascade trigger for 5HT drop) |
| SEROTONIN_CASCADE_DELTA | 0.05 | state.rs:40 |
| DA_CHANGE_THRESHOLD | 0.3 | state.rs:42 (trigger for NE alertness) |
| NE_ALERTNESS_DELTA | 0.1 | state.rs:44 |

---

## Data Flow (Current vs Required)

### Current Flow (BROKEN)

```
get_steering_feedback
    -> compute_feedback() -> SteeringReward { value: [-1, 1] }
    -> Return JSON (reward NOT propagated to neuromod)
```

### Required Flow (FIXED)

```
get_steering_feedback
    -> compute_feedback() -> SteeringReward { value: [-1, 1] }
    -> neuromod_manager.on_goal_progress_with_cascades(reward.value)
        -> DA adjustment (delta * 0.1)
        -> 5HT cascade (if DA > 4.0 or DA < 2.0)
        -> NE cascade (if |DA_change| > 0.3)
    -> Return JSON with neuromodulation cascade report
```

---

## Cascade Effects (Documented in state.rs)

1. **DA -> 5HT Mood Cascade**
   - If DA > 4.0 after adjustment: 5HT += 0.05
   - If DA < 2.0 after adjustment: 5HT -= 0.05

2. **DA -> NE Alertness Cascade**
   - If |DA_actual_change| > 0.3: NE += 0.1

---

## Testing Strategy

### Existing Tests (Must Pass)

```bash
cargo test -p context-graph-mcp -- steering
cargo test -p context-graph-core -- neuromod
```

### New Tests Recommended

Add FSV test at `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/full_state_verification_gwt/steering_neuromod_tests.rs`:

1. Call `get_steering_feedback` tool
2. Verify `neuromodulation.propagated == true`
3. Read DA value from Source of Truth (neuromod_manager)
4. Verify `da_new` in response matches Source of Truth

---

## Key Insights for Coder Agent

1. **neuromod_manager is Optional**: Check for `Some` before calling. If `None`, log warning but don't fail.

2. **Use on_goal_progress_with_cascades**: This returns a `CascadeReport` with all cascade effects, which should be included in the JSON response.

3. **Preserve Existing Response**: Add `neuromodulation` field to JSON, don't remove any existing fields.

4. **Lock for Write**: Must use `.write()` on the `RwLock` since `on_goal_progress_with_cascades` mutates state.

5. **Import Statement Needed**: May need `use context_graph_core::neuromod::CascadeReport;`

---

## Recommended Next Steps

1. **Coder Agent**: Implement TASK-NEURO-P1-001 following the task spec
2. **Tester Agent**: Add FSV test for steering -> neuromod integration
3. **Reviewer Agent**: Verify cascade report accuracy matches Source of Truth

---

## Reference Documents

- Constitution: `/home/cabdru/contextgraph/docs2/constitution.yaml`
- PRD to Spec Guide: `/home/cabdru/contextgraph/docs2/prdtospec.md`
- Architect Output: `/home/cabdru/contextgraph/docs/specs/.agent_memory/architect_output.md`
- Task Spec: `/home/cabdru/contextgraph/docs/specs/tasks/TASK-NEURO-P1-001.md`

---

*Neuromodulation Task Spec Writer Agent Complete*
