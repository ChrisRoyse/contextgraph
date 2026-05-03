# DynamicJEPA Subsystem

**Source files covered:**
- `crates/context-graph-dynamicjepa/src/lib.rs`
- `crates/context-graph-dynamicjepa/src/config.rs` (TrainConfig)
- `crates/context-graph-dynamicjepa/src/model/mod.rs` (TinyMlpJepa, VICReg loss, EMA target encoder)
- `crates/context-graph-dynamicjepa/src/service.rs` (train / predict / plan / record_surprise / G_tau)
- `crates/context-graph-dynamicjepa/src/artifact.rs` (hash-verified safetensors load)
- `crates/context-graph-dynamicjepa/src/transfer.rs` (cross-domain transfer pilot)
- `crates/context-graph-dynamicjepa/src/mi_audit.rs` (KSG-1 pairwise MI)
- `crates/context-graph-core/src/dynamicjepa/**/*.rs` (record types, ids, schema, validation)
- `crates/context-graph-storage/src/dynamicjepa/**/*.rs` (28-CF RocksDB plane)
- `crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs`
- `crates/context-graph-mcp/src/tools/definitions/dynamicjepa.rs` (35 tools)

**Cross-links:** [04_database_schema.md](04_database_schema.md), [09_storage_layer.md](09_storage_layer.md), [11_cuda_acceleration.md](11_cuda_acceleration.md), [16_training_subsystem.md](16_training_subsystem.md), [17_constellation_subsystem.md](17_constellation_subsystem.md), [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md), [20_learning_subsystem.md](20_learning_subsystem.md). [01_system_overview.md](01_system_overview.md) should reference this doc in its subsystem index.

## 1. Scope
DynamicJEPA is a **Joint-Embedding Predictive Architecture** trained, predicted, planned, and audited entirely inside Rust on RTX 5090 / CUDA 13.2 via Candle 0.9.2-alpha. Its "Dynamic" variant takes a `(latent_panel, action)` pair and predicts the next latent panel. It is **independent** of:
- E1-E14 content embedders ([05_embedding_subsystem.md](05_embedding_subsystem.md))
- LLM / DPO style transfer ([19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md))
- Learner UTL state ([20_learning_subsystem.md](20_learning_subsystem.md))

Phase split (per `lib.rs:3-7`):
- **Phase 7**: training Candle MLP (online encoder + EMA target encoder + predictor), artifact writing, hash-checked safetensors load.
- **Phase 8**: hash-checked prediction, planning with guard policy, surprise persistence.

The subsystem owns **28 RocksDB column families** (all prefixed `dj_*`); none overlap with the teleological / content data plane.

## 2. Architecture
```
input panel x ──▶ online encoder f_θ ──▶ z_online
                                            │
                              concat(z, action)
                                            │
                                       predictor g_φ ──▶ ẑ
                                                          │
target panel x' ─▶ target encoder f_ξ (EMA, detached) ──▶ z_target
```

| Component | Type | Update rule |
| --- | --- | --- |
| Online encoder `f_θ` | MLP (ReLU between layers) | AdamW gradient |
| Target encoder `f_ξ` | MLP, same shape, **detached** | EMA: `ξ ← m·ξ + (1-m)·θ` |
| Predictor `g_φ` | MLP, input = latent ⊕ action | AdamW gradient |

EMA momentum `m ∈ [0, 1)` is required by `TrainConfig::validate` (`config.rs:131`).

## 3. Loss (Phase 7)
| Term | Formula | Default weight |
| --- | --- | --- |
| `latent_mse` | `‖ẑ − stop_grad(z_target)‖²` mean | `latent_mse_weight` |
| `vicreg_variance` | `mean(relu(target_std − sqrt(var(z_online) + 1e-4)))` | `vicreg_variance_weight` |
| `vicreg_covariance` | off-diagonal `‖cov(z_online)‖²_F / latent_dim` | `vicreg_covariance_weight` |
| **Total** | weighted sum of the three above | — |

`vicreg_target_std` is the per-dim variance floor.

