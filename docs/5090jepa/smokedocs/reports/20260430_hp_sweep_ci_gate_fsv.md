# 2026-04-30 HP Sweep / CI Gate Full-State Verification

## Scope

Implemented the doc 22/doc 23 integration gate for DynamicJEPA HP sweeps. This
work did not touch `clipcannon/`.

Primary source of truth:

- Sweep bundle:
  `tmp/mejepa_research_runs/20260430T_hp_sweep_smoke_batch16_v2`
- Sweep manifest:
  `tmp/mejepa_research_runs/20260430T_hp_sweep_smoke_batch16_v2/sweep_manifest.json`
- Sweep FSV report:
  `tmp/mejepa_research_runs/20260430T_hp_sweep_smoke_batch16_v2/sweep_fsv_report.json`
- Child career RocksDB:
  `tmp/mejepa_research_runs/20260430T_hp_sweep_smoke_batch16_v2/cells/batch_size_16_seed42/dbs/batch_size_16_seed42_career_taxonomy_rocksdb`

## Research Inputs

- NIST DOE handbook: design of experiments is a systematic approach for valid,
  defensible engineering conclusions under run/time/cost constraints:
  https://www.itl.nist.gov/div898/handbook/pmd/section3/pmd31.htm
- NIST OFAT warning: one-factor-at-a-time studies do not expose factor
  interactions, so the current reduced CI sweep is regression coverage only,
  not the final paper sensitivity proof:
  https://www.itl.nist.gov/div898/handbook/pri/section2/pri212.htm
- Wang & Isola, ICML/PMLR 2020: alignment and uniformity are established SSL
  representation diagnostics:
  https://proceedings.mlr.press/v119/wang20k.html

## Implementation

- Added `dynamicjepa hp-sweep`.
- Supported axes: `lr`, `latent_dim`, `batch_size`.
- Each cell is a full immutable `research-smoke` child bundle under `cells/`.
- HP overrides are written into generated O*NET train configs and read back
  from persisted artifact configs.
- `research-smoke` now emits `paper_tables/table_collapse_diagnostics.csv`.
- HP sweep bundles emit:
  - `sweep_manifest.json`
  - `sweep_fsv_report.json`
  - `paper_tables/table_hp_sensitivity.csv`
  - `paper_tables/table_collapse_diagnostics.csv`
  - `plots/hp_sensitivity.svg`
  - `plots/collapse_diagnostic_f5.svg`
- Tier B CI now runs a reduced real O*NET 4-cell sweep:
  `batch_size=16,32` x seeds `42,43`.
- Fixed clippy failures in the DynamicJEPA Jacobi eigenvalue routine by
  removing range loops used only for indexing.

## Root Cause Fixed During FSV

The first real run,
`tmp/mejepa_research_runs/20260430T_hp_sweep_smoke_batch16_v1`, failed after
the child research-smoke cell completed. The sweep FSV writer tried to hash
`sweep_fsv_report.json` before writing it.

Fix: removed the self-referential FSV report from the required-file hash list.
The report is still written and separately read back; its required-file hashes
cover the manifest, tables, and plots.

## Happy Path

Command:

```bash
CUDA_COMPUTE_CAP=120 target/debug/context-graph dynamicjepa hp-sweep \
  --base-config configs/dynamicjepa/verification/career_taxonomy_train_config.json \
  --sweep-axis batch_size \
  --sweep-values 16 \
  --output-root tmp/mejepa_research_runs/20260430T_hp_sweep_smoke_batch16_v2 \
  --fixtures-root configs/dynamicjepa \
  --purpose smoke \
  --career-event-count 120 \
  --career-training-seed 42 \
  --onet-text-zip tmp/onet_probe/db_30_2_text.zip \
  --system-specs docs2/systemspecs.md \
  --json
```

Before state: output root did not exist.  
After state: output root exists and command returned `status=ok`.

Readback:

- `sweep_manifest.json`: `status=passed`, `cell_count=1`,
  `sweep_axis=batch_size`, `sweep_values=[16]`.
- `sweep_fsv_report.json`: `status=passed`, `hp_sensitivity_rows=1`,
  `collapse_rows=1`.
- Child `run_manifest.json`: `status=passed`, `hyperparameter_origin` points
  back to the HP sweep bundle.

Career DB readback from `inspect-counts`:

```text
dj_raw_events=124
dj_latent_panels=120
dj_dataset_shards=3
dj_training_runs=1
dj_model_artifacts=1
dj_predictions=6
dj_guard_decisions=5
```

Centerpoint metrics:

```text
lr=0.001
latent_dim=24
batch_size=16
seed=42
val_latent_cosine=0.8776115717741934
val_latent_mse=0.17999517062789205
collapse_eff_rank_ratio=0.10463224596920466
collapse_alignment=0.25158559765665167
collapse_uniformity=-0.32215603716030655
latent_collapsed=0
```

Hash readback matched FSV report:

```text
7dfc1390cd65d44ebac6607f724d70955624b14f85839941e0ce2535f472db50  sweep_manifest.json
3ce016c798aca9a79e8651fd480cc74cd1d0aeed85eda044840dda04b88424d1  table_hp_sensitivity.csv
daa6e64e88371dc7a0b61d3542e5a8bc088c010d8e3493bab4402ae1ba9e4c1f  table_collapse_diagnostics.csv
f8c6b9219914341b9fccd3c02763cc8f509963f6bd192003352fde1ee4cb9fcd  hp_sensitivity.svg
93b0cb5a3253d8c93003ca34770f95e762dbb2e74a9b32919ab5a81a709cd918  collapse_diagnostic_f5.svg
9720abe4116cb12416cfaf5accebde2575810fa9ba350dcc06b2ea4377093a08  plot_manifest.json
```

## Manual Edge Cases

### Unknown Axis

Before: `tmp/mejepa_research_runs/20260430T_hp_sweep_edge_unknown_axis`
did not exist.

Result:

```text
error_code=VALIDATION
field=hp_sweep.sweep_axis
message contains: unknown HP sweep axis
exit_code=1
```

After: output root still did not exist.

### Invalid LR Value

Before: `tmp/mejepa_research_runs/20260430T_hp_sweep_edge_invalid_lr` did not
exist.

Result:

```text
error_code=VALIDATION
field=hp_sweep.sweep_values
message contains: not numeric
exit_code=1
```

After: output root still did not exist.

### Existing Output Root

Before: `tmp/mejepa_research_runs/20260430T_hp_sweep_edge_existing_root`
existed.

Result:

```text
error_code=VALIDATION
field=hp_sweep.output_root
message contains: output root already exists
exit_code=1
```

After: output root still existed and was not overwritten.

## Test Evidence

```text
cargo check -p context-graph-cli --all-features
passed

cargo clippy -p context-graph-cli --all-targets --all-features -- -D warnings
passed

cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv hp_sweep_fails_closed --all-features -- --nocapture
3 passed, 0 failed

cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv --all-features -- --nocapture
10 passed, 0 failed, 188.32 seconds
```

## Remaining

This closes the implementation/CI gate for HP sweep mechanics and collapse
diagnostic export. It does not complete the full 12-cell paper-med sweep; that
still needs a longer dedicated run before making a paper sensitivity claim.
