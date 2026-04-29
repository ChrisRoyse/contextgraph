# 14 — Full State Verification Protocol

This file is the playbook the demo's verification harness follows. It is the **authoritative source** for every count delta, every decoded-record assertion, and every edge case the demo has to exercise. If a test or harness disagrees with this file, fix the test or harness; do not change this file unless the design itself changes.

## 1. Source Of Truth

Primary:

- the RocksDB database opened at `--db <path>`;
- the 25 DynamicJEPA CFs;
- decoded records read through `dynamicjepa get-*` and `dynamicjepa inspect-cf`.

Secondary:

- artifact files under `--artifact-root`;
- file hashes recomputed during verification.

Both are read independently of the operations that wrote them. Return values are not evidence.

## 2. Required Fixtures

```
configs/dynamicjepa/domain_packs/counter_world.v1.toml
configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml
configs/dynamicjepa/verification/counter_world_happy_path.jsonl
configs/dynamicjepa/verification/counter_world_edge_empty_input.jsonl
configs/dynamicjepa/verification/counter_world_edge_invalid_action.jsonl
configs/dynamicjepa/verification/counter_world_edge_max_panel.jsonl
configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl
configs/dynamicjepa/verification/gridworld_5x5_edge_empty.jsonl
configs/dynamicjepa/verification/gridworld_5x5_edge_invalid_action.jsonl
configs/dynamicjepa/verification/gridworld_5x5_edge_oob_state.jsonl
configs/dynamicjepa/verification/tiny_train_config.json
configs/dynamicjepa/verification/gridworld_5x5_train_config.json
```

Every fixture file has a known SHA-256 listed in `tests/fixtures.rs`. If a fixture's hash does not match, the test fails with `Validation { field: "fixture_hash" }` — protects against accidental edits.

## 3. Happy-Path Verification (counter_world)

Setup:

```bash
export DJ_DB=/tmp/5090jepa-cw-fsv
export DJ_ARTIFACTS=/tmp/5090jepa_artifacts
rm -rf "$DJ_DB" "$DJ_ARTIFACTS"
mkdir -p "$DJ_ARTIFACTS"
```

Required command sequence (each line is one `dynamicjepa` subcommand; harness records the JSON output of each):

```bash
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa register-domain --db "$DJ_DB" --file configs/dynamicjepa/domain_packs/counter_world.v1.toml --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa ingest-event --db "$DJ_DB" --domain counter_world --adapter json_event --file configs/dynamicjepa/verification/counter_world_happy_path.jsonl --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa materialize-panel --db "$DJ_DB" --all-pending --domain counter_world --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa compile-trajectories --db "$DJ_DB" --domain counter_world --policy by_domain_session --json
context-graph dynamicjepa compile-dataset --db "$DJ_DB" --domain counter_world --policy one_step --split train --json
context-graph dynamicjepa compile-dataset --db "$DJ_DB" --domain counter_world --policy one_step --split val --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa train --db "$DJ_DB" --dataset-id $TRAIN_SHARD --config configs/dynamicjepa/verification/tiny_train_config.json --artifact-root "$DJ_ARTIFACTS" --json
context-graph dynamicjepa get-training-run --db "$DJ_DB" --id $TRAINING_RUN --json
context-graph dynamicjepa get-artifact --db "$DJ_DB" --id $ARTIFACT --verify-files --json
context-graph dynamicjepa predict --db "$DJ_DB" --artifact-id $ARTIFACT --panel-id $PANEL_T0 --action-id $ACTION_T0 --json
context-graph dynamicjepa plan --db "$DJ_DB" --artifact-id $ARTIFACT --panel-id $PANEL_T0 --skill-id $COUNTER_INCREMENT_SKILL --json
context-graph dynamicjepa get-plan-trace --db "$DJ_DB" --id $PLAN_TRACE --include-predictions --include-guards --json
context-graph dynamicjepa record-surprise --db "$DJ_DB" --prediction-id $PREDICTION --observed-outcome-id $OUTCOME_T1 --observed-panel-id $PANEL_T1 --json
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
```

Expected final counts (for the three-event happy path; values are **exact** unless noted):

| CF | Final count |
| --- | --- |
| `dj_domain_packs` | 1 |
| `dj_domain_pack_by_nv` | 1 |
| `dj_instrument_registry` | 2 |
| `dj_adapter_registry` | 1 |
| `dj_raw_events` | 3 |
| `dj_adapter_runs` | 3 (all `Completed`) |
| `dj_normalized_states` | 3 |
| `dj_actions` | 3 observed + 5 hypothetical = 8 |
| `dj_outcomes` | 3 |
| `dj_transitions` | 3 |
| `dj_instrument_readings` | 6 (3 events × 2 required instruments) |
| `dj_latent_panels` | 3 |
| `dj_bindings` | 0 (counter_world has no entities; demo skips binding step) |
| `dj_bindings_by_entity` | 0 |
| `dj_trajectories` | 1 |
| `dj_dataset_shards` | 2 (train + val) |
| `dj_training_runs` | 1 (`Completed`) |
| `dj_model_artifacts` | 1 (`Active`) |
| `dj_predictions` | 1 (manual `predict`) + 5 (from `plan`) = 6 |
| `dj_skill_policies` | 1 (counter_increment_skill, written during `plan` if not already present) |
| `dj_plan_traces` | 1 |
| `dj_guard_decisions` | 5 (one per candidate) |
| `dj_surprise_events` | 0 if cosine ≥ 0.85 else 1 |
| `dj_verification_runs` | 1 (`Passed`) |
| `dj_audit_log` | ≥ 30 (every operation) |

