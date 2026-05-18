---
date: 2026-05-09
type: report
tags: smap,ruvector,tier1,dynamicjepa,witness,solver,fsv
---

# Session Report: RuVector Tier-1 + DynamicJEPA FSV hardening

## Summary

Implemented and verified the RuVector-inspired Tier-1 clean-room pieces and
closed the DynamicJEPA/RocksDB FSV failure mode without fallbacks.

## Commits at handoff

- `9e50929a fsv+preflight: strict readback assertions + RocksDB CURRENT manifest validation`
- `c5da2166 mcp/dynamicjepa: parallel-safe phase9 FSV runtime root (pid + uuid)`
- `23a5e209 cli/dynamicjepa: preflight RocksDB existence with directory + CURRENT file checks`
- `56a0c6fe solver: rename InverseRank->Rank with correct polarity + MMR similarity range gate + tier1 FSV`
- `01b3263b witness/sign: fail-closed segment validation + weak-key + strict verify + ed25519 FSV`

## Full State Verification

Source of truth:

- DynamicJEPA: RocksDB column families under
  `tmp/5090jepa_evidence/phase9/mcp_phase9_fsv_runtime_1599452_833dc708-1336-470e-86ab-e2972a43daf0/dynamicjepa_phase9_mcp_rocksdb`.
- Solver/witness Tier-1 examples: files under `/tmp/contextgraph-tier1-fsv/`.

Evidence read after execution:

- DynamicJEPA `inspect-counts` read back:
  `dj_raw_events=61`, `dj_transitions=61`, `dj_training_runs=1`,
  `dj_model_artifacts=1`, `dj_predictions=6`, `dj_audit_log=315`,
  `dj_audit_witness_chain=315`.
- `dj_training_runs` row read back:
  `status=Completed`, `parameter_count_trainable=180`,
  `val_latent_mse=0.0039066545650910086`,
  `val_latent_cosine=0.9998904050804506`,
  `vicreg_variance_per_dim_min=0.9203901322105109`,
  `shuffled_target_cosine=0.062355396595818006`.
- `dj_model_artifacts` row read back:
  `status=Active`, `model.safetensors size=1384`,
  sha256 `72ca86da7071a2abdb6d92bc5e4c197104b0f8c0c295ba53141804f016596020`.
- RocksDB preflight read back:
  `CURRENT=MANIFEST-000466`, manifest size `17157`.
- Solver evidence file read back:
  MMR expected/actual orders match; conformal rank predicted indices `[0,1,3]`
  and top-ranked candidate included.
- Witness evidence file read back:
  chain entries `3`, segment bytes `219`, signature codec `96`, tampered
  signature and key-pin mismatch fail closed with expected codes.

Edge cases manually exercised:

- Missing DB path with `allow-missing`: returns zero counts and creates no path.
- Incomplete DB directory: returns structured `STORAGE` error before RocksDB
  opens and leaves only the directory itself.
- DynamicJEPA Phase 9 edges: empty id, duplicate domain registration, invalid CF,
  missing model artifact, and existing cross-domain-transfer output all preserve
  before/after source-of-truth counts.
- Solver edges: negative lambda, zero limit, NaN relevance, out-of-range
  similarity, uncalibrated conformal prediction, alpha zero.
- Witness signing edges: empty segment, truncated segment, broken segment link,
  tampered segment byte, tampered signature byte, pinned public-key mismatch,
  short seed.

## Verification Commands

- `cargo fmt -p context-graph-cli -p context-graph-mcp -p context-graph-solver -p context-graph-witness -- --check`
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

## Residual Risk

Ignored active-runtime repair tests remain intentionally ignored because they
mutate/read the active ccreality runtime and require operator-provided
`CG_EXPECTED_LEGACY_WITNESS_SHA256`.

