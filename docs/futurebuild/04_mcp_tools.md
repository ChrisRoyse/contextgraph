# 04 — MCP Tools Reference

**Last refreshed:** 2026-05-18.

Source files covered: `crates/context-graph-mcp/src/tools/definitions/mod.rs`, `crates/context-graph-mcp/src/tools/names.rs`, `crates/context-graph-mcp/src/tools/definitions/mejepa.rs`, all sibling `definitions/*.rs`, `crates/context-graph-mcp/src/deprecation.rs`.

**Total tools registered** in `get_tool_definitions()`: **196** (test-enforced at `definitions/mod.rs:150`).

---

## 1. Registry Structure

| Function | Returns | Notes |
|---|---|---|
| `get_tool_definitions()` | `Vec<ToolDefinition>` (196 tools) | Default `--mode stdio` registry. Builds from 28 submodules. |
| `get_reality_loop_tool_definitions()` | smaller subset | `--mode reality-loop` lightweight surface (post-pivot) |
| `is_reality_loop_mode()` | bool | Toggles inclusion of `reality::definitions()` |

Tests enforce:
- `test_total_tool_count_and_no_duplicates` — 196 total, no duplicate names, all have description+schema.
- `test_submodule_counts` — exact per-submodule counts (table below).
- `retired_cgreality_tools_are_not_advertised_by_tool_registries` — `RETIRED_CGREALITY_TOOLS` from `deprecation.rs` must NOT appear in any registry. Retired callers receive `CCREALITY_ENGINE_RETIRED`.

---

## 2. Tool Count by Submodule

| Submodule | Count | What it covers |
|---|---|---|
| `capability` | 1 | Compact map of embedders + CFs + tools (`get_capability_matrix`) |
| `core` | 5 | `store_memory`, `store_memories`, `search_graph`, `boost_importance`, `inject_context` (merged) |
| `merge` | 1 | `merge_concepts` |
| `curation` | 2 | `forget_concept` + curation helpers |
| `topic` | 4 | `detect_topics`, `get_topic_portfolio`, `get_topic_stability`, `get_divergence_alerts` |
| `file_watcher` | 4 | `list_watched_files`, `get_file_watcher_stats`, `reconcile_files`, `delete_file_content` |
| `sequence` | 4 | E4 V_ordering: `get_session_timeline`, `get_conversation_context`, `compare_session_states`, `traverse_memory_chain` |
| `causal` | 4 | E5 retired surface (handlers fail closed for legacy callers) |
| `causal_discovery` | 0 | Retired local-model surface |
| `keyword` | 1 | `search_by_keywords` (E6 sparse) |
| `code` | 1 | `search_code` (E7 1536-d Qodo) |
| `graph` | 2 | Deterministic graph read/query |
| `robustness` | 1 | `search_robust` (E9 typo-tolerant) |
| `entity` | 6 | E11 KEPLER/TransE: `extract_entities`, `infer_relationship`, `validate_knowledge`, `search_by_entities`, `find_related_entities`, `get_entity_graph` |
| `embedder` | 10 + 2 standalone | Per-embedder search surfaces (`search_by_embedder`, embedder-cluster, compare-views, etc.) plus `search_by_tokens` (E12) + standalone E13 |
| **`mejepa`** | **49** | Phase 4 inference + Phase 5 heal + Phase 6 hygiene + Phase 8 eval + Phase F/G/EK runbook — see §3 |
| `temporal` | 2 | `search_recent` (E2), `search_periodic` (E3) |
| `graph_link` | 4 | K-NN navigation + typed edges (`get_unified_neighbors`, `get_typed_edges`, `list_typed_edge_records`, `record_graph_learning_event`) |
| `maintenance` | 1 | `repair_causal_relationships` (data repair / cleanup) |
| `provenance` | 3 | `get_audit_trail`, `get_merge_history`, `get_provenance_chain` |
| `daemon` | 1 | `daemon_status` (multi-agent observability) |
| `training` | 4 | Training data export to `CF_TRAINING_RECORDS` family |
| `learning` | 14 | Event storage + signal embedders + graph learning + matrix dataset export |
| `utl` | 18 | UTL learner-state surface (measurement substrate + retrieval policy) |
| `constellation` | 6 | Phase 2 compiler + derived anchors + read/score/delete |
| `contrastive` | 4 | Phase 3 miner + read surface |
| `typed_edges` | 3 | Phase 4 typed-edge training (F1/F2/F4 + list) |
| `dynamicjepa` | 39 | 5090jepa MCP surface (Phase 6) |
| `reality` (only in reality-loop mode) | n/a | post-pivot reality-loop subset |
| **Total** | **196** | |