Required physical readbacks (the harness asserts each):

```bash
context-graph dynamicjepa get-domain --db "$DJ_DB" --id counter_world --json
context-graph dynamicjepa get-panel --db "$DJ_DB" --panel-id $PANEL_T0 --include-readings --json
context-graph dynamicjepa get-trajectory --db "$DJ_DB" --id $TRAJECTORY --json
context-graph dynamicjepa get-dataset-shard --db "$DJ_DB" --dataset-id $DATASET --shard-id $TRAIN_SHARD --json
```

The harness asserts:

- `panel.ordered_slots.len() == 2` (counter scalar + delta scalar).
- `trajectory.ordered_transition_ids.len() == 3`.
- `train_shard.row_count == 1`, `val_shard.row_count == 1`.
- `training_run.status == Completed` and `len(training_run.artifact_ids) == 1`.
- `artifact.files` recomputed hashes match registry (every file).
- `prediction.model_artifact_hash_at_inference == artifact.files["model.safetensors"].sha256`.
- `plan_trace.candidate_action_ids.len() == 5` and `plan_trace.selected_action_id` is set and references one of the candidates.

## 4. Edge Cases (counter_world)

For each edge case below the harness:

1. Records `before_counts`.
2. Runs the failing command (expects non-zero exit, captures JSON error body).
3. Records `after_counts`.
4. Confirms the deltas in the table match exactly.
5. Decodes the relevant audit/error rows for evidence.

### EC1: Empty Input

Fixture: `counter_world_edge_empty_input.jsonl` (zero bytes).

| CF | Δ |
| --- | --- |
| `dj_raw_events` | 0 |
| `dj_adapter_runs` | 0 |
| every other CF | 0 |
| `dj_audit_log` | +1 (failed ingest) |

Error body:

```json
{
  "error_code": "ADAPTER_FAILED",
  "error_message": "empty input file",
  "field_path": "",
  "wrote_raw_event": false,
  "wrote_adapter_run_failed": false
}
```

### EC2: Invalid Action

Fixture: `counter_world_edge_invalid_action.jsonl` (`action.delta = "two"`).

| CF | Δ |
| --- | --- |
| `dj_raw_events` | +1 |
| `dj_adapter_runs` | +1 (`Failed`, `field_path = action.delta`, `expected = i64`, `actual = string`) |
| `dj_normalized_*` / `dj_transitions` | 0 |
| `dj_latent_panels` / `dj_instrument_readings` | 0 |
| `dj_audit_log` | +2 (raw event ingest + failed adapter run) |

### EC3: Max Panel (Out-Of-Bounds Outcome)

Fixture: `counter_world_edge_max_panel.jsonl` (`outcome.next_counter = 1032 > max 1024`).

| CF | Δ |
| --- | --- |
| `dj_raw_events` | +1 |
| `dj_adapter_runs` | +1 (`Failed`, `field_path = outcome.next_counter`, `value = 1032`, `max = 1024`) |
| `dj_normalized_*` / `dj_transitions` | 0 |
| `dj_audit_log` | +2 |

## 5. Happy-Path Verification (gridworld_5x5)

Setup:

```bash
export DJ_DB=/tmp/5090jepa-gw-fsv
export DJ_ARTIFACTS=/tmp/5090jepa_artifacts/gw
rm -rf "$DJ_DB" "$DJ_ARTIFACTS"
mkdir -p "$DJ_ARTIFACTS"
```

Run the same command sequence as Section 3, substituting `gridworld_5x5` and the gridworld fixtures. Differences:

- `ingest-event` consumes 8000 lines.
- `materialize-panel --all-pending` produces 8000 panels.
- `compile-trajectories` produces 1000 trajectories.
- `compile-dataset` is run three times (train/val/test).

Expected final counts (gridworld); ranges where the deterministic split bucket boundary creates ±50 jitter:

