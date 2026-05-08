# DynamicJEPA Subsystem

**Source files covered:**
- `crates/context-graph-dynamicjepa/src/lib.rs`
- `crates/context-graph-dynamicjepa/src/config.rs` (`TrainConfig`, `TargetArchitecture`)
- `crates/context-graph-dynamicjepa/src/model/mod.rs` (`TinyMlpJepa`, VICReg loss, EMA target encoder, frozen-instrument-projection target, surprise calibration)
- `crates/context-graph-dynamicjepa/src/service.rs` (train / predict / plan / `record_surprise` / G_tau / per-segment surprise calibration)
- `crates/context-graph-dynamicjepa/src/artifact.rs` (hash-verified safetensors load)
- `crates/context-graph-dynamicjepa/src/transfer.rs` (cross-domain transfer pilot)
- `crates/context-graph-dynamicjepa/src/mi_audit.rs` (KSG-1 pairwise MI)
- `crates/context-graph-core/src/dynamicjepa/**` (record types, ids, schema, validation; pack `mod.rs`)
- `crates/context-graph-storage/src/dynamicjepa/**` (28-CF RocksDB plane: `column_families.rs`, `keys.rs`, `common.rs`, `audit.rs`, `bridge.rs`, `encode.rs`, `mod.rs`, `inspection.rs`, `tests.rs`, plus per-store modules `domain_pack_store`, `event_store`, `panel_store`, `pairwise_store`, `surprise_store`, `verification_store`, `prediction_store`, `planning_store`, `binding_store`, `trajectory_store`, `dataset_store`, `artifact_store`, `constellation_store`, `threshold_store`)
- `crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs`
- `crates/context-graph-mcp/src/tools/definitions/dynamicjepa.rs` (35 tool schemas)
- `crates/context-graph-mcp/src/tools/names.rs` (36 `DYNAMICJEPA_*` name constants — 35 callable + `DYNAMICJEPA_INSPECT_DATASET_ROW` is the 35th tool definition)
- `crates/context-graph-cli/src/commands/dynamicjepa.rs` (`DynamicJepaCommands` enum: 43 subcommands; engine binary entry point used by the MCP→CLI shim)

**Cross-links:** [04_database_schema.md](04_database_schema.md), [09_storage_layer.md](09_storage_layer.md), [11_cuda_acceleration.md](11_cuda_acceleration.md), [16_training_subsystem.md](16_training_subsystem.md), [17_constellation_subsystem.md](17_constellation_subsystem.md), [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md), [20_learning_subsystem.md](20_learning_subsystem.md), [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md). [01_system_overview.md](01_system_overview.md) should reference this doc in its subsystem index.

5090jepa demo plan: see `docs/5090jepa/00_TASK.md` (canonical entry for the 18-file fresh-agent build plan; RTX 5090 + CUDA 13.2 + Candle 0.9.2-alpha; counter_world + gridworld_5x5 packs).

## 1. Scope
DynamicJEPA is a **Joint-Embedding Predictive Architecture** trained, predicted, planned, and audited entirely inside Rust on RTX 5090 / CUDA 13.2 via Candle 0.9.2-alpha. Its "Dynamic" variant takes a `(latent_panel, action)` pair and predicts the next latent panel. It is **independent** of:
- E1-E14 content embedders ([05_embedding_subsystem.md](05_embedding_subsystem.md))
- LLM / DPO style transfer ([19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md))
- Learner UTL state ([20_learning_subsystem.md](20_learning_subsystem.md))

Phase split (per `lib.rs:3-7`):
- **Phase 7**: training Candle MLP (online encoder + target branch + predictor), artifact writing, hash-checked safetensors load.
- **Phase 8**: hash-checked prediction, planning with guard policy, surprise persistence with calibrated cosine threshold.
- **Phase 9**: 35 MCP tools (`dynamicjepa_*`) wired through `mcp__cgreality__*` registry; the MCP server fork-execs the engine CLI for write paths (MCP→CLI shim).

The subsystem owns **28 RocksDB column families** (all prefixed `dj_*`); none overlap with the teleological / content data plane. `DJ_CF_COUNT == 28` is asserted in `crates/context-graph-storage/src/dynamicjepa/tests.rs:1126-1127`.

## 2. Architecture

```
input panel x ──▶ online encoder f_θ ──▶ z_online
                                            │
                              concat(z, action)
                                            │
                                       predictor g_φ ──▶ ẑ
                                                          │
target panel x' ─▶  (a) f_ξ EMA-target encoder ──▶ z_target
                    (b) identity (frozen instrument projection)
```

| Component | Type | Update rule |
| --- | --- | --- |
| Online encoder `f_θ` | MLP, ReLU between layers (`MlpNetwork::forward`, `model/mod.rs:211`) | AdamW gradient |
| Target branch `f_ξ` (`EmaEncoder`) | MLP, same shape, **detached** | EMA: `ξ ← m·ξ + (1-m)·θ` (`LinearLayer::ema_update`, `model/mod.rs:141`) |
| Target branch (`FrozenInstrumentProjection`) | Identity; the raw flattened target panel **is** `z_target` | None — target encoder is `None` in `TinyMlpJepa` (`model/mod.rs:306`) |
| Predictor `g_φ` | MLP, input = latent ⊕ action (`TinyMlpJepa::predict`, `model/mod.rs:341`) | AdamW gradient |

`TargetArchitecture` is set per-config (`config.rs:29-36`):

| Variant | Encoder used at train time | Encoder used at inference (`record_surprise`) |
| --- | --- | --- |
| `EmaEncoder` (default) | Trained, EMA-tracked | `target.encoder` MLP forward (`service.rs:1086-1094`) |
| `FrozenInstrumentProjection` | None — target = identity | Identity: returns `flatten_panel_with_action_override` (`service.rs:1095`) |

`FrozenInstrumentProjection` requires `model.ema_momentum == 0.0` (`config.rs:158-171`) and lifts the `predictor.out_dim == encoder.out_dim` constraint — instead `predictor.out_dim` must equal `target_panel` dim, checked at train time (`model/mod.rs:399-409`). This is the configuration used by the SWE reality loop (see §16).

## 3. Loss (Phase 7)

| Term | Formula | Default weight |
| --- | --- | --- |
| `latent_mse` | `‖ẑ − stop_grad(z_target)‖²` mean (`mse_tensor`, `model/mod.rs:926`) | `latent_mse_weight` |
| `vicreg_variance` | `mean(relu(target_std − sqrt(var(z_online) + 1e-4)))` (`vicreg_variance`, `model/mod.rs:930`) | `vicreg_variance_weight` |
| `vicreg_covariance` | off-diagonal `Σ cov² · mask / latent_dim` (`vicreg_covariance`, `model/mod.rs:938`) | `vicreg_covariance_weight` |
| **Total** | weighted sum of the three above | — |

`vicreg_target_std` is the per-dim variance floor (default 1.0 for the SWE loop; see §16). `z_target` is detached via `.detach()` before MSE; only `online.encoder` and `predictor` parameters reach AdamW (`model/mod.rs:315-319`).

## 4. TrainConfig Schema (`config.rs`)

