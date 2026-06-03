# 03 — Storage Schema and Policy

**Source files:** `crates/context-graph-mejepa-cf/src/lib.rs` (canonical ME-JEPA CF registry), `crates/context-graph-storage/src/lib.rs`, `crates/context-graph-storage/src/teleological/`, `crates/context-graph-mejepa-instruments/src/panel_store.rs`, `CLAUDE.md §8`.

---

## 1. Storage backend

Single RocksDB instance per runtime root. CFs are opened deterministically at daemon startup. Schema change = wipe-and-rebuild (no migrations, per `CLAUDE.md §6.6`). Durable rows are content-addressed by SHA256 where appropriate; provenance metadata is JSON.

- Opener: `crates/context-graph-storage/src/lib.rs`. Daemon opens every CF in `ALL_HYGIENE_REFERENCED_CFS` (`mejepa-cf/lib.rs:471`).
- Compaction: per-CF `Options` (`column_families.rs::*_options`).
- Total live CFs: `MEJEPA_HYGIENE_REFERENCED_CF_COUNT` ME-JEPA + 19 teleological + 13 quantized + 5 code + 2 causal + system CFs ≈ 200+.

---

## 2. ME-JEPA column families (canonical registry)

**SoT:** `crates/context-graph-mejepa-cf/src/lib.rs`. Every constant ident equals its on-disk string (enforced by `flywheel_cf_names_match_const_idents`, `fingerprint_cf_names_match_const_idents`).

### 2.1 Self-healing — `SELF_HEALING_CFS` + extensions

| CF | Purpose |
|---|---|
| `CF_MEJEPA_FISHER_SNAPSHOTS` | EWC++ Fisher diagonal snapshots per training window |
| `CF_MEJEPA_DRIFT_WINDOW` | Active rolling drift window |
| `CF_MEJEPA_DRIFT_HISTORY` | Per-window drift summary |
| `CF_MEJEPA_HEAL_REPORTS` | Heal scheduler decision rows |
| `CF_MEJEPA_DISTILL_STEPS` | Per-distillation-step rows |
| `CF_MEJEPA_ACTIVE_POINTERS` | Active model pointer (which checkpoint serves) |
| `CF_MEJEPA_CALIBRATION_HISTORY` | Per-window calibration history |
| `CF_MEJEPA_PLASTICITY_HISTORY` | Continual backprop dormancy history |
| `CF_MEJEPA_WEIGHT_BLOBS` | Model weight blobs |
| `CF_MEJEPA_SHIFT_WATERMARK` | Shift-log durability gate |
| `CF_MEJEPA_TRAIN_CERTS` | Training certificates (incl. CPΦ `conditional_description_length_bits`) |

