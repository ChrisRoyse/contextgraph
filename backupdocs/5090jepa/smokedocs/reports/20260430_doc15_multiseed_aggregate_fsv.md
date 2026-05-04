# Doc 15 Multiseed Aggregate FSV

Date: 2026-04-30.

## Scope

Implemented and manually verified `dynamicjepa multiseed-aggregate` for the
paper-small 1K O*NET-derived seed panel.

Source of truth:

- aggregate bundle:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_multiseed_42_46_v2`;
- child run manifests and FSV reports for seeds 42-46;
- child RocksDB databases for passed career runs and failed A3 ablation runs.

## Implementation

Added `dynamicjepa multiseed-aggregate` with fail-closed validation:

- accepts either `--bundle-glob` or repeated `--bundle`, not both;
- rejects existing output roots;
- enforces expected child bundle count, minimum passed count, expected purpose,
  expected career event count, required seed set, unique run IDs, and one passed
  bundle per seed;
- reads only passed child CSVs into means/stddev/BCa intervals;
- lists failed child bundles in `table_seed_failures.csv`;
- writes `aggregate_manifest.json`, `aggregate_fsv_report.json`, three
  aggregate tables, one failure table, and two SVG plots;
- performs readback of JSON and CSV source-of-truth files before returning
  success.

Added a RocksDB lock safety fix after FSV exposed a real defect:

- previous open path used `flock` to classify a RocksDB `LOCK` as stale and
  removed it;
- RocksDB uses its own advisory lock path, so that removal could permit two
  write opens on the same DB;
- new behavior inspects `LOCK` for diagnostics only and never deletes it;
- regression test `test_live_lock_second_open_fails_without_removing_lock`
  proves a second live open fails closed and the DB reopens after the first
  handle drops.

## Seed Panel

| Seed | Bundle | Status | Notes |
| ---: | --- | --- | --- |
| 42 | `20260430T_paper_small_1k_seed42_v2` | passed | included in means |
| 43 | `20260430T_paper_small_1k_seed43_v2` | passed | included in means |
| 44 | `20260430T_paper_small_1k_seed44_v1` | passed | included in means |
| 45 | `20260430T_paper_small_1k_seed45_v2` | failed | A3 no-action metric gate failure |
| 46 | `20260430T_paper_small_1k_seed46_v1` | failed | A3 no-action metric gate failure |

Seed45 v1 is preserved but excluded from the final aggregate because manual
parallel inspection under the old lock remover left its A3 failed-run RocksDB
with a MANIFEST/SST inconsistency. Seed45 v2 was rerun with the lock fix and is
physically inspectable.

## Happy Path Command

```bash
CUDA_COMPUTE_CAP=120 target/debug/context-graph dynamicjepa multiseed-aggregate \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed42_v2 \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed43_v2 \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed44_v1 \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed45_v2 \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_seed46_v1 \
  --output-root tmp/mejepa_research_runs/20260430T_paper_small_1k_multiseed_42_46_v2 \
  --expected-bundle-count 5 \
  --min-passed 3 \
  --expected-purpose paper_small \
  --expected-career-event-count 1000 \
  --required-career-training-seeds 42,43,44,45,46 \
  --bootstrap-iters 10000 \
  --bootstrap-seed 20260430 \
  --confidence 0.95 \
  --json