---

## 3. The 49 `mejepa_*` Tools (the product surface for AI agents)

All names live in `crates/context-graph-mcp/src/tools/names.rs:96-203`. Most are namespaced `mcp__cgreality__mejepa_*`; a small set use bare `mejepa_*`.

### 3.1 Predictor / Inference (Q1-Q5 RealityPrediction)

| Tool | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_verify` | `MEJEPA_VERIFY` | System 2 verifier: evaluate a System 1 LLM candidate patch through Phase 4 inference gate ladder. Fails closed unless `CONTEXTGRAPH_MEJEPA_ALLOW_DETERMINISTIC_VERIFY` set. | `patch: object`, `context: object`, `includeProvenance: bool=false` | Reads panel, runs predictor; no durable write |
| `mcp__cgreality__mejepa_predict_latest` | `MEJEPA_PREDICT_LATEST` | Read latest persisted `RealityPrediction` rows for an agent session | `sessionId` | Read-only: `CF_MEJEPA_LIVE_PREDICTIONS` |
| `mcp__cgreality__mejepa_predict_what_if` | `MEJEPA_PREDICT_WHAT_IF` | Counterfactual prediction for a proposed input change | candidate panel deltas | Read-only |
| `mcp__cgreality__mejepa_search_latent_actions` | `MEJEPA_SEARCH_LATENT_ACTIONS` | TASK-FP-103 best-of-K latent-action search | candidate seeds, K | Read-only |
| `mcp__cgreality__mejepa_rank_candidates` | `MEJEPA_RANK_CANDIDATES` | TASK-FP-105 counterfactual repair ranking | candidates list | Read-only |
| `mcp__cgreality__mejepa_explain_prediction` | `MEJEPA_EXPLAIN_PREDICTION` | Return per-head feature attributions + matched fingerprint + closest exemplars + violating embedders for a `prediction_id` | `predictionId` | Read-only |
| `mcp__cgreality__mejepa_inspect_prediction` | `MEJEPA_INSPECT_PREDICTION` | Raw prediction row dump | `predictionId` | Read-only |
| `mcp__cgreality__mejepa_replay_prediction` | `MEJEPA_REPLAY_PREDICTION` | Replay a past prediction against current model + calibration | `predictionId` | Read-only |

### 3.2 Reality Compiler (per-project ingest)

| Tool | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_project_ingest` | `MEJEPA_PROJECT_INGEST` | Scan an operator-permitted Python project file or directory tree, materialize per-project cache under `/zfs/archive/contextgraph/projects/<project_id>/`, route through AST chunker, persist per-file `RealityPrediction` rows | `repoPath`, `projectId` (regex `^[A-Za-z0-9_.-]+$`, max 96), `mode: Full|Incremental` (default Full), `scope: SourceOnly|SourceAndTests|All` (default SourceOnly), `overwrite: bool` (required for Full re-ingest), `changedPaths: []` (optional for Incremental from a watcher/hook) | Writes `manifest.json`, project-local CF rows, predictions; cache lives in `projects/<project_id>/cache/` |
| `mcp__cgreality__mejepa_project_report` | `MEJEPA_PROJECT_REPORT` | Read project-local `CF_MEJEPA_LIVE_PREDICTIONS`, render durable `report.json` + `report.md`, persist `CF_MEJEPA_PROJECT_REPORTS` | `projectId` (regex pattern same; rejects `..`), `section: "predictions"`, `offset: int` | Writes `report.json`, `report.md`, `CF_MEJEPA_PROJECT_REPORTS` |

