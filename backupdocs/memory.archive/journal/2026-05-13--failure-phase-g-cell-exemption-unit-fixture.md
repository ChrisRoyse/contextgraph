---
namespace: journal
agent_id: codex
timestamp: 2026-05-13T09:16:48-05:00
status: active
tags: phase-g, task-g-007, unit-test, failure, fail-closed
---

# Failure: Cell Exemption Unit Fixture Included Non-Exempt Failure

## What Happened

After tightening the ship-gate predicate to keep non-exempt persisted failures blocking, `cargo test -p context-graph-mejepa --lib` failed in `eval::ship_gate_stability::tests::exempt_low_cell_does_not_reset_passing_window`.

The test created the report with `passed=false`, which also seeded a generic non-cell `per_window_correlation_below_0.95` failure. That made the new fail-closed behavior correctly reset the streak to zero.

## Why It Matters

Per-cell exemptions must only remove failures that mention the exempt cell. Conformal, OOD, Q1/Q3, generic window, and other non-exempt failures must still block the ship gate.

## Resolution

Update the unit fixture so the report contains only the exempt-cell failure. The FSV harness already used the correct shape.