### 2.2 Inference / live predictions — `INFER_CFS`

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_LIVE_PREDICTIONS` | `(session_id, prediction_id)` | JSON `RealityPrediction` | Primary Q1-Q5 output |
| `CF_MEJEPA_LIVE_SESSION_TRACES` | `(session_hex32, ts_ns)` | JSON shift trace | End-to-end session trace |
| `CF_MEJEPA_HIERARCHICAL_PREDICTIONS` | `(prediction_id, level)` | JSON | File/function/AST/chunk hierarchy |
| `CF_MEJEPA_ORACLE_VERDICTS` | `(prediction/test binding key)` | bincode `CalibrationExample` | Runtime oracle-binding calibration examples |
| `CF_MEJEPA_PREDICTION_VERIFICATIONS` | `(prediction_id, verifier_id)` | JSON | PostToolUse capture + manual verifier bindings |
| `CF_MEJEPA_REALITY_IMPACT` | `prediction_id` | bincode `RealityImpactRecord` | Q5 shift-log replay rows |
| `CF_MEJEPA_Q5_CALIBRATIONS` | `(calibration_id, language, mutation_category, side_effect_kind)` | bincode `Q5CalibrationRecord` | Q5 replay calibration |
| `CF_MEJEPA_CLAIM_RECONCILIATION` | `(prediction_id, claim_id)` | JSON | Q1 claim graph |
| `CF_MEJEPA_PROJECT_REPORTS` | `project_id` | JSON `ProjectReport` | Per-project reality report |
| `CF_MEJEPA_STRESS_RUNS` | `(run_id, case_id)` | JSON | End-to-end stress harness |
| `CF_MEJEPA_SYNTHETIC_STRESS_RESULTS` | `(run_id, case_id)` | JSON | Synthetic 30-case stress |
| `CF_MEJEPA_PANELS` | `panel_id` | JSON-encoded `Panel` (array of 15 per-embedder vectors serialized as 5,120 f32 contiguous buffer + per-slot offsets + `filled_mask`) | Materialized panels |
| `CF_MEJEPA_PANEL_GRAPHS` | `panel_id` | JSON-encoded `PanelGraphEnvelope` (canonical chunk nodes, typed chunk edges, dependency-subgraph SHA, root chunk, doctrine policy flags) | BLINDSPOT-004 / #726 variable-arity multi-chunk panel rows |
| `CF_MEJEPA_PANEL_CACHE` / `CF_MEJEPA_PANEL_META` | — | — | Hot panel cache + metadata |
| `CF_MEJEPA_AUX_SIGNALS_OVERRIDES` | `(panel_id, slot)` | JSON | Operator override slot values |
| `CF_MEJEPA_EVAL_REPORTS` | `(eval_run_id, cell)` | JSON | Holdout/ship-gate eval rows |
| `CF_MEJEPA_ABLATION_REPORTS` | `(run_id, action_class)` | JSON | Negative-action ablation reports |
| `CF_MEJEPA_TASK_GRAPH` / `CF_MEJEPA_TASK_GRAPH_HISTORY` | `task_id` / `(task_id, ts_ns)` | JSON | Task DAG + mutation log |
| `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` | `(priority, entry_id)` | JSON `ActiveLearningQueueEntry` | Active learning queue (incl. `curiosity_score`) |
| `CF_MEJEPA_ACTIVE_LEARNING_LABELS` / `CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS` | `entry_id` | JSON | Operator labels + eviction audit |
| `CF_MEJEPA_OOD_ESCALATIONS` | `(prediction_id, ts_ns)` | JSON | OOD escalation log |
| `CF_MEJEPA_CURIOSITY_TELEMETRY` | `(window_id, kind)` | JSON | Per-window curiosity distributions |
| `CF_MEJEPA_MODEL_PROMOTIONS` | `(model_id, ts_ns)` | JSON | Promotion log |
| `CF_MEJEPA_PRODUCTION_TELEMETRY` | `(ts_ns, kind)` | JSON | Production telemetry |
| `CF_MEJEPA_DDA_SIGNALS` | `(chunk_id, signal_class)` | JSON | DDA feature vector |
| `CF_MEJEPA_HEAD_PROJECTION_REGISTRY` | `head_id` | JSON | Per-head slice map + schema version |
| `CF_MEJEPA_FAILURE_EXEMPLARS` | `exemplar_id` | JSON | Closest-failure exemplars |
| `CF_MEJEPA_AGENT_FEEDBACK` | `(session_id, ts_ns)` | JSON | Agent feedback events |
| `CF_MEJEPA_OPERATOR_OVERRIDES` | `prediction_id` | JSON | Operator override records |
| `CF_MEJEPA_OPERATOR_CONTRIBUTIONS` | `contribution_id` | JSON | Operator contribution events |
| `CF_MEJEPA_LABEL_TRANSFER_DECISIONS` | `(source_label_id, target_label_id)` | JSON | Label transfer audit |
| `CF_MEJEPA_PARK_LIST` | `prediction_id` | JSON | Fail-closed retry park list |
| `CF_MEJEPA_COLD_CELL_METRICS` | `cell_key` | JSON | Cold cells (`n_supporting < 50`) |
| `CF_MEJEPA_HEAD_CALIBRATIONS` | `(head, class)` | JSON | Per-head calibration / producer-heldout row |
| `CF_MEJEPA_CONTRADICTION_THRESHOLDS` | `cell_id` | bincode `ContradictionThresholds` | Per-cell `(tau_oracle, tau_failure_count)` |
| `CF_MEJEPA_CONSTELLATION_PATTERNS` | `pattern_id` | bincode `ConstellationRelationshipPattern` | Promoted slot-pair patterns |
| `CF_MEJEPA_REPLAY_BUFFER` | `(priority, entry_id)` | JSON | Surprise-prioritized M(t) replay |
| `CF_MEJEPA_SAMPLER_REWARDS` | `(ts_ns, kind)` | JSON | Online observe reward |
| `CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT` | `(ts_ns, cluster_id)` | JSON | Novel-cluster audit |
| `CF_MEJEPA_REDACTION_LOG` | `(chunk_id, ts_ns)` | JSON | Source-chunk secret-redaction audit |
| `CF_MEJEPA_SIGNAL_DROP_LOG` | `(ts_ns, source)` | JSON | Fail-closed signal-drop events |
| `CF_MEJEPA_OOD_HARVEST` | `(harvest_run_id, prediction_id)` | JSON | Live-session OOD harvest rows |
| `CF_MEJEPA_OOD_CALIBRATIONS` | `(generated_at_unix_ms, calibration_id)` | bincode `OodCalibrationReport` | Serving calibrator state |

**Corpus oracle store separation.** The 300×8 corpus generator uses a separate corpus RocksDB store with lowercase CFs `mejepa_mutation_corpus`, `mejepa_oracle_verdicts`, `mejepa_corpus_provenance`. Rows are JSON `MutationOutcome` / `OracleVerdict` source rows and are NOT proof that canonical runtime/gate CFs are complete. Inspectors must report corpus-store completeness separately from canonical gate-countable completeness.

### 2.3 Failure-mode labeling — `FAILURE_MODE_CFS` (shipped #404 / #414)

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_FAILURE_MODE_VOCABULARY` | `vocabulary_id` | bincode | Operator-curated failure-mode vocabulary |
| `CF_MEJEPA_FAILURE_MODE_LEVEL1` | `group_id` | bincode | Level-1 groups (8) |
| `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS` | `skill_id` | bincode | Level-2 named skills (157) |
| `CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS` | `constellation_id` | bincode | Level-3 behavioral consequence constellations (379) |
| `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` | `(chunk_id, code_state_key, skill_id)` | bincode | Many-to-many chunk → skill rows (36,773 + 3,169 Level-3) |
| `CF_MEJEPA_SKILL_REVERSE_INDEX` | `(skill_id, code_state_key)` | bincode | Skill → chunks lookup (10,579) |
| `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT` | `skill_audit_id` | bincode | Append-only audit of update/split/create/demote/no-change (128) |
| `CF_MEJEPA_NAMED_PREDICTIONS` | `prediction_id` | bincode `NamedFailureMode` | Per-prediction named-failure-mode attachment (536) |