### 3.3 Live capture + observe (closing the agent feedback loop)

| Tool | Const | Purpose |
|---|---|---|
| `mcp__cgreality__mejepa_observe_shift` | `MEJEPA_OBSERVE_SHIFT` | Receive an Edit/test/oracle shift event from a hook → enqueue for shift-subscriber |
| `mcp__cgreality__mejepa_subscriber_status` | `MEJEPA_SUBSCRIBER_STATUS` | Subscriber latency + watermark + throughput |
| `mcp__cgreality__mejepa_capture_audit` | `MEJEPA_CAPTURE_AUDIT` | Append-only audit of a capture event |
| `mcp__cgreality__mejepa_record_agent_feedback` | `MEJEPA_RECORD_AGENT_FEEDBACK` | Receive `Confirmed`/`Refuted`/etc. agent feedback for a `prediction_id` → `CF_MEJEPA_AGENT_FEEDBACK` + active-learning queue |
| `mcp__cgreality__mejepa_pause_predictions` | `MEJEPA_PAUSE_PREDICTIONS` | Pause prediction serving (Phase F pause-state) |

### 3.4 Heal / self-optimization / operator override

| Tool | Const | Purpose |
|---|---|---|
| `mcp__cgreality__mejepa_heal_status` | `MEJEPA_HEAL_STATUS` | Heal-scheduler state + drift window + Fisher snapshot age |
| `mcp__cgreality__mejepa_daemon_status` | `MEJEPA_DAEMON_STATUS` | Multi-agent observability; daemon liveness + active sessions |
| `mcp__cgreality__mejepa_operator_override_prediction` | `MEJEPA_OPERATOR_OVERRIDE_PREDICTION` | REQ-PREDICT-12 operator override of a `RealityPrediction` |
| `mcp__cgreality__mejepa_operator_contributions` | `MEJEPA_OPERATOR_CONTRIBUTIONS` | TASK-EK-014 operator contribution events |
| `mcp__cgreality__mejepa_promote_approval` | `MEJEPA_PROMOTE_APPROVAL` | TASK-EK-018H first-promotion + catastrophic-class operator approval |
| `mcp__cgreality__mejepa_rollback_to` | `MEJEPA_ROLLBACK_TO` | Rollback active model pointer to a previous checkpoint |

### 3.5 Hygiene

| Tool | Const | Purpose |
|---|---|---|
| `mcp__cgreality__mejepa_gc_run` | `MEJEPA_GC_RUN` | Trigger GC pass; reports rows touched per CF |
| `mcp__cgreality__mejepa_quota_status` | `MEJEPA_QUOTA_STATUS` | Per-CF quota state (from `CF_MEJEPA_QUOTA_STATE`) |
| `mcp__cgreality__mejepa_witness_compress` | `MEJEPA_WITNESS_COMPRESS` | Witness-chain Merkle compression batch |

### 3.6 Evaluation / Ship-gate dashboard

| Tool | Const | Purpose |
|---|---|---|
| `mcp__cgreality__mejepa_eval_run` | `MEJEPA_EVAL_RUN` | Run a holdout eval; writes `CF_MEJEPA_EVAL_REPORTS` |
| `mcp__cgreality__mejepa_eval_build_graph` | `MEJEPA_EVAL_BUILD_GRAPH` | Build the per-cell precision/recall + failure-mode-class graph |
| `mcp__cgreality__mejepa_ship_gate_status` | `MEJEPA_SHIP_GATE_STATUS` | TASK-G-006 ship-gate stability across 4 rolling windows; per-cell green/yellow/red |
| `mcp__cgreality__mejepa_weekly_eval_dashboard` | `MEJEPA_WEEKLY_EVAL_DASHBOARD` | Phase F weekly dashboard from `CF_MEJEPA_EVAL_REPORTS` + `exports/weekly/<date>/` |
| `mcp__cgreality__mejepa_compression_progress` | `MEJEPA_COMPRESSION_PROGRESS` | TASK-EK-001 CPΦ readback from `CF_MEJEPA_TRAIN_CERTS` |
| `mcp__cgreality__mejepa_bootstrap_status` | `MEJEPA_BOOTSTRAP_STATUS` | Cold-start + baseline checkpoint status |