## 4. TrainConfig Schema (`config.rs`)
| Section | Field | Type | Constraint |
| --- | --- | --- | --- |
| `model.encoder` | `kind` | string | must be `"mlp"` |
| | `hidden` | `Vec<usize>` | non-empty, no zero |
| | `out_dim` | `usize` | > 0 |
| `model.predictor` | `kind` | string | `"mlp"` |
| | `hidden` | `Vec<usize>` | non-empty, no zero |
| | `in_action_dim` | `usize` | matches batch action dim |
| | `out_dim` | `usize` | **must equal** `encoder.out_dim` |
| | `ignore_action` | bool | default false |
| `model.ema_momentum` | f64 | `[0.0, 1.0)` |
| `loss.*` | f64 | finite, ≥ 0 |
| `optim.kind` | string | must be `"adamw"` (Phase 7) |
| `optim.lr`, `weight_decay` | f64 | finite, ≥ 0 |
| `optim.warmup_steps` | u64 | linear warmup to `lr` |
| `schedule.epochs` | usize | > 0 |
| `schedule.batch_size` | usize | > 0 |
| `schedule.device` | string | **must be `"cuda"`**; CPU fallback explicitly forbidden |
| `stopping.metric` | string | `<split>_<metric>` form, e.g. `val_latent_mse` |
| `stopping.target` | f64 | finite, ≥ 0 |
| `stopping.max_seconds` | u64 | > 0 |
| `seed` | u64 | RNG seed (set on CUDA device) |

`TrainConfig::canonical_bytes()` produces the JSON used for `training_config_hash` (SHA-256).

## 5. Training Loop (`model/mod.rs::train_tiny_jepa`)
1. Validate every `TrainExample {input_panel, target_panel, action, negative_panel, split_name}`. Reject identical positive/negative pairs and any non-finite value.
2. Optionally zero out actions when `predictor.ignore_action = true`.
3. Initialize CUDA device 0 (panic if compute_cap ≠ 12.0); CUDA RNG seed; warm-up matmul + synchronize.
4. Build `TinyMlpJepa` (online + target + predictor). Target encoder copies online weights at init.
5. Compute baseline metrics (`evaluate_model` with no collapse diagnostics).
6. AdamW over `online.params() ∪ predictor.params()` (target encoder is **never** in the optimizer).
7. For each epoch:
   - Shuffle train indices with deterministic PRNG keyed by `(seed, epoch)`.
   - For each batch: forward online + target (detached) + predictor; compute MSE + VICReg variance + VICReg covariance; weighted sum; `opt.backward_step`; `model.ema_update(momentum)`; `device.synchronize()`.
   - Wall-clock guard: error out if `started.elapsed() > stopping.max_seconds`.
   - Re-evaluate; if `stopping_metric ≤ stopping.target`, break converged.
8. Final evaluation with collapse diagnostics. If not converged AND final stopping metric > target → `TrainingFailed`.

## 6. Collapse Diagnostics (final epoch only)
Computed in `evaluate_model` + `collapse_diagnostics` (`model/mod.rs:1173`):

| Metric | Definition |
| --- | --- |
| `val_var_min/p25/median/p75/max` | Quantiles of per-dim variance |
| `covar_frob` | Frobenius norm of off-diagonal covariance |
| `covar_rms` | RMS off-diagonal covariance |
| `covar_loss_scale` | `Σ off² / latent_dim` (matches VICReg loss scale) |
| `eff_rank` | `exp(H)` over normalized eigenvalue spectrum |
| `eff_rank_ratio` | `eff_rank / latent_dim` |
| `alignment` | Mean squared distance between L2-normalized prediction & target |
| `uniformity` | `log mean exp(-2·‖z_i − z_j‖²)` over pairs |
| `sigreg_kl` | Diagonal KL `Σ 0.5·(σ² + μ² − 1 − ln σ²)`; clamped at `1e12`, var floor `1e-12` |
| `latent_collapsed` | true when `var_min < 1e-8` OR `eff_rank_ratio < 0.10` OR any non-finite |

Eigenvalues use a Jacobi rotation routine (`jacobi_symmetric_eigenvalues`) with `max_iter = max(n²·32, 64)` and tolerance `1e-12`.

## 7. Evaluation Splits
For each split (`train`, `val`, `test`) the report contains:
- `latent_mse` — mean `‖pred − target‖²`
- `latent_cosine` — mean cos(pred, target)
- `action_contrast_acc` — fraction where `cos(pred, target) > cos(pred, negative)`
- `shuffled_target_cosine` — centered cosine vs negative; baseline check

The full `EvaluationReport` (`model/mod.rs:27`) also stores `random_init_baseline`, `shuffled_target_baseline`, every VICReg variance/covariance scalar, the `EpochMetric` history, and `parameter_count_trainable`.

