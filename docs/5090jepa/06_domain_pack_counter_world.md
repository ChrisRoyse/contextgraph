# 06 — Domain Pack: counter_world.v1

`counter_world` is the Occam's‑razor proof that the entire DynamicJEPA pipeline plumbs end‑to‑end. Three integer transitions, one scalar instrument, deterministic arithmetic, ground truth always equal to `state + action`. A correctly trained tiny JEPA reaches >0.99 cosine on the held‑out transition; a randomly initialized one cannot.

Current Phase 10 status as of 2026-04-30: the checked-in TOML and three-event happy-path JSONL register, ingest, adapt, instrument, materialize into three panels, compile into one ordered trajectory, compile one-step dataset shards, and support explicit binding through the real CLI and real RocksDB. Phase 7/8/10 FSV uses a generated 60-event counter_world fixture, not the three-event plumbing fixture, so train-only convergence and prediction/planning/surprise can be verified without fitting on validation rows. Bundled `verify-counter-world` is implemented and persists a `Passed` `dj_verification_runs` row.

## 1. File Locations

```
configs/dynamicjepa/domain_packs/counter_world.v1.toml
configs/dynamicjepa/verification/counter_world_happy_path.jsonl
configs/dynamicjepa/verification/counter_world_edge_empty_input.jsonl
configs/dynamicjepa/verification/counter_world_edge_invalid_action.jsonl
configs/dynamicjepa/verification/counter_world_edge_max_panel.jsonl
configs/dynamicjepa/verification/tiny_train_config.json
```

These files are operator inputs. They are SHA‑256 hashed at registration; the hash is stored in the `DomainPack.source_hash` field. Re‑editing a TOML *requires* re‑registering with a bumped `version` (e.g. `1.0.1`); the registry cannot silently overwrite.

## 2. Domain Pack TOML

```toml
# configs/dynamicjepa/domain_packs/counter_world.v1.toml
[domain]
id = "counter_world"
version = "1.0.0"
title = "Counter World — Deterministic Arithmetic Verification Domain"
schema_version = 1

[time]
field = { name = "ts_ms", kind = "unix_ms", required = true }

[[state.fields]]
name = "counter"
kind = { type = "i64" }
required = true
min = -1024
max = 1024

[[action.fields]]
name = "delta"
kind = { type = "i64" }
required = true
min = -8
max = 8

[[outcome.fields]]
name = "next_counter"
kind = { type = "i64" }
required = true
min = -1024
max = 1024

# entity schema is empty for counter_world (no entities)
[entity]
fields = []

# Two instruments: scalar over the counter; scalar over the delta.
[[instruments]]
id = "counter_scalar"
kind = { type = "scalar" }
input_fields = ["state.counter"]
output_shape = [1]
normalization = { type = "standard_score", mean = 0.0, std = 16.0 }
required = true
version = 1
[instruments.validation]
require_finite = true
reject_nan = true
reject_inf = true
min = -64.0
max = 64.0

[[instruments]]
id = "delta_scalar"
kind = { type = "scalar" }
input_fields = ["action.delta"]
output_shape = [1]
normalization = { type = "standard_score", mean = 0.0, std = 4.0 }
required = true
version = 1
[instruments.validation]
require_finite = true
reject_nan = true
reject_inf = true
min = -16.0
max = 16.0

# One adapter: declarative JSON path mapping.
[[adapters]]
id = "json_event"
kind = "json_event"
version = 1
[adapters.mapping]
"state.counter" = "$.state.counter"
"action.delta" = "$.action.delta"
"outcome.next_counter" = "$.outcome.next_counter"
"time.ts_ms" = "$.ts"

# One objective: latent_delta one-step prediction.
[[objectives]]
id = "predict_next_counter"
kind = "latent_delta"
input_panel = "state_action"
target = "outcome.next_counter"
loss_weight = 1.0
required_dataset_fields = ["target_panel_id", "input_panel_id", "action_id"]

# Invariant: next_counter must equal state.counter + action.delta. Used by
# guards at planning time; not a training signal.
[[invariants]]
id = "arithmetic_consistency"
expression = "outcome.next_counter == state.counter + action.delta"
severity = "fatal"

[dataset_policy]
default_split = "by_time"
default_objective = "predict_next_counter"
negative_sampling = "same_domain_different_outcome"
min_negatives_per_row = 1

[planner_policy]
candidate_actions = { kind = "enumerated", deltas = [-2, -1, 0, 1, 2] }
guards = ["bounds_check"]
surprise_threshold = 0.85   # cosine

[verification_policy]
required_cfs = [
  "dj_domain_packs", "dj_raw_events", "dj_adapter_runs",
  "dj_normalized_states", "dj_actions", "dj_outcomes", "dj_transitions",
  "dj_instrument_readings", "dj_latent_panels",
  "dj_trajectories", "dj_dataset_shards",
  "dj_training_runs", "dj_model_artifacts",
  "dj_predictions", "dj_plan_traces", "dj_guard_decisions",
  "dj_verification_runs", "dj_audit_log"
]
expected_event_count = 3
expected_panel_count = 3
expected_dataset_row_count = 2
```