| Section | Field | Type | Constraint |
| --- | --- | --- | --- |
| `model.encoder` | `kind` | string | must be `"mlp"` (`config.rs:236`) |
| | `hidden` | `Vec<usize>` | non-empty, no zero (`config.rs:242-247`) |
| | `out_dim` | `usize` | > 0 |
| `model.predictor` | `kind` | string | `"mlp"` |
| | `hidden` | `Vec<usize>` | non-empty, no zero |
| | `in_action_dim` | `usize` | matches batch action dim (`model/mod.rs:392-398`) |
| | `out_dim` | `usize` | **must equal** `encoder.out_dim` when `target_architecture=ema_encoder` (`config.rs:134-147`); when `frozen_instrument_projection`, must equal `target_panel` dim |
| | `ignore_action` | bool | default false; when true, action is zeroed at the per-batch dataset boundary (`model/mod.rs:411-425`) and at inference (`service.rs:1041-1042, 1083-1084`) |
| `model.target_architecture` | enum | `"ema_encoder"` (default, `config.rs:34`) or `"frozen_instrument_projection"` |
| `model.ema_momentum` | f64 | `[0.0, 1.0)`; **must be 0.0** when `target_architecture=frozen_instrument_projection` (`config.rs:158-171`) |
| `loss.*` | f64 | finite, ≥ 0 |
| `optim.kind` | string | must be `"adamw"` (Phase 7, `config.rs:195-201`) |
| `optim.lr`, `weight_decay` | f64 | finite, ≥ 0 |
| `optim.warmup_steps` | u64 | linear warmup to `lr` (`apply_lr_schedule`, `model/mod.rs:763`) |
| `schedule.epochs` | usize | > 0 |
| `schedule.batch_size` | usize | > 0 |
| `schedule.device` | string | **must be `"cuda"`**; CPU fallback explicitly forbidden (`config.rs:209-217`) |
| `stopping.metric` | string | `<split>_<metric>` form, e.g. `val_latent_mse` |
| `stopping.target` | f64 | finite, ≥ 0 |
| `stopping.max_seconds` | u64 | > 0 |
| `seed` | u64 | RNG seed (set on CUDA device via `device.set_seed`) |

`TrainConfig::canonical_bytes()` (`config.rs:111`) produces the JSON used for `training_config_hash` (SHA-256, persisted on `TrainingRunRecord`).

## 5. Training Loop (`model/mod.rs::train_tiny_jepa`, line 370)

1. Validate every `TrainExample {input_panel, target_panel, action, negative_panel, split_name, segments}` (`model/mod.rs:9-16`). Reject identical positive/negative pairs (line 836) and any non-finite value (line 821-835).
2. Optionally zero out actions when `predictor.ignore_action = true`.
3. Initialize CUDA device 0 (`Device::new_cuda(0)`, line 436); panic if compute_cap ≠ 12.0; CUDA RNG seed; warm-up matmul + synchronize (`cuda_warmup`, line 896).
4. Build `TinyMlpJepa` (online + optional target + predictor, `model/mod.rs:269-313`). When `EmaEncoder`, target encoder copies online weights at init (`set_from`).
5. Compute baseline metrics (`evaluate_model` with no collapse diagnostics).
6. AdamW over `online.params() ∪ predictor.params()` (target encoder is **never** in the optimizer, `model/mod.rs:315-319`).
7. For each epoch:
   - Shuffle train indices with deterministic PRNG keyed by `(seed, epoch)` (`shuffled_epoch_indices`, line 859).
   - For each batch: forward online + target (detached) + predictor; compute MSE + VICReg variance + VICReg covariance; weighted sum; `opt.backward_step`; `model.update_target(config)` (EMA when `EmaEncoder`, no-op when frozen, line 353); `device.synchronize()`.
   - Wall-clock guard: error `TrainingFailed` if `started.elapsed() > stopping.max_seconds`.
   - Re-evaluate on the `stopping_eval_interval(epochs) = max(1, epochs/50)` cadence (line 773); if `stopping_metric ≤ stopping.target`, break converged.
8. Final evaluation with collapse diagnostics. If not converged AND final stopping metric > target → `TrainingFailed`.

Surprise calibration (Phase 8) is computed by `surprise_calibration_report` and `surprise_segment_calibration_reports` (`model/mod.rs:1412, 1454`) at the end of training and stored in `evaluation_report.json` (see §11).

## 6. Collapse Diagnostics (final epoch only)

Computed in `evaluate_model` + `collapse_diagnostics` (`model/mod.rs:1173+`):

| Metric | Definition |
| --- | --- |
| `val_var_min/p25/median/p75/max` | Quantiles of per-dim variance |
| `covar_frob` | Frobenius norm of off-diagonal covariance |
| `covar_rms` | RMS off-diagonal covariance |
| `covar_loss_scale` | `Σ off² / latent_dim` (matches VICReg loss scale) |
| `eff_rank` | `exp(H)` over normalized eigenvalue spectrum (`effective_rank_from_eigenvalues`, line 1512) |
| `eff_rank_ratio` | `eff_rank / latent_dim` |
| `alignment` | Mean squared distance between L2-normalized prediction & target |
| `uniformity` | `log mean exp(-2·‖z_i − z_j‖²)` over pairs |
| `sigreg_kl` | Diagonal KL `Σ 0.5·(σ² + μ² − 1 − ln σ²)`; clamped at `1e12`, var floor `1e-12` |
| `latent_collapsed` | true when `var_min < 1e-8` OR `eff_rank_ratio < 0.10` OR any non-finite |

Eigenvalues use a Jacobi rotation routine (`jacobi_symmetric_eigenvalues`, line 1532) with `max_iter = max(n²·32, 64)` and tolerance `1e-12`.

## 7. Evaluation Splits

For each split (`train`, `val`, `test`) the report contains:
- `latent_mse` — mean `‖pred − target‖²`
- `latent_cosine` — mean cos(pred, target)
- `action_contrast_acc` — fraction where `cos(pred, target) > cos(pred, negative)`
- `shuffled_target_cosine` — centered cosine vs negative; baseline check

The full `EvaluationReport` (`model/mod.rs:27`) also stores:
- `objective_id`, `target_architecture`
- `surprise_calibration: SurpriseCalibrationReport` — the global calibrated cosine threshold computed from the val split (p10 of finite cosines, see §11)
- `surprise_segment_calibrations: BTreeMap<String, SurpriseCalibrationReport>` — per-segment calibrations keyed by `action.<field>=<value>` for each example segment (`model/mod.rs:1454-1510`)
- `random_init_baseline.cosine`, `shuffled_target_baseline.cosine` (both required to be present, `model/mod.rs:1107-1126`)
- VICReg variance/covariance scalars (`vicreg_variance_per_dim_min/mean`, `vicreg_covariance_off_diag_frobenius/rms`, `vicreg_covariance_loss_scale`)
- `epoch_metrics: Vec<EpochMetric>`, `parameter_count_trainable`

## 8. Service API (`service.rs`)