## 8. Service API (`service.rs`)
| Function | Inputs | Persists |
| --- | --- | --- |
| `train_dataset` | `dataset_id`, `&TrainConfig`, `artifact_root` | `CF_DJ_TRAINING_RUNS` (Started → Completed/Failed), `CF_DJ_MODEL_ARTIFACTS`, `CF_DJ_AUDIT_LOG` + safetensors files |
| `predict_next_panel` | `artifact_id`, `panel_id`, `action_id` | `CF_DJ_PREDICTIONS` + audit |
| `plan_next_action` | `artifact_id`, `panel_id`, `skill_id` | `CF_DJ_ACTIONS` (candidates), `CF_DJ_PREDICTIONS`, `CF_DJ_GUARD_DECISIONS`, `CF_DJ_PLAN_TRACES`, optional `CF_DJ_SKILL_POLICIES`, audit |
| `record_surprise` | `prediction_id`, `observed_outcome_id`, `observed_panel_id` | `CF_DJ_SURPRISE_EVENTS` + audit (only when `cosine < threshold`) |
| `cross_domain_transfer` | `CrossDomainTransferConfig` | `transfer_results.json`, paper table, plots, bridge artifacts (filesystem) |

After every persistence step, the service performs an **immediate readback** and bincode equality check against the in-memory record; mismatch returns `StorageInvariantViolation`. Training runs that error after the started-row was persisted are flipped to `TrainingRunStatus::Failed { error }` before the error propagates.

## 9. Predict / Plan Mechanics
`predict_vector` (`service.rs:871`):
1. Build `action_dense_vector` from declared instrument specs (scalar normalized via `Normalization`, categorical → one-hot in declared variant order).
2. `flatten_panel_with_action_override(panel, Some(&action))` → input vector.
3. MLP forward through `online.encoder` (ReLU between layers, no activation on last) → `z`.
4. Concatenate `z ⊕ action` → MLP forward through `predictor`.
5. Reject any non-finite intermediate; bail with `TrainingFailed`.

`uncertainty = ‖predicted‖₂ / sqrt(predicted.len())`. Stored objective score is `-uncertainty`.

`plan_next_action` enumerates candidates per `domain.planner_policy.candidate_actions`:
- `EnumeratedDeltas { deltas }` — one i64 action field, e.g. counter_world `[-1, 0, 1]`.
- `EnumeratedMoves { moves }` — one categorical action field, e.g. gridworld `["up","down","left","right"]`.

For each candidate it predicts, computes utility, runs every guard in `planner_policy.guards`, then picks the highest-utility allowed action (or — when all are rejected and a constellation is registered — the highest-utility candidate). Resulting status: `PlanTraceStatus::Selected | Rejected`.

## 10. Guard System
Phase 3 supports `guards = ["bounds_check"]` only (`service.rs:1392`). `bounds_check` produces:
- A **utility decision** based on `domain.state_schema` per-field min/max applied to the **predicted next state** (deterministically derived via `predicted_state_after_action`).
- An optional **G_tau decision** when `domain.constellation` is set: project the panel/action into each modality, compare cosine-to-centroid against the calibrated threshold `tau`, and reject if any required modality fails.

`GuardDecisionRecord` (CF `CF_DJ_GUARD_DECISIONS`) stores both decisions, `cosine_to_centroid_per_modality`, `tau_per_modality`, `gtau_failed_modalities`, and the `constellation_uuid` used.

## 11. Surprise (`record_surprise`)
1. Re-encode the **observed** panel under the **same artifact** via the target encoder + zeroed action (or candidate action if `predictor.ignore_action = false`).
2. Compute `cosine(prediction.predicted_next_panel_vec, observed_vec)` and `error_norm = L2 distance`.
3. Threshold = `domain.planner_policy.surprise_threshold`.
4. **If `cosine ≥ threshold`** → no surprise persisted; return `wrote_surprise = false`.
5. **Else** → persist `SurpriseEventRecord { surprise_kind: UnexpectedOutcome, cosine, threshold, error_norm }` + audit.

