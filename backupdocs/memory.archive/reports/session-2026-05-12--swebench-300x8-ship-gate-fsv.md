# Session Report: SWE-bench 300x8 Ship-Gate FSV

Date: 2026-05-12T21:58:00Z

## Scope

Closed GitHub issue #5: `[TEST] Run full SWE-bench Lite 300x8 corpus ship-gate FSV`.

## Root Cause

The train CLI was not consuming the verified corpus split source of truth. It loaded all rows, ignored the `bucket` field, and recomputed train/calibration/holdout splits locally. The same loader also encoded `oracle_all_passed` into `panel_t01`, leaking the target label into model input. Separately, `Trainer::run_full_training` and `Trainer::evaluate_holdout` fabricated a perfect `1.0` holdout agreement, which could make a broken or unevaluated model look green.

## Changes

- Replaced corpus loading with `load_corpus_index_dataset`, which requires `index.json` bucket assignments and fails closed on missing/unknown buckets.
- Removed oracle-label leakage from corpus feature panels; panels now use task/category identity only.
- Added `swebench_300x8_ship_gate_fsv` to `context-graph-mejepa-train`.
- Changed `TrainingResult.final_holdout_agreement` to `Option<f32>` and made full training return `None` unless a real evaluator supplies a measured value.
- Changed `Trainer::evaluate_holdout` to fail closed instead of returning a synthetic perfect report.
- Updated `docs/futurebuild/02_CURRENT_STATE_HONEST.md` to point at the new real FSV and record the current 0.95 gate blockers.

## Full State Verification

Primary source of truth:

- Corpus index: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json`
- Corpus verify evidence: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/verify-evidence.json`
- RocksDB: `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/train-eval-rocksdb`
- FSV ledger: `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json`

Physical readback:

- ledger size: 19,583 bytes
- ledger mode: `0600`
- ledger sha256: `47db9c5a0e33bda349b7eb62e03d821b97efbb5ad511814232dbeff84948e472`
- RocksDB size: 196 KiB
- loaded split counts: train=1920, calibration=240, holdout=240
- `CF_MEJEPA_TRAIN_CERTS`: 30 physical rows, chain verified steps 0..29
- `CF_MEJEPA_HOLDOUT_REPORTS`: 4 physical rows

Results:

- Rolling holdout windows: 0.7750, 0.8375, 0.9875 agreement.
- Final holdout agreement: 0.8666667.
- Ship gate target: 0.95.
- Ship gate passed: false.
- Blockers: overall agreement below target; off_by_one, subtle_flip, and swap_variable below target.

Edge cases:

- Missing `bucket` fails closed without mutating RocksDB.
- Invalid `bucket=validation` fails closed with allowed bucket context and no RocksDB mutation.
- Two entries differing only in `oracle_all_passed` produce identical feature tensors, proving the oracle label is not encoded into features.

## Commands

- `cargo test -p context-graph-mejepa-train --examples --no-run`
- `cargo run -p context-graph-mejepa-train --example swebench_300x8_ship_gate_fsv`
- `cargo test -p context-graph-mejepa-train -- --nocapture`
- `git diff --check`

## Web Research

Used Exa MCP for ML split/leakage and production monitoring guidance:

- Google Cloud ML quality guidelines: separate train/validation/holdout, no holdout tuning, quality gates on pipeline inputs and outputs.
- AWS Prescriptive Guidance on splits and leakage: prevent data leakage by keeping train/validation/test separate and avoiding unavailable-at-inference information.
- Google production ML monitoring: validate raw/feature-engineered data and track rolling model performance against thresholds.
