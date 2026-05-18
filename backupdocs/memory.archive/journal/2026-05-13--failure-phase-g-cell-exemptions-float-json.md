---
namespace: journal
agent_id: codex
timestamp: 2026-05-13T09:08:33-05:00
status: active
tags: phase-g, task-g-007, fsv, failure, float-json
---

# Failure: Phase G Cell Exemptions FSV Float JSON Exactness

## What Happened

The first `scripts/ci/phase_g_cell_exemptions_fsv.sh` run failed on the assertion:

`active_status["perCellCorrelation"][EXEMPT_CELL] == 0.10`

The MCP payload read back the expected exempt cell from RocksDB, but JSON serialization of the `f32` correlation produced `0.10000000149011612`.

## Why It Matters

The behavior under test is whether the low exempt cell remains visible and non-blocking. Literal JSON float equality is the wrong Source-of-Truth assertion for that property because the physical value is a binary `f32`.

## Resolution

Adjust the FSV harness to compare the visible correlation with a small numeric tolerance while keeping the ship-gate pass/fail, exemption reason, failure filtering, and reopened RocksDB count assertions exact.