## 12. Identifiers (`core/dynamicjepa/ids.rs`)
**String IDs** (validated by `validate_string_id`): `DomainPackId`, `InstrumentId`, `AdapterId`.
**UUID IDs**: `EventId`, `StateId`, `ActionId`, `OutcomeId`, `TransitionId`, `PanelId`, `BindingId`, `TrajectoryId`, `DatasetShardId`, `DatasetId`, `TrainingRunId`, `ModelArtifactId`, `PredictionId`, `SkillId`, `PlanTraceId`, `GuardDecisionId`, `SurpriseEventId`, `VerificationRunId`, `PairwiseReadingId`, `ConstellationId`, `ThresholdCalibrationId`.

## 13. Record Versions
| Constant | Value | File |
| --- | --- | --- |
| `DOMAIN_PACK_RECORD_VERSION` | 1 | `domain_pack.rs:13` |
| `INSTRUMENT_READING_RECORD_VERSION` | 1 | `instrument.rs:10` |
| `ADAPTER_RUN_RECORD_VERSION` | 1 | `adapter.rs:11` |
| `NORMALIZED_STATE_RECORD_VERSION` | 1 | `state_action_outcome.rs:8` |
| `NORMALIZED_ACTION_RECORD_VERSION` | 1 | `state_action_outcome.rs:9` |
| `NORMALIZED_OUTCOME_RECORD_VERSION` | 1 | `state_action_outcome.rs:10` |
| `LATENT_PANEL_RECORD_VERSION` | 2 | `panel.rs:10` |
| `BINDING_RECORD_VERSION` | 1 | `binding.rs:8` |
| `TRAJECTORY_RECORD_VERSION` | 1 | `trajectory.rs:8` |
| `DATASET_SHARD_RECORD_VERSION` | 2 | `dataset.rs:7` |
| `MODEL_ARTIFACT_RECORD_VERSION` | 1 | `artifact.rs:8` |
| `TRAINING_RUN_RECORD_VERSION` | 1 | (header) |
| `PREDICTION_RECORD_VERSION` | 1 | (header) |
| `SKILL_POLICY_RECORD_VERSION` | 1 | `skill.rs:7` |
| `PLAN_TRACE_RECORD_VERSION` | 2 | `planner.rs:11` |
| `SURPRISE_EVENT_RECORD_VERSION` | 1 | `planner.rs:12` |
| `GUARD_DECISION_RECORD_VERSION` | 2 | `guard.rs:9` |
| `PAIRWISE_READING_RECORD_VERSION` | 1 | `meaning_compression.rs:12` |
| `CONSTELLATION_CENTROID_RECORD_VERSION` | 1 | `meaning_compression.rs:13` |
| `THRESHOLD_CALIBRATION_RECORD_VERSION` | 1 | `meaning_compression.rs:14` |
| `VERIFICATION_RUN_RECORD_VERSION` | 1 | `verification.rs:9` |

Every record carries a `DjRecordHeader { id, version, domain_pack_id, domain_pack_version, created_at_unix_ms, run_id, content_hash }`. `refresh_content_hash()` is called before write; readback equality is enforced.

## 14. RocksDB Column Families (28 total, prefix `dj_`)
Source: `crates/context-graph-storage/src/dynamicjepa/column_families.rs`.

| CF | Role | CF options |
| --- | --- | --- |
| `dj_domain_packs` | Domain pack records by storage UUID | uuid_point |
| `dj_domain_pack_by_nv` | `(name, version) → uuid` | composite_index |
| `dj_instrument_registry` | Instrument spec ledger | composite_index |
| `dj_adapter_registry` | Adapter spec ledger | composite_index |
| `dj_raw_events` | Raw JSONL events (post-ingest) | uuid_point |
| `dj_normalized_states` | Adapter-output `NormalizedState` | uuid_point |
| `dj_actions` | Persisted `NormalizedAction` (observed + hypothetical) | uuid_point |
| `dj_outcomes` | Persisted `NormalizedOutcome` | uuid_point |
| `dj_transitions` | `(state, action, next_state, outcome)` tuples | uuid_point |
| `dj_adapter_runs` | Per-event adapter telemetry | append_only |
| `dj_instrument_readings` | Per-instrument projections of an event | composite_index |
| `dj_latent_panels` | `LatentPanel` records (large blobs) | large_blob |
| `dj_pairwise_readings` | Cosine pair readings between instruments | append_only |
| `dj_constellations` | DynamicJEPA constellation centroids | composite_index |
| `dj_threshold_calibrations` | G_tau thresholds | composite_index |
| `dj_bindings` | Source-of-truth bindings | uuid_point |
| `dj_bindings_by_entity` | Reverse index by entity | secondary_index |
| `dj_trajectories` | Compiled trajectories | uuid_point |
| `dj_dataset_shards` | One-step dataset rows (large) | large_blob |
| `dj_training_runs` | Run lifecycle (Started → Completed/Failed) | append_only |
| `dj_model_artifacts` | Artifact registry (paths + per-file hashes) | uuid_point |
| `dj_predictions` | Predictions + objective scores | append_only |
| `dj_skill_policies` | Skill policies (e.g. `EnumerateDeclaredActions`) | uuid_point |
| `dj_plan_traces` | Plan traces, candidate IDs, guards, selection | append_only |
| `dj_guard_decisions` | Per-guard outcomes (utility + G_tau) | append_only |
| `dj_surprise_events` | Persisted surprise events | append_only |
| `dj_verification_runs` | Calibration + audit verification runs | append_only |
| `dj_audit_log` | Universal audit (every operation) | append_only (8-byte prefix) |

