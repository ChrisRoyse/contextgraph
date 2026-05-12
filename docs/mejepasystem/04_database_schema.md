# 04 — Database Schema (RocksDB Column Families)

Source files covered: `crates/context-graph-storage/src/column_families.rs`, `crates/context-graph-storage/src/lib.rs`, `crates/context-graph-mejepa-cf/src/lib.rs`, `crates/context-graph-mejepa-hygiene/src/storage.rs`, `crates/context-graph-storage/src/indexes.rs`, `crates/context-graph-storage/src/teleological/`, `crates/context-graph-storage/src/dynamicjepa/`, `crates/context-graph-storage/src/graph_edges/`, `crates/context-graph-storage/src/code/`, `crates/context-graph-mejepa/src/eval/store.rs`, `crates/context-graph-mejepa-shift-subscriber/src/watermark.rs`, `crates/context-graph-mejepa-tct/src/store.rs`, `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md §5`, `docs/futurebuild/07_TRAINING_DATA_FLYWHEEL.md §9`, `docs/ruvectorfindings/specs/_README.md`.

> **Total column families: 101** across the workspace (28+ ME-JEPA-specific). The canonical CF name registry lives in `crates/context-graph-mejepa-cf/src/lib.rs` (dependency-free crate). Other CFs are declared in `crates/context-graph-storage/src/column_families.rs`.

---

## 1. Storage backend

| Property | Value |
|---|---|
| Engine | RocksDB 0.21.x |
| Hybrid backend | RocksDB + AgentDB per `CLAUDE.md §4` |
| HNSW indexes | enabled (15 indexes: E1 + E1Matryoshka128 + E2/E3/E4 + E5+E5Cause+E5Effect (retired) + E7 + E8 + E9 + E10 + E10Multimodal + E11(retired) + E14) |
| Path | `/mnt/d/contextgraph/storage/contextgraph-rocksdb/` (per `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`) |
| Tier compression | hot 8-bit / warm 7-bit / cool 5-bit / cold 3-bit / frozen 1-bit per `context-graph-tier-compression` |
| Per-CF access | direct via `db.cf_handle(name).ok_or(...)` — fail-closed on missing CF |
| Quota | 115 GB hard cap on entire `/mnt/d/contextgraph/` |
| Permissions | `0600` on persisted JSON files |

---

## 2. Connection management

Source: `crates/context-graph-mcp/src/main.rs`, `crates/context-graph-mejepa-hygiene/src/storage.rs::open_hygiene_rocksdb`.

```rust
// Pseudocode of daemon startup
let opts = rocksdb::Options::default();
opts.create_if_missing(true);
opts.create_missing_column_families(true);

let cf_descriptors: Vec<ColumnFamilyDescriptor> = ALL_CF_NAMES
    .iter()
    .map(|name| ColumnFamilyDescriptor::new(*name, rocksdb::Options::default()))
    .collect();

let db = rocksdb::DB::open_cf_descriptors(&opts, path, cf_descriptors)?;
```

**No CF auto-create at runtime**. Schema is fully known at compile-time via the registry crate. RocksDB requires ALL CFs in the on-disk store to be opened on every connection — adding a CF requires a workspace-wide rebuild AND a one-time first-start that creates the new CF.

**No connection pooling**: RocksDB DB is wrapped in `Arc<rocksdb::DB>` and shared across tokio tasks. Per-CF write batches via `WriteBatch` for atomic multi-CF writes.

---

## 3. Schema versioning strategy

Per `CLAUDE.md` + `memory/feedback_no_backwards_compat.md`:

- **Pre-production**: synthetic data only. **Wipe DB on schema change.** No migration shims.
- **Schema changes are tracked** via the `EMBEDDING_REGISTRY` CF (per-fingerprint model-version tags) so stale embeddings can be re-embedded.
- **`provenance.predictor_version`** on every `RealityPrediction` and `TrainingCertificate` enables version-aware downstream consumption.
- **Post-1.0**: explicit migration tools required (`TASK-API-002` in futurebuild Phase F).

---

## 4. Column families (all 101)

