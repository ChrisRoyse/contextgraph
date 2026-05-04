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
| `20260430T_paper_small_1k_seed42_v2` | 42 | passed | Final seed-control semantics. |
| `20260430T_paper_small_1k_seed43_v2` | 43 | passed | Main seed varied; ablation control seed fixed. |
| `20260430T_paper_small_1k_seed44_v1` | 44 | passed | Third explicit-seed paper-small run. |

## Current Learning

The multi-seed protocol needs to compare the same metric surface across runs.
For this repo, that means:

- Main career seed varies across bundles.
- Same-seed reproducibility remains within each bundle.
- Ablation matrix remains a controlled comparison unless the operator explicitly
  varies ablation seed with `--career-ablation-training-seed`.
- Failed bundles remain part of the record and are inspected through
  `run_manifest.json`, command logs, RocksDB counts, and decoded rows.

## Final Explicit-Seed Metrics

These rows were read from each bundle's
`paper_tables/table_prediction_metrics.csv` career row.

| Run ID | Seed | Val MSE | Val Cosine | Test MSE | Test Cosine | Val Action Contrast |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `20260430T_paper_small_1k_seed42_v2` | 42 | 0.14071947055420186 | 0.9048476588324232 | 0.2017295513225419 | 0.8942226140087763 | 0.875 |
| `20260430T_paper_small_1k_seed43_v2` | 43 | 0.15950453028461578 | 0.9476699998495745 | 0.2869484249392498 | 0.9334579971102336 | 1.0 |
| `20260430T_paper_small_1k_seed44_v1` | 44 | 0.16600535812875747 | 0.9004208990365291 | 0.384965547409772 | 0.82687494874531 | 1.0 |

Mean/stddev across the three passed explicit-seed bundles:

| Metric | Mean | Stddev | N |
| --- | ---: | ---: | ---: |
| `val_latent_mse` | 0.155409786323 | 0.013130850075 | 3 |
| `val_latent_cosine` | 0.917646185906 | 0.026095423035 | 3 |
| `test_latent_mse` | 0.291214507891 | 0.091692459686 | 3 |
| `test_latent_cosine` | 0.884851853288 | 0.053905889447 | 3 |
| `val_action_contrast_acc` | 0.958333333333 | 0.072168783649 | 3 |
| `random_init_cosine` | 0.100776216976 | 0.358013148962 | 3 |
| `shuffled_target_cosine` | 0.749516001018 | 0.119329863311 | 3 |

## Source-Of-Truth Checks

For all three final explicit-seed bundles:

- `run_manifest.json` status: `passed`.
- `fsv_report.json` status: `passed`.
- FSV readbacks: 9.
- Edge cases: 11.
- Artifact hash checks: 9.
- Career DB counts include 1,000 normalized states, 1 model artifact, 6
  predictions, 5 guard decisions, 1 plan trace, and 1 verification row.
- Generated main career train config seeds were 42, 43, and 44 respectively.
- Generated A0 ablation config seed was 42 in all three final bundles because
  ablations were held as controlled comparisons.

## Remaining Verification Steps

The multi-seed gate is complete. The failed `seed43_v1` bundle remains part of
the record and should be referenced as a seed-control root-cause finding, not as
a passing paper-small run.

