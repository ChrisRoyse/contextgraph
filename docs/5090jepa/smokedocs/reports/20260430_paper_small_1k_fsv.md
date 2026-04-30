# 2026-04-30 Paper-Small 1K FSV Report

## Scope

Promote the validated O*NET smoke path to a `paper_small` run with 1,000
O*NET-derived career events, then perform full-state verification from persisted
sources of truth instead of relying on command return status alone.

## Source Of Truth

- Bundle:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1`
- RocksDB root:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/dbs`
- Career DB:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/dbs/20260430T_paper_small_1k_v1_career_taxonomy_rocksdb`
- Artifact root:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/artifacts`
- Evidence root:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/fsv_evidence`

## Trigger

```bash
CUDA_COMPUTE_CAP=120 target/debug/context-graph dynamicjepa research-smoke \
  --run-id 20260430T_paper_small_1k_v1 \
  --run-root tmp/mejepa_research_runs \
  --fixtures-root configs/dynamicjepa \
  --purpose paper_small \
  --career-event-count 1000 \
  --onet-text-zip tmp/onet_probe/db_30_2_text.zip \
  --system-specs docs2/systemspecs.md \
  --json
```

## Result

- CLI status: `ok`
- `run_manifest.json` status: `passed`
- `fsv_report.json` status: `passed`
- Command count: 78
- Verification domains: 9
- Training metric rows: 3
- Artifact hash rows: 9
- Edge-case rows: 11
- Ablation rows: 6

## Career DB Readback

Independent `inspect-counts` readback from the career RocksDB returned:

| CF | Count |
| --- | ---: |
| `dj_raw_events` | 1004 |
| `dj_adapter_runs` | 1004 |
| `dj_normalized_states` | 1000 |
| `dj_instrument_readings` | 10000 |
| `dj_latent_panels` | 1000 |
| `dj_transitions` | 1000 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 |
| `dj_model_artifacts` | 1 |
| `dj_predictions` | 6 |
| `dj_guard_decisions` | 5 |
| `dj_plan_traces` | 1 |
| `dj_verification_runs` | 1 |
| `dj_audit_log` | 3019 |

Decoded row checks confirmed:

- Dataset split: train 699, val 200, test 100.
- Model artifact status: `Active`.
- Model artifact files: `config.json`, `evaluation_report.json`,
  `manifest.json`, `metrics.json`, `model.safetensors`.
- Plan trace: 5 candidate actions, 5 guard decisions, 5 predictions, selected
  action, utility scores, and linked model artifact ID.
- Normalized career states include `occupation_cluster`, `job_zone`,
  `skill_vector`, `experience_years`, and `wage_band`.

## Data Evidence

- O*NET source ZIP:
  `tmp/onet_probe/db_30_2_text.zip`
- O*NET source SHA-256:
  `b5479271931796b838f7173dc0f673a9ec961b7833ac87168fd11e92e7453741`
- Generated fixture rows: 1000
- Generated walk CSV lines: 1001, including header
- Selected O*NET occupations: 218
- Selected related edges: 330
- Source occupations: 1016

The first generated walk rows are grounded in Software Developers
`15-1252.00` and Database Architects `15-1243.00`, with edge hashes tied to the
source ZIP hash.

## Tables And Plots

Read and verified:

- `paper_tables/table_prediction_metrics.csv`
- `paper_tables/table_ablation.csv`
- `paper_tables/table_edge_cases.csv`
- `plots/prediction_val_mse.svg`
- `plots/career_ablation_val_mse.svg`
- `plots/plot_manifest.json`

Artifact hash verification found zero mismatches across the nine artifact rows.

## Manual FSV Notes

Manual DB inspection must be sequential per RocksDB path. A parallel
`inspect-cf` attempt against the same career DB correctly failed with a typed
`STORAGE` error on the RocksDB `LOCK` file and remediation text. This is expected
database behavior and a useful operator diagnostic, not a bundle failure.