### 4.1 ME-JEPA-specific CFs (28+; canonical registry in `context-graph-mejepa-cf`)

| CF | Key | Value (serde JSON unless noted) | Purpose | Status |
|---|---|---|---|---|
| `CF_MEJEPA_PANELS` | `(session_id, panel_id)` | tier-compressed Panel (5,120-d) + metadata | Per-(attempt, time_step) panel | shipped |
| `CF_MEJEPA_PANEL_META` | `panel_id` | `{ tier, access_count_30d, created_at, last_accessed }` | Tier policy state for `tier_for_score` | shipped |
| `CF_MEJEPA_WEIGHT_BLOBS` | `theta_sha` | weight blob (deduplicated by SHA) | Predictor weights + LoRA adapter variants | shipped |
| `CF_MEJEPA_TRAIN_CERTS` | `step` (u64 BE) | TrainingCertificate header (ΔP/ΔK/ΔΩ/ΔΞ + components + grad_norm + drift_coverage + merkle_root + pairwise_mi columns + per-head delta columns) | Hash-chained per-step audit trail | shipped |
| `CF_MEJEPA_ORACLE_VERDICTS` | `(task_id, mutation_category)` or `(session_id, test_id, ts)` | `OracleVerdict { resolved, F2P, P2P, F2F, P2F, duration_ms }` | Docker oracle outcomes + live test outcomes | shipped |
| `CF_MEJEPA_MUTATION_CORPUS` | `(task_id, mutation_category)` | mutated source + before/after | Phase 0 corpus rows | shipped |
| `CF_MEJEPA_LIVE_PREDICTIONS` | `session_id` (latest) + (session_id, prediction_id) (history) | `RealityPrediction` | Latest verify() output per session | shipped |
| `CF_MEJEPA_SHIFT_WATERMARK` | `session_id` | `{ byte_offset, last_shift_id, ts }` | Restart-safe shift subscriber cursor | shipped |
| `CF_MEJEPA_AGENT_FEEDBACK` | `(prediction_id, agent_id, ts)` | `SurpriseEvent { kind, severity, agent_explanation, actual_outcome, witness_hash, sampling_weight_bump }` | Agent-side feedback ledger | **shipped (2026-05-11)** |
| `CF_MEJEPA_FAILURE_EXEMPLARS` | `(failure_mode_class, embedder_violation_signature)` | `Vec<ExemplarRef { task_id, mutation_kind, chunk_id, oracle_outcome, witness_hash }>` | Per-failure-mode reverse index | **shipped (2026-05-11)** |
| `CF_MEJEPA_DDA_SIGNALS` | `(panel_id, chunk_id)` | `DdaSignals { per_embedder_cosine: [f32; N], pairwise_cosine: [[f32; N]; N], pairwise_mi: [[f32; N]; N] }` | n·(N+N²) signal cache per chunk | **shipped (2026-05-11)** |
| `CF_MEJEPA_CONSTELLATION` | `(version_id, category, entity_kind, language, embedder_id)` | `ConstellationCell { centroid, bayesian_shrunk, n_supporting, tau_m, freshness_ts, corpus_provenance }` | TCT per-cell centroids | shipped |
| `CF_MEJEPA_FISHER_SNAPSHOTS` | `(checkpoint_id, head_id)` | `FisherSnapshot { trunk_fisher, per_head_fisher_map, witness_hash, ts, source_examples_window }` | EWC++ Fisher matrices | shipped |
| `CF_MEJEPA_DRIFT_WINDOW` | `(scope, timestamp)` | rolling-1000 metrics | Drift detector window state | shipped |
| `CF_MEJEPA_DRIFT_HISTORY` | `(timestamp, drift_class)` | historical drift events | Per-cell drift trends | shipped |
| `CF_MEJEPA_HEAL_REPORTS` | `(report_id, severity)` | `HealReport { intervention_kind, before_metric, after_metric, witness_hash, operator_alert_required }` | Self-heal action audit | shipped |
| `CF_MEJEPA_DISTILL_STEPS` | `(distillation_id, step)` | per-step distillation metrics | aux-head distillation history | shipped |
| `CF_MEJEPA_ACTIVE_POINTERS` | `pointer_name` (e.g., `"production"`, `"rollback_30d"`) | `ActivePointer { theta_sha, calibration_version, constellation_version, promoted_at, witness_hash }` | A/B/C variant pointers | shipped |
| `CF_MEJEPA_EVAL_REPORTS` | `(report_id, week_iso)` | `EvalReport { per_cell_correlations, conformal_coverage, ood_auc, ship_gate_passed, ship_gate_failures }` | Weekly evaluation reports | shipped |
| `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` | `(priority, entry_id)` | `ActiveLearningEntry { kind, severity, prediction_id, chunk_id, surfaced_at }` | High-priority queue (uncertain + AgentSurprise) | shipped |
| `CF_MEJEPA_PATCH_GRAPH` | `(patch_id_a, patch_id_b)` | `PatchGraphEdge { similarity_score, embedder_breakdown }` | Patch-similarity graph for cross-task transfer | shipped |
| `CF_MEJEPA_CORPUS_PROVENANCE` | `corpus_id` | `{ corpus_sha, embedder_versions, frozen_at, code_version }` | Corpus + constellation provenance | shipped |
| `CF_MEJEPA_EMBEDDING_REGISTRY` | `(fingerprint_id, embedder_id)` | `{ model_version, quantization, generated_at }` | Stale-embedding detection | shipped |
| `CF_MEJEPA_LEARNER_STATE_HEAL_REPORTS` | `(scheduler_tick, report_id)` | readback-verified scheduler status | Phase A skeleton state | shipped |
| `CF_MEJEPA_RUN_REPORTS` | `(eval_run_id, ts)` | `EvalRunReport` from `mejepa_eval_run` | One-off + cron eval results | shipped |
| `CF_MEJEPA_SHIP_GATE_STATUS` | `(week_iso, cell_id)` | per-cell ship-gate status | `mejepa_ship_gate_status` MCP | shipped |
| `CF_MEJEPA_STATE_TRANSFER` | `(week_iso, cell_id)` | `StateTransferDiagnostic { T_cell, performance_deploy_cell }` | REQ-UTML-08 per-cell T | shipped |

