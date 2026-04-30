# 13 - Current Implementation And Benchmark Status

Status date: 2026-04-30.

This is the current source-of-truth status for the 5090 ME-JEPA research demo.
The implementation has completed the paper-small 1K O*NET-derived gate and a
three-run explicit-seed set.

## Implemented Surface

DynamicJEPA now has a complete research-smoke path:

- `dynamicjepa verify-counter-world`;
- `dynamicjepa verify-gridworld`;
- `dynamicjepa verify-career-taxonomy`;
- `dynamicjepa research-smoke`;
- O*NET 30.2-backed career fixture generation from the real text ZIP;
- immutable research bundles under `tmp/mejepa_research_runs/<run_id>/`;
- per-domain RocksDB source-of-truth DBs;
- generated paper tables and SVG smoke plots;
- career same-seed reproducibility;
- career ablations A0-A5;
- wrapper fail-closed validation for missing O*NET source, invalid run IDs,
  duplicate run IDs, custom event counts without O*NET, and bad O*NET paths.

The O*NET generator reads these real source files from the ZIP:

- `Occupation Data.txt`;
- `Job Zones.txt`;
- `Skills.txt`;
- `Related Occupations.txt`.

It starts from `15-1252.00` Software Developers, restricts the smoke walk to
core computer occupations, writes generated fixtures into the run bundle, and
records the source ZIP hash plus per-edge walk evidence.

## Environment Evidence

Observed on 2026-04-30:

| Item | Observed |
| --- | --- |
| GPU | NVIDIA GeForce RTX 5090 |
| Driver | 596.21 |
| Compute capability | 12.0 |
| VRAM | 32607 MiB |
| CUDA | 13.2, V13.2.78 |
| Rust | rustc 1.95.0 |
| Cargo | cargo 1.95.0 |
| Kernel | Linux 6.6.87.2-microsoft-standard-WSL2 |
| RAM | 110GiB visible to WSL2 |

`docs2/systemspecs.md` is hashed into every research-smoke bundle.

## Latest Full O*NET Smoke FSV

Command:

```bash
RUN_ID=20260430T_onet30_demo_fsv_v12
CUDA_COMPUTE_CAP=120 target/debug/context-graph dynamicjepa research-smoke \
  --run-id "$RUN_ID" \
  --run-root tmp/mejepa_research_runs \
  --fixtures-root configs/dynamicjepa \
  --purpose smoke \
  --onet-text-zip tmp/onet_probe/db_30_2_text.zip \
  --system-specs docs2/systemspecs.md \
  --json
```

Bundle source of truth:

- bundle: `tmp/mejepa_research_runs/20260430T_onet30_demo_fsv_v12`;
- RocksDB root: `tmp/mejepa_research_runs/20260430T_onet30_demo_fsv_v12/dbs`;
- artifact root: `tmp/mejepa_research_runs/20260430T_onet30_demo_fsv_v12/artifacts`;
- FSV evidence root: `tmp/mejepa_research_runs/20260430T_onet30_demo_fsv_v12/fsv_evidence`;
- generated O*NET fixtures:
  `tmp/mejepa_research_runs/20260430T_onet30_demo_fsv_v12/generated_fixtures/onet_30_2`.

Run result:

| Item | Value |
| --- | ---: |
| status | passed |
| command count | 78 |
| total elapsed ms | 169132 |
| artifact hash rows | 9 |
| edge case rows | 11 |
| ablation rows | 6 |
| career events | 120 |
| selected O*NET occupations | 218 |
| selected O*NET related edges | 330 |
| O*NET source occupations | 1016 |

O*NET source evidence:

| Field | Value |
| --- | --- |
| source ZIP | `tmp/onet_probe/db_30_2_text.zip` |
| source ZIP SHA-256 | `b5479271931796b838f7173dc0f673a9ec961b7833ac87168fd11e92e7453741` |
| generated JSONL SHA-256 | `a789749be8987132cb98c264ca497bfe9f6287e67bf780023a360dbe6ca29a8d` |
| generated walk CSV SHA-256 | `c837dc2b427ceab7c6eb61e9a5eb15f55c0c877c63263dec8e088a117d0459ba` |
| source manifest SHA-256 | `7f1323cedcfdf8110e4acf251b1b9c581d3684068ecd67b2467d0aa3f43a5a58` |
| allowed use | `research-demo` |

