# 2026-04-30 Smoke Test Repair FSV

## Scope

This report records the repair and full-state verification for the two failing smoke-test paths that blocked further 5090 ME-JEPA work:

1. `tests/test_5090jepa_phase0.py` failed because the Phase 0 verifier was stale relative to the checked-in Phase 10 generated fixtures and the current FSV protocol.
2. `crates/context-graph-cli/tests/dynamicjepa_phase10_fsv.rs` failed because collapse diagnostics were being computed during every epoch-level stopping evaluation, causing gridworld to exceed its configured `max_seconds` budget before convergence.

While promoting the repair to the full workspace smoke gate, several benchmark binaries also failed to compile under `--workspace --all-features`; those were API drift fixes, not behavior workarounds.

## Source Of Truth

- Phase 0 fixture contract: `configs/dynamicjepa/**` files on disk, plus `docs/5090jepa/14_full_state_verification_protocol.md`.
- Phase 0 verification evidence: `tmp/5090jepa_test_phase0/phase0_fixture_verification.json` and manual read-only evidence `tmp/5090jepa_manual_edges/phase0_manual_verify.json`.
- Phase 10 runtime state: RocksDB column families under:
  - `tmp/5090jepa_evidence/phase10/test_runtime/counter_world_rocksdb`
  - `tmp/5090jepa_evidence/phase10/test_runtime/gridworld_5x5_rocksdb`
  - `tmp/5090jepa_evidence/phase10/test_runtime/career_taxonomy_rocksdb`
- Research smoke bundle state: `tmp/5090jepa_evidence/research_smoke/test_runtime/test_research_smoke_66840`.
- Research smoke bundle manifests/tables/plots:
  - `run_manifest.json`
  - `fsv_report.json`
  - `data_manifest.json`
  - `paper_tables/table_prediction_metrics.csv`
  - `paper_tables/table_ablation.csv`
  - `paper_tables/table_edge_cases.csv`
  - `plots/plot_manifest.json`

## External References Checked

- Cargo documents that `cargo test` compiles and runs test targets, with `--no-fail-fast` available when complete failure inventory is needed: https://doc.rust-lang.org/cargo/commands/cargo-test.html
- The Cargo testing guide describes integration-style tests under `tests/`: https://doc.rust-lang.org/stable/cargo/guide/tests.html
- GitHub Actions CI guidance treats build and test automation as the gate that catches introduced errors before merge: https://docs.github.com/en/actions/concepts/overview/continuous-integration
- GitHub's Rust workflow guide confirms the standard CI shape for Rust build/test jobs: https://docs.github.com/en/actions/tutorials/build-and-test-code/rust

These references support keeping the broad workspace smoke gate as a compile-and-run gate rather than narrowing it to only the failing package.

## Root Causes

### Phase 0 verifier

The verifier still encoded an older three-event counter-world contract:

- Expected counter rows were `3` events, `3` adapter runs, `6` instrument readings, and `2` dataset shards.
- The current checked-in counter-world pack and protocol describe the generated Phase 10 verifier: `60` accepted events, `62` raw/adapter rows including failed edge probes, `120` instrument readings, and `3` dataset shards.
- Gridworld train-config expectations were stale: expected `lr=0.0005`, `warmup_steps=200`, `epochs=30`, `max_seconds=90`; current tracked config uses `lr=0.001`, `warmup_steps=50`, `epochs=120`, `max_seconds=240`.
- Gridworld split expectations used nominal integer percentages; current deterministic split is exact `train=4977`, `val=1330`, `test=693`.

### Phase 10 smoke

The model evaluation path computed full collapse diagnostics for baseline and every epoch-level stopping evaluation. Those diagnostics require additional matrix/statistical work over the validation tensors and made gridworld exceed its real training time budget:

`TRAINING_FAILED ... max_seconds budget 240 exceeded before convergence at epoch 21 step 0`

The persisted DB after that failure proved partial execution:

- `dj_training_runs=1`
- `dj_model_artifacts=0`
- `dj_predictions=0`
- `dj_plan_traces=0`
- `dj_guard_decisions=0`

This was not a convergence issue in the model or a reason to raise the time budget. It was instrumentation cost in the stopping loop.

### Workspace smoke compile gate

The full workspace smoke compile also exposed API drift in benchmark binaries:

- Missing imports in `sparse_bench.rs`, `gpu_bench.rs`, and `mcp_bench.rs`.
- Stale `Handlers::with_defaults` calls missing the graph discovery service argument and `Result` handling.
- Stale `CausalModel::new()` call missing path/config arguments.
- Stale `CausalDiscoveryConfig` initializer missing new defaulted fields.
- Missing `EMBEDDING_DIMS` in `graph_linking_gpu_bench.rs`.

These were corrected to match current APIs so `cargo test --workspace --all-features smoke` compiles the whole workspace.

## Fixes Applied

- Updated `scripts/5090jepa/verify_phase0.py` locked hashes and expected Phase 10 fixture/count contracts.
- Updated `tests/test_5090jepa_phase0.py` to assert the current counter-world instrument-readings contract.
- Updated `docs/5090jepa/14_full_state_verification_protocol.md` to record the generated 60-event counter verifier and exact current final counts.
- Updated `crates/context-graph-dynamicjepa/src/model/mod.rs` so collapse diagnostics are:
  - skipped for baseline evaluation,
  - skipped for per-epoch stopping checks,
  - computed once for final evaluation and persisted into flattened metrics/evaluation reports.
- Updated benchmark binaries to current public APIs without adding mock paths or fallback behavior.

## Execute And Inspect

### Phase 0

Command:

```bash
python3 tests/test_5090jepa_phase0.py -v
```

Result: passed `2/2`.

Separate source-of-truth readback from `tmp/5090jepa_test_phase0/phase0_fixture_verification.json`:

```json
{
  "status": "ok",
  "source_of_truth": {
    "primary": "configs/dynamicjepa files on disk",
    "secondary": "docs/5090jepa/14_full_state_verification_protocol.md"
  },
  "counter_world_final_counts_from_protocol": {
    "dj_raw_events": 62,
    "dj_adapter_runs": 62,
    "dj_instrument_readings": 120,
    "dj_dataset_shards": 3,
    "dj_audit_log": 196
  },
  "gridworld_dataset_row_expectations": {
    "train": 4977,
    "val": 1330,
    "test": 693
  }
}
```

### Phase 10

Command:

```bash
cargo test -p context-graph-cli --test dynamicjepa_phase10_fsv --all-features -- --nocapture
```

Result: passed `1/1`.

Separate RocksDB readbacks:

```json
{
  "counter_world": {
    "dj_raw_events": 62,
    "dj_adapter_runs": 62,
    "dj_instrument_readings": 120,
    "dj_dataset_shards": 3,
    "dj_training_runs": 1,
    "dj_model_artifacts": 1,
    "dj_predictions": 6,
    "dj_plan_traces": 1,
    "dj_guard_decisions": 5,
    "dj_surprise_events": 1,
    "dj_audit_log": 196
  },
  "gridworld_5x5": {
    "dj_raw_events": 8002,
    "dj_adapter_runs": 8002,
    "dj_instrument_readings": 24000,
    "dj_dataset_shards": 3,
    "dj_training_runs": 1,
    "dj_model_artifacts": 1,
    "dj_predictions": 6,
    "dj_plan_traces": 1,
    "dj_guard_decisions": 5,
    "dj_surprise_events": 0,
    "dj_audit_log": 25014
  },
  "career_taxonomy": {
    "dj_raw_events": 19,
    "dj_adapter_runs": 19,
    "dj_instrument_readings": 150,
    "dj_dataset_shards": 3,
    "dj_training_runs": 1,
    "dj_model_artifacts": 1,
    "dj_predictions": 6,
    "dj_plan_traces": 1,
    "dj_guard_decisions": 5,
    "dj_audit_log": 64
  }
}
```