### 4.2 Memory / fingerprint / embedder CFs

Source: `crates/context-graph-storage/src/column_families.rs` + `crates/context-graph-storage/src/teleological/`.

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEMORY_FINGERPRINTS` | `memory_uuid` | `TeleologicalFingerprint` (13 embedders + provenance) | Primary memory store |
| `CF_FINGERPRINT_INDEX` | secondary key | UUID list | Indexes for fast lookup |
| `CF_E1_E5_LARGE` | `memory_uuid` | E1 vector (1024-d) | E1 e5-large-v2 |
| `CF_E2_TIME_PE` | `memory_uuid` | E2 vector (512-d) | E2 sinusoidal time-PE |
| `CF_E3_RECURRENCE_PE` | `memory_uuid` | E3 vector (512-d) | E3 recurrence cycle PE |
| `CF_E4_SESSION_PE` | `memory_uuid` | E4 vector (512-d) | E4 session position PE |
| `CF_E5_CAUSAL_CAUSE` | (retired) | (deserialize-fail or empty) | E5 retired post-pivot |
| `CF_E5_CAUSAL_EFFECT` | (retired) | (retired) | E5 retired |
| `CF_E6_SPARSE_KEYWORD` | `memory_uuid` | SPLADE-style sparse vector | E6 BM25-like |
| `CF_E7_QODO_CODE` | `memory_uuid` | E7 vector (1536-d) | E7 Qodo code |
| `CF_E8_GRAPH` | `memory_uuid` | E8 vector (1024-d) | E8 graph-attention |
| `CF_E9_HDC` | `memory_uuid` | E9 vector (1024-d) | E9 HDC binary |
| `CF_E10_PARAPHRASE` | `memory_uuid` | E10 vector (1024-d) | E10 doc-asym |
| `CF_E10_MULTIMODAL_PARAPHRASE` | `memory_uuid` | E10 vector (1024-d) | E10 multimodal asym |
| `CF_E11_KEPLER_ENTITY` | (retired) | (retired) | E11 retired |
| `CF_E12_COLBERT` | `memory_uuid` | per-token matrix | E12 ColBERT |
| `CF_E13_SPLADE` | `memory_uuid` | sparse expansion | E13 SPLADE |
| `CF_E14_BGE_M3_DENSE` | `memory_uuid` | E14 vector (1536-d) | E14 second-opinion code |
| `CF_AUDIT_LOG` | `(memory_uuid, ts)` | `AuditRecord { operation: AuditOperation, result: AuditResult, operator_id, ... }` | Append-only audit log |
| `CF_TOOL_CALL_INDEX` | `tool_use_id` | memory list | Tool → memory mapping |
| `CF_MERGE_HISTORY` | `merged_uuid` | merge lineage (permanent) | Merge provenance |
| `CF_IMPORTANCE_HISTORY` | `(memory_uuid, ts)` | importance delta | Importance over time |
| `CF_ENTITY_PROVENANCE` | `entity_id` | `EntityProvenance { source_memory_id, extraction_method, confidence, transe_score }` | E11 entity provenance |
| `CF_SOURCE_METADATA` | `memory_uuid` | source metadata (JSON, NOT bincode) | Per-memory provenance |
| `CF_SESSION_INDEX` | `session_id` | sequence index | Per-session ordering |

### 4.3 Teleological / training / learning CFs

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_TELEOLOGICAL_FINGERPRINTS` | `memory_uuid` | full TeleologicalFingerprint | Teleological store |
| `CF_TELEOLOGICAL_SOURCE_OF_TRUTH` | `memory_uuid` | source-of-truth + signature | Phase 2 source-of-truth artifacts |
| `CF_TRAINING_RECORDS` | `uuid` | `TrainingRecord { content, embeddings, labels }` | `export_training_corpus` output |
| `CF_LEARNING_EVENTS` | `uuid` | `LearningEvent { before, after, context, ts }` | UTL Learning-as-UTL event ledger |
| `CF_LEARNING_SIGNAL_EMBEDDINGS` | `(event_uuid, embedder_id)` | learning signal vector | Deterministic signal embeddings |
| `CF_LEARNER_TRAINING_DATASETS` | `uuid` | matrix-shaped dataset | `export_learner_training_dataset` output |
| `CF_LEARNER_PROFILE` | `learner_id` | `{ modality, profile }` | Learner-state registration |
| `CF_LEARNER_SESSION_OBSERVATIONS` | `(learner_id, session_id, ts)` | state vector + observation | UTL session observations |
| `CF_LEARNER_DELTA_S` | `session_observation_uuid` | UTL Δ S surprise signal | Per-observation surprise |
| `CF_LEARNER_DELTA_C` | `session_observation_uuid` | UTL Δ C coherence | Per-observation coherence |
| `CF_LEARNER_DELTA_E` | `session_observation_uuid` | UTL Δ E embodied state | Per-observation embodied |
| `CF_LEARNER_L_TRACE` | `(learner_id, ts)` | L = ΔS · ΔC · ΔE | Composite UTL L |
| `CF_LEARNER_M_TRACE` | `(learner_id, ts)` | M(t) trace | Model state at time t |
| `CF_LEARNER_NEXT_REVIEW` | `trace_uuid` | next-review timestamp | Spaced retrieval |
| `CF_LEARNER_K_SLEEP` | `(learner_id, ts)` | sleep-derived k(τ) | Consolidation rate |
| `CF_LEARNER_RETRIEVAL_HISTORY` | `(learner_id, trace_uuid, ts)` | retrieval outcome | Retrieval practice |
| `CF_LEARNER_GOAL_CENTROID` | `(learner_id, skill_id)` | expert goal centroid | Per-skill regulated baseline |
| `CF_LEARNER_CONSTELLATION` | `learner_id` | regulated-state baseline | Phase 2 learner constellation |
| `CF_LEARNER_RETRIEVAL_POLICY` | `(learner_id, session_id)` | state-conditioned weight profile | Retrieval policy resolution |

