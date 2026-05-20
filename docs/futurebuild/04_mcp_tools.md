# 04 — MCP Tools Reference

**Last refreshed:** 2026-05-19.

Source files covered: `crates/context-graph-mcp/src/tools/definitions/mod.rs`, `crates/context-graph-mcp/src/tools/names.rs`, `crates/context-graph-mcp/src/tools/definitions/mejepa.rs`, all sibling `definitions/*.rs`, `crates/context-graph-mcp/src/deprecation.rs`.

**Total tools registered** in `get_tool_definitions()`: **201** (test-enforced at `definitions/mod.rs:150`).

---

## 1. Registry Structure

| Function | Returns | Notes |
|---|---|---|
| `get_tool_definitions()` | `Vec<ToolDefinition>` (201 tools) | Default `--mode stdio` registry. Builds from 28 submodules. |
| `get_reality_loop_tool_definitions()` | smaller subset | `--mode reality-loop` lightweight surface (post-pivot) |
| `is_reality_loop_mode()` | bool | Toggles inclusion of `reality::definitions()` |

Tests enforce:
- `test_total_tool_count_and_no_duplicates` — 201 total, no duplicate names, all have description+schema.
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
| **`mejepa`** | **54** | Phase 4 inference + Phase 5 heal + Phase 6 hygiene + Phase 8 eval + Phase F/G/EK runbook + first #417 chunk-skill linkage surface — see §3 |
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
| **Total** | **201** | |

---

## 3. The 54 `mejepa_*` Tools (the product surface for AI agents)

All names live in `crates/context-graph-mcp/src/tools/names.rs:96-203`. Most are namespaced `mcp__cgreality__mejepa_*`; a small set use bare `mejepa_*`.

### 3.1 Predictor / Inference (Q1-Q5 RealityPrediction)