Training metric readbacks confirmed final diagnostics were physically persisted:

```json
{
  "career_taxonomy": {
    "collapse_diagnostics_computed": 1.0,
    "collapse_eff_rank": 4.250025730542526,
    "collapse_latent_collapsed": 0.0,
    "val_latent_mse": 0.19737686395995757
  },
  "counter_world": {
    "collapse_diagnostics_computed": 1.0,
    "collapse_eff_rank": 1.012378682022522,
    "collapse_latent_collapsed": 0.0,
    "val_latent_mse": 0.004330990884665671
  },
  "gridworld_5x5": {
    "collapse_diagnostics_computed": 1.0,
    "collapse_eff_rank": 24.959747857242522,
    "collapse_latent_collapsed": 0.0,
    "val_latent_mse": 0.03998297027601839
  }
}
```

Artifact hash readback: all three Phase 10 domains had `5` model artifact files each and `0` hash mismatches.

### Full workspace smoke

Command:

```bash
cargo test --workspace --all-features smoke -- --nocapture
```

Result: passed.

Additional targeted smoke confirmations:

```bash
cargo test -p context-graph-dynamicjepa --test cuda_smoke --all-features -- --nocapture
cargo test -p context-graph-cuda --test native_cubin_smoke --all-features -- --nocapture
cargo fmt --check
```

Results:

- DynamicJEPA CUDA smoke passed `1/1` and printed tensor copyback source-of-truth: `[[58.0, 64.0], [139.0, 154.0]]`.
- Native CUBIN smoke passed `3/3`.
- Formatting check passed.

## Research Smoke Bundle Readback

Bundle: `tmp/5090jepa_evidence/research_smoke/test_runtime/test_research_smoke_66840`.

Manifest readbacks:

```json
{
  "run_manifest_status": "passed",
  "fsv_report_status": "passed",
  "purpose": "smoke",
  "career_event_count": 120,
  "career_training_seed": 42
}
```

Career RocksDB readback from the bundle:

```json
{
  "dj_raw_events": 124,
  "dj_adapter_runs": 124,
  "dj_instrument_readings": 1200,
  "dj_latent_panels": 120,
  "dj_dataset_shards": 3,
  "dj_training_runs": 1,
  "dj_model_artifacts": 1,
  "dj_predictions": 6,
  "dj_guard_decisions": 5,
  "dj_plan_traces": 1,
  "dj_verification_runs": 1,
  "dj_audit_log": 379
}
```

Required generated files exist:

```text
data_manifest.json                              2603 bytes
paper_tables/table_prediction_metrics.csv       804 bytes
paper_tables/table_ablation.csv                2183 bytes
paper_tables/table_edge_cases.csv              2883 bytes
plots/plot_manifest.json                        897 bytes
```

Artifact hashes in `fsv_report.json`: `9` domains, `5` files per domain, `0` mismatches.

## Boundary And Edge Case Audit

### Edge 1: paper_small without O*NET source

Trigger:

```bash
target/debug/context-graph dynamicjepa research-smoke \
  --run-id manual_edge_no_onet \
  --run-root tmp/5090jepa_manual_edges \
  --fixtures-root configs/dynamicjepa \
  --purpose paper_small \
  --json
```

Before state: target run directory did not exist.

After state:

```text
exit_code=1
run_dir_exists=false
error_code=VALIDATION
field=research_smoke.onet_text_zip
```

Logged remediation:

```text
download https://www.onetcenter.org/dl_files/database/db_30_2_text.zip and pass its local path
```

Expected outcome was fail-closed before bundle creation. Verified.

### Edge 2: career training seed without O*NET source

Trigger:

```bash
target/debug/context-graph dynamicjepa research-smoke \
  --run-id manual_edge_seed_no_onet_75893 \
  --run-root tmp/5090jepa_manual_edges \
  --fixtures-root configs/dynamicjepa \
  --purpose smoke \
  --career-training-seed 777 \
  --json
```