## Source-Of-Truth DB Readback

The primary career DB was independently inspected after the smoke run:

```bash
target/debug/context-graph dynamicjepa inspect-counts \
  --db tmp/mejepa_research_runs/20260430T_onet30_demo_fsv_v12/dbs/20260430T_onet30_demo_fsv_v12_career_taxonomy_rocksdb \
  --json
```

Career counts:

| CF | Count |
| --- | ---: |
| `dj_raw_events` | 124 |
| `dj_adapter_runs` | 124 |
| `dj_normalized_states` | 120 |
| `dj_instrument_readings` | 1200 |
| `dj_latent_panels` | 120 |
| `dj_trajectories` | 1 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 |
| `dj_model_artifacts` | 1 |
| `dj_predictions` | 6 |
| `dj_plan_traces` | 1 |
| `dj_guard_decisions` | 5 |
| `dj_verification_runs` | 1 |

Decoded readbacks confirmed:

- normalized career state fields include `occupation_cluster`, `job_zone`,
  `skill_vector`, `experience_years`, and `wage_band`;
- dataset shards are `train=83`, `val=24`, `test=12` with zero leakage counts;
- model artifact row references five hashed files including `model.safetensors`;
- plan trace persisted five candidate actions, five guard decisions, five
  predictions, selected action, utility scores, and model artifact hash.

## Prediction Metrics

From `paper_tables/table_prediction_metrics.csv`:

| Domain | Train | Val | Test | Params | Val MSE | Val Cosine | Test MSE | Test Cosine |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| counter_world | 41 | 12 | 6 | 480 | 0.004330990884665671 | 0.9996336712676964 | 0.013100919986046714 | 0.9991602338714566 |
| gridworld_5x5 | 4977 | 1330 | 693 | 16960 | 0.03936942384623141 | 0.9950110445426108 | 0.04023547898409154 | 0.9949794096655592 |
| career_taxonomy | 83 | 24 | 12 | 15344 | 0.17999517062789205 | 0.8776115717741934 | 0.25307116609684355 | 0.867371052118299 |

## Career Ablations

From `paper_tables/table_ablation.csv`:

| ID | Panel | Action | Pairwise | VICReg | Val MSE | Val Cosine | Test MSE | Test Cosine |
| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| A0 | taxonomy only | yes | no | yes | 0.07541437962770774 | 0.9745100258571918 | 0.15218315756551268 | 0.9637773457224168 |
| A1 | taxonomy + skills | yes | no | yes | 0.17999517062789205 | 0.8776115717741934 | 0.25307116609684355 | 0.867371052118299 |
| A2 | taxonomy + skills + transition | yes | no | yes | 0.07861760961555968 | 0.9584134219819948 | 0.2269089035423139 | 0.9097942446694102 |
| A3 | A2 | no | no | yes | 0.35150993421354887 | 0.7115692482591077 | 0.6428963516399238 | 0.5567848258865903 |
| A4 | A2 + pairwise | yes | yes | yes | 0.07653267888245231 | 0.9316788348516672 | 0.11159787261840416 | 0.9196961741053704 |
| A5 | A2 | yes | no | no | 0.07947118619845824 | 0.91059956073034 | 0.19526649207584376 | 0.8155035850933209 |

The no-action ablation has an explicit generated training target of `0.50`.
The generic career verifier reads that persisted config before applying the
metric gate.

## Paper-Small 1K Promotion

Primary 1K promotion bundle:

- `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1`;
- purpose: `paper_small`;
- career events: 1000;
- status: `passed`;
- FSV readbacks: 9;
- edge cases: 11;
- artifact hash rows: 9;
- ablation rows: 6.