| Tool | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_verify` | `MEJEPA_VERIFY` | System 2 verifier: evaluate a System 1 LLM candidate patch through Phase 4 inference gate ladder. Fails closed unless `CONTEXTGRAPH_MEJEPA_ALLOW_DETERMINISTIC_VERIFY` set. | `patch: object`, `context: object`, `includeProvenance: bool=false` | Reads panel, runs predictor; no durable write |
| `mcp__cgreality__mejepa_predict_latest` | `MEJEPA_PREDICT_LATEST` | Read latest persisted `RealityPrediction` rows for an agent session | `sessionId` | Read-only: `CF_MEJEPA_LIVE_PREDICTIONS` |
| `mcp__cgreality__mejepa_predict_what_if` | `MEJEPA_PREDICT_WHAT_IF` | Counterfactual prediction for a proposed input change | candidate panel deltas | Read-only |
| `mcp__cgreality__mejepa_search_latent_actions` | `MEJEPA_SEARCH_LATENT_ACTIONS` | TASK-FP-103 best-of-K latent-action search | candidate seeds, K | Read-only |
| `mcp__cgreality__mejepa_rank_candidates` | `MEJEPA_RANK_CANDIDATES` | TASK-FP-105 counterfactual repair ranking | candidates list | Read-only |
| `mcp__cgreality__mejepa_explain_prediction` | `MEJEPA_EXPLAIN_PREDICTION` | Return per-head feature attributions + matched fingerprint + closest exemplars + violating embedders + **per-slot OOD reasons (TASK-PY-G-089 / #389)** for a `prediction_id` | `predictionId` | Read-only |
| `mcp__cgreality__mejepa_inspect_prediction` | `MEJEPA_INSPECT_PREDICTION` | Raw prediction row dump; exposes `RealityPrediction.per_slot_ood_reasons` byte-replayed from `CF_MEJEPA_LIVE_PREDICTIONS` | `predictionId` | Read-only |
| `mcp__cgreality__mejepa_replay_prediction` | `MEJEPA_REPLAY_PREDICTION` | Replay a past prediction against current model + calibration | `predictionId` | Read-only |
| `mcp__cgreality__mejepa_reality_impact` | `MEJEPA_REALITY_IMPACT` | TASK-PY-G-045 Q5 replay: compare a persisted prediction's expected consequences to the durable session shift log and return confirmed/missed/not-yet-observed/surprise rows | `predictionId`, `runtimeRoot`, `replayWindowMs=3600000` | Writes `CF_MEJEPA_REALITY_IMPACT` with sync + readback |

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

### 3.11 Skill↔code linkage first slice (TASK-PY-G-120 / #417)

These tools expose the shipped #414 chunk-skill substrate as read-only navigation evidence. They do not add a prediction head and they preserve the binary-leaf boundary at prediction time.

| Tool | Const | Purpose | Side effects |
|---|---|---|---|
| `mcp__cgreality__mejepa_skill_to_code` | `MEJEPA_SKILL_TO_CODE` | Forward linkage: given `skillId`, return member chunks with source bytes/provenance when an AST chunk JSONL is supplied | Read-only over `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS`, `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP`, AST chunk JSONL |
| `mcp__cgreality__mejepa_code_to_skill` | `MEJEPA_CODE_TO_SKILL` | Reverse linkage: given `chunkId`, return every skill/higher-ability membership for that chunk and optional `codeStateKey` | Read-only over `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` |
| `mcp__cgreality__mejepa_skill_set_query` | `MEJEPA_SKILL_SET_QUERY` | Boolean set query over memberships: chunks that have all `mustHave` skills and none of `mustNotHave` | Read-only over `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` |
| `mcp__cgreality__mejepa_skill_coverage_audit` | `MEJEPA_SKILL_COVERAGE_AUDIT` | Coverage-gap audit: partition chunks with zero, one, or multiple skill memberships | Read-only over `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` plus optional AST chunk JSONL |

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
| `--mode stdio` (default) | 201 (`get_tool_definitions`) | JSON-RPC over stdio | Production agent surface |
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

---

## 9. Planned New MCP Tools (Q1/Q2/Q5 binary contract + labeling + cross-panel + mistake loop)

Planned MCP surfaces supporting the 2026-05-19 binary doctrine (`01_overview.md §1.1`, `05_predictor_pipeline.md §0`). Tracked under epic issues #404 / #405 / #406. All planned tools are `mejepa_*` namespaced and registered through `definitions/mejepa.rs`.

### 9.1 Failure-mode labeling tools — CLI SHIPPED via #413; MCP wrappers planned via #404

**Shipped CLI surface (deterministic Level-0 compiler, 2026-05-19):**

| CLI command | Status | Purpose |
|---|---|---|
| `python-auto-label-discovery` (registered via `scripts/swebench_lite_ops/cli.py`) | ✅ SHIPPED 2026-05-19 (#413) | Run the full deterministic label compiler against the aiwonder corpus. Writes all 7 Level-0 JSONL artifacts under `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/` plus the FSV at `/zfs/archive/contextgraph/fsv/task-py-g-117-auto-label-discovery-usefulness-fsv/`. Deterministic by `FORMULA_VERSION=deterministic_slot_label_compiler_v1`. |

**Planned MCP wrappers (`mejepa_*` namespace; on top of the shipped CLI) — open under #404 / TASK-PY-G-110:**

| Tool (planned) | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_list_failure_modes` | `MEJEPA_LIST_FAILURE_MODES` | List the operator-approved failure-mode vocabulary + level-1/2/3 catalog (built on top of the 491-row Level-0 accepted registry). Optional cell filter. | `cell_key?`, `level: 1\|2\|3` | Read-only: `CF_MEJEPA_FAILURE_MODE_*` + `accepted_label_registry.jsonl` |
| `mcp__cgreality__mejepa_label_failure_cluster` | `MEJEPA_LABEL_FAILURE_CLUSTER` | Operator-facing tool to name an aggregated Level-1 group of Level-0 labels. Top suggestions from `CF_MEJEPA_FAILURE_MODE_VOCABULARY` cosine match; operator can approve, rename, or merge with an existing skill. | `group_id`, `chosen_name`, `merge_into_group_id?` | Writes `CF_MEJEPA_FAILURE_MODE_LEVEL1` (operator_approved_name), `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS` |
| `mcp__cgreality__mejepa_explain_named_failure_mode` | `MEJEPA_EXPLAIN_NAMED_FAILURE_MODE` | Given a `prediction_id`, return the named failure mode + top-K closest exemplars + slot trace + vocabulary source + the Level-0 accepted label IDs that triggered the naming. Replaces the legacy `mejepa_explain_prediction` output for Fail rows. | `prediction_id`, `k: int=5` | Read-only: `CF_MEJEPA_NAMED_PREDICTIONS`, `CF_MEJEPA_PATCH_SIMILARITY_META`, `accepted_label_registry.jsonl` |
| `mcp__cgreality__mejepa_failure_mode_coverage_audit` | `MEJEPA_FAILURE_MODE_COVERAGE_AUDIT` | Read-only audit: for the 300×8 holdout, what fraction of Fail rows have an attached named failure mode? Used as gate evidence for #404's FSV. | (no params) | Read-only |
| `mcp__cgreality__mejepa_list_accepted_labels` | `MEJEPA_LIST_ACCEPTED_LABELS` | Read-only operator surface over the shipped 491-row accepted Level-0 registry. Filter by family / leakage status / minimum lift / partition. | `family?`, `min_lift?`, `leakage_status?: live_input\|target_side_only` | Read-only: `accepted_label_registry.jsonl` |
| `mcp__cgreality__mejepa_label_compiler_status` | `MEJEPA_LABEL_COMPILER_STATUS` | Surface the FSV manifest + report from the latest `python-auto-label-discovery` run (last formula version, row counts, sha256s). | (no params) | Read-only |

