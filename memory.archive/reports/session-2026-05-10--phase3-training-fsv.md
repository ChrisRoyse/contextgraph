# Session Report: Phase 3 Training + FSV

Date: 2026-05-10

## Scope
- Implemented `crates/context-graph-mejepa-train` for Phase 3 training tasks TRAIN-001..014.
- Added UTML learning-signal primitives locally because no UTML crate existed in the workspace.
- Added real RocksDB source-of-truth CFs for per-step certs, holdout reports, epoch witness entries, and weight blobs.

## Evidence
- Unit tests: `cargo test -p context-graph-mejepa-train --lib` passed 18/18.
- Clippy: `cargo clippy -p context-graph-mejepa-train --all-targets -- -D warnings` passed.
- FSV examples:
  - `train_smoke_fsv`: 100 persisted certs, chain verified, target_collapse violations empty.
  - `train_cert_chain_fsv`: 100,000 persisted certs, replay verified 100,000 entries in ~1.78s.
  - `train_holdout_eval_fsv`: persisted holdout report, agreement 0.875, 8 category keys, 11 language keys.
  - `train_full_fsv`: verified four primary evidence files and modes.
  - `train_edge_cases_fsv`: verified fail-closed target_collapse, tampered chain, and empty holdout before/after states.

## Source Of Truth
- `/tmp/contextgraph-mejepa-train-fsv/smoke-rocksdb`
- `/tmp/contextgraph-mejepa-train-fsv/chain-rocksdb`
- `/tmp/contextgraph-mejepa-train-fsv/holdout-rocksdb`
- `/tmp/contextgraph-mejepa-train-fsv/edge-rocksdb`
- Evidence JSON files in `/tmp/contextgraph-mejepa-train-fsv/*.json`, all mode `0600`.

## Notes
- Phase 0 corpus PRNG was made public and gained state save/restore helpers so Phase 3 sampler checkpoint/resume can be byte-exact.
- The shipped AdamW optimizer has global decoupled weight decay; Phase 3 wiring excludes LayerNorm/bias tensors rather than applying incorrect decay.
