# 03 — Storage Schema and Policy

**Last refreshed:** 2026-05-19.

Source files covered: `crates/context-graph-mejepa-cf/src/lib.rs` (entire file — canonical ME-JEPA CF registry), `crates/context-graph-storage/src/lib.rs`, `crates/context-graph-storage/src/teleological/`, `crates/context-graph-mejepa-instruments/src/panel_store.rs`, `CLAUDE.md §8`, GitHub issues #363/#369.

---

## 1. Storage Backend

Single RocksDB instance per runtime root. CFs opened deterministically at daemon startup. Schema change = wipe-and-rebuild (no migrations; per `CLAUDE.md §6.6`). All durable rows are content-addressed by SHA256 where appropriate; provenance metadata is JSON (not bincode — bincode + `skip_serializing_if` was found broken in earlier audits).

Connection management:
- Opener: `crates/context-graph-storage/src/lib.rs` re-exports CF descriptor builders. Daemon opens every CF in `ALL_HYGIENE_REFERENCED_CFS` (per `mejepa-cf/lib.rs:471`) at startup. RocksDB requires every on-disk CF to be in the opener list — adding/removing a CF MUST be a visible schema change.
- Compaction: per-CF `Options` (see `column_families.rs::edges_options`, `embeddings_options`, `index_options`, `nodes_options`, `system_options`).
- Total RocksDB CFs (live): `MEJEPA_HYGIENE_REFERENCED_CF_COUNT` ME-JEPA + 19 teleological + 13 quantized embedder + 5 code + 2 causal + a few system CFs ≈ 200+.

---

## 2. ME-JEPA Column Families (canonical registry)

**Source of truth:** `crates/context-graph-mejepa-cf/src/lib.rs`. Every constant ident must equal its on-disk string (enforced by `flywheel_cf_names_match_const_idents`, `fingerprint_cf_names_match_const_idents`).

### 2.1 Self-Healing (7 CFs) — `SELF_HEALING_CFS`

| CF Constant | Purpose | Issue / Task |
|---|---|---|
| `CF_MEJEPA_FISHER_SNAPSHOTS` | EWC++ Fisher diagonal snapshots per training window | TASK-PY-G-065 |
| `CF_MEJEPA_DRIFT_WINDOW` | Active rolling drift window | self-healing |
| `CF_MEJEPA_DRIFT_HISTORY` | Per-window drift summary | self-healing |
| `CF_MEJEPA_HEAL_REPORTS` | Heal scheduler decision rows | self-healing |
| `CF_MEJEPA_DISTILL_STEPS` | Per-distillation-step rows | self-healing |
| `CF_MEJEPA_ACTIVE_POINTERS` | Active model pointer (which checkpoint serves) | self-healing |
| `CF_MEJEPA_CALIBRATION_HISTORY` | Per-window calibration history | self-healing |

### 2.2 Self-Healing Referenced (extends 2.1) — `SELF_HEALING_REFERENCED_CFS`

Adds `CF_MEJEPA_PLASTICITY_HISTORY` (TASK-PY-G-066 continual backprop), `CF_MEJEPA_WEIGHT_BLOBS`, `CF_MEJEPA_SHIFT_WATERMARK`, `CF_MEJEPA_TRAIN_CERTS`.

### 2.3 Inference / Live Predictions (~50 CFs) — `INFER_CFS`

