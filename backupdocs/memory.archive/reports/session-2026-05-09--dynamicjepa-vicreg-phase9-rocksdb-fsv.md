---
date: 2026-05-09
type: session-report
topic: dynamicjepa-vicreg-phase9-rocksdb-fsv
status: verified
---

# DynamicJEPA VICReg and Phase9 RocksDB FSV

## Context

Read the ruvector findings docs, the SOLO Agent Memory Protocol, recent
memories, and recent git history. RuVector is reference-only clean-room
research context; current implementation direction is ContextGraph reality
compiler / DynamicJEPA with full-state verification and fail-closed behavior.

Web research used primary sources only:
- VICReg paper: https://arxiv.org/abs/2105.04906
- Official Facebook Research VICReg implementation:
  https://raw.githubusercontent.com/facebookresearch/vicreg/main/main_vicreg.py

Research conclusion: VICReg variance and covariance terms are not optional
cosmetic losses. They are batch-statistical anti-collapse regularizers. The
fix should strengthen the training contract and quality gates, not relax gates
or add fallbacks.

## Root Causes

1. DynamicJEPA Phase7/Phase9 used a 2D frozen target projection. The Phase7
   counter fixture has a constant action delta in one path, which made one
   target coordinate low-information. With weaker VICReg regularization and a
   short training budget, the online latent could satisfy MSE while collapsing a
   dimension or leaking shuffled target similarity.

2. Phase9 MCP FSV previously used a fixed runtime evidence directory. In the
   full parallel MCP suite this allowed another test process to observe a
   partial RocksDB directory before CURRENT/MANIFEST existed, producing an
   intermittent source-of-truth open failure.

3. Existing RocksDB inspect preflight did not fully validate that a discovered
   DB directory had a readable CURRENT pointer to a non-empty MANIFEST file.

## Current Fix State

Current HEAD includes the Phase9 unique runtime root fix:
`c5da2166 mcp/dynamicjepa: parallel-safe phase9 FSV runtime root (pid + uuid)`.

Current HEAD also includes the committed DynamicJEPA tiny config hardening:
2D frozen target, `vicreg_variance_weight=8.0`,
`vicreg_covariance_weight=1.5`, `epochs=400`, `max_seconds=180`, and the
Phase7 trainable parameter expectation of 180.

Current worktree has uncommitted robustness edits in:
- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/witness_chain_repair_fsv.rs`
- `crates/context-graph-solver/examples/tier1_fsv.rs`
- `crates/context-graph-solver/src/conformal.rs`
- `crates/context-graph-solver/src/mmr.rs`
- `crates/context-graph-witness/examples/ed25519_fsv.rs`
- `crates/context-graph-witness/src/sign.rs`

The CLI edit strengthens `preflight_existing_rocksdb` to reject regular files,
directories without CURRENT, invalid/path-traversing CURRENT pointers, missing
manifest files, and empty manifest files. It adds unit tests for those cases.
The solver/witness edits add stronger fail-closed checks, documentation, and
example-level evidence readback assertions.

## Full-State Verification Evidence

Phase7 CLI source of truth:
`tmp/5090jepa_evidence/phase7/cli_phase7_fsv_runtime/dynamicjepa_phase7_counter_rocksdb`

Direct `inspect-counts` readback after rerun:
- `dj_raw_events=60`
- `dj_actions=60`
- `dj_latent_panels=60`
- `dj_dataset_shards=3`
- `dj_training_runs=1`
- `dj_model_artifacts=1`
- `dj_audit_log=307`
- `dj_audit_witness_chain=307`

Phase7 artifact metrics readback:
- `val_latent_mse=0.00009189393105032047`
- `val_latent_cosine=0.9999969125283464`
- `vicreg_variance_per_dim_min=0.572641043498592`
- `shuffled_target_cosine=0.03242301209550708`
- `vicreg_covariance_off_diag_rms=0.7171646894791455`
- `parameter_count_trainable=180.0`

Phase9 MCP source of truth:
`tmp/5090jepa_evidence/phase9/mcp_phase9_fsv_runtime_1432706_467d47a2-a1bb-41b7-b6eb-93169bc77cde/dynamicjepa_phase9_mcp_rocksdb`

Direct `inspect-counts` readback after final full MCP suite:
- `dj_raw_events=61`
- `dj_actions=66`
- `dj_latent_panels=60`
- `dj_dataset_shards=3`
- `dj_training_runs=1`
- `dj_model_artifacts=1`
- `dj_predictions=6`
- `dj_plan_traces=1`
- `dj_surprise_events=1`
- `dj_audit_log=315`
- `dj_audit_witness_chain=315`

Phase9 test printed decoded persisted `dj_predictions`,
`dj_surprise_events`, and `dj_model_artifacts` rows, including artifact
`5501218d-5caa-41b0-8b6e-c60480ed9243` and files
`config.json`, `evaluation_report.json`, `manifest.json`, `metrics.json`, and
`model.safetensors`.

## Edge Cases Manually Exercised

DynamicJEPA Phase9 MCP FSV printed before/after state for:
- Empty domain-pack id: validation error, DB absent/unchanged.
- Duplicate domain registration: `VALIDATION`, counts unchanged.
- Invalid column family: `VALIDATION`, counts unchanged.
- Missing model artifact during predict: `SOURCE_OF_TRUTH_MISSING`,
  `dj_predictions` unchanged at zero.
- Existing cross-domain transfer output root: `VALIDATION`, directory state
  unchanged.

CLI RocksDB preflight unit tests cover:
- Regular file source-of-truth path rejected without mutation.
- Directory missing CURRENT rejected without mutation.
- Invalid/path-traversing CURRENT pointer rejected.
- CURRENT pointing to missing MANIFEST rejected.
- CURRENT pointing to non-empty MANIFEST accepted.

Solver Tier-1 example wrote and read back
`/tmp/contextgraph-tier1-fsv/solver-tier1-evidence.json`, asserting MMR order,
conformal thresholds, rank inclusion, and fail-closed boundary error codes.

Witness ed25519 example wrote and read back
`/tmp/contextgraph-tier1-fsv/witness-ed25519-evidence.json`, asserting segment
length, codec length, disk roundtrip equality, tamper rejection, pinned-key
rejection, and short-seed rejection.

## Verification Commands

Passed:
- `cargo test -p context-graph-cli --test dynamicjepa_phase7_fsv -- --nocapture`
- `cargo test -p context-graph-mcp --lib test_dynamicjepa_phase9_mcp_full_state_verification -- --nocapture`
- `cargo test -p context-graph-mcp --lib -- --nocapture`
- `cargo test -p context-graph-cli rocksdb_preflight -- --nocapture`
- `cargo test -p context-graph-solver --lib -- --nocapture`
- `cargo test -p context-graph-solver --examples -- --nocapture`
- `cargo run -p context-graph-solver --example tier1_fsv`
- `cargo test -p context-graph-witness --lib -- --nocapture`
- `cargo test -p context-graph-witness --features ed25519 --lib -- --nocapture`
- `cargo run -p context-graph-witness --features ed25519 --example ed25519_fsv`
- `cargo test -p context-graph-mcp --lib witness_chain_repair -- --nocapture`
- `cargo fmt --all`
- `git diff --check`

Final full MCP suite result against current working tree:
`411 passed; 0 failed; 6 ignored; finished in 70.28s`.

## Residual Notes

The worktree is intentionally dirty with the files listed above. These edits
were verified in-place and should not be reverted without reviewing their
source-of-truth and fail-closed behavior.