### 3.7 Active learning + audit

| Tool | Const | Purpose |
|---|---|---|
| `mcp__cgreality__mejepa_active_learning_queue` | `MEJEPA_ACTIVE_LEARNING_QUEUE` | Read/filter active-learning queue (optionally rank by curiosity per TASK-EK-006) |
| `mcp__cgreality__mejepa_audit_reward_signals` | `MEJEPA_AUDIT_REWARD_SIGNALS` | TASK-RWD-315 reward-signal completeness audit; fail-closed `CF_MEJEPA_SIGNAL_DROP_LOG` readback |
| `mcp__cgreality__mejepa_audit_pairwise_mi` | `MEJEPA_AUDIT_PAIRWISE_MI` | TASK-PY-G-007 pairwise mutual-information audit over instrument readings |
| `mcp__cgreality__mejepa_constellation_inspect` | `MEJEPA_CONSTELLATION_INSPECT` | Per-cell constellation centroids / thresholds / freshness |
| `mcp__cgreality__mejepa_mincut_panel` | `MEJEPA_MINCUT_PANEL` | TASK-EK-003 Stoer-Wagner mincut on embedder/constellation graph |

### 3.8 Fingerprint catalog (TASK-FP-006 — 7 tools)

| Tool | Const | Purpose |
|---|---|---|
| `mcp__cgreality__mejepa_fingerprint_list` | `MEJEPA_FINGERPRINT_LIST` | List all canonical fingerprints |
| `mcp__cgreality__mejepa_fingerprint_inspect` | `MEJEPA_FINGERPRINT_INSPECT` | Per-fingerprint stats + ship-gate windows + Fisher / dormancy |
| `mcp__cgreality__mejepa_fingerprint_classify` | `MEJEPA_FINGERPRINT_CLASSIFY` | Classify a new candidate against existing fingerprints |
| `mcp__cgreality__mejepa_fingerprint_suggest_new` | `MEJEPA_FINGERPRINT_SUGGEST_NEW` | Suggest novel fingerprint shapes from accumulated misclassifications |
| `mcp__cgreality__mejepa_fingerprint_label` | `MEJEPA_FINGERPRINT_LABEL` | Operator label for a candidate fingerprint |
| `mcp__cgreality__mejepa_fingerprint_promote_canonical` | `MEJEPA_FINGERPRINT_PROMOTE_CANONICAL` | Promote a candidate to canonical (TASK-FP-007 ship-gate-gated) |
| `mcp__cgreality__mejepa_fingerprint_recalibrate` | `MEJEPA_FINGERPRINT_RECALIBRATE` | Trigger per-fingerprint conformal recalibration |
| `mcp__cgreality__mejepa_fingerprint_catalog_stability` | `MEJEPA_FINGERPRINT_CATALOG_STABILITY` | TASK-FP-008 EWC/CBP stability readback |

### 3.9 Self-designed embedders + instrument proposals (TASK-EK-018, TASK-EK-002)

| Tool | Const | Purpose |
|---|---|---|
| `mcp__cgreality__mejepa_propose_instrument` | `MEJEPA_PROPOSE_INSTRUMENT` | TASK-EK-002 propose a new frozen instrument from absence-detection |
| `mcp__cgreality__mejepa_promote_instrument_proposal` | `MEJEPA_PROMOTE_INSTRUMENT_PROPOSAL` | Promote a proposal to active (gated) |
| `mcp__cgreality__mejepa_pending_embedder_proposals` | `MEJEPA_PENDING_EMBEDDER_PROPOSALS` | TASK-EK-018B queue of proposed embedder shapes |
| `mcp__cgreality__mejepa_pending_embedder_approvals` | `MEJEPA_PENDING_EMBEDDER_APPROVALS` | TASK-EK-018H operator approval surface |
| `mcp__cgreality__mejepa_check_bedrock_consistency` | `MEJEPA_CHECK_BEDROCK_CONSISTENCY` | TASK-EK-016 bedrock-consistency verifier |
| `mcp__cgreality__mejepa_library_foundationality` | `MEJEPA_LIBRARY_FOUNDATIONALITY` | TASK-EK-017 library-level foundationality |