Level-0 deterministic labels are JSONL files (not CFs) under `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/`:

| Artifact | Rows |
|---|---|
| `chunk_constellation_labels.jsonl` | 399,777 |
| `file_path_constellations.jsonl` | 4,224 |
| `code_state_constellations.jsonl` | 3,000 |
| `failure_evidence_sets.jsonl` | 1,139 |
| `label_usefulness_metrics.jsonl` | 549 |
| `accepted_label_registry.jsonl` | 491 |
| `learning_bridge_manifest.json` | 1 |

### 2.4 Mistake-driven online learning (shipped #406 / #523)

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_MISTAKE_LOG` | `mistake_id` | bincode `MistakeLogRow` | Append-only log of disagreements |
| `CF_MEJEPA_MISTAKE_REPLAY_PRIORITY` | `mistake_id` | bincode `{priority, boost_factor=8.0, scheduled_at, replayed_at}` | Replay-buffer priority overlay |
| `CF_MEJEPA_ONLINE_HEAD_STATE` | `online_head:<panel_signature_hash>` | bincode `OnlineHeadStateRow` | Key-local exact-panel online-head state |
| `CF_MEJEPA_ONLINE_HEAD_STATE` | `mistake_repeat:<cell+label+skill+ability+membership hash>` | bincode `OnlineHeadRepeatMetricRow` | Byte-readable mistake-repeat metric |

Current protection mode: `exactPanelKeyIsolation`. `claims_fisher_ewc_protection=false`. #524 guards future Fisher/EWC claims (require durable Fisher/EWC rows).

### 2.5 Constellation / TCT — `TCT_CFS`

| CF | Purpose |
|---|---|
| `CF_MEJEPA_CONSTELLATION` | Per-cell calibrated centroids + thresholds |
| `CF_MEJEPA_GUARD_DECISIONS` | Gτ guard decision log |
| `CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS` | Refresh report |
| `CF_MEJEPA_CONSTELLATION_REFRESH_LOG` | Per-cell freshness scheduler log |

### 2.6 Fingerprints — `FINGERPRINT_CFS`

| CF | Purpose |
|---|---|
| `CF_MEJEPA_FAILURE_FINGERPRINTS` | First-class failure-shape catalog |
| `CF_MEJEPA_FINGERPRINT_REFERENCES` / `CF_MEJEPA_FINGERPRINT_REVERSE_INDEX` | Fingerprint ↔ exemplar |
| `CF_MEJEPA_FINGERPRINT_CALIBRATION` | Per-fingerprint conformal scores |
| `CF_MEJEPA_FINGERPRINT_AUDIT` | Lifecycle audit |
| `CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS` | 4-window per-fingerprint ship-gate |
| `CF_MEJEPA_FINGERPRINT_FISHER` / `CF_MEJEPA_FINGERPRINT_DORMANCY` | Per-fingerprint EWC + CBP |

### 2.7 Adversarial + explanatory knowledge

| CF | Purpose |
|---|---|
| `CF_MEJEPA_ADVERSARIAL_CORPUS` | 30-case adversarial Python rows |
| `CF_MEJEPA_MINCUT_REPORTS` | Stoer-Wagner mincut reports |
| `CF_MEJEPA_PAIRWISE_MI` | Pairwise MI matrix rows |
| `CF_MEJEPA_PATCH_SIMILARITY_META` | Patch-similarity HNSW index metadata |
| `CF_MEJEPA_INSTRUMENT_PROPOSALS` | Absence-as-existence proposal queue |
| `CF_MEJEPA_EMBEDDER_PROPOSALS` / `CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS` | Ranked queue + falsified candidates |
| `CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY` / `CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE` | Runtime hot-swap registry |
| `CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH` / `CF_MEJEPA_CHUNK_FOUNDATIONALITY` | PageRank chunk |
| `CF_MEJEPA_LIBRARY_REGISTRY` / `CF_MEJEPA_LIBRARY_FOUNDATIONALITY` / `CF_MEJEPA_CROSS_LIBRARY_REFERENCES` | PageRank library |
| `CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH` / `CF_MEJEPA_EMBEDDER_FOUNDATIONALITY` | PageRank embedder |

### 2.8 Hygiene + OOD harvest

| CF | Purpose |
|---|---|
| `CF_MEJEPA_MUTATION_CORPUS` / `CF_MEJEPA_LIVE_ATTEMPT_CORPUS` | Mutation / live-attempt corpus row stores |
| `CF_MEJEPA_QUOTA_STATE` | Quota state |
| `CF_MEJEPA_WITNESS_CHAIN` / `CF_MEJEPA_WITNESS_SEGMENT_META` | Witness chain segments |
| `CF_MEJEPA_PANEL_DISK_OVERFLOW` | Panel overflow to disk |
| `CF_MEJEPA_GC_HISTORY` | GC report history |

### 2.9 Q4 frozen CFs (deprecated-readable; not deleted)

Q4 prediction track closed under the ambiguity boundary (#408). CFs remain readable for forensic continuity:

| Frozen CF | Final state |
|---|---|
| `CF_MEJEPA_Q4_PERF_LABELS` | 260 rows frozen |
| `CF_MEJEPA_Q4_ACCURACY_LABELS` | 240 rows frozen |
| `CF_MEJEPA_Q4_COST_LABELS` | 240 rows frozen |
| `CF_MEJEPA_Q4_REASONING_LABELS` | 300 rows frozen |
| `CF_MEJEPA_Q4_PASS_ANNOTATIONS` | 500 rows frozen |
| `CF_MEJEPA_Q4_SECURITY_LABELS` | Frozen |
| `CF_MEJEPA_HEAD_CALIBRATIONS:q4cal::*` | Q4 sub-rows frozen (non-Q4 rows remain active) |

`FROZEN_Q4_CFS` in `mejepa-cf/src/lib.rs` is the weekly row-count-delta watch list. Baseline at `/zfs/archive/contextgraph/state/q4-freeze/q4_frozen_cf_counts.json`.

### 2.10 Canonical opener slice

`ALL_HYGIENE_REFERENCED_CFS` (`mejepa-cf/lib.rs:471-569`) is the single canonical opener list. Drift between this list and the grouped union is rejected by the `canonical_hygiene_cfs_match_group_union` test.

---

## 3. Teleological column families (19)

`crates/context-graph-storage/src/teleological/`. `TELEOLOGICAL_CF_COUNT = 19`.

| CF | Purpose |
|---|---|
| `CF_FINGERPRINTS` / `CF_FINGERPRINTS_LEARNER` | `TeleologicalFingerprint` rows |
| `CF_E1_MATRYOSHKA_128` | E1 (e5-large-v2) 128-d projections |
| `CF_E13_SPLADE_INVERTED` | E13 SPLADE inverted index |
| `CF_TOPIC_PROFILES` / `CF_GOAL_CENTROIDS` | Topic + goal centroid rows |
| `CF_LEARNER_*` (9 CFs) | Per-learner state, audit, K-sleep, M-per-trace, profile, retrieval log, state history, goal states, training datasets, constellations, delta log |
| `CF_LEARNING_EVENTS` | Learning event journal |
| `CF_MERGE_HISTORY` / `CF_IMPORTANCE_HISTORY` | Lifecycle provenance |
| `CF_AUDIT_LOG` / `CF_AUDIT_BY_TARGET` | Append-only audit log |
| `CF_CUSTOM_WEIGHT_PROFILES` | Custom weight profiles |
| `CF_ENTITY_PROVENANCE` | Entity provenance |
| `CF_TOOL_CALL_INDEX` | tool_use_id → memory mapping |
| `CF_CONSOLIDATION_RECOMMENDATIONS` | Consolidation hints |
| `CF_EMBEDDING_REGISTRY` | Embedder version tracking |

---

## 4. Other storage CF groups

| Group | CFs |
|---|---|
| **Quantized embedders** | `CF_EMB_0` … `CF_EMB_12` (13 CFs) |
| **Code** | `CF_CODE_ENTITIES`, `CF_CODE_E7_EMBEDDINGS`, `CF_CODE_FILE_INDEX`, `CF_CODE_NAME_INDEX`, `CF_CODE_SIGNATURE_INDEX` |
| **Causal** | `causal_relationships`, `causal_by_source` |
| **Edges** | K-NN edges + typed edges + secondary index |
| **DynamicJEPA** | Separate dataplane CFs (~15) |

Dimension constants: `E1_DIM`, `E1_MATRYOSHKA_DIM`, `E2_DIM`, …, `E13_SPLADE_VOCAB`, `NUM_EMBEDDERS` — exported from `crates/context-graph-storage/src/lib.rs:146-161`.

---

## 5. Storage policy

| Tier | Path | Purpose | Retention |
|---|---|---|---|
| **Production durable SoT** | `/zfs/archive/contextgraph/` (aiwonder) | All production: corpus, FSV, runtime shift logs, test outcomes, project-ingest outputs, weekly exports, durable state | Multi-month; never auto-pruned |
| **Production hot** | `/zfs/hot/contextgraph/` (aiwonder) | Hot work: repo checkout, model weights, stable hot inputs, active scheduler logs | Days to weeks |
| **Aiwonder scratch** | Explicitly budgeted dirs | Issue-scoped smokes | Per-issue |
| **Ephemeral** | `/tmp/contextgraph-*` | Test scratch | Process lifetime |

### 5.1 Aiwonder production directory layout

```
/zfs/archive/contextgraph/
├── corpus/
│   ├── python-ast-chunks-v1/
│   │   ├── python_ast_chunks.jsonl              # 399,777 chunks
│   │   └── source-files-300x8-20260515T013205Z/ # 4,224 files / 148,346,301 bytes (canonical AST source)
│   ├── python-forward-cache-v1-aiwonder-prod-<TS>/  # legacy v1 cache (retained as evidence; not clean ship substrate)
│   ├── python-forward-cache-v2-static-ast-<TS>/     # #715 v2 9-slot static cache (rebuild pending)
│   ├── frozen-instrument-registry/
│   ├── constellation-tensors-v1/                # #388 cross-embedder tensors
│   ├── python-q4-*-labels-v1/                   # frozen Q4 producer raw outputs
│   └── swebench-lite-python-300x8-v1-<TS>/index.json
├── fsv/                                         # production FSV target
├── runtime/
│   ├── shift-log.jsonl                          # JSONL shift log (tailed by shift-subscriber)
│   ├── test-outcomes/<hash>.jsonl               # PostToolUse capture per session
│   ├── posttooluse-mejepa-capture.jsonl
│   ├── observe-ticker.jsonl
│   ├── scheduler_events.jsonl
│   └── agent-history/                           # Q1 follow-up: agent response bodies
├── projects/<project_id>/
│   ├── manifest.json
│   ├── cache/
│   ├── predictions/
│   └── report/{report.json, report.md}
├── state/                                       # durable runtime state surviving restarts
│   ├── calibration/per-slot-v1/                 # #389 per-slot calibration + OOD policy
│   ├── labels/task-py-g-117-auto-label-discovery-v1/   # #413 Level-0 label artifacts
│   ├── labels/task-py-g-136-per-embedder-geometry-labels-v1/  # #657 geometry labels (planned)
│   ├── forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1/  # #614 mask
│   ├── regeneration-plans/task-py-g-135c-delete-test-call-regeneration-v1/  # #615 plan
│   ├── mejepa-heads/reasoning-head-v1/          # frozen reasoning checkpoint
│   ├── q4-trust-gate/evidence-catalog.json      # Q4 freeze baseline
│   └── q4-freeze/q4_frozen_cf_counts.json
└── exports/
    └── weekly/<YYYY-MM-DD>/                     # weekly.md, ablation_report.md, compression_progress.md