### 9.2 Cross-panel triangulation tools (#405 / TASK-PY-G-111)

| Tool (planned) | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_cross_panel_score` | `MEJEPA_CROSS_PANEL_SCORE` | Trigger Panel B evaluation for a specific `prediction_id`; returns per-panel verdicts + agreement metric + `CROSS_PANEL_GOODHART_DETECTED` flag if applicable. Synchronous (loads Panel B encoders for the call). | `prediction_id`, `force_reload_panel_b: bool=false` | Writes `CF_MEJEPA_PANEL_B_OBSERVATIONS`, `CF_MEJEPA_CROSS_PANEL_AGREEMENT` |
| `mcp__cgreality__mejepa_cross_panel_dashboard` | `MEJEPA_CROSS_PANEL_DASHBOARD` | Aggregate dashboard for the current ship-gate window: per-cell Panel-A vs Panel-B correlation, agreement delta, count of `CROSS_PANEL_GOODHART_DETECTED` rows. | `window_id?` | Read-only: `CF_MEJEPA_CROSS_PANEL_AGREEMENT`, `CF_MEJEPA_EVAL_REPORTS` |

### 9.3 Mistake-driven loop tools (#406 / TASK-PY-G-112)

| Tool (planned) | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_record_mistake` | `MEJEPA_RECORD_MISTAKE` | Land a `MistakeRow` for a `prediction_id` that disagreed with reality. Called by the PostToolUse capture, the shift-log replay, the operator override, and the oracle-eval pipeline. Idempotent on `prediction_id` + `ground_truth_source`. | `prediction_id`, `ground_truth_verdict: Pass\|Fail`, `ground_truth_source`, `slot_traces?` | Writes `CF_MEJEPA_MISTAKE_LOG`, boosts `CF_MEJEPA_REPLAY_BUFFER` 8×, writes `CF_MEJEPA_MISTAKE_REPLAY_PRIORITY` |
| `mcp__cgreality__mejepa_mistake_history` | `MEJEPA_MISTAKE_HISTORY` | Read-only: list mistakes for a `cell_key` or `named_failure_mode` in a window. Drives operator triage and the heal scheduler's per-cell attention. | `cell_key?`, `named_failure_mode?`, `since_ts_ns?`, `limit: int=100` | Read-only: `CF_MEJEPA_MISTAKE_LOG` |
| `mcp__cgreality__mejepa_mistake_loop_status` | `MEJEPA_MISTAKE_LOOP_STATUS` | Aggregate: rolling `mistake_repeat_rate` per `(cell_key × named_failure_mode)`, support-set size, last-update-ts. Target convergence: ≤ 0.05 repeat rate within window. | (no params) | Read-only: `CF_MEJEPA_MISTAKE_LOG`, `CF_MEJEPA_ONLINE_HEAD_STATE` |

### 9.4 Q4 tool deprecation