| CF Constant | Key shape | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_LIVE_PREDICTIONS` | `(session_id, prediction_id)` | JSON `RealityPrediction` row | Primary Q1-Q5 output served by `mejepa_predict_latest` |
| `CF_MEJEPA_LIVE_SESSION_TRACES` | `(session_hex32, ts_ns)` | JSON shift trace | TASK-PY-G-056 end-to-end session trace |
| `CF_MEJEPA_HIERARCHICAL_PREDICTIONS` | `(prediction_id, level)` | JSON | TASK-FP-102 file/function/AST/chunk hierarchy |
| `CF_MEJEPA_ORACLE_VERDICTS` | `(task_id, category)` | JSON `OracleVerdict` | Persistent verdict cache from `swebench.rs` |
| `CF_MEJEPA_PREDICTION_VERIFICATIONS` | `(prediction_id, verifier_id)` | JSON `{kind: Confirmed|Refuted, ...}` | TASK-PY-G-069/070 PostToolUse + Stop hook capture |
| `CF_MEJEPA_REALITY_IMPACT` | `prediction_id` | bincode `RealityImpactRecord` | TASK-PY-G-045 Q5 shift-log replay: matched/missed/not-yet-observed/surprise consequence rows |
| `CF_MEJEPA_Q5_CALIBRATIONS` | `(calibration_id, language, mutation_category, side_effect_kind)` | bincode `Q5CalibrationRecord` | TASK-PY-G-013 Q5 replay calibration: precision/recall/F1/support/surprise-rate plus replay exclusions |
| `CF_MEJEPA_CLAIM_RECONCILIATION` | `(prediction_id, claim_id)` | JSON | TASK-PY-G-041 Q1 claim graph |
| `CF_MEJEPA_PROJECT_REPORTS` | `project_id` | JSON `ProjectReport` | TASK-PY-G-064 per-project reality report |
| `CF_MEJEPA_STRESS_RUNS` | `(run_id, case_id)` | JSON | TASK-PY-G-072 end-to-end stress harness |
| `CF_MEJEPA_SYNTHETIC_STRESS_RESULTS` | `(run_id, case_id)` | JSON | TASK-PY-G-071 synthetic 30-case stress |
| `CF_MEJEPA_TRAIN_CERTS` | `cert_id (sha)` | JSON `TrainingCertificate` (incl. `conditional_description_length_bits` per TASK-EK-001) | Training step certificates |
| `CF_MEJEPA_PANELS` | `panel_id` | JSON-encoded `Panel` (array of 15 per-embedder vectors serialized as a 5,120 f32 contiguous buffer + per-slot byte offsets + `filled_mask`; SEMANTIC unit is the per-embedder array, NOT a fused 5,120-d embedding) | Materialized panels |
| `CF_MEJEPA_SHIFT_WATERMARK` | (singleton) | watermark cursor | Durability gate for shift-log replay |
| `CF_MEJEPA_PANEL_CACHE` | LRU pointer | — | Hot panel cache |
| `CF_MEJEPA_PANEL_META` | `panel_id` | JSON `PanelMeta` | Panel metadata index |
| `CF_MEJEPA_AUX_SIGNALS_OVERRIDES` | `(panel_id, slot)` | JSON | Operator override slot values |
| `CF_MEJEPA_EVAL_REPORTS` | `(eval_run_id, cell)` | JSON | Holdout/ship-gate eval rows |
| `CF_MEJEPA_ABLATION_REPORTS` | `(run_id, action_class)` | JSON | TASK-PY-G-054 negative-action ablation reports |
| `CF_MEJEPA_TASK_GRAPH` | `task_id` | JSON | Task DAG |
| `CF_MEJEPA_TASK_GRAPH_HISTORY` | `(task_id, ts_ns)` | JSON | Task DAG mutation log |
| `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` | `(priority, entry_id)` | JSON `ActiveLearningQueueEntry` (incl. `curiosity_score` per TASK-EK-006) | Active learning queue |
| `CF_MEJEPA_ACTIVE_LEARNING_LABELS` | `entry_id` | JSON | Operator labels for active-learning entries |
| `CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS` | `entry_id` | JSON | Eviction audit |
| `CF_MEJEPA_OOD_ESCALATIONS` | `(prediction_id, ts_ns)` | JSON | OOD escalation log |
| `CF_MEJEPA_CURIOSITY_TELEMETRY` | `(window_id, kind)` | JSON | TASK-EK-006 compression-progress curiosity per-window distributions |
| `CF_MEJEPA_RESEARCH_QUESTIONS` | `question_id` | JSON | Research questions |
| `CF_MEJEPA_MODEL_PROMOTIONS` | `(model_id, ts_ns)` | JSON | Promotion log |
| `CF_MEJEPA_PRODUCTION_TELEMETRY` | `(ts_ns, kind)` | JSON | Production telemetry |
| `CF_MEJEPA_DDA_SIGNALS` | `(chunk_id, signal_class)` | JSON DDA feature vector | REQ-FLYWHEEL-10 |
| `CF_MEJEPA_HEAD_PROJECTION_REGISTRY` | `head_id` | JSON per-head slice map + schema version | TASK-PY-G-043 |
| `CF_MEJEPA_FAILURE_EXEMPLARS` | `exemplar_id` | JSON | REQ-FLYWHEEL-10 closest-failure exemplars |
| `CF_MEJEPA_AGENT_FEEDBACK` | `(session_id, ts_ns)` | JSON | Agent feedback events |
| `CF_MEJEPA_FLYWHEEL_JOB_RUNS` | `job_run_id` | JSON | Flywheel job run audit |
| `CF_MEJEPA_OPERATOR_OVERRIDES` | `prediction_id` | JSON | REQ-PREDICT-12 operator override records |
| `CF_MEJEPA_OPERATOR_CONTRIBUTIONS` | `contribution_id` | JSON | TASK-EK-014 operator contribution events |
| `CF_MEJEPA_LABEL_TRANSFER_DECISIONS` | `(source_label_id, target_label_id)` | JSON | TASK-PREDICT-LABEL-012 durable label transfer audit |
| `CF_MEJEPA_PARK_LIST` | `prediction_id` | JSON | TASK-FLYWHEEL-007 fail-closed retry park list |
| `CF_MEJEPA_COLD_CELL_METRICS` | `(cell_key)` | JSON | TASK-PY-G-061 cold cells (n_supporting < 50) |
| `CF_MEJEPA_STATIC_ANALYSIS_SIGNALS` | `(chunk_id, tool, code)` | JSON `StaticAnalysisSignalRecord` | TASK-RWD-208 / TASK-PY-G-074 |
| `CF_MEJEPA_Q4_SECURITY_LABELS` | `(chunk_id, detector, code)` | JSON `Q4SecuritySignalRecord` | TASK-RWD-204 / TASK-PY-G-046 |
| `CF_MEJEPA_Q4_PERF_LABELS` | `(corpus_row_id, metric)` | JSON `Q4PerfSignalRecord` | TASK-PY-G-047 |
| `CF_MEJEPA_Q4_ACCURACY_LABELS` | `(corpus_row_id, metric, source_test_hash)` | JSON `Q4AccuracySignalRecord` | TASK-PY-G-048 |
| `CF_MEJEPA_Q4_COST_LABELS` | `(corpus_row_id, kind)` | JSON `Q4CostSignalRecord` | TASK-PY-G-049 |
| `CF_MEJEPA_Q4_REASONING_LABELS` | `(corpus_row_id, session_id)` | JSON `Q4ReasoningSignalRecord` | TASK-PY-G-050 |
| `CF_MEJEPA_HEAD_CALIBRATIONS` | `(head, class)` | JSON per-head calibration / producer-heldout row | TASK-PY-G-012 / TASK-PY-G-050 |
| `CF_MEJEPA_CONTRADICTION_THRESHOLDS` | `cell_id` | bincode `ContradictionThresholds` | TASK-PY-G-044 per-cell `(tau_oracle, tau_failure_count)` gate for multi-head contradictions |
| `CF_MEJEPA_CONSTELLATION_PATTERNS` | `pattern_id` | bincode `ConstellationRelationshipPattern` | TASK-PY-G-090 promoted slot-pair relationship patterns: consensus, contradiction, redundancy, novelty, blind spots, and lift over own-slot baselines |
| `CF_MEJEPA_COVERAGE_SIGNALS` | `(chunk_id)` | JSON `CoverageLabel` | TASK-RWD-209 / TASK-PY-G-075 |
| `CF_MEJEPA_COMPLEXITY_SIGNALS` | `(chunk_id)` | JSON `ComplexityLabel` | TASK-RWD-209 |
| `CF_MEJEPA_CHURN_SIGNALS` | `(chunk_id)` | JSON `ChurnLabel` | TASK-RWD-209 |
| `CF_MEJEPA_DOC_COVERAGE_SIGNALS` | `(chunk_id)` | JSON `DocCoverageLabel` | TASK-RWD-209 |
| `CF_MEJEPA_DEP_CVE_SIGNALS` | `(chunk_id)` | JSON `DepCveLabel` | TASK-RWD-209 |
| `CF_MEJEPA_TYPE_GRAPH_SIGNALS` | `(chunk_id, call_site_id)` | JSON | TASK-RWD-205 |
| `CF_MEJEPA_TELEMETRY_SIGNALS` | `(symbol_id)` | JSON | TASK-RWD-206 |
| `CF_MEJEPA_LIVE_SESSION_SIGNALS` | `(session_id, ts_ns)` | JSON | TASK-RWD-207 |
| `CF_MEJEPA_REDACTION_LOG` | `(chunk_id, ts_ns)` | JSON | TASK-PY-G-059 source-chunk secret-redaction audit |
| `CF_MEJEPA_SIGNAL_DROP_LOG` | `(ts_ns, source)` | JSON `{reason, code, …}` | TASK-RWD-315 fail-closed signal-drop events |
| `CF_MEJEPA_REPLAY_BUFFER` | `(priority, entry_id)` | JSON | TASK-RWD-210 / TASK-PY-G-068 surprise-prioritized M(t) replay |
| `CF_MEJEPA_SAMPLER_REWARDS` | `(ts_ns, kind)` | JSON | TASK-PY-G-053 oracle-derived online observe reward |
| `CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT` | `(ts_ns, cluster_id)` | JSON | TASK-RWD-211 / TASK-PY-G-067 novel-cluster accepted/refuted audit |
| `CF_MEJEPA_OOD_HARVEST` | `(harvest_run_id, prediction_id)` | JSON | TASK-PY-G-057 / #276 live-session OOD harvest rows (above-threshold predictions, RocksDB byte anchors, orphan/degenerate quarantines) |
| `CF_MEJEPA_OOD_CALIBRATIONS` | `(generated_at_unix_ms, calibration_id)` | bincode `OodCalibrationReport` | TASK-PY-G-057 harvest audit + TASK-PY-G-014 serving calibrator: per-cell threshold/AUC/FPR/recall, selected-for-serving flag, empty-harvest and cell-regression flags |

### 2.4 Constellation / TCT (4 CFs) — `TCT_CFS`

| CF Constant | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_CONSTELLATION` | `cell_key` | JSON `Constellation` | Per-cell calibrated centroids + thresholds |
| `CF_MEJEPA_GUARD_DECISIONS` | `(prediction_id, ts_ns)` | JSON `GuardDecision` | Gτ guard decision log |
| `CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS` | `(refresh_run_id)` | JSON | Refresh report (TCT-005) |
| `CF_MEJEPA_CONSTELLATION_REFRESH_LOG` | `(ts_ns, cell_key)` | JSON | TASK-PY-G-052 per-cell freshness scheduler log |

