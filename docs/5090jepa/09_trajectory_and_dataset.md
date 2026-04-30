# 09 — Trajectory & Dataset Compiler (v1)

After events become panels, trajectories order them and dataset shards turn pairs of panels into training rows. The demo uses a single segmentation policy and a single objective; everything is deterministic.

Current implementation status as of the Phase 10 review on 2026-04-30: the upstream source-of-truth records this phase consumes (`dj_transitions`, `dj_instrument_readings`, `dj_latent_panels`, and `dj_trajectories`) are written and FSV-verified, and the trajectory plus one-step dataset compiler commands are operational through the CLI/storage path. Phase 7/10 training consumes those real dataset shards directly from RocksDB and verifies source hashes before training; Phase 10 records split counts in the persisted `VerificationRunRecord`.

## 1. Trajectory Compilation

`compile_trajectories(domain_pack_id, segmentation_policy)`:

1. Resolve the policy. Demo supports exactly one: `by_domain_session`. Any other policy → `Validation { field: "segmentation_policy_id" }`.
2. For `by_domain_session`:
   - Counter_world: every event in `counter_world_happy_path.jsonl` is one trajectory (ordered by `time.ts_ms`). Result = 1 trajectory.
   - Gridworld: events are grouped by `entity.trajectory_id`, ordered by `time.step_index`. Result = 1000 trajectories.
3. Compute `trajectory_hash = sha256(canonical_bytes_of(domain_pack_id, segmentation_policy_id, ordered_transition_ids))`.
4. Persist one `TrajectoryRecord` per group + one audit row in one batch.

Validation:

- `ordered_transition_ids` must be sorted by their `timestamp_ms` field. Strict ascending — duplicate timestamps reject.
- Every transition id must read back from `dj_transitions`.
- Every panel id in `ordered_panel_ids` must read back from `dj_latent_panels`.
- Re-compiling a trajectory that already exists rejects unless `--rebuild-if-exists` is passed.

## 2. Dataset Shard Compilation

`compile_dataset_shard(domain_pack_id, dataset_policy_id, split)`:

Demo supports one dataset policy: `one_step` (latent next-panel prediction). Pseudocode:

```
for trajectory in select_trajectories_for_split(split):
    for i in 0..(trajectory.len - 1):
        input_panel  = trajectory.panels[i]
        target_panel = trajectory.panels[i+1]
        action       = trajectory.transitions[i].action
        negative     = sample_negative(target_panel, policy.negative_sampling)
        rows.push(DatasetRow { input_panel, target_panel, action, negative })
write DatasetShardRecord { rows... }
```

### 2.1 Splits

- Counter_world: `by_time` — first ⌈N*0.7⌉ panels train, next ⌈N*0.2⌉ val, rest test. With N=3 transitions and one trajectory we hand-roll the split: row 0 (`t0→t1`) goes to train, row 1 (`t1→t2`) goes to val. Test split is empty for counter_world (acceptable; the verifier asserts `test_row_count == 0` is allowed when `expected_test_rows_*` is unset).
- Gridworld: `by_domain_key_hash` over `entity.trajectory_id`. Bucket sizes from `dataset_policy.split_buckets`. Deterministic via `blake3_128(trajectory_id) mod 1000` then bucket boundaries (`0..700` train, `700..900` val, `900..1000` test).

### 2.2 Negatives

Demo policy: `same_domain_different_outcome`. For each row, sample a panel from the same domain pack whose `next_state` value differs from the row's `target_panel.next_state` value. If the candidate pool is exhausted (no eligible negatives), fail with `DatasetLeakageDetected { message: "insufficient negatives", required: row_count, found: ... }`. **Never** invent a phantom negative. **Never** silently skip rows.

For counter_world the candidate pool is small (3 panels). The compiler must succeed because each row has at least one panel with a different `next_counter`.

For gridworld the candidate pool is 8000 panels; the compiler samples deterministically with a hash-derived seed `blake3_128((trajectory_id, step))`, so re-running produces identical negatives.

### 2.3 Leakage Audit

Before writing the shard, compile a `LeakageReport`:

```rust
pub struct LeakageReport {
    pub future_in_input_count: u64,        // must be 0
    pub same_panel_input_target_count: u64, // must be 0
    pub negative_equals_target_count: u64,  // must be 0
    pub split_overlap_count: u64,           // must be 0 (an entity in train must not appear in val/test)
}
```