Per `05_predictor_pipeline.md §16`, the following existing tools remain registered but are reclassified as **read-only-historical** and no longer accept writes:

| Tool | Status after 2026-05-19 |
|---|---|
| `mcp__cgreality__mejepa_predict_latest` `q4.*` fields | Read-only; new predictions populate the field as empty objects |
| `mcp__cgreality__mejepa_explain_prediction` `q4_evidence` section | Read-only; historical rows still visible |
| `mcp__cgreality__mejepa_inspect_prediction` `q4.*` | Read-only |
| Q4-specific producers / calibration tools (if any operator-visible ones existed via `mejepa_eval_run` Q4 subcommand) | Disabled by default; gated behind `--allow-deprecated-q4` env var for forensic re-runs only |

No tool deletion (would break stored agent prompts referencing them); behaviour change only. Documented in `04_mcp_tools.md §9.4` + `01_overview.md §1.1`.

### 9.5 Tool-count update

Current count after the first #417 slice is **201**. When #404/#405/#406 land, the total tool count increases by the exact tools shipped in those slices and the `test_total_tool_count_and_no_duplicates` threshold must be updated in the same change. Q4 tool deprecation does not change the count (tools remain registered, just behaviour-changed).

### 9.6 Pathway exploration surface tools (#416 / TASK-PY-G-119, planned)

The butterfly-effect pathway tree (`docs/futurebuild/01_overview.md §1.5`, `05_predictor_pipeline.md §17bis`) — top-K most-probable future trajectories per candidate patch as chains of Q1/Q2/Q5 binary leaves only — adds four MCP tools. Strict binary-leaf rule applies: any non-Q1/Q2/Q5 leaf in a pathway request fails closed with `PATHWAY_AMBIGUOUS_LEAF_REJECTED`.

