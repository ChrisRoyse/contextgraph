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

## 2. Current Implemented FSV Scope (Phase 7/8/9/10)

Phase 7 verifies the full real source-of-truth path through domain-pack registration, JSONL event ingestion, adapter runs, normalized records, instrument readings, latent panels, trajectory rows, one-step dataset shards, CUDA-only training, training-run persistence, model-artifact registration, and artifact hash verification. Phase 8 extends the same real path through prediction, planning, guard decisions, and surprise recording. Phase 9 exposes the operational Phase 3-8 surface through MCP tool definitions, strict DTOs, handlers, dispatch entries, and capability matrix runtime state. Phase 10 adds bundled CLI verifiers for counter_world and gridworld that run the full loop, exercise three adapter edge cases per domain, persist `VerificationRunRecord`, and write evidence bundles. Current Phase 7/8/9/10 counter_world FSV tests use a generated 60-event counter_world fixture so the trainer has enough train rows to converge without fitting on validation data.

The current completed source-of-truth surface is:

```bash
export DJ_DB=tmp/5090jepa_evidence/phase7/cli_phase7_fsv_runtime/dynamicjepa_phase7_counter_rocksdb
export DJ_ARTIFACTS=tmp/5090jepa_evidence/phase7/cli_phase7_fsv_runtime/dynamicjepa_phase7_artifacts
rm -rf "$DJ_DB" "$DJ_ARTIFACTS"
context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
context-graph dynamicjepa register-domain --db "$DJ_DB" --file configs/dynamicjepa/domain_packs/counter_world.v1.toml --json
context-graph dynamicjepa ingest-event --db "$DJ_DB" --domain counter_world --adapter json_event --file <generated_60_event_counter_world.jsonl> --json
context-graph dynamicjepa materialize-panel --db "$DJ_DB" --all-pending --domain counter_world --json
context-graph dynamicjepa compile-trajectories --db "$DJ_DB" --domain counter_world --policy by_domain_session --json
context-graph dynamicjepa compile-dataset --db "$DJ_DB" --domain counter_world --policy one_step --split train --json
context-graph dynamicjepa compile-dataset --db "$DJ_DB" --domain counter_world --policy one_step --split val --json
context-graph dynamicjepa compile-dataset --db "$DJ_DB" --domain counter_world --policy one_step --split test --json
context-graph dynamicjepa get-dataset-shard --db "$DJ_DB" --dataset-id <dataset-id> --shard-id <shard-id> --json
context-graph dynamicjepa inspect-dataset --db "$DJ_DB" --dataset-id <dataset-id> --shard-id <shard-id> --row 0 --json
context-graph dynamicjepa train --db "$DJ_DB" --dataset-id <dataset-id> --config configs/dynamicjepa/verification/tiny_train_config.json --artifact-root "$DJ_ARTIFACTS" --json
context-graph dynamicjepa get-training-run --db "$DJ_DB" --id <training-run-id> --json
context-graph dynamicjepa get-artifact --db "$DJ_DB" --id <artifact-id> --verify-files --json
```

Expected current Phase 7 counter_world training/artifact counts after the happy path:

| CF | Final count |
| --- | --- |
| `dj_domain_packs` | 1 |
| `dj_domain_pack_by_nv` | 1 |
| `dj_instrument_registry` | 2 |
| `dj_adapter_registry` | 1 |
| `dj_raw_events` | 60 |
| `dj_adapter_runs` | 60 (`Completed`) |
| `dj_normalized_states` | 60 |
| `dj_actions` | 60 |
| `dj_outcomes` | 60 |
| `dj_transitions` | 60 |
| `dj_instrument_readings` | 120 |
| `dj_latent_panels` | 60 |
| `dj_trajectories` | 1 |
| `dj_dataset_shards` | 3 |
| `dj_training_runs` | 1 (`Completed`) |
| `dj_model_artifacts` | 1 (`Active`) |
| `dj_predictions` | 0 |
| `dj_plan_traces` | 0 |
| `dj_surprise_events` | 0 |
| `dj_verification_runs` | 0 |
| `dj_audit_log` | 187 |

Current evidence:

- Automated Phase 7 CLI evidence: `tmp/5090jepa_evidence/phase7/cli_phase7_fsv.json`.
- Runtime source-of-truth DB/artifact files: `tmp/5090jepa_evidence/phase7/cli_phase7_fsv_runtime/`.
- Current Phase 7 test: `crates/context-graph-cli/tests/dynamicjepa_phase7_fsv.rs`.
- Current Phase 8 evidence: `tmp/5090jepa_evidence/phase8/cli_phase8_fsv.json`.
- Current Phase 8 test: `crates/context-graph-cli/tests/dynamicjepa_phase8_fsv.rs`.
- Phase 8 final observed counts include `dj_actions=71`, `dj_predictions=11`, `dj_guard_decisions=10`, `dj_plan_traces=2`, `dj_skill_policies=1`, `dj_surprise_events=1`, `dj_training_runs=1`, `dj_model_artifacts=1`, and `dj_audit_log=194`.
- Phase 8 decoded proof includes `get-prediction`, `get-plan-trace --include-predictions --include-guards`, `get-surprise`, and direct `ldb dump --count_only` counts against `dj_actions`, `dj_predictions`, `dj_skill_policies`, `dj_guard_decisions`, `dj_plan_traces`, `dj_surprise_events`, `dj_audit_log`, `dj_model_artifacts`, `dj_latent_panels`, and `dj_outcomes`.
- Phase 8 edge proof includes selected counter_world hypothetical action `delta=2`, observed outcome/panel mismatch failing with `VALIDATION`, and corrupt artifact failures for `predict`, `plan`, and `record-surprise` leaving prediction/plan/surprise/audit counts unchanged.
- Current Phase 9 MCP evidence: `tmp/5090jepa_evidence/phase9/mcp_phase9_fsv.json`.
- Current Phase 9 MCP test: `crates/context-graph-mcp/src/handlers/tests/dynamicjepa_tools.rs`.
- Phase 9 calls all 27 DynamicJEPA tools through real MCP `tools/call`, verifies 137 total tool definitions with the `llm` feature, trains on CUDA, then independently opens the RocksDB source of truth and reads counts plus decoded rows from `dj_predictions`, `dj_surprise_events`, and `dj_model_artifacts`.
- Phase 9 final observed source-of-truth counts include `dj_domain_packs=1`, `dj_raw_events=61`, `dj_latent_panels=60`, `dj_trajectories=1`, `dj_dataset_shards=3`, `dj_training_runs=1`, `dj_model_artifacts=1`, `dj_predictions=6`, `dj_guard_decisions=5`, `dj_plan_traces=1`, `dj_surprise_events=1`, `dj_actions=66`, `dj_outcomes=61`, `dj_transitions=61`, and `dj_audit_log=193`.
- Phase 9 physical `ldb dump --count_only` proof matched decoded counts for `dj_domain_packs`, `dj_predictions`, `dj_surprise_events`, `dj_model_artifacts`, `dj_raw_events`, `dj_latent_panels`, `dj_training_runs`, and `dj_plan_traces`.
- Phase 9 edge proof includes empty id rejected before storage, duplicate domain registration leaving counts unchanged, invalid CF leaving counts unchanged, and missing artifact prediction leaving prediction counts unchanged.
- Current Phase 10 CLI evidence: `tmp/5090jepa_evidence/counter_world_final/4d2be585-cd38-4da7-9b57-d33dd611cc03/` and `tmp/5090jepa_evidence/gridworld_final/4d198b3b-1cd0-4268-a1e7-e4edefc13a17/`.
- Current Phase 10 test: `crates/context-graph-cli/tests/dynamicjepa_phase10_fsv.rs`.
- Phase 10 counter_world final source-of-truth counts include `dj_raw_events=62`, `dj_adapter_runs=62`, `dj_latent_panels=60`, `dj_dataset_shards=3`, `dj_training_runs=1`, `dj_model_artifacts=1`, `dj_predictions=6`, `dj_guard_decisions=5`, `dj_plan_traces=1`, `dj_surprise_events=1`, `dj_verification_runs=1`, and `dj_audit_log=196`.
- Phase 10 gridworld final source-of-truth counts include `dj_raw_events=8002`, `dj_adapter_runs=8002`, `dj_latent_panels=8000`, `dj_dataset_shards=3`, `dj_training_runs=1`, `dj_model_artifacts=1`, `dj_predictions=6`, `dj_guard_decisions=5`, `dj_plan_traces=1`, `dj_surprise_events=0`, `dj_verification_runs=1`, and `dj_audit_log=25014`.
- Phase 10 physical `ldb dump --count_only` proof matched decoded counts for both domains on `dj_raw_events`, `dj_adapter_runs`, `dj_latent_panels`, `dj_trajectories`, `dj_dataset_shards`, `dj_training_runs`, `dj_model_artifacts`, `dj_predictions`, `dj_guard_decisions`, `dj_plan_traces`, `dj_surprise_events`, `dj_verification_runs`, and `dj_audit_log`.
- Phase 10 artifact evidence recomputed every registered artifact file hash and recorded `equal=true` for all five files per domain.
- Predecessor tests remain required: `dynamicjepa_phase3_fsv.rs`, `dynamicjepa_phase4_fsv.rs`, `dynamicjepa_phase5_fsv.rs`, and `dynamicjepa_phase6_fsv.rs`.