| Function | Inputs | Persists |
| --- | --- | --- |
| `train_dataset` | `dataset_id`, `&TrainConfig`, `artifact_root` | `CF_DJ_TRAINING_RUNS` (Started → Completed/Failed), `CF_DJ_MODEL_ARTIFACTS`, `CF_DJ_AUDIT_LOG` + safetensors files |
| `predict_next_panel` | `artifact_id`, `panel_id`, `action_id` | `CF_DJ_PREDICTIONS` + audit |
| `plan_next_action` | `artifact_id`, `panel_id`, `skill_id` | `CF_DJ_ACTIONS` (candidates), `CF_DJ_PREDICTIONS`, `CF_DJ_GUARD_DECISIONS`, `CF_DJ_PLAN_TRACES`, optional `CF_DJ_SKILL_POLICIES`, audit |
| `plan_next_action_with_candidate` | same + `candidate_fields: BTreeMap<String, FieldValue>` (operator-injected hypothetical action; `service.rs:418`) | same as `plan_next_action` |
| `record_surprise` | `prediction_id`, `observed_outcome_id`, `observed_panel_id` | `CF_DJ_SURPRISE_EVENTS` + audit (only when `cosine < calibrated threshold`) |
| `cross_domain_transfer` | `CrossDomainTransferConfig` | `transfer_results.json`, paper table, plots, bridge artifacts (filesystem) |

After every persistence step, the service performs an **immediate readback** and bincode equality check against the in-memory record; mismatch returns `StorageInvariantViolation` (`service.rs:147, 222, 388, 645, 838`). Training runs that error after the started-row was persisted are flipped to `TrainingRunStatus::Failed { error }` before the error propagates (`service.rs:255-290`).

## 9. Predict / Plan Mechanics

`predict_vector` (`service.rs:1033`):
1. Build `action_dense_vector` from declared instrument specs (`service.rs:1193`): scalar normalized via `Normalization`, categorical → one-hot in declared variant order; only instruments with `input_fields` starting with `action.` contribute.
2. `flatten_panel_with_action_override(panel, Some(&action))` → input vector.
3. MLP forward through `online.encoder` (ReLU between layers, no activation on last) → `z`.
4. Concatenate `z ⊕ action` → MLP forward through `predictor`.
5. Reject any non-finite intermediate (`mlp_forward`, `service.rs:1099`); bail with `TrainingFailed`.

`uncertainty = ‖predicted‖₂ / sqrt(predicted.len())`. Stored objective score is `-uncertainty`.

`plan_next_action` enumerates candidates per `domain.planner_policy.candidate_actions`:
- `EnumeratedDeltas { deltas }` — one i64 action field, e.g. counter_world `[-1, 0, 1]` (`service.rs:1375`).
- `EnumeratedMoves { moves }` — one categorical action field, e.g. gridworld `["up","down","left","right"]` (`service.rs:1387`).
- `EnumeratedActionRecords { records }` — fully-structured candidate action records, each a `BTreeMap<String, FieldValue>` validated against the action schema (`service.rs:1401`, validator `validate_structured_action_record:1470`). The SWE reality-loop pack uses this variant (see §16).

`plan_next_action_with_candidate` additionally appends one operator-supplied structured action to the candidate set (`service.rs:454-461`) — used by the reality loop to score the live planned action alongside declared candidates.

For each candidate it predicts, computes utility (`utility_score`, `service.rs:2143`), runs every guard in `planner_policy.guards`, then picks the highest-utility allowed action (or — when all are rejected and a constellation is registered — the highest-utility candidate, `service.rs:540-545`). Resulting status: `PlanTraceStatus::Selected | Rejected | Failed { error }` (`core/planner.rs:36-41`). Utility rules currently support: counter (predicted `counter`), gridworld (`-uncertainty`), career (`job_zone − uncertainty`), `service.rs:2143-2162`.

## 10. Guard System

Phase 3 supports `guards = ["bounds_check"]` only (`service.rs:1616`). `bounds_check` produces:
- A **utility decision** based on `domain.state_schema` per-field min/max applied to the **predicted next state** (deterministically derived via `predicted_state_after_action`, `service.rs:1988`). State bounds are intersected with each instrument's `validation.{min,max}` via `effective_state_bounds` (`service.rs:1967`).
- An optional **G_tau decision** when `domain.constellation` is set (`evaluate_gtau_guard`, `service.rs:1713`): project the panel/action into each modality, compare cosine-to-centroid against the calibrated threshold `tau`, and reject if any required modality fails. Constellation lookup is by `(domain_uuid, subject_id, modality.ordinal)` and the threshold by the same key plus `supersede_seq=0`.

`GuardDecisionRecord` (CF `CF_DJ_GUARD_DECISIONS`, version 2) stores both decisions, `cosine_to_centroid_per_modality`, `tau_per_modality`, `gtau_failed_modalities`, and the `constellation_uuid` used (`core/guard.rs:11-36`). Combined-decision rules are in `combined_guard_decision` (`service.rs:1932`): both reject → `UTILITY_AND_GTAU_FAILED`; either reject alone wins; both allow → allow.

`predicted_state_after_action` is **only** populated for the registered demo-domain shapes (`counter`/`delta`, gridworld `x`/`y`/`move`, career `job_zone`/`move`); the SWE reality-loop pack uses the `counter`/`delta` branch.

## 11. Surprise (`record_surprise`)

1. Re-encode the **observed** panel under the **same artifact** via `encode_panel_vector` (`service.rs:1076`). When `EmaEncoder`, this runs the target encoder MLP; when `FrozenInstrumentProjection`, the observed panel is returned as-is (the target encoder is the instrument projection itself, **frozen** by construction).
2. Compute `cosine(prediction.predicted_next_panel_vec, observed_vec)` (`cosine_f32`, `service.rs:2318`) and `error_norm = L2 distance`.
3. Threshold = `calibrated_surprise_threshold(&artifact, &candidate_action)` (`service.rs:880`):
   - For non-SWE-loop domains: read `evaluation_report.json → surprise_calibration → threshold_cosine`.
   - For domain `contextgraph_swe_reality_loop_v1`: read **per-segment** `evaluation_report.json → surprise_segment_calibrations[segment_key]`, where `segment_key = surprise_calibration_segment_key(action)` (`service.rs:978`):
     - `tool_family == "workspace_mutation"` → `action.path_risk=<value>`
     - otherwise → `action.tool_family=<value>`
   - Either branch fails if `status != "calibrated"` (e.g. `insufficient_segment_data`); threshold must be finite in `[-1, 1]`.
4. **If `cosine ≥ threshold`** → no surprise persisted; return `wrote_surprise = false` (`service.rs:777-786`).
5. **Else** → persist `SurpriseEventRecord { surprise_kind: UnexpectedOutcome, cosine, threshold, error_norm }` + audit row (`signal_yield = 1` from `signal_yield_for_operation("record_surprise", _)`).

## 12. Identifiers (`core/dynamicjepa/ids.rs`)

**String IDs** (validated by `validate_string_id`, lowercase ASCII regex `^[a-z][a-z0-9_.-]{2,127}$`, length 3..=128, line 7-34):
`DomainPackId`, `InstrumentId`, `AdapterId`.