| Tool (planned) | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_pathway_surface` | `MEJEPA_PATHWAY_SURFACE` | Compute and return the top-K (default 5) most-probable pathways for a candidate patch. Each pathway is a chain of binary Q1/Q2/Q5 leaves with calibrated joint probability, closest historical exemplar from the 491-row Level-0 registry (#413), and named failure mode on any Fail leaf (#404). | `predictionId`, `k: int=5`, `beamWidth: int=32`, `maxDepth: int=6`, `pruningEpsilon: f32=0.01` | Writes `CF_MEJEPA_PATHWAY_TREES`, `CF_MEJEPA_SURFACED_PATHWAYS` |
| `mcp__cgreality__mejepa_pathway_inspect` | `MEJEPA_PATHWAY_INSPECT` | Return full leaf chain + per-leaf probabilities + conformal intervals + closest historical exemplar for one pathway. | `predictionId`, `pathwayId` | Read-only |
| `mcp__cgreality__mejepa_pathway_record_choice` | `MEJEPA_PATHWAY_RECORD_CHOICE` | Record the operator's chosen pathway (idempotent). Feeds #335 `mejepa_reverse_prediction` when it ships — the system then produces patch-shape constraints that satisfy the chosen pathway's binary leaves. | `predictionId`, `pathwayId`, `rationaleText?` | Writes `CF_MEJEPA_OPERATOR_PATHWAY_CHOICES` |
| `mcp__cgreality__mejepa_pathway_history` | `MEJEPA_PATHWAY_HISTORY` | Read-only audit over `CF_MEJEPA_OPERATOR_PATHWAY_CHOICES`. Filter by window/session/operator. | `windowId?`, `sessionId?`, `limit: int=100` | Read-only |

When #416 lands, the total tool count moves further by 4 additional pathway tools.

### 9.7 Skill↔code linkage capability surface tools (#417 / TASK-PY-G-120)

Exposes the already-shipped bidirectional chunk↔skill data (#413 Level-0 registry + #414 skill memberships and reverse index) through operator-facing MCP tools. Read-only navigation over training substrate; no new prediction surface, no Q4 reanimation, binary-leaf constraint at prediction boundaries preserved. See `01_overview.md §1.5bis` and #417 for the full unlocked capability set.

| Tool | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_skill_to_code` | `MEJEPA_SKILL_TO_CODE` | **Shipped first slice:** forward linkage from skill to member chunks with literal source bytes when `chunksJsonl` is supplied | `skillId`, `limit: int=20`, `chunksJsonl?`, `requireSourceText?` | Read-only over #414 skill CFs + AST chunks |
| `mcp__cgreality__mejepa_code_to_skill` | `MEJEPA_CODE_TO_SKILL` | **Shipped first slice:** reverse linkage from chunk to all skill memberships, optionally narrowed by code state | `chunkId`, `codeStateKey?`, `limit: int=20`, `chunksJsonl?`, `requireSourceText?` | Read-only over `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` |
| `mcp__cgreality__mejepa_skill_set_query` | `MEJEPA_SKILL_SET_QUERY` | **Shipped first slice:** Boolean set query over memberships: chunks that have skills `mustHave` AND NOT `mustNotHave` | `mustHave: [skillId]`, `mustNotHave: [skillId]?`, `limit: int=100` | Read-only |
| `mcp__cgreality__mejepa_skill_coverage_audit` | `MEJEPA_SKILL_COVERAGE_AUDIT` | **Shipped first slice:** coverage-gap audit over chunks with zero/one/multiple skill memberships | `chunksJsonl?`, `sampleLimit: int=100` | Read-only over membership CF + optional AST chunk JSONL |
| `mcp__cgreality__mejepa_skill_impact` | `MEJEPA_SKILL_IMPACT` | Refactoring blast-radius: chunk → skills → transitive co-member chunks, with TASK-EK-016 foundationality weighting | `chunkId`, `depth: int=2` | Read-only |
| `mcp__cgreality__mejepa_skill_graph_inspect` | `MEJEPA_SKILL_GRAPH_INSPECT` | Read the skill-of-skills graph (depends-on / composes-with / mutually-exclusive) | `skillId?` | Read-only over `CF_MEJEPA_SKILL_GRAPH` |
| `mcp__cgreality__mejepa_cross_repo_skill_transfer` | `MEJEPA_CROSS_REPO_SKILL_TRANSFER` | Run the deterministic Level-0 compiler over a new project's AST chunks, match against existing skill signatures, persist matches | `targetProjectId` | Writes `CF_MEJEPA_CROSS_REPO_SKILL_MATCHES` |
| `mcp__cgreality__mejepa_skill_conflict_graph` | `MEJEPA_SKILL_CONFLICT_GRAPH` | Read the mutually-exclusive-skill graph derived from zero-co-occurrence in the shipped 36,773 memberships | (no params) | Read-only |
| `mcp__cgreality__mejepa_skill_browse` | `MEJEPA_SKILL_BROWSE` | Operator-facing skill catalog browse: 128 skills + chunk counts + lifecycle audit | `filter?: {minSupport, minLift, status}` | Read-only |

The first #417 slice adds 4 tools and moves the current registry to **201**. Remaining #417 tools still planned: skill impact, skill graph inspect, cross-repo skill transfer, skill conflict graph, and skill browse.

### 9.7 Stars-as-Vectors navigation tools (#417 sub-capability, planned)

Source: `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md` Proposal D. Operator-facing tools that formalize the teleological-constellation primitive as a celestial-cartography surface over the already-shipped #413 + #414 substrate. Read-only; no new prediction head.

