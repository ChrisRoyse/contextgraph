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
| `CF_MEJEPA_CLAIM_RECONCILIATION` | `(prediction_id, claim_id)` | JSON | TASK-PY-G-041 Q1 claim graph |
| `CF_MEJEPA_PROJECT_REPORTS` | `project_id` | JSON `ProjectReport` | TASK-PY-G-064 per-project reality report |
| `CF_MEJEPA_STRESS_RUNS` | `(run_id, case_id)` | JSON | TASK-PY-G-072 end-to-end stress harness |
| `CF_MEJEPA_SYNTHETIC_STRESS_RESULTS` | `(run_id, case_id)` | JSON | TASK-PY-G-071 synthetic 30-case stress |
| `CF_MEJEPA_TRAIN_CERTS` | `cert_id (sha)` | JSON `TrainingCertificate` (incl. `conditional_description_length_bits` per TASK-EK-001) | Training step certificates |
| `CF_MEJEPA_PANELS` | `panel_id` | JSON-encoded `Panel` (5,120 f32 + `filled_mask`) | Materialized panels |
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
| `CF_MEJEPA_OOD_CALIBRATIONS` | `(harvest_run_id, calibration_id)` | JSON | TASK-PY-G-057 OOD calibration reports + false-positive-rate audit (e.g. `OOD_GATE_OVER_FLAGGING` flag) |

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
| `CF_MEJEPA_OOD_CALIBRATIONS` | OOD calibration reports per harvest run; surfaces flags like `OOD_GATE_OVER_FLAGGING` when the false-positive rate is too high | TASK-PY-G-057 / #276 |

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

GitHub issue #363 (closed, `type:decision`) — supersedes the pre-migration D-drive production policy.

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
| Write production FSV evidence | ✅ `/zfs/archive/...fsv/<task>-fsv/` | ❌ | ❌ | ❌ unless issue-scoped local smoke |
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