| CF | Final count |
| --- | --- |
| `dj_domain_packs` | 1 |
| `dj_domain_pack_by_nv` | 1 |
| `dj_instrument_registry` | 3 |
| `dj_adapter_registry` | 1 |
| `dj_raw_events` | 8000 |
| `dj_adapter_runs` | 8000 (`Completed`) |
| `dj_normalized_states` | 8000 |
| `dj_actions` | 8000 observed + 5 hypothetical = 8005 |
| `dj_outcomes` | 8000 |
| `dj_transitions` | 8000 |
| `dj_instrument_readings` | 24000 (8000 × 3) |
| `dj_latent_panels` | 8000 |
| `dj_bindings` | 0 (demo skips bindings unless wired in) |
| `dj_trajectories` | 1000 |
| `dj_dataset_shards` | 3 (train/val/test) |
| `dj_training_runs` | 1 (`Completed`) |
| `dj_model_artifacts` | 1 (`Active`) |
| `dj_predictions` | 1 + 5 = 6 |
| `dj_skill_policies` | 1 |
| `dj_plan_traces` | 1 |
| `dj_guard_decisions` | 5 |
| `dj_surprise_events` | 0 happy path |
| `dj_verification_runs` | 1 (`Passed`) |

The harness asserts dataset row counts within the declared ranges (`expected_train_rows_min..max`, etc.).

## 6. Edge Cases (gridworld_5x5)

EC1 empty input: same shape as counter_world EC1.

EC2 invalid action (`move = "diagonal"`): same shape as counter_world EC2 with `field_path = action.move`, `expected = categorical{up,down,left,right,noop}`, `actual = string`.

EC3 OOB state (`state.x = 7`): raw event +1, failed adapter run +1, no normalized state, `field_path = state.x`, `value = 7`, `max = 4`.

## 7. Convergence Audits (Both Packs)

After training:

```bash
context-graph dynamicjepa get-artifact --db "$DJ_DB" --id $ARTIFACT --verify-files --json | jq '.evaluation_report'
```

Asserts (decoded JSON, not return values):

```text
counter_world: val_latent_cosine ≥ 0.99 ∧ val_latent_mse ≤ 0.01 ∧ action_contrast_acc ≥ 0.99
gridworld:     val_latent_cosine ≥ 0.95 ∧ val_latent_mse ≤ 0.05 ∧ test_latent_cosine ≥ 0.92 ∧ action_contrast_acc ≥ 0.95
both:          shuffled_target_cosine ≤ 0.30 (cw 0.10) ∧ random_init_cosine ≤ 0.30 (cw 0.10) ∧ vicreg_variance_per_dim_min ≥ 0.5
```

## 8. Artifact Hash Audit (Both Packs)

```bash
ARTIFACT_JSON=$(context-graph dynamicjepa get-artifact --db "$DJ_DB" --id $ARTIFACT --verify-files --json)
echo "$ARTIFACT_JSON" | jq '.artifact_hashes'
```

Every entry must have `equal: true`. The harness then **mutates** one byte of `model.safetensors`:

```bash
printf '\xff' | dd of="$DJ_ARTIFACTS/<artifact dir>/model.safetensors" bs=1 count=1 conv=notrunc seek=128
context-graph dynamicjepa predict --db "$DJ_DB" --artifact-id $ARTIFACT --panel-id $PANEL_T0 --action-id $ACTION_T0 --json
```

Expected response:

```json
{
  "operation": "dynamicjepa_predict",
  "status": "error",
  "error_code": "ARTIFACT_HASH_MISMATCH",
  "file": "model.safetensors",
  "expected": "<hex>",
  "actual": "<different hex>",
  "wrote_prediction": false
}
```

The `dj_predictions` count after this must equal the count before. If a prediction row was written, the no-fallback rule has been violated and the demo fails verification.

## 9. Verification Run Record

After every harness run, `tmp/5090jepa_evidence/<domain>/<verification_run_uuid>/` must contain:

```
verification_run_record.json     # the persisted VerificationRunRecord
command_log.txt                  # exact CLI commands executed, in order
before_counts.json
after_counts.json
decoded/                         # one JSON per important record
edge_*.json                      # one per edge case with before/after counts + error body
artifact_files_recomputed_hashes.json
```

The `VerificationRunRecord.status` is `Passed` only if every count assertion, every readback, every convergence target, and every edge-case error body matched the spec. Any mismatch sets status to `Failed { failure_details: ..., evidence_path: ... }` and writes the same evidence directory.

## 10. Operator Sign-Off Checklist (Per Run)

Paste this block into the closing PR description for each domain:

```text
[ ] inspect-counts before run = all zeros
[ ] register-domain success: source_hash matches TOML
[ ] ingest-event success: <N> raw events written
[ ] materialize-panel success: <M> panels written
[ ] compile-trajectories success: <K> trajectories
[ ] compile-dataset success: train/val/test row counts in spec
[ ] train success: metrics meet convergence targets
[ ] artifact recomputed hashes equal registry
[ ] predict success: prediction row present, hash-at-inference matches artifact
[ ] plan success: plan trace present, candidate count = 5, selected action set
[ ] record-surprise success: surprise row written iff cosine < threshold
[ ] EC1 empty input: counts unchanged, error code ADAPTER_FAILED
[ ] EC2 invalid action: raw event +1, failed adapter run +1, no normalized
[ ] EC3 OOB state/panel: raw event +1, failed adapter run +1, no normalized
[ ] artifact hash mismatch test: prediction call refused, no row written
[ ] verification_run_record.json status=Passed
```

If a box is unchecked, the run is not done.