**UUID IDs** (`uuid_id!` macro rejects nil, line 73-142):
`EventId`, `StateId`, `ActionId`, `OutcomeId`, `TransitionId`, `PanelId`, `BindingId`, `TrajectoryId`, `DatasetShardId`, `DatasetId`, `TrainingRunId`, `ModelArtifactId`, `PredictionId`, `SkillId`, `PlanTraceId`, `GuardDecisionId`, `SurpriseEventId`, `VerificationRunId`, `PairwiseReadingId`, `ConstellationId`, `ThresholdCalibrationId`.

## 13. Record Versions

| Constant | Value | File |
| --- | --- | --- |
| `DOMAIN_PACK_RECORD_VERSION` | 1 | `domain_pack.rs:14` |
| `INSTRUMENT_READING_RECORD_VERSION` | 1 | `instrument.rs:10` |
| `ADAPTER_RUN_RECORD_VERSION` | 1 | `adapter.rs:11` |
| `NORMALIZED_STATE_RECORD_VERSION` | 1 | `state_action_outcome.rs:8` |
| `NORMALIZED_ACTION_RECORD_VERSION` | 1 | `state_action_outcome.rs:9` |
| `NORMALIZED_OUTCOME_RECORD_VERSION` | 1 | `state_action_outcome.rs:10` |
| `STATE_TRANSITION_RECORD_VERSION` | 1 | `state_action_outcome.rs:11` |
| `LATENT_PANEL_RECORD_VERSION` | 2 | `panel.rs:10` |
| `BINDING_RECORD_VERSION` | 1 | `binding.rs:8` |
| `TRAJECTORY_RECORD_VERSION` | 1 | `trajectory.rs:8` |
| `DATASET_SHARD_RECORD_VERSION` | 2 | `dataset.rs:7` |
| `MODEL_ARTIFACT_RECORD_VERSION` | 1 | `artifact.rs:8` |
| `TRAINING_RUN_RECORD_VERSION` | 1 | `predictor.rs:10` |
| `PREDICTION_RECORD_VERSION` | 1 | `predictor.rs:11` |
| `RAW_DOMAIN_EVENT_RECORD_VERSION` | 1 | `event.rs:7` |
| `SKILL_POLICY_RECORD_VERSION` | 1 | `skill.rs:7` |
| `PLAN_TRACE_RECORD_VERSION` | 2 | `planner.rs:11` |
| `SURPRISE_EVENT_RECORD_VERSION` | 1 | `planner.rs:12` |
| `GUARD_DECISION_RECORD_VERSION` | 2 | `guard.rs:9` |
| `PAIRWISE_READING_RECORD_VERSION` | 1 | `meaning_compression.rs:12` |
| `CONSTELLATION_CENTROID_RECORD_VERSION` | 1 | `meaning_compression.rs:13` |
| `THRESHOLD_CALIBRATION_RECORD_VERSION` | 1 | `meaning_compression.rs:14` |
| `VERIFICATION_RUN_RECORD_VERSION` | 1 | `verification.rs:9` |
| `DJ_AUDIT_RECORD_VERSION` | 2 | `storage/audit.rs:7` |

Every record carries a `DjRecordHeader { id, version, domain_pack_id, domain_pack_version, created_at_unix_ms, run_id, content_hash }`. `refresh_content_hash()` is called before write; readback equality is enforced.

## 14. RocksDB Column Families (28 total, prefix `dj_`)

Source: `crates/context-graph-storage/src/dynamicjepa/column_families.rs`. `DJ_CF_NAMES` declares the canonical order; `DJ_CF_COUNT == DJ_CF_NAMES.len()` and the test asserts both equal 28 (`tests.rs:1126-1127`).

| CF | Role | CF options helper |
| --- | --- | --- |
| `dj_domain_packs` | Domain pack records by storage UUID | `dj_uuid_point_cf_options` |
| `dj_domain_pack_by_nv` | `(name, version) → uuid` | `dj_composite_index_cf_options` |
| `dj_instrument_registry` | Instrument spec ledger | `dj_composite_index_cf_options` |
| `dj_adapter_registry` | Adapter spec ledger | `dj_composite_index_cf_options` |
| `dj_raw_events` | Raw JSONL events (post-ingest) | `dj_uuid_point_cf_options` |
| `dj_normalized_states` | Adapter-output `NormalizedState` | `dj_uuid_point_cf_options` |
| `dj_actions` | Persisted `NormalizedAction` (observed + hypothetical) | `dj_uuid_point_cf_options` |
| `dj_outcomes` | Persisted `NormalizedOutcome` | `dj_uuid_point_cf_options` |
| `dj_transitions` | `(state, action, next_state, outcome)` tuples | `dj_uuid_point_cf_options` |
| `dj_adapter_runs` | Per-event adapter telemetry | `dj_append_only_cf_options(prefix=16)` |
| `dj_instrument_readings` | Per-instrument projections of an event | `dj_composite_index_cf_options` |
| `dj_latent_panels` | `LatentPanel` records (large blobs, 64KB block / 8MB write buf) | `dj_large_blob_cf_options` |
| `dj_pairwise_readings` | Cosine pair readings between instruments | `dj_append_only_cf_options(prefix=16)` |
| `dj_constellations` | DynamicJEPA constellation centroids | `dj_composite_index_cf_options` |
| `dj_threshold_calibrations` | G_tau thresholds | `dj_composite_index_cf_options` |
| `dj_bindings` | Source-of-truth bindings | `dj_uuid_point_cf_options` |
| `dj_bindings_by_entity` | Reverse index by entity (no bloom) | `dj_secondary_index_cf_options` |
| `dj_trajectories` | Compiled trajectories | `dj_uuid_point_cf_options` |
| `dj_dataset_shards` | One-step dataset rows (large blobs) | `dj_large_blob_cf_options` |
| `dj_training_runs` | Run lifecycle (Started → Completed/Failed) | `dj_append_only_cf_options(prefix=16)` |
| `dj_model_artifacts` | Artifact registry (paths + per-file hashes) | `dj_uuid_point_cf_options` |
| `dj_predictions` | Predictions + objective scores | `dj_append_only_cf_options(prefix=16)` |
| `dj_skill_policies` | Skill policies (e.g. `EnumerateDeclaredActions`) | `dj_uuid_point_cf_options` |
| `dj_plan_traces` | Plan traces, candidate IDs, guards, selection | `dj_append_only_cf_options(prefix=16)` |
| `dj_guard_decisions` | Per-guard outcomes (utility + G_tau) | `dj_append_only_cf_options(prefix=16)` |
| `dj_surprise_events` | Persisted surprise events | `dj_append_only_cf_options(prefix=16)` |
| `dj_verification_runs` | Calibration + audit verification runs | `dj_append_only_cf_options(prefix=16)` |
| `dj_audit_log` | Universal audit (every operation) | `dj_append_only_cf_options(prefix=8)` |

CF option helpers (`column_families.rs:107-125`): all use Lz4 + level compaction; `large_blob` uses 64KB blocks + 8MB write buffer; uuid_point/composite_index use 8KB blocks + 4MB write buffer + 16-byte prefix bloom; secondary_index has no bloom. `dj_audit_log` uses an 8-byte prefix extractor; everything else with prefix uses 16 bytes.