CF options helpers: `dj_uuid_point_cf_options`, `dj_composite_index_cf_options`, `dj_secondary_index_cf_options`, `dj_append_only_cf_options`, `dj_large_blob_cf_options` (`column_families.rs:107-125`). All use Lz4 + level compaction; large_blob uses 64KB blocks + 8MB write buffer; uuid_point uses 8KB blocks + 4MB write buffer + 16-byte prefix bloom.

## 15. Domain Pack (`core/dynamicjepa/domain_pack.rs`)
A domain pack is a strict TOML registered into `dj_domain_packs`. Required fields:
- `state_schema`, `action_schema`, `outcome_schema`, `entity_schema`, `time_schema` (typed `FieldKind`: `I64`, `F64`, `Categorical { variants }`, etc.).
- `instrument_specs` — at least one; each requires `kind ∈ {Scalar, Categorical, Onehot, Vector, Time}`, declared `output_shape`, optional `Normalization::{None, StandardScore { mean, std }}`, validation bounds, and **must include** `PairKindName::Cosine` in `pair_kinds` (`instrument.rs:150`).
- `adapter_specs` — registered adapters that consume raw events.
- `objective_specs` — Phase 1 / 5090 demo accepts `kind = "latent_delta"` only; positive `loss_weight`; non-empty `required_dataset_fields`.
- `invariants` — `severity = "fatal"` only.
- `dataset_policy` — `negative_sampling = "same_domain_different_outcome"`, `min_negatives_per_row > 0`.
- `planner_policy` — `candidate_actions = EnumeratedDeltas | EnumeratedMoves`, `guards`, `surprise_threshold`.
- `constellation` (optional) — `ConstellationConfig { global, subjects }` with `ConstellationModalities = All | List(Vec<InstrumentId>)`.
- `verification_policy` — required CFs, expected counts.
- `source_hash: [u8; 32]` — SHA-256 of TOML source.

## 16. Demo Domains
| Domain | ID | Action shape | Default surprise threshold |
| --- | --- | --- | --- |
| Counter world | `counter_world` (1.0.0) | `EnumeratedDeltas { deltas: [-1, 0, 1] }` | configurable |
| Gridworld 5×5 | `gridworld_5x5` (1.0.0) | `EnumeratedMoves { moves: ["up","down","left","right"] }` | configurable |
| Counter→Grid bridge | `COUNTER_TO_GRID_DOMAIN_ID` (1.0.0) | bridge instruments tied via `counter_to_grid_*` helpers | — |

## 17. Cross-Domain Transfer Pilot (`transfer.rs`)
| Constant | Value |
| --- | --- |
| `TRANSFER_DEFAULT_SEEDS` | `[42, 43, 44, 45, 46]` |
| `TRANSFER_DEFAULT_BOOTSTRAP_ITERS` | 10000 |
| `BRIDGE_EFFECT_BASIS_DIM` | 3 |
| `POSITION_MEAN, POSITION_STD` | 12.0, 12.0 |
| `SHUFFLED_TARGET_PERMUTATIONS` | 32 |

`CrossDomainTransferConfig` validators:
- `output_root` must not exist (immutable evidence).
- ≥ 1 unique seed.
- `source_events ≥ 20`, `target_events ≥ 20`.
- `bootstrap_iters > 0`.
- All training schedule fields > 0.
- `learning_rate > 0` finite, `stopping_target > 0` finite.
- `validate_counter_to_grid_bridge_instrument_count(COUNTER_TO_GRID_BRIDGE_INSTRUMENT_IDS.len())` enforces bridge instrument count.

