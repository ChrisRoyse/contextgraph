# Appendix

## A. Domain Pack: `career_taxonomy.v1` (verbatim TOML)

The full domain pack is `configs/dynamicjepa/domain_packs/career_taxonomy.v1.toml`.
Reproduced verbatim:

```toml
[domain]
id = "career_taxonomy"
version = "1.0.0"
title = "Career Taxonomy - Public Occupational Transition Smoke Benchmark"
schema_version = 1

[time]
field = { name = "step_index", kind = "i64", required = true, min = 0, max = 1000000 }

[[state.fields]]
name = "occupation_cluster"
kind = { type = "categorical", variants = ["software", "data", "systems", "management"] }
required = true

[[state.fields]]
name = "job_zone"
kind = { type = "i64" }
required = true
min = 1
max = 5

[[state.fields]]
name = "skill_vector"
kind = { type = "vector", dim = 4 }
required = true
min = 0.0
max = 1.0

[[state.fields]]
name = "experience_years"
kind = { type = "f64" }
required = true
min = 0.0
max = 40.0

[[state.fields]]
name = "wage_band"
kind = { type = "categorical", variants = ["low", "mid", "high"] }
required = true

[[action.fields]]
name = "move"
kind = { type = "categorical", variants = ["skill_gain_adjacent", "skill_gain_high_demand", "role_move_near", "experience_gain", "noop"] }
required = true

[[outcome.fields]]
name = "next_occupation_cluster"
kind = { type = "categorical", variants = ["software", "data", "systems", "management"] }
required = true

[[outcome.fields]]
name = "next_job_zone"
kind = { type = "i64" }
required = true
min = 1
max = 5

[[outcome.fields]]
name = "next_skill_vector"
kind = { type = "vector", dim = 4 }
required = true
min = 0.0
max = 1.0

[[outcome.fields]]
name = "mobility_label"
kind = { type = "categorical", variants = ["same", "near_transition", "far_transition"] }
required = true

[[entity.fields]]
name = "trajectory_id"
kind = { type = "string" }
required = true

[[entity.fields]]
name = "source_dataset"
kind = { type = "string" }
required = true
```

(File length and headers may grow in future versions; this excerpt is
the current `career_taxonomy.v1` shipped with the v1.0.0 artifact.)

The other two domain packs are at:

- `configs/dynamicjepa/domain_packs/counter_world.v1.toml`
- `configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml`

Both are <100 lines and are included in the artifact archive verbatim.

## B. RocksDB Column Family Reference

The 25 DynamicJEPA column families and their contents:

| CF | Contents (first key fields) |
| --- | --- |
| dj_domain_packs | `domain_pack_id` (uuid) → `DomainPack` |
| dj_domain_pack_by_nv | `(name, version)` → `domain_pack_id` |
| dj_instrument_registry | `instrument_id` (uuid) → `InstrumentSpec` |
| dj_adapter_registry | `adapter_id` (uuid) → `AdapterSpec` |
| dj_raw_events | `event_id` (uuid) → `RawDomainEvent` |
| dj_normalized_states | `state_id` (uuid) → `NormalizedState` |
| dj_actions | `action_id` (uuid) → `NormalizedAction` |
| dj_outcomes | `outcome_id` (uuid) → `NormalizedOutcome` |
| dj_transitions | `transition_id` (uuid) → `StateTransition` |
| dj_adapter_runs | `(domain, adapter, run_id)` → `AdapterRunRecord` |
| dj_instrument_readings | `(event_id, instrument_id)` → `InstrumentReading` |
| dj_latent_panels | `panel_id` (uuid) → `LatentPanel` |
| dj_bindings | `binding_id` (uuid) → `BindingRecord` |
| dj_bindings_by_entity | `(entity_id, binding_id)` (secondary) |
| dj_trajectories | `trajectory_id` (uuid) → `TrajectoryRecord` |
| dj_dataset_shards | `dataset_shard_id` (uuid) → `DatasetShardRecord` |
| dj_training_runs | `(domain, run_id)` → training row |
| dj_model_artifacts | `model_artifact_id` (uuid) → artifact + 5 hashed files |
| dj_predictions | `(domain, prediction_id)` → prediction row |
| dj_skill_policies | `skill_id` (uuid) → policy |
| dj_plan_traces | `(domain, plan_trace_id)` → plan trace |
| dj_guard_decisions | `(domain, guard_decision_id)` → guard row |
| dj_surprise_events | `(domain, surprise_event_id)` → surprise row |
| dj_verification_runs | `(domain, verification_run_id)` → verification row |
| dj_audit_log | `(prefix, sequence)` → `DjAuditRecord` |

Source:
[`crates/context-graph-storage/src/dynamicjepa/column_families.rs`](../../../crates/context-graph-storage/src/dynamicjepa/column_families.rs).

## C. Subcommand Cheat-Sheet

`context-graph dynamicjepa <subcommand>`. From
[`crates/context-graph-cli/src/commands/dynamicjepa.rs`](../../../crates/context-graph-cli/src/commands/dynamicjepa.rs)
lines 75–149.

