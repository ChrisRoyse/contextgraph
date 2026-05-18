# Phase 6 Hygiene Root Cause and FSV

## Timestamp
2026-05-10T23:12:28Z

## Context
Phase 6 hygiene was requested after the ME-JEPA pivot and the Phase 1.5/FSV hardening docs. The relevant specs were `docs/ruvectorfindings/specs/implementation-plan.md` and `docs/ruvectorfindings/specs/tasks/phase6-hygiene/*`.

## Root Cause
- Phase 6 behavior was not exposed end-to-end: hygiene primitives existed only partially and were not wired into MCP or the `mejepa` CLI.
- Cross-cutting CF names were duplicated across ME-JEPA modules, making long-term hygiene unsafe because new ops could drift from the actual RocksDB source of truth.
- Witness data could grow without segment compression and had no fail-closed archive verification path for old segments.
- Quota enforcement lacked sticky, inspectable unrecoverable state for categories that cannot be evicted because all candidates are protected.

## Implemented Correction
- Added DB-backed Phase 6 ops for witness segment compression, nightly GC, quota status/eviction, and MCP request wrappers.
- Wired MCP tools:
  - `mcp__cgreality__mejepa_gc_run`
  - `mcp__cgreality__mejepa_quota_status`
  - `mcp__cgreality__mejepa_witness_compress`
- Added `mejepa stress` to seed real RocksDB state, run GC, and print source-of-truth evidence.
- Hardened tier transitions with readback verification, corrupt metadata marking, and distinct promote/demote sweeps.
- Wrote FSV examples for Merkle compression, quota overflow, and GC stress.

## Full State Verification
- Merkle compression source of truth: `CF_MEJEPA_WITNESS_CHAIN`, `CF_MEJEPA_WITNESS_SEGMENT_META`, archive files.
- Quota source of truth: direct RocksDB CF scans plus sticky state in `CF_MEJEPA_QUOTA_STATE`.
- GC source of truth: `CF_MEJEPA_GC_HISTORY`, panel cache CFs, witness archives, witness segment metadata.

Evidence files:
- `/tmp/contextgraph-mejepa-hygiene-merkle-fsv/aggregate-evidence.json`
- `/tmp/contextgraph-mejepa-hygiene-quota-fsv/aggregate-evidence.json`
- `/tmp/contextgraph-mejepa-hygiene-gc-stress-fsv/aggregate-evidence.json`

Manual results:
- Merkle: 2048 witness rows compressed to 2 live rows, 2 archives, 2048 entries archived.
- Quota: 6 MiB over-budget synthetic shift data evicted to 4 MiB, zero remaining category violations.
- GC stress: 10000 entries, 7 GC passes, 2 compressed witness entries, zero quota violations.

## Notes
No mocks were used in the FSV examples. The tests seed real RocksDB databases and inspect persisted CF/file state after each operation.
