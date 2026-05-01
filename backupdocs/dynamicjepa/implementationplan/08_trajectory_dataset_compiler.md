# Trajectory And Dataset Compiler

DynamicJEPA needs reliable sequence construction before model training. A trajectory is an ordered chain of observed transitions and panels. A dataset shard is a physical training input derived from trajectories.

## Trajectory Construction

A trajectory groups transitions by a domain-defined identity:

- session id;
- entity id;
- actor id;
- task id;
- file path;
- graph walk id;
- learner id;
- custom stable key declared by the domain pack.

`TrajectoryRecord` fields:

- `trajectory_id`;
- `domain_pack_id`;
- `segmentation_policy_id`;
- `ordered_transition_ids`;
- `ordered_panel_ids`;
- `start_time_unix_ms`;
- `end_time_unix_ms`;
- `entity_refs`;
- `binding_refs`;
- `trajectory_hash`;
- `record_count`.

Rules:

- timestamps must be monotonic after sorting;
- duplicate transition ids are rejected;
- missing panel ids are rejected unless compiling raw-only diagnostic trajectories;
- trajectory hash is deterministic from ordered ids and policy id.

## Dataset Shard Construction

Dataset shards convert trajectories into row-major examples for JEPA training.

`DatasetShardRecord` fields:

- `dataset_id`;
- `shard_id`;
- `domain_pack_id`;
- `source_trajectory_ids`;
- `split_name`;
- `row_count`;
- `input_panel_ids`;
- `target_panel_ids`;
- `action_ids`;
- `negative_panel_ids`;
- `objective_ids`;
- `shape_summary`;
- `source_hashes`;
- `leakage_report`;
- `compiler_version`.

Rules:

- dataset rows reference persisted panel ids, not raw inline vectors only;
- serialized vectors may be included for speed, but references are required for verification;
- split assignment is deterministic by declared split key;
- no future leakage in training inputs;
- every negative sample must be explainable by a declared policy.

## Split Policy

Start with:

- `by_entity`: same entity never crosses train/validation/test.
- `by_time`: train before validation before test.
- `by_domain_key_hash`: deterministic hash buckets.

Reject:

- random unseeded split;
- split key missing from domain pack;
- overlap between splits.

## Negative Sampling Policy

Supported v1 policies:

- `same_domain_different_outcome`;
- `same_entity_different_time`;
- `constellation_antipode`;
- `contrastive_pair_cf`.

Rules:

- negative rows must reference persisted source panel ids;
- no generated phantom negatives;
- if there are not enough real negatives, compilation fails with an explicit count error;
- the failure must not write a partial shard.

## APIs

Service:

- `compile_trajectories(domain_pack_id, segmentation_policy)`
- `compile_dataset_shard(domain_pack_id, dataset_policy, split)`
- `inspect_dataset_row(dataset_id, shard_id, row_index)`

Storage:

- `put_trajectory_batch`
- `get_trajectory`
- `list_trajectories`
- `put_dataset_shard`
- `get_dataset_shard`
- `count_dataset_shards`

MCP:

- `dynamicjepa_compile_trajectories`
- `dynamicjepa_list_trajectories`
- `dynamicjepa_get_trajectory`
- `dynamicjepa_compile_dataset`
- `dynamicjepa_get_dataset_shard`

CLI:

- `dynamicjepa compile-trajectories --domain ...`
- `dynamicjepa compile-dataset --domain ... --split train`
- `dynamicjepa inspect-dataset --dataset-id ... --shard-id ... --row 0`

## FSV Expectations

For the deterministic `counter_world` fixture with three events:

```text
t0: counter 0 + 1 = 1
t1: counter 1 + 2 = 3
t2: counter 3 + 1 = 4
```

Expected:

- 3 raw events;
- 3 transitions;
- 3 panels;
- 1 trajectory with 3 ordered transition ids;
- dataset shard row count 2 for one-step prediction pairs;
- row 0 input panel references t0 and target panel references t1;
- row 1 input panel references t1 and target panel references t2.

Manual verification must read the trajectory and dataset shard from RocksDB and print the actual ordered ids and row count.
