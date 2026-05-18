---
date: 2026-05-11
task: phase8-evaluation-and-utml-learning-formula
status: complete
---

# Session Report: Phase 8 Evaluation + UTML Learning Formula

## Scope

Implemented and verified the remaining UTML learning-formula surface and Phase 8 ME-JEPA evaluation surface required by:

- `docs/ruvectorfindings/specs/tasks/utml-learning-formula`
- `docs/ruvectorfindings/specs/tasks/phase8-evaluation`
- `docs/ruvectorfindings/00_INDEX.md` through `15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`

Read and followed `/home/cabdru/leapablememory/docs2/SOLO-AGENT-MEMORY-PROTOCOL.md`.

## Root Causes Found

1. Phase 8 eval existed as concepts/internal training pieces but needed a public executable eval surface with RocksDB source-of-truth readback, CLI, MCP, active-learning queue, patch graph, and FSV evidence.
2. UTML had the base `L_step` signal but needed the remaining matrix audit, classifier/pruning, consolidation, state-transfer, dormant-unit reinit, and FSV evidence.
3. During manual CLI FSV, conformal health incorrectly treated over-coverage as a ship-gate failure. This violated the lower-bound nature of split-conformal coverage health. Fixed `conformal_health` to fail on under-coverage only: `empirical + band >= expected`.

## Implementation Notes

- UTML now includes real fail-closed modules for:
  - 15-slot pairwise MI matrix CSV audit with readback
  - 14-embedder coherence measurement
  - UTML state classifier and prune decisions
  - consolidation `M(t)`
  - state-transfer diagnostics with W1/MMD/KL options
  - dormant-unit detection and deterministic reinitialization
  - `/tmp/contextgraph-utml-fsv` FSV harness
- Phase 8 now includes:
  - public `context_graph_mejepa::eval` module
  - public `MutationCategory` with 8 variants
  - deterministic `EvalReport`, source-of-truth RocksDB store, queue, graph, metrics, runner, fixtures, report writer
  - Phase 8 eval CF constants in `context-graph-mejepa-cf`
  - `mejepa eval-run` and `mejepa eval-build-graph`
  - MCP tool names/definitions/handlers for:
    - `mcp__cgreality__mejepa_eval_run`
    - `mcp__cgreality__mejepa_ship_gate_status`
    - `mcp__cgreality__mejepa_active_learning_queue`
    - `mcp__cgreality__mejepa_eval_build_graph`
  - `/tmp/contextgraph-mejepa-phase8-eval-fsv` FSV harness with 17 evidence files

## Verification

Build/test commands run:

- `cargo check -p context-graph-mejepa-train`
- `cargo check -p context-graph-mejepa --bin mejepa --example phase8_eval_fsv`
- `cargo check -p context-graph-mcp`
- `cargo test -p context-graph-mejepa-train learning_signal --lib`
- `cargo test -p context-graph-mejepa --example phase8_eval_fsv`
- `cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates`
- `cargo test -p context-graph-mejepa eval --lib`

Manual FSV commands run:

- `cargo run -p context-graph-mejepa-train --example utml_l_step_fsv`
- `cargo run -p context-graph-mejepa --example phase8_eval_fsv`
- `cargo run -p context-graph-mejepa --bin mejepa -- eval-run --db-path /tmp/contextgraph-mejepa-phase8-cli-fsv/rocksdb --repo-root /tmp/contextgraph-mejepa-phase8-cli-fsv/repo --output-fsv /tmp/contextgraph-mejepa-phase8-cli-fsv --report-date 2026-05-11`
- `cargo run -p context-graph-mejepa --bin mejepa -- eval-build-graph --db-path /tmp/contextgraph-mejepa-phase8-cli-fsv/rocksdb --output-fsv /tmp/contextgraph-mejepa-phase8-cli-fsv --threshold 0.85 --top-k 3`

Source-of-truth readback:

- UTML files were read from `/tmp/contextgraph-utml-fsv`; all JSON/CSV evidence files are mode `0600`.
- Phase 8 files were read from `/tmp/contextgraph-mejepa-phase8-eval-fsv`; all evidence JSON files are mode `0600`.
- RocksDB was independently inspected with `ldb`.
- `ldb list_column_families` confirmed all eval CFs exist.
- `ldb dump --count_only` confirmed:
  - `CF_MEJEPA_EVAL_REPORTS`: 1 row
  - `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`: 1 row
  - `CF_MEJEPA_TASK_GRAPH`: 1 row
  - `CF_MEJEPA_OOD_ESCALATIONS`: 1 row
- `ldb scan --no_value` confirmed keys:
  - eval report key: `2026-05-11::00000001778466131193`
  - queue key: `active`
  - graph key: `active`
  - OOD escalation key: `phase8-ood_wrong_file`

## Evidence

- UTML happy path:
  - `l_step = 0.23622818`
  - classifier state: `boring`
  - dormant units: `[0, 2]`
  - W1 transfer score: `0.9277435`
  - pairwise MI CSV: `/tmp/contextgraph-utml-fsv/utml-pairwise-mi-matrix-00000042.csv`
- UTML edge cases:
  - empty pairwise panels -> `UTML_EMPTY_INPUT`
  - target collapse -> `MEJEPA_INSTRUMENT_GRADIENT_LEAK`
  - invalid state-transfer lambda -> `UTML_OUT_OF_RANGE`
- Phase 8 happy path:
  - ship gate passed: `true`
  - holdout count: `8`
  - overall correlation: `1.0`
  - OOD AUC: `1.0`
  - Gτ pass rate: `1.0`
  - active-learning queue entries: `7`
  - OOD escalations: `1`
  - patch graph edges: `12`
- Phase 8 edge cases:
  - empty holdout -> `MEJEPA_EVAL_EMPTY_HOLDOUT`
  - train/holdout leakage -> `MEJEPA_EVAL_LEAKAGE_DETECTED`
  - empty patch graph -> `MEJEPA_EVAL_INVALID_INPUT`

## Research Applied

- Split conformal prediction is a lower-bound/marginal coverage guarantee, so over-coverage should not fail a ship gate.
- RocksDB CF operations need explicit CF handles and sync/readback for source-of-truth persistence.
- MCP tool results should return structured content and operational errors as tool-level errors where appropriate.
- Active-learning queue selection uses uncertainty/entropy-style prioritization.
- OOD health uses ROC-AUC over scores and labels.
- State transfer uses W1 distance over empirical distributions.
- Dormant-unit reinitialization follows continual-backprop/plasticity-loss guidance.