```

### 5.2 Aiwonder hot layout

```
/zfs/hot/contextgraph/
├── repo/                                        # aiwonder git checkout + release binaries
├── models/                                      # Embedder model artifacts
│   └── .cache/huggingface/hub/datasets--princeton-nlp--SWE-bench_Lite/
├── corpus/                                      # Hot inputs (chunks.jsonl, stable substrate)
└── runtime/                                     # Active scheduler logs / hot run state
```

### 5.3 Path discipline (behavioral)

`CLAUDE.md §7` — these paths must not be edited (no MCP-server-side gate; honor behaviorally):

- `.env` and `.env.*` anywhere
- `swebench/**` (upstream evaluator)
- `tests/oracle_**/**`
- `crates/*/src/reward_specs/**`
- `crates/*/src/verifiers/**`
- `*.pem`, `*.key`, `**/credentials.json`

CI lint blocks C-drive paths (`scripts/ci/check_no_c_drive_paths.sh`). Mounted-drive roots (`/mnt/d`, `/mnt/c`) forbidden in active code/FSV.

### 5.4 Fail-closed on non-aiwonder production roots

`context-graph-paths` rejects production runs whose durable root is not under `/zfs/archive/contextgraph/`. Local-only smokes opt in via env var + matching issue label.

---

## 6. Serialization format rules

| Format | When | Where |
|---|---|---|
| `serde_json` (JSON) | Provenance / metadata / `SourceMetadata` / any field with `skip_serializing_if` | All ME-JEPA durable rows |
| `bincode` | Hot loops only; never durable provenance | Rare |
| `postcard` | Compact binary encoding for hot vectors | Some quantized stores |
| Custom magic+version header | Tier compression blobs | `context-graph-tier-compression` (22-byte header) |
| SHA256 `sha256:<hex>` | Corpus provenance row hash | `crates/context-graph-mejepa-corpus/src/lib.rs:567-600` |
| ed25519 signature | Witness chain segments | `crates/context-graph-witness/src/cert.rs` |
| Merkle aggregation | Witness chain compression | `crates/context-graph-mejepa-hygiene/src/witness_compress.rs` |

---

## 7. Provenance invariants

1. **Constant name == on-disk CF name.** Renaming a CF constant must change the on-disk string. Deletion = explicit schema version bump.
2. **No schema migrations.** Wipe DB on schema change (`CLAUDE.md §6.6`).
3. **Independent RocksDB readback** for FSV: flush/drop/reopen through production opener or equivalent separate handle, then decode same row back (`FSV-PROTOCOL §6.5`).
4. **Source-of-truth before claim.** A return value is a claim; the SoT is the verdict (`FSV-PROTOCOL §1`).

---

## 8. Planned new column families

### 8.1 Cross-panel triangulation (#405)

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_PANEL_B_OBSERVATIONS` | `(prediction_id, panel_b_run_id)` | JSON-encoded `Panel` (Panel B encoders) | Materialized Panel B observation per prediction |
| `CF_MEJEPA_CROSS_PANEL_AGREEMENT` | `(prediction_id, panel_pair_id)` | bincode `{verdict_a, verdict_b, score_a, score_b, oracle_verdict, agree_flag, cell_key, ts_ns}` | Drives `cross_panel_oracle_correlation`; fail-closed `CROSS_PANEL_GOODHART_DETECTED` |