Any non-zero value → `DatasetLeakageDetected` and **no shard row is written**.

### 2.4 Shape Summary

```rust
pub struct ShapeSummary {
    pub input_dim: usize,         // panel dense dim
    pub target_dim: usize,        // panel dense dim
    pub action_dim: usize,        // dim of the action onehot/scalar slot
    pub n_train_rows: u32,
    pub n_val_rows: u32,
    pub n_test_rows: u32,
}
```

The training loop reads this off the shard record so it never needs to re-walk the data to learn shapes.

## 3. Source Hashes

`source_hashes` is a list of `[content_hash]` from each input panel and target panel. Used to detect "the upstream panels changed" — if a shard's source hashes differ from the live panels, training must abort with `DatasetLeakageDetected { message: "shard source hashes diverged from live panels" }`. This catches accidental panel rewrites between dataset compile time and training time.

## 4. APIs

```rust
// Current trajectory/dataset implementation lives in the CLI/storage path.
// The Phase 7 service crate consumes the resulting DatasetShardRecord for training.
pub fn compile_trajectories(...) -> Result<Vec<TrajectoryId>, DynamicJepaError>;
pub fn compile_dataset_shard(...) -> Result<DatasetShardId, DynamicJepaError>;
pub fn inspect_dataset_row(...) -> Result<DatasetRowSnapshot, DynamicJepaError>;

// Storage layer
pub fn put_trajectory_batch(...);
pub fn get_trajectory(db, id) -> Result<Option<TrajectoryRecord>>;
pub fn list_trajectories_for_domain(...);
pub fn put_dataset_shard(...);
pub fn put_dataset_shard_with_audit_batch(...);
pub fn get_dataset_shard(db, dataset_id, shard_id) -> Result<Option<DatasetShardRecord>>;
pub fn count_dataset_shards(db) -> Result<u64>;
```

CLI:

```
dynamicjepa compile-trajectories --domain ID --policy by_domain_session
dynamicjepa get-trajectory --id UUID --json
dynamicjepa list-trajectories --domain ID --json
dynamicjepa compile-dataset --domain ID --policy one_step --split train
dynamicjepa get-dataset-shard --dataset-id UUID --shard-id UUID --json
dynamicjepa inspect-dataset --dataset-id UUID --shard-id UUID --row 0 --json
```

`inspect-dataset` decodes one row and prints:

```json
{
  "decoded_records": {
    "dataset_shard": {"row_count": 4977, "split_name": "train"},
    "row": {
      "row_index": 0,
      "input_panel_id": "<uuid>",
      "input_panel_dense_norm": 1.0,
      "input_state_fields": {"x": {"I64": 2}, "y": {"I64": 3}},
      "input_action_fields": {"move": {"String": "down"}},
      "target_panel_id": "<uuid>",
      "target_state_fields": {"x": {"I64": 2}, "y": {"I64": 4}},
      "negative_panel_id": "<uuid>",
      "negative_state_fields": {"x": {"I64": 0}, "y": {"I64": 0}},
      "source_hash_checks": {"input_equal": true, "target_equal": true}
    }
  }
}
```

## 5. FSV Expectations (Both Domains)

Counter_world:

- 1 trajectory; ordered transitions `[t0, t1, t2]`.
- 1 dataset shard, split=`train`, row_count=1 (`t0→t1`).
- 1 dataset shard, split=`val`, row_count=1 (`t1→t2`).
- `test` split has 0 rows; the harness logs this fact.

Gridworld:

- 1000 trajectories; each with 8 panels, 7 transitions.
- 3 dataset shards (train/val/test) covering 7000 rows total. The checked-in corpus currently splits to train=4977, val=1330, test=693.

`crates/context-graph-cli/tests/dynamicjepa_phase6_fsv.rs` asserts these through the real CLI, real RocksDB, and decoded source-of-truth reads.

## 6. Edge Cases

- **Insufficient trajectories for split** (e.g. demo run a one-trajectory subset of gridworld): compiler fails with `Validation { message: "split bucket train received 0 trajectories" }`. No shard row written.
- **Negative pool empty** (e.g. all panels share the same target): `DatasetLeakageDetected`. No shard row.
- **Source hash drift** (panel rewritten between trajectory compile and dataset compile): `DatasetLeakageDetected { message: "shard source hashes diverged from live panels" }`. The remediation hint says: "wipe `/tmp/<dbpath>` and re-run pipeline from `register-domain`."