Domain pack: `register-domain`, `list-domains`, `get-domain`,
`inspect-counts`, `inspect-cf`.

Ingest: `ingest-event`, `run-adapter`.

Panel: `materialize-panel`, `get-panel`, `list-instrument-readings`,
`bind`, `list-bindings`.

Trajectory/dataset: `compile-trajectories`, `get-trajectory`,
`list-trajectories`, `compile-dataset`, `get-dataset-shard`,
`inspect-dataset`.

Train/predict/plan/surprise: `train`, `get-training-run`,
`get-artifact`, `predict`, `plan`, `record-surprise`,
`get-prediction`, `get-plan-trace`, `get-surprise`.

Domain verification: `verify-counter-world`, `verify-gridworld`,
`verify-career-taxonomy`.

Research bundle: `research-smoke`, `hp-sweep`, `multiseed-aggregate`.

Reference checking: `capture-reference`, `check-reference`,
`check-release-aggregate-reference`.

## D. Worked Failed Run: Seed-43 Ablation

Preserved bundle:
`tmp/mejepa_research_runs/20260430T_paper_small_1k_seed43_v1`
(the original seed-43 attempt, before the seed-control patch).

Symptom: A3 (action removed) ablation training run produced
`val_latent_cosine = 0.6275242888485578`,
`val_latent_mse = 0.38418309787311644`. The career metric gate
required `val_latent_cosine ≥ 0.50`; this passed. But A3 was failing
in the *aggregate* because the same gate failed on a different seed
(see seed-45).

Root cause (after investigation): `career_ablation_training_seed`
defaulted to the main career training seed. With main seed = 43,
A3's training data ordering differed from the source-fixture control;
A3's no-action-conditioning negative control became seed-sensitive at
n = 1{,}000.

Fix: separate `--career-training-seed` (main career run) from
`--career-ablation-training-seed` (ablation training). The default
for the latter is the source-fixture control seed (effective `42`)
unless explicitly overridden. The `run_manifest.json` records both:

```json
{
  "career_training_seed": 45,
  "career_ablation_training_seed": null,
  "career_ablation_training_seed_effective": 42,
  "career_ablation_training_seed_origin":
    "source_fixture_control_separated_from_main_seed"
}
```

(Source: seed-45 v3 run manifest; full evidence at
[reports/20260430_seed45_46_release_aggregate_fsv.md](../smokedocs/reports/20260430_seed45_46_release_aggregate_fsv.md).)

The failed bundles are intentionally retained as evidence in
`tmp/mejepa_research_runs/20260430T_paper_small_1k_seed{43_v1,45_v1,45_v2,46_v1}`.

## E. Hash Reference

Critical hashes for §7 and §8:

```text
O*NET 30.2 source ZIP
  b5479271931796b838f7173dc0f673a9ec961b7833ac87168fd11e92e7453741

Release-C aggregate
  aggregate_manifest.json
    734a698c7ed946c292cf9005c4083f7205a2a37b271037870418103eb68670e0
  aggregate_fsv_report.json
    9b1bc096c850fbd87d44447f53d33da58c35cd4337292ffbde82f63cc6b6a9b6
  table_prediction_metrics_multiseed.csv
    6bd81ee3b53b90c495263378c492a7fed2b123cbcfd5007ee0a14a99702608e7
  table_ablation_multiseed.csv
    1934260ce6d0a39b6d04d0c857e097ff3d19c3e407fca6b00760548e5936c6a9
  table_paired_ablation_tests.csv
    26218e9705c2170b25a7251f51234a4fb18b02eed9ab92339189f22a6dd88d80
  table_seed_failures.csv
    8c4e8bfb875f646993fb453e8e35203faca6fd99c41373ed8ef1ed455ec52d53
  multiseed_val_mse_box.svg
    69db6c4d2ec8b837100f23fbfe74c633c2c1aa29d0a981806dcf57c4123ed77c
  multiseed_paired_delta_forest.svg
    79f8050a1cf0c8c6b4f544f71461ecb2e8cc0672c987b14f1b84ee1b93bb04e4

Container artifacts
  Docker tar (deterministic, two-of-two)
    099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e
  Apptainer SIF (built via docker-archive)
    30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825
  CUDA base image (pinned by digest)
    sha256:d266e59b88c295bc5fa0e4cef9064eaff84939381b09e2c3d76a5532a303e42d

Release archive (rc5)
  release_manifest.json reference
    f5212d7fbf1da4df0f2dbbf6e520d4a97bc520f3e8d9a4dba1c113b73dd5c614
  bin/context-graph
    41560d160acdaade55f16153678db373c33127ce7a12b4eb5a418d30e86ec0eb
  mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz
    8cccf4980893fe949db9b50b8111e592e72ab8419438c4cd08e10c8974943ecd

Host environment fingerprint
  docs2/systemspecs.md
    acdc549a5913c59f46376414c08373a8363d646fd3c36d1fc6289a193c0e1205

Per-seed run manifests (release-C v2)
  seed42 run_manifest.json
    d1e072bdf8750c8f5b66810ec36441d315581de88f20a365998e0aa654cf6c3d
  seed43 run_manifest.json
    92c0d893eb2aa2a307c171cd768bafbb46021bc842f9348fabd08245786a11b5
  seed44 run_manifest.json
    4ed02856a89892fe249f7aeab9b9a8c4af0c432092a0b539fa2c06ff38688de1
  seed45 run_manifest.json
    ca729f76b2804652bbe1b668617e5d96c5739415ffc0804a4ac5d5245aaaf422
  seed46 run_manifest.json
    61bbd8ccc602d1553e4fc14650e8f64497a7debaad46fd5f645f13d4be99c8a6
```