### 8.2 Pathway exploration (#416)

All three are **`leakage_policy:target_side_only`** (chosen pathway is downstream of operator choice, never live predictor input). Strict write-time assertion: non-Q1/Q2/Q5 leaves fail closed with `PATHWAY_AMBIGUOUS_LEAF_REJECTED`.

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_PATHWAY_TREES` | `(prediction_id, tree_run_id)` | bincode `PathwayTree` | Full pathway tree per candidate |
| `CF_MEJEPA_SURFACED_PATHWAYS` | `(prediction_id, pathway_id)` | bincode `SurfacedPathway` | Top-K returned to operator |
| `CF_MEJEPA_OPERATOR_PATHWAY_CHOICES` | `(prediction_id, chooser_id)` | bincode `{chosen_pathway_id, chose_at_ts_ns, rationale_text?}` | Operator's chosen future |

### 8.3 Bidirectional skill linkage (#417 remaining slice)

#414 already shipped `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP`, `CF_MEJEPA_SKILL_REVERSE_INDEX`, `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT`. #417 adds:

| CF | Purpose |
|---|---|
| `CF_MEJEPA_SKILL_GRAPH` | Depends-on / composes-with / prerequisite-of |
| `CF_MEJEPA_SKILL_COVERAGE_AUDIT` | Per-chunk coverage partition |
| `CF_MEJEPA_CROSS_REPO_SKILL_MATCHES` | Transfer skills to newly ingested projects |
| `CF_MEJEPA_SKILL_CONFLICT_GRAPH` | Mutually exclusive / contradictory outcome |
| `CF_MEJEPA_PATHWAY_SKILL_ATTACHMENTS` | Join #416 binary pathway leaves to concrete chunk/skill evidence |

### 8.3bis Causal-embedder track (#744 / #745 / #746 TASK-CR-006/007/008)

All aiwonder-only per `CLAUDE.md §6.7`.

| CF | Issue | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_MEJEPA_CROSS_CLUSTER_TRANSITIONS` | #744 | `(embedder_id, cluster_before, cluster_after)` | bincode `{transition_count, asymmetry_score, holdout_aug, p_value}` | Cross-cluster transition tensor per embedder, computed by auto-labeler extension (#413) on (commit_before, commit_after) chunk pairs. Directional asymmetry signal for causal direction detection. |
| `CF_MEJEPA_EMBEDDER_REPLACEMENT_AUDIT` | #745 | `(slot_id, replacement_id, audit_ts_ns)` | bincode `{old_model_sha, new_model_sha, training_corpus_sha, gate_outcomes: {novelty_pass, utility_pass, quality_pass, leakage_pass}, promotion_decision, rollback_evidence}` | Per-slot in-slot replacement audit log. Used for promotion gate + revert path. |
| `CF_MEJEPA_EMBEDDER_SIGNAL_AUDIT` | #746 | `(slot_id, audit_run_id)` | bincode `{drop_delta, per_cell_attribution, live_input_variance, decision, rank}` | Low-signal embedder audit output (Phase 1-4 of #746). Authoritative replacement-priority ranking feeding #745. |

