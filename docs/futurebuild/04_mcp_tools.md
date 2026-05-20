# 04 — MCP Tools Reference

⚠ **Synthetic-vector remediation (2026-05-19/20):** every MCP tool whose handler routes through the inference path (`mejepa_verify`, `mejepa_predict_latest`, `mejepa_predict_what_if`, `mejepa_search_latent_actions`, `mejepa_rank_candidates`, `mejepa_explain_prediction`, `mejepa_inspect_prediction`, `mejepa_replay_prediction`, `mejepa_reality_impact`, `mejepa_project_ingest`, `mejepa_project_report`, `mejepa_pathway_*`, `mejepa_record_mistake`, etc.) returned synthetic-input outputs before 2026-05-19 22:45 UTC. The Rust code path is now real (per `01_overview.md §1.7`); any persisted prediction rows produced before the fix are contaminated. Tools that read pure-substrate CFs (`mejepa_skill_to_code`, `mejepa_code_to_skill`, `mejepa_skill_set_query`, `mejepa_skill_coverage_audit`, `mejepa_chunk_as_star`, `mejepa_constellation_membership`, `mejepa_list_accepted_labels`, fingerprint surface, hygiene surface, daemon/heal status) are unaffected.

Source files: `crates/context-graph-mcp/src/tools/definitions/mod.rs`, `crates/context-graph-mcp/src/tools/names.rs`, `crates/context-graph-mcp/src/tools/definitions/mejepa.rs`, all sibling `definitions/*.rs`, `crates/context-graph-mcp/src/deprecation.rs`.

**Total tools registered** in `get_tool_definitions()`: **214** (test-enforced at `definitions/mod.rs:150`).

---

## 1. Registry Structure

| Function | Returns | Notes |
|---|---|---|
| `get_tool_definitions()` | `Vec<ToolDefinition>` (214 tools) | Default `--mode stdio` registry. Builds from 28 submodules. |
| `get_reality_loop_tool_definitions()` | smaller subset | `--mode reality-loop` lightweight surface |

Tests enforce: exact total count + no duplicate names, exact per-submodule counts, retired `RETIRED_CGREALITY_TOOLS` not advertised in any registry. Retired callers receive `CCREALITY_ENGINE_RETIRED`.

---

## 2. Tool Count by Submodule

| Submodule | Count | Covers |
|---|---|---|
| `core` | 5 | `store_memory`, `store_memories`, `search_graph`, `boost_importance`, `inject_context` |
| `merge` / `curation` | 1 + 2 | `merge_concepts`, curation helpers |
| `topic` | 4 | `detect_topics`, `get_topic_portfolio`, `get_topic_stability`, `get_divergence_alerts` |
| `file_watcher` | 4 | `list_watched_files`, `get_file_watcher_stats`, `reconcile_files`, `delete_file_content` |
| `sequence` | 4 | E4 V_ordering: timeline, conversation context, compare sessions, traverse memory chain |
| `causal` | 4 | E5 retired (handlers fail closed for legacy callers) |
| `keyword` / `code` / `robustness` | 1 + 1 + 1 | `search_by_keywords` (E6), `search_code` (E7), `search_robust` (E9) |
| `graph` | 2 | Deterministic graph read/query |
| `entity` | 6 | E11 KEPLER/TransE: `extract_entities`, `infer_relationship`, `validate_knowledge`, etc. |
| `embedder` | 10 + 2 standalone | Per-embedder search + `search_by_tokens` (E12) + standalone E13 |
| **`mejepa`** | **67** | Predictor + heal + observe + ship-gate + chunk-skill linkage + mistake loop — see §3 |
| `temporal` | 2 | `search_recent` (E2), `search_periodic` (E3) |
| `graph_link` | 4 | K-NN navigation + typed edges |
| `maintenance` | 1 | `repair_causal_relationships` |
| `provenance` | 3 | `get_audit_trail`, `get_merge_history`, `get_provenance_chain` |
| `daemon` | 1 | `daemon_status` (multi-agent observability) |
| `training` | 4 | Training data export |
| `learning` | 14 | Event storage + signal embedders + matrix dataset export |
| `utl` | 18 | UTL learner-state surface |
| `constellation` | 6 | Compiler + derived anchors + read/score/delete |
| `contrastive` | 4 | Phase 3 miner + read surface |
| `typed_edges` | 3 | Typed-edge training surface |
| `dynamicjepa` | 39 | 5090jepa MCP surface |
| `capability` | 1 | `get_capability_matrix` |
| **Total** | **214** | |

