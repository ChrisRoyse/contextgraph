# 04 - Smoke Ladder And Gates

The smoke ladder is the iteration loop before any full paper run. A gate can
only pass when the command succeeds and the source of truth is independently
read back.

## Gate 0 - Environment

Trigger:

```bash
nvidia-smi --query-gpu=name,driver_version,compute_cap,memory.total --format=csv,noheader
nvcc --version
rustc --version
cargo --version
```

Source of truth:

- `environment.json`;
- `hardware.json`;
- command log entries.

Pass criteria:

- RTX 5090-class GPU visible;
- compute capability 12.0;
- CUDA 13.2 available;
- `CUDA_COMPUTE_CAP=120`;
- Rust/Cargo available.

Edge cases:

- missing CUDA;
- wrong compute cap;
- missing `target/debug/context-graph`.

## Gate 1 - Storage And CLI

Trigger:

```bash
./target/debug/context-graph dynamicjepa inspect-counts --db "$DB" --json
./target/debug/context-graph dynamicjepa inspect-cf --db "$DB" --cf not_a_cf --json
```

Source of truth:

- RocksDB at `$DB`;
- `db_counts_before.json`;
- error command log for invalid CF.

Pass criteria:

- all DynamicJEPA CFs open and count zero on fresh DB;
- invalid CF fails with typed error;
- no data CF mutates on invalid CF.

## Gate 2 - Phase 10 Runtime Controls

Trigger:

```bash
./target/debug/context-graph dynamicjepa verify-counter-world ...
./target/debug/context-graph dynamicjepa verify-gridworld ...
```

Source of truth:

- `dj_verification_runs`;
- Phase 10 evidence bundles;
- artifact hashes;
- physical `ldb --count_only` spot checks.

Pass criteria:

- both verification rows have `status=Passed`;
- artifact hash checks all equal;
- edge case files exist and expected deltas match;
- decoded counts match `inspect-counts`.

## Gate 3 - Career Domain Registration

Trigger:

```bash
./target/debug/context-graph dynamicjepa register-domain \
  --db "$DB" \
  --file configs/dynamicjepa/domain_packs/career_taxonomy.v1.toml \
  --json
```

Source of truth:

- `dj_domain_packs`;
- `dj_domain_pack_by_nv`;
- `dj_instrument_registry`;
- `dj_adapter_registry`;
- `data_manifest.json`.

Pass criteria:

- domain row reads back with expected source hash;
- data sources include URL, license, local hash, and row count;
- missing license/provenance blocks the run before ingest.

Edge cases:

- malformed TOML;
- duplicate instrument ID;
- missing source license manifest.

## Gate 4 - Career Event Ingest

Trigger:

```bash
./target/debug/context-graph dynamicjepa ingest-event \
  --db "$DB" \
  --domain career_taxonomy \
  --adapter json_event \
  --file "$CAREER_FIXTURE" \
  --json
```

Source of truth:

- `dj_raw_events`;
- `dj_adapter_runs`;
- `dj_normalized_states`;
- `dj_actions`;
- `dj_outcomes`;
- `dj_transitions`;
- `dj_audit_log`.

Pass criteria:

- 10-50 events ingest for smoke;
- no unexpected extra JSON fields;
- every event carries provenance/license fields;
- decoded sample rows match expected career fixture values.

Edge cases:

- empty file;
- missing provenance field;
- invalid action category;
- out-of-bounds numeric field.

## Gate 5 - Panel And Trajectory

Trigger:

```bash
./target/debug/context-graph dynamicjepa materialize-panel --db "$DB" --all-pending --domain career_taxonomy --json
./target/debug/context-graph dynamicjepa compile-trajectories --db "$DB" --domain career_taxonomy --policy by_domain_session --json
```

Source of truth:

- `dj_instrument_readings`;
- `dj_latent_panels`;
- `dj_trajectories`.

Pass criteria:

- every required career instrument has an `Ok` reading;
- panel dimensions match domain pack;
- trajectory timestamps are strict ascending;
- entity grouping by pseudonymous trajectory/person ID works.

## Gate 6 - Dataset And Tiny Training

Trigger:

```bash
./target/debug/context-graph dynamicjepa compile-dataset --db "$DB" --domain career_taxonomy --policy one_step --split train --json
./target/debug/context-graph dynamicjepa compile-dataset --db "$DB" --domain career_taxonomy --policy one_step --split val --json
./target/debug/context-graph dynamicjepa compile-dataset --db "$DB" --domain career_taxonomy --policy one_step --split test --json
./target/debug/context-graph dynamicjepa train --db "$DB" --dataset-id "$DATASET" --config "$TRAIN_CONFIG" --artifact-root "$ARTIFACT_ROOT" --json
```

Source of truth:

- `dj_dataset_shards`;
- `dj_training_runs`;
- `dj_model_artifacts`;
- artifact files and hashes.

Pass criteria:

- split counts are recorded;
- leakage report is zero for declared policy;
- training run completes;
- artifact hashes recompute;
- random-init/shuffled-target baselines are recorded.

## Gate 7 - Prediction, Planning, Surprise

Trigger:

```bash
./target/debug/context-graph dynamicjepa predict ...
./target/debug/context-graph dynamicjepa plan ...
./target/debug/context-graph dynamicjepa record-surprise ...
```

Source of truth:

- `dj_predictions`;
- `dj_skill_policies`;
- `dj_guard_decisions`;
- `dj_plan_traces`;
- `dj_surprise_events`.

Pass criteria:

- prediction references verified artifact hash;
- plan stores concrete candidate actions and guard decisions;
- forced surprise writes exactly one surprise row;
- invalid observed panel/outcome mismatch fails without mutation.

## Gate 8 - Ablation Smoke

Run a tiny matrix:

- taxonomy only;
- taxonomy + skill vector;
- taxonomy + skill + trajectory features;
- action-conditioned vs no-action baseline if implemented;
- VICReg on/off if implemented as a config switch.

Pass criteria:

- every ablation has a run manifest, metrics, artifact hash, and FSV readback;
- table rows are generated automatically;
- failed ablations are recorded as failed, not omitted.

## Gate 9 - Reproducibility

Run the same smoke config twice with the same seed.

Pass criteria:

- both runs complete;
- split manifests match;
- metric variance is recorded;
- artifact hashes either match exactly or the nondeterminism is explained and
  bounded in `reproducibility.json`.