### 2.5 Fingerprints (8 CFs) — `FINGERPRINT_CFS`

| CF Constant | Purpose | Task |
|---|---|---|
| `CF_MEJEPA_FAILURE_FINGERPRINTS` | First-class failure-shape fingerprint catalog | TASK-FP-001/002 |
| `CF_MEJEPA_FINGERPRINT_REFERENCES` | Fingerprint → exemplar pointer | TASK-FP-001/002 |
| `CF_MEJEPA_FINGERPRINT_REVERSE_INDEX` | Exemplar → fingerprint | TASK-FP-001/002 |
| `CF_MEJEPA_FINGERPRINT_CALIBRATION` | Per-fingerprint conformal scores | TASK-FP-001/002 |
| `CF_MEJEPA_FINGERPRINT_AUDIT` | Lifecycle audit | TASK-FP-001/002 |
| `CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS` | 4-window per-fingerprint ship-gate | TASK-FP-007 |
| `CF_MEJEPA_FINGERPRINT_FISHER` | Per-fingerprint EWC Fisher diagonal | TASK-FP-008 |
| `CF_MEJEPA_FINGERPRINT_DORMANCY` | Per-fingerprint CBP dormancy snapshots | TASK-FP-008 |

### 2.6 Adversarial — `ADVERSARIAL_CFS`

| CF Constant | Purpose | Task |
|---|---|---|
| `CF_MEJEPA_ADVERSARIAL_CORPUS` | 30-case adversarial Python rows feeding fingerprint catalog | TASK-PY-G-060 / TASK-FP-009 |

### 2.7 Explanatory Knowledge (17 CFs) — `EXPLANATORY_KNOWLEDGE_CFS`

| CF Constant | Purpose | Task |
|---|---|---|
| `CF_MEJEPA_MINCUT_REPORTS` | Stoer-Wagner mincut reports over ME-JEPA panel graphs | TASK-EK-003 |
| `CF_MEJEPA_PAIRWISE_MI` | Pairwise MI matrix rows for embedder-panel audits | TASK-PY-G-007 |
| `CF_MEJEPA_PATCH_SIMILARITY_META` | Patch-similarity HNSW index metadata | TASK-PY-G-042 |
| `CF_MEJEPA_INSTRUMENT_PROPOSALS` | Absence-as-existence instrument proposal queue | TASK-EK-002 |
| `CF_MEJEPA_EMBEDDER_PROPOSALS` | Ranked absence-shape queue for self-designed embedders | TASK-EK-018B |
| `CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS` | Falsified dynamic-embedder candidate architectures | TASK-EK-018C |
| `CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY` | Runtime dynamic-embedder hot-swap registry | TASK-EK-018A |
| `CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE` | Dynamic embedder provenance | TASK-EK-018A |
| `CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH` | Chunk dependency graph | TASK-EK-016 |
| `CF_MEJEPA_CHUNK_FOUNDATIONALITY` | PageRank-derived chunk foundationality | TASK-EK-016 |
| `CF_MEJEPA_LIBRARY_REGISTRY` | Library registration | TASK-EK-017 |
| `CF_MEJEPA_LIBRARY_FOUNDATIONALITY` | Per-library foundationality | TASK-EK-017 |
| `CF_MEJEPA_CROSS_LIBRARY_REFERENCES` | Cross-library reference counts | TASK-EK-017 |
| `CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH` | Embedder dependency graph | TASK-EK-016-ext |
| `CF_MEJEPA_EMBEDDER_FOUNDATIONALITY` | PageRank-derived embedder foundationality | TASK-EK-016-ext |
| `CF_MEJEPA_OPERATOR_CONTRIBUTIONS` | (also in INFER_CFS) | TASK-EK-014 |
| `CF_MEJEPA_CURIOSITY_TELEMETRY` | (also in INFER_CFS) | TASK-EK-006 |

### 2.8 Hygiene-only (subset of HYGIENE_CFS not already enumerated)