## F. Glossary

- **JEPA** — Joint-Embedding Predictive Architecture. Predicts target
  representations rather than reconstructing observations.
- **ME-JEPA** — Multi-Embedder JEPA. The targets are produced by a
  panel of frozen, externally-defined per-domain instruments.
- **Dynamic/ME-JEPA** — The runtime in this paper. Adds domain packs,
  source-of-truth storage, full-state verification, and audited
  planning/surprise.
- **Domain pack** — A TOML manifest declaring state schema, action
  schema, instruments, and panel slots; the only thing a new domain
  ships.
- **FSV** — Full-State Verification. Every operation has a separate
  read path that re-reads the source of truth.
- **CF** — RocksDB column family. The runtime uses 25 DynamicJEPA-
  prefixed CFs plus the audit log.
- **BCa bootstrap** — Bias-corrected and accelerated bootstrap CI.
  10{,}000 iterations, seed `20260430`, confidence 0.95.
- **Sign-flip permutation test** — Paired-sample non-parametric test
  with two-sided $p$ obtained by enumerating all $2^n$ sign assignments
  ($p = 0.0625$ floor at $n = 5$).
- **Reference (release reference)** — A checked-in JSON specifying the
  expected aggregate hash chain and the numeric tolerance schema.
  Drift fails the gate.
- **Release candidate (rc5)** — The current local Docker + SIF
  release-candidate archive; Zenodo DOI publication is the only
  remaining final-release blocker.

## G. Glossary of Bundle Files

What each per-bundle JSON/CSV contains. (Files reproduced into every
research-smoke bundle.)

- `run_manifest.json` — top-level run metadata (status, seed, event
  count, hardware/software fingerprint).
- `fsv_report.json` — verification rows, artifact hash checks, edge
  cases.
- `data_manifest.json` — source URLs, source ZIP SHA-256, generated
  walk hash, license attribution string.
- `domain_pack_manifest.json` — registered domain packs and their
  hashes.
- `environment.json` — `nvcc`, `nvidia-smi`, `rustc`, `cargo`, `uname`,
  `lscpu`, `free`, `cargo tree -p candle-core` outputs.
- `hardware.json` — GPU name/driver/compute-capability/memory; CPU.
- `git_state.json` — repo HEAD, dirty bit, untracked-file count.
- `evaluation_report.json` — model-side eval metrics (referenced by
  artifact hashes).
- `training_metrics.json` — per-epoch loss, validation metrics, peak
  VRAM, throughput.
- `command_log.jsonl` — chronological list of subcommands run.
- `command_outputs/` — captured stdout/stderr per subcommand.
- `edge_case_results.json` — 11 edge-case rows.
- `ablation_results.json` — 6 ablation rows + decoded training-run
  cross-references.
- `db_counts_before.json`, `db_counts_after.json` — snapshot of every
  CF's row count for each domain.
- `dbs/<run_id>_<domain>_rocksdb/` — three per-domain RocksDB stores
  (counter\_world, gridworld\_5x5, career\_taxonomy).
- `generated_fixtures/onet_30_2/` — generated walk JSONL + CSV under
  the verified O\*NET source-ZIP hash.
- `decoded_records/` — selected `inspect-cf` decoded rows.
- `fsv_evidence/` — pre/post-state captures for each FSV harness.
- `local_ci_inspect_counts_career_taxonomy.json` — independent
  `inspect-counts` readback from the career RocksDB.
- `onet_source_preflight.json` — per-bundle archive preflight evidence
  (file count = 41, archive entries = 42, required member row counts,
  license marker, release marker, source-ZIP SHA-256).
- `artifact_hashes.json` — registry-vs-recomputed file hashes for
  every model artifact; the gate enforces zero mismatches.
- `paper_tables/*.csv` — `table_prediction_metrics.csv`,
  `table_ablation.csv`, `table_runtime.csv`, `table_auditability.csv`,
  `table_edge_cases.csv`, `table_data_sources.csv`,
  `table_domain_runtime_coverage.csv`,
  `table_single_gpu_feasibility.csv`,
  `table_collapse_diagnostics.csv`.
- `plots/*.svg` and `plots/plot_manifest.json`.

The aggregate bundle adds `aggregate_manifest.json`,
`aggregate_fsv_report.json`, the four `*_multiseed.csv` files, and
the two multiseed plots described in §6.7 and §7.6.
