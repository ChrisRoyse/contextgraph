# 18 — MCP Tool Reference

Source files covered: `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`, `crates/context-graph-mcp/src/handlers/tools/names.rs`, `crates/context-graph-mcp/src/handlers/tools/`, `docs/futurebuild/08_LIVE_PREDICTION_AGENT_API.md`, `docs/ruvectorfindings/specs/_README.md`, `.claude/loop.md §4.2`.

> **206 MCP tools total** registered in `dispatch.rs:137-375`. 19 are ME-JEPA-specific (the live surface); the rest cover memory, embedder, search, learning, learner-state, DynamicJEPA, reality-loop / ccreality, autoresearch, provenance, daemon.

---

## 1. ME-JEPA tools (19 live, all shipped)

| Tool | Behavior | Key parameters | Status |
|---|---|---|---|
| `mejepa_verify` | Take patch + context, return full `RealityPrediction` (Q1–Q5) | `patch` (unified diff or file_changes), `context { session_id, task_id?, repo_root, agent_response? }`, `include_provenance`, `include_explanations` | shipped |
| `mejepa_predict_latest` | Latest prediction for session from `CF_MEJEPA_LIVE_PREDICTIONS` | `session_id`, `max_age_seconds?` | shipped |
| `mejepa_constellation_inspect` | TCT cell lookup + force refit | `constellation_id?`, `category?`, `language?`, `force_refit`, `force_allow_stale` | shipped |
| `mejepa_heal_status` | Drift level + queued interventions + last A/B/C result | — | shipped |
| `mejepa_rollback_to` | Operator-forced rollback to prior model variant | `witness_offset`, `reason?` | shipped |
| `mejepa_load_learner_state` | Load saved learner-state (E15–E21) snapshot | `learner_id`, `checkpoint_id?` | shipped |
| `mejepa_routing_lookup` | Query (Language × EntityType) → embedder set | `language`, `entity_type` | shipped |
| `mejepa_vram_budget_report` | GPU VRAM accounting vs budget | — | shipped |
| `mejepa_gc_run` | Force nightly GC run | `target_cfs?`, `force_eviction?` | shipped |
| `mejepa_quota_status` | Disk usage vs 115 GB cap + per-slice breakdown | — | shipped |
| `mejepa_witness_compress` | Force Merkle compression of segments > 90 days | `up_to_offset` | shipped |
| `mejepa_observe_shift` | Manually trigger observe() over a specific shift_id | `shift_id`, `run_id` | shipped |
| `mejepa_subscriber_status` | Shift subscriber lag + latency p50/p99 + last panic | — | shipped |
| `mejepa_capture_audit` | Bind outer-loop SWE-bench attempt to corpus | `run_id`, `attempt` | shipped |
| `mejepa_eval_run` | Force holdout-eval; per-(category × language) correlation | `dataset_id?`, `model_id?` | shipped |
| `mejepa_ship_gate_status` | Per-cell ≥0.95 ship-gate status with shrinkage | — | shipped |
| `mejepa_active_learning_queue` | High-priority queue contents | `top_n?` | shipped |
| `mejepa_eval_build_graph` | Rebuild patch-similarity graph for cross-task transfer | `patch_ids?` | shipped |
| `mejepa_audit_pairwise_mi` | Weekly pairwise-MI audit with CSV readback | — | shipped |

---

## 2. Futurebuild ME-JEPA additions

| Tool | Behavior | Phase |
|---|---|---|
| `mejepa_record_agent_feedback` | Agent records `SurpriseEvent { kind, severity, agent_explanation, actual_outcome }` bound to `prediction_id`; rate-limited 100/hour/agent | A (`TASK-PREDICT-002`) |
| `mejepa_predict_what_if` | Hypothetical prediction without applying the edit | B (`TASK-PREDICT-003`) |
| `mejepa_explain_prediction` | Per-prediction saliency + contributing signals + counterfactual minimum edit | B (`TASK-PREDICT-004`) |
| `mejepa_promote_approval` | Operator approval for Catastrophic A/B/C promotions | E (`TASK-HEAL-006`) |
| `mejepa_daemon_status` | Aggregator wrapping subscriber + heal + quota + vram | F (`TASK-OBS-003`) |
| `mejepa_inspect_prediction` | Full provenance trace for a `prediction_id` | F (`TASK-OBS-009`) |
| `mejepa_pause_predictions` | Suspend prediction emission for `duration-mins` | F (incident response) |

---

## 3. Core memory + search tools

