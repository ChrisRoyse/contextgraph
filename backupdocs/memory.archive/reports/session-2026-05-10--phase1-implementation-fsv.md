# Session Report: Phase 1 Implementation And FSV

Date: 2026-05-10

## Intent

Implement and verify Phase 1 AST chunking, embedder routing/storage invariants, and ME-JEPA instrument panel behavior. Followed SAMP directive from `/home/cabdru/leapablememory/docs2/SOLO-AGENT-MEMORY-PROTOCOL.md`.

## Root Causes Found

- `context-graph-embeddings` still had stale 13/14-model assumptions after `ModelId::BgeM3Dense` was added. Correct state is:
  - `ModelId::all().len() == 15`
  - `ModelId::production().len() == 14`
  - `storage::NUM_EMBEDDERS == 14`
  - `dimensions::MODEL_COUNT == 15`
  - `dimensions::TOTAL_DIMENSION == 13056`
- `StoredQuantizedFingerprint::new` allowed wrong quantization methods at construction and only exposed a later boolean validator. It now fails closed during construction if any slot's method does not match `ModelId::production()[slot]`.
- GPU build-time arch detection failed without `CUDA_COMPUTE_CAP=120`; this is correct for RTX 5090 / compute capability 12.0. Runtime CUDA tests still fail because WSL reports `nvidia-smi: GPU access blocked by the operating system`.

## Implemented

- Added ME-JEPA instrument modules:
  - `crates/context-graph-mejepa-instruments/src/frozen_hook.rs`
  - `crates/context-graph-mejepa-instruments/src/materialize.rs`
  - `crates/context-graph-mejepa-instruments/src/panel_json.rs`
  - `crates/context-graph-mejepa-instruments/src/linear_probe.rs`
- Extended `phase1_extended_fsv` to cover:
  - all 15 panel slots
  - T0/T2 materialization masks
  - frozen-target mutation and gradient detection
  - panel JSON schema/provenance/hash validation
  - real-vector linear probe with holdout accuracy
  - 16 edge cases with before/after state
- Updated embedder tests/storage helpers to use production slot order rather than `0..13`.
- Added `ModelId` string parsing for `BgeM3Dense`.
- Improved GPU initialization failure diagnostics so the runtime error names WSL GPU-access blockage and explains that `CUDA_COMPUTE_CAP=120` only fixes build-time detection.

## Full State Verification Evidence

- AST chunker multilingual FSV:
  - source of truth: `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`
  - `language_count = 11`
  - edge cases passed: empty source, invalid syntax, mixed PHP/HTML
- SWE-bench AST FSV:
  - source of truth: `/tmp/contextgraph-swebench-ast-fsv/swebench-ast-evidence.json`
  - `passes = true`, `chunk_count = 20`
  - manifest: `tasks/swebench-lite/sympy__sympy-20590.json`
- AST embedder smoke FSV:
  - source of truth: `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json`
  - `passes = true`, `panel_dim = 5120`, `panel_filled_mask = 32767`, `sample_count = 11`
  - physical files read back: `chunks.json`, `panel.json`
- Phase 1 extended instruments FSV:
  - source of truth: `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json`
  - `passes = true`, `panel_dim = 5120`, `panel_filled_mask = 32767`
  - `materialized_t0_mask = 2047`, `materialized_t2_mask = 32767`
  - frozen snapshot hash: `75bfc0e3c363dbb903720aed8bdbb46d72e8b87f3ef65aaa7d78b1939be9e2cc`
  - linear probe: `sample_count = 30`, `holdout_count = 12`, `top1_accuracy = 1.0`
  - physical files read back: `panel-all-slots.json`, `panel-envelope-t2.json`
- Embedder routing FSV:
  - source of truth: `/tmp/contextgraph-mejepa-embedders-fsv/routing_coverage.json`
  - `row_count = 143`

## Test Results

- `cargo test -p context-graph-core memory::ast::tests -- --nocapture`: passed 10.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-mejepa-instruments -- --nocapture`: passed 53.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-embeddings --test dimension_test -- --nocapture`: passed 66.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-embeddings --test benchmark_test -- --nocapture`: passed 23.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-embeddings --test search_test -- --nocapture`: passed 26.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-embeddings --test storage_roundtrip_test -- --nocapture`: passed 36.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-embeddings --test no_fake_data_test -- --nocapture`: passed 25.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-embeddings -- --nocapture`: 1141 passed, 17 failed. Failures are real GPU/weight tests blocked by WSL runtime GPU access, not skipped.

## Remaining Blocker

Runtime CUDA access is blocked in WSL:

```text
nvidia-smi
Failed to initialize NVML: GPU access blocked by the operating system
```

Until that Windows/WSL GPU-access issue is fixed, tests that actually initialize CUDA or load pretrained weights will continue to fail. Build-time CUDA compilation works with `CUDA_COMPUTE_CAP=120`.