---

## 3. The 67 `mejepa_*` Tools (the product surface)

Names live in `crates/context-graph-mcp/src/tools/names.rs`. Most are namespaced `mcp__cgreality__mejepa_*`; a small set use bare `mejepa_*`.

### 3.1 Predictor / Inference (Q1-Q5 RealityPrediction)

| Tool | Purpose | Side effects |
|---|---|---|
| `mejepa_verify` | System 2 verifier; evaluates LLM candidate patch through inference gate ladder. Fails closed unless `CONTEXTGRAPH_MEJEPA_ALLOW_DETERMINISTIC_VERIFY` set. | Read panel, run predictor; no write |
| `mejepa_predict_latest` | Read latest persisted `RealityPrediction` rows for an agent session | Read `CF_MEJEPA_LIVE_PREDICTIONS` |
| `mejepa_predict_what_if` | Counterfactual prediction for proposed input change | Read-only |
| `mejepa_search_latent_actions` | Best-of-K latent-action search (debug/training-time only) | Read-only |
| `mejepa_rank_candidates` | Counterfactual repair ranking | Read-only |
| `mejepa_explain_prediction` | Per-head feature attributions + matched fingerprint + closest exemplars + violating embedders + **per-slot OOD reasons** | Read-only |
| `mejepa_inspect_prediction` | Raw prediction row dump; exposes `RealityPrediction.per_slot_ood_reasons` byte-replayed | Read-only |
| `mejepa_replay_prediction` | Replay past prediction against current model + calibration | Read-only |
| `mejepa_reality_impact` | Q5 replay: compare predicted consequences to durable session shift log; returns confirmed/missed/not-yet-observed/surprise rows | Writes `CF_MEJEPA_REALITY_IMPACT` with sync + readback |

### 3.2 Mistake-driven loop (#406 / #523, shipped 2026-05-20)

FSV: `/zfs/archive/contextgraph/fsv/task-py-g-112b-mistake-loop-mcp-tools-fsv/run-20260520T113125Z/`, sha256 `9a9e50b6…`.

| Tool | Purpose | Side effects |
|---|---|---|
| `mejepa_record_mistake` | Land `MistakeLogRow` + `ReplayBufferRow` for a binary prediction that disagreed with reality; apply exact-panel online-head update | Writes `CF_MEJEPA_MISTAKE_LOG`, `CF_MEJEPA_REPLAY_BUFFER`, `CF_MEJEPA_ONLINE_HEAD_STATE`; rejects non-binary verdicts, target-side oracle labels as live inputs, row/context mismatches, invalid panel keys, missing CFs |
| `mejepa_mistake_history` | List mistake rows; optionally joined with replay rows and skill lifecycle audit | Read-only |
| `mejepa_mistake_loop_status` | Aggregate: exact panel online-head state, repeat metrics, latest mistake, source-of-truth CF counts. Target: `mistake_repeat_rate ≤ 0.05`. | Read-only |

### 3.3 Skill↔code linkage (#417 read surface, shipped 2026-05-19/20)

Read-only navigation over training substrate; no new prediction surface. Binary-leaf constraint preserved at prediction boundaries.

| Tool | Purpose |
|---|---|
| `mejepa_skill_to_code` | Forward: given `skillId`, return member chunks with source bytes/provenance |
| `mejepa_code_to_skill` | Reverse: given `chunkId`, return every skill/higher-ability membership |
| `mejepa_skill_set_query` | Boolean set query: chunks that have all `mustHave` skills AND none of `mustNotHave` |
| `mejepa_skill_coverage_audit` | Coverage-gap audit: partition chunks with zero, one, or multiple memberships |
| `mejepa_chunk_as_star` | Slot-routed constellation card for a chunk; preserves per-slot labels and does not flatten vectors |
| `mejepa_constellation_membership` | Level-0/Level-1/Level-2/higher-ability membership view for a chunk |
| `mejepa_skill_impact` | Refactoring blast-radius traversal: chunk → skills → co-member chunks |
| `mejepa_skill_graph_inspect` | Skill co-membership graph readback derived from persisted chunk-skill rows |
| `mejepa_skill_conflict_graph` | Zero-cooccurrence mutually-exclusive candidate pairs over persisted memberships |
| `mejepa_skill_browse` | Operator browse over the Level-2 skill catalog with membership counts and source key samples |

