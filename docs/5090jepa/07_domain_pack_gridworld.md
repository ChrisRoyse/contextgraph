# 07 — Domain Pack: gridworld_5x5.v1

`gridworld_5x5` proves universality. The same pipeline that solves counter_world without code changes also handles a deterministic discrete-navigation domain whose state is a 2-D coordinate, action is one of five discrete moves, and outcome is the new coordinate after wall-clipping. A randomly initialized predictor cannot solve gridworld; a correctly trained tiny JEPA reaches ≥0.95 latent cosine on the held-out transitions.

Current Phase 10 status as of 2026-04-30: the checked-in TOML and full 8000-event corpus register, ingest, adapt, instrument, materialize into panels, compile into 1000 trajectories and train/val/test dataset shards, train on CUDA, write a real artifact, predict, plan, and persist a `Passed` `VerificationRunRecord` through `verify-gridworld`. The same service code is used for counter_world and gridworld; the Phase 10 fix removed future-action leakage from target panel encodings rather than adding a domain-specific branch.

## 1. World Definition

- Grid: 5 × 5 cells, x ∈ {0..4}, y ∈ {0..4}.
- Actions: `up`, `down`, `left`, `right`, `noop`.
- Transition: `(x, y) → (x', y')` where `x'/y' = clamp(0..4, ...)`. Walls clip; the agent does not wrap.
- Reward / outcome: not modeled in v1 — the JEPA target is the next *latent panel*, not a scalar reward.
- Trajectory length: exactly 8 steps. The starting cell is uniform random over the 25 cells; the action sequence is uniform random over the 5 actions.
- Corpus size: 1000 trajectories = 8000 events.
- Splits: 700 trajectories train / 200 val / 100 test by `by_domain_key_hash` over `trajectory_id`.

## 2. File Locations

```
configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml
configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl       # 8000 lines, generated
configs/dynamicjepa/verification/gridworld_5x5_edge_empty.jsonl
configs/dynamicjepa/verification/gridworld_5x5_edge_invalid_action.jsonl
configs/dynamicjepa/verification/gridworld_5x5_edge_oob_state.jsonl
configs/dynamicjepa/verification/gridworld_5x5_train_config.json
scripts/5090jepa/generate_gridworld_corpus.py                     # deterministic generator
```

The generator writes the corpus deterministically with a hard-coded seed. Re-running it produces byte-identical output. The corpus is **committed** to the repo (8000 lines, ~600 KB) so demo runs do not depend on Python being available; the generator script is a reproducibility artifact.

## 3. Domain Pack TOML