Independent career DB readback:

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

## Paper-Small Multi-Seed Results

Final explicit-seed bundles:

| Run ID | Seed | Status |
| --- | ---: | --- |
| `20260430T_paper_small_1k_seed42_v2` | 42 | passed |
| `20260430T_paper_small_1k_seed43_v2` | 43 | passed |
| `20260430T_paper_small_1k_seed44_v1` | 44 | passed |

Career metric mean/stddev across the three passed explicit-seed bundles:

| Metric | Mean | Stddev | N |
| --- | ---: | ---: | ---: |
| `val_latent_mse` | 0.155409786323 | 0.013130850075 | 3 |
| `val_latent_cosine` | 0.917646185906 | 0.026095423035 | 3 |
| `test_latent_mse` | 0.291214507891 | 0.091692459686 | 3 |
| `test_latent_cosine` | 0.884851853288 | 0.053905889447 | 3 |
| `val_action_contrast_acc` | 0.958333333333 | 0.072168783649 | 3 |

Failed bundle kept for evidence:

- `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed43_v1`;
- failure: A0 taxonomy-only ablation did not converge with
  `val_latent_mse=0.510142` against target `0.080000`;
- root cause: the first seed-control implementation varied the ablation seed
  together with the main career seed;
- fix: `--career-training-seed` now varies main career training, while
  ablations stay on the checked-in control seed unless
  `--career-ablation-training-seed` is explicitly supplied.

## Manual Edge FSV

Career edge cases in the current bundle:

| Edge | Raw Delta | Adapter Delta | State Delta | Transition Delta | Audit Delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| empty input | 0 | 0 | 0 | 0 | 1 |
| missing source license | 1 | 1 | 0 | 0 | 2 |
| invalid action | 1 | 1 | 0 | 0 | 2 |
| wrong skill-vector dimension | 1 | 1 | 0 | 0 | 2 |
| out-of-bounds job zone | 1 | 1 | 0 | 0 | 2 |

Wrapper edge cases manually verified:

| Edge | Source-Of-Truth Outcome |
| --- | --- |
| `paper_small` without `--onet-text-zip` | typed `VALIDATION`, no bundle dir created |
| `--career-event-count` without `--onet-text-zip` | typed `VALIDATION`, no bundle dir created |
| bad O*NET ZIP path | typed `VALIDATION`, failed bundle with `run_manifest.json` and `fsv_report.json` |

## Root-Cause Fixes Closed

- Removed undeclared O*NET event fields from adapter input and moved per-edge
  provenance to the generated walk CSV.
- Stabilized event-level source hashes so the generated walk compiles into one
  trajectory instead of one trajectory per transition.
- Scaled generated `experience_years` to remain within the domain max of 40.0.
- Restricted the O*NET smoke walk to core computer occupations and required the
  Software Developers start node.
- Prioritized occupation title semantics before description semantics during
  cluster classification.
- Added explicit no-action ablation training target and verifier config
  readback.
- Moved `--career-event-count` without O*NET validation before bundle creation.

## Automated Test Evidence

Commands to run before promotion:

```bash
cargo check -p context-graph-cli --all-features
cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv --all-features -- --nocapture
```

The integration test uses the real CLI binary, real RocksDB stores, the real
O*NET 30.2 ZIP, and real CUDA training. Additional fail-closed tests assert
that paper runs without O*NET and custom event counts without O*NET do not
create a bundle source of truth.

Latest result:

- `cargo check -p context-graph-cli --all-features`: passed;
- `cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv
  --all-features -- --nocapture`: 3 passed, 0 failed, 177.74 seconds.

## Remaining Work Before Full Demo Run

- The `paper_small` 1K O*NET run is complete.
- The three-seed paper-small career benchmark is complete.
- Keep the paper claim scoped to an auditable public-taxonomy benchmark, not
  real recruiting prediction.
- Decide whether ESCO is added as a replication/crosswalk source after the
  O*NET paper-small run.