State directory additions (aiwonder-only):

| Path | Issue | Purpose |
|---|---|---|
| `/zfs/archive/contextgraph/corpus/commitpackft-v1-<TS>/` | #744 | CommitPackFT 2GB ingestion + AST chunks per (commit_before, commit_after) |
| `/zfs/archive/contextgraph/state/cross-cluster-transitions/task-cr-006-v1/` | #744 | Cross-cluster transition tensors per embedder + asymmetric AUC validation |
| `/zfs/archive/contextgraph/corpus/commitpack-v1-<TS>/` | #745 | CommitPack 4TB ingestion (PHASE 2; if #744 PASSES) |
| `/zfs/archive/contextgraph/corpus/causenet-v1-<TS>/` | #745 | CauseNet 11M + ConceptNet + ATOMIC ingestion |
| `/zfs/archive/contextgraph/corpus/bugfix-corpus-v1-<TS>/` | #745 | BugsInPy + CommitPackFT bug-fix + SWE-bench Lite for E14 → C_BugFix |
| `/zfs/archive/contextgraph/state/embedders/c-temporal-commit-v1/` | #745 | Trained C_TemporalCommit weights + SHA manifest |
| `/zfs/archive/contextgraph/state/embedders/c-commonsense-causal-v1/` | #745 | Trained C_CommonsenseCausal weights + SHA manifest |
| `/zfs/archive/contextgraph/state/embedders/c-bugfix-v1/` | #745 | Trained C_BugFix weights + SHA manifest |
| `/zfs/archive/contextgraph/state/embedder-signal-audit/task-cr-008-signal-ranking-v1/embedder_replacement_priority.jsonl` | #746 | Authoritative replacement-priority output |
| `/zfs/archive/contextgraph/fsv/task-cr-{006,007,008}-*-fsv/run-<TS>Z/` | #744/#745/#746 | FSV evidence per phase, manual SoT readback per `06 §1.9` |