## 15. Domain Pack (`core/dynamicjepa/domain_pack.rs`)

A domain pack is a strict TOML registered into `dj_domain_packs`. Required fields:
- `state_schema`, `action_schema`, `outcome_schema`, `entity_schema`, `time_schema` (typed `FieldKind`: `I64`, `F64`, `Categorical { variants }`, `Bool`, `String`, `Vector`, `UnixMs`).
- `instrument_specs` — at least one (`domain_pack.rs:349-364`); each requires `kind ∈ {Scalar, Categorical, Onehot, Vector, Time}`, declared `output_shape`, optional `Normalization::{None, StandardScore { mean, std }}`, validation bounds, and **must include** `PairKindName::Cosine` in `pair_kinds` (`instrument.rs:150-155`).
- `adapter_specs` — at least one; every required instrument input field must be produced by some adapter mapping (`domain_pack.rs:381-397`).
- `objective_specs` — Phase 1 / 5090 demo accepts `kind = "latent_delta"` only; positive `loss_weight`; non-empty `required_dataset_fields`. Each `target` must reference a declared schema field (`domain_pack.rs:398-410`).
- `invariants` — `severity = "fatal"` only (`domain_pack.rs:181-188`).
- `dataset_policy` — `negative_sampling = "same_domain_different_outcome"`, `min_negatives_per_row > 0`, optional split buckets sum to 1.0.
- `planner_policy` — `candidate_actions = EnumeratedDeltas | EnumeratedMoves | EnumeratedActionRecords`; `guards`; `surprise_threshold ∈ [0, 1]`. `EnumeratedActionRecords` records validate against the action schema (`domain_pack.rs:251-280`).
- `constellation` (optional) — `ConstellationConfig { global, subjects }` with `ConstellationModalities = All | List(Vec<InstrumentId>)`. Subject `field` must be declared, and `value` must match a categorical variant or be a non-empty string (`domain_pack.rs:454-525`).
- `verification_policy` — non-empty, deduplicated `required_cfs`; optional expected counts.
- `source_hash: [u8; 32]` — SHA-256 of TOML source.

`schema_version ∈ {1, 2}` is permitted (`domain_pack.rs:330-336`).

## 16. The Reality-Loop Domain Pack — `contextgraph_swe_reality_loop_v1` (1.0.0)

`configs/dynamicjepa/domain_packs/contextgraph_swe_reality_loop.v1.toml` registers the structured tool-transition pack DynamicJEPA uses to score live SWE-bench reality-loop attempts (commit efd50b20 / 8ceb08e6 / 7d3debdb / 22fae1fe). Highlights:

| Section | Cardinality | Notable contents |
| --- | --- | --- |
| `state.fields` | 13 | `counter` (i64, `[-10000, 10000]`), `attempt`, `round`, plus 10 categorical fields covering `tool_family`, `repo_dirty_state`, `prompt_version`, `model_id`, `primary_path_bucket`, `symbol_bucket`, `dependency_layer`, `test_target_kind`, `error_signature`, `ownership_bucket` |
| `action.fields` | 21 | `delta` (i64, bounds-checked); `tool_family`, `command_kind`, `edit_size_bucket`, `path_risk`, `target_path_bucket`, `symbol_bucket`, `dependency_layer`, `test_target_kind`, `ownership_bucket`, `patch_intent_bucket`, **`patch_shape_bucket`**, **`patch_scope_bucket`**, `ast_language`, `ast_parse_status`, `ast_node_kind`, `ast_symbol_kind`, `ast_edit_kind`; numeric `planned_patch_bytes`, `planned_changed_file_count`, `ast_touched_node_count`, `ast_symbol_count` |
| `outcome.fields` | 16 | `next_counter`, `outcome_label ∈ {Useful, Neutral, Harmful, Unknown}`, `result_status`, `verifier_status`, `test_failure_class`, `incident_code`, `changed_file_count`, `patch_bytes`, `correction_required`, `file_risk`, `error_signature`, `symbol_bucket`, `dependency_layer`, `test_target_kind`, `ownership_bucket`, `transition_id` |
| `entity.fields` | 2 | `trajectory_id`, `run_id` |
| `time` | 1 | `ts_ms` i64 |
| `instruments` | 39 | scalar (`tool_progress_counter`, `attempt_index`, `round_index`, `tool_utility_delta`, `planned_patch_bytes`, `planned_changed_file_count`, `action_ast_touched_node_count`, `action_ast_symbol_count`, `observed_progress_counter`, `changed_file_count`, `patch_bytes`); categorical (32, all with `pair_kinds=["cosine"]`); plus one `time` instrument `transition_time` (`fractional_day_of_week`) |
| `adapters` | 1 | `json_event` v1 mapping every state/action/outcome/time/entity field to a `$.field` JSONPath |
| `objectives` | 1 | `predict_next_tool_progress` (`kind=latent_delta`, `target=outcome.next_counter`, `loss_weight=1.0`, requires `target_panel_id`, `input_panel_id`, `action_id`) |
| `invariants` | 1 | `tool_progress_conservation`: `outcome.next_counter == state.counter + action.delta` (severity `fatal`) |
| `dataset_policy` | — | `default_split = "by_time"`, `negative_sampling = "same_domain_different_outcome"`, `min_negatives_per_row = 1` |
| `planner_policy` | 8 | `candidate_actions = EnumeratedActionRecords` with 8 fully-typed records (mix of `workspace_mutation`/`verification`/`repository_navigation`); `guards = ["bounds_check"]`; `surprise_threshold = 0.85` |
| `constellation.global` | 39 modalities | `require_loo_min = 0.70`, `calibration_percentile = 10`, `calibration_set_size = 12` |
| `verification_policy` | — | 22 required CFs; expected 1 trajectory / 60 events / 60 panels / 59 dataset rows; train/val/test split 41/12/6 |

Training (`configs/dynamicjepa/verification/swe_reality_loop_train_config.json`) uses `target_architecture = "frozen_instrument_projection"` with `ema_momentum = 0.0`, encoder out_dim 16, predictor out_dim 272 (= the target panel dim built from instrument projections), action dim 138, hidden `[128]`, `lr = 0.01`, `weight_decay = 0.0001`, AdamW, `epochs = 800`, `batch_size = 41`, `device = "cuda"`, `vicreg_target_std = 1.0`, `stopping = { metric = val_latent_mse, target = 0.45, max_seconds = 120 }`, `seed = 42`.

The reality-loop binary (`crates/context-graph-cli/src/bin/reality-loop.rs:74`) hard-codes `DYNAMICJEPA_REALITY_DOMAIN_PACK = "contextgraph_swe_reality_loop_v1"`, requires either both an artifact and the matching pack (and refuses to use an artifact whose `domain_pack_id` differs, line 5683-5686), and bootstraps a cold-start panel that **must** have non-empty instrument readings (line 5963 `RUST_REALITY_DYNAMICJEPA_COLD_START_PANEL_EMPTY`). Per-CF count delta after cold-start ingest is checked exactly (+1 across `dj_raw_events`, `dj_adapter_runs`, `dj_normalized_states`, `dj_actions`, `dj_outcomes`, `dj_transitions`, `dj_latent_panels`, line 5989-6016).