| Tool (planned) | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_chunk_as_star` | `MEJEPA_CHUNK_AS_STAR` | "Stellar parameter card" view of an AST chunk: per-slot vector summaries (norm, sparsity, occupancy), slot-specific calibration cell membership, closest accepted Level-0 labels from #413's 491-row registry, active skills from #414's 128 skills + 36,773 memberships, closest exemplars from `CF_MEJEPA_PATCH_SIMILARITY_META`. Output structurally analogous to a Gaia DR3 catalog row. | `chunkId`, `includeExemplars?: bool=true`, `exemplarLimit: int=5` | Read-only over `CF_MEJEPA_PANELS`, label CFs, skill CFs, exemplar CFs |
| `mcp__cgreality__mejepa_constellation_membership` | `MEJEPA_CONSTELLATION_MEMBERSHIP` | Named constellations the chunk participates in across all hierarchy levels: Level-0 accepted labels (from #413), Level-1 file-path constellations, Level-2 code-state constellations, Level-2.5 skills (from #414), Level-3+ higher abilities (planned via #418). | `chunkId`, `levels?: [0,1,2,2.5,3+]=all` | Read-only |

### 9.8 Information-physics planned tools (#511 / #512 / #513 / #514)

Source: `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md` Proposals A/B/C/E. Inspection surfaces over the planned info-physics CFs.

| Tool (planned) | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_entropy_training_status` | `MEJEPA_ENTROPY_TRAINING_STATUS` | #511 inspection: current λ for `L_entropy`, latent entropy in bits, holdout Brier impact, λ-auto-calibration history | `windowLimit: int=10` | Read-only over `CF_MEJEPA_TRAIN_CERT_ENTROPY` |
| `mcp__cgreality__mejepa_panel_ecc_inspect` | `MEJEPA_PANEL_ECC_INSPECT` | #512 inspection: panel ECC syndrome, recovered slots, uncorrectable detection | `predictionId` OR `panelId` | Read-only over `CF_MEJEPA_PANEL_ECC_SYNDROME` |
| `mcp__cgreality__mejepa_connectome_topology` | `MEJEPA_CONNECTOME_TOPOLOGY` | #513 inspection: per-cell rich-club coefficient, dominant 3-node motif, modularity Q, identified rich-club pair IDs | `cellId`, `windowIdx?` | Read-only over `CF_MEJEPA_CONNECTOME_TOPOLOGY_FEATURES` |
| `mcp__cgreality__mejepa_cold_load_status` | `MEJEPA_COLD_LOAD_STATUS` | #514 inspection: last cold-load drift (correlation Δ, per-cell distribution Δ), promotion-time baseline snapshot | `promotionId?` | Read-only over `CF_MEJEPA_COLD_LOAD_SNAPSHOTS` |

The four info-physics tools are inspection-only (no mutations). They expose the planned CFs from `03_storage_schema.md §9.4quart` once those CFs land.

### 9.9 Creativity-engine inspection tools (post-0.95, #516 / #517 / #518 / #519 / #520)

Source: `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md` + creativity-engine synthesis. Read-only inspection over the planned creativity-engine CFs (`03 §9.4quint`).

| Tool (planned) | Const | Purpose | Key params | Side effects |
|---|---|---|---|---|
| `mcp__cgreality__mejepa_embedder_proposal_audit` | `MEJEPA_EMBEDDER_PROPOSAL_AUDIT` | #516 — list embedder proposals with novelty/utility decisions and rejection reasons | `limit: int=50`, `filter?: {decision, embedder_id_prefix}` | Read-only over `CF_MEJEPA_EMBEDDER_PROPOSAL_AUDIT` |
| `mcp__cgreality__mejepa_embedder_demand_inspect` | `MEJEPA_EMBEDDER_DEMAND_INSPECT` | #517 — list current mistake-driven embedder demands with residual signatures and closest-inadequate slots | `limit: int=20`, `cellId?` | Read-only over `CF_MEJEPA_EMBEDDER_DEMAND` |
| `mcp__cgreality__mejepa_embedder_retirement_audit` | `MEJEPA_EMBEDDER_RETIREMENT_AUDIT` | #518 — list retired embedders with dominating-combination evidence and storage freed | `limit: int=50` | Read-only over `CF_MEJEPA_EMBEDDER_RETIREMENT_AUDIT` |
| `mcp__cgreality__mejepa_domain_routing_status` | `MEJEPA_DOMAIN_ROUTING_STATUS` | #519 — domain registry + per-embedder universal vs extension classification + per-chunk domain assignment counts | `domainId?` | Read-only over the three #519 CFs |
| `mcp__cgreality__mejepa_consolidation_status` | `MEJEPA_CONSOLIDATION_STATUS` | #520 — consolidation audit history + per-chunk consolidated-vs-raw breakdown + storage savings cumulative | `limit: int=20` | Read-only over `CF_MEJEPA_CONSOLIDATION_AUDIT` |

The CLI surface also gains one mutation:

| CLI command | Issue | Purpose |
|---|---|---|
| `context-graph-cli mejepa retire-embedder <id>` | #518 | Manual operator override to force-retire a specific embedder. Writes the retirement audit row with operator attribution; respects the grace period. |
