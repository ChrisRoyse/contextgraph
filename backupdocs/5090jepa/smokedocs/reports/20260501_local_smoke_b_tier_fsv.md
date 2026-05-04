# 2026-05-01 Local Smoke-B Tier FSV

## Scope

Completed the next doc 23 implementation step: promote the free/local CI gate
from Tier A to a passing Tier B run on the local RTX 5090 using the verified
O*NET 30.2 text ZIP.

This report records the source-of-truth paths, root causes, code changes,
manual FSV reads, fail-closed edge audits, and remaining constraints.

## Source Of Truth

Primary successful run:

```text
run id: 20260501T_local_smoke_b_free_gate_v5
local CI manifest: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_local_ci_manifest.json
smoke bundle: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_smoke_a
paper-small A: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_paper_small_seed42_a
paper-small B: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_paper_small_seed42_b
HP sweep: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_hp_sweep_batch_size
manual FSV log root: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_manual_fsv
```

Reference inputs:

```text
O*NET ZIP: tmp/onet_probe/db_30_2_text.zip
O*NET ZIP SHA-256: b5479271931796b838f7173dc0f673a9ec961b7833ac87168fd11e92e7453741
system specs: docs2/systemspecs.md
system specs SHA-256: acdc549a5913c59f46376414c08373a8363d646fd3c36d1fc6289a193c0e1205
```

Manual FSV readbacks were written under:

```text
tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_manual_fsv/
00_local_ci_manifest_readback.json
01_bundle_manifest_fsv_readback.jsonl
02_hp_sweep_manifest_readback.json
03_hp_sweep_fsv_readback.json
04_rocksdb_counts_readback.jsonl
05_artifact_data_readback_v2.jsonl
06_table_readbacks.txt
07_plot_readbacks.txt
08_edge_case_audit.txt
```

## Root Causes

The remaining blocker was not CUDA capacity. The 120-event HP sweep proxy was
too small for the `batch_size=32` transition ablation path. It failed at A2
with `val_latent_mse` far above the convergence target. A real 1K O*NET probe
passed, so the CI gate now uses `--career-event-count 1000` for Tier B HP
sweeps.

Batch-size sweeps previously changed batch size without preserving optimizer
step budget. The sweep now scales epochs for the batch-size axis. With base
`batch_size=16`, `epochs=1000`, and 1K O*NET rows, `batch_size=32` cells persist
`epochs=2000`. The persisted epoch value is verified from child bundle
hyperparameters and emitted into both HP sweep tables.

During investigation, `inspect-counts` and `inspect-cf` were found to create a
missing RocksDB path because they used the normal writer/open path. That could
turn a read-only inspection into a mutated source of truth. Inspection commands
now open existing stores only. The smoke verifier uses explicit hidden
`--allow-missing` for pre-creation before-state reads and returns zero counts
without creating a directory.

`cargo clippy --all-targets -D warnings` found two real integration issues
after the inspection change: the MCP `InspectCountsArgs` initializer needed the
new field, and the top-level CLI enum crossed clippy's `large_enum_variant`
threshold. The MCP initializer is strict by default, and the DynamicJEPA
subcommand is boxed in the CLI enum.

## Implementation Changes

- `scripts/5090jepa/local_ci_gate.sh`: Tier B HP sweep now runs 1K real O*NET
  events.
- `crates/context-graph-cli/src/commands/dynamicjepa.rs`: added hidden
  `--allow-missing`, read-only inspection opens, HP epoch override, batch-size
  epoch scaling, persisted HP epoch verification, and `epochs` columns in HP
  sweep tables.
- `crates/context-graph-storage/src/teleological/rocksdb_store/{types,store}.rs`:
  added `create_missing_column_families` to store config so read-only opens do
  not silently repair/create incomplete stores.
- `crates/context-graph-cli/src/main.rs`: boxed the DynamicJEPA subcommand for
  clippy.
- `crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs`: MCP
  inspection uses strict `allow_missing=false`.
- Phase 3/4/5/6/7/8 FSV tests now mark intentional missing-DB before-state
  reads with `--allow-missing`.

## Successful Gate

Command:

```bash
LOCAL_CI_RUN_ID_PREFIX=20260501T_local_smoke_b_free_gate_v5 \
  scripts/5090jepa/local_ci_gate.sh smoke-b
```

Result:

```text
status: passed
exit_code: 0
manifest: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_local_ci_manifest.json
```

The local gate is free/local only. It scans workflow files and rejects
GitHub-hosted runners, artifact upload, cache, Codecov, and similar paid or
external reporting references.

## Manual FSV Evidence

Manifest readback:

```text
status=passed
exit_code=0
tier=smoke-b
evidence_paths=6
```

Bundle readback:

```text
smoke_a: run_manifest=passed, fsv_report=passed, career_event_count=120
paper_small_seed42_a: run_manifest=passed, fsv_report=passed, career_event_count=1000
paper_small_seed42_b: run_manifest=passed, fsv_report=passed, career_event_count=1000
```

Career RocksDB readback:

```text
smoke_a: raw=124, panels=120, transitions=120, training_runs=1, model_artifacts=1, predictions=6, guards=5, verification_runs=1
paper_a: raw=1004, panels=1000, transitions=1000, training_runs=1, model_artifacts=1, predictions=6, guards=5, verification_runs=1
paper_b: raw=1004, panels=1000, transitions=1000, training_runs=1, model_artifacts=1, predictions=6, guards=5, verification_runs=1
hp batch16 seed42: raw=1004, panels=1000, transitions=1000, training_runs=1, model_artifacts=1, predictions=6, guards=5, verification_runs=1
hp batch32 seed42: raw=1004, panels=1000, transitions=1000, training_runs=1, model_artifacts=1, predictions=6, guards=5, verification_runs=1
hp batch16 seed43: raw=1004, panels=1000, transitions=1000, training_runs=1, model_artifacts=1, predictions=6, guards=5, verification_runs=1
hp batch32 seed43: raw=1004, panels=1000, transitions=1000, training_runs=1, model_artifacts=1, predictions=6, guards=5, verification_runs=1
```

Artifact and data readback:

```text
artifact_mismatch_count=0 for smoke_a, paper_a, and paper_b
O*NET source preflight status=passed for all three bundles
paper bundles source row_count=1000
source_occupation_count=1016
selected_occupation_count=218
selected_related_edge_count=330
```

Table and plot readback:

```text
smoke_a prediction rows=3, ablation rows=6, edge rows=11
paper_a prediction rows=3, ablation rows=6, edge rows=11
paper_b prediction rows=3, ablation rows=6, edge rows=11
HP sensitivity rows=4
HP collapse diagnostic rows=4
all checked SVG/plot manifest files existed and had nonzero byte size
```

HP sweep readback:

```text
status=passed
cell_count=4
career_event_count=1000
seeds=42,43
sweep_values=16,32
batch_size_16_seed42: epochs=1000, fsv_status=passed
batch_size_32_seed42: epochs=2000, fsv_status=passed
batch_size_16_seed43: epochs=1000, fsv_status=passed
batch_size_32_seed43: epochs=2000, fsv_status=passed
```

## Edge Audits

All edge audits printed before and after state in
`tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_manual_fsv/08_edge_case_audit.txt`.

1. Unsupported tier:
   `not-a-tier` exited `2`, printed `LOCAL_CI_INVALID_TIER`, and did not create
   a manifest.

2. Missing O*NET ZIP:
   `ONET_TEXT_ZIP=tmp/onet_probe/does_not_exist_for_edge.zip` exited `1`,
   printed `LOCAL_CI_ONET_ZIP_MISSING`, wrote a failed manifest, and created no
   bundles.

3. Existing bundle path:
   pre-created `tmp/mejepa_local_ci_runs/20260501T_edge_existing_root_v1_smoke_a`
   exited `1`, printed `LOCAL_CI_SMOKE_A_ROOT_EXISTS`, wrote a failed manifest,
   and preserved the sentinel file SHA-256 unchanged.

4. Missing RocksDB inspection:
   strict `inspect-counts` on a missing DB exited `1`, returned a validation
   error, and did not create a DB path. Hidden `--allow-missing` exited `0`,
   returned `db_exists=false` with zero counts, and still did not create a DB
   path.

## Verification Commands

Additional targeted FSV tests passed after the code changes:

```bash
cargo test -p context-graph-cli \
  --test dynamicjepa_phase3_fsv \
  --test dynamicjepa_phase7_fsv \
  --test dynamicjepa_phase8_fsv \
  --all-features -- --nocapture
```

Observed result:

```text
dynamicjepa_phase3_cli_fsv_real_rocksdb ... ok
dynamicjepa_phase7_cli_fsv_real_cuda_artifacts ... ok
dynamicjepa_phase8_cli_fsv_real_prediction_plan_surprise ... ok
```

## Preserved Failures

The failed bundles and logs were not deleted:

```text
tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v1_paper_small_seed42_a
tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v2_paper_small_seed42_b
tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v3_hp_sweep_batch_size
tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v4_local_ci_manifest.json
tmp/mejepa_local_ci_runs/20260501T_hp_sweep_batch32_budget_probe_v4
```

These preserve the stale paper-small reference failure, the shuffled-target
tolerance failure, the 120-row HP sweep convergence failure, the clippy/MCP
integration failure, and the post-budget 120-row A2 failure.

## Research Notes

GitHub Actions billing documentation distinguishes self-hosted runner execution
from billed hosted minutes/storage. The local gate therefore keeps evidence on
disk and rejects hosted runners, upload-artifact, cache, and third-party
reporting references.

Reference: https://docs.github.com/en/billing/concepts/product-billing/github-actions

PyTorch's reproducibility guidance notes that exact reproducibility is not
guaranteed across releases, platforms, or CPU/GPU execution. Tier B therefore
uses table structure plus numeric tolerances for same-seed CUDA comparison
instead of exact model-file byte equality.

Reference: https://docs.pytorch.org/docs/2.11/notes/randomness.html

Google's deep learning tuning playbook emphasizes treating training duration
and step budget as meaningful experiment controls. The HP sweep now preserves
optimizer-step budget across batch-size cells and persists the resulting epoch
count for FSV.

Reference: https://developers.google.com/machine-learning/guides/deep-learning-tuning-playbook/number-of-steps

## Remaining Work

Tier B is now complete for doc 23. The next doc 23-adjacent step is release-C
hardening: five paper-small seeds, aggregate readback, release reference
comparison, and container/reproduce gates where release prerequisites exist.
