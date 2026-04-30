# 13 - Current Implementation And Benchmark Status

Status date: 2026-04-30.

This document records the first implemented 5090 ME-JEPA research-smoke slice
after the planning docs. It is intentionally evidence-oriented: every result
below points at a physical DB path, artifact root, or evidence directory.

## Implemented Surface

Added career-domain smoke support to the DynamicJEPA runtime:

- `dynamicjepa verify-career-taxonomy`
- `dynamicjepa research-smoke`
- `configs/dynamicjepa/domain_packs/career_taxonomy.v1.toml`
- `configs/dynamicjepa/verification/career_taxonomy_smoke.jsonl`
- career edge fixtures:
  - empty input;
  - missing source license/provenance;
  - invalid action;
  - wrong skill-vector dimension;
  - out-of-bounds `job_zone`.
- `configs/dynamicjepa/verification/career_taxonomy_train_config.json`
- `configs/dynamicjepa/verification/career_taxonomy_data_manifest.json`

Runtime support added:

- vector instruments for fixed-dimension skill vectors;
- adapter numeric min/max checks for vector components;
- career `bounds_check` planning guard for `job_zone`/`move` schemas;
- career utility score based on bounded predicted `job_zone` minus uncertainty.
- immutable research bundle wrapper with environment capture, git state,
  command log JSONL, per-domain DBs, decoded CF readbacks, artifact hash
  readbacks, reproducibility, ablation, and generated paper tables.
- full career ablation matrix A0-A5 with generated domain-pack/config/fixture
  variants and independent RocksDB readback.
- target-encoder surprise comparison, matching the JEPA training objective.
- no-action ablation support through persisted `model.predictor.ignore_action`
  with zeroed action vectors during training, prediction, and surprise
  observation encoding.

The existing real CLI FSV test now covers:

- counter_world;
- gridworld_5x5;
- career_taxonomy.

## Environment Evidence

Observed local hardware/software:

| Item | Observed |
| --- | --- |
| GPU | NVIDIA GeForce RTX 5090 |
| Driver | 596.21 |
| Compute capability | 12.0 |
| VRAM | 32607 MiB |
| CUDA | 13.2, V13.2.78 |
| Rust | rustc 1.95.0 |
| Cargo | cargo 1.95.0 |

## Latest Manual FSV Runs

### Counter World

Command:

```bash
CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa verify-counter-world \
  --db /tmp/mejepa-counter-smoke-20260430T145900Z \
  --fixtures-root configs/dynamicjepa \
  --artifact-root /tmp/mejepa-counter-artifacts-20260430T145900Z \
  --evidence-root tmp/mejepa_research_runs/20260430T145900Z_counter_verify \
  --json
```

Evidence:

- DB: `/tmp/mejepa-counter-smoke-20260430T145900Z`
- Artifact root: `/tmp/mejepa-counter-artifacts-20260430T145900Z`
- Evidence directory:
  `tmp/mejepa_research_runs/20260430T145900Z_counter_verify/e6c098e2-c215-472a-a42f-3c050b958a08`
- Verification run: `e6c098e2-c215-472a-a42f-3c050b958a08`
- Status: passed

Key counts:

| CF | Count |
| --- | ---: |
| `dj_raw_events` | 62 |
| `dj_latent_panels` | 60 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 |
| `dj_model_artifacts` | 1 |
| `dj_predictions` | 6 |
| `dj_plan_traces` | 1 |
| `dj_guard_decisions` | 5 |
| `dj_surprise_events` | 1 |
| `dj_verification_runs` | 1 |

Metrics:

| Metric | Value |
| --- | ---: |
| params | 480 |
| train rows | 41 |
| val rows | 12 |
| test rows | 6 |
| val latent MSE | 0.004330990884665671 |
| val latent cosine | 0.9996336712676964 |
| test latent MSE | 0.013100919986046714 |
| test latent cosine | 0.9991602338714566 |

### Gridworld 5x5

Command:

```bash
CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa verify-gridworld \
  --db /tmp/mejepa-grid-smoke-20260430T150020Z \
  --fixtures-root configs/dynamicjepa \
  --artifact-root /tmp/mejepa-grid-artifacts-20260430T150020Z \
  --evidence-root tmp/mejepa_research_runs/20260430T150020Z_grid_verify \
  --json
```

Evidence:

- DB: `/tmp/mejepa-grid-smoke-20260430T150020Z`
- Artifact root: `/tmp/mejepa-grid-artifacts-20260430T150020Z`
- Evidence directory:
  `tmp/mejepa_research_runs/20260430T150020Z_grid_verify/4229ec40-09c1-4127-ac96-a7e9d08d8359`
