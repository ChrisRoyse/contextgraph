# 2026-04-30 Multi-Seed Status Report

## Goal

Collect at least three `paper_small` 1K O*NET-derived runs for paper-quality
career metric means/stddev, while preserving failed runs.

## Runs

| Run ID | Seed | Status | Notes |
| --- | ---: | --- | --- |
| `20260430T_paper_small_1k_v1` | implicit checked-in seed | passed | First 1K promotion run. |
| `20260430T_paper_small_1k_seed42_v1` | 42 | passed | Explicit-seed run before ablation seed separation. |
| `20260430T_paper_small_1k_seed43_v1` | 43 | failed | A0 ablation failed `val_latent_mse <= 0.08`; bundle preserved. |
| `20260430T_paper_small_1k_seed42_v2` | 42 | running | Final seed-control semantics. |

## Current Learning

The multi-seed protocol needs to compare the same metric surface across runs.
For this repo, that means:

- Main career seed varies across bundles.
- Same-seed reproducibility remains within each bundle.
- Ablation matrix remains a controlled comparison unless the operator explicitly
  varies ablation seed with `--career-ablation-training-seed`.
- Failed bundles remain part of the record and are inspected through
  `run_manifest.json`, command logs, RocksDB counts, and decoded rows.

## Next Verification Steps

After the final seed-control runs complete:

1. Read each `run_manifest.json` and `fsv_report.json`.
2. Inspect each career RocksDB with `inspect-counts`.
3. Read `paper_tables/table_prediction_metrics.csv` for career rows.
4. Compute career metric mean/stddev across passed explicit-seed bundles.
5. Record failed bundles separately; do not exclude them silently.