## 17. Demo Domains

| Domain | ID | Action shape | Default surprise threshold |
| --- | --- | --- | --- |
| Counter world | `counter_world` (1.0.0) | `EnumeratedDeltas { deltas: [-1, 0, 1] }` | configurable |
| Gridworld 5×5 | `gridworld_5x5` (1.0.0) | `EnumeratedMoves { moves: ["up","down","left","right"] }` | configurable |
| Counter→Grid bridge | `COUNTER_TO_GRID_DOMAIN_ID` (1.0.0) | bridge instruments tied via `counter_to_grid_*` helpers | — |
| ContextGraph SWE reality loop | `contextgraph_swe_reality_loop_v1` (1.0.0) | `EnumeratedActionRecords { records: 8 }` | 0.85 (per-segment calibrated, see §11) |

## 18. Cross-Domain Transfer Pilot (`transfer.rs`)

| Constant | Value |
| --- | --- |
| `TRANSFER_DEFAULT_SEEDS` | `[42, 43, 44, 45, 46]` |
| `TRANSFER_DEFAULT_BOOTSTRAP_ITERS` | 10000 |
| `BRIDGE_EFFECT_BASIS_DIM` | 3 |
| `POSITION_MEAN`, `POSITION_STD` | 12.0, 12.0 |
| `SHUFFLED_TARGET_PERMUTATIONS` | 32 |

`CrossDomainTransferConfig::validate` (`transfer.rs:43`) enforces:
- `output_root` must not exist (immutable evidence).
- ≥ 1 unique seed.
- `source_events ≥ 20`, `target_events ≥ 20`.
- `bootstrap_iters > 0`.
- All training schedule fields > 0.
- `learning_rate > 0` finite, `stopping_target > 0` finite.
- `validate_counter_to_grid_bridge_instrument_count(COUNTER_TO_GRID_BRIDGE_INSTRUMENT_IDS.len())` enforces bridge instrument count.

Phase 1: train a counter-world bridge predictor per seed. Phase 2: evaluate on gridworld bridge events with paired bootstrap CIs. Outputs include `transfer_results.json`, paper table (`table_cross_domain_transfer_csv`), `cross_domain_transfer_box_svg`, and per-seed bridge artifacts.

## 19. Pairwise Mutual-Information Audit (`mi_audit.rs`)

| Constant | Value |
| --- | --- |
| `MI_AUDIT_MIN_SAMPLE_SIZE` | 50 |
| `MI_AUDIT_LOOSE_THRESHOLD` | 0.7 |
| `MI_AUDIT_STRICT_THRESHOLD` | 0.9 |

KSG-1 estimator (`estimate_ksg_1`, `mi_audit.rs:85`):
1. Validate sample size and finite values per pair (`xs`, `ys`).
2. Compute local MI contributions from k-nearest-neighbor distances using Chebyshev distance and digamma corrections.
3. `mi_nats = mean(local_mi_nats)`; reject non-finite.
4. `x_entropy = empirical_entropy_nats(xs)`, `y_entropy = empirical_entropy_nats(ys)`.
5. `normalizer = min(x_entropy, y_entropy)`; reject ≤ 0 or non-finite.
6. `mi_normalised = (max(mi_nats, 0) / normalizer).clamp(0, 1)`.

`bootstrap_ksg_1_ci` (`mi_audit.rs:121`) does deterministic bootstrap resampling of `local_mi_nats` with `SplitMix64` PRNG to produce `(low, high)` 95% CIs; `bootstrap_iters=0` errors `MiAuditBootstrapDegenerate`.

`PairwiseMiSummary` contains `n_redundant_pairs_loose/strict` (count of pairs over each threshold) and an effective panel-slot count.

## 20. MCP Tools (35 total, `definitions/dynamicjepa.rs`)

Source-of-truth name constants live in `tools/names.rs:275-343` (36 constants — 35 are wired into `definitions::dynamicjepa::definitions()`; both `dynamicjepa_get_dataset_shard` and `dynamicjepa_inspect_dataset_row` exist). All schemas are `additionalProperties: false`. Common helpers (`db_path`, `path_prop`, `string_prop`, `string_default`, `bool_default`, `number_default`, `integer_min`, `integer_default`, `integer_array_default`, `number_positive_default`, `limit`, `offset`) are private to `definitions/dynamicjepa.rs`.

### Domain pack & registry (3)

| Tool | Purpose |
| --- | --- |
| `dynamicjepa_register_domain_pack` | Register a strict TOML domain pack into `dj_domain_packs` |
| `dynamicjepa_list_domain_packs` | Paginate registered packs |
| `dynamicjepa_get_domain_pack` | Read one pack by `(id, version)` |

### Ingest & adaptation (3)

| Tool | Purpose |
| --- | --- |
| `dynamicjepa_ingest_event` | JSONL fixture → registered adapter run |
| `dynamicjepa_run_adapter` | Re-run adapter for one persisted raw event |
| `dynamicjepa_list_instrument_readings` | Per-event readings |

### Panels & bindings (4)

| Tool | Purpose |
| --- | --- |
| `dynamicjepa_materialize_panel` | One panel or all pending for a domain |
| `dynamicjepa_get_panel` | Read panel + optional readings |
| `dynamicjepa_create_binding` | Persist deterministic binding `(left, right)` |
| `dynamicjepa_list_bindings` | Filter by entity |

### Trajectory & dataset (6)

| Tool | Purpose |
| --- | --- |
| `dynamicjepa_compile_trajectories` | Policy-driven (`by_domain_session`) compile |
| `dynamicjepa_get_trajectory` | Read one trajectory |
| `dynamicjepa_list_trajectories` | Per-domain pagination |
| `dynamicjepa_compile_dataset` | One-step rows → dataset shard (split: train/val/test) |
| `dynamicjepa_get_dataset_shard` | Read shard |
| `dynamicjepa_inspect_dataset_row` | Decode one row + linked source-of-truth records |

### Training & artifacts (3)

| Tool | Purpose |
| --- | --- |
| `dynamicjepa_train` | CUDA-only train; persists run + artifact + audit |
| `dynamicjepa_get_training_run` | Read run by UUID |
| `dynamicjepa_get_artifact` | Read artifact; toggle `verifyFiles=true` to recompute hashes |

### Inference & planning (6)

| Tool | Purpose |
| --- | --- |
| `dynamicjepa_predict` | Persist hash-verified prediction |
| `dynamicjepa_plan` | Persist plan trace + candidates + predictions + guards (optional `candidateActionJson` injects one structured live action) |
| `dynamicjepa_record_surprise` | Persist surprise when calibrated cosine threshold breached |
| `dynamicjepa_get_prediction` | Read one prediction |
| `dynamicjepa_get_plan_trace` | Read plan trace; toggle `includePredictions`/`includeGuards` |
| `dynamicjepa_get_surprise` | Read surprise event |

### Constellation & threshold (5)