| Tool | Behavior |
|---|---|
| `store_memory` | Store content with TeleologicalFingerprint (13 embedders) |
| `store_memories` | Batch up to 64 memories |
| `search_graph` | Multi-space semantic search across E1–E14 with temporal post-processing; strategies: `e1_only` / `multi_space` (default) / `pipeline` |
| `get_memetic_status` | Memory store health metrics |
| `trigger_consolidation` | Merge redundant fingerprints |
| `merge_concepts` | Merge two memories |
| `forget_concept` | Permanently remove a memory |
| `boost_importance` | Increase importance |

---

## 4. Topic + divergence tools

| Tool | Behavior |
|---|---|
| `get_topic_portfolio` | Active topics with profiles + stability |
| `get_topic_stability` | Variance over time |
| `detect_topics` | Force HDBSCAN clustering (threshold ≥ 2.5) |
| `get_divergence_alerts` | Topics where embedders disagree significantly |

---

## 5. File watcher tools

| Tool | Behavior |
|---|---|
| `list_watched_files` | Monitored file paths |
| `get_file_watcher_stats` | Health: inode limit, last-scan, queue depth |
| `delete_file_content` | Remove file's embeddings (privacy) |
| `reconcile_files` | Audit watcher state vs filesystem; repair drift |

---

## 6. Sequence / temporal / graph tools

| Tool | Embedder | Behavior |
|---|---|---|
| `get_conversation_context` | E4 | Per-session memory chain |
| `get_session_timeline` | E4 | All memories in a session ordered |
| `traverse_memory_chain` | E1 + graph | Walk linked-memory graph |
| `compare_session_states` | n/a | Diff two session snapshots |
| `search_connections` | E1 + graph | Multi-hop semantic graph |
| `get_graph_path` | E1 + graph | Shortest semantic path |

---

## 7. Per-embedder search tools

| Tool | Embedder | Behavior |
|---|---|---|
| `search_by_keywords` | E6 sparse | BM25 keyword search |
| `search_code` | E7 | Code-pattern + identifier search |
| `search_robust` | E1 + E9 (HDC) | Blind-spot: what E1 misses (typos, char variations) |
| `search_recent` | E2/E3 | E1 + temporal recency |
| `search_periodic` | E3/E4 | Daily/weekly/monthly patterns |
| `search_by_tokens` | E12 ColBERT | Character-precision matching |
| `search_by_expansion` | E13 SPLADE | Synonym/morphological expansion |
| `search_by_embedder` | any | Use single embedder as primary perspective |
| `search_cross_embedder_anomalies` | any | Find memories with high in one / low in another |

E5 + E11 retired: `search_causes`, `search_effects`, `get_causal_chain`, `search_causal_relationships`, `extract_entities`, `search_by_entities`, `infer_relationship`, `find_related_entities`, `validate_knowledge`, `get_entity_graph` all **disabled / fail-closed**.

---

## 8. Graph linking tools (K-NN)

| Tool | Behavior |
|---|---|
| `get_memory_neighbors` | K-nearest in E1 HNSW |
| `get_typed_edges` | Edges classified by embedder agreement (direction-aware per BUG-1 fix) |
| `traverse_graph` | BFS/DFS over K-NN graph |
| `get_unified_neighbors` | Weighted RRF fusion across all embedders (E2/E3/E4 excluded per AP-60; toggleable post-E2-fix) |

---

## 9. Learning + UTL learner-state tools

### 9.1 Learning-as-UTL (`CF_LEARNING_EVENTS`)

| Tool | Behavior |
|---|---|
| `record_learning_event` | Persist `LearningEvent { before, after, context }` |
| `list_learning_events` | List by UUID |
| `get_learning_event` | Fetch by UUID |
| `count_learning_events` | Count rows |
| `list_learning_signal_embedders` | List deterministic signal embedders |
| `compute_learning_signals` | Compute features + embeddings from inline transition |
| `embed_learning_event_signals` | Embed persisted event |
| `estimate_learning_outcome` | Predict outcome from nearest cases |
| `export_learner_training_dataset` | Compile to matrix-shaped dataset |
| `list_learner_training_datasets`, `get_learner_training_dataset`, `count_learner_training_datasets` | Dataset queries |

### 9.2 UTL learner-state (E15–E21)