- Verification run: `4229ec40-09c1-4127-ac96-a7e9d08d8359`
- Status: passed

Key counts:

| CF | Count |
| --- | ---: |
| `dj_raw_events` | 8002 |
| `dj_latent_panels` | 8000 |
| `dj_trajectories` | 1000 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 |
| `dj_model_artifacts` | 1 |
| `dj_predictions` | 6 |
| `dj_plan_traces` | 1 |
| `dj_guard_decisions` | 5 |
| `dj_verification_runs` | 1 |

Metrics:

| Metric | Value |
| --- | ---: |
| params | 16960 |
| train rows | 4977 |
| val rows | 1330 |
| test rows | 693 |
| val latent MSE | 0.039493253560132675 |
| val latent cosine | 0.9944826529322832 |
| val action contrast | 0.9977443609022556 |
| test latent MSE | 0.03902848705576137 |
| test latent cosine | 0.9946155877235692 |

### Career Taxonomy

Command:

```bash
CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa verify-career-taxonomy \
  --db /tmp/mejepa-career-smoke-20260430T145720Z \
  --fixtures-root configs/dynamicjepa \
  --artifact-root /tmp/mejepa-career-artifacts-20260430T145720Z \
  --evidence-root tmp/mejepa_research_runs/20260430T145720Z_career_verify \
  --json
```

Evidence:

- DB: `/tmp/mejepa-career-smoke-20260430T145720Z`
- Artifact root: `/tmp/mejepa-career-artifacts-20260430T145720Z`
- Evidence directory:
  `tmp/mejepa_research_runs/20260430T145720Z_career_verify/5bd36a89-96b4-4b9a-9f93-2efb441981a3`
- Verification run: `5bd36a89-96b4-4b9a-9f93-2efb441981a3`
- Status: passed

Key counts:

| CF | Count |
| --- | ---: |
| `dj_raw_events` | 19 |
| `dj_adapter_runs` | 19 |
| `dj_normalized_states` | 15 |
| `dj_instrument_readings` | 150 |
| `dj_latent_panels` | 15 |
| `dj_trajectories` | 1 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 |
| `dj_model_artifacts` | 1 |
| `dj_predictions` | 6 |
| `dj_plan_traces` | 1 |
| `dj_guard_decisions` | 5 |
| `dj_verification_runs` | 1 |

Metrics:

| Metric | Value |
| --- | ---: |
| params | 15344 |
| train rows | 9 |
| val rows | 3 |
| test rows | 2 |
| val latent MSE | 0.19737686395995757 |
| val latent cosine | 0.7586874403033157 |
| val action contrast | 1.0 |
| test latent MSE | 0.2169700311804264 |
| test latent cosine | 0.8023594464623571 |

Artifact hash verification:

- all five files matched registry hashes:
  `config.json`, `evaluation_report.json`, `manifest.json`, `metrics.json`,
  `model.safetensors`.

Edge-case evidence:

| Edge | Expected Delta |
| --- | --- |
| empty input | raw +0, adapter +0, normalized +0, transition +0, audit +1 |
| missing source license | raw +1, adapter +1, normalized +0, transition +0, audit +2 |
| invalid action | raw +1, adapter +1, normalized +0, transition +0, audit +2 |
| wrong skill-vector dimension | raw +1, adapter +1, normalized +0, transition +0, audit +2 |
| out-of-bounds job zone | raw +1, adapter +1, normalized +0, transition +0, audit +2 |

## Research-Smoke Wrapper FSV

Command:

```bash
CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa research-smoke \
  --run-id 20260430T_smokedocs_full_matrix_fsv_v4 \
  --run-root tmp/mejepa_research_runs \
  --fixtures-root configs/dynamicjepa \
  --json
```

Bundle source of truth:

- Bundle:
  `tmp/mejepa_research_runs/20260430T_smokedocs_full_matrix_fsv_v4`
- RocksDB root:
  `tmp/mejepa_research_runs/20260430T_smokedocs_full_matrix_fsv_v4/dbs`
- Artifact root:
  `tmp/mejepa_research_runs/20260430T_smokedocs_full_matrix_fsv_v4/artifacts`
- Verifier evidence root:
  `tmp/mejepa_research_runs/20260430T_smokedocs_full_matrix_fsv_v4/fsv_evidence`

Wrapper result:

| Item | Value |
| --- | ---: |
| command log rows | 78 |
| training metric rows | 3 |
| artifact hash rows | 9 |
| edge case rows | 11 |
| ablation rows | 6 |
| same-seed repro metric deltas | 0.0 |

Readback counts recorded in `fsv_report.json` and rechecked with
`dynamicjepa inspect-counts`:

| Domain | Verification Runs | Artifacts | Predictions | Plan Traces | Guard Decisions | Surprise |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| counter_world | 1 | 1 | 6 | 1 | 5 | 1 |
| gridworld_5x5 | 1 | 1 | 6 | 1 | 5 | 0 |
| career_taxonomy | 1 | 1 | 6 | 1 | 5 | 0 |
| career_taxonomy_repro | 1 | 1 | 6 | 1 | 5 | 0 |
| career_taxonomy_ablation_A0_taxonomy_only | 1 | 1 | 6 | 1 | 5 | 0 |
| career_taxonomy_ablation_A2_transition | 1 | 1 | 6 | 1 | 5 | 0 |
| career_taxonomy_ablation_A3_no_action | 1 | 1 | 6 | 1 | 5 | 0 |
| career_taxonomy_ablation_A4_pairwise | 1 | 1 | 6 | 1 | 5 | 0 |
| career_taxonomy_ablation_A5_no_vicreg | 1 | 1 | 6 | 1 | 5 | 0 |

Ablation DB readback highlights:

| Ablation | Instruments | Readings | Input Dim | Val MSE | Val Cosine |
| --- | ---: | ---: | ---: | ---: | ---: |
| A0 taxonomy only | 8 | 120 | 22 | 0.07929272507681613 | 0.8989859668810602 |
| A1 base | 10 | 150 | 30 | 0.19737686395995757 | 0.7586874403033157 |
| A2 transition | 11 | 165 | 31 | 0.07951988157148059 | 0.8350982279978277 |
| A3 no action | 11 | 165 | 31 | 0.07769318943716391 | 0.8615845653067465 |
| A4 pairwise | 12 | 180 | 51 | 0.0779502250590159 | 0.8361759719794662 |
| A5 no VICReg | 11 | 165 | 31 | 0.07685959563824735 | 0.8186410244156163 |

Generated tables:

- `paper_tables/table_runtime.csv` - 79 lines;
- `paper_tables/table_prediction_metrics.csv` - 4 lines;
- `paper_tables/table_ablation.csv` - 7 lines;
- `paper_tables/table_auditability.csv` - 10 lines;
- `paper_tables/table_edge_cases.csv` - 12 lines;
- `paper_tables/table_data_sources.csv` - 2 lines.
- `paper_tables/table_domain_runtime_coverage.csv` - 10 lines;
- `paper_tables/table_single_gpu_feasibility.csv` - 2 lines.

Generated plots:

- `plots/prediction_val_mse.svg` - 1620 bytes;
- `plots/career_ablation_val_mse.svg` - 1331 bytes;
- `plots/plot_manifest.json` - status `rendered`.

Manual wrapper edge tests:

| Edge | Before State | After State |
| --- | --- | --- |
| invalid run id `../bad_smoke_edge` | no matching bundle dirs | typed `VALIDATION`, no bundle dir created |
| duplicate run id | manifest SHA-256 `70072d343be37d09f8d0418aad25027a6b1e328fbfb43e84d1981e6340faa4cf` | same manifest SHA-256, typed `VALIDATION` |
| missing fixtures root | run dir absent | failed bundle with `run_manifest.json` and `fsv_report.json`; status `failed` |

Root-cause fix found during FSV:

- The first A0 ablation trained to the generic smoke target but produced a
  known-target surprise (`cosine=0.4463`, threshold `0.5`). Fixed by giving the
  weaker A0 panel a stricter generated training target.
- A2 exposed a JEPA consistency bug: training evaluated predictions against the
  target encoder, while surprise encoded observations with the online encoder.
  Fixed by using the persisted target encoder for surprise observation vectors.
- A4 pairwise fixture generation initially failed on the intentionally invalid
  action edge because the derived pair contained `unsupported_move`. Fixed by
  keeping the invalid action as the edge trigger while deriving a schema-valid
  neutral pair for that state feature.

## Automated Test Evidence

Commands:

```bash
cargo test -p context-graph-cli --test dynamicjepa_phase10_fsv --all-features -- --nocapture
cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv --all-features -- --nocapture
```

Result:

- Phase 10 real CLI FSV test passed;
- domains covered: counter_world, gridworld_5x5, career_taxonomy;
- finished in 95.91 seconds after test binary build;
- research-smoke real CLI FSV test passed;
- wrapper test finished in 176.57 seconds;
- no mocks were used.

## Current Remaining Work

The smoke harness is implemented and passing. Remaining work before a full
paper run:

- finalize paper figure styling/dimensions if the generated SVG smoke plots are
  not the desired publication layout;
- add larger public-taxonomy/career benchmark data if a licensed source is
  selected;
- keep controlled taxonomy results clearly separated from claims about real
  recruiting histories.