### 4.4 Constellation / contrastive / typed-edge CFs

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_CONSTELLATIONS` | `uuid` | per-embedder centroids + spread stats | Phase 2 constellation compiler |
| `CF_CONSTELLATIONS_INDEX` | secondary | uuid list | Constellation selector |
| `CF_CONTRASTIVE_PAIRS` | `(anchor_id, negative_id)` | cross-embedder anomaly pair | Phase 3 miner output |
| `CF_TYPED_EDGE_RECORDS` | `(source, target, edge_type)` | `TypedEdgeTrainingRecord` | F1 training corpus |
| `CF_TYPED_EDGE_SECONDARY_INDEX` | `[type(1)][source(16)][target(16)]` (33 bytes) | edge | Direction-aware lookup (multi-edge-safe per BUG-3 fix) |
| `CF_GRAPH_LEARNING_EVENTS` | `event_uuid` | `{ source, target, path, traversal_time, success }` | Graph navigation outcomes |
| `CF_GRAPH_LEARNING_POLICIES` | `policy_id` | edge ranking policy | Learned from graph events |
| `CF_CAUSAL_RELATIONSHIPS` | `(source_uuid, target_uuid)` | causal edge | Causal graph (E5 retired but CF kept for repair) |

### 4.5 Code-store CFs

Source: `crates/context-graph-storage/src/code/`.

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_CODE_FILES` | `(repo_id, path)` | file content + SHA + git metadata (commit hash, author, branch, timestamp) | Code file index |
| `CF_CODE_CHUNKS` | `chunk_id` | AST-perfect chunk + language + kind + byte_range + line_range | Atomic chunk store |
| `CF_CODE_CHUNK_INDEX` | secondary | chunk_id list | Per-file chunk index |
| `CF_CODE_E7_EMBEDDINGS` | `chunk_id` | E7 (Qodo) vector | Code semantic |
| `CF_CODE_E12_EMBEDDINGS` | `chunk_id` | E12 (ColBERT) per-token matrix | Code per-token |