| Tool | Behavior |
|---|---|
| `register_learner` | Register learner profile in `CF_LEARNER_PROFILE` |
| `record_session_observation` | Persist observation + state vector |
| `compute_delta_s` | UTL Δ S surprise signal |
| `compute_delta_c` | UTL Δ C coherence |
| `compute_delta_e` | UTL Δ E embodied state |
| `compute_l` | `L = ΔS · ΔC · ΔE` |
| `get_learner_m` | M(t) trace |
| `next_review_for_trace` | Spaced retrieval timing |
| `list_learner_embedders` | E1–E21 + learner-state matrix |
| `preflight_learner_assets` | Inspect cached E15–E21 model assets |
| `count_learner_state`, `get_learner_state` | State queries |
| `record_learner_k_sleep` | Sleep-derived k(τ) row |
| `record_learner_retrieval` | Retrieval-practice row |
| `upsert_goal_centroid` | Expert goal centroid for skill |
| `get_goal_distance` | Compare session embedding to centroid |
| `compile_learner_constellation` | Regulated-state baseline |
| `resolve_learner_retrieval_policy` | State-conditioned weight profile |

---

## 10. Constellation + contrastive + typed-edge tools

| Tool | Behavior |
|---|---|
| `compile_constellation` | Compile per-embedder centroids + spread stats; persist to `CF_CONSTELLATIONS` |
| `list_constellations`, `get_constellation` | Queries |
| `score_against_constellation` | Candidate vs existing constellation |
| `derive_constellation` | Interpolation / add / subtract / antipole mining |
| `delete_constellation` | Delete |
| `mine_contrastive_pairs` | Cross-embedder anomaly pairs to `CF_CONTRASTIVE_PAIRS` |
| `list_contrastive_pairs`, `get_contrastive_pair`, `count_contrastive_pairs` | Queries |
| `export_typed_edges_corpus` | Export to `CF_TYPED_EDGE_RECORDS` (F1) |
| `derive_anomalies_from_edges` | Derive anomaly pairs (F2) |
| `list_typed_edge_records` | Direction-aware lookup (multi-edge-safe key) |

---

## 11. DynamicJEPA tools (45)

Prefix `dynamicjepa_` — full list:

- **Domain pack**: `register_domain_pack`, `list_domain_packs`, `get_domain_pack`
- **Events**: `ingest_event`, `run_adapter`, `list_instrument_readings`
- **Panels**: `materialize_panel`, `get_panel`
- **Bindings**: `create_binding`, `list_bindings`
- **Trajectories**: `compile_trajectories`, `get_trajectory`, `list_trajectories`
- **Datasets**: `compile_dataset`, `get_dataset_shard`, `inspect_dataset_row`
- **Training**: `train`, `get_training_run`, `get_artifact`
- **Inference**: `predict`, `plan`, `record_surprise`
- **Constellation**: `build_constellation`, `list_constellations`, `get_constellation`
- **Thresholds**: `calibrate_threshold`, `recalibrate_threshold`
- **Audit**: `compute_mc_ratio`, `audit_pairwise_mi`
- **Cross-domain**: `cross_domain_transfer`
- **Code-specific**: `build_semantic_index`, `validate_corpus_diversity`, `attribute_test_delta`, `compare_shadow_utility`
- **Getters**: `get_prediction`, `get_plan_trace`, `get_surprise`
- **CF inspection**: `inspect_counts`, `inspect_cf`
- **Reality (for attempts)**: `reality_for_attempt`

Plus 6 verification: `verify_counter_world`, `verify_gridworld`, `verify_career_taxonomy`, `research_smoke`, `hp_sweep`, `multiseed_aggregate`, `capture_reference`, `check_reference`, `check_release_aggregate_reference`.

---

## 12. Reality-loop / ccreality tools

### 12.1 Read-side reality artifacts (12)

| Tool | Behavior |
|---|---|
| `reality_latest_root` | Latest run_root directory for target |
| `reality_attempt_summary` | Summary of one attempt |
| `reality_official_report` | Structured JSON report (diagnostics + FSV) |
| `reality_problem_packet` | Problem statement + constraints + tests |
| `reality_signal` | Outcome signal with confidence |
| `dynamicjepa_reality_for_attempt` | DJ predictions / MC-ratio evidence |
| `reality_failure` | Failure root cause |
| `reality_trigger_decision` | Recorded should-run policy decision |
| `reality_harness_transitions` | List file edits recorded for attempt |
| `reality_compare_attempts` | Diff two attempts |
| `reality_audit_trail` | Full audit log for attempt |
| `reality_replay_artifact` | Replay recorded transition |
| `reality_query_ledger` | Query BlackBox ledger |
| `reality_shift_log` | Per-session shift log chronological |
| `reality_shift_compare_to_my_view` | Compare recorded shift state vs current filesystem; detect external drift |

### 12.2 Harness editing (6 — DEEP TREATMENT)

