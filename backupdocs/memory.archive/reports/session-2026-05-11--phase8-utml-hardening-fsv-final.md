---
date: 2026-05-11
task: phase8-utml-hardening-full-state-verification
status: complete
---

# Session Report: Phase 8 + UTML Hardening FSV

## Scope

Hardened and re-verified Phase 8 evaluation, UTML learning-signal behavior, pairwise-MI training certificate persistence, and continual-backprop regulation. Followed `/home/cabdru/leapablememory/docs2/SOLO-AGENT-MEMORY-PROTOCOL.md`.

## Code State Observed

Current branch HEAD during final verification:

- `809f422a feat(mejepa-train): typed pairwise-MI column in TrainingCertificate`
- prior relevant commits included `167daa73 feat(mejepa-utml): typed Δp/Δκ/ΔΩ/Δξ components, MCP pairwise-MI audit tool, eval Q1/Q3/regression gates` and `b435815b chore(mejepa): continual-backprop FSV example, ship-gate honesty pass, rustfmt nits`

Working tree at final check had one remaining unstaged change:

- `crates/context-graph-mejepa-train/examples/train_smoke_fsv.rs`
  - changed smoke FSV batch size from 1 to 2 because pairwise-MI train-cert evidence now fail-closes on batches with fewer than two examples
  - changed expected persisted train cert count from 100 to 50, matching 100 synthetic examples / batch size 2

## Root Causes Fixed

- Phase 8 previously had ways for evaluation evidence to look accepted without source-of-truth side-effect proof. Ship gate now reads `CF_MEJEPA_LIVE_PREDICTIONS`; missing Q3 evidence fails instead of being fabricated.
- Regression checks previously did not require a prior persisted report. Phase 8 FSV now writes a baseline report and a current report, and the current report is checked against the persisted baseline.
- UTML formula components were too implicit. The live code now persists and validates typed ΔP, ΔK, ΔΩ, and ΔΞ component structs, including SNR floor, Fisher/ECE/plasticity terms, and target-collapse fail-closed checks.
- Pairwise-MI predictor redundancy was not a typed training-certificate source-of-truth column. `TrainingCertificate` now has `predictor_redundancy_pairwise_mi` and validates it against `signal.delta_xi_components.predictor_redundancy`.
- Continual-backprop regulation needed physical readback proof. The FSV example verifies `CF_MEJEPA_WEIGHT_BLOBS` after dormant-unit reinitialization.

## Web Research Applied

- Split conformal prediction: preserve lower-bound coverage semantics; overcoverage is not a ship-gate failure.
- Elastic Weight Consolidation/Fisher information: use a Fisher-violation term in ΔK and fail closed on destructive negative gradient alignment.
- Calibration/ECE: include ECE in ΔK instead of relying on raw prediction confidence.
- Continual backprop/loss-of-plasticity: regulate low-plasticity dormant units with readback evidence.
- RocksDB write/readback: verify the column family source of truth directly with a separate read, not return values.

## Verification Commands

Automated checks:

- `cargo fmt --check` passed after formatting.
- `cargo test -p context-graph-mejepa-train --lib` passed: 24 tests.
- `cargo test -p context-graph-mejepa --lib` passed: 70 tests.
- `cargo test -p context-graph-mcp mejepa_utml_tools --lib` passed: 2 tests.
- `cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates --lib` passed: 1 test.

Manual FSV commands:

- `cargo run -p context-graph-mejepa --example phase8_eval_fsv` passed.
- `cargo run -p context-graph-mejepa-train --example utml_l_step_fsv` passed.
- `cargo run -p context-graph-mejepa --example continual_backprop_regulation_fsv` passed.
- `cargo run -p context-graph-mejepa-train --example train_smoke_fsv` passed.

## Source-of-Truth Evidence

Phase 8 eval source of truth:

- DB: `/tmp/contextgraph-mejepa-phase8-eval-fsv/rocksdb`
- Evidence: `/tmp/contextgraph-mejepa-phase8-eval-fsv/holdout-eval-evidence.json`
- `ship_gate_passed = true`, `ship_gate_failures = []`, `q1_pass_rate = 1.0`, `q3_side_effect_agreement = 1.0`, regression previous/current/drop = `1.0/1.0/0.0`
- `CF_MEJEPA_EVAL_REPORTS`: 2 rows
- `CF_MEJEPA_LIVE_PREDICTIONS`: 6 rows
- `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`: 1 row

UTML learning-signal source of truth:

- Files under `/tmp/contextgraph-utml-fsv`, all mode `0600`
- `utml-l-step-evidence.json`: `l_step = 0.4415854513645172`, ΔP = `0.699999988079071`, ΔK = `0.8550000190734863`, ΔΩ = `0.7652177214622498`, ΔΞ = `0.9641965627670288`
- `utml-pairwise-mi-matrix-00000042.csv` persisted and read back.
- Edge cases:
  - empty pairwise panels -> `UTML_EMPTY_INPUT`
  - target collapse -> `MEJEPA_INSTRUMENT_GRADIENT_LEAK`
  - invalid state-transfer lambda -> `UTML_OUT_OF_RANGE`

Continual-backprop source of truth:

- DB: `/mnt/d/contextgraph/fsv/contextgraph-mejepa-continual-backprop-fsv/happy-path/heal-rocksdb`
- `CF_MEJEPA_WEIGHT_BLOBS`: 1 row at `continual_backprop/7/0`
- `CF_MEJEPA_TRAIN_CERTS`: 3 rows
- Evidence file mode `0600`
- Edge cases preserve source-of-truth state on invalid input, missing embedder, and zero stability floor.

Train smoke source of truth:

- DB: `/tmp/contextgraph-mejepa-train-fsv/smoke-rocksdb`
- `CF_MEJEPA_TRAIN_CERTS`: 50 rows
- Evidence: `/tmp/contextgraph-mejepa-train-fsv/training-smoke-evidence.json`
- Chain verification: `verified = 50`, `broken_at = null`
- Target-collapse violations: `[]`

## Notes

There were concurrent agents/processes in the same repository during verification. Final reporting should describe the current repository state, not earlier transient `git status` snapshots.
