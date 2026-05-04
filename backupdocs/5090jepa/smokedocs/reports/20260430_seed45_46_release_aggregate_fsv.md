# Seed45/46 Repair And Release Aggregate FSV

Date: 2026-04-30 local run context. Some command logs are UTC and show
2026-05-01.

## Scope

Continue doc 23 implementation after the HP-sweep gate by resolving the two
remaining paper-small seed failures and promoting the release aggregate from
three passed seeds to five passed seeds. ClipCannon was not touched.

## Source Of Truth

Primary persisted state:

- seed45 rerun:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed45_v3`;
- seed46 rerun:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed46_v2`;
- release aggregate:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_multiseed_42_46_v4`;
- child RocksDB databases under each seed bundle's `dbs/`;
- aggregate files: `aggregate_manifest.json`, `aggregate_fsv_report.json`,
  `paper_tables/*.csv`, and `plots/*.svg`.

Preserved failed evidence:

- old seed45 failure:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed45_v2`;
- old seed46 failure:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed46_v1`;
- failed all-pass aggregate attempt:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_multiseed_42_46_v3`.

## Root Causes

1. Seed45/seed46 old failures were isolated to
   `career_taxonomy_ablation_A3_no_action_verify-career-taxonomy`.
   The DBs showed ingest, paneling, dataset creation, training, and model
   persistence succeeded, but the A3 verifier failed the career metric gate.
2. The old failed bundles had `career_ablation_training_seed` equal to the main
   career seed. That made the A3 no-action negative-control ablation
   seed-sensitive and broke two release seeds. Current code now keeps ablations
   on the source fixture control seed unless `--career-ablation-training-seed`
   is explicitly supplied.
3. The first five-pass aggregate attempt failed because
   `table_seed_failures.csv` correctly had only a header when every child bundle
   passed, but the generic CSV readback rejected any empty table. The fix allows
   empty rows only for `table_seed_failures.csv`; metric tables still require
   data rows.
4. CI Tier B/Tier C still deleted run roots before execution. Those deletes
   were replaced with explicit existing-root failures so CI cannot overwrite an
   immutable evidence bundle.

## Implementation Changes

- `run_manifest.json` now records:
  - `career_ablation_training_seed`: requested CLI override;
  - `career_ablation_training_seed_effective`: seed actually written into
    generated ablation train configs;
  - `career_ablation_training_seed_origin`: `explicit_cli`,
    `source_fixture_control`, or
    `source_fixture_control_separated_from_main_seed`.
- A3 and other ablations now use the recorded effective ablation seed.
- Career metric-gate failures now log both `val_latent_cosine` and the
  `min_val_latent_cosine` threshold.
- `multiseed-aggregate` reads an empty `table_seed_failures.csv` only through
  an explicit allow-empty path.
- `.github/workflows/ci_smoke_b.yml` and
  `.github/workflows/ci_smoke_c.yml` now fail if generated bundle roots already
  exist instead of deleting them.
- Tier C now runs `multiseed-aggregate` over the five release bundles with
  `--min-passed 5`.

## Seed45 Happy Path FSV

Command:

```bash
CUDA_COMPUTE_CAP=120 target/debug/context-graph dynamicjepa research-smoke \
  --run-id 20260430T_paper_small_1k_seed45_v3 \
  --run-root tmp/mejepa_research_runs \
  --fixtures-root configs/dynamicjepa \
  --purpose paper_small \
  --career-event-count 1000 \
  --career-training-seed 45 \
  --onet-text-zip tmp/onet_probe/db_30_2_text.zip \
  --system-specs docs2/systemspecs.md \
  --json
```

Readback:

- `run_manifest.json`: `status=passed`, `career_training_seed=45`,
  `career_ablation_training_seed_effective=42`,
  `career_ablation_training_seed_origin=source_fixture_control_separated_from_main_seed`.
- `fsv_report.json`: `status=passed`, 9 verification runs, 9 artifact hash
  checks, 11 edge cases.
- A3 generated train config: `seed=42`, `ignore_action=true`,
  `stopping.target=0.5`.
- A3 table row:
  `val_latent_mse=0.2688891870223466`,
  `val_latent_cosine=0.7900839456285774`.

Primary career RocksDB counts:

| CF | Count |
| --- | ---: |
| `dj_raw_events` | 1004 |
| `dj_latent_panels` | 1000 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 |
| `dj_model_artifacts` | 1 |
| `dj_predictions` | 6 |
| `dj_guard_decisions` | 5 |
| `dj_verification_runs` | 1 |

A3 RocksDB counts:

| CF | Count |
| --- | ---: |
| `dj_raw_events` | 1004 |
| `dj_latent_panels` | 1000 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 |
| `dj_model_artifacts` | 1 |
| `dj_predictions` | 6 |
| `dj_guard_decisions` | 5 |
| `dj_verification_runs` | 1 |

Hashes:

| File | SHA-256 |
| --- | --- |
| `run_manifest.json` | `55e3a137d82ba83837bec73cdbb34908253db05af4ccbaa96073a226262c30f1` |
| `fsv_report.json` | `d374b3f4822c0e65e9c6e029be7edd24fcb39354d0d174dc14750d827fb72950` |
| `table_prediction_metrics.csv` | `ae6bdc2ce7e9d90133162cb0acf6fa17e29466c3a7b9029777f5a78b7255501c` |
| `table_ablation.csv` | `dbd75e874d8df57ea48aa07d278c6ad8b8e6ef4e154057338cbdb7d3b7ac1594` |
| `table_collapse_diagnostics.csv` | `295fc3c413316aa256dcaa02dbb337ae3245a2446d8310094acc8030a36a8b5d` |

## Seed46 Happy Path FSV

Command:

```bash
CUDA_COMPUTE_CAP=120 target/debug/context-graph dynamicjepa research-smoke \
  --run-id 20260430T_paper_small_1k_seed46_v2 \
  --run-root tmp/mejepa_research_runs \
  --fixtures-root configs/dynamicjepa \
  --purpose paper_small \
  --career-event-count 1000 \
  --career-training-seed 46 \
  --onet-text-zip tmp/onet_probe/db_30_2_text.zip \
  --system-specs docs2/systemspecs.md \
  --json
```

Readback:

- `run_manifest.json`: `status=passed`, `career_training_seed=46`,
  `career_ablation_training_seed_effective=42`,
  `career_ablation_training_seed_origin=source_fixture_control_separated_from_main_seed`.
- `fsv_report.json`: `status=passed`, 9 verification runs, 9 artifact hash
  checks, 11 edge cases.
- A3 generated train config: `seed=42`, `ignore_action=true`,
  `stopping.target=0.5`.
- A3 table row:
  `val_latent_mse=0.2688891870223466`,
  `val_latent_cosine=0.7900839456285774`.

Primary career and A3 RocksDB counts both include:

| CF | Count |
| --- | ---: |
| `dj_raw_events` | 1004 |
| `dj_latent_panels` | 1000 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 |
| `dj_model_artifacts` | 1 |
| `dj_predictions` | 6 |
| `dj_guard_decisions` | 5 |
| `dj_verification_runs` | 1 |

Hashes:

| File | SHA-256 |
| --- | --- |
| `run_manifest.json` | `ee3d12f64badd35ff5773d4d5b8b1d1eefadce5e5c72b7e33dc03ec5c5dfd65d` |
| `fsv_report.json` | `2d76fcd24e400c592321dd2dbee4ae51fb7cd206feceb92ad2ded2b57f7d45eb` |
| `table_prediction_metrics.csv` | `3662284657750b0fb12453a8c2cb65f8287d8f9dc32ffb70275e9e4aa09f5a76` |
| `table_ablation.csv` | `4a8d055e00a2976326dec6791069a17906bf730c15d8f73a18213ff96c2cd9dc` |
| `table_collapse_diagnostics.csv` | `8d9ca1b5e211f07b66de23ae48a2b683ef78a092475932c36dc1fe11cb4e10f9` |

## Five-Pass Aggregate FSV

Command:

```bash
CUDA_COMPUTE_CAP=120 target/debug/context-graph dynamicjepa multiseed-aggregate \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed42_v2 \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed43_v2 \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed44_v1 \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed45_v3 \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed46_v2 \
  --output-root tmp/mejepa_research_runs/20260430T_paper_small_1k_multiseed_42_46_v4 \
  --expected-bundle-count 5 \
  --min-passed 5 \
  --expected-purpose paper_small \
  --expected-career-event-count 1000 \
  --required-career-training-seeds 42,43,44,45,46 \
  --bootstrap-iters 10000 \
  --bootstrap-seed 20260430 \
  --confidence 0.95 \
  --json
```

Aggregate readback:

- `aggregate_manifest.json`: `status=passed`, `passed_count=5`,
  `failed_count=0`;
- `aggregate_fsv_report.json`: `status=passed`, `child_bundle_count=5`,
  `passed_child_bundle_count=5`, `failed_child_bundle_count=0`;
- row counts: prediction `33`, ablation `78`, paired ablation tests `35`,
  seed failures `0`;
- plots exist and begin with `<svg`.

Aggregate output hashes:

| File | SHA-256 |
| --- | --- |
| `aggregate_manifest.json` | `734a698c7ed946c292cf9005c4083f7205a2a37b271037870418103eb68670e0` |
| `aggregate_fsv_report.json` | `9b1bc096c850fbd87d44447f53d33da58c35cd4337292ffbde82f63cc6b6a9b6` |
| `table_prediction_metrics_multiseed.csv` | `6bd81ee3b53b90c495263378c492a7fed2b123cbcfd5007ee0a14a99702608e7` |
| `table_ablation_multiseed.csv` | `1934260ce6d0a39b6d04d0c857e097ff3d19c3e407fca6b00760548e5936c6a9` |
| `table_paired_ablation_tests.csv` | `26218e9705c2170b25a7251f51234a4fb18b02eed9ab92339189f22a6dd88d80` |
| `table_seed_failures.csv` | `8c4e8bfb875f646993fb453e8e35203faca6fd99c41373ed8ef1ed455ec52d53` |
| `multiseed_val_mse_box.svg` | `69db6c4d2ec8b837100f23fbfe74c633c2c1aa29d0a981806dcf57c4123ed77c` |
| `multiseed_paired_delta_forest.svg` | `79f8050a1cf0c8c6b4f544f71461ecb2e8cc0672c987b14f1b84ee1b93bb04e4` |

Career aggregate rows:

| Metric | Mean | Stddev | BCa 95% low | BCa 95% high | N |
| --- | ---: | ---: | ---: | ---: | ---: |
| `val_latent_mse` | 0.152213508681 | 0.010272102979 | 0.144629287164 | 0.160877982989 | 5 |
| `val_latent_cosine` | 0.904866698041 | 0.043638221350 | 0.861495866042 | 0.931127632841 | 5 |
| `test_latent_mse` | 0.268471774864 | 0.072050852408 | 0.224106912986 | 0.345758698422 | 5 |
| `test_latent_cosine` | 0.867637059212 | 0.067656811317 | 0.806301640199 | 0.912141387437 | 5 |
| `val_action_contrast_acc` | 0.956000000000 | 0.051889305256 | 0.906000000000 | 0.988000000000 | 5 |

## Boundary And Edge Audit

All edge tests used real existing paper-small child bundles.

| Edge | Before | Trigger | Expected | After |
| --- | --- | --- | --- | --- |
| existing output root | root existed, zero files | aggregate with existing `--output-root` | typed `VALIDATION`, no overwrite | root still existed, zero files |
| expected count mismatch | output root absent | five bundles with `--expected-bundle-count 6` | typed `VALIDATION`, no output | output root absent |
| duplicate passed seed | output root absent | seed42 v1 and seed42 v2 in same set | typed `VALIDATION`, no output | output root absent |

Observed fields:

- existing root: `multiseed_aggregate.output_root`;
- count mismatch: `multiseed_aggregate.expected_bundle_count`;
- duplicate seed: `multiseed_aggregate.career_training_seed`.

## Verification Commands

Passed:

```bash
cargo fmt --all -- --check
cargo check -p context-graph-cli --all-features
cargo build -p context-graph-cli --bin context-graph --all-features
cargo clippy -p context-graph-cli --all-targets --all-features -- -D warnings
cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv --all-features -- --nocapture
ruby -e 'require "yaml"; ARGV.each { |path| YAML.load_file(path); puts "ok #{path}" }' \
  .github/workflows/ci_smoke_a.yml .github/workflows/ci_smoke_b.yml .github/workflows/ci_smoke_c.yml
```

Test result:

- `dynamicjepa_research_smoke_fsv`: 10 passed, 0 failed, 190.92 seconds on
  the final rerun.

## Web Research Notes

- GitHub Actions matrix failure behavior: `strategy.fail-fast` defaults to
  cancelling queued/in-progress matrix jobs when one fails. Tier C uses an
  explicit loop and preserves attempted bundle paths before returning failure:
  <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax>.
- GitHub artifact docs emphasize uploading/downloading workflow artifacts and
  digest validation. This supports preserving evidence bundles and logs rather
  than deleting them during setup:
  <https://docs.github.com/en/actions/tutorials/store-and-share-data>.
- NIST warns that one-factor-at-a-time experiments do not account for
  interactions. The HP sweep remains a small CI drift gate, not a substitute
  for paper-scale experimental design:
  <https://www.itl.nist.gov/div898/handbook/pri/section2/pri212.htm>.

## Status

The two failed paper-small release seeds are repaired and rerun as immutable
passed bundles. The release aggregate now requires and achieves five passed
O*NET-backed seeds. The remaining claim remains scoped to an auditable O*NET
public-taxonomy ME-JEPA demo on one RTX 5090.