```toml
# configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml
[domain]
id = "gridworld_5x5"
version = "1.0.0"
title = "Gridworld 5x5 — Deterministic Discrete Navigation"
schema_version = 1

[time]
field = { name = "step_index", kind = "i64", required = true }   # monotonic per trajectory; not unix_ms here

[[state.fields]]
name = "x"
kind = { type = "i64" }
required = true
min = 0
max = 4

[[state.fields]]
name = "y"
kind = { type = "i64" }
required = true
min = 0
max = 4

[[action.fields]]
name = "move"
kind = { type = "categorical", variants = ["up", "down", "left", "right", "noop"] }
required = true

[[outcome.fields]]
name = "next_x"
kind = { type = "i64" }
required = true
min = 0
max = 4

[[outcome.fields]]
name = "next_y"
kind = { type = "i64" }
required = true
min = 0
max = 4

[[entity.fields]]
name = "trajectory_id"
kind = { type = "string" }
required = true

# Instruments: 25-d onehot for (x, y), 5-d onehot for action, scalar step index.
# Onehot rather than scalar because the world is discrete and we want the JEPA
# to learn the latent map, not memorize integer arithmetic.
[[instruments]]
id = "state_xy_onehot"
kind = { type = "onehot", dim = 25 }
input_fields = ["state.x", "state.y"]
output_shape = [25]
normalization = { type = "none" }
required = true
version = 1
[instruments.validation]
require_finite = true
reject_nan = true
reject_inf = true

[[instruments]]
id = "action_move_onehot"
kind = { type = "categorical", variants = ["up", "down", "left", "right", "noop"] }
input_fields = ["action.move"]
output_shape = [5]
normalization = { type = "none" }
required = true
version = 1
[instruments.validation]
require_finite = true
reject_nan = true
reject_inf = true

[[instruments]]
id = "step_scalar"
kind = { type = "scalar" }
input_fields = ["time.step_index"]
output_shape = [1]
normalization = { type = "standard_score", mean = 3.5, std = 2.0 }
required = false                     # optional time slot; mask if absent
version = 1
[instruments.validation]
require_finite = true
reject_nan = true
reject_inf = true
min = 0.0
max = 7.0

[[adapters]]
id = "json_event"
kind = "json_event"
version = 1
[adapters.mapping]
"state.x" = "$.state.x"
"state.y" = "$.state.y"
"action.move" = "$.action.move"
"outcome.next_x" = "$.outcome.next_x"
"outcome.next_y" = "$.outcome.next_y"
"time.step_index" = "$.step"
"entity.trajectory_id" = "$.trajectory_id"

[[objectives]]
id = "predict_next_xy"
kind = "latent_delta"
input_panel = "state_action"
target = "outcome.next_xy_onehot"
loss_weight = 1.0
required_dataset_fields = ["target_panel_id", "input_panel_id", "action_id"]

[[invariants]]
id = "wall_clip_consistency"
expression = """
  (action.move == 'up'    -> outcome.next_y == max(state.y - 1, 0)) AND
  (action.move == 'down'  -> outcome.next_y == min(state.y + 1, 4)) AND
  (action.move == 'left'  -> outcome.next_x == max(state.x - 1, 0)) AND
  (action.move == 'right' -> outcome.next_x == min(state.x + 1, 4)) AND
  (action.move == 'noop'  -> outcome.next_x == state.x AND outcome.next_y == state.y)
"""
severity = "fatal"

[dataset_policy]
default_split = "by_domain_key_hash"
split_key = "entity.trajectory_id"
split_buckets = { train = 0.70, val = 0.20, test = 0.10 }
default_objective = "predict_next_xy"
negative_sampling = "same_domain_different_outcome"
min_negatives_per_row = 1

[planner_policy]
candidate_actions = { kind = "enumerated", moves = ["up", "down", "left", "right", "noop"] }
guards = ["bounds_check"]
surprise_threshold = 0.85

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
expected_trajectory_count = 1000
expected_event_count = 8000
expected_panel_count = 8000
# train/val/test row counts: each trajectory yields 7 one-step pairs ⇒ 7000 rows total
# train ≈ 0.70 * 7000 = 4900, val ≈ 1400, test ≈ 700 (plus or minus boundary).
expected_train_rows_min = 4800
expected_train_rows_max = 5000
expected_val_rows_min = 1300
expected_val_rows_max = 1500
expected_test_rows_min = 600
expected_test_rows_max = 800
```

## 4. Corpus Generator (deterministic Python)

`scripts/5090jepa/generate_gridworld_corpus.py`:

```python
import json
import hashlib
import random
import sys
from pathlib import Path

OUT_PATH = Path(sys.argv[1])  # configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl
SEED = 0x5090
N_TRAJ = 1000
TRAJ_LEN = 8
GRID = 5

random.seed(SEED)
ACTIONS = ["up", "down", "left", "right", "noop"]

def step(x, y, a):
    if a == "up":    return x, max(y - 1, 0)
    if a == "down":  return x, min(y + 1, GRID - 1)
    if a == "left":  return max(x - 1, 0), y
    if a == "right": return min(x + 1, GRID - 1), y
    if a == "noop":  return x, y
    raise ValueError(a)

with OUT_PATH.open("w", encoding="utf-8") as f:
    for t in range(N_TRAJ):
        traj_id = hashlib.blake2b(f"gridworld5x5:{t}".encode(), digest_size=8).hexdigest()
        x = random.randrange(GRID)
        y = random.randrange(GRID)
        for s in range(TRAJ_LEN):
            a = random.choice(ACTIONS)
            nx, ny = step(x, y, a)
            row = {
                "trajectory_id": traj_id,
                "step": s,
                "state": {"x": x, "y": y},
                "action": {"move": a},
                "outcome": {"next_x": nx, "next_y": ny},
            }
            f.write(json.dumps(row, separators=(",", ":")) + "\n")
            x, y = nx, ny

print(f"wrote {N_TRAJ * TRAJ_LEN} events to {OUT_PATH}")
```

Run once:

```bash
python3 scripts/5090jepa/generate_gridworld_corpus.py configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl
```

Generator writes byte-identical output across machines because the seed is fixed and the JSON serialization is canonical (no whitespace). A SHA-256 of the output file is recorded in the test suite (`tests/gridworld_fsv.rs`) as a tripwire: if anyone edits the generator and forgets to update the hash, the test fails loudly.

## 5. Edge-Case Fixtures