| Tool | Purpose |
| --- | --- |
| `dynamicjepa_build_constellation` | Build immutable centroids from instrument readings |
| `dynamicjepa_list_constellations` | Filter by domain/subject |
| `dynamicjepa_get_constellation` | One centroid |
| `dynamicjepa_calibrate_threshold` | Calibrate G_tau thresholds from disjoint held-out events |
| `dynamicjepa_recalibrate_threshold` | Supersede prior threshold (requires `supersedes` + `reason`) |

### Audits & evidence (5)

| Tool | Purpose |
| --- | --- |
| `dynamicjepa_compute_mc_ratio` | Aggregate `dj_audit_log` `signal_yield` rows → MC-ratio paper evidence |
| `dynamicjepa_audit_pairwise_mi` | KSG-1 pairwise MI audit; sample size 50-1000; bootstrap CIs |
| `dynamicjepa_cross_domain_transfer` | counter_world → gridworld pilot |
| `dynamicjepa_inspect_counts` | Row counts for every `dj_*` CF |
| `dynamicjepa_inspect_cf` | Decode rows from one CF (or one exact key by `keyHex`) |

The MCP→CLI shim (introduced 2026-05-07) means every write tool fork-execs `target/release/reality-loop harness <subcommand>` (or `context-graph-cli dynamicjepa <subcommand>`); changes to `crates/context-graph-cli/src/commands/dynamicjepa.rs` propagate to the next MCP invocation without an `/mcp reconnect`. The CLI declares 43 `DynamicJepaCommands` variants (`commands/dynamicjepa.rs:94-184`) — the 35 MCP tools plus 8 CLI-only verifier/research commands (`VerifyCounterWorld`, `VerifyGridworld`, `VerifyCareerTaxonomy`, `ResearchSmoke`, `HpSweep`, `MultiseedAggregate`, `CaptureReference`, `CheckReference`, `CheckReleaseAggregateReference`).

## 21. Artifact Layout (`artifact.rs`)

Per training run, the artifact root is `<base_root>/<domain_id>/<training_run_id>` and **must not exist before** the run (`ensure_clean_run_dir`, `artifact.rs:191`). Files written:
- `model.safetensors` — full tensor map: `online.encoder.layerN.{weight,bias}`, `predictor.layerN.{weight,bias}`, plus `target.encoder.layerN.{weight,bias}` only when `target_architecture = ema_encoder`.
- `config.json` — canonical `TrainConfig` JSON.
- `metrics.json` — flattened metric scalars.
- `evaluation_report.json` — full `EvaluationReport` (incl. global + per-segment `surprise_calibration*`).
- `manifest.json` — pre-manifest list of relative_path/sha256/size_bytes for every other file.

`ModelArtifactRecord.files: Vec<ArtifactFile>` stores `relative_path`, `sha256: [u8; 32]`, `size_bytes` for every file. `verify_artifact_files` recomputes hashes and refuses any mismatch (`ArtifactHashMismatch` error). `load_artifact_for_inference` rejects on corrupt safetensors and stores `model_file_sha256` for inclusion in `PredictionRecord.model_artifact_hash_at_inference` (`artifact.rs:151-189`).

## 22. Audit Log

Every operation writes a `DjAuditRecord` (`storage/audit.rs:9`, version 2) to `dj_audit_log` with:
- `audit_id` (Uuid), `timestamp_unix_nanos`
- `operation` (e.g. `train_predictor`, `register_artifact`, `predict`, `plan`, `record_surprise`, `bind`, `materialize_panel`, …)
- `actor: String` (the writer; service writes `"dynamicjepa_service"`)
- `input_ids: Vec<String>`, `output_ids: Vec<String>`
- `cfs_touched: Vec<String>`, `content_hashes: Vec<[u8; 32]>` (every refreshed record hash)
- `status: AuditStatus::{Ok, Failed { error_code }}`
- `verification_run_id: Option<Uuid>`
- `signal_yield: u32` — set via `signal_yield_for_operation(operation, dimensions)` (`storage/audit.rs:69`); used by `compute_mc_ratio`. Known operations: `ingest_event=1`, `run_adapter_success=1`, `materialize_panel=n`, `materialize_pairwise=n*(n-1)/2`, `compile_transition=2`, `predict=n+m`, `plan=k*(n+m+1)`, `record_surprise=1`, `bind=1`; everything else is 0; unknown ops error `SignalYieldUnknownOperation`.

Failed runs are also audited: `service.rs:269-289` writes a `Failed` row even when the original `Started` row was already persisted, ensuring every state transition is in the log.

## 23. Storage Helpers (selected)

- `put_training_run_with_audit_batch`, `put_model_artifact_completion_batch` — atomic write of training-run row + artifact row + audit row in a single RocksDB batch (`storage/dynamicjepa/common.rs`).
- `put_plan_batch` (`storage/dynamicjepa/planning_store.rs`) — atomic write of skill policy (optional), candidate actions, predictions, guard decisions, plan trace, audit.
- `put_prediction_with_audit_batch`, `put_surprise_event_with_audit_batch` — single-row + audit.
- `signal_yield_for_operation(op, dims) → u32` — produces `signal_yield` for the audit row used by paper evidence aggregation.
- `domain_pack_storage_uuid(id, version)` (`storage/dynamicjepa/keys.rs`) — deterministic UUID derived from `(id, version)` strings; the same UUID is used as the prefix in the constellation/threshold composite indexes.

## 24. Error Surface

`DynamicJepaError` (`core/dynamicjepa/error.rs`, 51 variants) is the universal result type. Every variant has a stable `code()` string used by audit logs (`error.rs:332-393`).