| Tool | Parameters | Behavior |
|---|---|---|
| `harness_open_window` | `path`, `start_line`, `end_line` | Read line-range window with `observed_sha256` for pre-edit verification |
| `harness_apply_line_window_edit` | `path`, `start_line`, `end_line`, `observed_sha256`, `replace`, `session_id`, `preedit_state_path?`, `after_source_path?` | Atomic edit: acquire EditLock per session → verify pre-hash → apply → capture post-state → append harness transition → witness chain. Returns `{ sha256_before, sha256_after, lines_added, lines_removed, git_diff_stat, harness_transition_path, witness_chain_segment }`. Errors: `CCREALITY_EDIT_LOCK_HELD`. |
| `harness_run_command` | `command`, `timeout_seconds?` (default 30), `env?` | Shell command in project root; captures stdout/stderr/exit_code/elapsed_ms. Blocking; kills on timeout. |
| `harness_git_diff` | `context_lines?` (default 3), `paths?` | Git diff for working tree; returns `{ diff_text, files_changed, lines_added, lines_removed }` |
| `harness_git_status` | — | Staged/unstaged/untracked + branch |
| `harness_verify_state` | `checks?` | Diagnostic: binary + ledger + runtime_root + target writability |

### 12.3 Optimizer recording (3)

| Tool | Behavior |
|---|---|
| `optimizer_record_decision` | Record should-run policy decision (`policy`, `should_run`, `reasons`); persists `decision-turn-NN.json` + ledger + witness chain + shift log |
| `optimizer_record_recommendation` | Record structured recommendation (Stop-hook-REQUIRED). Schema validated. |
| `optimizer_record_harness_transition` | Record file edit `(before_sha256, after_sha256, preedit_state, after_source, lines_added, lines_removed, git_diff_stat)` to `round-NN.json` + shift log + witness chain |

### 12.4 Optimizer analysis (8)

| Tool | Behavior |
|---|---|
| `optimizer_bandit_select` | Select action via epsilon-greedy or UCB |
| `optimizer_bandit_record_reward` | Record reward (0..1) for action |
| `optimizer_bandit_state` | Read all arm stats |
| `optimizer_recall_recommendations` | Read all recommendation records for run+attempt |
| `optimizer_compute_influence` | Influence score for each decision/recommendation on final outcome |
| `optimizer_witness_chain_verify` | Verify witness-chain.bin integrity + Merkle commitments |
| `optimizer_witness_chain_diff` | Diff segment between two offsets |
| `optimizer_witness_chain_repair_legacy` | One-time migration from pre-v2 format |

### 12.5 Retired tools (fail-closed)

| Tool | Error |
|---|---|
| `reality_run_attempt` | `CCREALITY_ENGINE_RETIRED` — engine binary deleted; do not respawn |

---

## 13. Autoresearch (12)

`experiment_registry_list`, `experiment_registry_get`, `experiment_registry_propose`, `experiment_registry_update_outcome`, `champion_state_get`, `champion_state_promote`, `attempts_history_query`, `attempts_query_reflexion`, `attempts_critique_summary`, `attempts_success_strategies`, `attempts_synthesize`.

---

## 14. Provenance (Phase P3, 3)

| Tool | Behavior |
|---|---|
| `get_audit_trail` | Audit log for memory or time range (from `CF_AUDIT_LOG`) |
| `get_merge_history` | Merge lineage (from `CF_MERGE_HISTORY`) |
| `get_provenance_chain` | Full provenance: source → embedding chain |

---

## 15. Admin

| Tool | Behavior |
|---|---|
| `daemon_status` | Active connections + model state + background tasks |
| `get_capability_matrix` | All embedders + learner-state slots + tool groups + source-of-truth CF names |
| `record_graph_learning_event` | Persist graph navigation outcome |
| `resolve_graph_learning_policy` | Learn edge ranking |
| `repair_causal_relationships` | Data hygiene on `CF_CAUSAL_RELATIONSHIPS` |

---

## 16. Rate limiting

| Tool | Limit | Reason |
|---|---|---|
| `mejepa_verify` | unbounded; CUDA queue serializes | predictor forward GPU-bound |
| `mejepa_predict_latest` | unbounded | pure RocksDB read |
| `mejepa_record_agent_feedback` | 100/hour/agent_id | anti-DoS |
| `mejepa_constellation_inspect` | unbounded | RocksDB read |

---

## 17. Cross-references

[`02_source_code_map.md §3.9`](02_source_code_map.md) (MCP server source layout), [`17_prediction_taxonomy_reference.md`](17_prediction_taxonomy_reference.md) (output schema for `mejepa_verify`), [`19_cli_reference.md`](19_cli_reference.md) (CLI wraps many of these).