### 3.4 Reality Compiler (per-project ingest)

| Tool | Purpose | Side effects |
|---|---|---|
| `mejepa_project_ingest` | Scan operator-permitted Python project tree, materialize per-project cache, persist per-file `RealityPrediction` rows | Writes `manifest.json`, project-local CF rows under `/zfs/archive/contextgraph/projects/<project_id>/` |
| `mejepa_project_report` | Render durable `report.json` + `report.md`, persist `CF_MEJEPA_PROJECT_REPORTS` | Writes report files + CF |

### 3.5 Live capture + observe (agent feedback loop)

| Tool | Purpose |
|---|---|
| `mejepa_observe_shift` | Receive Edit/test/oracle shift event from a hook → enqueue for shift-subscriber |
| `mejepa_subscriber_status` | Subscriber latency + watermark + throughput |
| `mejepa_capture_audit` | Append-only audit of a capture event |
| `mejepa_record_agent_feedback` | Receive `Confirmed`/`Refuted` agent feedback for a `prediction_id` → `CF_MEJEPA_AGENT_FEEDBACK` + active-learning queue |
| `mejepa_pause_predictions` | Pause prediction serving |

### 3.6 Heal / self-optimization / operator override

| Tool | Purpose |
|---|---|
| `mejepa_heal_status` | Heal-scheduler state + drift window + Fisher snapshot age |
| `mejepa_daemon_status` | Multi-agent observability; daemon liveness + active sessions |
| `mejepa_operator_override_prediction` | Operator override of a `RealityPrediction` |
| `mejepa_operator_contributions` | Operator contribution events |
| `mejepa_promote_approval` | First-promotion + catastrophic-class operator approval |
| `mejepa_rollback_to` | Rollback active model pointer |

### 3.7 Hygiene

| Tool | Purpose |
|---|---|
| `mejepa_gc_run` | Trigger GC pass; reports rows touched per CF |
| `mejepa_quota_status` | Per-CF quota state |
| `mejepa_witness_compress` | Witness-chain Merkle compression batch |

### 3.8 Evaluation / Ship-gate dashboard

| Tool | Purpose |
|---|---|
| `mejepa_eval_run` | Run holdout eval; writes `CF_MEJEPA_EVAL_REPORTS` |
| `mejepa_eval_build_graph` | Build per-cell precision/recall + failure-mode-class graph |
| `mejepa_ship_gate_status` | 4-window stability; per-cell green/yellow/red |
| `mejepa_weekly_eval_dashboard` | Phase F weekly dashboard from `CF_MEJEPA_EVAL_REPORTS` + `exports/weekly/<date>/` |
| `mejepa_compression_progress` | CPΦ readback from `CF_MEJEPA_TRAIN_CERTS` |
| `mejepa_bootstrap_status` | Cold-start + baseline checkpoint status |

### 3.9 Active learning + audit

| Tool | Purpose |
|---|---|
| `mejepa_active_learning_queue` | Read/filter active-learning queue (optionally ranked by curiosity) |
| `mejepa_audit_reward_signals` | Reward-signal completeness audit; `CF_MEJEPA_SIGNAL_DROP_LOG` readback |
| `mejepa_audit_pairwise_mi` | Pairwise MI audit over instrument readings |
| `mejepa_constellation_inspect` | Per-cell constellation centroids / thresholds / freshness |
| `mejepa_mincut_panel` | Stoer-Wagner mincut on embedder/constellation graph |

### 3.10 Fingerprint catalog

| Tool | Purpose |
|---|---|
| `mejepa_fingerprint_list` / `mejepa_fingerprint_inspect` | List all canonical fingerprints / inspect one |
| `mejepa_fingerprint_classify` | Classify candidate against existing fingerprints |
| `mejepa_fingerprint_suggest_new` | Suggest novel fingerprint shapes from misclassifications |
| `mejepa_fingerprint_label` / `mejepa_fingerprint_promote_canonical` | Operator label + promote to canonical |
| `mejepa_fingerprint_recalibrate` | Per-fingerprint conformal recalibration |
| `mejepa_fingerprint_catalog_stability` | EWC/CBP stability readback |

### 3.11 Self-designed embedders + instrument proposals

