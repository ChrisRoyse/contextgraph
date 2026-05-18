---
namespace: reports
agent_id: codex
timestamp: 2026-05-10T07:58:50-05:00
status: completed
tags: [mejepa, phase4, tct, constellation, fsv, rocksdb, mcp]
---

# Phase 4b TCT Constellation Implementation

## Scope

Implemented `docs/ruvectorfindings/specs/tasks/phase4-tct-constellation` end to end:

- Added the new workspace crate `crates/context-graph-mejepa-tct`.
- Wired real TCT Gτ checking into ME-JEPA inference.
- Replaced the training zero-loss constellation fallback with fail-closed centroid validation.
- Added MCP read-only inspect support for persisted TCT constellations.
- Added full-state FSV evidence generation and manual source-of-truth verification.

## Root Causes

- Phase 4b had specifications but no `context-graph-mejepa-tct` implementation crate.
- Inference had only a deterministic scenario-string guard, not a real `Gτ` guard over predicted panels.
- `l_constellation_centroid` silently returned zero loss when centroids were absent.
- The TCT specs included a permissive under-sampled calibration placeholder; the implementation intentionally fails closed instead.
- Store `latest_version()` initially used RocksDB key/hash order; this was corrected to use newest persisted `frozen_at`.

## Implementation Notes

- Added public/internal TCT models, fail-closed `TctError` codes, builder, shrinkage, held-out threshold calibration, deterministic `gtau_check`, chunk-level localization, hinge output, RocksDB store, freshness policy, violation-rate aggregation, and inspect summary.
- RocksDB source of truth:
  - `CF_MEJEPA_CONSTELLATION`
  - `CF_MEJEPA_GUARD_DECISIONS`
- Added read-after-write validation and reopen validation for persisted constellations.
- Added provenance checks against all 21 runtime embedder digests.
- Added stale-constellation rejection with default 90-day max age and audited override path.
- Added `mcp__cgreality__mejepa_constellation_inspect`; output returns metadata/counts/thresholds only, never raw centroid vectors.
- Added 8 MCP inspect tests covering summary output, raw centroid non-leakage, latest-by-timestamp, provenance mismatch, stale rejection, malformed version id, missing DB path, and empty store.

## Research Inputs

- RocksDB official column-family guidance confirmed the use of explicit column-family descriptors and explicit CF handles for read/write verification: https://github.com/facebook/rocksdb/wiki/column-families
- Conformal prediction calibration practice informed held-out KnownGood threshold calibration and fail-closed insufficient-sample behavior: https://arxiv.org/abs/2107.07511

## Verification

Commands that passed:

- `cargo test -p context-graph-mejepa-tct -p context-graph-mejepa-train -p context-graph-mejepa -- --nocapture`
  - ME-JEPA: 24 passed
  - TCT: 7 passed
  - ME-JEPA train: 18 passed
- `cargo test -p context-graph-mcp mejepa_constellation_inspect -- --nocapture`
  - lib target: 8 passed
  - bin target: 8 passed
- `cargo test -p context-graph-mcp tools::definitions::tests -- --nocapture`
  - lib target: 3 passed
  - bin target: 3 passed
- `cargo check -p context-graph-mcp`
- `cargo clippy -p context-graph-mejepa-tct -p context-graph-mejepa -p context-graph-mejepa-train --no-deps -- -D warnings`
- `cargo clippy -p context-graph-mcp --no-deps -- -D warnings -A clippy::result_large_err`
- `cargo run --release -p context-graph-mejepa-tct --example tct_fsv`

Known verification note:

- `cargo clippy -p context-graph-mcp --no-deps -- -D warnings` still fails on pre-existing `clippy::result_large_err` debt across MCP reality-loop modules. This is outside the TCT implementation; with only that existing lint class allowed, MCP clippy passes.

## Full State Verification Evidence

FSV source of truth:

- RocksDB path: `/tmp/contextgraph-mejepa-tct-fsv/tct-rocksdb`
- Constellation version: `3aa2428e302c0fbe4275877e00844b657ec0355221021ca214330a8c90d022f8`
- Real Phase 1 panel hash: `75bfc0e3c363dbb903720aed8bdbb46d72e8b87f3ef65aaa7d78b1939be9e2cc`

Evidence files, all mode `0600`:

- `/tmp/contextgraph-mejepa-tct-fsv/constellation-evidence.json`
- `/tmp/contextgraph-mejepa-tct-fsv/gtau-evidence.json`
- `/tmp/contextgraph-mejepa-tct-fsv/threshold-calibration-evidence.json`
- `/tmp/contextgraph-mejepa-tct-fsv/chunk-level-rejection-evidence.json`
- `/tmp/contextgraph-mejepa-tct-fsv/edge-case-audit.json`

Manual source-of-truth checks:

- RocksDB column families present: `{default, CF_MEJEPA_CONSTELLATION, CF_MEJEPA_GUARD_DECISIONS}`
- `CF_MEJEPA_CONSTELLATION` scan count: `1`
- `CF_MEJEPA_GUARD_DECISIONS` scan count: `0`
- Persisted row count for version: `1`
- Inspect summary:
  - `panel_level_cells = 63`
  - `per_chunk_type_cells = 21`
  - `shrunken_cells = 0`
  - `thresholds_len = 21`
  - sample support: `n_lt_5 = 0`, `n_5_to_50 = 0`, `n_50_plus = 21`
- Happy path Gτ:
  - `gtau_satisfied = true`
  - `violations = []`
  - `evaluated_embedder_count = 21`
- Negative path:
  - negated E7 slot rejected
  - `observed_cos = -1.0`
  - `threshold = 1.0`
  - `margin = -2.0`
- Calibration:
  - `decision_count = 42`
  - `strict_cells = 21`
  - `max_observed_fpr = 0.0`
  - `min_sample_count = 50`
- Chunk localization:
  - `aggregate_satisfied = false`
  - `violating_chunk_count = 1`
  - failing slot: `e_ast`

Manual edge-case audit:

- Empty validation:
  - before: `validation_samples = 0`
  - after: `MEJEPA_TCT_INSUFFICIENT_SAMPLES`
  - `state_mutated = false`
- Stale constellation:
  - before: `max_age_days = 90`
  - after: `MEJEPA_INFER_GTAU_STALE_CONSTELLATION`
  - `state_mutated = false`
- Provenance mismatch:
  - before: `runtime_embedder_versions = 21`
  - after: `MEJEPA_TCT_PROVENANCE_MISMATCH`
  - `state_mutated = false`

## Files Touched

- New: `crates/context-graph-mejepa-tct/**`
- Workspace: `Cargo.toml`, `Cargo.lock`
- ME-JEPA inference: `crates/context-graph-mejepa/src/compiler.rs`, `error.rs`, `lib.rs`, crate dependency
- ME-JEPA training: `crates/context-graph-mejepa-train/src/loss/constellation.rs`, `src/loss/mod.rs`
- MCP: `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs`, dispatch, names, definitions, dependency
- Minor clippy cleanup in touched dependency paths:
  - `crates/context-graph-mejepa-embedders/src/embedder_id.rs`
  - `crates/context-graph-mcp/src/handlers/tools/reality_loop/recommendations.rs`
  - `crates/context-graph-mcp/src/handlers/tools/reality_loop/witness_chain_io.rs`
  - `crates/context-graph-mcp/src/handlers/tools/reality_loop/witness_chain_legacy.rs`
