---
kind: report
session: 2026-05-12-cache-and-baseline-checkpoint-fsv
created_at: 2026-05-12
agent: codex
repo: /home/cabdru/contextgraph
---

# Session Report: Cache LRU/Telemetry + Baseline Checkpoint FSV

## Directives Followed
- Read and followed SMAP memory protocol, hardening prompt, flag prompt, and `docs/futurebuild/` direction.
- Used GitHub Issues as the tracked source of truth.
- Used Exa research guidance gathered for cache eviction/telemetry and checkpoint integrity: Redis eviction docs, OpenTelemetry metric guidance, and checkpoint integrity/atomic-write research.

## Closed Issues
- Closed #17: `[TEST] Prove embedder cache LRU and daemon telemetry`.
- Closed #19: `[TEST] Materialize and verify bootstrap baseline checkpoint`.
- Commented progress on parent #15.

## #17 Evidence
- Source of truth: physical embedder cache directory plus `.embedder-cache-telemetry.json`.
- FSV evidence: `/mnt/d/contextgraph/fsv/phase-d-fsv/embedder-cache-fsv.json`.
- Manual readback verified:
  - Happy cache file exists: 11170 bytes.
  - Happy telemetry file exists: 291 bytes.
  - LRU victim path absent.
  - LRU survivor files present: 11373 bytes and 11157 bytes.
- Edge cases covered: empty cache miss, max+1 LRU eviction, corrupt JSON entry fail-closed, max-bytes rejection with write_rejections telemetry.
- Daemon/subscriber status now exposes cache telemetry path and persisted telemetry.

## #19 Evidence
- Source of truth: checkpoint artifact plus schema-versioned manifest JSON.
- FSV evidence: `/mnt/d/contextgraph/fsv/baseline-checkpoint-fsv/baseline-checkpoint-fsv.json`.
- Manual readback verified:
  - Checkpoint exists: `/mnt/d/contextgraph/fsv/baseline-checkpoint-fsv/run-1778607964545/happy/best.checkpoint.json`, 1053 bytes, SHA-256 `620a45298418f80efb5c7d6903e27ae08518663f904f1a5df2e7ce49bf50164e`.
  - Manifest exists: `/mnt/d/contextgraph/fsv/baseline-checkpoint-fsv/run-1778607964545/happy/best.checkpoint.manifest.json`, 504 bytes, schema_version=1, payload_step=42.
  - Verified load read back step=42, sampler_rng_state=12648430, predictor key `prediction_head.weight`, aux key `test_pass.bias`.
- Edge cases covered: missing artifact, hash mismatch, manifest schema mismatch; all fail closed with `MEJEPA_TRAIN_CHECKPOINT_CORRUPT`.

## Verification Commands
- `cargo fmt --all -- --check`
- `cargo test -p context-graph-mejepa-embedders --lib`
- `cargo test -p context-graph-mejepa-shift-subscriber --lib`
- `cargo test -p context-graph-mejepa-train --lib`
- `cargo run -p context-graph-mejepa-embedders --example phase_d_embedder_cache_fsv`
- `cargo run -p context-graph-mejepa-train --example baseline_checkpoint_fsv`
- `cargo clippy -p context-graph-mejepa-embedders -p context-graph-mejepa-shift-subscriber --all-targets -- -D warnings`
- `cargo clippy -p context-graph-mejepa-train --all-targets -- -D warnings`

## Remaining Open P1s
- #18: Phase B label analyzers fail-closed FSV.
- #16: Persisted DDA feature consumption in prediction heads.
- #5: Full SWE-bench Lite 300x8 ship-gate FSV.
- #4: Replace deterministic instrument heads with analyzer-backed heads.