| Tool | Purpose |
|---|---|
| `mejepa_propose_instrument` / `mejepa_promote_instrument_proposal` | Propose new frozen instrument / promote to active (gated) |
| `mejepa_pending_embedder_proposals` / `mejepa_pending_embedder_approvals` | Queue of proposed embedder shapes / operator approval surface |
| `mejepa_check_bedrock_consistency` | Bedrock-consistency verifier |
| `mejepa_library_foundationality` | Library-level foundationality |

### 3.12 Lightweight helpers (`mejepa_*` without `mcp__cgreality__` prefix)

| Tool | Purpose |
|---|---|
| `mejepa_load_learner_state` | Load serialized learner state |
| `mejepa_routing_lookup` | Embedder routing lookup |
| `mejepa_vram_budget_report` | VRAM budget + per-embedder resident bytes |

---

## 4. Other Tool Domains (summary)

| Domain | Count | Key tools |
|---|---|---|
| Core memory + curation + topic | 12 | `store_memory`, `search_graph`, `boost_importance`, `inject_context`, `merge_concepts`, `forget_concept`, `detect_topics` |
| Embedder search | 10 + 2 | Per-embedder search surfaces with weight profiles, `compare_embedder_views`, `search_cross_embedder_anomalies`, `get_embedder_clusters` |
| Entity (E11 KEPLER/TransE) | 6 | `extract_entities`, `infer_relationship`, `validate_knowledge` |
| Graph + sequence | 10 | E4 sequence tools + K-NN + typed edges + deterministic graph |
| Provenance + maintenance + file watcher | 8 | `get_audit_trail`, `repair_causal_relationships`, `list_watched_files` |
| UTL learner-state | 18 | `register_learner`, `get_learner_state`, `record_learning_event` |
| Constellation + Contrastive + Typed-edges | 13 | `compile_constellation`, `derive_constellation`, `mine_contrastive_pairs` |
| DynamicJEPA | 39 | `dynamicjepa_predict`, `dynamicjepa_plan`, `dynamicjepa_train`, `dynamicjepa_record_surprise` |

---

## 5. Tool Definition Schema

`ToolDefinition` (`crates/context-graph-mcp/src/tools/types.rs`):

```rust
pub struct ToolDefinition {
    pub name: String,        // matches one of pub const in tool_names::
    pub description: String, // human-readable
    pub input_schema: Value, // JSON Schema with "type": "object", required, properties
}
```

Every schema sets `additionalProperties: false`. Numeric params use `"type": "integer"` with explicit min/max. Strings use `pattern` for IDs (e.g. `^[A-Za-z0-9_.-]+$` for `projectId`).

Response format: tools return JSON. Errors carry `code: SCREAMING_SNAKE_CASE` matching the originating Rust `Error::code()`.

---

## 6. Server Modes

| Mode | Tools advertised | Transport | Purpose |
|---|---|---|---|
| `--mode stdio` (default) | 214 (`get_tool_definitions`) | JSON-RPC over stdio | Production agent surface |
| `--mode reality-loop` | smaller subset | JSON-RPC over stdio | Post-pivot Claude-Code-outer-loop surface |
| HTTP transport | same as configured mode | HTTP | Multi-agent deployments |

Reality-loop subset contains at least `reality_attempt_summary`, `dynamicjepa_reality_for_attempt`, `optimizer_record_recommendation`, `optimizer_compute_influence`. Does NOT contain `search_graph`, `get_memory_neighbors`, `get_audit_trail`.

---

## 7. Side Effects Summary

Tools either:
1. **Read-only** — most predict/inspect/status/list tools.
2. **Write durable CF row** — observe, record_feedback, record_mistake, project_ingest, project_report, eval_run, gc_run, witness_compress, operator_override, fingerprint_label, fingerprint_promote, propose_instrument.
3. **Spawn external process** — `mejepa_eval_run` may invoke `swebench.harness.run_evaluation` (Docker); `project_ingest` runs AST chunker.
4. **Heal scheduler trigger** — `mejepa_rollback_to`, `mejepa_promote_approval`.

No tool sends HTTP from action paths. No tool writes outside `/zfs/archive/contextgraph/` for production durable state.

---

## 8. Q4 Tool Deprecation

Per `05_predictor_pipeline.md §16`, the following remain registered but are **read-only-historical** (no writes):