### 8.4 Information-physics (#511 / #512 / #514)

| CF | Issue | Purpose |
|---|---|---|
| `CF_MEJEPA_TRAIN_CERT_ENTROPY` | #511 | Per-window entropy auxiliary loss component (λ auto-calibration) |
| `CF_MEJEPA_PANEL_ECC_SYNDROME` | #512 | Per-panel syndrome record for self-dual doubly-even ECC over 15-slot occupancy; `PANEL_ECC_UNCORRECTABLE` typed reason |
| `CF_MEJEPA_COLD_LOAD_SNAPSHOTS` | #514 | Promotion-time baseline snapshot for cold-load behavioural FSV |

### 8.5 Creativity-engine CFs (post-0.95: #516 / #517 / #518 / #519 / #520)

| CF | Issue | Purpose |
|---|---|---|
| `CF_MEJEPA_EMBEDDER_PROPOSAL_AUDIT` | #516 | Audit for every NAS / cross-library / operator embedder proposal |
| `CF_MEJEPA_EMBEDDER_DEMAND` | #517 | Mistake-loop residual signature when no slot can explain disagreement |
| `CF_MEJEPA_EMBEDDER_RETIREMENT_AUDIT` | #518 | Retired embedders + grace-period archival |
| `CF_MEJEPA_DOMAIN_REGISTRY` / `CF_MEJEPA_CHUNK_DOMAIN_ASSIGNMENT` / `CF_MEJEPA_EMBEDDER_DOMAIN_MEMBERSHIP` | #519 | Domain-routed sparse panel; **schema change** (variable-K filled_mask) |
| `CF_MEJEPA_CHUNK_CONSOLIDATED` / `CF_MEJEPA_CONSOLIDATION_AUDIT` | #520 | Episodic-to-semantic consolidation: settled chunks → `(cluster_id, residual)` |

