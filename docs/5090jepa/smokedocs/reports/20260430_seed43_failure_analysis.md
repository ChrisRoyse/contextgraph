# 2026-04-30 Seed 43 Failure Analysis

## Scope

Run a second 1K `paper_small` bundle with explicit career seed 43 and inspect
any failure without overwriting or hiding the failed bundle.

## Failed Bundle

- Bundle:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed43_v1`
- Status:
  `failed`
- Purpose:
  `paper_small`
- Career events:
  1000
- Career training seed:
  43

## Failure

The wrapper failed closed during the A0 ablation:

- Command:
  `career_taxonomy_ablation_A0_taxonomy_only_verify-career-taxonomy`
- Error code:
  `TRAINING_FAILED`
- Failing metric:
  `val_latent_mse`
- Final value:
  `0.510142`
- Target:
  `0.080000`
- Training run:
  `935a486f-8c9c-4a2d-9fc3-3037097f31a7`

## Source-Of-Truth Readback

The failed A0 RocksDB was independently inspected:

| CF | Count |
| --- | ---: |
| `dj_raw_events` | 1004 |
| `dj_adapter_runs` | 1004 |
| `dj_normalized_states` | 1000 |
| `dj_instrument_readings` | 8000 |
| `dj_latent_panels` | 1000 |
| `dj_transitions` | 1000 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 |
| `dj_model_artifacts` | 0 |
| `dj_predictions` | 0 |
| `dj_guard_decisions` | 0 |
| `dj_plan_traces` | 0 |
| `dj_verification_runs` | 1 |

This proves the failure occurred after ingest, paneling, trajectory compilation,
dataset creation, and training-run persistence, but before model artifact
registration and planning.

## Main Career Slice

The same seed 43 bundle completed the main `career_taxonomy` run before failing
the A0 ablation. Independent career DB readback showed:

- `dj_normalized_states`: 1000
- `dj_model_artifacts`: 1
- `dj_predictions`: 6
- `dj_guard_decisions`: 5
- `dj_plan_traces`: 1

Decoded main career metrics:

- Train MSE: `0.05670160324620004`
- Val MSE: `0.15950453028461578`
- Test MSE: `0.2869484249392498`
- Val cosine: `0.9476699998495743`
- Test cosine: `0.9334579971102336`
- Val action-contrast accuracy: `1.0`

## Root Cause

The data path was not broken. The A0 taxonomy-only ablation gate was too
seed-sensitive when the same seed override was applied to both the main career
run and all ablation controls.

The correction is to separate main career seed variation from the controlled
ablation matrix:

- `--career-training-seed` varies main career and same-seed reproducibility.
- Ablations keep the checked-in control seed by default.
- `--career-ablation-training-seed` is available only when intentionally varying
  ablations.

## Status

The failed bundle is preserved as evidence. It should not be deleted or
overwritten.