| CF Constant | Purpose |
|---|---|
| `CF_MEJEPA_MUTATION_CORPUS` | Per-mutation corpus row store |
| `CF_MEJEPA_LIVE_ATTEMPT_CORPUS` | Live-attempt corpus row store |
| `CF_MEJEPA_QUOTA_STATE` | Quota state (Phase 6 hygiene) |
| `CF_MEJEPA_WEIGHT_BLOBS` + `CF_MEJEPA_WEIGHT_BLOBS_META` | Model weight blobs + metadata |
| `CF_MEJEPA_WITNESS_CHAIN` + `CF_MEJEPA_WITNESS_SEGMENT_META` | Witness chain segments |
| `CF_MEJEPA_PANEL_DISK_OVERFLOW` | Panel overflow to disk |
| `CF_MEJEPA_GC_HISTORY` | GC report history |

### 2.9 OOD Harvest — `OOD_HARVEST_CFS`

| CF Constant | Purpose | Task |
|---|---|---|
| `CF_MEJEPA_OOD_HARVEST` | Live-session above-threshold OOD prediction rows (with RocksDB byte anchors and orphan/degenerate quarantines) | TASK-PY-G-057 / #276 |
| `CF_MEJEPA_OOD_CALIBRATIONS` | OOD calibration reports; TASK-PY-G-014 selects serving τ_ood, fails closed on missing/corrupt/unselected calibrator when strict mode is enabled, and surfaces flags like `OOD_HARVEST_EMPTY`, `OOD_CELL_AUC_REGRESSION`, and `OOD_GATE_OVER_FLAGGING` | TASK-PY-G-057 / #276 + TASK-PY-G-014 / #129 |

Harvested rows are projected into `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` at 4× weight (downweighted to 1× when an operator labels the row in-distribution).

### 2.10 Canonical opener slice

`ALL_HYGIENE_REFERENCED_CFS` (`mejepa-cf/lib.rs:471-569`) is the single canonical opener list. Length = `MEJEPA_HYGIENE_REFERENCED_CF_COUNT`. Drift between this list and `SELF_HEALING_REFERENCED_CFS ∪ INFER_CFS ∪ HYGIENE_CFS ∪ TCT_CFS ∪ FLYWHEEL_CFS ∪ FINGERPRINT_CFS ∪ ADVERSARIAL_CFS ∪ REWARD_SIGNAL_CFS ∪ EXPLANATORY_KNOWLEDGE_CFS ∪ OOD_HARVEST_CFS` is rejected by `canonical_hygiene_cfs_match_group_union` test.

---

## 3. Teleological Column Families (19 — pre-ME-JEPA, still active)

`crates/context-graph-storage/src/teleological/`. `TELEOLOGICAL_CF_COUNT = 19`.

| CF Constant | Purpose |
|---|---|
| `CF_FINGERPRINTS` | `TeleologicalFingerprint` rows |
| `CF_FINGERPRINTS_LEARNER` | Per-learner fingerprints |
| `CF_E1_MATRYOSHKA_128` | E1 (e5-large-v2) 128-d Matryoshka projections |
| `CF_E13_SPLADE_INVERTED` | E13 SPLADE inverted index |
| `CF_TOPIC_PROFILES` | Topic profile rows |
| `CF_GOAL_CENTROIDS` | Goal centroid rows |
| `CF_LEARNER_*` (9 CFs) | Per-learner state, audit, K-sleep, M-per-trace, profile, retrieval log, state history, goal states, training datasets, constellations, delta log |
| `CF_LEARNING_EVENTS` | Learning event journal |
| `CF_MERGE_HISTORY` | Phase 4 lifecycle provenance — merge history |
| `CF_IMPORTANCE_HISTORY` | Importance change history |
| `CF_AUDIT_LOG` + `CF_AUDIT_BY_TARGET` | Append-only audit log |
| `CF_CUSTOM_WEIGHT_PROFILES` | Custom weight profiles |
| `CF_ENTITY_PROVENANCE` | Entity provenance |
| `CF_TOOL_CALL_INDEX` | tool_use_id → memory mapping |
| `CF_CONSOLIDATION_RECOMMENDATIONS` | Consolidation hints |
| `CF_EMBEDDING_REGISTRY` | Embedder version tracking |

`TELEOLOGICAL_VERSION` is the schema version constant.

---

## 4. Other Storage CF Groups

| Group | CFs | Source |
|---|---|---|
| **Quantized embedders** | `CF_EMB_0` … `CF_EMB_12` (13 CFs) | `quantized_embedder_cf_options` |
| **Code** | `CF_CODE_ENTITIES`, `CF_CODE_E7_EMBEDDINGS`, `CF_CODE_FILE_INDEX`, `CF_CODE_NAME_INDEX`, `CF_CODE_SIGNATURE_INDEX` | `crates/context-graph-storage/src/teleological/column_families.rs` |
| **Causal** | `causal_relationships`, `causal_by_source` (2 CFs) | per `column_families.rs` |
| **Edges** | K-NN edges + typed edges + secondary index | `graph_edges/` |
| **DynamicJEPA** | (separate dataplane CFs, ~15) | `crates/context-graph-storage/src/dynamicjepa/` |

Dimension constants: `E1_DIM`, `E1_MATRYOSHKA_DIM`, `E2_DIM`, `E3_DIM`, `E4_DIM`, `E5_DIM`, `E6_SPARSE_VOCAB`, `E7_DIM`, `E8_DIM`, `E9_DIM`, `E10_DIM`, `E11_DIM`, `E12_TOKEN_DIM`, `E13_SPLADE_VOCAB`, `NUM_EMBEDDERS` — all exported from `crates/context-graph-storage/src/lib.rs:146-161`.

---

## 5. Storage Policy (post-migration 2026-05-18)

GitHub issue #363 (closed, `type:decision`) — supersedes the pre-migration aiwonder production policy.

| Tier | Path | Purpose | Retention |
|---|---|---|---|
| **Production durable SoT** | `/zfs/archive/contextgraph/` (aiwonder) | All post-migration production runs: corpus, FSV evidence, runtime shift logs, test outcomes, project-ingest outputs, weekly exports, durable state | Multi-month; never auto-pruned |
| **Production hot** | `/zfs/hot/contextgraph/` (aiwonder) | Hot work: repo checkout, model weights, stable hot inputs, active scheduler logs | Days to weeks |
| **Aiwonder scratch** | Explicitly budgeted aiwonder dirs | Issue-scoped one-off smokes | Per-issue |
| **Legacy / local** | `/mnt/d/contextgraph/` (operator WSL2) | Pre-migration corpus, FSV, shift-log — historical evidence only; #369 reclassifies | Frozen; no new writes for production work |
| **Ephemeral** | `/tmp/contextgraph-*` | Test scratch | Process lifetime |