## 3. Required Fixtures

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

## 4. Full-Loop Verification (counter_world, Phase 8/10)

This section is implemented today as the Phase 8 CLI FSV test and the Phase 10 bundled `verify-counter-world` command. Use Section 2 for the current evidence paths.

Setup:

```bash
export DJ_DB=/tmp/5090jepa-cw-fsv
export DJ_ARTIFACTS=/tmp/5090jepa_artifacts
rm -rf "$DJ_DB" "$DJ_ARTIFACTS"
mkdir -p "$DJ_ARTIFACTS"
```

Phase 8/10 required command sequence (each line is one `dynamicjepa` subcommand; harness records the JSON output of each):

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

Phase 10 expected final counts (for the generated 60-event counter verifier plus three edge-case probes; values are **exact** unless noted):

| CF | Final count |
| --- | --- |
| `dj_domain_packs` | 1 |
| `dj_domain_pack_by_nv` | 1 |
| `dj_instrument_registry` | 2 |
| `dj_adapter_registry` | 1 |
| `dj_raw_events` | 62 (60 generated happy-path events + 2 failed edge-case rows) |
| `dj_adapter_runs` | 62 |
| `dj_normalized_states` | 60 |
| `dj_actions` | 60 observed + 5 hypothetical = 65 |
| `dj_outcomes` | 60 |
| `dj_transitions` | 60 |
| `dj_instrument_readings` | 120 (60 events x 2 required instruments) |
| `dj_latent_panels` | 60 |
| `dj_bindings` | 0 (counter_world has no entities; demo skips binding step) |
| `dj_bindings_by_entity` | 0 |
| `dj_trajectories` | 1 |
| `dj_dataset_shards` | 3 (train/val/test) |
| `dj_training_runs` | 1 (`Completed`) |
| `dj_model_artifacts` | 1 (`Active`) |
| `dj_predictions` | 1 (manual `predict`) + 5 (from `plan`) = 6 |
| `dj_skill_policies` | 1 (counter_increment_skill, written during `plan` if not already present) |
| `dj_plan_traces` | 1 |
| `dj_guard_decisions` | 5 (one per candidate) |
| `dj_surprise_events` | 1 (forced counter surprise edge) |
| `dj_verification_runs` | 1 (`Passed`) |
| `dj_audit_log` | 196 observed in the current Phase 10 FSV run |

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

## 5. Edge Cases (counter_world)

For each edge case below the harness:

1. Records `before_counts`.
2. Runs the failing command (expects non-zero exit, captures JSON error body).
3. Records `after_counts`.
4. Confirms the deltas in the table match exactly.
5. Decodes the relevant audit/error rows for evidence.

Current Phase 7 model/artifact edge cases:

- Missing dataset id: `dynamicjepa train` fails with `SOURCE_OF_TRUTH_MISSING`; `dj_training_runs`, `dj_model_artifacts`, and `dj_audit_log` stay unchanged.
- Invalid training device: a config with `schedule.device = "cpu"` fails with a typed training/config error; no artifact row is written.
- Corrupted artifact: after a successful train, mutate one byte of `model.safetensors`; `get-artifact --verify-files` fails with `ARTIFACT_HASH_MISMATCH`, reports `wrote_prediction=false`, and `dj_predictions` remains unchanged at zero.

Current Phase 8 prediction/planning/surprise edge cases:

- Mismatched observed outcome/panel: `record-surprise` fails with `VALIDATION` at `record-surprise.observed_outcome_id`; `dj_surprise_events` and `dj_audit_log` stay unchanged.
- Corrupted artifact at prediction: `predict` fails with `ARTIFACT_HASH_MISMATCH`; `dj_predictions` and `dj_audit_log` stay unchanged.
- Corrupted artifact at planning: `plan` fails with `ARTIFACT_HASH_MISMATCH`; `dj_actions`, `dj_predictions`, `dj_guard_decisions`, `dj_plan_traces`, and `dj_audit_log` stay unchanged.
- Corrupted artifact at surprise comparison: `record-surprise` fails with `ARTIFACT_HASH_MISMATCH`; `dj_surprise_events` and `dj_audit_log` stay unchanged.

The adapter edge cases below remain prerequisites inherited from Phase 4:

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

## 6. Happy-Path Verification (gridworld_5x5)

Setup:

```bash
export DJ_DB=/tmp/5090jepa-gw-fsv
export DJ_ARTIFACTS=/tmp/5090jepa_artifacts/gw
rm -rf "$DJ_DB" "$DJ_ARTIFACTS"
mkdir -p "$DJ_ARTIFACTS"
```

