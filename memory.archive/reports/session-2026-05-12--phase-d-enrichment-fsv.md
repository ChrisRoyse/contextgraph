# Session Report: Phase D Enrichment FSV

Date: 2026-05-12

## Scope

Implemented the Phase D enrichment hardening pass for:

- Active content embedder forward coverage, including algorithmic E2/E3/E4/E9 wrappers and pretrained wrapper separation.
- Content-addressed embedder cache keyed by schema version, embedder id, model version, and chunk SHA-256.
- DDA signal computation, validation, and RocksDB persistence/readback for `CF_MEJEPA_DDA_SIGNALS`.
- Inference RocksDB opener coverage for `CF_MEJEPA_DDA_SIGNALS` and `CF_MEJEPA_FAILURE_EXEMPLARS`.
- Per-language `ReinventedWheel` stdlib registry with 11 tracked YAML files and fail-closed detection.

## Root Cause

The DDA column family had been registered in the flywheel CF group, but the runtime inference CF list did not open `CF_MEJEPA_DDA_SIGNALS` or `CF_MEJEPA_FAILURE_EXEMPLARS`. The practical effect was that daemon/inference runtime paths could open a DB that could not physically persist or read DDA signals. Phase D also lacked a durable embedder cache and a structured registry-backed stdlib signal source.

## Source Of Truth Verification

Manual FSV artifacts were written under `/mnt/d/contextgraph/fsv/phase-d-fsv`:

- DDA RocksDB source of truth: `/mnt/d/contextgraph/fsv/phase-d-fsv/dda-rocks-1778591532205`
- DDA evidence: `/mnt/d/contextgraph/fsv/phase-d-fsv/dda-signal-compute.json`
- Embedder cache evidence: `/mnt/d/contextgraph/fsv/phase-d-fsv/embedder-cache-fsv.json`
- Stdlib registry evidence: `/mnt/d/contextgraph/fsv/phase-d-fsv/stdlib-registry-fsv.json`

Observed physical state:

- DDA happy path: 0 rows before, 30 rows after reopen-read verification, expected 30.
- DDA edge cases: empty input, unsorted embedder ids, and zero vector all failed closed with `MEJEPA_TRAIN_CONFIG_INVALID` and left row count at 30.
- Cache happy path: 0 files before, 1 cache JSON after first write, still 1 after cache hit; corrupt cache failed closed instead of recomputing.
- Registry happy path: 11 tracked YAML registries loaded and Python retry-backoff detected; empty source produced zero detections; unknown language and missing file failed closed.

## Verification Commands

- `cargo check --examples -p context-graph-mejepa-embedders -p context-graph-mejepa-train -p context-graph-mejepa-instruments -p context-graph-mejepa-shift-subscriber`
- `cargo test -p context-graph-mejepa-embedders -p context-graph-mejepa-train -p context-graph-mejepa-instruments -p context-graph-mejepa-cf -p context-graph-mejepa-shift-subscriber`
- `cargo clippy --all-targets -p context-graph-mejepa-embedders -p context-graph-mejepa-train -p context-graph-mejepa-instruments -p context-graph-mejepa-cf -p context-graph-mejepa-shift-subscriber -- -D warnings`
- `cargo run -p context-graph-mejepa-train --example phase_d_dda_fsv`
- `cargo run -p context-graph-mejepa-embedders --example phase_d_embedder_cache_fsv`
- `cargo run -p context-graph-mejepa-instruments --example phase_d_stdlib_registry_fsv`

## Notes

No live secrets were used. Existing dirty worktree changes outside this Phase D scope were left intact. A small set of pre-existing clippy blockers were fixed because they blocked `-D warnings` verification of the touched package set.