### 4.6 DynamicJEPA CFs (44 ops surface)

Source: `crates/context-graph-storage/src/dynamicjepa/`. Prefix: `CF_DJ_*`.

| CF | Purpose |
|---|---|
| `dj_domain_packs` | Registered TOML domain packs |
| `dj_events_raw` | Raw event ingestion |
| `dj_instrument_readings` | Per-event instrument readings |
| `dj_latent_panels` | Materialized latent panels |
| `dj_bindings` | Deterministic source-of-truth bindings |
| `dj_trajectories` | Compiled panel/transition trajectories |
| `dj_dataset_shards` | One-step dataset shards |
| `dj_training_runs` | Training run records |
| `dj_artifacts` | DynamicJEPA model artifacts (hash-verified files) |
| `dj_predictions` | Hash-verified predictions |
| `dj_plan_traces` | Plan candidates + guard evaluations |
| `dj_surprises` | Surprise events (prediction vs observed) |
| `dj_constellations` | DynamicJEPA constellations |
| `dj_thresholds` | G_τ threshold calibrations |
| `dj_audit_log` | Audit log (signal_yield, MC-ratio evidence) |
| `dj_pairwise_mi` | Pairwise MI estimates |
| `dj_semantic_indexes` | Compiler/LSP-backed semantic indexes |
| `dj_corpus_diversity` | Real corpus diversity validation |
| `dj_test_deltas` | Verifier/test delta attribution |
| `dj_shadow_utility` | Candidate-vs-active shadow utility |

### 4.7 Embedder / clustering / topic CFs