### 5.1 Aiwonder production directory layout

```
/zfs/archive/contextgraph/
├── corpus/
│   ├── python-forward-cache-v1-aiwonder-prod-<TS>/
│   │   ├── shards/                          # SoT for shard progress (NOT /zfs/hot/.../runtime/)
│   │   ├── packed_records/                  # Packed stores
│   │   └── ...
│   └── frozen-instrument-registry/          # ~78 KB
├── fsv/                                     # post-migration target; currently empty as of 2026-05-18
│                                            # production FSV still being authored under /mnt/d (TASK-AIWONDER-018 / #369 tracks revalidation)
├── runtime/
│   ├── shift-log.jsonl                      # JSONL shift log (tailed by shift-subscriber)
│   ├── test-outcomes/<hash>.jsonl           # PostToolUse capture (per session)
│   ├── posttooluse-mejepa-capture.jsonl     # TASK-PY-G-069 capture log
│   ├── stop-mejepa-self-verify.jsonl        # TASK-PY-G-070 Stop-hook log
│   ├── observe-ticker.jsonl                 # Observe ticker outcomes
│   ├── scheduler_events.jsonl               # Scheduler event log
│   └── agent-history/                       # Q1 follow-up: agent response bodies
├── projects/<project_id>/
│   ├── manifest.json                        # Merkle-tree manifest (TASK-PY-G-062)
│   ├── cache/                               # Per-project chunk/embed cache
│   ├── predictions/                         # Project-local CF_MEJEPA_LIVE_PREDICTIONS dump
│   └── report/                              # report.json + report.md (TASK-PY-G-064)
├── state/                                   # Durable runtime state surviving restarts
│   └── calibration/per-slot-v1/             # TASK-PY-G-089 / #389 per-slot calibration + OOD policy
│       ├── per_slot_calibration.jsonl       #   12 global slot calibrations
│       ├── per_cell_slot_calibration.jsonl  #   84 fail-closed per-cell rows
│       ├── per_pair_calibration.jsonl       #   66 pair calibrations
│       ├── ood_guard_policy.json            #   strict-conjunction OOD policy
│       └── per_slot_calibration_report.json #   readback report
└── exports/
    └── weekly/<YYYY-MM-DD>/                 # Weekly eval reports, ablation_report.md, compression_progress.md, etc.
```

### 5.2 Aiwonder hot layout

```
/zfs/hot/contextgraph/
├── repo/                                    # aiwonder git checkout + release binaries
├── models/                                  # Embedder model artifacts (HF cache)
│   └── .cache/huggingface/hub/datasets--princeton-nlp--SWE-bench_Lite/
├── corpus/                                  # Hot inputs (chunks.jsonl, stable substrate)
└── runtime/                                 # Active scheduler logs / hot run state (NOT durable progress)
```

### 5.3 What is allowed where

| Action | Production durable | Production hot | Scratch | Local legacy |
|---|---|---|---|---|
| Write production FSV evidence | ✅ `/zfs/archive/...fsv/<task>-fsv/` | ❌ | ❌ | ❌ never for production work |
| Tail shift log | ✅ | (read mirror) | — | (historical) |
| Project-ingest cache | ✅ | (hot-mirror) | — | (historical) |
| Reads from training daemon | ✅ | ✅ | — | (historical) |
| RocksDB live writes | ✅ (single instance per runtime root) | — | — | (historical) |
| Test-outcome JSONL | ✅ | — | — | (historical) |
| Operator scratch | — | — | ✅ | ✅ |

### 5.4 Path discipline (behavioral; gate removed 2026-05-07)

`CLAUDE.md §7` — these paths must not be edited (no MCP-server-side gate; honor behaviorally):
- `.env` and `.env.*` anywhere
- `swebench/**` (upstream evaluator)
- `tests/oracle_**/**`
- `crates/context-graph-benchmark/datasets/swe_bench_*/**`
- `crates/*/src/reward_specs/**`
- `crates/*/src/verifiers/**`
- `*.pem`, `*.key`, `**/credentials.json`

`scripts/ci/check_no_c_drive_paths.sh` blocks C-drive paths from code + FSV files at CI time.

### 5.5 Fail-closed on non-aiwonder production roots

Recent commit `c4fd94a0 Fail closed on non-aiwonder production roots` — `context-graph-paths` rejects production runs whose durable root is not under `/zfs/archive/contextgraph/`. Local-only smokes must opt in via env var + matching issue label.

Predecessor: `a1e6a842 Make aiwonder production storage policy explicit` — declarative policy doc.

---

## 6. Serialization Format Rules

| Format | When to use | Where |
|---|---|---|
| `serde_json` (JSON) | Provenance / metadata / `SourceMetadata` / any field with `skip_serializing_if` | All ME-JEPA durable rows (decision after bincode + skip_serializing_if was found broken) |
| `bincode` | Internal hot loops only; never durable provenance | Rare |
| `postcard` | Compact binary encoding for hot vectors | Some quantized stores |
| Custom magic+version header | Tier compression blobs | `context-graph-tier-compression` (22-byte header) |
| SHA256 `sha256:<hex>` | Corpus provenance row hash | `crates/context-graph-mejepa-corpus/src/lib.rs:567-600` |
| ed25519 signature | Witness chain segments | `crates/context-graph-witness/src/cert.rs` |
| Merkle aggregation | Witness chain compression | `crates/context-graph-mejepa-hygiene/src/witness_compress.rs` |

---

## 7. Provenance Invariants

1. **Constant name == on-disk CF name.** Renaming a CF constant must change the on-disk string (enforced by `flywheel_cf_names_match_const_idents` and `fingerprint_cf_names_match_const_idents` tests). Deletion = explicit schema version bump.
2. **No schema migrations.** Wipe DB on schema change (`CLAUDE.md §6.6`).
3. **Independent RocksDB readback** for FSV: flush/drop/reopen through the production opener or an equivalent separate handle, then decode the same row back (`FSV-PROTOCOL §6.5`).
4. **Source-of-truth before claim.** A return value is a claim; the SoT is the verdict (`FSV-PROTOCOL §1`).

---