Phase 1: train a counter-world bridge predictor per seed. Phase 2: evaluate on gridworld bridge events with paired bootstrap CIs. Outputs include `transfer_results.json`, paper table, plots, and bridge artifacts.

## 18. Pairwise Mutual-Information Audit (`mi_audit.rs`)
| Constant | Value |
| --- | --- |
| `MI_AUDIT_MIN_SAMPLE_SIZE` | 50 |
| `MI_AUDIT_LOOSE_THRESHOLD` | 0.7 |
| `MI_AUDIT_STRICT_THRESHOLD` | 0.9 |

KSG-1 estimator (`estimate_ksg_1`):
1. Validate sample size and finite values per pair (`xs`, `ys`).
2. Compute local MI contributions from k-nearest-neighbor distances.
3. `mi_nats = mean(local_mi_nats)`; reject non-finite.
4. `x_entropy = empirical_entropy_nats(xs)`, `y_entropy = empirical_entropy_nats(ys)`.
5. `normalizer = min(x_entropy, y_entropy)`; reject ≤ 0 or non-finite.
6. `mi_normalised = (max(mi_nats, 0) / normalizer).clamp(0, 1)`.

`PairwiseMiSummary` contains `n_redundant_pairs_loose/strict` (count of pairs over each threshold) and an effective panel-slot count. PRNG is `SplitMix64`.

## 19. MCP Tools (35 total, `definitions/dynamicjepa.rs`)
Source-of-truth name constants live in `tools/names.rs:275-343`.

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

### Trajectory & dataset (5)
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

### Inference & planning (5)
| Tool | Purpose |
| --- | --- |
| `dynamicjepa_predict` | Persist hash-verified prediction |
| `dynamicjepa_plan` | Persist plan trace + candidates + predictions + guards |
| `dynamicjepa_record_surprise` | Persist surprise when threshold breached |
| `dynamicjepa_get_prediction` | Read one prediction |
| `dynamicjepa_get_plan_trace` | Read plan trace; toggle `includePredictions`/`includeGuards` |
| `dynamicjepa_get_surprise` | Read surprise event |

### Constellation & threshold (4)
| Tool | Purpose |
| --- | --- |
| `dynamicjepa_build_constellation` | Build immutable centroids from instrument readings |
| `dynamicjepa_list_constellations` | Filter by domain/subject |
| `dynamicjepa_get_constellation` | One centroid |
| `dynamicjepa_calibrate_threshold` | Calibrate G_tau thresholds from disjoint held-out events |
| `dynamicjepa_recalibrate_threshold` | Supersede prior threshold (requires `supersedes` + `reason`) |

### Audits & evidence (4)
| Tool | Purpose |
| --- | --- |
| `dynamicjepa_compute_mc_ratio` | Aggregate audit-log signal_yield rows → MC-ratio paper evidence |
| `dynamicjepa_audit_pairwise_mi` | KSG-1 pairwise MI audit; sample size 50-1000; bootstrap CIs |
| `dynamicjepa_cross_domain_transfer` | counter_world → gridworld pilot |
| `dynamicjepa_inspect_counts` | Row counts for every `dj_*` CF |
| `dynamicjepa_inspect_cf` | Decode rows from one CF (or one exact key by `keyHex`) |

All tool schemas use `additionalProperties: false`. Common path/limit helpers (`db_path`, `path_prop`, `string_prop`, `string_default`, `bool_default`, `number_default`, `integer_min`, `integer_default`, `integer_array_default`, `number_positive_default`, `limit`, `offset`) are private to `definitions/dynamicjepa.rs`.

## 20. Artifact Layout (`artifact.rs`)
Per training run, the artifact root is `<base_root>/<domain_id>/<training_run_id>` and **must not exist before** the run (`ensure_clean_run_dir` errors if it does). The directory contains:
- `model.safetensors` — full tensor map (online + target encoders + predictor; `target.encoder.layerN.weight` and `online.encoder.layerN.weight` are stored separately to enable inference-time EMA snapshot).
- `config.json` — canonical `TrainConfig` JSON.
- `evaluation_report.json` — full `EvaluationReport`.

