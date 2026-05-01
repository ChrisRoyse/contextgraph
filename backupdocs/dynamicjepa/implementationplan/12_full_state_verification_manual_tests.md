# Full State Verification Manual Tests

This file specifies the manual verification protocol required before claiming DynamicJEPA works. It intentionally uses deterministic synthetic real input through the actual system and a real RocksDB database.

## Source Of Truth

Primary:

- RocksDB database passed through `--db`;
- DynamicJEPA column families;
- decoded records from CLI `inspect-cf`, `get-*`, and `inspect-counts`.

Secondary:

- artifact files under `--artifact-root`;
- file hashes recomputed during verification.

Evidence must include actual decoded record data after execution.

## Required Fixture Domain

Use `counter_world.v1.toml` and event fixtures.

Events:

```text
event A: state.counter=0, action.delta=1, outcome.next_counter=1
event B: state.counter=1, action.delta=2, outcome.next_counter=3
event C: state.counter=3, action.delta=1, outcome.next_counter=4
```

Known expected outcomes:

- each outcome equals `state.counter + action.delta`;
- trajectory order is A, B, C;
- one-step dataset has 2 rows: A->B and B->C;
- panel count is 3 after all events materialize.

## Happy Path Manual Test

Use a fresh real RocksDB directory:

```bash
export DJ_DB=/tmp/contextgraph-dynamicjepa-fsv
export DJ_ARTIFACTS=/tmp/contextgraph-dynamicjepa-artifacts
rm -rf "$DJ_DB" "$DJ_ARTIFACTS"
```

Commands to implement and run:

```bash
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa register-domain --db "$DJ_DB" --file configs/dynamicjepa/domain_packs/counter_world.v1.toml --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa ingest-event --db "$DJ_DB" --domain counter_world --adapter json_event --file configs/dynamicjepa/verification/counter_world_happy_path.jsonl --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa materialize-panel --db "$DJ_DB" --all-pending --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa compile-trajectories --db "$DJ_DB" --domain counter_world --policy by_domain_session --json
context-graph dynamicjepa compile-dataset --db "$DJ_DB" --domain counter_world --policy one_step --split train --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa train --db "$DJ_DB" --dataset-id DATASET_ID --config configs/dynamicjepa/verification/tiny_train_config.json --artifact-root "$DJ_ARTIFACTS" --json
context-graph dynamicjepa predict --db "$DJ_DB" --artifact-id ARTIFACT_ID --panel-id PANEL_B --action-id ACTION_C --json
context-graph dynamicjepa plan --db "$DJ_DB" --artifact-id ARTIFACT_ID --panel-id PANEL_B --skill-id COUNTER_INCREMENT_SKILL --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
```

Physical readbacks required:

```bash
context-graph dynamicjepa get-domain --db "$DJ_DB" --id DOMAIN_PACK_ID --json
context-graph dynamicjepa inspect-panel --db "$DJ_DB" --panel-id PANEL_A --include-readings --json
context-graph dynamicjepa get-trajectory --db "$DJ_DB" --id TRAJECTORY_ID --json
context-graph dynamicjepa get-dataset-shard --db "$DJ_DB" --dataset-id DATASET_ID --shard-id SHARD_ID --json
context-graph dynamicjepa get-training-run --db "$DJ_DB" --id TRAINING_RUN_ID --json
context-graph dynamicjepa get-artifact --db "$DJ_DB" --id ARTIFACT_ID --verify-files --json
context-graph dynamicjepa get-prediction --db "$DJ_DB" --id PREDICTION_ID --json
context-graph dynamicjepa get-plan-trace --db "$DJ_DB" --id PLAN_TRACE_ID --json
```

Expected final counts for the three-event happy path:

- domain packs: 1;
- raw events: 3;
- adapter runs: 3 completed;
- states: 3;
- actions: 3;
- outcomes: 3;
- transitions: 3;
- instrument readings: at least 3, exact count equals `event_count * required_instrument_count`;
- panels: 3;
- trajectories: 1;
- dataset shards: 1;
- training runs: 1 completed;
- model artifacts: at least 1;
- predictions: 1;
- plan traces: 1.

## Edge Case 1: Empty Input

Before:

```bash
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
```

Action:

```bash
context-graph dynamicjepa ingest-event --db "$DJ_DB" --domain counter_world --adapter json_event --file configs/dynamicjepa/verification/empty_input.json --json
```

Expected:

- command fails with validation error;
- raw event count unchanged if the file is empty before payload creation;
- no adapter run;
- no state/action/outcome;
- no panel.

After:

```bash
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa inspect-cf --db "$DJ_DB" --cf CF_DJ_AUDIT_LOG --limit 20 --json
```

## Edge Case 2: Invalid Action Format

Fixture:

```json
{"state":{"counter":2},"action":{"delta":"two"},"outcome":{"next_counter":4},"ts":2000}
```

Expected:

- raw event may be written if ingestion captures payload first;
- adapter run is written with status `failed`;
- normalized action not written;
- transition not written;
- panel not written;
- error includes field path `action.delta` and expected type `i64`.

Verify before/after counts and read the failed adapter run row.

## Edge Case 3: Maximum Declared Panel Shape

Create a domain pack variant with declared max slots and max dimensions allowed by v1 limits.

Expected:

- registration succeeds if within declared limits;
- event materialization writes exactly the declared number of readings;
- panel shape summary matches declared total dimensions;
- dataset compiler accepts the panel if all required slots exist;
- any dimension above max fails before panel write.

Verify by reading:

- domain pack max shape;
- every instrument reading;
- latent panel shape summary;
- count deltas.

## Failure Handling Rule

If any manual test produces an unexpected count, missing row, mismatched hash, missing linked reference, or ambiguous error, stop. Identify root cause from the source-of-truth records and logs. Fix the implementation and repeat the full happy path plus the failed edge case.