## 8. What This Doc Does NOT Cover

- Per-CF RocksDB tuning (block cache, bloom filters, write buffer size) — see per-crate `column_families.rs::*_options` functions.
- The exact byte-level key encoding of every CF — see each CF's writer call site (search `put_cf` across the workspace).
- Historical (retired) CFs — none currently retired; deletions are visible schema events per (1).

---

## 9. Planned New Column Families (Q1/Q2/Q5 binary contract + labeling + cross-panel + mistake loop)

Additive CFs supporting the 2026-05-19 binary doctrine (see `01_overview.md §1.1`, `05_predictor_pipeline.md §0`, `02_source_map.md §12`). The #414 skill-membership CFs are registered in `mejepa-cf/src/lib.rs`; #404/#405/#406 runtime consumers remain tracked by their issue gates. No rename of existing CFs.

### 9.1 Failure-mode labeling — Level-0 SHIPPED as JSONL artifacts; #414 skill CFs registered

**Level 0 (shipped 2026-05-19 via #413 / TASK-PY-G-117) materializes label state as durable JSONL files under `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/`, not as RocksDB CFs.** This was a deliberate choice: the labels are computed deterministically by `python_auto_label_discovery.py` from the forward-cache substrate, and storing them as JSONL keeps the artifact byte-replayable and independent of the RocksDB schema. The accepted label registry is the "stable label ID" surface other CFs (mistake log, replay buffer, constellation patterns) will reference.

**Level 0 shipped artifacts (JSONL on aiwonder):**

| Artifact | Path | Rows | Stable sha256 | Purpose |
|---|---|---|---|---|
| `chunk_constellation_labels.jsonl` | `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/` | 399,777 | `4ad438ff343781bd11d2ce1e12e7ac135d9392666fdd6440cf814faccc513d16` | Per-chunk Level-0 labels: AST/source surface, source-site relation, mutation mechanism, per-slot scalar/region labels, pair relationships, group signatures |
| `file_path_constellations.jsonl` | same | 4,224 | (per FSV) | Per-file constellation aggregates |
| `code_state_constellations.jsonl` | same | 3,000 | `582cc0de0e19fac5480a29914e573e10a207b372ca3df11db5cbce4050810538` | `(task_instance_id, mutation_category, workspace_state)` constellations with target-side outcome labels (`leakage_policy:target_side_only`) |
| `failure_evidence_sets.jsonl` | same | 1,139 | `747f82980f689aa137f4e5f38fd1ff9a5bef81ba56b446e6edc350fd1db424ee` | Three localization modes: single-point / multi-point / diffuse / working-state |
| `label_usefulness_metrics.jsonl` | same | 549 | `923a618f035eb1b6dd4d6f34ebbce6d9ad035a24a93c8f02a3a1a4d8b1b035a8` | Support / pass-fail lift / precision / recall / stability / closest-exemplar agreement / leakage status per label family |
| `accepted_label_registry.jsonl` | same | 491 | `3846bef398ac9508dfa1f8401b4f4bd19e30d2b19a409e51af5bcc408c3394ff` | The durable Level-0 baseline registry that other systems reference by stable label ID |
| `learning_bridge_manifest.json` | same | 1 | `5fc7e11b4e9480257be4be405131f6ffddff3c88dee8b3bccdbe7d68dd219ab3` | Maps accepted label IDs into UTML/replay consumers (`delta_p.per_chunk_values`, `delta_xi.constellation_violation_rate`, `ReplayBufferRow.cell_id`, `CF_MEJEPA_CONSTELLATION_PATTERNS`, `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`, `CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT`) |
| FSV manifest | `/zfs/archive/contextgraph/fsv/task-py-g-117-auto-label-discovery-usefulness-fsv/python_auto_label_discovery_fsv.json` | 1 | manifest `3cd0e3c6…`, report `3b3e4b0b…` | `passes=true`, `slot_identity_preserved=true`, `flat_vector_concat_used=false` |

**Why JSONL instead of CFs at Level 0:** the label compiler is deterministic and the registry is replayable from inputs (forward cache + chunks.jsonl + oracle index). JSONL keeps the audit surface human-readable, lets the FSV byte-compare against the materialized stream, and avoids a CF schema change on every label-formula version bump (`FORMULA_VERSION = "deterministic_slot_label_compiler_v1"`).

**Level 1/2/3 hierarchical naming and skill-membership CFs (#404 / #414):**

When the hierarchical naming layer lands on top of the shipped Level-0 registry, these CFs are added. They reference `accepted_label_registry.jsonl` row IDs as the stable Level-0 key.

| CF Constant | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_FAILURE_MODE_VOCABULARY` | `vocabulary_id` (e.g. `solution_leak`, `wrong_file`, `type_signature_drift`) | bincode `{name, description, seed_source, embedding_e1, embedding_e7, operator_approved, created_at, updated_at}` | Operator-curated failure-mode vocabulary seeded from published SWE-bench taxonomies (Aleithan et al. 2025 / Yang et al. 2024 / SWE-Bench Pro 2025) plus our 8-mutation operator set plus adversarial corpus. Extensible via `mejepa_label_failure_cluster`. |
| `CF_MEJEPA_FAILURE_MODE_LEVEL1` | `group_id` | bincode `{member_accepted_label_ids: Vec<AcceptedLabelId>, sample_count, top_vocabulary_matches_with_cosine, operator_approved_name, created_at}` | Level-1 groups aggregated over Level-0 accepted labels; each group named via cosine-vs-vocabulary against `CF_MEJEPA_FAILURE_MODE_VOCABULARY`; operator can override the name. |
| `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS` | `skill_id` | bincode `{skill_name, parent_group_ids: Vec<GroupId>, parent_skill_ids: Vec<SkillId>, ordered_step_label_ids: Vec<AcceptedLabelId>, prerequisite_label_ids: Vec<AcceptedLabelId>, transition_edges, support, lift, stability, oracle_outcome_distribution: {pass: u64, fail: u64}, live_input_allowed, promotion_status, operator_approved}` | Level-2 named skills (`patch_breaks_test_isolation`, `mock_only_change`, ...). Maps Level-1 groups and lower skills into ordered, operator-reviewable abilities. |
| `CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS` | `constellation_id` | bincode `{constellation_name, member_skill_ids: Vec<SkillId>, ordered_skill_edges, co_occurrence_count, cell_keys_where_observed: Vec<CellKey>, outcome_distribution, live_input_allowed}` | Level-3 behavioral consequence constellations (ordered compositions of skills that explain pass/fail reality patterns). Examples: `unsafe_security_change`, `incompatible_refactor`. |
| `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` | `(chunk_id, code_state_key, skill_id)` | bincode `{constellation_id, hierarchy_level, membership_score, source_accepted_label_ids, ordered_step_evidence, live_input_allowed, provenance_hashes, first_seen_ts, last_seen_ts}` | Reverse membership rows: every chunk can state which skills/higher-level abilities it participates in before prediction. Many-to-many by design. |
| `CF_MEJEPA_SKILL_REVERSE_INDEX` | `(skill_id, code_state_key)` | bincode `{chunk_ids: Vec<ChunkId>, file_paths: Vec<ProjectRelativePath>, ordered_step_spans, support, latest_membership_hash}` | Fast lookup from a skill/ability to the chunks/files/steps that currently instantiate it in a compiled project reality. |
| `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT` | `skill_audit_id` | bincode `{prediction_id, mistake_id: Option<MistakeId>, previous_skill_id: Option<SkillId>, decision: UpdateExistingSkill|SplitMixedSkill|CreateNewCandidateSkill|DemoteUnstableSkill|NoChangeWithEvidence, candidate_skill_id: Option<SkillId>, evidence_label_ids, evidence_chunk_ids, reason, created_at}` | Append-only proof that wrong predictions repair the skill/ability model or create a candidate instead of silently repeating the same error. |
| `CF_MEJEPA_NAMED_PREDICTIONS` | `prediction_id` | bincode `NamedFailureMode {level1_group, level2_skill, level3_constellation, confidence, closest_exemplars, vocabulary_source}` | Per-prediction named-failure-mode attachment for `Verdict::Fail` rows. Exposed via `mejepa_explain_prediction` / `mejepa_predict_latest`; joins to chunk-skill membership rows for explanation. |

### 9.2 Cross-panel triangulation CFs (#405 / TASK-PY-G-111)

| CF Constant (planned) | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_PANEL_B_OBSERVATIONS` | `(prediction_id, panel_b_run_id)` | JSON-encoded `Panel` (array of 15 per-embedder vectors under Panel B's deliberately non-overlapping encoders, serialized as a 5,120 f32 contiguous buffer + per-slot byte offsets + `filled_mask`; same array-of-vectors semantic unit, distinct encoder model artifacts per learned slot) | Materialized Panel B observation for each prediction Panel A made. Generated by the Panel B worker reading `CF_MEJEPA_LIVE_PREDICTIONS` periodically. |
| `CF_MEJEPA_CROSS_PANEL_AGREEMENT` | `(prediction_id, panel_pair_id)` | bincode `{verdict_a, verdict_b, score_a, score_b, oracle_verdict, agree_flag, cell_key, ts_ns}` | Per-prediction agreement record. Drives the `cross_panel_oracle_correlation` ship-gate metric. Fail-closed flag `CROSS_PANEL_GOODHART_DETECTED` raised when corr(A,oracle) ≥ 0.95 AND corr(B,oracle) < 0.85. |

### 9.3 Mistake-driven online-learning CFs (#406 / TASK-PY-G-112)

| CF Constant (planned) | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_MISTAKE_LOG` | `mistake_id` | bincode `MistakeRow { prediction_id, ts_ns, cell_key, predicted_verdict, ground_truth_verdict, predicted_named_failure_mode, ground_truth_named_failure_mode, source_panel, panel_signature_hash, slot_traces, accepted_label_ids, active_skill_ids, label_signature_hash, skill_signature_hash, skill_lifecycle_decision, skill_lifecycle_audit_id, closest_exemplar_at_predict, ground_truth_source, replayed_to_trainer }` | Append-only log of every prediction that disagreed with reality. Secondary index by `cell_key` and ordered `skill_signature_hash` for the `mistake_repeat_rate` metric. |
| `CF_MEJEPA_MISTAKE_REPLAY_PRIORITY` | `mistake_id` | bincode `{priority: f32, boost_factor: 8.0, scheduled_at: ts_ns, replayed_at: Option<ts_ns>}` | Replay-buffer priority overlay; bridges mistake rows into `CF_MEJEPA_REPLAY_BUFFER` at 8× weight. |
| `CF_MEJEPA_ONLINE_HEAD_STATE` | `head_id` | bincode `{support_set: Vec<SupportPoint>, weights_blob_sha: [u8;32], mistake_count, last_update_ts, mistake_bound_remaining_estimate}` | Persisted Perceptron / Second-Order-Perceptron / Projectron support set + head weights blob. Allows the online head to survive restarts without re-walking the entire mistake log. |

### 9.4 Q4 CF freeze (NOT deleted; deprecated-readable)

Per `05_predictor_pipeline.md §16`, the Q4 prediction track is closed and the following CFs are **frozen** — no new writes, but rows remain readable:

| Frozen CF | Final state | Migration |
|---|---|---|
| `CF_MEJEPA_Q4_PERF_LABELS` | 260 rows frozen 2026-05-19 | Read-only via `mejepa_explain_prediction`; not consumed by predictor |
| `CF_MEJEPA_Q4_ACCURACY_LABELS` | 240 rows frozen | Read-only |
| `CF_MEJEPA_Q4_COST_LABELS` | 240 rows frozen | Read-only |
| `CF_MEJEPA_Q4_REASONING_LABELS` | 300 rows frozen | Read-only |
| `CF_MEJEPA_Q4_PASS_ANNOTATIONS` | 500 rows frozen | Read-only |
| `CF_MEJEPA_Q4_SECURITY_LABELS` | Frozen at last write | Read-only |
| `CF_MEJEPA_HEAD_CALIBRATIONS` (Q4 sub-rows: `q4cal::*`) | Q4 calibration rows frozen | Read-only |
| `CF_MEJEPA_CONTRADICTION_THRESHOLDS` | Frozen | Read-only (Q4-vs-Q2 contradiction no longer ship-gate-relevant) |

`FROZEN_Q4_CFS` in `mejepa-cf/src/lib.rs` is the weekly row-count-delta watch list. `CF_MEJEPA_HEAD_CALIBRATIONS` is mixed-use, so only its Q4 `q4cal::*` sub-rows are frozen by policy and tracked through the `CF_MEJEPA_HEAD_CALIBRATIONS:q4cal::*` counter; non-Q4 calibration rows remain valid. The weekly baseline lives under the active durable root at `state/q4-freeze/q4_frozen_cf_counts.json`, and #408 proved the one-time freeze at `/zfs/archive/contextgraph/fsv/q4-doctrine-freeze-fsv/` with `q4RowCountDelta=0`.

Future cleanup (dropping these CFs in a schema-version bump) tracked under `phase:cleanup`; not in current Phase G scope. Constants stay in `mejepa-cf/src/lib.rs` with a `// FROZEN 2026-05-19: see futurebuild/05 §16` comment.

### 9.4bis Pathway exploration surface CFs (#416 / TASK-PY-G-119, planned)

The butterfly-effect pathway tree (`01_overview.md §1.5`, `05_predictor_pipeline.md §17bis`) adds three CFs. All three are **target-side leakage-policy:target_side_only** for live-input training purposes (the chosen pathway is downstream of the operator's choice, never a live predictor input).

| CF Constant (planned) | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_PATHWAY_TREES` | `(prediction_id, tree_run_id)` | bincode `PathwayTree { root_patch_id, nodes: Vec<PathwayNode>, beam_width, max_depth, pruning_epsilon, formula_version }` where every `PathwayNode.leaf_kind ∈ {Q1, Q2, Q5}` (strict; non-binary leaves rejected at write time) | Full pathway tree per candidate patch; byte-replayable per FSV |
| `CF_MEJEPA_SURFACED_PATHWAYS` | `(prediction_id, pathway_id)` | bincode `SurfacedPathway { leaf_chain: Vec<BinaryLeaf>, joint_probability, closest_historical_pathway, named_failure_mode_if_fail_leaf, cold_cell_warning }` | The top-K (3-5) pathways actually returned to the operator |
| `CF_MEJEPA_OPERATOR_PATHWAY_CHOICES` | `(prediction_id, chooser_id)` | bincode `{chosen_pathway_id, chose_at_ts_ns, rationale_text?}` | Operator's chosen future; feeds #335 `mejepa_reverse_prediction` when shipped |

Strict write-time assertion: any attempt to write a `PathwayNode` with `leaf_kind` outside `{Q1, Q2, Q5}` fails closed with `PATHWAY_AMBIGUOUS_LEAF_REJECTED`.

### 9.4ter Bidirectional skill/ability linkage CFs (#417 / TASK-PY-G-120, planned)

#414 already materialized the base bidirectional rows: `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP`, `CF_MEJEPA_SKILL_REVERSE_INDEX`, and `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT`. #417 exposes those rows as a live capability surface and adds small derived CFs for skill graphs, coverage gaps, cross-repo transfer, and conflict detection.

| CF Constant (planned) | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_SKILL_GRAPH` | `(skill_a_id, skill_b_id)` | bincode `{relation: DependsOn|ComposesWith|PrerequisiteOf, support_count, derived_from: ChunkCallGraph|CoMembership|OrderedSequence, evidence_chunk_ids, latest_membership_hash}` | Skill-of-skills graph. Lets the predictor explain how changing chunks for one ability may affect another ability. |
| `CF_MEJEPA_SKILL_COVERAGE_AUDIT` | `(audit_run_id, chunk_id)` | bincode `{membership_count, gap_class: NoMemberships|OneShotMember|LowConfidence|Covered, active_skill_ids, active_higher_ability_ids}` | Coverage partition over every compiled chunk. Chunks with no memberships become active-learning candidates. |
| `CF_MEJEPA_CROSS_REPO_SKILL_MATCHES` | `(target_project_id, chunk_id, matched_skill_id)` | bincode `{match_confidence, level0_label_signature, live_input_allowed, source_project_id, provenance_hashes}` | Transfers existing skills to newly ingested projects when live-observable label signatures overlap. |
| `CF_MEJEPA_SKILL_CONFLICT_GRAPH` | `(skill_a_id, skill_b_id)` | bincode `{relation: MutuallyExclusive|RareCoFire|ContradictoryOutcome, support_count, co_occurrence_count, evidence_window_id}` | Cross-skill consistency/conflict graph derived from shipped membership co-occurrence and outcome distributions. |
| `CF_MEJEPA_PATHWAY_SKILL_ATTACHMENTS` | `(prediction_id, pathway_id, leaf_id)` | bincode `{active_skill_ids, active_higher_ability_ids, source_chunk_membership_keys, unknown_reason: Option<UNKNOWN_PATHWAY_SIGNATURE>, live_input_allowed}` | Joins #416 binary pathway leaves to concrete chunk/skill evidence without adding ambiguous leaf types. |

No #417 row may use target-side oracle/Docker/test labels as live prediction inputs. Target-side labels may only score, calibrate, promote/demote, or update skill lifecycle rows after reality responds.

### 9.5 Canonical opener slice update

When CFs land, they get added to `ALL_HYGIENE_REFERENCED_CFS` (`mejepa-cf/lib.rs`). #414 registered `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP`, `CF_MEJEPA_SKILL_REVERSE_INDEX`, and `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT` in `FAILURE_MODE_CFS`, `INFER_CFS`, `EVAL_CFS`, `HYGIENE_CFS`, and the canonical all-CF opener. The `canonical_hygiene_cfs_match_group_union` test now explicitly includes `FAILURE_MODE_CFS` and continues to enforce that the grouped CF contract matches `ALL_HYGIENE_REFERENCED_CFS`.

#414 production FSV (`/zfs/archive/contextgraph/fsv/task-py-g-118-skill-sequence-discovery-fsv/run-1779213075188/task_py_g_118_aiwonder_skill_materialization_fsv.json`, `sha256:0df1437ae5e0542b356777b29df700b521b83b054113697ca280ae3d242d3dee`) streamed all 399,777 #413 chunk-label rows and wrote/read back 128 `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS` rows, 36,773 `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` rows, 10,579 `CF_MEJEPA_SKILL_REVERSE_INDEX` rows, and 128 `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT` rows. External aiwonder `ldb` readback with `LD_LIBRARY_PATH=/zfs/hot/contextgraph/lib` matched the reopened Rust counts exactly.

Because adding a CF is a schema change and ME-JEPA has no migrations (`CLAUDE.md §6.6`), landing #404/#405/#406 means a deliberate DB wipe + rebuild. The wipe is scheduled as a single combined operation rather than 3 separate wipes.