`ModelArtifactRecord.files: Vec<ArtifactFile>` stores `relative_path`, `sha256: [u8; 32]`, `size_bytes` for every file. `verify_artifact_files` recomputes hashes and refuses any mismatch (`ArtifactHashMismatch` error). `load_artifact_for_inference` rejects on corrupt safetensors and stores `model_file_sha256` for inclusion in `PredictionRecord.model_artifact_hash_at_inference`.

## 21. Audit Log
Every operation writes a `DjAuditRecord` to `dj_audit_log` with:
- `operation` (e.g. `train_predictor`, `register_artifact`, `predict`, `plan`, `record_surprise`)
- `status: AuditStatus::{Ok, Failed { error_code }}`
- `inputs: Vec<String>` — string-form IDs (uuids/dataset ids)
- `outputs: Vec<String>`
- `touched_cfs: Vec<String>`
- `content_hashes: Vec<[u8; 32]>` — every refreshed record hash
- `signal_yield: SignalYieldDimensions` — set via `signal_yield_for_operation` (`audit.rs`); used by `compute_mc_ratio`

Failed runs are also audited: `service.rs:269-289` writes a `Failed` row even when the original `Started` row was already persisted, ensuring every state transition is in the log.

## 22. Storage Helpers (selected)
- `put_training_run_with_audit_batch`, `put_model_artifact_completion_batch` — atomic write of training-run row + artifact row + audit row in a single RocksDB batch.
- `put_plan_batch` — atomic write of skill policy (optional), candidate actions, predictions, guard decisions, plan trace, audit.
- `put_prediction_with_audit_batch`, `put_surprise_event_with_audit_batch` — single-row + audit.
- `signal_yield_for_operation(op, dims) → SignalYield` — produces `signal_yield` for the audit row used by paper evidence aggregation.
- `domain_pack_storage_uuid(id, version)` — deterministic UUID derived from `(id, version)` strings.

## 23. Error Surface
`DynamicJepaError` (`core/dynamicjepa/error.rs`) is the universal result type. Notable variants:
- `Validation { field, message, remediation }` — schema/config rejection.
- `Storage { operation, cf, message, remediation }` — RocksDB / filesystem.
- `SourceOfTruthMissing { cf, key }` — readback after persistence found nothing.
- `StorageInvariantViolation { message }` — persisted row ≠ in-memory row.
- `TrainingFailed { training_run_id, message, remediation }` — model error; non-finite output; CUDA failure.
- `PanelShapeMismatch { domain_pack_id, expected, actual }` — vector dim drift.
- `ArtifactHashMismatch { artifact_id, file, expected, actual }` — disk artifact divergence.
- `DomainPackNotFound`, `PredictionInputMissing`, `MiAuditDegenerateInput`, `GuardConstellationMissing`, `GuardThresholdMissing`, `GuardInstrumentHashDrift`, `PairwiseCosineKindMissing`.

Every error carries a `code()` string used by audit logs.

## 24. Source Notes & Lessons
- `schedule.device = "cuda"` is mandatory; `TrainConfig::validate` (`config.rs:178`) explicitly forbids any CPU fallback path. Phase 7's 5090 demo refuses to run on machines without compute_cap 12.0.
- `model.predictor.out_dim == model.encoder.out_dim` is asserted at config validation time — the predictor output is in latent space, not panel space, since the loss compares against the **target encoder** output, not raw target panels.
- AdamW updates **only** `online.encoder.params() ∪ predictor.params()`. The target encoder is updated via EMA (`set_from` at init, `ema_update` per step). Stop-gradient is applied via `.detach()` on `z_target`.
- Action zeroing for `ignore_action = true` happens **per-batch in inference paths** and **once over the whole dataset in training** (`model/mod.rs:356`).
- Identity panel/negative pairs are forbidden (`validate_examples`, line 720): `target_panel == negative_panel` errors out so the contrast accuracy isn't inflated by leakage.
- Hash-verified inference: `load_artifact_for_inference` recomputes every artifact file hash on every inference; mismatch refuses to instantiate `InferenceArtifact`.
- All persistence sites use **batch-write + readback verify**. Any divergence raises `StorageInvariantViolation` and aborts the operation — no partial state is exposed.
- `PlanTraceStatus::Rejected` is only produced when (a) every guard rejects every candidate AND (b) the domain has no constellation fallback. With a constellation, the highest-utility candidate is selected anyway and `no_accepted_candidate = true` is recorded for downstream auditors.
