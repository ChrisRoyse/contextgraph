# 2026-04-30 Final Multi-Seed Results

## Scope

Run three explicit-seed `paper_small` bundles with 1,000 O*NET-derived career
events and aggregate the career prediction metrics from persisted paper tables.

## Passed Bundles

| Run ID | Career Seed | Ablation Seed Policy | Status |
| --- | ---: | --- | --- |
| `20260430T_paper_small_1k_seed42_v2` | 42 | checked-in control seed | passed |
| `20260430T_paper_small_1k_seed43_v2` | 43 | checked-in control seed | passed |
| `20260430T_paper_small_1k_seed44_v1` | 44 | checked-in control seed | passed |

Each passed bundle produced:

- 78 command log rows.
- 9 verification domains.
- 9 FSV readbacks.
- 11 edge-case rows.
- 9 artifact hash-check rows.
- 6 career ablation rows.

## Career RocksDB Evidence

Each final explicit-seed career DB was independently inspected with
`dynamicjepa inspect-counts`.

Expected and observed per career DB:

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

## Career Metric Rows

| Run ID | Seed | Val MSE | Val Cosine | Test MSE | Test Cosine | Val Action Contrast | Random Init Cosine | Shuffled Target Cosine |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `20260430T_paper_small_1k_seed42_v2` | 42 | 0.14071947055420186 | 0.9048476588324232 | 0.2017295513225419 | 0.8942226140087763 | 0.875 | -0.05370796178074603 | 0.8459799174965487 |
| `20260430T_paper_small_1k_seed43_v2` | 43 | 0.15950453028461578 | 0.9476699998495745 | 0.2869484249392498 | 0.9334579971102336 | 1.0 | 0.5100941918313674 | 0.7864934958410387 |
| `20260430T_paper_small_1k_seed44_v1` | 44 | 0.16600535812875747 | 0.9004208990365291 | 0.384965547409772 | 0.82687494874531 | 1.0 | -0.15405757912187773 | 0.6160745897167113 |

## Mean And Stddev

| Metric | Mean | Stddev | N |
| --- | ---: | ---: | ---: |
| `val_latent_mse` | 0.155409786323 | 0.013130850075 | 3 |
| `val_latent_cosine` | 0.917646185906 | 0.026095423035 | 3 |
| `test_latent_mse` | 0.291214507891 | 0.091692459686 | 3 |
| `test_latent_cosine` | 0.884851853288 | 0.053905889447 | 3 |
| `val_action_contrast_acc` | 0.958333333333 | 0.072168783649 | 3 |
| `random_init_cosine` | 0.100776216976 | 0.358013148962 | 3 |
| `shuffled_target_cosine` | 0.749516001018 | 0.119329863311 | 3 |

## Claim Boundary

These results support an auditable O*NET public-taxonomy ME-JEPA demo on one RTX
5090-class machine. They do not support a real recruiting prediction claim.

## Verification Commands

Completed successfully after the seed-control patch:

```bash
cargo check -p context-graph-cli --all-features
cargo build -p context-graph-cli --bin context-graph --all-features
cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv --all-features -- --nocapture
```

The integration test result was 5 passed, 0 failed, finished in 302.98 seconds.