### 8.6 Neuroscience-inspired reserve (#538 – #545, p3 — DO NOT START until current queue concludes)

| CF Constant (planned) | Issue | Key | Value | Purpose |
|---|---|---|---|---|
| `CF_MEJEPA_PER_SLOT_PREDICTIONS` | #538 | `(prediction_id, slot)` | bincode `{predicted_vector, observed_vector, residual_norm, slot_identity_preserved: true}` | Top-down prediction + observation pair per slot |
| `CF_MEJEPA_PREDICTION_ERRORS` | #538 | `(prediction_id, slot)` | bincode `{residual_l2, residual_cosine_to_predicted, error_class}` | Aggregated prediction error per slot per prediction |
| `CF_MEJEPA_REPLAY_SCHEDULE` | #539 | `(replay_batch_id)` | bincode `{schema_skill_ids, reverse_order, expected_utility_score}` | Schema-grouped batch schedule audit |
| `CF_MEJEPA_GENERATIVE_REPLAY_AUDIT` | #539 | `(idle_run_id, rehearsal_id)` | bincode `{schema_id, synthetic_panel_sha, forgetting_drift_observed}` | Idle-time rehearsal accountability; explicit `synthetic=true` |
| `CF_MEJEPA_BINDING_BASIS_REGISTRY` | #541 | `(basis_function_id)` | bincode `{slot_pair, summary_statistics, learned_params_sha, validation_correlation_lift}` | Per-basis-function registry with empirical contribution audit |

No new CFs from #540 (consumes shipped #404 / #413 / #414 via backward prediction), #542 / #544 / #545 (design-influence), or #543 (rides existing `CF_EMB_0..CF_EMB_12`).

### 8.7 Canonical opener slice update

When CFs land, they're added to `ALL_HYGIENE_REFERENCED_CFS`. The `canonical_hygiene_cfs_match_group_union` test enforces that the grouped contract matches the canonical list. Adding a CF is a schema change → deliberate DB wipe + rebuild (no migrations).

---

## 9. What this doc does NOT cover

- Per-CF RocksDB tuning (block cache, bloom filters, write buffer size) — see per-crate `column_families.rs::*_options` functions.
- Exact byte-level key encoding of every CF — see each CF's writer call site (`put_cf` across the workspace).