Validation rejects any unknown TOML key (strict mode). Comments are allowed. Environment variable substitution is forbidden — registration fails on any `${...}` substring.

## 3. Happy‑Path Fixture (3 events)

```jsonl
# configs/dynamicjepa/verification/counter_world_happy_path.jsonl
{"state":{"counter":0},"action":{"delta":1},"outcome":{"next_counter":1},"ts":1700000000000}
{"state":{"counter":1},"action":{"delta":2},"outcome":{"next_counter":3},"ts":1700000001000}
{"state":{"counter":3},"action":{"delta":1},"outcome":{"next_counter":4},"ts":1700000002000}
```

Expected RocksDB writes after `dynamicjepa ingest-event --file counter_world_happy_path.jsonl --domain counter_world --adapter json_event`:

| CF | Δrows |
| --- | --- |
| `dj_raw_events` | +3 |
| `dj_adapter_runs` | +3 (all `completed`) |
| `dj_normalized_states` | +3 |
| `dj_actions` | +3 |
| `dj_outcomes` | +3 |
| `dj_transitions` | +3 |
| `dj_audit_log` | +6 (3 ingest + 3 adapter complete) |

After `dynamicjepa materialize-panel --all-pending`:

| CF | Δrows |
| --- | --- |
| `dj_instrument_readings` | +6 (3 events × 2 required instruments) |
| `dj_latent_panels` | +3 |
| `dj_audit_log` | +3 |

Phase 4 FSV independently confirmed the happy path in `/tmp/5090jepa-phase4-manual-cw`: after registration, ingest, and panel materialization the source-of-truth counts are `dj_domain_packs=1`, `dj_domain_pack_by_nv=1`, `dj_instrument_registry=2`, `dj_adapter_registry=1`, `dj_raw_events=3`, `dj_adapter_runs=3`, `dj_normalized_states=3`, `dj_actions=3`, `dj_outcomes=3`, `dj_transitions=3`, `dj_instrument_readings=6`, `dj_latent_panels=3`, and `dj_audit_log=10`.

After `dynamicjepa compile-trajectories --domain counter_world --policy by_domain_session`:

| CF | Δrows |
| --- | --- |
| `dj_trajectories` | +1 |

After `dynamicjepa compile-dataset --domain counter_world --policy one_step --split train` on the checked-in three-event fixture:

| CF | Δrows |
| --- | --- |
| `dj_dataset_shards` | +1 (`row_count=1`, panel pair `t0→t1`) |

After `dynamicjepa train --dataset-id <id> --config tiny_train_config.json --artifact-root /tmp/5090jepa_artifacts` on the current 60-event Phase 7 fixture:

| CF | Δrows |
| --- | --- |
| `dj_training_runs` | +1 (`completed`) |
| `dj_model_artifacts` | +1 |

Current Phase 8 FSV for `dynamicjepa predict --artifact-id <a> --panel-id <p1> --action-id <a1>`:

| CF | Δrows |
| --- | --- |
| `dj_predictions` | +1 |

Current Phase 8 FSV for `dynamicjepa plan --artifact-id <a> --panel-id <p1> --skill-id <skill>`:

| CF | Δrows |
| --- | --- |
| `dj_actions` | +5 (hypothetical, one per declared candidate delta) |
| `dj_predictions` | +5 |
| `dj_guard_decisions` | +5 |
| `dj_plan_traces` | +1 |

Phase 10 `dynamicjepa verify-counter-world` (the bundled FSV harness, runs everything above plus all edge cases):

| CF | Δrows |
| --- | --- |
| `dj_verification_runs` | +1 (`Passed`) |
| `dj_audit_log` | +many (all of the above) |

## 4. Edge‑Case Fixtures

### 4.1 `counter_world_edge_empty_input.jsonl`

```
```

(Zero‑byte file.) Expected: `dynamicjepa ingest-event` fails with `DynamicJepaError::AdapterFailed { adapter_id: "json_event", message: "empty input file", ... }`. `dj_raw_events`, `dj_adapter_runs`, all normalized CFs, readings, and panels remain unchanged; `dj_audit_log` increases by exactly 1 with a failed ingest audit row.

### 4.2 `counter_world_edge_invalid_action.jsonl`

```jsonl
{"state":{"counter":2},"action":{"delta":"two"},"outcome":{"next_counter":4},"ts":1700000003000}
```

Expected: `dj_raw_events` +1 (payload captured first), `dj_adapter_runs` +1 with `status = Failed`, `field_path = "action.delta"`, `expected_kind = "i64"`, `actual_kind = "string"`. **No** `dj_normalized_*` rows. `dj_audit_log` records the ingest + the failed adapter run.