| CF | Purpose |
|---|---|
| `CF_TOPICS` | Discovered topics with profiles + stability metrics (cap 10,000) |
| `CF_TOPIC_MEMBERSHIP` | Memory→topic membership (pruned on eviction) |
| `CF_WEIGHT_PROFILES` | Session-scoped custom embedder weight profiles (cap 1,000) |
| `CF_CUSTOM_PROFILES` | User custom profiles |
| `CF_FILE_HASHES` | Per-file SHAs (cleaned on deletion) |
| `CF_FILE_WATCHER_STATE` | File-watcher inode + last-scan + queue depth |
| `CF_SNAPSHOTS` | DB snapshots (cap 1,440 = 24h × minutely; 30d retention aligned to 7d) |
| `CF_SYSTEM` | system state (soft-delete tombstones, etc.) |

### 4.8 ME-JEPA eval-specific CFs (9 in `RocksDbEvalStore`)

Source: `crates/context-graph-mejepa/src/eval/store.rs`.

| CF | Purpose |
|---|---|
| `CF_MEJEPA_EVAL_RUN_INDEX` | Per-run-id index |
| `CF_MEJEPA_EVAL_HOLDOUT_RESULTS` | Per-cell holdout correlations |
| `CF_MEJEPA_EVAL_CONFORMAL_HEALTH` | Per-language conformal coverage |
| `CF_MEJEPA_EVAL_OOD_AUC` | Per-language OOD AUC |
| `CF_MEJEPA_EVAL_GTAU_RATE` | Per-language Gτ pass rate |
| `CF_MEJEPA_EVAL_SHIP_GATE_FAILURES` | Per-cell ship-gate fail entries |
| `CF_MEJEPA_EVAL_AUX_DISTILL` | Aux-head distillation summary |
| `CF_MEJEPA_EVAL_STATE_TRANSFER` | Per-cell state-transfer T |
| `CF_MEJEPA_EVAL_PATCH_GRAPH_META` | Patch-similarity graph metadata |

---

## 5. Indexes

Source: `crates/context-graph-storage/src/indexes.rs`.

### 5.1 HNSW vector indexes (15)

Each HNSW index is stored separately as a `.hnsw` file alongside RocksDB:

| Index | Dim | M (graph degree) | efConstruction | efSearch (default) | MAX_VECTORS |
|---|---|---|---|---|---|
| E1 | 1024 | 32 | 200 | 50 | 2,000,000 |
| E1Matryoshka128 | 128 | 32 | 200 | 50 | 2,000,000 |
| E2 (sinusoidal) | 512 | 16 | 100 | 30 | 2M |
| E3 (recurrence) | 512 | 16 | 100 | 30 | 2M |
| E4 (session) | 512 | 16 | 100 | 30 | 2M |
| E5CausalCause (retired) | 768 | 32 | 200 | 50 | — |
| E5CausalEffect (retired) | 768 | 32 | 200 | 50 | — |
| E7 (Qodo code) | 1536 | 32 | 200 | 50 | 2M |
| E8 (graph) | 1024 | 32 | 200 | 50 | 2M |
| E9 (HDC) | 1024 | 16 | 100 | 30 | 2M |
| E10 (paraphrase) | 1024 | 32 | 200 | 50 | 2M |
| E10MultimodalParaphrase | 1024 | 32 | 200 | 50 | 2M |
| E10MultimodalContext | 1024 | 32 | 200 | 50 | 2M |
| E11 (retired) | 768 | 32 | 200 | 50 | — |
| E14 (BGE-M3 / CodeT5+) | 1536 | 32 | 200 | 50 | 2M |

### 5.2 Inverted indexes

| Index | Used for |
|---|---|
| E6 sparse keyword | BM25 search (`search_by_keywords`) |
| E13 SPLADE expansion | term-expansion search (`search_by_expansion`) |

### 5.3 Secondary indexes (RocksDB CFs used as indexes)

| Index CF | Key | Maps to |
|---|---|---|
| `CF_TYPED_EDGE_SECONDARY_INDEX` | `[type(1)][source(16)][target(16)]` | edges (multi-edge-safe; 33-byte key prevents collisions when same source has multiple same-type edges) |
| `CF_TOOL_CALL_INDEX` | `tool_use_id` | memory list |
| `CF_FINGERPRINT_INDEX` | misc secondary keys | UUIDs |
| `CF_SESSION_INDEX` | `session_id` | sequence index |
| `CF_CODE_CHUNK_INDEX` | `(repo_id, path)` | chunk_id list |