### 3.10 Lightweight / namespaced helpers (`mejepa_*` without `mcp__cgreality__` prefix)

| Tool | Const | Purpose |
|---|---|---|
| `mejepa_load_learner_state` | `MEJEPA_LOAD_LEARNER_STATE` | Load a serialized learner state |
| `mejepa_routing_lookup` | `MEJEPA_ROUTING_LOOKUP` | Embedder routing lookup table |
| `mejepa_vram_budget_report` | `MEJEPA_VRAM_BUDGET_REPORT` | VRAM budget + per-embedder resident bytes |

---

## 4. Other Tool Domains (summary; full schemas in source)

### 4.1 Core memory + curation + topic (12 tools)

Operator-facing memory store/retrieve. `core::definitions()` (5): `store_memory`, `store_memories` (batch), `search_graph`, `boost_importance`, plus `inject_context` merged into store. `merge_concepts`, `forget_concept`, curation helpers. Topic: `detect_topics`, `get_topic_portfolio`, `get_topic_stability`, `get_divergence_alerts`.

### 4.2 Embedder search (10 + 2 standalone)

Per-embedder search surfaces with weight profiles. E5 retired (4 tools that fail closed for legacy callers in `causal::definitions()`). E12 ColBERT MaxSim via `search_by_tokens`. E13 SPLADE via standalone. `compare_embedder_views`, `search_cross_embedder_anomalies`, `get_embedder_clusters`, etc.

### 4.3 Entity (E11 KEPLER/TransE — 6 tools)

`extract_entities`, `infer_relationship`, `validate_knowledge`, `search_by_entities`, `find_related_entities`, `get_entity_graph`.

### 4.4 Graph + sequence (10 tools)

Sequence (E4): `get_session_timeline`, `get_conversation_context`, `compare_session_states`, `traverse_memory_chain`. Graph link: `get_unified_neighbors`, `get_typed_edges`, `list_typed_edge_records`, `record_graph_learning_event`. Deterministic graph: `traverse_graph`, `get_graph_path`. Causal: `search_causes`, `search_effects`, `search_connections`, `search_causal_relationships` (E5 retired surface).

### 4.5 Provenance + maintenance + file watcher (8 tools)

`get_audit_trail`, `get_merge_history`, `get_provenance_chain` (Phase P3). `repair_causal_relationships` (data cleanup). `list_watched_files`, `get_file_watcher_stats`, `reconcile_files`, `delete_file_content`.

### 4.6 UTL learner-state (18 tools) + Learning (14 tools)

UTL: register_learner, count_learner_state, get_learner_state, get_learner_M, get_learner_training_dataset, list_learner_training_datasets, export_learner_training_dataset, count_learner_training_datasets, list_learner_embedders, count_learning_events, list_learning_events, get_learning_event, record_learning_event, embed_learning_event_signals, list_learning_signal_embedders, resolve_learner_retrieval_policy, record_learner_retrieval, record_learner_k_sleep, record_session_observation. Learning: training corpus / typed-edges corpus / contrastive pair mining / record_graph_learning_event / etc.

### 4.7 Constellation (6 tools) + Contrastive (4 tools) + Typed-edges (3 tools)

Phase 2 compiler + derived anchors + read/score/delete (`compile_constellation`, `derive_constellation`, `compile_learner_constellation`, `score_against_constellation`, `get_constellation`, `delete_constellation`, `list_constellations`). Contrastive: `mine_contrastive_pairs`, `count_contrastive_pairs`, `list_contrastive_pairs`, `get_contrastive_pair`. Typed-edges: F1/F2/F4 surfaces + list.