| Variant | code() | When raised |
| --- | --- | --- |
| `Validation` | `VALIDATION` | Schema/config rejection, structured candidate validation, segment string lookup |
| `SourceOfTruthMissing` | `SOURCE_OF_TRUTH_MISSING` | Readback after persistence found nothing |
| `Storage` | `STORAGE` | RocksDB / filesystem / artifact-file IO |
| `Codec` | `CODEC` | bincode/JSON version/payload mismatch |
| `InstrumentFailed` | `INSTRUMENT_FAILED` | Adapter or projection failed for an event |
| `RequiredInstrumentMissing` | `REQUIRED_INSTRUMENT_MISSING` | Required instrument projection was None for a transition |
| `PanelShapeMismatch` | `PANEL_SHAPE_MISMATCH` | Vector dim drift in panel/action/predicted slots |
| `BindingEvidenceMissing` | `BINDING_EVIDENCE_MISSING` | Persisted binding ref points to nothing |
| `TrajectoryInvariantViolation` | `TRAJECTORY_INVARIANT_VIOLATION` | Trajectory invariant evaluation failed |
| `DatasetLeakageDetected` | `DATASET_LEAKAGE_DETECTED` | Multi-domain shard, missing train split, leakage-report nonzero, source-hash drift |
| `TrainingFailed` | `TRAINING_FAILED` | Convergence failure, NaN/Inf, CUDA error, missing artifact tensor, missing/uncalibrated surprise calibration |
| `ArtifactHashMismatch` | `ARTIFACT_HASH_MISMATCH` | Disk artifact divergence from registry |
| `PredictionInputMissing` | `PREDICTION_INPUT_MISSING` | Panel / action source row not found at predict/plan time |
| `GuardRejected` | `GUARD_REJECTED` | A persisted guard rejected reason was empty/invalid |
| `VerificationFailed` | `VERIFICATION_FAILED` | Deep verification harness failure |
| `StorageInvariantViolation` | `STORAGE_INVARIANT_VIOLATION` | Persisted row ≠ in-memory row after readback |
| `DomainPackNotFound` | `DOMAIN_PACK_NOT_FOUND` | Predict/plan referenced a domain not in `dj_domain_packs` |
| `DomainPackVersionMismatch` | `DOMAIN_PACK_VERSION_MISMATCH` | DomainPack `version` ≠ `header.domain_pack_version` |
| `SchemaValidationFailed` | `SCHEMA_VALIDATION_FAILED` | Schema-level rejection (e.g. unsupported `schema_version`) |
| `AdapterFailed` | `ADAPTER_FAILED` | Per-event adapter run failed |
| `BundleSchemaVersionMismatch` | `BUNDLE_SCHEMA_VERSION_MISMATCH` | Research-smoke bundle manifest schema mismatch |
| `PairwiseCosineKindMissing` | `PAIRWISE_COSINE_KIND_MISSING` | Instrument's `pair_kinds` lacks `Cosine` |
| `PairwiseUnsupportedKind` / `PairwiseAsymmetricOrdering` / `PairwiseRowCountMismatch` / `PairwiseCosineDrift` / `PairwiseInstrumentHashDrift` | `PAIRWISE_*` | Per-pair invariant failures |
| `ConstellationReferenceSetTooSmall` / `ConstellationZeroNormMajority` / `ConstellationLooBelowThreshold` / `ConstellationInstrumentHashMismatch` / `ConstellationSubjectFieldUndeclared` / `ConstellationSubjectValueUndeclared` | `CONSTELLATION_*` | Centroid build invariants |
| `CalibrationSetNotDisjoint` / `CalibrationSupersessionRequired` | `CALIBRATION_*` | Threshold calibration invariants |
| `GuardConstellationMissing` / `GuardThresholdMissing` / `GuardInstrumentHashDrift` / `GuardThresholdDuplicated` | `GUARD_*` | G_tau guard invariants |
| `SignalYieldUnknownOperation` / `SignalYieldTableDrift` / `SignalYieldEventCountMismatch` | `SIGNAL_YIELD_*` | Audit signal_yield invariants |
| `MiAuditSampleSizeTooSmall` / `MiAuditDegenerateInput` / `MiAuditBootstrapDegenerate` / `MiAuditMineNonconvergent` / `MiAuditOutputDirExists` / `MiAuditInstrumentHashDrift` | `MI_AUDIT_*` | KSG audit invariants |
| `BridgeActionMappingIncomplete` / `BridgeInstrumentCountDrift` / `BridgePredictorInputDimMismatch` | `BRIDGE_*` | Cross-domain transfer invariants |

`From<DynamicJepaError> for ContextGraphError` (`error.rs:397-410`) maps storage/codec/SourceOfTruthMissing/StorageInvariantViolation → `ContextGraphError::Storage(Corruption)`; `TrainingFailed` → `Internal`; everything else → `Validation`.

## 25. Source Notes & Lessons

- `schedule.device = "cuda"` is mandatory; `TrainConfig::validate` (`config.rs:209-217`) explicitly forbids any CPU fallback path. Phase 7's 5090 demo refuses to run on machines without compute_cap 12.0.
- `model.predictor.out_dim == model.encoder.out_dim` is asserted **only** when `target_architecture = ema_encoder`. Under `frozen_instrument_projection` the predictor output is in **panel space**, not latent space, since the loss compares against the raw target panel.
- AdamW updates **only** `online.encoder.params() ∪ predictor.params()`. Stop-gradient is applied via `.detach()` on `z_target`. EMA branch uses `set_from` at init and `ema_update` per step; frozen branch is a no-op.
- Action zeroing for `ignore_action = true` happens **per-batch in inference paths** and **once over the whole dataset in training** (`model/mod.rs:411-425`).
- Identity panel/negative pairs are forbidden (`validate_examples`, `model/mod.rs:836`): `target_panel == negative_panel` errors out so the contrast accuracy isn't inflated by leakage.
- Hash-verified inference: `load_artifact_for_inference` recomputes every artifact file hash on every inference (`artifact.rs:151`); mismatch refuses to instantiate `InferenceArtifact`.
- All persistence sites use **batch-write + readback verify**. Any divergence raises `StorageInvariantViolation` and aborts the operation — no partial state is exposed.
- `PlanTraceStatus::Rejected` is only produced when (a) every guard rejects every candidate AND (b) the domain has no constellation fallback. With a constellation, the highest-utility candidate is selected anyway and `no_accepted_candidate = true` is recorded for downstream auditors (`service.rs:533-549`).
- The **frozen-instrument-projection target** (commit 8ceb08e6) means there is no learnable target encoder for the SWE reality loop; surprise is measured in instrument-projection space, not learned latent space. This eliminates EMA collapse failure modes at the cost of requiring the predictor to map directly to instrument-projection dimensions.
- The **per-segment surprise calibration** (commit 7d3debdb) computes a separate `threshold_cosine` for every `action.<field>=<value>` segment present in the val split; the SWE-loop `record_surprise` path picks the segment by `tool_family` (with `path_risk` override for `workspace_mutation`), so the threshold tracks the action class rather than a single global value.
- The **artifact lifecycle gating** (commit efd50b20) means a trained DynamicJEPA artifact is **required** for the SWE reality loop's reward features; the ledger row is verified from SQLite (commit 420fc827) before the loop will use the artifact.

## 26. Relationship to the Reality Loop

The DynamicJEPA subsystem and the SWE-bench reality loop ([22_reality_loop_subsystem.md](22_reality_loop_subsystem.md)) are independent subsystems that share a single audit substrate (the BlackBoxFix SQLite ledger at `LEDGER_SCHEMA_VERSION = 14`):

| Subsystem | Owns | Persists into |
| --- | --- | --- |
| DynamicJEPA | Latent panels, predictions, planning, surprise, MI audit, transfer | 28 `dj_*` RocksDB CFs + `dj_audit_log` (per-attempt reality blocks for `contextgraph_swe_reality_loop_v1`) |
| Reality loop | SWE-bench task validation, candidate patches, official Docker readback, GPU VRAM budget | `blackbox.sqlite` (BlackBoxFix), `reality-events.jsonl`, per-attempt JSON artifacts |

Both subsystems write fail-closed audit rows for every persistence operation; both raise typed validation errors on schema or readback mismatch; neither can claim success without the other's source-of-truth confirmation in scope.

The reality-loop binary's structured tool-transition ingest path (`crates/context-graph-cli/src/bin/reality-loop.rs`) emits one `(state, action, outcome)` transition per inner-actor round into the SWE-loop domain pack at attempt-start (cold-start panel bootstrap, lines 5940-6016) and at every subsequent round (live transition ingest). The DynamicJEPA service's `predict` / `plan` / `record_surprise` are then invoked over those persisted panels using the trained SWE-loop artifact; the resulting trained-vs-projection cosine surprise (calibrated per action segment) feeds back into the reality-loop reward features that the outer optimizer reads on the next turn.