---

## 6. Special features

### 6.1 Tier compression (per CF policy)

Source: `crates/context-graph-mejepa-hygiene/src/tier.rs`.

| Tier | Bits | Compression ratio | Cosine retention (768-d Gaussian) | Trigger |
|---|---|---|---|---|
| Hot | f32 raw | 1× | 1.000 | `access_count_30d > 100` |
| Warm | 8-bit groupwise | 3.89× | 0.99999 | `30 < access_count_30d ≤ 100` |
| Cool | 5-bit groupwise | 6.12× | 0.99960 | `5 < access_count_30d ≤ 30` |
| Cold | 3-bit groupwise | 9.91× | 0.99192 | `access_count_30d ≤ 5` |
| Frozen | 1-bit or marked | n/a | n/a | constellation cells past freshness; locked baselines |

Header: 22 bytes (magic + version + min/max/scale per group). `decode_from_tier` reverses; round-trip verified in FSV. Source: `crates/context-graph-tier-compression/src/lib.rs`.

### 6.2 Witness chain (append-only)

Every persistent artifact (panel, training cert, prediction, feedback event) binds a `witness_hash`. Per `crates/context-graph-witness/`:

- 73-byte SHAKE-256 chain entries
- Ed25519 segment signing (feature-gated `ed25519`)
- Stage 0 / Stage F certificates
- Merkle compression for chains > 90 days
- Hash chain: `entry_hash_n+1 = SHAKE-256(entry_hash_n || payload_n+1)`

### 6.3 Source-of-truth metadata

Every JSON in the storage tree includes:

```json
{
  "source_of_truth": "file:/mnt/d/contextgraph/storage/.../entry.json",
  "sha256": "<sha>",
  "ts": "<iso8601>",
  "witness_hash": "shake256:73:<hex>",
  "predictor_version": "v0.4.2-bf16-cuda13.2",
  "code_version": "<git-sha>"
}
```

Error messages and recommendations always begin path references with `file:` (or `sqlite:` / `git:`).

### 6.4 Path-policy gate (REMOVED 2026-05-07)

`crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs::ensure_path_allowed` is gutted: it now rejects ONLY empty paths or paths containing control characters. The deny-list (which previously blocked writes to many paths) is GONE. Path discipline is now **behavioral** per `CLAUDE.md §8` — the operator trusts the agent not to write to `.env`, `swebench/**`, `tests/oracle_**/**`, etc.

---

## 7. Query helper functions

Source: `crates/context-graph-storage/src/lib.rs`.

| Function | Purpose |
|---|---|
| `get_memory_by_uuid(db, uuid) -> Option<TeleologicalFingerprint>` | Primary key lookup |
| `iter_cf_prefix(db, cf, prefix) -> Iter` | Prefix scan |
| `batch_write_atomic(db, batch: WriteBatch)` | Multi-CF atomic write |
| `cf_count(db, cf) -> u64` | Per-CF row count |
| `cf_handle(db, cf_name) -> ColumnFamily` | Fail-closed CF handle resolver |
| `cf_iter_decoded::<T: DeserializeOwned>(db, cf) -> impl Iterator<Item=Result<T>>` | Type-safe decoding |

Per ME-JEPA CF lifecycle:

| Function | Purpose |
|---|---|
| `open_hygiene_rocksdb(d_root) -> rocksdb::DB` | Daemon's standard RocksDB open with all CFs |
| `persist_panel(db, panel)` | Write Panel + meta + DDA signals atomically |
| `read_latest_prediction(db, session_id) -> Option<RealityPrediction>` | Per-session latest |
| `enqueue_active_learning(db, entry)` | Priority-queue enqueue |
| `record_agent_feedback(db, surprise_event)` | Multi-CF write: feedback + sampling-weight bump + queue entry |

---

## 8. Migrations (when needed)

Per `CLAUDE.md` pre-production: **wipe DB on schema change**. Post-1.0 (`TASK-API-002` in `docs/futurebuild/24_API_STABILITY.md`):