```

Returned:

- `matched_bundle_count=5`;
- `passed_bundle_count=3`;
- `failed_bundle_count=2`;
- `table_prediction_metrics_multiseed=33` rows;
- `table_ablation_multiseed=78` rows;
- `table_paired_ablation_tests=35` rows;
- `table_seed_failures=2` rows.

## Aggregate Readback

`aggregate_manifest.json`:

- `status=passed`;
- `passed_count=3`;
- `failed_count=2`;
- passed run IDs are seed42 v2, seed43 v2, seed44 v1;
- source bundles include manifest and FSV SHA-256 values for all five seeds.

`aggregate_fsv_report.json`:

- `status=passed`;
- `child_bundle_count=5`;
- `passed_child_bundle_count=3`;
- `failed_child_bundle_count=2`;
- readback checks match the table row counts above.

Output hashes:

| File | SHA-256 |
| --- | --- |
| `aggregate_manifest.json` | `b1a0fcf613ca640dd85f13941db745db0344321745b5fb6f1e74e4b545db4ebc` |
| `aggregate_fsv_report.json` | `dbd39a70feae485a09bda16f093d8cf90d5aacaf3a6032a016bfa6eeacbce4dd` |
| `table_prediction_metrics_multiseed.csv` | `bf6f55abd5c3f972cd8f68387347fe8af61efa8f266aa5a08eda8948e9737976` |
| `table_ablation_multiseed.csv` | `2b5f71e0ff84c8b31bac4d8f3ae0f0f336f85e8a4c18689385a75b945e5d13d2` |
| `table_paired_ablation_tests.csv` | `69a83ea56ba20804fbccd6f03d585783623e36cf24abcfce3a33b0ac0bdf8389` |
| `table_seed_failures.csv` | `75ba77b40e6f9d020cb505e151969123df654b738033a686db57fc11373f08e3` |
| `multiseed_val_mse_box.svg` | `8ab0878f30d22dcdedd8f8ab2538dd5fcd2d5b3a4e5e7f193ca13dc46a137d8b` |
| `multiseed_paired_delta_forest.svg` | `d41d7079fd075372a0ef1dd6945c89f4b8e67205ff06690f5cf4f0026d04309f` |

Career aggregate rows from
`paper_tables/table_prediction_metrics_multiseed.csv`:

| Metric | Mean | Stddev | BCa low | BCa high | N |
| --- | ---: | ---: | ---: | ---: | ---: |
| `val_latent_mse` | 0.155409786323 | 0.013130850075 | 0.140719470554 | 0.163838415514 | 3 |
| `val_latent_cosine` | 0.917646185906 | 0.026095423035 | 0.900420899037 | 0.933395886177 | 3 |
| `test_latent_mse` | 0.291214507891 | 0.091692459686 | 0.201729551323 | 0.352293173253 | 3 |
| `test_latent_cosine` | 0.884851853288 | 0.053905889447 | 0.826874948745 | 0.920379536076 | 3 |
| `val_action_contrast_acc` | 0.958333333333 | 0.072168783649 | 0.875000000000 | 1.000000000000 | 3 |

Plots exist and begin with `<svg`:

- `plots/multiseed_val_mse_box.svg`, 2537 bytes;
- `plots/multiseed_paired_delta_forest.svg`, 2696 bytes.

## Child DB FSV

Serial `inspect-counts` readbacks:

| Seed | DB | Raw | Transitions | Training | Artifacts | Verifications | Predictions | Guards |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 42 | main career | 1004 | 1000 | 1 | 1 | 1 | 6 | 5 |
| 43 | main career | 1004 | 1000 | 1 | 1 | 1 | 6 | 5 |
| 44 | main career | 1004 | 1000 | 1 | 1 | 1 | 6 | 5 |
| 45 | failed A3 | 1004 | 1000 | 1 | 1 | 1 | 0 | 0 |
| 46 | failed A3 | 1004 | 1000 | 1 | 1 | 1 | 0 | 0 |

Serial `inspect-cf` on `seed45_v2` failed A3 `dj_training_runs` read one
decoded training row:

- `training_run_id=887e809f-2593-44b0-979d-09a7a3e55b8f`;
- `status=Completed`;
- `parameter_count_trainable=15408`;
- `val_latent_mse=0.38418309787311644`;
- `val_latent_cosine=0.6275242888485578`;
- `test_latent_mse=0.4782284317592376`;
- `test_latent_cosine=0.6016092655590208`.

Serial `inspect-cf` on `seed46_v1` failed A3 `dj_training_runs` read
`val_latent_cosine=0.43491496685455616`.

## Edge Cases

Each edge printed before and after output-root state. All output roots remained
absent after failure.

| Edge | Expected | Observed |
| --- | --- | --- |
| zero matched bundles | fail validation, create no output | `VALIDATION`, `matched zero child bundles`, output absent before/after |
| duplicate passed seed | fail validation, create no output | `VALIDATION`, `multiple passed bundles use career_training_seed 42`, output absent before/after |
| missing required seed | fail validation, create no output | `VALIDATION`, `missing required career training seeds: [46]`, output absent before/after |

## Verification Commands

Passed:

```bash
cargo test -p context-graph-storage teleological::tests::stale_lock -- --nocapture
cargo check -p context-graph-storage -p context-graph-cli --all-features
cargo clippy -p context-graph-storage -p context-graph-cli --all-targets --all-features -- -D warnings
cargo build -p context-graph-cli --bin context-graph --all-features
```

## Web Research Notes

- RocksDB’s own guidance and issue history treat missing SST files referenced by
  MANIFEST as corruption requiring explicit repair/recovery, not silent masking:
  <https://github.com/facebook/rocksdb/wiki/RocksDB-Repairer> and
  <https://github.com/facebook/rocksdb/issues/9419>.
- RocksDB lock behavior is exclusive for process opens; concurrent sharing of
  the same DB for reads is not generally available in the normal open path:
  <https://rocksdb.org/blog/2014/05/14/lock.html>.
- BCa bootstrap intervals are an established bootstrap CI method with bias and
  acceleration correction:
  <https://pmc.ncbi.nlm.nih.gov/articles/PMC7958418/>.

## Status

Doc 15 implementation and FSV are complete for the CLI aggregate/reporting
piece. The scientific finding from this seed panel is that seeds 45 and 46 fail
the A3 no-action ablation gate while seeds 42-44 pass; the safe paper claim
remains an auditable O*NET public-taxonomy ME-JEPA demo on one RTX 5090, not
real recruiting prediction.