Run the same command sequence as Section 4, substituting `gridworld_5x5` and the gridworld fixtures. Differences:

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

The Phase 6/10 manual gridworld FSV observed the checked-in corpus split as train=4977, val=1330, test=693, totaling 7000 one-step rows. Phase 10 records these exact counts in `summary.json` and in the persisted `VerificationRunRecord.actual_results`.

## 7. Edge Cases (gridworld_5x5)

EC1 empty input: same shape as counter_world EC1.

EC2 invalid action (`move = "diagonal"`): same shape as counter_world EC2 with `field_path = action.move`, `expected = categorical{up,down,left,right,noop}`, `actual = string`.

EC3 OOB state (`state.x = 7`): raw event +1, failed adapter run +1, no normalized state, `field_path = state.x`, `value = 7`, `max = 4`.

## 8. Convergence Audits

After training:

```bash
context-graph dynamicjepa get-artifact --db "$DJ_DB" --id $ARTIFACT --verify-files --json | jq '.evaluation_report'
```

Current Phase 7 counter_world asserts on the generated 60-event fixture (decoded JSON, not return values):

```text
counter_world current: val_latent_cosine ≥ 0.99 ∧ val_latent_mse ≤ 0.005 ∧ vicreg_variance_per_dim_min ≥ 0.5 ∧ parameter_count_trainable = 480
gridworld current:     val_latent_cosine ≥ 0.95 ∧ val_latent_mse ≤ 0.05 ∧ test_latent_cosine ≥ 0.92 ∧ action_contrast_acc ≥ 0.95 ∧ parameter_count_trainable < 20000
recorded but open:     shuffled_target_cosine/covariance strict bounds remain model-quality audit gaps
```

## 9. Artifact Hash Audit (Both Packs)

```bash
ARTIFACT_JSON=$(context-graph dynamicjepa get-artifact --db "$DJ_DB" --id $ARTIFACT --verify-files --json)
echo "$ARTIFACT_JSON" | jq '.artifact_hashes'
```

Every entry must have `equal: true`. The current Phase 7 harness then **mutates** one byte of `model.safetensors`:

```bash
printf '\xff' | dd of="$DJ_ARTIFACTS/<artifact dir>/model.safetensors" bs=1 count=1 conv=notrunc seek=128
context-graph dynamicjepa get-artifact --db "$DJ_DB" --id $ARTIFACT --verify-files --json
```

Expected response:

```json
{
  "operation": "dynamicjepa_get_artifact",
  "status": "error",
  "error_code": "ARTIFACT_HASH_MISMATCH",
  "file": "model.safetensors",
  "expected": "<hex>",
  "actual": "<different hex>",
  "wrote_prediction": false
}
```

The `dj_predictions` count after this must equal the count before. Phase 8 FSV proves `predict` refuses to write any prediction row on artifact hash mismatch.

## 10. Verification Run Record

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

The `VerificationRunRecord.status` is `Passed` only if every count assertion, every readback, every convergence target, and every edge-case error body matched the spec. Phase 10 writes `dj_verification_runs` and the matching audit row atomically; earlier Phase 7/8/9 tests still write their standalone JSON evidence under `tmp/5090jepa_evidence/phase7/`, `tmp/5090jepa_evidence/phase8/`, and `tmp/5090jepa_evidence/phase9/`.

## 11. Operator Sign-Off Checklist (Per Run)

Paste this block into the closing PR description for each domain:

```text
[ ] inspect-counts before run = all zeros
[ ] register-domain success: source_hash matches TOML
[ ] ingest-event success: <N> raw events written
[ ] materialize-panel success: <M> panels written
[ ] compile-trajectories success: <K> trajectories
[ ] compile-dataset success: train/val/test row counts in spec
[ ] train success: metrics meet current convergence targets
[ ] artifact recomputed hashes equal registry
[ ] Phase 7 corrupt artifact: get-artifact --verify-files refused, no prediction row written
[ ] Phase 8 predict success: prediction row present, hash-at-inference matches artifact
[ ] Phase 8 plan success: plan trace present, candidate count = 5, selected action set
[ ] Phase 8 record-surprise success: surprise row written iff cosine < threshold
[ ] EC1 empty input: counts unchanged, error code ADAPTER_FAILED
[ ] EC2 invalid action: raw event +1, failed adapter run +1, no normalized
[ ] EC3 OOB state/panel: raw event +1, failed adapter run +1, no normalized
[ ] artifact hash mismatch test: load/verification call refused, no row written
[ ] Phase 10 verification_run_record.json status=Passed
```

If a box is unchecked, the run is not done.