| Migration class | Algorithm |
|---|---|
| New CF added | Auto-created via `create_missing_column_families=true` on first start |
| CF dropped | Remove from registry crate; old data orphaned (RocksDB tolerates extra CFs on disk if `error_if_exists=false`) |
| CF schema change (key or value layout) | Wipe DB; document in `memory/decisions/<date>--<schema-change>.md` per CLAUDE.md §10.3 |
| Embedder version change | Re-embed via `CF_MEJEPA_EMBEDDING_REGISTRY` stale-detection + `backfill-e14` style backfill |
| Constellation rebuild | Force-refit via `mejepa_constellation_inspect --force-refit` |

---

## 9. Storage budget per CF (illustrative)

Per `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md §9` at 30k panels.

| Data class | Per-entity size | At scale | Compressed | After 90d cold |
|---|---|---|---|---|
| Panel (5,120-d × f32) | 20 KB | 600 MB | 8-bit hot 5 KB / 5-bit cool | 150 MB cold |
| DDA signals per chunk | ~240 B | ~30k × 30 chunks × 240 B = 216 MB | 5-bit cool 67 B | 60 MB cold |
| Per-embedder vectors per chunk (cached) | 5 × 1024 × f32 = 20 KB | 18 GB | 5 KB / chunk cool | 4.5 GB cold |
| Constellation cells (13,728 max) | ~1 KB | 13 MB | n/a | 13 MB |
| Training certs | ~2 KB / step | 100k × 2 KB = 200 MB | n/a | 200 MB |
| Weight blobs (deduplicated) | ~30 MB / variant | 10 variants = 300 MB | n/a | 300 MB |
| Witness chain | 73 B / step | 100k × 73 = 7 MB | Merkle compress @ 90d | 1 MB |
| Live predictions | ~5 KB | bounded by session count | tier_demote(promote) | 100 MB warm |
| Agent feedback events | ~1 KB | bounded ~10k | n/a | 10 MB |
| **Total** | — | ~19 GB hot | — | ~5.4 GB cold |

Headroom: 30 GB hot + 70 GB cold = ~100 GB under the 115 GB hard quota.

---

## 10. Locked schema rules

1. **CF names live in `context-graph-mejepa-cf`** (dependency-free) to prevent drift.
2. **JSON, NOT bincode**, for any value that uses `serde::Serialize` with `skip_serializing_if` (per past bug: `bincode + skip_serializing_if = broken`).
3. **Soft delete writes to `CF_SYSTEM` prefix** (per past bug: in-memory HashMap was lost on restart).
4. **Multi-edge keys must include target** (`[type(1)][source(16)][target(16)]` = 33 bytes per BUG-3 fix in `graph_link_tools.rs`).
5. **Always sort inverted-index lists** on update (per past bug: concurrent ops corrupted unsorted lists in `remove`).
6. **Zero-norm vectors rejected** (`STOR-M1`).
7. **PQ-8 must store actual min/max** (per past bug: hardcoded `[-1, 1]` corrupted dequantization).
8. **Importance clamping at handler level** (constructor clamp bypassed by serde deserialization).
9. **session_id filtering happens in post-filter** (not in store layer) via `source_metadata.session_id`.
10. **AgentDB CFs share the same `/mnt/d/contextgraph/storage/contextgraph-rocksdb/`** in the hybrid backend; do not split.

---

## 11. What is NOT a CF

- The shift log JSONL (`/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl`) — append-only file, not a CF.
- Embedder model artifacts (`/mnt/d/cabdru/contextgraph/models/`) — filesystem, not CF.
- FSV evidence (`/mnt/d/contextgraph/fsv/`) — filesystem, mode 0600.
- Operator-facing exports (`/mnt/d/contextgraph/exports/`) — filesystem (markdown / CSV / JSON).
- BlackBox ledger (`~/.blackbox/`) — separate SQLite.
- HNSW index files — separate `.hnsw` files alongside RocksDB.
- Daemon logs (`/mnt/d/contextgraph/logs/`) — rotated daily.