| Tool | Status |
|---|---|
| `mejepa_predict_latest` `q4.*` fields | Read-only; new predictions return empty Q4 objects |
| `mejepa_explain_prediction` `q4_evidence` section | Read-only; historical rows visible |
| `mejepa_inspect_prediction` `q4.*` | Read-only |
| Q4 producer / calibration tools | Disabled by default; gated behind `--allow-deprecated-q4` for forensic re-runs |

No tool deletion (would break stored agent prompts); behaviour change only.

---

## 9. Planned MCP Tools

### 9.1 Failure-mode labeling MCP wrappers (#417 remaining)

| Tool (planned) | Purpose |
|---|---|
| `mejepa_list_failure_modes` | List operator-approved vocabulary + Level-1/2/3 catalog |
| `mejepa_label_failure_cluster` | Operator-name a Level-1 group |
| `mejepa_explain_named_failure_mode` | Named failure mode + top-K closest exemplars + slot trace |
| `mejepa_failure_mode_coverage_audit` | Holdout coverage audit |
| `mejepa_list_accepted_labels` | Operator surface over the 491-row accepted registry |
| `mejepa_label_compiler_status` | Manifest + report from latest `python-auto-label-discovery` run |

### 9.2 Cross-panel triangulation (#405)

| Tool (planned) | Purpose |
|---|---|
| `mejepa_cross_panel_score` | Trigger Panel B evaluation for a `prediction_id`; per-panel verdicts + agreement metric + `CROSS_PANEL_GOODHART_DETECTED` flag |
| `mejepa_cross_panel_dashboard` | Aggregate dashboard for the current ship-gate window |

### 9.3 Pathway exploration (#416)

Strict binary-leaf rule: any non-Q1/Q2/Q5 leaf → `PATHWAY_AMBIGUOUS_LEAF_REJECTED`.

| Tool (planned) | Purpose |
|---|---|
| `mejepa_pathway_surface` | Top-K (default 5) most-probable pathways for a candidate patch |
| `mejepa_pathway_inspect` | Full leaf chain + per-leaf probabilities + conformal intervals + closest historical exemplar |
| `mejepa_pathway_record_choice` | Record operator's chosen pathway (idempotent); feeds #335 reverse-prediction when ready |
| `mejepa_pathway_history` | Audit over `CF_MEJEPA_OPERATOR_PATHWAY_CHOICES` |

### 9.4 Remaining #417 skill linkage

| Tool (planned) | Purpose |
|---|---|
| `mejepa_cross_repo_skill_transfer` | Run Level-0 compiler over new project; match against existing skill signatures |

### 9.5 Information-physics inspection (#511 / #512 / #514)

| Tool (planned) | Purpose |
|---|---|
| `mejepa_entropy_training_status` | #511 — current λ for `L_entropy`, latent entropy in bits, holdout Brier impact |
| `mejepa_panel_ecc_inspect` | #512 — panel ECC syndrome, recovered slots, uncorrectable detection |
| `mejepa_cold_load_status` | #514 — last cold-load drift; promotion-time baseline snapshot |

### 9.6 Post-0.95 creativity-engine inspection (#516-#520)

| Tool (planned) | Purpose |
|---|---|
| `mejepa_embedder_proposal_audit` | #516 — embedder proposals with novelty/utility decisions |
| `mejepa_embedder_demand_inspect` | #517 — mistake-driven embedder demands with residual signatures |
| `mejepa_embedder_retirement_audit` | #518 — retired embedders with dominating-combination evidence |
| `mejepa_domain_routing_status` | #519 — domain registry + universal vs extension classification |
| `mejepa_consolidation_status` | #520 — consolidation audit history + storage savings |

CLI mutation:

| CLI command | Purpose |
|---|---|
| `context-graph-cli mejepa retire-embedder <id>` | Manual operator override (#518); writes audit row with operator attribution |

When these land, the tool count moves accordingly. Q4 tool deprecation does not change the count.

---

## 10. What This Doc Does NOT Cover

- Full JSON schema for every parameter — read `definitions/*.rs::definitions()`; each has a literal `json!({...})` schema.
- Per-tool handler implementations — see `crates/context-graph-mcp/src/handlers/`.
- Response body shapes — derive from source plus the `RealityPrediction` / `ProjectReport` / etc. types in `crates/context-graph-mejepa/src/types.rs`.
- HTTP transport configuration — see `crates/context-graph-mcp/src/server/http_transport.rs`.