### 4.3 `counter_world_edge_max_panel.jsonl`

```jsonl
{"state":{"counter":1024},"action":{"delta":8},"outcome":{"next_counter":1032},"ts":1700000004000}
```

`outcome.next_counter = 1032` violates the declared `max = 1024`. Expected: `dj_raw_events` +1, `dj_adapter_runs` +1 with `status = Failed`, error includes `field_path = "outcome.next_counter"`, `value = 1032`, `max = 1024`. **No** `dj_normalized_outcomes` row, no `dj_transitions` row, no panel.

Phase 4 edge-case FSV confirmed the final source-of-truth counts after running all three edge cases on top of the happy path: `dj_raw_events=5`, `dj_adapter_runs=5`, `dj_normalized_states=3`, `dj_actions=3`, `dj_outcomes=3`, `dj_transitions=3`, `dj_instrument_readings=6`, `dj_latent_panels=3`, and `dj_audit_log=15`. Evidence lives under `tmp/5090jepa_evidence/phase4/manual_counter/`.

## 5. Tiny Train Config

```json
// configs/dynamicjepa/verification/tiny_train_config.json
{
  "model": {
    "encoder": { "kind": "mlp", "hidden": [16], "out_dim": 8 },
    "predictor": { "kind": "mlp", "hidden": [16], "in_action_dim": 1, "out_dim": 8 },
    "ema_momentum": 0.99
  },
  "loss": {
    "latent_mse_weight": 1.0,
    "vicreg_variance_weight": 1.0,
    "vicreg_covariance_weight": 0.04,
    "vicreg_target_std": 1.0
  },
  "optim": {
    "kind": "adamw",
    "lr": 1e-3,
    "weight_decay": 1e-4,
    "warmup_steps": 0
  },
  "schedule": {
    "epochs": 200,
    "batch_size": 2,
    "device": "cuda"
  },
  "stopping": {
    "metric": "val_latent_mse",
    "target": 0.005,
    "max_seconds": 30
  },
  "seed": 42
}
```

Runs on the 5090 in well under 30 seconds for the current generated 60-event Phase 7 fixture. The original three-event fixture remains useful for plumbing, but it is not enough to prove train-only convergence. If training time exceeds 30 s, halt with `TrainingFailed { remediation: "check device=cuda; nvidia-smi shows GPU utilization >0% during training" }`.

## 6. Convergence Asserts

`crates/context-graph-cli/tests/dynamicjepa_phase7_fsv.rs` currently asserts after training:

- `metrics.val_latent_cosine >= 0.99`
- `metrics.val_latent_mse <= 0.005`
- `metrics.parameter_count_trainable == 480`
- `metrics.vicreg_variance_per_dim_min >= 0.5`

The same report records `action_contrast_acc`, `shuffled_target_cosine`, `random_init_cosine`, and covariance. The strict shuffled-target and covariance gates are not yet passing and remain open; do not remove those metrics or lower the future acceptance bar to hide the issue.

## 7. Verification Run Evidence

Current Phase 7 evidence is written by the CLI FSV test to `tmp/5090jepa_evidence/phase7/cli_phase7_fsv.json` plus runtime DB/artifact files under `tmp/5090jepa_evidence/phase7/cli_phase7_fsv_runtime/`.

Phase 10, after `dynamicjepa verify-counter-world --db /tmp/5090jepa-cw-fsv --json`, the harness writes:

```
tmp/5090jepa_evidence/counter_world/<verification_run_uuid>/
  command_log.txt
  before_counts.json
  after_counts.json
  decoded/
    domain_pack.json
    raw_event_t0.json
    adapter_run_t0_completed.json
    transition_t0.json
    panel_t0.json
    trajectory.json
    dataset_shard.json
    training_run.json
    artifact.json
    artifact_files_recomputed_hashes.json
    prediction.json
    plan_trace.json
    edge_empty_input/        # subdirectory per edge case
      before_counts.json
      after_counts.json
      adapter_error.json
    edge_invalid_action/...
    edge_max_panel/...
  verification_run_record.json
```

This evidence package is the operator's proof. Every JSON file is decoded from RocksDB *after* the operation, never the operation's return value.

Manual Phase 10 counter_world FSV on 2026-04-30 used `/tmp/5090jepa-phase10-cw-fsv2` and evidence root `tmp/5090jepa_evidence/counter_world_final/4d2be585-cd38-4da7-9b57-d33dd611cc03/`. The final independent count read found `dj_raw_events=62`, `dj_adapter_runs=62`, `dj_latent_panels=60`, `dj_dataset_shards=3`, `dj_training_runs=1`, `dj_model_artifacts=1`, `dj_predictions=6`, `dj_plan_traces=1`, `dj_surprise_events=1`, `dj_verification_runs=1`, and `dj_audit_log=196`.