### 4.8 DynamicJEPA (39 tools)

5090jepa MCP surface. `dynamicjepa_predict`, `dynamicjepa_plan`, `dynamicjepa_train`, `dynamicjepa_record_surprise`, `dynamicjepa_get_*`, `dynamicjepa_list_*`, `dynamicjepa_register_domain_pack`, `dynamicjepa_run_adapter`, etc. (Separate dataplane CFs.)

### 4.9 Reality-loop mode tools (`get_reality_loop_tool_definitions`)

Smaller subset for `--mode reality-loop`. Contains at least:
- `reality_attempt_summary`
- `dynamicjepa_reality_for_attempt`
- `optimizer_record_recommendation`
- `optimizer_compute_influence`

Does NOT contain: `search_graph`, `get_memory_neighbors`, `get_audit_trail` (enforced by `test_reality_loop_lightweight_registry_only_lists_callable_tools`).

### 4.10 Retired tools (fail-closed)

`crates/context-graph-mcp/src/deprecation.rs` lists `RETIRED_CGREALITY_TOOLS`. Test `retired_cgreality_tools_are_not_advertised_by_tool_registries` blocks any registry from advertising them. Callers attempting them receive `CCREALITY_ENGINE_RETIRED`.

---

## 5. Tool Definition Schema

`ToolDefinition` (`crates/context-graph-mcp/src/tools/types.rs`):

```rust
pub struct ToolDefinition {
    pub name: String,        // matches one of the `pub const` in tool_names::
    pub description: String, // human-readable; what + when to use
    pub input_schema: Value, // JSON Schema with "type": "object", required, properties
}
```

Every schema sets `additionalProperties: false` (enforced for at least 24 schemas per audit #9 in memory). Numeric params use `"type": "integer"` with explicit min/max. Strings use `pattern` for IDs (e.g. `^[A-Za-z0-9_.-]+$` for `projectId`).

Response format: tools return JSON. Errors carry `code: SCREAMING_SNAKE_CASE` matching the originating Rust `Error::code()` (see `01_overview.md §8`).

---

## 6. Server Modes

| Mode | Tools advertised | Transport | Purpose |
|---|---|---|---|
| `--mode stdio` (default) | 196 (`get_tool_definitions`) | JSON-RPC over stdio | Production agent surface |
| `--mode reality-loop` | smaller subset (`get_reality_loop_tool_definitions`) | JSON-RPC over stdio | Post-pivot Claude-Code-outer-loop surface; lightweight subset |
| HTTP transport | same as configured mode | HTTP | Multi-agent deployments (see `server/http_transport.rs`) |

---

## 7. Side Effects Summary

Tools either:
1. **Read-only** — most predict/inspect/status/list tools. No CF writes.
2. **Write durable CF row** — observe, record_feedback, project_ingest, project_report, eval_run, gc_run, witness_compress, operator_override, fingerprint_label, fingerprint_promote, propose_instrument, etc.
3. **Spawn external process** — `mejepa_eval_run` may invoke `swebench.harness.run_evaluation` (Docker); `project_ingest` runs AST chunker.
4. **Heal scheduler trigger** — `mejepa_rollback_to`, `mejepa_promote_approval`.

No tool sends HTTP from action paths (CLAUDE.md §6.8). No tool writes outside `/zfs/archive/contextgraph/` for production durable state (CLAUDE.md §6.5, GitHub #363).

---

## 8. What This Doc Does NOT Cover

- Full JSON schema for every parameter of every tool — read the source `definitions/*.rs::definitions()` functions; each has a literal `json!({...})` schema.
- Per-tool handler implementations — see `crates/context-graph-mcp/src/handlers/`.
- Response body shapes — derive from the source plus the `RealityPrediction` / `ProjectReport` / etc. types in `crates/context-graph-mejepa/src/types.rs`.
- HTTP transport configuration — see `crates/context-graph-mcp/src/server/http_transport.rs`.
- Adapter shims to legacy ccreality tool names — see `crates/context-graph-mcp/src/adapters/` (transport-only; same handlers).