Before/after state printed by the manual audit:

```text
BEFORE edge=seed_no_onet run_dir=tmp/5090jepa_manual_edges/manual_edge_seed_no_onet_75893 exists=false
AFTER edge=seed_no_onet exit_code=1 run_dir=tmp/5090jepa_manual_edges/manual_edge_seed_no_onet_75893 exists=false stdout_bytes=546 stderr_bytes=473
```

Structured error:

```text
error_code="VALIDATION"
field=research_smoke.career_training_seed
remediation=supply --onet-text-zip so research-smoke can materialize an O*NET career train config with the requested seed
```

Expected outcome was fail-closed before bundle creation. Verified.

### Edge 3: invalid run id containing slash

Trigger:

```bash
target/debug/context-graph dynamicjepa research-smoke \
  --run-id manual_edge_bad/run_75894 \
  --run-root tmp/5090jepa_manual_edges \
  --fixtures-root configs/dynamicjepa \
  --purpose smoke \
  --json
```

Before/after state printed by the manual audit:

```text
BEFORE edge=invalid_run_id run_dir=tmp/5090jepa_manual_edges/manual_edge_bad/run_75894 exists=false parent_exists=false
AFTER edge=invalid_run_id exit_code=1 run_dir=tmp/5090jepa_manual_edges/manual_edge_bad/run_75894 exists=false parent_exists=false stdout_bytes=481 stderr_bytes=436
```

Structured error:

```text
error_code="VALIDATION"
field=research_smoke.run_id
remediation=use only ASCII letters, digits, dot, dash, or underscore; do not include slashes or empty IDs
```

Expected outcome was fail-closed before creating either the nested run directory or its parent. Verified.

### Edge 4: read-only Phase 0 verifier state audit

Trigger:

```bash
python3 scripts/5090jepa/verify_phase0.py \
  --evidence tmp/5090jepa_manual_edges/phase0_manual_verify.json
```

Before/after state printed by the manual audit:

```text
BEFORE edge=phase0_readonly evidence=tmp/5090jepa_manual_edges/phase0_manual_verify.json exists=false counter_sha=46b8707285022da76221e445ed8d7b4e6e0ab87a707090a6ddf2759141a1d731 grid_train_config_sha=5af5849c2c1e2dc940cec4c8365e0dfdedfcde5257a56992cdeb330cbe86cf3f
AFTER edge=phase0_readonly exit_code=0 evidence_exists=true counter_sha=46b8707285022da76221e445ed8d7b4e6e0ab87a707090a6ddf2759141a1d731 grid_train_config_sha=5af5849c2c1e2dc940cec4c8365e0dfdedfcde5257a56992cdeb330cbe86cf3f
```

Evidence readback:

```json
{
  "status": "ok",
  "source_of_truth": {
    "primary": "configs/dynamicjepa files on disk",
    "secondary": "docs/5090jepa/14_full_state_verification_protocol.md"
  },
  "counter_world_final_counts_from_protocol": {
    "dj_raw_events": 62,
    "dj_adapter_runs": 62,
    "dj_instrument_readings": 120,
    "dj_audit_log": 196
  },
  "gridworld_dataset_row_expectations": {
    "train": 4977,
    "val": 1330,
    "test": 693
  }
}
```

Expected outcome was no mutation of tracked fixture source of truth, plus a persisted evidence JSON file. Verified.

## Conclusion

The smoke-test failures were fixed at their root causes:

- Phase 0 now verifies the real current fixture/protocol contract.
- Phase 10 keeps heavy collapse diagnostics in the persisted final evaluation without spending that cost inside every stopping check.
- Full workspace smoke now compiles and passes under `--workspace --all-features`.
- Manual FSV confirmed persisted DB rows, manifests, tables, plots, artifact hashes, and fail-closed validation behavior.

No mock data, fallback path, or passing test that hides a broken state was added.