Phase 4 FSV independently confirmed the one-trajectory happy path in `/tmp/5090jepa-phase4-manual-gw`: after registration, ingest, and panel materialization the source-of-truth counts are `dj_domain_packs=1`, `dj_domain_pack_by_nv=1`, `dj_instrument_registry=3`, `dj_adapter_registry=1`, `dj_raw_events=8`, `dj_adapter_runs=8`, `dj_normalized_states=8`, `dj_actions=8`, `dj_outcomes=8`, `dj_transitions=8`, `dj_instrument_readings=24`, `dj_latent_panels=8`, and `dj_audit_log=25`.

### 5.1 `gridworld_5x5_edge_empty.jsonl`

Zero-byte file. Same expectation as counter_world: ingestion fails before raw/adapter/normalized writes, all data CFs stay unchanged, and `dj_audit_log` increases by exactly 1 with a failed ingest audit row.

### 5.2 `gridworld_5x5_edge_invalid_action.jsonl`

```jsonl
{"trajectory_id":"edge_invalid","step":0,"state":{"x":2,"y":2},"action":{"move":"diagonal"},"outcome":{"next_x":3,"next_y":3}}
```

Expected: `dj_raw_events +1`, `dj_adapter_runs +1` with `status = Failed`, `field_path = "action.move"`, `expected = "categorical{up,down,left,right,noop}"`, `actual = "diagonal"`. No normalized rows. No panel.

### 5.3 `gridworld_5x5_edge_oob_state.jsonl`

```jsonl
{"trajectory_id":"edge_oob","step":0,"state":{"x":7,"y":2},"action":{"move":"noop"},"outcome":{"next_x":7,"next_y":2}}
```

`state.x = 7` violates `max = 4`. Expected: same shape as 5.2 — raw event + failed adapter run; no normalized state.

Phase 4 edge-case FSV confirmed the final source-of-truth counts after running all three edge cases on top of the one-trajectory happy path: `dj_raw_events=10`, `dj_adapter_runs=10`, `dj_normalized_states=8`, `dj_actions=8`, `dj_outcomes=8`, `dj_transitions=8`, `dj_instrument_readings=24`, `dj_latent_panels=8`, and `dj_audit_log=30`. Evidence lives under `tmp/5090jepa_evidence/phase4/manual_gridworld/`.

## 6. Train Config

```json
// configs/dynamicjepa/verification/gridworld_5x5_train_config.json
{
  "model": {
    "encoder": { "kind": "mlp", "hidden": [64, 64], "out_dim": 32 },
    "predictor": { "kind": "mlp", "hidden": [64, 64], "in_action_dim": 5, "out_dim": 32 },
    "ema_momentum": 0.995
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
    "warmup_steps": 50
  },
  "schedule": {
    "epochs": 120,
    "batch_size": 256,
    "device": "cuda"
  },
  "stopping": {
    "metric": "val_latent_mse",
    "target": 0.04,
    "max_seconds": 240
  },
  "seed": 42
}
```

Total parameters: encoder ~6.5K + predictor ~7K ≈ 13.5K. Comfortably under the 1 GB VRAM ceiling. Training is expected to complete inside the 5 minute gridworld FSV budget on the 5090.

## 7. Convergence Asserts

Phase 10 gridworld model FSV asserts:

- `metrics.val_latent_cosine >= 0.95`
- `metrics.val_latent_mse <= 0.05`
- `metrics.action_contrast_acc >= 0.95`
- `metrics.shuffled_target_cosine <= 0.30`
- `metrics.random_init_cosine <= 0.30`
- `metrics.vicreg_variance_per_dim_min >= 0.5`
- `metrics.test_latent_cosine >= 0.92` (held-out test trajectories that the model never saw)

Manual Phase 10 gridworld FSV on 2026-04-30 recorded `val_latent_mse=0.03979081088279681`, `val_latent_cosine=0.9942557877646839`, `test_latent_mse=0.04028264938553979`, `action_contrast_acc=0.9969924812030075`, and `parameter_count_trainable=16960`.

## 8. Universality Audits

`tests/universality_audit.rs` proves the same code handles both packs by asserting:

- The set of `crates/context-graph-dynamicjepa/src/` changes needed for gridworld remains domain-neutral. Phase 10 changed action-slot encoding for all domains so target/negative panels use the candidate action supplied by the row, preventing future-action leakage without `if domain == ...` branching.
- `dynamicjepa register-domain` of both TOMLs against a single fresh DB produces the expected DOMAIN_PACK_BY_NV row count of 2.
- Phase 10: running `verify-counter-world` and `verify-gridworld` independently passes and persists verification rows. A same-DB sequential universality run remains a useful follow-up audit because the current manual proof used separate DB paths.

If any of those audits fails, the demo's universality claim is broken.
