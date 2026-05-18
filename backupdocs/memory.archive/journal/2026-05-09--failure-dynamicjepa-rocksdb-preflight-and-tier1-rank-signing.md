---
date: 2026-05-09
type: failure
tags: dynamicjepa,rocksdb,solver,witness,fsv,fail-closed
---

# DynamicJEPA RocksDB preflight + Tier-1 rank/signing hardening

## Symptom

The Phase 9 DynamicJEPA MCP FSV initially failed before training: an edge-case
inspection encountered a RocksDB directory containing `LOCK`, `LOG`, and
`OPTIONS-*`, but no `CURRENT`. Opening with `create_if_missing=false` then
returned `CURRENT: does not exist`, and the test could not continue.

The same session also exposed two Tier-1 correctness issues:

- Rank-based conformal scoring used inverse-rank polarity, which could include
  weak candidates while excluding the strongest candidate.
- Ed25519 witness segment signing accepted arbitrary byte slices and used normal
  verification instead of strict verification; segment structure was not checked
  before signing/verifying.

## Root Cause

- RocksDB read-only opens can still leave diagnostic/open artifacts when pointed
  at an incomplete directory. Inspection code only checked path existence, not
  whether the path was a complete RocksDB source of truth.
- The Phase 9 FSV reused a stable evidence directory, allowing stale partial DB
  directories to contaminate later runs.
- Rank conformal nonconformity must increase with worse rank. `1/(1+rank)` has
  the opposite polarity.
- Witness signatures must bind canonical witness segments, not arbitrary bytes.

## Fix

- `dynamicjepa` read-only DB opens now preflight `path`, `CURRENT`, and the
  referenced non-empty `MANIFEST-*` file before calling RocksDB.
- Phase 9 MCP FSV now uses a PID + UUID runtime root.
- Conformal rank scoring is now `Rank` with score = 0-based rank.
- MMR rejects similarities outside `[0,1]`.
- Witness Ed25519 signing requires non-empty 73-byte-entry segments with valid
  internal prev-hash links, rejects weak keys, and uses `verify_strict`.
- Tier-1 examples write evidence files and immediately read them back.

## Source-of-Truth Evidence

- DynamicJEPA RocksDB:
  `tmp/5090jepa_evidence/phase9/mcp_phase9_fsv_runtime_1599452_833dc708-1336-470e-86ab-e2972a43daf0/dynamicjepa_phase9_mcp_rocksdb`
- Counts after full MCP FSV:
  `dj_training_runs=1`, `dj_model_artifacts=1`, `dj_predictions=6`,
  `dj_audit_log=315`, `dj_audit_witness_chain=315`.
- Training source-of-truth row:
  `status=Completed`, `parameter_count_trainable=180`,
  `val_latent_mse=0.0039066545650910086`,
  `vicreg_variance_per_dim_min=0.9203901322105109`,
  `shuffled_target_cosine=0.062355396595818006`.
- Artifact files physically present:
  `config.json`, `evaluation_report.json`, `manifest.json`, `metrics.json`,
  `model.safetensors`.
- Tier-1 evidence:
  `/tmp/contextgraph-tier1-fsv/solver-tier1-evidence.json`,
  `/tmp/contextgraph-tier1-fsv/witness-ed25519-evidence.json`,
  `/tmp/contextgraph-tier1-fsv/witness-segment.{bin,sig,pub}`.

## Verification

- `cargo test -p context-graph-cli --lib rocksdb_preflight -- --nocapture`
- `cargo test -p context-graph-solver -- --nocapture`
- `cargo test -p context-graph-witness --features ed25519 -- --nocapture`
- `cargo test -p context-graph-mcp --lib witness_chain_repair -- --nocapture`
- `cargo run -p context-graph-solver --example tier1_fsv`
- `cargo run -p context-graph-witness --features ed25519 --example ed25519_fsv`
- `cargo test -p context-graph-mcp --lib`
- `cargo test -p context-graph-cli --test dynamicjepa_phase7_fsv -- --nocapture`
- `cargo build --release -p context-graph-mcp -p context-graph-cli -p context-graph-solver`
- `cargo build --release -p context-graph-witness --features ed25519`

